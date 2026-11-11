//! Pojazdy ciężkie (`bodyType = "podwozie"`): kabina nad silnikiem (1+1, 1+2, 1+1+4 według liczby
//! Miejsc), rama z podniesionym tyłem (kąt zejścia 23°), osie 4x2/4x4 i wspólne klocki (belka,
//! walec, graniastosłup). Moduły zabudowy na ramie leżą w `zabudowa.rs`.
//! Konwencja jak w `body.rs`: przód +Z, lewa strona +X, origin = środek między osiami na gruncie.

use crate::Layout;
use crate::tablice::plate;
use crate::{roll, P};
use procedura_kit::mesh::{add, cross, len, norm, scale, sub, MatKey, MaterialSpec, MeshBuilder, V3};

/// Kąt najazdu i zejścia z przetargów PSP.
const ANGLE: f64 = 23.0;

pub(crate) fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Cab {
    /// 1+1: krótka kabina dzienna.
    Short,
    /// 1+2: kabina dzienna z ławką.
    Day,
    /// 1+1+4 (i warianty): kabina brygadowa z jednym lub dwoma rzędami z tyłu.
    Crew,
}

/// Wymiary wspólne dla kabiny, ramy i zabudowy.
pub(crate) struct D {
    pub w: f64,
    pub h: f64,
    pub clr: f64,
    pub r: f64,
    pub wb: f64,
    pub track: f64,
    pub zaf: f64,
    pub zar: f64,
    /// Przód zderzaka i tył pojazdu.
    pub zf: f64,
    pub zr: f64,
    /// Górna krawędź podłużnic.
    pub y_fr: f64,
    pub archtop: f64,
    pub arch_a: f64,
    pub arch_at: f64,
    pub cab: Cab,
    pub seats: usize,
    /// Rzędy tylne kabiny brygadowej.
    pub rows: usize,
    /// Czoło i tył kabiny.
    pub zc: f64,
    pub zb: f64,
    pub cab_top: f64,
    pub ycb: f64,
    /// Półmaska przed kabiną (Daily); 0 = kabina nad silnikiem.
    pub nose: f64,
    /// Osie przednie i tylne (z); pierwsza przednia = `zaf`, pierwsza tylna = `zar`.
    pub front_axles: Vec<f64>,
    pub rear_axles: Vec<f64>,
    /// Ostatnia oś tylna (kąt zejścia, zwis tylny).
    pub zlr: f64,
    /// Bliźniacze koła tylne.
    pub dual: bool,
    pub tw: f64,
    /// Ciągnik siodłowy: wysokość siodła (m) i jego położenie (z).
    pub fifth_wheel: Option<(f64, f64)>,
}

impl D {
    pub fn new(p: &P) -> D {
        let (l, w, h, wb) = (p.f("length"), p.f("width"), p.f("height"), p.f("wheelbase"));
        let (clr, r, tw) = (p.f("clearance"), p.f("wheelDiameter") / 2.0, p.f("tyreWidth"));
        let (fo, ro) = (p.f("frontOverhang"), p.f("rearOverhang"));
        // osie: 8x4 ma dwie przednie (druga 1,95 m za pierwszą, w rozstawie), reszta tylne co `axleGap`
        let axles = (p.f("axles").round() as usize).clamp(2, 4);
        let front_n = if p.s("drive") == "8x4" && axles >= 3 { 2 } else { 1 };
        let rear_n = axles - front_n;
        let gap = p.f("axleGap");
        let oh = (l - wb - gap * (rear_n - 1) as f64).max(0.8);
        let (zaf, zar) = (wb / 2.0, -wb / 2.0);
        let zlr = zar - gap * (rear_n - 1) as f64;
        let (zf, zr) = (zaf + oh * fo / (fo + ro), zlr - oh * ro / (fo + ro));
        let front_axles: Vec<f64> = (0..front_n).map(|i| zaf - 1.95 * i as f64).collect();
        let rear_axles: Vec<f64> = (0..rear_n).map(|i| zar - gap * i as f64).collect();
        let dual = p.s("rearTyres") == "dual";
        // siodło 0,5 m przed tylną osią (środkiem tandemu), wysokość ok. 1,15 m
        let fifth_wheel = (p.s("module") == "siodlo").then(|| (p.0.get("couplingHeight").and_then(|v| v.as_f64()).unwrap_or(1.15), (zar + zlr) / 2.0 + 0.5));
        let ra = r + p.f("wheelArchGap");
        let seats = p.f("seats") as usize;
        let rows = if seats > 3 { ((seats - 2).div_ceil(2)).min(2) } else { 0 };
        let cab = match seats {
            0..=2 => Cab::Short,
            3 => Cab::Day,
            _ => Cab::Crew,
        };
        // długość kabiny: jawna (`cabLength`), sypialna 2,28 m (Scania R/S), inaczej z liczby Miejsc
        let cab_len = match (p.f("cabLength"), p.s("cabType")) {
            (l, _) if l > 0.5 => l,
            (_, "sleeper") => 2.28,
            _ => match cab {
                Cab::Short => 1.6,
                Cab::Day => 1.83,
                Cab::Crew => 1.83 + 0.72 * rows as f64,
            },
        };
        let y_fr = r + 0.36;
        let zc = zf - 0.14;
        let nose = p.0.get("cabNose").and_then(|v| v.as_f64()).unwrap_or(0.0);
        D {
            w,
            h,
            clr,
            r,
            wb,
            track: (p.f("trackRatio") * w).min(w - 0.03 - tw),
            zaf,
            zar,
            zf,
            zr,
            y_fr,
            archtop: r + ra,
            arch_a: ra * 1.12,
            arch_at: ra * 0.55,
            cab,
            seats,
            rows,
            zc,
            zb: zc - cab_len - nose,
            cab_top: p.f("cabHeight").min(h),
            // półmaska: kabina niżej, podłoga tuż nad ramą
            ycb: if nose > 0.05 { y_fr - 0.05 } else { y_fr - 0.25 },
            nose,
            front_axles,
            rear_axles,
            zlr,
            dual,
            tw,
            fifth_wheel,
        }
        .with_cab_floor()
    }

