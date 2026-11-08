//! Snapshot GLB: hash każdego z 19 archetypów (seed 3) musi zostać bez zmian przy
//! refaktorach (wydzielenie procedura-kit). Brak pliku wzorcowego → zapisz go.

use pierzeja_core::*;
use serde_json::json;

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

#[test]
fn glb_snapshot() {
    let mut got = String::new();
    for a in &data().archetypes {
        let spec: BuildingSpec = serde_json::from_value(json!({ "archetype": a.id, "seed": 3 })).unwrap();
        let m = generate_from_spec(&spec);
        let glb = to_glb(&[ExportNode { name: a.id.clone(), mesh: &m.mesh, offset: None, parent: None, rotation: None, extras: None }]);
        got += &format!("{} {:016x}\n", a.id, fnv1a(&glb));
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/glb_hashes.txt");
    match std::fs::read_to_string(&path) {
        Ok(want) => assert_eq!(got, want.replace("\r\n", "\n"), "GLB zmieniony; jeśli celowo, usuń {path:?}"),
        Err(_) => std::fs::write(&path, &got).unwrap(),
    }
}
