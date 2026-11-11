use serde_json::{json, Value};
use tabor_core::{generate, to_glb, TaborSpec, VehicleModel};

const CARS: [&str; 7] = ["hatchback", "hatch_miejski", "sedan_liftback", "coupe", "kombi", "suv", "suv_duzy"];

fn spec(v: Value) -> TaborSpec {
    serde_json::from_value(v).unwrap()
}

fn make(archetype: &str) -> VehicleModel {
    generate(&spec(json!({ "archetype": archetype, "seed": 3 })))
}

fn glb_json(glb: &[u8]) -> Value {
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + len]).unwrap()
}

fn param(m: &VehicleModel, k: &str) -> f64 {
    m.params[k].as_f64().unwrap()
}

#[test]
fn glb_is_deterministic() {
    for a in CARS.iter().chain(&["van"]) {
        assert_eq!(to_glb(&make(a)), to_glb(&make(a)), "{a}");
    }
    // seed i wariacja zmieniają wynik, ten sam seed nie
    let s = |seed| to_glb(&generate(&spec(json!({ "archetype": "hatchback", "seed": seed, "variation": 0.5 }))));
    assert_eq!(s(1), s(1));
    assert_ne!(s(1), s(2));
}

#[test]
fn triangle_floors() {
    // tylko dolna granica: Tabor nie ma limitu trójkątów (decyzja 2026-11-10), ma wyglądać low-poly
    for a in CARS {
        let t = make(a).stats.triangles;
        assert!(t >= 800, "{a}: {t} trójkątów");
    }
    let t = make("van").stats.triangles;
    assert!(t >= 1000, "van: {t} trójkątów");
}

#[test]
fn dimensions_match_params_within_5_percent() {
    for a in CARS.iter().chain(&["van"]) {
        let m = make(a);
        let b = m.bounds();
        for (name, got, key) in [("długość", b.max[2] - b.min[2], "length"), ("szerokość", m.stats.width, "width"), ("wysokość", b.max[1] - b.min[1], "height")] {
            let want = param(&m, key);
            assert!((got / want - 1.0).abs() <= 0.05, "{a}: {name} {got:.3} vs {want:.3}");
        }
        assert!(b.min[1].abs() < 1e-6, "{a}: koła stoją na gruncie (y=0), jest {}", b.min[1]);
    }
}

#[test]
fn glb_has_wheel_and_seat_nodes() {
    for a in ["hatchback", "van"] {
        let m = make(a);
        let js = glb_json(&to_glb(&m));
        let nodes = js["nodes"].as_array().unwrap();
        let named = |n: &str| nodes.iter().position(|x| x["name"] == n).unwrap_or_else(|| panic!("{a}: brak węzła {n}"));
        assert_eq!(named("Body"), 0);
        let wheel_mesh = &nodes[named("Wheel_FL")]["mesh"];
        for n in ["Wheel_FL", "Wheel_FR", "Wheel_RL", "Wheel_RR"] {
            assert_eq!(&nodes[named(n)]["mesh"], wheel_mesh, "{a}: {n} dzieli siatkę koła");
        }
        assert_eq!(js["meshes"].as_array().unwrap().len(), 2, "{a}: nadwozie + jedno koło");
        let seats = param(&m, "seats") as usize;
        for i in 0..seats {
            let n = &nodes[named(&format!("Seat_{i}"))];
            assert!(n.get("mesh").is_none());
        }
        assert!(!nodes.iter().any(|n| n["name"] == format!("Seat_{seats}")));
        assert_eq!(js["scenes"][0]["nodes"], json!([0]), "reszta węzłów to dzieci nadwozia");
    }
}

#[test]
fn anchors_follow_conventions() {
    let m = make("sedan_liftback");
    let (wb, r) = (param(&m, "wheelbase"), param(&m, "wheelDiameter") / 2.0);
    let [fl, fr, rl, rr] = m.wheels.map(|(_, p)| p);
    // origin = środek między osiami, przód +Z, lewa strona +X, piasta na wysokości promienia
    assert_eq!((fl[2], rl[2]), (wb / 2.0, -wb / 2.0));
    assert_eq!(fl[0], -fr[0]);
    assert!(fl[0] > 0.0 && rl[0] > 0.0 && rr[0] < 0.0 && fr[2] == fl[2] && rr[2] == rl[2]);
    assert!((fl[1] - r).abs() < 1e-9);
    assert!(m.seats[0][0] > 0.0, "kierowca po lewej (+X)");
    assert_eq!(m.seats.len(), 5);
}

#[test]
fn m1_wheelbases_are_applied() {
    // M1: sedan/kombi 2,8 m, van 3,7 m (RapidResponse/mods/core/data/pojazdy.toml)
    for (base, wb) in [("sedan_liftback", 2.8), ("kombi", 2.8), ("van", 3.7)] {
        let m = generate(&spec(json!({ "archetype": base, "overrides": { "wheelbase": wb } })));
        let [fl, _, rl, _] = m.wheels.map(|(_, p)| p);
        assert!((fl[2] - rl[2] - wb).abs() < 1e-9, "{base}: rozstaw kół {}", fl[2] - rl[2]);
        assert_eq!(m.stats.wheelbase, wb);
        let len = param(&m, "length");
        assert!((m.stats.length / len - 1.0).abs() <= 0.05, "{base}: długość {} vs {len}", m.stats.length);
    }
}

#[test]
fn mixed_archetypes_generate() {
    // mieszanka wektorów (kształt vana, wygląd SUV-a) i skrajne nadpisania nie psują siatki
    let m = generate(&spec(json!({ "weights": { "hatchback": 0.4, "van": 0.6 }, "groupWeights": { "material": { "suv": 1 } }, "seed": 9 })));
    assert!(m.stats.triangles > 800);
    let m = generate(&spec(json!({ "archetype": "suv", "overrides": { "wheelbase": 3.4, "length": 4.3, "seats": 7 } })));
    assert_eq!(m.seats.len(), 7);
}
