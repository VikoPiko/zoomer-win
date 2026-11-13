# Windows interactive acceptance

For 0.2.0, generated-pixel native tests verify pen draw/pan routing, ink color,
zoom/pan/mirror, undo and dismissal cleanup. Fifteen core tests and both native
tests pass. The user also confirmed physical freehand drawing in the installed
build. Remaining pen controls/mixed-DPI checks are tracked in PEN_UPDATE.md.
The original 0.1.0 evidence and broader manual matrix follow below.

**Build and basic real activation verified on 2026-11-10; broader acceptance pending.**
Full Windows compilation and automated checks pass. Two explicit native tests
pass using generated pixels: GPU color/orientation/mirror/flashlight readback
and overlay survival of queued startup geometry events. The latter reproduced
and now guards the blink/immediate-dismissal defect. The user confirmed the
fixed real workflow is working. This general confirmation is not individual
signoff for every control or every display scenario below.

The Windows UI automation native pipe is unavailable (OS error 2 after retries),
so remaining interactive cases require manual testing or restoration of that
helper. No unexecuted case is marked passing from core tests.

Record before testing:

| Evidence | Value |
|---|---|
| Tester/date | User real activation; agent native tests, 2026-11-10 |
| Windows edition/build | Windows 11 Pro 10.0.26200 |
| CPU/RAM/GPU/driver | i7-8700K @ 3.70 GHz; GTX 1080 Ti; driver 32.0.15.6094; RAM not recorded |
| Monitor resolution/origin/scaling/refresh per display | 1920x1080 at (0,0) and (1920,0) from real activation diagnostics; DPI/refresh not verified |
| Git commit/tag, Rust version, release command | No Git checkout; Rust 1.99.1; cargo build --release --locked --offline --target x86_64-pc-windows-msvc |
| EXE SHA-256 and ZIP SHA-256 | EXE: 196d37df1d8e6106bb370343018a113aafdd4a18689b1f391ecfe8518d08b7f7; ZIP: adjacent .sha256 file |

Use an ordinary user account and a **release** build. Record PASS/FAIL/NOT RUN
and notes for each row, with observed latency or resource counts where relevant.
Keep confidential desktop contents out of any optional test evidence.

| Scenario | Steps and expected observation | Status |
|---|---|---|
| 0.2.0 pen | P/button toggles, left-drag circles, click dots, right-drag pans, palette/1–5 colors, [ ]/buttons size, Ctrl+Z undo, Delete clear, P back to pan; zoom/mirror/reset attachment and Esc discards ink | Basic physical drawing USER PASS; model/GPU/App routing PASS; broader control/DPI matrix pending |
| Tray startup | Launch once: tray icon/menu, no overlay; check overflow area. Launch again: only one process/icon | NOT RUN |
| Real 1080p image | User confirmed the fixed real snapshot workflow works; GDI/upload trace succeeds on both 1080p monitors. Exact real color-chart/1:1 inspection still pending; generated-color GPU test passes | PARTIAL: user activation PASS |
| Global toggle | Repeated real hotkey activations logged and user confirmed working; exact held-chord and foreground-app matrix still pending | PARTIAL |
| Zoom anchor/easing | Place pointer on a character; wheel repeatedly and reverse. Character stays under pointer throughout animation; stop converges with no further draws | NOT RUN |
| Keyboard zoom | +/= and - zoom; leave cursor outside window and verify center fallback | NOT RUN |
| Drag and inertia | Drag in both axes; release moving to coast, release after pause to stop. New drag/zoom cancels old momentum. Set pan half-life 0 and reload to disable inertia | NOT RUN |
| Mirror | M flips around image center; zoom remains anchored in mirror mode. Repeated M restores original geometry | NOT RUN |
| Flashlight | F dims outside smooth radial edge; cursor follows; Ctrl+Wheel changes radius to bounds without changing scale | NOT RUN |
| Reset | After zoom/pan/mirror/flashlight/radius changes, 0 restores 1x, origin, effects off, default radius and zero velocity | NOT RUN |
| Escape/focus | Escape and Q hide immediately, keep tray alive; previous app can be used. Alt+Tab does not force focus back | NOT RUN |
| Tray actions | Capture, Reload, About work; About/error dialogs stay usable. Quit terminates process, removes tray icon and frees shortcut for another app | NOT RUN |
| Config absence/error | No file: defaults with no file created. Malformed TOML/NaN/unknown fields: warning and no overwrite. Invalid reload retains prior settings | NOT RUN |
| Hotkey collision | Reserve configured shortcut in another app. Startup warns; tray Capture works. Choose alternative/reload. Failed replacement preserves previous binding | NOT RUN |
| Negative origin | Put secondary left of primary (e.g. x=-2560); pointer on it chooses the right screen and fullscreen bounds | NOT RUN |
| DPI 100/125/150% | Repeat text/anchor/capture checks at all three scales and across mixed-DPI monitors. Check no double scaling, shift or clipped edge | NOT RUN |
| 4K capture | Real 3840x2160 capture renders sharply; measure activation, peak/private/working-set memory and frame pacing | NOT RUN |
| Rapid activation | Rapidly activate/dismiss and invoke tray Capture during capture; no duplicate worker accumulation, stale reopening or self-capture | NOT RUN |
| Display changes | Change resolution/scaling, disconnect/reconnect monitor while viewing/capturing. Overlay dismisses or reports recoverable error; next activation works | NOT RUN |
| Lock/sleep | Lock/unlock and sleep/resume while idle/viewing; no stuck topmost window, hotkey or permanent animation | NOT RUN |
| Capture failures | Secure desktop/protected windows may fail or return black; app remains recoverable. No stored pixels, privilege escalation or workaround | NOT RUN |
| GPU failure/limits | On a test machine without desktop GL3.3, or forced driver failure, actionable error and tray remains alive; oversized texture rejects gracefully | NOT RUN |
| First frame | User-reported blink reproduced as stale startup resize, fixed; generated-frame native regression stays visible and user confirmed the corrected real overlay | PASS basic startup; wider matrix pending |
| Idle/static CPU | Monitor 60 seconds in tray and 60 seconds static viewer. No periodic capture/repaint; near-zero CPU, stable resource counts | NOT RUN |
| 100 cycles | Compare GDI/USER handles, thread count, private bytes and working set before/after 100 capture/dismiss cycles, allowing stabilization. No steady growth | NOT RUN |
| Privacy/offline | Run offline; all features operate. Observe network activity: none from Zoomer. No captures written to disk/clipboard | NOT RUN |
| Requested startup | Verify installed EXE/hash and quoted per-user HKCU Run value, then verify tray startup at the next sign-in without forcing logout | Registration verification recorded in local report; next sign-in NOT RUN |

For measurements, record tray-ready time, BitBlt/readback time if instrumented,
total hotkey-to-valid-frame time (separate 1080p/4K), peak/idle memory, frame
pacing, CPU duration deltas and handle counts. Use Process Explorer or Task
Manager for CPU, memory and GDI/USER columns. Compare release builds only.
Record actual EXE and ZIP byte sizes and hardware with results.

After ALL required cases pass for the exact candidate: update this matrix,
IMPLEMENTATION_STATUS.md and IMPLEMENTATION_TASKS.md, complete third-party
notices, rebuild/package, attach hashes, and review the draft
release before publication. Headless CI is not interactive acceptance.
