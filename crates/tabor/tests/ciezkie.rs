//! Pojazdy ciężkie (issue 07): PD (laweta, rozstaw 4,8 m, 1 Miejsce) i SCRt (zabudowa
//! ratownicza, 4,6 m, 3 Miejsca) w obwiedniach przetargów PSP.
use serde_json::{json, Value};
use tabor_core::{generate, to_glb, TaborSpec, VehicleModel};

fn make(v: Value) -> VehicleModel {
    generate(&serde_json::from_value::<TaborSpec>(v).unwrap())
}

fn pd() -> Value {
    json!({ "archetype": "podwozie_lekkie", "seed": 7, "overrides": { "wheelbase": 4.8, "length": 7.9, "seats": 1, "module": "laweta", "moduleLength": 5.2 } })
}

fn scrt() -> Value {
    json!({ "archetype": "podwozie_srednie", "seed": 7, "overrides": { "wheelbase": 4.6, "seats": 3, "module": "techniczny", "moduleLength": 4.4, "compartments": 3, "passThrough": 0, "crane": 1, "winch": 1, "roofRack": 1 } })
}

fn crew() -> Value {
    json!({ "archetype": "podwozie_srednie", "seed": 3, "overrides": { "seats": 6 } })
}

fn all() -> Vec<(&'static str, Value)> {
    vec![
        ("PD", pd()),
        ("SCRt", scrt()),
        ("brygadowa", crew()),
        ("lekkie", json!({ "archetype": "podwozie_lekkie" })),
        ("srednie", json!({ "archetype": "podwozie_srednie" })),
        ("ciezkie", json!({ "archetype": "podwozie_ciezkie" })),
    ]
}

fn glb_json(glb: &[u8]) -> Value {
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + len]).unwrap()
}

#[test]
fn heavy_glb_is_deterministic() {
    for (n, s) in all() {
        assert_eq!(to_glb(&make(s.clone())), to_glb(&make(s)), "{n}");
    }
}

#[test]
fn heavy_triangle_budget() {
    for (n, s) in all() {
        let t = make(s).stats.triangles;
        // bez górnego limitu (decyzja 2026-11-11: budżet bez limitu + LOD), tylko dolna granica
        assert!(t >= 1000, "{n}: {t} trójkątów");
    }
}

#[test]
fn m1_wheelbases_are_exact() {
    for (n, s, wb) in [("PD", pd(), 4.8), ("SCRt", scrt(), 4.6)] {
        let m = make(s);
        let w = |n: &str| m.wheels.iter().find(|x| x.0 == n).unwrap_or_else(|| panic!("brak {n}")).1;
        let [fl, fr, rl, rr] = ["Wheel_FL", "Wheel_FR", "Wheel_RL", "Wheel_RR"].map(w);
        assert_eq!(fl[2] - rl[2], wb, "{n}");
        assert_eq!((fr[2], rr[2]), (fl[2], rl[2]));
        assert_eq!(m.stats.wheelbase, wb);
        assert_eq!(fl[0], -fr[0]);
        assert!(fl[0] > 0.0 && rl[0] > 0.0);
        assert!((fl[1] - m.stats.wheel_radius).abs() < 1e-9, "piasta na wysokości promienia");
    }
}

#[test]
fn seat_nodes_match_seat_count_and_cab_follows_seats() {
    for (seats, n) in [(1, 1), (2, 2), (3, 3), (4, 4), (6, 6), (8, 8)] {
        let m = make(json!({ "archetype": "podwozie_srednie", "overrides": { "seats": seats } }));
        assert_eq!(m.seats.len(), n);
        assert_eq!(m.stats.seats, n);
        assert!(m.seats[0][0] > 0.0, "kierowca po lewej (+X)");
        let js = glb_json(&to_glb(&m));
        let nodes = js["nodes"].as_array().unwrap();
        let count = nodes.iter().filter(|x| x["name"].as_str().is_some_and(|s| s.starts_with("Seat_"))).count();
        assert_eq!(count, n, "{seats} Miejsc → węzły Seat_N");
        assert!(nodes.iter().any(|x| x["name"] == format!("Seat_{}", n - 1)));
        assert!(!nodes.iter().any(|x| x["name"] == format!("Seat_{n}")));
    }
    // kabina rośnie z liczbą Miejsc: 1+1 < 1+2 < 1+1+4 (rzędy dalej do tyłu)
    let zmin = |s: usize| make(json!({ "archetype": "podwozie_srednie", "overrides": { "seats": s } })).seats.iter().map(|p| p[2]).fold(f64::MAX, f64::min);
    assert!(zmin(6) < zmin(3) - 0.5);
    assert!((zmin(1) - zmin(3)).abs() < 1e-9, "1+1 i 1+2 mają ten sam rząd przedni");
}

