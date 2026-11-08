//! Głowa: czaszka z lofta, twarz z płatów (oczy, brwi, nos, usta, okulary), włosy jako skorupa
//! z bryłami (kok, kucyk, tafla), zarost i nakrycie głowy. Wszystko w kości `Head`, w pozie spoczynkowej.
//!
//! 8-boczne pierścienie mają przód na PŁASKIEJ ściance (wierzchołki ±22,5° od +Z), więc płaty
//! twarzy leżą na jednej płaszczyźnie i nie chowają się w załamaniu.

use crate::skel::{idx, Joint, Scale};
use crate::P;
use procedura_kit::mesh::{add, cross, norm, scale, sub, MatKey, MeshBuilder, V3};
use std::f64::consts::{FRAC_PI_4, FRAC_PI_8, TAU};

const N: usize = 8;
/// (ułamek wysokości, półgłębokość, półszerokość, przesunięcie środka w +Z) dla głowy 1,78 m.
const SKULL: [(f64, f64, f64, f64); 6] = [(0.0, 0.056, 0.038, 0.026), (0.13, 0.08, 0.07, 0.008), (0.36, 0.097, 0.09, 0.0), (0.62, 0.102, 0.09, 0.0), (0.88, 0.090, 0.077, -0.002), (1.0, 0.052, 0.047, -0.004)];

struct Prof {
    y0: f64,
    h: f64,
    k: f64,
}

fn theta(k: usize) -> f64 {
    FRAC_PI_8 + k as f64 * FRAC_PI_4
}

impl Prof {
    fn at(&self, f: f64) -> (f64, f64, f64, f64) {
        let f = f.clamp(0.0, 1.0);
        let i = SKULL.windows(2).position(|w| f <= w[1].0).unwrap_or(SKULL.len() - 2);
        let (a, b) = (SKULL[i], SKULL[i + 1]);
        let u = (f - a.0) / (b.0 - a.0);
        let l = |x: f64, y: f64| (x + (y - x) * u) * self.k;
        (l(a.1, b.1), l(a.2, b.2), l(a.3, b.3), self.y0 + f * self.h)
    }
    /// Punkt na powierzchni (kąt `th` od +Z ku lewej stronie +X), `grow` metrów na zewnątrz.
    fn pt(&self, f: f64, th: f64, grow: f64) -> V3 {
        let (d, w, cz, y) = self.at(f);
        let f = f.clamp(0.0, 1.0);
        [(w + grow) * th.sin(), y + grow * f.powi(3), cz + (d + grow) * th.cos()]
    }
    fn ring(&self, f: f64, grow: f64) -> Vec<V3> {
        (0..N).map(|k| self.pt(f, theta(k), grow)).collect()
    }
    /// Punkt na przedniej ściance: `u` ∈ [-1,1] w poprzek (prawa→lewa), `off` wzdłuż normalnej.
    fn front(&self, f: f64, u: f64, off: f64) -> V3 {
        let p = |f: f64| {
            let (r, l) = (self.pt(f, theta(N - 1), 0.0), self.pt(f, theta(0), 0.0));
            add(r, scale(sub(l, r), (u + 1.0) / 2.0))
        };
        let (r, l) = (self.pt(f, theta(N - 1), 0.0), self.pt(f, theta(0), 0.0));
        let df = sub(p((f + 0.01).min(1.0)), p((f - 0.01).max(0.0)));
        add(p(f), scale(norm(cross(sub(l, r), df)), off))
    }
}

struct Shell {
    front: f64,
    back: f64,
    tau: f64,
    m: usize,
    top: f64,
    flare: f64,
}

fn shell(front: f64, back: f64, tau: f64) -> Shell {
    Shell { front, back, tau, m: 3, top: 1.0, flare: 0.0 }
}

fn emit_shell(mb: &mut MeshBuilder, pr: &Prof, s: &Shell, mat: MatKey) {
    let rings: Vec<Vec<V3>> = (0..=s.m)
        .map(|i| {
            let t = i as f64 / s.m as f64;
            (0..N)
                .map(|k| {
                    let wk = ((theta(k).cos() - 0.1) / 0.8).clamp(0.0, 1.0);
                    let low = s.back + (s.front - s.back) * wk;
                    pr.pt(low + (s.top - low) * t, theta(k), s.tau + s.flare * (1.0 - t) * (1.0 - wk))
                })
                .collect()
        })
        .collect();
    mb.loft(mat, &rings, false);
    if s.top >= 1.0 {
        mb.poly(mat, rings.last().unwrap(), Some([0.0, 1.0, 0.0]));
    }
}