    /// Spód kabiny nie schodzi pod linię najazdu przy czole.
    fn with_cab_floor(mut self) -> D {
        self.ycb = self.ycb.max(self.app(self.zc) + 0.02);
        self
    }

    fn off() -> f64 {
        // styczna do okręgu opony pod kątem ANGLE: przesunięcie linii względem punktu styku
        -(1.0 / ANGLE.to_radians().cos() - 1.0)
    }

    /// Najniższa dozwolona wysokość za ostatnią osią tylną (kąt zejścia), 0 przed nią.
    pub fn dep(&self, z: f64) -> f64 {
        if z < self.zlr { (ANGLE.to_radians().tan() * (self.zlr - z) + Self::off() * self.r).max(0.0) } else { 0.0 }
    }

    /// Obrys nadkoli wszystkich osi tylnych (wysokość dolnej krawędzi boku zabudowy).
    pub fn arch_r(&self, z: f64) -> f64 {
        self.rear_axles.iter().map(|&za| self.arch(z, za)).fold(0.0, f64::max)
    }

    /// Piasty kół: osie przednie `Wheel_F*`, tylne `Wheel_R*`; bliźniak wewnętrzny z sufiksem `i`.
    pub fn wheels(&self) -> Vec<(String, V3)> {
        let mut out = Vec::new();
        let name = |front: bool, i: usize| format!("{}{}", if front { "F" } else { "R" }, if i == 0 { String::new() } else { (i + 1).to_string() });
        for (i, &z) in self.front_axles.iter().enumerate() {
            let x = self.track / 2.0;
            out.push((format!("Wheel_{}L", name(true, i)), [x, self.r, z]));
            out.push((format!("Wheel_{}R", name(true, i)), [-x, self.r, z]));
        }
        for (i, &z) in self.rear_axles.iter().enumerate() {
            let n = name(false, i);
            if self.dual {
                // bliźniaki: zewnętrzne lico przy szerokości całkowitej, wewnętrzne 3 cm obok
                let xo = self.w / 2.0 - 0.02 - self.tw / 2.0;
                let xi = xo - self.tw - 0.03;
                out.extend([(format!("Wheel_{n}L"), [xo, self.r, z]), (format!("Wheel_{n}R"), [-xo, self.r, z]), (format!("Wheel_{n}Li"), [xi, self.r, z]), (format!("Wheel_{n}Ri"), [-xi, self.r, z])]);
            } else {
                let x = self.track / 2.0;
                out.extend([(format!("Wheel_{n}L"), [x, self.r, z]), (format!("Wheel_{n}R"), [-x, self.r, z])]);
            }
        }
        // kolejność węzłów: FL, FR, RL, RR (kontrakt), potem pozostałe
        let rank = |n: &str| ["Wheel_FL", "Wheel_FR", "Wheel_RL", "Wheel_RR"].iter().position(|k| *k == n).unwrap_or(4);
        out.sort_by_key(|(n, _)| rank(n));
        out
    }

