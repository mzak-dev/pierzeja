//! Nadwozie osobówek i vana: reguła „ground” (osie) → zwisy → loft ze stacji wzdłuż Z.
//! Każda stacja to półprzekrój (7 wierzchołków burty i dachu + 2 wnęki koła); loft buduje lewą
//! połowę, prawa jest jej odbiciem trójkąt po trójkącie, więc nadwozie jest dokładnie lustrzane.
//! Profil boczny (maska, szyba, dach, tył), nadkola i linia okien to funkcje z, próbkowane w stacjach.

use crate::sluzba::{self, Hull};
use crate::tablice::plate;
use crate::{roll, P};
use procedura_kit::mesh::{cross, len, sub, MatKey, MeshBuilder, MeshData, V3};

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
    // nadkole: pół-ośmiokąt opisany na łuku o promieniu `ra`; pół-szerokości u podstawy, w połowie i u góry
    let (a, am, at) = (ra * 1.12, ra * 0.9, ra * 0.55);
    let (archtop, archmid) = (r + ra, r + 0.44 * ra);

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
    // osobówka: maska prawie płaska, wysokie czoło (współczesne auta); van: krótka maska stromo
    // opada do wysokiego grilla (Sprinter: ok. 1,35 m przy szybie, 1,05 m nad grillem)
    let ynose = (yh - if kind == Kind::Van { 0.22 } else { 0.05 + 0.03 * round }).max(clr + 0.45);
    let sb = clr + 0.07; // dolna krawędź boku poza nadkolami
    let mid_base = sb + 0.20; // talia (górna krawędź okładziny), w nadkolach wyżej
    let tumble = p.f("tumblehome");

    // profil boczny: maska → szyba → dach → tylna szyba → pokrywa. Van (Sprinter, Transit):
    // podstawa szyby prawie nad przednią osią, szyba kończy się 0,3 m pod dachem, nad nią daszek.
    let van = kind == Kind::Van;
    let cowl = if van { zaf - 0.08 } else { zaf - a }.min(zf - 0.45);
    let ws_tan = p.f("windshieldAngle").to_radians().tan();
    // van: szyba ma ok. 0,8 m wysokości; w wysokim dachu (H2/H3) nad nią jest czapa w kolorze nadwozia
    let y_wt = if van { (yh + 0.8).min(h - 0.3) } else { h };
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
            if d <= at {
                archtop
            } else if d < am {
                lerp(archtop, archmid, (d - at) / (am - at))
            } else if d < a {
                lerp(archmid.max(sb), sb, (d - am) / (a - am))
            } else {
                sb
            }
        };
        arch(zaf).max(arch(zar))
    };
    // plan: pełna szerokość między nadkolami, lekkie zwężenie do 13 cm przed końcem, dalej ścięty
    // narożnik (im większe `roundness`, tym mocniej) — z góry auto nie jest prostokątem
    // van ma tępy, szeroki przód (zwężenie 6%), osobówka wyraźnie ścięte narożniki
    let (nmid, nfac) = if van { (0.985, 0.94) } else { (0.96 - 0.05 * round, 0.88 - 0.12 * round) };
    // van: tylne drzwi na pełną szerokość, więc tył prawie prosty
    let (tmid, tfac) = if van { (0.99, 0.97) } else { (0.97 - 0.04 * round, 0.91 - 0.10 * round) };
    let zrn = (zr + 0.13).min(zar - a - 0.01);
    let plan_f = [(zaf + a, 1.0), (zn.max(zaf + a + 0.01), nmid), (zf, nfac)];
    let plan_r = [(zr, tfac), (zrn, tmid), (zar - a, 1.0)];
    let taper = |z: f64| {
        if z > zaf + a {
            pl(&plan_f, z)
        } else if z < zar - a {
            pl(&plan_r, z)
        } else {
            1.0
        }
    };
    // szyby boczne: z przodu od 30% długości słupka A (niżej blacha), z tyłu do końca dachu
    // (hatchback, kombi, SUV: słupek D lakierowany) albo 40% w głąb spadku (sedan, liftback: słupek C)
    let z_sa = cowl - 0.3 * (cowl - z_wt);
    let z_sg = if matches!(kind, Kind::Liftback | Kind::Sedan) { roof_r - 0.4 * (roof_r - zg) } else { roof_r };

    // półprzekrój w z: dno, próg, talia, linia okien, róg szyby, krawędź dachu, oś dachu
    let half = |z: f64| -> Half {
        let tz = t_at(z);
        let hz = hw * taper(z);
        let g = ((tz - yb) / (h - yb)).clamp(0.0, 1.0);
        // bok najszerszy w talii, pochylony do linii okien (osobówka 4%, van 1,5%), wyżej tumblehome
        let hb = hz * if kind == Kind::Van { 0.985 } else { 0.96 };
        let hr = hb * (1.0 - tumble * g);
        let cc = c.min(0.3 * hr);
        let y4 = win_top(tz, cc);
        let bel = yb.min(y4 - 0.03);
        let sill = sill_at(z);
        let mid = mid_base.max(sill + 0.07).min(bel - 0.04);
        let sill = sill.min(mid - 0.02);
        // próg schowany pod bok (osobówki mocniej)
        let tuck = if kind == Kind::Van { 0.96 } else { 0.93 };
        [[0.0, clr, z], [hz * tuck, sill, z], [hz, mid, z], [hb, bel, z], [hr, y4, z], [hr - cc, tz, z], [0.0, tz, z]]
    };
    // przekrój lofta: płaskie dno do wewnętrznej ściany wnęki koła, strop wnęki, dalej półprzekrój
    let xi = (track / 2.0 - tw / 2.0 - 0.03).min(0.7 * hw);
    let sect = |z: f64| -> [V3; 9] {
        let v = half(z);
        let yw = if v[1][1] > sb + 1e-6 { v[1][1] } else { clr };
        [v[0], [xi, clr, z], [xi, yw, z], v[1], v[2], v[3], v[4], v[5], v[6]]
    };

    // stacje: (z, priorytet): 4 = końce, 3 = nadkola (nie łączą się ze sobą), 2 = profil.
    // Stacja profilu bliżej niż 12 cm od sąsiada wpada w niego (wygrywa wyższy priorytet).
    let mut st: Vec<(f64, u8)> = vec![(zr, 4), (zf, 4), (zn, 2), (cowl, 2), (roof_f, 2), (z_peak, 2), (roof_r, 2)];
    st.push((zrn, 2));
    if van {
        st.extend([(z_dw, 2), (z_wt, 2)]);
    } else {
        st.extend([(z_sa, 2), (z_sg, 2)]);
    }
    for za in [zaf, zar] {
        for d in [a, am, at] {
            st.extend([(za - d, 3), (za + d, 3)]);
        }
    }
    st.retain(|&(z, pr)| pr == 4 || (z > zr + 0.02 && z < zf - 0.02));
    st.sort_by(|x, y| x.0.total_cmp(&y.0));
    while let Some(i) = (0..st.len() - 1)
        .filter(|&i| st[i].1.min(st[i + 1].1) < 3 && st[i + 1].0 - st[i].0 < 0.12)
        .min_by(|&i, &j| (st[i + 1].0 - st[i].0).total_cmp(&(st[j + 1].0 - st[j].0)))
    {
        let (x, y) = (st[i], st[i + 1]);
        let z = if x.1 == y.1 { (x.0 + y.0) / 2.0 } else if x.1 > y.1 { x.0 } else { y.0 };
        st[i] = (z, x.1.max(y.1));
        st.remove(i + 1);
    }
    let zs: Vec<f64> = st.iter().map(|s| s.0).collect();
    let secs: Vec<[V3; 9]> = zs.iter().map(|&z| sect(z)).collect();

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
    // loft lewej połowy; prawa to odbicie trójkąt po trójkącie, więc nadwozie jest dokładnie lustrzane
    let flip = |v: V3| [-v[0], v[1], v[2]];
    for (i, w) in secs.windows(2).enumerate() {
        let zm = (zs[i] + zs[i + 1]) / 2.0;
        let zn = zone(zm);
        let cabin = zn != Zone::Hood && zn != Zone::Deck;
        let side_glass = if van { zn == Zone::Wind || (zn == Zone::Roof && zm > z_dw) } else { cabin && zm < z_sa && zm > z_sg };
        // segment 0 (dno) to jeden czworokąt niżej; 1–2 to wnęka koła, dalej jak w półprzekroju
        for s in 1..8usize {
            let mat = match s.saturating_sub(2) {
                0 => MatKey::Plastic,
                1 if cladding => MatKey::Plastic,
                3 if side_glass => MatKey::Glass,
                4 if cabin => roof_mat,
                5 if matches!(zn, Zone::Wind | Zone::RearGlass) => MatKey::Glass,
                5 if zn == Zone::Roof => roof_mat,
                _ => MatKey::Paint,
            };
            let (a, b, c, d) = (w[0][s], w[0][s + 1], w[1][s + 1], w[1][s]);
            for t in [[a, b, c], [a, c, d]] {
                if len(cross(sub(t[1], t[0]), sub(t[2], t[0]))) > 1e-9 {
                    mb.poly(mat, &t, None);
                    mb.poly(mat, &[flip(t[0]), flip(t[2]), flip(t[1])], None);
                }
            }
        }
    }
    mb.quad(MatKey::Plastic, [-xi, clr, zr], [xi, clr, zr], [xi, clr, zf], [-xi, clr, zf], Some([0.0, -1.0, 0.0]));
    // zaślepki czoła i tyłu (bez zdublowanego punktu wnęki)
    for (sec, dir) in [(&secs[0], -1.0), (&secs[secs.len() - 1], 1.0)] {
        let l: Vec<V3> = sec.iter().enumerate().filter(|(k, _)| *k != 2).map(|(_, v)| *v).collect();
        let ring: Vec<V3> = l.iter().copied().chain(l[1..l.len() - 1].iter().rev().map(|&v| flip(v))).collect();
        mb.poly(MatKey::Paint, &ring, Some([0.0, 0.0, dir]));
    }

    details(mb, p, seed, &Ctx { kind, hw, h, clr, ynose, yf, zn, ytail, yb, zf, zr, cowl, z_dw, run, z_sg, zar }, &half, &taper);
    let hull = Hull { zs: &zs, half: &half, top: &t_at, van: kind == Kind::Van, h, yb, clr, ytail, zf, zn, zr, zg, cowl, roof_f, roof_r, z_dw, surf: None };
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
    z_dw: f64,
    run: f64,
    /// Tylna granica szyb bocznych i tylna oś (szew tylnych drzwi nad nadkolem).
    z_sg: f64,
    zar: f64,
}