/// Pierścień 6-boczny w płaszczyźnie XZ, obiegany przeciwnie do ruchu wskazówek wokół kierunku wzrostu lofta.
fn ring6(c: V3, rx: f64, rz: f64, down: bool) -> Vec<V3> {
    (0..6).map(|k| {
        let a = (if down { -1.0 } else { 1.0 }) * TAU * k as f64 / 6.0;
        [c[0] + rx * a.sin(), c[1], c[2] + rz * a.cos()]
    }).collect()
}

fn hair_shell(style: &str) -> Option<Shell> {
    Some(match style {
        "buzz" => Shell { m: 2, ..shell(0.66, 0.50, 0.004) },
        "fade" => shell(0.64, 0.38, 0.006),
        "krotkie_klasyczne" | "kok" | "kucyk" => Shell { m: 4, ..shell(0.70, 0.30, 0.02) },
        "medium" | "dlugie" => shell(0.62, 0.12, 0.018),
        "bob" => Shell { flare: 0.012, ..shell(0.62, 0.0, 0.02) },
        "lysiejacy" => Shell { top: 0.62, ..shell(0.62, 0.30, 0.012) },
        _ => return None,
    })
}

/// (skorupa, długość daszka m, brzeg tylny)
fn hat(kind: &str) -> Option<(Shell, f64)> {
    Some(match kind {
        "czapka_daszek" => (shell(0.60, 0.45, 0.02), 0.07),
        "czapka_mundurowa" | "czapka_policji_rd" => (shell(0.59, 0.50, 0.026), 0.06),
        "kaszkiet" => (shell(0.58, 0.50, 0.016), 0.045),
        "czapka_zimowa" => (shell(0.52, 0.28, 0.022), 0.0),
        "beret" => (Shell { flare: 0.02, ..shell(0.62, 0.55, 0.03) }, 0.0),
        "kaptur" => (Shell { flare: 0.015, ..shell(0.40, 0.02, 0.04) }, 0.0),
        "kask_budowlany" => (shell(0.58, 0.52, 0.03), 0.04),
        "helm_psp" => (shell(0.55, 0.20, 0.034), 0.03),
        _ => return None,
    })
}

