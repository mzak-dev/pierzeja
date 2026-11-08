//! Model danych generatora: parametry pogrupowane w wektory, archetypy, epoki.
//! Każde narzędzie wczytuje własny `<narzędzie>-dane.json` do `Dataset`.

use crate::mix::normalize_params;
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
    /// `true` = wartość ze źródła ([Ź] w raporcie), `false` = szacunek ([S]); brak = nieoznaczone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
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
    #[serde(default)]
    pub overrides: ParamMap,
    /// Pełny, znormalizowany wektor; gdy brak w JSON, liczy go `Dataset::load`.
    #[serde(default)]
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
    /// Wczytuje JSON i uzupełnia puste `vector` archetypów jako `normalize_params(defaults + overrides)`.
    /// Panikuje na błędnym JSON (dane są wkompilowane, błąd = błąd programisty).
    pub fn load(json: &str) -> Dataset {
        let mut ds: Dataset = serde_json::from_str(json).expect("dane generatora (JSON)");
        let fill: Vec<Option<ParamMap>> = ds.archetypes.iter().map(|a| a.vector.is_empty().then(|| normalize_params(&ds, &a.overrides))).collect();
        for (a, v) in ds.archetypes.iter_mut().zip(fill) {
            if let Some(v) = v {
                a.vector = v;
            }
        }
        ds
    }
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
