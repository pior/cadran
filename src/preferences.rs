use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, ProtocolObject};
use objc2::sel;
use objc2::{
    define_class, msg_send, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly,
    Message,
};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSComboBox, NSDragOperation, NSDraggingContext,
    NSDraggingDestination, NSDraggingInfo, NSDraggingItem, NSDraggingSession, NSDraggingSource,
    NSImage, NSPasteboard, NSStackView, NSStackViewDistribution, NSTextField,
    NSTextFieldBezelStyle, NSUserInterfaceLayoutOrientation, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{ns_string, NSInteger, NSNotification, NSObject, NSObjectProtocol, NSString};

use crate::search::{self, TimezoneSearch};
use crate::settings;
use crate::timezone::TimezoneEntry;

fn row_drag_type() -> Retained<NSString> {
    ns_string!("com.pior.clock.row").retain()
}

// -- ComboBoxDataSource: case-insensitive prefix completion --

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ComboDataSourceIvars]
    struct ComboDataSource;

    unsafe impl NSObjectProtocol for ComboDataSource {}

    impl ComboDataSource {
        #[unsafe(method(numberOfItemsInComboBox:))]
        fn number_of_items(&self, _combo_box: &NSComboBox) -> NSInteger {
            self.ivars().items.len() as NSInteger
        }

        #[unsafe(method(comboBox:objectValueForItemAtIndex:))]
        fn object_value_for_item(
            &self,
            _combo_box: &NSComboBox,
            index: NSInteger,
        ) -> *mut AnyObject {
            let items = &self.ivars().items;
            let idx = index as usize;
            if idx < items.len() {
                Retained::autorelease_return(NSString::from_str(&items[idx])) as *mut NSString
                    as *mut AnyObject
            } else {
                std::ptr::null_mut()
            }
        }

        #[unsafe(method(comboBox:completedString:))]
        fn completed_string(
            &self,
            _combo_box: &NSComboBox,
            string: &NSString,
        ) -> *mut NSString {
            let query = string.to_string().to_lowercase();
            match self
                .ivars()
                .items
                .iter()
                .find(|item| item.to_lowercase().starts_with(&query))
            {
                Some(item) => Retained::autorelease_return(NSString::from_str(item)),
                None => std::ptr::null_mut()
            }
        }
    }
);

struct ComboDataSourceIvars {
    items: Vec<String>,
}

impl ComboDataSource {
    fn new(mtm: MainThreadMarker, items: Vec<String>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ComboDataSourceIvars { items });
        unsafe { msg_send![super(this), init] }
    }
}

struct EntryRow {
    view: Retained<NSView>,
    label_field: Retained<NSTextField>,
    iana_combo: Retained<NSComboBox>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PrefsControllerIvars]
    pub struct PrefsController;

    unsafe impl NSObjectProtocol for PrefsController {}

    unsafe impl NSDraggingDestination for PrefsController {
        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, sender: &NSObject) -> NSDragOperation {
            let info: &ProtocolObject<dyn NSDraggingInfo> = unsafe { std::mem::transmute(sender) };
            let pboard = info.draggingPasteboard();
            let types = objc2_foundation::NSArray::from_retained_slice(&[row_drag_type()]);
            if pboard.availableTypeFromArray(&types).is_some() {
                NSDragOperation::Move
            } else {
                NSDragOperation::None
            }
        }

        #[unsafe(method(performDragOperation:))]
        fn perform_drag_operation(&self, sender: &NSObject) -> Bool {
            let info: &ProtocolObject<dyn NSDraggingInfo> = unsafe { std::mem::transmute(sender) };
            let pboard = info.draggingPasteboard();
            let point = info.draggingLocation();
            let view_point = self.ivars().rows_stack.convertPoint_fromView(point, None);

            if let Some(src_idx_str) = pboard.stringForType(&row_drag_type()) {
                let src_idx: usize = src_idx_str.to_string().parse().unwrap_or(0);
                self.do_reorder(src_idx, view_point);
                return Bool::YES;
            }
            Bool::NO
        }
    }

    impl PrefsController {
        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, _notification: &NSNotification) {
            self.do_save();
        }

        #[unsafe(method(controlTextDidEndEditing:))]
        fn control_text_did_end_editing(&self, _notification: &NSNotification) {
            self.do_canonicalize();
        }

        #[unsafe(method(comboBoxSelectionDidChange:))]
        fn combo_box_selection_did_change(&self, _notification: &NSNotification) {
            self.do_canonicalize();
        }

        #[unsafe(method(addEntry:))]
        unsafe fn add_entry(&self, _sender: &AnyObject) {
            let mtm = MainThreadMarker::from(self);
            self.do_add_entry(mtm);
        }

        #[unsafe(method(removeEntry:))]
        unsafe fn remove_entry(&self, sender: &NSButton) {
            let mtm = MainThreadMarker::from(self);
            self.do_remove_entry(mtm, sender);
        }

        #[unsafe(method(showWindow))]
        unsafe fn show_window_objc(&self) {
            self.show();
        }
    }
);

