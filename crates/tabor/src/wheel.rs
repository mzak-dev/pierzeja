//! Koło: opona obrotowa z profilem (bieżnik, bark, wybrzuszony bok, stopka) i felga z głębią.
//! Oś = lokalne X, środek w piaście; siatka symetryczna względem płaszczyzny YZ (felga po obu
//! stronach), więc ta sama służy kołom lewym i prawym. Dolny wierzchołek bieżnika leży dokładnie
//! w y = −r, więc koło stoi na gruncie bez szczeliny.

use crate::P;
use procedura_kit::mesh::{MatKey, MaterialSpec, MeshBuilder, MeshData, V3};
use procedura_kit::powloka::Powloka;
use std::f64::consts::{PI, TAU};

/// Styl felgi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Felga {
    /// Aluminiowa, 5 szerokich ramion.
    Alu5,
    /// Aluminiowa, 10 cienkich ramion.
    AluMulti,
    /// Stalowa z kołpakiem (pełna tarcza z otworami wentylacyjnymi).
    Kolpak,
    /// Ciężarowa: tarcza stalowa, 10 nakrętek, wypukła piasta.
    Ciezarowa,
}


/// `sides` = liczba boków opony (LOD0 32, niżej 20 / 12 / 8).
pub fn build(p: &P, materials: Vec<MaterialSpec>, sides: usize) -> MeshData {
    let r = p.f("wheelDiameter") / 2.0;
    let hw = p.f("tyreWidth") / 2.0;
    let truck = matches!(p.s("bodyType"), "podwozie" | "naczepa");
    let steel = materials.iter().any(|m| m.key == MatKey::Rim && m.metalness < 0.5);
    let felga = if truck {
        Felga::Ciezarowa
    } else if steel {
        Felga::Kolpak
    } else if p.f("rimRatio") > 0.66 {
        Felga::AluMulti
    } else {
        Felga::Alu5
    };
    let rr = r * p.f("rimRatio");
    let mut mb = MeshBuilder::default();
    tyre(&mut mb, r, hw, rr, sides);
    for s in [-1.0, 1.0] {
        rim(&mut mb, felga, s, hw, rr, sides);
    }
    mb.build(materials)
}

/// Punkt na okręgu w płaszczyźnie YZ; kąt 0 = dół (−Y), żeby wierzchołek leżał na gruncie.
fn at(x: f64, rad: f64, a: f64) -> V3 {
    [x, -rad * a.cos(), rad * a.sin()]
}

fn ang(k: usize, n: usize) -> f64 {
    k as f64 / n as f64 * TAU
}

/// Opona: profil (x, promień) obrócony wokół osi X; gładkie normalne z załamaniem na krawędziach bieżnika.
fn tyre(mb: &mut MeshBuilder, r: f64, hw: f64, rr: f64, n: usize) {
    let side = r - rr; // wysokość boku opony
    let prof: Vec<(f64, f64)> = vec![
        (-0.80 * hw, rr + 0.012),
        (-0.97 * hw, rr + 0.30 * side),
        (-1.0 * hw, rr + 0.62 * side),
        (-0.93 * hw, r - 0.06 * side),
        (-0.72 * hw, r - 0.004),
        (0.0, r),
        (0.72 * hw, r - 0.004),
        (0.93 * hw, r - 0.06 * side),
        (1.0 * hw, rr + 0.62 * side),
        (0.97 * hw, rr + 0.30 * side),
        (0.80 * hw, rr + 0.012),
    ];
    let mut pw = Powloka::default();
    let ids: Vec<Vec<u32>> = (0..=n).map(|k| prof.iter().map(|&(x, rad)| pw.vert(at(x, rad, ang(k % n, n)))).collect()).collect();
    for k in 0..n {
        for j in 0..prof.len() - 1 {
            // profil rośnie w +X, obrót od −Y ku +Z: ta kolejność daje normalne na zewnątrz
            pw.quad(ids[k][j], ids[k][j + 1], ids[k + 1][j + 1], ids[k + 1][j], MatKey::Tyre);
        }
    }
    pw.weld(1e-9);
    pw.emit(mb, 50.0);
}