    /// Kotwice: siodło ciągnika.
    pub fn anchors(&self) -> Vec<(String, V3)> {
        self.fifth_wheel.map(|(h, z)| vec![("Anchor_FifthWheel".to_string(), [0.0, h, z])]).unwrap_or_default()
    }

    /// Jak `dep`, ale przed przednią osią (kąt najazdu).
    pub fn app(&self, z: f64) -> f64 {
        if z > self.zaf { (ANGLE.to_radians().tan() * (z - self.zaf) + Self::off() * self.r).max(0.0) } else { 0.0 }
    }

    /// Obrys nadkola (trapez) wokół osi `za`: wysokość dolnej krawędzi boku.
    pub fn arch(&self, z: f64, za: f64) -> f64 {
        let dz = (z - za).abs();
        if dz <= self.arch_at {
            self.archtop
        } else if dz < self.arch_a {
            lerp(self.archtop, 0.0, (dz - self.arch_at) / (self.arch_a - self.arch_at))
        } else {
            0.0
        }
    }

    /// Współrzędna z, przy której linia zejścia osiąga wysokość `y` (za ostatnią osią tylną).
    pub fn kick_z(&self, y: f64) -> f64 {
        self.zlr - (y - Self::off() * self.r) / ANGLE.to_radians().tan()
    }
}

pub(crate) fn extra_materials(p: &P) -> Vec<MaterialSpec> {
    let m = |key, name, color: &str, roughness, metalness| MaterialSpec { key, name, color: color.into(), roughness, metalness, opacity: 1.0, double_sided: false, texture: "none", texture_scale: 1.0, emissive: None };
    vec![m(MatKey::Frame, "Rama", p.s("chassisColor"), 0.7, 0.3), m(MatKey::Metal, "Aluminium", "#aeb4b8", 0.45, 0.8)]
}

/// Punkty biodrowe Miejsc: kierowca po lewej (+X); 1+1 dwa fotele, 1+2 ławka na trzech,
/// brygadowa: przód 2, z tyłu 1–2 rzędy po 2–4.
pub(crate) fn seat_points(p: &P) -> Vec<V3> {
    let d = D::new(p);
    // punkt H w kabinie nad kołem: podłoga kabiny 4x4 ok. 1,2–1,3 m, siedzisko ok. 0,4 m wyżej
    let y = 2.0 * d.r + 0.15 + 0.5;
    let hwc = d.w / 2.0 - 0.03;
    let row = |n: usize, z: f64, out: &mut Vec<V3>| {
        let pitch = ((2.0 * hwc - 0.3) / n as f64).min(1.2);
        let x0 = if n == 1 { 0.5 * pitch } else { (n - 1) as f64 / 2.0 * pitch };
        out.extend((0..n).map(|j| [x0 - j as f64 * pitch, y, z]));
    };
    let mut out = Vec::new();
    // punkt H 1,08 m za czołem kabiny: stopy w pozie `drive` (ok. 0,7 m przed H) zostają w kabinie
    let zf0 = d.zc - d.nose - 1.08 + if d.nose > 0.05 { 0.25 } else { 0.0 };
    let front = match d.cab {
        Cab::Day => 3,
        _ => d.seats.min(2),
    };
    row(front, zf0, &mut out);
    if d.cab == Cab::Crew {
        let rest = d.seats - 2;
        for i in 0..d.rows {
            let n = rest / d.rows + usize::from(i < rest % d.rows);
            row(n, zf0 - 0.72 * (i + 1) as f64, &mut out);
        }
    }
    out.truncate(d.seats);
    out
}

// ───────────────────────── klocki ─────────────────────────

/// Belka od `a` do `b` o półprzekroju `hs` × `hv` (os „bok” = prostopadle do osi i do Y).
pub(crate) fn beam(mb: &mut MeshBuilder, mat: MatKey, a: V3, b: V3, hs: f64, hv: f64) {
    let u = norm(sub(b, a));
    let mut s = cross([0.0, 1.0, 0.0], u);
    if len(s) < 1e-6 {
        s = [1.0, 0.0, 0.0];
    }
    let s = norm(s);
    let v = cross(u, s);
    let c = |o: V3, i: f64, j: f64| add(add(o, scale(s, i * hs)), scale(v, j * hv));
    let cs = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let (ra, rb): (Vec<V3>, Vec<V3>) = (cs.iter().map(|&(i, j)| c(a, i, j)).collect(), cs.iter().map(|&(i, j)| c(b, i, j)).collect());
    mb.poly(mat, &ra, Some(scale(u, -1.0)));
    mb.poly(mat, &rb, Some(u));
    let outs = [scale(v, -1.0), s, v, scale(s, -1.0)];
    for k in 0..4 {
        let n = (k + 1) % 4;
        mb.quad(mat, ra[k], ra[n], rb[n], rb[k], Some(outs[k]));
    }
}

