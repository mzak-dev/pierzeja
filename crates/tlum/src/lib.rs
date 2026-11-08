//! Tłum — proceduralny generator ludzi (ProceduraSuite, ADR 0022).
//! Na razie tylko dane (`data/tlum-dane.json`); siatka to osobny krok.

use procedura_kit::data::Dataset;
use serde_json::Value;
use std::sync::OnceLock;

const JSON: &str = include_str!("../../../data/tlum-dane.json");

/// Dane Tłumu: wektory, parametry, archetypy (year = rok urodzenia, scale = formalność stroju).
pub fn data() -> &'static Dataset {
    static D: OnceLock<Dataset> = OnceLock::new();
    D.get_or_init(|| Dataset::load(JSON))
}

/// Cały dokument jako JSON: sekcje spoza `Dataset` (`szkielet`, `proporcje`, `palety`, `mundury`, …).
// ponytail: surowy Value zamiast struktur serde — typuj sekcje dopiero, gdy generator (issue 13) ich użyje.
pub fn extra() -> &'static Value {
    static V: OnceLock<Value> = OnceLock::new();
    V.get_or_init(|| serde_json::from_str(JSON).expect("tlum-dane.json"))
}
