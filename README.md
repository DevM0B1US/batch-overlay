# Batch Overlay

A lightweight batch image overlay tool built with **Tauri v2 + Rust + Svelte 5 + TypeScript + Vite**.

Point it at a folder of images, pick a template (transparent PNG works best), hit one button — every image in the folder gets the template composited on top and saved to the output folder. The UI never freezes: all image work runs on parallel Rust worker threads with live progress.

## Features

- **Select Source Folder** — native folder picker; shows the number of supported images found (`.jpg`, `.jpeg`, `.png`).
- **Select Template** — file picker filtered to PNG/JPEG; live preview rendered on a transparency checkerboard.
- **Start Batch Processing** — processes every image with proper alpha blending, entirely in Rust (`image` crate + `rayon`), with per-file progress (`current / total + filename`) and a summary at the end.

### Settings

| Setting | Options |
| --- | --- |
| Output format | PNG (lossless) or JPEG |
| Quality | 0–100 slider, used for JPEG only |
| Output folder | Defaults to an `output` subfolder inside the source folder; browse or type any path |
| Template placement | Center (native size, scaled down if larger) / Stretch (fills the image, aspect ignored) / Fit (largest uniform scale that fits) |
| Resize source images | Optional; max width/height, aspect preserved, never upscales |
| Filename suffix | Default `_overlay` → `photo_overlay.jpg` |
| Overwrite | On: replace existing files. Off: skip files already in the output folder |

Files are decoded by content (not extension), so mislabeled files still process; anything that fails is reported per-file in the summary without stopping the batch.

## Platform support

- **Windows** 10/11 (WebView2, MSI/NSIS installers)
- **Linux** — Fedora (Wayland + X11) and other glibc distros (deb / rpm / AppImage bundles)

## Getting started

Prerequisites:

- [Node.js](https://nodejs.org) 18+ and npm
- [Rust](https://rustup.rs) (stable)

Platform-specific system deps (per the [Tauri v2 guide](https://v2.tauri.app/start/prerequisites/)):

- **Windows:** Microsoft Visual Studio C++ Build Tools; WebView2 is preinstalled on Win10/11.
- **Fedora:**
  ```sh
  sudo dnf install webkit2gtk4.1-devel build-tools curl wget file \
       libappindicator-gtk3-devel librsvg2-devel gcc gcc-c++ make
  ```
- **Debian/Ubuntu:**
  ```sh
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
       libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

Then:

```sh
npm install

# development (hot reload)
npm run tauri dev

# production build + bundles
npm run tauri build
```

Installers land in `src-tauri/target/release/bundle/` (`msi`/`nsis` on Windows, `deb`/`rpm`/`appimage` on Linux).

## How it works

```
batch-overlay/
├── src/                      # Svelte 5 frontend
│   ├── App.svelte            # 3-step main flow + progress + summary
│   ├── lib/api.ts            # typed invoke()/event() wrappers
│   └── lib/components/       # SettingsPanel, ProgressBar
└── src-tauri/
    └── src/
        ├── overlay.rs        # pure engine: resize, placement, blend, encode (+ unit tests)
        ├── commands.rs       # IPC: scan, preview, batch (spawn_blocking + rayon + events)
        └── lib.rs            # Tauri builder, plugins (dialog, fs)
```

- The batch runs on a blocking worker thread; each file is decoded → (optionally resized) → template composited with `image::imageops::overlay` alpha blending → encoded → written.
- Progress is emitted as a `progress` event per completed file; results are returned as a typed summary.
- All filesystem access lives in Rust commands; the webview only receives a base64 preview of the chosen template.

## Verification

The engine is dependency-free from Tauri and unit-tested (`placement`, `blend`, `resize`, `encode` round-trips), plus an end-to-end pipeline test that processes real generated JPEG/PNG files.

## License

All rights reserved. Add a LICENSE if you plan to distribute.
