//! Wariant służby (WT §§ 25–38): Policja, Medycy, Straż, Techniczne. Nie rusza bryły — dokłada
//! oklejenie (pasy, prostokąty, szachownica, jodełka), napisy z fontu bitmapowego 5×7 (tylko
//! potrzebne znaki) i belkę świetlną w gnieździe na dachu (węzeł `Lightbar`).
//!
//! Oklejenie to wielokąty tuż nad ścianą: ściana boczna (segment półprzekroju talia→linia okien)
//! jest próbkowana tak jak loft, więc pas, przycięty do ściany (nadkola, maska), leży na nadwoziu.
//! Role kolorów: A = `Accent`, B = `Trim`, C = `Joint` (materiały wspólne z budynkami, bez zmian w kicie).

use crate::P;
use procedura_kit::data::ParamMap;
use procedura_kit::mesh::{add, cross, norm, scale, MatKey, MaterialSpec, MeshBuilder, MeshData, V3};
use procedura_kit::random::hash01_seeded;
use serde_json::json;
use std::f64::consts::PI;

pub(crate) type Half = [V3; 7];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Svc {
    Cywilny,
    Policja,
    Medycy,
    Straz,
    Techniczne,
}

fn svc_of(s: &str) -> Svc {
    match s {
        "policja" => Svc::Policja,
        "medycy" => Svc::Medycy,
        "straz" => Svc::Straz,
        "techniczne" => Svc::Techniczne,
        _ => Svc::Cywilny,
    }
}

/// Barwa nadwozia i zderzaków wynika z przepisów (§ 27, 28, 31, 38) i zastępuje kolory z wektora.
fn body_colors(s: Svc) -> Option<(&'static str, &'static str)> {
    match s {
        Svc::Cywilny => None,
        Svc::Policja => Some(("#a5a8a6", "#a5a8a6")), // ≈ RAL 9006
        Svc::Medycy => Some(("#f1dd38", "#f1dd38")),  // RAL 1016
        Svc::Straz => Some(("#ab2524", "#f7f9ef")),   // RAL 3000, zderzaki białe
        Svc::Techniczne => Some(("#f4f8f4", "#f4f8f4")),
    }
}

/// Po rozwiązaniu wektora: kolory służby i domyślna liczba lamp (WT § 26: 1–30).
pub(crate) fn defaults(params: &mut ParamMap) {
    let s = svc_of(params.get("service").and_then(|v| v.as_str()).unwrap_or("cywilny"));
    let Some((body, bumper)) = body_colors(s) else { return };
    for k in ["paintColor", "roofColor"] {
        params.insert(k.into(), json!(body));
    }
    params.insert("bumperColor".into(), json!(bumper));
    if params.get("lightbarLamps").and_then(|v| v.as_f64()).unwrap_or(0.0) < 1.0 {
        let n = match s {
            Svc::Policja => 10,
            Svc::Techniczne => 6,
            _ => 8,
        };
        params.insert("lightbarLamps".into(), json!(n));
    }
}

fn checker_of(p: &P) -> bool {
    p.s("livery") == "checker"
}

/// Role kolorów [A, B, C] (hex).
fn palette(s: Svc, checker: bool) -> [&'static str; 3] {
    match (s, checker) {
        (Svc::Policja, false) => ["#154889", "#f4f8f4", "#dff020"],
        (Svc::Policja, true) => ["#f9a800", "#232d53", "#232d53"], // Straż Miejska § 32b
        (Svc::Medycy, _) => ["#c1121c", "#154889", "#22224c"],
        (Svc::Straz, _) => ["#f7f9ef", "#ab2524", "#f7f9ef"],
        (Svc::Techniczne, _) => ["#e15501", "#f7b500", "#222426"],
        (Svc::Cywilny, _) => ["#000000"; 3],
    }
}

fn lamp(s: Svc) -> (&'static str, &'static str) {
    if s == Svc::Techniczne {
        ("#ffb20f", "#ffc94a") // żółte (§ 38)
    } else {
        ("#2f6bff", "#5a8cff")
    }
}

