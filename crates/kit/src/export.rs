//! Eksport: GLB (glTF 2.0) i OBJ + MTL.

use crate::color::{hex_to_linear, hex_to_rgb};
use crate::mesh::{MeshData, V3};
use serde_json::{json, Value};

pub struct ExportNode<'a> {
    pub name: String,
    pub mesh: &'a MeshData,
    pub offset: Option<V3>,
    /// Indeks węzła-rodzica w tej samej tablicy (offset i obrót są wtedy lokalne); `None` = korzeń sceny.
    pub parent: Option<usize>,
    /// Obrót węzła jako kwaternion glTF `[x, y, z, w]`.
    pub rotation: Option<[f64; 4]>,
    /// Metadane węzła (glTF `extras`), np. spec i metryka.
    pub extras: Option<Value>,
}

/// Binarny glTF 2.0: jeden węzeł na budynek, jeden prymityw na materiał.
/// Węzły o tym samym `&MeshData` dzielą jeden mesh glTF; pusta siatka = węzeł bez meshu (kotwica).
pub fn to_glb(nodes: &[ExportNode]) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let (mut views, mut accessors, mut materials, mut meshes, mut gnodes) = (vec![], vec![], vec![], vec![], vec![]);
    let mut add_view = |bin: &mut Vec<u8>, bytes: &[u8], target: u32| {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        views.push(json!({ "buffer": 0, "byteOffset": bin.len(), "byteLength": bytes.len(), "target": target }));
        bin.extend_from_slice(bytes);
        views.len() - 1
    };
    let f32b = |v: &[f32]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let mut mesh_of: Vec<Option<usize>> = Vec::with_capacity(nodes.len());
    for (ni, node) in nodes.iter().enumerate() {
        if node.mesh.parts.is_empty() {
            mesh_of.push(None);
            continue;
        }
        if let Some(j) = nodes[..ni].iter().position(|o| std::ptr::eq(o.mesh, node.mesh)) {
            mesh_of.push(mesh_of[j]);
            continue;
        }
        let mut prims = vec![];
        for part in &node.mesh.parts {
            let key = part.material.expect("materiał części");
            let spec = node.mesh.material(key);
            let lin = hex_to_linear(spec.map_or("#999999", |s| s.color.as_str()));
            let opacity = spec.map_or(1.0, |s| s.opacity);
            let transparent = opacity < 0.99;
            let mut mat = json!({
                "name": format!("{}_{}", node.name, key.as_str()),
                "pbrMetallicRoughness": {
                    "baseColorFactor": [lin[0], lin[1], lin[2], if transparent { opacity } else { 1.0 }],
                    "metallicFactor": spec.map_or(0.0, |s| s.metalness),
                    "roughnessFactor": spec.map_or(0.8, |s| s.roughness),
                },
                "doubleSided": spec.is_some_and(|s| s.double_sided),
            });
            if transparent {
                mat["alphaMode"] = "BLEND".into();
            }
            if let Some(e) = spec.and_then(|s| s.emissive) {
                // ponytail: glTF nie ma „tylko nocą” — emisja zapisana w extras, nie w emissiveFactor
                mat["extras"] = json!({ "emissiveNight": e });
            }
            materials.push(mat);
            let count = part.positions.len() / 3;
            let (mut min, mut max) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
            for p in part.positions.chunks_exact(3) {
                for c in 0..3 {
                    min[c] = min[c].min(p[c]);
                    max[c] = max[c].max(p[c]);
                }
            }
            let vp = add_view(&mut bin, &f32b(&part.positions), 34962);
            accessors.push(json!({ "bufferView": vp, "componentType": 5126, "count": count, "type": "VEC3", "min": min, "max": max }));
            let vn = add_view(&mut bin, &f32b(&part.normals), 34962);
            accessors.push(json!({ "bufferView": vn, "componentType": 5126, "count": count, "type": "VEC3" }));
            let vt = add_view(&mut bin, &f32b(&part.uvs), 34962);
            accessors.push(json!({ "bufferView": vt, "componentType": 5126, "count": count, "type": "VEC2" }));
            let ib: Vec<u8> = part.indices.iter().flat_map(|x| x.to_le_bytes()).collect();
            let vi = add_view(&mut bin, &ib, 34963);
            accessors.push(json!({ "bufferView": vi, "componentType": 5125, "count": part.indices.len(), "type": "SCALAR" }));
            let n = accessors.len();
            prims.push(json!({ "attributes": { "POSITION": n - 4, "NORMAL": n - 3, "TEXCOORD_0": n - 2 }, "indices": n - 1, "material": materials.len() - 1, "mode": 4 }));
        }
        meshes.push(json!({ "name": node.name, "primitives": prims }));
        mesh_of.push(Some(meshes.len() - 1));
    }
    for (node, mesh) in nodes.iter().zip(&mesh_of) {
        let mut gn = json!({ "name": node.name });
        if let Some(m) = mesh {
            gn["mesh"] = json!(m);
        }
        if let Some(o) = node.offset {
            gn["translation"] = json!(o);
        }
        if let Some(r) = node.rotation {
            gn["rotation"] = json!(r);
        }
        if let Some(e) = &node.extras {
            gn["extras"] = e.clone();
        }
        let kids: Vec<usize> = (0..nodes.len()).filter(|&k| nodes[k].parent == Some(gnodes.len())).collect();
        if !kids.is_empty() {
            gn["children"] = json!(kids);
        }
        gnodes.push(gn);
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let doc = json!({
        "asset": { "version": "2.0", "generator": "Pierzeja – generator budynków (Rust)" },
        "scene": 0,
        "scenes": [{ "nodes": (0..gnodes.len()).filter(|&i| nodes[i].parent.is_none()).collect::<Vec<_>>() }],
        "nodes": gnodes,
        "meshes": meshes,
        "materials": materials,
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{ "byteLength": bin.len() }],
    });
    let mut js = serde_json::to_vec(&doc).expect("json");
    while js.len() % 4 != 0 {
        js.push(b' ');
    }
    let total = 12 + 8 + js.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    for v in [0x4654_6c67u32, 2, total as u32, js.len() as u32, 0x4e4f_534a] {
        out.extend(v.to_le_bytes());
    }
    out.extend(&js);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(0x004e_4942u32.to_le_bytes());
    out.extend(&bin);
    out
}

