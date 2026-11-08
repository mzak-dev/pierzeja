//! Model danych generatora: parametry pogrupowane w wektory, archetypy, epoki.
//! Każde narzędzie wczytuje własny `<narzędzie>-dane.json` do `Dataset`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Wagi archetypów; kolejność wpisów ma znaczenie (sumy, remisy).
pub type WeightMap = IndexMap<String, f64>;
/// Pełny wektor parametrów w postaci ogólnej (klucze jak w JSON, camelCase).
pub type ParamMap = Map<String, Value>;

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

/// Dane jednego generatora: wektory (`groups`), parametry, archetypy, mapa rok × skala.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dataset {
    pub default_archetype: String,
    pub map_years: [f64; 2],
    pub eras: Vec<Era>,
    pub groups: Vec<Group>,
    pub params: Vec<ParamDef>,
    pub defaults: ParamMap,
    pub archetypes: Vec<Archetype>,
}

impl Dataset {
    pub fn archetype(&self, id: &str) -> Option<&Archetype> {
        self.archetypes.iter().find(|a| a.id == id)
    }
    pub fn param(&self, key: &str) -> Option<&ParamDef> {
        self.params.iter().find(|d| d.key == key)
    }
    /// Pełny wektor archetypu (nieznany id → archetyp domyślny).
    pub fn vector(&self, id: &str) -> &ParamMap {
        &self.archetype(id).or_else(|| self.archetype(&self.default_archetype)).expect("archetyp domyślny").vector
    }
}
