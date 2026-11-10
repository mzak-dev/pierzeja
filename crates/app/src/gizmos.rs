//! Gizmo edycji rysowane painterem egui w widoku 3D (rzut `proj` 3D→ekran), osobne dla budynku,
//! pojazdu i człowieka: linie wymiarowe, uchwyty do przeciągania (zmieniają nadpisania parametrów
//! w `Doc`, więc działają z historią cofania jak każda inna edycja), znaczniki kotwic GLB i szkielet.
//! Włącza je przycisk w pasku widoku (ikona „wektor”) oraz klawisz G; kategorie w menu obok.

use crate::theme::*;
use crate::ui::{dim_label, Cmd};
use crate::*;
use bevy_egui::egui::{self, Align2, Color32, Id, LayerId, Order, Pos2, Rect, Sense, Stroke, Vec2};
use procedura_kit::data::Dataset;
use tlum_core::Joint;

// ------------------------------------------------------------------ dane modelu dla gizmo

/// Migawka modelu pojazdu lub człowieka (to, czego nie ma w `Derived::resolved`).
#[derive(Default, Clone)]
pub enum Snap {
    #[default]
    None,
    Vehicle(VehSnap),
    Human(HumSnap),
}

#[derive(Clone)]
pub struct VehSnap {
    pub wheels: Vec<(&'static str, [f64; 3])>,
    pub seats: Vec<[f64; 3]>,
    /// Belka świetlna: środek i połowy rozmiarów (od przesunięcia węzła `Lightbar`).
    pub lightbar: Option<([f64; 3], [f64; 3])>,
    pub body_type: String,
    pub wheelbase: f64,
    pub track: f64,
    pub wheel_radius: f64,
}

#[derive(Clone)]
pub struct HumSnap {
    pub joints: Vec<Joint>,
    pub hip: [f64; 3],
}

impl Snap {
    pub fn vehicle(m: &tabor_core::VehicleModel) -> Snap {
        let lightbar = m.lightbar.as_ref().map(|(mesh, o)| {
            let b = mesh.bounds;
            (std::array::from_fn(|k| o[k] + (b.min[k] + b.max[k]) / 2.0), std::array::from_fn(|k| (b.max[k] - b.min[k]) / 2.0))
        });
        Snap::Vehicle(VehSnap {
            wheels: m.wheels.to_vec(),
            seats: m.seats.clone(),
            lightbar,
            body_type: m.stats.body_type.clone(),
            wheelbase: m.stats.wheelbase,
            track: m.stats.track,
            wheel_radius: m.stats.wheel_radius,
        })
    }
    pub fn human(m: &tlum_core::HumanModel) -> Snap {
        Snap::Human(HumSnap { joints: m.joints.clone(), hip: m.hip })
    }
}

// ------------------------------------------------------------------ przycisk w pasku widoku

/// Przełącznik gizmo (G) i menu kategorii (wymiary / kotwice / szkielet).
pub fn toolbar_button(ui: &mut egui::Ui, mode: Mode, view: &mut View, cmds: &mut Vec<Cmd>) {
    if icon_btn(ui, ic::VECTOR_TWO, view.gizmos, "Gizmo (G)").clicked() {
        cmds.push(Cmd::Gizmos);
    }
    let caret = icon_btn(ui, ic::CARET_UP, false, "Kategorie gizmo");
    egui::Popup::menu(&caret).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_min_width(190.0);
        ui.add_enabled_ui(view.gizmos, |ui| {
            ui.checkbox(&mut view.gz_dims, "Wymiary i uchwyty");
            ui.checkbox(&mut view.gz_anchors, if mode == Mode::Building { "Front i znaczniki" } else { "Kotwice GLB i punkty" });
            if mode == Mode::Human {
                ui.checkbox(&mut view.gz_skeleton, "Szkielet");
            }
        });
        if !view.gizmos {
            ui.label(egui::RichText::new("Gizmo wyłączone (G)").size(12.0).color(DIM));
        }
    });
}

// ------------------------------------------------------------------ rysowanie

struct G<'a> {
    ctx: &'a egui::Context,
    p: egui::Painter,
    proj: &'a dyn Fn(Vec3) -> Option<Pos2>,
    vp: Rect,
}

fn w(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x as f32, y as f32, z as f32)
}

fn v3(a: [f64; 3]) -> Vec3 {
    w(a[0], a[1], a[2])
}

