# Zoomer 0.2.0 — temporary pen

This extension was explicitly requested by the user after the 0.1.0 viewer
worked. It adds freehand circling/drawing on the frozen image, with no export,
file persistence, live desktop annotation or new dependencies.

## Controls and lifetime

- Open with Ctrl+Alt+Z. Press P or click PEN P at the top left.
- Left-drag draws; a click makes a dot. Right-drag pans in either mode.
- Click a color dot or press 1–5: red, yellow, green, blue, white.
- Click toolbar −/+ or press [ / ] for a 2–24 px pen (default red, 4 px).
- Ctrl+Z / Undo removes the last stroke; Delete/Backspace / Clear removes all.
- P returns left-drag to pan and leaves marks visible. Wheel/+/- zoom as before.
- 0 resets the view/effects but preserves ink. Mirror transforms ink with image.
- Escape/Q/global dismissal drops snapshot and ink. A new snapshot starts in pan
  mode with no marks. Color/size are not persisted between snapshots.

Width is in screen pixels when a stroke starts and scales with subsequent image
zoom. Existing strokes keep their original color/width. Camera motion stops
before drawing, so new ink does not drift during easing/inertia.

## Implementation and impact

Portable state holds image-coordinate point lists, per-stroke radius/RGB, bounded
at 32,768 points / 512 strokes. Samples closer than 0.75 screen pixels are
coalesced; nonfinite/overflow-prone coordinates/scales are rejected. The toolbar
shows a limit hint rather than growing memory without bound.

A second GL program draws instanced round antialiased capsules and a DPI-scaled
toolbar with original 5x7 glyph geometry. Ink is above the flashlight for
legibility. The VBO uploads only on stroke/style/UI-scale changes; camera motion
uses uniforms. No CPU screenshot copy, annotation texture, font/image crate, UI
toolkit or periodic redraw is added. GL resources and CPU ink drop on dismissal.
The optimized EXE is 978,432 bytes versus 958,464 for 0.1.0 (+19,968).

## Verification evidence (2026-11-10)

Full Windows stack, Rust stable 1.99.1 / MSVC x64, CARGO_HOME=.local/cargo:

| Check | Result |
|---|---|
| cargo fmt --all -- --check | PASS |
| cargo check --all-targets --locked --offline | PASS |
| cargo test --locked --offline | PASS: 15 core tests; 2 desktop tests ignored in this default run |
| cargo clippy --all-targets --locked --offline -- -D warnings | PASS |
| cargo build --release --locked --offline --target x86_64-pc-windows-msvc | PASS |
| Explicit release desktop tests below | PASS: 2 |

```powershell
cargo test --release --locked --offline --target x86_64-pc-windows-msvc --bin zoomer -- --ignored --nocapture --test-threads=1
```

Pure tests verify dots/strokes, style, undo/clear, memory bounds, invalid input,
mirrored camera round trips, toolbar hit tests at 100/125/150/200% and finite GPU
instance data. Existing camera/config tests pass. Native tests use generated pixels:

- GPU readback checks ink under mirror/pan/zoom and verifies undo removes uploaded
  geometry; screenshot/effect checks remain.
- Actual App routing toggles via toolbar, draws instead of panning, right-drags
  to pan, undoes, survives startup events, then clears pen state on dismissal.
  Handlers are called without injecting desktop input or capturing a real screen.

The user tried the installed build after instructions to press P and left-drag
a circle, and confirmed: "yes it works!" This establishes the basic physical
pen workflow. Individual control-by-control signoff, real mixed-DPI toolbar
comfort and long stress tests remain separate manual checks. Native tests do
not establish those cases. Install/ZIP verification is recorded in the versioned
delivery report.

## Changed source and delivery

Added src/annotation.rs and pen_ui.rs; updated lib.rs, camera.rs, app.rs and
graphics.rs. Updated package/lock/manifest to 0.2.0, packaging defaults and docs.
Previous 0.1.0 artifacts are preserved. The installed EXE uses the same stable
path, preserving startup/live config and backing up the previous installation.
