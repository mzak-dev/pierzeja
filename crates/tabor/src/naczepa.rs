//! Naczepa (`bodyType = "naczepa"`): kurtynowa / kontenerowa 13,6 m na trzech osiach.
//! Origin na gruncie pod sworzniem królewskim (gra ustawia go w punkcie siodła ciągnika
//! i obraca wokół Y); przód +Z. Wymiary: `frontOverhang` · długość = od czoła do sworznia,
//! `wheelbase` = od sworznia do pierwszej osi, osie co `axleGap`.

use crate::ciezkie::{cyl, side_quad};
use crate::tablice::plate;
use crate::{Layout, P};
use procedura_kit::krzywe::lerp;
use procedura_kit::mesh::{MatKey, MeshBuilder, V3};

pub(crate) struct Naczepa {
    pub zf: f64,
    pub zr: f64,
    pub hw: f64,
    pub h: f64,
    pub deck: f64,
    pub coupling: f64,
    pub r: f64,
    pub track: f64,
    pub axles: Vec<f64>,
    pub z_legs: f64,
}

impl Naczepa {
    pub fn new(p: &P) -> Naczepa {
        let l = p.f("length");
        let zf = p.f("frontOverhang") * l;
        let r = p.f("wheelDiameter") / 2.0;
        let n = (p.f("axles").round() as usize).clamp(1, 4);
        let (wb, gap) = (p.f("wheelbase"), p.f("axleGap"));
        let axles: Vec<f64> = (0..n).map(|i| -wb - gap * i as f64).collect();
        let coupling = p.0.get("couplingHeight").and_then(|v| v.as_f64()).unwrap_or(1.15);
        Naczepa {
            zf,
            zr: zf - l,
            hw: p.f("width") / 2.0,
            h: p.f("height"),
            // podłoga ok. 0,15 m nad płytą sworznia
            deck: coupling + 0.15,
            coupling,
            r,
            track: (p.f("trackRatio") * p.f("width")).min(p.f("width") - 0.03 - p.f("tyreWidth")),
            axles,
            // nogi podporowe ok. 2,9 m za czołem
            z_legs: zf - 2.9,
        }
    }

    pub fn wheels(&self) -> Vec<(String, V3)> {
        let mut out = Vec::new();
        for (i, &z) in self.axles.iter().enumerate() {
            let n = if i == 0 { String::new() } else { (i + 1).to_string() };
            let x = self.track / 2.0;
            out.push((format!("Wheel_R{n}L"), [x, self.r, z]));
            out.push((format!("Wheel_R{n}R"), [-x, self.r, z]));
        }
        out
    }
}

