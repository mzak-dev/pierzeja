//! Detale osobówki rzutowane na powłokę (`Rzutnik`): reflektory z oprawą, grill, wloty, tablice,
//! lampy tylne zawinięte na bok, dyfuzor, szczeliny drzwi, klamki, wycieraczki; lusterka jako
//! osobne bryły. Wszystko symetryczne (lewa połowa odbita), poza tablicami.

use crate::karoseria::{Ksztalt, Rodzaj, CREASE};
use crate::tablice::plate;
use crate::{roll, P};
use procedura_kit::krzywe::{lerp, superelipsa};
use procedura_kit::mesh::{MatKey, MeshBuilder};
use procedura_kit::powloka::Powloka;
use procedura_kit::rzutnik::{Rzutnik, Widok};

/// Krok siatki naklejek (m): drobny, bo naklejka ma iść za krzywizną.
const STEP: f64 = 0.035;

fn sym(poly: &[[f64; 2]]) -> Vec<[f64; 2]> {
    poly.iter().map(|q| [-q[0], q[1]]).collect()
}

/// Pasek (linia szczeliny) między punktami mapy, szerokość `w`.
fn line(a: [f64; 2], b: [f64; 2], w: f64) -> Vec<[f64; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx.hypot(dy).max(1e-9);
    let (nx, ny) = (-dy / l * w / 2.0, dx / l * w / 2.0);
    vec![[a[0] + nx, a[1] + ny], [b[0] + nx, b[1] + ny], [b[0] - nx, b[1] - ny], [a[0] - nx, a[1] - ny]]
}

/// Elipsa jako wielokąt.
fn oval(cx: f64, cy: f64, rx: f64, ry: f64, n: usize) -> Vec<[f64; 2]> {
    (0..n).map(|i| {
        let a = i as f64 / n as f64 * std::f64::consts::TAU;
        [cx + rx * a.cos(), cy + ry * a.sin()]
    }).collect()
}

/// Powiększenie wypukłego wielokąta o `d` od środka ciężkości (oprawa wokół lampy).
fn grow(poly: &[[f64; 2]], d: f64) -> Vec<[f64; 2]> {
    let c = poly.iter().fold([0.0, 0.0], |a, q| [a[0] + q[0], a[1] + q[1]]);
    let c = [c[0] / poly.len() as f64, c[1] / poly.len() as f64];
    poly.iter().map(|q| {
        let (dx, dy) = (q[0] - c[0], q[1] - c[1]);
        let l = dx.hypot(dy).max(1e-9);
        [q[0] + dx / l * d, q[1] + dy / l * d]
    }).collect()
}