fn mat(key: MatKey, name: &'static str, color: &str, rough: f64, metal: f64, emissive: Option<&'static str>) -> MaterialSpec {
    MaterialSpec { key, name, color: color.into(), roughness: rough, metalness: metal, opacity: 1.0, double_sided: false, texture: "none", texture_scale: 1.0, emissive }
}

/// Materiały oklejenia dopisywane do nadwozia (puste dla cywilnego).
pub(crate) fn materials(p: &P) -> Vec<MaterialSpec> {
    let s = svc_of(p.s("service"));
    if s == Svc::Cywilny {
        return vec![];
    }
    let [a, b, c] = palette(s, checker_of(p));
    vec![mat(MatKey::Accent, "Oklejenie A", a, 0.35, 0.1, None), mat(MatKey::Trim, "Oklejenie B", b, 0.35, 0.1, None), mat(MatKey::Joint, "Oklejenie C", c, 0.4, 0.0, None)]
}

// ---------------------------------------------------------------- font 5×7

fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        // Ż = Z + kropka nad literą (rysowana osobno w `text`)
        'Z' | 'Ż' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        _ => [0; 7],
    }
}

/// Szerokość napisu w komórkach (znak 5 + odstęp 1).
fn text_cols(s: &str) -> usize {
    6 * s.chars().count() - 1
}

/// Napis jako prostokąty (ciągi pikseli w wierszu). `o` = lewy dolny róg, `u` = kierunek czytania,
/// `v` = góra liter, `n` = normalna powierzchni; `mirror` = odbicie lustrzane całego napisu.
fn text(mb: &mut MeshBuilder, m: MatKey, s: &str, o: V3, u: V3, v: V3, n: V3, cell: f64, mirror: bool) {
    let cols = text_cols(s);
    let pt = |c: usize, r: usize| add(add(o, scale(u, c as f64 * cell)), scale(v, r as f64 * cell));
    for (i, ch) in s.chars().enumerate() {
        // ciągi pikseli w wierszu, scalane w dół, gdy kolejny wiersz ma ten sam ciąg
        let mut boxes: Vec<(usize, usize, usize, usize)> = vec![]; // (c0, c1, wiersz od góry, wysokość)
        for (r, bits) in glyph(ch).iter().enumerate() {
            let mut c = 0;
            while c < 5 {
                if bits >> (4 - c) & 1 == 0 {
                    c += 1;
                    continue;
                }
                let c0 = c;
                while c < 5 && bits >> (4 - c) & 1 == 1 {
                    c += 1;
                }
                match boxes.iter_mut().find(|b| b.0 == c0 && b.1 == c && b.2 + b.3 == r) {
                    Some(b) => b.3 += 1,
                    None => boxes.push((c0, c, r, 1)),
                }
            }
        }
        if ch == 'Ż' {
            boxes.push((2, 3, usize::MAX, 0)); // kropka: wiersz nad literą
        }
        for (c0, c1, r, hgt) in boxes {
            let (x0, x1) = (6 * i + c0, 6 * i + c1);
            let (x0, x1) = if mirror { (cols - x1, cols - x0) } else { (x0, x1) };
            let (y0, y1) = if r == usize::MAX { (8, 9) } else { (7 - r - hgt, 7 - r) };
            mb.quad(m, pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1), Some(n));
        }
    }
}

// ---------------------------------------------------------------- ściana boczna

/// Nadwozie widziane jako ściana: stacje lofta i ich półprzekroje.
pub(crate) struct Hull<'a> {
    pub zs: &'a [f64],
    pub half: &'a dyn Fn(f64) -> Half,
    pub top: &'a dyn Fn(f64) -> f64,
    pub van: bool,
    pub h: f64,
    pub yb: f64,
    pub clr: f64,
    pub ytail: f64,
    pub zf: f64,
    /// Krawędź maski (góra pochylonego czoła).
    pub zn: f64,
    pub zr: f64,
    pub zg: f64,
    pub cowl: f64,
    pub roof_f: f64,
    pub roof_r: f64,
    pub z_dw: f64,
}

