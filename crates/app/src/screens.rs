//! Ekran startowy (grafika = żywy podgląd 3D, presety, karty) i ekran serwera MCP
//! (placeholder: wbudowany serwer HTTP jeszcze nie istnieje, MCP działa jako stdio).

use crate::theme::*;
use crate::ui::{logo, Cmd};
use crate::{out_dir, Docs, View};
use bevy_egui::egui::{self, Align2, Color32, Frame, Margin, RichText, Sense, Stroke, Vec2};
use pierzeja_core::data;

/// Rysuje ekran startowy; zwraca prostokąt grafiki, w którym kamera 3D renderuje budynek.
pub fn start(root: &mut egui::Ui, docs: &Docs, cmds: &mut Vec<Cmd>) {
    let area = root.available_rect_before_wrap();
    let size = Vec2::new(area.width().min(1000.0) - 40.0, area.height().min(780.0) - 40.0);
    let card = egui::Rect::from_center_size(area.center(), size);
    let hero = egui::Rect::from_min_max(card.min, egui::pos2(card.right(), card.top() + card.height() * 0.42));
    root.painter().rect_filled(area, 0, BG);
    // grafika: statyczny render sceny, kadrowany jak CSS `object-fit: cover`
    static HERO: std::sync::OnceLock<egui::TextureHandle> = std::sync::OnceLock::new();
    let tex = HERO.get_or_init(|| texture(root.ctx(), "splash", include_bytes!("../assets/splash.png")));
    let (ia, ha) = (tex.aspect_ratio(), hero.aspect_ratio());
    let uv = if ha > ia {
        let vh = ia / ha;
        egui::Rect::from_min_size(egui::pos2(0.0, (1.0 - vh) * 0.55), Vec2::new(1.0, vh))
    } else {
        let uw = ha / ia;
        egui::Rect::from_min_size(egui::pos2((1.0 - uw) * 0.5, 0.0), Vec2::new(uw, 1.0))
    };
    root.painter().add(egui::epaint::RectShape::filled(hero, egui::CornerRadius { nw: 10, ne: 10, sw: 0, se: 0 }, Color32::WHITE).with_texture(tex.id(), uv));
    let lower = egui::Rect::from_min_max(egui::pos2(card.left(), hero.bottom()), card.max);
    let p = root.painter();
    p.rect_filled(lower, egui::CornerRadius { nw: 0, ne: 0, sw: 10, se: 10 }, Color32::from_rgb(0x22, 0x1e, 0x1c));
    // przejście grafiki w tło dolnej części
    let mut m = egui::epaint::Mesh::default();
    let (t, b) = (Color32::TRANSPARENT, Color32::from_rgba_unmultiplied(0x22, 0x1e, 0x1c, 200));
    let g = egui::Rect::from_min_max(egui::pos2(hero.left(), hero.bottom() - 90.0), hero.max);
    for (pos, c) in [(g.left_top(), t), (g.right_top(), t), (g.right_bottom(), b), (g.left_bottom(), b)] {
        m.colored_vertex(pos, c);
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(m);
    // przyciemnienie lewej części grafiki pod napisami
    let mut m = egui::epaint::Mesh::default();
    let (dk, t) = (Color32::from_black_alpha(130), Color32::TRANSPARENT);
    let g = egui::Rect::from_min_max(hero.min, egui::pos2(hero.left() + hero.width() * 0.6, hero.bottom()));
    for (pos, c) in [(g.left_top(), dk), (g.right_top(), t), (g.right_bottom(), t), (g.left_bottom(), dk)] {
        m.colored_vertex(pos, c);
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(m);
    p.rect_stroke(card, 10, Stroke::new(1.0, BORDER_HI), egui::StrokeKind::Outside);
    // nagłówek na grafice
    let o = hero.left_top() + Vec2::new(44.0, 40.0);
    logo(p, egui::Rect::from_min_size(o, Vec2::new(190.0, 48.0)), Color32::WHITE, 3.0);
    p.text(o + Vec2::new(0.0, 64.0), Align2::LEFT_TOP, "PIERZEJA", egui::FontId::new(46.0, bold()), Color32::WHITE);
    p.text(o + Vec2::new(0.0, 122.0), Align2::LEFT_TOP, "Proceduralny generator zabudowy", egui::FontId::proportional(16.0), Color32::from_rgb(0xf0, 0xf0, 0xf0));
    let badge = format!("{} · Mapa {:.0}–{:.0}", env!("CARGO_PKG_VERSION"), data().map_years[0], data().map_years[1]);
    let gal = p.layout_no_wrap(badge, mono(13.0), Color32::WHITE);
    let br = egui::Rect::from_min_size(egui::pos2(hero.right() - gal.size().x - 34.0, hero.top() + 18.0), gal.size() + Vec2::new(18.0, 10.0));
    p.rect_filled(br, 6, Color32::from_rgba_unmultiplied(0x15, 0x15, 0x15, 210));
    p.galley(br.min + Vec2::new(9.0, 5.0), gal, Color32::WHITE);

    let inner = lower.shrink2(Vec2::new(44.0, 26.0));
    let col_w = (inner.width() - 40.0) / 2.0;
    let bottom_h = 60.0;
    let cols = egui::Rect::from_min_max(inner.min, egui::pos2(inner.right(), inner.bottom() - bottom_h));
    let mut left = root.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(cols.min, Vec2::new(col_w, cols.height()))));
    left.label(RichText::new("Nowy budynek").family(bold()).size(15.0));
    left.add_space(6.0);
    let d = data();
    for (id, name) in [("familok", "Familok śląski"), ("kamienica_eklektyczna", "Kamienica"), ("kamienica_modernistyczna", "Modernizm"), ("blok_poodwilzowy", "Blok 1960"), ("dworek", "Dworek")] {
        let Some(a) = d.archetype(id) else { continue };
        let col = crate::rgb(a.vector.get("wallColor").and_then(|v| v.as_str()).unwrap_or("#808080"));
        if list_row(&mut left, Some(Color32::from_rgb(col[0], col[1], col[2])), None, name, &format!("{}–{}", a.years[0], a.years[1])).on_hover_text(&a.description).clicked() {
            cmds.push(Cmd::NewBuilding(id.into()));
        }
    }
    left.add_space(4.0);
    let r = Frame::new().stroke(Stroke::new(1.0, BORDER_HI)).corner_radius(6).show(&mut left, |ui| list_row(ui, None, Some(ic::CHART_BAR), "Nowa ulica (pierzeja)", "Ctrl+Shift+N")).inner;
    if r.clicked() {
        cmds.push(Cmd::NewStreet);
    }

    let mut right = root.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(egui::pos2(cols.left() + col_w + 40.0, cols.top()), Vec2::new(col_w, cols.height()))));
    right.label(RichText::new("Ostatnie pliki").family(bold()).size(15.0));
    right.add_space(6.0);
    right.label(RichText::new("Zapis projektów (.pzj) jeszcze nie działa — tu pojawią się ostatnio otwierane pliki.").size(12.5).color(DIM));
    right.add_space(8.0);
    right.label(RichText::new("Otwarte karty").size(12.5).color(FAINT));
    for (i, t) in docs.tabs.iter().enumerate() {
        if list_row(&mut right, None, Some(ic::FILE), &t.doc.name, t.doc.mode.label()).clicked() {
            cmds.push(Cmd::Tab(i));
        }
    }

    let foot = egui::Rect::from_min_max(egui::pos2(inner.left(), inner.bottom() - bottom_h + 14.0), inner.max);
    root.painter().hline(inner.x_range(), foot.top() - 4.0, Stroke::new(1.0, BORDER));
    let mut f = root.new_child(egui::UiBuilder::new().max_rect(foot.shrink2(Vec2::new(0.0, 4.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    f.add_enabled(false, egui::Button::new(RichText::new(format!("{}  Otwórz…", ic::FOLDER)).family(bold()).size(14.0).color(BG)).fill(Color32::from_rgb(0xf2, 0xf2, 0xf2)).min_size(Vec2::new(0.0, 34.0)))
        .on_disabled_hover_text("Format pliku .pzj — do ustalenia");
    f.add_enabled(false, egui::Button::new(RichText::new("Przywróć ostatnią sesję").size(14.0)).min_size(Vec2::new(0.0, 34.0))).on_disabled_hover_text("Wymaga zapisu sesji (.pzj)");
    f.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.add(egui::Label::new(RichText::new(format!("Wróć do edytora {}", ic::ARROW_RIGHT)).size(13.5).color(DIM)).sense(Sense::click())).clicked() {
            cmds.push(Cmd::Editor);
        }
    });
}

fn list_row(ui: &mut egui::Ui, swatch: Option<Color32>, icon: Option<&str>, name: &str, right: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(r, 6, Color32::from_rgba_unmultiplied(0xff, 0xff, 0xff, 14));
    }
    let x = r.left() + 16.0;
    if let Some(c) = swatch {
        p.rect_filled(egui::Rect::from_center_size(egui::pos2(x + 2.0, r.center().y), Vec2::splat(12.0)), 2, c);
    }
    if let Some(i) = icon {
        p.text(egui::pos2(x + 2.0, r.center().y), Align2::CENTER_CENTER, i, egui::FontId::proportional(15.0), DIM);
    }
    p.text(egui::pos2(x + 22.0, r.center().y), Align2::LEFT_CENTER, name, egui::FontId::proportional(15.0), TEXT);
    p.text(r.right_center() - Vec2::new(12.0, 0.0), Align2::RIGHT_CENTER, right, egui::FontId::proportional(13.5), DIM);
    resp
}

