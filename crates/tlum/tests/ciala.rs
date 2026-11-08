use procedura_kit::mesh::MatKey;
use serde_json::{json, Value};
use tlum_core::{data, extra, generate, to_glb, HumanModel, Pose, TlumSpec};

fn make(v: Value) -> HumanModel {
    generate(&serde_json::from_value::<TlumSpec>(v).unwrap())
}

const POSES: [&str; 5] = ["rest", "stand", "walk", "sit", "drive"];

fn glb_json(glb: &[u8]) -> Value {
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + len]).unwrap()
}

#[test]
fn glb_is_deterministic() {
    for p in POSES {
        let s = json!({ "archetype": "robotnik", "pose": p, "phase": 0.3 });
        assert_eq!(to_glb(&make(s.clone())), to_glb(&make(s)), "{p}");
    }
    let v = |seed| to_glb(&make(json!({ "seed": seed, "variation": 0.8 })));
    assert_eq!(v(1), v(1));
    assert_ne!(v(1), v(2));
}

#[test]
fn triangle_budget_lod0() {
    for a in &data().archetypes {
        for p in &POSES[1..] {
            let t = make(json!({ "archetype": a.id, "pose": p })).stats.triangles;
            assert!((800..=1200).contains(&t), "{} / {p}: {t} trójkątów", a.id);
        }
    }
    // najgorszy przypadek: długie włosy, pełna broda, okulary, kask, puchówka, kamizelka
    let t = make(json!({ "overrides": { "hairStyle": "dlugie", "beard": "pelna_broda", "glasses": "korekcyjne", "headType": "helm_psp", "outerType": "puchowka", "vest": "odblask_zolta", "feetType": "buty_strazackie" } })).stats.triangles;
    assert!(t <= 1200, "najgorszy przypadek: {t}");
}

#[test]
fn rest_joints_match_rr_human_within_2cm() {
    let m = make(json!({ "pose": "rest" }));
    let want = extra()["szkielet"]["jointy"].as_array().unwrap();
    assert_eq!(m.joints.len(), want.len());
    for (j, w) in m.joints.iter().zip(want) {
        assert_eq!(j.name, w["name"].as_str().unwrap());
        for (key, got) in [("head", j.head), ("tail", j.tail)] {
            for k in 0..3 {
                let d = (got[k] - w[key][k].as_f64().unwrap()).abs();
                assert!(d <= 0.02, "{}.{key}[{k}] odchylka {d:.4} m", j.name);
            }
        }
    }
}

#[test]
fn height_scales_with_spec() {
    for h in [1.2, 1.5, 1.78, 2.0] {
        let m = make(json!({ "pose": "stand", "overrides": { "height": h } }));
        let got = m.bounds().max[1] - m.bounds().min[1];
        assert!((got / h - 1.0).abs() <= 0.03, "height {h}: siatka {got:.3}");
        assert!(m.bounds().min[1].abs() < 0.01, "stopy na gruncie: {}", m.bounds().min[1]);
    }
}

#[test]
fn poses_are_distinct_and_grounded() {
    let hip = |p: &str| make(json!({ "pose": p })).hip[1];
    assert!(hip("sit") < hip("stand") - 0.2 && hip("drive") < hip("stand") - 0.2);
    let a = to_glb(&make(json!({ "pose": "walk", "phase": 0.1 })));
    let b = to_glb(&make(json!({ "pose": "walk", "phase": 0.6 })));
    assert_ne!(a, b);
    for p in &POSES[1..] {
        let m = make(json!({ "pose": p, "phase": 0.2 }));
        assert!(m.bounds().min[1].abs() < 0.03, "{p}: najniższy punkt {}", m.bounds().min[1]);
    }
    assert_eq!(Pose::parse("walk"), Some(Pose::Walk));
}

fn slots(m: &HumanModel) -> Vec<MatKey> {
    m.body.parts.iter().filter_map(|p| p.material).collect()
}

#[test]
fn material_slots() {
    let base = make(json!({}));
    assert_eq!(base.body.materials.len(), 9);
    for k in [MatKey::Skin, MatKey::Top, MatKey::Bottom, MatKey::Shoes, MatKey::Hair, MatKey::Face] {
        assert!(slots(&base).contains(&k), "{k:?}");
    }
    assert!(!slots(&base).contains(&MatKey::HiVis));
    let full = make(json!({ "overrides": { "outerType": "kurtka_lekka", "vest": "odblask_zolta", "headType": "kask_budowlany" } }));
    for k in [MatKey::Outer, MatKey::HiVis, MatKey::Helmet] {
        assert!(slots(&full).contains(&k), "{k:?}");
    }
}

#[test]
fn glb_has_body_and_skeleton_nodes() {
    let j = glb_json(&to_glb(&make(json!({ "pose": "sit" }))));
    let nodes = j["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1 + 29);
    assert_eq!(nodes[0]["name"], "Body");
    assert_eq!(j["meshes"].as_array().unwrap().len(), 1);
    assert!(nodes.iter().any(|n| n["name"] == "LeftHandGrip"));
}
