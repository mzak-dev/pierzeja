//! Ciemny motyw wg makiety (docs/ui-mock): fonty Lato + Cascadia Mono + ikony Phosphor,
//! kolory, ramki paneli i małe widżety (pole liczbowe z jednostką, przycisk wstążki, przełącznik).

use crate::anim::{self, hex, mix};
use bevy_egui::egui::{self, Color32, CornerRadius, FontFamily, FontId, Frame, Margin, RichText, Sense, Stroke, Vec2};
pub use egui_phosphor::regular as ic;

pub const BG: Color32 = Color32::from_rgb(0x15, 0x15, 0x15);
pub const PANEL: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x1e);
pub const INPUT: Color32 = Color32::from_rgb(0x16, 0x16, 0x16);
pub const HOVER: Color32 = Color32::from_rgb(0x2a, 0x2a, 0x2a);
pub const ACTIVE: Color32 = Color32::from_rgb(0x30, 0x30, 0x30);
pub const BORDER: Color32 = Color32::from_rgb(0x2e, 0x2e, 0x2e);
pub const BORDER_HI: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xe8, 0xe8);
pub const DIM: Color32 = Color32::from_rgb(0x9a, 0x9a, 0x9a);
pub const FAINT: Color32 = Color32::from_rgb(0x66, 0x66, 0x66);
pub const ACCENT: Color32 = Color32::from_rgb(0xe8, 0x73, 0x5a);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x7a, 0x3b, 0x30);
pub const WARN: Color32 = Color32::from_rgb(0xe8, 0xc0, 0x5a);
/// Paski powłoki z mocka „tryby edytora” (tytuł, menu, wstążka, stan).
pub const BAR2: Color32 = Color32::from_rgb(0x1b, 0x1b, 0x1b);
pub const LINE: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);
pub const OK: Color32 = Color32::from_rgb(0x6c, 0xc2, 0x8a);

pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let font = |b: &'static [u8]| std::sync::Arc::new(egui::FontData::from_static(b));
    fonts.font_data.insert("lato".into(), font(include_bytes!("../assets/fonts/LatoWeb-Regular.ttf")));
    fonts.font_data.insert("lato-semi".into(), font(include_bytes!("../assets/fonts/LatoWeb-Semibold.ttf")));
    fonts.font_data.insert("cascadia".into(), font(include_bytes!("../assets/fonts/CascadiaMono.ttf")));
    fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "lato".into());
    fonts.families.get_mut(&FontFamily::Monospace).unwrap().insert(0, "cascadia".into());
    let mut b = fonts.families[&FontFamily::Proportional].clone();
    b[0] = "lato-semi".into();
    fonts.families.insert(bold(), b);
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    // ikony także w rodzinach mono i pogrubionej
    for fam in [FontFamily::Monospace, bold()] {
        fonts.families.get_mut(&fam).unwrap().push("phosphor".into());
    }
    ctx.set_fonts(fonts);

    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|s| {
        use egui::TextStyle::*;
        s.text_styles = [
            (Heading, FontId::new(15.0, bold())),
            (Body, FontId::proportional(13.5)),
            (Button, FontId::proportional(13.5)),
            (Small, FontId::proportional(11.5)),
            (Monospace, FontId::monospace(12.5)),
        ]
        .into();
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(8.0, 4.0);
        s.spacing.interact_size.y = 26.0;
        s.interaction.selectable_labels = false;
        let v = &mut s.visuals;
        *v = egui::Visuals::dark();
        v.panel_fill = PANEL;
        v.window_fill = PANEL;
        v.window_stroke = Stroke::new(1.0, BORDER);
        v.window_corner_radius = CornerRadius::same(8);
        v.menu_corner_radius = CornerRadius::same(6);
        v.extreme_bg_color = INPUT;
        v.faint_bg_color = HOVER;
        v.override_text_color = Some(TEXT);
        v.selection.bg_fill = ACCENT_DIM;
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.hyperlink_color = ACCENT;
        let w = &mut v.widgets;
        for (st, fill) in [(&mut w.noninteractive, PANEL), (&mut w.inactive, HOVER), (&mut w.hovered, ACTIVE), (&mut w.active, ACTIVE), (&mut w.open, ACTIVE)] {
            st.bg_fill = fill;
            st.weak_bg_fill = fill;
            st.corner_radius = CornerRadius::same(6);
            st.fg_stroke.color = TEXT;
        }
        w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        w.noninteractive.fg_stroke.color = DIM;
        w.inactive.bg_stroke = Stroke::new(1.0, BORDER_HI);
        w.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x5a, 0x5a, 0x5a));
        w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    });
}

/// Ramka pływającego panelu nad widokiem 3D.
pub fn panel_frame() -> Frame {
    Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(8).shadow(egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(90) })
}

