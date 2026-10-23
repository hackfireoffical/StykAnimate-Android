//! Paints `styk-geometry` drawables with egui. No scene logic lives here.

use eframe::egui::{self, Color32, Pos2, Stroke};
use styk_geometry::{Drawable, FrameGeometry, Point, Shape};

/// Maps scene pixels to screen pixels.
pub struct View {
    pub origin: Pos2,
    pub scale: f32,
}

impl View {
    fn map(&self, p: Point) -> Pos2 {
        Pos2::new(self.origin.x + p.x * self.scale, self.origin.y + p.y * self.scale)
    }
}

pub fn paint(painter: &egui::Painter, view: &View, geometry: &FrameGeometry) {
    for drawable in &geometry.drawables {
        paint_one(painter, view, drawable);
    }
}

fn color(d: &Drawable) -> Color32 {
    // Object opacity is multiplied in per primitive. Overlaps inside one translucent object
    // therefore show through; the original composites the object as a single layer.
    let a = (d.alpha * d.opacity).clamp(0.0, 1.0);
    Color32::from_rgba_unmultiplied(d.rgb[0], d.rgb[1], d.rgb[2], (a * 255.0).round() as u8)
}

fn paint_one(painter: &egui::Painter, view: &View, d: &Drawable) {
    let c = color(d);
    match &d.shape {
        Shape::Line { points, width, square_cap } => {
            let w = (width * view.scale).max(1.0);
            let pts: Vec<Pos2> = points.iter().map(|p| view.map(*p)).collect();
            let (Some(first), Some(last)) = (pts.first().copied(), pts.last().copied()) else {
                return;
            };
            painter.add(egui::Shape::line(pts, Stroke::new(w, c)));
            if !square_cap {
                // egui strokes have flat caps; the original uses round caps.
                painter.circle_filled(first, w / 2.0, c);
                painter.circle_filled(last, w / 2.0, c);
            }
        }
        Shape::Circle { center, radius, filled, white_fill, stroke_width } => {
            let center = view.map(*center);
            let r = radius * view.scale;
            if *filled {
                painter.circle_filled(center, r, c);
                return;
            }
            if *white_fill {
                let a = (d.alpha * d.opacity).clamp(0.0, 1.0);
                painter.circle_filled(center, r, Color32::from_white_alpha((a * 255.0).round() as u8));
            }
            let w = stroke_width * view.scale;
            if w > 0.0 {
                painter.circle_stroke(center, r, Stroke::new(w.max(1.0), c));
            }
        }
        Shape::Polygon { points } => {
            let pts: Vec<Pos2> = points.iter().map(|p| view.map(*p)).collect();
            // convex_polygon fills concave outlines incorrectly; proper triangulation comes later.
            painter.add(egui::Shape::convex_polygon(pts, c, Stroke::NONE));
        }
    }
}
