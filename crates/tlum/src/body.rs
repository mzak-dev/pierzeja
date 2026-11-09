//! Ciało z pierścieni przypiętych do kości. Pierścień powstaje w pozie spoczynkowej i dostaje
//! wagi kości (jedna lub dwie przy stawie), więc w pozie łokcie, kolana i kręgosłup zginają się
//! bez szczelin. Palce: każdy człon to prostopadłościan przypięty do własnej kości (`mb.push`).
//! Odzież: sloty materiałów + warstwy (kamizelka, taśmy odblaskowe, napisy, pas, krawat, poły płaszcza).

use crate::font;
use crate::skel::{idx, sole_points, Joint, Scale, FINGERS, PHALANGES};
use crate::P;
use procedura_kit::mesh::{add, cross, dot, len, norm, scale, sub, MatKey, MeshBuilder, Xform, V3};
use std::f64::consts::TAU;

// ------------------------------------------------------------------ wagi i pierścienie

/// Wagi kości wierzchołka (suma 1).
pub(crate) type Wt = Vec<(usize, f64)>;

pub(crate) fn w1(b: usize) -> Wt {
    vec![(b, 1.0)]
}
pub(crate) fn w2(a: usize, b: usize, t: f64) -> Wt {
    if t <= 0.0 {
        w1(a)
    } else if t >= 1.0 {
        w1(b)
    } else {
        vec![(a, 1.0 - t), (b, t)]
    }
}
fn wmix(a: &Wt, b: &Wt, t: f64) -> Wt {
    let mut out: Wt = a.iter().map(|&(i, x)| (i, x * (1.0 - t))).collect();
    for &(i, x) in b {
        match out.iter_mut().find(|e| e.0 == i) {
            Some(e) => e.1 += x * t,
            None => out.push((i, x * t)),
        }
    }
    out.retain(|e| e.1 > 1e-9);
    out
}

/// Transformacje kości w pozie (z uniesieniem na grunt).
pub(crate) struct Rig<'a> {
    pub w: &'a [Xform],
}

impl Rig<'_> {
    pub fn p(&self, q: V3, wt: &Wt) -> V3 {
        wt.iter().fold([0.0; 3], |a, &(b, x)| add(a, scale(self.w[b].point(q), x)))
    }
    pub fn d(&self, v: V3, wt: &Wt) -> V3 {
        wt.iter().fold([0.0; 3], |a, &(b, x)| add(a, scale(self.w[b].dir(v), x)))
    }
    /// Wielokąt wypukły w spoczynku → w pozie; `want` = zamierzona normalna w spoczynku.
    pub fn poly(&self, mb: &mut MeshBuilder, mat: MatKey, pts: &[V3], wt: &Wt, want: V3) {
        let v: Vec<V3> = pts.iter().map(|&q| self.p(q, wt)).collect();
        mb.poly(mat, &v, Some(self.d(want, wt)));
    }
}

#[derive(Clone)]
pub(crate) struct Ring {
    pub pts: Vec<V3>,
    /// Wagi na wierzchołek (zwykle wszystkie równe).
    pub wt: Vec<Wt>,
}

impl Ring {
    fn new(pts: Vec<V3>, wt: Wt) -> Ring {
        let n = pts.len();
        Ring { pts, wt: vec![wt; n] }
    }
    fn lerp(a: &Ring, b: &Ring, t: f64) -> Ring {
        Ring { pts: a.pts.iter().zip(&b.pts).map(|(&p, &q)| add(p, scale(sub(q, p), t))).collect(), wt: a.wt.iter().zip(&b.wt).map(|(x, y)| wmix(x, y, t)).collect() }
    }
    /// Normalna płaszczyzny pierścienia (Newell), zgodna z kierunkiem obiegu.
    fn axis(&self) -> V3 {
        let n = self.pts.len();
        let mut s = [0.0; 3];
        for i in 0..n {
            let (a, b) = (self.pts[i], self.pts[(i + 1) % n]);
            s[0] += (a[1] - b[1]) * (a[2] + b[2]);
            s[1] += (a[2] - b[2]) * (a[0] + b[0]);
            s[2] += (a[0] - b[0]) * (a[1] + b[1]);
        }
        norm(s)
    }
    /// Pierścień odsunięty o `off` na zewnątrz (w płaszczyźnie pierścienia).
    fn grow(&self, off: f64) -> Ring {
        let n = self.pts.len();
        let ax = self.axis();
        let out = |i: usize| norm(cross(sub(self.pts[(i + 1) % n], self.pts[i]), ax));
        let pts = (0..n).map(|i| add(self.pts[i], scale(norm(add(out(i), out((i + n - 1) % n))), off))).collect();
        Ring { pts, wt: self.wt.clone() }
    }
    fn posed(&self, rig: &Rig) -> Vec<V3> {
        self.pts.iter().zip(&self.wt).map(|(&p, w)| rig.p(p, w)).collect()
    }
}

fn loft(mb: &mut MeshBuilder, rig: &Rig, rings: &[Ring], pick: impl Fn(usize, usize) -> MatKey) {
    let posed: Vec<Vec<V3>> = rings.iter().map(|r| r.posed(rig)).collect();
    mb.loft_with(&posed, None, pick);
}

fn cap(mb: &mut MeshBuilder, rig: &Rig, r: &Ring, mat: MatKey, out: V3) {
    mb.poly(mat, &r.posed(rig), Some(rig.d(out, &r.wt[0])));
}

/// Pierścień 8-boczny o ściankach zwróconych w ±ex i ±ey (wierzchołki co 45°, od 22,5°).
fn ring8(c: V3, ex: V3, ey: V3, rd: f64, rw: f64) -> Vec<V3> {
    (0..8)
        .map(|k| {
            let a = TAU / 16.0 + TAU * k as f64 / 8.0;
            add(c, add(scale(ex, rd * a.cos() / (TAU / 16.0).cos()), scale(ey, rw * a.sin() / (TAU / 16.0).cos())))
        })
        .collect()
}

pub(crate) fn frame(dir: V3, hint: V3) -> (V3, V3, V3) {
    let ez = norm(dir);
    let ex = norm(sub(hint, scale(ez, dot(hint, ez))));
    (ex, cross(ez, ex), ez)
}

// ------------------------------------------------------------------ ubiór

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Tape {
    None,
    /// Srebrne taśmy (PRM, odzież robocza).
    Silver,
    /// Żółto-srebrne taśmy ubrania specjalnego PSP.
    Fire,
}

