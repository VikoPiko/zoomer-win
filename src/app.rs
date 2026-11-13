use crate::{
    graphics::Viewer,
    platform::{self, Foreground, Instance},
};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use std::time::{Duration, Instant};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key, NamedKey},
    window::{CursorIcon, WindowId},
};
use zoomer::{
    annotation::Drawing,
    camera::{Camera, Point},
    capture::{CaptureError, Display, Frame, windows as capture},
    config::{self, Config},
    lifecycle::{Lifecycle, State},
    pen_ui::{self, Hit, PenAction},
};

#[derive(Debug)]
enum Event {
    Hotkey(u32),
    Capture,
    Reload,
    About,
    Quit,
    Captured(u64, Result<Frame, CaptureError>),
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let Some(_instance) = Instance::acquire()? else {
        platform::message("Zoomer is already running. Use its tray menu or activation shortcut.");
        return Ok(());
    };
    let event_loop = EventLoop::<Event>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(event_loop.create_proxy());
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct Desktop {
    // Retained for the entire tray lifetime; dropping manager also destroys its message window.
    _tray: TrayIcon,
    manager: GlobalHotKeyManager,
    hotkey: Option<HotKey>,
}
impl Desktop {
    fn new(proxy: EventLoopProxy<Event>) -> Result<Self, Box<dyn std::error::Error>> {
        let menu = Menu::new();
        for (id, label) in [
            ("capture", "Capture screen"),
            ("reload", "Reload configuration"),
            ("about", "About Zoomer"),
            ("quit", "Quit Zoomer"),
        ] {
            menu.append(&MenuItem::with_id(id, label, true, None))?;
        }
        let p = proxy.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let event = match event.id.0.as_str() {
                "capture" => Event::Capture,
                "reload" => Event::Reload,
                "about" => Event::About,
                "quit" => Event::Quit,
                _ => return,
            };
            let _ = p.send_event(event);
        }));
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state == HotKeyState::Pressed {
                let _ = proxy.send_event(Event::Hotkey(event.id));
            }
        }));
        let tray = TrayIconBuilder::new()
            .with_tooltip("Zoomer — Ctrl+Alt+Z; right-click for menu")
            .with_icon(Icon::from_rgba(platform::icon_rgba(), 32, 32)?)
            .with_menu(Box::new(menu))
            .build()?;
        Ok(Self {
            _tray: tray,
            manager: GlobalHotKeyManager::new()?,
            hotkey: None,
        })
    }

    /// Register replacement first; a collision leaves the old shortcut operational.
    fn register(&mut self, shortcut: &str) -> Result<(), String> {
        let new = shortcut
            .parse::<HotKey>()
            .map_err(|e| format!("Invalid shortcut: {e}"))?;
        if self.hotkey == Some(new) {
            return Ok(());
        }
        self.manager.register(new).map_err(|e| format!("Could not register {shortcut}: {e}. Another app may use it. Edit %APPDATA%\\Zoomer\\config.toml and choose Reload configuration in the tray. Capture screen remains available."))?;
        if let Some(old) = self.hotkey
            && let Err(error) = self.manager.unregister(old)
        {
            let _ = self.manager.unregister(new);
            return Err(format!(
                "Could not replace previous hotkey: {error}; keeping previous configuration"
            ));
        }
        self.hotkey = Some(new);
        platform::diagnostic(format_args!(
            "registered shortcut {shortcut}, id {}",
            new.id()
        ));
        Ok(())
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        if let Some(hotkey) = self.hotkey.take() {
            let _ = self.manager.unregister(hotkey);
        }
    }
}