pub(crate) fn build(mb: &mut MeshBuilder, p: &P, seed: u32, k: &Ksztalt, pw: &Powloka, cladding: bool) {
    // każdy widok widzi tylko zewnętrzną warstwę nadwozia (bez wnęk kół i podłogi)
    let front = Rzutnik::limited(pw, CREASE, Widok::Przod, k.zf - 0.6);
    let rear = Rzutnik::limited(pw, CREASE, Widok::Tyl, -k.zr - 0.6);
    let left = Rzutnik::limited(pw, CREASE, Widok::Lewy, k.hw - 0.3);
    let right = Rzutnik::limited(pw, CREASE, Widok::Prawy, k.hw - 0.3);
    let top = Rzutnik::new(pw, CREASE, Widok::Gora);
    let both = |mb: &mut MeshBuilder, r: &Rzutnik, mat: MatKey, poly: &[[f64; 2]], lift: f64| {
        r.patch(mb, mat, poly, STEP, lift);
        r.patch(mb, mat, &sym(poly), STEP, lift);
    };
    let sides = |mb: &mut MeshBuilder, mat: MatKey, poly: &[[f64; 2]], lift: f64| {
        left.patch(mb, mat, poly, STEP, lift);
        right.patch(mb, mat, poly, STEP, lift);
    };
    let hw = k.hw;
    let clr = k.clr;
    let suv = k.kind == Rodzaj::Suv;

    // ---------------------------------------------------------------- przód
    let style = p.s("lampStyle");
    let ht = k.y_he - 0.025;
    let (hb, xi) = match style {
        "oval" => (k.y_he - 0.19, 0.40 * hw),
        "strip" => (k.y_he - 0.105, 0.34 * hw),
        _ => (k.y_he - 0.14, 0.36 * hw),
    };
    let xo = hw - 0.02;
    let lamp: Vec<[f64; 2]> = match style {
        "oval" => {
            let (cx, cy) = ((xi + xo) / 2.0, (hb + ht) / 2.0);
            oval(cx, cy, (xo - xi) / 2.0, (ht - hb) / 2.0, 14)
        }
        "strip" => vec![[xi, hb + 0.03], [xo - 0.08, hb], [xo, hb + 0.03], [xo, ht - 0.01], [xi + 0.02, ht]],
        _ => vec![[xi, hb + 0.04], [xo - 0.1, hb], [xo, hb + 0.04], [xo, ht], [xi + 0.03, ht]],
    };
    both(mb, &front, MatKey::Plastic, &grow(&lamp, 0.014), 0.003);
    both(mb, &front, MatKey::Headlight, &lamp, 0.006);
    // kierunkowskaz w zewnętrznym dolnym rogu lampy
    both(mb, &front, MatKey::Indicator, &[[xo - 0.11, hb + 0.012], [xo - 0.02, hb + 0.032], [xo - 0.02, hb + 0.05], [xo - 0.11, hb + 0.04]], 0.008);
    let grill = p.f("grill");
    // grill między reflektorami, większy w dół przy dużym `grill`
    let gb = (hb - grill * 0.16).max(clr + 0.30);
    let gpoly = [[0.0, gb], [xi - 0.03, gb + 0.015], [xi - 0.015, ht - 0.03], [0.0, ht - 0.025]];
    both(mb, &front, MatKey::Plastic, &gpoly, 0.004);
    if style == "strip" {
        both(mb, &front, MatKey::Headlight, &[[0.0, ht - 0.045], [xi, ht - 0.04], [xi, ht - 0.03], [0.0, ht - 0.035]], 0.007);
    }
    // dolny wlot w zderzaku, narożne wloty / światła przeciwmgłowe, czarna warga
    let iy0 = clr + 0.10 + if suv { 0.04 } else { 0.0 };
    let iy1 = iy0 + 0.09 + 0.07 * grill;
    both(mb, &front, MatKey::Plastic, &[[0.0, iy0], [0.5 * hw, iy0 + 0.01], [0.56 * hw, iy1], [0.0, iy1]], 0.004);
    both(mb, &front, MatKey::Plastic, &[[0.66 * hw, iy0 + 0.02], [0.86 * hw, iy0 + 0.03], [0.84 * hw, iy1 - 0.01], [0.68 * hw, iy1 - 0.02]], 0.004);
    both(mb, &front, MatKey::Plastic, &[[0.0, k.bottom(k.zf) - 0.02], [hw, k.bottom(k.zf) - 0.02], [hw, clr + 0.085], [0.0, clr + 0.085]], 0.002);
    if let Some(h) = front.hit(0.0, iy1 + 0.075) {
        plate(mb, seed, 7.0, [0.0, iy1 + 0.075, h.p[2] + 0.012], 1.0);
    }

    // ---------------------------------------------------------------- tył
    let tt = k.y_tail - 0.03;
    let tb = tt - if suv { 0.18 } else { 0.13 };
    let tl = [[0.48 * hw, tb + 0.02], [hw - 0.01, tb], [hw - 0.01, tt], [0.52 * hw, tt]];
    both(mb, &rear, MatKey::Plastic, &grow(&tl, 0.012), 0.003);
    both(mb, &rear, MatKey::Taillight, &tl, 0.006);
    both(mb, &rear, MatKey::Indicator, &[[0.78 * hw, tb + 0.012], [hw - 0.03, tb + 0.006], [hw - 0.03, tb + 0.04], [0.78 * hw, tb + 0.045]], 0.008);
    // zawinięcie lampy na bok (mapa boku: z, y)
    let wrap = [[k.zr - 0.02, tb], [k.zr + 0.2, tb + 0.03], [k.zr + 0.24, tt], [k.zr - 0.02, tt]];
    sides(mb, MatKey::Plastic, &grow(&wrap, 0.01), 0.003);
    sides(mb, MatKey::Taillight, &wrap, 0.006);
    if roll(seed, 4.0, p.f("rearLightBar")) {
        both(mb, &rear, MatKey::Taillight, &[[0.0, tt - 0.04], [0.5 * hw, tt - 0.035], [0.5 * hw, tt - 0.015], [0.0, tt - 0.02]], 0.006);
    }
    let rb = k.bottom(k.zr);
    both(mb, &rear, MatKey::Plastic, &[[0.0, rb - 0.02], [hw, rb - 0.02], [hw, clr + 0.15], [0.0, clr + 0.15]], 0.002);
    both(mb, &rear, MatKey::Taillight, &[[0.72 * hw, clr + 0.2], [0.9 * hw, clr + 0.2], [0.9 * hw, clr + 0.225], [0.72 * hw, clr + 0.225]], 0.004);
    // tablica na zderzaku (nad nim zostaje miejsce na napisy służb i znaczek)
    let py = (k.rear_bumper_top - 0.085).max(clr + 0.2);
    if let Some(h) = rear.hit(0.0, py) {
        plate(mb, seed, 7.0, [0.0, py, h.p[2] - 0.012], -1.0);
    }

    // ---------------------------------------------------------------- bok
    let sill = k.bottom(0.0) + 0.11;
    let w = 0.006;
    let belt = |z: f64| k.belt(z) - 0.012;
    let z_fd = (k.zaf - k.ra - 0.06).min(k.z_c - 0.02);
    let z_rd = k.zar + k.ra + 0.10;
    let coupe = k.kind == Rodzaj::Coupe;
    let mut seams = vec![line([z_fd, sill], [z_fd, belt(z_fd)], w), line([z_rd.min(k.z_b - 0.25), sill], [z_fd, sill], w)];
    if coupe {
        let z_door_r = lerp(k.z_b, z_rd, 0.4);
        seams.push(line([z_door_r, sill], [z_door_r, belt(z_door_r)], w));
    } else {
        seams.push(line([k.z_b, sill], [k.z_b, belt(k.z_b)], w));
        let arch_top = k.arch(z_rd).unwrap_or(sill) + 0.04;
        seams.push(line([z_rd, arch_top], [z_rd, belt(z_rd)], w));
    }
    for s in &seams {
        sides(mb, MatKey::Plastic, s, 0.0015);
    }
    // tylna krawędź okien równoległa do tylnej szyby (słupek C/D w kolorze nadwozia)
    let y_top = k.top(k.z_win_r + 0.05) - k.crown - 0.04;
    let b_r = k.belt(k.z_win_r);
    let run = ((y_top - b_r) / k.rear_tan).min(0.45);
    sides(mb, MatKey::Paint, &[[k.z_win_r - 0.01, b_r - 0.005], [k.z_win_r + 0.012, b_r - 0.005], [k.z_win_r + run, y_top + 0.02], [k.z_win_r - 0.01, y_top + 0.02]], 0.0015);
    // słupek B (i C między szybami) czarny, jak w większości aut po 2005 r.
    let pillar = |mb: &mut MeshBuilder, zp: f64, half: f64| {
        let (b0, t0) = (k.belt(zp) - 0.004, k.top(zp) - k.crown - 0.05);
        sides(mb, MatKey::Plastic, &[[zp - half, b0], [zp + half, b0], [zp + half, t0], [zp - half, t0]], 0.002);
    };
    pillar(mb, k.z_b, 0.05);
    if let Some(zc) = k.z_c_pillar {
        pillar(mb, zc, 0.065);
    }
    // okładziny: czarne nakładki nadkoli i progi (SUV, crossovery)
    if cladding {
        for za in [k.zaf, k.zar] {
            let (r0, r1) = (k.ra + 0.006, k.ra + 0.07);
            let n = 18;
            for i in 0..n {
                let a0 = (i as f64 / n as f64 - 0.5) * std::f64::consts::PI;
                let a1 = ((i + 1) as f64 / n as f64 - 0.5) * std::f64::consts::PI;
                let pt = |r: f64, a: f64| [za + r * a.sin(), k.r + r * a.cos()];
                let q = [pt(r0, a0), pt(r0, a1), pt(r1, a1), pt(r1, a0)];
                if q.iter().all(|v| v[1] > k.bottom(v[0]) + 0.04) {
                    sides(mb, MatKey::Plastic, &q, 0.006);
                }
            }
        }
        let (z0, z1) = (k.zar + k.ra + 0.04, k.zaf - k.ra - 0.04);
        sides(mb, MatKey::Plastic, &[[z0, k.bottom(0.0) + 0.04], [z1, k.bottom(0.0) + 0.04], [z1, k.bottom(0.0) + 0.17], [z0, k.bottom(0.0) + 0.17]], 0.004);
    }
    // klamki: wnęka (ciemna) i uchwyt w kolorze nadwozia
    let doors: Vec<f64> = if coupe { vec![lerp(k.z_b, z_rd, 0.4) + 0.2] } else { vec![k.z_b + 0.2, z_rd + 0.17] };
    for zh in doors {
        let y = belt(zh) - 0.075;
        sides(mb, MatKey::Plastic, &[[zh - 0.07, y - 0.012], [zh + 0.07, y - 0.012], [zh + 0.07, y + 0.004], [zh - 0.07, y + 0.004]], 0.002);
        sides(mb, MatKey::Paint, &[[zh - 0.065, y - 0.004], [zh + 0.065, y - 0.004], [zh + 0.065, y + 0.016], [zh - 0.065, y + 0.016]], 0.009);
    }
    // wycieraczki u podstawy szyby (mapa z góry: x, z)
    for x0 in [-0.55 * hw, 0.05 * hw] {
        let a = [x0, k.z_c - 0.06];
        let b = [x0 + 0.6 * hw, k.z_c - 0.2];
        top.patch(mb, MatKey::Plastic, &line(a, b, 0.018), 0.01, 0.003);
    }

    mirrors(mb, k);
}

