//! Moduły zabudowy na ramie pojazdów ciężkich: platforma lawety (PD), zabudowa ratownicza ze
//! skrytkami i żaluzjami (pożarnicza/techniczna), skrzynia; dodatki: HDS za kabiną, maszt
//! oświetleniowy i wyposażenie dachu. Gniazda: tył kabiny (HDS), tył ramy (zderzak), dach.

use crate::ciezkie::{beam, cyl, full_ring, half_seg, lerp, prism_x, side_quad, D};
use crate::tablice::plate;
use crate::P;
use procedura_kit::mesh::{MatKey, MeshBuilder, V3};

/// Zakres modułu wzdłuż ramy i wysokość jego dachu.
struct Span {
    z_r: f64,
    z_f: f64,
    ytop: f64,
    /// Półszerokość burty.
    hx: f64,
}

fn span(d: &D, p: &P) -> Span {
    let z_r = d.zr + 0.12;
    // 10 cm luzu na przechył kabiny, 1 m dla HDS za kabiną
    let avail = d.zb - 0.10 - if p.f("crane") > 0.5 { 1.0 } else { 0.0 };
    let want = p.f("moduleLength");
    let z_f = if want > 0.0 { (z_r + want).min(avail) } else { avail };
    let tall = p.f("crane") > 0.5 || p.f("roofRack") > 0.5;
    Span { z_r, z_f, ytop: if tall { d.h - 0.30 } else { d.h - 0.05 }, hx: d.w / 2.0 - 0.03 }
}

pub(crate) fn module(mb: &mut MeshBuilder, d: &D, p: &P, seed: u32) {
    let sp = span(d, p);
    match p.s("module") {
        "none" => {}
        "laweta" => laweta(mb, d, &sp, seed),
        "pozarniczy" => rescue(mb, d, p, &sp, false, seed),
        "techniczny" => rescue(mb, d, p, &sp, true, seed),
        "furgon" | "chlodnia" | "kontener" => box_body(mb, d, &sp, seed, false),
        "plandeka" => box_body(mb, d, &sp, seed, true),
        "wywrotka" => tipper(mb, d, &sp, seed),
        "siodlo" => fifth_wheel(mb, d, &sp, seed),
        // ponytail: doka i minibus jeszcze jako skrzynia
        _ => flatbed(mb, d, &sp, seed),
    }
    if p.f("crane") > 0.5 {
        crane(mb, d, p);
    }
}

/// Zderzak tylny (pod linią zejścia) z zespołami lamp (zewnętrzna część pomarańczowa —
/// kierunkowskaz) i tablicą pośrodku; `hb` = półszerokość.
fn rear(mb: &mut MeshBuilder, d: &D, hb: f64, seed: u32) {
    let y0 = (d.dep(d.zr) + 0.03).max(d.clr + 0.1);
    // czoło zderzaka 6 mm przed `zr`: lampy i tablica leżą na nim i nie wystają poza długość
    mb.bx(MatKey::Bumper, [-hb, y0, d.zr + 0.006], [hb, y0 + 0.25, d.zr + 0.12]);
    let z = d.zr + 0.003;
    for side in [1.0f64, -1.0] {
        let q = |x: f64, y: f64, dz: f64| [x * hb * side, y, z - dz];
        mb.quad(MatKey::Taillight, q(0.66, y0 + 0.04, 0.0), q(0.97, y0 + 0.04, 0.0), q(0.97, y0 + 0.21, 0.0), q(0.66, y0 + 0.21, 0.0), Some([0.0, 0.0, -1.0]));
        mb.quad(MatKey::Indicator, q(0.86, y0 + 0.045, 0.001), q(0.965, y0 + 0.045, 0.001), q(0.965, y0 + 0.205, 0.001), q(0.86, y0 + 0.205, 0.001), Some([0.0, 0.0, -1.0]));
    }
    plate(mb, seed, 7.0, [0.0, y0 + 0.125, z + 0.003], -1.0); // tło na zr+0,005, znaki na zr+0,004
}

