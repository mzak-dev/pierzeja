//! Mieszanie archetypów w wektorach: wagi (IDW / jawne / per wektor), mieszanie
//! wartości (liczby, OKLab, głosowanie), wariacja, nadpisania, normalizacja.
//! Port 1:1 z Pierzei (HANDOFF 4.4), sparametryzowany `&Dataset`.

use crate::color::{adjust_hex, blend_hex};
use crate::data::{Dataset, MapPoint, ParamDef, ParamKind, ParamMap, WeightMap};
use crate::random::{jround, js_u32, Mulberry32};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Wspólne pola spec każdego generatora (wchodzą do spec przez `#[serde(flatten)]`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MixSpec {
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
}

pub fn map_to_unit(ds: &Dataset, p: MapPoint) -> (f64, f64) {
    let [y0, y1] = ds.map_years;
    ((p.year - y0) / (y1 - y0), p.scale)
}

pub fn unit_to_map(ds: &Dataset, x: f64, y: f64) -> MapPoint {
    let [y0, y1] = ds.map_years;
    MapPoint { year: y0 + x * (y1 - y0), scale: y }
}

/// Wagi IDW (Shepard) z k najbliższych archetypów na mapie rok × skala.
pub fn map_weights(ds: &Dataset, cursor: MapPoint, sharpness: f64, neighbors: f64) -> WeightMap {
    let (cx, cy) = map_to_unit(ds, cursor);
    let mut d: Vec<(&str, f64)> = ds
        .archetypes
        .iter()
        .map(|a| {
            let (ax, ay) = map_to_unit(ds, a.map);
            (a.id.as_str(), (ax - cx).hypot(ay - cy))
        })
        .collect();
    d.sort_by(|p, q| p.1.total_cmp(&q.1));
    if d[0].1 < 0.012 {
        return IndexMap::from([(d[0].0.to_string(), 1.0)]);
    }
    let k = (neighbors.min(d.len() as f64).max(1.0)) as usize;
    let mut w: WeightMap = d[..k].iter().map(|&(id, dist)| (id.to_string(), 1.0 / dist.powf(sharpness))).collect();
    let sum: f64 = w.values().sum();
    w.values_mut().for_each(|v| *v /= sum);
    w
}

pub fn default_weights(ds: &Dataset) -> WeightMap {
    IndexMap::from([(ds.default_archetype.clone(), 1.0)])
}

/// Odrzuca nieznane id i wagi ≤ 0, normalizuje do sumy 1.
pub fn normalize_weights(ds: &Dataset, w: Option<&WeightMap>) -> WeightMap {
    let mut out: WeightMap = w.into_iter().flatten().filter(|(id, v)| ds.archetype(id).is_some() && **v > 0.0).map(|(k, v)| (k.clone(), *v)).collect();
    let sum: f64 = out.values().sum();
    if !(sum > 0.0) {
        return default_weights(ds);
    }
    out.values_mut().for_each(|v| *v /= sum);
    out
}

/// Nieznane id archetypów w spec (do komunikatów błędów API).
pub fn unknown_archetypes(ds: &Dataset, spec: &MixSpec) -> Vec<String> {
    let mut ids: Vec<&String> = Vec::new();
    ids.extend(spec.archetype.iter());
    ids.extend(spec.weights.iter().flat_map(|w| w.keys()));
    ids.extend(spec.group_weights.iter().flat_map(|g| g.values().flat_map(|w| w.keys())));
    let mut out: Vec<String> = ids.into_iter().filter(|id| ds.archetype(id).is_none()).cloned().collect();
    out.dedup();
    out
}

/// Wagi jednego wektora według pierwszeństwa pól specyfikacji.
pub fn group_weights(ds: &Dataset, spec: &MixSpec, group: &str) -> WeightMap {
    let sharp = spec.sharpness.unwrap_or(3.0);
    let k = spec.neighbors.unwrap_or(3.0);
    if let Some(w) = spec.group_weights.as_ref().and_then(|g| g.get(group)) {
        return normalize_weights(ds, Some(w));
    }
    if let Some(c) = spec.group_cursors.as_ref().and_then(|g| g.get(group)) {
        return map_weights(ds, *c, sharp, k);
    }
    if let Some(a) = spec.archetype.as_deref().filter(|a| ds.archetype(a).is_some()) {
        return IndexMap::from([(a.to_string(), 1.0)]);
    }
    if let Some(w) = &spec.weights {
        return normalize_weights(ds, Some(w));
    }
    if let Some(c) = spec.cursor {
        return map_weights(ds, c, sharp, k);
    }
    default_weights(ds)
}

fn blend_value(ds: &Dataset, def: &ParamDef, weights: &WeightMap) -> Value {
    match def.kind {
        ParamKind::Number | ParamKind::Int => {
            let v: f64 = weights.iter().map(|(id, w)| ds.vector(id)[&def.key].as_f64().unwrap_or(0.0) * w).fold(0.0, |a, b| a + b);
            clamp_param(def, &num(v))
        }
        ParamKind::Color => Value::String(blend_hex(weights.iter().map(|(id, w)| (ds.vector(id)[&def.key].as_str().unwrap_or("#808080"), *w)))),
        ParamKind::Enum => {
            let mut votes: IndexMap<&str, f64> = IndexMap::new();
            for (id, w) in weights {
                *votes.entry(ds.vector(id)[&def.key].as_str().unwrap_or("")).or_insert(0.0) += w;
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

pub fn apply_variation(ds: &Dataset, p: &mut ParamMap, seed: u32, variation: f64) {
    if !(variation > 0.0) {
        return;
    }
    let mut rnd = Mulberry32::new(seed ^ 0x9e37_79b9);
    for def in &ds.params {
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

/// Wektor parametrów po mieszaniu, wariacji i nadpisaniach (bez typowania):
/// (parametry, wagi per wektor, seed).
pub fn resolve_map(ds: &Dataset, spec: &MixSpec) -> (ParamMap, IndexMap<String, WeightMap>, u32) {
    let seed = spec.seed.filter(|s| s.is_finite()).map(|s| js_u32(s.floor())).unwrap_or(1);
    let weights: IndexMap<String, WeightMap> = ds.groups.iter().map(|g| (g.id.clone(), group_weights(ds, spec, &g.id))).collect();
    let mut p: ParamMap = ds.params.iter().map(|def| (def.key.clone(), blend_value(ds, def, &weights[def.group.as_str()]))).collect();
    apply_variation(ds, &mut p, seed, spec.variation.unwrap_or(0.0).clamp(0.0, 1.0));
    if let Some(o) = &spec.overrides {
        for (k, v) in o {
            p.insert(k.clone(), v.clone());
        }
        p = normalize_params(ds, &p);
    }
    (p, weights, seed)
}

/// Archetyp o największej łącznej wadze i rok szacowany z wag (średnia ważona `map.year`).
pub fn dominant_and_year(ds: &Dataset, weights: &IndexMap<String, WeightMap>) -> (String, i32) {
    let mut total: IndexMap<&str, f64> = IndexMap::new();
    let (mut year_sum, mut w_sum) = (0.0, 0.0);
    for g in weights.values() {
        for (id, w) in g {
            *total.entry(id.as_str()).or_insert(0.0) += w;
            year_sum += ds.archetype(id).map_or(0.0, |a| a.map.year) * w;
            w_sum += w;
        }
    }
    let mut dominant = ds.default_archetype.clone();
    let mut best = -1.0;
    for (id, w) in total {
        if w > best {
            best = w;
            dominant = id.to_string();
        }
    }
    (dominant, jround(year_sum / w_sum.max(1e-9)) as i32)
}

/// Czytelny opis mieszanki, np. „62% Kamienica · 38% Plomba”.
pub fn describe_mix(ds: &Dataset, w: &WeightMap, max: usize) -> String {
    let mut e: Vec<(&String, f64)> = w.iter().map(|(k, v)| (k, *v)).collect();
    e.sort_by(|a, b| b.1.total_cmp(&a.1));
    e.into_iter()
        .take(max)
        .filter(|(_, v)| *v >= 0.03)
        .map(|(id, v)| format!("{}% {}", jround(v * 100.0), ds.archetype(id).map_or(id.as_str(), |a| a.short.as_str())))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Wagi łączne (średnia po wektorach).
pub fn overall_weights(weights: &IndexMap<String, WeightMap>) -> WeightMap {
    let mut out = WeightMap::new();
    for g in weights.values() {
        for (id, w) in g {
            *out.entry(id.clone()).or_insert(0.0) += w / weights.len() as f64;
        }
    }
    out
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
            num(if def.kind == ParamKind::Int { jround(v) } else { v })
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
pub fn normalize_params(ds: &Dataset, p: &ParamMap) -> ParamMap {
    ds.params
        .iter()
        .map(|def| {
            let v = match p.get(&def.key) {
                Some(v) => clamp_param(def, v),
                None => ds.defaults.get(&def.key).cloned().unwrap_or_else(|| def.default.clone()),
            };
            (def.key.clone(), v)
        })
        .collect()
}

pub fn default_params(ds: &Dataset) -> ParamMap {
    normalize_params(ds, &ParamMap::new())
}
