//! Ciało z pierścieni na kość (port segmentów `build.py`: kapsuły → loft przekrojów eliptycznych).
//! Każda część powstaje w pozie spoczynkowej i dostaje transformację kości z FK.

use crate::skel::{idx, Joint, Scale};
use crate::P;
use procedura_kit::mesh::{add, cross, dot, norm, scale, sub, len, MatKey, MeshBuilder, V3};
use std::f64::consts::TAU;

/// Przekrój: położenie wzdłuż osi (ułamek długości), półosie (`d` wzdłuż `ex`, `w` wzdłuż `ey`) i przesunięcie środka.
#[derive(Clone, Copy)]
pub(crate) struct St {
    pub f: f64,
    pub d: f64,
    pub w: f64,
    pub sd: f64,
}

pub(crate) fn st(f: f64, d: f64, w: f64) -> St {
    St { f, d, w, sd: 0.0 }
}

pub(crate) struct Part {
    pub bone: usize,
    pub origin: V3,
    pub dir: V3,
    pub hint: V3,
    pub len: f64,
    pub n: usize,
    pub st: Vec<St>,
    /// Materiał każdego przedziału między przekrojami.
    pub mats: Vec<MatKey>,
    pub cap0: bool,
    pub cap1: bool,
}

/// Prawoskrętny układ (ex, ey, ez=dir); pierścień obiega ez przeciwnie do ruchu wskazówek, więc loft ma normalne na zewnątrz.
pub(crate) fn frame(dir: V3, hint: V3) -> (V3, V3, V3) {
    let ez = norm(dir);
    let ex = norm(sub(hint, scale(ez, dot(hint, ez))));
    (ex, cross(ez, ex), ez)
}

pub(crate) fn emit(mb: &mut MeshBuilder, p: &Part) {
    let (ex, ey, ez) = frame(p.dir, p.hint);
    let rings: Vec<Vec<V3>> = p
        .st
        .iter()
        .map(|s| {
            let c = add(add(p.origin, scale(ez, s.f * p.len)), scale(ex, s.sd));
            (0..p.n)
                .map(|k| {
                    let a = TAU * k as f64 / p.n as f64;
                    add(c, add(scale(ex, s.d * a.cos()), scale(ey, s.w * a.sin())))
                })
                .collect()
        })
        .collect();
    mb.loft_with(&rings, None, |i, _| p.mats[i]);
    if p.cap0 {
        mb.poly(p.mats[0], &rings[0], Some(scale(ez, -1.0)));
    }
    if p.cap1 {
        mb.poly(*p.mats.last().unwrap(), rings.last().unwrap(), Some(ez));
    }
}

/// Wybory ubioru wpływające na bryłę i przydział slotów (kolory składa `lib.rs`).
pub(crate) struct Fit {
    pub torso: MatKey,
    pub hips: MatKey,
    pub sleeve: MatKey,
    pub forearm: MatKey,
    pub vest: bool,
    pub bulk: f64,
    pub cut_legs: bool,
    pub skirt: bool,
    pub boot: f64,
}

impl Fit {
    pub fn new(p: &P) -> Fit {
        let outer = p.s("outerType");
        let jacket = !matches!(outer, "brak" | "kamizelka");
        let vest = p.s("vest") != "brak";
        let short = matches!(p.s("upperBaseType"), "tshirt" | "polo" | "koszulka_ratownicza") && p.s("upperMidType") == "brak" && !jacket;
        let lower = p.s("lowerType");
        let sleeve = if jacket { MatKey::Outer } else { MatKey::Top };
        Fit {
            torso: if jacket || outer == "kamizelka" { MatKey::Outer } else { MatKey::Top },
            hips: if matches!(outer, "plaszcz" | "parka" | "puchowka") { MatKey::Outer } else { MatKey::Bottom },
            sleeve,
            forearm: if short { MatKey::Skin } else { sleeve },
            vest,
            bulk: match outer {
                "puchowka" => 1.14,
                "parka" => 1.10,
                "plaszcz" | "ubranie_specjalne" => 1.07,
                "brak" => 1.0,
                "kamizelka" => 1.03,
                _ => 1.05,
            } * match p.s("upperMidType") {
                "brak" => 1.0,
                _ => 1.03,
            },
            cut_legs: matches!(lower, "szorty" | "spodnica"),
            skirt: lower == "spodnica",
            boot: match p.s("feetType") {
                "botki" => 0.18,
                "buty_zimowe" => 0.32,
                "buty_robocze" | "buty_mundurowe" => 0.22,
                "buty_strazackie" => 0.5,
                _ => 0.0,
            },
        }
    }
}

