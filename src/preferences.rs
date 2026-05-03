use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, ProtocolObject};
use objc2::sel;
use objc2::{
    define_class, msg_send, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly,
    Message,
};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSButtonType, NSColor, NSComboBox, NSDragOperation,
    NSDraggingContext, NSDraggingDestination, NSDraggingInfo, NSDraggingItem, NSDraggingSession,
    NSDraggingSource, NSImage, NSLayoutAttribute, NSLayoutRelation, NSPasteboardItem,
    NSStackView, NSStackViewDistribution, NSTextField, NSTextFieldBezelStyle,
    NSUserInterfaceLayoutOrientation, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_app_kit::{NSControlStateValueOff, NSControlStateValueOn};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{ns_string, NSInteger, NSNotification, NSObject, NSObjectProtocol, NSString};

use crate::search::{self, TimezoneSearch};
use crate::settings;
use crate::timezone::TimezoneEntry;

fn row_drag_type() -> Retained<NSString> {
    ns_string!("com.pior.clock.row").retain()
}

// Row subview indices
const IDX_STAR: usize = 0;
const _IDX_HANDLE: usize = 1;
const IDX_LABEL: usize = 2;
const IDX_TIMEZONE: usize = 3;
const _IDX_DELETE: usize = 4;
const ROW_SUBVIEW_COUNT: usize = 5;

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

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PrefsControllerIvars]
    pub struct PrefsController;

    unsafe impl NSObjectProtocol for PrefsController {}

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

        #[unsafe(method(setFavorite:))]
        unsafe fn set_favorite(&self, sender: &NSButton) {
            self.do_set_favorite(sender);
        }

        #[unsafe(method(toggleLaunchAtLogin:))]
        unsafe fn toggle_launch_at_login(&self, sender: &NSButton) {
            self.do_toggle_launch_at_login(sender);
        }

        #[unsafe(method(showWindow))]
        unsafe fn show_window_objc(&self) {
            self.show();
        }

        #[unsafe(method(reordered))]
        fn reordered(&self) {
            self.do_save();
        }
    }
);

pub struct PrefsControllerIvars {
    window: Retained<NSWindow>,
    rows_stack: Retained<PrefsStackView>,
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

        let rows_stack = PrefsStackView::new(mtm);
        rows_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        rows_stack.setSpacing(8.0);
        rows_stack.setAlignment(NSLayoutAttribute::Width);
        rows_stack.setTranslatesAutoresizingMaskIntoConstraints(false);

        let search = TimezoneSearch::new();
        let combo_items: Vec<String> =
            search.combo_items().into_iter().map(String::from).collect();
        let combo_data_source = ComboDataSource::new(mtm, combo_items);

        let this = Self::alloc(mtm).set_ivars(PrefsControllerIvars {
            window: window.retain(),
            rows_stack: rows_stack.retain(),
            combo_data_source,
            on_save,
        });
        let controller: Retained<Self> = unsafe { msg_send![super(this), init] };
        rows_stack.ivars().controller.replace(Some(controller.retain()));

        let add_row = create_add_button_row(mtm, &controller);
        rows_stack.addArrangedSubview(&add_row);

        for entry in entries {
            controller.do_add_entry_with(mtm, &entry.label, entry.iana_id(), entry.favorite);
        }

        controller.update_tab_order();

        let launch_checkbox = create_launch_at_login_checkbox(mtm, &controller);
        launch_checkbox.setTranslatesAutoresizingMaskIntoConstraints(false);

        let content_view = window.contentView().unwrap();
        content_view.addSubview(&rows_stack);
        content_view.addSubview(&launch_checkbox);

