//! Pojazdy ciężkie (`bodyType = "podwozie"`): kabina nad silnikiem (1+1, 1+2, 1+1+4 według liczby
//! Miejsc), rama z podniesionym tyłem (kąt zejścia 23°), osie 4x2/4x4 i wspólne klocki (belka,
//! walec, graniastosłup). Moduły zabudowy na ramie leżą w `zabudowa.rs`.
//! Konwencja jak w `body.rs`: przód +Z, lewa strona +X, origin = środek między osiami na gruncie.

use crate::body::Layout;
use crate::{roll, P};
use procedura_kit::mesh::{add, cross, len, norm, scale, sub, MatKey, MaterialSpec, MeshBuilder, V3};

/// Kąt najazdu i zejścia z przetargów PSP.
const ANGLE: f64 = 23.0;

pub(crate) fn lerp(a: f64, b: f64, t: f64) -> f64 {
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
    pub cab_len: f64,
    /// Czoło i tył kabiny.
    pub zc: f64,
    pub zb: f64,
    pub cab_top: f64,
    pub ycb: f64,
}

impl D {
    pub fn new(p: &P) -> D {
        let (l, w, h, wb) = (p.f("length"), p.f("width"), p.f("height"), p.f("wheelbase"));
        let (clr, r, tw) = (p.f("clearance"), p.f("wheelDiameter") / 2.0, p.f("tyreWidth"));
        let (fo, ro) = (p.f("frontOverhang"), p.f("rearOverhang"));
        let oh = (l - wb).max(0.8);
        let (zaf, zar) = (wb / 2.0, -wb / 2.0);
        let (zf, zr) = (zaf + oh * fo / (fo + ro), zar - oh * ro / (fo + ro));
        let ra = r + p.f("wheelArchGap");
        let seats = p.f("seats") as usize;
        let rows = if seats > 3 { ((seats - 2).div_ceil(2)).min(2) } else { 0 };
        let cab = match seats {
            0..=2 => Cab::Short,
            3 => Cab::Day,
            _ => Cab::Crew,
        };
        let cab_len = match cab {
            Cab::Short => 1.6,
            Cab::Day => 1.83,
            Cab::Crew => 1.83 + 0.72 * rows as f64,
        };
        let y_fr = r + 0.36;
        let zc = zf - 0.14;
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
            cab_len,
            zc,
            zb: zc - cab_len,
            cab_top: p.f("cabHeight").min(h),
            ycb: y_fr - 0.25,
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

    /// Najniższa dozwolona wysokość za tylną osią (kąt zejścia), 0 przed nią.
    pub fn dep(&self, z: f64) -> f64 {
        if z < self.zar { (ANGLE.to_radians().tan() * (self.zar - z) + Self::off() * self.r).max(0.0) } else { 0.0 }
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

    /// Współrzędna z, przy której linia zejścia osiąga wysokość `y` (za tylną osią).
    pub fn kick_z(&self, y: f64) -> f64 {
        self.zar - (y - Self::off() * self.r) / ANGLE.to_radians().tan()
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
    let y = 2.0 * d.r + 0.15 + 0.4;
    let hwc = d.w / 2.0 - 0.09;
    let row = |n: usize, z: f64, out: &mut Vec<V3>| {
        let pitch = ((2.0 * hwc - 0.3) / n as f64).min(1.2);
        let x0 = if n == 1 { 0.5 * pitch } else { (n - 1) as f64 / 2.0 * pitch };
        out.extend((0..n).map(|j| [x0 - j as f64 * pitch, y, z]));
    };
    let mut out = Vec::new();
    let zf0 = d.zc - 0.95;
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

pub(crate) fn build(p: &P, seed: u32, mb: &mut MeshBuilder) -> Layout {
    let d = D::new(p);
    frame(mb, &d);
    let stations = cab(mb, &d, p, seed);
    front(mb, &d, p);
    crate::zabudowa::module(mb, &d, p);
    // ponytail: no livery/lightbar on heavy vehicles yet (sluzba::apply needs a car hull), add with issue 10
    Layout { wheelbase: d.wb, track: d.track, stations, lightbar: None }
}

/// Podłużnice (z kopniakiem na tyle), poprzeczki, osie z resorami, zbiornik i akumulator.
fn frame(mb: &mut MeshBuilder, d: &D) {
    let yb = d.y_fr - 0.24;
    let (x0, x1) = (0.34, 0.43);
    let z_front = d.zc - 0.05;
    let z_end = d.zr + 0.12;
    // punkt, w którym linia zejścia przecina spód podłużnicy
    let zk = d.kick_z(yb).clamp(z_end, d.zar);
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
    for z in [d.zaf, d.zar] {
        mb.bx(MatKey::Metal, [-ax, d.r - 0.08, z - 0.09], [ax, d.r + 0.08, z + 0.09]);
        for side in [1.0f64, -1.0] {
            let (a, b) = (0.30 * side, 0.46 * side);
            mb.bx(MatKey::Frame, [a.min(b), d.r + 0.08, z - 0.07], [a.max(b), yb, z + 0.07]);
        }
    }
    let dh = 0.2;
    mb.bx(MatKey::Metal, [-0.26, (d.r - dh).max(d.clr), d.zar - 0.22], [0.26, d.r + dh, d.zar + 0.22]);
    // zbiornik paliwa (lewa) i akumulatory (prawa) między osiami
    mb.bx(MatKey::Metal, [0.46, yb - 0.1, -0.55], [0.98, yb + 0.4, 0.55]);
    mb.bx(MatKey::Plastic, [-0.8, yb, -0.3], [-0.46, yb + 0.3, 0.3]);
}

/// Kabina: loft ze stacji (profil boku z nadkolem, szyba pochylona wg `windshieldAngle`),
/// potem okna, drzwi, lampy, grill, lusterka, stopnie.
fn cab(mb: &mut MeshBuilder, d: &D, p: &P, seed: u32) -> usize {
    let hwc = d.w / 2.0 - 0.09;
    let (zc, zb, top, ycb) = (d.zc, d.zb, d.cab_top, d.ycb);
    let ywsb = ycb + 0.38 * (top - ycb);
    let ybel = ywsb + 0.1;
    let z_rf = zc - ((top - ywsb) / p.f("windshieldAngle").to_radians().tan()).max(0.15);
    let prof = [(zb, top), (z_rf, top), (zc, ywsb)];
    let tumble = p.f("tumblehome");
    let half = |z: f64| -> Half {
        let tz = pl(&prof, z);
        let g = ((tz - ybel) / (top - ybel)).clamp(0.0, 1.0);
        let hr = hwc * (1.0 - tumble * g);
        let cc = 0.12f64.min(0.3 * hr);
        let bel = ybel.min(tz - cc - 0.03);
        let sill = ycb.max(d.arch(z, d.zaf));
        let mid = (ycb + 0.25).max(sill + 0.07).min(bel - 0.04);
        let sill = sill.min(mid - 0.02);
        [[0.0, ycb, z], [hwc * 0.97, sill, z], [hwc, mid, z], [hwc, bel, z], [hr, tz - cc, z], [hr - cc, tz, z], [0.0, tz, z]]
    };
    // stacje: końce i szyba (priorytet), nadkole
    let mut zs = vec![zb, z_rf, zc];
    let mut arches: Vec<f64> = [d.zaf - d.arch_a, d.zaf - d.arch_at, d.zaf + d.arch_at, d.zaf + d.arch_a].into_iter().filter(|&z| z > zb + 0.05 && z < zc - 0.05).collect();
    arches.retain(|&z| zs.iter().all(|k| (k - z).abs() > 0.08));
    arches.dedup_by(|a, b| (*a - *b).abs() < 0.08);
    zs.extend(arches);
    zs.sort_by(|a, b| a.total_cmp(b));
    let rings: Vec<Vec<V3>> = zs.iter().map(|&z| full_ring(half(z))).collect();
    let roof_mat = if roll(seed, 1.0, p.f("twoToneRoof")) { MatKey::RoofPaint } else { MatKey::Paint };
    mb.loft_with(&rings, Some(MatKey::Paint), |i, s| {
        let zm = (zs[i] + zs[i + 1]) / 2.0;
        let wind = zm > z_rf;
        match half_seg(s) {
            0 => MatKey::Plastic,
            1 => MatKey::Plastic,
            3 => MatKey::Glass,
            4 | 5 if wind => MatKey::Glass,
            4 | 5 => roof_mat,
            _ => MatKey::Paint,
        }
    });

    // pas na ściance bocznej między segmentami półprzekroju (po obu stronach)
    let strip = |mb: &mut MeshBuilder, mat: MatKey, (za, zb2): (f64, f64), (k0, k1): (usize, usize), e: f64| {
        let (a, b) = (half(za), half(zb2));
        for k in k0..k1 {
            for side in [1.0f64, -1.0] {
                let q = |v: V3| [(v[0] + e) * side, v[1], v[2]];
                mb.quad(mat, q(a[k]), q(b[k]), q(b[k + 1]), q(a[k + 1]), Some([side, 0.0, 0.0]));
            }
        }
    };
    // szwy drzwi i słupki: A/B (+ C w brygadowej), grube na szkle jako słupki
    let door_z = |i: usize| zc - 0.2 - 1.0 * i as f64;
    let mut seams = vec![door_z(0), door_z(1)];
    if d.cab == Cab::Crew {
        seams.push(door_z(1) - (d.cab_len - 1.5).max(0.8) * 0.5);
        seams.push(zb + 0.12);
    }
    seams.retain(|&z| z > zb + 0.05 && z < z_rf);
    for z in &seams {
        strip(mb, MatKey::Frame, (z - 0.015, z + 0.015), (1, 3), 0.004);
        strip(mb, MatKey::Frame, (z - 0.04, z + 0.04), (3, 4), 0.004);
    }
    let handle = |mb: &mut MeshBuilder, z: f64| {
        let bel = half(z)[3];
        side_quad(mb, MatKey::Plastic, bel[0] + 0.005, bel[1] - 0.2, bel[1] - 0.17, z - 0.09, z + 0.09);
    };
    handle(mb, door_z(0) - 0.12);
    if d.cab == Cab::Crew {
        handle(mb, door_z(1) - 0.12);
    }

    // czoło: grill, reflektory z oprawą, wycieraczki, daszek
    let hz = |z: f64| half(z)[2][0];
    let (zfc, hf) = (zc + 0.004, hz(zc));
    let (yg0, yg1) = (ycb + 0.22, ycb + 0.22 + 0.5 * (ywsb - ycb - 0.2));
    let gx = 0.40 * hf;
    mb.quad(MatKey::Plastic, [-gx, yg0, zfc], [gx, yg0, zfc], [gx, yg1, zfc], [-gx, yg1, zfc], Some([0.0, 0.0, 1.0]));
    let g = p.f("grill").max(0.2);
    for i in 1..=3 {
        let y = lerp(yg0, yg1, i as f64 / 4.0);
        mb.quad(MatKey::Metal, [-gx * (0.5 + 0.5 * g), y - 0.012, zfc + 0.003], [gx * (0.5 + 0.5 * g), y - 0.012, zfc + 0.003], [gx * (0.5 + 0.5 * g), y + 0.012, zfc + 0.003], [-gx * (0.5 + 0.5 * g), y + 0.012, zfc + 0.003], Some([0.0, 0.0, 1.0]));
    }
    let (ly0, ly1) = (ycb + 0.2, ycb + 0.2 + 0.2);
    for side in [1.0f64, -1.0] {
        let q = |x: f64, y: f64, dz: f64| [x * side, y, zfc + dz];
        mb.quad(MatKey::Plastic, q(0.50 * hf, ly0 - 0.02, 0.0), q(0.97 * hf, ly0 - 0.02, 0.0), q(0.97 * hf, ly1 + 0.02, 0.0), q(0.50 * hf, ly1 + 0.02, 0.0), Some([0.0, 0.0, 1.0]));
        mb.quad(MatKey::Headlight, q(0.54 * hf, ly0, 0.003), q(0.93 * hf, ly0, 0.003), q(0.93 * hf, ly1, 0.003), q(0.54 * hf, ly1, 0.003), Some([0.0, 0.0, 1.0]));
    }
    // daszek nad szybą
    let (zv, ho) = (z_rf, hwc * 0.8);
    mb.bx(MatKey::Plastic, [-ho, top - 0.02, zv - 0.02], [ho, top + 0.1f64.min(d.h - top), zv + 0.12]);

    // lusterka (do szerokości całkowitej) i stopnie
    for side in [1.0f64, -1.0] {
        let (xa, xb) = (hwc * side, d.w / 2.0 * side);
        let zm = zc - 0.3;
        let ym = ywsb + 0.25;
        mb.bx(MatKey::Plastic, [xa.min(xb), ym + 0.05, zm - 0.03], [xa.max(xb), ym + 0.08, zm + 0.03]);
        let xh = if side > 0.0 { (xb - 0.05, xb) } else { (xb, xb + 0.05) };
        mb.bx(MatKey::Plastic, [xh.0, ym - 0.2, zm - 0.07], [xh.1, ym + 0.12, zm + 0.07]);
        let z0 = door_z(0) - 0.85;
        let (sa, sb) = (hwc * side, (hwc - 0.3) * side);
        let ys = (ycb - 0.2).max(d.app(z0 + 0.75) + 0.02);
        mb.bx(MatKey::Plastic, [sa.min(sb), ys, z0], [sa.max(sb), ys + 0.04, z0 + 0.75]);
    }
    zs.len()
}

/// Zderzak przedni (kolor `bumperColor`), z wyciągarką w otworze, światła tylne w zabudowie.
fn front(mb: &mut MeshBuilder, d: &D, p: &P) {
    let hb = d.w / 2.0 - 0.1;
    let y0 = (d.clr + 0.05).max(d.app(d.zf) + 0.02);
    let (y1, z0) = (y0 + 0.42, d.zc - 0.01);
    if p.f("winch") > 0.5 {
        let o = 0.5;
        mb.bx(MatKey::Bumper, [-hb, y0, z0], [-o, y1, d.zf]);
        mb.bx(MatKey::Bumper, [o, y0, z0], [hb, y1, d.zf]);
        mb.bx(MatKey::Bumper, [-o, y1 - 0.08, z0], [o, y1, d.zf]);
        mb.bx(MatKey::Bumper, [-o, y0, z0], [o, y0 + 0.08, d.zf]);
        // bęben i hak
        let (zm, ym) = (d.zf - 0.09, (y0 + y1) / 2.0);
        cyl(mb, MatKey::Metal, [-o, ym, zm], [o, ym, zm], 0.065, 8, true);
        mb.bx(MatKey::Plastic, [-0.07, y0 - 0.0, d.zf - 0.05], [0.07, y0 + 0.08, d.zf]);
    } else {
        mb.bx(MatKey::Bumper, [-hb, y0, z0], [hb, y1, d.zf]);
    }
}
