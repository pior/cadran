use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::sel;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSComboBox, NSStackView, NSStackViewDistribution, NSTextField,
    NSTextFieldBezelStyle, NSUserInterfaceLayoutOrientation, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{ns_string, NSInteger, NSObject, NSObjectProtocol, NSString};

use crate::search::{self, TimezoneSearch};
use crate::settings;
use crate::timezone::TimezoneEntry;

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
                None => std::ptr::null_mut(),
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
    label_field: Retained<NSTextField>,
    iana_combo: Retained<NSComboBox>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PrefsControllerIvars]
    pub struct PrefsController;

    unsafe impl NSObjectProtocol for PrefsController {}

    impl PrefsController {
        #[unsafe(method(addEntry:))]
        unsafe fn add_entry(&self, _sender: &AnyObject) {
            let mtm = MainThreadMarker::from(self);
            self.do_add_entry(mtm);
        }

        #[unsafe(method(removeLastEntry:))]
        unsafe fn remove_last_entry(&self, _sender: &AnyObject) {
            let mtm = MainThreadMarker::from(self);
            self.do_remove_last_entry(mtm);
        }

        #[unsafe(method(saveEntries:))]
        unsafe fn save_entries(&self, _sender: &AnyObject) {
            self.do_save();
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
        let app = objc2_app_kit::NSApplication::sharedApplication(MainThreadMarker::from(self));
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
        row_stack.setDistribution(NSStackViewDistribution::FillEqually);

        let label_field = create_text_field(mtm, "Label", label);
        let iana_combo = create_timezone_combo(mtm, iana_id, &ivars.combo_data_source);

        row_stack.addArrangedSubview(&label_field);
        row_stack.addArrangedSubview(&iana_combo);

        ivars.rows_stack.addArrangedSubview(&row_stack);
        ivars.rows.borrow_mut().push(EntryRow {
            label_field,
            iana_combo,
        });
    }

    fn do_remove_last_entry(&self, _mtm: MainThreadMarker) {
        let ivars = self.ivars();
        let mut rows = ivars.rows.borrow_mut();
        if rows.is_empty() {
            return;
        }
        rows.pop();
        let subviews = ivars.rows_stack.arrangedSubviews();
        if let Some(last) = subviews.lastObject() {
            ivars.rows_stack.removeArrangedSubview(&last);
            last.removeFromSuperview();
        }
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
                row.iana_combo
                    .setStringValue(&NSString::from_str(entry.iana_id()));
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

    let remove_btn = unsafe {
        NSButton::buttonWithTitle_target_action(
            ns_string!("\u{2212} Remove Last"),
            Some(target_obj),
            Some(sel!(removeLastEntry:)),
            mtm,
        )
    };

    let save_btn = unsafe {
        NSButton::buttonWithTitle_target_action(
            ns_string!("Save"),
            Some(target_obj),
            Some(sel!(saveEntries:)),
            mtm,
        )
    };

    row.addArrangedSubview(&add_btn);
    row.addArrangedSubview(&remove_btn);
    let spacer = NSView::new(mtm);
    row.addArrangedSubview(&spacer);
    row.addArrangedSubview(&save_btn);

    row
}