impl G<'_> {
    fn pt(&self, p: Vec3) -> Option<Pos2> {
        (self.proj)(p).filter(|q| self.vp.expand(300.0).contains(*q))
    }
    fn line(&self, a: Vec3, b: Vec3, stroke: Stroke) {
        if let (Some(a), Some(b)) = (self.pt(a), self.pt(b)) {
            self.p.line_segment([a, b], stroke);
        }
    }
    fn dashed(&self, a: Vec3, b: Vec3, stroke: Stroke) {
        if let (Some(a), Some(b)) = (self.pt(a), self.pt(b)) {
            self.p.extend(egui::Shape::dashed_line(&[a, b], stroke, 6.0, 4.0));
        }
    }
    fn dot(&self, at: Vec3, r: f32, col: Color32) {
        if let Some(a) = self.pt(at) {
            self.p.circle_filled(a, r, col);
        }
    }
    /// Podpis z ciemnym konturem (czytelny na jasnym i ciemnym tle).
    fn tag(&self, at: Pos2, align: Align2, text: &str, col: Color32) {
        self.p.text(at + Vec2::new(1.0, 1.0), align, text, mono(11.0), Color32::from_black_alpha(210));
        self.p.text(at, align, text, mono(11.0), col);
    }
    fn tag3(&self, at: Vec3, off: Vec2, align: Align2, text: &str, col: Color32) {
        if let Some(a) = self.pt(at) {
            self.tag(a + off, align, text, col);
        }
    }
    /// Linia wymiarowa z liniami pomocniczymi (przesunięcie `off` w świecie), znacznikami końców i etykietą.
    fn dim(&self, a: Vec3, b: Vec3, off: Vec3, label: &str, col: Color32) {
        self.dim_at(a, b, off, label, col, 0.5, Vec2::ZERO);
    }
    /// Jak `dim`, z położeniem etykiety wzdłuż linii (`t` 0–1) i przesunięciem w pikselach.
    #[allow(clippy::too_many_arguments)]
    fn dim_at(&self, a: Vec3, b: Vec3, off: Vec3, label: &str, col: Color32, t: f32, shift: Vec2) {
        let (Some(a0), Some(b0), Some(a1), Some(b1)) = (self.pt(a), self.pt(b), self.pt(a + off), self.pt(b + off)) else { return };
        let thin = Stroke::new(1.0, col.gamma_multiply(0.45));
        self.p.line_segment([a0, a1], thin);
        self.p.line_segment([b0, b1], thin);
        self.p.line_segment([a1, b1], Stroke::new(1.5, col));
        let d = b1 - a1;
        if d.length() > 1.0 {
            let n = Vec2::new(-d.y, d.x).normalized() * 4.5;
            for e in [a1, b1] {
                self.p.line_segment([e - n, e + n], Stroke::new(1.5, col));
            }
            if !label.is_empty() {
                dim_label(&self.p, a1.lerp(b1, t) + Vec2::new(0.0, 12.0) + shift, label, col);
            }
        }
    }
    fn arrow(&self, a: Vec3, b: Vec3, col: Color32) {
        let (Some(a), Some(b)) = (self.pt(a), self.pt(b)) else { return };
        let d = (b - a).normalized();
        let n = Vec2::new(-d.y, d.x);
        self.p.line_segment([a, b], Stroke::new(2.5, col));
        self.p.add(egui::Shape::convex_polygon(vec![b + d * 4.0, b - d * 10.0 + n * 6.0, b - d * 10.0 - n * 6.0], col, Stroke::NONE));
    }

    /// Uchwyt do przeciągania wzdłuż osi świata `dir`. Zwraca (wartość na początku przeciągania,
    /// przesunięcie w metrach od początku) w każdej klatce przeciągania.
    fn handle(&self, id: &str, at: Vec3, dir: Vec3, cur: f64, col: Color32, tip: &str) -> Option<(f64, f64)> {
        let a = self.pt(at)?;
        let per_m = (self.proj)(at + dir)? - a;
        let aid = Id::new(("gizmo-handle", id));
        let dragging = self.ctx.data(|d| d.get_temp::<bool>(aid.with("on"))).unwrap_or(false);
        // pod panelem (nie nad nim): uchwyt się chowa
        if !dragging && self.ctx.layer_id_at(a).is_some_and(|l| l.order != Order::Background && l.id != aid) {
            return None;
        }
        let size = 22.0;
        let mut out = None;
        let enabled = per_m.length() > 3.0;
        egui::Area::new(aid).fixed_pos(a - Vec2::splat(size / 2.0)).order(Order::Middle).constrain(false).show(self.ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(size), if enabled { Sense::drag() } else { Sense::hover() });
            let hot = enabled && (resp.hovered() || resp.dragged());
            let c = r.center();
            let p = ui.painter();
            p.circle(c, if hot { 8.5 } else { 6.5 }, Color32::from_rgb(0x20, 0x20, 0x20), Stroke::new(2.0, if enabled { col } else { FAINT }));
            p.circle_filled(c, 2.5, if enabled { col } else { FAINT });
            if enabled {
                // strzałki wzdłuż osi przeciągania
                let d = per_m.normalized();
                for s in [-1.0, 1.0] {
                    p.line_segment([c + d * s * 11.0, c + d * s * 15.0], Stroke::new(2.0, col.gamma_multiply(if hot { 1.0 } else { 0.55 })));
                }
            }
            let resp = resp.on_hover_text(tip);
            if resp.drag_started() {
                self.ctx.data_mut(|d| d.insert_temp(aid, (cur, 0.0f64)));
            }
            self.ctx.data_mut(|d| d.insert_temp(aid.with("on"), resp.dragged()));
            if resp.dragged() {
                let (s, t) = self.ctx.data(|d| d.get_temp::<(f64, f64)>(aid)).unwrap_or((cur, 0.0));
                let t = t + (resp.drag_delta().dot(per_m) / per_m.length_sq()) as f64;
                self.ctx.data_mut(|d| d.insert_temp(aid, (s, t)));
                out = Some((s, t));
            }
        });
        out
    }
}

