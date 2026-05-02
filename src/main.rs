#![deny(unsafe_op_in_unsafe_fn)]

mod preferences;
mod search;
mod settings;
mod timezone;

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::sel;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSColor, NSFont, NSMenu,
    NSMenuItem, NSStatusBar, NSStatusItem, NSTextAlignment, NSTextField, NSVariableStatusItemLength,
    NSView,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{ns_string, NSNotification, NSObject, NSObjectProtocol, NSString, NSTimer};

use timezone::{default_entries, TimezoneEntry};

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppDelegateIvars]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            let mtm = MainThreadMarker::from(self);
            self.update_display(mtm);
            self.schedule_timer(mtm);
        }
    }

    impl AppDelegate {
        #[unsafe(method(timerFired:))]
        unsafe fn timer_fired(&self, _timer: &NSTimer) {
            let mtm = MainThreadMarker::from(self);
            self.update_display(mtm);
            self.schedule_timer(mtm);
        }

        #[unsafe(method(showPreferences:))]
        unsafe fn show_preferences(&self, _sender: &AnyObject) {
            let mtm = MainThreadMarker::from(self);
            self.open_preferences(mtm);
        }

        #[unsafe(method(reloadEntries:))]
        unsafe fn reload_entries(&self, _sender: &AnyObject) {
            let mtm = MainThreadMarker::from(self);
            if let Some(entries) = settings::load_entries() {
                *self.ivars().entries.borrow_mut() = entries;
            }
            self.update_display(mtm);
        }
    }
);

struct AppDelegateIvars {
    status_item: Retained<NSStatusItem>,
    entries: RefCell<Vec<TimezoneEntry>>,
    prefs_controller: RefCell<Option<Retained<NSObject>>>,
}

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);
        let entries = settings::load_entries().unwrap_or_else(default_entries);
        settings::save_entries(&entries);

        let this = Self::alloc(mtm).set_ivars(AppDelegateIvars {
            status_item,
            entries: RefCell::new(entries),
            prefs_controller: RefCell::new(None),
        });
        unsafe { msg_send![super(this), init] }
    }

    fn update_display(&self, mtm: MainThreadMarker) {
        let ivars = self.ivars();
        let entries = ivars.entries.borrow();
        let now = jiff::Zoned::now();

        if let Some(first) = entries.first() {
            let formatted = first.format(&now);
            let title = NSString::from_str(&format!("\u{1F310} {}", formatted.time));
            if let Some(button) = ivars.status_item.button(mtm) {
                button.setTitle(&title);
            }
        }

        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);

        let menu_width = 250.0;
        for entry in entries.iter() {
            let formatted = entry.format(&now);
            let time_text = if formatted.relative_day.is_empty() {
                formatted.time.clone()
            } else {
                format!("{}  {}", formatted.time, formatted.relative_day)
            };
            let view = create_entry_view(mtm, &formatted.label, &time_text, menu_width);
            let item = NSMenuItem::new(mtm);
            item.setView(Some(&view));
            menu.addItem(&item);
        }

        menu.addItem(&NSMenuItem::separatorItem(mtm));

        let preferences_item = create_menu_item(
            mtm,
            ns_string!("Preferences\u{2026}"),
            Some(sel!(showPreferences:)),
        );
        unsafe { preferences_item.setTarget(Some(self as &AnyObject)) };
        preferences_item.setEnabled(true);
        menu.addItem(&preferences_item);

        menu.addItem(&NSMenuItem::separatorItem(mtm));

        let quit_item = create_menu_item(mtm, ns_string!("Quit"), Some(sel!(terminate:)));
        quit_item.setKeyEquivalent(ns_string!("q"));
        menu.addItem(&quit_item);

        ivars.status_item.setMenu(Some(&menu));
    }

    fn open_preferences(&self, mtm: MainThreadMarker) {
        let ivars = self.ivars();

        // Reuse existing window if it exists
        {
            let controller_ref = ivars.prefs_controller.borrow();
            if let Some(obj) = controller_ref.as_ref() {
                let _: () = unsafe { msg_send![obj, showWindow] };
                return;
            }
        }

        let app_delegate_ptr = self as *const AppDelegate as usize;
        let on_save = Box::new(move || {
            let obj = app_delegate_ptr as *const AnyObject;
            unsafe {
                let _: () = msg_send![obj, reloadEntries: std::ptr::null::<AnyObject>()];
            }
        });

        let controller = {
            let entries = ivars.entries.borrow();
            preferences::PrefsController::new(mtm, &entries, on_save)
        };

        controller.show();
        *ivars.prefs_controller.borrow_mut() = Some(Retained::into_super(controller));
    }

    fn schedule_timer(&self, _mtm: MainThreadMarker) {
        let now = jiff::Zoned::now();
        let seconds_past_minute = now.second() as f64 + now.subsec_nanosecond() as f64 / 1e9;
        let delay = 60.0 - seconds_past_minute;

        unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                delay,
                self as &AnyObject,
                sel!(timerFired:),
                None,
                false,
            );
        }
    }
}

