//! Tryb Ludzie (generator Tłum): dokument, wstążka, przeglądarka wektorów, mapa typologii
//! (rok urodzenia × formalność stroju) i inspektor wektora. Wzorowany na trybie Pojazdy.

use crate::editors::typology_map;
use crate::theme::*;
use crate::ui::{body, footer_link, param_field_in, tree_row, Cmd, Rb};
use crate::*;
use bevy_egui::egui::{self, Pos2, RichText};

/// Okruszki i grupy wektora Tłumu: etykieta, ikona, id grupy (`typ` = archetyp, ziarno, poza).
pub const HSECTIONS: [(&str, &str, &str); 7] = [
    ("Typ", ic::CHART_SCATTER, "typ"),
    ("Ciało", ic::PERSON_SIMPLE, "cialo"),
    ("Głowa", ic::SMILEY, "glowa"),
    ("Ubiór", ic::T_SHIRT, "ubior"),
    ("Akcesoria", ic::BACKPACK, "akcesoria"),
    ("Sezon", ic::CLOUD_SUN, "sezon"),
    ("Materiał", ic::SWATCHES, "material"),
];

/// Pozy statyczne Tłumu: id, etykieta, ikona.
pub const POSES: [(&str, &str, &str); 5] = [
    ("stand", "Stoi", ic::PERSON_SIMPLE),
    ("walk", "Idzie", ic::PERSON_SIMPLE_WALK),
    ("sit", "Siedzi", ic::ARMCHAIR),
    ("drive", "Kieruje", ic::STEERING_WHEEL),
    ("rest", "T-pose", ic::PERSON_ARMS_SPREAD),
];

impl Doc {
    /// Człowiek Tłumu: te same pola mieszania co budynek/pojazd (`source`, `overrides`, `seed`, `variation`).
    pub fn human(archetype: &str) -> Doc {
        let mut d = Doc::building(archetype);
        d.mode = Mode::Human;
        d
    }
    pub fn human_spec(&self) -> tlum_core::TlumSpec {
        let walk = self.pose == "walk";
        tlum_core::TlumSpec { mix: self.spec().mix, pose: Some(self.pose.clone()), phase: walk.then_some(self.phase), version: None }
    }
}

/// Tekst panelu przeglądarki („• nazwa — wartość”).
pub fn info(m: &tlum_core::HumanModel) -> String {
    let n = |k: &str| m.params.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let pose = POSES.iter().find(|p| serde_json::to_value(m.stats.pose).ok().and_then(|v| v.as_str().map(|s| s == p.0)).unwrap_or(false)).map_or("", |p| p.1);
    format!("Człowiek\n• Poza — {pose}\n• Wzrost — {:.2} m\n• Wiek — {:.0} l.\n• BMI — {:.1}", m.stats.height, n("age"), n("bmi"))
}

fn rb(icon: &'static str, label: &'static str, cmd: Cmd, on: bool) -> Rb {
    Rb { icon, label, cmd, on, enabled: true }
}

/// Zakładka wstążki „Człowiek”.
pub fn ribbon(doc: &Doc, view: &View) -> Vec<(&'static str, Vec<Rb>)> {
    let hum = doc.mode == Mode::Human;
    let h = |id: &'static str, icon: &'static str, label: &'static str| Rb { enabled: hum, ..rb(icon, label, Cmd::Item(Item::Hum(id)), view.item == Some(Item::Hum(id))) };
    vec![
        ("Generuj", vec![h("typ", ic::CHART_SCATTER, "Typ"), Rb { enabled: hum, ..rb(ic::DICE_FIVE, "Losuj", Cmd::Reseed, false) }]),
        ("Postać", vec![h("cialo", ic::PERSON_SIMPLE, "Ciało"), h("glowa", ic::SMILEY, "Głowa")]),
        ("Ubiór", vec![h("ubior", ic::T_SHIRT, "Ubiór"), h("akcesoria", ic::BACKPACK, "Akcesoria")]),
        ("Otoczenie", vec![h("sezon", ic::CLOUD_SUN, "Sezon"), h("material", ic::SWATCHES, "Materiał")]),
        ("Poza", POSES.iter().map(|(id, label, icon)| Rb { enabled: hum, ..rb(icon, label, Cmd::Pose((*id).into()), hum && doc.pose == *id) }).collect()),
    ]
}