struct Wall<'a> {
    zs: &'a [f64],
    hs: Vec<Half>,
}

impl Wall<'_> {
    /// (x ściany, dolna krawędź ściany pionowej, linia okien) w z — interpolacja liniowa jak w locie.
    fn at(&self, z: f64) -> (f64, f64, f64) {
        let i = self.zs.windows(2).position(|w| z <= w[1]).unwrap_or(self.zs.len() - 2);
        let (z0, z1) = (self.zs[i], self.zs[i + 1]);
        let t = if z1 - z0 < 1e-9 { 0.0 } else { ((z - z0) / (z1 - z0)).clamp(0.0, 1.0) };
        let l = |f: &dyn Fn(&Half) -> f64| f(&self.hs[i]) + (f(&self.hs[i + 1]) - f(&self.hs[i])) * t;
        (l(&|h| h[2][0]), l(&|h| h[2][1]), l(&|h| h[3][1]))
    }
    fn breaks(&self, z0: f64, z1: f64) -> Vec<f64> {
        let mut v = vec![z0];
        v.extend(self.zs.iter().copied().filter(|&z| z > z0 + 1e-6 && z < z1 - 1e-6));
        v.push(z1);
        v
    }
    /// Czy prostokąt (z0..z1, y0..y1) leży w całości na pionowej ścianie.
    fn fits(&self, z0: f64, z1: f64, y0: f64, y1: f64) -> bool {
        self.breaks(z0, z1).iter().all(|&z| {
            let (_, m, b) = self.at(z);
            y0 >= m + 0.004 && y1 <= b - 0.01
        })
    }
}

fn sided(side: f64, p: V3) -> V3 {
    [p[0] * side, p[1], p[2]]
}

/// Najdłuższy odcinek z w `z0..z1`, na którym górna krawędź pasa `y1` mieści się pod linią okien
/// (pas ma stałą szerokość i kończy się prosto, zamiast zwężać się klinem na masce i tyle).
fn band_range(w: &Wall, z0: f64, z1: f64, y1: f64) -> Option<(f64, f64)> {
    let n = ((z1 - z0) / 0.02).ceil().max(1.0) as usize;
    let ok = |i: usize| y1 <= w.at(z0 + (z1 - z0) * i as f64 / n as f64).2 - 0.01;
    let (mut best, mut cur): (Option<(usize, usize)>, Option<usize>) = (None, None);
    for i in 0..=n {
        match (ok(i), cur) {
            (true, None) => cur = Some(i),
            (false, Some(s)) => {
                if best.is_none_or(|b| i - 1 - s > b.1 - b.0) {
                    best = Some((s, i - 1));
                }
                cur = None;
            }
            _ => {}
        }
    }
    if let Some(s) = cur {
        if best.is_none_or(|b| n - s > b.1 - b.0) {
            best = Some((s, n));
        }
    }
    let z = |i: usize| z0 + (z1 - z0) * i as f64 / n as f64;
    best.filter(|b| b.1 > b.0).map(|b| (z(b.0), z(b.1)))
}

/// Pas na ścianie boczej (obie strony): stała wysokość na odcinku z `band_range`, dół przycięty do talii.
fn band(mb: &mut MeshBuilder, w: &Wall, m: MatKey, (z0, z1): (f64, f64), (y0, y1): (f64, f64), e: f64) {
    let Some((z0, z1)) = band_range(w, z0, z1, y1) else { return };
    let pts: Vec<(f64, f64, f64, f64)> = w
        .breaks(z0, z1)
        .into_iter()
        .map(|z| {
            let (x, mid, _) = w.at(z);
            let lo = y0.max(mid);
            (z, x + e, lo, y1.max(lo))
        })
        .collect();
    // ponytail: kolejne stacje o tej samej geometrii nie są scalane (tylko ~1 kwadrat na stację)
    for k in pts.windows(2) {
        let (a, b) = (k[0], k[1]);
        if a.3 - a.2 < 0.002 && b.3 - b.2 < 0.002 {
            continue;
        }
        for side in [1.0, -1.0] {
            mb.quad(m, sided(side, [a.1, a.2, a.0]), sided(side, [b.1, b.2, b.0]), sided(side, [b.1, b.3, b.0]), sided(side, [a.1, a.3, a.0]), Some([side, 0.0, 0.0]));
        }
    }
}