const TOOLS: [(&str, &str); 10] = [
    ("list_archetypes", "lista 19 archetypów"),
    ("describe_archetype", "opis i źródła archetypu"),
    ("get_parameter_schema", "schemat 71 parametrów"),
    ("resolve_parameters", "mieszanie wag → parametry"),
    ("generate_building", "budynek → GLB/OBJ"),
    ("render_building", "budynek → PNG"),
    ("generate_street", "pierzeja z profilu miasta"),
    ("generate_scene", "osiedle, skrzyżowanie, rondo"),
    ("list_city_profiles", "profile ulic"),
    ("morph_sequence", "sekwencja przejść między typami"),
];

fn mcp_exe() -> std::path::PathBuf {
    let dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default();
    dir.join(if cfg!(windows) { "pierzeja-mcp.exe" } else { "pierzeja-mcp" })
}

pub fn mcp(root: &mut egui::Ui, view: &mut View, cmds: &mut Vec<Cmd>) {
    let area = root.available_rect_before_wrap();
    root.painter().rect_filled(area, 0, BG);
    let w = (area.width() - 80.0).min(1160.0);
    let col = egui::Rect::from_min_size(egui::pos2(area.center().x - w / 2.0, area.top() + 40.0), Vec2::new(w, area.height() - 60.0));
    let mut ui = root.new_child(egui::UiBuilder::new().max_rect(col));
    let exe = mcp_exe();
    ui.horizontal(|ui| {
        if ui.add(egui::Label::new(RichText::new(format!("{} Edytor", ic::ARROW_LEFT)).size(14.0).color(DIM)).sense(Sense::click())).clicked() {
            cmds.push(Cmd::Editor);
        }
        ui.add_space(8.0);
        ui.label(RichText::new("Serwer MCP").family(bold()).size(20.0));
        ui.label(RichText::new("stdio · pierzeja-mcp").font(mono(13.0)).color(DIM));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if pill(ui, "", "Kopiuj konfigurację", false).on_hover_text("Wpis do claude_desktop_config.json / .mcp.json").clicked() {
                let cfg = serde_json::json!({ "mcpServers": { "pierzeja": { "command": exe.display().to_string() } } });
                ui.ctx().copy_text(serde_json::to_string_pretty(&cfg).unwrap_or_default());
            }
            ui.add_enabled(false, egui::Button::new(RichText::new("Uruchom 127.0.0.1:7337").size(14.0)).min_size(Vec2::new(0.0, 34.0))).on_disabled_hover_text("Wbudowany serwer HTTP — jeszcze niezaimplementowany");
        });
    });
    ui.add_space(18.0);
    ui.horizontal(|ui| {
        for (k, label) in [(0u8, "Log"), (1, "Ustawienia")] {
            let on = view.mcp_tab == k;
            let r = ui.add(egui::Label::new(RichText::new(label).size(14.5).color(if on { TEXT } else { DIM })).sense(Sense::click()));
            if on {
                ui.painter().hline(r.rect.x_range(), r.rect.bottom() + 6.0, Stroke::new(2.0, TEXT));
            }
            if r.clicked() {
                view.mcp_tab = k;
            }
            ui.add_space(12.0);
        }
    });
    ui.add_space(6.0);
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, BORDER));
    ui.add_space(18.0);
    let row = |ui: &mut egui::Ui, a: &str, b: &str, c: &str, cc: Color32| {
        Frame::new().inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(Vec2::new(200.0, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_width(200.0);
                    ui.label(RichText::new(a).size(14.0).color(DIM));
                });
                ui.label(RichText::new(b).font(mono(14.0)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(RichText::new(c).size(14.0).color(cc)));
            });
        });
    };
    if view.mcp_tab == 0 {
        Frame::new().fill(PANEL).corner_radius(6).inner_margin(Margin::same(14)).show(&mut ui, |ui| {
            ui.set_width(w - 28.0);
            ui.label(RichText::new("Log wywołań nie jest jeszcze dostępny.").family(bold()).size(14.0));
            ui.label(
                RichText::new("Wbudowany serwer HTTP (127.0.0.1:7337) z wieloma klientami i podglądem na żywo nie jest zaimplementowany. Teraz MCP działa jako osobny proces stdio: klient (Claude Desktop, Cursor, VS Code) uruchamia pierzeja-mcp i rozmawia z nim bezpośrednio, więc aplikacja nie widzi jego wywołań.")
                    .size(13.0)
                    .color(DIM),
            );
        });
        ui.add_space(14.0);
        ui.label(RichText::new("Narzędzia serwera").size(13.0).color(FAINT));
        for (name, desc) in TOOLS {
            row(&mut ui, "narzędzie", name, desc, DIM);
        }
    } else {
        let ok = exe.exists();
        row(&mut ui, "Plik serwera", &exe.display().to_string(), if ok { "jest" } else { "brak — cargo build -p pierzeja-mcp" }, if ok { DIM } else { WARN });
        row(&mut ui, "Katalog wyjściowy", &out_dir().display().to_string(), "PIERZEJA_OUT", DIM);
        row(&mut ui, "Port HTTP", "127.0.0.1:7337", "wkrótce", FAINT);
    }
}