/// Boczne światła obrysowe (pomarańczowe, co ok. 2,5 m) przy dolnej krawędzi zabudowy w `x = hx`;
/// `y(z)` = wysokość dolnej krawędzi lampy (nad nadkolem wyżej).
fn side_markers(mb: &mut MeshBuilder, hx: f64, y: &dyn Fn(f64) -> f64, z0: f64, z1: f64) {
    let n = ((z1 - z0) / 2.5).ceil().max(1.0) as usize;
    for i in 0..=n {
        let z = lerp(z0, z1, i as f64 / n as f64);
        let y = y(z);
        side_quad(mb, MatKey::Indicator, hx + 0.004, y, y + 0.05, z - 0.06, z + 0.06);
    }
}

// ───────────────────────── laweta / skrzynia ─────────────────────────

/// Pokład: wysokość spodu nad kołami tylnymi, podwozie pomocnicze, rampa na tyle.
fn deck(mb: &mut MeshBuilder, d: &D, sp: &Span) -> f64 {
    let yb = (d.y_fr + 0.12).max(2.0 * d.r + 0.1).max(d.dep(sp.z_r) + 0.02);
    let hxd = d.w / 2.0 - 0.02;
    let zs = sp.z_r + 0.5;
    mb.bx(MatKey::Metal, [-hxd, yb, zs], [hxd, yb + 0.1, sp.z_f]);
    // rampa: klin opadający ku tyłowi
    prism_x(mb, MatKey::Metal, &[(zs, yb + 0.1), (sp.z_r, yb + 0.03), (sp.z_r, yb), (zs, yb)], -hxd, hxd);
    // podwozie pomocnicze: dwie belki wzdłużne i poprzeczki
    for side in [1.0f64, -1.0] {
        let (a, b) = (0.43 * side, 0.7 * side);
        let z0 = d.kick_z(d.y_fr + 0.04).max(sp.z_r + 0.3);
        mb.bx(MatKey::Frame, [a.min(b), d.y_fr, z0], [a.max(b), yb, sp.z_f - 0.1]);
    }
    for z in [lerp(sp.z_r, sp.z_f, 0.25), lerp(sp.z_r, sp.z_f, 0.6), sp.z_f - 0.3] {
        mb.bx(MatKey::Frame, [-hxd + 0.1, yb - 0.1, z - 0.05], [hxd - 0.1, yb, z + 0.05]);
    }
    // żebra antypoślizgowe
    for i in 0..12 {
        let z = lerp(zs + 0.1, sp.z_f - 0.5, i as f64 / 11.0);
        mb.quad(MatKey::Frame, [-hxd + 0.12, yb + 0.103, z - 0.02], [hxd - 0.12, yb + 0.103, z - 0.02], [hxd - 0.12, yb + 0.103, z + 0.02], [-hxd + 0.12, yb + 0.103, z + 0.02], Some([0.0, 1.0, 0.0]));
    }
    yb + 0.1
}

fn headboard(mb: &mut MeshBuilder, sp: &Span, hxd: f64, yt: f64, h: f64) {
    let z0 = sp.z_f - 0.12;
    for side in [1.0f64, -1.0] {
        let (a, b) = ((hxd - 0.1) * side, hxd * side);
        mb.bx(MatKey::Paint, [a.min(b), yt, z0], [a.max(b), yt + h, sp.z_f]);
    }
    mb.bx(MatKey::Paint, [-hxd, yt + h - 0.08, z0], [hxd, yt + h, sp.z_f]);
    mb.bx(MatKey::Paint, [-hxd, yt, z0], [hxd, yt + 0.1, sp.z_f]);
    mb.quad(MatKey::Plastic, [-hxd + 0.1, yt + 0.1, z0 + 0.02], [hxd - 0.1, yt + 0.1, z0 + 0.02], [hxd - 0.1, yt + h - 0.08, z0 + 0.02], [-hxd + 0.1, yt + h - 0.08, z0 + 0.02], Some([0.0, 0.0, -1.0]));
}