pub struct PrefsControllerIvars {
    window: Retained<NSWindow>,
    rows_stack: Retained<NSStackView>,
    rows: RefCell<Vec<EntryRow>>,
    combo_data_source: Retained<ComboDataSource>,
    on_save: Box<dyn Fn()>,
}

impl PrefsController {
    pub fn new(
        mtm: MainThreadMarker,
        entries: &[TimezoneEntry],
        on_save: Box<dyn Fn()>,
    ) -> Retained<Self> {
        let frame = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(560.0, 400.0));
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Resizable;

        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                frame,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        window.setTitle(ns_string!("Cadran Preferences"));
        window.center();
        unsafe { window.setReleasedWhenClosed(false) };

        let rows_stack = NSStackView::new(mtm);
        rows_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        rows_stack.setSpacing(8.0);

        let content_view = NSView::initWithFrame(NSView::alloc(mtm), frame);
        window.setContentView(Some(&content_view));

        let search = TimezoneSearch::new();
        let combo_items: Vec<String> =
            search.combo_items().into_iter().map(String::from).collect();
        let combo_data_source = ComboDataSource::new(mtm, combo_items);

        let rows = RefCell::new(Vec::new());
        let this = Self::alloc(mtm).set_ivars(PrefsControllerIvars {
            window,
            rows_stack,
            rows,
            combo_data_source,
            on_save,
        });
        let controller: Retained<Self> = unsafe { msg_send![super(this), init] };

        // Register for drag and drop
        unsafe {
            let types = objc2_foundation::NSArray::from_retained_slice(&[row_drag_type()]);
            let _: () = msg_send![&controller.ivars().rows_stack, registerForDraggedTypes: &*types];
        }

        for entry in entries {
            controller.do_add_entry_with(mtm, &entry.label, entry.iana_id());
        }

        let button_row = create_button_row(mtm, &controller);

        let outer_stack = NSStackView::new(mtm);
        outer_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        outer_stack.setSpacing(12.0);
        let insets = objc2_foundation::NSEdgeInsets {
            top: 16.0,
            left: 16.0,
            bottom: 16.0,
            right: 16.0,
        };
        outer_stack.setEdgeInsets(insets);
        outer_stack.setFrame(frame);

        outer_stack.addArrangedSubview(&controller.ivars().rows_stack);
        outer_stack.addArrangedSubview(&button_row);

        content_view.addSubview(&outer_stack);