/// Wybory ubioru wpływające na bryłę i przydział slotów (kolory składa `lib.rs`).
pub(crate) struct Fit {
    pub torso: MatKey,
    pub hips: MatKey,
    pub sleeve: MatKey,
    pub short: bool,
    pub jacket: bool,
    pub coat: bool,
    pub vest: bool,
    pub bulk: f64,
    pub cut_legs: bool,
    pub skirt: bool,
    pub boot: f64,
    pub tape: Tape,
    pub suit: bool,
    pub tie: bool,
    pub collar: bool,
    pub turtleneck: bool,
    pub hood: bool,
    pub belt: Option<MatKey>,
    pub duty_belt: bool,
    pub police: bool,
    pub vest_dark: bool,
    pub medic: bool,
    pub backpack: bool,
    pub bag: bool,
    pub scba: bool,
    pub scarf: bool,
}

impl Fit {
    pub fn new(p: &P) -> Fit {
        let outer = p.s("outerType");
        let mid = p.s("upperMidType");
        let jacket = !matches!(outer, "brak" | "kamizelka");
        let suit = mid == "marynarka" && !jacket;
        let short = matches!(p.s("upperBaseType"), "tshirt" | "polo" | "koszulka_ratownicza") && mid == "brak" && !jacket;
        let lower = p.s("lowerType");
        let sleeve = if jacket || suit { MatKey::Outer } else { MatKey::Top };
        let gear = p.s("gear");
        let carry = p.s("carryType");
        Fit {
            torso: if jacket || suit || outer == "kamizelka" { MatKey::Outer } else { MatKey::Top },
            hips: if jacket || suit { MatKey::Outer } else { MatKey::Bottom },
            sleeve,
            short,
            jacket: jacket || suit,
            coat: matches!(outer, "plaszcz" | "parka"),
            vest: p.s("vest") != "brak",
            bulk: match outer {
                "puchowka" => 1.12,
                "parka" => 1.08,
                "plaszcz" | "ubranie_specjalne" => 1.06,
                "brak" => 1.0,
                "kamizelka" => 1.02,
                _ => 1.04,
            } * if matches!(mid, "brak" | "marynarka") { 1.0 } else { 1.025 },
            cut_legs: matches!(lower, "szorty" | "spodnica"),
            skirt: lower == "spodnica",
            // cholewka: ułamek goleni od kostki; spodnie ubrania specjalnego zachodzą na buty
            boot: match p.s("feetType") {
                _ if lower == "spodnie_ubranie_specjalne" => 0.0,
                "botki" => 0.3,
                "buty_zimowe" => 0.34,
                "buty_robocze" | "buty_mundurowe" => 0.32,
                "buty_strazackie" => 0.55,
                _ => 0.0,
            },
            tape: match outer {
                "ubranie_specjalne" => Tape::Fire,
                "kurtka_ratownicza" => Tape::Silver,
                _ => Tape::None,
            },
            suit,
            tie: p.s("neckFaceType") == "krawat",
            collar: matches!(p.s("upperBaseType"), "koszula" | "polo") || suit,
            turtleneck: p.s("upperBaseType") == "golf" && !jacket,
            hood: mid == "bluza_kaptur" && !jacket,
            // pas: biały (drogówka), czarny służbowy; zwykły pasek w kolorze półbutów albo ciemny
            belt: match gear {
                "pas_bialy" => Some(MatKey::Detail),
                "pas_sluzbowy" | "pas_narzedziowy" => Some(MatKey::Face),
                _ if !jacket && mid != "marynarka" && !matches!(lower, "dres" | "spodnica") => Some(if matches!(p.s("feetType"), "polbuty" | "botki") { MatKey::Shoes } else { MatKey::Face }),
                _ => None,
            },
            duty_belt: matches!(gear, "pas_sluzbowy" | "pas_bialy" | "pas_narzedziowy"),
            police: outer == "kurtka_mundurowa" && p.s("vest") != "brak",
            vest_dark: matches!(p.s("vest"), "kamizelka_policji" | "kam_niebieska"),
            medic: outer == "kurtka_ratownicza",
            backpack: carry == "plecak",
            bag: matches!(carry, "torba" | "torba_medyczna"),
            scba: carry == "aparat_oddechowy",
            scarf: p.s("neckFaceType") == "szalik",
        }
    }
}

/// Kolor akcentu (`Detail`): krawat, biały pas, żółta taśma, torba/plecak.
pub(crate) fn detail_color<'a>(p: &'a P) -> &'a str {
    if p.s("neckFaceType") == "krawat" {
        p.s("neckFaceColor")
    } else if p.s("gear") == "pas_bialy" {
        "#F2F2EE"
    } else if p.s("outerType") == "ubranie_specjalne" {
        "#DFF020"
    } else if matches!(p.s("carryType"), "plecak" | "torba" | "torba_medyczna") {
        p.s("carryColor")
    } else {
        p.s("neckFaceColor")
    }
}

// ------------------------------------------------------------------ tułów

/// Przekrój tułowia: wysokość, półszerokość, głębokość przodu/tyłu, środek w Z, szerokość płaskiego przodu/tyłu.
#[derive(Clone)]
struct Ts {
    y: f64,
    w: f64,
    f: f64,
    b: f64,
    cz: f64,
    af: f64,
    ab: f64,
    wt: Wt,
}

impl Ts {
    fn lerp(a: &Ts, b: &Ts, t: f64) -> Ts {
        let l = |x: f64, y: f64| x + (y - x) * t;
        Ts { y: l(a.y, b.y), w: l(a.w, b.w), f: l(a.f, b.f), b: l(a.b, b.b), cz: l(a.cz, b.cz), af: l(a.af, b.af), ab: l(a.ab, b.ab), wt: wmix(&a.wt, &b.wt, t) }
    }
    /// 8 wierzchołków przeciwnie do ruchu wskazówek wokół +Y (przód → lewa → tył → prawa).
    fn ring(&self, off: f64) -> Ring {
        let (w, f, b) = (self.w + off, self.f + off, self.b + off);
        let (cf, cb) = (0.45, 0.4);
        let pts = [(self.af * w, f), (w, cf * f), (w, -cb * b), (self.ab * w, -b), (-self.ab * w, -b), (-w, -cb * b), (-w, cf * f), (-self.af * w, f)];
        Ring::new(pts.iter().map(|&(x, z)| [x, self.y, z + self.cz]).collect(), self.wt.clone())
    }
}