/// Prostokąt na ścianie (obie strony), jeśli się mieści.
fn rect(mb: &mut MeshBuilder, w: &Wall, m: MatKey, (z0, z1): (f64, f64), (y0, y1): (f64, f64), e: f64) -> bool {
    if !w.fits(z0, z1, y0, y1) {
        return false;
    }
    let (xa, xb) = (w.at(z0).0 + e, w.at(z1).0 + e);
    for side in [1.0, -1.0] {
        mb.quad(m, sided(side, [xa, y0, z0]), sided(side, [xb, y0, z1]), sided(side, [xb, y1, z1]), sided(side, [xa, y1, z0]), Some([side, 0.0, 0.0]));
    }
    true
}

/// Napis na bokach, czytelny od przodu do tyłu po obu stronach; `zc` = środek, `y0` = dół liter.
fn side_text(mb: &mut MeshBuilder, w: &Wall, m: MatKey, s: &str, zc: f64, y0: f64, cell: f64, e: f64) {
    let len = text_cols(s) as f64 * cell;
    let x = w.at(zc).0 + e;
    text(mb, m, s, [x, y0, zc + len / 2.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], cell, false);
    text(mb, m, s, [-x, y0, zc - len / 2.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], cell, false);
}

/// Komórka napisu mieszcząca się w odcinku `room` (m) i nie większa niż `want_h`/7.
fn fit_cell(s: &str, want_h: f64, room: f64) -> f64 {
    (want_h / 7.0).min(room / text_cols(s) as f64)
}

// ---------------------------------------------------------------- gniazda dachu

/// Jednowymiarowa siatka zajętości dachu wzdłuż z (komórka 5 cm).
struct Slots {
    z0: f64,
    used: Vec<bool>,
}

const CELL: f64 = 0.05;

impl Slots {
    fn new(z0: f64, z1: f64) -> Slots {
        Slots { z0, used: vec![false; ((z1 - z0) / CELL).floor().max(0.0) as usize] }
    }
    /// Zajmuje odcinek `len` od przodu (największe z); zwraca środek.
    fn claim_front(&mut self, len: f64) -> Option<f64> {
        let n = (len / CELL).ceil() as usize;
        if n == 0 || n > self.used.len() {
            return None;
        }
        let start = (0..=self.used.len() - n).rev().find(|&s| self.used[s..s + n].iter().all(|u| !u))?;
        self.used[start..start + n].iter_mut().for_each(|u| *u = true);
        Some(self.z0 + (start as f64 + n as f64 / 2.0) * CELL)
    }
}

// ---------------------------------------------------------------- jodełka

/// Wielokąt przycięty do półpłaszczyzny `a·x + b·y ≤ c` (Sutherland–Hodgman).
fn clip(poly: &[(f64, f64)], a: f64, b: f64, c: f64) -> Vec<(f64, f64)> {
    let f = |p: (f64, f64)| a * p.0 + b * p.1 - c;
    let mut out = vec![];
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        let (fp, fq) = (f(p), f(q));
        if fp <= 0.0 {
            out.push(p);
        }
        if (fp < 0.0) != (fq < 0.0) && (fp - fq).abs() > 1e-12 {
            let t = fp / (fp - fq);
            out.push((p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t));
        }
    }
    out
}