        let padding = 16.0;
        unsafe {
            let leading = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*rows_stack, NSLayoutAttribute::Leading, NSLayoutRelation::Equal,
                Some(&*content_view), NSLayoutAttribute::Leading, 1.0, padding,
            );
            let trailing = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*rows_stack, NSLayoutAttribute::Trailing, NSLayoutRelation::Equal,
                Some(&*content_view), NSLayoutAttribute::Trailing, 1.0, -padding,
            );
            let top = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*rows_stack, NSLayoutAttribute::Top, NSLayoutRelation::Equal,
                Some(&*content_view), NSLayoutAttribute::Top, 1.0, padding,
            );
            let cb_leading = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*launch_checkbox, NSLayoutAttribute::Leading, NSLayoutRelation::Equal,
                Some(&*content_view), NSLayoutAttribute::Leading, 1.0, padding,
            );
            let cb_top = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*launch_checkbox, NSLayoutAttribute::Top, NSLayoutRelation::Equal,
                Some(&*rows_stack), NSLayoutAttribute::Bottom, 1.0, 16.0,
            );
            leading.setActive(true);
            trailing.setActive(true);
            top.setActive(true);
            cb_leading.setActive(true);
            cb_top.setActive(true);
        }

        controller
    }

    fn do_toggle_launch_at_login(&self, sender: &NSButton) {
        use objc2_service_management::{SMAppService, SMAppServiceStatus};

        let service = unsafe { SMAppService::mainAppService() };
        let enabled = sender.state() == NSControlStateValueOn;

        if enabled {
            if let Err(err) = unsafe { service.registerAndReturnError() } {
                eprintln!("Failed to enable launch at login: {err}");
                sender.setState(NSControlStateValueOff);
            }
        } else {
            if let Err(err) = unsafe { service.unregisterAndReturnError() } {
                eprintln!("Failed to disable launch at login: {err}");
                let status = unsafe { service.status() };
                if status == SMAppServiceStatus::Enabled {
                    sender.setState(NSControlStateValueOn);
                }
            }
        }
    }

    pub fn show(&self) {
        self.ivars().window.makeKeyAndOrderFront(None);
        let mtm = MainThreadMarker::from(self);
        let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
        app.activate();
    }

    fn do_add_entry(&self, mtm: MainThreadMarker) {
        self.do_add_entry_with(mtm, "", "", false);
    }

    fn do_add_entry_with(&self, mtm: MainThreadMarker, label: &str, iana_id: &str, favorite: bool) {
        let ivars = self.ivars();

        let row_stack = NSStackView::new(mtm);
        row_stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        row_stack.setSpacing(8.0);
        row_stack.setDistribution(NSStackViewDistribution::Fill);

        let delegate: &AnyObject = self;

        // 0. Star (favorite) button
        let star_btn = create_star_button(mtm, delegate, favorite);
        star_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
        add_width_constraint(&star_btn, 24.0);

        // 1. Drag Handle
        let handle = DragHandle::new(mtm);
        handle.setTranslatesAutoresizingMaskIntoConstraints(false);
        add_width_constraint(&handle, 20.0);

        // 2. Fields
        let label_field = create_text_field(mtm, "Label", label);
        let iana_combo = create_timezone_combo(mtm, iana_id, &ivars.combo_data_source);

        label_field.setTranslatesAutoresizingMaskIntoConstraints(false);
        iana_combo.setTranslatesAutoresizingMaskIntoConstraints(false);

        unsafe {
            let _: () = msg_send![&label_field, setDelegate: delegate];
            let _: () = msg_send![&iana_combo, setDelegate: delegate];
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
        delete_btn.setBezelStyle(objc2_app_kit::NSBezelStyle::SmallSquare);
        delete_btn.setBordered(false);
        delete_btn.setTranslatesAutoresizingMaskIntoConstraints(false);
        add_width_constraint(&delete_btn, 24.0);

        row_stack.addArrangedSubview(&star_btn);
        row_stack.addArrangedSubview(&handle);
        row_stack.addArrangedSubview(&label_field);
        row_stack.addArrangedSubview(&iana_combo);
        row_stack.addArrangedSubview(&delete_btn);

        // Make both fields equal width
        unsafe {
            let constraint = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &iana_combo,
                NSLayoutAttribute::Width,
                NSLayoutRelation::Equal,
                Some(&label_field),
                NSLayoutAttribute::Width,
                1.0,
                0.0
            );
            row_stack.addConstraint(&*constraint);
        }

        // Insert before the "Add" button (last arranged subview)
        let count = ivars.rows_stack.arrangedSubviews().count();
        if count > 0 {
            ivars.rows_stack.insertArrangedSubview_atIndex(&row_stack, (count - 1) as NSInteger);
        } else {
            ivars.rows_stack.addArrangedSubview(&row_stack);
        }

        self.update_tab_order();
    }

    fn do_remove_entry(&self, _mtm: MainThreadMarker, sender: &NSButton) {
        let ivars = self.ivars();
        let subviews = ivars.rows_stack.arrangedSubviews();

        let mut was_favorite = false;
        for i in 0..subviews.count() {
            let row_view: Retained<NSView> = subviews.objectAtIndex(i).downcast().unwrap();
            if unsafe { row_view.isDescendantOf(&sender.superview().unwrap()) } {
                if let Ok(row_stack) = row_view.clone().downcast::<NSStackView>() {
                    let row_subviews = row_stack.arrangedSubviews();
                    if row_subviews.count() >= ROW_SUBVIEW_COUNT {
                        let star: Retained<NSButton> =
                            row_subviews.objectAtIndex(IDX_STAR).downcast().unwrap();
                        was_favorite = star.state() == NSControlStateValueOn;
                    }
                }
                ivars.rows_stack.removeArrangedSubview(&row_view);
                row_view.removeFromSuperview();
                break;
            }
        }

        if was_favorite {
            self.set_first_row_favorite();
        }

        self.update_tab_order();
        self.do_save();
    }

    fn do_set_favorite(&self, sender: &NSButton) {
        let ivars = self.ivars();
        let subviews = ivars.rows_stack.arrangedSubviews();

        for i in 0..subviews.count() {
            let Ok(row_view) = subviews.objectAtIndex(i).downcast::<NSStackView>() else {
                continue;
            };
            let row_subviews = row_view.arrangedSubviews();
            if row_subviews.count() < ROW_SUBVIEW_COUNT {
                continue;
            }
            let star: Retained<NSButton> = row_subviews.objectAtIndex(IDX_STAR).downcast().unwrap();
            let is_target = sender.isDescendantOf(&row_view);
            star.setState(if is_target {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
        self.do_save();
    }

    fn do_canonicalize(&self) {
        let ivars = self.ivars();
        let subviews = ivars.rows_stack.arrangedSubviews();

        for i in 0..subviews.count() {
            let Ok(row_view) = subviews.objectAtIndex(i).downcast::<NSStackView>() else {
                continue;
            };
            let row_subviews = row_view.arrangedSubviews();
            if row_subviews.count() < ROW_SUBVIEW_COUNT {
                continue;
            }
            let iana_combo: Retained<NSComboBox> =
                row_subviews.objectAtIndex(IDX_TIMEZONE).downcast().unwrap();

            let raw_value = iana_combo.stringValue().to_string();
            let iana_id = search::iana_id_from_display(&raw_value);

            if !iana_id.is_empty() && iana_id != raw_value {
                if let Some(entry) = TimezoneEntry::try_new("", iana_id, false) {
                    iana_combo.setStringValue(&NSString::from_str(entry.iana_id()));
                }
            }
        }
        self.do_save();
    }

    fn set_first_row_favorite(&self) {
        let subviews = self.ivars().rows_stack.arrangedSubviews();
        for i in 0..subviews.count() {
            let Ok(row_view) = subviews.objectAtIndex(i).downcast::<NSStackView>() else {
                continue;
            };
            let row_subviews = row_view.arrangedSubviews();
            if row_subviews.count() < ROW_SUBVIEW_COUNT {
                continue;
            }
            let star: Retained<NSButton> =
                row_subviews.objectAtIndex(IDX_STAR).downcast().unwrap();
            star.setState(NSControlStateValueOn);
            return;
        }
    }

    fn update_tab_order(&self) {
        let subviews = self.ivars().rows_stack.arrangedSubviews();
        let mut prev_combo: Option<Retained<NSView>> = None;

        for i in 0..subviews.count() {
            let Ok(row_view) = subviews.objectAtIndex(i).downcast::<NSStackView>() else {
                continue;
            };
            let row_subviews = row_view.arrangedSubviews();
            if row_subviews.count() < ROW_SUBVIEW_COUNT {
                continue;
            }
            let label_field: Retained<NSView> =
                row_subviews.objectAtIndex(IDX_LABEL).downcast().unwrap();
            let iana_combo: Retained<NSView> =
                row_subviews.objectAtIndex(IDX_TIMEZONE).downcast().unwrap();

            unsafe { label_field.setNextKeyView(Some(&iana_combo)) };

            if let Some(prev) = prev_combo {
                unsafe { prev.setNextKeyView(Some(&label_field)) };
            }
            prev_combo = Some(iana_combo);
        }
    }

    fn do_save(&self) {
        let ivars = self.ivars();
        let mut entries = Vec::new();

        let subviews = ivars.rows_stack.arrangedSubviews();
        for i in 0..subviews.count() {
            let Ok(row_view) = subviews.objectAtIndex(i).downcast::<NSStackView>() else {
                continue;
            };
            let row_subviews = row_view.arrangedSubviews();
            if row_subviews.count() < ROW_SUBVIEW_COUNT {
                continue;
            }

            let star: Retained<NSButton> =
                row_subviews.objectAtIndex(IDX_STAR).downcast().unwrap();
            let label_field: Retained<NSTextField> =
                row_subviews.objectAtIndex(IDX_LABEL).downcast().unwrap();
            let iana_combo: Retained<NSComboBox> =
                row_subviews.objectAtIndex(IDX_TIMEZONE).downcast().unwrap();

            let label = label_field.stringValue().to_string();
            let raw_value = iana_combo.stringValue().to_string();
            let iana_id = search::iana_id_from_display(&raw_value);
            let favorite = star.state() == NSControlStateValueOn;

            if iana_id.is_empty() {
                continue;
            }
            if let Some(entry) = TimezoneEntry::try_new(&label, iana_id, favorite) {
                entries.push(entry);
            }
        }

        settings::save_entries(&entries);
        (ivars.on_save)();
    }
}

fn add_width_constraint(view: &NSView, width: f64) {
    unsafe {
        let constraint =
            objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                view,
                NSLayoutAttribute::Width,
                NSLayoutRelation::Equal,
                None,
                NSLayoutAttribute::NotAnAttribute,
                1.0,
                width,
            );
        view.addConstraint(&*constraint);
    }
}

fn create_star_button(
    mtm: MainThreadMarker,
    target: &AnyObject,
    favorite: bool,
) -> Retained<NSButton> {
    let btn = NSButton::new(mtm);
    btn.setButtonType(NSButtonType::Toggle);
    btn.setBordered(false);

    let star_empty = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        ns_string!("star"),
        Some(ns_string!("Not favorite")),
    );
    let star_filled = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        ns_string!("star.fill"),
        Some(ns_string!("Favorite")),
    );
    btn.setImage(star_empty.as_deref());
    btn.setAlternateImage(star_filled.as_deref());

    btn.setState(if favorite {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    });

    unsafe {
        btn.setTarget(Some(target));
        btn.setAction(Some(sel!(setFavorite:)));
    }
    btn
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

