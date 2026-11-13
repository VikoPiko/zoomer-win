//! Temporary, image-anchored ink. Contains no platform, capture or GL APIs.
use crate::camera::Point;

pub const COLORS: [[f32; 3]; 5] = [
    [1.0, 0.22, 0.25],
    [1.0, 0.84, 0.12],
    [0.25, 1.0, 0.40],
    [0.20, 0.75, 1.0],
    [1.0, 1.0, 1.0],
];
pub const MAX_POINTS: usize = 32_768;
pub const MAX_STROKES: usize = 512;

#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub start: Point,
    pub end: Point,
    pub radius: f32,
    pub color: [f32; 3],
}

#[derive(Debug)]
struct Stroke {
    points: Vec<Point>,
    radius: f32,
    color: [f32; 3],
}

#[derive(Debug)]
pub struct Drawing {
    strokes: Vec<Stroke>,
    points: usize,
    active: bool,
    enabled: bool,
    color: usize,
    width_px: u8,
    revision: u64,
}

impl Default for Drawing {
    fn default() -> Self {
        Self {
            strokes: Vec::new(),
            points: 0,
            active: false,
            enabled: false,
            color: 0,
            width_px: 4,
            revision: 0,
        }
    }
}

impl Drawing {
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn color(&self) -> usize {
        self.color
    }
    pub fn width_px(&self) -> u8 {
        self.width_px
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn at_capacity(&self) -> bool {
        self.points >= MAX_POINTS || self.strokes.len() >= MAX_STROKES
    }
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn toggle(&mut self) {
        self.finish();
        self.enabled = !self.enabled;
        self.changed();
    }
    pub fn set_color(&mut self, index: usize) {
        if index < COLORS.len() && index != self.color {
            self.finish();
            self.color = index;
            self.changed();
        }
    }
    pub fn change_width(&mut self, delta: i16) {
        let width = (i16::from(self.width_px) + delta.clamp(-24, 24)).clamp(2, 24) as u8;
        if width != self.width_px {
            self.finish();
            self.width_px = width;
            self.changed();
        }
    }
    /// Freeze width in image units so ink follows the image through later zoom/pan.
    pub fn begin(&mut self, image: Point, scale: f64) -> bool {
        self.finish();
        if !self.enabled || !valid_point(image) || !valid_scale(scale) || self.at_capacity() {
            return false;
        }
        self.strokes.push(Stroke {
            points: vec![image],
            radius: (f64::from(self.width_px) / scale / 2.0) as f32,
            color: COLORS[self.color],
        });
        self.points += 1;
        self.active = true;
        self.changed();
        true
    }
    pub fn append(&mut self, image: Point, scale: f64) -> bool {
        if !self.active || !valid_point(image) || !valid_scale(scale) {
            return false;
        }
        if self.points >= MAX_POINTS {
            self.finish();
            return false;
        }
        let Some(stroke) = self.strokes.last_mut() else {
            return false;
        };
        let Some(last) = stroke.points.last() else {
            return false;
        };
        if (image.x - last.x).hypot(image.y - last.y) * scale < 0.75 {
            return false;
        }
        stroke.points.push(image);
        self.points += 1;
        self.changed();
        true
    }
    pub fn finish(&mut self) {
        self.active = false;
    }
    pub fn undo(&mut self) {
        self.finish();
        if let Some(stroke) = self.strokes.pop() {
            self.points -= stroke.points.len();
            self.changed();
        }
    }
    pub fn clear(&mut self) {
        self.finish();
        if !self.strokes.is_empty() {
            self.strokes.clear();
            self.points = 0;
            self.changed();
        }
    }
    pub fn segments(&self) -> impl Iterator<Item = Segment> + '_ {
        self.strokes.iter().flat_map(|s| {
            // A single click makes a dot; subsequent capsules include round joins/caps.
            let dot = s.points.iter().take(1).map(|p| Segment {
                start: *p,
                end: *p,
                radius: s.radius,
                color: s.color,
            });
            dot.chain(s.points.windows(2).map(|p| Segment {
                start: p[0],
                end: p[1],
                radius: s.radius,
                color: s.color,
            }))
        })
    }
}

fn valid_point(p: Point) -> bool {
    p.finite() && p.x.abs() <= 1e9 && p.y.abs() <= 1e9
}
fn valid_scale(scale: f64) -> bool {
    scale.is_finite() && (0.01..=1024.0).contains(&scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{camera::Camera, config::CameraConfig};
    #[test]
    fn dot_stroke_style_undo_and_clear() {
        let mut ink = Drawing::default();
        assert!(!ink.begin(Point::default(), 1.0));
        ink.toggle();
        assert!(ink.begin(Point::new(10.0, 20.0), 2.0));
        assert_eq!(ink.segments().count(), 1); // A click remains visible.
        assert!(!ink.append(Point::new(10.1, 20.0), 2.0));
        assert!(ink.append(Point::new(30.0, 20.0), 2.0));
        let segment = ink.segments().last().unwrap();
        assert_eq!(segment.radius, 1.0);
        ink.set_color(2);
        ink.change_width(2);
        assert!(!ink.active());
        assert!(ink.begin(Point::default(), 1.0));
        assert_eq!(ink.segments().last().unwrap().color, COLORS[2]);
        ink.undo();
        assert_eq!(ink.segments().count(), 2);
        let rev = ink.revision();
        ink.clear();
        assert!(ink.revision() != rev);
        assert_eq!(ink.segments().count(), 0);
        assert!(ink.enabled());
    }
    #[test]
    fn image_anchoring_round_trips_through_mirror_and_navigation() {
        let mut camera = Camera::new(&CameraConfig::default());
        let mut ink = Drawing::default();
        ink.toggle();
        let screen = Point::new(400.0, 300.0);
        ink.begin(camera.image_at(screen, 1920.0), camera.scale());
        camera.zoom(5.0, screen);
        for _ in 0..500 {
            camera.advance(0.016, false);
        }
        let point = ink.segments().next().unwrap().start;
        assert!((camera.screen_at(point, 1920.0).x - screen.x).abs() < 1e-6);
        camera.mirror = true;
        let mirrored = camera.screen_at(point, 1920.0);
        assert_eq!(camera.image_at(mirrored, 1920.0), point);
        camera.reset();
        assert_eq!(camera.screen_at(point, 1920.0), screen);
    }
    #[test]
    fn bounded_memory_and_invalid_input() {
        let mut ink = Drawing::default();
        ink.toggle();
        for scale in [0.0, f64::NAN, f64::INFINITY, 1e-100] {
            assert!(!ink.begin(Point::default(), scale));
        }
        assert!(!ink.begin(Point::new(f64::MAX, 0.0), 1.0));
        ink.change_width(i16::MAX);
        assert_eq!(ink.width_px(), 24);
        ink.change_width(i16::MIN);
        assert_eq!(ink.width_px(), 2);
        ink.set_color(usize::MAX);
        assert_eq!(ink.color(), 0);
        ink.begin(Point::default(), 1.0);
        for x in 1..MAX_POINTS + 10 {
            ink.append(Point::new(x as f64, 0.0), 1.0);
        }
        assert_eq!(ink.points, MAX_POINTS);
        assert!(!ink.active());
        ink.clear();
        for _ in 0..MAX_STROKES + 1 {
            ink.begin(Point::default(), 1.0);
        }
        assert_eq!(ink.strokes.len(), MAX_STROKES);
        ink.undo();
        assert!(ink.begin(Point::default(), 1.0));
    }
}