/// Jodełka (∧) na prostokącie tyłu: pasy 45° o szerokości `sw` co `pitch`, w obu połowach.
fn chevron(mb: &mut MeshBuilder, m: MatKey, (xw, y0, y1): (f64, f64, f64), z: f64, (sw, pitch): (f64, f64)) {
    let big = 3.0;
    let mut c0 = y0 - 0.2; // pierwsza linia y + x = c0
    while c0 < y1 + xw {
        // pas c0 ≤ y + x ≤ c0 + sw, x w [0, xw]
        let mut poly = vec![(-big, c0 + big), (big, c0 - big), (big, c0 + sw - big), (-big, c0 + sw + big)];
        for (a, b, c) in [(-1.0, 0.0, 0.0), (1.0, 0.0, xw), (0.0, -1.0, -y0), (0.0, 1.0, y1)] {
            poly = clip(&poly, a, b, c);
        }
        if poly.len() >= 3 {
            for side in [1.0, -1.0] {
                let pts: Vec<V3> = poly.iter().map(|&(x, y)| [x * side, y, z]).collect();
                mb.poly(m, &pts, Some([0.0, 0.0, -1.0]));
            }
        }
        c0 += pitch;
    }
}

// ---------------------------------------------------------------- całość

/// Dokłada oklejenie do nadwozia; zwraca belkę świetlną (siatka + przesunięcie względem origin).
pub(crate) fn apply(mb: &mut MeshBuilder, p: &P, seed: u32, g: &Hull) -> Option<(MeshData, V3)> {
    let s = svc_of(p.s("service"));
    if s == Svc::Cywilny {
        return None;
    }
    let (a, b) = (MatKey::Accent, MatKey::Trim);
    let checker = checker_of(p);
    let w = Wall { zs: g.zs, hs: g.zs.iter().map(|&z| (g.half)(z)).collect() };
    // pas leży poniżej linii okien (§ 36 ust. 2) i poniżej podstawy szyby, żeby biegł od błotnika do tyłu
    let ytop = g.yb.min((g.top)(g.cowl)) - 0.03;
    let span = (g.zr + 0.05, (g.zf - 0.05).min(g.cowl + 0.15)); // pas kończy się na błotniku, nie schodzi na maskę
    let (e_band, e_det, e_txt) = (0.004, 0.007, 0.010);

    // --- pasy boczne
    let mut stack: Vec<(f64, MatKey)> = match s {
        Svc::Medycy => vec![(0.06, a), (0.08, b), (0.15, a)], // czerwony–niebieski–czerwony (Dz.U. 2026/227)
        Svc::Straz => vec![(0.15, a)],
        _ if checker => vec![(0.25, b)],
        _ => vec![(0.20, a)],
    };
    let mut y = ytop;
    let band_top = ytop;
    for (t, m) in stack.drain(..) {
        band(mb, &w, m, span, (y - t, y), e_band);
        y -= t;
    }
    let band_bot = y;
    // van: pasy przechodzą na tylne drzwi (między pionowymi lampami)
    if g.van {
        let hr = 0.84 * (g.half)(g.zr)[2][0];
        let mut y = ytop;
        let stack: Vec<(f64, MatKey)> = match s {
            Svc::Medycy => vec![(0.06, a), (0.08, b), (0.15, a)],
            Svc::Straz => vec![(0.15, a)],
            _ if checker => vec![(0.25, b)],
            _ => vec![(0.20, a)],
        };
        for (t, m) in stack {
            let z = g.zr - 0.004;
            mb.quad(m, [hr, y - t, z], [-hr, y - t, z], [-hr, y, z], [hr, y, z], Some([0.0, 0.0, -1.0]));
            y -= t;
        }
    }

    // --- wzór na pasie
    if checker && s != Svc::Medycy && s != Svc::Straz {
        // dwurzędowa szachownica: żółte pola 7×7 cm co 14 cm, przesunięte co rząd (na odcinku pasa)
        let q = 0.07;
        if let Some((za, zb)) = band_range(&w, span.0, span.1, band_top) {
            for row in 0..2 {
                let (ya, yb) = (band_top - 0.01 - q * (row + 1) as f64, band_top - 0.01 - q * row as f64);
                let mut z = za + q * (row % 2) as f64;
                while z + q < zb {
                    rect(mb, &w, a, (z, z + q), (ya, yb), e_det);
                    z += 2.0 * q;
                }
            }
        }
    } else if s == Svc::Policja {
        // białe prostokąty przy górnej i dolnej krawędzi pasa
        for (ya, yb) in [(band_top - 0.04, band_top - 0.01), (band_bot + 0.01, band_bot + 0.04)] {
            let mut z = span.0;
            while z + 0.12 < span.1 {
                rect(mb, &w, b, (z, z + 0.12), (ya, yb), e_det);
                z += 0.3;
            }
        }
    }

    // --- napisy
    let door_end = if g.van { g.z_dw } else { (g.roof_r + 0.2).max(g.zg + 0.2) };
    let door = (door_end, g.cowl - 0.12);
    match s {
        Svc::Policja if !checker => {
            let h = (band_top - band_bot - 0.09).max(0.06);
            let cell = fit_cell("POLICJA", h, 1.0);
            side_text(mb, &w, b, "POLICJA", 0.0, band_bot + 0.045, cell, e_txt);
        }
        Svc::Policja => {
            // Straż Miejska: żółty napis pod szachownicą, na przednich drzwiach
            let zc = (door.0 + door.1) / 2.0;
            let cell = fit_cell("STRAŻ MIEJSKA", band_top - band_bot - 0.17, (door.1 - door.0) * 0.95);
            side_text(mb, &w, a, "STRAŻ MIEJSKA", zc, band_bot + 0.02, cell, e_txt);
        }
        Svc::Straz => {
            let cell = fit_cell("STRAŻ", 0.14, (door.1 - door.0) * 0.9);
            let (zc, y0) = ((door.0 + door.1) / 2.0, band_bot - 0.04 - 7.0 * cell);
            if cell > 0.008 && w.fits(zc - 15.0 * cell, zc + 15.0 * cell, y0, y0 + 7.0 * cell) {
                side_text(mb, &w, a, "STRAŻ", zc, y0, cell, e_txt);
            }
        }
        _ => {}
    }

    // --- Medycy: lustrzany AMBULANS na masce, logo P w okręgu na boku vana
    if s == Svc::Medycy {
        let (zn, zs) = (g.zn - 0.03, g.cowl); // maska: od krawędzi czoła do podstawy szyby
        let hood_len = zn - zs;
        if hood_len > 0.2 {
            let zmid = (zn + zs) / 2.0;
            let half_w = (g.half)(zmid)[5][0];
            let cell = fit_cell("AMBULANS", (hood_len * 0.6).min(0.22), 1.8 * half_w);
            if cell > 0.01 {
                let (zb, v) = (zmid + 3.5 * cell, norm([0.0, (g.top)(zmid - 0.05) - (g.top)(zmid + 0.05), -0.1]));
                let n = norm(cross([1.0, 0.0, 0.0], v));
                let o = add([-(text_cols("AMBULANS") as f64) * cell / 2.0, (g.top)(zb), zb], scale(n, 0.006));
                text(mb, a, "AMBULANS", o, [1.0, 0.0, 0.0], v, n, cell, true);
            }
        }
        if g.van {
            // oznaczenie zespołu na gładkiej ścianie ładowni: nad pasami, za drzwiami przesuwnymi
            let (yc, r) = (ytop + 0.26, 0.2);
            let zc = g.z_dw - 1.75;
            if zc - r > g.zr + 0.1 {
                let x = w.at(zc).0 + e_det;
                for k in 0..12 {
                    let (t0, t1) = (2.0 * PI * k as f64 / 12.0, 2.0 * PI * (k + 1) as f64 / 12.0);
                    let q = |t: f64, rad: f64| [x, yc + rad * t.sin(), zc + rad * t.cos()];
                    for side in [1.0, -1.0] {
                        mb.quad(a, sided(side, q(t0, r)), sided(side, q(t1, r)), sided(side, q(t1, r - 0.04)), sided(side, q(t0, r - 0.04)), Some([side, 0.0, 0.0]));
                    }
                }
                side_text(mb, &w, a, "P", zc, yc - 3.5 * 0.028, 0.028, e_txt);
            }
        }
    }

    // --- tył: jodełka (livery = chevron)
    let ht = (g.half)(g.zr)[2][0];
    let bumper_top = g.clr + 0.08 + if g.van { 0.32 } else { 0.24 };
    let (ry0, ry1) = (bumper_top + 0.03, if g.van { g.h - 0.1 } else { g.ytail - 0.27 });
    let chevron_on = p.s("livery") == "chevron" && ry1 - ry0 > 0.08;
    if chevron_on {
        chevron(mb, a, (0.8 * ht, ry0, ry1), g.zr - 0.006, (0.12, 0.24));
    }
    // napis z tyłu (czytany zza pojazdu) i z przodu na masce (czytany sprzed pojazdu)
    let rear_word = match s {
        Svc::Policja if checker => Some(("STRAŻ MIEJSKA", b)),
        Svc::Policja => Some(("POLICJA", a)),
        Svc::Medycy => Some(("AMBULANS", a)),
        _ => None,
    };
    if let Some((word, m)) = rear_word.filter(|_| !chevron_on) {
        let (y0, y1) = if g.van { (ytop + 0.04, g.h * 0.6 - 0.04) } else { (ry0 + 0.02, g.ytail - 0.19) };
        let cell = fit_cell(word, (y1 - y0).min(0.12), if g.van { 1.6 * ht } else { 0.95 * ht });
        let wid = text_cols(word) as f64 * cell;
        text(mb, m, word, [wid / 2.0, (y0 + y1) / 2.0 - 3.5 * cell, g.zr - 0.006], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0], cell, false);
    }
    if s == Svc::Policja && !checker {
        // niebieskie pole z białym POLICJA przy przedniej krawędzi maski
        let (zh, zs) = (g.zn - 0.03, g.cowl);
        if zh - zs > 0.25 {
            let half_w = (g.half)(zh - 0.1)[5][0];
            let cell = fit_cell("POLICJA", 0.1, 1.2 * half_w);
            let (wid, ht7) = (text_cols("POLICJA") as f64 * cell, 7.0 * cell);
            let zb = zh - 0.04;
            let v = norm([0.0, (g.top)(zb - ht7) - (g.top)(zb), -ht7]);
            let n = norm(cross([1.0, 0.0, 0.0], v));
            let at = |x: f64, t: f64, lift: f64| add(add([x, (g.top)(zb), zb], scale(v, t)), scale(n, lift));
            let (m0, m1) = (-0.03, ht7 + 0.03);
            mb.quad(a, at(-wid / 2.0 - 0.04, m0, 0.005), at(wid / 2.0 + 0.04, m0, 0.005), at(wid / 2.0 + 0.04, m1, 0.005), at(-wid / 2.0 - 0.04, m1, 0.005), Some(n));
            text(mb, b, "POLICJA", at(-wid / 2.0, 0.0, 0.008), [1.0, 0.0, 0.0], v, n, cell, false);
        }
    }

    // --- dach: belka i numer operacyjny
    let mut slots = Slots::new(g.roof_r + 0.03, g.roof_f - 0.03);
    let lamps = p.f("lightbarLamps") as usize;
    let mut bar = None;
    if lamps > 0 {
        let depth = ((g.roof_f - g.roof_r) - 0.06).clamp(0.12, 0.28);
        if let Some(zc) = slots.claim_front(depth) {
            let (z0, z1) = (zc - depth / 2.0, zc + depth / 2.0);
            let roof_x = [z0, z1].iter().map(|&z| (g.half)(z)[5][0]).fold(f64::MAX, f64::min);
            let lx = (1.7 * roof_x).clamp(0.9, 1.4);
            let ymax = [z0, zc, z1].iter().map(|&z| (g.top)(z)).fold(f64::MIN, f64::max);
            let y = ymax + if g.van { 0.02 } else { 0.0 } - 0.01;
            bar = Some((lightbar(s, lamps, lx, depth), [0.0, y, zc]));
        }
    }
    if s == Svc::Straz {
        let number = format!("{:03} L {:02}", 1 + (hash01_seeded(seed, &[21.0]) * 998.0) as u32, 1 + (hash01_seeded(seed, &[22.0]) * 98.0) as u32);
        let hroof = (g.half)(g.roof_r + 0.5 * (g.roof_f - g.roof_r))[5][0];
        let cell = (0.2f64 / 7.0).min(1.7 * hroof / text_cols(&number) as f64);
        let mut placed = false;
        if cell >= 0.15 / 7.0 {
            if let Some(zc) = slots.claim_front(7.0 * cell) {
                let (wid, yt) = (text_cols(&number) as f64 * cell, (g.top)(zc) + 0.004);
                text(mb, a, &number, [wid / 2.0, yt, zc - 3.5 * cell], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], cell, false);
                placed = true;
            }
        }
        if !placed && !chevron_on {
            // brak miejsca na dachu: numer na tylnej ściance, 15 cm
            let cell = (0.15f64 / 7.0).min((ry1 - ry0) / 7.0).min(1.5 * ht / text_cols(&number) as f64);
            let wid = text_cols(&number) as f64 * cell;
            text(mb, a, &number, [wid / 2.0, (ry0 + ry1) / 2.0 - 3.5 * cell, g.zr - 0.006], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0], cell, false);
        }
    }
    bar
}

