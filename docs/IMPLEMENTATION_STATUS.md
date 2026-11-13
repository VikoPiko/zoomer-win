# Implementation status — 2026-11-10

**Current update: 0.2.0 adds the user-requested temporary pen.** Full checks,
15 core tests and two explicit Windows tests pass; the user confirmed physical
drawing in the installed build. See [PEN_UPDATE.md](PEN_UPDATE.md)
for controls, implementation, test evidence and manual limits. Current artifacts
use Zoomer-v0.2.0-windows-x64 names; the records below describe the original 0.1.0
delivery and registry/startup troubleshooting history.

## Previous 0.1.0 delivery record

**Windows build and local bundle available. Basic real activation confirmed by the user.**
The complete release acceptance matrix is not yet closed, and no public release
or remote CI success is claimed.

## Registry diagnosis and resolution

The original EACCES was an environmental restriction, not an application stack
problem. Sandboxed HTTPS connections to crates.io failed; the existing user
Cargo registry allowed the sandbox group read/execute access only. No overriding
Cargo configuration, environment proxy or Windows proxy was found. The original
cache lacked the selected graphics/hotkey crates (it had tray-icon 0.25.1 and
embed-resource 3.0.12).

An approved `cargo fetch --target x86_64-pc-windows-msvc` outside the network
sandbox succeeded with CARGO_HOME set to the writable workspace `.local/cargo`.
It generated the real root Cargo.lock (185 packages in the full lock graph).
The Windows metadata graph includes the app and 67 registry dependencies.
All subsequent checks and builds passed offline in the normal sandbox. No
cache ACL, firewall, proxy, execution policy or global Cargo configuration was
changed. See REGISTRY_TROUBLESHOOTING.md for commands and fresh-cache requirements.

The declared minimum Rust version was corrected from 1.88 to 1.90, matching
tray-icon 0.26.1 / muda 0.21.2. The installed stable compiler is Rust 1.99.1
(58cf8f9c3, 2026-10-13), Cargo 1.99.1, Windows MSVC x64.

## Actual defect found and fixed

The user initially reported a blink instead of a persistent viewer. Opt-in
lifecycle diagnostics showed successful hotkey delivery, native capture and
rendering, followed by a queued startup Resized event carrying 1440x753 even
though the live fullscreen framebuffer was already 1920x1080. The old handler
interpreted that historical size as a display change and immediately dismissed.

The handler now checks the live framebuffer. DPI events similarly validate
current physical monitor geometry instead of dismissing during initial
placement. Genuine changed physical geometry still dismisses safely. The
native regression test reproduced the original failure and passes with the
fix. The user then reported: "Yes it is working" and requested packaging and
startup registration. Diagnostics show repeated real activations on both
1920x1080 monitors (origins 0,0 and 1920,0), without the stale-resize dismissal.
Individual control-by-control signoff is not implied by that general report.

Opt-in ZOOMER_DIAGNOSTICS writes lifecycle metadata to stderr only. The test
launcher redirected it into ignored .local files. No screen pixels are logged,
saved, exported or sent over a network. Normal launches create no log file.

## Validation actually executed

With `$env:CARGO_HOME = Join-Path (Get-Location) '.local/cargo'`:

| Command | Result |
|---|---|
| cargo fmt --all -- --check | PASS |
| cargo check --all-targets --locked --offline | PASS, entire Windows application |
| cargo test --locked --offline | PASS: 10 core tests; 2 desktop tests intentionally ignored in this default run |
| cargo clippy --all-targets --locked --offline -- -D warnings | PASS |
| cargo build --release --locked --offline --target x86_64-pc-windows-msvc | PASS |
| Explicit release desktop tests below | PASS: 2 tests, including child-process runs |

```powershell
cargo test --release --locked --offline --target x86_64-pc-windows-msvc --bin zoomer -- --ignored --nocapture --test-threads=1
```

The explicit tests ran outside the sandbox on the interactive Windows desktop:

- Hidden generated-color test: real GL context/shader compilation, BGRA upload,
  orientation/channel order, opaque output despite GDI-style zero alpha, mirror
  mapping and flashlight brightness, checked by GPU readback.
- Startup event regression: briefly show generated pixels, process real queued
  creation/fullscreen events, assert the overlay remains visible, then dismiss.
  A fresh child process per test respects winit's one-event-loop-per-process rule.

Neither test captures desktop content or injects keyboard/mouse input. Actual
GDI capture was triggered by the user's hotkey actions, not by a test bypass.
Core tests cover camera anchors/mirror, reversal, finite bounds, convergence,
frame-rate-independent friction, disabled inertia, frame layouts/overflow,
negative physical coordinates, configuration defaults/errors/file preservation,
shortcut syntax and stale capture generations.

The direct UI automation helper remained unavailable: native pipe connection
returned OS error 2 after retry and session reset. The alternate app-control
inventory reported the same missing pipe. Therefore agent-driven interactive
wheel/drag/tray testing was not possible. This is separate from the two
successful purpose-built native tests and the user's real activation check.