/// Walec od `a` do `b` o promieniu `r` (`seg` boków).
pub(crate) fn cyl(mb: &mut MeshBuilder, mat: MatKey, a: V3, b: V3, r: f64, seg: usize, caps: bool) {
    let u = norm(sub(b, a));
    let mut s = cross([0.0, 1.0, 0.0], u);
    if len(s) < 1e-6 {
        s = [1.0, 0.0, 0.0];
    }
    let s = norm(s);
    let v = cross(u, s);
    let dir = |k: usize| {
        let t = (k as f64 + 0.5) * std::f64::consts::TAU / seg as f64;
        add(scale(s, t.cos()), scale(v, t.sin()))
    };
    let ring = |o: V3| -> Vec<V3> { (0..seg).map(|k| add(o, scale(dir(k), r))).collect() };
    let (ra, rb) = (ring(a), ring(b));
    for k in 0..seg {
        let n = (k + 1) % seg;
        mb.quad(mat, ra[k], ra[n], rb[n], rb[k], Some(add(dir(k), dir(n))));
    }
    if caps {
        mb.poly(mat, &ra, Some(scale(u, -1.0)));
        mb.poly(mat, &rb, Some(u));
    }
}

/// Wypukły wielokąt (z, y) wyciągnięty wzdłuż X od `x0` do `x1`.
pub(crate) fn prism_x(mb: &mut MeshBuilder, mat: MatKey, pts: &[(f64, f64)], x0: f64, x1: f64) {
    let at = |x: f64| -> Vec<V3> { pts.iter().map(|&(z, y)| [x, y, z]).collect() };
    mb.poly(mat, &at(x1), Some([1.0, 0.0, 0.0]));
    mb.poly(mat, &at(x0), Some([-1.0, 0.0, 0.0]));
    let n = pts.len();
    let c = pts.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0 / n as f64, a.1 + p.1 / n as f64));
    for k in 0..n {
        let (p, q) = (pts[k], pts[(k + 1) % n]);
        let mid = ((p.0 + q.0) / 2.0 - c.0, (p.1 + q.1) / 2.0 - c.1);
        mb.quad(mat, [x0, p.1, p.0], [x1, p.1, p.0], [x1, q.1, q.0], [x0, q.1, q.0], Some([0.0, mid.1, mid.0]));
    }
}

/// Pionowy prostokąt przy burcie (x stałe, y0..y1, z0..z1), po obu stronach.
pub(crate) fn side_quad(mb: &mut MeshBuilder, mat: MatKey, x: f64, y0: f64, y1: f64, z0: f64, z1: f64) {
    for side in [1.0f64, -1.0] {
        mb.quad(mat, [x * side, y0, z0], [x * side, y0, z1], [x * side, y1, z1], [x * side, y1, z0], Some([side, 0.0, 0.0]));
    }
}

type Half = [V3; 7];

/// Pełny pierścień (12 wierzchołków) z półprzekroju (lustro po X).
pub(crate) fn full_ring(v: Half) -> Vec<V3> {
    let mut ring = v.to_vec();
    ring.extend((1..=5).rev().map(|i| [-v[i][0], v[i][1], v[i][2]]));
    ring
}

/// Strefa bocznej ścianki wg pozycji w pierścieniu: 0..5 prawa połowa, 6 = góra, 7..11 lustro.
pub(crate) fn half_seg(s: usize) -> usize {
    if s <= 5 { s } else { 11 - s }
}

// ───────────────────────── budowa ─────────────────────────

pub(crate) fn build(p: &P, seed: u32, mb: &mut MeshBuilder, lod: u8) -> Layout {
    let d = D::new(p);
    frame(mb, &d);
    let (stations, geom) = cab(mb, &d, p, seed, lod);
    front(mb, &d, p, seed);
    crate::zabudowa::module(mb, &d, p, seed);
    // oklejenie kabiny i belka na dachu kabiny (zabudowa: tylko lampy i obrysówki)
    let lightbar = crate::sluzba::apply_heavy(mb, p, seed, &geom);
    Layout { wheelbase: d.wb, track: d.track, stations, body_width: d.w, lightbar, wheels: Some(d.wheels()), anchors: d.anchors() }
}

