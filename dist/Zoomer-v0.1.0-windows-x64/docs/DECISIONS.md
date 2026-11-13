# Zoomer implementation decisions

## D001 — 2026-11-10: Windows native stack

One Rust 2024 Cargo package, winit 0.30 ApplicationHandler, glutin 0.32 WGL,
glutin-winit 0.5 surface helpers and glow 0.18. Windows desktop dependencies
are target gated. The portable library contains camera math, configuration,
frame validation and lifecycle transitions. No web runtime, async runtime,
network client, screen export or startup registration is included.

Direct WGL display construction plus glutin-winit's GlWindow avoids the
infallible config-picker callback in DisplayBuilder: an empty configuration
list returns an actionable error instead of panicking. This retains the
specified stack. Signatures were checked against
[glutin 0.32.3 source](https://github.com/rust-windowing/glutin/blob/v0.32.3/glutin/src/display.rs),
[ConfigTemplateBuilder](https://docs.rs/glutin/latest/glutin/config/struct.ConfigTemplateBuilder.html)
and [GlWindow](https://docs.rs/glutin-winit/0.5.0/glutin_winit/trait.GlWindow.html).
Full integration compilation now passes with the root Cargo.lock. The initial
registry restriction was resolved as recorded in D007 below.

## D002 — 2026-11-10: Win32 bindings and resources

Use windows-sys 0.61 with narrow Win32 features rather than higher-level
windows bindings. It supplies GDI, monitor geometry, foreground restoration,
message boxes and a named mutex. Small RAII guards own DCs/bitmaps and restore
selected objects on error paths. No capture handles cross threads. This module
compiled locally. GetDIBits runs after deselecting the bitmap and produces
packed top-down BGRA; the shader ignores GDI's unused alpha byte.

Use build-only embed-resource 3 for the PerMonitorV2/asInvoker manifest and
original procedural ICO. It is cached on this host and adds no runtime library.
The generated icon is included; generation is optional. The actual build script
successfully compiled these resources in the isolated Windows verification build.

## D003 — 2026-11-10: Screenshot and GL lifetime

Capture once on a short-lived worker after explicit activation. Select the
physical monitor at input time. Return only owned bytes/geometry tagged by
generation. Coalesce in-flight activations and reject obsolete results. Upload
once, drop CPU pixels, draw/swap while hidden, then show. Borderless fullscreen
uses the matching winit monitor; mismatched physical framebuffer sizes fail
safely. Winit cursor coordinates already use physical pixels, so do not apply
a second DPI multiplier.

Before GDI capture, DwmFlush synchronizes the compositor with the previous
viewer's hide/destruction. This avoids capturing a stale overlay on rapid
reactivation. Synchronization failure is recoverable; it adds at most the
compositor synchronization wait to explicitly triggered capture, with no idle work.

Destroy the viewer, GL texture/program/VAO, context and surface on dismiss.
This favors simple lifetimes and low idle allocation over retaining a context.
Activation cost is NOT benchmarked; consider reuse only after measurements.
Frames above 512 MiB or GL_MAX_TEXTURE_SIZE are rejected rather than tiled.
The M0 temporary test-pattern window was superseded by the real capture path
in source; its separate GUI verification was not possible on this build.

## D004 — 2026-11-10: Motion and mirror

Each eased scale step adjusts translation around the input anchor. Mirror
flips texture coordinates around the image center, leaving the geometric
footprint unchanged; inverse mapping applies the same flip. Tests retain
the anchor in both modes. Pan velocity uses an exponential average of mouse
samples, capped at 10,000 px/s. Friction integrates analytically to the exact
1 px/s stop threshold. A pause of 80 ms before release cancels stale inertia.
Zero pan half-life disables inertia. Motion dt is capped at 50 ms after stalls.

WaitUntil schedules roughly 60 Hz animation with optional vsync, returning to
Wait on convergence. Flashlight redraws on cursor/input events. No timer/draw
remains while hidden. Alt+Tab dismisses without forcing focus back; Escape and
the hotkey attempt best-effort restoration.

## D005 — 2026-11-10: Configuration and shortcut

Missing config uses in-memory defaults. Files are never created/rewritten.
Invalid startup config warns and uses defaults; invalid reload keeps all prior
settings. Register the replacement shortcut before unregistering the old one.
Portable grammar: Ctrl/Control, Alt, Shift, Super and one A-Z/0-9/F1-F24 key.
Native parsing/registration is an additional gate. Dismiss the viewer before
showing modal error dialogs, so warnings cannot hide behind a topmost overlay.

Select current stable global-hotkey 0.10 (reference was 0.9), as reported by
[official crate documentation](https://docs.rs/global-hotkey/latest/global_hotkey/).
The upstream Windows implementation uses MOD_NOREPEAT and sleeps between
release-state probes instead of spinning while held. This probing is limited
to a held key; Zoomer's idle event loop does not poll. Callbacks use
EventLoopProxy. The selected crate is now downloaded, locked and compiled.

## D006 — 2026-11-10: Initial release gate (historical blocker resolved)

Initially the sandbox could not reach crates.io and lacked graphics/hotkey packages.
Do not fabricate Cargo.lock, ZIPs, GUI results, size numbers or performance
evidence. A temporary isolated manifest references the actual library and
Win32 capture source using cached crates. It is not a second product package
or full-app check. Workflows require a real checked-in lockfile, run all checks
and create only an unpublished draft until interactive acceptance. Project
licensing is left to the owner.

## D007 — 2026-11-10: Registry/cache access and exact dependencies

An approved Cargo fetch outside the network sandbox succeeded. Use the writable
workspace CARGO_HOME (.local/cargo) rather than changing the read-only sandbox
ACL on the user's existing cache. Subsequent full builds and checks pass offline.
The root lock pins dependencies; the correct Rust minimum is 1.90, required by
tray-icon/muda. No stack changes or global proxy/security changes were needed.

## D008 — 2026-11-10: Historical window events are not current geometry

Actual hotkey capture rendered correctly but immediately dismissed because an
old queued 1440x753 startup resize arrived after the framebuffer reached
1920x1080 fullscreen. Compare current framebuffer/physical monitor geometry
instead of blindly dismissing on the event payload or every DPI event. The
generated-pixel startup regression reproduced the bug and passes with the fix;
the user confirmed the corrected real activation works. GPU readback also
verifies channels/orientation, mirror and flashlight. Native GUI tests run in
isolated child processes because winit supports one event loop per process.

## D009 — 2026-11-10: Explicitly requested per-user startup

After confirming operation, the user requested bundling and Windows startup.
Install a stable copy under %LOCALAPPDATA%/Programs/Zoomer and register only a
quoted per-user HKCU Run value. The app itself never silently registers startup.
This is an opt-in local setup, requiring no installer framework, service,
administrator privilege or scheduled task. Keep portable ZIP behavior unchanged
and document how to remove the single Run value. Next-sign-in behavior remains
a manual check rather than forcing a logout to test it.
