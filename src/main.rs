#![deny(unsafe_op_in_unsafe_fn)]

mod settings;
mod timezone;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::sel;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSMenu, NSMenuItem,
    NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{
    ns_string, NSNotification, NSObject, NSObjectProtocol, NSString, NSTimer,
};

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
    }
);

struct AppDelegateIvars {
    status_item: Retained<NSStatusItem>,
    entries: Vec<TimezoneEntry>,
}

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);
        let entries = settings::load_entries().unwrap_or_else(default_entries);
        settings::save_entries(&entries);

        let this = Self::alloc(mtm).set_ivars(AppDelegateIvars {
            status_item,
            entries,
        });
        unsafe { msg_send![super(this), init] }
    }

    fn update_display(&self, mtm: MainThreadMarker) {
        let ivars = self.ivars();
        let now = jiff::Zoned::now();

        // Update menu bar title
        if let Some(first) = ivars.entries.first() {
            let formatted = first.format(&now);
            let title = NSString::from_str(&format!("\u{1F310} {}", formatted.time));
            if let Some(button) = ivars.status_item.button(mtm) {
                button.setTitle(&title);
            }
        }

        // Rebuild dropdown menu
        let menu = NSMenu::new(mtm);

        for entry in &ivars.entries {
            let formatted = entry.format(&now);
            let title = format!(
                "{} \u{2014} {}    {}  {}",
                formatted.label, formatted.city, formatted.time, formatted.relative_day,
            );
            let item = create_menu_item(mtm, &NSString::from_str(&title), None);
            menu.addItem(&item);
        }

        menu.addItem(&NSMenuItem::separatorItem(mtm));

        let preferences_item = create_menu_item(mtm, ns_string!("Preferences\u{2026}"), None);
        menu.addItem(&preferences_item);

        menu.addItem(&NSMenuItem::separatorItem(mtm));

        let quit_item = create_menu_item(mtm, ns_string!("Quit"), Some(sel!(terminate:)));
        quit_item.setKeyEquivalent(ns_string!("q"));
        menu.addItem(&quit_item);

        ivars.status_item.setMenu(Some(&menu));
    }

    fn schedule_timer(&self, _mtm: MainThreadMarker) {
        let now = jiff::Zoned::now();
        let seconds_past_minute = now.second() as f64 + now.subsec_nanosecond() as f64 / 1e9;
        let delay = 60.0 - seconds_past_minute;

        // One-shot timer aligned to the next minute boundary;
        // timerFired: re-schedules for the following minute.
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

    let delegate = AppDelegate::new(mtm);
    let object = ProtocolObject::from_ref(&*delegate);
    app.setDelegate(Some(object));

    app.run();
}