/// Wavefront OBJ + MTL (grupa na materiał w każdym budynku).
pub fn to_obj(nodes: &[ExportNode], mtl_name: &str) -> (String, String) {
    use std::fmt::Write;
    let mut o = format!("# Pierzeja – generator budynków\nmtllib {mtl_name}\n");
    let mut m = String::from("# Pierzeja – materiały\n");
    let f3 = |x: f64| if x.abs() < 1e-6 { "0".to_string() } else { format!("{x:.4}") };
    let mut base = 1usize;
    for node in nodes {
        let [ox, oy, oz] = node.offset.unwrap_or([0.0; 3]);
        writeln!(o, "o {}", node.name).ok();
        for part in &node.mesh.parts {
            let key = part.material.expect("materiał części");
            let name = format!("{}_{}", node.name, key.as_str());
            let spec = node.mesh.material(key);
            let [r, g, b] = hex_to_rgb(spec.map_or("#999999", |s| s.color.as_str()));
            let rough = spec.map_or(0.8, |s| s.roughness);
            let op = spec.map_or(1.0, |s| s.opacity);
            writeln!(m, "newmtl {name}\nKd {r:.4} {g:.4} {b:.4}\nKa 0 0 0\nNs {}\nd {op:.3}\nillum 2\n", ((1.0 - rough) * 200.0).round()).ok();
            writeln!(o, "usemtl {name}").ok();
            let p = &part.positions;
            for v in p.chunks_exact(3) {
                writeln!(o, "v {} {} {}", f3(v[0] as f64 + ox), f3(v[1] as f64 + oy), f3(v[2] as f64 + oz)).ok();
            }
            for v in part.normals.chunks_exact(3) {
                writeln!(o, "vn {} {} {}", f3(v[0] as f64), f3(v[1] as f64), f3(v[2] as f64)).ok();
            }
            for v in part.uvs.chunks_exact(2) {
                writeln!(o, "vt {} {}", f3(v[0] as f64), f3(v[1] as f64)).ok();
            }
            for t in part.indices.chunks_exact(3) {
                let (a, b, c) = (t[0] as usize + base, t[1] as usize + base, t[2] as usize + base);
                writeln!(o, "f {a}/{a}/{a} {b}/{b}/{b} {c}/{c}/{c}").ok();
            }
            base += p.len() / 3;
        }
    }
    (o, m)
}