/// Laweta: pokład z rampą, siłowniki, burta czołowa, wciągarka, niskie burty, zderzak z lampami.
fn laweta(mb: &mut MeshBuilder, d: &D, sp: &Span, seed: u32) {
    let yt = deck(mb, d, sp);
    let hxd = d.w / 2.0 - 0.02;
    headboard(mb, sp, hxd, yt, 0.75);
    // niskie burty (opuszczone szyny mocujące)
    for side in [1.0f64, -1.0] {
        let (a, b) = (hxd * side, (hxd - 0.05) * side);
        mb.bx(MatKey::Paint, [a.min(b), yt, sp.z_r + 0.5], [a.max(b), yt + 0.1, sp.z_f - 0.12]);
    }
    // wciągarka: bęben na wspornikach tuż za burtą czołową
    let (zw, yw) = (sp.z_f - 0.4, yt + 0.16);
    cyl(mb, MatKey::Metal, [-0.4, yw, zw], [0.4, yw, zw], 0.12, 8, true);
    for side in [1.0f64, -1.0] {
        let (a, b) = (0.4 * side, 0.46 * side);
        mb.bx(MatKey::Frame, [a.min(b), yt, zw - 0.14], [a.max(b), yw + 0.14, zw + 0.14]);
    }
    // siłowniki pochylania pod pokładem
    for side in [1.0f64, -1.0] {
        cyl(mb, MatKey::Metal, [0.56 * side, d.y_fr + 0.05, sp.z_f - 0.9], [0.56 * side, yt - 0.12, lerp(sp.z_r, sp.z_f, 0.45)], 0.045, 6, true);
    }
    side_markers(mb, hxd, &|_| yt - 0.08, sp.z_r + 0.6, sp.z_f - 0.3);
    rear(mb, d, hxd - 0.05, seed);
}

fn flatbed(mb: &mut MeshBuilder, d: &D, sp: &Span, seed: u32) {
    let yt = deck(mb, d, sp);
    let hxd = d.w / 2.0 - 0.02;
    headboard(mb, sp, hxd, yt, 0.9);
    // burty: dwie boczne i tylna (Paint)
    for side in [1.0f64, -1.0] {
        let (a, b) = (hxd * side, (hxd - 0.04) * side);
        mb.bx(MatKey::Paint, [a.min(b), yt, sp.z_r + 0.1], [a.max(b), yt + 0.4, sp.z_f - 0.12]);
    }
    mb.bx(MatKey::Paint, [-hxd, yt, sp.z_r + 0.06], [hxd, yt + 0.4, sp.z_r + 0.1]);
    side_markers(mb, hxd, &|_| yt + 0.05, sp.z_r + 0.3, sp.z_f - 0.3);
    rear(mb, d, hxd - 0.05, seed);
}

// ───────────────────────── zabudowa ratownicza ─────────────────────────

/// Żaluzja: ramka, lamele aluminium z żebrami i uchwyt; po obu stronach (`side_quad`).
fn shutter(mb: &mut MeshBuilder, hx: f64, z0: f64, z1: f64, y0: f64, y1: f64) {
    side_quad(mb, MatKey::Frame, hx + 0.006, y0, y1, z0, z1);
    let (z0, z1, y0, y1) = (z0 + 0.035, z1 - 0.035, y0 + 0.035, y1 - 0.035);
    let x = hx + 0.012;
    side_quad(mb, MatKey::Metal, x, y0, y1, z0, z1);
    let n = ((y1 - y0) / 0.17).floor().max(1.0) as usize;
    for i in 1..n {
        let y = lerp(y0, y1, i as f64 / n as f64);
        side_quad(mb, MatKey::Frame, x + 0.003, y - 0.007, y + 0.007, z0, z1);
    }
    let zm = (z0 + z1) / 2.0;
    side_quad(mb, MatKey::Plastic, x + 0.005, y0 + 0.03, y0 + 0.07, zm - 0.1, zm + 0.1);
}