/// Ustawia parametr z zachowaniem zakresu i kroku z definicji w danych.
fn set(doc: &mut Doc, ds: &Dataset, key: &str, v: f64) {
    let Some(def) = ds.param(key) else { return };
    let mut v = v.clamp(def.min.unwrap_or(f64::MIN), def.max.unwrap_or(f64::MAX));
    if let Some(st) = def.step.filter(|s| *s > 0.0) {
        v = (v / st).round() * st;
    }
    if matches!(def.kind, pierzeja_core::ParamKind::Int) {
        v = v.round();
    }
    v = (v * 1e6).round() / 1e6;
    if doc.overrides.get(key).and_then(|x| x.as_f64()) != Some(v) {
        doc.overrides.insert(key.to_string(), pc::params::num(v));
    }
}

/// Wejście z `editor_panels`: rysuje gizmo bieżącego dokumentu.
pub fn draw(ctx: &egui::Context, docs: &mut Docs, der: &Derived, view: &View, proj: &dyn Fn(Vec3) -> Option<Pos2>) {
    if !view.gizmos || !view.viewport.is_positive() {
        return;
    }
    let p = ctx.layer_painter(LayerId::new(Order::Background, "gizmos".into())).with_clip_rect(view.viewport);
    let g = G { ctx, p, proj, vp: view.viewport };
    let doc = docs.cur_mut();
    match doc.mode {
        Mode::Building => building(&g, doc, der, view),
        Mode::Vehicle => vehicle(&g, doc, der, view),
        Mode::Human => human(&g, doc, der, view),
        _ => {}
    }
}

fn num(der: &Derived, k: &str) -> f64 {
    der.resolved.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0)
}

// ------------------------------------------------------------------ budynek

/// Wysokości górnych krawędzi kondygnacji (jak `Gen::rows_for` w generatorze).
fn storey_tops(der: &Derived, floors: usize) -> Vec<f64> {
    let (gf, fh, decay) = (num(der, "groundFloorHeight"), num(der, "floorHeight"), num(der, "floorHeightDecay"));
    let plinth = num(der, "plinthHeight").clamp(0.0, (gf - 1.2).max(0.0));
    let mut tops = vec![plinth + gf];
    for k in 1..floors {
        let t = if floors > 2 { (k - 1) as f64 / (floors - 2) as f64 } else { 0.0 };
        tops.push(tops[k - 1] + (fh * (1.0 - decay * t)).max(2.3));
    }
    tops
}

