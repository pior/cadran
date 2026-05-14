# Milestone 1 Plan: Lightweight macOS Timezone Menu Bar App

## Goal

Build a lightweight macOS menu bar app for quickly checking the time in colleagues' timezones.

The first milestone should prove the core product loop:

1. The app runs as a menu bar-only macOS app.
2. The menu bar shows the current time for a primary timezone.
3. Clicking the menu bar item shows a list of saved timezones.
4. Users can configure the list from a preferences window.
5. The app stays lightweight and avoids heavy runtimes such as webviews.

## Product Scope

### Core Use Case

A user wants to quickly answer questions like:

- What time is it for Sam in London?
- Is it already tomorrow in Tokyo?
- Which timezone am I currently showing in the menu bar?

The app should optimize for fast glanceability and low background cost.

## Milestone 1 Requirements

### Menu Bar Item

The app should create a macOS status/menu bar item.

The menu bar item should show:

- A static clock or globe-style icon.
- The current time for the primary timezone.

Example:

```text
🌐 14:32
```

The primary timezone is the first entry in the saved timezone list.

### Dropdown Menu

Clicking the menu bar item should open a dropdown menu showing the saved timezones.

Each timezone row should include:

- Custom label.
- City or IANA timezone identifier.
- Current local time.
- Relative day indicator: Today, Tomorrow, or Yesterday.

Example:

```text
🌐 14:32

Sam — London        14:32 Today
Mika — Tokyo        22:32 Today
Ana — New York      09:32 Today
Priya — Bangalore   19:02 Today
Leo — Sydney        23:32 Today

Preferences…
Quit
```

### Saved Timezones

Milestone 1 should comfortably support around 5 saved timezone entries.

Each entry should store:

- Custom label, for example `Sam` or `Support APAC`.
- Display location/city, for example `London`.
- IANA timezone ID, for example `Europe/London`.

The app should use the IANA timezone ID for all time calculations.

### Preferences Window

Milestone 1 should include a preferences window.

The preferences window should support:

- Add timezone entry.
- Remove timezone entry.
- Edit label.
- Edit city/timezone.
- Reorder entries.
- Configure time format.
- Configure launch at login if practical.

The first entry in the ordered list is the primary timezone shown in the menu bar.

If launch-at-login creates packaging, signing, or macOS permission complications, treat it as a stretch item and defer it until after the core app is working.

### Timezone Picker

The timezone picker should support both:

- City search, such as `Paris`, `Tokyo`, or `New York`.
- IANA timezone ID search, such as `Europe/Paris`, `Asia/Tokyo`, or `America/New_York`.

The picker may be simple in milestone 1, but it should store canonical IANA timezone IDs internally.

### Time Format

Default behavior:

- Follow the macOS system 12-hour / 24-hour preference.

Preferences may expose an explicit format setting if straightforward:

- Follow system preference.
- Always 24-hour.
- Always 12-hour with AM/PM.

### Update Frequency

Displayed times should update once per minute.

Do not show seconds.

The app should avoid continuous work while idle.

### Persistence

Use native macOS user defaults/preferences for milestone 1 persistence.

The saved preferences should include:

- Ordered timezone entries.
- Time format setting, if implemented.
- Launch-at-login setting, if implemented.

Preferences must persist across app restarts.

## Technical Direction

### Runtime and Framework Constraints

Lightweight behavior is a priority.

The implementation should avoid:

- Webviews.
- Electron-style runtimes.
- Tauri, unless later evidence shows the overhead is acceptable.
- Background polling faster than once per minute.

Preferred direction:

- Rust application.
- Native macOS status item and menu APIs.
- Small Rust/macOS abstraction layer only if it keeps runtime overhead tiny.

Candidate implementation options to evaluate:

- `objc2` / `objc2-app-kit` for native AppKit bindings.
- A small Rust crate that wraps macOS menu bar/status item APIs, if actively maintained and minimal.

### Timezone Data

Use a Rust timezone/date-time library that supports IANA timezone IDs.

Candidate crates:

- `jiff`
- `chrono` plus `chrono-tz`
- `time` plus a timezone database integration, if appropriate

Selection criteria:

- Correct IANA timezone handling.
- Handles daylight saving time correctly.
- Small dependency footprint.
- Simple formatting API.

### App Shape

Initial app shape:

- Menu bar-only app.
- No dock icon if practical.
- Status item owns the menu.
- Preferences window can be opened from the menu.
- Quit item exits the app.

