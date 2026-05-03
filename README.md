# Cadran

A lightweight multi-timezone macOS menu bar app.

![menu bar screenshot](docs/capture-menubar.png)
![preferences screenshot](docs/capture-preferences.png)

## Features

- Lives in the menu bar with no dock icon
- Shows the current time for your favorite timezone at a glance
- Dropdown displays saved timezones with custom labels, current time, and relative day context
- Marks relative days as Today, Tomorrow, or Yesterday when the saved timezone is on an adjacent local date
- Preferences window for adding, removing, editing, favoriting, and reordering timezone entries
- Timezone picker accepts city-style search results, IANA IDs, UTC offsets, and timezone abbreviations
- Launch at Login toggle in preferences
- Very light CPU usage
- Persists your timezone list across restarts using macOS user defaults
- Native macOS app with no Electron, webview, or heavy runtime

## Install

Requires Rust 1.75+.

```
git clone https://github.com/pior/cadran.git
cd cadran
cargo run
```

## Status

Early development.

## License

[MIT](LICENSE)