/// Zabudowa ratownicza: powłoka lofta z nadkolami i kopniakiem na tyle, skrytki z żaluzjami
/// w 2 rzędach, przedział przelotowy, dach roboczy, tył ze skrytką, wyposażenie dachu.
fn rescue(mb: &mut MeshBuilder, d: &D, p: &P, sp: &Span, tech: bool, seed: u32) {
    let hx = sp.hx;
    let pass = p.f("passThrough").min((sp.z_f - sp.z_r - 1.0).max(0.0));
    let z_a = sp.z_f - pass; // przód zamkniętej powłoki
    let base = d.y_fr + 0.02;
    let ymid = 0.5 * (sp.ytop + 1.2);
    let half = |z: f64| -> [V3; 7] {
        let yb = base.max(d.dep(z));
        let sl = yb.max(d.arch_r(z));
        [[0.0, yb, z], [hx - 0.06, sl, z], [hx, sl + 0.06, z], [hx, ymid.max(sl + 0.1), z], [hx, sp.ytop - 0.05, z], [hx - 0.05, sp.ytop, z], [0.0, sp.ytop, z]]
    };
    // powłoka zamknięta aż do przodu zabudowy; przedział przelotowy to skrytka z żaluzjami po obu stronach
    let mut zs = vec![sp.z_r, sp.z_f];
    let kick = d.kick_z(base);
    zs.push(kick);
    for &za in &d.rear_axles {
        zs.extend([za - d.arch_a, za - d.arch_at, za + d.arch_at, za + d.arch_a].into_iter().filter(|&z| z > sp.z_r + 0.05 && z < sp.z_f - 0.05));
    }
    zs.sort_by(|a, b| a.total_cmp(b));
    zs.dedup_by(|a, b| (*a - *b).abs() < 0.08);
    let rings: Vec<Vec<V3>> = zs.iter().map(|&z| full_ring(half(z))).collect();
    mb.loft_with(&rings, Some(MatKey::Paint), |_, s| match half_seg(s) {
        0 | 1 => MatKey::Frame,
        5 => MatKey::Metal,
        _ => MatKey::Paint,
    });

    // skrytki: kolumny równej szerokości, dolny rząd omija nadkole i linię zejścia
    let n = p.f("compartments") as usize;
    if n > 0 {
        let (c0, c1) = (sp.z_r + 0.2, z_a - 0.1);
        // żaluzje mają realnie ≤ 1,6 m szerokości: dłuższa zabudowa dostaje więcej kolumn
        let n = n.max(((c1 - c0) / 1.6).ceil() as usize);
        let wc = (c1 - c0) / n as f64;
        let yu1 = sp.ytop - 0.14;
        let yu0 = yu1 - 0.75;
        for i in 0..n {
            let (za, zb) = (c0 + wc * i as f64 + 0.03, c0 + wc * (i + 1) as f64 - 0.03);
            shutter(mb, hx, za, zb, yu0, yu1);
            let mut yl0 = base + 0.12;
            // nadkole: tylko jeśli kolumna zachodzi na trapez
            if d.rear_axles.iter().any(|&ax| zb > ax - d.arch_a && za < ax + d.arch_a) {
                yl0 = yl0.max(d.archtop + 0.06);
            }
            yl0 = yl0.max(d.dep(za) + 0.06);
            let yl1 = yu0 - 0.05;
            if yl1 - yl0 > 0.3 {
                shutter(mb, hx, za, zb, yl0, yl1);
            }
        }
    }
    // przedział przelotowy: jedna wysoka skrytka dostępna z obu stron
    if pass > 0.0 {
        let yl0 = (base + 0.12).max(d.dep(z_a) + 0.06);
        shutter(mb, hx, z_a + 0.03, sp.z_f - 0.03, yl0, sp.ytop - 0.14);
    }

    // tył: zderzak z lampami, skrytka tylna; boczne obrysówki nad dolną krawędzią
    side_markers(mb, hx, &|z| (base + 0.1).max(d.arch_r(z) + 0.1).max(d.dep(z) + 0.1), sp.z_r + 0.3, z_a - 0.3);
    rear(mb, d, hx - 0.05, seed);
    let zt = sp.z_r - 0.004;
    let yb = base.max(d.dep(sp.z_r));
    let (ry0, ry1) = (yb + 0.5, sp.ytop - 0.3);
    if ry1 - ry0 > 0.4 {
        let (x0, x1) = (-0.6, 0.6);
        mb.quad(MatKey::Frame, [x0, ry0, zt], [x1, ry0, zt], [x1, ry1, zt], [x0, ry1, zt], Some([0.0, 0.0, -1.0]));
        mb.quad(MatKey::Metal, [x0 + 0.03, ry0 + 0.03, zt - 0.004], [x1 - 0.03, ry0 + 0.03, zt - 0.004], [x1 - 0.03, ry1 - 0.03, zt - 0.004], [x0 + 0.03, ry1 - 0.03, zt - 0.004], Some([0.0, 0.0, -1.0]));
        for i in 1..5 {
            let y = lerp(ry0, ry1, i as f64 / 5.0);
            mb.quad(MatKey::Frame, [x0 + 0.03, y - 0.007, zt - 0.007], [x1 - 0.03, y - 0.007, zt - 0.007], [x1 - 0.03, y + 0.007, zt - 0.007], [x0 + 0.03, y + 0.007, zt - 0.007], Some([0.0, 0.0, -1.0]));
        }
    }

    if p.f("roofRack") > 0.5 {
        roof(mb, d, sp, z_a, tech);
    }
}