/// Wszystkie części ciała poza głową (głowa, twarz i włosy: `head.rs`).
pub(crate) fn parts(sk: &[Joint], sc: &Scale, p: &P) -> Vec<Part> {
    let fit = Fit::new(p);
    let fat = ((p.f("bmi") - 23.5) / 10.0).clamp(-0.9, 1.6);
    let sex = p.f("sex");
    let (gd, gw, gl, gn) = (1.0 + 0.30 * fat, 1.0 + 0.22 * fat, 1.0 + 0.20 * fat, 1.0 + 0.12 * fat);
    let v = |n: &str| -> (usize, V3, V3, f64) {
        let i = idx(sk, n);
        let d = sub(sk[i].tail, sk[i].head);
        (i, sk[i].head, d, len(d))
    };
    let (z, y) = ([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]);
    let mut out = Vec::new();
    let put = |out: &mut Vec<Part>, n: &str, hint: V3, sides: usize, sts: Vec<St>, mats: Vec<MatKey>, cap0: bool, cap1: bool| {
        let (bone, origin, dir, l) = v(n);
        out.push(Part { bone, origin, dir, hint, len: l, n: sides, st: sts, mats, cap0, cap1 });
    };
    let k = sc.sv;
    // tułów: d = głębokość, w = szerokość; kw/kh skalują barki i biodra
    let waist = 1.0 - 0.06 * (1.0 - sex);
    let bust = 1.0 + 0.08 * (1.0 - sex);
    let t = |f: f64, d: f64, w: f64, wk: f64| st(f, d * k * gd * fit.bulk, w * k * gw * wk * fit.bulk);
    let (kh, kw) = (sc.kh, sc.kw);
    let km = (kh + kw) / 2.0;
    let torso = |bone: &str| if fit.vest && bone != "Hips" { MatKey::HiVis } else { fit.torso };

    put(&mut out, "Hips", z, 8, vec![t(-0.45, 0.095, 0.135, kh), t(0.0, 0.108, 0.155, kh), t(0.55, 0.098, 0.14, kh), t(1.0, 0.092, 0.128 * waist, km)], vec![fit.hips; 3], true, true);
    if fit.hips == MatKey::Bottom {
        // pasek: dwa pierścienie lekko nad spodniami
        let b = |f: f64, d: f64, w: f64, wk: f64| st(f, (d + 0.005) * k * gd, (w + 0.005) * k * gw * wk);
        put(&mut out, "Hips", z, 8, vec![b(0.6, 0.097, 0.139, kh), b(0.9, 0.094, 0.133 * waist, km)], vec![MatKey::Shoes], false, false);
    }
    put(&mut out, "Spine", z, 8, vec![t(0.0, 0.092, 0.128 * waist, km), t(0.5, 0.09, 0.122 * waist, km), t(1.0, 0.094, 0.14 * waist, km)], vec![torso("Spine"); 2], false, true);
    put(&mut out, "Chest", z, 8, vec![t(0.0, 0.094, 0.14 * waist, km), t(0.5, 0.108 * bust, 0.165, kw), t(1.0, 0.104 * bust, 0.18, kw)], vec![torso("Chest"); 2], false, true);
    put(&mut out, "UpperChest", z, 8, vec![t(0.0, 0.104 * bust, 0.18, kw), t(0.3, 0.097, 0.205, kw), t(0.62, 0.088, 0.195, kw), t(0.86, 0.07, 0.12, kw), t(1.0, 0.056, 0.085, kw)], vec![torso("UpperChest"); 4], false, true);
    let nk = |f: f64, r: f64| st(f, r * k * gn, r * k * gn);
    put(&mut out, "Neck", z, 6, vec![nk(0.0, 0.05), nk(1.15, 0.045)], vec![MatKey::Skin], false, false);

    for (s, sx) in [("Left", 1.0), ("Right", -1.0)] {
        let _ = sx;
        let a = |f: f64, r: f64| st(f, r * k * gl * fit.bulk, r * k * gl * fit.bulk);
        let sleeve_end = if fit.forearm == MatKey::Skin { MatKey::Skin } else { fit.sleeve };
        // krótki rękaw kończy się w połowie ramienia
        put(&mut out, &format!("{s}UpperArm"), z, 8, vec![a(0.0, 0.04), a(0.12, 0.056), a(0.5, 0.052), a(1.0, 0.045)], vec![fit.sleeve, fit.sleeve, sleeve_end], true, true);
        let f = |f: f64, r: f64| st(f, r * k * gl, r * k * gl);
        put(&mut out, &format!("{s}LowerArm"), z, 8, vec![f(0.0, 0.048), f(0.25, 0.05), f(1.0, 0.034)], vec![fit.forearm; 2], true, false);
        let h = |f: f64, d: f64, w: f64| st(f, d * k * 1.25, w * k * 1.2);
        put(&mut out, &format!("{s}Hand"), y, 6, vec![h(0.0, 0.02, 0.032), h(0.3, 0.022, 0.045), h(0.75, 0.016, 0.04), h(1.0, 0.01, 0.03)], vec![MatKey::Skin; 3], false, true);

        let cut = fit.cut_legs;
        let flare = if fit.skirt { 1.3 } else { 1.0 };
        let l = |f: f64, r: f64| st(f, r * k * gl, r * k * gl);
        let lw = |f: f64, r: f64, m: f64| st(f, r * k * gl * m, r * k * gl * m);
        let thigh_end = if cut { MatKey::Skin } else { MatKey::Bottom };
        put(&mut out, &format!("{s}UpperLeg"), z, 8, vec![l(-0.08, 0.075), l(0.0, 0.09), l(0.3, 0.088), lw(0.6, 0.074, flare), l(1.0, 0.058)], vec![MatKey::Bottom, MatKey::Bottom, MatKey::Bottom, thigh_end], true, true);
        let shin = if cut { MatKey::Skin } else { MatKey::Bottom };
        let mut sts = vec![l(0.0, 0.058), l(0.3, 0.064), l(1.0, 0.04)];
        let mut mats = vec![shin; 2];
        if fit.boot > 0.0 {
            sts = vec![l(0.0, 0.06), l(0.3, 0.063), lw(1.0 - fit.boot, 0.052, 1.0), lw(1.0 - fit.boot + 0.001, 0.056, 1.0), l(1.0, 0.05)];
            mats = vec![shin, shin, MatKey::Shoes, MatKey::Shoes];
        }
        put(&mut out, &format!("{s}LowerLeg"), z, 8, sts, mats, true, false);

        // stopa w poziomym układzie (przód +Z, ex = góra), spód dokładnie na gruncie
        let (fi, fh, _, _) = v(&format!("{s}Foot"));
        let (ti, th, _, _) = v(&format!("{s}Toes"));
        let heel = 0.055 * k;
        let ball = th[2] - fh[2];
        let o = [fh[0], 0.0, fh[2] - heel];
        let flen = ball + heel;
        let ft = |zz: f64, h: f64, w: f64| St { f: (zz * k + heel) / flen, d: h * k, w: w * k, sd: h * k };
        out.push(Part { bone: fi, origin: o, dir: [0.0, 0.0, 1.0], hint: y, len: flen, n: 6, st: vec![ft(-0.055, 0.042, 0.033), ft(0.0, 0.052, 0.036), ft(0.085, 0.033, 0.045), ft(ball / k, 0.025, 0.047)], mats: vec![MatKey::Shoes; 3], cap0: true, cap1: false });
        let tl = 0.07 * k;
        out.push(Part { bone: ti, origin: [th[0], 0.0, th[2]], dir: [0.0, 0.0, 1.0], hint: y, len: tl, n: 6, st: vec![ft_at(0.0, 0.025 * k, 0.047 * k), ft_at(1.0, 0.013 * k, 0.036 * k)], mats: vec![MatKey::Shoes], cap0: false, cap1: true });
    }
    out
}

fn ft_at(f: f64, h: f64, w: f64) -> St {
    St { f, d: h, w, sd: h }
}
