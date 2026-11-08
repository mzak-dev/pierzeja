//! Tabor — proceduralny generator pojazdów (ProceduraSuite, ADR 0023).
//! Na razie tylko dane (`data/tabor-dane.json`); geometria to osobny krok.

use procedura_kit::data::Dataset;
use std::sync::OnceLock;

/// Dane Tabora: wektory, parametry, archetypy, mapa rok designu × klasa wielkości.
pub fn data() -> &'static Dataset {
    static D: OnceLock<Dataset> = OnceLock::new();
    D.get_or_init(|| Dataset::load(include_str!("../../../data/tabor-dane.json")))
}
