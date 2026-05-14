# Milestone 1: Technical Decisions

## macOS Framework: `objc2` / `objc2-app-kit`

### Decision

Use `objc2` and `objc2-app-kit` for all macOS API interactions.

### Alternatives considered

| Crate | Verdict | Reason |
|-------|---------|--------|
| `cacao` | Rejected | Semi-maintained (stalled ~2024), incomplete NSStatusItem support, would require forking |
| `tao` + `tray-icon` (Tauri ecosystem) | Rejected | Simple tray menus only, no native form controls for preferences window, heavier event loop |
| `cocoa` (servo) | Rejected | Legacy crate, superseded by `objc2` |

### Rationale

- Full coverage for every API we need: NSStatusItem, NSMenu, NSMenuItem, NSWindow, NSTextField, NSButton, NSTableView, NSUserDefaults.
- Actively maintained (~0.6.x as of mid-2025) with frequent releases.
- Zero runtime overhead — compiles to the same calls as native Objective-C.
- No dependency risk — we are never blocked by a wrapper crate missing an API.

### Tradeoff

Verbose. Expect ~2-3x more code than the equivalent Swift app. We mitigate this by building a thin app-specific abstraction layer over the `objc2` calls we actually use, rather than calling raw bindings everywhere.

### Key crates

- `objc2` — Objective-C runtime bindings
- `objc2-foundation` — Foundation framework (NSString, NSArray, NSUserDefaults, etc.)
- `objc2-app-kit` — AppKit framework (NSApplication, NSStatusItem, NSMenu, NSWindow, etc.)

## Timezone Library: `jiff`

### Decision

Use `jiff` for all timezone and date/time operations.

### Alternatives considered

| Crate | Verdict | Reason |
|-------|---------|--------|
| `chrono` + `chrono-tz` | Rejected | DST gap/overlap handling requires manual care, `NaiveDateTime` footgun, chrono-tz bundles entire tzdb via codegen increasing compile time |
| `time` + `time-tz` | Rejected | No built-in IANA timezone support, `time-tz` is third-party and less maintained |

### Rationale

- `Zoned` type natively ties an instant to a timezone — exactly what we need.
- DST transitions are correct by default (no silent truncation of ambiguous/gap times).
- Uses system IANA tzdb on macOS — the OS keeps it updated, no compile-time bundling.
- Supports `strftime`-style formatting including 12-hour (`%I`) and AM/PM (`%p`).
- Minimal dependency footprint and fast compile times.
- Author (BurntSushi) has strong track record for correctness and quality.

### Tradeoff

Pre-1.0 (API may shift). The risk is acceptable given the crate's quality and the limited surface area we use.

### Key operations

- Get current time in a named timezone: `jiff::Zoned::now().with_time_zone(tz)`
- Format time: `strftime`-style with `%H:%M` or `%I:%M %p`
- Relative day: compare `zoned.date()` across timezones

## City/Timezone Search: Custom Lookup

### Decision

Build a small custom lookup (~100 lines) that searches IANA timezone IDs by city name component.

### Rationale

No mature Rust crate exists for city-to-timezone mapping. The IANA timezone IDs already contain city names (e.g., `Asia/Tokyo`, `Europe/Paris`, `America/New_York`). For milestone 1, extracting and fuzzy-matching the city component is sufficient.

### Approach

- At build time or startup, enumerate all known IANA timezone IDs (available from `jiff`).
- Extract the city component (last segment after `/`, with underscores replaced by spaces).
- Match user input against both the city component and the full IANA ID.
- Return matching IANA timezone IDs for selection.

If richer search is needed later (alternate city names, non-English names), the GeoNames cities database is a freely available CSV that can be embedded.

## Persistence: NSUserDefaults

### Decision

Use macOS NSUserDefaults via `objc2-foundation` for all preference storage.

### Rationale

- Native macOS API, no additional dependencies.
- Persists across app restarts automatically.
- Appropriate for the small amount of data we store (~5 timezone entries + format setting).
- Accessible through `objc2-foundation::NSUserDefaults`.

## App Configuration

### Decision

- Menu bar-only app (no dock icon) via `NSApplication::setActivationPolicy` with `NSApplicationActivationPolicyAccessory`.
- Minute-granularity timer via `NSTimer` or `CFRunLoopTimer` for time updates.
- Single `NSStatusItem` with icon + text for the menu bar.

## Summary of Key Dependencies

```toml
[dependencies]
jiff = "0.1"       # Timezone and date/time
objc2 = "0.6"      # Objective-C runtime
objc2-foundation = { version = "0.3", features = ["NSString", "NSArray", "NSUserDefaults", "NSNotification"] }
objc2-app-kit = { version = "0.3", features = ["NSApplication", "NSStatusItem", "NSStatusBar", "NSMenu", "NSMenuItem", "NSWindow", "NSTextField", "NSButton", "NSImage", "NSTimer"] }
```

Version numbers are indicative — pin to the latest compatible versions at project creation time.
