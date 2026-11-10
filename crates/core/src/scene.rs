//! Przykładowe zespoły zabudowy: osiedle bloków, skrzyżowanie i rondo z budynkami
//! narożnymi na obrysach wielokątnych. Wszystkie siatki są już ustawione w świecie.
//!
//! Liczby (kalibracja przykładowa, nie projekt drogowy):
//! - odstęp między blokami ≥ wysokość budynku przesłaniającego (WT 2002 § 13, przesłanianie 60°),
//!   w praktyce osiedli PRL większy — domyślnie 1,5 × H;
//! - rondo miejskie jednopasowe: średnica zewnętrzna 35 m, pierścień 6 m (katalog GDDKiA do WR-D-31-3);
//! - jezdnia 2 × 3,5 m + opaski, chodniki 3,6 m (jak w trybie ulicy).

use crate::blend::{normalize_weights, BuildingSpec, MixSpec};
use crate::data::{data, WeightMap};
use crate::footprint::{Footprint, P2};
use crate::generator::{fixed1, generate_from_spec, BuildingStats};
use crate::materials::build_materials;
use crate::mesh::{Bounds, MatKey, MeshBuilder, MeshData};
use crate::params::{default_params, Params};
use crate::random::{hash01, Mulberry32};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SceneKind {
    #[default]
    Estate,
    Crossroads,
    Roundabout,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneSpec {
    pub kind: SceneKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<f64>,
    /// Osiedle: odstęp między rzędami bloków jako wielokrotność wysokości (domyślnie 1,5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<f64>,
    /// Osiedle: liczba rzędów bloków (domyślnie 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u32>,
    /// Skrzyżowanie/rondo: profil miasta dla pierzei na ramionach (domyślnie Warszawa).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Długość ramion od środka [m] (domyślnie 90).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variation: Option<f64>,
}

pub struct SceneItem {
    pub name: String,
    pub mesh: MeshData,
    pub spec: Option<BuildingSpec>,
    pub stats: Option<BuildingStats>,
}

pub struct Scene {
    pub title: String,
    pub items: Vec<SceneItem>,
    pub notes: Vec<String>,
    pub bounds: Bounds,
}

impl Scene {
    pub fn describe(&self) -> String {
        let mut out = vec![self.title.clone()];
        for it in &self.items {
            if let Some(s) = &it.stats {
                out.push(format!("• {} — {} kond., {} m", it.name, s.floors, fixed1(s.height_total, false)));
            }
        }
        out.extend(self.notes.iter().map(|n| format!("◦ {n}")));
        out.join("\n")
    }
}

const HW: f64 = 3.9; // pół jezdni
const SW: f64 = 3.6; // chodnik
const BL: f64 = HW + SW; // linia zabudowy od osi ulicy

// ------------------------------------------------------------------ kontekst (jezdnie, chodniki)
struct Ctx {
    mb: MeshBuilder,
}

impl Ctx {
    fn new() -> Self {
        Self { mb: MeshBuilder::default() }
    }
    /// Prostokąt osiowy [x0,x1]×[z0,z1] od y = −0,02 do `top`.
    fn rect(&mut self, mat: MatKey, x0: f64, x1: f64, z0: f64, z1: f64, top: f64) {
        self.mb.bx(mat, [x0.min(x1), -0.02, z0.min(z1)], [x0.max(x1), top, z0.max(z1)]);
    }
    fn road(&mut self, x0: f64, x1: f64, z0: f64, z1: f64) {
        self.rect(MatKey::Road, x0, x1, z0, z1, 0.02);
    }
    fn walk(&mut self, x0: f64, x1: f64, z0: f64, z1: f64) {
        self.rect(MatKey::Sidewalk, x0, x1, z0, z1, 0.12);
    }
    /// Pierścień (łuk) na wysokości `top` — jezdnia ronda, chodnik, wyspa.
    fn ring(&mut self, mat: MatKey, r0: f64, r1: f64, top: f64, skip: impl Fn(f64, f64) -> bool) {
        let n = 72;
        for k in 0..n {
            let (t0, t1) = (k as f64 / n as f64 * 2.0 * PI, (k + 1) as f64 / n as f64 * 2.0 * PI);
            let tm = (t0 + t1) / 2.0;
            if skip(tm.cos() * (r0 + r1) / 2.0, tm.sin() * (r0 + r1) / 2.0) {
                continue;
            }
            let p = |r: f64, t: f64, y: f64| [r * t.cos(), y, r * t.sin()];
            self.mb.quad(mat, p(r0, t0, top), p(r0, t1, top), p(r1, t1, top), p(r1, t0, top), Some([0.0, 1.0, 0.0]));
            if r0 > 0.0 {
                self.mb.quad(mat, p(r0, t0, -0.02), p(r0, t1, -0.02), p(r0, t1, top), p(r0, t0, top), Some([-tm.cos(), 0.0, -tm.sin()]));
            }
            self.mb.quad(mat, p(r1, t0, -0.02), p(r1, t1, -0.02), p(r1, t1, top), p(r1, t0, top), Some([tm.cos(), 0.0, tm.sin()]));
        }
    }
    /// Przejście dla pieszych: pasy wzdłuż osi `along_x`, w poprzek jezdni o półszerokości HW.
    fn zebra(&mut self, along_x: bool, c0: f64, c1: f64) {
        let mut t = -HW + 0.5;
        while t < HW - 0.4 {
            if along_x {
                self.rect(MatKey::Trim, c0, c1, t, t + 0.5, 0.03);
            } else {
                self.rect(MatKey::Trim, t, t + 0.5, c0, c1, 0.03);
            }
            t += 1.0;
        }
    }
    /// Przerywana linia środkowa na odcinku osi.
    fn center_line(&mut self, along_x: bool, a: f64, b: f64) {
        let mut s = a.min(b);
        while s + 3.0 < a.max(b) {
            if along_x {
                self.rect(MatKey::Trim, s, s + 3.0, -0.08, 0.08, 0.025);
            } else {
                self.rect(MatKey::Trim, -0.08, 0.08, s, s + 3.0, 0.025);
            }
            s += 6.0;
        }
    }
    fn build(self) -> MeshData {
        let mut dp = default_params();
        dp.insert("trimColor".into(), "#e9e7df".into());
        dp.insert("wallColor".into(), "#b9b6ad".into());
        let keep = [MatKey::Road, MatKey::Sidewalk, MatKey::Trim, MatKey::Ground, MatKey::Slab, MatKey::Metal];
        let mats = build_materials(&Params::from_map(&dp)).into_iter().filter(|m| keep.contains(&m.key)).collect();
        self.mb.build(mats)
    }
}

// ------------------------------------------------------------------ budynki
fn building(name: String, spec: BuildingSpec, n: [f64; 2], origin: [f64; 3]) -> SceneItem {
    let m = generate_from_spec(&spec);
    SceneItem { name, mesh: m.mesh.placed(n, origin), spec: Some(spec), stats: Some(m.stats) }
}

fn pick_weighted(w: &WeightMap, r: f64) -> String {
    let mut acc = 0.0;
    for (id, v) in w {
        acc += v;
        if r <= acc {
            return id.clone();
        }
    }
    w.keys().last().cloned().unwrap_or_default()
}

/// Pierzeja wzdłuż linii zabudowy: od `start` w kierunku `along`, ulica po stronie `n`.
/// Budynki, które się nie mieszczą w `length`, są pomijane (zostaje luka, jak na końcu kwartału).
type Aabb = [f64; 4]; // x0, x1, z0, z1

fn overlaps(a: &Aabb, b: &Aabb) -> bool {
    a[0] < b[1] - 0.05 && b[0] < a[1] - 0.05 && a[2] < b[3] - 0.05 && b[2] < a[3] - 0.05
}

/// Rzut budynku (z oficynami) w świecie dla ustawienia z `MeshData::placed`.
fn footprint_aabb(p: &Params, n: [f64; 2], origin: P2) -> Aabb {
    let w = p.segments * p.segment_width;
    let wings = p.wing_length * p.wing_left.max(p.wing_right) + if p.wing_rear >= 0.5 { p.wing_width } else { 0.0 };
    let back = p.street_setback + p.depth + wings + p.stagger;
    let d = [n[1], -n[0]];
    let corners = [(-w / 2.0, -p.street_setback), (w / 2.0, -p.street_setback), (-w / 2.0, -back), (w / 2.0, -back)];
    let pts: Vec<P2> = corners.iter().map(|&(x, z)| [origin[0] + x * d[0] + z * n[0], origin[1] + x * d[1] + z * n[1]]).collect();
    let f = |k: usize, mx: bool| pts.iter().map(|q| q[k]).fold(if mx { f64::NEG_INFINITY } else { f64::INFINITY }, |a, b| if mx { a.max(b) } else { a.min(b) });
    [f(0, false), f(0, true), f(1, false), f(1, true)]
}

#[allow(clippy::too_many_arguments)]
fn frontage_row(items: &mut Vec<SceneItem>, placed: &mut Vec<Aabb>, label: &str, start: P2, along: P2, n: [f64; 2], length: f64, weights: &WeightMap, seed: u32, variation: f64) {
    let mut rnd = Mulberry32::new(seed);
    let mut s = 0.0;
    let mut prev_gap: Option<f64> = None;
    for i in 0..30 {
        let a = pick_weighted(weights, rnd.next());
        let bseed = (hash01(&[seed as f64, i as f64, 5.0]) * 1e9).floor();
        let mut spec = BuildingSpec { mix: MixSpec { archetype: Some(a.clone()), seed: Some(bseed), variation: Some(variation), ..Default::default() }, ..Default::default() };
        let mut p = crate::blend::resolve_spec(&spec).params;
        let width = p.segments * p.segment_width;
        let gap = match prev_gap {
            Some(pg) if pg < 0.5 && p.side_gap < 0.5 => 0.0,
            Some(pg) => pg.max(p.side_gap),
            None => 0.0,
        };
        if s + gap + width > length {
            break;
        }
        let c = s + gap + width / 2.0;
        let origin = [start[0] + along[0] * c, start[1] + along[1] * c];
        // kolizja z budynkami z prostopadłej ulicy (wnętrze kwartału): najpierw bez oficyn, potem luka
        if placed.iter().any(|b| overlaps(b, &footprint_aabb(&p, n, origin))) {
            spec.mix.overrides = Some(serde_json::from_value(json!({ "wingLeft": 0, "wingRight": 0, "wingRear": 0 })).unwrap());
            p = crate::blend::resolve_spec(&spec).params;
            if placed.iter().any(|b| overlaps(b, &footprint_aabb(&p, n, origin))) {
                s += gap + width;
                prev_gap = Some(p.side_gap);
                continue;
            }
        }
        placed.push(footprint_aabb(&p, n, origin));
        let name = format!("{label} {} · {}", i + 1, data().archetype(&a).map_or(a.as_str(), |x| x.short.as_str()));
        items.push(building(name, spec, n, [origin[0], 0.0, origin[1]]));
        s += gap + width;
        prev_gap = Some(p.side_gap);
    }
}

fn mirror(pts: &[P2], sx: f64, sz: f64) -> Vec<P2> {
    pts.iter().map(|p| [p[0] * sx, p[1] * sz]).collect()
}

fn poly_aabb(pts: &[P2]) -> Aabb {
    let f = |k: usize, mx: bool| pts.iter().map(|q| q[k]).fold(if mx { f64::NEG_INFINITY } else { f64::INFINITY }, |a, b| if mx { a.max(b) } else { a.min(b) });
    [f(0, false), f(0, true), f(1, false), f(1, true)]
}

fn corner_item(name: &str, archetype: &str, seed: f64, overrides: serde_json::Value, points: Vec<P2>, front: Vec<usize>, blind: Vec<usize>) -> SceneItem {
    let spec: BuildingSpec = serde_json::from_value(json!({ "archetype": archetype, "seed": seed, "overrides": overrides })).expect("spec");
    let spec = BuildingSpec { footprint: Some(Footprint { points, front, blind }), ..spec };
    building(format!("{name} · {}", data().archetype(archetype).map_or(archetype, |a| a.short.as_str())), spec, [0.0, 1.0], [0.0; 3])
}

fn depth_of(archetype: &str) -> f64 {
    data().vector(archetype)["depth"].as_f64().unwrap_or(13.0)
}

fn profile_weights(s: &SceneSpec) -> (WeightMap, String) {
    let d = data();
    let p = s.profile.as_deref().and_then(|id| d.city(id)).unwrap_or(&d.city_profiles[0]);
    (normalize_weights(Some(&p.weights)), p.name.clone())
}

fn finish(title: String, mut items: Vec<SceneItem>, ctx: Ctx, notes: Vec<String>) -> Scene {
    items.push(SceneItem { name: "teren".into(), mesh: ctx.build(), spec: None, stats: None });
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for it in &items {
        for k in 0..3 {
            lo[k] = lo[k].min(it.mesh.bounds.min[k]);
            hi[k] = hi[k].max(it.mesh.bounds.max[k]);
        }
    }
    Scene { title, items, notes, bounds: Bounds { min: lo, max: hi } }
}

// ------------------------------------------------------------------ skrzyżowanie
/// Narożnik w kształcie L (ćwiartka +x,+z): fronty wzdłuż obu ulic, ścięty albo obły narożnik.
fn corner_l(w1: f64, w2: f64, d: f64, chamfer: f64, round: bool) -> (Vec<P2>, Vec<usize>, Vec<usize>) {
    let mut pts: Vec<P2> = vec![];
    // narożnik: od ulicy A (z = BL) do ulicy B (x = BL)
    if round {
        let k = 5;
        for i in 0..=k {
            let t = -PI / 2.0 - (i as f64 / k as f64) * PI / 2.0; // od dołu do lewej strony łuku
            pts.push([BL + chamfer + chamfer * t.cos(), BL + chamfer + chamfer * t.sin()]);
        }
    } else if chamfer > 0.1 {
        pts.push([BL + chamfer, BL]);
        pts.push([BL, BL + chamfer]);
    } else {
        pts.push([BL, BL]);
    }
    let nc = pts.len();
    pts.push([BL, BL + w2]);
    pts.push([BL + d, BL + w2]);
    pts.push([BL + d, BL + d]);
    pts.push([BL + w1, BL + d]);
    pts.push([BL + w1, BL]);
    // krawędzie: łuk/ścięcie 0..nc−1, ulica B = nc−1, ślepa nc, podwórze nc+1..nc+2, ślepa nc+3, ulica A = nc+4
    let front: Vec<usize> = (0..nc).chain([nc + 4]).collect();
    (pts, front, vec![nc, nc + 3])
}

fn crossroads(s: &SceneSpec) -> Scene {
    let seed = crate::random::js_u32(s.seed.unwrap_or(11.0));
    let arm = s.arm.unwrap_or(90.0).clamp(40.0, 300.0);
    let var = s.variation.unwrap_or(0.3);
    let (weights, pname) = profile_weights(s);
    let mut ctx = Ctx::new();
    let e = arm + 15.0;
    ctx.road(-e, e, -HW, HW);
    ctx.road(-HW, HW, -e, e);
    for (sx, sz) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        ctx.walk(sx * HW, sx * e, sz * HW, sz * BL);
        ctx.walk(sx * HW, sx * BL, sz * HW, sz * e);
    }
    for sg in [1.0, -1.0] {
        ctx.zebra(true, sg * (BL + 0.5), sg * (BL + 4.5));
        ctx.zebra(false, sg * (BL + 0.5), sg * (BL + 4.5));
        ctx.center_line(true, sg * (BL + 6.0), sg * e);
        ctx.center_line(false, sg * (BL + 6.0), sg * e);
    }
    // cztery narożniki: ścięty, ścięty z wieżyczką, obły (modernizm), ostry (współczesny)
    let corners: [(&str, f64, bool, serde_json::Value, (f64, f64)); 4] = [
        ("kamienica_eklektyczna", 3.5, false, json!({}), (1.0, 1.0)),
        ("kamienica_secesyjna", 4.0, false, json!({ "turret": 1 }), (-1.0, 1.0)),
        ("kamienica_modernistyczna", 5.0, true, json!({}), (1.0, -1.0)),
        ("apartamentowiec", 0.0, false, json!({}), (-1.0, -1.0)),
    ];
    let mut items = Vec::new();
    let mut placed: Vec<Aabb> = Vec::new();
    for (k, (a, ch, round, ov, (sx, sz))) in corners.iter().enumerate() {
        let d = depth_of(a).clamp(10.0, 18.0);
        let (w1, w2) = (26.0, 22.0);
        let (pts, front, blind) = corner_l(w1, w2, d, *ch, *round);
        let pts = mirror(&pts, *sx, *sz);
        placed.push(poly_aabb(&pts));
        items.push(corner_item(&format!("Narożnik {}", k + 1), a, 3.0 + k as f64, ov.clone(), pts, front, blind));
        // pierzeje na ramionach za narożnikiem
        let len_a = arm - (BL + w1);
        let len_b = arm - (BL + w2);
        frontage_row(&mut items, &mut placed, "Pierzeja", [sx * (BL + w1), sz * BL], [*sx, 0.0], [0.0, -sz], len_a, &weights, seed ^ (k as u32 * 7919 + 1), var);
        frontage_row(&mut items, &mut placed, "Pierzeja", [sx * BL, sz * (BL + w2)], [0.0, *sz], [-sx, 0.0], len_b, &weights, seed ^ (k as u32 * 7919 + 2), var);
    }
    let notes = vec![
        format!("Pierzeje na ramionach z profilu: {pname}."),
        "Narożniki na obrysach wielokątnych: ścięty (kamienica), ścięty z wieżyczką (secesja), obły (modernizm), ostry (apartamentowiec); ściany graniczne bez okien.".into(),
        "Jezdnia 7,8 m, chodniki 3,6 m, przejścia dla pieszych przy wlotach.".into(),
    ];
    finish("Skrzyżowanie czterowlotowe".into(), items, ctx, notes)
}