pub(crate) fn build(p: &P, seed: u32, mb: &mut MeshBuilder) -> Layout {
    let n = Naczepa::new(p);
    let (zf, zr, hw, deck) = (n.zf, n.zr, n.hw, n.deck);
    // rama: dwie podłużnice, płyta sworznia, poprzeczki
    for sx in [1.0f64, -1.0] {
        let (a, b) = (0.42 * sx, 0.5 * sx);
        mb.bx(MatKey::Frame, [a.min(b), deck - 0.45, zr + 0.3], [a.max(b), deck - 0.02, zf - 0.3]);
    }
    mb.bx(MatKey::Frame, [-0.6, n.coupling, -0.6], [0.6, deck - 0.02, zf - 0.05]);
    cyl(mb, MatKey::Metal, [0.0, n.coupling - 0.08, 0.0], [0.0, n.coupling, 0.0], 0.045, 8, true);
    let mut z = zf - 0.6;
    while z > zr + 0.5 {
        mb.bx(MatKey::Frame, [-hw + 0.05, deck - 0.2, z - 0.04], [hw - 0.05, deck - 0.02, z + 0.04]);
        z -= 1.2;
    }
    // zabudowa: podłoga, plandeka (Paint) z pasami, dach (RoofPaint), czoło, tylne drzwi
    let (yt, y1) = (deck, n.h);
    mb.bx(MatKey::Frame, [-hw, yt - 0.02, zr], [hw, yt + 0.1, zf]);
    mb.bx(MatKey::Paint, [-hw + 0.005, yt + 0.1, zr + 0.06], [hw - 0.005, y1 - 0.08, zf - 0.06]);
    mb.bx(MatKey::RoofPaint, [-hw, y1 - 0.08, zr], [hw, y1, zf]);
    mb.bx(MatKey::Metal, [-hw, yt + 0.1, zf - 0.06], [hw, y1 - 0.08, zf]);
    mb.bx(MatKey::Metal, [-hw, yt + 0.1, zr], [hw, y1 - 0.08, zr + 0.06]);
    let posts = ((zf - zr) / 0.62).floor() as usize;
    for i in 1..posts {
        let zz = lerp(zr + 0.06, zf - 0.06, i as f64 / posts as f64);
        side_quad(mb, MatKey::Plastic, hw - 0.002, yt + 0.12, y1 - 0.1, zz - 0.022, zz + 0.022);
    }
    // słupek środkowy i dolna listwa burty (aluminium)
    side_quad(mb, MatKey::Metal, hw + 0.004, yt - 0.02, yt + 0.12, zr, zf);
    for zz in [lerp(zr, zf, 0.5)] {
        side_quad(mb, MatKey::Metal, hw + 0.003, yt + 0.1, y1 - 0.08, zz - 0.04, zz + 0.04);
    }
    // tylne drzwi: szczelina, 4 rygle, zawiasy
    let zb = zr - 0.003;
    let rq = |mb: &mut MeshBuilder, m: MatKey, x0: f64, x1: f64, ya: f64, yb: f64| mb.quad(m, [x1, ya, zb], [x0, ya, zb], [x0, yb, zb], [x1, yb, zb], Some([0.0, 0.0, -1.0]));
    rq(mb, MatKey::Plastic, -0.008, 0.008, yt + 0.12, y1 - 0.1);
    for x in [-0.62, -0.3, 0.3, 0.62].map(|f| f * hw) {
        rq(mb, MatKey::Frame, x - 0.017, x + 0.017, yt + 0.15, y1 - 0.12);
    }
    // nogi podporowe z korbą
    for sx in [1.0f64, -1.0] {
        let x = 0.85 * sx;
        mb.bx(MatKey::Frame, [x - 0.07, 0.25, n.z_legs - 0.07], [x + 0.07, deck - 0.45, n.z_legs + 0.07]);
        mb.bx(MatKey::Metal, [x - 0.05, 0.0, n.z_legs - 0.05], [x + 0.05, 0.3, n.z_legs + 0.05]);
        mb.bx(MatKey::Frame, [x - 0.15, 0.0, n.z_legs - 0.15], [x + 0.15, 0.03, n.z_legs + 0.15]);
    }
    // osłony boczne między nogami a pierwszą osią, osie, błotniki i chlapacze
    let first = n.axles[0];
    let last = *n.axles.last().expect("oś naczepy");
    for y in [0.48, 0.78] {
        side_quad(mb, MatKey::Metal, hw - 0.04, y, y + 0.08, first + n.r + 0.2, n.z_legs - 0.2);
    }
    for &za in &n.axles {
        mb.bx(MatKey::Metal, [-n.track / 2.0 + 0.15, n.r - 0.07, za - 0.07], [n.track / 2.0 - 0.15, n.r + 0.07, za + 0.07]);
        for sx in [1.0f64, -1.0] {
            let (a, b) = (0.42 * sx, 0.5 * sx);
            mb.bx(MatKey::Frame, [a.min(b), n.r + 0.07, za - 0.4], [a.max(b), deck - 0.45, za + 0.4]);
        }
    }
    side_quad(mb, MatKey::Plastic, hw - 0.03, 2.0 * n.r + 0.04, deck - 0.02, last - n.r - 0.12, first + n.r + 0.12);
    side_quad(mb, MatKey::Plastic, hw - 0.04, 0.22, 2.0 * n.r + 0.06, last - n.r - 0.18, last - n.r - 0.14);
    // obrysówki boczne co ok. 2,5 m
    let mut zz = zf - 0.5;
    while zz > zr + 0.3 {
        side_quad(mb, MatKey::Indicator, hw + 0.006, yt - 0.01, yt + 0.04, zz - 0.04, zz + 0.04);
        zz -= 2.5;
    }
    // belka przeciwnajazdowa, lampy i tablica z tyłu
    let (ub0, ub1) = (0.42, 0.55);
    mb.bx(MatKey::Frame, [-hw + 0.1, ub0, zr + 0.02], [hw - 0.1, ub1, zr + 0.14]);
    for sx in [1.0f64, -1.0] {
        let (a, b) = (0.6 * sx, 0.7 * sx);
        mb.bx(MatKey::Frame, [a.min(b), ub1, zr + 0.05], [a.max(b), deck - 0.4, zr + 0.12]);
    }
    let zl = zr + 0.015;
    for sx in [1.0f64, -1.0] {
        let q = |x: f64, y: f64| [x * sx, y, zl];
        let lamp = |mb: &mut MeshBuilder, m: MatKey, x0: f64, x1: f64, y0: f64, y1: f64| mb.quad(m, q(x1, y0), q(x0, y0), q(x0, y1), q(x1, y1), Some([0.0, 0.0, -1.0]));
        lamp(mb, MatKey::Taillight, hw - 0.42, hw - 0.12, ub0 + 0.02, ub1 - 0.02);
        lamp(mb, MatKey::Indicator, hw - 0.12, hw - 0.02, ub0 + 0.02, ub1 - 0.02);
    }
    plate(mb, seed, 7.0, [0.0, deck - 0.27, zr + 0.01], -1.0);
    let anchors = vec![("Anchor_Kingpin".to_string(), [0.0, n.coupling, 0.0]), ("Anchor_LandingGear".to_string(), [0.0, 0.0, n.z_legs])];
    Layout { wheelbase: p.f("wheelbase"), track: n.track, stations: 0, body_width: 2.0 * hw, lightbar: None, wheels: Some(n.wheels()), anchors }
}