/// Podłużnice (z kopniakiem na tyle), poprzeczki, osie z resorami, zbiornik i akumulator.
fn frame(mb: &mut MeshBuilder, d: &D) {
    let yb = d.y_fr - 0.24;
    let (x0, x1) = (0.34, 0.43);
    let z_front = d.zc - 0.05;
    let z_end = d.zr + 0.12;
    // punkt, w którym linia zejścia przecina spód podłużnicy
    let zk = d.kick_z(yb).clamp(z_end, d.zlr);
    for side in [1.0f64, -1.0] {
        let (a, b) = (x0 * side, x1 * side);
        let (xa, xb) = (a.min(b), a.max(b));
        mb.bx(MatKey::Frame, [xa, yb, zk], [xb, d.y_fr, z_front]);
        if zk - z_end > 0.05 {
            let (ya, yz) = (d.dep(z_end).max(yb), d.dep(z_end).max(yb));
            prism_x(mb, MatKey::Frame, &[(z_end, ya), (zk, yb), (zk, d.y_fr), (z_end, yz + 0.24)], xa, xb);
        }
    }
    for z in [d.zaf + 0.55, 0.0, d.zar + 0.45] {
        mb.bx(MatKey::Frame, [-x0, yb + 0.03, z - 0.05], [x0, yb + 0.19, z + 0.05]);
    }
    // osie: belka, resory; tylna z mostem
    let ax = d.track / 2.0 - 0.1;
    for &z in d.front_axles.iter().chain(&d.rear_axles) {
        mb.bx(MatKey::Metal, [-ax, d.r - 0.08, z - 0.09], [ax, d.r + 0.08, z + 0.09]);
        for side in [1.0f64, -1.0] {
            let (a, b) = (0.30 * side, 0.46 * side);
            mb.bx(MatKey::Frame, [a.min(b), d.r + 0.08, z - 0.07], [a.max(b), yb, z + 0.07]);
        }
    }
    let dh = 0.2;
    for &z in &d.rear_axles {
        mb.bx(MatKey::Metal, [-0.26, (d.r - dh).max(d.clr), z - 0.22], [0.26, d.r + dh, z + 0.22]);
    }
    // zbiornik paliwa (lewa) i akumulatory (prawa) między osiami
    mb.bx(MatKey::Frame, [0.46, yb - 0.05, -0.5], [0.92, yb + 0.35, 0.3]);
    mb.bx(MatKey::Plastic, [-0.8, yb, -0.3], [-0.46, yb + 0.3, 0.3]);
}

