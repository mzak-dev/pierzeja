//! Edytory 2D: mapa typologii (rok × skala) i edytor obrysu wielokątnego.

use crate::theme::*;
use crate::{Doc, Source};
use bevy_egui::egui::{self, Color32, Stroke};
use pierzeja_core::data;
use pierzeja_core::footprint::{self as fp, P2};

/// Mapa typologii rok × skala: punkty archetypów, kursor IDW, linie wpływu.
pub fn typology_map(ui: &mut egui::Ui, doc: &mut Doc, weights: &[(String, f64)], height: f32) {
    let d = data();
    let [y0, y1] = d.map_years;
    let (resp, painter) = ui.allocate_painter(egui::vec2(ui.available_width(), height), egui::Sense::click_and_drag());
    let r = resp.rect.shrink2(egui::vec2(14.0, 12.0));
    let to_px = |year: f64, scale: f64| egui::pos2(r.left() + ((year - y0) / (y1 - y0)) as f32 * r.width(), r.top() + (1.0 - scale as f32) * r.height());
    for e in d.eras.iter().skip(1) {
        let x = to_px(e.from, 0.0).x;
        painter.vline(x, resp.rect.y_range(), Stroke::new(1.0, BORDER));
    }
    let cursor = match &doc.source {
        Source::Cursor(y, s) => Some(to_px(*y, *s)),
        _ => None,
    };
    if let Some(cp) = cursor {
        for (id, w) in weights {
            if let Some(a) = d.archetype(id) {
                painter.line_segment([cp, to_px(a.map.year, a.map.scale)], Stroke::new(1.0 + 4.0 * *w as f32, Color32::from_rgba_unmultiplied(232, 115, 90, 150)));
            }
        }
    }
    let hover = resp.hover_pos();
    // etykiety bez nakładania: najpierw wybrany i wskazany, kolejne tylko w wolnym miejscu
    let mut order: Vec<_> = d.archetypes.iter().collect();
    order.sort_by_key(|a| {
        let p = to_px(a.map.year, a.map.scale);
        !(doc.source == Source::Archetype(a.id.clone()) || hover.is_some_and(|h| h.distance(p) < 12.0))
    });
    let mut taken: Vec<egui::Rect> = Vec::new();
    for a in order {
        let p = to_px(a.map.year, a.map.scale);
        let sel = doc.source == Source::Archetype(a.id.clone());
        if sel {
            painter.circle_filled(p, 7.0, ACCENT);
            painter.circle_stroke(p, 7.0, Stroke::new(1.5, Color32::from_rgb(0xff, 0xb0, 0xa0)));
        } else {
            painter.circle_filled(p, 2.6, Color32::from_rgb(0xc8, 0xc8, 0xc8));
        }
        let near = hover.is_some_and(|h| h.distance(p) < 12.0);
        let (off, align) = if p.x > r.right() - 50.0 { (-9.0, egui::Align2::RIGHT_CENTER) } else { (if sel { 9.0 } else { 6.0 }, egui::Align2::LEFT_CENTER) };
        let g = painter.layout_no_wrap(a.short.clone(), egui::FontId::proportional(12.0), if sel || near { TEXT } else { Color32::from_rgb(0xb8, 0xb8, 0xb8) });
        let lr = align.anchor_size(p + egui::vec2(off, 0.0), g.size());
        if taken.iter().any(|t| t.expand(1.0).intersects(lr)) {
            continue;
        }
        taken.push(lr);
        painter.galley(lr.min, g, TEXT);
    }
    if let Some(cp) = cursor {
        painter.circle_stroke(cp, 7.0, Stroke::new(2.0, ACCENT));
    }
    if let Some(mut p) = resp.interact_pointer_pos().filter(|_| resp.clicked() || resp.dragged()) {
        // magnes: blisko punktu archetypu = dokładnie ten archetyp (100%)
        if let Some(a) = d.archetypes.iter().find(|a| to_px(a.map.year, a.map.scale).distance(p) < 11.0) {
            if resp.clicked() {
                doc.source = Source::Archetype(a.id.clone());
                return;
            }
            p = to_px(a.map.year, a.map.scale);
        }
        let tx = ((p.x - r.left()) / r.width()).clamp(0.0, 1.0) as f64;
        let ty = ((p.y - r.top()) / r.height()).clamp(0.0, 1.0) as f64;
        doc.source = Source::Cursor((y0 + tx * (y1 - y0)).round(), ((1.0 - ty) * 100.0).round() / 100.0);
    }
}

