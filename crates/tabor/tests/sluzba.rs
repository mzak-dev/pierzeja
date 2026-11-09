use procedura_kit::mesh::MatKey;
use serde_json::{json, Value};
use tabor_core::{generate, to_glb, TaborSpec, VehicleModel};

const SERVICES: [&str; 4] = ["policja", "medycy", "straz", "techniczne"];
const BODIES: [&str; 3] = ["sedan_liftback", "kombi", "van"];

fn make(base: &str, service: &str, extra: Value) -> VehicleModel {
    let mut o = json!({ "service": service });
    o.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    generate(&serde_json::from_value::<TaborSpec>(json!({ "archetype": base, "seed": 3, "overrides": o })).unwrap())
}

fn glb_json(glb: &[u8]) -> Value {
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + len]).unwrap()
}

fn paint_tris(m: &VehicleModel, k: MatKey) -> usize {
    m.body.parts.iter().filter(|p| p.material == Some(k)).map(|p| p.indices.len() / 3).sum()
}

#[test]
fn livery_is_deterministic() {
    for b in BODIES {
        for s in SERVICES {
            assert_eq!(to_glb(&make(b, s, json!({}))), to_glb(&make(b, s, json!({}))), "{b}/{s}");
        }
    }
}

#[test]
fn every_service_is_present_on_car_and_van() {
    for b in BODIES {
        let civ = make(b, "cywilny", json!({}));
        assert!(civ.lightbar.is_none() && paint_tris(&civ, MatKey::Accent) == 0, "{b}: cywilny bez oklejenia");
        for s in SERVICES {
            let m = make(b, s, json!({}));
            // oklejenie: pas (Accent) i belka z lampami; bryła ta sama co cywilna
            assert!(paint_tris(&m, MatKey::Accent) >= 2, "{b}/{s}: brak pasa");
            let (bar, off) = m.lightbar.as_ref().unwrap_or_else(|| panic!("{b}/{s}: brak belki"));
            assert!(bar.triangles() >= 12 && bar.material(MatKey::Beacon).unwrap().emissive.is_some());
            assert!(off[1] > 1.2, "{b}/{s}: belka na dachu, y={}", off[1]);
            assert_eq!(m.stats.wheelbase, civ.stats.wheelbase);
            assert!((m.body.bounds.max[1] - civ.body.bounds.max[1]).abs() < 0.04, "{b}/{s}: bryła nie rośnie");
            let js = glb_json(&to_glb(&m));
            assert!(js["nodes"].as_array().unwrap().iter().any(|n| n["name"] == "Lightbar"), "{b}/{s}: węzeł Lightbar");
        }
    }
}

#[test]
fn colours_follow_regulations() {
    let body = |b, s| make(b, s, json!({})).body.material(MatKey::Paint).unwrap().color.clone();
    // lakier jest lekko przygaszony weathering, więc sprawdzamy odcień dominującego kanału
    let rgb = |c: &str| [1, 3, 5].map(|i| u8::from_str_radix(&c[i..i + 2], 16).unwrap() as i32);
    let y = rgb(&body("van", "medycy"));
    assert!(y[0] > 200 && y[1] > 180 && y[2] < 120, "PRM żółty RAL 1016: {y:?}");
    let r = rgb(&body("sedan_liftback", "straz"));
    assert!(r[0] > 130 && r[1] < 60 && r[2] < 60, "PSP czerwień RAL 3000: {r:?}");
    let m = make("sedan_liftback", "straz", json!({}));
    assert!(rgb(&m.body.material(MatKey::Bumper).unwrap().color).iter().all(|&c| c > 190), "zderzaki PSP białe");
    let t = make("kombi", "techniczne", json!({}));
    assert_eq!(t.lightbar.unwrap().0.material(MatKey::Beacon).unwrap().emissive, Some("#ffc94a"), "żółte lampy § 38");
    let p = make("kombi", "policja", json!({}));
    assert_eq!(p.lightbar.unwrap().0.material(MatKey::Beacon).unwrap().emissive, Some("#5a8cff"));
}

#[test]
fn lamp_count_scales_lightbar() {
    let t = |n| make("sedan_liftback", "policja", json!({ "lightbarLamps": n })).lightbar.unwrap().0.triangles();
    assert!(t(2) < t(10) && t(10) < t(30));
}

#[test]
fn texts_and_patterns_add_geometry() {
    let all = |m: &VehicleModel| m.stats.triangles;
    let base = |s, extra| all(&make("van", s, extra));
    // wzór szachownicy i jodełki dokłada geometrię względem pasów ciągłych
    assert!(base("policja", json!({ "livery": "checker" })) != base("policja", json!({})));
    assert!(base("straz", json!({ "livery": "chevron" })) > base("straz", json!({})));
    // „Battenburg” nie występuje w polskich przepisach: wynik jak pasy ciągłe
    assert_eq!(base("policja", json!({ "livery": "battenburg" })), base("policja", json!({})));
}