/// Zderzaki, grill, światła, szwy drzwi, klamki, słupek B, lusterka; van: zabudowa i tylne drzwi.
fn details(mb: &mut MeshBuilder, p: &P, seed: u32, c: &Ctx, half: &dyn Fn(f64) -> Half, taper: &dyn Fn(f64) -> f64) {
    let Ctx { kind, hw, h, clr, ynose, yf, zn, ytail, yb, zf, zr, cowl, z_dw, run, z_sg, zar, .. } = *c;
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
    // zderzaki schodzą do spodu nadwozia (bez paska lakieru pod spodem); w planie ścięte narożniki,
    // nie szersze niż nadwozie przy progu
    let (ya, yb2) = (clr + 0.01, clr + 0.08 + bh);
    let (wf, wr) = (hn * if van { 0.985 } else { 0.95 }, ht * if van { 0.985 } else { 0.95 });
    slab(mb, MatKey::Bumper, &[[-wf, zf - 0.06], [wf, zf - 0.06], [wf, zf], [wf - 0.1, zf + 0.04], [-wf + 0.1, zf + 0.04], [-wf, zf]], ya, yb2);
    slab(mb, MatKey::Bumper, &[[-wr, zr], [-wr + 0.1, zr - 0.04], [wr - 0.1, zr - 0.04], [wr, zr], [wr, zr + 0.06], [-wr, zr + 0.06]], ya, yb2);
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
    let hn = hw * taper(face_z(ym)); // szerokość nadwozia tam, gdzie leżą lampy (czoło zwęża się ku przodowi)
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
    let side_x = |z: f64, y: f64| side_at(&half(z), y) + 0.003;
    let wrap = |mb: &mut MeshBuilder, mat: MatKey, (za, zb): (f64, f64), (zc, zd): (f64, f64), (ya, yb2): (f64, f64)| {
        // dwa trójkąty, nie czworokąt: pochylony bok daje czworokąt niepłaski, a `poly` odwraca kolejność
        // wierzchołków po prawej stronie, więc przekątna wypadałaby inaczej niż po lewej
        let q = [[side_x(za, ya), ya, za], [side_x(zb, ya), ya, zb], [side_x(zd, yb2), yb2, zd], [side_x(zc, yb2), yb2, zc]];
        mirror(mb, mat, &[q[0], q[1], q[2]], [1.0, 0.0, 0.0]);
        mirror(mb, mat, &[q[0], q[2], q[3]], [1.0, 0.0, 0.0]);
    };
    // przód: bez zawinięcia na błotnik — ścięty narożnik sam odsłania lampę z boku
    // kierunkowskaz przedni: zewnętrzny dolny róg oprawy (pasek LED: koniec paska)
    let (ix0, iy0, iy1) = if p.s("lampStyle") == "strip" { (0.84 * hn, ym - 0.04, ym + 0.025) } else { (0.78 * hn, y0 - 0.012, y0 + 0.035) };
    mirror(mb, MatKey::Indicator, &on_face(&[[ix0, iy0], [0.955 * hn, iy0], [0.955 * hn, iy1], [ix0, iy1]], 0.004), fwd);

    // tył: światła (w vanie pionowe pasy w narożach), pasek między nimi, szyba na pionowym tyle
    let zt = zr - 0.003;
    let (ly0, ly1, lx0, lx1) = if van { (clr + 0.45, clr + 1.05, 0.86 * ht, 0.985 * ht) } else { (ytail - 0.17, ytail - 0.035, 0.5 * ht, 0.97 * ht) };
    // ciemna oprawa lamp tylnych (czytelne także na czerwonym lakierze)
    let o = 0.015;
    mirror(mb, MatKey::Plastic, &[[lx1 + o, ly0 - o, zt + 0.001], [lx0 - o, ly0 - o, zt + 0.001], [lx0 - o, ly1 + o, zt + 0.001], [lx1 + o, ly1 + o, zt + 0.001]], [0.0, 0.0, -1.0]);
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
    let zc = if van { z_dw } else { (zar + 0.15).max(z_sg) };
    let zb = (za + zc) / 2.0 + 0.05;
    let d = 0.007;
    let seam_top = if van { 4 } else { 3 };
    let seams = if van { vec![za, zc] } else { vec![za, zb, zc] };
    for z in seams {
        strip(mb, MatKey::Plastic, half, (z - d, z + d), (2, seam_top), 0.003);
    }
    let handle = |mb: &mut MeshBuilder, z: f64, sides: &[f64]| {
        let v = half(z);
        let (y0, y1) = (v[3][1] - 0.09, v[3][1] - 0.065);
        strip_quad_on(mb, MatKey::Plastic, side_at(&v, y0) + 0.005, y0, y1, z - 0.07, z + 0.07, sides);
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
        // słupek C, gdy za tylnymi drzwiami jest jeszcze okno
        if zc - z_sg > 0.25 {
            strip(mb, MatKey::Paint, half, (zc - 0.08, zc + 0.01), (3, 4), 0.004);
        }
        // klamki przy tylnej krawędzi każdych drzwi
        handle(mb, zb + 0.14, &BOTH);
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

/// Szerokość burty na wysokości `y` (bok jest pochylony między talią a linią okien).
fn side_at(v: &Half, y: f64) -> f64 {
    lerp(v[2][0], v[3][0], ((y - v[2][1]) / (v[3][1] - v[2][1]).max(1e-6)).clamp(0.0, 1.0))
}

/// Bryła pionowa: wypukły wielokąt w planie (x, z), od `y0` do `y1`.
fn slab(mb: &mut MeshBuilder, mat: MatKey, pts: &[[f64; 2]], y0: f64, y1: f64) {
    let at = |y: f64| -> Vec<V3> { pts.iter().map(|&[x, z]| [x, y, z]).collect() };
    mb.poly(mat, &at(y1), Some([0.0, 1.0, 0.0])); // spód niewidoczny — pominięty
    let _ = y0;
    let n = pts.len();
    let (cx, cz) = pts.iter().fold((0.0, 0.0), |a, p| (a.0 + p[0] / n as f64, a.1 + p[1] / n as f64));
    for k in 0..n {
        let ([xa, za], [xb, zb]) = (pts[k], pts[(k + 1) % n]);
        let out = [(xa + xb) / 2.0 - cx, 0.0, (za + zb) / 2.0 - cz];
        mb.quad(mat, [xa, y0, za], [xb, y0, zb], [xb, y1, zb], [xa, y1, za], Some(out));
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