/// Wyposażenie dachu: skrzynia, drabina i działko (pożarnicza) albo leżący maszt (techniczna).
fn roof(mb: &mut MeshBuilder, d: &D, sp: &Span, z_a: f64, tech: bool) {
    let y = sp.ytop;
    mb.bx(MatKey::Metal, [0.15, y, z_a - 1.4], [0.95, y + 0.26, z_a - 0.25]);
    if tech {
        // maszt oświetleniowy złożony wzdłuż prawej krawędzi, głowica z lampami na końcu
        let (x, zr0, zr1) = (-0.85, sp.z_r + 0.7, z_a - 0.3);
        cyl(mb, MatKey::Metal, [x, y + 0.12, zr0], [x, y + 0.12, zr1], 0.05, 6, true);
        for z in [zr0 + 0.3, zr1 - 0.3] {
            mb.bx(MatKey::Frame, [x - 0.06, y, z - 0.04], [x + 0.06, y + 0.12, z + 0.04]);
        }
        mb.bx(MatKey::Plastic, [x - 0.4, y + 0.12, zr0 - 0.2], [x + 0.4, y + 0.26, zr0]);
        mb.quad(MatKey::Headlight, [x - 0.36, y + 0.263, zr0 - 0.18], [x + 0.36, y + 0.263, zr0 - 0.18], [x + 0.36, y + 0.263, zr0 - 0.02], [x - 0.36, y + 0.263, zr0 - 0.02], Some([0.0, 1.0, 0.0]));
    } else {
        // drabina: dwa dźwigary i szczeble
        let (x0, x1, za, zb) = (-0.95, -0.55, sp.z_r + 0.5, z_a - 0.4);
        for x in [x0, x1] {
            mb.bx(MatKey::Metal, [x - 0.02, y, za], [x + 0.02, y + 0.07, zb]);
        }
        let k = ((zb - za) / 0.4).floor() as usize;
        for i in 0..=k {
            let z = lerp(za, zb, i as f64 / k as f64);
            mb.bx(MatKey::Metal, [x0, y + 0.03, z - 0.015], [x1, y + 0.06, z + 0.015]);
        }
        // działko dachowe
        let zm = z_a - 0.35;
        cyl(mb, MatKey::Frame, [0.0, y, zm], [0.0, y + 0.14, zm], 0.07, 6, true);
        beam(mb, MatKey::Metal, [0.0, y + 0.14, zm], [0.0, y + 0.2, zm - 0.5], 0.04, 0.04);
    }
    let _ = d;
}

// ───────────────────────── HDS ─────────────────────────

