use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, ProtocolObject, Sel};
use objc2::sel;
use objc2::{
    define_class, msg_send, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message,
};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSButtonType, NSColor, NSComboBox, NSControl, NSDragOperation,
    NSDraggingContext, NSDraggingDestination, NSDraggingInfo, NSDraggingItem, NSDraggingSession,
    NSDraggingSource, NSFont, NSImage, NSLayoutAttribute, NSLayoutRelation, NSPasteboardItem,
    NSStackView, NSStackViewDistribution, NSTextField, NSTextFieldBezelStyle,
    NSUserInterfaceLayoutOrientation, NSView, NSWindow, NSWindowStyleMask, NSWorkspace,
};
use objc2_app_kit::{NSControlStateValueOff, NSControlStateValueOn};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{
    ns_string, NSInteger, NSNotification, NSObject, NSObjectProtocol, NSString, NSURL,
};

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
const MAX_ENTRY_ROWS: usize = 15;
const PREF_WINDOW_WIDTH: f64 = 560.0;
const CONTENT_PADDING: f64 = 16.0;
const SECTION_GAP: f64 = 10.0;
const GITHUB_URL: &str = "https://github.com/pior/cadran";

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
            self.ivars().items.borrow().len() as NSInteger
        }

        #[unsafe(method(comboBox:objectValueForItemAtIndex:))]
        fn object_value_for_item(
            &self,
            _combo_box: &NSComboBox,
            index: NSInteger,
        ) -> *mut AnyObject {
            let items = self.ivars().items.borrow();
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
            _string: &NSString,
        ) -> *mut NSString {
            std::ptr::null_mut()
        }
    }
);

struct ComboDataSourceIvars {
    search: TimezoneSearch,
    items: RefCell<Vec<String>>,
}

impl ComboDataSource {
    fn new(mtm: MainThreadMarker, search: TimezoneSearch) -> Retained<Self> {
        let items = RefCell::new(search.combo_items());
        let this = Self::alloc(mtm).set_ivars(ComboDataSourceIvars { search, items });
        unsafe { msg_send![super(this), init] }
    }

    fn update_filter(&self, query: &str) {
        self.ivars()
            .items
            .replace(self.ivars().search.completions_for(query));
    }

    fn should_show_popup(&self, query: &str) -> bool {
        let query = query.trim();
        if query.is_empty() {
            return false;
        }

        let items = self.ivars().items.borrow();
        if items.is_empty() {
            return false;
        }

        items.len() > 1 || items.first().is_some_and(|item| item != query)
    }

