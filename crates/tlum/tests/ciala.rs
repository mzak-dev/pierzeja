use procedura_kit::mesh::MatKey;
use serde_json::{json, Value};
use tlum_core::{data, extra, generate, to_glb, HumanModel, Pose, TlumSpec};

fn make(v: Value) -> HumanModel {
    generate(&serde_json::from_value::<TlumSpec>(v).unwrap())
}

const POSES: [&str; 6] = ["rest", "stand", "walk", "sit", "drive", "ride"];

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
            // z palcami (30 członów) budżet LOD0 to 1500
            assert!((1000..=1500).contains(&t), "{} / {p}: {t} trójkątów", a.id);
        }
    }
    // najcięższa głowa (długie włosy, pełna broda, okulary) na każdym archetypie, także mundurowym z napisami
    for a in &data().archetypes {
        let t = make(json!({ "archetype": a.id, "overrides": { "hairStyle": "dlugie", "beard": "pelna_broda", "glasses": "korekcyjne" } })).stats.triangles;
        assert!(t <= 1500, "{} z ciężką głową: {t}", a.id);
    }
    // najgorszy przypadek: długie włosy, pełna broda, okulary, hełm, puchówka, kamizelka
    let t = make(json!({ "overrides": { "hairStyle": "dlugie", "beard": "pelna_broda", "glasses": "korekcyjne", "headType": "helm_psp", "outerType": "puchowka", "vest": "odblask_zolta", "feetType": "buty_strazackie" } })).stats.triangles;
    assert!(t <= 1500, "najgorszy przypadek: {t}");
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
    assert_eq!(base.body.materials.len(), 13);
    for k in [MatKey::Skin, MatKey::Top, MatKey::Bottom, MatKey::Shoes, MatKey::Hair, MatKey::Face] {
        assert!(slots(&base).contains(&k), "{k:?}");
    }
    assert!(!slots(&base).contains(&MatKey::HiVis));
    let full = make(json!({ "overrides": { "outerType": "kurtka_lekka", "vest": "odblask_zolta", "headType": "kask_budowlany" } }));
    for k in [MatKey::Outer, MatKey::HiVis, MatKey::Helmet, MatKey::Reflective, MatKey::Eye] {
        assert!(slots(&full).contains(&k), "{k:?}");
    }
    // zarost to osobna, cienka warstwa (nie włosy)
    let stubble = make(json!({ "archetype": "robotnik" }));
    assert!(slots(&stubble).contains(&MatKey::Stubble));
    // krawat biznesu i biały pas drogówki w slocie akcentu
    assert!(slots(&make(json!({ "archetype": "biznes" }))).contains(&MatKey::Detail));
    assert!(slots(&make(json!({ "archetype": "policjant_ruchu_drogowego" }))).contains(&MatKey::Detail));
}

