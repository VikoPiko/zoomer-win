use crate::config::CameraConfig;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// All coordinates are physical framebuffer pixels, with origin at top left.
#[derive(Debug)]
pub struct Camera {
    scale: f64,
    target: f64,
    offset: Point,
    velocity: Point,
    anchor: Point,
    settings: CameraConfig,
    pub mirror: bool,
}

impl Camera {
    pub fn new(settings: &CameraConfig) -> Self {
        // Public math API remains safe even if called without validated configuration.
        let mut checked = crate::config::Config {
            camera: settings.clone(),
            ..Default::default()
        };
        if checked.validate().is_err() {
            checked.camera = CameraConfig::default();
        }
        Self {
            scale: 1.0,
            target: 1.0,
            offset: Point::default(),
            velocity: Point::default(),
            anchor: Point::default(),
            settings: checked.camera,
            mirror: false,
        }
    }
    pub fn scale(&self) -> f64 {
        self.scale
    }
    pub fn offset(&self) -> Point {
        self.offset
    }
    pub fn reset(&mut self) {
        *self = Self::new(&self.settings);
    }
    pub fn cancel_motion(&mut self) {
        self.target = self.scale;
        self.velocity = Point::default();
    }
    pub fn zoom(&mut self, steps: f64, anchor: Point) {
        if !steps.is_finite() || !anchor.finite() || anchor.x.abs() > 1e9 || anchor.y.abs() > 1e9 {
            return;
        }
        self.velocity = Point::default();
        self.anchor = anchor;
        self.target = (self.target * self.settings.wheel_factor.powf(steps.clamp(-100.0, 100.0)))
            .clamp(self.settings.min_scale, self.settings.max_scale);
    }
    pub fn drag(&mut self, delta: Point, dt: f64) {
        if !delta.finite() {
            return;
        }
        self.target = self.scale;
        self.offset.x = (self.offset.x + delta.x.clamp(-1e6, 1e6)).clamp(-1e9, 1e9);
        self.offset.y = (self.offset.y + delta.y.clamp(-1e6, 1e6)).clamp(-1e9, 1e9);
        if dt.is_finite() && dt > 0.001 && self.settings.pan_half_life_ms > 0.0 {
            // An exponential average uses recent samples rather than a single noisy mouse event.
            let alpha = 1.0 - (-dt / 0.03).exp();
            self.velocity.x += ((delta.x / dt).clamp(-10000.0, 10000.0) - self.velocity.x) * alpha;
            self.velocity.y += ((delta.y / dt).clamp(-10000.0, 10000.0) - self.velocity.y) * alpha;
        } else {
            self.stop_inertia();
        }
    }
    pub fn stop_inertia(&mut self) {
        self.velocity = Point::default();
    }
    pub fn moving(&self) -> bool {
        (self.target - self.scale).abs() > 1e-7 || self.velocity.x.hypot(self.velocity.y) > 1.0
    }
    pub fn needs_frame(&self, dragging: bool) -> bool {
        self.scale != self.target || (!dragging && self.moving())
    }
    /// Exact exponential integration makes zoom anchoring and inertia frame-rate independent.
    pub fn advance(&mut self, seconds: f64, dragging: bool) {
        if !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        let dt = seconds.min(0.05);
        if self.scale != self.target {
            let alpha = 1.0
                - (-std::f64::consts::LN_2 * dt * 1000.0 / self.settings.zoom_half_life_ms).exp();
            let mut next = self.scale + (self.target - self.scale) * alpha;
            if (next - self.target).abs() < 1e-7 {
                next = self.target;
            }
            let ratio = next / self.scale;
            self.offset.x = self.anchor.x - (self.anchor.x - self.offset.x) * ratio;
            self.offset.y = self.anchor.y - (self.anchor.y - self.offset.y) * ratio;
            self.scale = next;
        }
        if !dragging
            && self.velocity.x.hypot(self.velocity.y) > 1.0
            && self.settings.pan_half_life_ms > 0.0
        {
            let decay = std::f64::consts::LN_2 * 1000.0 / self.settings.pan_half_life_ms;
            // Integrate only to the exact stop threshold, regardless of frame rate.
            let until_stop = self.velocity.x.hypot(self.velocity.y).ln() / decay;
            let retained = (-decay * dt.min(until_stop)).exp();
            self.offset.x =
                (self.offset.x + self.velocity.x * (1.0 - retained) / decay).clamp(-1e9, 1e9);
            self.offset.y =
                (self.offset.y + self.velocity.y * (1.0 - retained) / decay).clamp(-1e9, 1e9);
            self.velocity.x *= retained;
            self.velocity.y *= retained;
            if dt >= until_stop {
                self.stop_inertia();
            }
        } else if !dragging {
            self.stop_inertia();
        }
    }
    pub fn image_at(&self, screen: Point, image_width: f64) -> Point {
        let x = (screen.x - self.offset.x) / self.scale;
        Point::new(
            if self.mirror { image_width - x } else { x },
            (screen.y - self.offset.y) / self.scale,
        )
    }
    pub fn screen_at(&self, image: Point, image_width: f64) -> Point {
        Point::new(
            (if self.mirror {
                image_width - image.x
            } else {
                image.x
            }) * self.scale
                + self.offset.x,
            image.y * self.scale + self.offset.y,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }
    #[test]
    fn anchor_is_fixed_at_every_easing_step_in_both_mirror_modes() {
        for mirror in [false, true] {
            for p in [
                Point::new(0.0, 0.0),
                Point::new(741.25, 538.5),
                Point::new(3839.0, 2159.0),
            ] {
                let mut c = Camera::new(&CameraConfig::default());
                c.mirror = mirror;
                c.drag(Point::new(-120.0, 70.0), 0.016);
                let image = c.image_at(p, 3840.0);
                c.zoom(10.0, p);
                for _ in 0..400 {
                    c.advance(1.0 / 60.0, false);
                    let actual = c.image_at(p, 3840.0);
                    close(actual.x, image.x);
                    close(actual.y, image.y);
                }
                assert!(!c.moving());
            }
        }
    }
    #[test]
    fn clamped_and_invalid_input_terminates() {
        let mut c = Camera::new(&CameraConfig::default());
        c.zoom(f64::NAN, Point::default());
        c.zoom(5.0, Point::new(f64::MAX, f64::MAX));
        c.advance(f64::INFINITY, false);
        c.drag(Point::new(f64::INFINITY, 0.0), 0.0);
        assert_eq!(c.scale(), 1.0);
        c.zoom(1e100, Point::default());
        for _ in 0..500 {
            c.advance(0.02, false);
        }
        close(c.scale(), 16.0);
        assert!(!c.moving());
        c.zoom(-1e100, Point::default());
        for _ in 0..500 {
            c.advance(0.02, false);
        }
        close(c.scale(), 0.5);
        c.reset();
        assert_eq!(c.offset(), Point::default());
        assert!(!c.mirror);
    }
    #[test]
    fn time_based_motion_and_disabled_inertia() {
        let cfg = CameraConfig::default();
        let mut a = Camera::new(&cfg);
        let mut b = Camera::new(&cfg);
        a.drag(Point::new(10.0, 4.0), 0.01);
        b.drag(Point::new(10.0, 4.0), 0.01);
        for _ in 0..30 {
            a.advance(1.0 / 30.0, false);
        }
        for _ in 0..120 {
            b.advance(1.0 / 120.0, false);
        }
        close(a.offset().x, b.offset().x);
        let mut c = Camera::new(&CameraConfig {
            pan_half_life_ms: 0.0,
            ..cfg
        });
        c.drag(Point::new(10.0, 0.0), 0.01);
        c.advance(0.01, false);
        close(c.offset().x, 10.0);
        assert!(!c.moving());
    }
    #[test]
    fn reversing_zoom_and_changing_anchor_remain_stable() {
        let mut camera = Camera::new(&CameraConfig::default());
        camera.zoom(8.0, Point::new(100.0, 100.0));
        for _ in 0..5 {
            camera.advance(0.016, false);
        }
        let anchor = Point::new(650.0, 340.0);
        let image = camera.image_at(anchor, 1920.0);
        camera.zoom(-4.0, anchor);
        for _ in 0..500 {
            camera.advance(0.016, false);
            let actual = camera.image_at(anchor, 1920.0);
            close(actual.x, image.x);
            close(actual.y, image.y);
        }
        assert!(!camera.needs_frame(false));
        camera.drag(Point::new(100.0, 0.0), 0.02);
        assert!(!camera.needs_frame(true));
        assert!(camera.needs_frame(false));
        camera.cancel_motion();
        assert!(!camera.needs_frame(false));
    }
}