fn building(g: &G, doc: &mut Doc, der: &Derived, view: &View) {
    let Some(s) = &der.stats else { return };
    let col = ACCENT;
    let (min, max) = der.bounds;
    let poly = doc.use_poly;
    let segs = pc::random::jround(num(der, "segments")).max(1.0);
    let (x0, x1, zf) = if poly { (min.x as f64, max.x as f64, max.z as f64) } else { (-s.frontage / 2.0, s.frontage / 2.0, 0.0) };
    let depth = if poly { (max.z - min.z) as f64 } else { s.depth };
    let floors = s.floors as usize;
    let tops = storey_tops(der, floors);
    let wall = s.height_walls;
    let pitched = num(der, "roofPitch") >= 3.0;
    let ridge = if pitched { s.height_total } else { wall + num(der, "parapetHeight") };

    if view.gz_dims {
        // szerokość frontu: linia przed budynkiem (strona ulicy), uchwyt na prawym końcu
        let off = Vec3::new(0.0, 0.0, 2.6);
        g.dim(w(x0, 0.0, zf), w(x1, 0.0, zf), off, &format!("front {:.1} m", x1 - x0), col);
        if !poly && let Some((st, mv)) = g.handle("b-width", w(x1, 0.0, zf) + off, Vec3::X, num(der, "segmentWidth"), col, "Szerokość segmentu (segmentWidth) — przeciągnij") {
            set(doc, pc::data(), "segmentWidth", st + 2.0 * mv / segs);
        }
        // głębokość: linia z boku, uchwyt na tylnym końcu
        let off = Vec3::new(2.6, 0.0, 0.0);
        g.dim(w(x1, 0.0, zf), w(x1, 0.0, zf - depth), off, &format!("głębokość {depth:.1} m"), col);
        if !poly && let Some((st, mv)) = g.handle("b-depth", w(x1, 0.0, zf - depth) + off, Vec3::NEG_Z, num(der, "depth"), col, "Głębokość (depth) — przeciągnij") {
            set(doc, pc::data(), "depth", st + mv);
        }
        // wysokość: pionowa linia przy lewym narożniku frontu, znaczniki poziomów kondygnacji
        let (hx, hz) = (x0 - 1.8, zf + 0.4);
        let hl = format!("ściany {wall:.1} m");
        g.dim_at(w(x0, 0.0, zf), w(x0, wall, zf), w(hx - x0, 0.0, hz - zf), &hl, col, 0.5, Vec2::new(-(hl.chars().count() as f32 * 3.8 + 16.0), 0.0));
        let mut prev = 0.0;
        for (k, t) in tops.iter().enumerate() {
            g.line(w(hx - 0.35, *t, hz), w(hx + 0.35, *t, hz), Stroke::new(1.5, col));
            g.dashed(w(hx, *t, hz), w(x0, *t, zf), Stroke::new(1.0, col.gamma_multiply(0.35)));
            let mid = (prev + t) / 2.0;
            g.tag3(w(hx, mid, hz), Vec2::new(9.0, 0.0), Align2::LEFT_CENTER, &if k == 0 { "parter".to_string() } else { format!("{}", k + 1) }, DIM);
            prev = *t;
        }
        if let Some((st, mv)) = g.handle("b-height", w(hx, wall, hz), Vec3::Y, num(der, "floors"), col, "Wysokość — przeciągnij: zmienia liczbę kondygnacji (floors)") {
            let target = storey_tops(der, (st.round() as usize).max(1)).last().copied().unwrap_or(0.0) + mv;
            // najbliższa liczba kondygnacji: porównanie z wysokościami wg bieżących parametrów
            let best = (1..=30usize).min_by(|a, b| {
                let ta = *storey_tops(der, *a).last().unwrap();
                let tb = *storey_tops(der, *b).last().unwrap();
                (ta - target).abs().total_cmp(&(tb - target).abs())
            });
            if let Some(f) = best {
                set(doc, pc::data(), "floors", f as f64);
            }
        }
        // kalenica / attyka: przerywana linia nad frontem z wysokością
        if ridge > wall + 0.05 {
            let c = col.gamma_multiply(0.9);
            g.dashed(w(x0 - 0.8, ridge, zf), w(x1 + 0.8, ridge, zf), Stroke::new(1.5, c));
            if let Some(a) = g.pt(w(x1 + 0.8, ridge, zf)) {
                g.p.circle_stroke(a, 5.0, Stroke::new(1.5, c));
                dim_label(&g.p, a + Vec2::new(-62.0, 12.0), &format!("{} {ridge:.1} m", if pitched { "kalenica" } else { "attyka" }), col);
            }
        } else {
            g.dashed(w(x0 - 0.8, wall, zf), w(x1 + 0.8, wall, zf), Stroke::new(1.5, col.gamma_multiply(0.7)));
        }
    }
    if view.gz_anchors {
        // strzałka frontu: strona ulicy (+Z)
        let c = Color32::from_rgb(0x8a, 0xb4, 0xff);
        let xm = (x0 + x1) / 2.0;
        g.arrow(w(xm, 0.05, zf + 0.3), w(xm, 0.05, zf + 2.2), c);
        if let Some(a) = g.pt(w(xm, 0.05, zf + 2.2)) {
            dim_label(&g.p, a + Vec2::new(-76.0, 12.0), "FRONT · ulica +Z", c);
        }
    }
}

// ------------------------------------------------------------------ pojazd