/// Kabina na powłoce `karoseria` (rodzaj Van z jawnym układem osi): nad silnikiem (czoło prawie
/// pionowe, podłoga nad kołem) albo z półmaską (`cabNose` > 0, Daily). Przednie nadkole wycięte
/// w kabinie, tył ucięty przed zabudową. Detale (lampy, grill, drzwi, lusterka) jak w vanie.
fn cab(mb: &mut MeshBuilder, d: &D, p: &P, seed: u32, lod: u8) -> (usize, crate::sluzba::HeavyCab) {
    use crate::karoseria::{self, Rama};
    use serde_json::json;
    let (top, ycb) = (d.cab_top, d.ycb);
    let semi = d.nose > 0.05;
    // dolna krawędź szyby: nad kołem (kabina nad silnikiem) albo nad maską (półmaska)
    let ywsb = if semi { (ycb + 0.62).max(d.r * 2.0 + 0.35) } else { (ycb + 0.38 * (top - ycb)).max(2.0 * d.r + 0.6) };
    let y_wt = if semi { (top - 0.22).max(ywsb + 0.6) } else { (top - 0.32).max(ywsb + 0.6) };
    let (zf, zr) = (d.zc, d.zb);
    let len = zf - zr;
    let mut m = p.0.clone();
    let mut set = |k: &str, v: serde_json::Value| {
        m.insert(k.to_string(), v);
    };
    set("bodyType", json!("van"));
    set("length", json!(len));
    set("height", json!(top));
    set("width", json!(d.w - 0.06));
    set("clearance", json!(ycb));
    set("beltline", json!((ywsb + 0.08) / top));
    set("hoodHeight", json!(ywsb / top));
    set("noseHeight", json!(if semi { (ycb + 0.36) / top } else { (ywsb - 0.05) / top }));
    set("noseRake", json!(if semi { 0.08 } else { 0.03 }));
    set("hoodLength", json!(if semi { d.nose + 0.05 } else { 0.07 } / len));
    set("windshieldAngle", json!(if semi { 60.0 } else { 82.0 }));
    set("windshieldTop", json!(y_wt / top));
    set("rearGlassAngle", json!(90.0));
    set("tailLean", json!(0.0));
    set("tailHeight", json!(0.99));
    set("roofDrop", json!(0.0));
    set("roofCrown", json!(0.03));
    set("roofPeak", json!(0.2));
    set("tumblehome", json!(0.02));
    set("cornerRadius", json!(if semi { 0.22 } else { 0.16 }));
    set("shoulderDrop", json!(0.22));
    set("sillTuck", json!(0.01));
    set("beltRise", json!(0.0));
    set("quarterWindow", json!(0));
    set("cargoWindows", json!(if d.cab == Cab::Crew { 1 } else { 0 }));
    set("rearBumperTop", json!(0.3));
    let pc = P(&m);
    let roof_mat = if roll(seed, 1.0, p.f("twoToneRoof")) { MatKey::RoofPaint } else { MatKey::Paint };
    let w = karoseria::build_with(&pc, roof_mat, karoseria::Gestosc::lod(lod), Some(Rama { zaf: d.zaf, zar: -1.0e3, zf, zr }));
    w.powloka.emit(mb, karoseria::CREASE);
    crate::detale::build(mb, &pc, seed, &w.ksztalt, &w.powloka, false, lod);
    let k = &w.ksztalt;
    if lod == 0 {
        crate::detale::interior(mb, k, &seat_points(p));
    }
    // stopnie pod drzwiami
    for side in [1.0f64, -1.0] {
        let z0 = k.z_b + 0.05;
        let (sa, sb) = ((k.hw - 0.02) * side, (k.hw - 0.3) * side);
        let ys = (ycb - 0.22).max(d.app(z0 + 0.7) + 0.02);
        mb.bx(MatKey::Plastic, [sa.min(sb), ys, z0], [sa.max(sb), ys + 0.04, z0 + 0.7]);
    }
    let geom = crate::sluzba::HeavyCab { hwc: k.hw, top, y_wt, z_wt: k.z_wt, z_rf: k.z_rf, ybel: k.belt_f, door: (k.z_b + 0.05, k.z_c - 0.12) };
    (w.stations, geom)
}

/// Zderzak przedni (kolor `bumperColor`), z wyciągarką w otworze i tablicą (przy wyciągarce
/// przesuniętą na prawą stronę zderzaka); światła tylne w zabudowie.
fn front(mb: &mut MeshBuilder, d: &D, p: &P, seed: u32) {
    let hb = d.w / 2.0 - 0.1;
    let y0 = (d.clr + 0.05).max(d.app(d.zf) + 0.02);
    // czoło zderzaka 4 mm za `zf`: tablica wystaje, a długość całkowita zostaje równa `length`
    let (y1, z0, zfb) = (y0 + 0.42, d.zc - 0.01, d.zf - 0.004);
    if p.f("winch") > 0.5 {
        let o = 0.5;
        mb.bx(MatKey::Bumper, [-hb, y0, z0], [-o, y1, zfb]);
        mb.bx(MatKey::Bumper, [o, y0, z0], [hb, y1, zfb]);
        mb.bx(MatKey::Bumper, [-o, y1 - 0.08, z0], [o, y1, zfb]);
        mb.bx(MatKey::Bumper, [-o, y0, z0], [o, y0 + 0.08, zfb]);
        // bęben i hak
        let (zm, ym) = (d.zf - 0.09, (y0 + y1) / 2.0);
        cyl(mb, MatKey::Metal, [-o, ym, zm], [o, ym, zm], 0.065, 8, true);
        mb.bx(MatKey::Plastic, [-0.07, y0 - 0.0, d.zf - 0.05], [0.07, y0 + 0.08, d.zf]);
        plate(mb, seed, 7.0, [-(o + hb) / 2.0, (y0 + y1) / 2.0, zfb], 1.0);
    } else {
        mb.bx(MatKey::Bumper, [-hb, y0, z0], [hb, y1, zfb]);
        plate(mb, seed, 7.0, [0.0, (y0 + y1) / 2.0, zfb], 1.0);
    }
}
