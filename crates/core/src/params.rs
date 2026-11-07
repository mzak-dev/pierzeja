//! Parametry: normalizacja wartości ogólnych i typowany widok dla generatora.

use crate::data::{data, ParamDef, ParamKind, ParamMap};
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

fn as_f64(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        // ponytail: parseFloat tylko dla czystych liczb w tekście („3.5”), bez „3.5m”
        Value::String(s) => s.trim().parse().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn num(v: f64) -> Value {
    serde_json::Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null)
}

/// Przycina wartość do zakresu definicji (liczby całkowite zaokrągla jak `Math.round`).
pub fn clamp_param(def: &ParamDef, value: &Value) -> Value {
    match def.kind {
        ParamKind::Number | ParamKind::Int => {
            let mut v = as_f64(value);
            if !v.is_finite() {
                v = def.default.as_f64().unwrap_or(0.0);
            }
            v = def.max.unwrap_or(v).min(def.min.unwrap_or(v).max(v));
            num(if def.kind == ParamKind::Int { crate::random::jround(v) } else { v })
        }
        ParamKind::Enum => {
            let s = js_string(value);
            let ok = def.options.as_ref().is_some_and(|o| o.iter().any(|o| o.value == s));
            if ok { Value::String(s) } else { def.default.clone() }
        }
        ParamKind::Color => {
            let s = js_string(value);
            let ok = s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit());
            if ok { Value::String(s.to_lowercase()) } else { def.default.clone() }
        }
    }
}

/// Uzupełnia brakujące wartości domyślnymi i przycina wszystko do zakresów
/// (nieznane klucze są pomijane). Kolejność kluczy = kolejność definicji.
pub fn normalize_params(p: &ParamMap) -> ParamMap {
    let d = data();
    d.params
        .iter()
        .map(|def| {
            let v = match p.get(&def.key) {
                Some(v) => clamp_param(def, v),
                None => d.defaults.get(&def.key).cloned().unwrap_or_else(|| def.default.clone()),
            };
            (def.key.clone(), v)
        })
        .collect()
}

pub fn default_params() -> ParamMap {
    normalize_params(&ParamMap::new())
}
