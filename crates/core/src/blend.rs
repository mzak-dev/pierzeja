//! Mieszanie wektorów budynku: cienka warstwa nad `procedura_kit::mix`
//! (wagi IDW / jawne / per wektor, mieszanie, wariacja, nadpisania) z danymi Pierzei.

use crate::data::{data, MapPoint, ParamMap, WeightMap};
use crate::params::Params;
use indexmap::IndexMap;
use procedura_kit::mix;
pub use procedura_kit::mix::MixSpec;
use serde::{Deserialize, Serialize};

/// Kontrakt wymiany (UI, MCP, Godot, eksport). Zgodny z prototypem; pola
/// mieszania są wspólne z ProceduraSuite (`MixSpec`), JSON bez zmian.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingSpec {
    #[serde(flatten)]
    pub mix: MixSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulations: Option<bool>,
    /// Obrys wielokątny [x, z] zamiast prostokąta z parametrów bryły (np. z OSM).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footprint: Option<crate::footprint::Footprint>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footprint: Option<crate::footprint::Footprint>,
}

pub fn map_to_unit(p: MapPoint) -> (f64, f64) {
    mix::map_to_unit(data(), p)
}

pub fn unit_to_map(x: f64, y: f64) -> MapPoint {
    mix::unit_to_map(data(), x, y)
}

/// Wagi IDW (Shepard) z k najbliższych archetypów na mapie rok × skala.
pub fn map_weights(cursor: MapPoint, sharpness: f64, neighbors: f64) -> WeightMap {
    mix::map_weights(data(), cursor, sharpness, neighbors)
}

pub fn default_weights() -> WeightMap {
    mix::default_weights(data())
}

/// Odrzuca nieznane id i wagi ≤ 0, normalizuje do sumy 1.
pub fn normalize_weights(w: Option<&WeightMap>) -> WeightMap {
    mix::normalize_weights(data(), w)
}

/// Nieznane id archetypów w spec (do komunikatów błędów API).
pub fn unknown_archetypes(spec: &BuildingSpec) -> Vec<String> {
    mix::unknown_archetypes(data(), &spec.mix)
}

/// Wagi jednego wektora według pierwszeństwa pól specyfikacji.
pub fn group_weights(spec: &BuildingSpec, group: &str) -> WeightMap {
    mix::group_weights(data(), &spec.mix, group)
}

pub fn apply_variation(p: &mut ParamMap, seed: u32, variation: f64) {
    mix::apply_variation(data(), p, seed, variation)
}

/// Wektor parametrów po mieszaniu, wariacji i nadpisaniach (bez typowania).
pub fn resolve_map(spec: &BuildingSpec) -> (ParamMap, IndexMap<String, WeightMap>, u32) {
    mix::resolve_map(data(), &spec.mix)
}

/// Główna funkcja: specyfikacja → pełny wektor parametrów.
pub fn resolve_spec(spec: &BuildingSpec) -> ResolvedBuilding {
    let (p, weights, seed) = resolve_map(spec);
    let (dominant, year) = mix::dominant_and_year(data(), &weights);
    ResolvedBuilding {
        params: Params::from_map(&p),
        weights,
        year,
        seed,
        regulations: spec.regulations != Some(false),
        dominant,
        footprint: spec.footprint.clone(),
    }
}

/// Czytelny opis mieszanki, np. „62% Kamienica · 38% Plomba”.
pub fn describe_mix(w: &WeightMap, max: usize) -> String {
    mix::describe_mix(data(), w, max)
}

pub use mix::overall_weights;