struct App {
    proxy: EventLoopProxy<Event>,
    desktop: Option<Desktop>,
    life: Lifecycle,
    config: Config,
    viewer: Option<Viewer>,
    display: Option<Display>,
    camera: Camera,
    ink: Drawing,
    cursor: Point,
    cursor_inside: bool,
    dragging: bool,
    pan_button: Option<MouseButton>,
    ctrl: bool,
    flashlight: bool,
    radius: f64,
    last_drag: Instant,
    last_frame: Instant,
    next_frame: Option<Instant>,
    foreground: Foreground,
    had_focus: bool,
}
impl App {
    fn new(proxy: EventLoopProxy<Event>) -> Self {
        let config = Config::default();
        Self {
            proxy,
            desktop: None,
            life: Lifecycle::default(),
            viewer: None,
            display: None,
            camera: Camera::new(&config.camera),
            ink: Drawing::default(),
            cursor: Point::default(),
            cursor_inside: false,
            dragging: false,
            pan_button: None,
            ctrl: false,
            flashlight: false,
            radius: config.flashlight.radius_px,
            config,
            last_drag: Instant::now(),
            last_frame: Instant::now(),
            next_frame: None,
            foreground: Foreground::default(),
            had_focus: false,
        }
    }
    fn redraw(&self) {
        if let Some(viewer) = &self.viewer {
            viewer.window.request_redraw();
        }
    }
    fn dismiss(&mut self) {
        platform::diagnostic(format_args!("dismiss from {:?}", self.life.state()));
        if let Some(viewer) = self.viewer.take() {
            viewer.window.set_visible(false);
            drop(viewer);
        }
        self.life.dismiss();
        self.display = None;
        self.next_frame = None;
        self.dragging = false;
        self.pan_button = None;
        self.ink = Drawing::default();
        self.ctrl = false;
        self.camera.cancel_motion();
        self.foreground.restore();
    }
    fn recover(&mut self, error: impl std::fmt::Display) {
        self.dismiss();
        platform::message(&format!(
            "{error}\n\nZoomer is still in the tray. Try Capture screen again."
        ));
    }
    fn reload(&mut self, initial: bool) {
        let result = config::path().and_then(|p| config::load(&p));
        let new = match result {
            Ok(c) => c,
            Err(e) => {
                self.dismiss();
                platform::message(&format!(
                    "{e}\nUsing {}. Configuration file has not been modified.",
                    if initial {
                        "documented defaults"
                    } else {
                        "previous settings"
                    }
                ));
                if initial {
                    Config::default()
                } else {
                    return;
                }
            }
        };
        if let Some(desktop) = &mut self.desktop
            && let Err(e) = desktop.register(&new.hotkeys.activate)
        {
            self.dismiss();
            platform::message(&e);
            if !initial {
                return;
            }
        }
        self.camera = Camera::new(&new.camera);
        self.finish_gesture();
        self.radius = new.flashlight.radius_px;
        self.flashlight = false;
        if let Some(viewer) = &self.viewer {
            viewer.set_filter(new.appearance.filter);
        }
        self.config = new;
        self.redraw();
    }
    fn activate(&mut self) {
        platform::diagnostic(format_args!("activate from {:?}", self.life.state()));
        // global-hotkey registers MOD_NOREPEAT on Windows; in-flight captures coalesce below.
        match self.life.state() {
            State::Visible => {
                self.dismiss();
                return;
            }
            State::Capturing(_) => return,
            State::Idle => {}
        }
        self.foreground.remember();
        let (display, position) = match capture::target_under_cursor() {
            Ok(target) => target,
            Err(error) => {
                self.recover(error);
                return;
            }
        };
        let (x, y) = display.local_point(position.0, position.1);
        platform::diagnostic(format_args!("capture selected {display:?}"));
        self.cursor = Point::new(x, y);
        let Some(generation) = self.life.begin() else {
            return;
        };
        let proxy = self.proxy.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("screen-capture".into())
            .spawn(move || {
                let _ = proxy.send_event(Event::Captured(generation, capture::capture(display)));
            })
        {
            self.recover(format!("Could not start capture worker: {error}"));
        }
    }
    fn show(&mut self, event_loop: &ActiveEventLoop, generation: u64, frame: Frame) {
        platform::diagnostic(format_args!(
            "capture complete generation {generation}, showing"
        ));
        if !self.life.accepts(generation) {
            return;
        }
        match capture::displays() {
            Ok(displays) if displays.contains(&frame.display) => {}
            _ => {
                self.recover("Display layout changed during capture; try again");
                return;
            }
        }
        self.camera = Camera::new(&self.config.camera);
        self.ink = Drawing::default();
        self.flashlight = false;
        self.radius = self.config.flashlight.radius_px;
        self.cursor_inside = true;
        self.had_focus = false;
        let viewer = match Viewer::new(event_loop, &frame, &self.config) {
            Ok(viewer) => viewer,
            Err(e) => {
                self.recover(e);
                return;
            }
        };
        self.display = Some(frame.display);
        drop(frame); // screenshot CPU memory is no longer retained after successful GPU upload.
        if let Err(e) = viewer.draw(
            &self.camera,
            self.cursor,
            self.flashlight,
            self.radius,
            &self.config,
            &self.ink,
        ) {
            drop(viewer);
            self.recover(e);
            return;
        }
        self.life.show(generation);
        viewer.window.set_visible(true);
        viewer.window.focus_window();
        platform::diagnostic("viewer rendered and revealed");
        self.last_frame = Instant::now();
        viewer.window.request_redraw();
        self.viewer = Some(viewer);
    }
    fn zoom(&mut self, steps: f64) {
        self.ink.finish();
        let anchor = if self.cursor_inside {
            self.cursor
        } else {
            self.viewer.as_ref().map_or(Point::default(), |v| {
                let s = v.window.inner_size();
                Point::new(f64::from(s.width) / 2.0, f64::from(s.height) / 2.0)
            })
        };
        if !self.camera.moving() {
            self.last_frame = Instant::now();
        }
        self.camera.zoom(steps, anchor);
        self.redraw();
    }
    fn draw(&mut self) {
        let now = Instant::now();
        self.camera.advance(
            now.duration_since(self.last_frame).as_secs_f64(),
            self.dragging,
        );
        self.last_frame = now;
        let result = self.viewer.as_ref().map(|v| {
            v.draw(
                &self.camera,
                self.cursor,
                self.flashlight,
                self.radius,
                &self.config,
                &self.ink,
            )
        });
        if let Some(Err(e)) = result {
            self.recover(e);
            return;
        }
        self.next_frame = if self.camera.needs_frame(self.dragging) {
            Some(now + Duration::from_micros(16_667))
        } else {
            None
        };
    }
    fn ui_scale(&self) -> f64 {
        self.viewer
            .as_ref()
            .map_or(1.0, |v| v.window.scale_factor().clamp(0.5, 4.0))
    }
    fn toolbar_hit(&self) -> Hit {
        pen_ui::hit_test(self.cursor, self.ui_scale(), self.ink.enabled())
    }
    fn update_cursor(&self) {
        if let Some(viewer) = &self.viewer {
            let cursor = if self.dragging {
                CursorIcon::Grabbing
            } else if self.toolbar_hit() != Hit::Outside {
                CursorIcon::Pointer
            } else if self.ink.enabled() {
                CursorIcon::Crosshair
            } else {
                CursorIcon::Default
            };
            viewer.window.set_cursor(cursor);
        }
    }
    fn finish_gesture(&mut self) {
        self.ink.finish();
        self.dragging = false;
        self.pan_button = None;
        self.camera.cancel_motion();
    }
    fn pen_action(&mut self, action: PenAction) {
        self.finish_gesture();
        match action {
            PenAction::Toggle => self.ink.toggle(),
            PenAction::Color(i) => self.ink.set_color(i),
            PenAction::Thinner => self.ink.change_width(-2),
            PenAction::Thicker => self.ink.change_width(2),
            PenAction::Undo => self.ink.undo(),
            PenAction::Clear => self.ink.clear(),
        }
        self.update_cursor();
        self.redraw();
    }
    fn mouse_button(&mut self, state: ElementState, button: MouseButton) {
        if state == ElementState::Released {
            if button == MouseButton::Left {
                self.ink.finish();
            }
            if self.pan_button == Some(button) {
                self.dragging = false;
                self.pan_button = None;
                if self.last_drag.elapsed() > Duration::from_millis(80) {
                    self.camera.stop_inertia();
                }
                self.last_frame = Instant::now();
                self.redraw();
            }
            self.update_cursor();
            return;
        }
        if !matches!(button, MouseButton::Left | MouseButton::Right) {
            return;
        }
        if button == MouseButton::Left {
            match self.toolbar_hit() {
                Hit::Action(action) => {
                    self.pen_action(action);
                    return;
                }
                Hit::Background => {
                    self.finish_gesture();
                    self.update_cursor();
                    return;
                }
                Hit::Outside => {}
            }
        }
        self.finish_gesture();
        if button == MouseButton::Left && self.ink.enabled() {
            if let Some(display) = self.display {
                let point = self.camera.image_at(self.cursor, f64::from(display.width));
                self.ink.begin(point, self.camera.scale());
                self.redraw();
            }
        } else {
            self.dragging = true;
            self.pan_button = Some(button);
            self.last_drag = Instant::now();
        }
        self.update_cursor();
    }
    fn cursor_moved(&mut self, point: Point) {
        if !point.finite() {
            return;
        }
        if self.dragging {
            let now = Instant::now();
            self.camera.drag(
                Point::new(point.x - self.cursor.x, point.y - self.cursor.y),
                now.duration_since(self.last_drag).as_secs_f64(),
            );
            self.last_drag = now;
            self.last_frame = now;
        } else if self.ink.active()
            && let Some(display) = self.display
        {
            let image = self.camera.image_at(point, f64::from(display.width));
            if self.ink.append(image, self.camera.scale()) {
                self.redraw();
            }
        }
        self.cursor = point;
        self.cursor_inside = true;
        self.update_cursor();
        if self.dragging || self.flashlight {
            self.redraw();
        }
    }
}

impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.desktop.is_none() {
            match Desktop::new(self.proxy.clone()) {
                Ok(desktop) => {
                    self.desktop = Some(desktop);
                    self.reload(true);
                }
                Err(e) => {
                    platform::message(&format!("Could not create system tray: {e}"));
                    event_loop.exit();
                }
            }
        }
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.dismiss();
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        match event {
            Event::Hotkey(id) => {
                platform::diagnostic(format_args!("hotkey event {id}"));
                if self
                    .desktop
                    .as_ref()
                    .and_then(|d| d.hotkey)
                    .is_some_and(|h| h.id() == id)
                {
                    self.activate();
                }
            }
            Event::Capture => self.activate(),
            Event::Reload => self.reload(false),
            Event::About => {
                self.dismiss();
                platform::message(concat!(
                    "Zoomer ",
                    env!("CARGO_PKG_VERSION"),
                    "\nFrozen screen magnifier with temporary pen.\nCtrl+Alt+Z: toggle; Esc: dismiss; wheel: zoom; drag: pan\nP: pen; left-drag draws, right-drag pans; 1-5: colors; [ ]: width\nCtrl+Z: undo; Delete: clear ink; 0: reset view; M: mirror; F: flashlight\nCtrl+wheel: flashlight radius; R: reload config. Quit from tray.\nInspired by tsoding/boomer; independent implementation.\nNo network, telemetry, or saved screenshots/ink."
                ));
            }
            Event::Quit => {
                self.dismiss();
                self.desktop = None;
                event_loop.exit();
            }
            Event::Captured(generation, result) => {
                if self.life.accepts(generation) {
                    match result {
                        Ok(frame) => self.show(event_loop, generation, frame),
                        Err(e) => self.recover(e),
                    }
                }
            }
        }
    }
    fn window_event(&mut self, _: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.viewer.as_ref().is_none_or(|v| v.window.id() != id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => self.dismiss(),
            WindowEvent::Focused(focused) => {
                platform::diagnostic(format_args!(
                    "focus event {focused}, had_focus {}",
                    self.had_focus
                ));
                if !focused && self.had_focus {
                    // Respect an intentional switch to another app (Alt+Tab, tray, taskbar).
                    self.foreground.forget();
                    self.dismiss();
                }
                self.had_focus = focused;
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::Resized(size) => {
                // Creation/fullscreen transitions can queue an earlier window size.
                // Validate the live framebuffer instead of dismissing on stale event data.
                let current = self.viewer.as_ref().map(|v| v.window.inner_size());
                platform::diagnostic(format_args!("resize event {size:?}, live {current:?}"));
                if self
                    .display
                    .zip(current)
                    .is_some_and(|(d, s)| d.width != s.width || d.height != s.height)
                {
                    self.dismiss();
                } else {
                    self.redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                platform::diagnostic(format_args!("scale factor event {scale_factor}"));
                // DPI changes also occur during initial monitor placement. Physical
                // monitor/framebuffer geometry determines whether this frame is stale.
                let monitor = self
                    .viewer
                    .as_ref()
                    .and_then(|v| v.window.current_monitor());
                let changed = self.display.zip(monitor).is_none_or(|(d, m)| {
                    let p = m.position();
                    let s = m.size();
                    d.x != p.x || d.y != p.y || d.width != s.width || d.height != s.height
                });
                if changed {
                    self.dismiss();
                } else {
                    self.redraw();
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => self.ctrl = modifiers.state().control_key(),
            WindowEvent::CursorLeft { .. } => {
                self.cursor_inside = false;
                self.finish_gesture();
            }
            WindowEvent::CursorMoved { position, .. } => {
                // winit cursor positions already use physical framebuffer pixels.
                self.cursor_moved(Point::new(position.x, position.y));
            }
            WindowEvent::MouseInput { state, button, .. } => self.mouse_button(state, button),
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    MouseScrollDelta::PixelDelta(p) => p.y / 120.0,
                };
                if !steps.is_finite() {
                    return;
                }
                if self.ctrl {
                    let f = &self.config.flashlight;
                    self.radius = (self.radius * 1.1_f64.powf(steps.clamp(-100.0, 100.0)))
                        .clamp(f.min_radius_px, f.max_radius_px);
                    if self.flashlight {
                        self.redraw();
                    }
                } else {
                    self.zoom(steps);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.logical_key {
                    Key::Named(NamedKey::Escape) => self.dismiss(),
                    Key::Named(NamedKey::Delete | NamedKey::Backspace) if !event.repeat => {
                        self.pen_action(PenAction::Clear)
                    }
                    Key::Character(key) => match key.to_ascii_lowercase().as_str() {
                        "q" => self.dismiss(),
                        "p" if !event.repeat => self.pen_action(PenAction::Toggle),
                        "z" if self.ctrl && !event.repeat => self.pen_action(PenAction::Undo),
                        "[" if self.ink.enabled() => self.pen_action(PenAction::Thinner),
                        "]" if self.ink.enabled() => self.pen_action(PenAction::Thicker),
                        "1" | "2" | "3" | "4" | "5" if self.ink.enabled() && !event.repeat => {
                            let index = usize::from(key.as_bytes()[0] - b'1');
                            self.pen_action(PenAction::Color(index));
                        }
                        "+" | "=" => self.zoom(1.0),
                        "-" => self.zoom(-1.0),
                        "0" if !event.repeat => {
                            self.finish_gesture();
                            self.camera.reset();
                            self.flashlight = false;
                            self.radius = self.config.flashlight.radius_px;
                            self.redraw();
                        }
                        "f" if !event.repeat => {
                            self.flashlight = !self.flashlight;
                            self.redraw();
                        }
                        "m" if !event.repeat => {
                            self.finish_gesture();
                            self.camera.mirror = !self.camera.mirror;
                            self.redraw();
                        }
                        "r" if !event.repeat => self.reload(false),
                        _ => {}
                    },
                    _ => {}
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(deadline) = self.next_frame {
            if Instant::now() >= deadline {
                self.next_frame = None;
                self.redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.dismiss();
        self.desktop = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::platform::windows::EventLoopBuilderExtWindows;
    use zoomer::capture::{PixelFormat, buffer_layout};

    #[test]
    #[ignore = "requires an interactive Windows desktop; briefly shows generated pixels only"]
    fn overlay_survives_queued_startup_geometry_events() {
        if platform::gui_test_in_child(
            "app::tests::overlay_survives_queued_startup_geometry_events",
        ) {
            return;
        }
        struct Smoke {
            app: App,
            deadline: Option<Instant>,
            survived: bool,
        }
        impl ApplicationHandler<Event> for Smoke {
            fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                let (display, _) = capture::target_under_cursor().unwrap();
                let (stride, size) = buffer_layout(display.width, display.height).unwrap();
                let frame = Frame {
                    display,
                    stride,
                    format: PixelFormat::Bgra8,
                    pixels: vec![64; size],
                };
                let generation = self.app.life.begin().unwrap();
                self.app.show(event_loop, generation, frame);
                // Exercise the application's own event routing with generated data,
                // not injected desktop input or a real screen capture.
                self.app.cursor =
                    Point::new(30.0 * self.app.ui_scale(), 30.0 * self.app.ui_scale());
                self.app
                    .mouse_button(ElementState::Pressed, MouseButton::Left);
                self.app
                    .mouse_button(ElementState::Released, MouseButton::Left);
                assert!(self.app.ink.enabled()); // Toolbar toggle.
                self.app.cursor_moved(Point::new(300.0, 200.0));
                self.app
                    .mouse_button(ElementState::Pressed, MouseButton::Left);
                self.app.cursor_moved(Point::new(400.0, 200.0));
                self.app
                    .mouse_button(ElementState::Released, MouseButton::Left);
                assert_eq!(self.app.ink.segments().count(), 2);
                assert_eq!(self.app.camera.offset(), Point::default()); // Drawing does not pan.
                self.app
                    .mouse_button(ElementState::Pressed, MouseButton::Right);
                self.app.cursor_moved(Point::new(430.0, 220.0));
                self.app
                    .mouse_button(ElementState::Released, MouseButton::Right);
                assert_eq!(self.app.camera.offset(), Point::new(30.0, 20.0));
                self.app.pen_action(PenAction::Undo);
                assert_eq!(self.app.ink.segments().count(), 0);
                self.app.camera.reset();
                self.deadline = Some(Instant::now() + Duration::from_millis(300));
            }
            fn window_event(
                &mut self,
                event_loop: &ActiveEventLoop,
                id: WindowId,
                event: WindowEvent,
            ) {
                self.app.window_event(event_loop, id, event);
            }
            fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
                if event_loop.exiting() {
                    return;
                }
                let deadline = self.deadline.unwrap();
                if Instant::now() >= deadline {
                    self.survived = self.app.life.state() == &State::Visible
                        && self
                            .app
                            .viewer
                            .as_ref()
                            .is_some_and(|v| v.window.is_visible() == Some(true));
                    self.app.dismiss();
                    assert!(!self.app.ink.enabled());
                    assert_eq!(self.app.ink.segments().count(), 0);
                    event_loop.exit();
                } else {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
                }
            }
        }
        let event_loop = EventLoop::<Event>::with_user_event()
            .with_any_thread(true)
            .build()
            .unwrap();
        let mut smoke = Smoke {
            app: App::new(event_loop.create_proxy()),
            deadline: None,
            survived: false,
        };
        event_loop.run_app(&mut smoke).unwrap();
        assert!(
            smoke.survived,
            "startup window events dismissed a valid fullscreen overlay"
        );
    }
}
