//! Obrysy wielokątne i zespoły zabudowy: poprawna, skończona, deterministyczna geometria.

use pierzeja_core::scene::{generate_scene, SceneKind, SceneSpec};
use pierzeja_core::*;
use serde_json::json;

fn spec(v: serde_json::Value) -> BuildingSpec {
    serde_json::from_value(v).unwrap()
}

fn finite(m: &MeshData) -> bool {
    m.parts.iter().all(|p| p.positions.iter().chain(&p.normals).all(|x| x.is_finite()) && p.indices.iter().all(|&i| (i as usize) < p.positions.len() / 3))
}

#[test]
fn polygon_buildings_are_valid_and_deterministic() {
    let cases = [
        // narożnik ze ściętym rogiem i wieżyczką, mansarda
        json!({ "archetype": "kamienica_secesyjna", "seed": 3, "overrides": { "turret": 1 },
            "footprint": { "points": [[-10, 12], [-13, 9], [-13, -10], [-1, -10], [-1, 0], [13, 0], [13, 12]], "front": [0, 1, 6], "blind": [2, 5] } }),
        // blok w kształcie T, dach płaski z attyką, kilka klatek
        json!({ "archetype": "blok_plyta_5", "footprint": { "points": [[-30, 6], [30, 6], [30, -6], [6, -6], [6, -34], [-6, -34], [-6, -6], [-30, -6]] } }),
        // dom na nieregularnym sześciokącie (obieg przeciwny), dach kopertowy
        json!({ "archetype": "dom_90", "seed": 4, "footprint": { "points": [[0, 0], [9, -1], [13, 4], [11, 10], [3, 11], [-2, 6]] } }),
        // apartamentowiec z cofniętymi piętrami na obrysie wielokątnym
        json!({ "archetype": "apartamentowiec", "footprint": { "points": [[-16, 9], [16, 9], [16, -9], [-4, -9], [-16, 0]] } }),
    ];
    for c in cases {
        let s = spec(c.clone());
        let a = generate_from_spec(&s);
        let b = generate_from_spec(&s);
        assert!(finite(&a.mesh), "{c}");
        assert!(a.stats.triangles > 300, "{c}: {}", a.stats.triangles);
        assert!(!a.stats.notes.iter().any(|n| n.contains("nieprawidłowy")), "{c}");
        let glb = |m: &BuildingModel| to_glb(&[ExportNode { name: "x".into(), mesh: &m.mesh, offset: None, extras: None }]);
        assert_eq!(glb(&a), glb(&b), "{c}");
    }
    // pole zabudowy = pole obrysu
    let t = generate_from_spec(&spec(json!({ "archetype": "blok_plyta_5", "footprint": { "points": [[0, 0], [20, 0], [20, 10], [0, 10]] } })));
    assert!((t.stats.footprint - 200.0).abs() < 1e-6);
}

#[test]
fn invalid_footprint_falls_back_to_rectangle() {
    let m = generate_from_spec(&spec(json!({ "archetype": "kostka", "footprint": { "points": [[0, 0], [10, 10], [10, 0], [0, 10]] } })));
    assert!(m.stats.notes.iter().any(|n| n.contains("nieprawidłowy")));
    assert!((m.stats.footprint - 100.0).abs() < 1.0, "{}", m.stats.footprint);
}

#[test]
fn scenes_generate() {
    for kind in [SceneKind::Estate, SceneKind::Crossroads, SceneKind::Roundabout] {
        let sc = generate_scene(&SceneSpec { kind, ..Default::default() });
        let buildings = sc.items.iter().filter(|i| i.stats.is_some()).count();
        assert!(buildings >= 5, "{kind:?}: {buildings}");
        assert!(sc.items.iter().all(|i| finite(&i.mesh)));
        assert!(sc.bounds.max[0] > sc.bounds.min[0]);
    }
    // odstęp rzędów na osiedlu ≥ H (WT 2002 § 13), także przy współczynniku < 1
    let sc = generate_scene(&SceneSpec { kind: SceneKind::Estate, spacing: Some(0.8), ..Default::default() });
    assert!(sc.notes.iter().any(|n| n.contains("× H")));
}