fn create_launch_at_login_checkbox(
    mtm: MainThreadMarker,
    target: &PrefsController,
) -> Retained<NSButton> {
    use objc2_service_management::{SMAppService, SMAppServiceStatus};

    let target_obj: &AnyObject = target;
    let btn = unsafe {
        NSButton::buttonWithTitle_target_action(
            ns_string!("Launch at Login"),
            Some(target_obj),
            Some(sel!(toggleLaunchAtLogin:)),
            mtm,
        )
    };
    btn.setButtonType(NSButtonType::Switch);

    let service = unsafe { SMAppService::mainAppService() };
    let status = unsafe { service.status() };
    btn.setState(if status == SMAppServiceStatus::Enabled {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    });

    btn
}

fn create_add_button_row(mtm: MainThreadMarker, target: &PrefsController) -> Retained<NSStackView> {
    let target_obj: &AnyObject = target;
    let btn = unsafe {
        NSButton::buttonWithTitle_target_action(
            ns_string!("＋"),
            Some(target_obj),
            Some(sel!(addEntry:)),
            mtm,
        )
    };
    btn.setBezelStyle(objc2_app_kit::NSBezelStyle::SmallSquare);
    btn.setBordered(false);
    btn.setTranslatesAutoresizingMaskIntoConstraints(false);
    add_width_constraint(&btn, 24.0);

    let row = NSStackView::new(mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);

    let spacer = NSView::new(mtm);
    spacer.setTranslatesAutoresizingMaskIntoConstraints(false);

    row.addArrangedSubview(&spacer);
    row.addArrangedSubview(&btn);

    row
}

