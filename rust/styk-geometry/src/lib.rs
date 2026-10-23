//! Pose evaluation: one keyframe -> back-to-front list of drawables.
//!
//! Ports the core of `buildGeometrySteps` in `src/pivot/geometry.mjs`:
//! segment chaining, bend arcs, object joins, colour override, z-order and object draw order.
//!
//! Not covered yet: tweening between keyframes, IK / rig constraints, movieclips, spatial
//! meshes, images, text, gradients, the camera, curved polygon edges and `_styk.colorMode`.
//! Segments that are images or text are skipped with a warning.

use std::collections::HashMap;
use styk_scene::{FigureType, Keyframe, Object, Primitive, Scene};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// Polyline stroked with round joins. `square_cap` is the "squareLine" primitive.
    Line { points: Vec<Point>, width: f32, square_cap: bool },
    /// `filled`: solid disc. Otherwise a ring; `white_fill` paints white inside it first
    /// (the plain "circle" primitive does, "circleClear" does not).
    Circle { center: Point, radius: f32, filled: bool, white_fill: bool, stroke_width: f32 },
    Polygon { points: Vec<Point> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Drawable {
    /// Index of the object in the keyframe.
    pub slot: usize,
    pub z: f32,
    pub rgb: [u8; 3],
    /// Per-primitive opacity, 0.0..=1.0.
    pub alpha: f32,
    /// Whole-object opacity, 0.0..=1.0 (the original composites each object as one layer).
    pub opacity: f32,
    pub shape: Shape,
}

#[derive(Debug, Clone, Default)]
pub struct FrameGeometry {
    /// Back to front.
    pub drawables: Vec<Drawable>,
    pub warnings: Vec<String>,
}

struct ObjectGeometry {
    joints: HashMap<u32, Point>,
    drawables: Vec<Drawable>,
}

/// Evaluate the stored pose of one keyframe (no interpolation).
pub fn evaluate_keyframe(scene: &Scene, frame: &Keyframe) -> FrameGeometry {
    let mut out = FrameGeometry::default();
    let count = frame.objects.len();

    // Parents before children, like the original's ready queue.
    let mut children: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut ready: Vec<usize> = Vec::new();
    for (slot, object) in frame.objects.iter().enumerate() {
        match &object.join {
            Some(join) => children.entry(join.target_object_slot).or_default().push(slot),
            None => ready.push(slot),
        }
    }

    let mut orders: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut built: Vec<Option<ObjectGeometry>> = (0..count).map(|_| None).collect();
    let mut queue = 0;
    while queue < ready.len() {
        let slot = ready[queue];
        queue += 1;
        if let Some(kids) = children.get(&slot) {
            ready.extend(kids.iter().copied());
        }
        let object = &frame.objects[slot];
        let Some(figure) = scene.figure(object.figure_type_id) else {
            out.warnings.push(format!("Object {slot} uses an unknown figure type"));
            continue;
        };

        let mut root = Point { x: object.root_x, y: object.root_y };
        if let Some(join) = &object.join {
            let target = built
                .get(join.target_object_slot)
                .and_then(|b| b.as_ref())
                .and_then(|b| b.joints.get(&join.target_handle_id))
                .copied();
            match target {
                Some(point) => root = point,
                None => out.warnings.push(format!("Unresolved join at object {slot}")),
            }
        }

        let order = orders
            .entry(figure.figure_type_id)
            .or_insert_with(|| segment_order(figure));
        built[slot] = Some(build_object(scene, object, slot, figure, order, root, &mut out.warnings));
    }

    let draw_order: Vec<usize> = frame.object_draw_order.clone().unwrap_or_else(|| (0..count).collect());
    for slot in draw_order {
        let Some(mut geometry) = built.get_mut(slot).and_then(Option::take) else {
            continue;
        };
        // Stable sort: ties keep polygons first, then segments in figure order.
        geometry.drawables.sort_by(|a, b| a.z.total_cmp(&b.z));
        out.drawables.extend(geometry.drawables);
    }
    for (slot, leftover) in built.iter().enumerate() {
        if leftover.is_none() && !ready.contains(&slot) {
            out.warnings.push(format!("Cyclic join or unresolved target at object {slot}"));
        }
    }
    out
}

/// Segment indices with every parent before its children.
fn segment_order(figure: &FigureType) -> Vec<usize> {
    let ids: HashMap<u32, usize> = figure.segments.iter().enumerate().map(|(i, s)| (s.segment_id, i)).collect();
    let mut children: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut order: Vec<usize> = Vec::new();
    for (i, segment) in figure.segments.iter().enumerate() {
        match ids.get(&segment.parent_id) {
            Some(parent) => children.entry(*parent).or_default().push(i),
            None => order.push(i),
        }
    }
    let mut queue = 0;
    while queue < order.len() {
        if let Some(kids) = children.get(&order[queue]) {
            let kids = kids.clone();
            order.extend(kids);
        }
        queue += 1;
    }
    order
}

fn build_object(
    scene: &Scene,
    object: &Object,
    slot: usize,
    figure: &FigureType,
    order: &[usize],
    root: Point,
    warnings: &mut Vec<String>,
) -> ObjectGeometry {
    let scale = f64::from(object.scale);
    let opacity = object.opacity();
    let mut joints: HashMap<u32, Point> = HashMap::new();
    joints.insert(0, root);
    let mut drawables: Vec<Drawable> = Vec::new();
    let mut warned_asset = false;

    for &i in order {
        let segment = &figure.segments[i];
        let start = joints.get(&segment.parent_id).copied().unwrap_or(root);
        let angle = object.angles.get(i).copied().unwrap_or(segment.angle_rad);
        let bend = if object.bend_flag {
            object.bends.get(i).copied().unwrap_or(0.0)
        } else {
            segment.bend_rad.unwrap_or(0.0)
        };
        let base_length = if object.len_flag {
            object.lengths.get(i).copied().unwrap_or(f64::from(segment.length))
        } else {
            f64::from(segment.length)
        };
        let length = base_length * scale;

        let points = arc(start, angle, length, bend);
        let end = points.last().copied().unwrap_or(start);
        joints.insert(segment.segment_id, end);

        let primitive = segment.primitive();
        let thickness = f64::from(segment.thickness) * scale.abs();
        let width = thickness as f32;
        let visible = segment.primitive_code.unwrap_or(0) != 0 || segment.thickness > 0.01;
        if !visible {
            continue;
        }

        let rgb = object_color(scene.version, segment.color_rgb.unwrap_or([0, 0, 0]), object);
        let alpha = segment.opacity();
        let z = segment.z_order.unwrap_or((i + 1) as f32);

        let shape = match primitive {
            Primitive::Line | Primitive::SquareLine => {
                if points.len() < 2 || width <= 0.0 {
                    continue;
                }
                Shape::Line { points, width, square_cap: primitive == Primitive::SquareLine }
            }
            Primitive::Circle | Primitive::CircleFill | Primitive::CircleClear => {
                let filled = primitive == Primitive::CircleFill || thickness >= length.abs();
                let radius = (length.abs() + if filled { thickness } else { 0.0 }) / 2.0;
                if radius == 0.0 {
                    continue;
                }
                Shape::Circle {
                    center: Point { x: (start.x + end.x) / 2.0, y: (start.y + end.y) / 2.0 },
                    radius: radius as f32,
                    filled,
                    white_fill: !filled && primitive == Primitive::Circle,
                    stroke_width: width,
                }
            }
            Primitive::Image | Primitive::Text => {
                if !warned_asset {
                    warnings.push(format!("Object {slot}: image and text segments are not drawn yet"));
                    warned_asset = true;
                }
                continue;
            }
        };
        drawables.push(Drawable { slot, z, rgb, alpha, opacity, shape });
    }

    // Polygons are listed before segments so equal-z ties paint them underneath.
    let mut polygons: Vec<Drawable> = Vec::new();
    for polyfill in &figure.polyfills {
        let Some(ids) = polyfill.extra.get("segment_ids").and_then(|v| v.as_array()) else {
            continue;
        };
        let mut points: Vec<Point> = ids
            .iter()
            .filter_map(|id| id.as_u64())
            .filter_map(|id| u32::try_from(id).ok())
            .filter_map(|id| joints.get(&id).copied())
            .collect();
        if points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        if points.len() < 3 {
            continue;
        }
        let alpha = match polyfill.alpha_raw {
            Some(raw) => 1.0 - f32::from(raw) / 255.0,
            None => polyfill.color_alpha.unwrap_or(1.0),
        };
        polygons.push(Drawable {
            slot,
            z: polyfill.z_order.unwrap_or(0.0),
            rgb: object_color(scene.version, polyfill.color_rgb.unwrap_or([0, 0, 0]), object),
            alpha,
            opacity,
            shape: Shape::Polygon { points },
        });
    }
    polygons.extend(drawables);
    ObjectGeometry { joints, drawables: polygons }
}

/// Points along a segment. Angles are absolute (not relative to the parent). A straight
/// segment is two points; a bent one is an arc sampled like the original.
fn arc(start: Point, angle: f64, length: f64, bend: f64) -> Vec<Point> {
    let (sx, sy) = (f64::from(start.x), f64::from(start.y));
    if !(sx.is_finite() && sy.is_finite() && angle.is_finite() && length.is_finite() && bend.is_finite()) {
        return Vec::new();
    }
    let round = |x: f64, y: f64| Point { x: x as f32, y: y as f32 };
    if bend.abs() < 1e-7 {
        return vec![start, round(sx + length * angle.cos(), sy + length * angle.sin())];
    }
    let steps = ((bend.abs() * 16.0).ceil().min((length.abs() / 4.0).ceil()).min(160.0)).max(8.0) as usize;
    let radius = length / bend;
    (0..=steps)
        .map(|i| {
            let a = angle + bend * i as f64 / steps as f64;
            round(sx + radius * (a.sin() - angle.sin()), sy + radius * (angle.cos() - a.cos()))
        })
        .collect()
}

/// Colour after the object's colour override (`color_override_raw`, packed 0xAABBGGRR).
/// Version 2-4 scenes use the override colour outright.
pub fn object_color(version: u8, base: [u8; 3], object: &Object) -> [u8; 3] {
    let raw = object.color_override_raw.unwrap_or(0) as u32;
    if version < 5 {
        return if (raw as i32) < 0 { [0, 0, 0] } else { unpack_rgb(raw) };
    }
    mix_color(base, raw)
}

fn unpack_rgb(raw: u32) -> [u8; 3] {
    [(raw & 255) as u8, ((raw >> 8) & 255) as u8, ((raw >> 16) & 255) as u8]
}

/// Blend `base` toward the override colour by the override's alpha byte (0 = no change).
pub fn mix_color(base: [u8; 3], raw: u32) -> [u8; 3] {
    let k = (raw >> 24) as i32;
    if k == 0 {
        return base;
    }
    let target = unpack_rgb(raw);
    if k == 255 {
        return target;
    }
    let mut out = base;
    for i in 0..3 {
        let v = i32::from(base[i]);
        out[i] = (v * 256 + k * (i32::from(target[i]) - v)).div_euclid(256) as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::f64::consts::FRAC_PI_2;

    fn scene(segments: Value, objects: Value) -> Scene {
        Scene::from_value(json!({
            "version": 5, "canvas_width": 100, "canvas_height": 100, "fps": 12,
            "figure_types": [{"figure_type_id": 1, "segments": segments}],
            "keyframes": [{"duration_raw": 1, "objects": objects}]
        }))
        .unwrap()
    }

    fn eval(scene: &Scene) -> FrameGeometry {
        evaluate_keyframe(scene, &scene.keyframes[0])
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn straight_segment_uses_root_and_scale() {
        let s = scene(
            json!([{"segment_id": 1, "parent_id": 0, "length": 10.0, "thickness": 2.0, "primitive_code": 0}]),
            json!([{"figure_type_id": 1, "unique_id": 1, "root_x": 5.0, "root_y": 5.0, "scale": 2.0, "angles": [0.0]}]),
        );
        let g = eval(&s);
        assert_eq!(g.drawables.len(), 1);
        match &g.drawables[0].shape {
            Shape::Line { points, width, .. } => {
                assert_eq!(points.len(), 2);
                assert!(near(points[0].x, 5.0) && near(points[0].y, 5.0));
                assert!(near(points[1].x, 25.0) && near(points[1].y, 5.0));
                assert!(near(*width, 4.0));
            }
            other => panic!("expected a line, got {other:?}"),
        }
    }

    #[test]
    fn child_starts_at_parent_end_and_angles_are_absolute() {
        let s = scene(
            json!([
                {"segment_id": 1, "parent_id": 0, "length": 10.0, "thickness": 2.0},
                {"segment_id": 2, "parent_id": 1, "length": 10.0, "thickness": 2.0}
            ]),
            json!([{"figure_type_id": 1, "unique_id": 1, "angles": [0.0, FRAC_PI_2]}]),
        );
        let g = eval(&s);
        let Shape::Line { points, .. } = &g.drawables[1].shape else { panic!() };
        assert!(near(points[0].x, 10.0) && near(points[0].y, 0.0));
        assert!(near(points[1].x, 10.0) && near(points[1].y, 10.0));
    }

    #[test]
    fn circles() {
        let s = scene(
            json!([
                {"segment_id": 1, "parent_id": 0, "length": 10.0, "thickness": 4.0, "primitive_code": 3},
                {"segment_id": 2, "parent_id": 0, "length": 10.0, "thickness": 2.0, "primitive_code": 1}
            ]),
            json!([{"figure_type_id": 1, "unique_id": 1, "angles": [0.0, 0.0]}]),
        );
        let g = eval(&s);
        let Shape::Circle { center, radius, filled, .. } = &g.drawables[0].shape else { panic!() };
        assert!(*filled && near(*radius, 7.0) && near(center.x, 5.0));
        let Shape::Circle { radius, filled, white_fill, stroke_width, .. } = &g.drawables[1].shape else { panic!() };
        assert!(!*filled && *white_fill && near(*radius, 5.0) && near(*stroke_width, 2.0));
    }

    #[test]
    fn joined_object_roots_at_parent_handle() {
        let s = scene(
            json!([{"segment_id": 1, "parent_id": 0, "length": 10.0, "thickness": 2.0}]),
            json!([
                {"figure_type_id": 1, "unique_id": 1, "angles": [0.0]},
                {"figure_type_id": 1, "unique_id": 2, "root_x": 999.0, "angles": [0.0],
                 "join": {"target_object_slot": 0, "target_handle_id": 1}}
            ]),
        );
        let g = eval(&s);
        assert_eq!(g.drawables.len(), 2);
        let Shape::Line { points, .. } = &g.drawables[1].shape else { panic!() };
        assert!(near(points[0].x, 10.0) && near(points[1].x, 20.0));
        assert!(g.warnings.is_empty());
    }

    #[test]
    fn z_order_sorts_within_an_object() {
        let s = scene(
            json!([
                {"segment_id": 1, "parent_id": 0, "length": 5.0, "thickness": 3.0, "z_order": 5.0},
                {"segment_id": 2, "parent_id": 0, "length": 5.0, "thickness": 4.0, "z_order": 1.0}
            ]),
            json!([{"figure_type_id": 1, "unique_id": 1, "angles": [0.0, 0.0]}]),
        );
        let g = eval(&s);
        assert!(near(g.drawables[0].z, 1.0) && near(g.drawables[1].z, 5.0));
    }

    #[test]
    fn bend_produces_an_arc() {
        let s = scene(
            json!([{"segment_id": 1, "parent_id": 0, "length": 10.0, "thickness": 2.0, "bend_rad": 1.0}]),
            json!([{"figure_type_id": 1, "unique_id": 1, "angles": [0.0]}]),
        );
        let g = eval(&s);
        let Shape::Line { points, .. } = &g.drawables[0].shape else { panic!() };
        assert_eq!(points.len(), 9);
        let last = points.last().unwrap();
        assert!(near(last.x, (10.0 * 1.0f64.sin()) as f32));
        assert!(near(last.y, (10.0 * (1.0 - 1.0f64.cos())) as f32));
    }

    #[test]
    fn colour_mixing() {
        assert_eq!(mix_color([100, 100, 100], 0), [100, 100, 100]);
        assert_eq!(mix_color([100, 100, 100], (255 << 24) | (30 << 16) | (20 << 8) | 10), [10, 20, 30]);
        // Half way from 100 toward the override channels (10, 0, 0).
        assert_eq!(mix_color([100, 100, 100], (128 << 24) | 10), [55, 50, 50]);
    }
}