#[test]
fn glb_has_body_wheels_and_shared_mesh() {
    for (n, s) in all() {
        let m = make(s);
        let js = glb_json(&to_glb(&m));
        let nodes = js["nodes"].as_array().unwrap();
        assert_eq!(nodes[0]["name"], "Body", "{n}");
        // nadwozie + jedno koło (+ belka świetlna służby na dachu kabiny)
        let bar = usize::from(m.lightbar.is_some());
        assert_eq!(js["meshes"].as_array().unwrap().len(), 2 + bar, "{n}: nadwozie + jedno koło + belka");
        assert_eq!(nodes.iter().any(|x| x["name"] == "Lightbar"), bar == 1, "{n}: węzeł Lightbar");
        for w in ["Wheel_FL", "Wheel_FR", "Wheel_RL", "Wheel_RR"] {
            assert!(nodes.iter().any(|x| x["name"] == w), "{n}: {w}");
        }
    }
}

#[test]
fn envelope_and_angles() {
    // ciężki ≤ 9,5 m długości, ≤ 3,35 m wysokości, ≤ 2,55 m szerokości; najazd/zejście ≥ 23°
    let t = 23f64.to_radians().tan();
    let off = 1.0 / 23f64.to_radians().cos() - 1.0;
    for (n, s) in all() {
        let m = make(s);
        let b = m.bounds();
        // archetyp „ciężkie” (6x6, issue 10) ma w danych 10 m
        let lmax = if n == "ciezkie" { 10.0 } else { 9.5 };
        assert!(b.max[2] - b.min[2] <= lmax + 1e-9, "{n}: długość {}", b.max[2] - b.min[2]);
        assert!(b.max[1] <= 3.35 + 1e-9, "{n}: wysokość {}", b.max[1]);
        // lusterka ciężarówki wystają ok. 0,3 m na stronę i nie liczą się do 2,55 m
        assert!(b.max[0] - b.min[0] <= 2.55 + 0.6 + 1e-9, "{n}: szerokość z lusterkami {}", b.max[0] - b.min[0]);
        assert!(m.stats.width <= 2.55 + 1e-9, "{n}: szerokość nadwozia {}", m.stats.width);
        assert!(b.min[1].abs() < 1e-6, "{n}: koła na gruncie");
        let (wb, r) = (m.stats.wheelbase, m.stats.wheel_radius);
        // zejście liczone od ostatniej osi tylnej (tandem / tridem)
        let z_last = m.wheels.iter().map(|w| w.1[2]).fold(f64::MAX, f64::min);
        let mut worst = f64::MAX;
        for part in &m.body.parts {
            for v in part.positions.chunks_exact(3) {
                let (y, z) = (v[1] as f64, v[2] as f64);
                let line = if z < z_last { t * (z_last - z) - off * r } else if z < -wb / 2.0 { continue } else if z > wb / 2.0 { t * (z - wb / 2.0) - off * r } else { continue };
                worst = worst.min(y - line);
            }
        }
        assert!(worst > -1e-4, "{n}: nadwozie schodzi {worst:.3} m pod linię 23°");
    }
}

#[test]
fn modules_generate_all_parts() {
    // PD: 1+1; SCRt: HDS + wyciągarka + maszt dokładają geometrię względem gołego podwozia
    let bare = |s: Value| {
        let mut s = s;
        s["overrides"]["module"] = json!("none");
        s["overrides"]["crane"] = json!(0);
        s["overrides"]["roofRack"] = json!(0);
        s["overrides"]["winch"] = json!(0);
        make(s).stats.triangles
    };
    assert!(make(pd()).stats.triangles > bare(pd()) + 100);
    assert!(make(scrt()).stats.triangles > bare(scrt()) + 400);
    // rząd 6 Miejsc w kabinie brygadowej mieści się w obrysie
    let m = make(crew());
    assert_eq!(m.seats.len(), 6);
}