define_class!(
    #[unsafe(super(NSStackView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PrefsStackViewIvars]
    struct PrefsStackView;

    unsafe impl NSObjectProtocol for PrefsStackView {}

    unsafe impl NSDraggingDestination for PrefsStackView {
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
            let view: &NSView = self;
            let view_point = view.convertPoint_fromView(point, None);

            if let Some(src_idx_str) = pboard.stringForType(&row_drag_type()) {
                let src_idx: usize = src_idx_str.to_string().parse().unwrap_or(0);
                self.reorder_logic(src_idx, view_point);
                return Bool::YES;
            }
            Bool::NO
        }
    }

    impl PrefsStackView {
        #[unsafe(method(doReorder:point:))]
        fn do_reorder(&self, src_idx: usize, point: CGPoint) {
            self.reorder_logic(src_idx, point);
        }
    }
);

impl PrefsStackView {
    fn reorder_logic(&self, src_idx: usize, point: CGPoint) {
        let subviews = self.arrangedSubviews();
        if src_idx >= subviews.count() {
            return;
        }

        let mut dest_idx = 0;
        for i in 0..subviews.count() {
            let view: Retained<NSView> = subviews.objectAtIndex(i).downcast().unwrap();
            let frame = view.frame();
            if point.y > frame.origin.y + frame.size.height / 2.0 {
                dest_idx = i;
                break;
            }
            dest_idx = i + 1;
        }

        if src_idx == dest_idx || (dest_idx > 0 && src_idx == dest_idx - 1) {
            return;
        }

        let src_view: Retained<NSView> = subviews.objectAtIndex(src_idx).downcast().unwrap();
        let final_dest = if dest_idx > src_idx { dest_idx - 1 } else { dest_idx };

        self.removeArrangedSubview(&src_view);
        self.insertArrangedSubview_atIndex(&src_view, final_dest.try_into().unwrap());

        if let Some(controller) = self.ivars().controller.borrow().as_ref() {
            let _: () = unsafe { msg_send![controller, reordered] };
        }
    }
}