pub(crate) struct Body<'a> {
    sk: &'a [Joint],
    sc: &'a Scale,
    rig: Rig<'a>,
    fit: Fit,
    ts: Vec<Ts>,
    fat: f64,
    male: f64,
    adult: f64,
}

fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl<'a> Body<'a> {
    pub fn new(sk: &'a [Joint], sc: &'a Scale, p: &P, w: &'a [Xform]) -> Body<'a> {
        let age = p.f("age");
        // BMI względem normy wieku (dziecko 16, dorosły 22,5)
        let norm_bmi = 16.0 + 6.5 * ((age - 9.0) / 9.0).clamp(0.0, 1.0);
        let fat = ((p.f("bmi") - norm_bmi) / 8.0).clamp(-0.8, 2.0);
        let adult = ((age - 11.0) / 6.0).clamp(0.0, 1.0);
        let male = 0.5 + (smooth(p.f("sex")) - 0.5) * adult;
        let mut b = Body { sk, sc, rig: Rig { w }, fit: Fit::new(p), ts: vec![], fat, male, adult };
        b.ts = b.torso_stations();
        b
    }

    fn j(&self, n: &str) -> &Joint {
        &self.sk[idx(self.sk, n)]
    }
    fn i(&self, n: &str) -> usize {
        idx(self.sk, n)
    }

    fn torso_stations(&self) -> Vec<Ts> {
        let k = self.sc.sv;
        let (fat, fem, ad) = (self.fat, 1.0 - self.male, self.adult);
        let bust = fem * ad;
        let (kw, kh) = (self.sc.kw, self.sc.kh);
        let km = (kw + kh) / 2.0;
        let bulk = self.fit.bulk;
        let hj = self.j("LeftUpperLeg").head[1];
        let (hy, sy, cy, uy, ny, ay) = (self.j("Hips").head[1], self.j("Spine").head[1], self.j("Chest").head[1], self.j("UpperChest").head[1], self.j("Neck").head[1], self.j("LeftUpperArm").head[1]);
        let (hips, spine, chest, upper, neck) = (self.i("Hips"), self.i("Spine"), self.i("Chest"), self.i("UpperChest"), self.i("Neck"));
        // (y, półszer., przód, tył, środek z, ułamek płaskiego przodu/tyłu, wagi); bulk = grubość okrycia
        let mk = |y: f64, w: f64, f: f64, b: f64, cz: f64, wt: Wt, cloth: bool| {
            let g = if cloth { bulk } else { 1.0 };
            let o = (g - 1.0) * 0.12;
            Ts { y, w: w * k + o * k, f: f * k + o * k, b: b * k + o * k, cz: cz * k, af: 0.55 - 0.08 * bust, ab: 0.6, wt }
        };
        vec![
            mk(hj - 0.075 * k, 0.122 * kh * (1.0 + 0.12 * fat), 0.07 + 0.01 * fat, 0.085 + 0.012 * fem, -0.008, w1(hips), false),
            mk(hj + 0.005 * k, (0.163 + 0.014 * fem) * kh * (1.0 + 0.12 * fat), 0.083 + 0.02 * fat, 0.1 + 0.016 * fem + 0.02 * fat, -0.008, w1(hips), false),
            mk(hy + 0.07 * k, (0.152 + 0.004 * fem) * km * (1.0 + 0.16 * fat), 0.092 + 0.05 * fat, 0.088, -0.002, w1(hips), true),
            mk(sy + 0.06 * k, (0.142 - 0.016 * fem * ad) * kw * (1.0 + 0.18 * fat), 0.092 + 0.07 * fat, 0.08, 0.004, w1(spine), true),
            mk(cy, (0.148 - 0.012 * fem * ad) * kw * (1.0 + 0.13 * fat), 0.098 + 0.05 * fat + 0.008 * bust, 0.086, 0.004, w2(spine, chest, 0.5), true),
            mk(cy + 0.075 * k, (0.16 - 0.014 * fem * ad) * kw * (1.0 + 0.1 * fat), 0.104 + 0.03 * bust + 0.025 * fat, 0.092, 0.004, w1(chest), true),
            mk(uy + 0.035 * k, (0.168 - 0.016 * fem * ad) * kw * (1.0 + 0.07 * fat), 0.094 + 0.012 * bust, 0.098, 0.0, w2(chest, upper, 0.7), true),
            mk(ay + 0.026 * k, (0.176 - 0.016 * fem * ad) * kw, 0.056, 0.07, -0.006, w1(upper), true),
            mk(ny + 0.046 * k, 0.064 - 0.006 * fem, 0.044, 0.056, -0.014, w2(upper, neck, 0.35), true),
        ]
    }

    fn torso_at(&self, y: f64) -> Ts {
        let s = &self.ts;
        let i = s.windows(2).position(|w| y <= w[1].y).unwrap_or(s.len() - 2);
        Ts::lerp(&s[i], &s[i + 1], ((y - s[i].y) / (s[i + 1].y - s[i].y)).clamp(0.0, 1.0))
    }

    /// Punkt na powierzchni tułowia na wysokości `y`: `u` = łuk od środka przodu (`front`) lub tyłu w stronę +X.
    fn surf(&self, y: f64, u: f64, off: f64, front: bool) -> (V3, V3, Wt) {
        let t = self.torso_at(y);
        let r = t.ring(off);
        // ścieżka od środka przodu/tyłu w stronę +X i −X
        let (start, seq_pos, seq_neg): (V3, [usize; 4], [usize; 4]) = if front {
            (scale(add(r.pts[0], r.pts[7]), 0.5), [0, 1, 2, 3], [7, 6, 5, 4])
        } else {
            (scale(add(r.pts[3], r.pts[4]), 0.5), [3, 2, 1, 0], [4, 5, 6, 7])
        };
        let seq = if u >= 0.0 { seq_pos } else { seq_neg };
        let ccw = front == (u >= 0.0);
        let mut left = u.abs();
        let mut a = start;
        for &k in &seq {
            let b = r.pts[k];
            let l = len(sub(b, a));
            if left <= l || k == seq[3] {
                let d = norm(sub(b, a));
                let p = add(a, scale(d, left.min(l)));
                let n = norm(cross(d, [0.0, 1.0, 0.0]));
                let n = if ccw { n } else { scale(n, -1.0) };
                return (p, n, t.wt.clone());
            }
            left -= l;
            a = b;
        }
        unreachable!()
    }

    pub fn emit(&self, mb: &mut MeshBuilder) {
        self.torso(mb);
        self.neck(mb);
        for (s, sx) in [("Left", 1.0), ("Right", -1.0)] {
            self.arm(mb, s, sx);
            self.leg(mb, s, sx);
            self.foot(mb, s);
        }
        if self.fit.coat || self.fit.skirt {
            self.skirt(mb);
        }
        self.clothes(mb);
    }

    pub fn hands(&self, mb: &mut MeshBuilder, w: &[Xform]) {
        for (s, sx) in [("Left", 1.0), ("Right", -1.0)] {
            self.hand(mb, w, s, sx);
        }
    }

    fn torso(&self, mb: &mut MeshBuilder) {
        let rings: Vec<Ring> = self.ts.iter().map(|t| t.ring(0.0)).collect();
        let f = &self.fit;
        let long = f.coat;
        let mats: Vec<MatKey> = (0..rings.len() - 1)
            .map(|i| match i {
                0 => if long { MatKey::Outer } else { MatKey::Bottom },
                1 => f.hips,
                _ => f.torso,
            })
            .collect();
        loft(mb, &self.rig, &rings, |i, _| mats[i]);
        // krocze zasłaniają uda, górny otwór — szyja
    }

    fn neck(&self, mb: &mut MeshBuilder) {
        let k = self.sc.sv;
        let r = (0.054 - 0.007 * (1.0 - self.male)) * k * (1.0 + 0.1 * self.fat);
        let (ny, hy) = (self.j("Neck").head[1], self.j("Head").head[1]);
        let (upper, head) = (self.i("UpperChest"), self.i("Head"));
        let ring = |y: f64, rd: f64, rw: f64, cz: f64, wt: Wt| Ring::new(ring8([0.0, y, cz * k], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], rd, rw), wt);
        let top = hy + 0.075 * self.sc.hh / 0.237;
        let rings = vec![
            ring(ny - 0.01 * k, r * 1.05, r * 1.12, -0.016, w1(upper)),
            ring(top, r * 0.92, r * 0.95, -0.02, w1(head)),
        ];
        let f = &self.fit;
        loft(mb, &self.rig, &rings, |i, _| if i == 0 && (f.turtleneck || f.scarf) { f.torso } else { MatKey::Skin });
        let cover = if f.scarf { Some((MatKey::Detail, 0.05, 0.012)) } else if f.turtleneck { Some((MatKey::Top, 0.075, 0.006)) } else if f.collar { Some((MatKey::Top, 0.035, 0.005)) } else if f.jacket { Some((f.torso, 0.03, 0.007)) } else { None };
        if let Some((mat, h, off)) = cover {
            let lo = Ring::lerp(&rings[0], &rings[1], 0.15).grow(off * k);
            let hi = Ring::lerp(&rings[0], &rings[1], 0.15 + h * k / (rings[1].pts[0][1] - rings[0].pts[0][1])).grow((off + 0.003) * k);
            loft(mb, &self.rig, &[lo, hi], |_, _| mat);
        }
    }

    // ------------------------------------------------------------------ kończyny

    /// Pierścienie kończyny: (kość, ułamek długości, promień przód-tył, promień w bok, przesunięcie ku przodowi, wagi).
    fn limb(&self, sts: &[(usize, f64, f64, f64, f64, Wt)], hint: V3) -> Vec<Ring> {
        sts.iter()
            .map(|(b, f, rd, rw, sd, wt)| {
                let j = &self.sk[*b];
                let d = sub(j.tail, j.head);
                let (ex, ey, _) = frame(d, hint);
                let c = add(add(j.head, scale(d, *f)), scale(ex, *sd));
                Ring::new(ring8(c, ex, ey, *rd, *rw), wt.clone())
            })
            .collect()
    }

    fn arm(&self, mb: &mut MeshBuilder, s: &str, _sx: f64) {
        let k = self.sc.sv * (1.0 + 0.12 * self.fat) * (1.0 + 0.08 * (self.male - 0.5));
        let (sh, ua, la, hd) = (self.i(&format!("{s}Shoulder")), self.i(&format!("{s}UpperArm")), self.i(&format!("{s}LowerArm")), self.i(&format!("{s}Hand")));
        let f = &self.fit;
        let g = if f.sleeve == MatKey::Top && !f.short { 1.0 + (f.bulk - 1.0) * 0.5 } else { f.bulk };
        let c = |r: f64| r * k * g;
        let a = |r: f64| r * k;
        let mut sts = vec![
            (ua, -0.1, c(0.045), c(0.03), 0.0, w1(sh)),
            (ua, 0.1, c(0.06), c(0.058), 0.0, w1(ua)),
            (ua, 0.45, c(0.06), c(0.054), 0.002, w1(ua)),
        ];
        let mut mats = vec![f.sleeve, f.sleeve];
        if f.short {
            sts.push((ua, 0.47, a(0.054), a(0.049), 0.002, w1(ua)));
            mats.push(f.sleeve);
        }
        let skin = if f.short { MatKey::Skin } else { f.sleeve };
        sts.extend([(ua, 1.0, c(0.044), c(0.046), 0.0, w2(ua, la, 0.5)), (la, 0.22, c(0.05), c(0.052), 0.004, w1(la)), (la, 0.98, c(0.03), c(0.036), 0.0, w2(la, hd, 0.2))]);
        mats.extend([skin, skin, skin]);
        let rings = self.limb(&sts, [0.0, 0.0, 1.0]);
        loft(mb, &self.rig, &rings, |i, _| mats[i]);
        // nadgarstek chowa się w dłoni — bez zaślepki
        // taśmy na rękawie (przedramię)
        if f.tape != Tape::None {
            let at = |t: f64| Ring::lerp(&rings[rings.len() - 2], &rings[rings.len() - 1], t);
            self.band(mb, &at(0.42), &at(0.62), 0.004 * k, if f.tape == Tape::Fire { MatKey::Detail } else { MatKey::Reflective });
            if f.tape == Tape::Fire {
                self.band(mb, &at(0.48), &at(0.56), 0.007 * k, MatKey::Reflective);
            }
        }
    }

    /// Pas (taśma, mankiet) między dwoma pierścieniami, odsunięty o `off`.
    fn band(&self, mb: &mut MeshBuilder, a: &Ring, b: &Ring, off: f64, mat: MatKey) {
        loft(mb, &self.rig, &[a.grow(off), b.grow(off)], |_, _| mat);
    }

    fn leg(&self, mb: &mut MeshBuilder, s: &str, _sx: f64) {
        let k = self.sc.sv;
        let (fat, fem) = (self.fat, (1.0 - self.male) * self.adult);
        let (hips, ul, ll, ft) = (self.i("Hips"), self.i(&format!("{s}UpperLeg")), self.i(&format!("{s}LowerLeg")), self.i(&format!("{s}Foot")));
        let f = &self.fit;
        let g = |r: f64| r * k * (1.0 + 0.16 * fat);
        let gs = |r: f64| r * k * (1.0 + 0.08 * fat);
        let mut sts = vec![
            (ul, -0.02, g(0.1 + 0.006 * fem), g(0.098 + 0.012 * fem), -0.004 * k, w2(hips, ul, 0.55)),
            (ul, 0.5, g(0.08 + 0.004 * fem), g(0.077 + 0.006 * fem), 0.004 * k, w1(ul)),
            (ul, 1.0, gs(0.057), gs(0.054), 0.004 * k, w2(ul, ll, 0.5)),
            (ll, 0.28, gs(0.064), gs(0.055), -0.012 * k, w1(ll)),
            (ll, 0.82, gs(0.04), gs(0.036), -0.004 * k, w1(ll)),
            (ll, 1.0, gs(0.037), gs(0.034), 0.0, w2(ll, ft, 0.4)),
        ];
        let mut mats = vec![MatKey::Bottom; 5];
        if f.cut_legs {
            // szorty / spódnica: nogawka kończy się nad kolanem
            sts.insert(1, (ul, 0.42, g(0.083), g(0.08), 0.004 * k, w1(ul)));
            mats = vec![if f.skirt { MatKey::Skin } else { MatKey::Bottom }, MatKey::Skin, MatKey::Skin, MatKey::Skin, MatKey::Skin, MatKey::Skin];
        }
        if f.boot > 0.0 {
            // cholewka od ułamka `boot` goleni (licząc od kostki)
            let fb = 1.0 - f.boot;
            let fl = |x: &(usize, f64, f64, f64, f64, Wt)| if x.0 == ll { x.1 } else { x.1 - 1.0 };
            let pos = sts.iter().position(|x| fl(x) > fb).unwrap();
            let (a, b) = (&sts[pos - 1], &sts[pos]);
            let t = (fb - fl(a)) / (fl(b) - fl(a));
            let l = |x: f64, y: f64| x + (y - x) * t;
            sts.insert(pos, (ll, fb, l(a.2, b.2) * 1.1, l(a.3, b.3) * 1.1, l(a.4, b.4), w1(ll)));
            mats.insert(pos - 1, mats[pos - 1]);
            for m in mats.iter_mut().skip(pos) {
                *m = MatKey::Shoes;
            }
        }
        let rings = self.limb(&sts, [0.0, 0.0, 1.0]);
        loft(mb, &self.rig, &rings, |i, _| mats[i]);
        if f.tape != Tape::None && !f.cut_legs {
            let n = sts.iter().position(|x| x.0 == ll && x.1 > 0.5).unwrap();
            let at = |t: f64| Ring::lerp(&rings[n - 1], &rings[n], t);
            let gk = 0.004 * k;
            self.band(mb, &at(0.45), &at(0.75), gk, if f.tape == Tape::Fire { MatKey::Detail } else { MatKey::Reflective });
            if f.tape == Tape::Fire {
                self.band(mb, &at(0.54), &at(0.66), gk * 1.8, MatKey::Reflective);
            }
        }
    }

    /// But: przekrój z płaską podeszwą (osobny materiał), od pięty przez kostkę do czubka.
    fn foot(&self, mb: &mut MeshBuilder, s: &str) {
        let k = self.sc.sv;
        let (fi, ti) = (self.i(&format!("{s}Foot")), self.i(&format!("{s}Toes")));
        let fh = self.sk[fi].head;
        let [(_, heel), (_, ball), (_, tip)] = sole_points(self.sk, self.sc, s);
        let f = &self.fit;
        let chunky = if f.boot > 0.0 { 1.08 } else { 1.0 };
        let sole = if f.boot > 0.0 { 0.026 } else { 0.022 } * k;
        // (z, półszerokość, wysokość cholewki, wagi)
        let st = [
            (heel[2], 0.031, 0.07, w1(fi)),
            (fh[2] - 0.012 * k, 0.037, if f.boot > 0.0 { 0.13 } else { 0.105 }, w1(fi)),
            (ball[2], 0.047, 0.05, w2(fi, ti, 0.5)),
            (tip[2] - 0.004 * k, 0.037, 0.032, w1(ti)),
        ];
        let x = fh[0];
        let rings: Vec<Ring> = st
            .iter()
            .map(|(z, w, h, wt)| {
                let (w, h) = (w * k * chunky, h * k);
                // przeciwnie do ruchu wskazówek wokół +Z: lewy dół → lewa podeszwa → góra → prawa → prawy dół
                let pts = [(w, 0.0), (w, sole), (0.55 * w, h), (-0.55 * w, h), (-w, sole), (-w, 0.0)];
                Ring::new(pts.iter().map(|&(dx, y)| [x + dx, y, *z]).collect(), wt.clone())
            })
            .collect();
        let shoe = MatKey::Shoes;
        loft(mb, &self.rig, &rings, |_, seg| if matches!(seg, 0 | 4 | 5) { MatKey::Face } else { shoe });
        cap(mb, &self.rig, &rings[0], shoe, [0.0, 0.0, -1.0]);
        cap(mb, &self.rig, rings.last().unwrap(), shoe, [0.0, 0.0, 1.0]);
    }

    /// Dłoń: śródręcze (4-boczne) + 15 członów palców, każdy przypięty do swojej kości.
    fn hand(&self, mb: &mut MeshBuilder, w: &[Xform], s: &str, sx: f64) {
        let k = self.sc.sv * (1.0 + 0.06 * (self.male - 0.5)) * (1.0 + 0.05 * self.fat);
        let hd = self.i(&format!("{s}Hand"));
        let h = &self.sk[hd];
        let ex = [sx, 0.0, 0.0];
        // przekrój prostokątny: szerokość wzdłuż Z (kciuk +Z), grubość wzdłuż Y
        let rect = |x: f64, hw: f64, ht: f64, cz: f64, cy: f64| -> Vec<V3> {
            let c = add(h.head, [sx * x * k, cy * k, cz * k]);
            // obieg przeciwnie do ruchu wskazówek wokół kierunku palców (sx·X)
            let pts = [(ht, -hw), (ht, hw), (-ht, hw), (-ht, -hw)];
            let mut v: Vec<V3> = pts.iter().map(|&(y, z)| add(c, [0.0, y * k, z * k])).collect();
            if sx < 0.0 {
                v.reverse();
            }
            v
        };
        let rings = [rect(-0.01, 0.03, 0.017, 0.002, -0.002), rect(0.094, 0.041, 0.012, 0.0, -0.002)];
        let mut b = MeshBuilder::default();
        b.loft(MatKey::Skin, &rings, false);
        b.poly(MatKey::Skin, &rings[1], Some(ex));
        mb.push(&b.build(vec![]), &w[hd]);
        for f in FINGERS {
            for (n, ph) in PHALANGES.iter().enumerate() {
                let bi = self.i(&format!("{s}{f}{ph}"));
                let j = &self.sk[bi];
                let (hw, ht) = match (f, n) {
                    ("Thumb", 0) => (0.0125, 0.012),
                    ("Thumb", 1) => (0.0105, 0.0095),
                    ("Thumb", _) => (0.0095, 0.0082),
                    (_, 0) => (0.0092, 0.0085),
                    (_, 1) => (0.0085, 0.0077),
                    _ => (0.0078, 0.0068),
                };
                let sz = if f == "Little" { 0.86 } else { 1.0 } * k;
                let d = sub(j.tail, j.head);
                let (up, side, ez) = frame(d, if f == "Thumb" { [0.0, 1.0, -0.35] } else { [0.0, 1.0, 0.0] });
                let l = len(d);
                let start = add(j.head, scale(ez, -0.006 * k));
                let end = add(j.head, scale(ez, l + if n == 2 { 0.002 * k } else { 0.0 }));
                let tip = if n == 2 { 0.72 } else { 0.95 };
                let sec = |c: V3, s: f64| -> Vec<V3> {
                    [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].iter().map(|&(a, bb)| add(c, add(scale(up, a * ht * sz * s), scale(side, bb * hw * sz * s)))).collect()
                };
                let (r0, r1) = (sec(start, 1.0), sec(end, tip));
                let mut b = MeshBuilder::default();
                b.loft(MatKey::Skin, &[r0, r1.clone()], false);
                if n == 2 {
                    b.poly(MatKey::Skin, &r1, Some(ez));
                }
                mb.push(&b.build(vec![]), &w[bi]);
            }
        }
    }

    /// Poły płaszcza / spódnica: pierścienie od bioder do kolan, wierzchołki ważone udami wg strony.
    fn skirt(&self, mb: &mut MeshBuilder) {
        let k = self.sc.sv;
        let (hips, lu, ru) = (self.i("Hips"), self.i("LeftUpperLeg"), self.i("RightUpperLeg"));
        let top = self.ts[1].clone();
        let knee = self.j("LeftLowerLeg").head[1];
        let hj = self.j("LeftUpperLeg").head[1];
        let (len_, mat, flare) = if self.fit.coat { (hj - knee + 0.06 * k, MatKey::Outer, 1.0) } else { (0.75 * (hj - knee), MatKey::Bottom, 0.92) };
        let mk = |t: f64, grow: f64| -> Ring {
            let mut s = top.clone();
            s.y = hj - len_ * t;
            s.w = s.w.max(0.205 * k * self.sc.kh) * (1.0 + grow) * flare;
            // poły muszą zakryć uda (przód uda ok. 0,1 m przed stawem biodrowym)
            s.f = s.f.max(0.118 * k) + grow * 0.5 * k;
            s.b = s.b.max(0.118 * k) + grow * 0.3 * k;
            s.af = 0.6;
            let mut r = s.ring(0.004 * k);
            let wt: Vec<Wt> = r
                .pts
                .iter()
                .map(|p| {
                    let side = smooth(0.5 + p[0] / (0.2 * k));
                    let leg = 0.25 + 0.6 * t;
                    vec![(hips, 1.0 - leg), (lu, leg * side), (ru, leg * (1.0 - side))].into_iter().filter(|e| e.1 > 1e-6).collect()
                })
                .collect();
            r.wt = wt;
            r
        };
        // od dołu do góry: pierścienie obiegają +Y
        let rings = [mk(1.0, 0.06), mk(0.45, 0.03), mk(-0.05, 0.0)];
        loft(mb, &self.rig, &rings, |_, _| mat);
    }

    // ------------------------------------------------------------------ warstwy odzieży

    fn clothes(&self, mb: &mut MeshBuilder) {
        let k = self.sc.sv;
        let f = &self.fit;
        let ts = &self.ts;
        let y = |i: usize| ts[i].y;
        // pas spodni / pas służbowy
        if let Some(m) = f.belt {
            let (h, off) = if f.duty_belt { (0.026, 0.012) } else { (0.018, 0.005) };
            let c = y(2) - 0.004 * k;
            let (a, b) = (self.torso_at(c - h * k).ring(off * k), self.torso_at(c + h * k).ring(off * k));
            loft(mb, &self.rig, &[a, b], |_, _| m);
            if f.duty_belt {
                // kabura na prawym biodrze
                self.pouch(mb, c - 0.03 * k, -0.13 * k, 0.05 * k, 0.13 * k, 0.05 * k, off * k, MatKey::Face);
            } else {
                // klamra
                let (p, n, wt) = self.surf(c, 0.0, (off + 0.002) * k, true);
                let (dx, dy) = (0.018 * k, 0.014 * k);
                self.rig.poly(mb, MatKey::Reflective, &[add(p, [-dx, -dy, 0.0]), add(p, [dx, -dy, 0.0]), add(p, [dx, dy, 0.0]), add(p, [-dx, dy, 0.0])], &wt, n);
            }
        }
        // taśmy na kurtce (PRM, PSP)
        if f.tape != Tape::None {
            let base = if f.tape == Tape::Fire { MatKey::Detail } else { MatKey::Reflective };
            // pas na piersi; ratownik ma drugi w talii
            let bands: &[(f64, f64)] = if f.tape == Tape::Silver { &[(y(5) + 0.01 * k, 0.025), (y(1) + 0.035 * k, 0.025)] } else { &[(y(5) + 0.01 * k, 0.025)] };
            for &(c, h) in bands {
                let (a, b) = (self.torso_at(c - h * k).ring(0.004 * k), self.torso_at(c + h * k).ring(0.004 * k));
                loft(mb, &self.rig, &[a, b], |_, _| base);
                if f.tape == Tape::Fire {
                    let (a, b) = (self.torso_at(c - 0.4 * h * k).ring(0.007 * k), self.torso_at(c + 0.4 * h * k).ring(0.007 * k));
                    loft(mb, &self.rig, &[a, b], |_, _| MatKey::Reflective);
                }
            }
        }
        if f.medic {
            self.text_back(mb, &["RATOWNIK", "MEDYCZNY"], y(6) + 0.06 * k, 0.0056 * k, 0.005 * k, MatKey::Reflective);
        }
        if f.vest {
            self.vest(mb);
        }
        if f.suit || f.tie {
            self.shirt_front(mb);
        }
        if f.hood {
            self.hood(mb);
        }
        if f.backpack {
            self.backpack(mb);
        }
        if f.bag {
            self.bag(mb);
        }
        if f.scba {
            self.scba(mb);
        }
    }

    /// Prostopadłościan przyczepiony do powierzchni tułowia (kabura, ładownica, plecak).
    #[allow(clippy::too_many_arguments)]
    fn pouch(&self, mb: &mut MeshBuilder, yc: f64, u: f64, w: f64, h: f64, d: f64, off: f64, mat: MatKey) {
        let front = u.abs() < 0.2;
        let (p, n, wt) = self.surf(yc, u, off, front || u.abs() > 0.0);
        let side = norm(cross([0.0, 1.0, 0.0], n));
        let c = add(p, scale(n, d * 0.5 - 0.004));
        self.boxy(mb, mat, c, side, n, w, h, d, &wt);
    }

    #[allow(clippy::too_many_arguments)]
    fn boxy(&self, mb: &mut MeshBuilder, mat: MatKey, c: V3, side: V3, out: V3, w: f64, h: f64, d: f64, wt: &Wt) {
        let up = [0.0, 1.0, 0.0];
        let corner = |a: f64, b: f64, e: f64| add(c, add(add(scale(side, a * w * 0.5), scale(up, b * h * 0.5)), scale(out, e * d * 0.5)));
        for (n, q) in [
            (out, [corner(-1.0, -1.0, 1.0), corner(1.0, -1.0, 1.0), corner(1.0, 1.0, 1.0), corner(-1.0, 1.0, 1.0)]),
            (side, [corner(1.0, -1.0, 1.0), corner(1.0, -1.0, -1.0), corner(1.0, 1.0, -1.0), corner(1.0, 1.0, 1.0)]),
            (scale(side, -1.0), [corner(-1.0, -1.0, -1.0), corner(-1.0, -1.0, 1.0), corner(-1.0, 1.0, 1.0), corner(-1.0, 1.0, -1.0)]),
            // spód niewidoczny z kamery gry
            (up, [corner(-1.0, 1.0, 1.0), corner(1.0, 1.0, 1.0), corner(1.0, 1.0, -1.0), corner(-1.0, 1.0, -1.0)]),
        ] {
            self.rig.poly(mb, mat, &q, wt, n);
        }
    }

    /// Kamizelka: osobna warstwa nad okryciem, węższa na barkach (pachy), dwie poziome taśmy, napis POLICJA.
    fn vest(&self, mb: &mut MeshBuilder) {
        let k = self.sc.sv;
        let ts = &self.ts;
        let off = 0.011 * k;
        let lo = ts[2].y + 0.012 * k;
        let mut top = ts[7].clone();
        top.y += 0.02 * k;
        top.w *= 0.6;
        top.f += 0.004 * k;
        let st = [self.torso_at(lo), ts[3].clone(), ts[5].clone(), ts[6].clone(), top];
        let n = st.len();
        let rings: Vec<Ring> = st.iter().enumerate().map(|(i, t)| t.ring(if i + 1 == n { off + 0.01 * k } else { off })).collect();
        loft(mb, &self.rig, &rings, |_, _| MatKey::HiVis);
        for c in [ts[3].y + 0.005 * k, ts[5].y - 0.004 * k] {
            let h = 0.024 * k;
            let (a, b) = (self.torso_at(c - h).ring(off + 0.0025 * k), self.torso_at(c + h).ring(off + 0.0025 * k));
            loft(mb, &self.rig, &[a, b], |_, _| MatKey::Reflective);
        }
        if self.fit.police {
            let letters = if self.fit.vest_dark { MatKey::Reflective } else { MatKey::Outer };
            self.text_back(mb, &["POLICJA"], ts[6].y + 0.062 * k, 0.0068 * k, off + 0.003 * k, letters);
        }
    }

    fn text_back(&self, mb: &mut MeshBuilder, lines: &[&str], y_top: f64, cell: f64, off: f64, mat: MatKey) {
        for (li, line) in lines.iter().enumerate() {
            let cols = font::text_cols(line) as f64;
            let y0 = y_top - (li as f64 + 1.0) * 9.0 * cell;
            // czytane zza pleców: od +X ku −X
            let u = |c: f64| (cols / 2.0 - c) * cell;
            font::text(line, |c, r| self.surf(y0 + r * cell, u(c), off, false).0, |q, c, r| {
                let (_, n, wt) = self.surf(y0 + r * cell, u(c), off, false);
                self.rig.poly(mb, mat, &q, &wt, n);
            });
        }
    }

    /// Marynarka: wycięcie w serek z koszulą, klapy i krawat.
    fn shirt_front(&self, mb: &mut MeshBuilder) {
        let k = self.sc.sv;
        let ts = &self.ts;
        let (top, bot) = (ts[8].y - 0.004 * k, if self.fit.suit { ts[4].y + 0.02 * k } else { ts[8].y - 0.02 * k });
        let hw_top = 0.048 * k;
        let n_steps = 5;
        let ys: Vec<f64> = (0..=n_steps).map(|i| top + (bot - top) * i as f64 / n_steps as f64).collect();
        let hw = |y: f64| hw_top * ((y - bot) / (top - bot)).clamp(0.0, 1.0);
        if self.fit.suit {
            for w in ys.windows(2) {
                let (a, b) = (w[0], w[1]);
                let (p0, n, wt) = self.surf(a, -hw(a), 0.003 * k, true);
                let p1 = self.surf(a, hw(a), 0.003 * k, true).0;
                let p2 = self.surf(b, hw(b), 0.003 * k, true).0;
                let p3 = self.surf(b, -hw(b), 0.003 * k, true).0;
                self.rig.poly(mb, MatKey::Top, &[p0, p1, p2, p3], &wt, n);
                // klapy: pasy wzdłuż krawędzi wycięcia
                for s in [-1.0, 1.0] {
                    let lw = 0.024 * k;
                    let q = [self.surf(a, s * hw(a), 0.006 * k, true).0, self.surf(a, s * (hw(a) + lw), 0.005 * k, true).0, self.surf(b, s * (hw(b) + lw * 0.9), 0.005 * k, true).0, self.surf(b, s * hw(b), 0.006 * k, true).0];
                    self.rig.poly(mb, MatKey::Outer, &q, &wt, n);
                }
            }
        }
        if self.fit.tie {
            let tb = ts[3].y;
            let ys: Vec<f64> = (0..=4).map(|i| top - 0.01 * k + (tb - top) * i as f64 / 4.0).collect();
            for (i, w) in ys.windows(2).enumerate() {
                let (a, b) = (w[0], w[1]);
                let wa = if i == 0 { 0.012 } else { 0.016 + 0.008 * i as f64 / 3.0 } * k;
                let wb = 0.016 * k + 0.008 * k * (i + 1) as f64 / 3.0;
                let off = 0.006 * k;
                let (p0, n, wt) = self.surf(a, -wa, off, true);
                let q = [p0, self.surf(a, wa, off, true).0, self.surf(b, wb, off, true).0, self.surf(b, -wb, off, true).0];
                self.rig.poly(mb, MatKey::Detail, &q, &wt, n);
            }
        }
    }

    fn hood(&self, mb: &mut MeshBuilder) {
        let k = self.sc.sv;
        let t = &self.ts[7];
        let wt = t.wt.clone();
        let c = [0.0, t.y + 0.01 * k, t.cz - t.b - 0.01 * k];
        let r = |dy: f64, w: f64, d: f64, dz: f64| Ring::new(ring8(add(c, [0.0, dy * k, dz * k]), [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], d * k, w * k), wt.clone());
        let rings = [r(-0.07, 0.06, 0.02, 0.012), r(-0.02, 0.1, 0.035, 0.0), r(0.04, 0.085, 0.03, 0.01)];
        loft(mb, &self.rig, &rings, |_, _| MatKey::Top);
        cap(mb, &self.rig, &rings[2], MatKey::Top, [0.0, 1.0, 0.0]);
    }

    fn backpack(&self, mb: &mut MeshBuilder) {
        let k = self.sc.sv;
        let yc = (self.ts[4].y + self.ts[6].y) / 2.0;
        let (p, n, wt) = self.surf(yc, 0.0, 0.004 * k, false);
        let side = norm(cross([0.0, 1.0, 0.0], n));
        self.boxy(mb, MatKey::Detail, add(p, scale(n, 0.055 * k)), side, n, 0.27 * k, 0.32 * k, 0.11 * k, &wt);
        // szelki na barkach
        for s in [-1.0, 1.0] {
            let u = 0.075 * k;
            let pts = [self.surf(self.ts[6].y + 0.03 * k, s * u, 0.014 * k, true), self.surf(self.ts[5].y, s * u * 1.05, 0.014 * k, true), self.surf(self.ts[4].y - 0.02 * k, s * u * 1.1, 0.014 * k, true)];
            let sw = [0.02 * k, 0.0, 0.0];
            for w in pts.windows(2) {
                let (a, b) = (&w[0], &w[1]);
                self.rig.poly(mb, MatKey::Detail, &[sub(a.0, sw), add(a.0, sw), add(b.0, sw), sub(b.0, sw)], &a.2, a.1);
            }
        }
    }

    fn bag(&self, mb: &mut MeshBuilder) {
        let k = self.sc.sv;
        // torba na prawym biodrze, pasek przez lewy bark
        let (p, n, wt) = self.surf(self.ts[1].y + 0.02 * k, -0.2 * k, 0.0, true);
        let side = norm(cross([0.0, 1.0, 0.0], n));
        self.boxy(mb, MatKey::Detail, add(p, scale(n, 0.05 * k)), side, n, 0.26 * k, 0.2 * k, 0.1 * k, &wt);
        let a = self.surf(self.ts[7].y + 0.005 * k, 0.07 * k, 0.008 * k, true);
        let b = self.surf(self.ts[2].y, -0.12 * k, 0.012 * k, true);
        let sw = [0.0, 0.022 * k, 0.0];
        self.rig.poly(mb, MatKey::Detail, &[sub(a.0, sw), add(a.0, sw), add(b.0, sw), sub(b.0, sw)], &a.2, a.1);
    }

    /// Aparat oddechowy: płyta nośna + butla na plecach.
    fn scba(&self, mb: &mut MeshBuilder) {
        let k = self.sc.sv;
        let yc = (self.ts[2].y + self.ts[6].y) / 2.0;
        let (p, n, wt) = self.surf(yc, 0.0, 0.004 * k, false);
        let side = norm(cross([0.0, 1.0, 0.0], n));
        self.boxy(mb, MatKey::Face, add(p, scale(n, 0.012 * k)), side, n, 0.22 * k, 0.42 * k, 0.025 * k, &wt);
        let c = add(p, scale(n, 0.1 * k));
        let r = 0.075 * k;
        let ring = |dy: f64, rr: f64| -> Ring {
            let pts: Vec<V3> = (0..6).map(|i| {
                let a = TAU * i as f64 / 6.0;
                add(add(c, [0.0, dy, 0.0]), add(scale(side, rr * a.cos()), scale(n, rr * a.sin())))
            }).collect();
            let mut pts = pts;
            if dot(cross(side, n), [0.0, 1.0, 0.0]) < 0.0 {
                pts.reverse();
            }
            Ring::new(pts, wt.clone())
        };
        let rings = [ring(-0.25 * k, r * 0.85), ring(0.17 * k, r), ring(0.24 * k, r * 0.6)];
        loft(mb, &self.rig, &rings, |_, _| MatKey::Reflective);
        cap(mb, &self.rig, &rings[0], MatKey::Reflective, [0.0, -1.0, 0.0]);
        cap(mb, &self.rig, &rings[2], MatKey::Reflective, [0.0, 1.0, 0.0]);
    }
}