// ------------------------------------------------------------------ rondo
fn roundabout(s: &SceneSpec) -> Scene {
    let seed = crate::random::js_u32(s.seed.unwrap_or(13.0));
    let arm = s.arm.unwrap_or(90.0).clamp(50.0, 300.0);
    let var = s.variation.unwrap_or(0.3);
    let (weights, pname) = profile_weights(s);
    let (r_out, ring) = (17.5, 6.0);
    let r_in = r_out - ring;
    let rb = r_out + SW; // linia zabudowy po łuku
    let mut ctx = Ctx::new();
    let in_arm = |x: f64, z: f64| x.abs() < BL || z.abs() < BL;
    ctx.ring(MatKey::Road, r_in, r_out, 0.02, |_, _| false);
    ctx.ring(MatKey::Sidewalk, r_in - 1.5, r_in, 0.08, |_, _| false); // opaska wyspy (dla ciężarówek)
    ctx.ring(MatKey::Ground, 0.0, r_in - 1.5, 0.15, |_, _| false);
    ctx.ring(MatKey::Sidewalk, r_out, rb, 0.12, |x, z| in_arm(x, z) && (x.abs() < HW || z.abs() < HW));
    let e = arm + 15.0;
    let x0 = (r_out * r_out - HW * HW).sqrt() - 0.5;
    for sg in [1.0, -1.0] {
        ctx.road(sg * x0, sg * e, -HW, HW);
        ctx.road(-HW, HW, sg * x0, sg * e);
        for sd in [1.0, -1.0] {
            ctx.walk(sg * (r_out - 0.5), sg * e, sd * HW, sd * BL);
            ctx.walk(sd * HW, sd * BL, sg * (r_out - 0.5), sg * e);
        }
        ctx.zebra(true, sg * (rb + 1.0), sg * (rb + 5.0));
        ctx.zebra(false, sg * (rb + 1.0), sg * (rb + 5.0));
        ctx.center_line(true, sg * (rb + 7.0), sg * e);
        ctx.center_line(false, sg * (rb + 7.0), sg * e);
    }
    let corners = ["kamienica_eklektyczna", "socrealizm", "kamienica_modernistyczna", "deweloperski"];
    let quads = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
    let mut items = Vec::new();
    let mut placed: Vec<Aabb> = Vec::new();
    for (k, (a, (sx, sz))) in corners.iter().zip(quads).enumerate() {
        let d = depth_of(a).clamp(10.0, 16.0);
        let w = 18.0;
        let (ta, tb) = ((BL / rb).asin(), (BL / rb).acos());
        let (ra, ta2, tb2) = (rb + d, ((BL + d) / (rb + d)).asin(), ((BL + d) / (rb + d)).acos());
        let xa = rb * ta.cos();
        let zb = rb * tb.sin();
        let mut pts: Vec<P2> = vec![[xa + w, BL], [xa, BL]];
        let arc = 6;
        for i in 1..=arc {
            let t = ta + (tb - ta) * i as f64 / arc as f64;
            pts.push([rb * t.cos(), rb * t.sin()]);
        }
        pts.push([BL, zb + w]);
        pts.push([BL + d, zb + w]);
        let inner = 4;
        for i in 0..=inner {
            let t = tb2 - (tb2 - ta2) * i as f64 / inner as f64;
            pts.push([ra * t.cos(), ra * t.sin()]);
        }
        pts.push([xa + w, BL + d]);
        // krawędzie: 0 = ulica A, 1..=arc = łuk, arc+1 = ulica B, arc+2 = ślepa, ostatnia = ślepa
        let n = pts.len();
        let front: Vec<usize> = (0..=arc + 1).collect();
        let blind = vec![arc + 2, n - 1];
        let pts = mirror(&pts, sx, sz);
        placed.push(poly_aabb(&pts));
        items.push(corner_item(&format!("Narożnik {}", k + 1), a, 5.0 + k as f64, json!({}), pts, front, blind));
        frontage_row(&mut items, &mut placed, "Pierzeja", [sx * (xa + w), sz * BL], [sx, 0.0], [0.0, -sz], arm - (xa + w), &weights, seed ^ (k as u32 * 7919 + 1), var);
        frontage_row(&mut items, &mut placed, "Pierzeja", [sx * BL, sz * (zb + w)], [0.0, sz], [-sx, 0.0], arm - (zb + w), &weights, seed ^ (k as u32 * 7919 + 2), var);
    }
    let notes = vec![
        format!("Rondo jednopasowe: średnica zewnętrzna {} m, pierścień {} m, wyspa z opaską 1,5 m.", 2.0 * r_out, ring),
        "Budynki narożne z frontem po łuku (6 odcinków) — obrys wklęsły od strony ronda.".into(),
        format!("Pierzeje na ramionach z profilu: {pname}."),
    ];
    finish("Rondo".into(), items, ctx, notes)
}