/// Nagłówek panelu: tytuł (pogrubiony) i szary dopisek z prawej; zwraca odpowiedź dopisku.
pub fn panel_header(ui: &mut egui::Ui, icon: Option<&str>, title: &str, right: impl FnOnce(&mut egui::Ui)) {
    Frame::new().inner_margin(Margin { left: 12, right: 10, top: 9, bottom: 8 }).show(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(i) = icon {
                ui.label(RichText::new(i).color(ACCENT).size(15.0));
            }
            ui.label(RichText::new(title).family(bold()).size(14.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
        });
    });
    hline(ui);
}

pub fn hline(ui: &mut egui::Ui) {
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, BORDER));
    ui.add_space(1.0);
}

/// Pływający panel w stałym miejscu (prawy-górny róg = `pos`, gdy `anchor_right`).
pub fn floating<R>(ctx: &egui::Context, id: &str, pos: egui::Pos2, width: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    egui::Area::new(egui::Id::new(id)).fixed_pos(pos).order(egui::Order::Middle).show(ctx, |ui| {
        panel_frame()
            .show(ui, |ui| {
                ui.set_width(width);
                add(ui)
            })
            .inner
    })
}

/// Inspektor: wjeżdża z prawej (240 ms) przy otwarciu, treść przenika się (150 ms) przy zmianie elementu.
pub fn inspector<R>(ctx: &egui::Context, key: impl std::hash::Hash + std::fmt::Debug, pos: egui::Pos2, width: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    let slide = anim::appear(ctx, egui::Id::new("insp-slide"), anim::MED);
    let op = anim::appear(ctx, egui::Id::new("insp-op"), anim::FAST);
    let content = anim::appear(ctx, egui::Id::new(("insp-item", key)), anim::FAST);
    let pos = pos + Vec2::new((1.0 - slide) * (width + 32.0), 0.0);
    egui::Area::new(egui::Id::new("inspector")).fixed_pos(pos).constrain(false).order(egui::Order::Middle).show(ctx, |ui| {
        ui.set_opacity(op);
        panel_frame()
            .show(ui, |ui| {
                ui.set_width(width);
                ui.set_opacity(op * content);
                add(ui)
            })
            .inner
    })
}

/// Przycisk-ikona bez ramki (pasek tytułu, pasek widoku).
pub fn icon_btn(ui: &mut egui::Ui, icon: &str, on: bool, tip: &str) -> egui::Response {
    let size = Vec2::splat(30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let ctx = ui.ctx();
    let h = anim::ease(ctx, resp.id.with("hov"), resp.hovered() as u8 as f32, anim::FAST);
    let bg = if on { ACTIVE } else { mix(Color32::TRANSPARENT, HOVER, h) };
    let r = anim::scaled(rect, anim::press(ctx, &resp));
    ui.painter().rect_filled(r, 6, bg);
    let col = if on { TEXT } else { mix(DIM, TEXT, h) };
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, icon, FontId::proportional(17.0 * r.width() / rect.width()), col);
    if tip.is_empty() { resp } else { resp.on_hover_text(tip) }
}

/// Duży przycisk wstążki: ikona nad podpisem.
pub fn ribbon_btn(ui: &mut egui::Ui, icon: &str, label: &str, on: bool, enabled: bool) -> egui::Response {
    let font = FontId::proportional(12.0);
    let w = (ui.fonts_mut(|f| f.layout_no_wrap(label.into(), font.clone(), TEXT).size().x) + 16.0).max(60.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 52.0), if enabled { Sense::click() } else { Sense::hover() });
    let ctx = ui.ctx();
    let h = anim::ease(ctx, resp.id.with("hov"), (resp.hovered() && enabled) as u8 as f32, anim::FAST);
    let o = anim::ease(ctx, resp.id.with("on"), on as u8 as f32, anim::FAST);
    let r = anim::scaled(rect, anim::press(ctx, &resp));
    let p = ui.painter();
    p.rect(r, 6, mix(mix(Color32::TRANSPARENT, hex(0x262626), h), ACTIVE, o), Stroke::new(1.0, mix(Color32::TRANSPARENT, BORDER_HI, o)), egui::StrokeKind::Inside);
    let col = if !enabled { FAINT } else { mix(hex(0xd4d4d4), TEXT, h.max(o)) };
    p.text(r.center_top() + Vec2::new(0.0, 17.0), egui::Align2::CENTER_CENTER, icon, FontId::proportional(20.0), col);
    p.text(r.center_bottom() - Vec2::new(0.0, 10.0), egui::Align2::CENTER_CENTER, label, font, col);
    resp
}

