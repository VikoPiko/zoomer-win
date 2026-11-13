# Zoomer

A small Windows 10/11 x64 utility that freezes the monitor under the pointer
for smooth zooming and panning. Rust + winit + glutin/glutin-winit + glow,
native Win32 GDI capture, a tray menu and a configurable global shortcut.

Version **0.2.0** adds a temporary pen: open the snapshot, press **P** (or click
the small **PEN P** button at the top left), then left-drag to circle/draw.
Right-drag still pans. Ink moves with the image and is discarded on dismissal.

**Status: Windows build available; activation and pen drawing confirmed by the user.**
The full application compiles, fifteen core tests pass, and two explicit Windows
desktop tests pass. A startup resize bug that immediately dismissed the viewer
was reproduced and fixed. Broader manual acceptance remains tracked in
[test evidence](docs/IMPLEMENTATION_STATUS.md) and the
[Windows checklist](docs/WINDOWS_SMOKE_TEST.md). Pen implementation and test
evidence are in [the 0.2.0 update](docs/PEN_UPDATE.md); older records describe 0.1.0.

## Build and run

Install [Rust stable](https://rustup.rs/) (minimum 1.90) and Visual Studio C++ Build Tools
with MSVC x64 and the Windows SDK. Build and run as a regular user.

The root Cargo.lock pins dependencies. Use `--locked` for builds. On this
managed host the writable Cargo cache is `.local/cargo`; see
[registry troubleshooting](docs/REGISTRY_TROUBLESHOOTING.md). Once downloaded,
all Windows checks/builds also work with `--offline`.

```powershell
$env:CARGO_HOME = Join-Path (Get-Location) '.local/cargo'
cargo fetch --locked --target x86_64-pc-windows-msvc
cargo fmt --all -- --check
cargo check --all-targets --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo run --locked --bin zoomer
cargo build --release --locked --target x86_64-pc-windows-msvc
```

Build output: `target/x86_64-pc-windows-msvc/release/zoomer.exe`;
the portable ZIP names the executable `Zoomer.exe`.
Release builds use the Windows GUI subsystem; debug builds keep diagnostics.
The embedded manifest requests PerMonitorV2 DPI awareness and ordinary user
privileges. Zoomer does not register itself for startup, create services or
create scheduled tasks. The optional per-user startup setup below requires
an explicit user choice.

After building and completing acceptance, package using
`pwsh -File scripts/Package.ps1 -Version 0.2.0`. It creates a portable ZIP and
SHA-256 in `dist/`. This host prevents unsigned PowerShell scripts from running;
do not weaken system policy for this task. CI uses its configured PowerShell
runner. Packaging commands can be reviewed and executed individually in an
allowed development environment.

For a validated portable ZIP: extract it, run Zoomer.exe, and find its tray
icon (including the Windows overflow menu). Running the executable needs no
Rust installation. A second launch reports the existing instance.

## Optional Windows startup

For this user's requested setup, a stable copy is installed under
`%LOCALAPPDATA%\Programs\Zoomer`, with a `Zoomer` value in
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` pointing to the quoted
Zoomer.exe path. This starts the tray app when that user signs in, with no
administrator privileges. Keep the installed folder in place. The portable
ZIP itself does not create this entry automatically.

To stop launching at sign-in, disable Zoomer in Windows Startup Apps, or run:

```powershell
Remove-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name Zoomer
```

This removes only the startup entry. Quit the running app from its tray menu.
No screenshot or configuration file is added to the installation.

## Controls

| Input | Action |
|---|---|
| Ctrl+Alt+Z globally | Capture/open; dismiss when already viewing |
| Wheel | Smooth zoom around pointer |
| + / =, - | Zoom; center anchor when pointer is outside viewer |
| Left drag | Pan; release with damped inertia |
| P / PEN P button | Toggle pen mode; left-drag draws while enabled |
| Right drag | Pan in either mode |
| 1–5 / color dots | Red, yellow, green, blue, white (pen mode) |
| [ and ] / − and + toolbar buttons | Adjust pen width, 2–24 px at stroke start |
| Ctrl+Z / Undo | Remove the last stroke |
| Delete or Backspace / Clear | Remove all ink |
| 0 | Reset camera, inertia, mirror, flashlight and radius |
| M | Mirror horizontally around image center |
| F | Toggle radial flashlight |
| Ctrl+Wheel | Change flashlight radius without zooming |
| R | Reload configuration |
| Esc or Q | Dismiss to tray |
| Tray menu | Capture screen, Reload configuration, About, Quit |

Escape leaves the tray process alive. Tray Quit unregisters the hotkey and
exits. Alt+Tab dismisses without pulling focus away from the newly selected
app. Snapshot pixels are captured once per activation, uploaded once, and
never written to disk/clipboard or sent over a network. A static/hidden viewer
has no scheduled animation. In-flight capture requests coalesce.

Pen defaults are red and 4 px. Select color/width on the toolbar or with the
shortcuts above; these controls do not change old strokes. Width is measured
in screen pixels at stroke start, then scales with the marked image. Single
clicks make dots. Press P again to return left-drag to panning; existing marks
remain. **0 resets the view but preserves ink**; Delete clears it. Escape/Q or
the global toggle discards the snapshot and all marks. Nothing is exported or
saved. A bounded 32,768 points / 512 strokes per snapshot prevents unbounded
allocation; the toolbar asks you to undo or clear when full.

## Configuration

Copy [config.example.toml](config.example.toml) to
`%APPDATA%\Zoomer\config.toml` (create the folder), edit it, then reload from
the tray or press R. Missing configuration uses defaults in memory. Files
are never created or overwritten by Zoomer. Invalid startup settings warn
and use defaults; invalid reload keeps the entire previous configuration.
Unknown fields are errors to help catch typos.

Shortcut syntax: Ctrl/Control, Alt, Shift, Super, then one A-Z, 0-9 or F1-F24
key separated by `+`, e.g. `Ctrl+Shift+F8`. Windows can reject reserved or
colliding combinations. Replacement registration precedes removing the old
shortcut. Tray Capture remains available after initial registration failure.
No keyboard hooks are used.

Scales must bracket 1 within 0.01..1024; wheel factor 1.001..4; zoom half-life
1..2000 ms; pan half-life 0..2000 ms (0 disables inertia). Flashlight radii
must be ordered, positive and <=32768 px; brightness 0..1. Numeric values must
be finite. Filters are `linear` or `nearest`.

## Troubleshooting and limitations

- Shortcut unavailable: edit the config and reload, or use tray Capture.
- Graphics failure: update the GPU vendor's driver for desktop OpenGL 3.3 and
  retry. Viewer/capture errors leave the tray process alive.
- Display changed: dismiss and capture again. Real physical geometry changes
  dismiss the overlay; old queued startup-size events are safely ignored.
- Protected content, secure desktop, lock screen and some exclusive games can
  return blank pixels or fail GDI capture. Successful GDI calls cannot prove
  protected pixels were captured; no privilege workaround is attempted.
- Frames above 512 MiB or the GPU texture dimension limit are rejected.
- No signed binary, installer, auto-update, export, telemetry or network feature.
- Mixed DPI, negative monitor origins, hardware GL, focus, and 100-cycle
  resource stability need the broader Windows checklist. Hardware GL has
  passed generated-pixel tests; that does not prove every display configuration.

For local troubleshooting only, set `ZOOMER_DIAGNOSTICS=1` before launch and
redirect stderr to a chosen file. It reports registration/capture/geometry
lifecycle events, never pixels or screen content. No diagnostic log is written
by default.

## Development and release

[BUILD_RELEASE.md](BUILD_RELEASE.md) describes quality checks and packaging.
[Dependencies](docs/DEPENDENCIES.md) records purpose and rough impact;
[decisions](docs/DECISIONS.md) records tradeoffs. Windows CI checks formatting,
compilation, tests, Clippy and a release build. Version tags create a draft
candidate after checks. Publish only after manual acceptance and third-party
notice review. The real Cargo.lock is included. The input folder was not a Git
checkout, so no commit or remote CI run is claimed.

Inspired by [tsoding/boomer](https://github.com/tsoding/boomer), independently
implemented; this is not an official port. See
[third-party notices](THIRD_PARTY_NOTICES.md). Project licensing remains an
owner decision; no project license has been assigned.