/// Żuraw za kabiną: podstawa z podporami, kolumna, dwa człony złożone nad dachem kabiny.
fn crane(mb: &mut MeshBuilder, d: &D, p: &P) {
    let sp = span(d, p);
    let zp = (d.zb - 0.1 - 0.5).max(sp.z_f + 0.5);
    let hx = d.w / 2.0 - 0.1;
    mb.bx(MatKey::Frame, [-0.55, d.y_fr, zp - 0.4], [0.55, d.y_fr + 0.22, zp + 0.4]);
    // podpory stabilizujące (złożone)
    for side in [1.0f64, -1.0] {
        let (a, b) = (0.55 * side, hx * side);
        mb.bx(MatKey::Frame, [a.min(b), d.y_fr + 0.02, zp - 0.15], [a.max(b), d.y_fr + 0.16, zp + 0.15]);
    }
    let y0 = (d.cab_top - 0.7).max(d.y_fr + 0.8);
    cyl(mb, MatKey::Paint, [0.0, d.y_fr + 0.22, zp], [0.0, y0, zp], 0.2, 8, true);
    // złożenie: człon 1 w górę przed tył kabiny, człon 2 poziomo nad dachem kabiny
    let yc = (d.cab_top + 0.12).min(d.h - 0.08);
    let p1 = [0.0, yc, d.zb + 0.15];
    beam(mb, MatKey::Paint, [0.0, y0, zp], p1, 0.13, 0.15);
    let p2 = [0.0, yc, (d.zb + 1.4).min(d.zc - 0.9)];
    beam(mb, MatKey::Metal, p1, p2, 0.09, 0.1);
    mb.bx(MatKey::Plastic, [-0.06, yc - 0.3, p2[2] - 0.05], [0.06, yc - 0.1, p2[2] + 0.05]);
}

// ───────────────────────── kontener, plandeka, wywrotka, siodło ─────────────────────────

/// Kontener (furgon, chłodnia) albo plandeka na podwoziu: skrzynia z narożnymi słupkami
/// i ramą z aluminium, tylne drzwi z ryglami; plandeka ma pionowe pasy napinające.
fn box_body(mb: &mut MeshBuilder, d: &D, sp: &Span, seed: u32, curtain: bool) {
    let yt = deck(mb, d, sp);
    let hx = d.w / 2.0 - 0.01;
    let (z0, z1, y1) = (sp.z_r + 0.06, sp.z_f, sp.ytop);
    mb.bx(MatKey::Paint, [-hx, yt, z0], [hx, y1, z1]);
    // słupki narożne i ramy dachu / podłogi (aluminium)
    let p = 0.05;
    for sx in [1.0f64, -1.0] {
        for &zz in &[z0, z1 - p] {
            let (a, b) = ((hx + 0.006) * sx, (hx - p) * sx);
            mb.bx(MatKey::Metal, [a.min(b), yt, zz], [a.max(b), y1 + 0.006, zz + p]);
        }
        side_quad(mb, MatKey::Metal, hx + 0.004, y1 - 0.07, y1, z0, z1);
        side_quad(mb, MatKey::Metal, hx + 0.004, yt, yt + 0.09, z0, z1);
    }
    if curtain {
        // pasy napinające plandeki co ok. 0,6 m i dolna listwa
        let n = ((z1 - z0) / 0.6).floor() as usize;
        for i in 1..n {
            let zz = lerp(z0, z1, i as f64 / n as f64);
            side_quad(mb, MatKey::Plastic, hx + 0.005, yt + 0.09, y1 - 0.07, zz - 0.02, zz + 0.02);
        }
    }
    // tylne drzwi: szczelina środkowa i rygle
    let zr = z0 - 0.003;
    let q = |mb: &mut MeshBuilder, m: MatKey, x0: f64, x1: f64, ya: f64, yb: f64| mb.quad(m, [x1, ya, zr], [x0, ya, zr], [x0, yb, zr], [x1, yb, zr], Some([0.0, 0.0, -1.0]));
    q(mb, MatKey::Plastic, -0.006, 0.006, yt + 0.05, y1 - 0.05);
    for x in [-0.55 * hx, -0.25 * hx, 0.25 * hx, 0.55 * hx] {
        q(mb, MatKey::Metal, x - 0.015, x + 0.015, yt + 0.12, y1 - 0.1);
    }
    side_markers(mb, hx + 0.006, &|_| yt + 0.05, sp.z_r + 0.3, sp.z_f - 0.3);
    rear(mb, d, hx - 0.05, seed);
}

