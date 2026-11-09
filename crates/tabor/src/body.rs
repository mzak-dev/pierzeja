//! Nadwozie osobówek i vana: reguła „ground” (osie) → zwisy → loft ze stacji wzdłuż Z.
//! Każda stacja to pierścień z półprzekroju 7 wierzchołków (lustro po X → 12). Profil boczny
//! (maska, szyba, dach, tył), nadkola i linia okien to funkcje z, próbkowane w stacjach.

use crate::sluzba::{self, Hull};
use crate::tablice::plate;
use crate::{roll, P};
use procedura_kit::mesh::{MatKey, MeshBuilder, MeshData, V3};

pub struct Layout {
    pub wheelbase: f64,
    /// Rozstaw środków opon.
    pub track: f64,
    /// Liczba stacji lofta.
    pub stations: usize,
    /// Szerokość nadwozia bez lusterek (tak podają producenci i dyrektywa 96/53/WE).
    pub body_width: f64,
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
fn strip(mb: &mut MeshBuilder, mat: MatKey, half: &dyn Fn(f64) -> Half, zab: (f64, f64), kk: (usize, usize), e: f64) {
    strip_on(mb, mat, half, zab, kk, e, &BOTH);
}

const BOTH: [f64; 2] = [1.0, -1.0];
/// Prawa burta (−X): drzwi przesuwne vana w ruchu prawostronnym.
const RIGHT: [f64; 1] = [-1.0];

fn strip_on(mb: &mut MeshBuilder, mat: MatKey, half: &dyn Fn(f64) -> Half, (za, zb): (f64, f64), (k0, k1): (usize, usize), e: f64, sides: &[f64]) {
    let (a, b) = (half(za), half(zb));
    for k in k0..k1 {
        for &side in sides {
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
    let (hw, clr) = (w / 2.0 - 0.005, p.f("clearance")); // lusterka wystają poza `width`
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
    // osobówka: maska prawie płaska, wysokie czoło (współczesne auta); van: maska opada do grilla
    let ynose = (yh - if kind == Kind::Van { 0.10 + 0.06 * round } else { 0.05 + 0.03 * round }).max(clr + 0.45);
    let sb = clr + 0.07; // dolna krawędź boku poza nadkolami
    let mid_base = sb + 0.20; // talia (górna krawędź okładziny), w nadkolach wyżej
    let tumble = p.f("tumblehome");

    // profil boczny: maska → szyba → dach → tylna szyba → pokrywa. Van (Sprinter, Transit):
    // podstawa szyby prawie nad przednią osią, szyba kończy się 0,3 m pod dachem, nad nią daszek.
    let van = kind == Kind::Van;
    let cowl = if van { zaf - 0.08 } else { zaf - a }.min(zf - 0.45);
    let ws_tan = p.f("windshieldAngle").to_radians().tan();
    let y_wt = if van { h - 0.3 } else { h };
    let z_wt = cowl - (y_wt - yh) / ws_tan;
    let roof_f = if van { z_wt - 0.35 } else { z_wt };
    let (deck, mut ytail) = match kind {
        Kind::Hatch | Kind::Kombi | Kind::Suv => (0.0, yb * 0.985),
        Kind::Liftback => (0.14, yb * 0.985),
        Kind::Sedan => (0.17 * l, yb * 0.98),
        Kind::Van => (0.0, h),
    };
    let zg = zr + deck;
    // sedan: tylna szyba stromsza niż w liftbacku (trzy bryły: maska, kabina, bagażnik)
    let rear_angle = if kind == Kind::Sedan { p.f("rearGlassAngle").max(40.0) } else { p.f("rearGlassAngle") };
    let mut run = if kind == Kind::Van { 0.0 } else { (h - ytail) / rear_angle.to_radians().tan() };
    if run < 0.06 {
        (run, ytail) = (0.0, h); // pionowy tył: szyba jako naklejka na zaślepce
    }
    // ponytail: skrajne nadpisania (kabina krótsza niż 0,5 m) są tylko przycinane, nie rozwiązywane
    let roof_r = (zg + run).min(roof_f - 0.5).max(zg);
    let roof_f = roof_f.max(roof_r + 0.2);
    let z_wt = z_wt.max(roof_f);
    let cowl = cowl.max(z_wt + 0.1);
    // dach z lekkim wybrzuszeniem (van: płaski), maska opada ostrzej tuż przed czołem
    let (crown_r, crown_f) = if kind == Kind::Van { (0.0, 0.0) } else { (0.045, 0.025) };
    let z_peak = roof_f - 0.45 * (roof_f - roof_r);
    let mut top = vec![(zr, ytail), (zg, ytail), (roof_r, h - crown_r), (z_peak, h), (roof_f, h - crown_f)];
    if z_wt > roof_f + 1e-6 {
        top.push((z_wt, y_wt));
    }
    // czoło pochylone (ok. 30° od pionu): krawędź maski `zn` na `ynose`, przód na `yf`; niżej pionowo
    let (zn, yf) = (zf - 0.13, ynose - 0.22);
    top.extend([(cowl, yh), (zn, ynose), (zf, yf)]);
    let t_at = |z: f64| pl(&top, z);
    // vany: szyby tylko w kabinie (słupek B ok. 1,2 m za podstawą szyby), dalej burta zabudowy
    let z_dw = if van { (cowl - 1.45).max(roof_r) } else { f64::NEG_INFINITY };
    // górna krawędź okien bocznych: osobówka pod fazką dachu, van na wysokości górnej krawędzi szyby
    let win_top = |tz: f64, cc: f64| if van { (tz - cc).min(y_wt) } else { tz - cc };

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
        let y4 = win_top(tz, cc);
        let bel = yb.min(y4 - 0.03);
        let sill = sill_at(z);
        let mid = mid_base.max(sill + 0.07).min(bel - 0.04);
        let sill = sill.min(mid - 0.02);
        // próg schowany pod bok (osobówki mocniej), bok pionowy od talii do linii okien
        let tuck = if kind == Kind::Van { 0.96 } else { 0.93 };
        [[0.0, clr, z], [hz * tuck, sill, z], [hz, mid, z], [hz, bel, z], [hr, y4, z], [hr - cc, tz, z], [0.0, tz, z]]
    };
    let full = |z: f64| -> Vec<V3> {
        let v = half(z);
        let mut ring = v.to_vec();
        ring.extend((1..=5).rev().map(|i| [-v[i][0], v[i][1], v[i][2]]));
        ring
    };

    // stacje: (z, priorytet przy scalaniu); max 12, bliższe niż 12 cm łączą się
    let mut st: Vec<(f64, u8)> = vec![(zr, 3), (zf, 3), (zn, 2), (cowl, 2), (roof_f, 2), (z_peak, 2), (roof_r, 2)];
    if van {
        st.extend([(z_dw, 2), (z_wt, 2)]);
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
        z if z > z_wt => Zone::Wind,
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

    details(mb, p, seed, &Ctx { kind, hw, h, clr, ynose, yf, zn, ytail, yb, zf, zr, cowl, roof_r, zg, z_dw, run }, &half, &taper);
    let hull = Hull { zs: &zs, half: &half, top: &t_at, van: kind == Kind::Van, h, yb, clr, ytail, zf, zn, zr, zg, cowl, roof_f, roof_r, z_dw };
    let lightbar = sluzba::apply(mb, p, seed, &hull);
    Layout { wheelbase: wb, track, stations: zs.len(), body_width: 2.0 * hw, lightbar }
}

struct Ctx {
    kind: Kind,
    hw: f64,
    h: f64,
    clr: f64,
    ynose: f64,
    /// Dolna krawędź pochylonego czoła (na `zf`) i krawędź maski (`zn`, na `ynose`).
    yf: f64,
    zn: f64,
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
    let Ctx { kind, hw, h, clr, ynose, yf, zn, ytail, yb, zf, zr, cowl, roof_r, zg, z_dw, run } = *c;
    // z czoła na wysokości y: pionowo do `yf`, wyżej pochylone do krawędzi maski
    let face_z = |y: f64| if y <= yf { zf } else { zf - (zf - zn) * ((y - yf) / (ynose - yf)).min(1.0) };
    let on_face = |pts: &[[f64; 2]], dz: f64| -> Vec<V3> { pts.iter().map(|&[x, y]| [x, y, face_z(y) + dz]).collect() };
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
    // zderzaki schodzą do spodu nadwozia (bez paska lakieru pod spodem)
    mb.bx(MatKey::Bumper, [-hn * 0.985, clr + 0.01, zf - 0.06], [hn * 0.985, clr + 0.08 + bh, zf + 0.04]);
    mb.bx(MatKey::Bumper, [-ht * 0.985, clr + 0.01, zr - 0.04], [ht * 0.985, clr + 0.08 + bh, zr + 0.06]);
    // tablice UE na zderzakach (przód przed dolnym wlotem)
    plate(mb, seed, 7.0, [0.0, clr + 0.08 + bh / 2.0, zf + 0.044], 1.0);
    plate(mb, seed, 7.0, [0.0, clr + 0.08 + bh / 2.0, zr - 0.04], -1.0);

    // grill między reflektorami: 0 = mały zamknięty panel, 1 = duży; dolny wlot w zderzaku
    let g = p.f("grill");
    let gw = lerp(0.2, 0.4, g) * hn;
    let gh = lerp(0.06, 0.18, g) * ((ynose - clr) / 0.7).clamp(1.0, 2.0);
    let gy = ynose - 0.05 - gh;
    // grill na czole: część pionowa i pochylona osobno (czoło ma załamanie na `yf`)
    let (ga, gb) = (gy, gy + gh);
    for (ya, yb2) in [(ga, gb.min(yf)), (ga.max(yf), gb)] {
        if yb2 - ya > 0.005 {
            mb.poly(MatKey::Plastic, &on_face(&[[-gw, ya], [gw, ya], [gw, yb2], [-gw, yb2]], 0.003), Some([0.0, 0.3, 1.0]));
        }
    }
    let iw = lerp(0.30, 0.50, g) * hn;
    mb.quad(MatKey::Plastic, [-iw, clr + 0.13, zf + 0.043], [iw, clr + 0.13, zf + 0.043], [iw, clr + 0.08 + bh - 0.05, zf + 0.043], [-iw, clr + 0.08 + bh - 0.05, zf + 0.043], Some([0.0, 0.0, 1.0]));

    // reflektory według `lampStyle` (osobówki węższe, zawinięte na błotnik)
    let (y0, y1) = if van { (ynose - 0.20, ynose - 0.045) } else { (ynose - 0.165, ynose - 0.05) };
    let ym = (y0 + y1) / 2.0;
    let zl = 0.003;
    // ciemna oprawa pod lampą, żeby biały reflektor nie ginął na białym lakierze
    let sx0 = gw + 0.03; // pasek zaczyna się za grillem
    let (hx0, hx1) = if p.s("lampStyle") == "strip" { (sx0, 0.96 * hn) } else { (0.39 * hn, 0.96 * hn) };
    let (hy0, hy1) = if p.s("lampStyle") == "strip" { (ym - 0.05, ym + 0.04) } else { (y0 - 0.015, y1 + 0.015) };
    let fwd = [0.0, 0.3, 1.0];
    mirror(mb, MatKey::Plastic, &on_face(&[[hx0, hy0], [hx1, hy0], [hx1, hy1], [hx0, hy1]], 0.002), fwd);
    let lamp: Vec<[f64; 2]> = match p.s("lampStyle") {
        "oval" => vec![[0.44 * hn, y0 + 0.01], [0.86 * hn, y0 + 0.01], [0.94 * hn, ym], [0.86 * hn, y1], [0.44 * hn, y1], [0.40 * hn, ym]],
        "strip" => vec![[sx0 + 0.01, ym - 0.02], [0.95 * hn, ym - 0.04], [0.95 * hn, ym + 0.025], [sx0 + 0.01, ym + 0.025]],
        _ => vec![[0.46 * hn, y0 + 0.05], [0.95 * hn, y0 + 0.01], [0.95 * hn, y1], [0.40 * hn, y1]],
    };
    mirror(mb, MatKey::Headlight, &on_face(&lamp, zl), fwd);
    // zawinięcie lampy na bok nadwozia (płaszczyzna boku w z: `hw * taper(z)`)
    let side_x = |z: f64| hw * taper(z) + 0.003;
    let wrap = |mb: &mut MeshBuilder, mat: MatKey, (za, zb): (f64, f64), (zc, zd): (f64, f64), (ya, yb2): (f64, f64)| {
        mirror(mb, mat, &[[side_x(za), ya, za], [side_x(zb), ya, zb], [side_x(zd), yb2, zd], [side_x(zc), yb2, zc]], [1.0, 0.0, 0.0]);
    };
    if !van {
        // przednia krawędź zawinięcia idzie po pochylonym czole
        let (ya, yb2) = if p.s("lampStyle") == "strip" { (ym - 0.03, ym + 0.025) } else { (ym - 0.02, y1) };
        let (fa, fb) = (face_z(ya) - 0.01, face_z(yb2) - 0.01);
        wrap(mb, MatKey::Headlight, (fa - 0.13, fa), (fb - 0.13, fb), (ya, yb2));
    }
    // kierunkowskaz przedni: zewnętrzny dolny róg oprawy (pasek LED: koniec paska)
    let (ix0, iy0, iy1) = if p.s("lampStyle") == "strip" { (0.84 * hn, ym - 0.04, ym + 0.025) } else { (0.78 * hn, y0 - 0.012, y0 + 0.035) };
    mirror(mb, MatKey::Indicator, &on_face(&[[ix0, iy0], [0.955 * hn, iy0], [0.955 * hn, iy1], [ix0, iy1]], 0.004), fwd);

    // tył: światła (w vanie pionowe pasy w narożach), pasek między nimi, szyba na pionowym tyle
    let zt = zr - 0.003;
    let (ly0, ly1, lx0, lx1) = if van { (clr + 0.45, clr + 1.05, 0.86 * ht, 0.985 * ht) } else { (ytail - 0.17, ytail - 0.035, 0.5 * ht, 0.97 * ht) };
    mirror(mb, MatKey::Taillight, &[[lx1, ly0, zt], [lx0, ly0, zt], [lx0, ly1, zt], [lx1, ly1, zt]], [0.0, 0.0, -1.0]);
    if !van {
        wrap(mb, MatKey::Taillight, (zr + 0.015, zr + 0.17), (zr + 0.015, zr + 0.17), (ly0 + 0.04, ly1));
    }
    // kierunkowskaz tylny: w vanie środkowy segment pionowej lampy, w osobówce dolna zewnętrzna część
    let (ia, ib, ja, jb) = if van { (lx0, lx1, lerp(ly0, ly1, 0.4), lerp(ly0, ly1, 0.62)) } else { (lerp(lx0, lx1, 0.5), lx1, ly0, lerp(ly0, ly1, 0.4)) };
    mirror(mb, MatKey::Indicator, &[[ib, ja, zt - 0.001], [ia, ja, zt - 0.001], [ia, jb, zt - 0.001], [ib, jb, zt - 0.001]], [0.0, 0.0, -1.0]);
    if roll(seed, 4.0, p.f("rearLightBar")) && !van {
        mb.quad(MatKey::Taillight, [-lx0, ly1 - 0.05, zt], [-lx0, ly1 - 0.01, zt], [lx0, ly1 - 0.01, zt], [lx0, ly1 - 0.05, zt], Some([0.0, 0.0, -1.0]));
    }
    if run == 0.0 {
        let (wy0, wy1) = (if van { h * 0.6 } else { yb + 0.04 }, h - 0.22);
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
    let seams = if van { vec![za, zc] } else { vec![za, zb, zc] };
    for z in seams {
        strip(mb, MatKey::Plastic, half, (z - d, z + d), (2, seam_top), 0.003);
    }
    let handle = |mb: &mut MeshBuilder, z: f64, sides: &[f64]| {
        let bel = half(z)[3];
        strip_quad_on(mb, MatKey::Plastic, bel[0] + 0.005, bel[1] - 0.09, bel[1] - 0.065, z - 0.07, z + 0.07, sides);
    };
    if van {
        handle(mb, zc + 0.14, &BOTH);
        // drzwi przesuwne tylko po prawej (−X): szwy, klamka, szyna dolna
        for z in [z_dw - 0.12, z_dw - 1.37] {
            strip_on(mb, MatKey::Plastic, half, (z - d, z + d), (2, seam_top), 0.003, &RIGHT);
        }
        handle(mb, z_dw - 1.22, &RIGHT);
        strip_quad_on(mb, MatKey::Plastic, half(z_dw - 0.7)[2][0] + 0.004, half(z_dw)[2][1] + 0.02, half(z_dw)[2][1] + 0.05, z_dw - 1.37, z_dw - 0.12, &RIGHT);
    } else {
        strip(mb, MatKey::Paint, half, (zb - 0.04, zb + 0.04), (3, 4), 0.004);
        handle(mb, zb - 0.14, &BOTH);
        handle(mb, zc + 0.12, &BOTH);
    }

    if van {
        van_extras(mb, c, half, taper);
    }

    // wycieraczki: dwa ramiona na dole szyby, tuż nad podstawą (`cowl`)
    let ws = |dz: f64| {
        let z = cowl - dz;
        (z, half(z)[6][1] + 0.008)
    };
    let ((za0, ya0), (zb0, yb0)) = (ws(0.04), ws(0.08));
    let hwz = half(cowl - 0.06)[5][0];
    for x0 in [-0.9 * hwz, 0.04] {
        let x1 = x0 + 0.82 * hwz;
        mb.quad(MatKey::Plastic, [x0, ya0, za0], [x1, ya0, za0], [x1, yb0, zb0], [x0, yb0, zb0], Some([0.0, 1.0, 0.4]));
    }

    // lusterka na ramieniu u podstawy słupka A (osobówka ok. 0,14 m poza burtę, van 0,2 m),
    // na obudowie boczny powtarzacz kierunkowskazu
    let m = half(za - 0.08)[3];
    let (out, hgt, dep, lift) = if van { (0.2, 0.3, 0.1, 0.02) } else { (0.14, 0.11, 0.09, 0.03) };
    let (zm0, zm1) = (za - 0.08 - dep, za - 0.08);
    for side in [1.0f64, -1.0] {
        let sx = |a: f64, b: f64| ((m[0] + a) * side, (m[0] + b) * side);
        let (a, b) = sx(-0.01, out * 0.4);
        mb.bx(MatKey::Plastic, [a.min(b), m[1] + lift + 0.02, zm0 + 0.02], [a.max(b), m[1] + lift + 0.05, zm1 - 0.02]);
        let (a, b) = sx(out * 0.35, out);
        mb.bx(MatKey::Plastic, [a.min(b), m[1] + lift, zm0], [a.max(b), m[1] + lift + hgt, zm1]);
        let xo = (m[0] + out + 0.001) * side;
        mb.quad(MatKey::Indicator, [xo, m[1] + lift + 0.01, zm0 + 0.01], [xo, m[1] + lift + 0.01, zm1 - 0.005], [xo, m[1] + lift + 0.035, zm1 - 0.005], [xo, m[1] + lift + 0.035, zm0 + 0.01], Some([side, 0.0, 0.0]));
    }
}

/// Pionowy prostokąt przy burcie (x stałe, y0..y1, z0..z1) po stronach `sides` (+1 lewa, −1 prawa).
fn strip_quad_on(mb: &mut MeshBuilder, mat: MatKey, x: f64, y0: f64, y1: f64, z0: f64, z1: f64, sides: &[f64]) {
    for &side in sides {
        mb.quad(mat, [x * side, y0, z0], [x * side, y0, z1], [x * side, y1, z1], [x * side, y1, z0], Some([side, 0.0, 0.0]));
    }
}

/// Zabudowa vana: żebra dachu, listwy ochronne, szyny drzwi przesuwnych, tylne drzwi.
fn van_extras(mb: &mut MeshBuilder, c: &Ctx, half: &dyn Fn(f64) -> Half, taper: &dyn Fn(f64) -> f64) {
    let Ctx { hw, h, clr, zr, z_dw, .. } = *c;
    let ht = hw * taper(zr);
    // przetłoczenia dachu (3, w kolorze lakieru)
    for i in 0..3 {
        let z = lerp(z_dw - 0.3, zr + 0.6, i as f64 / 2.0);
        let hr = half(z)[5][0] - 0.04;
        mb.bx(MatKey::Paint, [-hr, h - 0.01, z - 0.04], [hr, h + 0.012, z + 0.04]);
    }
    let wall = |z: f64| half(z)[2][0];
    for side in [1.0, -1.0] {
        let sbx = |mb: &mut MeshBuilder, x: f64, y0: f64, y1: f64, z0: f64, z1: f64, out: f64| {
            let (a, b) = (x * side, (x + out) * side);
            mb.bx(MatKey::Plastic, [a.min(b), y0, z0], [a.max(b), y1, z1]);
        };
        // listwa ochronna: za drzwiami przesuwnymi (prawa), na całej burcie ładowni (lewa)
        let (z0, z1) = (zr + 0.25, if side < 0.0 { z_dw - 1.42 } else { z_dw - 0.1 });
        sbx(mb, wall((z0 + z1) / 2.0) - 0.01, clr + 0.62, clr + 0.7, z0, z1, 0.025);
        if side < 0.0 {
            // szyna górna drzwi przesuwnych (za szybą kabiny)
            let (s0, s1) = (z_dw - 1.37, z_dw - 0.12);
            let y = h * 0.6;
            sbx(mb, wall((s0 + s1) / 2.0) - 0.01, y, y + 0.04, s0, s1, 0.025);
        }
        // zawiasy i klamka tylnych drzwi
        for y in [clr + 0.5, h - 0.45] {
            mb.bx(MatKey::Plastic, [(ht - 0.04) * side - 0.04, y, zr - 0.03], [(ht - 0.04) * side + 0.04, y + 0.14, zr + 0.01]);
        }
        mb.bx(MatKey::Plastic, [0.05 * side - 0.03, 1.0, zr - 0.03], [0.05 * side + 0.03, 1.08, zr + 0.01]);
    }
    // stopień nad zderzakiem
    mb.bx(MatKey::Plastic, [-0.55 * ht, clr + 0.4, zr - 0.07], [0.55 * ht, clr + 0.43, zr]);
}
