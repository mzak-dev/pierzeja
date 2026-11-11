//! Poziomy szczegółów: mniej trójkątów na każdym kolejnym poziomie, ten sam obrys (±3 cm),
//! te same węzły kół, Miejsca i kotwice (gra podmienia siatki bez zmiany hierarchii).
use serde_json::json;
use tabor_core::{generate, TaborSpec, VehicleModel};

fn make(archetype: &str, lod: u8) -> VehicleModel {
    generate(&serde_json::from_value::<TaborSpec>(json!({ "archetype": archetype, "seed": 2, "lod": lod })).unwrap())
}

#[test]
fn lods_shrink_triangles_and_keep_the_envelope() {
    for a in ["hatchback", "kombi", "suv", "coupe", "van_sredni", "van", "podwozie_srednie", "ciagnik_siodlowy"] {
        let ms: Vec<VehicleModel> = (0..4).map(|l| make(a, l)).collect();
        let tris: Vec<usize> = ms.iter().map(|m| m.stats.triangles).collect();
        assert!(tris.windows(2).all(|w| w[1] < w[0]), "{a}: trójkąty nie maleją {tris:?}");
        let b0 = ms[0].bounds();
        for (l, m) in ms.iter().enumerate().skip(1) {
            let b = m.bounds();
            for k in 0..3 {
                assert!((b.min[k] - b0.min[k]).abs() < 0.03 && (b.max[k] - b0.max[k]).abs() < 0.03, "{a} LOD{l}: obrys oś {k} {:?} vs {:?}", (b.min, b.max), (b0.min, b0.max));
            }
            let names = |m: &VehicleModel| m.wheels.iter().map(|w| w.0.clone()).collect::<Vec<_>>();
            assert_eq!(names(m), names(&ms[0]), "{a} LOD{l}: węzły kół");
            assert_eq!(m.seats, ms[0].seats, "{a} LOD{l}: Miejsca");
            assert_eq!(m.anchors, ms[0].anchors, "{a} LOD{l}: kotwice");
        }
        println!("{a}: {tris:?}");
    }
}
