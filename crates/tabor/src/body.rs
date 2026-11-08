//! Nadwozie osobówek i vana: reguła „ground” (osie) → zwisy → loft ze stacji wzdłuż Z.
//! Każda stacja to pierścień z półprzekroju 7 wierzchołków (lustro po X → 12). Profil boczny
//! (maska, szyba, dach, tył), nadkola i linia okien to funkcje z, próbkowane w stacjach.

use crate::{roll, P};
use crate::sluzba::{self, Hull};
use procedura_kit::mesh::{MatKey, MeshBuilder, MeshData, V3};

pub struct Layout {
    pub wheelbase: f64,
    /// Rozstaw środków opon.
    pub track: f64,
    /// Liczba stacji lofta.
    pub stations: usize,
    /// Belka świetlna służby (siatka, przesunięcie od origin); `None` dla cywilnego.
    pub lightbar: Option<(MeshData, V3)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Hatch,
    Liftback,
    Sedan,
    Kombi,
    Suv,
    Van,
}

#[derive(Clone, Copy, PartialEq)]
enum Zone {
    Hood,
    Wind,
    Roof,
    RearGlass,
    Deck,
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Interpolacja liniowa po punktach (z rosnące), poza zakresem stała.
fn pl(pts: &[(f64, f64)], z: f64) -> f64 {
    if z <= pts[0].0 {
        return pts[0].1;
    }
    for w in pts.windows(2) {
        if z <= w[1].0 {
            let d = w[1].0 - w[0].0;
            return if d < 1e-9 { w[1].1 } else { lerp(w[0].1, w[1].1, (z - w[0].0) / d) };
        }
    }
    pts[pts.len() - 1].1
}

pub(crate) type Half = [V3; 7];

/// Pasek na ściance bocznej między segmentami półprzekroju `k0..k1`, przesunięty na zewnątrz o `e`, po obu stronach.
fn strip(mb: &mut MeshBuilder, mat: MatKey, half: &dyn Fn(f64) -> Half, (za, zb): (f64, f64), (k0, k1): (usize, usize), e: f64) {
    let (a, b) = (half(za), half(zb));
    for k in k0..k1 {
        for side in [1.0, -1.0] {
            let p = |v: V3| [(v[0] + e) * side, v[1], v[2]];
            mb.quad(mat, p(a[k]), p(b[k]), p(b[k + 1]), p(a[k + 1]), Some([side, 0.0, 0.0]));
        }
    }
}

pub fn build(p: &P, seed: u32, mb: &mut MeshBuilder) -> Layout {
    let kind = match p.s("bodyType") {
        "hatchback" => Kind::Hatch,
        "liftback" => Kind::Liftback,
        "sedan" => Kind::Sedan,
        "kombi" => Kind::Kombi,
        "suv" => Kind::Suv,
        // ponytail: „podwozie” (ciężarowe, issue 07) na razie dostaje profil vana
        _ => Kind::Van,
    };
    let (l, w, h, wb) = (p.f("length"), p.f("width"), p.f("height"), p.f("wheelbase"));
    let round = p.f("roundness");
    let (hw, clr) = (w / 2.0 - 0.025, p.f("clearance")); // 2,5 cm zapasu na lusterka
    let (r, tw) = (p.f("wheelDiameter") / 2.0, p.f("tyreWidth"));
    let ra = r + p.f("wheelArchGap");
    let (a, at) = (ra * 1.12, ra * 0.55); // pół-szerokość nadkola u podstawy i u góry (trapez)
    let archtop = r + ra;

    // reguła „ground”: osie wyznaczają wszystko; zwisy skalowane tak, by długość = `length`
    let (fo, ro) = (p.f("frontOverhang"), p.f("rearOverhang"));
    let oh = (l - wb).max(0.8);
    let (zaf, zar) = (wb / 2.0, -wb / 2.0);
    let zf = zaf + oh * fo / (fo + ro) - 0.04; // 4 cm na wystające zderzaki
    let zr = zar - oh * ro / (fo + ro) + 0.04;
    let track = (p.f("trackRatio") * w).min(w - 0.03 - tw);

    // wysokości
    let yb = (p.f("beltline") * h).max(archtop + 0.12).min(h - 0.3);
    let yh = (p.f("hoodHeight") * h).min(yb + 0.04).clamp(clr + 0.5, h - 0.2);
    let c = (p.f("chamfer") + 0.06 * round).min(0.14);
    let ynose = (yh - 0.10 - 0.06 * round).max(clr + 0.45);
    let sb = clr + 0.07; // dolna krawędź boku poza nadkolami
    let mid_base = sb + 0.20; // talia (górna krawędź okładziny), w nadkolach wyżej
    let tumble = p.f("tumblehome");

    // profil boczny: maska → szyba → dach → tylna szyba → pokrywa
    let cowl = (zaf - a).min(zf - 0.45);
    let roof_f = cowl - (h - yh) / p.f("windshieldAngle").to_radians().tan();
    let (deck, mut ytail) = match kind {
        Kind::Hatch | Kind::Kombi | Kind::Suv => (0.0, yb * 0.985),
        Kind::Liftback => (0.14, yb * 0.985),
        Kind::Sedan => (0.21 * l, yb * 0.98),
        Kind::Van => (0.0, h),
    };
    let zg = zr + deck;
    let mut run = if kind == Kind::Van { 0.0 } else { (h - ytail) / p.f("rearGlassAngle").to_radians().tan() };
    if run < 0.06 {
        (run, ytail) = (0.0, h); // pionowy tył: szyba jako naklejka na zaślepce
    }
    // ponytail: skrajne nadpisania (kabina krótsza niż 0,5 m) są tylko przycinane, nie rozwiązywane
    let roof_r = (zg + run).min(roof_f - 0.5).max(zg);
    let roof_f = roof_f.max(roof_r + 0.2);
    let cowl = cowl.max(roof_f + 0.1);
    // dach z lekkim wybrzuszeniem (van: płaski), maska opada ostrzej tuż przed czołem
    let (crown_r, crown_f) = if kind == Kind::Van { (0.0, 0.0) } else { (0.045, 0.025) };
    let z_peak = roof_f - 0.45 * (roof_f - roof_r);
    let top = [(zr, ytail), (zg, ytail), (roof_r, h - crown_r), (z_peak, h), (roof_f, h - crown_f), (cowl, yh), (zf - 0.18, ynose + 0.03), (zf, ynose)];
    let t_at = |z: f64| pl(&top, z);
    // vany: szyby tylko w kabinie (do 0,55 m za przednim brzegiem dachu), dalej burta zabudowy
    let z_dw = if kind == Kind::Van { (roof_f - 0.55).max(roof_r) } else { f64::NEG_INFINITY };

    // nadkola: dolna krawędź boku podnosi się do trapezu wokół osi
    let sill_at = |z: f64| {
        let arch = |za: f64| {
            let d = (z - za).abs();
            if d <= at { archtop } else if d < a { lerp(archtop, sb, (d - at) / (a - at)) } else { sb }
        };
        arch(zaf).max(arch(zar))
    };
    // zwężenie planu przy czole i tyle (roundness)
    let (nfac, tfac) = (1.0 - 0.10 * round - 0.02, 1.0 - 0.08 * round - 0.02);
    let taper = |z: f64| {
        if z > zaf + a {
            lerp(1.0, nfac, ((z - zaf - a) / (zf - zaf - a)).clamp(0.0, 1.0))
        } else if z < zar - a {
            lerp(1.0, tfac, ((zar - a - z) / (zar - a - zr)).clamp(0.0, 1.0))
        } else {
            1.0
        }
    };

    // półprzekrój w z: dno, próg, talia, linia okien, róg szyby, krawędź dachu, oś dachu
    let half = |z: f64| -> Half {
        let tz = t_at(z);
        let hz = hw * taper(z);
        let g = ((tz - yb) / (h - yb)).clamp(0.0, 1.0);
        let hr = hz * (1.0 - tumble * g);
        let cc = c.min(0.3 * hr);
        let bel = yb.min(tz - cc - 0.03);
        let sill = sill_at(z);
        let mid = mid_base.max(sill + 0.07).min(bel - 0.04);
        let sill = sill.min(mid - 0.02);
        [[0.0, clr, z], [hz * 0.96, sill, z], [hz, mid, z], [hz, bel, z], [hr, tz - cc, z], [hr - cc, tz, z], [0.0, tz, z]]
    };
    let full = |z: f64| -> Vec<V3> {
        let v = half(z);
        let mut ring = v.to_vec();
        ring.extend((1..=5).rev().map(|i| [-v[i][0], v[i][1], v[i][2]]));
        ring
    };

    // stacje: (z, priorytet przy scalaniu); max 12, bliższe niż 12 cm łączą się
    let mut st: Vec<(f64, u8)> = vec![(zr, 3), (zf, 3), (zf - 0.18, 2), (cowl, 2), (roof_f, 2), (z_peak, 2), (roof_r, 2)];
    if kind == Kind::Van {
        st.push((z_dw, 2));
    }
    for za in [zaf, zar] {
        st.extend([(za - a, 1), (za - at, 1), (za + at, 1), (za + a, 1)]);
    }
    st.retain(|&(z, pr)| pr == 3 || (z > zr + 0.02 && z < zf - 0.02));
    st.sort_by(|x, y| x.0.total_cmp(&y.0));
    loop {
        let (i, gap) = (0..st.len() - 1).map(|i| (i, st[i + 1].0 - st[i].0)).min_by(|x, y| x.1.total_cmp(&y.1)).unwrap();
        if gap >= 0.12 && st.len() <= 12 {
            break;
        }
        let (x, y) = (st[i], st[i + 1]);
        let z = if x.1 == y.1 { (x.0 + y.0) / 2.0 } else if x.1 > y.1 { x.0 } else { y.0 };
        st[i] = (z, x.1.max(y.1));
        st.remove(i + 1);
    }
    let zs: Vec<f64> = st.iter().map(|s| s.0).collect();
    let rings: Vec<Vec<V3>> = zs.iter().map(|&z| full(z)).collect();

    // materiały pasów: strefa (maska/szyba/dach/tylna szyba/pokrywa) × segment półprzekroju
    let roof_mat = if roll(seed, 1.0, p.f("twoToneRoof")) { MatKey::RoofPaint } else { MatKey::Paint };
    let cladding = roll(seed, 2.0, p.f("claddingBlack"));
    let zone = |zm: f64| match zm {
        z if z > cowl => Zone::Hood,
        z if z > roof_f => Zone::Wind,
        z if z > roof_r => Zone::Roof,
        z if z > zg => Zone::RearGlass,
        _ => Zone::Deck,
    };
    mb.loft_with(&rings, Some(MatKey::Paint), |i, s| {
        let zm = (zs[i] + zs[i + 1]) / 2.0;
        let zn = zone(zm);
        let cabin = zn != Zone::Hood && zn != Zone::Deck;
        let side_glass = matches!(zn, Zone::Wind | Zone::RearGlass) || (zn == Zone::Roof && zm > z_dw);
        match if s <= 5 { s } else { 11 - s } {
            0 => MatKey::Plastic,
            1 if cladding => MatKey::Plastic,
            3 if side_glass => MatKey::Glass,
            4 if cabin => roof_mat,
            5 if matches!(zn, Zone::Wind | Zone::RearGlass) => MatKey::Glass,
            5 if zn == Zone::Roof => roof_mat,
            _ => MatKey::Paint,
        }
    });

    details(mb, p, seed, &Ctx { kind, hw, h, clr, ynose, ytail, yb, zf, zr, cowl, roof_r, zg, z_dw, run }, &half, &taper);
    let hull = Hull { zs: &zs, half: &half, top: &t_at, van: kind == Kind::Van, h, yb, clr, ytail, zf, zr, zg, cowl, roof_f, roof_r, z_dw };
    let lightbar = sluzba::apply(mb, p, seed, &hull);
    Layout { wheelbase: wb, track, stations: zs.len(), lightbar }
}

struct Ctx {
    kind: Kind,
    hw: f64,
    h: f64,
    clr: f64,
    ynose: f64,
    ytail: f64,
    yb: f64,
    zf: f64,
    zr: f64,
    cowl: f64,
    roof_r: f64,
    zg: f64,
    z_dw: f64,
    run: f64,
}

/// Zderzaki, grill, światła, szwy drzwi, klamki, słupek B, lusterka; van: zabudowa i tylne drzwi.
fn details(mb: &mut MeshBuilder, p: &P, seed: u32, c: &Ctx, half: &dyn Fn(f64) -> Half, taper: &dyn Fn(f64) -> f64) {
    let Ctx { kind, hw, h, clr, ynose, ytail, yb, zf, zr, cowl, roof_r, zg, z_dw, run } = *c;
    let van = kind == Kind::Van;
    let (hn, ht) = (hw * taper(zf), hw * taper(zr));
    let mirror = |mb: &mut MeshBuilder, mat: MatKey, pts: &[V3], want: V3| {
        for side in [1.0, -1.0] {
            let q: Vec<V3> = pts.iter().map(|v| [v[0] * side, v[1], v[2]]).collect();
            mb.poly(mat, &q, Some([want[0] * side, want[1], want[2]]));
        }
    };

    // zderzaki jako wysunięte bryły (kolor `bumperColor`)
    let bh = if van { 0.32 } else { 0.24 };
    mb.bx(MatKey::Bumper, [-hn * 0.985, clr + 0.08, zf - 0.06], [hn * 0.985, clr + 0.08 + bh, zf + 0.04]);
    mb.bx(MatKey::Bumper, [-ht * 0.985, clr + 0.08, zr - 0.04], [ht * 0.985, clr + 0.08 + bh, zr + 0.06]);

    // grill między reflektorami: 0 = mały zamknięty panel, 1 = duży; dolny wlot w zderzaku
    let g = p.f("grill");
    let gw = lerp(0.14, 0.36, g) * hn;
    let gh = lerp(0.05, 0.17, g) * ((ynose - clr) / 0.7).clamp(1.0, 2.0);
    let gy = ynose - 0.05 - gh;
    mb.quad(MatKey::Plastic, [-gw, gy, zf + 0.003], [gw, gy, zf + 0.003], [gw, gy + gh, zf + 0.003], [-gw, gy + gh, zf + 0.003], Some([0.0, 0.0, 1.0]));
    let iw = lerp(0.30, 0.50, g) * hn;
    mb.quad(MatKey::Plastic, [-iw, clr + 0.13, zf + 0.043], [iw, clr + 0.13, zf + 0.043], [iw, clr + 0.08 + bh - 0.05, zf + 0.043], [-iw, clr + 0.08 + bh - 0.05, zf + 0.043], Some([0.0, 0.0, 1.0]));

    // reflektory według `lampStyle`
    let (y0, y1) = (ynose - 0.20, ynose - 0.045);
    let ym = (y0 + y1) / 2.0;
    let zl = zf + 0.003;
    // ciemna oprawa pod lampą, żeby biały reflektor nie ginął na białym lakierze
    let sx0 = gw + 0.03; // pasek zaczyna się za grillem
    let (hx0, hx1) = if p.s("lampStyle") == "strip" { (sx0, 0.96 * hn) } else { (0.39 * hn, 0.96 * hn) };
    let (hy0, hy1) = if p.s("lampStyle") == "strip" { (ym - 0.05, ym + 0.04) } else { (y0 - 0.015, y1 + 0.015) };
    mirror(mb, MatKey::Plastic, &[[hx0, hy0, zf + 0.002], [hx1, hy0, zf + 0.002], [hx1, hy1, zf + 0.002], [hx0, hy1, zf + 0.002]], [0.0, 0.0, 1.0]);
    match p.s("lampStyle") {
        "oval" => mirror(mb, MatKey::Headlight, &[[0.44 * hn, y0 + 0.01, zl], [0.86 * hn, y0 + 0.01, zl], [0.94 * hn, ym, zl], [0.86 * hn, y1, zl], [0.44 * hn, y1, zl], [0.40 * hn, ym, zl]], [0.0, 0.0, 1.0]),
        "strip" => mirror(mb, MatKey::Headlight, &[[sx0 + 0.01, ym - 0.02, zl], [0.95 * hn, ym - 0.04, zl], [0.95 * hn, ym + 0.025, zl], [sx0 + 0.01, ym + 0.025, zl]], [0.0, 0.0, 1.0]),
        _ => mirror(mb, MatKey::Headlight, &[[0.46 * hn, y0 + 0.05, zl], [0.95 * hn, y0 + 0.01, zl], [0.95 * hn, y1, zl], [0.40 * hn, y1, zl]], [0.0, 0.0, 1.0]),
    }

    // tył: światła (w vanie pionowe pasy w narożach), pasek między nimi, szyba na pionowym tyle
    let zt = zr - 0.003;
    let (ly0, ly1, lx0, lx1) = if van { (clr + 0.45, clr + 1.05, 0.86 * ht, 0.985 * ht) } else { (ytail - 0.06 - 0.17, ytail - 0.06, 0.55 * ht, 0.95 * ht) };
    mirror(mb, MatKey::Taillight, &[[lx1, ly0, zt], [lx0, ly0, zt], [lx0, ly1, zt], [lx1, ly1, zt]], [0.0, 0.0, -1.0]);
    if roll(seed, 4.0, p.f("rearLightBar")) && !van {
        mb.quad(MatKey::Taillight, [-lx0, ly1 - 0.05, zt], [-lx0, ly1 - 0.01, zt], [lx0, ly1 - 0.01, zt], [lx0, ly1 - 0.05, zt], Some([0.0, 0.0, -1.0]));
    }
    if run == 0.0 {
        let (wy0, wy1) = (if van { h * 0.55 } else { yb + 0.04 }, h - 0.22);
        mirror(mb, MatKey::Glass, &[[0.80 * ht, wy0, zt], [0.0, wy0, zt], [0.0, wy1, zt], [0.80 * ht, wy1, zt]], [0.0, 0.0, -1.0]);
    }
    if van {
        // tylne drzwi dwuskrzydłowe: szew pośrodku i zawiasy
        mb.quad(MatKey::Plastic, [0.008, clr + 0.3, zt], [-0.008, clr + 0.3, zt], [-0.008, h - 0.05, zt], [0.008, h - 0.05, zt], Some([0.0, 0.0, -1.0]));
        for hy in [clr + 0.45, h - 0.4] {
            mirror(mb, MatKey::Plastic, &[[hw * taper(zr) * 0.985, hy, zt], [hw * taper(zr) * 0.94, hy, zt], [hw * taper(zr) * 0.94, hy + 0.12, zt], [hw * taper(zr) * 0.985, hy + 0.12, zt]], [0.0, 0.0, -1.0]);
        }
    }

    // drzwi: szwy z A/B/C, klamki, słupek B na szkle; van: tylko kabina + drzwi przesuwne
    let za = cowl - 0.12;
    let zc = if van { z_dw } else { (roof_r + 0.2).max(zg + 0.2) };
    let zb = (za + zc) / 2.0 + 0.05;
    let d = 0.007;
    let seam_top = if van { 4 } else { 3 };
    let seams = if van { vec![za, zc, z_dw - 0.12, z_dw - 1.27] } else { vec![za, zb, zc] };
    for z in seams {
        strip(mb, MatKey::Plastic, half, (z - d, z + d), (2, seam_top), 0.003);
    }
    let handle = |mb: &mut MeshBuilder, z: f64| {
        let bel = half(z)[3];
        strip_quad(mb, MatKey::Plastic, bel[0] + 0.005, bel[1] - 0.09, bel[1] - 0.065, z - 0.07, z + 0.07);
    };
    if van {
        handle(mb, zc + 0.14);
        handle(mb, z_dw - 1.12);
        // szyna drzwi przesuwnych
        strip_quad(mb, MatKey::Plastic, half(z_dw - 0.7)[2][0] + 0.004, half(z_dw)[2][1] + 0.02, half(z_dw)[2][1] + 0.05, z_dw - 1.27, z_dw - 0.12);
    } else {
        strip(mb, MatKey::Paint, half, (zb - 0.04, zb + 0.04), (3, 4), 0.004);
        handle(mb, zb - 0.14);
        handle(mb, zc + 0.12);
    }

    if van {
        van_extras(mb, c, half, taper);
    }

    // lusterka (wystają do 1,5 cm poza szerokość)
    let m = half(za - 0.08)[3];
    for side in [1.0, -1.0] {
        let (x0, x1) = if side > 0.0 { (m[0] - 0.02, m[0] + 0.04) } else { (-m[0] - 0.04, -m[0] + 0.02) };
        mb.bx(MatKey::Plastic, [x0, m[1] + 0.01, za - 0.2], [x1, m[1] + 0.1, za - 0.02]);
    }
}

/// Pionowy prostokąt przy burcie (x stałe, y0..y1, z0..z1), po obu stronach.
fn strip_quad(mb: &mut MeshBuilder, mat: MatKey, x: f64, y0: f64, y1: f64, z0: f64, z1: f64) {
    for side in [1.0, -1.0] {
        mb.quad(mat, [x * side, y0, z0], [x * side, y0, z1], [x * side, y1, z1], [x * side, y1, z0], Some([side, 0.0, 0.0]));
    }
}

/// Zabudowa vana: żebra dachu, listwy ochronne, szyny drzwi przesuwnych, tylne drzwi.
fn van_extras(mb: &mut MeshBuilder, c: &Ctx, half: &dyn Fn(f64) -> Half, taper: &dyn Fn(f64) -> f64) {
    let Ctx { hw, h, clr, zr, z_dw, .. } = *c;
    let ht = hw * taper(zr);
    // żebra dachu
    let roof_hi = z_dw + 0.3;
    for i in 0..8 {
        let z = lerp(roof_hi, zr + 0.3, i as f64 / 7.0);
        let hr = half(z)[5][0] - 0.02;
        mb.bx(MatKey::Plastic, [-hr, h - 0.01, z - 0.025], [hr, h + 0.02, z + 0.025]);
    }
    let wall = |z: f64| half(z)[2][0];
    for side in [1.0, -1.0] {
        let sbx = |mb: &mut MeshBuilder, x: f64, y0: f64, y1: f64, z0: f64, z1: f64, out: f64| {
            let (a, b) = (x * side, (x + out) * side);
            mb.bx(MatKey::Plastic, [a.min(b), y0, z0], [a.max(b), y1, z1]);
        };
        // listwy ochronne za drzwiami przesuwnymi
        let (z0, z1) = (zr + 0.25, z_dw - 1.32);
        for y in [0.95, 1.22] {
            sbx(mb, wall((z0 + z1) / 2.0) - 0.01, y, y + 0.06, z0, z1, 0.03);
        }
        // szyny drzwi przesuwnych (góra i dół)
        let (s0, s1) = (z_dw - 1.27, z_dw - 0.12);
        for y in [1.55, 0.5] {
            sbx(mb, wall((s0 + s1) / 2.0) - 0.01, y, y + 0.05, s0, s1, 0.03);
        }
        // zawiasy i klamka tylnych drzwi
        for y in [clr + 0.5, h - 0.45] {
            mb.bx(MatKey::Plastic, [(ht - 0.04) * side - 0.04, y, zr - 0.03], [(ht - 0.04) * side + 0.04, y + 0.14, zr + 0.01]);
        }
        mb.bx(MatKey::Plastic, [0.05 * side - 0.03, 1.0, zr - 0.03], [0.05 * side + 0.03, 1.08, zr + 0.01]);
    }
    // stopień zderzaka i tablica
    mb.bx(MatKey::Plastic, [-0.55 * ht, clr + 0.32, zr - 0.06], [0.55 * ht, clr + 0.35, zr]);
    mb.bx(MatKey::Rim, [-0.26, clr + 0.36, zr - 0.03], [0.26, clr + 0.48, zr]);
}