## Artifacts and packaging

- Version: 0.1.0, Windows x86_64-pc-windows-msvc, optimized/stripped thin-LTO build.
- Build EXE: target/x86_64-pc-windows-msvc/release/zoomer.exe, **958,464 bytes**.
- EXE SHA-256: 196d37df1d8e6106bb370343018a113aafdd4a18689b1f391ecfe8518d08b7f7.
- Portable directory: dist/Zoomer-v0.1.0-windows-x64/, named Zoomer.exe.
- ZIP: dist/Zoomer-v0.1.0-windows-x64.zip; SHA-256 in the adjacent .sha256 file.
- Bundle contains EXE, README, build instructions, Cargo.lock, example config,
  dependency notices and documentation. It contains no screenshots or live config.
- Notices for all 67 Windows registry dependencies (including build tools) were
  collected from the downloaded locked sources; the generation script is included
  in the source. The project is licensed under MIT (see LICENSE).
- The local PowerShell policy is AllSigned. Packaging uses reviewed inline native
  commands equivalent to scripts/Package.ps1; no execution-policy bypass is used.
- No Git repository/remote was present, so no commit, tag or remote CI run exists.
  Cargo.lock and Windows CI/draft-release workflows are supplied for version control.

## Requested startup setup

The user explicitly requested startup registration after confirming the fix.
The stable installation location is %LOCALAPPDATA%\Programs\Zoomer\Zoomer.exe.
Per-user startup uses the quoted EXE path in the Zoomer value under
HKCU\Software\Microsoft\Windows\CurrentVersion\Run. No administrator privilege,
service or scheduled task is required. The application does not self-register.
Registration/file/hash verification is recorded in the local release report;
a new sign-in has not been forced or tested. README documents removal of just
this startup entry. This opt-in setup supersedes the original no-startup scope
only for the user's explicit request.

## Host and performance evidence

- Windows 11 Pro 10.0.26200, Intel Core i7-8700K @ 3.70 GHz.
- NVIDIA GeForce GTX 1080 Ti, driver 32.0.15.6094.
- Two monitors observed in real capture diagnostics: 1920x1080 at (0,0) and
  (1920,0). Scaling/refresh and negative-origin configurations are not verified.
- Baseline pre-fix release idle sample, using a monotonic clock: 30.0488 seconds,
  CPU delta 0.0 seconds, 143 handles before/after, 1 thread after settling,
  working set 11,558,912 bytes, private bytes 1,871,872. This baseline is not
  presented as a 100-cycle leak test or as a final-build performance guarantee.
- Startup latency, exact capture latency, 4K, smoothness/frame pacing, peak
  memory and 100-cycle GDI/USER stability remain NOT MEASURED.

## Remaining acceptance work

Run the outstanding rows in WINDOWS_SMOKE_TEST.md: individually verify all
controls, negative origins, 100/125/150% mixed DPI, 4K, 100 cycles, hotkey
collision/reload rollback, tray Quit, GPU/capture failure recovery, display
removal, lock/sleep/resume, and next-sign-in startup. The minimum requirement
for automated UI coverage is a working Windows Computer Use native helper;
these checks can also be done manually without changing the app architecture.
No broader release acceptance or unexecuted test is marked passing.

## Files changed in the troubleshooting continuation

- Added root Cargo.lock and docs/REGISTRY_TROUBLESHOOTING.md.
- Updated Cargo.toml (accurate Rust minimum), src/app.rs (geometry fix and native
  regression), src/graphics.rs (GPU smoke test), src/platform.rs (opt-in lifecycle
  diagnostics and test process isolation).
- Added scripts/collect-notices.mjs and generated full THIRD_PARTY_NOTICES.md.
- Updated scripts/Package.ps1, README.md, BUILD_RELEASE.md, IMPLEMENTATION_TASKS.md,
  docs/DECISIONS.md, DEPENDENCIES.md, WINDOWS_SMOKE_TEST.md and this status file.
- Added ignored dist artifacts and workspace-local .local Cargo cache/logs.
- Installed the user's requested per-user startup copy/entry outside the source
  folder only after the explicit startup request; no existing project files deleted.

## Final artifact / installation verification

ZIP created and inspected: 499,372 bytes; SHA-256
c9869b3f1473a61e3541e15360fb06ae0b36542719cedea5cf6f8755e16e8921.
The ZIP's embedded EXE matches the tested build and installed EXE hash above.
The requested HKCU Run command and installed process path were verified.
Installed process responds; next-sign-in behavior has not been forced/tested.
The final installed-build quiet sample was 30.0116493 seconds,
CPU delta 0.015625 seconds, handles 304
to 304, working set 66457600 bytes,
private bytes 82956288. It is not a 100-cycle leak test.
See dist/RELEASE_REPORT.md for the local delivery record; the portable ZIP
contains the pre-installation documentation snapshot and no personal startup data.
