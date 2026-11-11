//! ProceduraSuite — wspólny kit generatorów Pierzeja, Tabor i Tłum (ADR 0021):
//! deterministyczna losowość, kolory OKLab, siatki flat-shaded, eksport GLB/OBJ,
//! dane generatora (`Dataset`) i mieszanie archetypów w wektorach (`mix`).

pub mod color;
pub mod data;
pub mod export;
pub mod krzywe;
pub mod mesh;
pub mod mix;
pub mod powloka;
pub mod random;
