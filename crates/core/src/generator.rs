//! Gramatyka budynku (split grammar w wersji stałych reguł): bryły → odsłonięte
//! ściany → osie × kondygnacje → komórki z otworami → elementy → dachy.
//! Port 1:1 z `src/core/generator.ts` prototypu (HANDOFF rozdz. 6).

use crate::blend::{resolve_spec, BuildingSpec, ResolvedBuilding};
use crate::materials::build_materials;
use crate::mesh::{frame_at, shift_frame, to_world, Frame, MatKey, MeshBuilder, MeshData, V3};
use crate::params::Params;
use crate::random::{hash01_seeded, jround};
use serde::Serialize;
use std::f64::consts::PI;

use MatKey as M;

#[derive(Clone, Copy, PartialEq)]
enum Face {
    Front,
    Back,
    Left,
    Right,
}
const FACES: [Face; 4] = [Face::Front, Face::Back, Face::Left, Face::Right];

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Front,
    Back,
    Side,
    Court,
}

#[derive(Clone, Copy, PartialEq)]
enum VolKind {
    Main,
    WingL,
    WingR,
    Rear,
    Pent,
}

#[derive(Clone, Copy)]
struct Rect {
    x0: f64,
    x1: f64,
    z0: f64,
    z1: f64,
}

#[derive(Clone, Copy)]
struct Row {
    b0: f64,
    b1: f64,
    /// Poziom podłogi (dla parteru = wierzch cokołu).
    level: f64,
    /// 0 = parter.
    floor: i32,
}