fn vehicle(g: &G, doc: &mut Doc, der: &Derived, view: &View) {
    let Snap::Vehicle(s) = &der.gz else { return };
    let col = Fam::Pojazdy.color();
    let (min, max) = der.bounds;
    let (wb, rad, hub_x) = (s.wheelbase, s.wheel_radius, s.track / 2.0);
    let body_w = num(der, "width");
    let heavy = s.body_type == "podwozie";
    let td = tabor_core::data();

    if view.gz_dims {
        // osie: linie przez piasty, tylna z uchwytem rozstawu osi
        let axle = Stroke::new(2.0, col.gamma_multiply(0.85));
        for z in [wb / 2.0, -wb / 2.0] {
            g.line(w(-hub_x, rad, z), w(hub_x, rad, z), axle);
            g.dot(w(0.0, rad, z), 3.5, col);
        }
        if let Some((st, mv)) = g.handle("v-axle", w(0.0, rad, -wb / 2.0), Vec3::NEG_Z, num(der, "wheelbase"), col, "Tylna oś — przeciągnij: rozstaw osi (wheelbase)") {
            set(doc, td, "wheelbase", st - 2.0 * mv);
        }
        // oś wzdłużna: długość w trzech odcinkach (zwis tylny, rozstaw, zwis przedni) i całkowita
        let (x, side) = (max.x as f64 + 0.35, 0.0);
        let zs = [min.z as f64, -wb / 2.0, wb / 2.0, max.z as f64];
        let names = ["zwis tył", "rozstaw osi", "zwis przód"];
        for k in 0..3 {
            g.dim(w(x - 0.35, side, zs[k]), w(x - 0.35, side, zs[k + 1]), Vec3::new(0.35, 0.0, 0.0), &format!("{} {:.2} m", names[k], zs[k + 1] - zs[k]), col);
        }
        g.dim(w(x - 0.35, 0.0, zs[0]), w(x - 0.35, 0.0, zs[3]), Vec3::new(1.15, 0.0, 0.0), &format!("długość {:.2} m", zs[3] - zs[0]), col);
        let rear_frac = {
            let (fo, ro) = (num(der, "frontOverhang"), num(der, "rearOverhang"));
            (ro / (fo + ro).max(1e-6)).clamp(0.2, 0.8)
        };
        if let Some((st, mv)) = g.handle("v-length", w(0.0, 0.45, min.z as f64), Vec3::NEG_Z, num(der, "length"), col, "Tył pojazdu — przeciągnij: długość (length)") {
            set(doc, td, "length", st + mv / rear_frac);
        }
        // wysokość: pionowo za tyłem
        let hz = min.z as f64 - 0.9;
        g.dim(w(max.x as f64, 0.0, hz), w(max.x as f64, max.y as f64, hz), Vec3::new(0.5, 0.0, 0.0), &format!("wysokość {:.2} m", max.y - 0.0), col);
        let hh = num(der, "height");
        if let Some((st, mv)) = g.handle("v-height", w(0.0, hh, 0.0), Vec3::Y, hh, col, "Dach — przeciągnij: wysokość (height)") {
            set(doc, td, "height", st + mv);
        }
        // szerokość nadwozia i obrys z lusterkami: przed przodem
        g.dim(w(-body_w / 2.0, 0.0, max.z as f64), w(body_w / 2.0, 0.0, max.z as f64), Vec3::new(0.0, 0.0, 0.35), &format!("szer. nadwozia {body_w:.2} m"), col);
        let outline = (max.x - min.x) as f64;
        if (outline - body_w).abs() > 0.02 {
            g.dim(w(min.x as f64, 0.0, max.z as f64), w(max.x as f64, 0.0, max.z as f64), Vec3::new(0.0, 0.0, 0.95), &format!("z lusterkami {outline:.2} m"), WARN);
        }
        if let Some((st, mv)) = g.handle("v-width", w(body_w / 2.0, num(der, "clearance") + (hh - num(der, "clearance")) * 0.45, 0.0), Vec3::X, body_w, col, "Bok — przeciągnij: szerokość nadwozia (width)") {
            set(doc, td, "width", st + 2.0 * mv);
        }
        // prześwit: krótki wymiar pod podłogą
        let clr = num(der, "clearance");
        let (cx, cz) = (body_w / 2.0, 0.0);
        g.dim(w(cx, 0.0, cz), w(cx, clr, cz), w(0.4, 0.0, 0.0), &format!("prześwit {clr:.2} m"), col);
        if let Some((st, mv)) = g.handle("v-clear", w(cx + 0.4, clr, cz), Vec3::Y, clr, col, "Podłoga — przeciągnij: prześwit (clearance)") {
            set(doc, td, "clearance", st + mv);
        }
        if heavy {
            heavy_limits(g, body_w, min, max, rad, wb);
        }
    }
    if view.gz_anchors {
        for (name, pos) in &s.wheels {
            let at = v3(*pos);
            if let Some(a) = g.pt(at) {
                g.p.circle_stroke(a, 7.0, Stroke::new(1.5, TEXT));
                g.p.line_segment([a - Vec2::new(10.0, 0.0), a + Vec2::new(10.0, 0.0)], Stroke::new(1.0, TEXT));
                g.p.line_segment([a - Vec2::new(0.0, 10.0), a + Vec2::new(0.0, 10.0)], Stroke::new(1.0, TEXT));
                let right = pos[0] < 0.0;
                g.tag(a + Vec2::new(if right { -12.0 } else { 12.0 }, 0.0), if right { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER }, name, TEXT);
            }
        }
        for (i, pos) in s.seats.iter().enumerate() {
            let at = v3(*pos);
            let Some(a) = g.pt(at) else { continue };
            let c = if i == 0 { WARN } else { Color32::from_rgb(0x5a, 0xc2, 0xd8) };
            g.p.circle(a, if i == 0 { 9.5 } else { 8.0 }, Color32::from_rgb(0x18, 0x18, 0x18), Stroke::new(if i == 0 { 2.5 } else { 1.5 }, c));
            g.p.text(a, Align2::CENTER_CENTER, i, egui::FontId::new(10.5, bold()), c);
            if i == 0 {
                g.tag(a + Vec2::new(14.0, 0.0), Align2::LEFT_CENTER, "Seat_0 · kierowca", WARN);
            }
        }
        if let Some((c, h)) = &s.lightbar {
            let k = Color32::from_rgb(0x6a, 0x9c, 0xff);
            let corner = |sx: f64, sy: f64, sz: f64| w(c[0] + sx * h[0], c[1] + sy * h[1], c[2] + sz * h[2]);
            for (a, b) in [((-1.0, -1.0, -1.0), (1.0, -1.0, -1.0)), ((-1.0, 1.0, -1.0), (1.0, 1.0, -1.0)), ((-1.0, -1.0, 1.0), (1.0, -1.0, 1.0)), ((-1.0, 1.0, 1.0), (1.0, 1.0, 1.0)), ((-1.0, -1.0, -1.0), (-1.0, 1.0, -1.0)), ((1.0, -1.0, -1.0), (1.0, 1.0, -1.0)), ((-1.0, -1.0, 1.0), (-1.0, 1.0, 1.0)), ((1.0, -1.0, 1.0), (1.0, 1.0, 1.0)), ((-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0)), ((1.0, -1.0, -1.0), (1.0, -1.0, 1.0)), ((-1.0, 1.0, -1.0), (-1.0, 1.0, 1.0)), ((1.0, 1.0, -1.0), (1.0, 1.0, 1.0))] {
                g.line(corner(a.0, a.1, a.2), corner(b.0, b.1, b.2), Stroke::new(1.2, k));
            }
            g.tag3(corner(0.0, 1.0, 0.0), Vec2::new(0.0, -10.0), Align2::CENTER_BOTTOM, "Lightbar", k);
        }
        g.dot(w(0.0, 0.0, 0.0), 3.0, TEXT);
        g.tag3(w(0.0, 0.0, 0.0), Vec2::new(6.0, 8.0), Align2::LEFT_CENTER, "origin", DIM);
    }
}