        controller
    }

    pub fn show(&self) {
        self.ivars().window.makeKeyAndOrderFront(None);
        let mtm = MainThreadMarker::from(self);
        let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
        app.activate();
    }

    fn do_add_entry(&self, mtm: MainThreadMarker) {
        self.do_add_entry_with(mtm, "", "");
    }

    fn do_add_entry_with(&self, mtm: MainThreadMarker, label: &str, iana_id: &str) {
        let ivars = self.ivars();

        let row_stack = NSStackView::new(mtm);
        row_stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row_stack.setSpacing(8.0);
        row_stack.setDistribution(NSStackViewDistribution::Fill);

        // 1. Drag Handle
        let handle = DragHandle::new(mtm);
        handle.setStringValue(ns_string!("≡"));
        handle.setEditable(false);
        handle.setSelectable(false);
        handle.setBezeled(false);
        handle.setDrawsBackground(false);
        handle.setAlignment(objc2_app_kit::NSTextAlignment::Center);
        // Fixed width for handle
        unsafe {
            let constraint = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &handle,
                objc2_app_kit::NSLayoutAttribute::Width,
                objc2_app_kit::NSLayoutRelation::Equal,
                None,
                objc2_app_kit::NSLayoutAttribute::NotAnAttribute,
                1.0,
                20.0
            );
            let _: () = msg_send![&handle, addConstraint: &*constraint];
        }

        // 2. Fields
        let label_field = create_text_field(mtm, "Label", label);
        let iana_combo = create_timezone_combo(mtm, iana_id, &ivars.combo_data_source);

        let delegate: &AnyObject = self;
        unsafe {
            let _: () = msg_send![&label_field, setDelegate: delegate];
            let _: () = msg_send![&iana_combo, setDelegate: delegate];
        }

        // Fix layout: make both fields equal width
        unsafe {
            let constraint = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &iana_combo,
                objc2_app_kit::NSLayoutAttribute::Width,
                objc2_app_kit::NSLayoutRelation::Equal,
                Some(&label_field),
                objc2_app_kit::NSLayoutAttribute::Width,
                1.0,
                0.0
            );
            let _: () = msg_send![&iana_combo, addConstraint: &*constraint];
        }

        // 3. Delete Button
        let delete_btn = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("✕"),
                Some(delegate),
                Some(sel!(removeEntry:)),
                mtm,
            )
        };
        delete_btn.setBezelStyle(objc2_app_kit::NSBezelStyle::RegularSquare);
        delete_btn.setBordered(false);

        // Fixed width for delete button
        unsafe {
            let constraint = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &delete_btn,
                objc2_app_kit::NSLayoutAttribute::Width,
                objc2_app_kit::NSLayoutRelation::Equal,
                None,
                objc2_app_kit::NSLayoutAttribute::NotAnAttribute,
                1.0,
                24.0
            );
            let _: () = msg_send![&delete_btn, addConstraint: &*constraint];
        }

        row_stack.addArrangedSubview(&handle);
        row_stack.addArrangedSubview(&label_field);
        row_stack.addArrangedSubview(&iana_combo);
        row_stack.addArrangedSubview(&delete_btn);

        ivars.rows_stack.addArrangedSubview(&row_stack);
        ivars.rows.borrow_mut().push(EntryRow {
            view: Retained::into_super(row_stack),
            label_field,
            iana_combo,
        });
    }

    fn do_remove_entry(&self, _mtm: MainThreadMarker, sender: &NSButton) {
        let ivars = self.ivars();
        let mut rows = ivars.rows.borrow_mut();

        // Find which row contains this button
        let mut found_idx = None;
        for (idx, row) in rows.iter().enumerate() {
            if unsafe { row.view.isDescendantOf(&sender.superview().unwrap()) } {
                found_idx = Some(idx);
                break;
            }
        }

        if let Some(idx) = found_idx {
            let row = rows.remove(idx);
            ivars.rows_stack.removeArrangedSubview(&row.view);
            row.view.removeFromSuperview();
            self.do_save();
        }
    }

    fn do_reorder(&self, src_idx: usize, point: CGPoint) {
        let ivars = self.ivars();
        let mut rows = ivars.rows.borrow_mut();
        if src_idx >= rows.len() {
            return;
        }

        // Determine destination index based on drop point
        let mut dest_idx = 0;
        let subviews = ivars.rows_stack.arrangedSubviews();
        for i in 0..subviews.count() {
            let view: Retained<NSView> = subviews.objectAtIndex(i).downcast().unwrap();
            let frame = view.frame();
            if point.y > frame.origin.y + frame.size.height / 2.0 {
                dest_idx = i as usize;
                break;
            }
            dest_idx = (i + 1) as usize;
        }

        if dest_idx > rows.len() {
            dest_idx = rows.len();
        }

        if src_idx == dest_idx || (dest_idx > 0 && src_idx == dest_idx - 1) {
            return;
        }

        let row = rows.remove(src_idx);
        let final_dest = if dest_idx > src_idx { dest_idx - 1 } else { dest_idx };
        rows.insert(final_dest, row);

        // Update StackView
        let subviews = ivars.rows_stack.arrangedSubviews();
        for i in (0..subviews.count()).rev() {
            let view: Retained<NSView> = subviews.objectAtIndex(i).downcast().unwrap();
            ivars.rows_stack.removeArrangedSubview(&view);
        }

        for row in rows.iter() {
            ivars.rows_stack.addArrangedSubview(&row.view);
        }

        self.do_save();
    }

    fn do_canonicalize(&self) {
        let ivars = self.ivars();
        let rows = ivars.rows.borrow();

        for row in rows.iter() {
            let raw_value = row.iana_combo.stringValue().to_string();
            let iana_id = search::iana_id_from_display(&raw_value);

            if !iana_id.is_empty() && iana_id != raw_value {
                if let Some(entry) = TimezoneEntry::try_new("", iana_id) {
                    row.iana_combo
                        .setStringValue(&NSString::from_str(entry.iana_id()));
                }
            }
        }
        self.do_save();
    }

    fn do_save(&self) {
        let ivars = self.ivars();
        let rows = ivars.rows.borrow();
        let mut entries = Vec::new();

        for row in rows.iter() {
            let label = row.label_field.stringValue().to_string();
            let raw_value = row.iana_combo.stringValue().to_string();
            let iana_id = search::iana_id_from_display(&raw_value);

            if iana_id.is_empty() {
                continue;
            }
            if let Some(entry) = TimezoneEntry::try_new(&label, iana_id) {
                entries.push(entry);
            }
        }

        settings::save_entries(&entries);
        (ivars.on_save)();
    }
}

