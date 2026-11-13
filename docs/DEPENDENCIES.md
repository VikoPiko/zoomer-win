# Dependency inventory

Zoomer has no runtime network requests; Cargo's development-time package
downloads are separate. Cargo.lock now pins the resolved graph; the complete
Windows stack compiles and passes Clippy. Windows metadata contains 67 registry
dependencies, including build tools, with copied notices in THIRD_PARTY_NOTICES.md.

| Dependency | Purpose | Rough impact |
|---|---|---|
| winit 0.30.13 | Native window, input, event-loop wakeups | Largest platform/window component; no widgets |
| glutin 0.32.3 | WGL context and surface | Windows driver integration; WGL feature only |
| glutin-winit 0.5 | Surface creation/resizing helpers | Small integration layer; WGL only |
| glow 0.18 | OpenGL loading and drawing | Thin bindings, no bundled graphics runtime |
| raw-window-handle 0.6 | Borrow native handles | Small handle types/traits |
| tray-icon 0.26.1 | Tray/menu | Native message windows; Linux defaults disabled |
| global-hotkey 0.10 | RegisterHotKey callbacks | Hidden message window; D005 explains version choice |
| windows-sys 0.61 | GDI, monitors, mutex, dialogs, focus | Narrow FFI linked to Windows system DLLs |
| serde 1 | Typed configuration | Build-time derives, small runtime glue |
| toml 0.9 | Settings parser | Parser code; no settings GUI |
| thiserror 2 | Typed contextual errors | Build-time derives; no error service |
| embed-resource 3 (build only) | Manifest/icon | Windows resource compiler discovery; no runtime cost |

Desktop dependencies are Windows gated. The app enables no GTK, webview, image
decoder, async runtime, network stack or capture framework. Exact binary/ZIP
and per-dependency sizes are not separately profiled. The 0.1.0 EXE was 958,464
bytes; 0.2.0 with the pen is 978,432 bytes (19,968 bytes extra). The pen adds no
dependency; only bounded stroke points and a small GPU geometry buffer/program.
A 4K BGRA buffer occupies 33,177,600
bytes briefly on CPU, plus a GPU texture and a GDI bitmap during capture.
No CPU frame survives successful upload.
