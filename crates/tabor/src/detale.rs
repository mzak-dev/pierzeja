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

/// Krok siatki naklejek (m) wg LOD: drobny, bo naklejka ma iść za krzywizną; niżej rzadszy.
const STEPS: [f64; 4] = [0.035, 0.08, 0.18, 0.4];

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

/// `lod` ≥ 1: bez szczelin, klamek, wycieraczek i słupków (lampy, grill, tablice i lusterka zostają).
pub(crate) fn build(mb: &mut MeshBuilder, p: &P, seed: u32, k: &Ksztalt, pw: &Powloka, cladding: bool, lod: u8) {
    // każdy widok widzi tylko zewnętrzną warstwę nadwozia (bez wnęk kół i podłogi)
    let front = Rzutnik::limited(pw, CREASE, Widok::Przod, k.zf - 0.6);
    let rear = Rzutnik::limited(pw, CREASE, Widok::Tyl, -k.zr - 0.6);
    let left = Rzutnik::limited(pw, CREASE, Widok::Lewy, k.hw - 0.3);
    let right = Rzutnik::limited(pw, CREASE, Widok::Prawy, k.hw - 0.3);
    let top = Rzutnik::new(pw, CREASE, Widok::Gora);
    let step = STEPS[lod.min(3) as usize];
    let both = |mb: &mut MeshBuilder, r: &Rzutnik, mat: MatKey, poly: &[[f64; 2]], lift: f64| {
        r.patch(mb, mat, poly, step, lift);
        r.patch(mb, mat, &sym(poly), step, lift);
    };
    let sides = |mb: &mut MeshBuilder, mat: MatKey, poly: &[[f64; 2]], lift: f64| {
        left.patch(mb, mat, poly, step, lift);
        right.patch(mb, mat, poly, step, lift);
    };
    let hw = k.hw;
    let clr = k.clr;
    let suv = k.kind == Rodzaj::Suv;
    let wipers = |mb: &mut MeshBuilder| {
        // wycieraczki u podstawy szyby (mapa z góry: x, z)
        for x0 in [-0.55 * hw, 0.05 * hw] {
            let a = [x0, k.z_c - 0.06];
            let b = [x0 + 0.6 * hw, k.z_c - 0.2];
            top.patch(mb, MatKey::Plastic, &line(a, b, 0.018), 0.01, 0.003);
        }
    };
    if k.kind == Rodzaj::Van {
        van(mb, p, seed, k, &front, &rear, &left, &right, lod);
        if lod == 0 {
            wipers(mb);
        }
        mirrors(mb, k, if k.cab_only { 1.6 } else { 1.5 });
        return;
    }

    // ---------------------------------------------------------------- przód
    let style = p.s("lampStyle");
    let ht = k.y_he - 0.025;
    let (hb, xi) = match style {
        "oval" => (k.y_he - 0.19, 0.40 * hw),
        "strip" => (k.y_he - 0.105, 0.34 * hw),
        _ => (k.y_he - 0.14, 0.36 * hw),
    };
    // lampa z przodu kończy się przed narożnikiem; dalej zawinięcie rzutowane z boku
    let xo = hw - 0.09;
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
    let zw = k.zf - 0.02;
    let wrap_f = match style {
        "oval" => vec![[zw - 0.2, (hb + ht) / 2.0 - 0.03], [zw, hb + 0.02], [zw, ht - 0.01], [zw - 0.2, (hb + ht) / 2.0 + 0.03]],
        _ => vec![[zw - 0.26, hb + 0.045], [zw, hb + 0.01], [zw, ht - 0.005], [zw - 0.22, ht - 0.005]],
    };
    sides(mb, MatKey::Plastic, &grow(&wrap_f, 0.012), 0.003);
    sides(mb, MatKey::Headlight, &wrap_f, 0.006);
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

    if lod >= 1 {
        // lusterka zostają na każdym poziomie: wyznaczają szerokość z lusterkami
        mirrors(mb, k, 1.0);
        return;
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
    wipers(mb);
    mirrors(mb, k, 1.0);
}

/// Detale vana: pionowe lampy, duży grill, czarne zderzaki z narożnikami, listwy boczne, drzwi
/// kabiny, drzwi przesuwne tylko po prawej (−X, ruch prawostronny), dwuskrzydłowe tylne drzwi z szybami.
#[allow(clippy::too_many_arguments)]
fn van(mb: &mut MeshBuilder, p: &P, seed: u32, k: &Ksztalt, front: &Rzutnik, rear: &Rzutnik, left: &Rzutnik, right: &Rzutnik, lod: u8) {
    let step = STEPS[lod.min(3) as usize];
    let both = |mb: &mut MeshBuilder, r: &Rzutnik, mat: MatKey, poly: &[[f64; 2]], lift: f64| {
        r.patch(mb, mat, poly, step, lift);
        r.patch(mb, mat, &sym(poly), step, lift);
    };
    let sides = |mb: &mut MeshBuilder, mat: MatKey, poly: &[[f64; 2]], lift: f64| {
        left.patch(mb, mat, poly, step, lift);
        right.patch(mb, mat, poly, step, lift);
    };
    let (hw, clr, h) = (k.hw, k.clr, k.h);
    let bumper_top = clr + 0.34;
    // kabina nad silnikiem: reflektory nisko, przy dolnej krawędzi czoła (nad zderzakiem ramy)
    let cab_over = k.cab_only && k.zf - k.z_c < 0.3;
    // --- przód: lampy pod krawędzią maski, grill do zderzaka, czarny zderzak zawinięty na boki
    let (ht, hb) = if cab_over { (clr + 0.36, clr + 0.14) } else { (k.y_he - 0.03, k.y_he - 0.21) };
    let (xi, xo) = (0.45 * hw, hw - 0.08);
    let lamp = [[xi, hb + 0.03], [xo - 0.06, hb], [xo, hb + 0.05], [xo, ht], [xi + 0.02, ht]];
    both(mb, front, MatKey::Plastic, &grow(&lamp, 0.016), 0.003);
    both(mb, front, MatKey::Headlight, &lamp, 0.006);
    both(mb, front, MatKey::Indicator, &[[xo - 0.12, hb + 0.012], [xo - 0.02, hb + 0.04], [xo - 0.02, hb + 0.07], [xo - 0.12, hb + 0.045]], 0.008);
    let zw = k.zf - 0.03;
    let wrap = [[zw - 0.2, hb + 0.05], [zw, hb + 0.02], [zw, ht - 0.01], [zw - 0.16, ht - 0.01]];
    sides(mb, MatKey::Plastic, &grow(&wrap, 0.012), 0.003);
    sides(mb, MatKey::Headlight, &wrap, 0.006);
    let grill = p.f("grill");
    // grill: w vanie między lampami do zderzaka, w kabinie nad silnikiem od lamp do szyby
    let (gb, gt, gx) = if cab_over { (ht + 0.05, k.y_he - 0.08, 0.62 * hw) } else { (lerp(hb + 0.02, bumper_top + 0.02, grill), ht - 0.02, xi - 0.03) };
    both(mb, front, MatKey::Plastic, &[[0.0, gb], [gx - 0.01, gb], [gx, gt], [0.0, gt]], 0.004);
    let slats = if cab_over { 6 } else { 4 };
    for i in 1..slats {
        let y = lerp(gb, gt, i as f64 / slats as f64);
        both(mb, front, MatKey::Bumper, &[[0.0, y - 0.008], [gx - 0.02, y - 0.008], [gx - 0.02, y + 0.008], [0.0, y + 0.008]], 0.006);
    }
    if k.cab_only && lod >= 1 {
        return;
    }
    if k.cab_only {
        // kabina ciężarówki: zderzak, tablica i tył ma rama; tu tylko drzwi i stopnie
        let sill = k.bottom(0.0) + 0.06;
        let w = 0.008;
        let belt = |z: f64| k.belt(z) - 0.01;
        let roof_seam = |z: f64| k.top(z) - k.crown - 0.08;
        let z_fd = k.z_c - 0.12;
        for s in [line([z_fd, sill], [z_fd, belt(z_fd)], w), line([k.z_b, sill], [k.z_b, roof_seam(k.z_b)], w), line([k.z_b, sill], [z_fd, sill], w)] {
            sides(mb, MatKey::Plastic, &s, 0.0015);
        }
        let zh = k.z_b + 0.18;
        let y = belt(zh) - 0.12;
        sides(mb, MatKey::Plastic, &[[zh - 0.09, y - 0.025], [zh + 0.09, y - 0.025], [zh + 0.09, y + 0.025], [zh - 0.09, y + 0.025]], 0.004);
        // kierunkowskaz boczny za nadkolem
        let zi = k.zaf - k.ra - 0.12;
        let yi = (k.r + k.ra * 0.4).max(clr + 0.1);
        sides(mb, MatKey::Indicator, &[[zi - 0.1, yi], [zi, yi], [zi, yi + 0.05], [zi - 0.1, yi + 0.05]], 0.005);
        return;
    }
    let fb = k.bottom(k.zf) - 0.03;
    both(mb, front, MatKey::Bumper, &[[0.0, fb], [hw, fb], [hw, bumper_top], [0.0, bumper_top]], 0.008);
    // narożnik zderzaka na boku: od przodu nadkola, dolna krawędź nad spodem (promienie pod spód nie trafiają)
    let zb0 = (k.zaf + k.ra + 0.04).max(k.zf - 0.45);
    let fbs = k.bottom(zb0) + 0.035;
    sides(mb, MatKey::Bumper, &[[zb0, fbs], [k.zf + 0.02, fbs], [k.zf + 0.02, bumper_top], [zb0 + 0.06, bumper_top]], 0.008);
    both(mb, front, MatKey::Plastic, &[[0.0, clr + 0.1], [0.55 * hw, clr + 0.1], [0.58 * hw, clr + 0.2], [0.0, clr + 0.2]], 0.011);
    if let Some(hh) = front.hit(0.0, clr + 0.27) {
        plate(mb, seed, 7.0, [0.0, clr + 0.27, hh.p[2] + 0.014], 1.0);
    }

    // --- tył: pionowe lampy w narożach, tylne drzwi dwuskrzydłowe z szybami, zderzak
    let (ly0, ly1) = (bumper_top + 0.06, (bumper_top + 0.75).min(h * 0.62));
    let tl = [[0.84 * hw, ly0], [hw - 0.02, ly0], [hw - 0.02, ly1], [0.84 * hw, ly1]];
    both(mb, rear, MatKey::Plastic, &grow(&tl, 0.012), 0.003);
    both(mb, rear, MatKey::Taillight, &tl, 0.006);
    let lm = lerp(ly0, ly1, 0.45);
    both(mb, rear, MatKey::Indicator, &[[0.84 * hw, lm], [hw - 0.02, lm], [hw - 0.02, lm + 0.1], [0.84 * hw, lm + 0.1]], 0.008);
    let rb = k.bottom(k.zr) - 0.03;
    both(mb, rear, MatKey::Bumper, &[[0.0, rb], [hw, rb], [hw, bumper_top], [0.0, bumper_top]], 0.008);
    let zb1 = (k.zar - k.ra - 0.04).min(k.zr + 0.45);
    let rbs = k.bottom(zb1) + 0.035;
    sides(mb, MatKey::Bumper, &[[k.zr - 0.02, rbs], [zb1, rbs], [zb1 - 0.06, bumper_top], [k.zr - 0.02, bumper_top]], 0.008);
    let (dy0, dy1) = (bumper_top + 0.02, h - 0.08);
    // szczeliny drzwi tylnych: środek i obrys
    both(mb, rear, MatKey::Plastic, &[[0.0, dy0], [0.004, dy0], [0.004, dy1], [0.0, dy1]], 0.002);
    both(mb, rear, MatKey::Plastic, &line([0.8 * hw, dy0], [0.8 * hw, dy1], 0.006), 0.002);
    both(mb, rear, MatKey::Plastic, &line([0.0, dy1], [0.8 * hw, dy1], 0.006), 0.002);
    // szyby tylnych drzwi
    let (wy0, wy1) = ((h * 0.58).max(ly1 + 0.05), h - 0.16);
    if wy1 - wy0 > 0.15 {
        let win = [[0.05, wy0], [0.74 * hw, wy0], [0.74 * hw, wy1], [0.05, wy1]];
        both(mb, rear, MatKey::Plastic, &grow(&win, 0.015), 0.003);
        both(mb, rear, MatKey::Glass, &win, 0.005);
    }
    both(mb, rear, MatKey::Plastic, &[[0.04, lerp(dy0, wy0, 0.5) - 0.02], [0.2, lerp(dy0, wy0, 0.5) - 0.02], [0.2, lerp(dy0, wy0, 0.5) + 0.01], [0.04, lerp(dy0, wy0, 0.5) + 0.01]], 0.006);
    if let Some(hh) = rear.hit(0.0, bumper_top - 0.13) {
        plate(mb, seed, 7.0, [0.0, bumper_top - 0.13, hh.p[2] - 0.014], -1.0);
    }

    if lod >= 1 {
        return;
    }
    // --- bok: drzwi kabiny po obu stronach, listwa ochronna
    let sill = k.bottom(0.0) + 0.1;
    let w = 0.007;
    let belt = |z: f64| k.belt(z) - 0.01;
    let roof_seam = |z: f64| k.top(z) - k.crown - 0.07;
    let z_fd = (k.zaf - k.ra - 0.05).min(k.z_c - 0.02);
    for s in [line([z_fd, sill + 0.15], [z_fd, belt(z_fd)], w), line([k.z_b, sill], [k.z_b, roof_seam(k.z_b)], w), line([k.z_b, sill], [k.zaf - k.ra - 0.02, sill], w)] {
        sides(mb, MatKey::Plastic, &s, 0.0015);
    }
    for zh in [k.z_b + 0.2] {
        let y = belt(zh) - 0.09;
        sides(mb, MatKey::Plastic, &[[zh - 0.08, y - 0.02], [zh + 0.08, y - 0.02], [zh + 0.08, y + 0.02], [zh - 0.08, y + 0.02]], 0.004);
    }
    // listwa ochronna (czarna) na wysokości barku od nadkola do nadkola i za tylnym
    let yl = lerp(sill, k.belt(0.0), 0.35);
    let strip = |z0: f64, z1: f64| [[z0, yl - 0.04], [z1, yl - 0.04], [z1, yl + 0.04], [z0, yl + 0.04]];
    sides(mb, MatKey::Bumper, &strip(k.zar + k.ra + 0.05, k.zaf - k.ra - 0.05), 0.007);
    sides(mb, MatKey::Bumper, &strip(k.zr + 0.28, k.zar - k.ra - 0.05), 0.007);
    // drzwi przesuwne po prawej (−X): przednia i tylna krawędź, górna, szyna nad oknami, klamka
    let (zs0, zs1) = (k.z_b - 0.04, k.z_b - 0.04 - (1.25f64).min(0.42 * (k.z_b - k.zr)));
    let ys = roof_seam(zs0);
    for s in [line([zs0, sill], [zs0, ys], w), line([zs1, sill + 0.05], [zs1, ys], w), line([zs1, ys], [zs0, ys], w)] {
        right.patch(mb, MatKey::Plastic, &s, step, 0.0015);
    }
    right.patch(mb, MatKey::Plastic, &line([zs1, belt(zs1) - 0.02], [k.zr + 0.05, belt(zs1) - 0.02], 0.022), step, 0.004);
    right.patch(mb, MatKey::Plastic, &[[zs0 - 0.2, belt(zs0) - 0.11], [zs0 - 0.05, belt(zs0) - 0.11], [zs0 - 0.05, belt(zs0) - 0.07], [zs0 - 0.2, belt(zs0) - 0.07]], step, 0.004);
}

/// Lusterka: ramię i obudowa (superelipsa w przekroju), szkło od tyłu, kierunkowskaz z przodu.
fn mirrors(mb: &mut MeshBuilder, k: &Ksztalt, size: f64) {
    let z0 = k.z_c - 0.17 * size;
    let y0 = k.belt(z0) + 0.045 * size;
    let x_in = k.hw - 0.035;
    // vany: obudowa wyższa niż szeroka (duże lustro na ramieniu)
    let (len_x, ry, rz) = (0.17 * size, 0.068 * size * size, 0.055 * size);
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

/// Proste wnętrze widoczne przez półprzezroczyste szyby: ciemna wykładzina kabiny (od środka),
/// fotele z oparciami i zagłówkami w punktach H, deska rozdzielcza i kierownica (materiał
/// `Detail` — niesymetryczne z natury).
pub(crate) fn interior(mb: &mut MeshBuilder, k: &Ksztalt, seats: &[[f64; 3]]) {
    use crate::karoseria::{R_BELT, R_GT, R_LOW, R_RAIL, R_SH, R_TOP};
    // wykładzina: przekroje kabiny cofnięte o 3 cm do środka, normalne do wnętrza
    let (z0, z1) = ((k.z_win_r - 0.1).max(k.zr + 0.15), k.z_c + 0.02);
    let floor = k.clr + 0.12;
    let n = ((z1 - z0) / 0.1).ceil().max(2.0) as usize;
    let mut pw = Powloka::default();
    let rows: Vec<Vec<u32>> = (0..=n)
        .map(|i| {
            let z = lerp(z0, z1, i as f64 / n as f64);
            let q = k.key(z);
            let inset = |p: [f64; 2]| [(p[0] - 0.03).max(0.0), p[1]];
            let mut ring = vec![[0.0, floor], [inset(q[R_LOW])[0], floor]];
            for j in [R_LOW, R_SH, R_BELT, R_GT, R_RAIL, R_TOP] {
                let p = inset(q[j]);
                ring.push([p[0], if j == R_TOP { p[1] - 0.03 } else { p[1] }]);
            }
            ring.iter().map(|p| pw.vert([p[0], p[1].max(floor), z])).collect()
        })
        .collect();
    for i in 0..n {
        for j in 0..rows[i].len() - 1 {
            // odwrotna kolejność niż w powłoce: ściany zwrócone do środka kabiny
            pw.quad(rows[i][j], rows[i + 1][j], rows[i + 1][j + 1], rows[i][j + 1], MatKey::Plastic);
        }
    }
    pw.fan(&rows[n].iter().rev().copied().collect::<Vec<_>>(), MatKey::Plastic);
    pw.fan(&rows[0], MatKey::Plastic);
    pw.mirror_x();
    pw.weld(1e-7);
    pw.emit(mb, 40.0);
    // fotele
    for s in seats {
        let (x, y, z) = (s[0], s[1], s[2]);
        mb.bx(MatKey::Detail, [x - 0.24, y - 0.16, z - 0.05], [x + 0.24, y - 0.04, z + 0.42]);
        mb.bx(MatKey::Detail, [x - 0.24, y - 0.1, z - 0.2], [x + 0.24, y + 0.55, z - 0.06]);
        mb.bx(MatKey::Detail, [x - 0.12, y + 0.58, z - 0.17], [x + 0.12, y + 0.78, z - 0.09]);
    }
    // deska rozdzielcza przy podstawie szyby i kierownica przed kierowcą
    let zd = k.z_c - 0.05;
    let yd = k.belt(zd);
    let hx = (k.key(zd)[R_BELT][0] - 0.05).max(0.3);
    mb.bx(MatKey::Detail, [-hx, yd - 0.28, zd - 0.35], [hx, yd + 0.03, zd]);
    if let Some(d) = seats.first() {
        let (cx, cy, cz) = (d[0], d[1] + 0.33, d[2] + 0.5);
        let ring: Vec<[f64; 3]> = (0..12).map(|i| {
            let a = i as f64 / 12.0 * std::f64::consts::TAU;
            [cx + 0.19 * a.cos(), cy + 0.19 * a.sin() * 0.85, cz + 0.19 * a.sin() * 0.5]
        }).collect();
        for i in 0..12 {
            let (a, b) = (ring[i], ring[(i + 1) % 12]);
            mb.quad(MatKey::Detail, a, b, [b[0], b[1], b[2] - 0.03], [a[0], a[1], a[2] - 0.03], Some([0.0, 0.3, -1.0]));
        }
        mb.bx(MatKey::Detail, [cx - 0.03, cy - 0.03, cz], [cx + 0.03, cy + 0.03, zd - 0.2]);
    }
}
