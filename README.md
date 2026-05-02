# Cadran

A lightweight multi-timezone macOS menu bar app.

<!-- ![menu bar screenshot placeholder](docs/screenshot.png) -->

## Features

- Lives in the menu bar with no dock icon
- Shows the current time for your primary timezone at a glance
- Dropdown displays multiple timezones with labels, city names, and relative day (Today/Tomorrow/Yesterday)
- Updates every minute, idle between updates
- Persists your timezone list across restarts
- Native macOS app — no Electron, no webview, no heavy runtime

## Install

Requires Rust 1.75+.

```
git clone https://github.com/pior/cadran.git
cd cadran
cargo run
```

## Status

Early development. Preferences UI and timezone search are not yet implemented.

## License

[MIT](LICENSE)
