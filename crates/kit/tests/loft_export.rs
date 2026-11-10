use procedura_kit::export::{to_glb, ExportNode};
use procedura_kit::mesh::{MatKey, MeshBuilder, MeshData, Xform};
use serde_json::Value;

/// JSON z kawałka JSON w GLB.
fn glb_json(glb: &[u8]) -> Value {
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + len]).unwrap()
}

fn square(z: f64, h: f64) -> Vec<[f64; 3]> {
    // CCW w XY patrząc z +Z
    vec![[-h, -h, z], [h, -h, z], [h, h, z], [-h, h, z]]
}

fn mesh_of(mb: MeshBuilder) -> MeshData {
    mb.build(vec![])
}

#[test]
fn loft_normals_point_outward() {
    let mut mb = MeshBuilder::default();
    mb.loft(MatKey::Paint, &[square(0.0, 1.0), square(1.0, 0.5), square(2.0, 1.0)], true);
    let m = mesh_of(mb);
    // 2 pasy × 4 ściany × 2 trójkąty + 2 zaślepki po 2 trójkąty
    assert_eq!(m.triangles(), 2 * 4 * 2 + 2 * 2);
    let c = [0.0, 0.0, 1.0];
    for part in &m.parts {
        for t in part.indices.chunks_exact(3) {
            let p = |i: u32| &part.positions[i as usize * 3..i as usize * 3 + 3];
            let n = &part.normals[t[0] as usize * 3..t[0] as usize * 3 + 3];
            let mid: Vec<f64> = (0..3).map(|k| (p(t[0])[k] + p(t[1])[k] + p(t[2])[k]) as f64 / 3.0 - c[k]).collect();
            let d: f64 = (0..3).map(|k| n[k] as f64 * mid[k]).sum();
            assert!(d > 0.0, "normalna do wewnątrz: {mid:?} · {n:?}");
        }
    }
}

#[test]
fn xform_composes_and_pushes() {
    let q = std::f64::consts::FRAC_PI_2;
    let x = Xform::rot_y(q);
    let p = x.point([0.0, 0.0, 1.0]);
    assert!((p[0] - 1.0).abs() < 1e-12 && p[2].abs() < 1e-12);
    // najpierw obrót, potem przesunięcie
    let m = Xform::translate([0.0, 2.0, 0.0]).mul(&x);
    let p = m.point([0.0, 0.0, 1.0]);
    assert!((p[0] - 1.0).abs() < 1e-12 && (p[1] - 2.0).abs() < 1e-12);
    assert_eq!(Xform::identity().point([1.0, 2.0, 3.0]), [1.0, 2.0, 3.0]);

    let mut a = MeshBuilder::default();
    a.bx(MatKey::Paint, [0.0; 3], [1.0; 3]);
    let cube = mesh_of(a);
    let mut b = MeshBuilder::default();
    b.push(&cube, &Xform::translate([10.0, 0.0, 0.0]));
    let moved = mesh_of(b);
    assert_eq!(moved.triangles(), cube.triangles());
    assert_eq!(moved.bounds.min, [10.0, 0.0, 0.0]);
    assert_eq!(moved.bounds.max, [11.0, 1.0, 1.0]);
}

#[test]
fn glb_shares_meshes_and_nests_nodes() {
    let mut a = MeshBuilder::default();
    a.bx(MatKey::Paint, [0.0; 3], [1.0; 3]);
    let cube = mesh_of(a);
    let empty = MeshData::empty();
    let n = |name: &str, mesh, parent, rotation| ExportNode { name: name.into(), mesh, offset: Some([1.0, 0.0, 0.0]), parent, rotation, extras: None };
    let nodes = [n("Body", &cube, None, None), n("W1", &cube, Some(0), Some([0.0, 1.0, 0.0, 0.0])), n("Anchor", &empty, Some(0), None)];
    let js = glb_json(&to_glb(&nodes));
    assert_eq!(js["meshes"].as_array().unwrap().len(), 1, "te same &MeshData → jeden mesh");
    assert_eq!(js["nodes"][0]["mesh"], js["nodes"][1]["mesh"]);
    assert_eq!(js["nodes"][0]["children"], serde_json::json!([1, 2]));
    assert_eq!(js["nodes"][1]["rotation"], serde_json::json!([0.0, 1.0, 0.0, 0.0]));
    assert!(js["nodes"][2].get("mesh").is_none(), "pusta siatka = węzeł bez meshu");
    assert_eq!(js["scenes"][0]["nodes"], serde_json::json!([0]));
}