/// Skrzynia wywrotki: burty stalowe z żebrami, czołownica wyższa nad kabinę, klapa tylna,
/// siłownik przed skrzynią.
fn tipper(mb: &mut MeshBuilder, d: &D, sp: &Span, seed: u32) {
    let yt = deck(mb, d, sp);
    let hx = d.w / 2.0 - 0.03;
    let wall = (sp.ytop - yt).clamp(0.4, 1.5);
    let (z0, z1) = (sp.z_r + 0.1, sp.z_f - 0.25);
    for sx in [1.0f64, -1.0] {
        let (a, b) = (hx * sx, (hx - 0.05) * sx);
        mb.bx(MatKey::Paint, [a.min(b), yt, z0], [a.max(b), yt + wall, z1]);
        // pionowe żebra burty
        let n = ((z1 - z0) / 0.7).floor() as usize;
        for i in 0..=n {
            let zz = lerp(z0 + 0.05, z1 - 0.05, i as f64 / n.max(1) as f64);
            let (c, e) = ((hx + 0.03) * sx, hx * sx);
            mb.bx(MatKey::Paint, [c.min(e), yt, zz - 0.04], [c.max(e), yt + wall, zz + 0.04]);
        }
    }
    // czołownica z daszkiem nad kabiną i klapa tylna
    mb.bx(MatKey::Paint, [-hx, yt, z1], [hx, yt + wall + 0.25, z1 + 0.06]);
    mb.bx(MatKey::Paint, [-hx, yt + wall + 0.2, z1], [hx, yt + wall + 0.25, z1 + 0.45]);
    mb.bx(MatKey::Paint, [-hx, yt, z0 - 0.06], [hx, yt + wall, z0]);
    mb.bx(MatKey::Frame, [-hx + 0.05, yt, z0], [hx - 0.05, yt + 0.02, z1]);
    cyl(mb, MatKey::Metal, [0.0, d.y_fr, z1 + 0.15], [0.0, yt + 0.4, z1 + 0.15], 0.07, 8, true);
    rear(mb, d, hx - 0.05, seed);
}

/// Siodło ciągnika: płyta siodła na mocowaniu, błotniki nad osiami tylnymi, pomost za kabiną,
/// zderzak tylny z lampami (ciągnik nie ma zabudowy).
fn fifth_wheel(mb: &mut MeshBuilder, d: &D, sp: &Span, seed: u32) {
    let (h, zc) = d.fifth_wheel.unwrap_or((1.15, d.zar + 0.5));
    // mocowanie i płyta (Ø ok. 0,9 m) z wycięciem na sworzeń od tyłu
    mb.bx(MatKey::Frame, [-0.45, d.y_fr, zc - 0.45], [0.45, h - 0.08, zc + 0.45]);
    cyl(mb, MatKey::Frame, [0.0, h - 0.08, zc], [0.0, h, zc], 0.46, 16, true);
    mb.quad(MatKey::Plastic, [-0.06, h + 0.001, zc - 0.47], [0.06, h + 0.001, zc - 0.47], [0.06, h + 0.001, zc], [-0.06, h + 0.001, zc], Some([0.0, 1.0, 0.0]));
    // błotniki nad osiami tylnymi (od zewnętrznego koła do ramy)
    let xo = d.w / 2.0 - 0.02;
    for &za in &d.rear_axles {
        for sx in [1.0f64, -1.0] {
            let (a, b) = (xo * sx, 0.45 * sx);
            let y = 2.0 * d.r + 0.08;
            mb.bx(MatKey::Plastic, [a.min(b), y, za - 0.6], [a.max(b), y + 0.02, za + 0.6]);
            let (c, e) = (xo * sx, (xo - 0.02) * sx);
            mb.bx(MatKey::Plastic, [c.min(e), d.r + 0.05, za - 0.6], [c.max(e), y, za - 0.57]);
        }
    }
    // pomost (blacha ryflowana) za kabiną
    mb.bx(MatKey::Metal, [-0.5, d.y_fr, sp.z_f - 0.6], [0.5, d.y_fr + 0.02, sp.z_f]);
    rear(mb, d, d.w / 2.0 - 0.15, seed);
}
