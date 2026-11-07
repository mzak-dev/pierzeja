//! Mieszanie wektorów: wagi archetypów (IDW / jawne / per wektor), mieszanie
//! wartości (liczby, OKLab, głosowanie), wariacja, nadpisania. Port 1:1.

use crate::color::{adjust_hex, blend_hex};
use crate::data::{data, MapPoint, ParamDef, ParamKind, ParamMap, WeightMap, DEFAULT_ARCHETYPE, GROUP_IDS};
use crate::params::{clamp_param, normalize_params, num, Params};
use crate::random::{jround, js_u32, Mulberry32};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Kontrakt wymiany (UI, MCP, Godot, eksport). Zgodny z prototypem.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archetype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<WeightMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<MapPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_weights: Option<IndexMap<String, WeightMap>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_cursors: Option<IndexMap<String, MapPoint>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<ParamMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variation: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharpness: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neighbors: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulations: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedBuilding {
    pub params: Params,
    pub weights: IndexMap<String, WeightMap>,
    /// Rok szacowany z wag.
    pub year: i32,
    pub seed: u32,
    pub regulations: bool,
    /// Archetyp o największej łącznej wadze.
    pub dominant: String,
}

pub fn map_to_unit(p: MapPoint) -> (f64, f64) {
    let [y0, y1] = data().map_years;
    ((p.year - y0) / (y1 - y0), p.scale)
}

pub fn unit_to_map(x: f64, y: f64) -> MapPoint {
    let [y0, y1] = data().map_years;
    MapPoint { year: y0 + x * (y1 - y0), scale: y }
}

/// Wagi IDW (Shepard) z k najbliższych archetypów na mapie rok × skala.
pub fn map_weights(cursor: MapPoint, sharpness: f64, neighbors: f64) -> WeightMap {
    let (cx, cy) = map_to_unit(cursor);
    let mut ds: Vec<(&str, f64)> = data()
        .archetypes
        .iter()
        .map(|a| {
            let (ax, ay) = map_to_unit(a.map);
            (a.id.as_str(), (ax - cx).hypot(ay - cy))
        })
        .collect();
    ds.sort_by(|p, q| p.1.total_cmp(&q.1));
    if ds[0].1 < 0.012 {
        return IndexMap::from([(ds[0].0.to_string(), 1.0)]);
    }
    let k = (neighbors.min(ds.len() as f64).max(1.0)) as usize;
    let mut w: WeightMap = ds[..k].iter().map(|&(id, d)| (id.to_string(), 1.0 / d.powf(sharpness))).collect();
    let sum: f64 = w.values().sum();
    w.values_mut().for_each(|v| *v /= sum);
    w
}

pub fn default_weights() -> WeightMap {
    IndexMap::from([(DEFAULT_ARCHETYPE.to_string(), 1.0)])
}

/// Odrzuca nieznane id i wagi ≤ 0, normalizuje do sumy 1.
pub fn normalize_weights(w: Option<&WeightMap>) -> WeightMap {
    let d = data();
    let mut out: WeightMap = w.into_iter().flatten().filter(|(id, v)| d.archetype(id).is_some() && **v > 0.0).map(|(k, v)| (k.clone(), *v)).collect();
    let sum: f64 = out.values().sum();
    if !(sum > 0.0) {
        return default_weights();
    }
    out.values_mut().for_each(|v| *v /= sum);
    out
}

/// Nieznane id archetypów w spec (do komunikatów błędów API).
pub fn unknown_archetypes(spec: &BuildingSpec) -> Vec<String> {
    let d = data();
    let mut ids: Vec<&String> = Vec::new();
    ids.extend(spec.archetype.iter());
    ids.extend(spec.weights.iter().flat_map(|w| w.keys()));
    ids.extend(spec.group_weights.iter().flat_map(|g| g.values().flat_map(|w| w.keys())));
    let mut out: Vec<String> = ids.into_iter().filter(|id| d.archetype(id).is_none()).cloned().collect();
    out.dedup();
    out
}

/// Wagi jednego wektora według pierwszeństwa pól specyfikacji.
pub fn group_weights(spec: &BuildingSpec, group: &str) -> WeightMap {
    let sharp = spec.sharpness.unwrap_or(3.0);
    let k = spec.neighbors.unwrap_or(3.0);
    if let Some(w) = spec.group_weights.as_ref().and_then(|g| g.get(group)) {
        return normalize_weights(Some(w));
    }
    if let Some(c) = spec.group_cursors.as_ref().and_then(|g| g.get(group)) {
        return map_weights(*c, sharp, k);
    }
    if let Some(a) = spec.archetype.as_deref().filter(|a| data().archetype(a).is_some()) {
        return IndexMap::from([(a.to_string(), 1.0)]);
    }
    if let Some(w) = &spec.weights {
        return normalize_weights(Some(w));
    }
    if let Some(c) = spec.cursor {
        return map_weights(c, sharp, k);
    }
    default_weights()
}