### Packaging

For milestone 1, running locally from Cargo is acceptable:

```bash
cargo run
```

A polished `.app` bundle is not required for milestone 1.

However, if menu bar behavior, preferences, or launch-at-login require app bundle behavior, create the smallest local development `.app` bundle needed for testing.

Signing, notarization, Homebrew distribution, and installer work are out of scope.

## Suggested Implementation Steps

### Step 1: Project Skeleton

- Create a Rust project.
- Confirm the app can start a macOS application event loop.
- Configure it as menu bar-only if practical.

Done when:

- `cargo run` starts an app process without crashing.
- The app does not need a main window.

### Step 2: Basic Status Item

- Create a macOS status item.
- Show a static icon plus placeholder time.
- Add a dropdown menu with `Preferences…` and `Quit`.

Done when:

- A menu bar item appears.
- Clicking it opens a menu.
- `Quit` exits the app.

### Step 3: Timezone Time Calculation

- Add timezone/date-time dependency.
- Hardcode a small list of sample timezone entries.
- Format current local time for each entry.
- Compute Today/Tomorrow/Yesterday relative to the user's local date.

Done when:

- Menu shows correct current times for sample zones.
- DST-sensitive zones use correct offsets.

### Step 4: Minute Updates

- Add a timer that updates displayed times once per minute.
- Align updates near minute boundaries if straightforward.
- Ensure the menu bar text updates without reopening the app.

Done when:

- Menu bar time changes at minute granularity.
- Dropdown times are fresh when opened.
- Idle CPU remains effectively zero.

### Step 5: Preferences Persistence

- Define the persisted settings model.
- Store settings in macOS user defaults/preferences.
- Load settings on startup.
- Save changes immediately or on preferences close.

Done when:

- Timezone entries persist across restarts.

### Step 6: Preferences Window

- Add a preferences window.
- Support add, remove, edit, and reorder for timezone entries.
- First entry controls the menu bar timezone.
- Add time format setting if practical.

Done when:

- User can configure approximately 5 timezones without editing files.
- Reordering changes the primary menu bar timezone.

### Step 7: City and Timezone Search

- Provide a simple picker/search field.
- Search should match city names and IANA timezone IDs.
- Store selected entries as IANA timezone IDs.

Done when:

- Searching `Tokyo` can select `Asia/Tokyo`.
- Searching `Europe/Paris` can select `Europe/Paris`.

### Step 8: Lightweight Validation

- Confirm no webview or heavy runtime is present.
- Inspect CPU behavior while idle.
- Inspect memory footprint informally.
- Confirm updates happen once per minute, not continuously.

Done when:

- The app appears idle in Activity Monitor outside the minute timer.
- Dependency tree looks reasonable.

## Stretch Items

These are desirable but not required for milestone 1:

- Launch at login.
- Basic local `.app` bundle.
- Custom monochrome menu bar icon asset.
- More polished preferences layout.
- Keyboard shortcuts in preferences.
- Import/export settings.
- Working-hours indicators.

## Explicit Non-Goals

Milestone 1 should not include:

- Working-hours schedules.
- Calendar integration.
- Meeting-time suggestions.
- Seconds display.
- Multiple timezone chips directly in the menu bar.
- Cloud sync.
- Signed or notarized distribution.
- Homebrew packaging.
- Full onboarding flow.
- Heavy UI frameworks or webviews.

## Acceptance Criteria

Milestone 1 is complete when:

1. The app runs as a menu bar-only macOS app.
2. The menu bar shows a static icon plus the current time for the first configured timezone.
3. Clicking the menu bar item shows saved timezones with correct local times.
4. Dropdown entries show custom labels and Today/Tomorrow/Yesterday relative day context.
5. Preferences can add, remove, edit, and reorder around 5 timezone entries.
6. Preferences persist across app restarts.
7. Time updates once per minute.
8. The app follows the system 12-hour / 24-hour preference by default.
9. The implementation avoids webviews and heavy runtimes.
10. Idle CPU usage is effectively zero outside minute-level updates.

## Open Questions

- Which Rust/macOS framework or binding should be used for the first prototype?
- Which Rust timezone library offers the best balance of correctness and small dependency footprint?
- How much preference UI polish is possible without compromising lightweight implementation?
- Should launch-at-login remain in milestone 1 or become milestone 2?
