//! Dane referencyjne (parametry, archetypy, profile ulic) wczytywane z
//! `data/pierzeja-dane.json` — jedno źródło prawdy dla wszystkich aplikacji.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::OnceLock;

/// Wagi archetypów; kolejność wpisów ma znaczenie (sumy, remisy).
pub type WeightMap = IndexMap<String, f64>;
/// Pełny wektor parametrów w postaci ogólnej (klucze jak w JSON, camelCase).
pub type ParamMap = Map<String, Value>;

pub const GROUP_IDS: [&str; 5] = ["massing", "storeys", "roof", "facade", "material"];
pub const DEFAULT_ARCHETYPE: &str = "kamienica_eklektyczna";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamKind {
    Number,
    Int,
    Color,
    Enum,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub key: String,
    pub group: String,
    pub kind: ParamKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<EnumOption>>,
    pub default: Value,
    #[serde(default)]
    pub jitter: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct MapPoint {
    pub year: f64,
    pub scale: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Archetype {
    pub id: String,
    pub name: String,
    pub short: String,
    pub years: [i32; 2],
    pub map: MapPoint,
    pub description: String,
    pub features: Vec<String>,
    pub sources: Vec<SourceRef>,
    /// Tylko wartości różne od domyślnych.
    pub overrides: ParamMap,
    /// Pełny, znormalizowany wektor.
    pub vector: ParamMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CityProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub weights: WeightMap,
    pub mixing: f64,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Era {
    pub from: f64,
    pub to: f64,
    pub label: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub map_years: [f64; 2],
    pub eras: Vec<Era>,
    pub groups: Vec<Group>,
    pub params: Vec<ParamDef>,
    pub defaults: ParamMap,
    pub archetypes: Vec<Archetype>,
    pub city_profiles: Vec<CityProfile>,
}

impl Data {
    pub fn archetype(&self, id: &str) -> Option<&Archetype> {
        self.archetypes.iter().find(|a| a.id == id)
    }
    pub fn param(&self, key: &str) -> Option<&ParamDef> {
        self.params.iter().find(|d| d.key == key)
    }
    pub fn city(&self, id: &str) -> Option<&CityProfile> {
        self.city_profiles.iter().find(|c| c.id == id)
    }
    /// Pełny wektor archetypu (nieznany id → archetyp domyślny).
    pub fn vector(&self, id: &str) -> &ParamMap {
        &self.archetype(id).or_else(|| self.archetype(DEFAULT_ARCHETYPE)).expect("archetyp domyślny").vector
    }
}

pub fn data() -> &'static Data {
    static D: OnceLock<Data> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(include_str!("../../../data/pierzeja-dane.json")).expect("pierzeja-dane.json"))
}
