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

// ------------------------------------------------------------------ ładowanie (animacja z mocku „tryby edytora”)

const LOAD_STEPS: [&str; 10] = [
    "Inicjalizacja wgpu · Vulkan",
    "Kompilacja shaderów · 38",
    "Budynki · typologie Mapa 1893",
    "Budynki · reguły WT 2002",
    "Pojazdy · typy i podwozia · 9",
    "Pojazdy · PN-EN 1789",
    "Ludzie · archetypy · 9",
    "Ludzie · szkielet 54 kości",
    "Serwer MCP · 127.0.0.1:7337",
    "Gotowe",
];
/// Czas animacji [s]: 10 kroków po 0,5 s (jak w mocku); potem ekran startowy.
const LOAD_SECS: f32 = 5.0;

fn lerp_t(p: f32, a: f32, b: f32) -> f32 {
    ((p - a) / (b - a)).clamp(0.0, 1.0)
}
fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}
fn ease_back(x: f32) -> f32 {
    let (c1, x) = (1.6f32, x - 1.0);
    1.0 + (c1 + 1.0) * x.powi(3) + c1 * x * x
}
fn hex(h: u32) -> Color32 {
    Color32::from_rgb((h >> 16) as u8, (h >> 8) as u8, h as u8)
}

/// Pierwsze ładowanie edytora: miasto wyrasta, wjeżdża pojazd służb, pojawia się człowiek; trzy paski trybów.
pub fn loading(root: &mut egui::Ui, cmds: &mut Vec<Cmd>) {
    let ctx = root.ctx().clone();
    let now = ctx.input(|i| i.time);
    let t0 = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with::<f64>(egui::Id::new("load_t0"), || now));
    let t = (now - t0) as f32;
    if t >= LOAD_SECS || ctx.input(|i| i.pointer.any_click() || i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter)) {
        cmds.push(Cmd::Start);
        return;
    }
    ctx.request_repaint();
    let area = root.available_rect_before_wrap();
    let p = root.painter();
    p.rect_filled(area, 0, BG);
    let card = egui::Rect::from_center_size(area.center(), Vec2::new(720.0, 440.0));
    p.rect(card, 12, hex(0x1a1a1a), Stroke::new(1.0, hex(0x2c2c2c)), egui::StrokeKind::Inside);

    // nagłówek: znak rysowany kreską, nazwa, wersja
    let o = card.min + Vec2::new(32.0, 28.0);
    let k = 44.0 / 64.0;
    let pt = |x: f32, y: f32| o + Vec2::new(x * k, y * k);
    let paths: [&[(f32, f32)]; 3] = [&[(2.0, 36.0), (62.0, 36.0)], &[(6.0, 36.0), (6.0, 18.0), (16.0, 10.0), (26.0, 18.0), (26.0, 36.0)], &[(32.0, 36.0), (32.0, 24.0), (52.0, 24.0), (58.0, 30.0), (58.0, 36.0)]];
    let seg_len = |w: &[(f32, f32)]| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
    let total: f32 = paths.iter().map(|s| s.windows(2).map(seg_len).sum::<f32>()).sum();
    let mut left = total * ease_out(lerp_t(t / 5.0, 0.0, 0.14));
    let stroke = Stroke::new(2.0, hex(0xf2f2f2));
    for s in paths {
        let mut pts = vec![pt(s[0].0, s[0].1)];
        for w in s.windows(2) {
            let len = seg_len(w);
            let f = (left / len).clamp(0.0, 1.0);
            left -= len.min(left);
            pts.push(pt(w[0].0 + (w[1].0 - w[0].0) * f, w[0].1 + (w[1].1 - w[0].1) * f));
            if f < 1.0 {
                break;
            }
        }
        p.add(egui::Shape::line(pts, stroke));
    }
    if left > 0.0 {
        p.circle_stroke(pt(40.0, 14.0), 3.0 * k, stroke);
    }
    let mut x = o.x + 58.0;
    for (word, font, col) in [("PROCEDURA", egui::FontId::new(22.0, bold()), hex(0xf2f2f2)), ("SUITE", egui::FontId::proportional(22.0), hex(0xa9a9a9))] {
        for ch in word.chars() {
            x = p.text(egui::pos2(x, o.y + 14.0), Align2::LEFT_CENTER, ch, font.clone(), col).right() + 3.0;
        }
    }
    p.text(egui::pos2(card.right() - 32.0, o.y + 14.0), Align2::RIGHT_CENTER, format!("v{} · Mapa 1893", env!("CARGO_PKG_VERSION")), mono(12.0), hex(0x9a9a9a));

    // miasto: SVG 656×200, budynki rosną od podstawy (y = 170)
    let so = card.min + Vec2::new(32.0, 82.0);
    let s = |x: f32, y: f32| so + Vec2::new(x, y);
    let base = 170.0;
    let grow = |i: usize| ease_out(lerp_t((t - 0.06 * i as f32) / 5.0, 0.14, 0.30));
    let sy = |y: f32, g: f32| base - (base - y) * g;
    // okna: wzór 10×13 zakotwiczony w początku układu, rysowane tylko całe komórki wewnątrz bryły
    let windows = |x0: f32, y0: f32, w: f32, h: f32, g: f32| {
        let mut gx = (x0 / 10.0).floor() * 10.0;
        while gx < x0 + w {
            let mut gy = (y0 / 13.0).floor() * 13.0;
            while gy < y0 + h {
                let (cx, cy) = (gx + 3.0, gy + 4.0);
                if cx >= x0 && cx + 4.0 <= x0 + w && cy >= y0 && cy + 5.0 <= y0 + h {
                    p.rect_filled(egui::Rect::from_min_max(s(cx, sy(cy, g)), s(cx + 4.0, sy(cy + 5.0, g))), 0, hex(0x3b3b3b));
                }
                gy += 13.0;
            }
            gx += 10.0;
        }
    };
    // (indeks = opóźnienie, x, y, szerokość, wysokość, wypełnienie)
    let rects = [(0, 24.0, 100.0, 40.0, 70.0, 0x242424), (1, 70.0, 60.0, 34.0, 110.0, 0x262626), (3, 166.0, 40.0, 30.0, 130.0, 0x242424), (4, 202.0, 110.0, 44.0, 60.0, 0x282828), (5, 252.0, 70.0, 36.0, 100.0, 0x242424), (7, 348.0, 52.0, 32.0, 118.0, 0x242424), (8, 386.0, 104.0, 40.0, 66.0, 0x282828)];
    for (i, x0, y0, w, h, fill) in rects {
        let g = grow(i);
        if g > 0.0 {
            p.rect_filled(egui::Rect::from_min_max(s(x0, sy(y0, g)), s(x0 + w, base)), 0, hex(fill));
            windows(x0, y0, w, h, g);
        }
    }
    // domek (2, z pomarańczowym obrysem) i ratusz (6): wielokąty z oknami
    let house: &[(f32, f32)] = &[(110.0, 170.0), (110.0, 92.0), (135.0, 72.0), (160.0, 92.0), (160.0, 170.0)];
    let hall: &[(f32, f32)] = &[(294.0, 170.0), (294.0, 96.0), (306.0, 88.0), (330.0, 88.0), (342.0, 96.0), (342.0, 170.0)];
    for (i, pts, fill, line, win) in [(2, house, 0x3a2520, Stroke::new(2.0, hex(0xE8735A)), (120.0, 100.0, 30.0, 66.0)), (6, hall, 0x262626, Stroke::NONE, (298.0, 98.0, 40.0, 70.0))] {
        let g = grow(i);
        if g > 0.0 {
            p.add(egui::Shape::convex_polygon(pts.iter().map(|&(x, y)| s(x, sy(y, g))).collect(), hex(fill), line));
            windows(win.0, win.1, win.2, win.3, g);
        }
    }
    // pojazd służb wjeżdża z lewej
    let vp = t / 5.0;
    let ve = ease_out(lerp_t(vp, 0.38, 0.52));
    if ve > 0.0 {
        let (dx, a) = (-260.0 * (1.0 - ve), (ve * 255.0) as u8);
        let rgba = |h: u32| Color32::from_rgba_unmultiplied((h >> 16) as u8, (h >> 8) as u8, h as u8, a);
        let (ye, body) = (Stroke::new(2.0, rgba(0xE9B44C)), rgba(0x2e2a20));
        let v = |x: f32, y: f32| s(x + dx, y);
        p.rect(egui::Rect::from_min_max(v(462.0, 128.0), v(540.0, 164.0)), 3, body, ye, egui::StrokeKind::Middle);
        p.add(egui::Shape::convex_polygon([(540.0, 164.0), (540.0, 134.0), (556.0, 134.0), (568.0, 148.0), (568.0, 164.0)].map(|(x, y)| v(x, y)).to_vec(), body, ye));
        p.rect_filled(egui::Rect::from_min_max(v(540.0, 122.0), v(556.0, 128.0)), 2, rgba(if (t / 0.5) as u32 % 2 == 0 { 0x2f63d6 } else { 0x9ab8f5 }));
        for cx in [480.0, 552.0] {
            p.circle(v(cx, 166.0), 7.0, rgba(0x141414), ye);
        }
    }
    // człowiek „wyskakuje” (skala od podstawy z przeskokiem)
    let pe = ease_back(lerp_t(vp, 0.58, 0.70));
    if pe > 0.0 {
        let q = |x: f32, y: f32| s(612.0 + (x - 612.0) * pe, 172.0 - (172.0 - y) * pe);
        let st = Stroke::new(2.0, hex(0x4FB6C8));
        p.circle_stroke(q(612.0, 120.0), 8.0 * pe, st);
        let pts = [(598.0, 172.0), (598.0, 150.0), (600.0, 142.0), (605.0, 136.0), (612.0, 134.0), (619.0, 136.0), (624.0, 142.0), (626.0, 150.0), (626.0, 172.0)].map(|(x, y)| q(x, y)).to_vec();
        p.add(egui::Shape::convex_polygon(pts, hex(0x183238), st));
    }
    p.line_segment([s(16.0, 172.0), s(640.0, 172.0)], Stroke::new(1.5, hex(0x3a3a3a)));

    // dół: trzy tryby z paskami + status i procent
    let sf = t / 0.5;
    let step = (sf as usize).min(9);
    let inner = card.shrink2(Vec2::new(32.0, 24.0));
    let col_w = (inner.width() - 16.0) / 3.0;
    let bar_y = inner.bottom() - 16.0 - 10.0 - 4.0;
    for (i, (label, col, start)) in [("Budynki", 0xE8735A, 2.0f32), ("Pojazdy", 0xE9B44C, 4.0), ("Ludzie", 0x4FB6C8, 6.0)].into_iter().enumerate() {
        let f = ((sf - start + 1.0) / 2.0).clamp(0.0, 1.0);
        let x0 = inner.left() + i as f32 * (col_w + 8.0);
        let lc = if f == 0.0 { hex(0x8a8a8a) } else if f < 1.0 { Color32::WHITE } else { hex(col) };
        p.text(egui::pos2(x0, bar_y - 7.0), Align2::LEFT_BOTTOM, label, egui::FontId::proportional(12.0), lc);
        let track = egui::Rect::from_min_size(egui::pos2(x0, bar_y), Vec2::new(col_w, 4.0));
        p.rect_filled(track, 2, hex(0x2a2a2a));
        p.rect_filled(egui::Rect::from_min_size(track.min, Vec2::new(col_w * f, 4.0)), 2, hex(col));
    }
    p.text(egui::pos2(inner.left(), inner.bottom()), Align2::LEFT_BOTTOM, LOAD_STEPS[step], mono(12.0), hex(0xc4c4c4));
    p.text(egui::pos2(inner.right(), inner.bottom()), Align2::RIGHT_BOTTOM, format!("{}%", (step as f32 / 9.0 * 100.0).round()), mono(12.0), hex(0x9a9a9a));
}
