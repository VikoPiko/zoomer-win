# Cargo registry EACCES diagnosis and resolution

On this Windows host, the failure had two independent environmental causes:

1. Sandboxed processes could not open HTTPS connections to index.crates.io
   or static.crates.io. curl failed to connect and Node reported EACCES.
   The same Cargo fetch succeeded outside that sandbox through the tool's
   approved network route. This was not a malformed registry URL or bad proxy.
2. The existing user cache at `%USERPROFILE%\.cargo\registry` grants
   CodexSandboxUsers read/execute access, while the owner has full control.
   Cache writes from a sandboxed Cargo process are therefore inappropriate.

No overriding Cargo config was found at the workspace, ancestors or user Cargo
home. No Cargo/Rust/proxy environment overrides were set. User Internet Settings
had no configured proxy, and WinHTTP reported direct access. The old cache
contained embed-resource 3.0.12 and tray-icon 0.25.1, but lacked the selected
winit/glutin/glutin-winit/glow/global-hotkey packages.

Resolution: use a workspace-local Cargo home, with no global ACL, proxy,
firewall, registry-source or execution-policy changes:

```powershell
$env:CARGO_HOME = Join-Path (Get-Location) '.local/cargo'
cargo fetch --locked --target x86_64-pc-windows-msvc
```

The initial fetch was authorized outside the network sandbox and generated the
root Cargo.lock. It resolved 185 packages in the cross-platform lock graph;
the Windows metadata graph contains the app and 67 registry dependencies.
Windows dependencies are now downloaded into the writable `.local/cargo`.
Keep Cargo.lock under version control; `.local/` remains ignored.

Subsequent Windows builds and checks work offline inside the normal sandbox:

```powershell
$env:CARGO_HOME = Join-Path (Get-Location) '.local/cargo'
cargo check --all-targets --locked --offline
cargo test --locked --offline
cargo clippy --all-targets --locked --offline -- -D warnings
cargo build --release --locked --offline --target x86_64-pc-windows-msvc
```

On a fresh host/cache, the minimum requirement is one permitted fetch from the
official crates.io registry, or an approved offline cache containing the same
locked Windows dependencies and index records. Offline mode cannot manufacture
missing crates. No application architecture change or administrator privilege
is needed. Zoomer's running application makes no network requests.
