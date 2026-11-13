//! A small GPU-drawn toolbar: shared layout for rendering and pointer hit tests.
use crate::{
    annotation::{COLORS, Drawing, Segment},
    camera::Point,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenAction {
    Toggle,
    Color(usize),
    Thinner,
    Thicker,
    Undo,
    Clear,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Outside,
    Background,
    Action(PenAction),
}

pub fn hit_test(p: Point, ui_scale: f64, enabled: bool) -> Hit {
    if !p.finite() || !ui_scale.is_finite() || ui_scale <= 0.0 {
        return Hit::Outside;
    }
    let p = Point::new(p.x / ui_scale, p.y / ui_scale);
    let width = if enabled { 448.0 } else { 80.0 };
    let height = if enabled { 62.0 } else { 36.0 };
    if !inside(p, 16.0, 16.0, width, height) {
        return Hit::Outside;
    }
    if inside(p, 20.0, 20.0, 72.0, 28.0) {
        return Hit::Action(PenAction::Toggle);
    }
    if enabled {
        for i in 0..COLORS.len() {
            if (p.x - (112.0 + i as f64 * 26.0)).hypot(p.y - 34.0) <= 12.0 {
                return Hit::Action(PenAction::Color(i));
            }
        }
        for (x, w, action) in [
            (234.0, 28.0, PenAction::Thinner),
            (298.0, 28.0, PenAction::Thicker),
            (334.0, 54.0, PenAction::Undo),
            (396.0, 60.0, PenAction::Clear),
        ] {
            if inside(p, x, 20.0, w, 28.0) {
                return Hit::Action(action);
            }
        }
    }
    Hit::Background
}
fn inside(p: Point, x: f64, y: f64, w: f64, h: f64) -> bool {
    p.x >= x && p.y >= y && p.x < x + w && p.y < y + h
}

/// Nine f32s per capsule instance: endpoints, radius, screen-space flag and RGB.
/// Only called when strokes/style/UI scale change, never for camera animation alone.
pub fn instances(ink: &Drawing, ui_scale: f64) -> Vec<f32> {
    let mut data = Vec::new();
    for segment in ink.segments() {
        push(&mut data, segment, false);
    }
    let scale = if ui_scale.is_finite() {
        ui_scale.clamp(0.5, 4.0)
    } else {
        1.0
    };
    let mut ui = Ui {
        data: &mut data,
        scale,
    };
    let dark = [0.07, 0.09, 0.13];
    let button = [0.15, 0.18, 0.24];
    let text = [0.94, 0.96, 1.0];
    ui.rect(
        16.0,
        16.0,
        if ink.enabled() { 448.0 } else { 80.0 },
        if ink.enabled() { 62.0 } else { 36.0 },
        dark,
    );
    ui.rect(
        20.0,
        20.0,
        72.0,
        28.0,
        if ink.enabled() {
            [0.12, 0.32, 0.36]
        } else {
            button
        },
    );
    ui.text(27.0, 27.0, "PEN P", 2.0, text);
    if ink.enabled() {
        for (i, color) in COLORS.iter().enumerate() {
            let p = Point::new(112.0 + i as f64 * 26.0, 34.0);
            if i == ink.color() {
                ui.line(p, p, 12.0, text);
            }
            ui.line(p, p, 9.0, *color);
        }
        for (x, w, label) in [
            (234.0, 28.0, "-"),
            (298.0, 28.0, "+"),
            (334.0, 54.0, "UNDO"),
            (396.0, 60.0, "CLEAR"),
        ] {
            ui.rect(x, 20.0, w, 28.0, button);
            ui.text(
                x + (w - label.len() as f64 * 12.0 + 2.0) / 2.0,
                27.0,
                label,
                2.0,
                text,
            );
        }
        ui.text(268.0, 27.0, &ink.width_px().to_string(), 2.0, text);
        ui.text(
            24.0,
            59.0,
            if ink.at_capacity() {
                "LIMIT: UNDO OR CLEAR"
            } else {
                "RIGHT DRAG PAN  [ ] SIZE  CTRL Z UNDO  DEL CLEAR"
            },
            1.0,
            [0.68, 0.75, 0.85],
        );
    }
    data
}

fn push(data: &mut Vec<f32>, s: Segment, screen: bool) {
    data.extend_from_slice(&[
        s.start.x as f32,
        s.start.y as f32,
        s.end.x as f32,
        s.end.y as f32,
        s.radius,
        if screen { 1.0 } else { 0.0 },
        s.color[0],
        s.color[1],
        s.color[2],
    ]);
}
struct Ui<'a> {
    data: &'a mut Vec<f32>,
    scale: f64,
}
impl Ui<'_> {
    fn line(&mut self, a: Point, b: Point, r: f64, color: [f32; 3]) {
        push(
            self.data,
            Segment {
                start: Point::new(a.x * self.scale, a.y * self.scale),
                end: Point::new(b.x * self.scale, b.y * self.scale),
                radius: (r * self.scale) as f32,
                color,
            },
            true,
        );
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: [f32; 3]) {
        self.line(
            Point::new(x + h / 2.0, y + h / 2.0),
            Point::new(x + w - h / 2.0, y + h / 2.0),
            h / 2.0,
            color,
        );
    }
    fn text(&mut self, x: f64, y: f64, text: &str, size: f64, color: [f32; 3]) {
        for (column, c) in text.chars().enumerate() {
            for (row, bits) in glyph(c).iter().enumerate() {
                let mut bit = 0;
                while bit < 5 {
                    if bits & (1 << (4 - bit)) == 0 {
                        bit += 1;
                        continue;
                    }
                    let start = bit;
                    while bit < 5 && bits & (1 << (4 - bit)) != 0 {
                        bit += 1;
                    }
                    let x = x + (column as f64 * 6.0 + start as f64) * size;
                    let y = y + row as f64 * size;
                    self.rect(x, y, (bit - start) as f64 * size, size, color);
                }
            }
        }
    }
}
// Original 5x7 glyph geometry, only for the toolbar; no font runtime or image asset.
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'G' => [14, 17, 16, 23, 17, 17, 14],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 25, 21, 19, 19, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '+' => [0, 4, 4, 31, 4, 4, 0],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '[' => [14, 8, 8, 8, 8, 8, 14],
        ']' => [14, 2, 2, 2, 2, 2, 14],
        ':' => [0, 4, 4, 0, 4, 4, 0],
        _ => [0; 7],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toolbar_hits_match_scaled_visual_layout() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            assert_eq!(
                hit_test(Point::new(30.0 * scale, 30.0 * scale), scale, false),
                Hit::Action(PenAction::Toggle)
            );
            assert_eq!(
                hit_test(Point::new(138.0 * scale, 34.0 * scale), scale, true),
                Hit::Action(PenAction::Color(1))
            );
            assert_eq!(
                hit_test(Point::new(350.0 * scale, 30.0 * scale), scale, true),
                Hit::Action(PenAction::Undo)
            );
            assert_eq!(
                hit_test(Point::new(350.0 * scale, 30.0 * scale), scale, false),
                Hit::Outside
            );
        }
        assert_eq!(hit_test(Point::new(f64::NAN, 0.0), 1.0, true), Hit::Outside);
    }
    #[test]
    fn all_geometry_is_finite_and_ink_is_in_image_space() {
        let mut ink = Drawing::default();
        ink.toggle();
        ink.begin(Point::new(200.0, 200.0), 1.0);
        let data = instances(&ink, 1.5);
        assert_eq!(data.len() % 9, 0);
        assert!(data.iter().all(|v| v.is_finite()));
        assert_eq!(data[5], 0.0);
        assert!(data[9..].as_chunks::<9>().0.iter().all(|p| p[5] == 1.0));
    }
}