/// Grupa wstążki: przyciski w rzędzie, pod nimi szary podpis grupy, z prawej pionowa kreska.
pub fn ribbon_group(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    let text = label.to_string();
    let lw = ui.fonts_mut(|f| f.layout_no_wrap(text.clone(), FontId::proportional(11.0), DIM).size().x) + 8.0;
    let r = ui.vertical(|ui| {
        ui.set_min_width(lw);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            add(ui)
        });
        ui.add_space(-2.0);
        let w = ui.min_rect().width().max(lw);
        ui.allocate_ui_with_layout(Vec2::new(w, 16.0), egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(RichText::new(text).size(11.0).color(hex(0xa0a0a0))).extend());
        });
    });
    let x = r.response.rect.right() + 6.0;
    ui.painter().vline(x, r.response.rect.top() + 2.0..=r.response.rect.bottom() - 2.0, Stroke::new(1.0, BORDER));
    ui.add_space(12.0);
}

/// Wiersz właściwości: czerwona kropka (zmienione względem typologii), etykieta, widżet z prawej.
pub fn prop_row<R>(ui: &mut egui::Ui, label: &str, modified: bool, hint: Option<&str>, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(Vec2::new(8.0, 26.0), Sense::hover());
        if modified {
            ui.painter().circle_filled(dot.center(), 3.0, ACCENT);
        }
        // pole wartości ma stałe ~150 px; reszta na etykietę (bez sprzężenia z szerokością panelu)
        let w = (ui.available_width() - 158.0).max(60.0);
        let l = ui.allocate_ui_with_layout(Vec2::new(w, 26.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_width(w);
            ui.add(egui::Label::new(RichText::new(label).color(TEXT)).truncate())
        }).inner;
        if let Some(h) = hint
            && !h.is_empty()
        {
            l.on_hover_text(h);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add).inner
    })
    .inner
}

/// Pole liczbowe jak w makiecie: wartość mono z lewej, jednostka z prawej, czerwona ramka po zmianie.
pub fn num_field(ui: &mut egui::Ui, v: &mut f64, range: std::ops::RangeInclusive<f64>, step: f64, decimals: usize, unit: &str, modified: bool) -> egui::Response {
    let stroke = Stroke::new(1.0, if modified { ACCENT_DIM } else { BORDER_HI });
    let fr = Frame::new()
        .fill(INPUT)
        .stroke(stroke)
        .corner_radius(5)
        .inner_margin(Margin { left: 2, right: 8, top: 0, bottom: 0 })
        .show(ui, |ui| {
            ui.set_width(130.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.style_mut().override_font_id = Some(mono(12.5));
                let w = &mut ui.style_mut().visuals.widgets;
                for s in [&mut w.inactive, &mut w.hovered, &mut w.active] {
                    s.bg_fill = Color32::TRANSPARENT;
                    s.weak_bg_fill = Color32::TRANSPARENT;
                    s.bg_stroke = Stroke::NONE;
                }
                let speed = (step * 0.5).max((range.end() - range.start()) / 600.0);
                ui.add_space(6.0);
                let r = ui.add(egui::DragValue::new(v).range(range).speed(speed).max_decimals(decimals).min_decimals(decimals));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(unit).font(mono(11.0)).color(DIM));
                });
                r
            })
            .inner
        });
    // błysk po wpisaniu / strzałkach (nie przy ciągnięciu — wtedy wartość zmienia się co klatkę)
    let r = fr.inner;
    let a = anim::flash(ui.ctx(), r.id.with("flash"), r.changed() && !r.dragged());
    if a > 0.0 {
        ui.painter().rect_filled(fr.response.rect, 5, ACCENT.gamma_multiply(a));
    }
    r
}

/// Przełącznik „pigułka” z podpisem.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool, label: &str) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(34.0, 20.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = anim::ease_with(ui.ctx(), resp.id.with("knob"), *on as u8 as f32, anim::FAST, anim::ease_back);
    let c = anim::ease(ui.ctx(), resp.id.with("track"), *on as u8 as f32, anim::FAST);
    let p = ui.painter();
    p.rect_filled(rect, 10, mix(Color32::from_rgb(0x3a, 0x3a, 0x3a), ACCENT, c));
    let x = egui::lerp(rect.left() + 10.0..=rect.right() - 10.0, t);
    p.circle_filled(egui::pos2(x, rect.center().y), 7.0, Color32::WHITE);
    let l = ui.add(egui::Label::new(RichText::new(label).color(TEXT)).sense(Sense::click()));
    if l.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp | l
}

/// Przycisk-pigułka z ikoną i tekstem (np. „Eksport GLB”).
pub fn pill(ui: &mut egui::Ui, icon: &str, label: &str, primary: bool) -> egui::Response {
    let text = if icon.is_empty() { label.to_string() } else { format!("{icon}  {label}") };
    let rt = RichText::new(text).family(bold()).size(14.0).color(if primary { Color32::from_rgb(0x15, 0x15, 0x15) } else { TEXT });
    let b = egui::Button::new(rt).min_size(Vec2::new(0.0, 34.0)).corner_radius(6);
    let b = if primary { b.fill(Color32::from_rgb(0xf2, 0xf2, 0xf2)) } else { b.fill(HOVER).stroke(Stroke::new(1.0, BORDER_HI)) };
    ui.add(b)
}