/// Felga po stronie `s` (±1): obrzeże, cofnięta tarcza i ramiona albo pełna tarcza.
fn rim(mb: &mut MeshBuilder, felga: Felga, s: f64, hw: f64, rr: f64, n: usize) {
    let lip_x = s * 0.80 * hw;
    let face_x = s * if felga == Felga::Ciezarowa { 0.55 } else { 0.62 } * hw;
    let back_x = s * 0.15 * hw;
    let ring = |x: f64, rad: f64| -> Vec<V3> { (0..n).map(|k| at(x, rad, ang(k, n))).collect() };
    let nv = [s, 0.0, 0.0];
    // obrzeże: od stopki opony w głąb do tarczy (stożek)
    let (lip, inner) = (ring(lip_x, rr + 0.012), ring(face_x, rr * 0.93));
    for k in 0..n {
        let m = (k + 1) % n;
        mb.quad(MatKey::Rim, lip[k], lip[m], inner[m], inner[k], Some([s * 0.6, -inner[k][1], -inner[k][2]]));
    }
    match felga {
        Felga::Alu5 | Felga::AluMulti => {
            // ciemne wnętrze (bęben, tarcza hamulcowa) za ramionami
            mb.poly(MatKey::Plastic, &ring(back_x, rr * 0.93), Some(nv));
            let disc = ring(back_x + s * 0.004, rr * 0.62);
            mb.poly(MatKey::Rim, &disc, Some(nv));
            let (count, wid) = if felga == Felga::Alu5 { (5, 0.34) } else { (10, 0.13) };
            for i in 0..count {
                let a = i as f64 * TAU / count as f64;
                spoke(mb, s, face_x, rr, a, wid);
            }
            // piasta
            let hub: Vec<V3> = (0..10).map(|k| at(face_x + s * 0.012, rr * 0.2, k as f64 * TAU / 10.0)).collect();
            mb.poly(MatKey::Rim, &hub, Some(nv));
        }
        Felga::Kolpak => {
            // pełna tarcza kołpaka, lekko wypukła, z otworami (ciemne trapezy)
            let rim_ring = ring(face_x, rr * 0.93);
            let dome = ring(face_x + s * 0.02, rr * 0.55);
            for k in 0..n {
                let m = (k + 1) % n;
                mb.quad(MatKey::Rim, rim_ring[k], rim_ring[m], dome[m], dome[k], Some([s, 0.0, 0.0]));
            }
            mb.poly(MatKey::Rim, &dome, Some(nv));
            for i in 0..8 {
                let a = i as f64 * TAU / 8.0 + PI / 8.0;
                let q = [at(0.0, rr * 0.62, a - 0.13), at(0.0, rr * 0.62, a + 0.13), at(0.0, rr * 0.86, a + 0.09), at(0.0, rr * 0.86, a - 0.09)];
                let q: Vec<V3> = q.iter().map(|v| [face_x + s * 0.012, v[1], v[2]]).collect();
                mb.poly(MatKey::Plastic, &q, Some(nv));
            }
        }
        Felga::Ciezarowa => {
            mb.poly(MatKey::Rim, &ring(face_x, rr * 0.93), Some(nv));
            // otwory wentylacyjne tarczy
            for i in 0..8 {
                let a = i as f64 * TAU / 8.0;
                let c = at(face_x + s * 0.003, rr * 0.68, a);
                let hole: Vec<V3> = (0..8).map(|k| {
                    let b = k as f64 * TAU / 8.0;
                    [c[0], c[1] + rr * 0.09 * b.cos(), c[2] + rr * 0.09 * b.sin()]
                }).collect();
                mb.poly(MatKey::Plastic, &hole, Some(nv));
            }
            // piasta wypukła i 10 nakrętek
            let hub_base: Vec<V3> = (0..12).map(|k| at(face_x, rr * 0.33, k as f64 * TAU / 12.0)).collect();
            let hub_top: Vec<V3> = (0..12).map(|k| at(face_x + s * 0.05, rr * 0.22, k as f64 * TAU / 12.0)).collect();
            for k in 0..12 {
                let m = (k + 1) % 12;
                mb.quad(MatKey::Rim, hub_base[k], hub_base[m], hub_top[m], hub_top[k], Some([s, -hub_base[k][1], -hub_base[k][2]]));
            }
            mb.poly(MatKey::Plastic, &hub_top, Some(nv));
            for i in 0..10 {
                let a = i as f64 * TAU / 10.0;
                let c = at(face_x, rr * 0.42, a);
                nut(mb, s, c, rr * 0.04);
            }
        }
    }
}

/// Ramię felgi: płaski trapez na tarczy z bocznymi ściankami (głębokość 2,5 cm).
fn spoke(mb: &mut MeshBuilder, s: f64, face_x: f64, rr: f64, a: f64, wid: f64) {
    let (r0, r1) = (rr * 0.2, rr * 0.92);
    let x1 = face_x + s * 0.006;
    let x0 = face_x - s * 0.03;
    let pts = |x: f64| [at(x, r0, a - wid * 0.9), at(x, r0, a + wid * 0.9), at(x, r1, a + wid * 0.35), at(x, r1, a - wid * 0.35)];
    let (f, b) = (pts(x1), pts(x0));
    mb.poly(MatKey::Rim, &f, Some([s, 0.0, 0.0]));
    for k in 0..4 {
        let m = (k + 1) % 4;
        let mid = [0.0, (f[k][1] + f[m][1]) / 2.0, (f[k][2] + f[m][2]) / 2.0];
        let c = [0.0, at(0.0, (r0 + r1) / 2.0, a)[1], at(0.0, (r0 + r1) / 2.0, a)[2]];
        mb.quad(MatKey::Rim, f[k], f[m], b[m], b[k], Some([0.0, mid[1] - c[1], mid[2] - c[2]]));
    }
}

/// Nakrętka: sześciokątny graniastosłup 2 cm nad tarczą.
fn nut(mb: &mut MeshBuilder, s: f64, c: V3, q: f64) {
    let hex = |x: f64| -> Vec<V3> { (0..6).map(|k| [x, c[1] + q * (k as f64 * PI / 3.0).cos(), c[2] + q * (k as f64 * PI / 3.0).sin()]).collect() };
    let (b, t) = (hex(c[0]), hex(c[0] + s * 0.02));
    for k in 0..6 {
        let m = (k + 1) % 6;
        mb.quad(MatKey::Rim, b[k], b[m], t[m], t[k], Some([0.0, t[k][1] - c[1], t[k][2] - c[2]]));
    }
    mb.poly(MatKey::Rim, &t, Some([s, 0.0, 0.0]));
}
