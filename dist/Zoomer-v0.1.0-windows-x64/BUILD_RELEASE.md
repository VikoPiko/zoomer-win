# Build, dependencies, CI, release and performance

> Host result (2026-11-10): registry access is resolved using a writable
> workspace Cargo home and an approved fetch outside the network sandbox.
> Cargo.lock exists; the full check, tests, Clippy and Windows x64 release build
> pass offline. Ten core tests and two explicit desktop tests pass; the user
> confirmed real activation after the startup-resize fix. Broader manual tests
> remain open. See docs/IMPLEMENTATION_STATUS.md and REGISTRY_TROUBLESHOOTING.md.
> Rust minimum is 1.90 because tray-icon/muda require it. Actual choices include
> global-hotkey 0.10, windows-sys 0.61 and build-only embed-resource 3.
> The tag workflow creates a draft candidate; no remote CI run/publication is
> claimed. Package with `pwsh -File scripts/Package.ps1 -Version 0.1.0`.

## 1. Dependency policy

Target current stable Rust and compatible crate versions. Initial verified reference versions (November 2026; verify constraints before coding):

| Role | Crate | Reference version / notes |
|---|---|---|
| OS windows & event loop | `winit` | `0.30.13` stable; do not use 0.31 betas |
| GL context | `glutin` | `0.32.3` |
| GL-winit integration | `glutin-winit` | `0.5.0` |
| OpenGL bindings | `glow` | `0.18.0` |
| Tray | `tray-icon` | `0.26.1`; avoid GTK backend dependencies on Windows; explicit Linux features later |
| Global shortcut | `global-hotkey` | `0.9.0` |
| Windows native APIs | `windows` | stable compatible version, target-gated, minimal Win32 features |
| Config | `serde` + `toml` | choose compatible stable versions |
| Errors | `thiserror` (optional) | only if it actually improves error handling |
| Windows icon/manifest | `winresource` or equivalent (build-only) | include only if required for embedding resources |

Use target-specific Cargo dependencies for Windows-only crates. `Cargo.lock` must be tracked. Do not use `xcap`, `scap`, `wgpu`, `eframe`, `tauri` or `tokio` for MVP unless there is a thoroughly documented, unavoidable obstacle. The purpose is a small render stack and a one-shot native capture. Do not select prerelease crates without a documented reason.

### Initial Cargo release profiles (tune with measurements)

```toml
[profile.dev]
opt-level = 0

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
panic = "abort"
strip = "symbols"

# Optional measured alternative: optimize artifact size
[profile.release-small]
inherits = "release"
opt-level = "s"
```

Do not claim LTO always produces faster builds: thin LTO / a single codegen unit may increase compilation duration. The default *development* profile must prioritize fast edit-compile-test cycles. Compare release artifact size and performance before choosing a different profile.

### Windows subsystem

On release Windows builds, avoid opening a console when the app starts from Explorer. Use a Windows GUI subsystem attribute or properly configured resource/manifest; preserve helpful stderr/console behavior in debug builds. If adding CLI help in the future, account for the GUI subsystem's output behavior.

## 2. Local development

Windows x64 workstation with stable Rust toolchain, MSVC C++ build tools and Windows SDK:

On the implementation host, first set
`$env:CARGO_HOME = Join-Path (Get-Location) '.local/cargo'`.
Dependencies are already cached there; add `--offline` to work without network.
On a fresh machine/cache, perform a permitted `cargo fetch --locked --target
x86_64-pc-windows-msvc` first. Do not change global cache ACLs or security policy.

Explicit desktop tests (not run by headless CI):

```powershell
cargo test --release --locked --offline --target x86_64-pc-windows-msvc --bin zoomer -- --ignored --nocapture --test-threads=1
```

These use generated pixels to test GL upload/channel order/orientation,
mirror/flashlight and survival of queued startup geometry events. They capture
no desktop contents. They do not replace the real hotkey/capture manual matrix.

```powershell
rustup default stable
rustup target add x86_64-pc-windows-msvc
cargo fmt --all -- --check
cargo check --all-targets
cargo test
cargo clippy --all-targets -- -D warnings
cargo run
cargo build --release --locked
```