#[test]
fn glb_has_body_and_skeleton_nodes() {
    let j = glb_json(&to_glb(&make(json!({ "pose": "sit" }))));
    let nodes = j["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1 + 59);
    assert_eq!(nodes[0]["name"], "Body");
    assert_eq!(j["meshes"].as_array().unwrap().len(), 1);
    assert!(nodes.iter().any(|n| n["name"] == "LeftHandGrip"));
    for n in ["LeftThumbProximal", "LeftIndexDistal", "RightLittleDistal", "RightMiddleIntermediate"] {
        assert!(nodes.iter().any(|x| x["name"] == n), "brak węzła {n}");
    }
}

fn joint(m: &HumanModel, n: &str) -> tlum_core::Joint {
    m.joints.iter().find(|j| j.name == n).unwrap_or_else(|| panic!("brak {n}")).clone()
}

#[test]
fn fingers_are_rigged_after_rr_human() {
    let m = make(json!({ "pose": "rest" }));
    let names: Vec<&str> = m.joints.iter().map(|j| j.name.as_str()).collect();
    assert_eq!(names.len(), 59);
    // 29 kości rr_human bez zmian kolejności, palce na końcu
    assert_eq!(names[28], "RightHandGrip");
    assert_eq!(names[29], "LeftThumbProximal");
    for side in ["Left", "Right"] {
        for f in ["Thumb", "Index", "Middle", "Ring", "Little"] {
            let p = joint(&m, &format!("{side}{f}Proximal"));
            assert_eq!(m.joints[p.parent.unwrap()].name, format!("{side}Hand"));
            for (a, b) in [("Proximal", "Intermediate"), ("Intermediate", "Distal")] {
                let c = joint(&m, &format!("{side}{f}{b}"));
                assert_eq!(m.joints[c.parent.unwrap()].name, format!("{side}{f}{a}"));
            }
        }
    }
    // kciuk odwiedziony do przodu (+Z), palce dalej niż nadgarstek
    let (hand, thumb, mid) = (joint(&m, "LeftHand"), joint(&m, "LeftThumbDistal"), joint(&m, "LeftMiddleDistal"));
    assert!(thumb.tail[2] > hand.head[2] + 0.04, "kciuk {:?}", thumb.tail);
    assert!(mid.tail[0] > hand.head[0] + 0.15);
    // palce zginają się w chwycie kierownicy
    let drive = make(json!({ "pose": "drive" }));
    let (d_mid, d_hand) = (joint(&drive, "LeftMiddleDistal"), joint(&drive, "LeftHand"));
    let reach = |a: [f64; 3], b: [f64; 3]| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt();
    assert!(reach(d_mid.tail, d_hand.head) < reach(mid.tail, hand.head) - 0.03, "chwyt: palce zgięte");
}

#[test]
fn driver_pose_fits_a_car_seat() {
    for a in ["przechodzien_dorosly", "biznes", "senior", "strazak_bojowy"] {
        for pose in ["drive", "ride"] {
            let m = make(json!({ "archetype": a, "pose": pose }));
            let h = m.hip;
            let heel = m.body.bounds.min[1];
            assert!((0.24..=0.32).contains(&(h[1] - heel)), "{a}/{pose}: punkt H {:.3} nad piętą", h[1] - heel);
            let crown = joint(&m, "Head").tail;
            assert!(crown[1] - h[1] <= 0.86, "{a}/{pose}: czubek {:.3} nad H", crown[1] - h[1]);
            let (knee, ankle) = (joint(&m, "LeftLowerLeg").head, joint(&m, "LeftFoot").head);
            let thigh = ((knee[1] - h[1]) / (knee[2] - h[2])).atan().to_degrees();
            assert!((8.0..=16.0).contains(&thigh), "{a}/{pose}: udo {thigh:.1}°");
            assert!(ankle[2] - h[2] > 0.6, "{a}/{pose}: stopy przed biodrem");
            if pose == "drive" {
                for s in ["Left", "Right"] {
                    let g = joint(&m, &format!("{s}HandGrip")).head;
                    assert!((0.28..=0.40).contains(&(g[1] - h[1])), "{a}: dłoń {:.3} nad H", g[1] - h[1]);
                    assert!((0.35..=0.45).contains(&(g[2] - h[2])), "{a}: dłoń {:.3} przed H", g[2] - h[2]);
                    // łokieć wyraźnie zgięty (ok. 120–130°)
                    let (sh, el, wr) = (joint(&m, &format!("{s}UpperArm")).head, joint(&m, &format!("{s}LowerArm")).head, joint(&m, &format!("{s}Hand")).head);
                    let (u, v): (Vec<f64>, Vec<f64>) = ((0..3).map(|k| sh[k] - el[k]).collect(), (0..3).map(|k| wr[k] - el[k]).collect());
                    let l = |x: &Vec<f64>| x.iter().map(|q| q * q).sum::<f64>().sqrt();
                    let ang = ((0..3).map(|k| u[k] * v[k]).sum::<f64>() / l(&u) / l(&v)).acos().to_degrees();
                    assert!((105.0..=145.0).contains(&ang), "{a}: łokieć {ang:.0}°");
                }
            }
        }
    }
}

#[test]
fn child_legs_start_under_the_pelvis() {
    for a in ["dziecko", "przechodzien_dorosly"] {
        let m = make(json!({ "archetype": a, "pose": "rest" }));
        let hx = joint(&m, "LeftUpperLeg").head[0];
        let h = m.bounds().max[1];
        // rozstaw stawów biodrowych ok. 10% wzrostu
        assert!((0.04..=0.06).contains(&(hx / h)), "{a}: staw biodrowy {hx:.3} przy wzroście {h:.2}");
    }
}

#[test]
fn child_head_is_larger_relative_to_height() {
    // głów we wzroście: 5 lat ok. 6, 8 lat ok. 6,5, dorosły ok. 7,5 (staw głowy leży 0,928 wys. głowy pod czubkiem)
    for (age, h, want) in [(5.0, 1.1, 6.0), (8.0, 1.3, 6.5), (35.0, 1.78, 7.5)] {
        let m = make(json!({ "pose": "rest", "overrides": { "age": age, "height": h } }));
        let head = joint(&m, "Head");
        let n = h / ((head.tail[1] - head.head[1]) / 0.928);
        assert!((n - want).abs() < 0.2, "{age} lat: {n:.2} głowy we wzroście");
    }
}