/// Pojazd ciężki: obwiednia limitów 96/53/WE (2,55 × 4,00 m) i linie kątów najazdu/zejścia 23°.
fn heavy_limits(g: &G, body_w: f64, min: Vec3, max: Vec3, rad: f64, wb: f64) {
    const LW: f64 = 2.55;
    const LH: f64 = 4.0;
    let over = body_w > LW + 1e-6 || max.y as f64 > LH + 1e-6;
    let c = if over { Color32::from_rgb(0xe0, 0x5a, 0x50) } else { WARN };
    let (z0, z1) = (min.z as f64 - 0.15, max.z as f64 + 0.15);
    let st = Stroke::new(1.2, c.gamma_multiply(0.85));
    let (xl, h) = (LW / 2.0, LH);
    for (a, b) in [((xl, 0.0, z0), (xl, 0.0, z1)), ((-xl, 0.0, z0), (-xl, 0.0, z1)), ((xl, h, z0), (xl, h, z1)), ((-xl, h, z0), (-xl, h, z1)), ((xl, 0.0, z0), (xl, h, z0)), ((-xl, 0.0, z0), (-xl, h, z0)), ((xl, 0.0, z1), (xl, h, z1)), ((-xl, 0.0, z1), (-xl, h, z1)), ((xl, h, z0), (-xl, h, z0)), ((xl, h, z1), (-xl, h, z1))] {
        g.dashed(w(a.0, a.1, a.2), w(b.0, b.1, b.2), st);
    }
    if let Some(a) = g.pt(w(-xl, h, z1)) {
        dim_label(&g.p, a + Vec2::new(0.0, -2.0), &format!("96/53/WE · 2,55 × 4,00 m{}", if over { " · PRZEKROCZONE" } else { "" }), c);
    }
    // kąty najazdu i zejścia: styczna do opony z gruntu pod 23°, w płaszczyźnie burty
    let a = 23f64.to_radians();
    let lean = rad * (a / 2.0).tan();
    let x = body_w / 2.0 + 0.05;
    let ac = Color32::from_rgb(0x78, 0xc2, 0x5a);
    for (sign, label, end) in [(1.0, "najazd 23°", max.z as f64), (-1.0, "zejście 23°", min.z as f64)] {
        let zc = sign * (wb / 2.0 + lean);
        let len = (end - zc) * sign;
        if len <= 0.05 {
            continue;
        }
        let top = w(x, a.tan() * len, end);
        g.line(w(x, 0.0, zc), top, Stroke::new(1.8, ac));
        g.tag3(top, Vec2::new(9.0, 0.0), Align2::LEFT_CENTER, label, ac);
    }
}

// ------------------------------------------------------------------ człowiek