// ------------------------------------------------------------------ osiedle
fn estate(s: &SceneSpec) -> Scene {
    let seed = s.seed.unwrap_or(21.0);
    let factor = s.spacing.unwrap_or(1.5).clamp(0.8, 3.0);
    let nrows = s.rows.unwrap_or(3).clamp(1, 6) as usize;
    let var = s.variation.unwrap_or(0.15);
    let mut ctx = Ctx::new();
    let mut items = Vec::new();
    let mut notes = Vec::new();
    let passage = 24.0; // przejście między blokami w rzędzie (piesze, plac zabaw)
    let rows_types: [[(&str, serde_json::Value); 2]; 4] = [
        [("blok_plyta_5", json!({})), ("blok_plyta_5", json!({}))],
        [("blok_plyta_11", json!({})), ("blok_plyta_11", json!({}))],
        [("blok_plyta_5", json!({ "segments": 3 })), ("blok_plyta_11", json!({ "segments": 1, "segmentWidth": 22, "depth": 22 }))],
        [("blok_poodwilzowy", json!({})), ("blok_poodwilzowy", json!({}))],
    ];
    let mut z_front = -14.0; // pas zieleni 14 m od chodnika ulicy głównej
    let mut prev_h = 0.0f64;
    let mut gap_cur = 0.0f64;
    let (mut xmin, mut xmax) = (0.0f64, 0.0f64);
    let mut paved = 0.0;
    let mut built = 0.0;
    for r in 0..nrows {
        // wymiary i wysokości rzędu
        let pair = &rows_types[r % rows_types.len()];
        let specs: Vec<BuildingSpec> = pair
            .iter()
            .enumerate()
            .map(|(k, (a, ov))| serde_json::from_value(json!({ "archetype": a, "seed": seed + (r * 2 + k) as f64, "variation": var, "overrides": ov })).unwrap())
            .collect();
        let models: Vec<_> = specs.iter().map(generate_from_spec).collect();
        let h_row = models.iter().map(|m| m.stats.height_total).fold(0.0, f64::max);
        if r > 0 {
            let h = prev_h.max(h_row);
            let gap = (factor * h).max(h);
            notes.push(format!("Odstęp rzędów {}–{}: {} m = {} × H (H = {} m; minimum wg WT 2002 § 13: H).", r, r + 1, fixed1(gap, true), fixed1(gap / h, true), fixed1(h, true)));
            z_front -= gap;
            gap_cur = gap;
        }
        let depth_row = models.iter().map(|m| m.resolved.params.depth).fold(0.0, f64::max);
        for (k, (m, spec)) in models.iter().zip(&specs).enumerate() {
            let w = m.resolved.params.segments * m.resolved.params.segment_width;
            let dep = m.resolved.params.depth;
            let cx = if k == 0 { -(passage / 2.0 + w / 2.0) } else { passage / 2.0 + w / 2.0 };
            xmin = xmin.min(cx - w / 2.0);
            xmax = xmax.max(cx + w / 2.0);
            built += w * dep;
            let name = format!("Rząd {} · {}", r + 1, data().archetype(spec.mix.archetype.as_deref().unwrap_or("")).map_or("blok", |a| a.short.as_str()));
            items.push(SceneItem { name, mesh: m.mesh.placed([0.0, 1.0], [cx, 0.0, z_front]), spec: Some(spec.clone()), stats: Some(m.stats.clone()) });
            // chodnik wzdłuż frontu (wejścia do klatek) i opaska od tyłu
            ctx.walk(cx - w / 2.0 - 1.0, cx + w / 2.0 + 1.0, z_front, z_front + 2.5);
            paved += (w + 2.0) * 2.5;
        }
        // droga dojazdowa z parkingiem przed rzędem (od strony wejść), jeśli jest miejsce
        if r > 0 {
            let back_prev = z_front + gap_cur;
            let (road0, road1) = (z_front + 6.5, z_front + 12.0);
            if road1 + 5.0 < back_prev - 2.0 {
                ctx.road(xmin - 14.0, xmax + 6.0, road0, road1);
                ctx.rect(MatKey::Road, -passage / 2.0 - 30.0, -passage / 2.0 - 2.0, road1, road1 + 5.0, 0.02);
                ctx.rect(MatKey::Road, passage / 2.0 + 2.0, passage / 2.0 + 30.0, road1, road1 + 5.0, 0.02);
                let mut x = -passage / 2.0 - 30.0;
                while x <= passage / 2.0 + 30.0 {
                    if x.abs() > passage / 2.0 + 1.0 {
                        ctx.rect(MatKey::Trim, x - 0.06, x + 0.06, road1 + 0.3, road1 + 5.0, 0.03);
                    }
                    x += 2.5;
                }
                paved += (xmax - xmin + 20.0) * 5.5 + 56.0 * 5.0;
            }
            // plac zabaw w pierwszym szerokim odstępie, w przejściu między blokami
            if r == 1 {
                let zc = (z_front + back_prev) / 2.0 + 4.0;
                ctx.rect(MatKey::Slab, -7.0, 7.0, zc - 7.0, zc + 7.0, 0.06);
                for (dx, dz, h) in [(-4.0, -4.0, 2.2), (4.0, -3.0, 1.2), (-3.0, 4.0, 1.6), (3.5, 3.5, 2.6)] {
                    ctx.rect(MatKey::Metal, dx - 0.6, dx + 0.6, zc + dz - 0.6, zc + dz + 0.6, h);
                }
            }
        }
        prev_h = h_row;
        z_front -= depth_row;
    }
    let z_end = z_front - 15.0;
    // ulica główna (jak w trybie ulicy) i drogi osiedlowe
    let (ex0, ex1) = (xmin - 40.0, xmax + 40.0);
    ctx.walk(ex0, ex1, 0.0, 3.6);
    ctx.road(ex0, ex1, 3.6, 11.4);
    ctx.walk(ex0, ex1, 11.4, 15.0);
    let mut x = ex0 + 2.0;
    while x < ex1 - 3.0 {
        ctx.rect(MatKey::Trim, x, x + 3.0, 7.42, 7.58, 0.025);
        x += 6.0;
    }
    ctx.road(xmin - 20.0, xmin - 14.0, z_end, 3.6); // droga osiedlowa zachodnia
    ctx.walk(-1.5, 1.5, z_end, 0.0); // ciąg pieszy w przejściu między blokami
    paved += 6.0 * (3.6 - z_end) + 3.0 * (-z_end);
    // pawilon usługowy przy ulicy głównej
    let pav: BuildingSpec = serde_json::from_value(json!({ "archetype": "blok_plyta_5", "seed": seed + 99.0, "overrides": {
        "floors": 1, "segments": 2, "segmentWidth": 16, "depth": 16, "groundFloorHeight": 3.8, "groundShop": 1, "entranceType": "glass",
        "balconies": 0, "stairWindows": 0, "rooftopBoxes": 0.3, "plinthHeight": 0.2, "accentPanels": 0.4 } })).unwrap();
    let pm = generate_from_spec(&pav);
    let px = xmax + 26.0;
    built += 32.0 * 16.0;
    items.push(SceneItem { name: "Pawilon usługowy".into(), mesh: pm.mesh.placed([0.0, 1.0], [px, 0.0, -4.0]), spec: Some(pav), stats: Some(pm.stats) });
    let site = (xmax - xmin + 34.0) * (-z_end);
    let green = 1.0 - (built + paved) / site;
    notes.push(format!("Zieleń ≈ {}% terenu osiedla (normatyw urbanistyczny 1974: ≥ 50% terenu netto).", (green * 100.0).round()));
    notes.push("Wejścia do klatek od strony dróg dojazdowych z parkingiem prostopadłym; przejście piesze i plac zabaw między blokami.".into());
    finish(format!("Osiedle bloków (odstęp {} × H)", fixed1(factor, true)), items, ctx, notes)
}

pub fn generate_scene(s: &SceneSpec) -> Scene {
    match s.kind {
        SceneKind::Estate => estate(s),
        SceneKind::Crossroads => crossroads(s),
        SceneKind::Roundabout => roundabout(s),
    }
}
