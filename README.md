# Driver Updater

A Windows desktop app that scans the drivers installed on your PC and tells you
which ones have updates. Rust backend, Tauri 2 + React interface.

![Driver Updater showing scan results](docs/screenshot.png)

*Screenshot uses sample data.*

## What it does

- **Inventories every installed driver**: device, provider, version, date, signer, INF and hardware ID.
- **Asks Windows Update what is newer**, including the optional driver updates
  Windows tucks away in Settings. Each offer is matched to the installed driver
  by hardware ID and only shown as an update if it is actually newer.
- **Flags drivers worth checking by hand**: old graphics, network, audio, chipset
  and storage drivers where the vendor (NVIDIA, AMD, Intel, or your PC maker)
  likely has something newer than Windows Update. It links to the vendor's
  official download page; nothing is scraped or downloaded from third parties.
- **Surfaces problem devices**: missing drivers and Device Manager error codes.
- **Exports** the report as CSV or JSON.

It is read-only. It never installs, removes or changes a driver, and it does not
need administrator rights. To install an update it found, use the
**Open Windows Update** button.

## Install

Every push builds a Windows installer in GitHub Actions: open the latest
**Build** run under the Actions tab and download `driver-updater-installer`.
The installer is not code-signed yet, so SmartScreen will warn on first run.

## Develop

Requires [Rust](https://rustup.rs), Node 22+ and, on Windows, the
[Tauri prerequisites](https://tauri.app/start/prerequisites/).

```sh
npm install
npm run tauri dev      # run the app (Windows)
npm run tauri build    # build the installer (Windows)
npm run dev            # UI only, in a browser, on sample data (any OS)
cargo test -p driver-core
```

## Layout

| Path | What |
| --- | --- |
| `crates/driver-core` | All the logic. `platform/` talks to Windows (WMI for the inventory, the Windows Update Agent API for available updates); the rest is plain Rust with tests that run on any OS. |
| `src-tauri` | The Tauri shell: four commands exposing `driver-core` to the UI. |
| `src` | The React interface. |