fn blend_value(def: &ParamDef, weights: &WeightMap) -> Value {
    let d = data();
    match def.kind {
        ParamKind::Number | ParamKind::Int => {
            let v: f64 = weights.iter().map(|(id, w)| d.vector(id)[&def.key].as_f64().unwrap_or(0.0) * w).fold(0.0, |a, b| a + b);
            clamp_param(def, &num(v))
        }
        ParamKind::Color => Value::String(blend_hex(weights.iter().map(|(id, w)| (d.vector(id)[&def.key].as_str().unwrap_or("#808080"), *w)))),
        ParamKind::Enum => {
            let mut votes: IndexMap<&str, f64> = IndexMap::new();
            for (id, w) in weights {
                *votes.entry(d.vector(id)[&def.key].as_str().unwrap_or("")).or_insert(0.0) += w;
            }
            let mut best = def.default.clone();
            let mut best_w = -1.0;
            for (val, w) in votes {
                if w > best_w {
                    best = Value::String(val.to_string());
                    best_w = w;
                }
            }
            best
        }
    }
}

pub fn apply_variation(p: &mut ParamMap, seed: u32, variation: f64) {
    if !(variation > 0.0) {
        return;
    }
    let mut rnd = Mulberry32::new(seed ^ 0x9e37_79b9);
    for def in &data().params {
        let (r1, r2, r3) = (rnd.next() * 2.0 - 1.0, rnd.next() * 2.0 - 1.0, rnd.next() * 2.0 - 1.0);
        let j = def.jitter;
        if j <= 0.0 {
            continue;
        }
        let Some(v) = p.get(&def.key).cloned() else { continue };
        match def.kind {
            ParamKind::Number | ParamKind::Int => {
                let v = v.as_f64().unwrap_or(0.0);
                let span = def.max.unwrap_or(1.0) - def.min.unwrap_or(0.0);
                let mut nv = v + r1 * variation * j * span;
                // nie twórz elementów z niczego (np. lukarn), tylko zmieniaj istniejące
                if v == 0.0 && def.min.unwrap_or(0.0) == 0.0 {
                    nv = 0.0;
                }
                p.insert(def.key.clone(), clamp_param(def, &num(nv)));
            }
            ParamKind::Color => {
                let c = adjust_hex(v.as_str().unwrap_or("#808080"), r1 * variation * j * 1.4, 1.0 + r2 * variation * 0.25, r3 * variation * 0.18);
                p.insert(def.key.clone(), Value::String(c));
            }
            ParamKind::Enum => {}
        }
    }
}

/// Wektor parametrów po mieszaniu, wariacji i nadpisaniach (bez typowania).
pub fn resolve_map(spec: &BuildingSpec) -> (ParamMap, IndexMap<String, WeightMap>, u32) {
    let d = data();
    let seed = spec.seed.filter(|s| s.is_finite()).map(|s| js_u32(s.floor())).unwrap_or(1);
    let weights: IndexMap<String, WeightMap> = GROUP_IDS.iter().map(|g| (g.to_string(), group_weights(spec, g))).collect();
    let mut p: ParamMap = d.params.iter().map(|def| (def.key.clone(), blend_value(def, &weights[def.group.as_str()]))).collect();
    apply_variation(&mut p, seed, spec.variation.unwrap_or(0.0).clamp(0.0, 1.0));
    if let Some(o) = &spec.overrides {
        for (k, v) in o {
            p.insert(k.clone(), v.clone());
        }
        p = normalize_params(&p);
    }
    (p, weights, seed)
}

/// Główna funkcja: specyfikacja → pełny wektor parametrów.
pub fn resolve_spec(spec: &BuildingSpec) -> ResolvedBuilding {
    let d = data();
    let (p, weights, seed) = resolve_map(spec);
    let mut total: IndexMap<&str, f64> = IndexMap::new();
    let (mut year_sum, mut w_sum) = (0.0, 0.0);
    for g in weights.values() {
        for (id, w) in g {
            *total.entry(id.as_str()).or_insert(0.0) += w;
            year_sum += d.archetype(id).map_or(0.0, |a| a.map.year) * w;
            w_sum += w;
        }
    }
    let mut dominant = DEFAULT_ARCHETYPE.to_string();
    let mut best = -1.0;
    for (id, w) in total {
        if w > best {
            best = w;
            dominant = id.to_string();
        }
    }
    ResolvedBuilding {
        params: Params::from_map(&p),
        weights,
        year: jround(year_sum / w_sum.max(1e-9)) as i32,
        seed,
        regulations: spec.regulations != Some(false),
        dominant,
    }
}

/// Czytelny opis mieszanki, np. „62% Kamienica · 38% Plomba”.
pub fn describe_mix(w: &WeightMap, max: usize) -> String {
    let mut e: Vec<(&String, f64)> = w.iter().map(|(k, v)| (k, *v)).collect();
    e.sort_by(|a, b| b.1.total_cmp(&a.1));
    e.into_iter()
        .take(max)
        .filter(|(_, v)| *v >= 0.03)
        .map(|(id, v)| format!("{}% {}", jround(v * 100.0), data().archetype(id).map_or(id.as_str(), |a| a.short.as_str())))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Wagi łączne (średnia po wektorach).
pub fn overall_weights(weights: &IndexMap<String, WeightMap>) -> WeightMap {
    let mut out = WeightMap::new();
    for g in weights.values() {
        for (id, w) in g {
            *out.entry(id.clone()).or_insert(0.0) += w / GROUP_IDS.len() as f64;
        }
    }
    out
}