#[derive(Clone)]
struct Volume {
    id: usize,
    rect: Rect,
    rows: Vec<Row>,
    y0: f64,
    top: f64,
    kind: VolKind,
    has_above: bool,
    along_x: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum OpKind {
    Window,
    Door,
    Shop,
    Gate,
    GlassDoor,
    Stair,
    Basement,
    Void,
}

#[derive(Clone, Copy)]
struct Opening {
    a0: f64,
    a1: f64,
    b0: f64,
    b1: f64,
    kind: OpKind,
    depth: f64,
    floor: i32,
    /// Drzwi balkonowe (bez parapetu zewnętrznego).
    bd: bool,
}

impl Opening {
    fn new(a0: f64, a1: f64, b0: f64, b1: f64, kind: OpKind, depth: f64, floor: i32) -> Self {
        Self { a0, a1, b0, b1, kind, depth, floor, bd: false }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingStats {
    pub height_walls: f64,
    pub height_total: f64,
    pub floors: u32,
    pub height_class: &'static str,
    pub footprint: f64,
    pub gfa: f64,
    pub frontage: f64,
    pub depth: f64,
    pub staircases: u32,
    pub windows: u32,
    pub balconies: u32,
    pub flats_estimate: u32,
    pub triangles: usize,
    pub notes: Vec<String>,
}

pub struct BuildingModel {
    pub resolved: ResolvedBuilding,
    pub mesh: MeshData,
    pub stats: BuildingStats,
}

const EPS: f64 = 0.02;

/// `Math.min(b, Math.max(a, v))` — przy b < a wygrywa b (jak w prototypie).
fn clamp(v: f64, a: f64, b: f64) -> f64 {
    b.min(a.max(v))
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
fn rad(deg: f64) -> f64 {
    deg * PI / 180.0
}

/// `toFixed(1)` z JS (połówki w górę) z przecinkiem.
pub(crate) fn fixed1(v: f64, comma: bool) -> String {
    let s = format!("{:.1}", jround(v * 10.0) / 10.0);
    if comma { s.replace('.', ",") } else { s }
}

struct Gen<'a> {
    r: &'a ResolvedBuilding,
    p: &'a Params,
    mb: MeshBuilder,
    vols: Vec<Volume>,
    seed: u32,
    regulations: bool,
    plinth: f64,
    ground_top: f64,
    frontage: f64,
    loggia_by_rule: bool,
    no_balcony_above55: bool,
    ridge_top: f64,
}

impl<'a> Gen<'a> {
    fn new(r: &'a ResolvedBuilding) -> Self {
        let p = &r.params;
        let plinth = clamp(p.plinth_height, 0.0, (p.ground_floor_height - 1.2).max(0.0));
        Gen {
            r,
            p,
            mb: MeshBuilder::default(),
            vols: Vec::new(),
            seed: r.seed,
            regulations: r.regulations,
            plinth,
            ground_top: plinth + p.ground_floor_height,
            frontage: p.segments * p.segment_width,
            loggia_by_rule: false,
            no_balcony_above55: false,
            ridge_top: 0.0,
        }
    }

    fn h(&self, xs: &[f64]) -> f64 {
        hash01_seeded(self.seed, xs)
    }

    fn floors(&self) -> i32 {
        jround(self.p.floors).max(1.0) as i32
    }

    // ------------------------------------------------------------- układ pionowy
    fn rows_for(&self, from: i32, to: i32) -> Vec<Row> {
        let p = self.p;
        let f = self.floors();
        let mut rows = Vec::new();
        let mut y = 0.0;
        for k in 0..f {
            let (b0, b1, level) = if k == 0 {
                (0.0, self.ground_top, self.plinth)
            } else {
                let t = if f > 2 { (k - 1) as f64 / (f - 2) as f64 } else { 0.0 };
                let hgt = (p.floor_height * (1.0 - p.floor_height_decay * t)).max(2.3);
                (y, y + hgt, y)
            };
            if k >= from && k <= to {
                rows.push(Row { b0, b1, level, floor: k });
            }
            y = b1;
        }
        rows
    }

    // ------------------------------------------------------------- bryła
    fn layout(&mut self) {
        let p = self.p;
        let f = self.floors();
        let s = clamp(jround(p.setback_floors), 0.0, (f - 1) as f64) as i32;
        let segs = jround(p.segments).max(1.0) as usize;
        let sw = p.segment_width;
        let d = p.depth;
        let w = segs as f64 * sw;
        let main_top = f - 1 - s;
        let main_rows = self.rows_for(0, main_top);
        let pent_rows = if s > 0 { self.rows_for(main_top + 1, f - 1) } else { Vec::new() };
        let can_pent = s > 0 && d - p.setback_depth >= 3.5 && p.setback_depth > 0.4;
        let rows_main = if can_pent { main_rows } else { self.rows_for(0, f - 1) };
        let along_x = p.ridge_along_front >= 0.5;
        let top = rows_main.last().unwrap().b1;
        let mut id = 0;
        for k in 0..segs {
            let x0 = -w / 2.0 + k as f64 * sw;
            let zf = if k % 2 == 1 { -p.stagger } else { 0.0 };
            let rect = Rect { x0, x1: x0 + sw, z0: zf - d, z1: zf };
            self.vols.push(Volume { id, rect, rows: rows_main.clone(), y0: 0.0, top, kind: VolKind::Main, has_above: can_pent, along_x });
            id += 1;
            if can_pent {
                let prect = Rect { x0, x1: x0 + sw, z0: zf - d, z1: zf - p.setback_depth };
                let ptop = pent_rows.last().unwrap().b1;
                self.vols.push(Volume { id, rect: prect, rows: pent_rows.clone(), y0: top, top: ptop, kind: VolKind::Pent, has_above: false, along_x });
                id += 1;
            }
        }
        // oficyny (tylko na wysokość części głównej)
        let ww = clamp(p.wing_width, 3.0, (w / 2.0 - 1.0).max(3.0));
        let back_l = self.vols[0].rect.z0;
        let back_r = self.vols.iter().rev().find(|v| v.kind == VolKind::Main).unwrap().rect.z0;
        let ll = p.wing_length * p.wing_left;
        let lr = p.wing_length * p.wing_right;
        let has_l = ll >= 2.0 && w >= ww * 2.0 + 2.0;
        let has_r = lr >= 2.0 && w >= ww * 2.0 + 2.0;
        let rear = has_l && has_r && p.wing_rear >= 0.5;
        let lmin = ll.min(lr);
        let wing = |id, rect, kind, along_x| Volume { id, rect, rows: rows_main.clone(), y0: 0.0, top, kind, has_above: false, along_x };
        if has_l {
            let l = if rear { lmin } else { ll };
            self.vols.push(wing(id, Rect { x0: -w / 2.0, x1: -w / 2.0 + ww, z0: back_l - l, z1: back_l }, VolKind::WingL, false));
            id += 1;
        }
        if has_r {
            let l = if rear { lmin } else { lr };
            self.vols.push(wing(id, Rect { x0: w / 2.0 - ww, x1: w / 2.0, z0: back_r - l, z1: back_r }, VolKind::WingR, false));
            id += 1;
        }
        if rear {
            let zb = back_l.min(back_r) - lmin;
            self.vols.push(wing(id, Rect { x0: -w / 2.0, x1: w / 2.0, z0: zb - ww, z1: zb }, VolKind::Rear, true));
        }
    }

    // ------------------------------------------------------------- odsłonięcie ścian
    fn face_frame(r: &Rect, face: Face) -> Frame {
        match face {
            Face::Front => frame_at([r.x0, 0.0, r.z1], [0.0, 0.0, 1.0]),
            Face::Back => frame_at([r.x1, 0.0, r.z0], [0.0, 0.0, -1.0]),
            Face::Right => frame_at([r.x1, 0.0, r.z1], [1.0, 0.0, 0.0]),
            Face::Left => frame_at([r.x0, 0.0, r.z0], [-1.0, 0.0, 0.0]),
        }
    }

    /// Przedziały ściany niezasłonięte przez inne bryły budynku.
    fn exposed(&self, v: &Volume, skip: Option<usize>, face: Face, y_lo: f64, y_hi: f64) -> Vec<(f64, f64)> {
        let r = v.rect;
        let l = if matches!(face, Face::Front | Face::Back) { r.x1 - r.x0 } else { r.z1 - r.z0 };
        let mut cov: Vec<(f64, f64)> = Vec::new();
        for (i, o) in self.vols.iter().enumerate() {
            if Some(i) == skip {
                continue;
            }
            if y_hi.min(o.top) - y_lo.max(o.y0) < 0.1 {
                continue;
            }
            let q = o.rect;
            match face {
                Face::Front => {
                    if q.z0 <= r.z1 + EPS && q.z1 > r.z1 + EPS {
                        cov.push((q.x0.max(r.x0) - r.x0, q.x1.min(r.x1) - r.x0));
                    }
                }
                Face::Back => {
                    if q.z1 >= r.z0 - EPS && q.z0 < r.z0 - EPS {
                        cov.push((r.x1 - q.x1.min(r.x1), r.x1 - q.x0.max(r.x0)));
                    }
                }
                Face::Right => {
                    if q.x0 <= r.x1 + EPS && q.x1 > r.x1 + EPS {
                        cov.push((r.z1 - q.z1.min(r.z1), r.z1 - q.z0.max(r.z0)));
                    }
                }
                Face::Left => {
                    if q.x1 >= r.x0 - EPS && q.x0 < r.x0 - EPS {
                        cov.push((q.z0.max(r.z0) - r.z0, q.z1.min(r.z1) - r.z0));
                    }
                }
            }
        }
        cov.retain(|(a, b)| b - a > 0.01);
        cov.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut out = Vec::new();
        let mut cur = 0.0f64;
        for (a, b) in cov {
            if a > cur + 0.05 {
                out.push((cur, a));
            }
            cur = cur.max(b);
        }
        if l > cur + 0.05 {
            out.push((cur, l));
        }
        out
    }

    fn role_of(v: &Volume, face: Face) -> Role {
        match v.kind {
            VolKind::Main | VolKind::Pent => match face {
                Face::Front => Role::Front,
                Face::Back => Role::Back,
                _ => Role::Side,
            },
            VolKind::WingL => if face == Face::Right { Role::Court } else { Role::Side },
            VolKind::WingR => if face == Face::Left { Role::Court } else { Role::Side },
            VolKind::Rear => if face == Face::Front { Role::Court } else { Role::Side },
        }
    }

    // ------------------------------------------------------------- wybór osi
    fn pick(&self, n: usize, density: f64, salt: f64) -> Vec<bool> {
        let mut out = vec![false; n];
        if n == 0 || density <= 0.02 {
            return out;
        }
        if density >= 0.97 {
            return vec![true; n];
        }
        let m = jround(1.0 / density).max(1.0) as usize;
        if self.p.asymmetry >= 0.5 {
            let off = (self.h(&[salt, 17.0]) * m as f64).floor() as usize;
            for (i, o) in out.iter_mut().enumerate() {
                *o = (i + off) % m == 0;
            }
        } else if density >= 0.4 {
            for (i, o) in out.iter_mut().enumerate() {
                *o = i.min(n - 1 - i) % m == 0;
            }
        } else {
            let c = (n as f64 - 1.0) / 2.0;
            for (i, o) in out.iter_mut().enumerate() {
                *o = (jround((i as f64 - c).abs()) as usize) % m == 0;
            }
        }
        if !out.iter().any(|&b| b) && density * n as f64 >= 0.5 {
            out[jround((n as f64 - 1.0) / 2.0) as usize] = true;
        }
        out
    }

    // ------------------------------------------------------------- ściany z otworami
    #[allow(clippy::too_many_arguments)]
    fn wall_region(&mut self, f: &Frame, a0: f64, a1: f64, b0: f64, b1: f64, holes: &[Opening], accent: bool, rustic: bool, joints: Option<(bool, bool)>, c: f64) {
        if a1 - a0 < 1e-4 || b1 - b0 < 1e-4 {
            return;
        }
        let mut aa = vec![a0, a1];
        let mut bb = vec![b0, b1];
        for o in holes {
            aa.push(clamp(o.a0, a0, a1));
            aa.push(clamp(o.a1, a0, a1));
            bb.push(clamp(o.b0, b0, b1));
            bb.push(clamp(o.b1, b0, b1));
        }
        if self.plinth > b0 + 1e-3 && self.plinth < b1 - 1e-3 {
            bb.push(self.plinth);
        }
        for v in [&mut aa, &mut bb] {
            v.sort_by(|x, y| x.total_cmp(y));
            v.dedup();
        }
        let panel = self.p.panel_joints > 0.5;
        for i in 0..aa.len() - 1 {
            for j in 0..bb.len() - 1 {
                let ca = (aa[i] + aa[i + 1]) / 2.0;
                let cb = (bb[j] + bb[j + 1]) / 2.0;
                if aa[i + 1] - aa[i] < 1e-4 || bb[j + 1] - bb[j] < 1e-4 {
                    continue;
                }
                if holes.iter().any(|o| ca > o.a0 && ca < o.a1 && cb > o.b0 && cb < o.b1) {
                    continue;
                }
                let mat = if bb[j + 1] <= self.plinth + 1e-3 { M::Plinth } else if accent { M::Accent } else { M::Wall };
                self.mb.rect_n(f, mat, aa[i], aa[i + 1], bb[j], bb[j + 1], c, 1.0);
                // boniowanie: poziome fugi w parterze
                if rustic && mat != M::Plinth {
                    let step = 0.36;
                    let mut y = ((bb[j] + 0.05) / step).ceil() * step;
                    while y < bb[j + 1] - 0.05 {
                        self.mb.box_l(f, M::Joint, aa[i], aa[i + 1], y - 0.018, y + 0.018, c, c + 0.008, true);
                        y += step;
                    }
                }
                if let Some((left, bottom)) = joints
                    && panel
                    && mat != M::Plinth
                {
                    if bottom && (bb[j] - b0).abs() < 1e-4 {
                        self.mb.box_l(f, M::Joint, aa[i], aa[i + 1], bb[j] - 0.018, bb[j] + 0.018, c, c + 0.012, true);
                    }
                    if left && (aa[i] - a0).abs() < 1e-4 {
                        self.mb.box_l(f, M::Joint, aa[i] - 0.018, aa[i] + 0.018, bb[j], bb[j + 1], c, c + 0.012, true);
                    }
                }
            }
        }
    }

    fn plain_wall(&mut self, f: &Frame, a0: f64, a1: f64, b0: f64, b1: f64, holes: &[Opening], accent: bool) {
        self.wall_region(f, a0, a1, b0, b1, holes, accent, false, None, 0.0);
    }

    /// Ościeża, szyba, rama, szprosy.
    fn opening_fill(&mut self, f: &Frame, o: &Opening, lit: bool) {
        let c = 0.0;
        let d = o.depth;
        let (a0, a1, b0, b1) = (o.a0, o.a1, o.b0, o.b1);
        let rev = if self.p.wall_texture == "brick" || self.p.wall_texture == "wood" { M::Trim } else { M::Wall };
        let mb = &mut self.mb;
        mb.rect_u(f, rev, a0, b0, b1, c - d, c, 1.0);
        mb.rect_u(f, rev, a1, b0, b1, c - d, c, -1.0);
        mb.rect_v(f, rev, b1, a0, a1, c - d, c, -1.0);
        let sill = if matches!(o.kind, OpKind::Gate | OpKind::Door | OpKind::GlassDoor) { M::Slab } else { rev };
        mb.rect_v(f, sill, b0, a0, a1, c - d, c, 1.0);
        let cg = c - d;
        match o.kind {
            OpKind::Void => return,
            OpKind::Gate => {
                mb.rect_n(f, M::Door, a0, a1, b0, b1, cg, 1.0);
                // podział wrót
                mb.box_l(f, M::Frame, (a0 + a1) / 2.0 - 0.03, (a0 + a1) / 2.0 + 0.03, b0, b1, cg, cg + 0.04, true);
                return;
            }
            OpKind::Door => {
                let glass_top = b1 - 0.5f64.min((b1 - b0) * 0.22);
                mb.rect_n(f, M::Door, a0, a1, b0, glass_top, cg, 1.0);
                mb.rect_n(f, M::Glass, a0, a1, glass_top, b1, cg, 1.0);
                Self::frame_ring(mb, f, a0, a1, glass_top, b1, cg, 0.05);
                return;
            }
            _ => {}
        }
        mb.rect_n(f, if lit { M::GlassLit } else { M::Glass }, a0, a1, b0, b1, cg, 1.0);
        let fw = match o.kind {
            OpKind::Shop | OpKind::GlassDoor => 0.07,
            OpKind::Basement => 0.04,
            _ => 0.055,
        };
        Self::frame_ring(mb, f, a0, a1, b0, b1, cg, fw);
        // szprosy i słupki
        let w = a1 - a0;
        let h = b1 - b0;
        let m = self.p.mullions;
        let mut bar = |x0, x1, y0, y1| mb.rect_n(f, M::Frame, x0, x1, y0, y1, cg + 0.025, 1.0);
        if matches!(o.kind, OpKind::Shop | OpKind::GlassDoor) {
            let nv = jround(w / 1.3).max(1.0) as i32;
            for k in 1..nv {
                let x = a0 + (w * k as f64) / nv as f64;
                bar(x - 0.035, x + 0.035, b0, b1);
            }
            if h > 2.2 {
                bar(a0, a1, b1 - 0.55, b1 - 0.48);
            }
            return;
        }
        if o.kind == OpKind::Basement {
            return;
        }
        let nv = if w > 2.2 { jround(w / if m > 0.3 { 0.9 } else { 1.4 }).max(1.0) as i32 } else if m >= 0.15 { 2 } else { 1 };
        if m >= 0.15 || w > 2.2 {
            for k in 1..nv {
                let x = a0 + (w * k as f64) / nv as f64;
                bar(x - 0.03, x + 0.03, b0, b1);
            }
        }
        if m >= 0.45 && h > 1.0 {
            let y = b0 + h * 0.7;
            bar(a0, a1, y - 0.03, y + 0.03);
        }
        if m >= 0.85 && h > 1.4 {
            let y = b0 + h * 0.36;
            bar(a0, a1, y - 0.025, y + 0.025);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn frame_ring(mb: &mut MeshBuilder, f: &Frame, a0: f64, a1: f64, b0: f64, b1: f64, c: f64, fw: f64) {
        let cc = c + 0.03;
        mb.rect_n(f, M::Frame, a0, a1, b0, b0 + fw, cc, 1.0);
        mb.rect_n(f, M::Frame, a0, a1, b1 - fw, b1, cc, 1.0);
        mb.rect_n(f, M::Frame, a0, a0 + fw, b0 + fw, b1 - fw, cc, 1.0);
        mb.rect_n(f, M::Frame, a1 - fw, a1, b0 + fw, b1 - fw, cc, 1.0);
    }

    /// Opaski, naczółki, frontony, łuki wokół okna.
    fn window_decor(&mut self, f: &Frame, o: &Opening, orn: f64) {
        let p = self.p;
        let mb = &mut self.mb;
        let (a0, a1, b0, b1) = (o.a0, o.a1, o.b0, o.b1);
        let w = a1 - a0;
        let s = 0.09 + 0.08 * orn;
        let pd = 0.03 + 0.04 * orn;
        // parapet zewnętrzny
        if matches!(o.kind, OpKind::Window | OpKind::Stair) && !o.bd {
            mb.box_l(f, M::Trim, a0 - 0.06, a1 + 0.06, b0 - 0.06, b0, -0.02, 0.07, true);
        }
        if orn > 0.25 {
            mb.box_l(f, M::Trim, a0 - s, a0, b0 - 0.02, b1 + s, 0.0, pd, true);
            mb.box_l(f, M::Trim, a1, a1 + s, b0 - 0.02, b1 + s, 0.0, pd, true);
            mb.box_l(f, M::Trim, a0, a1, b1, b1 + s, 0.0, pd, true);
            if !o.bd {
                mb.box_l(f, M::Trim, a0 - s - 0.05, a1 + s + 0.05, b0 - 0.13, b0 - 0.02, 0.0, pd + 0.04, true);
            }
        }
        let arch_on = p.window_arch > 0.35;
        if orn > 0.5 && !arch_on {
            let yb = b1 + s + 0.02;
            mb.box_l(f, M::Trim, a0 - s - 0.12, a1 + s + 0.12, yb, yb + 0.15, 0.0, pd + 0.1, true);
            if orn > 0.72 && o.floor == 1 {
                let wb = w + 2.0 * s + 0.24;
                let cx = (a0 + a1) / 2.0;
                let y0 = yb + 0.15;
                let hp = 0.4;
                let cf = pd + 0.1;
                let l = to_world(f, cx - wb / 2.0, y0, cf);
                let r = to_world(f, cx + wb / 2.0, y0, cf);
                let t = to_world(f, cx, y0 + hp, cf);
                mb.poly(M::Trim, &[l, r, t], Some(f.n));
                let lb = to_world(f, cx - wb / 2.0, y0, 0.0);
                let rb = to_world(f, cx + wb / 2.0, y0, 0.0);
                let tb = to_world(f, cx, y0 + hp, 0.0);
                mb.quad(M::Trim, l, t, tb, lb, None);
                mb.quad(M::Trim, t, r, rb, tb, None);
            }
        }
        if arch_on {
            let chord = w + 2.0 * s;
            let semi = p.window_arch > 0.75;
            let half = if semi { PI / 2.0 } else { PI / 3.0 };
            let rr = chord / 2.0 / half.sin();
            let cx = (a0 + a1) / 2.0;
            let yc = if semi { b1 } else { b1 - rr * half.cos() };
            let rin = rr;
            let rout = rr + s * 1.1;
            let n = 9;
            let cf = pd + 0.02;
            for k in 0..n {
                let t0 = PI / 2.0 - half + (2.0 * half * k as f64) / n as f64;
                let t1 = PI / 2.0 - half + (2.0 * half * (k + 1) as f64) / n as f64;
                let i0 = to_world(f, cx + t0.cos() * rin, yc + t0.sin() * rin, cf);
                let i1 = to_world(f, cx + t1.cos() * rin, yc + t1.sin() * rin, cf);
                let o0 = to_world(f, cx + t0.cos() * rout, yc + t0.sin() * rout, cf);
                let o1 = to_world(f, cx + t1.cos() * rout, yc + t1.sin() * rout, cf);
                mb.quad(M::Trim, i0, i1, o1, o0, Some(f.n));
                let ob0 = to_world(f, cx + t0.cos() * rout, yc + t0.sin() * rout, 0.0);
                let ob1 = to_world(f, cx + t1.cos() * rout, yc + t1.sin() * rout, 0.0);
                mb.quad(M::Trim, o0, o1, ob1, ob0, None);
            }
            // klucz łuku
            if orn > 0.5 {
                mb.box_l(f, M::Trim, cx - 0.1, cx + 0.1, yc + rin - 0.05, yc + rout + 0.06, 0.0, cf + 0.04, true);
            }
        }
    }

    // ------------------------------------------------------------- balkon / loggia
    fn balcony(&mut self, f: &Frame, cx: f64, bw: f64, row: Row, win_w: f64, win_top: f64) -> Vec<Opening> {
        let p = self.p;
        let bwid = if p.balcony_width >= 0.97 { bw } else { clamp(p.balcony_width * bw, 1.0f64.min(bw), bw) };
        let mut lg = p.loggia;
        let above25 = self.regulations && row.level > 25.0 && self.r.year >= 1990;
        if above25 {
            if lg < 0.85 {
                self.loggia_by_rule = true;
            }
            lg = lg.max(0.85);
        }
        let depth = p.balcony_depth.max(0.6);
        let rec = depth * lg;
        let proj = depth - rec;
        let a0 = cx - bwid / 2.0;
        let a1 = cx + bwid / 2.0;
        let slab_t = 0.16;
        let lvl = row.level;
        // płyta
        self.mb.box_l(f, M::Slab, a0, a1, lvl - slab_t, lvl, if rec > 0.15 { -rec } else { -0.02 }, proj.max(0.02), false);
        let mut holes = Vec::new();
        if rec > 0.15 {
            // wcięcie loggii
            let inset = if bwid >= bw - 0.01 { 0.12 } else { 0.02 };
            let la0 = a0 + inset;
            let la1 = a1 - inset;
            let lb1 = row.b1 - 0.22;
            holes.push(Opening::new(la0, la1, lvl, lb1, OpKind::Void, rec, row.floor));
            self.mb.rect_u(f, M::Wall, la0, lvl, lb1, -rec, 0.0, 1.0);
            self.mb.rect_u(f, M::Wall, la1, lvl, lb1, -rec, 0.0, -1.0);
            self.mb.rect_v(f, M::Slab, lb1, la0, la1, -rec, 0.0, -1.0);
            let back = shift_frame(f, -rec);
            let dw = win_w.min(la1 - la0 - 0.3);
            let door = Opening::new(cx - dw / 2.0, cx + dw / 2.0, lvl + 0.02, win_top.min(lb1 - 0.15), OpKind::Window, 0.1, row.floor);
            self.plain_wall(&back, la0, la1, lvl, lb1, &[door], false);
            let lit = self.h(&[cx * 7.0, row.floor as f64, 3.0]) < 0.3;
            self.opening_fill(&back, &door, lit);
            self.mb.counters.windows += 1;
        } else {
            holes.push(Opening { bd: true, ..Opening::new(cx - win_w / 2.0, cx + win_w / 2.0, lvl + 0.02, win_top, OpKind::Window, p.window_depth, row.floor) });
        }
        // balustrada
        let mut rail = p.railing;
        if p.ornament > 0.5 && row.floor == 1 {
            rail -= 0.35; // „im wyżej, tym lżej”
        }
        // loggie powyżej 25 m wyłącznie z balustradą pełną
        if above25 && (0.33..0.66).contains(&rail) {
            rail = 0.2;
        }
        let cf = if proj > 0.08 { proj } else { 0.04 };
        let hr = 1.05;
        let sides = proj > 0.15;
        let mb = &mut self.mb;
        if rail < 0.33 {
            mb.box_l(f, M::Slab, a0, a1, lvl, lvl + hr, cf - 0.08, cf, false);
            if sides {
                mb.box_l(f, M::Slab, a0, a0 + 0.08, lvl, lvl + hr, 0.0, cf - 0.08, false);
                mb.box_l(f, M::Slab, a1 - 0.08, a1, lvl, lvl + hr, 0.0, cf - 0.08, false);
            }
        } else {
            let mat = if rail < 0.66 { M::Railing } else { M::GlassRail };
            mb.rect_n(f, mat, a0, a1, lvl, lvl + hr - 0.04, cf, 1.0);
            if sides {
                mb.rect_u(f, mat, a0, lvl, lvl + hr - 0.04, 0.0, cf, -1.0);
                mb.rect_u(f, mat, a1, lvl, lvl + hr - 0.04, 0.0, cf, 1.0);
            }
            mb.box_l(f, M::Metal, a0, a1, lvl + hr - 0.05, lvl + hr, cf - 0.03, cf + 0.02, false);
            if sides {
                mb.box_l(f, M::Metal, a0, a0 + 0.04, lvl + hr - 0.05, lvl + hr, 0.0, cf, false);
                mb.box_l(f, M::Metal, a1 - 0.04, a1, lvl + hr - 0.05, lvl + hr, 0.0, cf, false);
            }
        }
        mb.counters.balconies += 1;
        holes
    }

    // ------------------------------------------------------------- elewacja
    fn facade(&mut self, vi: usize, face: Face, s0: f64, s1: f64) {
        let p = self.p;
        let v = self.vols[vi].clone();
        let role = Self::role_of(&v, face);
        let f = Self::face_frame(&v.rect, face);
        let len = s1 - s0;
        let face_ix = FACES.iter().position(|&x| x == face).unwrap() as f64;
        let salt = v.id as f64 * 31.0 + face_ix * 7.0 + jround(s0 * 10.0);
        let is_main_front = role == Role::Front && v.kind == VolKind::Main;
        let is_pent = v.kind == VolKind::Pent;
        let margin = p.corner_margin.min(len * 0.22);
        let usable = len - 2.0 * margin;
        let mut n = jround(usable / p.bay_width.max(0.8)).max(1.0) as usize;
        if usable < 1.1 {
            n = 0;
        }
        let bw = if n > 0 { usable / n as f64 } else { 0.0 };
        let rows = &v.rows;
        let top = v.top;
        let recess = if is_main_front && p.ground_recess > 0.3 { p.ground_recess } else { 0.0 };
        let role_orn = match role {
            Role::Front => 1.0,
            Role::Side => 0.3,
            _ => 0.15,
        };
        let orn = p.ornament * role_orn;

        // akcenty (pasteloza)
        let acc_pattern = (self.h(&[91.0, v.id as f64]) * 4.0).floor() as i32;
        let nrows = rows.len();
        let accent_at = |g: &Self, i: usize, j: usize| -> bool {
            let a = p.accent_panels;
            if a < 0.03 {
                return false;
            }
            let (i, jf) = (i as f64, j as f64);
            match acc_pattern {
                0 => g.h(&[salt, i, 11.0]) < a * 0.55,
                1 => ((i == 0.0 || i == n as f64 - 1.0) && a > 0.25) || g.h(&[salt, i, jf, 12.0]) < a * 0.18,
                2 => ((j == nrows - 1 || j == 0) && a > 0.2) || g.h(&[salt, i, 13.0]) < a * 0.2,
                _ => g.h(&[salt, i, (jf / 2.0).floor(), 14.0]) < a * 0.45,
            }
        };

        // narożniki – pełny mur na całą wysokość
        let edge_accent = acc_pattern == 1 && p.accent_panels > 0.25;
        let rows_b0 = rows[0].b0;
        for (e0, e1) in [(s0, s0 + margin), (s1 - margin, s1)] {
            if e1 - e0 < 1e-3 {
                continue;
            }
            if recess > 0.0 {
                let gt = self.ground_top;
                self.wall_region(&f, e0, e1, rows_b0, gt, &[], false, p.rustication > 0.5, None, 0.0);
                let (a, sg) = if e0 == s0 { (e1, 1.0) } else { (e0, -1.0) };
                self.mb.rect_u(&f, M::Wall, a, rows_b0, gt, -recess, 0.0, sg);
                self.plain_wall(&f, e0, e1, gt, top, &[], edge_accent);
            } else {
                for row in rows {
                    let rustic = row.floor == 0 && p.rustication > 0.5 && role != Role::Side;
                    self.wall_region(&f, e0, e1, row.b0, row.b1, &[], edge_accent, rustic, Some((false, row.floor > 0)), 0.0);
                }
            }
        }
        if n == 0 {
            for row in rows {
                self.plain_wall(&f, s0 + margin, s1 - margin, row.b0, row.b1, &[], false);
            }
            self.decor_strip(&v, &f, role, s0, s1, 0, 0.0, 0.0);
            return;
        }

        let bay_a0 = |i: usize| s0 + margin + i as f64 * bw;
        let shift = if p.asymmetry >= 0.5 && n > 2 { if self.h(&[salt, 5.0]) < 0.5 { -1 } else { 0 } } else { 0 };
        let center_bay = ((n / 2) as i64 + shift).min(n as i64 - 1);
        let entrance_bay = if is_main_front && len >= 2.5 { center_bay } else { -1 };
        let stairs = entrance_bay >= 0 && p.stair_windows >= 0.5 && rows.len() >= 3;
        let mut balc_bays = if matches!(role, Role::Front | Role::Back) { self.pick(n, p.balconies, salt + 1.0) } else { vec![false; n] };
        let mut oriel_bays = if role == Role::Front && !is_pent && n >= 2 { self.pick(n, p.oriels, salt + 2.0) } else { vec![false; n] };
        if entrance_bay >= 0 && stairs {
            balc_bays[entrance_bay as usize] = false;
            oriel_bays[entrance_bay as usize] = false;
        }
        let ww = clamp(lerp(p.window_width, bw - 0.08, p.ribbon), 0.35, bw - if p.ribbon > 0.6 { 0.06 } else { 0.2 });
        let oriel_to = if rows.len() >= 4 { rows[rows.len() - if p.roof_pitch > 3.0 { 1 } else { 2 }].floor } else { -1 };
        let mut decor: Vec<Opening> = Vec::new();

        for i in 0..n {
            let a0 = bay_a0(i);
            let a1 = a0 + bw;
            let cx = (a0 + a1) / 2.0;
            let is_oriel = oriel_bays[i] && oriel_to >= 1;
            for (j, &row) in rows.iter().enumerate() {
                let mut holes: Vec<Opening> = Vec::new();
                let row_h = row.b1 - row.level;
                let mut sill = p.sill_height;
                let mut wh = p.window_height.min(row_h - sill - 0.22);
                if wh < 0.5 {
                    sill = (row_h - 0.22 - 0.5).max(0.1);
                    wh = p.window_height.min(row_h - sill - 0.22);
                }
                let win_top = row.level + sill + wh;
                let ground = row.floor == 0;
                let accent = accent_at(self, i, j);
                let fr = if ground && recess > 0.0 { shift_frame(&f, -recess) } else { f };
                let mut skip_window = false;

                if ground && !is_pent {
                    if i as i64 == entrance_bay {
                        self.entrance(&fr, &mut holes, cx, bw, row, &v);
                    } else if role == Role::Front && self.h(&[salt, i as f64, 21.0]) < p.ground_shop {
                        let sw = (bw - 0.3).min(ww.max(bw * 0.78));
                        holes.push(Opening::new(cx - sw / 2.0, cx + sw / 2.0, row.level + 0.05, row.b1 - 0.45, OpKind::Shop, 0.14, 0));
                        skip_window = true;
                    }
                    if self.plinth >= 0.45 && p.basement_windows > self.h(&[salt, i as f64, 23.0]) && holes.is_empty() {
                        holes.push(Opening::new(cx - 0.33, cx + 0.33, 0.1, (self.plinth - 0.08).min(0.55), OpKind::Basement, 0.12, 0));
                    }
                    if holes.iter().any(|h| h.kind != OpKind::Basement) {
                        skip_window = true;
                    }
                }
                if !ground && stairs && i as i64 == entrance_bay {
                    let sw = 0.95f64.min(bw - 0.4);
                    let y0 = row.level + 0.25;
                    holes.push(Opening::new(cx - sw / 2.0, cx + sw / 2.0, y0, y0 + 1.35f64.min(row_h - 0.55), OpKind::Stair, p.window_depth, row.floor));
                    skip_window = true;
                }
                if !ground && is_oriel && row.floor <= oriel_to {
                    skip_window = true;
                }

                let mut has_win = !skip_window;
                if has_win && role == Role::Side {
                    has_win = self.h(&[salt, i as f64, row.floor as f64, 31.0]) < p.side_windows;
                }
                let too_high = self.regulations && self.r.year >= 1990 && row.level > 55.0;
                if too_high && balc_bays[i] {
                    self.no_balcony_above55 = true;
                }
                if has_win && !ground && balc_bays[i] && row.floor as f64 >= p.balcony_from && !is_pent && !too_high {
                    let hs = self.balcony(&fr, cx, bw, row, ww.min(bw - 0.25), win_top);
                    holes.extend(hs);
                    has_win = false;
                }
                if has_win {
                    holes.push(Opening::new(cx - ww / 2.0, cx + ww / 2.0, row.level + sill, win_top, OpKind::Window, p.window_depth, row.floor));
                }
                let rustic = ground && p.rustication > 0.5 && role != Role::Side;
                self.wall_region(&fr, a0, a1, row.b0, row.b1, &holes, accent, rustic, Some((i > 0, row.floor > 0)), 0.0);
                for hh in &holes {
                    if hh.kind == OpKind::Void {
                        continue;
                    }
                    let lit = self.h(&[salt, i as f64, row.floor as f64, 41.0]) < 0.33;
                    self.opening_fill(&fr, hh, lit);
                    if matches!(hh.kind, OpKind::Window | OpKind::Stair) {
                        self.mb.counters.windows += 1;
                        if !(ground && (p.rustication > 0.5 || recess > 0.0)) {
                            decor.push(*hh);
                        }
                    }
                }
            }
            if is_oriel {
                let y0 = rows.iter().find(|r| r.floor == 1).unwrap().b0;
                let y1 = rows.iter().find(|r| r.floor == oriel_to).unwrap().b1;
                self.oriel(&f, a0 + 0.15, a1 - 0.15, y0, y1, ww, salt + i as f64);
            }
        }

        // podcienia: filary i strop
        if recess > 0.0 {
            let (g0, g1) = (s0 + margin, s1 - margin);
            let gt = self.ground_top;
            self.mb.rect_v(&f, M::Trim, gt, g0, g1, -recess, 0.0, -1.0);
            for i in 1..n {
                let a = bay_a0(i);
                self.mb.box_l(&f, M::Wall, a - 0.3, a + 0.3, 0.0, gt, -recess + 0.02, 0.0, false);
            }
        }

        // ornament okien („im wyżej, tym lżej”)
        for o in &decor {
            let fl = o.floor.max(1) as f64;
            let k = orn * (1.0 - 0.12 * (fl - 1.0));
            self.window_decor(&f, o, k.max(0.0));
        }
        self.decor_strip(&v, &f, role, s0, s1, n, bw, margin);
    }

    /// Gzymsy, lizeny – dekoracje na całym pasie elewacji.
    #[allow(clippy::too_many_arguments)]
    fn decor_strip(&mut self, v: &Volume, f: &Frame, role: Role, s0: f64, s1: f64, n: usize, bw: f64, margin: f64) {
        let p = self.p;
        let plinth = self.plinth;
        let mb = &mut self.mb;
        let top = v.top;
        let rows = &v.rows;
        let role_k = match role {
            Role::Front => 1.0,
            Role::Side => 0.5,
            _ => 0.4,
        };
        let blank = role == Role::Side && p.side_windows < 0.3;
        // gzymsy międzykondygnacyjne
        if v.kind != VolKind::Pent {
            for (k, row) in rows.iter().enumerate().skip(1) {
                let yb = row.b0;
                let first = row.floor == 1;
                let on = if first { p.string_courses > 0.15 || p.rustication > 0.5 } else { p.string_courses > 0.55 || (p.string_courses > 0.3 && k % 2 == 0) };
                if !on || role == Role::Court || blank {
                    continue;
                }
                let hgt = if first { 0.24 } else { 0.14 };
                let pr = (if first { 0.1 } else { 0.07 }) * (0.6 + 0.4 * role_k);
                mb.box_l(f, M::Trim, s0, s1, yb - hgt / 2.0, yb + hgt / 2.0, 0.0, pr, true);
            }
        }
        // gzyms koronujący
        if p.cornice > 0.05 {
            let c = p.cornice * if role == Role::Court { 0.4 } else { 1.0 };
            let ext = if matches!(role, Role::Front | Role::Back) { c } else { 0.0 };
            let h_low = 0.12 + 0.25 * p.cornice;
            mb.box_l(f, M::Trim, s0 - ext * 0.5, s1 + ext * 0.5, top - 0.12 - h_low, top - 0.12, 0.0, c * 0.5, true);
            mb.box_l(f, M::Trim, s0 - ext, s1 + ext, top - 0.12, top + 0.05, 0.0, c, true);
        }
        // lizeny / pilastry
        if p.pilasters > 0.12 && n > 0 && role != Role::Court && !blank && v.kind != VolKind::Pent {
            let pw = 0.3 + 0.18 * p.ornament;
            let pd = 0.05 + 0.05 * p.ornament;
            let y0 = if p.rustication > 0.3 && rows.len() > 1 { rows[1].b0 } else { plinth };
            let y1 = top - if p.cornice > 0.05 { 0.12 + 0.25 * p.cornice } else { 0.0 };
            let every = if p.pilasters > 0.7 { 1 } else { 2 };
            for i in 0..=n {
                let ends = i == 0 || i == n;
                if p.pilasters <= 0.35 && !ends {
                    continue;
                }
                if !ends && i % every != 0 {
                    continue;
                }
                let a = s0 + margin + i as f64 * bw;
                mb.box_l(f, M::Trim, a - pw / 2.0, a + pw / 2.0, y0, y1, 0.0, pd * role_k, true);
            }
        }
    }

    fn entrance(&mut self, f: &Frame, holes: &mut Vec<Opening>, cx: f64, bw: f64, row: Row, v: &Volume) {
        let p = self.p;
        let t = p.entrance_type.as_str();
        let lvl = row.level;
        if t == "gate" {
            let gw = clamp(bw * 0.9, 2.2, 3.4);
            let gh = clamp(row.b1 - 0.45, 2.6, 4.2);
            holes.push(Opening::new(cx - gw / 2.0, cx + gw / 2.0, 0.0, gh, OpKind::Gate, 0.55, 0));
            if p.ornament > 0.3 {
                self.mb.box_l(f, M::Trim, cx - gw / 2.0 - 0.18, cx + gw / 2.0 + 0.18, gh, gh + 0.22, 0.0, 0.08, true);
                self.mb.box_l(f, M::Trim, cx - 0.14, cx + 0.14, gh - 0.1, gh + 0.32, 0.0, 0.12, true);
            }
            return;
        }
        if t == "glass" {
            let gw = clamp(bw * 0.8, 1.8, 3.0);
            holes.push(Opening::new(cx - gw / 2.0, cx + gw / 2.0, lvl, (row.b1 - 0.35).min(lvl + 3.0), OpKind::GlassDoor, 0.16, 0));
        } else {
            let dw = clamp(bw * 0.42, 1.0, 1.5);
            holes.push(Opening::new(cx - dw / 2.0, cx + dw / 2.0, lvl, (row.b1 - 0.35).min(lvl + 2.35), OpKind::Door, 0.22, 0));
        }
        let door_top = holes.last().unwrap().b1;
        // schody przed wejściem (portyk ma własny podest)
        if lvl > 0.22 && t != "portico" {
            let steps = (lvl / 0.17).ceil() as i32;
            let sw = clamp(bw * 0.6, 1.4, 3.0);
            for k in 0..steps {
                let y1 = lvl - k as f64 * (lvl / steps as f64);
                self.mb.box_l(f, M::Slab, cx - sw / 2.0, cx + sw / 2.0, 0.0, y1, 0.0, 0.45 + k as f64 * 0.3, true);
            }
        }
        if t == "portico" {
            self.portico(f, cx, bw, row, v);
            return;
        }
        if p.entrance_canopy > 0.1 {
            let cw = clamp(bw * 0.9, 1.6, 3.6);
            let mat = if t == "glass" { M::Metal } else { M::Slab };
            self.mb.box_l(f, mat, cx - cw / 2.0, cx + cw / 2.0, door_top + 0.18, door_top + 0.34, 0.0, p.entrance_canopy, true);
        }
    }

    fn portico(&mut self, f: &Frame, cx: f64, bw: f64, row: Row, v: &Volume) {
        let mb = &mut self.mb;
        let front = v.rect.x1 - v.rect.x0;
        let pw = clamp((3.0 * bw).min(front * 0.45), 2.4, 9.0);
        let pd = clamp(pw * 0.35, 1.8, 2.8);
        let lvl = row.level;
        let top = row.b1;
        let eh = 0.45;
        // podest ze schodami
        mb.box_l(f, M::Slab, cx - pw / 2.0, cx + pw / 2.0, 0.0, lvl, 0.0, pd, true);
        if lvl > 0.2 {
            let steps = (lvl / 0.17).ceil() as i32;
            for k in 0..steps {
                let y1 = lvl - (k + 1) as f64 * (lvl / steps as f64);
                if y1 <= 0.01 {
                    break;
                }
                mb.box_l(f, M::Slab, cx - pw / 2.0 + 0.3, cx + pw / 2.0 - 0.3, 0.0, y1, pd, pd + 0.32 * (k + 1) as f64, true);
            }
        }
        let cols = if pw >= 4.2 { 4 } else { 2 };
        for k in 0..cols {
            let a = cx - pw / 2.0 + 0.35 + ((pw - 0.7) * k as f64) / (cols - 1) as f64;
            let pp = to_world(f, a, 0.0, pd - 0.35);
            mb.prism(M::Trim, pp[0], pp[2], lvl, top - eh, 0.19, 10, 0.0, false);
            mb.bx(M::Trim, [pp[0] - 0.26, top - eh - 0.12, pp[2] - 0.26], [pp[0] + 0.26, top - eh, pp[2] + 0.26]);
        }
        mb.box_l(f, M::Trim, cx - pw / 2.0, cx + pw / 2.0, top - eh, top, 0.0, pd, true);
        // fronton
        let hp = (pw / 2.0) * rad(24.0).tan();
        let l = to_world(f, cx - pw / 2.0 - 0.1, top, pd + 0.1);
        let r = to_world(f, cx + pw / 2.0 + 0.1, top, pd + 0.1);
        let t = to_world(f, cx, top + hp, pd + 0.1);
        let lb = to_world(f, cx - pw / 2.0 - 0.1, top, -0.3);
        let rb = to_world(f, cx + pw / 2.0 + 0.1, top, -0.3);
        let tb = to_world(f, cx, top + hp, -0.3);
        mb.poly(M::Trim, &[l, r, t], Some(f.n));
        mb.quad(M::Roof, l, t, tb, lb, None);
        mb.quad(M::Roof, t, r, rb, tb, None);
    }

    #[allow(clippy::too_many_arguments)]
    fn oriel(&mut self, f: &Frame, a0: f64, a1: f64, y0: f64, y1: f64, ww: f64, salt: f64) {
        let p = self.p;
        let od = p.oriel_depth;
        let of = shift_frame(f, od);
        let w = ww.min(a1 - a0 - 0.3);
        let cx = (a0 + a1) / 2.0;
        let rows = self.vols[0].rows.clone();
        for row in rows {
            if row.b0 < y0 - 1e-3 || row.b1 > y1 + 1e-3 {
                continue;
            }
            let row_h = row.b1 - row.level;
            let sill = p.sill_height.min(row_h - 1.2);
            let wh = p.window_height.min(row_h - sill - 0.22);
            let o = Opening::new(cx - w / 2.0, cx + w / 2.0, row.level + sill, row.level + sill + wh, OpKind::Window, 0.1, row.floor);
            self.plain_wall(&of, a0, a1, row.b0, row.b1, &[o], false);
            let lit = self.h(&[salt, row.floor as f64, 51.0]) < 0.33;
            self.opening_fill(&of, &o, lit);
            self.window_decor(&of, &o, p.ornament * 0.6);
            self.mb.counters.windows += 1;
        }
        let mb = &mut self.mb;
        mb.rect_u(f, M::Wall, a0, y0, y1, 0.0, od, -1.0);
        mb.rect_u(f, M::Wall, a1, y0, y1, 0.0, od, 1.0);
        mb.rect_v(f, M::Trim, y0, a0, a1, 0.0, od, -1.0);
        mb.box_l(f, M::Trim, a0 - 0.12, a1 + 0.12, y0 - 0.25, y0, 0.0, od + 0.1, true);
        mb.box_l(f, M::Trim, a0 - 0.12, a1 + 0.12, y1, y1 + 0.22, 0.0, od + 0.12, true);
    }

    // ------------------------------------------------------------- dachy
    fn roofs(&mut self) {
        let mut done = vec![false; self.vols.len()];
        for vi in 0..self.vols.len() {
            if done[vi] {
                continue;
            }
            let v = self.vols[vi].clone();
            if v.has_above {
                self.flat_roof(&v, Some(vi), true);
                continue;
            }
            if self.p.roof_pitch < 3.0 {
                self.flat_roof(&v, Some(vi), false);
                continue;
            }
            if matches!(v.kind, VolKind::Main | VolKind::Pent) && v.along_x {
                // sąsiednie segmenty o tej samej głębokości dostają jeden wspólny dach
                let same = |o: &Volume| {
                    o.kind == v.kind && !o.has_above && (o.rect.z0 - v.rect.z0).abs() < 1e-3 && (o.rect.z1 - v.rect.z1).abs() < 1e-3 && (o.top - v.top).abs() < 1e-3
                };
                let mut run: Vec<usize> = (0..self.vols.len()).filter(|&i| same(&self.vols[i])).collect();
                run.sort_by(|&a, &b| self.vols[a].rect.x0.total_cmp(&self.vols[b].rect.x0));
                let mut group: Vec<usize> = Vec::new();
                for o in run {
                    if group.is_empty() || (self.vols[*group.last().unwrap()].rect.x1 - self.vols[o].rect.x0).abs() < 1e-3 {
                        group.push(o);
                    } else if group.contains(&vi) {
                        break;
                    } else {
                        group = vec![o];
                    }
                }
                if !group.contains(&vi) {
                    group = vec![vi];
                }
                for &o in &group {
                    done[o] = true;
                }
                let mut merged = v.clone();
                merged.rect = Rect { x0: self.vols[group[0]].rect.x0, x1: self.vols[*group.last().unwrap()].rect.x1, z0: v.rect.z0, z1: v.rect.z1 };
                // scalona bryła nie jest żadną z istniejących (jak nowy obiekt w JS)
                self.pitched_roof(&merged, None);
                continue;
            }
            self.pitched_roof(&v, Some(vi));
        }
    }

    fn parapet_along(&mut self, v: &Volume, vi: Option<usize>, hgt: f64, rail: bool) {
        let t = 0.25;
        for face in FACES {
            let f = Self::face_frame(&v.rect, face);
            for (s0, s1) in self.exposed(v, vi, face, v.top - 0.05, v.top + hgt) {
                let mb = &mut self.mb;
                if rail {
                    let mat = if self.p.railing >= 0.66 { M::GlassRail } else { M::Railing };
                    mb.rect_n(&f, mat, s0, s1, v.top, v.top + 1.05, -0.12, 1.0);
                    mb.box_l(&f, M::Metal, s0, s1, v.top + 1.0, v.top + 1.06, -0.16, -0.08, false);
                } else {
                    mb.box_l(&f, M::Wall, s0, s1, v.top, v.top + hgt, -t, 0.0, false);
                    mb.box_l(&f, M::Trim, s0 - 0.03, s1 + 0.03, v.top + hgt, v.top + hgt + 0.07, -t - 0.03, 0.05, false);
                }
            }
        }
    }

    fn flat_roof(&mut self, v: &Volume, vi: Option<usize>, terrace: bool) {
        let p = self.p;
        let r = v.rect;
        let mut y = v.top;
        if !terrace && p.roof_overhang > 0.12 && p.parapet_height < 0.15 {
            let o = p.roof_overhang;
            self.mb.bx(M::Slab, [r.x0 - o, y, r.z0 - o], [r.x1 + o, y + 0.24, r.z1 + o]);
            y += 0.24;
        }
        let yy = y + 0.01;
        self.mb.quad(M::Roof, [r.x0, yy, r.z1], [r.x1, yy, r.z1], [r.x1, yy, r.z0], [r.x0, yy, r.z0], Some([0.0, 1.0, 0.0]));
        if terrace {
            self.parapet_along(v, vi, 1.05, true);
            return;
        }
        if p.parapet_height > 0.05 {
            self.parapet_along(v, vi, p.parapet_height, false);
        }
        self.ridge_top = self.ridge_top.max(y + p.parapet_height);
        if matches!(v.kind, VolKind::Main | VolKind::Pent) {
            let cx = (r.x0 + r.x1) / 2.0;
            let cz = (r.z0 + r.z1) / 2.0;
            if p.rooftop_boxes >= 0.6 {
                let bx = 3.2f64.min((r.x1 - r.x0) * 0.3);
                let bz = 3.8f64.min((r.z1 - r.z0) * 0.4);
                self.mb.bx(M::Wall, [cx - bx / 2.0, y, cz - bz / 2.0], [cx + bx / 2.0, y + 2.6, cz + bz / 2.0]);
                self.mb.bx(M::Trim, [cx - bx / 2.0 - 0.08, y + 2.6, cz - bz / 2.0 - 0.08], [cx + bx / 2.0 + 0.08, y + 2.75, cz + bz / 2.0 + 0.08]);
                self.ridge_top = self.ridge_top.max(y + 2.75);
            } else if p.rooftop_boxes >= 0.2 {
                self.mb.bx(M::Wall, [cx - 0.9, y, cz - 1.2], [cx + 0.9, y + 1.6, cz + 1.2]);
            }
        }
        if p.chimneys > 0.1 {
            let area = (r.x1 - r.x0) * (r.z1 - r.z0);
            let cnt = 12.0f64.min(jround((p.chimneys * area) / 70.0)) as i32;
            for k in 0..cnt {
                let x = lerp(r.x0 + 1.0, r.x1 - 1.0, self.h(&[v.id as f64, k as f64, 61.0]));
                let z = lerp(r.z0 + 1.0, r.z1 - 1.0, self.h(&[v.id as f64, k as f64, 62.0]));
                self.mb.bx(M::Plinth, [x - 0.3, y, z - 0.5], [x + 0.3, y + 1.0, z + 0.5]);
            }
        }
    }

    fn pitched_roof(&mut self, v: &Volume, vi: Option<usize>) {
        let p = self.p;
        let r = v.rect;
        let hh = v.top;
        let along_x = v.along_x;
        let hl = (if along_x { r.x1 - r.x0 } else { r.z1 - r.z0 }) / 2.0;
        let hw = (if along_x { r.z1 - r.z0 } else { r.x1 - r.x0 }) / 2.0;
        let cx = (r.x0 + r.x1) / 2.0;
        let cz = (r.z0 + r.z1) / 2.0;
        // (s, t, y) → świat; s wzdłuż kalenicy, t w poprzek
        let w3 = |s: f64, t: f64, y: f64| -> V3 { if along_x { [cx + s, y, cz + t] } else { [cx + t, y, cz + s] } };
        let dir = |a: f64, b: f64, c: f64| -> Option<V3> { Some([a, b, c]) };
        let th = rad(clamp(p.roof_pitch, 3.0, 70.0));
        let m = p.mansard;
        let stages: Vec<(f64, f64)> = if m > 0.05 {
            let w1 = hw * (0.12 + 0.2 * m);
            let steep = rad(60.0 + 18.0 * m);
            let th_u = rad(10.0).max(th * (1.0 - 0.55 * m));
            vec![(w1, w1 * steep.tan()), (hw - w1, (hw - w1) * th_u.tan())]
        } else {
            vec![(hw, hw * th.tan())]
        };
        let hip = if p.roof_hip < 0.15 { 0.0 } else { p.roof_hip };
        let mut ov = if p.parapet_height > 0.2 { -0.2 } else { p.roof_overhang };
        if m > 0.05 {
            ov = ov.min(0.25);
        }
        let slope0 = stages[0].1 / stages[0].0;
        // poziomy połaci (z okapem): (L, Wd, y)
        let mut lv: Vec<(f64, f64, f64)> = vec![(hl + ov, hw + ov, hh - ov * slope0.min(0.75))];
        let (mut l, mut wd, mut y) = (hl, hw, hh);
        for &(sw, sh) in &stages {
            wd -= sw;
            l = (l - hip * sw).max(0.0);
            y += sh;
            lv.push((if hip > 0.0 { l } else { hl + ov }, wd.max(0.0), y));
        }
        let ridge_y = y;
        self.ridge_top = self.ridge_top.max(ridge_y);
        for k in 0..lv.len() - 1 {
            let (a, b) = (lv[k], lv[k + 1]);
            for sg in [1.0, -1.0] {
                let pts = [w3(-a.0, sg * a.1, a.2), w3(a.0, sg * a.1, a.2), w3(b.0, sg * b.1, b.2), w3(-b.0, sg * b.1, b.2)];
                self.mb.poly(M::Roof, &pts, if along_x { dir(0.0, 1.0, sg) } else { dir(sg, 1.0, 0.0) });
                if hip > 0.0 {
                    let pts = [w3(sg * a.0, -a.1, a.2), w3(sg * a.0, a.1, a.2), w3(sg * b.0, b.1, b.2), w3(sg * b.0, -b.1, b.2)];
                    self.mb.poly(M::Roof, &pts, if along_x { dir(sg, 1.0, 0.0) } else { dir(0.0, 1.0, sg) });
                }
            }
        }
        // szczyty (ściany szczytowe) dla dachu dwuspadowego
        if hip == 0.0 {
            let mut prof: Vec<(f64, f64)> = vec![(-hw, hh)];
            let (mut wacc, mut yacc) = (hw, hh);
            for &(sw, sh) in &stages {
                wacc -= sw;
                yacc += sh;
                prof.push((-(wacc.max(0.0)), yacc));
            }
            for k in (0..stages.len()).rev() {
                prof.push((-prof[k].0, prof[k].1));
            }
            for sg in [1.0, -1.0] {
                let face = if along_x { if sg > 0.0 { Face::Right } else { Face::Left } } else if sg > 0.0 { Face::Front } else { Face::Back };
                let ex = self.exposed(v, vi, face, v.y0, v.top);
                let ex_len: f64 = ex.iter().map(|(a, b)| b - a).sum();
                if ex_len < hw * 2.0 * 0.5 {
                    continue;
                }
                let gw = if along_x { dir(sg, 0.0, 0.0) } else { dir(0.0, 0.0, sg) };
                let pts: Vec<V3> = prof.iter().map(|&(t, yy)| w3(sg * hl, t, yy)).collect();
                self.mb.poly(M::Wall, &pts, gw);
                let gg = p.gable_glazing;
                if gg > 0.05 {
                    let rt = ridge_y - hh;
                    let hb = 0.05;
                    let ht = rt * gg * 0.8;
                    let half_b = hw * gg * 0.92;
                    let half_t = half_b.min(hw * (1.0 - (ht + 0.25) / rt) - 0.1);
                    if half_t > 0.1 {
                        let so = sg * (hl + 0.03);
                        let pts = [w3(so, -half_b, hh + hb), w3(so, half_b, hh + hb), w3(so, half_t, hh + ht), w3(so, -half_t, hh + ht)];
                        self.mb.poly(M::Glass, &pts, gw);
                        self.mb.counters.windows += 1;
                    }
                }
            }
        }
        // kominy na kalenicy
        if p.chimneys > 0.1 {
            let ridge_l = lv.last().unwrap().0;
            let span = if hip > 0.0 { ridge_l } else { hl * 0.8 };
            let cnt = jround((p.chimneys * (2.0 * hl)) / 6.0).max(1.0) as i32;
            for k in 0..cnt {
                let s = if cnt == 1 { 0.0 } else { -span + (2.0 * span * k as f64) / (cnt - 1) as f64 };
                let pp = w3(s, 0.3, 0.0);
                self.mb.bx(M::Plinth, [pp[0] - 0.3, ridge_y - 0.9, pp[2] - 0.4], [pp[0] + 0.3, ridge_y + 0.85, pp[2] + 0.4]);
                self.mb.bx(M::Trim, [pp[0] - 0.36, ridge_y + 0.85, pp[2] - 0.46], [pp[0] + 0.36, ridge_y + 0.95, pp[2] + 0.46]);
            }
        }
        // lukarny na połaciach wzdłuż kalenicy
        if p.dormers > 0.05 && matches!(v.kind, VolKind::Main | VolKind::Rear) && hw > 2.4 && p.roof_pitch >= 22.0 {
            let steep = m > 0.4;
            let tf = if steep { 0.2 } else { 0.8 };
            let y_on = |t: f64| {
                let (mut yy, mut tt) = (hh, t);
                for &(sw, sh) in &stages {
                    let d = tt.min(sw);
                    yy += (d * sh) / sw;
                    tt -= d;
                    if tt <= 0.0 {
                        break;
                    }
                }
                yy
            };
            let yb = y_on(tf);
            let dh = if steep { 1.6f64.min(stages[0].1 * 0.8) } else { 1.35 };
            let slope = stages[0].1 / stages[0].0;
            let depth = clamp(dh / slope.max(0.3) + 0.3, 0.6, 3.0);
            let nb = jround((2.0 * hl - 1.0) / p.bay_width.max(2.2)).max(1.0) as usize;
            let picks = self.pick(nb, p.dormers, v.id as f64 + 77.0);
            let dw = clamp(p.bay_width * 0.5, 0.9, 1.6);
            for sg in [1.0, -1.0] {
                for (i, &picked) in picks.iter().enumerate() {
                    if !picked {
                        continue;
                    }
                    let s = -hl + ((i as f64 + 0.5) * 2.0 * hl) / nb as f64;
                    if hip > 0.0 && s.abs() + dw > lv[1].0 {
                        continue;
                    }
                    let t_front = hw - tf;
                    let t_back = (t_front - depth).max(0.2);
                    let s0 = s - dw / 2.0;
                    let s1 = s + dw / 2.0;
                    let y_top = yb + dh;
                    let face_dir = if along_x { dir(0.0, 0.0, sg) } else { dir(sg, 0.0, 0.0) };
                    // ściany lukarny
                    let mb = &mut self.mb;
                    mb.poly(M::Wall, &[w3(s0, sg * t_front, yb - 0.2), w3(s1, sg * t_front, yb - 0.2), w3(s1, sg * t_front, y_top), w3(s0, sg * t_front, y_top)], face_dir);
                    mb.poly(
                        M::Wall,
                        &[w3(s0, sg * t_front, yb - 0.2), w3(s0, sg * t_front, y_top), w3(s0, sg * t_back, y_top), w3(s0, sg * t_back, yb - 0.2)],
                        if along_x { dir(-1.0, 0.0, 0.0) } else { dir(0.0, 0.0, -1.0) },
                    );
                    mb.poly(
                        M::Wall,
                        &[w3(s1, sg * t_front, yb - 0.2), w3(s1, sg * t_back, yb - 0.2), w3(s1, sg * t_back, y_top), w3(s1, sg * t_front, y_top)],
                        if along_x { dir(1.0, 0.0, 0.0) } else { dir(0.0, 0.0, 1.0) },
                    );
                    let g0 = s0 + 0.16;
                    let g1 = s1 - 0.16;
                    let tg = sg * (t_front + 0.02);
                    mb.poly(M::Glass, &[w3(g0, tg, yb + 0.12), w3(g1, tg, yb + 0.12), w3(g1, tg, y_top - 0.18), w3(g0, tg, y_top - 0.18)], face_dir);
                    mb.counters.windows += 1;
                    // daszek lukarny
                    let hr = (dw / 2.0 + 0.12) * rad(38.0).tan();
                    let t_f = sg * (t_front + 0.12);
                    let t_b = sg * t_back;
                    let a1 = w3(s0 - 0.12, t_f, y_top);
                    let a2 = w3(s, t_f, y_top + hr);
                    let a3 = w3(s1 + 0.12, t_f, y_top);
                    let b1 = w3(s0 - 0.12, t_b, y_top);
                    let b2 = w3(s, t_b, y_top + hr);
                    let b3 = w3(s1 + 0.12, t_b, y_top);
                    mb.poly(M::Roof, &[a1, a2, b2, b1], None);
                    mb.poly(M::Roof, &[a2, a3, b3, b2], None);
                    mb.poly(M::Wall, &[a1, a3, a2], face_dir);
                }
            }
        }
    }

    fn turret(&mut self) {
        let p = self.p;
        if p.turret < 0.5 {
            return;
        }
        let right = self.h(&[97.0]) < 0.5;
        let mains: Vec<&Volume> = self.vols.iter().filter(|v| v.kind == VolKind::Main).collect();
        let v = (if right { mains[mains.len() - 1] } else { mains[0] }).clone();
        let r = v.rect;
        let rt = clamp(p.bay_width * 0.55, 1.2, 2.1);
        let x = if right { r.x1 - rt * 0.45 } else { r.x0 + rt * 0.45 };
        let z = r.z1 - rt * 0.45;
        let y0 = if v.rows.len() > 2 { self.ground_top } else { 0.0 };
        let roof_rise = if p.roof_pitch >= 3.0 { ((r.z1 - r.z0).min(r.x1 - r.x0) / 2.0) * rad(p.roof_pitch).tan() * 0.6 } else { 1.2 };
        let y1 = v.top + roof_rise.max(1.2);
        let mb = &mut self.mb;
        let rot = PI / 8.0;
        mb.prism(M::Wall, x, z, y0, y1, rt, 8, rot, true);
        mb.prism(M::Trim, x, z, y1 - 0.05, y1 + 0.2, rt + 0.12, 8, rot, true);
        mb.pyramid(M::Roof, x, z, y1 + 0.2, rt * 2.1, rt + 0.18, 8, rot);
        self.ridge_top = self.ridge_top.max(y1 + 0.2 + rt * 2.1);
        // okna wieżyczki
        let side = 2.0 * rt * (PI / 8.0).sin();
        for k in 0..8 {
            let t0 = PI / 8.0 + (k as f64 / 8.0) * PI * 2.0;
            let t1 = PI / 8.0 + ((k + 1) as f64 / 8.0) * PI * 2.0;
            let nx = ((t0 + t1) / 2.0).cos();
            let nz = ((t0 + t1) / 2.0).sin();
            if nz < 0.2 && (if right { nx < 0.2 } else { nx > -0.2 }) {
                continue;
            }
            let (ax, az) = (x + t0.cos() * rt, z + t0.sin() * rt);
            let (bx, bz) = (x + t1.cos() * rt, z + t1.sin() * rt);
            let f = frame_at([ax + nx * 0.02, 0.0, az + nz * 0.02], [nx, 0.0, nz]);
            let (dx, dz) = (bx - ax, bz - az);
            let dl = dx.hypot(dz);
            let ff = Frame { u: [dx / dl, 0.0, dz / dl], ..f };
            for row in &v.rows {
                if row.b0 < y0 - 1e-3 {
                    continue;
                }
                let wh = p.window_height.min(row.b1 - row.level - 1.0);
                if wh < 0.6 {
                    continue;
                }
                let w = side * 0.5;
                self.mb.rect_n(&ff, M::Glass, side / 2.0 - w / 2.0, side / 2.0 + w / 2.0, row.level + 0.85, row.level + 0.85 + wh, 0.0, 1.0);
                self.mb.counters.windows += 1;
            }
        }
    }

    // ------------------------------------------------------------- przebieg
    fn run(mut self) -> (MeshData, BuildingStats) {
        self.layout();
        for vi in 0..self.vols.len() {
            for face in FACES {
                let v = self.vols[vi].clone();
                for (s0, s1) in self.exposed(&v, Some(vi), face, v.y0, v.top) {
                    self.facade(vi, face, s0, s1);
                }
            }
        }
        self.roofs();
        self.turret();
        let stats_base = self.stats();
        let windows = self.mb.counters.windows;
        let balconies = self.mb.counters.balconies;
        let mesh = std::mem::take(&mut self.mb).build(build_materials(self.p));
        let stats = BuildingStats { windows, balconies, triangles: mesh.triangles(), ..stats_base };
        (mesh, stats)
    }

    fn stats(&self) -> BuildingStats {
        let p = self.p;
        let area = |v: &Volume| (v.rect.x1 - v.rect.x0) * (v.rect.z1 - v.rect.z0);
        let footprint: f64 = self.vols.iter().filter(|v| v.kind != VolKind::Pent).map(area).sum();
        let gfa: f64 = self.vols.iter().map(|v| area(v) * v.rows.len() as f64).sum();
        let height_walls = self.vols.iter().map(|v| v.top).fold(f64::NEG_INFINITY, f64::max);
        let height_total = height_walls.max(self.ridge_top);
        let floors = self.floors();
        let h = if p.roof_pitch < 3.0 { height_walls + p.parapet_height } else { height_total };
        let height_class = if h <= 12.0 || floors <= 4 {
            "N"
        } else if h <= 25.0 || floors <= 9 {
            "SW"
        } else if h <= 55.0 || floors <= 18 {
            "W"
        } else {
            "WW"
        };
        let year = self.r.year as f64;
        let single = floors <= 2 && jround(p.segments) == 1.0 && footprint < 260.0;
        let avg_flat = match year {
            y if y < 1918.0 => 62.0,
            y if y < 1945.0 => 58.0,
            y if y < 1959.0 => 50.0,
            y if y < 1974.0 => 42.0,
            y if y < 1990.0 => 52.0,
            y if y < 2008.0 => 58.0,
            _ => 64.0,
        };
        let flats = if single { 1.0 } else { jround((gfa * 0.72) / avg_flat).max(1.0) };
        let mut notes = Vec::new();
        if height_class == "N" {
            notes.push("Budynek niski (N) wg WT 2002: do 12 m lub do 4 kondygnacji.".to_string());
        } else {
            notes.push(format!("Grupa wysokości {height_class} wg WT 2002 (wysokość ok. {} m).", fixed1(h, true)));
        }
        if floors > 5 && (1950.0..1990.0).contains(&year) && p.rooftop_boxes < 0.3 && !single {
            notes.push("W PRL budynki powyżej 5 kondygnacji miały windy — dodaj maszynownię (nadbudówki).".to_string());
        }
        let max_level = self.vols.iter().flat_map(|v| v.rows.iter().map(|r| r.level)).fold(f64::NEG_INFINITY, f64::max);
        let level_diff = max_level - self.plinth;
        if year >= 2002.0 && !single && level_diff > 9.5 {
            notes.push(format!(
                "WT 2002 § 54: różnica poziomów posadzek parteru i najwyższej kondygnacji ({} m) przekracza 9,5 m — budynek wielorodzinny wymaga windy.",
                fixed1(level_diff, true)
            ));
        }
        if self.loggia_by_rule {
            notes.push("WT 2002 § 303: powyżej 25 m nad terenem zakaz balkonów — zamieniono je na loggie z balustradą pełną.".to_string());
        }
        if self.no_balcony_above55 {
            notes.push("WT 2002 § 303: powyżej 55 m nad terenem nie stosuje się także loggii.".to_string());
        }
        BuildingStats {
            height_walls,
            height_total,
            floors: floors as u32,
            height_class,
            footprint,
            gfa,
            frontage: self.frontage,
            depth: p.depth,
            staircases: if single { 1 } else { jround(p.segments) as u32 },
            windows: 0,
            balconies: 0,
            flats_estimate: flats as u32,
            triangles: 0,
            notes,
        }
    }
}

pub fn generate_building(resolved: ResolvedBuilding) -> BuildingModel {
    let (mesh, stats) = Gen::new(&resolved).run();
    BuildingModel { resolved, mesh, stats }
}

pub fn generate_from_spec(spec: &BuildingSpec) -> BuildingModel {
    generate_building(resolve_spec(spec))
}