pub(crate) fn build(mb: &mut MeshBuilder, sk: &[Joint], sc: &Scale, p: &P, hs: f64) {
    let hd = &sk[idx(sk, "Head")];
    let l = hd.tail[1] - hd.head[1];
    // głowa rośnie w dół (do szyi), żeby czubek zostawał w `height`
    let pr = Prof { y0: hd.tail[1] - l * hs, h: l * hs, k: sc.sv * hs * 1.4 };

    let skull: Vec<Vec<V3>> = SKULL.iter().map(|s| pr.ring(s.0, 0.0)).collect();
    mb.loft(MatKey::Skin, &skull, false);
    mb.poly(MatKey::Skin, &skull[0], Some([0.0, -1.0, 0.0]));
    mb.poly(MatKey::Skin, skull.last().unwrap(), Some([0.0, 1.0, 0.0]));

    // --- twarz
    let patch = |mb: &mut MeshBuilder, mat: MatKey, u0: f64, u1: f64, f0: f64, f1: f64, off: f64| {
        let q = [pr.front(f0, u0, off), pr.front(f0, u1, off), pr.front(f1, u1, off), pr.front(f1, u0, off)];
        let want = pr.front(f0, u0, 1.0);
        mb.quad(mat, q[0], q[1], q[2], q[3], Some(sub(want, pr.front(f0, u0, 0.0))));
    };
    // wielokąt płata we współrzędnych (u, f) na przedniej ściance
    let poly = |mb: &mut MeshBuilder, mat: MatKey, pts: &[(f64, f64)], off: f64| {
        let v: Vec<V3> = pts.iter().map(|&(u, f)| pr.front(f, u, off)).collect();
        let n = sub(pr.front(pts[0].1, pts[0].0, 1.0), pr.front(pts[0].1, pts[0].0, 0.0));
        mb.poly(mat, &v, Some(n));
    };
    for s in [-1.0f64, 1.0] {
        // oko: migdał; brew: równoległobok opadający ku nosowi
        let (c, w, h, f0) = (s * 0.5, 0.25, 0.034, 0.525);
        poly(mb, MatKey::Face, &[(c - w, f0), (c - w * 0.45, f0 + h), (c + w * 0.5, f0 + h * 1.1), (c + w, f0 + 0.004), (c + w * 0.5, f0 - h * 0.8), (c - w * 0.45, f0 - h * 0.9)], 0.002);
        let (i, o) = (s * 0.2, s * 0.82);
        poly(mb, MatKey::Hair, &[(i, 0.592), (o, 0.606), (o, 0.628), (i, 0.614)], 0.003);
        // nozdrze
        poly(mb, MatKey::Face, &[(s * 0.06, 0.322), (s * 0.15, 0.322), (s * 0.15, 0.337), (s * 0.06, 0.337)], 0.011);
    }
    // usta: łuk górnej wargi (ciemny) i wystająca dolna warga (skóra)
    poly(mb, MatKey::Face, &[(-0.4, 0.226), (-0.14, 0.24), (0.0, 0.234), (0.14, 0.24), (0.4, 0.226), (0.14, 0.208), (-0.14, 0.208)], 0.002);
    // nos: grzbiet, czubek i skrzydełka (5 trójkątów)
    let (top, mid, tip) = (pr.front(0.50, 0.0, 0.006), pr.front(0.42, 0.0, 0.017 * sc.sv), pr.front(0.355, 0.0, 0.032 * sc.sv));
    let (nl, nr) = (pr.front(0.34, 0.2, 0.012), pr.front(0.34, -0.2, 0.012));
    let inside = pr.front(0.42, 0.0, -0.04);
    for t in [vec![top, nl, tip, mid], vec![top, mid, tip, nr], vec![tip, nl, nr]] {
        let c = scale(t.iter().fold([0.0; 3], |a, &q| add(a, q)), 1.0 / t.len() as f64);
        mb.poly(MatKey::Skin, &t, Some(sub(c, inside)));
    }
    match p.s("glasses") {
        "przeciwsloneczne" => {
            for s in [-1.0f64, 1.0] {
                patch(mb, MatKey::Face, (s * 0.12).min(s * 0.86), (s * 0.12).max(s * 0.86), 0.47, 0.585, 0.006);
            }
            patch(mb, MatKey::Face, -0.12, 0.12, 0.545, 0.565, 0.006);
        }
        "korekcyjne" => {
            for s in [-1.0f64, 1.0] {
                let (a, b) = ((s * 0.14).min(s * 0.84), (s * 0.14).max(s * 0.84));
                let (f0, f1, tu, tf) = (0.47, 0.585, 0.07, 0.02);
                patch(mb, MatKey::Face, a, b, f0, f0 + tf, 0.006);
                patch(mb, MatKey::Face, a, b, f1 - tf, f1, 0.006);
                patch(mb, MatKey::Face, a, a + tu, f0 + tf, f1 - tf, 0.006);
                patch(mb, MatKey::Face, b - tu, b, f0 + tf, f1 - tf, 0.006);
            }
            patch(mb, MatKey::Face, -0.14, 0.14, 0.54, 0.56, 0.006);
        }
        _ => {}
    }

    // uszy: płaskie czworościany z boku czaszki
    for sd in [-1.0f64, 1.0] {
        let th = sd * std::f64::consts::FRAC_PI_2;
        let e = |f: f64, dz: f64, out: f64| {
            let mut q = pr.pt(f, th, out);
            q[2] += dz * pr.k;
            q
        };
        let (a, b, c) = (e(0.40, 0.0, 0.0), e(0.52, 0.0, 0.0), e(0.46, 0.0, 0.0));
        let (tip, back) = (e(0.47, -0.022, 0.018), e(0.46, -0.03, 0.0));
        let _ = c;
        let inside = pr.pt(0.46, 0.0, -0.05);
        for t in [[a, b, tip], [b, back, tip], [back, a, tip]] {
            let m = scale(add(add(t[0], t[1]), t[2]), 1.0 / 3.0);
            mb.poly(MatKey::Skin, &t, Some(sub(m, inside)));
        }
    }

    // --- zarost: skorupa, której tylne wierzchołki chowają się w głowie
    let (lo, hi, tau, thr) = match p.s("beard") {
        "kilkudniowy" => (0.04, 0.40, 0.003, -0.2),
        "krotka_broda" => (0.0, 0.42, 0.012, -0.2),
        "pelna_broda" => (-0.05, 0.46, 0.022, -0.3),
        "wasy" => (0.39, 0.46, 0.012, 0.85),
        "kozia" => (0.0, 0.2, 0.018, 0.85),
        _ => (0.0, 0.0, 0.0, 0.0),
    };
    if tau > 0.0 {
        let rings: Vec<Vec<V3>> = (0..3)
            .map(|i| {
                let f = lo + (hi - lo) * i as f64 / 2.0;
                (0..N).map(|k| pr.pt(f, theta(k), if theta(k).cos() > thr { tau } else { -0.006 })).collect()
            })
            .collect();
        mb.loft(MatKey::Hair, &rings, false);
    }

    // --- włosy i nakrycie głowy
    let cover = hat(p.s("headType"));
    let style = p.s("hairStyle");
    let hair = hair_shell(style);
    if let Some(h) = &hair {
        if cover.as_ref().is_none_or(|(c, _)| h.back < c.back - 0.2 || h.back < 0.15) {
            emit_shell(mb, &pr, h, MatKey::Hair);
        }
    }
    let nape = pr.at(0.28);
    let back = |f: f64| {
        let (d, _, cz, y) = pr.at(f);
        [0.0, y, cz - d]
    };
    match style {
        "dlugie" | "bob" => {
            let len = if style == "dlugie" { 0.30 } else { 0.09 } * sc.sv;
            let (z, y) = (nape.2 - nape.0 * 0.55, nape.3);
            let rings = vec![ring6([0.0, y, z], 0.08 * pr.k, 0.045 * pr.k, true), ring6([0.0, y - len * 0.5, z - 0.005], 0.085 * pr.k, 0.05 * pr.k, true), ring6([0.0, y - len, z - 0.01], 0.06 * pr.k, 0.035 * pr.k, true)];
            mb.loft(MatKey::Hair, &rings, false);
            mb.poly(MatKey::Hair, rings.last().unwrap(), Some([0.0, -1.0, 0.0]));
        }
        "kok" if cover.is_none() => {
            let c = add(back(0.88), [0.0, 0.0, -0.035 * pr.k]);
            let rings: Vec<Vec<V3>> = [(-0.035, 0.028), (0.0, 0.048), (0.035, 0.028)].iter().map(|&(dy, r)| ring6([c[0], c[1] + dy * pr.k, c[2]], r * pr.k, r * pr.k, false)).collect();
            mb.loft(MatKey::Hair, &rings, true);
        }
        "kucyk" if cover.is_none() => {
            let c = add(back(0.80), [0.0, 0.0, -0.02 * pr.k]);
            let len = 0.2 * sc.sv;
            let rings = vec![ring6(c, 0.03 * pr.k, 0.03 * pr.k, true), ring6([c[0], c[1] - len * 0.5, c[2] - 0.02], 0.026 * pr.k, 0.026 * pr.k, true), ring6([c[0], c[1] - len, c[2] - 0.01], 0.012 * pr.k, 0.012 * pr.k, true)];
            mb.loft(MatKey::Hair, &rings, false);
        }
        _ => {}
    }
    if let Some((c, brim)) = cover {
        emit_shell(mb, &pr, &c, MatKey::Helmet);
        if brim > 0.0 {
            let (r, l, f) = (pr.pt(c.front, theta(N - 1), c.tau), pr.pt(c.front, theta(0), c.tau), c.front);
            let fwd = [0.0, -0.012, brim * pr.k];
            let _ = f;
            let (r2, l2) = (add(r, fwd), add(l, fwd));
            mb.quad(MatKey::Helmet, r, l, l2, r2, Some([0.0, 1.0, 0.0]));
            mb.quad(MatKey::Helmet, r, r2, l2, l, Some([0.0, -1.0, 0.0]));
        }
    }
}