    fn single_completion(&self, query: &str) -> Option<String> {
        let query = query.trim();
        if query.is_empty() {
            return None;
        }

        let completions = self.ivars().search.completions_for(query);
        match completions.as_slice() {
            [completion] if completion != query => Some(completion.clone()),
            _ => None,
        }
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
        fn control_text_did_change(&self, notification: &NSNotification) {
            self.update_combo_suggestions(notification);
            self.do_save();
        }

        #[unsafe(method(controlTextDidBeginEditing:))]
        fn control_text_did_begin_editing(&self, notification: &NSNotification) {
            self.update_combo_suggestions(notification);
        }

        #[unsafe(method(controlTextDidEndEditing:))]
        fn control_text_did_end_editing(&self, notification: &NSNotification) {
            self.commit_single_combo_suggestion(notification);
            self.do_canonicalize();
        }

        #[unsafe(method(control:textView:doCommandBySelector:))]
        fn control_text_view_do_command_by_selector(
            &self,
            control: &NSControl,
            _text_view: &AnyObject,
            command_selector: Sel,
        ) -> Bool {
            if command_selector != sel!(insertNewline:)
                && command_selector != sel!(insertNewlineIgnoringFieldEditor:)
            {
                return Bool::NO;
            }

            let Some(combo) = control.downcast_ref::<NSComboBox>() else {
                return Bool::NO;
            };

            if self.commit_single_combo(combo) {
                self.dismiss_combo_popup(combo);
                self.do_canonicalize();
                return Bool::YES;
            }

            Bool::NO
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

        #[unsafe(method(openGitHub:))]
        unsafe fn open_github(&self, _sender: &AnyObject) {
            if let Some(url) = NSURL::URLWithString(&NSString::from_str(GITHUB_URL)) {
                NSWorkspace::sharedWorkspace().openURL(&url);
            }
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
        let frame = CGRect::new(
            CGPoint::new(0.0, 0.0),
            CGSize::new(PREF_WINDOW_WIDTH, 400.0),
        );
        let style =
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable;

        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                frame,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        window.setTitle(ns_string!("Cadran Settings"));
        window.center();
        unsafe { window.setReleasedWhenClosed(false) };

        let rows_stack = PrefsStackView::new(mtm);
        rows_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        rows_stack.setSpacing(8.0);
        rows_stack.setAlignment(NSLayoutAttribute::Width);
        rows_stack.setTranslatesAutoresizingMaskIntoConstraints(false);

        let search = TimezoneSearch::new();
        let combo_data_source = ComboDataSource::new(mtm, search);

        let this = Self::alloc(mtm).set_ivars(PrefsControllerIvars {
            window: window.retain(),
            rows_stack: rows_stack.retain(),
            combo_data_source,
            on_save,
        });
        let controller: Retained<Self> = unsafe { msg_send![super(this), init] };
        rows_stack
            .ivars()
            .controller
            .replace(Some(controller.retain()));

        let add_row = create_add_button_row(mtm, &controller);
        rows_stack.addArrangedSubview(&add_row);

        for entry in entries {
            controller.do_add_entry_with(mtm, &entry.label, entry.iana_id(), entry.favorite);
        }

        controller.update_tab_order();

        let launch_checkbox = create_launch_at_login_checkbox(mtm, &controller);
        launch_checkbox.setTranslatesAutoresizingMaskIntoConstraints(false);
        let footer = create_footer(mtm, &controller);
        footer.setTranslatesAutoresizingMaskIntoConstraints(false);

        let content_view = PrefsContentView::new(mtm);
        content_view.setTranslatesAutoresizingMaskIntoConstraints(false);
        window.setContentView(Some(&content_view));
        window.setInitialFirstResponder(Some(&content_view));

        content_view.addSubview(&rows_stack);
        content_view.addSubview(&launch_checkbox);
        content_view.addSubview(&footer);

        unsafe {
            let leading = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*rows_stack, NSLayoutAttribute::Leading, NSLayoutRelation::Equal,
                Some(&*content_view), NSLayoutAttribute::Leading, 1.0, CONTENT_PADDING,
            );
            let trailing = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*rows_stack, NSLayoutAttribute::Trailing, NSLayoutRelation::Equal,
                Some(&*content_view), NSLayoutAttribute::Trailing, 1.0, -CONTENT_PADDING,
            );
            let top = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*rows_stack, NSLayoutAttribute::Top, NSLayoutRelation::Equal,
                Some(&*content_view), NSLayoutAttribute::Top, 1.0, CONTENT_PADDING,
            );
            let cb_center = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*launch_checkbox, NSLayoutAttribute::CenterX, NSLayoutRelation::Equal,
                Some(&*content_view), NSLayoutAttribute::CenterX, 1.0, 0.0,
            );
            let cb_top = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*launch_checkbox, NSLayoutAttribute::Top, NSLayoutRelation::Equal,
                Some(&*rows_stack), NSLayoutAttribute::Bottom, 1.0, SECTION_GAP,
            );
            let footer_top = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*footer, NSLayoutAttribute::Top, NSLayoutRelation::Equal,
                Some(&*launch_checkbox), NSLayoutAttribute::Bottom, 1.0, SECTION_GAP,
            );
            let footer_bottom = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*footer, NSLayoutAttribute::Bottom, NSLayoutRelation::Equal,
                Some(&*content_view), NSLayoutAttribute::Bottom, 1.0, -CONTENT_PADDING,
            );
            let footer_leading = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*footer, NSLayoutAttribute::Leading, NSLayoutRelation::Equal,
                Some(&*content_view), NSLayoutAttribute::Leading, 1.0, CONTENT_PADDING,
            );
            let footer_trailing = objc2_app_kit::NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &*footer, NSLayoutAttribute::Trailing, NSLayoutRelation::Equal,
                Some(&*content_view), NSLayoutAttribute::Trailing, 1.0, -CONTENT_PADDING,
            );
            leading.setActive(true);
            trailing.setActive(true);
            top.setActive(true);
            cb_center.setActive(true);
            cb_top.setActive(true);
            footer_top.setActive(true);
            footer_bottom.setActive(true);
            footer_leading.setActive(true);
            footer_trailing.setActive(true);
        }
        window.makeFirstResponder(Some(&content_view));
        controller.resize_to_fit_entries();
        controller.update_add_button_state();

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
        let mtm = MainThreadMarker::from(self);
        let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
        app.activate();

        let window = &self.ivars().window;
        window.deminiaturize(None);
        window.makeKeyAndOrderFront(None);
        window.orderFrontRegardless();
    }

    fn do_add_entry(&self, mtm: MainThreadMarker) {
        if self.entry_count() >= MAX_ENTRY_ROWS {
            return;
        }
        let favorite = self.entry_count() == 0;
        self.do_add_entry_with(mtm, "", "", favorite);
    }

    fn do_add_entry_with(&self, mtm: MainThreadMarker, label: &str, iana_id: &str, favorite: bool) {
        if self.entry_count() >= MAX_ENTRY_ROWS {
            return;
        }

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
            ivars
                .rows_stack
                .insertArrangedSubview_atIndex(&row_stack, (count - 1) as NSInteger);
        } else {
            ivars.rows_stack.addArrangedSubview(&row_stack);
        }

        self.update_tab_order();
        self.resize_to_fit_entries();
        self.update_add_button_state();
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
        self.resize_to_fit_entries();
        self.update_add_button_state();
        self.do_save();
    }

    fn entry_count(&self) -> usize {
        self.ivars()
            .rows_stack
            .arrangedSubviews()
            .count()
            .saturating_sub(1) as usize
    }

    fn resize_to_fit_entries(&self) {
        let window = &self.ivars().window;
        if let Some(content_view) = window.contentView() {
            content_view.layoutSubtreeIfNeeded();
            let fitting = content_view.fittingSize();
            window.setContentSize(CGSize::new(PREF_WINDOW_WIDTH, fitting.height));
        }
    }

    fn update_add_button_state(&self) {
        let subviews = self.ivars().rows_stack.arrangedSubviews();
        if subviews.count() == 0 {
            return;
        }

        let Ok(add_row) = subviews
            .objectAtIndex(subviews.count() - 1)
            .downcast::<NSStackView>()
        else {
            return;
        };
        let add_row_subviews = add_row.arrangedSubviews();
        if add_row_subviews.count() < 2 {
            return;
        }
        let Ok(add_button) = add_row_subviews.objectAtIndex(1).downcast::<NSButton>() else {
            return;
        };
        add_button.setEnabled(self.entry_count() < MAX_ENTRY_ROWS);
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

    fn update_combo_suggestions(&self, notification: &NSNotification) {
        let Some(object) = notification.object() else {
            return;
        };
        let Ok(combo) = object.downcast::<NSComboBox>() else {
            return;
        };

        let query = combo.stringValue().to_string();
        self.ivars().combo_data_source.update_filter(&query);
        combo.noteNumberOfItemsChanged();
        combo.reloadData();

        if self.ivars().combo_data_source.should_show_popup(&query) {
            if let Some(cell) = combo.cell() {
                if cell.respondsToSelector(sel!(popUp:)) {
                    unsafe {
                        let _: () = msg_send![&*cell, popUp: &*combo];
                    }
                }
            }
        }
    }

    fn commit_single_combo_suggestion(&self, notification: &NSNotification) -> bool {
        let Some(object) = notification.object() else {
            return false;
        };
        let Ok(combo) = object.downcast::<NSComboBox>() else {
            return false;
        };

        self.commit_single_combo(&combo)
    }

    fn commit_single_combo(&self, combo: &NSComboBox) -> bool {
        let query = combo.stringValue().to_string();
        let Some(completion) = self.ivars().combo_data_source.single_completion(&query) else {
            return false;
        };

        combo.setStringValue(&NSString::from_str(&completion));
        true
    }

    fn dismiss_combo_popup(&self, combo: &NSComboBox) {
        if let Some(cell) = combo.cell() {
            if cell.respondsToSelector(sel!(dismissPopUp:)) {
                unsafe {
                    let _: () = msg_send![&*cell, dismissPopUp: &*combo];
                }
            }
        }
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
            let star: Retained<NSButton> = row_subviews.objectAtIndex(IDX_STAR).downcast().unwrap();
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

            let star: Retained<NSButton> = row_subviews.objectAtIndex(IDX_STAR).downcast().unwrap();
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

fn create_footer(mtm: MainThreadMarker, target: &PrefsController) -> Retained<NSStackView> {
    let footer = NSStackView::new(mtm);
    footer.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    footer.setSpacing(8.0);
    footer.setDistribution(NSStackViewDistribution::Fill);

    let font = NSFont::systemFontOfSize(NSFont::smallSystemFontSize());
    let secondary = NSColor::secondaryLabelColor();

    let name = NSTextField::labelWithString(ns_string!("Cadran V1.0"), mtm);
    name.setFont(Some(&font));
    name.setTextColor(Some(&secondary));

    let link = unsafe {
        NSButton::buttonWithTitle_target_action(
            ns_string!("github.com/pior/cadran"),
            Some(target as &AnyObject),
            Some(sel!(openGitHub:)),
            mtm,
        )
    };
    link.setBordered(false);
    link.setFont(Some(&font));
    link.setContentTintColor(Some(&NSColor::linkColor()));
    link.setToolTip(Some(&NSString::from_str(GITHUB_URL)));

    let spacer = NSView::new(mtm);
    spacer.setTranslatesAutoresizingMaskIntoConstraints(false);

    footer.addArrangedSubview(&name);
    footer.addArrangedSubview(&link);
    footer.addArrangedSubview(&spacer);
    footer
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
    combo.setCompletes(false);
    combo.setUsesDataSource(true);
    let ds_obj = data_source as &AnyObject;
    unsafe {
        let _: () = msg_send![&combo, setDataSource: ds_obj];
    }
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
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    struct PrefsContentView;

    unsafe impl NSObjectProtocol for PrefsContentView {}

    impl PrefsContentView {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &objc2_app_kit::NSEvent) {
            if let Some(window) = self.window() {
                window.makeFirstResponder(Some(self));
            }
            unsafe {
                let _: () = msg_send![super(self), mouseDown: event];
            }
        }
    }
);

impl PrefsContentView {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
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
        let final_dest = if dest_idx > src_idx {
            dest_idx - 1
        } else {
            dest_idx
        };

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