fn create_text_field(
    mtm: MainThreadMarker,
    placeholder: &str,
    value: &str,
) -> Retained<NSTextField> {
    let field = NSTextField::new(mtm);
    field.setPlaceholderString(Some(&NSString::from_str(placeholder)));
    field.setStringValue(&NSString::from_str(value));
    field.setEditable(true);
    field.setBezeled(true);
    field.setBezelStyle(NSTextFieldBezelStyle::RoundedBezel);
    field
}

fn create_timezone_combo(
    mtm: MainThreadMarker,
    value: &str,
    data_source: &ComboDataSource,
) -> Retained<NSComboBox> {
    let combo = NSComboBox::new(mtm);
    combo.setEditable(true);
    combo.setCompletes(true);
    combo.setUsesDataSource(true);
    let ds_obj = data_source as &AnyObject;
    unsafe { let _: () = msg_send![&combo, setDataSource: ds_obj]; }
    combo.setNumberOfVisibleItems(10);
    combo.setPlaceholderString(Some(ns_string!("Type to search...")));
    combo.setStringValue(&NSString::from_str(value));
    combo
}

fn create_button_row(mtm: MainThreadMarker, target: &PrefsController) -> Retained<NSStackView> {
    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setSpacing(8.0);

    let target_obj: &AnyObject = target;

    let add_btn = unsafe {
        NSButton::buttonWithTitle_target_action(
            ns_string!("+ Add"),
            Some(target_obj),
            Some(sel!(addEntry:)),
            mtm,
        )
    };

    row.addArrangedSubview(&add_btn);

    row
}

define_class!(
    #[unsafe(super(NSTextField))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DragHandleIvars]
    struct DragHandle;

    unsafe impl NSObjectProtocol for DragHandle {}

    unsafe impl NSDraggingSource for DragHandle {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn dragging_session_source_operation_mask(
            &self,
            _session: &NSDraggingSession,
            _context: NSDraggingContext,
        ) -> NSDragOperation {
            NSDragOperation::Move
        }
    }

    impl DragHandle {
        #[unsafe(method(mouseDown:))]
        unsafe fn mouse_down(&self, event: &objc2_app_kit::NSEvent) {
            let _mtm = MainThreadMarker::from(self);
            let view: &NSView = self;

            // Find our row index
            let mut row_idx = 0;
            if let Some(superview) = unsafe { view.superview() } {
                if let Some(stack) = unsafe { superview.superview() } {
                    let stack: Retained<NSStackView> = stack.downcast().unwrap();
                    let subviews = stack.arrangedSubviews();
                    for i in 0..subviews.count() {
                        if subviews.objectAtIndex(i).isEqual(Some(&superview)) {
                            row_idx = i;
                            break;
                        }
                    }
                }
            }

            let pboard = NSPasteboard::generalPasteboard();
            pboard.clearContents();
            let drag_type = row_drag_type();
            let pboard_writing = ProtocolObject::from_retained(drag_type);
            pboard.writeObjects(&objc2_foundation::NSArray::from_retained_slice(&[pboard_writing.clone()]));
            pboard.setString_forType(&NSString::from_str(&row_idx.to_string()), &row_drag_type());

            let drag_image = NSImage::initWithSize(NSImage::alloc(), CGSize::new(16.0, 16.0));

            let item = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), &*pboard_writing);
            unsafe { item.setDraggingFrame_contents(view.bounds(), Some(&drag_image)) };

            let items = objc2_foundation::NSArray::from_retained_slice(&[item]);
            let dragging_source: &ProtocolObject<dyn NSDraggingSource> = ProtocolObject::from_ref(self);
            unsafe {
                let _: () = msg_send![view, beginDraggingSessionWithItems: &*items, event: event, source: dragging_source];
            }
        }
    }
);

struct DragHandleIvars {}

impl DragHandle {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DragHandleIvars {});
        unsafe { msg_send![super(this), init] }
    }
}