/// Edytor obrysu 2D (widok z góry, ulica na dole = +z). Lewy klik przy krawędzi — nowy
/// punkt, przeciąganie — przesunięcie (siatka 0,5 m), prawy klik — usunięcie punktu,
/// klik w kwadrat na środku krawędzi — rola: zwykła → front → ślepa.
pub fn polygon_editor(ui: &mut egui::Ui, doc: &mut Doc, height: f32) {
    let (resp, painter) = ui.allocate_painter(egui::vec2(ui.available_width(), height), egui::Sense::click_and_drag());
    let r = resp.rect;
    painter.rect(r, 5, INPUT, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
    let pts = doc.poly.pts.clone();
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in &pts {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    // stała skala, dopóki punkt jest przeciągany (kadr nie „ucieka”)
    let drag_id = resp.id.with("drag");
    let view_id = resp.id.with("view");
    let mut drag: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_id)).flatten();
    let view: (f64, [f64; 2]) = if drag.is_some() {
        ui.ctx().data(|d| d.get_temp(view_id)).unwrap_or((5.0, [0.0, 0.0]))
    } else {
        let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(10.0) + 16.0;
        let v = ((r.width().min(r.height()) as f64) / span, [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0]);
        ui.ctx().data_mut(|d| d.insert_temp(view_id, v));
        v
    };
    let (sc, c) = view;
    let to_px = |p: P2| egui::pos2(r.center().x + ((p[0] - c[0]) * sc) as f32, r.center().y + ((p[1] - c[1]) * sc) as f32);
    let from_px = |q: egui::Pos2| [c[0] + (q.x - r.center().x) as f64 / sc, c[1] + (q.y - r.center().y) as f64 / sc];
    // siatka co 5 m
    let grid = Stroke::new(1.0, Color32::from_white_alpha(10));
    let (g0, g1) = (from_px(r.left_top()), from_px(r.right_bottom()));
    let mut x = (g0[0] / 5.0).floor() * 5.0;
    while x < g1[0] {
        painter.line_segment([to_px([x, g0[1]]), to_px([x, g1[1]])], grid);
        x += 5.0;
    }
    let mut z = (g0[1] / 5.0).floor() * 5.0;
    while z < g1[1] {
        painter.line_segment([to_px([g0[0], z]), to_px([g1[0], z])], grid);
        z += 5.0;
    }
    painter.text(r.center_bottom() - egui::vec2(0.0, 4.0), egui::Align2::CENTER_BOTTOM, "ulica (+z) ↓   siatka 5 m", egui::FontId::proportional(10.5), FAINT);
    let valid = fp::normalize(&doc.poly.footprint()).is_some();
    let mut mesh = egui::epaint::Mesh::default();
    let fill = if valid { Color32::from_rgba_unmultiplied(232, 115, 90, 40) } else { Color32::from_rgba_unmultiplied(220, 60, 60, 70) };
    for p in &pts {
        mesh.colored_vertex(to_px(*p), fill);
    }
    for t in fp::triangulate(&pts) {
        mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
    }
    painter.add(egui::Shape::mesh(mesh));
    let n = pts.len();
    let role_col = |r: u8| match r {
        1 => ACCENT,
        2 => FAINT,
        _ => Color32::from_rgb(0xc8, 0xc8, 0xc8),
    };
    for i in 0..n {
        let (a, b) = (to_px(pts[i]), to_px(pts[(i + 1) % n]));
        painter.line_segment([a, b], Stroke::new(if doc.poly.roles[i] == 1 { 3.0 } else { 1.5 }, role_col(doc.poly.roles[i])));
        let m = a + (b - a) * 0.5;
        painter.rect_filled(egui::Rect::from_center_size(m, egui::vec2(7.0, 7.0)), 1.0, role_col(doc.poly.roles[i]));
        painter.text(m + egui::vec2(6.0, -6.0), egui::Align2::LEFT_BOTTOM, format!("{:.1}", fp::edge_len(&pts, i)), mono(10.0), DIM);
    }
    for p in &pts {
        painter.circle_filled(to_px(*p), 4.0, Color32::WHITE);
    }
    let near = |q: egui::Pos2, tol: f32| (0..n).find(|&i| to_px(pts[i]).distance(q) < tol);
    let mid = |q: egui::Pos2| (0..n).find(|&i| (to_px(pts[i]) + (to_px(pts[(i + 1) % n]) - to_px(pts[i])) * 0.5).distance(q) < 7.0);
    let snap = |p: P2| [(p[0] * 2.0).round() / 2.0, (p[1] * 2.0).round() / 2.0];
    if resp.drag_started()
        && let Some(q) = resp.interact_pointer_pos()
    {
        drag = near(q, 10.0);
    }
    if let (Some(i), Some(q)) = (drag, resp.interact_pointer_pos())
        && resp.dragged()
    {
        doc.poly.pts[i] = snap(from_px(q));
    }
    if resp.drag_stopped() {
        drag = None;
    }
    ui.ctx().data_mut(|d| d.insert_temp(drag_id, drag));
    if resp.clicked()
        && let Some(q) = resp.interact_pointer_pos()
    {
        if let Some(i) = mid(q) {
            doc.poly.roles[i] = (doc.poly.roles[i] + 1) % 3;
        } else if near(q, 10.0).is_none() {
            // nowy punkt na najbliższej krawędzi (rola przechodzi na obie części)
            let best = (0..n)
                .map(|i| {
                    let (a, b) = (to_px(pts[i]), to_px(pts[(i + 1) % n]));
                    let t = ((q - a).dot(b - a) / (b - a).length_sq().max(1e-6)).clamp(0.0, 1.0);
                    (i, (a + (b - a) * t).distance(q))
                })
                .min_by(|x, y| x.1.total_cmp(&y.1));
            if let Some((i, dist)) = best
                && dist < 14.0
            {
                doc.poly.pts.insert(i + 1, snap(from_px(q)));
                let role = doc.poly.roles[i];
                doc.poly.roles.insert(i + 1, role);
            }
        }
    }
    if resp.secondary_clicked()
        && let Some(q) = resp.interact_pointer_pos()
        && let Some(i) = near(q, 10.0)
        && n > 3
    {
        doc.poly.pts.remove(i);
        doc.poly.roles.remove(i);
    }
    ui.label(if valid {
        egui::RichText::new(format!("{n} punktów · pole {:.0} m² · klik: punkt · prawy: usuń · kwadrat: rola", fp::area(&pts))).size(11.0).color(DIM)
    } else {
        egui::RichText::new("Obrys nieprawidłowy — generator użyje prostokąta.").size(11.0).color(Color32::from_rgb(240, 110, 110))
    });
}
