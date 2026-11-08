//! Parametry: normalizacja wartości ogólnych i typowany widok dla generatora.

use crate::data::{data, ParamMap};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Typowany wektor 71 parametrów (5 grup). Budowany z `ParamMap` po mieszaniu.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    // bryła
    pub segments: f64,
    pub segment_width: f64,
    pub depth: f64,
    pub wing_left: f64,
    pub wing_right: f64,
    pub wing_rear: f64,
    pub wing_length: f64,
    pub wing_width: f64,
    pub stagger: f64,
    pub setback_floors: f64,
    pub setback_depth: f64,
    pub street_setback: f64,
    pub side_gap: f64,
    // kondygnacje
    pub floors: f64,
    pub ground_floor_height: f64,
    pub floor_height: f64,
    pub floor_height_decay: f64,
    pub plinth_height: f64,
    pub basement_windows: f64,
    // dach
    pub roof_pitch: f64,
    pub roof_hip: f64,
    pub mansard: f64,
    pub ridge_along_front: f64,
    pub roof_overhang: f64,
    pub parapet_height: f64,
    pub dormers: f64,
    pub chimneys: f64,
    pub rooftop_boxes: f64,
    pub gable_glazing: f64,
    pub turret: f64,
    // elewacja
    pub bay_width: f64,
    pub corner_margin: f64,
    pub window_width: f64,
    pub window_height: f64,
    pub sill_height: f64,
    pub window_depth: f64,
    pub window_arch: f64,
    pub mullions: f64,
    pub ribbon: f64,
    pub ground_shop: f64,
    pub ground_recess: f64,
    pub entrance_type: String,
    pub entrance_canopy: f64,
    pub stair_windows: f64,
    pub balconies: f64,
    pub balcony_from: f64,
    pub balcony_depth: f64,
    pub balcony_width: f64,
    pub loggia: f64,
    pub railing: f64,
    pub oriels: f64,
    pub oriel_depth: f64,
    pub cornice: f64,
    pub string_courses: f64,
    pub ornament: f64,
    pub pilasters: f64,
    pub rustication: f64,
    pub panel_joints: f64,
    pub side_windows: f64,
    pub asymmetry: f64,
    // materiał
    pub wall_texture: String,
    pub wall_color: String,
    pub trim_color: String,
    pub plinth_color: String,
    pub accent_color: String,
    pub accent_panels: f64,
    pub roof_texture: String,
    pub roof_color: String,
    pub frame_color: String,
    pub glass_tint: String,
    pub weathering: f64,
}

impl Params {
    pub fn from_map(m: &ParamMap) -> Params {
        serde_json::from_value(Value::Object(normalize_params(m))).expect("pełny wektor parametrów")
    }
    pub fn to_map(&self) -> ParamMap {
        match serde_json::to_value(self) {
            Ok(Value::Object(m)) => m,
            _ => unreachable!(),
        }
    }
}

pub use procedura_kit::mix::{clamp_param, num};

/// Uzupełnia brakujące wartości domyślnymi i przycina wszystko do zakresów
/// (nieznane klucze są pomijane). Kolejność kluczy = kolejność definicji.
pub fn normalize_params(p: &ParamMap) -> ParamMap {
    procedura_kit::mix::normalize_params(data(), p)
}

pub fn default_params() -> ParamMap {
    procedura_kit::mix::default_params(data())
}
