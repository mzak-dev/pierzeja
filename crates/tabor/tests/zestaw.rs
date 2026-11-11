//! Ciągnik siodłowy i naczepa jako osobne modele sprzęgane kotwicami (decyzja 2026-11-11).
use serde_json::{json, Value};
use tabor_core::{generate, to_glb, TaborSpec, VehicleModel};

fn make(v: Value) -> VehicleModel {
    generate(&serde_json::from_value::<TaborSpec>(v).unwrap())
}

fn anchor(m: &VehicleModel, n: &str) -> [f64; 3] {
    m.anchors.iter().find(|a| a.0 == n).unwrap_or_else(|| panic!("brak {n}")).1
}

fn glb_names(m: &VehicleModel) -> Vec<String> {
    let glb = to_glb(m);
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let js: Value = serde_json::from_slice(&glb[20..20 + len]).unwrap();
    js["nodes"].as_array().unwrap().iter().map(|n| n["name"].as_str().unwrap().to_string()).collect()
}

#[test]
fn fifth_wheel_matches_kingpin_and_anchors_are_exported() {
    let t = make(json!({ "archetype": "ciagnik_siodlowy", "seed": 1 }));
    let s = make(json!({ "archetype": "naczepa_kurtynowa", "seed": 1 }));
    let fw = anchor(&t, "Anchor_FifthWheel");
    let kp = anchor(&s, "Anchor_Kingpin");
    assert!((fw[1] - kp[1]).abs() < 1e-9, "siodło {} vs sworzeń {}", fw[1], kp[1]);
    assert_eq!((kp[0], kp[2]), (0.0, 0.0), "origin naczepy pod sworzniem");
    // siodło przed tylną osią ciągnika
    let rear = t.wheels.iter().find(|w| w.0 == "Wheel_RL").unwrap().1[2];
    assert!(fw[2] > rear && fw[2] - rear < 1.0, "siodło {} przed osią {rear}", fw[2]);
    assert!(glb_names(&t).iter().any(|n| n == "Anchor_FifthWheel"));
    let names = glb_names(&s);
    assert!(names.iter().any(|n| n == "Anchor_Kingpin") && names.iter().any(|n| n == "Anchor_LandingGear"));
}

#[test]
fn wheel_nodes_follow_axles_and_twins() {
    let t = make(json!({ "archetype": "ciagnik_siodlowy", "seed": 1 }));
    let names: Vec<&str> = t.wheels.iter().map(|w| w.0.as_str()).collect();
    assert_eq!(&names[..4], ["Wheel_FL", "Wheel_FR", "Wheel_RL", "Wheel_RR"]);
    assert!(names.contains(&"Wheel_RLi") && names.contains(&"Wheel_RRi"), "{names:?}");
    let s = make(json!({ "archetype": "naczepa_kurtynowa", "seed": 1 }));
    let names: Vec<&str> = s.wheels.iter().map(|w| w.0.as_str()).collect();
    assert_eq!(names.len(), 6, "{names:?}");
    assert!(!names.iter().any(|n| n.starts_with("Wheel_F")));
    assert!(names.contains(&"Wheel_R3L"));
    // trzy osie co 1,31 m
    let zs: Vec<f64> = ["Wheel_RL", "Wheel_R2L", "Wheel_R3L"].iter().map(|n| s.wheels.iter().find(|w| w.0 == *n).unwrap().1[2]).collect();
    assert!((zs[0] - zs[1] - 1.31).abs() < 1e-9 && (zs[1] - zs[2] - 1.31).abs() < 1e-9, "{zs:?}");
    let w = make(json!({ "archetype": "wywrotka_8x4", "seed": 1 }));
    let names: Vec<&str> = w.wheels.iter().map(|w| w.0.as_str()).collect();
    assert!(names.contains(&"Wheel_F2L") && names.contains(&"Wheel_R2Li"), "{names:?}");
    for m in [&t, &s, &w] {
        assert!(m.bounds().min[1].abs() < 1e-6, "koła na gruncie");
    }
}

#[test]
fn coupled_set_has_no_overlap_around_the_fifth_wheel() {
    let t = make(json!({ "archetype": "ciagnik_siodlowy", "seed": 1 }));
    let s = make(json!({ "archetype": "naczepa_kurtynowa", "seed": 1 }));
    let fw = anchor(&t, "Anchor_FifthWheel");
    let legs = anchor(&s, "Anchor_LandingGear");
    let verts = |m: &VehicleModel| -> Vec<[f64; 3]> { m.body.parts.iter().flat_map(|p| p.positions.chunks_exact(3).map(|v| [v[0] as f64, v[1] as f64, v[2] as f64])).collect() };
    // naczepa: przed nogami nic poniżej płaszczyzny sprzęgu (poza samym sworzniem Ø 9 cm)
    for v in verts(&s) {
        if v[2] > legs[2] + 0.3 && v[0].hypot(v[2]) > 0.06 {
            assert!(v[1] >= fw[1] - 1e-6, "naczepa schodzi pod siodło w {v:?}");
        }
    }
    // ciągnik: za kabiną, w promieniu skrętu naczepy, nic ponad płaszczyznę sprzęgu
    let cab_back = fw[2] + 1.5;
    for v in verts(&t) {
        if v[2] < cab_back && (v[2] - fw[2]).abs() < 2.1 {
            // 3 mm luzu na naklejki (wycięcie siodła leży na płycie)
            assert!(v[1] <= fw[1] + 0.003, "ciągnik wystaje nad siodło w {v:?}");
        }
    }
    // nogi podporowe poza zderzakiem ciągnika
    let tail = t.bounds().min[2];
    assert!(fw[2] + legs[2] < tail - 0.3, "nogi naczepy ({}) nad zderzakiem ciągnika ({tail})", fw[2] + legs[2]);
}