/// Lusterka: ramię i obudowa (superelipsa w przekroju), szkło od tyłu, kierunkowskaz z przodu.
fn mirrors(mb: &mut MeshBuilder, k: &Ksztalt) {
    let z0 = k.z_c - 0.17;
    let y0 = k.belt(z0) + 0.045;
    let x_in = k.hw - 0.035;
    let (len_x, ry, rz) = (0.17, 0.068, 0.055);
    let mut pw = Powloka::default();
    let n = 16;
    let rings = 7;
    let ids: Vec<Vec<u32>> = (0..=rings)
        .map(|i| {
            let t = i as f64 / rings as f64;
            let x = x_in + 0.05 + len_x * t;
            // zwężenie na końcach obudowy
            let s = superelipsa(1.0, 1.0, 2.6, (2.0 * t - 1.0).abs()).max(0.05).sqrt();
            (0..n).map(|j| {
                let a = j as f64 / n as f64 * std::f64::consts::TAU;
                let (c, sn) = (a.cos(), a.sin());
                let e = 3.0;
                let f = |v: f64| v.signum() * v.abs().powf(2.0 / e);
                pw.vert([x, y0 + ry * s * f(sn), z0 + rz * s * f(c) - 0.02 * t])
            }).collect()
        })
        .collect();
    for i in 0..rings {
        for j in 0..n {
            let (a, b) = (j, (j + 1) % n);
            // tył obudowy (−z) to szkło
            let back = (j as f64 / n as f64 * std::f64::consts::TAU).cos() < -0.55;
            pw.set_group(u16::from(back));
            pw.quad(ids[i][a], ids[i + 1][a], ids[i + 1][b], ids[i][b], if back { MatKey::Glass } else { MatKey::Paint });
        }
    }
    pw.mirror_x();
    pw.emit(mb, 50.0);
    for side in [1.0, -1.0] {
        // ramię od podstawy szyby do obudowy
        let (xa, xb) = (side * (x_in - 0.01), side * (x_in + 0.07));
        mb.bx(MatKey::Plastic, [xa.min(xb), y0 - 0.045, z0 - 0.005], [xa.max(xb), y0 - 0.02, z0 + 0.035]);
        // kierunkowskaz na przodzie obudowy, przy zewnętrznym końcu
        let xi = side * (x_in + 0.05 + len_x * 0.7);
        let xo = side * (x_in + 0.05 + len_x * 0.97);
        mb.quad(MatKey::Indicator, [xi, y0 - 0.012, z0 + rz * 0.92], [xo, y0 - 0.012, z0 + rz * 0.6], [xo, y0 + 0.008, z0 + rz * 0.6], [xi, y0 + 0.008, z0 + rz * 0.92], Some([0.0, 0.0, 1.0]));
    }
}