/// Statystyka wzrostu z sekcji `wzrost` danych Tłumu: (średnia m, SD m lub None dla dzieci).
fn height_stats(der: &Derived, doc: &Doc) -> Option<(f64, Option<f64>)> {
    let w = tlum_core::extra().get("wzrost")?;
    let (age, sex) = (num(der, "age"), num(der, "sex").clamp(0.0, 1.0));
    if age >= 19.0 {
        let year = match &doc.source {
            Source::Cursor(y, _) => *y,
            _ => der.weights.iter().max_by(|a, b| a.1.total_cmp(&b.1)).and_then(|(id, _)| tlum_core::data().archetype(id)).map_or(1985.0, |a| a.map.year),
        };
        let rows = w.get("rocznikiDorosli")?.as_array()?;
        // wiersze malejąco po roczniku: interpolacja liniowa, poza zakresem stała
        let at = |sex_key: &str, field: &str| -> Option<f64> {
            let mut pts: Vec<(f64, f64)> = rows.iter().filter_map(|r| Some((r.get("rocznik")?.as_f64()?, r.get(sex_key)?.get(field)?.as_f64()?))).collect();
            pts.sort_by(|a, b| a.0.total_cmp(&b.0));
            let i = pts.iter().position(|p| p.0 >= year);
            Some(match i {
                None => pts.last()?.1,
                Some(0) => pts[0].1,
                Some(i) => pts[i - 1].1 + (pts[i].1 - pts[i - 1].1) * (year - pts[i - 1].0) / (pts[i].0 - pts[i - 1].0),
            })
        };
        // mieszanka dwóch rozkładów normalnych (K i M) o udziale M = `sex`
        let (mk, mm, sk, sm) = (at("k", "srednia")?, at("m", "srednia")?, at("k", "sd")?, at("m", "sd")?);
        let mean = mk + sex * (mm - mk);
        let var = (1.0 - sex) * sk * sk + sex * sm * sm + sex * (1.0 - sex) * (mm - mk) * (mm - mk);
        Some((mean / 100.0, Some(var.sqrt() / 100.0)))
    } else {
        let ch = w.get("dzieci")?;
        let ages: Vec<f64> = ch.get("wiek")?.as_array()?.iter().filter_map(|v| v.as_f64()).collect();
        let col = |k: &str| -> Option<Vec<f64>> { Some(ch.get(k)?.as_array()?.iter().filter_map(|v| v.as_f64()).collect()) };
        let (boys, girls) = (col("chlopcy")?, col("dziewczeta")?);
        if age < ages[0] {
            return None;
        }
        let i = ages.iter().position(|a| *a >= age).unwrap_or(ages.len() - 1);
        let lerp = |v: &[f64]| if i == 0 { v[0] } else { v[i - 1] + (v[i] - v[i - 1]) * ((age - ages[i - 1]) / (ages[i] - ages[i - 1])).clamp(0.0, 1.0) };
        Some(((lerp(&girls) + sex * (lerp(&boys) - lerp(&girls))) / 100.0, None))
    }
}

