//! Typed view of a decoded StykAnimate scene (the JSON document inside `.styk` / `.stkz`).
//!
//! Mirrors the shape checked by `validateScene` in `src/pivot/model.mjs`. Unknown fields are
//! ignored, so newer files still load; this crate only models what the renderer needs.
//! Feed it the `serde_json::Value` returned by `styk_format::decode`.

use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneError(pub String);

impl std::fmt::Display for SceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SceneError {}

fn fail<T>(message: impl Into<String>) -> Result<T, SceneError> {
    Err(SceneError(message.into()))
}

fn one() -> f32 {
    1.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct Scene {
    pub version: u8,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub fps: u32,
    pub frame_delay_ms: Option<u32>,
    #[serde(default)]
    pub figure_types: Vec<FigureType>,
    #[serde(default)]
    pub keyframes: Vec<Keyframe>,
    #[serde(default)]
    pub backgrounds: Vec<Background>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FigureType {
    pub figure_type_id: u32,
    #[serde(default)]
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub polyfills: Vec<Polyfill>,
}

/// One bone/shape of a figure. `parent_id` 0 means the figure origin.
#[derive(Debug, Clone, Deserialize)]
pub struct Segment {
    pub segment_id: u32,
    #[serde(default)]
    pub parent_id: u32,
    #[serde(default)]
    pub length: f32,
    #[serde(default)]
    pub angle_rad: f64,
    #[serde(default)]
    pub thickness: f32,
    pub color_rgb: Option<[u8; 3]>,
    /// 0 = opaque, 255 = fully transparent.
    pub transparency: Option<u8>,
    pub primitive_code: Option<u8>,
    pub aux_code: Option<u8>,
    pub z_order: Option<f32>,
    pub bend_rad: Option<f64>,
    pub secondary: Option<Gradient>,
}

impl Segment {
    pub fn primitive(&self) -> Primitive {
        Primitive::from_code(self.primitive_code.unwrap_or(0)).unwrap_or(Primitive::Line)
    }

    /// Opacity 0.0..=1.0 from the 0..=255 transparency byte.
    pub fn opacity(&self) -> f32 {
        1.0 - f32::from(self.transparency.unwrap_or(0)) / 255.0
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Gradient {
    pub rgb: [u8; 3],
    pub a: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Primitive {
    Line,
    Circle,
    Image,
    CircleFill,
    CircleClear,
    Text,
    SquareLine,
}

impl Primitive {
    pub fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Primitive::Line,
            1 => Primitive::Circle,
            2 => Primitive::Image,
            3 => Primitive::CircleFill,
            4 => Primitive::CircleClear,
            5 => Primitive::Text,
            6 => Primitive::SquareLine,
            _ => return None,
        })
    }
}

/// Filled polygon. Vertex data is kept untouched in `extra` until the renderer needs it.
#[derive(Debug, Clone, Deserialize)]
pub struct Polyfill {
    pub color_rgb: Option<[u8; 3]>,
    pub color_alpha: Option<f32>,
    pub alpha_raw: Option<u8>,
    pub z_order: Option<f32>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Keyframe {
    #[serde(default)]
    pub objects: Vec<Object>,
    #[serde(default)]
    pub duration_raw: u32,
    pub background_id: Option<u32>,
    pub object_draw_order: Option<Vec<usize>>,
    pub camera: Option<Camera>,
}

/// One placed figure in a keyframe. Pose arrays are indexed in `FigureType::segments` order.
#[derive(Debug, Clone, Deserialize)]
pub struct Object {
    pub figure_type_id: u32,
    pub unique_id: u32,
    #[serde(default)]
    pub root_x: f32,
    #[serde(default)]
    pub root_y: f32,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default)]
    pub angles: Vec<f64>,
    #[serde(default)]
    pub lengths: Vec<f64>,
    #[serde(default)]
    pub bends: Vec<f64>,
    #[serde(default)]
    pub len_flag: bool,
    #[serde(default)]
    pub bend_flag: bool,
    pub join: Option<Join>,
    pub color_override_raw: Option<i64>,
    /// 0 = opaque, 255 = fully transparent.
    pub field28: Option<u8>,
}

impl Object {
    pub fn opacity(&self) -> f32 {
        1.0 - f32::from(self.field28.unwrap_or(0)) / 255.0
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Join {
    pub target_object_slot: usize,
    pub target_handle_id: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Camera {
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default)]
    pub angle_rad: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Background {
    pub background_id: u32,
    #[serde(default)]
    pub kind: u8,
    pub color: Option<u32>,
    pub color_a: Option<u32>,
    pub color_b: Option<u32>,
    pub x1: Option<f32>,
    pub y1: Option<f32>,
    pub x2: Option<f32>,
    pub y2: Option<f32>,
}

impl Scene {
    /// Parse and validate a decoded document.
    pub fn from_value(value: Value) -> Result<Scene, SceneError> {
        let scene: Scene = serde_json::from_value(value)
            .map_err(|error| SceneError(format!("The scene is not a valid StykAnimate document: {error}")))?;
        scene.validate()?;
        Ok(scene)
    }

    pub fn figure(&self, id: u32) -> Option<&FigureType> {
        self.figure_types.iter().find(|f| f.figure_type_id == id)
    }

    pub fn validate(&self) -> Result<(), SceneError> {
        if !(2..=5).contains(&self.version) {
            return fail("Expected a decoded StykAnimate scene (version 2 to 5).");
        }
        if !(1..=100_000).contains(&self.canvas_width) || !(1..=100_000).contains(&self.canvas_height) {
            return fail("Canvas dimensions must be integers from 1 to 100000.");
        }
        if !(1..=1000).contains(&self.fps) {
            return fail("FPS must be between 1 and 1000.");
        }
        if self.keyframes.is_empty() || self.keyframes.len() > 100_000 {
            return fail("The scene must have between 1 and 100000 keyframes.");
        }

        let mut types: HashMap<u32, &FigureType> = HashMap::new();
        for figure in &self.figure_types {
            if types.insert(figure.figure_type_id, figure).is_some() {
                return fail("Invalid or repeated figure type.");
            }
            validate_segments(figure)?;
        }

        for frame in &self.keyframes {
            if frame.duration_raw > 65_535 {
                return fail("Invalid keyframe duration.");
            }
            let mut uids = HashSet::new();
            for object in &frame.objects {
                let Some(figure) = types.get(&object.figure_type_id) else {
                    return fail("Unknown figure type used by an object.");
                };
                if !uids.insert(object.unique_id) {
                    return fail("Repeated object identity in a keyframe.");
                }
                let count = figure.segments.len();
                if object.angles.len() != count
                    || (object.len_flag && object.lengths.len() != count)
                    || (object.bend_flag && object.bends.len() != count)
                {
                    return fail("A pose array does not match its figure.");
                }
                if object.scale < 0.0 {
                    return fail("Object scale cannot be negative.");
                }
                if let Some(join) = &object.join {
                    let Some(parent) = frame.objects.get(join.target_object_slot) else {
                        return fail("A join points to a missing figure.");
                    };
                    let handles = types.get(&parent.figure_type_id);
                    let known = join.target_handle_id == 0
                        || handles.map_or(false, |f| f.segments.iter().any(|s| s.segment_id == join.target_handle_id));
                    if !known {
                        return fail("A join points to a missing handle.");
                    }
                }
            }
            if let Some(order) = &frame.object_draw_order {
                let unique: HashSet<_> = order.iter().collect();
                if order.len() != frame.objects.len()
                    || unique.len() != order.len()
                    || order.iter().any(|i| *i >= frame.objects.len())
                {
                    return fail("Invalid figure layer order.");
                }
            }
            if let Some(camera) = &frame.camera {
                if !camera.is_default && camera.scale <= 0.0 {
                    return fail("Camera scale must be positive.");
                }
            }
        }
        Ok(())
    }
}

fn validate_segments(figure: &FigureType) -> Result<(), SceneError> {
    if figure.segments.len() > 30_000 {
        return fail("A figure supports at most 30000 segments.");
    }
    let mut parents: HashMap<u32, u32> = HashMap::new();
    for segment in &figure.segments {
        if !(1..=65_535).contains(&segment.segment_id) || parents.insert(segment.segment_id, segment.parent_id).is_some() {
            return fail("Invalid or repeated segment handle.");
        }
        if matches!(segment.primitive_code, Some(code) if code > 6) {
            return fail("Segment shape must be from 0 to 6.");
        }
    }
    // Every chain must reach the origin (id 0) without a loop.
    for start in parents.keys() {
        let mut id = *start;
        let mut steps = 0usize;
        while id != 0 {
            let Some(parent) = parents.get(&id) else {
                return fail("A figure has a missing parent.");
            };
            id = *parent;
            steps += 1;
            if steps > parents.len() {
                return fail("A figure has a parent cycle.");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "version": 5,
            "canvas_width": 640,
            "canvas_height": 360,
            "fps": 12,
            "figure_types": [{
                "figure_type_id": 1,
                "segments": [
                    {"segment_id": 1, "parent_id": 0, "length": 40.0, "angle_rad": 1.5, "thickness": 8.0,
                     "color_rgb": [10, 20, 30], "primitive_code": 0, "aux_code": 0},
                    {"segment_id": 2, "parent_id": 1, "length": 12.0, "angle_rad": 0.0, "thickness": 8.0,
                     "primitive_code": 1, "transparency": 51}
                ],
                "polyfills": [{"color_rgb": [1, 2, 3], "points": [1, 2, 3]}]
            }],
            "keyframes": [{
                "duration_raw": 3,
                "objects": [{
                    "figure_type_id": 1, "unique_id": 7, "root_x": 100.0, "root_y": 50.0, "scale": 1.0,
                    "angles": [0.1, 0.2], "field28": 0
                }],
                "object_draw_order": [0],
                "camera": {"is_default": true}
            }],
            "future_field": {"ignored": true}
        })
    }

    #[test]
    fn parses_a_valid_scene() {
        let scene = Scene::from_value(sample()).unwrap();
        assert_eq!(scene.canvas_width, 640);
        let figure = scene.figure(1).unwrap();
        assert_eq!(figure.segments.len(), 2);
        assert_eq!(figure.segments[0].primitive(), Primitive::Line);
        assert_eq!(figure.segments[1].primitive(), Primitive::Circle);
        assert!((figure.segments[1].opacity() - 0.8).abs() < 1e-6);
        assert!(figure.polyfills[0].extra.contains_key("points"));
        assert_eq!(scene.keyframes[0].objects[0].angles, vec![0.1, 0.2]);
    }

    #[test]
    fn rejects_unknown_figure() {
        let mut doc = sample();
        doc["keyframes"][0]["objects"][0]["figure_type_id"] = json!(99);
        assert!(Scene::from_value(doc).is_err());
    }

    #[test]
    fn rejects_short_pose_array() {
        let mut doc = sample();
        doc["keyframes"][0]["objects"][0]["angles"] = json!([0.1]);
        assert!(Scene::from_value(doc).is_err());
    }

    #[test]
    fn rejects_parent_cycle() {
        let mut doc = sample();
        doc["figure_types"][0]["segments"][0]["parent_id"] = json!(2);
        assert!(Scene::from_value(doc).is_err());
    }

    #[test]
    fn rejects_bad_draw_order_and_version() {
        let mut doc = sample();
        doc["keyframes"][0]["object_draw_order"] = json!([0, 0]);
        assert!(Scene::from_value(doc).is_err());
        let mut doc = sample();
        doc["version"] = json!(9);
        assert!(Scene::from_value(doc).is_err());
    }
}
