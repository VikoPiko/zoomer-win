# Zoomer

A small Windows 10/11 x64 utility that freezes the monitor under the pointer
so you can zoom, pan and annotate it. It runs from the system tray and is
activated with a global shortcut (default **Ctrl+Alt+Z**).

Written in Rust with winit, glutin and glow for rendering, native Win32 GDI
for capture, and a tray menu. Inspired by [tsoding/boomer](https://github.com/tsoding/boomer).

## Features

- Freeze the monitor under the pointer and zoom smoothly around the cursor
- Pan by dragging, with damped inertia on release
- Temporary pen with five colors, adjustable width, undo and clear
- Radial flashlight and horizontal mirror
- Configurable global shortcut, zoom limits, animation timing and filtering
- Runs as a tray app with ordinary user privileges
- Fully local: no network access, telemetry, or saved screenshots

## Installation

### Portable release (recommended)

Prebuilt Windows x64 bundles are in [`dist/`](dist/), one folder per version:

| Version | Folder | Notes |
|---|---|---|
| 0.2.0 | [`dist/Zoomer-v0.2.0-windows-x64`](dist/Zoomer-v0.2.0-windows-x64) | Adds the temporary pen |
| 0.1.0 | [`dist/Zoomer-v0.1.0-windows-x64`](dist/Zoomer-v0.1.0-windows-x64) | Initial release |

1. Download or clone the repository and open the folder for the version you want.
2. Run `Zoomer.exe`. No Rust toolchain or installer is needed.
3. Find the Zoomer icon in the system tray (it may be under the Windows
   overflow menu) and press **Ctrl+Alt+Z** to capture the current monitor.

A second launch reports the existing instance instead of starting another.
Each bundle also contains `config.example.toml`, the lock file, third-party
notices and further documentation in its `docs/` folder.

Tagged releases additionally publish a ZIP with a SHA-256 checksum through the
GitHub release workflow. To verify a download:

```powershell
(Get-FileHash .\Zoomer-v0.2.0-windows-x64.zip -Algorithm SHA256).Hash.ToLower()
# compare against the contents of Zoomer-v0.2.0-windows-x64.zip.sha256
```

> Binaries are not code-signed, so Windows SmartScreen may show a warning on
> first launch.

### Run at sign-in (optional)

The portable bundle does not register itself for startup. To start Zoomer when
you sign in, copy the folder somewhere permanent and add a per-user Run entry
(no administrator rights required):

```powershell
$dir = "$env:LOCALAPPDATA\Programs\Zoomer"
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item .\dist\Zoomer-v0.2.0-windows-x64\Zoomer.exe $dir
Set-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' `
  -Name Zoomer -Value "`"$dir\Zoomer.exe`""
```

To undo, disable Zoomer under Windows **Startup Apps**, or run:

```powershell
Remove-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name Zoomer
```

This only removes the startup entry. Quit a running instance from its tray menu.

### Build from source

Requirements:

- [Rust stable](https://rustup.rs/) 1.90 or newer
- Visual Studio C++ Build Tools with the MSVC x64 toolchain and the Windows SDK

```powershell
git clone https://github.com/VikoPiko/zoomer-win
cd zoomer-win
cargo build --release --locked --target x86_64-pc-windows-msvc
```

The executable is written to `target/x86_64-pc-windows-msvc/release/zoomer.exe`.
Release builds use the Windows GUI subsystem; debug builds keep console
diagnostics. Always pass `--locked` so the dependencies in `Cargo.lock` are used.

## Usage

Launch Zoomer, then press the global shortcut (or choose **Capture screen** in
the tray menu). The monitor under the pointer is frozen into a viewer. Press
**Esc**, **Q** or the shortcut again to dismiss it and return to the tray.

### Controls

| Input | Action |
|---|---|
| Ctrl+Alt+Z (global) | Capture and open; dismiss when already viewing |
| Wheel | Smooth zoom around the pointer |
| + / =, - | Zoom (centered when the pointer is outside the viewer) |
| Left drag | Pan, with inertia on release |
| Right drag | Pan in any mode |
| P / PEN P button | Toggle pen mode; left drag draws while enabled |
| 1–5 / color dots | Red, yellow, green, blue, white (pen mode) |
| [ and ] / toolbar − and + | Pen width, 2–24 px |
| Ctrl+Z / Undo | Remove the last stroke |
| Delete or Backspace / Clear | Remove all ink |
| 0 | Reset camera, inertia, mirror, flashlight and radius (ink is kept) |
| M | Mirror horizontally around the image center |
| F | Toggle the radial flashlight |
| Ctrl+Wheel | Change flashlight radius without zooming |
| R | Reload configuration |
| Esc or Q | Dismiss to tray |

Tray menu: **Capture screen**, **Reload configuration**, **About**, **Quit**.

### Pen

Enable the pen with **P**, then left-drag to draw. Defaults are red and 4 px.
Width is measured in screen pixels when a stroke starts and then scales with
the image. A single click makes a dot. Press **P** again to return left-drag to
panning; existing marks remain. Ink moves with the image and is discarded when
the viewer is dismissed; nothing is exported or saved. Each snapshot holds up
to 512 strokes / 32,768 points.

## Configuration

Copy [`config.example.toml`](config.example.toml) to
`%APPDATA%\Zoomer\config.toml` (create the folder), edit it, then press **R**
in the viewer or use **Reload configuration** from the tray.

```toml
[hotkeys]
activate = "Ctrl+Alt+Z"

[camera]
min_scale = 0.5
max_scale = 16.0
wheel_factor = 1.20
zoom_half_life_ms = 90
pan_half_life_ms = 120   # 0 disables release inertia

[flashlight]
radius_px = 240.0
min_radius_px = 40.0
max_radius_px = 1200.0
outside_brightness = 0.28

[appearance]
filter = "linear"        # or "nearest"
```

- Without a config file, built-in defaults are used. Zoomer never creates or
  overwrites the file.
- Invalid settings at startup fall back to defaults; an invalid reload keeps
  the previous configuration. Unknown fields are errors, to catch typos.
- **Shortcut syntax:** modifiers `Ctrl`/`Control`, `Alt`, `Shift`, `Super`
  followed by one `A-Z`, `0-9` or `F1-F24` key, joined with `+`
  (for example `Ctrl+Shift+F8`). Windows may reject reserved or already-used
  combinations; the tray **Capture screen** entry always remains available.
- **Limits:** scales must bracket 1 within 0.01–1024; `wheel_factor` 1.001–4;
  `zoom_half_life_ms` 1–2000; `pan_half_life_ms` 0–2000; flashlight radii
  positive, ordered and at most 32768; `outside_brightness` 0–1. All numbers
  must be finite.

## Privacy and security

- Snapshots are captured once per activation, kept in memory and GPU texture
  only, and never written to disk, the clipboard or the network.
- No keyboard hooks, services, scheduled tasks, auto-update or telemetry.
- Runs with ordinary user privileges (PerMonitorV2 DPI aware manifest).

## Troubleshooting

- **Shortcut doesn't work:** another app may own it. Change `activate` in the
  config and reload, or use tray **Capture screen**.
- **Graphics error:** update your GPU driver (desktop OpenGL 3.3 is required).
  Capture and viewer errors leave the tray process running.
- **Display layout changed:** dismiss and capture again.
- **Blank capture:** protected content, the secure desktop, the lock screen and
  some exclusive-fullscreen games cannot be captured through GDI.
- **Frame rejected:** captures above 512 MiB or the GPU texture size limit are
  refused.
- **Diagnostics:** set `ZOOMER_DIAGNOSTICS=1` before launch and redirect stderr
  to a file. It logs lifecycle events only, never pixels or screen content.

## Development

Run the same checks as CI:

```powershell
cargo fmt --all -- --check
cargo check --all-targets --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo run --locked --bin zoomer
```

### Packaging

After a release build, create a portable folder and ZIP with a SHA-256 file in
`dist/`. The version must match `Cargo.toml`:

```powershell
pwsh -File scripts/Package.ps1 -Version 0.2.0
```

Pushing a `v*` tag runs the Windows release workflow, which repeats the checks,
packages the build and creates a draft release for manual review. If the
default Cargo registry is unreachable, see the registry troubleshooting guide
in [`docs/`](docs/REGISTRY_TROUBLESHOOTING.md).

Further notes (build and release process, dependency rationale, design
decisions, Windows smoke-test checklist and the 0.2.0 pen update) are in
[`docs/`](docs) and
[`BUILD_RELEASE.md`](BUILD_RELEASE.md).

## Known limitations

- Windows 10/11 x64 only; binaries are unsigned and there is no installer.
- Mixed-DPI setups, negative monitor origins and long-running resource
  stability have had limited manual testing.

## License and acknowledgements

Zoomer is released under the [MIT License](LICENSE). Third-party dependency
licenses are listed in the
[third-party notices](THIRD_PARTY_NOTICES.md).
Zoomer is an independent implementation inspired by
[tsoding/boomer](https://github.com/tsoding/boomer) and is not an official port.