If toolchain not installed, document precise setup links/steps; do not bypass verification.

## 3. CI workflow

`/.github/workflows/ci.yml`, triggered on pushes and PRs:

1. Windows-latest runner, checkout and stable Rust/MSVC toolchain; cache Cargo dependencies and target directory where worthwhile.
2. `cargo fmt --all -- --check`.
3. `cargo check --all-targets --locked`.
4. `cargo test --locked`.
5. `cargo clippy --all-targets --locked -- -D warnings`.
6. `cargo build --release --locked`.
7. Upload the unmodified portable executable as CI artifact for manual testing; retention limited.

Linux/macOS jobs may compile pure modules later, but should not be presented as integration test evidence for unfinished platform adapters. Ensure Windows native dependency features are gated, or explicitly document why portable compilation is not currently enabled.

## 4. Release workflow

`/.github/workflows/release.yml`, triggered on tags `v*`:

- Run same Windows checks before packaging.
- Build `x86_64-pc-windows-msvc` release, using checked-in lockfile.
- Create `Zoomer-vX.Y.Z-windows-x64.zip` containing `Zoomer.exe`, README, sample config, third-party notices/credits, and license **only if an actual license has been chosen**.
- Generate a SHA-256 checksum file.
- Publish a GitHub Release (after tests complete), with short changelog, SHA-256, supported OS and limitations. Do not claim signed binary if not code-signed. Code signing requires owner's certificate and is outside MVP.
- Distinguish CI build artifact from released, manually tested app if manual GUI tests were not performed.

Optional: Windows ARM64, macOS, Linux packages **after** actual backend verification. Avoid pretending cross-compilation alone proves runtime compatibility.

## 5. Windows manual validation evidence

Create `docs/WINDOWS_SMOKE_TEST.md` containing: Windows version/build, GPU/driver, display configuration, scaling values, test date, branch/commit SHA, and per-test pass/fail/not-run with notes. Include verification of 1x/1.25x/1.5x scale, monitor at negative X, clean Esc, hotkey collision, tray Quit, invalid TOML, rapid reactivation, lock/unlock, failure recovery, protected content limitation.

**Important:** Headless CI cannot prove that a global hotkey, hardware GPU rendering, monitor screenshot and focus behavior work on an actual desktop session. Those require manual interactive testing. If the agent lacks Windows GUI access, say "NOT VERIFIED" rather than "PASS".

## 6. Performance measurements

Do not set hard pass/fail limits without hardware context. Suggested targets:

- Idle process: near-zero CPU, no periodic capture or continuous drawing.
- Typical 1080p activation: ideally under ~250 ms (measure; not a guarantee).
- Rendering: smooth at nominal 60 FPS while actively moving; no unnecessary frames when static.
- Memory: raw 3840x2160 BGRA pixel image ~31.6 MiB; strive to avoid extra retained full-resolution copies and large leaks.
- 100 cycles activate/dismiss: compare private bytes, GDI handles, USER handles, GL objects before/after.
- Package size: record actual EXE and ZIP sizes and compare with prior releases; no speculative 1 MB marketing claim.

Keep measurements in `docs/IMPLEMENTATION_STATUS.md` or a dedicated `docs/PERFORMANCE.md`. Include CPU model, GPU model, OS build, release mode and screen resolution.

## 7. Security and privacy

- Capture local screen only on explicit activation.
- Never write/copy screen pixels to disk, clipboard or network unless the user specifically requests a future export feature.
- Do not require administrator, accessibility privileges, or a system-wide input hook for Windows MVP.
- Windows capture may fail for secure desktop or protected applications; treat as a recoverable limitation.
- No auto-update, telemetry, remote calls, privileged service, persistence or autorun setup in MVP.

## 8. Developer handover

At the end Codex should state:

- What works on Windows and what remains incomplete.
- Exact commands run, and their outcomes.
- Whether the actual Windows GUI was exercised (if not, name unverified scenarios).
- File path for built `Zoomer.exe`/ZIP if generated.
- Release binary and ZIP file size, if measured.
- Changes to architecture/dependencies and reasons.
- Manual steps the project owner should execute on their Windows computer.