struct PrefsStackViewIvars {
    controller: RefCell<Option<Retained<PrefsController>>>,
}

impl PrefsStackView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PrefsStackViewIvars {
            controller: RefCell::new(None),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };

        unsafe {
            let types = objc2_foundation::NSArray::from_retained_slice(&[row_drag_type()]);
            let _: () = msg_send![&this, registerForDraggedTypes: &*types];
        }

        this
    }
}

define_class!(
    #[unsafe(super(NSView))]
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
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _rect: CGRect) {
            let bounds = self.bounds();
            let color = NSColor::tertiaryLabelColor();
            color.set();

            let cx = bounds.size.width / 2.0;
            let cy = bounds.size.height / 2.0;
            let line_w = 8.0;
            let spacing = 3.0;

            for i in [-1.0_f64, 0.0, 1.0] {
                let y = cy + i * spacing;
                let path = objc2_app_kit::NSBezierPath::new();
                path.moveToPoint(CGPoint::new(cx - line_w / 2.0, y));
                path.lineToPoint(CGPoint::new(cx + line_w / 2.0, y));
                path.setLineWidth(1.0);
                path.stroke();
            }
        }

        #[unsafe(method(mouseDown:))]
        unsafe fn mouse_down(&self, _event: &objc2_app_kit::NSEvent) {
        }

        #[unsafe(method(mouseDragged:))]
        unsafe fn mouse_dragged(&self, event: &objc2_app_kit::NSEvent) {
            let view: &NSView = self;

            let mut row_idx = 0;
            if let Some(row_view) = unsafe { view.superview() } {
                if let Some(stack_view) = unsafe { row_view.superview() } {
                    let stack: Retained<NSStackView> = stack_view.downcast().unwrap();
                    let subviews = stack.arrangedSubviews();
                    for i in 0..subviews.count() {
                        if subviews.objectAtIndex(i).isEqual(Some(&row_view)) {
                            row_idx = i;
                            break;
                        }
                    }
                }
            }

            let pb_item = NSPasteboardItem::new();
            pb_item.setString_forType(&NSString::from_str(&row_idx.to_string()), &row_drag_type());

            let pb_writer: Retained<ProtocolObject<dyn objc2_app_kit::NSPasteboardWriting>> =
                ProtocolObject::from_retained(pb_item);
            let item = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), &*pb_writer);

            let drag_image = NSImage::initWithSize(NSImage::alloc(), CGSize::new(20.0, 20.0));
            unsafe { item.setDraggingFrame_contents(view.bounds(), Some(&drag_image)) };

            let items = objc2_foundation::NSArray::from_retained_slice(&[item]);
            let source: &ProtocolObject<dyn NSDraggingSource> = ProtocolObject::from_ref(self);
            unsafe {
                let _: Retained<NSDraggingSession> = msg_send![view, beginDraggingSessionWithItems: &*items, event: event, source: source];
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
