//! Wektory testowe z HANDOFF, załącznik G (wartości policzone prototypem TS).

use indexmap::IndexMap;
use pierzeja_core::color::blend_hex;
use pierzeja_core::*;
use serde_json::json;

fn spec(v: serde_json::Value) -> BuildingSpec {
    serde_json::from_value(v).unwrap()
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn g1_idw_weights() {
    let cases: &[(f64, f64, f64, f64, &[(&str, f64)])] = &[
        (1965.0, 0.6, 3.0, 3.0, &[("blok_poodwilzowy", 0.460761), ("blok_plyta_5", 0.334635), ("socrealizm", 0.204604)]),
        (1900.0, 0.5, 3.0, 3.0, &[("kamienica_eklektyczna", 0.485175), ("familok", 0.348276), ("kamienica_secesyjna", 0.166549)]),
        (2000.0, 0.3, 3.0, 3.0, &[("dom_90", 0.408276), ("kostka", 0.374711), ("stodola", 0.217012)]),
        (1890.0, 0.62, 3.0, 3.0, &[("kamienica_eklektyczna", 1.0)]),
        (1950.0, 0.95, 3.0, 3.0, &[("falowiec", 0.659306), ("blok_plyta_11", 0.269496), ("socrealizm", 0.071198)]),
        (
            1965.0,
            0.6,
            1.5,
            5.0,
            &[("blok_poodwilzowy", 0.276928), ("blok_plyta_5", 0.236001), ("socrealizm", 0.184538), ("kamienica_modernistyczna", 0.153502), ("blok_90", 0.149031)],
        ),
    ];
    for (year, scale, sharp, k, want) in cases {
        let w = map_weights(MapPoint { year: *year, scale: *scale }, *sharp, *k);
        assert_eq!(w.len(), want.len(), "{year};{scale}: {w:?}");
        for (id, v) in *want {
            assert!(close(w[*id], *v, 1e-6), "{year};{scale} {id}: {} != {v}", w[*id]);
        }
        assert!(close(w.values().sum(), 1.0, 1e-12));
    }
}

#[test]
fn g2_blend_50_50() {
    let r = resolve_spec(&spec(json!({ "weights": { "kamienica_eklektyczna": 0.5, "blok_plyta_5": 0.5 } })));
    let p = &r.params;
    assert_eq!(p.floors, 5.0);
    assert_eq!(p.segments, 3.0);
    assert!(close(p.segment_width, 18.2, 1e-9));
    assert!(close(p.depth, 12.5, 1e-9));
    assert!(close(p.ground_floor_height, 3.55, 1e-9));
    assert!(close(p.floor_height, 3.225, 1e-9));
    assert!(close(p.roof_pitch, 9.0, 1e-9));
    assert!(close(p.balconies, 0.33, 1e-9));
    assert!(close(p.railing, 0.275, 1e-9));
    assert_eq!(p.entrance_type, "gate");
    assert_eq!(p.wall_texture, "plaster");
    assert_eq!(p.roof_texture, "sheet");
    assert_eq!(p.wall_color, "#c8beaa");
    assert_eq!(p.roof_color, "#484744");
    assert_eq!(p.frame_color, "#978b7f");
    assert_eq!(blend_hex([("#ff0000", 0.5), ("#0000ff", 0.5)]), "#8c53a2");
}

#[test]
fn g3_variation() {
    let p = resolve_spec(&spec(json!({ "archetype": "blok_plyta_5", "seed": 42, "variation": 1 }))).params;
    assert_eq!(p.floors, 5.0);
    assert_eq!(p.segments, 5.0);
    assert!(close(p.segment_width, 14.7731, 1e-4), "{}", p.segment_width);
    assert!(close(p.depth, 11.1129, 1e-4), "{}", p.depth);
    assert!(close(p.floor_height, 2.7327, 1e-4), "{}", p.floor_height);
    assert_eq!(p.wall_color, "#bab6ad");
    assert!(close(p.balconies, 0.5069, 1e-4), "{}", p.balconies);
    assert_eq!(p.dormers, 0.0);
}

/// (id, kond., wys. ścian, wys. całk., klasa, zabudowa, PC, klatki, okna, balkony, mieszk., trójkąty)
type Row = (&'static str, u32, f64, f64, &'static str, f64, f64, u32, u32, u32, u32, usize);
const G4: &[Row] = &[
    ("dworek", 1, 4.2, 11.03, "N", 264.0, 264.0, 1, 21, 0, 3, 2261),
    ("kamienica_eklektyczna", 5, 18.52, 20.79, "SW", 560.0, 2800.0, 1, 90, 4, 33, 16017),
    ("kamienica_secesyjna", 6, 20.95, 31.91, "SW", 580.0, 3480.0, 1, 114, 10, 40, 14432),
    ("familok", 3, 9.5, 13.8, "N", 275.0, 825.0, 2, 58, 0, 10, 7120),
    ("willa_drewniana", 2, 6.4, 12.4, "N", 120.0, 240.0, 1, 31, 4, 1, 2266),
    ("willa_modernistyczna", 2, 6.6, 7.05, "N", 143.0, 286.0, 1, 27, 2, 1, 1952),
    ("kamienica_modernistyczna", 6, 19.65, 20.45, "SW", 482.0, 2890.0, 1, 91, 21, 36, 7794),
    ("socrealizm", 6, 21.47, 24.93, "SW", 780.0, 4680.0, 3, 237, 30, 67, 32756),
    ("blok_poodwilzowy", 5, 14.8, 15.1, "SW", 572.0, 2860.0, 4, 169, 64, 49, 13598),
    ("blok_plyta_5", 5, 14.7, 14.95, "SW", 792.0, 3960.0, 5, 145, 80, 55, 20530),
    ("blok_plyta_11", 11, 31.2, 33.95, "W", 460.0, 5062.0, 3, 195, 120, 70, 28054),
    ("falowiec", 11, 31.5, 34.25, "W", 4212.0, 46332.0, 6, 1974, 1740, 794, 283356),
    ("kostka", 2, 6.7, 6.94, "N", 100.0, 200.0, 1, 23, 2, 1, 1962),
    ("dom_90", 2, 6.1, 12.27, "N", 154.0, 308.0, 1, 51, 2, 1, 4245),
    ("blok_90", 5, 14.8, 21.51, "SW", 585.0, 2925.0, 3, 175, 48, 36, 11400),
    ("deweloperski", 6, 17.95, 18.55, "SW", 672.0, 3912.0, 3, 167, 48, 49, 11726),
    ("plomba", 6, 19.65, 20.0, "SW", 252.0, 1472.0, 1, 55, 24, 17, 4624),
    ("apartamentowiec", 12, 39.9, 42.65, "W", 576.0, 6720.0, 2, 301, 144, 76, 23608),
    ("stodola", 1, 3.15, 6.98, "N", 136.0, 136.0, 1, 9, 0, 1, 442),
];

#[test]
fn g4_archetype_metrics() {
    let mut bad = Vec::new();
    for &(id, fl, hw, ht, cls, fp, gfa, st, win, bal, flats, tris) in G4 {
        let m = generate_from_spec(&spec(json!({ "archetype": id, "seed": 3 })));
        let s = &m.stats;
        let got = (s.floors, s.height_walls, s.height_total, s.height_class, s.footprint, s.gfa, s.staircases, s.windows, s.balconies, s.flats_estimate, s.triangles);
        let ok = s.floors == fl
            && close(s.height_walls, hw, 0.006)
            && close(s.height_total, ht, 0.006)
            && s.height_class == cls
            && close(s.footprint, fp, 0.6)
            && close(s.gfa, gfa, 0.6)
            && s.staircases == st
            && s.windows == win
            && s.balconies == bal
            && s.flats_estimate == flats
            && s.triangles == tris;
        if !ok {
            bad.push(format!("{id}: got {got:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn g4_notes() {
    let notes = |id| generate_from_spec(&spec(json!({ "archetype": id, "seed": 3 }))).stats.notes;
    assert_eq!(notes("blok_plyta_11"), vec!["Grupa wysokości W wg WT 2002 (wysokość ok. 31,5 m)."]);
    assert_eq!(notes("stodola"), vec!["Budynek niski (N) wg WT 2002: do 12 m lub do 4 kondygnacji."]);
    assert_eq!(notes("kamienica_secesyjna"), vec!["Grupa wysokości SW wg WT 2002 (wysokość ok. 31,9 m)."]);
    let a = notes("apartamentowiec");
    assert_eq!(a.len(), 3, "{a:?}");
    assert!(a[0].contains("40,9 m") && a[1].contains("§ 54") && a[1].contains("36,6 m") && a[2].contains("§ 303"));
}

#[test]
fn determinism_and_validity() {
    let s = spec(json!({ "cursor": { "year": 1960, "scale": 0.5 }, "seed": 9, "variation": 0.5 }));
    let a = generate_from_spec(&s);
    let b = generate_from_spec(&s);
    assert_eq!(to_glb(&[ExportNode { name: "a".into(), mesh: &a.mesh, offset: None, extras: None }]), to_glb(&[ExportNode { name: "a".into(), mesh: &b.mesh, offset: None, extras: None }]));
    for p in &a.mesh.parts {
        assert!(p.positions.iter().all(|x| x.is_finite()));
    }
    // każdy parametr w zakresie
    let map = a.resolved.params.to_map();
    for d in &data().params {
        if let (Some(v), Some(lo), Some(hi)) = (map[&d.key].as_f64(), d.min, d.max) {
            assert!(v >= lo && v <= hi, "{} = {v}", d.key);
        }
    }
    let w: IndexMap<String, f64> = a.resolved.weights["massing"].clone();
    assert!(close(w.values().sum(), 1.0, 1e-9));
}

#[test]
fn street_runs() {
    let st = generate_street(&StreetSpec { profile: Some("lodz_srodmiescie".into()), ..Default::default() });
    assert_eq!(st.buildings.len(), 10);
    assert!(!describe_street(&st).is_empty());
}