fn create_entry_view(
    mtm: MainThreadMarker,
    label: &str,
    time: &str,
    width: f64,
) -> Retained<NSView> {
    let height = 22.0;
    let padding = 14.0;
    let view = NSView::initWithFrame(
        NSView::alloc(mtm),
        CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(width, height)),
    );

    let font = NSFont::menuFontOfSize(0.0);

    let label_field = create_menu_label(mtm, label);
    label_field.setFont(Some(&font));
    label_field.setFrame(CGRect::new(
        CGPoint::new(padding, 0.0),
        CGSize::new(width * 0.6 - padding, height),
    ));

    let time_field = create_menu_label(mtm, time);
    time_field.setFont(Some(&font));
    time_field.setAlignment(NSTextAlignment::Right);
    time_field.setTextColor(Some(&NSColor::secondaryLabelColor()));
    time_field.setFrame(CGRect::new(
        CGPoint::new(width * 0.6, 0.0),
        CGSize::new(width * 0.4 - padding, height),
    ));

    view.addSubview(&label_field);
    view.addSubview(&time_field);
    view
}

fn create_menu_label(mtm: MainThreadMarker, text: &str) -> Retained<NSTextField> {
    let field = NSTextField::new(mtm);
    field.setStringValue(&NSString::from_str(text));
    field.setEditable(false);
    field.setBezeled(false);
    field.setBordered(false);
    field.setDrawsBackground(false);
    field
}

fn create_menu_item(
    mtm: MainThreadMarker,
    title: &NSString,
    action: Option<Sel>,
) -> Retained<NSMenuItem> {
    let item = NSMenuItem::new(mtm);
    item.setTitle(title);
    unsafe { item.setAction(action) };
    item
}

fn main() {
    let mtm = MainThreadMarker::new().unwrap();

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    // Create a main menu with an Edit menu to enable standard shortcuts like Copy/Paste
    let main_menu = NSMenu::new(mtm);

    let edit_menu_item = NSMenuItem::new(mtm);
    let edit_menu = NSMenu::new(mtm);
    edit_menu.setTitle(ns_string!("Edit"));

    let undo_item = NSMenuItem::new(mtm);
    undo_item.setTitle(ns_string!("Undo"));
    unsafe { undo_item.setAction(Some(sel!(undo:))) };
    undo_item.setKeyEquivalent(ns_string!("z"));
    edit_menu.addItem(&undo_item);

    let redo_item = NSMenuItem::new(mtm);
    redo_item.setTitle(ns_string!("Redo"));
    unsafe { redo_item.setAction(Some(sel!(redo:))) };
    redo_item.setKeyEquivalent(ns_string!("Z"));
    edit_menu.addItem(&redo_item);

    edit_menu.addItem(&NSMenuItem::separatorItem(mtm));

    let cut_item = NSMenuItem::new(mtm);
    cut_item.setTitle(ns_string!("Cut"));
    unsafe { cut_item.setAction(Some(sel!(cut:))) };
    cut_item.setKeyEquivalent(ns_string!("x"));
    edit_menu.addItem(&cut_item);

    let copy_item = NSMenuItem::new(mtm);
    copy_item.setTitle(ns_string!("Copy"));
    unsafe { copy_item.setAction(Some(sel!(copy:))) };
    copy_item.setKeyEquivalent(ns_string!("c"));
    edit_menu.addItem(&copy_item);

    let paste_item = NSMenuItem::new(mtm);
    paste_item.setTitle(ns_string!("Paste"));
    unsafe { paste_item.setAction(Some(sel!(paste:))) };
    paste_item.setKeyEquivalent(ns_string!("v"));
    edit_menu.addItem(&paste_item);

    let select_all_item = NSMenuItem::new(mtm);
    select_all_item.setTitle(ns_string!("Select All"));
    unsafe { select_all_item.setAction(Some(sel!(selectAll:))) };
    select_all_item.setKeyEquivalent(ns_string!("a"));
    edit_menu.addItem(&select_all_item);

    edit_menu_item.setSubmenu(Some(&edit_menu));
    main_menu.addItem(&edit_menu_item);

    app.setMainMenu(Some(&main_menu));

    let delegate = AppDelegate::new(mtm);
    let object = ProtocolObject::from_ref(&*delegate);
    app.setDelegate(Some(object));

    app.run();
}