/// Belka: obudowa i lampy (kwadraty emisyjne): 2 na końcach, reszta po równo z przodu i z tyłu.
fn lightbar(s: Svc, n: usize, lx: f64, d: f64) -> MeshData {
    let mut mb = MeshBuilder::default();
    let hgt = 0.11;
    mb.bx(MatKey::Metal, [-lx / 2.0, 0.0, -d / 2.0], [lx / 2.0, hgt, d / 2.0]);
    let (front, back, ends) = match n {
        0 => (0, 0, 0),
        1 => (1, 0, 0),
        _ => ((n - 2 + 1) / 2, (n - 2) / 2, 2),
    };
    let eps = 0.002;
    let face = |mb: &mut MeshBuilder, k: usize, z: f64, nz: f64| {
        let pitch = (lx - 0.06) / k.max(1) as f64;
        let wd = (pitch - 0.012).clamp(0.02, 0.12);
        for i in 0..k {
            let x = -lx / 2.0 + 0.03 + pitch * (i as f64 + 0.5);
            mb.quad(MatKey::GlassLit, [x - wd / 2.0, 0.015, z], [x + wd / 2.0, 0.015, z], [x + wd / 2.0, hgt - 0.015, z], [x - wd / 2.0, hgt - 0.015, z], Some([0.0, 0.0, nz]));
        }
    };
    face(&mut mb, front, d / 2.0 + eps, 1.0);
    face(&mut mb, back, -d / 2.0 - eps, -1.0);
    if ends == 2 {
        for side in [1.0, -1.0] {
            let x = side * (lx / 2.0 + eps);
            let wd = (d / 2.0 - 0.02).max(0.03);
            mb.quad(MatKey::GlassLit, [x, 0.015, -wd / 2.0], [x, 0.015, wd / 2.0], [x, hgt - 0.015, wd / 2.0], [x, hgt - 0.015, -wd / 2.0], Some([side, 0.0, 0.0]));
        }
    }
    let (col, em) = lamp(s);
    mb.build(vec![mat(MatKey::Metal, "Obudowa belki", "#1a1c1e", 0.5, 0.4, None), mat(MatKey::GlassLit, "Lampy błyskowe", col, 0.2, 0.0, Some(em))])
}