/// Panele człowieka: przeglądarka wektorów, mapa typologii (rok urodzenia × formalność) i inspektor wektora Tłumu.
#[allow(clippy::too_many_arguments)]
pub fn panels(ctx: &egui::Context, docs: &mut Docs, der: &Derived, view: &mut View, left: Pos2, insp: Pos2, insp_w: f32, cmds: &mut Vec<Cmd>) {
    let vp = view.viewport;
    let doc = docs.cur_mut();
    let td = tlum_core::data();
    let accent = Fam::Ludzie.color();
    let tree = floating(ctx, "browser", left, 290.0, |ui| {
        panel_header(ui, None, "Przeglądarka", |ui| {
            ui.label(RichText::new(format!("1 postać {}", ic::CARET_DOWN)).size(12.5).color(DIM));
        });
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.y = 0.0;
        tree_row(ui, 0, Some(true), Some(view.building), &doc.name, "", false);
        for g in &td.groups {
            let Some(id) = HSECTIONS.iter().map(|s| s.2).find(|s| *s == g.id) else { continue };
            let n = td.params.iter().filter(|p| p.group == g.id && doc.overrides.contains_key(&p.key)).count();
            let val = if n > 0 { format!("{n} zm.") } else { String::new() };
            if tree_row(ui, 1, None, None, &g.label, &val, view.item == Some(Item::Hum(id))).0.clicked() {
                cmds.push(Cmd::Item(Item::Hum(id)));
            }
        }
        for line in der.info.lines().skip(1) {
            let (name, val) = line.trim_start_matches("• ").split_once(" — ").unwrap_or((line, ""));
            tree_row(ui, 2, None, None, name, val, false);
        }
        let (r, eye) = tree_row(ui, 1, None, Some(!view.night), "Słońce", if view.night { "noc" } else { "dzień" }, false);
        if eye || r.clicked() {
            cmds.push(Cmd::Night);
        }
        ui.add_space(6.0);
    });
    let cur = match &doc.source {
        Source::Archetype(a) => td.archetype(a).map_or(a.clone(), |x| x.short.clone()),
        Source::Cursor(y, _) => format!("Mapa {y:.0}"),
        Source::Weights(_) => "Mieszanka".into(),
    };
    floating(ctx, "typology", egui::pos2(left.x, tree.response.rect.bottom() + 12.0), 290.0, |ui| {
        panel_header(ui, None, "Typ postaci", |ui| {
            if ui.add(egui::Label::new(RichText::new(format!("{cur} {}", ic::CARET_DOWN)).size(12.5).color(DIM)).sense(egui::Sense::click())).clicked() {
                cmds.push(Cmd::Item(Item::Hum("typ")));
            }
        });
        typology_map(ui, doc, td, accent, &der.weights, 170.0);
    });
    let Some(Item::Hum(g)) = view.item else { return };
    let (icon, title) = if g == "typ" { (ic::CHART_SCATTER, "Typ".to_string()) } else { (ic::PERSON_SIMPLE, td.groups.iter().find(|x| x.id == g).map_or(g.to_string(), |x| x.label.clone())) };
    floating(ctx, "inspector", insp, insp_w, |ui| {
        panel_header(ui, Some(icon), &title, |ui| {
            if icon_btn(ui, ic::X, false, "").clicked() {
                view.item = None;
            }
        });
        let keys: Vec<&str> = td.params.iter().filter(|p| p.group == g).map(|p| p.key.as_str()).collect();
        egui::ScrollArea::vertical().max_height(vp.height() - 220.0).show(ui, |ui| {
            body(ui, |ui| {
                if g == "typ" {
                    prop_row(ui, "Typ", false, None, |ui| {
                        egui::ComboBox::from_id_salt("htyp").width(124.0).truncate().selected_text(&cur).show_ui(ui, |ui| {
                            for a in &td.archetypes {
                                if ui.selectable_label(doc.source == Source::Archetype(a.id.clone()), format!("{} ({}–{})", a.name, a.years[0], a.years[1])).on_hover_text(&a.description).clicked() {
                                    doc.source = Source::Archetype(a.id.clone());
                                }
                            }
                        });
                    });
                    let mut seed = doc.seed as f64;
                    if prop_row(ui, "Ziarno", false, None, |ui| num_field(ui, &mut seed, 0.0..=99_999.0, 1.0, 0, "#", false)).changed() {
                        doc.seed = seed as u32;
                    }
                    prop_row(ui, "Wariacja", doc.variation > 0.0, Some("Losowy rozrzut parametrów wokół typu"), |ui| {
                        let m = doc.variation > 0.0;
                        num_field(ui, &mut doc.variation, 0.0..=1.0, 0.01, 2, "×", m)
                    });
                    prop_row(ui, "Poza", doc.pose != "stand", Some("Poza statyczna na szkielecie rr_human"), |ui| {
                        let label = POSES.iter().find(|p| p.0 == doc.pose).map_or("", |p| p.1);
                        egui::ComboBox::from_id_salt("hpose").width(124.0).selected_text(label).show_ui(ui, |ui| {
                            for (id, l, _) in POSES {
                                if ui.selectable_label(doc.pose == id, l).clicked() {
                                    doc.pose = id.into();
                                }
                            }
                        });
                    });
                    if doc.pose == "walk" {
                        prop_row(ui, "Faza kroku", doc.phase != 0.0, Some("0 = lewa noga w przód, 0,5 = prawa"), |ui| {
                            let m = doc.phase != 0.0;
                            num_field(ui, &mut doc.phase, 0.0..=1.0, 0.01, 2, "×", m)
                        });
                    }
                }
                for k in &keys {
                    param_field_in(ui, td, doc, der, k);
                }
            });
        });
        if !keys.is_empty() && footer_link(ui, "Przywróć typ", keys.iter().any(|k| doc.overrides.contains_key(*k))) {
            keys.iter().for_each(|k| _ = doc.overrides.remove(*k));
        }
    });
}
