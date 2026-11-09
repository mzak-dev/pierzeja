//! Koło: 12 boków, oś = lokalne X, środek w piaście. Symetryczne względem płaszczyzny YZ
//! (felga po obu stronach), więc ta sama siatka służy kołom lewym i prawym bez obrotu.

use crate::P;
use procedura_kit::mesh::{MatKey, MaterialSpec, MeshBuilder, MeshData, V3};
use std::f64::consts::PI;

const SIDES: usize = 12;

pub fn build(p: &P, materials: Vec<MaterialSpec>) -> MeshData {
    let (r, hw) = (p.f("wheelDiameter") / 2.0, p.f("tyreWidth") / 2.0);
    // wierzchołki na okręgu opisanym, żeby płaski bok dotykał gruntu dokładnie w y = 0
    let rv = r / (PI / SIDES as f64).cos();
    let rr = rv * p.f("rimRatio");
    let ang = |k: usize| (k as f64 + 0.5) * 2.0 * PI / SIDES as f64;
    let ring = |x: f64, rad: f64| -> Vec<V3> { (0..SIDES).map(|k| [x, rad * ang(k).cos(), rad * ang(k).sin()]).collect() };
    let radial = |k: usize| [0.0, (ang(k) + PI / SIDES as f64).cos(), (ang(k) + PI / SIDES as f64).sin()];

    let mut mb = MeshBuilder::default();
    // bieżnik
    let (a, b) = (ring(-hw, rv), ring(hw, rv));
    for k in 0..SIDES {
        let n = (k + 1) % SIDES;
        mb.quad(MatKey::Tyre, a[k], a[n], b[n], b[k], Some(radial(k)));
    }
    for s in [-1.0, 1.0] {
        // ścianka boczna opony (lekko wypukła), potem obrzeże felgi i tarcza
        let (outer, lip, dish) = (ring(s * hw, rv), ring(s * hw * 0.9, rr), ring(s * hw * 0.62, rr * 0.95));
        for k in 0..SIDES {
            let n = (k + 1) % SIDES;
            let rad = radial(k);
            mb.quad(MatKey::Tyre, outer[k], outer[n], lip[n], lip[k], Some([s, rad[1] * 0.5, rad[2] * 0.5]));
            mb.quad(MatKey::Rim, lip[k], lip[n], dish[n], dish[k], Some([s * 0.5, -rad[1] * 0.8, -rad[2] * 0.8]));
        }
        // ciężarowe: stalowa tarcza z 10 nakrętkami (bez szprych); osobowe: ciemna tarcza, 5 szprych, 4 nakrętki
        let truck = p.s("bodyType") == "podwozie";
        mb.poly(if truck { MatKey::Rim } else { MatKey::Plastic }, &dish, Some([s, 0.0, 0.0]));
        let sx = s * (hw * 0.62 + 0.006);
        let at = |ang: f64, rad: f64| [sx, rad * ang.cos(), rad * ang.sin()];
        if truck {
            let hub: Vec<V3> = (0..8).map(|k| [sx + s * 0.02, rr * 0.3 * (k as f64 * PI / 4.0).cos(), rr * 0.3 * (k as f64 * PI / 4.0).sin()]).collect();
            mb.poly(MatKey::Plastic, &hub, Some([s, 0.0, 0.0]));
            for i in 0..10 {
                let a = i as f64 * PI / 5.0;
                nut(&mut mb, s, sx, rr * 0.42 * a.cos(), rr * 0.42 * a.sin(), rr * 0.035);
            }
            continue;
        }
        for i in 0..5 {
            let a = i as f64 * 2.0 * PI / 5.0;
            mb.quad(MatKey::Rim, at(a - 0.2, rr * 0.2), at(a + 0.2, rr * 0.2), at(a + 0.11, rr * 0.92), at(a - 0.11, rr * 0.92), Some([s, 0.0, 0.0]));
        }
        let hub: Vec<V3> = (0..6).map(|k| [sx + s * 0.006, rr * 0.2 * (k as f64 * PI / 3.0).cos(), rr * 0.2 * (k as f64 * PI / 3.0).sin()]).collect();
        mb.poly(MatKey::Rim, &hub, Some([s, 0.0, 0.0]));
        for i in 0..4 {
            let a = (i as f64 + 0.5) * PI / 2.0;
            nut(&mut mb, s, sx, rr * 0.4 * a.cos(), rr * 0.4 * a.sin(), rr * 0.045);
        }
    }
    mb.build(materials)
}

/// Nakrętka: kwadratowy kołpak o półboku `q` 1 cm nad tarczą (boczne ścianki pominięte: niewidoczne).
fn nut(mb: &mut MeshBuilder, s: f64, x0: f64, cy: f64, cz: f64, q: f64) {
    let x1 = x0 + s * 0.012;
    let p = |dy: f64, dz: f64| [x1, cy + dy, cz + dz];
    mb.quad(MatKey::Rim, p(-q, -q), p(q, -q), p(q, q), p(-q, q), Some([s, 0.0, 0.0]));
}