fn human(g: &G, doc: &mut Doc, der: &Derived, view: &View) {
    let Snap::Human(s) = &der.gz else { return };
    let col = Fam::Ludzie.color();
    let (min, max) = der.bounds;
    let td = tlum_core::data();
    let height = num(der, "height");
    let joint = |n: &str| s.joints.iter().find(|j| j.name == n);
    // percentyle dotyczą wzrostu stojącego (nie siedzącego)
    let standing = matches!(doc.pose.as_str(), "stand" | "walk" | "rest");

    if view.gz_skeleton {
        let bone = Stroke::new(2.2, col.gamma_multiply(0.95));
        for j in s.joints.iter().filter(|j| !j.name.ends_with("IK") && j.name != "Root") {
            g.line(v3(j.head), v3(j.tail), bone);
        }
        for j in s.joints.iter().filter(|j| !j.name.ends_with("IK")) {
            if let Some(a) = g.pt(v3(j.head)) {
                let grip = j.name.ends_with("Grip");
                g.p.circle(a, if grip { 3.0 } else { 3.8 }, if grip { WARN } else { Color32::from_rgb(0x12, 0x12, 0x12) }, Stroke::new(1.5, if grip { WARN } else { Color32::WHITE }));
            }
        }
        if let Some(t) = joint("Head") {
            g.dot(v3(t.tail), 3.0, col);
        }
    }
    if view.gz_dims {
        // wzrost: pionowy wymiar z uchwytem na czubku głowy
        let hx = max.x as f64 + 0.4;
        g.dim_at(w(max.x as f64, 0.0, 0.0), w(max.x as f64, max.y as f64, 0.0), w(0.4, 0.0, 0.0), &format!("{} {:.2} m", if standing { "wzrost" } else { "wys." }, max.y), col, 0.72, Vec2::new(46.0, 0.0));
        if let Some((st, mv)) = g.handle("h-height", w(hx, max.y as f64, 0.0), Vec3::Y, height, col, "Czubek głowy — przeciągnij: wzrost (height)") {
            set(doc, td, "height", st + mv);
        }
        // percentyle wzrostu populacji (Wroński 2023; dzieci NCD-RisC): P5 / P50 / P95
        if let Some((mean, sd)) = height_stats(der, doc).filter(|_| standing) {
            let px = min.x as f64 - 0.35;
            let c = Color32::from_rgb(0xc8, 0xb4, 0x6a);
            let marks: Vec<(&str, f64)> = match sd {
                Some(sd) => vec![("P5", mean - 1.645 * sd), ("P50", mean), ("P95", mean + 1.645 * sd)],
                None => vec![("P50", mean)],
            };
            if marks.len() > 1 {
                g.line(w(px, marks[0].1, 0.0), w(px, marks[2].1, 0.0), Stroke::new(2.0, c.gamma_multiply(0.6)));
            }
            for (name, y) in marks {
                g.line(w(px - 0.1, y, 0.0), w(px + 0.1, y, 0.0), Stroke::new(1.8, c));
                g.dashed(w(px + 0.1, y, 0.0), w(min.x as f64, y, 0.0), Stroke::new(1.0, c.gamma_multiply(0.4)));
                g.tag3(w(px - 0.1, y, 0.0), Vec2::new(-4.0, 0.0), Align2::RIGHT_CENTER, &format!("{name} {y:.2} m"), c);
            }
        }
        // barki: wymiar w poziomie na wysokości stawów ramion, uchwyt na lewym końcu
        if let Some(ua) = joint("LeftUpperArm") {
            let sw = num(der, "shoulderWidth") * height;
            let ys = ua.head[1];
            let off = w(0.0, 0.16, 0.0);
            g.dim(w(-sw / 2.0, ys, 0.0), w(sw / 2.0, ys, 0.0), off, &format!("barki {sw:.2} m"), col);
            if let Some((st, mv)) = g.handle("h-shoulder", w(sw / 2.0, ys, 0.0) + off, Vec3::X, num(der, "shoulderWidth"), col, "Barki — przeciągnij: szerokość barków (shoulderWidth, × wzrostu)") {
                set(doc, td, "shoulderWidth", st + 2.0 * mv / height.max(0.5));
            }
        }
    }
    if view.gz_anchors {
        // oczy i biodro (H)
        let c = Color32::from_rgb(0x5a, 0xc2, 0xd8);
        if let Some(h) = joint("Head") {
            let eye = h.head[1] + (h.tail[1] - h.head[1]) * 0.5;
            let ex = max.x as f64 + 0.62;
            g.dashed(w(0.0, eye, 0.12), w(ex, eye, 0.0), Stroke::new(1.0, c.gamma_multiply(0.7)));
            g.dot(w(0.0, eye, 0.12), 3.0, c);
            g.tag3(w(ex, eye, 0.0), Vec2::new(5.0, 0.0), Align2::LEFT_CENTER, &format!("oczy {eye:.2} m"), c);
        }
        let hip = v3(s.hip);
        if let Some(a) = g.pt(hip) {
            g.p.circle(a, 6.5, Color32::from_rgb(0x18, 0x18, 0x18), Stroke::new(2.0, WARN));
            g.p.circle_filled(a, 2.0, WARN);
            let ex = max.x as f64 + 0.62;
            g.dashed(hip, w(ex, s.hip[1], s.hip[2]), Stroke::new(1.0, WARN.gamma_multiply(0.6)));
            g.tag3(w(ex, s.hip[1], s.hip[2]), Vec2::new(5.0, 0.0), Align2::LEFT_CENTER, &format!("H biodro {:.2} m", s.hip[1]), WARN);
        }
        // przód postaci (+Z) i grunt
        let a = Color32::from_rgb(0x8a, 0xb4, 0xff);
        g.arrow(w(0.0, 0.03, 0.3), w(0.0, 0.03, 0.8), a);
        if let Some(q) = g.pt(w(0.0, 0.03, 0.3)) {
            dim_label(&g.p, q + Vec2::new(58.0, 4.0), "przód +Z", a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storey_tops_match_generator_wall_height() {
        for a in ["familok", "kamienica_eklektyczna", "kamienica_modernistyczna", "blok_poodwilzowy"] {
            let m = pc::generate_from_spec(&Doc::building(a).spec());
            let der = Derived { resolved: m.resolved.params.to_map(), ..Default::default() };
            let tops = storey_tops(&der, m.stats.floors as usize);
            assert_eq!(tops.len(), m.stats.floors as usize, "{a}");
            assert!((tops[tops.len() - 1] - m.stats.height_walls).abs() < 1e-6, "{a}: {tops:?} vs {}", m.stats.height_walls);
        }
    }

    #[test]
    fn human_snapshot_and_height_percentiles() {
        for pose in ["stand", "walk", "sit", "drive", "rest"] {
            let mut doc = Doc::human(&tlum_core::data().default_archetype);
            doc.pose = pose.into();
            let m = tlum_core::generate(&doc.human_spec());
            let Snap::Human(s) = Snap::human(&m) else { panic!() };
            assert!(s.joints.iter().any(|j| j.name == "Head") && s.hip[1] > 0.3, "{pose}");
        }
        let doc = Doc::human(&tlum_core::data().default_archetype);
        let m = tlum_core::generate(&doc.human_spec());
        let der = Derived { resolved: m.params.clone(), weights: vec![(doc.name.clone(), 1.0)], ..Default::default() };
        let (mean, sd) = height_stats(&der, &doc).expect("sekcja wzrost");
        assert!((1.6..1.85).contains(&mean) && sd.is_some_and(|s| (0.04..0.12).contains(&s)), "{mean} {sd:?}");
        let child = Derived { resolved: [("age".to_string(), serde_json::json!(9)), ("sex".to_string(), serde_json::json!(0.5))].into_iter().collect(), ..Default::default() };
        let (mean, sd) = height_stats(&child, &doc).unwrap();
        assert!((1.2..1.5).contains(&mean) && sd.is_none());
    }

    #[test]
    fn set_clamps_and_snaps_to_param_definition() {
        let mut doc = Doc::building("familok");
        set(&mut doc, pc::data(), "floors", 99.4);
        assert_eq!(doc.overrides["floors"].as_f64(), Some(30.0));
        set(&mut doc, pc::data(), "depth", 12.34);
        assert_eq!(doc.overrides["depth"].as_f64(), Some(12.3));
    }
}
