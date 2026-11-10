//! Dane referencyjne (parametry, archetypy, profile ulic) wczytywane z
//! `data/pierzeja-dane.json` — jedno źródło prawdy dla wszystkich aplikacji.
//! Model danych jest wspólny z ProceduraSuite (`procedura_kit::data`).

use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::sync::OnceLock;

pub use procedura_kit::data::{Archetype, Dataset, EnumOption, Era, Group, MapPoint, ParamDef, ParamKind, ParamMap, SourceRef, WeightMap};

/// Id wektorów budynku (kolejność jak w `groups` danych).
pub const GROUP_IDS: [&str; 5] = ["massing", "storeys", "roof", "facade", "material"];
pub const DEFAULT_ARCHETYPE: &str = "kamienica_eklektyczna";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CityProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub weights: WeightMap,
    pub mixing: f64,
    pub count: u32,
}

/// Dane Pierzei: wspólny `Dataset` + profile miast (ulice).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    #[serde(flatten)]
    pub base: Dataset,
    pub city_profiles: Vec<CityProfile>,
}

// ponytail: Deref, żeby `data().archetypes`, `data().param(…)` działały bez zmian w frontendach
impl Deref for Data {
    type Target = Dataset;
    fn deref(&self) -> &Dataset {
        &self.base
    }
}

impl Data {
    pub fn city(&self, id: &str) -> Option<&CityProfile> {
        self.city_profiles.iter().find(|c| c.id == id)
    }
}

pub fn data() -> &'static Data {
    static D: OnceLock<Data> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(include_str!("../../../data/pierzeja-dane.json")).expect("pierzeja-dane.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_data() {
        let ids: Vec<&str> = data().groups.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(ids, GROUP_IDS);
        assert_eq!(data().default_archetype, DEFAULT_ARCHETYPE);
    }
}
