# Cadran

A lightweight multi-timezone macOS menu bar app built in Rust with native AppKit bindings.

## Build & Test

```
cargo build
cargo test
```

## Architecture

- `main.rs` — App delegate, status item, dropdown menu, timer
- `preferences.rs` — Settings window (AppKit views, drag-and-drop, Auto Layout)
- `timezone.rs` — `TimezoneEntry` model and time formatting
- `resolver.rs` — Timezone resolution (IANA IDs, UTC offsets, aliases)
- `search.rs` — Timezone search/completion engine
- `settings.rs` — Persistence via NSUserDefaults

Key dependencies: `objc2` / `objc2-app-kit` (native AppKit), `jiff` (timezone/datetime).

## Requirements

<!--
  This is an incremental requirements list. It serves as a non-regression
  contract, complementary to unit tests. Every user-facing behavior of the
  app should be captured here.

  HOW TO MAINTAIN:
  - When a new feature or behavior is added, append it to the relevant section.
  - When a behavior is changed, update the requirement in place.
  - When a requirement is removed, delete it (don't comment it out).
  - Each requirement should be specific and testable (manually or automatically).
  - Reference test functions where automated coverage exists.
  - Keep requirements grouped by area, not by chronology.
-->

### Menu Bar

- The app runs as a menu bar-only macOS app (no dock icon).
- A status item displays a globe icon followed by the current time of the favorite timezone (e.g. `🌐 14:32`).
- The displayed time updates once per minute, aligned to the next minute boundary.
- The app must not poll or do work between minute ticks (idle CPU ~ 0).

### Dropdown Menu

- Clicking the status item opens a dropdown menu showing all saved timezone entries.
- Each entry row displays: custom label, current local time, and relative day indicator.
- Relative day is one of: `Today`, `Tomorrow`, `Yesterday`, or blank if more than 1 day apart.
  - Tests: `relative_day_is_today_when_dates_match`, `relative_day_is_tomorrow_across_month_and_year_boundary`, `relative_day_is_yesterday_across_month_and_year_boundary`, `relative_day_is_blank_for_non_adjacent_dates`, `relative_day_is_tomorrow_for_paris_to_adelaide_evening`
- Time is formatted as `HH:MM` (24-hour).
  - Test: `format_produces_hhmm_time`
- The menu includes a `Settings...` item and a `Quit` item (with Cmd+Q shortcut).

### Settings Window

- The window title is "Cadran Settings".
- The window is not resizable; it auto-sizes to fit its content.
- The "Launch at Login" checkbox and footer (version + GitHub link) are always pinned to the bottom of the window.
- Entries can be added via a `+` button (up to 15 entries max). The `+` button is disabled at the limit.
- Entries can be removed via a per-row `x` button.
- Each entry row has: a star (favorite) toggle, a drag handle, a label text field, a timezone combo box, and a delete button.
- Label and timezone fields support standard Edit menu shortcuts (Undo, Redo, Cut, Copy, Paste, Select All).
- Tab order cycles through label and timezone fields in row order.
- Entries are reorderable via drag-and-drop on the drag handle.
- Changes are saved immediately on every edit (no explicit save button).
- Saving triggers an immediate refresh of the menu bar display.
- The settings window is reused if already open (not recreated).
- Clicking outside text fields dismisses the field editor (clicking the content view background).
- The footer shows `Cadran V1.0` and a clickable `github.com/pior/cadran` link that opens the browser.

### Favorite

- Exactly one entry is the favorite at all times (shown in the menu bar).
- Clicking a star makes that entry the favorite and unsets all others.
- If the favorite entry is deleted, the first remaining entry automatically becomes the favorite.
- If no entries are saved, the menu bar shows nothing (no favorite).

### Timezone Picker (Combo Box)

- The combo box supports typing to search and shows a popup with matching results.
- Search matches against: IANA timezone IDs, city names (extracted from IANA ID), timezone abbreviations (e.g. `JST`, `EST`), abbreviation families (e.g. `ET` matches both `EST` and `EDT` zones), and fuzzy/subsequence matching on city names (e.g. `newyork`, `ny`, `rga`).
  - Tests: `completions_for` and `combo_items` test suite in `search.rs`
- UTC offset inputs are supported (e.g. `UTC+5:30`, `GMT-8`, `+0530`). They are normalized to canonical form `UTC+HH:MM`.
  - Tests: `normalizes_utc_offsets`, `normalizes_bare_utc_and_gmt`, `resolves_fixed_offset_timezone`
- Offset suggestions are provided for every 15-minute increment from UTC-12:00 to UTC+14:00.
- On pressing Enter with a single completion match, the value is committed and the popup dismissed.
- On ending editing, if there is a single match, it is auto-committed.
- IANA IDs are canonicalized on edit end (e.g. `america/new_york` becomes `America/New_York`).
  - Test: `resolves_iana_id_with_canonical_case`
- Invalid timezone inputs are silently ignored (entry not saved).
  - Tests: `rejects_invalid_offsets`, `rejects_empty_and_whitespace`, `rejects_nonsense_input`

### Persistence

- Timezone entries are stored in macOS NSUserDefaults as JSON under the key `timezone_entries`.
- Each stored entry has: `label`, `iana_id`, and `favorite`.
- Entries persist across app restarts.
- On first launch with no saved data, a set of default example entries is created and saved.

### Lightweight Behavior

- No webview, Electron, or heavy runtime.
- Native Rust binary using `objc2` / `objc2-app-kit` for all macOS APIs.
- `jiff` for timezone calculations, using the system IANA timezone database.
- No background polling beyond the minute-aligned timer.
