//! Pierzeja — proceduralny generator budynków (Polska 1800–2015).
//!
//! `BuildingSpec` (JSON) → wagi archetypów → mieszanie wektorów → gramatyka
//! budynku → `MeshData` (części per materiał). Rdzeń bez zależności od
//! silnika: używają go Godot (GDExtension), serwer MCP i aplikacja natywna.
//! Specyfikacja algorytmu: `docs/HANDOFF.md`.

pub mod blend;
pub mod color;
pub mod data;
pub mod export;
pub mod footprint;
pub mod generator;
pub mod materials;
pub mod mesh;
pub mod params;
pub mod random;
pub mod scene;
pub mod street;

pub use blend::{describe_mix, map_weights, normalize_weights, overall_weights, resolve_spec, unknown_archetypes, BuildingSpec, ResolvedBuilding};
pub use data::{data, Archetype, CityProfile, MapPoint, ParamDef, ParamKind, WeightMap, DEFAULT_ARCHETYPE, GROUP_IDS};
pub use export::{to_glb, to_obj, ExportNode};
pub use generator::{generate_building, generate_from_spec, BuildingModel, BuildingStats};
pub use mesh::{Bounds, MatKey, MaterialSpec, MeshData, MeshPart};
pub use params::Params;
pub use street::{describe_street, generate_street, StreetModel, StreetSpec};
