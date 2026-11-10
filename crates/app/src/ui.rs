//! Powłoka aplikacji wg makiety: pasek tytułu (karty, Ctrl+K, przyciski okna), wstążka,
//! pływające panele nad widokiem 3D, inspektor zaznaczonego elementu wstążki, nakładki
//! (gizmo osi, wymiar wysokości, linia gzymsu), pasek widoku i pasek stanu.
//! Akcje UI to `Cmd` wykonywane na końcu klatki — panele tylko je zgłaszają.

use crate::editors::{polygon_editor, typology_map};
use crate::screens;
use crate::theme::*;
use crate::*;
use bevy::camera::Viewport;
use bevy::diagnostic::DiagnosticsStore;
use bevy::window::PrimaryWindow;
use bevy_egui::egui::{self, Align2, Color32, Frame, Margin, Pos2, RichText, Sense, Stroke, Vec2};
use bevy_egui::EguiContexts;
use pierzeja_core::ParamKind;

#[derive(Clone, PartialEq, Debug)]
pub enum Cmd {
    Item(Item),
    Fam(Fam),
    NewVehicle(String),
    NewHuman(String),
    Pose(String),
    /// Gizmo edycji w widoku: włącz/wyłącz wszystkie (G).
    Gizmos,
    Reseed,
    Mode(Mode),
    Night,
    Shadows,
    Preset(Preset),
    Frame,
    Screenshot,
    Export,
    CopySpec,
    OpenFolder,
    Start,
    NewBuilding(String),
    NewStreet,
    CloseTab(usize),
    Tab(usize),
    Mcp,
    Editor,
    Undo,
    Redo,
    Archetype(String),
    Profile(String),
    Parcels(i32),
    Parcel(usize),
    OpenParcel(usize),
    Ribbon(RibbonTab),
    Palette,
    Quit,
    Minimize,
    Maximize,
}

/// Ikona, tytuł i klucze parametrów, które pokazuje inspektor elementu wstążki.
pub fn item_info(i: Item) -> (&'static str, &'static str, &'static [&'static str]) {
    match i {
        Item::Typologia => (ic::CHART_SCATTER, "Typologia", &[]),
        Item::Segmenty => (ic::COLUMNS, "Segmenty", &["segments", "segmentWidth", "depth", "stagger"]),
        Item::Oficyny => (ic::BOUNDING_BOX, "Oficyny", &["wingLeft", "wingRight", "wingRear", "wingLength", "wingWidth"]),
        Item::Uskoki => (ic::STAIRS, "Uskoki", &["setbackFloors", "setbackDepth"]),
        Item::Kondygnacje => (ic::ROWS, "Kondygnacje", &["floors", "groundFloorHeight", "floorHeight", "floorHeightDecay", "plinthHeight", "basementWindows"]),
        Item::Dach => (ic::HOUSE, "Dach", &["roofPitch", "roofHip", "mansard", "ridgeAlongFront", "roofOverhang", "parapetHeight"]),
        Item::Lukarny => (ic::TRIANGLE, "Lukarny", &["dormers", "chimneys", "rooftopBoxes", "gableGlazing", "turret"]),
        Item::Okna => (ic::GRID_FOUR, "Okna", &["bayWidth", "cornerMargin", "windowWidth", "windowHeight", "sillHeight", "windowDepth", "windowArch", "mullions", "ribbon", "stairWindows", "sideWindows"]),
        Item::Balkony => (ic::SQUARE_HALF, "Balkony", &["balconies", "balconyFrom", "balconyDepth", "balconyWidth", "loggia", "railing", "oriels", "orielDepth"]),
        Item::Parter => (ic::HOUSE_LINE, "Parter", &["groundShop", "groundRecess", "entranceType", "entranceCanopy", "rustication"]),
        Item::Gzymsy => (ic::ARCHIVE, "Gzymsy", &["cornice", "stringCourses", "ornament", "pilasters", "panelJoints", "asymmetry"]),
        Item::Material => (
            ic::SWATCHES,
            "Materiał",
            &["wallTexture", "wallColor", "trimColor", "plinthColor", "accentColor", "accentPanels", "roofTexture", "roofColor", "frameColor", "glassTint", "weathering"],
        ),
        Item::Wt2002 => (ic::SHIELD_CHECK, "WT 2002", &[]),
        Item::Dzialka => (ic::MAP_TRIFOLD, "Działka", &["streetSetback", "sideGap"]),
        Item::Profil => (ic::BUILDINGS, "Profil ulicy", &[]),
        Item::Gzyms => (ic::ARCHIVE, "Linia gzymsu", &[]),
        Item::Parcel => (ic::RULER, "Działka", &[]),
        Item::Scena => (ic::DOTS_NINE, "Scena", &[]),
        Item::Veh(_) => (ic::CAR, "Pojazd", &[]),
        Item::Hum(_) => (ic::PERSON_SIMPLE, "Człowiek", &[]),
    }
}

pub struct Rb {
    pub icon: &'static str,
    pub label: &'static str,
    pub cmd: Cmd,
    pub on: bool,
    pub enabled: bool,
}

fn rb(icon: &'static str, label: &'static str, cmd: Cmd, on: bool) -> Rb {
    Rb { icon, label, cmd, on, enabled: true }
}

/// Zawartość wstążki (grupy przycisków) dla zakładki — używana też przez Ctrl+K.
pub fn ribbon_def(tab: RibbonTab, doc: &Doc, view: &View) -> Vec<(&'static str, Vec<Rb>)> {
    let it = |i: Item| {
        let (icon, label, _) = item_info(i);
        let mut b = rb(icon, label, Cmd::Item(i), view.item == Some(i));
        b.enabled = doc.mode == Mode::Building;
        b
    };
    let sel = |i: Item, label: &'static str| Rb { label, ..it(i) };
    let street = doc.mode.is_street();
    let veh = doc.mode == Mode::Vehicle;
    let v = |id: &'static str, icon: &'static str, label: &'static str| Rb { enabled: veh, ..rb(icon, label, Cmd::Item(Item::Veh(id)), view.item == Some(Item::Veh(id))) };
    match tab {
        RibbonTab::Czlowiek => crate::human::ribbon(doc, view),
        RibbonTab::Pojazd => vec![
            ("Generuj", vec![v("typ", ic::CHART_SCATTER, "Typ"), Rb { enabled: veh, ..rb(ic::DICE_FIVE, "Losuj", Cmd::Reseed, false) }]),
            ("Nadwozie", vec![v("bryla", ic::BOUNDING_BOX, "Bryła"), v("sylwetka", ic::SHAPES, "Sylwetka"), v("zabudowa", ic::TRUCK, "Zabudowa")]),
            ("Koła i podwozie", vec![v("podwozie", ic::CIRCLE_NOTCH, "Podwozie")]),
            ("Wyposażenie", vec![v("sluzba", ic::SIREN, "Służba")]),
            ("Wykończenie", vec![v("material", ic::SWATCHES, "Materiał")]),
        ],
        RibbonTab::Plik => vec![
            ("Dokument", vec![rb(ic::PLUS, "Nowy", Cmd::Start, false), rb(ic::ROAD_HORIZON, "Nowa ulica", Cmd::NewStreet, false), rb(ic::CAR, "Nowy pojazd", Cmd::NewVehicle(tabor_core::data().default_archetype.clone()), false), rb(ic::PERSON_SIMPLE, "Nowy człowiek", Cmd::NewHuman(tlum_core::data().default_archetype.clone()), false), Rb { enabled: false, ..rb(ic::FOLDER_OPEN, "Otwórz…", Cmd::Start, false) }, Rb { enabled: false, ..rb(ic::FLOPPY_DISK, "Zapisz", Cmd::Start, false) }]),
            ("Karta", vec![rb(ic::ARROW_COUNTER_CLOCKWISE, "Cofnij", Cmd::Undo, false), rb(ic::ARROW_CLOCKWISE, "Ponów", Cmd::Redo, false), rb(ic::X, "Zamknij", Cmd::CloseTab(usize::MAX), false)]),
            ("Integracje", vec![rb(ic::PLUGS_CONNECTED, "Serwer MCP", Cmd::Mcp, false)]),
        ],
        RibbonTab::Budynek => vec![
            ("Generuj", vec![it(Item::Typologia), Rb { enabled: !street, ..rb(ic::DICE_FIVE, "Losuj", Cmd::Reseed, false) }]),
            ("Bryła", vec![it(Item::Segmenty), it(Item::Oficyny), it(Item::Uskoki), it(Item::Kondygnacje)]),
            ("Dach i elewacja", vec![it(Item::Dach), it(Item::Lukarny), it(Item::Okna), it(Item::Balkony), it(Item::Parter), it(Item::Gzymsy)]),
            ("Wykończenie", vec![sel(Item::Material, "Materiał")]),
            ("Reguły", vec![it(Item::Wt2002), it(Item::Dzialka)]),
        ],
        RibbonTab::Ulica => {
            let s = |b: Rb| Rb { enabled: street, ..b };
            let m = |icon, label, mode: Mode| rb(icon, label, Cmd::Mode(mode), doc.mode == mode);
            vec![
                ("Generuj", vec![s(rb(ic::DICE_FIVE, "Losuj ulicę", Cmd::Reseed, false)), s(rb(ic::BUILDINGS, "Profil", Cmd::Item(Item::Profil), view.item == Some(Item::Profil)))]),
                (
                    "Działki",
                    vec![
                        Rb { enabled: doc.mode == Mode::Street, ..rb(ic::PLUS_SQUARE, "Dodaj", Cmd::Parcels(1), false) },
                        Rb { enabled: doc.mode == Mode::Street, ..rb(ic::MINUS_CIRCLE, "Usuń", Cmd::Parcels(-1), false) },
                        Rb { enabled: doc.mode == Mode::Street, ..rb(ic::RULER, "Działka", Cmd::Item(Item::Parcel), view.item == Some(Item::Parcel)) },
                    ],
                ),
                ("Ulica", vec![m(ic::ROAD_HORIZON, "Pierzeja", Mode::Street), Rb { enabled: doc.mode == Mode::Street, ..rb(ic::ARCHIVE, "Gzyms", Cmd::Item(Item::Gzyms), view.item == Some(Item::Gzyms)) }]),
                ("Sceny", vec![m(ic::DOTS_NINE, "Osiedle", Mode::Estate), m(ic::INTERSECT, "Skrzyżowanie", Mode::Crossroads), m(ic::CIRCLE, "Rondo", Mode::Roundabout)]),
            ]
        }
        RibbonTab::Render => vec![
            ("Oświetlenie", vec![rb(ic::MOON, "Noc", Cmd::Night, view.night), rb(ic::SUN, "Cienie", Cmd::Shadows, view.shadows)]),
            ("Widok", vec![rb(ic::SQUARE, "Front", Cmd::Preset(Preset::Front), false), rb(ic::CUBE, "Izometria", Cmd::Preset(Preset::Iso), false), rb(ic::GRID_NINE, "Z góry", Cmd::Preset(Preset::Top), false), rb(ic::CORNERS_OUT, "Kadruj", Cmd::Frame, false)]),
            ("Obraz", vec![rb(ic::CAMERA, "Zrzut ekranu", Cmd::Screenshot, false)]),
        ],
        RibbonTab::Eksport => vec![("Eksport", vec![rb(ic::DOWNLOAD_SIMPLE, "GLB", Cmd::Export, false), rb(ic::COPY, "Kopiuj spec", Cmd::CopySpec, false), rb(ic::FOLDER_OPEN, "Folder", Cmd::OpenFolder, false)])],
    }
}

fn ribbon_tabs(f: Fam) -> Vec<(RibbonTab, &'static str)> {
    match f {
        Fam::Pojazdy => vec![(RibbonTab::Plik, "Plik"), (RibbonTab::Pojazd, "Pojazd"), (RibbonTab::Render, "Render"), (RibbonTab::Eksport, "Eksport")],
        Fam::Ludzie => vec![(RibbonTab::Plik, "Plik"), (RibbonTab::Czlowiek, "Człowiek"), (RibbonTab::Render, "Render"), (RibbonTab::Eksport, "Eksport")],
        _ => vec![(RibbonTab::Plik, "Plik"), (RibbonTab::Budynek, "Budynek"), (RibbonTab::Ulica, "Ulica"), (RibbonTab::Render, "Render"), (RibbonTab::Eksport, "Eksport")],
    }
}

/// Zakładka wstążki i element inspektora startowy dla dokumentu.
fn home_of(mode: Mode) -> (RibbonTab, Item) {
    match mode {
        Mode::Vehicle => (RibbonTab::Pojazd, Item::Veh("bryla")),
        Mode::Human => (RibbonTab::Czlowiek, Item::Hum("cialo")),
        Mode::Building => (RibbonTab::Budynek, Item::Kondygnacje),
        Mode::Street => (RibbonTab::Ulica, Item::Gzyms),
        _ => (RibbonTab::Ulica, Item::Scena),
    }
}

fn plural(n: usize, one: &str, few: &str, many: &str) -> String {
    let w = if n == 1 {
        one
    } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
        few
    } else {
        many
    };
    format!("{n} {w}")
}

/// Liczba z odstępem tysięcy: 8 055.
fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

fn new_seed() -> u32 {
    #[cfg(target_arch = "wasm32")]
    return (crate::web::now_ms() * 1000.0 % 100_000.0) as u32;
    #[cfg(not(target_arch = "wasm32"))]
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.subsec_nanos() % 100_000)
}

// ------------------------------------------------------------------ system UI

#[allow(clippy::too_many_arguments)]
pub fn ui(
    mut contexts: EguiContexts,
    mut docs: ResMut<Docs>,
    der: Res<Derived>,
    mut view: ResMut<View>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    mut camera: Query<(&mut Camera, &GlobalTransform), With<MainCamera>>,
    diag: Res<DiagnosticsStore>,
    mut exit: MessageWriter<AppExit>,
    mut init: Local<bool>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    if !*init {
        *init = true;
        install(&ctx);
        // fonty (rodzina „bold”) obowiązują od następnej klatki
        return Ok(());
    }
    let mut cmds: Vec<Cmd> = Vec::new();

    // skróty (gdy żadne pole tekstowe nie ma fokusu)
    if ctx.memory(|m| m.focused().is_none()) {
        use egui::{Key, KeyboardShortcut as K, Modifiers as M};
        ctx.input_mut(|i| {
            if i.consume_shortcut(&K::new(M::COMMAND | M::SHIFT, Key::Z)) || i.consume_shortcut(&K::new(M::COMMAND, Key::Y)) {
                cmds.push(Cmd::Redo);
            }
            if i.consume_shortcut(&K::new(M::COMMAND, Key::Z)) {
                cmds.push(Cmd::Undo);
            }
            if i.consume_shortcut(&K::new(M::COMMAND | M::SHIFT, Key::N)) {
                cmds.push(Cmd::NewStreet);
            }
            for (k, f) in [(Key::Num1, Fam::Budynki), (Key::Num2, Fam::Pojazdy), (Key::Num3, Fam::Ludzie)] {
                if i.consume_shortcut(&K::new(M::COMMAND, k)) {
                    cmds.push(Cmd::Fam(f));
                }
            }
            if i.consume_shortcut(&K::new(M::NONE, Key::G)) {
                cmds.push(Cmd::Gizmos);
            }
            if i.consume_shortcut(&K::new(M::COMMAND, Key::K)) {
                cmds.push(Cmd::Palette);
            }
        });
    }

    let mut root = egui::Ui::new(ctx.clone(), "root".into(), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.viewport_rect()));
    title_bar(&mut root, &docs, &view, &mut window, &mut cmds);
    let cam = camera.single().ok().map(|(c, g)| (c.clone(), *g));
    let mut vp3d = egui::Rect::NOTHING;
    match view.screen {
        Screen::Editor => {
            ribbon(&mut root, &docs, &view, &mut cmds);
            let fps = diag.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
            bottom_bar(&mut root, &docs, &der, &view, fps, &mut cmds);
            vp3d = root.available_rect_before_wrap();
            view.viewport = vp3d;
            editor_panels(&ctx, &mut docs, &der, &mut view, cam.as_ref(), &mut cmds);
        }
        Screen::Loading => screens::loading(&mut root, &mut cmds),
        Screen::Start => screens::start(&mut root, &docs, &mut cmds),
        Screen::Mcp => screens::mcp(&mut root, &mut view, &mut cmds),
    }
    palette(&ctx, &docs, &mut view, &mut cmds);

    for c in cmds {
        run(c, &ctx, &mut docs, &der, &mut view, &mut window, &mut exit);
    }
    // historia: zmiana zatwierdzana po puszczeniu myszy (przeciąganie suwaka = jeden krok)
    if !ctx.input(|i| i.pointer.any_down()) && ctx.memory(|m| m.focused().is_none()) {
        docs.commit();
    }

    // kamera 3D rysuje tylko w obszarze widoku (edytor) albo w grafice ekranu startowego
    if let Ok((mut cam, _)) = camera.single_mut() {
        let sf = window.scale_factor();
        let (w, h) = (window.physical_width(), window.physical_height());
        cam.is_active = vp3d.is_positive() && w > 16 && h > 16;
        if cam.is_active {
            let x = ((vp3d.left() * sf) as u32).min(w - 8);
            let y = ((vp3d.top() * sf) as u32).min(h - 8);
            let sw = ((vp3d.width() * sf) as u32).clamp(8, w - x);
            let sh = ((vp3d.height() * sf) as u32).clamp(8, h - y);
            cam.viewport = Some(Viewport { physical_position: UVec2::new(x, y), physical_size: UVec2::new(sw, sh), ..default() });
        }
    }
    Ok(())
}

fn run(c: Cmd, ctx: &egui::Context, docs: &mut Docs, der: &Derived, view: &mut View, window: &mut Window, exit: &mut MessageWriter<AppExit>) {
    let mode = docs.cur().mode;
    match c {
        Cmd::Item(i) => {
            view.item = Some(i);
            view.screen = Screen::Editor;
        }
        Cmd::Reseed => {
            let s = new_seed();
            let d = docs.cur_mut();
            match mode {
                Mode::Building | Mode::Vehicle | Mode::Human => d.seed = s,
                Mode::Street => d.street_seed = s,
                _ => d.scene_seed = s,
            }
        }
        Cmd::Mode(m) => {
            if matches!(mode, Mode::Building | Mode::Vehicle | Mode::Human) {
                docs.open(Doc::street(m));
            } else {
                docs.cur_mut().mode = m;
            }
            view.item = Some(if m == Mode::Street { Item::Gzyms } else { Item::Scena });
            view.ribbon = RibbonTab::Ulica;
            view.screen = Screen::Editor;
        }
        Cmd::Night => view.night = !view.night,
        Cmd::Gizmos => view.gizmos = !view.gizmos,
        Cmd::Pose(p) => docs.cur_mut().pose = p,
        Cmd::Shadows => view.shadows = !view.shadows,
        Cmd::Preset(p) => {
            view.preset = Some(p);
            if mode == Mode::Street {
                view.sview = if p == Preset::Top { SView::Plan } else if p == Preset::Front { SView::Elewacja } else { SView::Perspektywa };
            }
        }
        Cmd::Frame => view.reframe = true,
        Cmd::Screenshot => view.screenshot = true,
        Cmd::Export => {
            view.msg = match export_glb(docs.cur()) {
                Ok(p) => format!("Zapisano {}", p.display()),
                Err(e) => format!("Błąd zapisu: {e}"),
            }
        }
        Cmd::CopySpec => {
            let d = docs.cur();
            let s = match d.mode.scene() {
                Some(k) => serde_json::to_string_pretty(&d.scene_spec(k)),
                None if d.mode == Mode::Vehicle => serde_json::to_string_pretty(&d.vehicle_spec()),
                None if d.mode == Mode::Human => serde_json::to_string_pretty(&d.human_spec()),
                None if d.mode == Mode::Street => serde_json::to_string_pretty(&d.street_spec()),
                None => serde_json::to_string_pretty(&d.spec()),
            };
            ctx.copy_text(s.unwrap_or_default());
            view.msg = "Skopiowano spec do schowka".into();
        }
        Cmd::OpenFolder if crate::WEB => view.msg = "W przeglądarce eksport trafia do pobranych plików".into(),
        Cmd::OpenFolder => {
            let dir = out_dir();
            let _ = std::fs::create_dir_all(&dir);
            let opener = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
            let _ = std::process::Command::new(opener).arg(&dir).spawn();
        }
        Cmd::Start => view.screen = Screen::Start,
        Cmd::NewBuilding(a) => {
            docs.open(Doc::building(&a));
            (view.screen, view.ribbon, view.item) = (Screen::Editor, RibbonTab::Budynek, Some(Item::Kondygnacje));
        }
        Cmd::Fam(f) => match f {
            _ => {
                match docs.tabs.iter().rposition(|t| t.doc.mode.fam() == f) {
                    Some(i) => docs.active = i,
                    None if f == Fam::Pojazdy => docs.open(Doc::vehicle(&tabor_core::data().default_archetype)),
                    None if f == Fam::Ludzie => docs.open(Doc::human(&tlum_core::data().default_archetype)),
                    None => docs.open(Doc::building("familok")),
                }
                view.screen = Screen::Editor;
                (view.ribbon, view.item) = (home_of(docs.cur().mode).0, Some(home_of(docs.cur().mode).1));
            }
        },
        Cmd::NewVehicle(a) => {
            docs.open(Doc::vehicle(&a));
            view.screen = Screen::Editor;
            (view.ribbon, view.item) = (RibbonTab::Pojazd, Some(Item::Veh("bryla")));
        }
        Cmd::NewHuman(a) => {
            docs.open(Doc::human(&a));
            view.screen = Screen::Editor;
            (view.ribbon, view.item) = (RibbonTab::Czlowiek, Some(Item::Hum("cialo")));
        }
        Cmd::NewStreet => {
            docs.open(Doc::street(Mode::Street));
            (view.screen, view.ribbon, view.item, view.parcel) = (Screen::Editor, RibbonTab::Ulica, Some(Item::Gzyms), None);
        }
        Cmd::CloseTab(i) => {
            let i = if i == usize::MAX { docs.active } else { i };
            docs.tabs.remove(i);
            if docs.tabs.is_empty() {
                docs.tabs.push(Tab::new(Doc::building("familok")));
                view.screen = Screen::Start;
            }
            docs.active = docs.active.min(docs.tabs.len() - 1);
            if i < docs.active {
                docs.active -= 1;
            }
        }
        Cmd::Tab(i) => {
            docs.active = i;
            view.screen = Screen::Editor;
            let (rib, item) = home_of(docs.cur().mode);
            let ok = match docs.cur().mode {
                Mode::Vehicle => view.ribbon == RibbonTab::Pojazd,
                Mode::Human => view.ribbon == RibbonTab::Czlowiek,
                Mode::Building => view.ribbon == RibbonTab::Budynek,
                _ => view.ribbon == RibbonTab::Ulica,
            };
            if !ok {
                (view.ribbon, view.item) = (rib, Some(item));
            }
        }
        Cmd::Mcp => view.screen = Screen::Mcp,
        Cmd::Editor => view.screen = Screen::Editor,
        Cmd::Undo => docs.undo(),
        Cmd::Redo => docs.redo(),
        Cmd::Archetype(a) => {
            if matches!(mode, Mode::Building | Mode::Vehicle | Mode::Human) {
                docs.cur_mut().source = Source::Archetype(a);
            } else {
                run(Cmd::NewBuilding(a), ctx, docs, der, view, window, exit);
            }
        }
        Cmd::Profile(p) => {
            let d = docs.cur_mut();
            if let Some(c) = data().city(&p) {
                (d.count, d.mixing) = (c.count.clamp(1, 40), c.mixing);
            }
            d.profile = p;
        }
        Cmd::Parcels(k) => {
            let d = docs.cur_mut();
            d.count = (d.count as i32 + k).clamp(1, 40) as u32;
        }
        Cmd::Parcel(i) => {
            view.parcel = Some(i);
            if view.item != Some(Item::Gzyms) {
                view.item = Some(Item::Parcel);
            }
        }
        Cmd::OpenParcel(i) => {
            if let Some(p) = der.parcels.get(i) {
                let mut d = Doc::building(pc::DEFAULT_ARCHETYPE);
                d.name = format!("{}_dz{}", docs.cur().name, i + 1);
                d.source = Source::Weights(p.weights.clone());
                (d.seed, d.variation) = (p.seed, p.variation);
                docs.open(d);
                (view.ribbon, view.item) = (RibbonTab::Budynek, Some(Item::Kondygnacje));
            }
        }
        Cmd::Ribbon(t) => view.ribbon = t,
        Cmd::Palette => view.palette = Some(String::new()),
        Cmd::Quit => {
            exit.write(AppExit::Success);
        }
        Cmd::Minimize => window.set_minimized(true),
        Cmd::Maximize => {
            view.maximized = !view.maximized;
            window.set_maximized(view.maximized);
        }
    }
}

// ------------------------------------------------------------------ pasek tytułu

/// Logo: linia dachów (jak na ekranie startowym).
pub fn logo(p: &egui::Painter, r: egui::Rect, col: Color32, w: f32) {
    let pts = [(0.0, 1.0), (0.2, 1.0), (0.3, 0.72), (0.4, 0.72), (0.5, 0.3), (0.6, 0.72), (0.7, 0.72), (0.8, 1.0), (1.0, 1.0)];
    let pts: Vec<Pos2> = pts.iter().map(|(x, y)| egui::pos2(r.left() + x * r.width(), r.top() + y * r.height())).collect();
    p.add(egui::Shape::line(pts, Stroke::new(w, col)));
    for (x, y) in [(0.47, 0.62), (0.53, 0.62), (0.47, 0.78), (0.53, 0.78)] {
        p.rect_filled(egui::Rect::from_center_size(egui::pos2(r.left() + x * r.width(), r.top() + y * r.height()), Vec2::splat(w * 1.1)), 0, col);
    }
}

/// Przełącznik trybu edytora (Budynki / Pojazdy / Ludzie) z licznikiem kart.
fn mode_pill(ui: &mut egui::Ui, f: Fam, n: usize, active: bool) -> egui::Response {
    let (icon, enabled) = match f {
        Fam::Budynki => (ic::BUILDINGS, true),
        Fam::Pojazdy => (ic::CAR, true),
        Fam::Ludzie => (ic::USER, true),
    };
    let font = egui::FontId::proportional(13.5);
    let tw = ui.fonts_mut(|g| g.layout_no_wrap(f.label().into(), font.clone(), TEXT).size().x);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(tw + 78.0, 30.0), Sense::click());
    let col = f.color();
    let p = ui.painter();
    if active {
        p.rect(rect, 15, col.gamma_multiply(0.16), Stroke::new(1.0, col.gamma_multiply(0.8)), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, 15, HOVER);
    }
    let fg = if active { col } else if enabled { DIM } else { FAINT };
    p.text(rect.left_center() + Vec2::new(14.0, 0.0), Align2::LEFT_CENTER, icon, egui::FontId::proportional(15.0), fg);
    p.text(rect.left_center() + Vec2::new(36.0, 0.0), Align2::LEFT_CENTER, f.label(), font, if active { TEXT } else { fg });
    p.text(rect.right_center() - Vec2::new(12.0, 0.0), Align2::RIGHT_CENTER, n, mono(11.0), fg);
    if enabled { resp } else { resp.on_hover_text("Niedostępne") }
}

fn tab_chip(ui: &mut egui::Ui, name: &str, active: bool, dirty: bool) -> (bool, bool) {
    let font = egui::FontId::proportional(13.5);
    let tw = ui.fonts_mut(|f| f.layout_no_wrap(name.into(), font.clone(), TEXT).size().x);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(tw + 58.0, 30.0), Sense::click());
    let p = ui.painter();
    if active {
        p.rect(rect, 6, ACTIVE, Stroke::new(1.0, BORDER_HI), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, 6, HOVER);
    }
    p.text(rect.left_center() + Vec2::new(14.0, 0.0), Align2::LEFT_CENTER, name, font, if active { TEXT } else { DIM });
    if dirty {
        p.circle_filled(rect.left_center() + Vec2::new(14.0 + tw + 10.0, 0.0), 3.5, ACCENT);
    }
    let x = egui::Rect::from_center_size(rect.right_center() - Vec2::new(15.0, 0.0), Vec2::splat(18.0));
    let over_x = resp.hover_pos().is_some_and(|h| x.contains(h));
    if over_x {
        p.rect_filled(x, 4, BORDER_HI);
    }
    p.text(x.center(), Align2::CENTER_CENTER, ic::X, egui::FontId::proportional(11.0), DIM);
    let close = (resp.clicked() && over_x) || resp.middle_clicked();
    (resp.clicked() && !close, close)
}

fn title_bar(root: &mut egui::Ui, docs: &Docs, view: &View, window: &mut Window, cmds: &mut Vec<Cmd>) {
    egui::Panel::top("title").exact_size(44.0).resizable(false).frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(10, 0))).show(root, |ui| {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            static ICON: std::sync::OnceLock<egui::TextureHandle> = std::sync::OnceLock::new();
            let icon = ICON.get_or_init(|| texture(ui.ctx(), "icon", include_bytes!("../assets/icon.png")));
            ui.add(egui::Image::new((icon.id(), Vec2::splat(26.0))));
            ui.add_space(6.0);
            ui.label(RichText::new("Procedura").family(bold()).size(15.0));
            ui.label(RichText::new("Suite").size(15.0).color(DIM));
            ui.add_space(12.0);
            let cur_fam = docs.cur().mode.fam();
            for f in [Fam::Budynki, Fam::Pojazdy, Fam::Ludzie] {
                let n = docs.tabs.iter().filter(|t| t.doc.mode.fam() == f).count();
                if mode_pill(ui, f, n, f == cur_fam && view.screen == Screen::Editor).clicked() {
                    cmds.push(Cmd::Fam(f));
                }
            }
            ui.add_space(8.0);
            icon_btn(ui, ic::FLOPPY_DISK, false, "Zapis projektu (.pzj) — jeszcze nie ma formatu pliku");
            let t = &docs.tabs[docs.active];
            if icon_btn(ui, ic::ARROW_COUNTER_CLOCKWISE, false, &format!("Cofnij (Ctrl+Z) · {} kroków", t.undo.len())).clicked() {
                cmds.push(Cmd::Undo);
            }
            if icon_btn(ui, ic::ARROW_CLOCKWISE, false, "Ponów (Ctrl+Y)").clicked() {
                cmds.push(Cmd::Redo);
            }
            ui.add_space(8.0);
            let y = ui.max_rect().y_range();
            ui.painter().vline(ui.cursor().left(), y.min + 12.0..=y.max - 12.0, Stroke::new(1.0, BORDER));
            ui.add_space(10.0);
            let cur_fam = docs.cur().mode.fam();
            for (i, t) in docs.tabs.iter().enumerate().filter(|(_, t)| t.doc.mode.fam() == cur_fam) {
                let (open, close) = tab_chip(ui, &t.doc.name, i == docs.active && view.screen == Screen::Editor, !t.undo.is_empty() || t.doc != t.committed);
                if open {
                    cmds.push(Cmd::Tab(i));
                }
                if close {
                    cmds.push(Cmd::CloseTab(i));
                }
            }
            if icon_btn(ui, ic::PLUS, view.screen == Screen::Start, "Nowy dokument").clicked() {
                cmds.push(Cmd::Start);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                // w przeglądarce nie ma okna do zamknięcia/minimalizacji
                if !crate::WEB {
                    let close = icon_btn(ui, ic::X, false, "");
                    if close.hovered() {
                        ui.painter().rect_filled(close.rect, 6, Color32::from_rgb(0xc4, 0x2b, 0x1c));
                        ui.painter().text(close.rect.center(), Align2::CENTER_CENTER, ic::X, egui::FontId::proportional(17.0), Color32::WHITE);
                    }
                    if close.clicked() {
                        cmds.push(Cmd::Quit);
                    }
                    if icon_btn(ui, if view.maximized { ic::COPY_SIMPLE } else { ic::SQUARE }, false, "").clicked() {
                        cmds.push(Cmd::Maximize);
                    }
                    if icon_btn(ui, ic::MINUS, false, "").clicked() {
                        cmds.push(Cmd::Minimize);
                    }
                }
                ui.add_space(10.0);
                ui.label(RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).font(mono(12.0)).color(DIM));
                icon_btn(ui, ic::QUESTION, false, "Mysz: lewy — narzędzie z paska widoku, prawy — obrót, środkowy — przesunięcie, kółko — zoom\nF — kadruj · G — gizmo edycji · Ctrl+Z/Ctrl+Y — cofnij/ponów · Ctrl+K — polecenia · Ctrl+Shift+N — nowa ulica");
                ui.add_space(8.0);
                // pole „Szukaj poleceń” (otwiera paletę)
                let (r, resp) = ui.allocate_exact_size(Vec2::new(270.0, 30.0), Sense::click());
                ui.painter().rect(r, 6, INPUT, Stroke::new(1.0, if resp.hovered() { BORDER_HI } else { BORDER }), egui::StrokeKind::Inside);
                ui.painter().text(r.left_center() + Vec2::new(12.0, 0.0), Align2::LEFT_CENTER, ic::MAGNIFYING_GLASS, egui::FontId::proportional(14.0), DIM);
                ui.painter().text(r.left_center() + Vec2::new(34.0, 0.0), Align2::LEFT_CENTER, "Szukaj poleceń", egui::FontId::proportional(13.5), FAINT);
                let k = egui::Rect::from_center_size(r.right_center() - Vec2::new(34.0, 0.0), Vec2::new(52.0, 20.0));
                ui.painter().rect(k, 4, Color32::TRANSPARENT, Stroke::new(1.0, BORDER_HI), egui::StrokeKind::Inside);
                ui.painter().text(k.center(), Align2::CENTER_CENTER, "Ctrl K", mono(11.0), DIM);
                if resp.clicked() {
                    cmds.push(Cmd::Palette);
                }
                // reszta paska: przeciąganie okna, dwuklik — maksymalizacja
                let drag = ui.allocate_response(ui.available_size(), Sense::click_and_drag());
                if drag.drag_started() && !crate::WEB {
                    window.start_drag_move();
                }
                if drag.double_clicked() && !crate::WEB {
                    cmds.push(Cmd::Maximize);
                }
            });
        });
    });
}

// ------------------------------------------------------------------ wstążka

fn ribbon(root: &mut egui::Ui, docs: &Docs, view: &View, cmds: &mut Vec<Cmd>) {
    egui::Panel::top("ribbon").resizable(false).frame(Frame::new().fill(BAR).inner_margin(Margin { left: 12, right: 14, top: 4, bottom: 6 }).stroke(Stroke::NONE)).show(root, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (t, label) in ribbon_tabs(docs.cur().mode.fam()) {
                let on = view.ribbon == t;
                let rt = RichText::new(label).size(14.0).color(if on { TEXT } else { DIM });
                let b = egui::Button::new(if on { rt.family(bold()) } else { rt }).fill(if on { ACTIVE } else { Color32::TRANSPARENT }).stroke(Stroke::NONE).min_size(Vec2::new(0.0, 28.0));
                if ui.add(b).clicked() {
                    cmds.push(Cmd::Ribbon(t));
                }
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for (group, buttons) in ribbon_def(view.ribbon, docs.cur(), view) {
                ribbon_group(ui, group, |ui| {
                    for b in buttons {
                        if ribbon_btn(ui, b.icon, b.label, b.on, b.enabled).clicked() {
                            cmds.push(b.cmd);
                        }
                    }
                });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.add_space(10.0);
                    if pill(ui, ic::DOWNLOAD_SIMPLE, "Eksport GLB", false).on_hover_text(format!("Zapis do {}", out_dir().display())).clicked() {
                        cmds.push(Cmd::Export);
                    }
                });
            });
        });
    });
    // linia pod wstążką
    let y = root.available_rect_before_wrap().top();
    root.painter().hline(root.max_rect().x_range(), y, Stroke::new(1.0, BORDER));
}

// ------------------------------------------------------------------ pasek dolny

/// Sekcje okruszków (budynek): etykieta, ikona, element wstążki, elementy należące do sekcji.
const SECTIONS: [(&str, &str, Item, &[Item]); 7] = [
    ("Typologia", ic::CHART_SCATTER, Item::Typologia, &[Item::Typologia]),
    ("Działka", ic::MAP_TRIFOLD, Item::Dzialka, &[Item::Dzialka, Item::Wt2002]),
    ("Bryła", ic::BOUNDING_BOX, Item::Segmenty, &[Item::Segmenty, Item::Oficyny, Item::Uskoki]),
    ("Kondygnacje", ic::ROWS, Item::Kondygnacje, &[Item::Kondygnacje]),
    ("Dach", ic::HOUSE, Item::Dach, &[Item::Dach, Item::Lukarny]),
    ("Elewacja", ic::GRID_FOUR, Item::Okna, &[Item::Okna, Item::Balkony, Item::Parter, Item::Gzymsy]),
    ("Materiał", ic::SWATCHES, Item::Material, &[Item::Material]),
];

/// Okruszki pojazdu: etykieta, ikona, id wektora Tabora.
const VSECTIONS: [(&str, &str, &str); 7] = [
    ("Typ", ic::CHART_SCATTER, "typ"),
    ("Bryła", ic::BOUNDING_BOX, "bryla"),
    ("Sylwetka", ic::SHAPES, "sylwetka"),
    ("Podwozie", ic::CIRCLE_NOTCH, "podwozie"),
    ("Zabudowa", ic::TRUCK, "zabudowa"),
    ("Służba", ic::SIREN, "sluzba"),
    ("Materiał", ic::SWATCHES, "material"),
];

fn chip(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>, on: bool) -> egui::Response {
    ui.add(egui::Button::new(text).min_size(Vec2::new(0.0, 28.0)).fill(if on { ACTIVE } else { PANEL }).stroke(Stroke::new(1.0, if on { Color32::from_rgb(0x6a, 0x6a, 0x6a) } else { BORDER })))
}

fn bottom_bar(root: &mut egui::Ui, docs: &Docs, der: &Derived, view: &View, fps: f64, cmds: &mut Vec<Cmd>) {
    let doc = docs.cur();
    let street = doc.mode == Mode::Street;
    egui::Panel::bottom("status").exact_size(if street { 62.0 } else { 46.0 }).resizable(false).frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(14, 0))).show(root, |ui| {
        let full = ui.max_rect();
        ui.painter().hline(full.x_range(), full.top(), Stroke::new(1.0, BORDER));
        ui.horizontal_centered(|ui| {
            // stan (z prawej), reszta paska dla okruszków/linijki
            let status_w = 300.0;
            let left_w = (ui.available_width() - status_w).max(100.0);
            ui.allocate_ui_with_layout(Vec2::new(left_w, full.height()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                if street {
                    parcel_ruler(ui, der, view, cmds);
                } else if doc.mode == Mode::Vehicle {
                    let cur = VSECTIONS.iter().position(|s| view.item == Some(Item::Veh(s.2)));
                    for (k, (label, icon, id)) in VSECTIONS.iter().enumerate() {
                        let after = cur.is_some_and(|c| k > c);
                        let rt = RichText::new(format!("{icon}  {label}")).size(13.5).color(if after { FAINT } else { TEXT });
                        if chip(ui, rt, cur == Some(k)).clicked() {
                            cmds.push(Cmd::Item(Item::Veh(id)));
                        }
                    }
                } else if doc.mode == Mode::Human {
                    let cur = crate::human::HSECTIONS.iter().position(|s| view.item == Some(Item::Hum(s.2)));
                    for (k, (label, icon, id)) in crate::human::HSECTIONS.iter().enumerate() {
                        let after = cur.is_some_and(|c| k > c);
                        let rt = RichText::new(format!("{icon}  {label}")).size(13.5).color(if after { FAINT } else { TEXT });
                        if chip(ui, rt, cur == Some(k)).clicked() {
                            cmds.push(Cmd::Item(Item::Hum(id)));
                        }
                    }
                } else if doc.mode == Mode::Building {
                    let cur = SECTIONS.iter().position(|s| view.item.is_some_and(|i| s.3.contains(&i)));
                    for (k, (label, icon, item, _)) in SECTIONS.iter().enumerate() {
                        let after = cur.is_some_and(|c| k > c);
                        let rt = RichText::new(format!("{icon}  {label}")).size(13.5).color(if after { FAINT } else { TEXT });
                        if chip(ui, rt, cur == Some(k)).clicked() {
                            cmds.push(Cmd::Item(*item));
                        }
                    }
                } else {
                    ui.label(RichText::new(der.info.lines().next().unwrap_or("")).color(DIM));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if fps > 0.0 && !street {
                    ui.label(RichText::new(format!("{fps:.0} fps")).font(mono(12.0)).color(DIM));
                    ui.label(RichText::new("·").color(FAINT));
                }
                ui.label(RichText::new(format!("{} tri", thousands(der.tris))).font(mono(12.0)).color(DIM));
                let mcp = ui.add(egui::Label::new(RichText::new("MCP · stdio").size(13.0).color(DIM)).sense(Sense::click())).on_hover_text("Serwer MCP — szczegóły");
                if mcp.clicked() {
                    cmds.push(Cmd::Mcp);
                }
                if !view.msg.is_empty() {
                    ui.add(egui::Label::new(RichText::new(&view.msg).size(12.0).color(FAINT)).truncate());
                }
            });
        });
    });
}

/// Linijka działek ulicy: skala w metrach i „chipy” działek proporcjonalne do frontu.
fn parcel_ruler(ui: &mut egui::Ui, der: &Derived, view: &View, cmds: &mut Vec<Cmd>) {
    ui.label(RichText::new("Działki").family(bold()).size(14.0));
    ui.add_space(10.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width() - 8.0, 52.0), Sense::hover());
    let Some(first) = der.parcels.first() else { return };
    let x0 = first.x0;
    let len = der.parcels.last().map_or(1.0, |p| p.x0 + p.width - x0).max(1.0);
    let sx = |x: f64| rect.left() + ((x - x0) / len) as f32 * rect.width();
    let p = ui.painter();
    let mut m = 0.0;
    while m <= len + 0.01 {
        p.text(egui::pos2(sx(x0 + m), rect.top() + 2.0), Align2::LEFT_TOP, format!("{m:.0}"), mono(10.5), FAINT);
        m += 20.0;
    }
    p.text(egui::pos2(rect.right(), rect.top() + 2.0), Align2::RIGHT_TOP, format!("{len:.0} m"), mono(10.5), FAINT);
    for (i, pc) in der.parcels.iter().enumerate() {
        let r = egui::Rect::from_x_y_ranges(sx(pc.x0) + 1.5..=sx(pc.x0 + pc.width) - 1.5, rect.top() + 18.0..=rect.bottom() - 4.0);
        let resp = ui.interact(r, ui.id().with(("parcel", i)), Sense::click());
        let sel = view.parcel == Some(i);
        let p = ui.painter();
        p.rect(r, 5, if resp.hovered() { HOVER } else { PANEL }, Stroke::new(if sel { 1.5 } else { 1.0 }, if sel { TEXT } else { BORDER }), egui::StrokeKind::Inside);
        let [cr, cg, cb] = pc.color;
        let sw = egui::Rect::from_center_size(r.left_center() + Vec2::new(12.0, 0.0), Vec2::splat(9.0));
        p.rect_filled(sw, 2, Color32::from_rgb(cr, cg, cb));
        let clip = p.with_clip_rect(r.shrink(2.0));
        clip.text(r.left_center() + Vec2::new(22.0, 0.0), Align2::LEFT_CENTER, format!("{} · {}", i + 1, pc.name), egui::FontId::proportional(13.0), TEXT);
        if r.width() > 150.0 {
            clip.text(r.right_center() - Vec2::new(8.0, 0.0), Align2::RIGHT_CENTER, format!("{:.1} m", pc.height_walls), mono(11.0), DIM);
        }
        if resp.clicked() {
            cmds.push(Cmd::Parcel(i));
        }
        resp.on_hover_text(format!("{} · front {:.1} m · {} kond. · {:.1} m", pc.name, pc.width, pc.floors, pc.height_total));
    }
}

// ------------------------------------------------------------------ panele edytora

pub(crate) fn tree_row(ui: &mut egui::Ui, depth: usize, caret: Option<bool>, eye: Option<bool>, label: &str, value: &str, selected: bool) -> (egui::Response, bool) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 0, ACTIVE);
    } else if resp.hovered() {
        p.rect_filled(rect, 0, HOVER);
    }
    let mut x = rect.left() + 12.0 + depth as f32 * 16.0;
    let cy = rect.center().y;
    if let Some(open) = caret {
        p.text(egui::pos2(x, cy), Align2::LEFT_CENTER, if open { ic::CARET_DOWN } else { ic::CARET_RIGHT }, egui::FontId::proportional(9.0), DIM);
    }
    x += 14.0;
    let mut eye_hit = false;
    if let Some(on) = eye {
        let er = egui::Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(18.0, rect.height()));
        eye_hit = resp.clicked() && resp.interact_pointer_pos().is_some_and(|q| er.contains(q));
        p.text(egui::pos2(x, cy), Align2::LEFT_CENTER, if on { ic::EYE } else { ic::EYE_SLASH }, egui::FontId::proportional(14.0), if on { DIM } else { FAINT });
    }
    x += 22.0;
    let dim = eye == Some(false);
    p.text(egui::pos2(x, cy), Align2::LEFT_CENTER, label, egui::FontId::proportional(13.5), if dim { FAINT } else { TEXT });
    p.text(rect.right_center() - Vec2::new(14.0, 0.0), Align2::RIGHT_CENTER, value, mono(11.5), DIM);
    (resp, eye_hit)
}

fn param_field(ui: &mut egui::Ui, doc: &mut Doc, der: &Derived, key: &str) {
    param_field_in(ui, data(), doc, der, key);
}

pub(crate) fn param_field_in(ui: &mut egui::Ui, ds: &procedura_kit::data::Dataset, doc: &mut Doc, der: &Derived, key: &str) {
    let Some(def) = ds.param(key) else { return };
    let modified = doc.overrides.contains_key(key);
    let cur = doc.overrides.get(key).or(der.resolved.get(key)).cloned().unwrap_or(Value::Null);
    let mut new: Option<Value> = None;
    prop_row(ui, &def.label, modified, def.hint.as_deref(), |ui| match def.kind {
        ParamKind::Number | ParamKind::Int => {
            let mut v = cur.as_f64().unwrap_or(0.0);
            let step = if def.kind == ParamKind::Int { 1.0 } else { def.step.unwrap_or(0.01) };
            let dec = if step >= 1.0 { 0 } else if step >= 0.1 { 1 } else { 2 };
            let unit = def.unit.as_deref().unwrap_or(if def.kind == ParamKind::Int { "szt." } else { "×" });
            if num_field(ui, &mut v, def.min.unwrap_or(0.0)..=def.max.unwrap_or(1.0), step, dec, unit, modified).changed() {
                new = Some(pc::params::num(v));
            }
        }
        ParamKind::Enum => {
            let opts = def.options.as_deref().unwrap_or(&[]);
            let cur_s = cur.as_str().unwrap_or("");
            let label = opts.iter().find(|o| o.value == cur_s).map_or(cur_s, |o| o.label.as_str());
            egui::ComboBox::from_id_salt(key).width(124.0).selected_text(label).show_ui(ui, |ui| {
                for o in opts {
                    if ui.selectable_label(o.value == cur_s, &o.label).clicked() {
                        new = Some(Value::String(o.value.clone()));
                    }
                }
            });
        }
        ParamKind::Color => {
            let hex = cur.as_str().unwrap_or("#808080");
            let mut rgb = crate::rgb(hex);
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                new = Some(Value::String(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])));
            }
            ui.label(RichText::new(hex).font(mono(12.0)).color(DIM));
        }
    });
    if let Some(v) = new {
        doc.overrides.insert(key.to_string(), v);
    }
}

fn readonly(ui: &mut egui::Ui, label: &str, value: String) {
    prop_row(ui, label, false, None, |ui| {
        ui.label(RichText::new(value).font(mono(12.5)).color(DIM));
    });
}

pub(crate) fn body<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    Frame::new().inner_margin(Margin { left: 10, right: 12, top: 8, bottom: 10 }).show(ui, add).inner
}

pub(crate) fn footer_link(ui: &mut egui::Ui, text: &str, enabled: bool) -> bool {
    hline(ui);
    let mut clicked = false;
    Frame::new().inner_margin(Margin { left: 12, right: 12, top: 8, bottom: 10 }).show(ui, |ui| {
        let r = ui.add_enabled(enabled, egui::Label::new(RichText::new(text).size(13.5).color(if enabled { TEXT } else { FAINT })).sense(Sense::click()));
        clicked = r.clicked();
    });
    clicked
}

fn editor_panels(ctx: &egui::Context, docs: &mut Docs, der: &Derived, view: &mut View, cam: Option<&(Camera, GlobalTransform)>, cmds: &mut Vec<Cmd>) {
    let vp = view.viewport;
    if !vp.is_positive() {
        return;
    }
    let left = egui::pos2(vp.left() + 16.0, vp.top() + 16.0);
    let insp_w = 330.0;
    let insp = egui::pos2(vp.right() - 16.0 - insp_w - 2.0, vp.top() + 112.0);
    let mode = docs.cur().mode;
    let proj = |p: Vec3| cam.and_then(|(c, g)| c.world_to_viewport(g, p).ok()).map(|v| egui::pos2(v.x, v.y));

    if mode == Mode::Building {
        let doc = docs.cur_mut();
        // stała wysokość drzewa (ok. 9 wierszy, więcej → przewijanie): liczba segmentów i oficyn zmienia się
        // przy przeciąganiu parametrów, a Typologia pod spodem nie może wtedy skakać
        let tree_h = 300f32.min(vp.height() * 0.45);
        let tree = floating(ctx, "browser", left, 290.0, |ui| {
            ui.set_height(tree_h);
            egui::ScrollArea::vertical().max_height(tree_h).show(ui, |ui| building_tree(ui, doc, der, view, cmds));
        })
        .response
        .rect;
        floating(ctx, "typology", egui::pos2(left.x, tree.bottom() + 12.0), 290.0, |ui| {
            let cur = match &doc.source {
                Source::Archetype(a) => data().archetype(a).map_or(a.clone(), |x| x.short.clone()),
                Source::Cursor(y, _) => format!("Mapa {y:.0}"),
                Source::Weights(_) => "Mieszanka".into(),
            };
            panel_header(ui, None, "Typologia", |ui| {
                if ui.add(egui::Label::new(RichText::new(format!("{cur} {}", ic::CARET_DOWN)).size(12.5).color(DIM)).sense(Sense::click())).clicked() {
                    cmds.push(Cmd::Item(Item::Typologia));
                }
            });
            typology_map(ui, doc, data(), ACCENT, &der.weights, 200.0);
        });
        if let Some(item) = view.item {
            let (icon, title, keys) = item_info(item);
            floating(ctx, "inspector", insp, insp_w, |ui| {
                panel_header(ui, Some(icon), title, |ui| {
                    if icon_btn(ui, ic::X, false, "").clicked() {
                        view.item = None;
                    }
                });
                egui::ScrollArea::vertical().max_height(vp.height() - 220.0).show(ui, |ui| body(ui, |ui| building_inspector(ui, item, doc, der)));
                if !keys.is_empty() {
                    let any = keys.iter().any(|k| doc.overrides.contains_key(*k));
                    if footer_link(ui, "Przywróć typologię", any) {
                        keys.iter().for_each(|k| _ = doc.overrides.remove(*k));
                    }
                }
            });
        }
        // wymiar wysokości: kropka na okapie, linia do etykiety nad dachem (przy włączonych gizmo zastępują go wymiary z `gizmos`)
        if let Some(s) = der.stats.as_ref().filter(|_| !view.gizmos) {
            let (min, max) = der.bounds;
            let cx = (min.x + max.x) / 2.0 - (max.x - min.x) * 0.12;
            if let (Some(a), Some(b)) = (proj(Vec3::new(cx, s.height_walls as f32, max.z)), proj(Vec3::new(cx, s.height_total as f32 + 2.5, max.z))) {
                let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, "dims".into())).with_clip_rect(vp);
                p.line_segment([a, b], Stroke::new(1.5, ACCENT));
                p.circle(a, 5.0, Color32::from_rgb(0x20, 0x20, 0x20), Stroke::new(1.5, Color32::WHITE));
                dim_label(&p, b, &format!("{:.1} m", s.height_total), ACCENT);
            }
        }
    } else if mode == Mode::Vehicle {
        vehicle_panels(ctx, docs, der, view, left, insp, insp_w, &proj, cmds);
    } else if mode == Mode::Human {
        crate::human::panels(ctx, docs, der, view, left, insp, insp_w, cmds);
    } else {
        street_panels(ctx, docs, der, view, left, insp, insp_w, cmds);
        if mode == Mode::Street && view.cornice_show && view.sview != SView::Plan {
            cornice_overlay(ctx, der, view, &proj);
        }
    }
    if let Some((_, g)) = cam
        && !(mode == Mode::Street && view.sview != SView::Perspektywa)
    {
        gizmo(ctx, vp, g, cmds);
    }
    if view.gizmos && (mode.fam() != Fam::Budynki || mode == Mode::Building) {
        crate::gizmos::draw(ctx, docs, der, view, &proj);
    }
    viewport_toolbar(ctx, docs, view, cmds);
}

/// Panele pojazdu: przeglądarka wektorów, mapa typów (rok × klasa) i inspektor wektora Tabora.
#[allow(clippy::too_many_arguments)]
fn vehicle_panels(ctx: &egui::Context, docs: &mut Docs, der: &Derived, view: &mut View, left: Pos2, insp: Pos2, insp_w: f32, proj: &dyn Fn(Vec3) -> Option<Pos2>, cmds: &mut Vec<Cmd>) {
    let vp = view.viewport;
    let doc = docs.cur_mut();
    let td = tabor_core::data();
    let accent = Fam::Pojazdy.color();
    let tree = floating(ctx, "browser", left, 290.0, |ui| {
        panel_header(ui, None, "Przeglądarka", |ui| {
            ui.label(RichText::new(format!("1 pojazd {}", ic::CARET_DOWN)).size(12.5).color(DIM));
        });
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.y = 0.0;
        tree_row(ui, 0, Some(true), Some(view.building), &doc.name, "", false);
        for g in &td.groups {
            let Some(id) = VSECTIONS.iter().map(|s| s.2).find(|s| *s == g.id) else { continue };
            let n = td.params.iter().filter(|p| p.group == g.id && doc.overrides.contains_key(&p.key)).count();
            let val = if n > 0 { format!("{n} zm.") } else { String::new() };
            if tree_row(ui, 1, None, None, &g.label, &val, view.item == Some(Item::Veh(id))).0.clicked() {
                cmds.push(Cmd::Item(Item::Veh(id)));
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
        panel_header(ui, None, "Typ pojazdu", |ui| {
            if ui.add(egui::Label::new(RichText::new(format!("{cur} {}", ic::CARET_DOWN)).size(12.5).color(DIM)).sense(Sense::click())).clicked() {
                cmds.push(Cmd::Item(Item::Veh("typ")));
            }
        });
        typology_map(ui, doc, td, accent, &der.weights, 170.0);
    });
    if let Some(Item::Veh(g)) = view.item {
        let (icon, title) = if g == "typ" { (ic::CHART_SCATTER, "Typ".to_string()) } else { (ic::CAR, td.groups.iter().find(|x| x.id == g).map_or(g.to_string(), |x| x.label.clone())) };
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
                            egui::ComboBox::from_id_salt("vtyp").width(124.0).truncate().selected_text(&cur).show_ui(ui, |ui| {
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
    // wymiary: wysokość przy tyle, długość pod pojazdem (przy włączonych gizmo zastępują je wymiary z `gizmos`)
    if view.gizmos {
        return;
    }
    let (min, max) = der.bounds;
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, "dims".into())).with_clip_rect(vp);
    if let (Some(a), Some(b)) = (proj(Vec3::new(max.x + 0.3, min.y, min.z - 0.1)), proj(Vec3::new(max.x + 0.3, max.y, min.z - 0.1))) {
        p.line_segment([a, b], Stroke::new(1.5, accent));
        dim_label(&p, b, &format!("{:.2} m", max.y - min.y), accent);
    }
    if let (Some(a), Some(b)) = (proj(Vec3::new(max.x + 0.3, min.y, min.z)), proj(Vec3::new(max.x + 0.3, min.y, max.z))) {
        p.line_segment([a, b], Stroke::new(1.5, accent));
        dim_label(&p, a.lerp(b, 0.5) + Vec2::new(0.0, 30.0), &format!("{:.2} m", max.z - min.z), accent);
    }
}

pub(crate) fn dim_label(p: &egui::Painter, at: Pos2, text: &str, border: Color32) {
    let g = p.layout_no_wrap(text.into(), mono(12.5), Color32::WHITE);
    let r = egui::Rect::from_center_size(at - Vec2::new(0.0, 12.0), g.size() + Vec2::new(16.0, 8.0));
    p.rect(r, 4, Color32::from_rgb(0x18, 0x18, 0x18), Stroke::new(1.0, border), egui::StrokeKind::Inside);
    p.galley(r.center() - g.size() / 2.0, g, Color32::WHITE);
}

fn building_tree(ui: &mut egui::Ui, doc: &mut Doc, der: &Derived, view: &mut View, cmds: &mut Vec<Cmd>) {
    panel_header(ui, None, "Przeglądarka", |ui| {
        ui.label(RichText::new(format!("1 bryła {}", ic::CARET_DOWN)).size(12.5).color(DIM));
    });
    ui.add_space(4.0);
    ui.spacing_mut().item_spacing.y = 0.0;
    let item = view.item;
    let (r, eye) = tree_row(ui, 0, Some(true), Some(view.building), &doc.name, "", false);
    if eye {
        view.building = !view.building;
    } else if r.clicked() {
        cmds.push(Cmd::Item(Item::Typologia));
    }
    let n = |k: &str| der.resolved.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    if let Some(s) = &der.stats {
        let (r, eye) = tree_row(ui, 1, Some(false), Some(view.ground), "Działka", &format!("{:.0}×{:.0}", s.frontage, s.depth), item == Some(Item::Dzialka));
        if eye {
            view.ground = !view.ground;
        } else if r.clicked() {
            cmds.push(Cmd::Item(Item::Dzialka));
        }
        let massing = matches!(item, Some(Item::Kondygnacje | Item::Uskoki));
        if tree_row(ui, 1, Some(true), None, &der.dominant, &format!("{} kond.", s.floors), massing).0.clicked() {
            cmds.push(Cmd::Item(Item::Kondygnacje));
        }
        if doc.use_poly {
            if tree_row(ui, 2, None, None, "Obrys wielokątny", &format!("{} pkt", doc.poly.pts.len()), item == Some(Item::Dzialka)).0.clicked() {
                cmds.push(Cmd::Item(Item::Dzialka));
            }
        } else {
            let segs = pc::random::jround(n("segments")).max(1.0) as usize;
            for k in 0..segs.min(6) {
                if tree_row(ui, 2, None, None, &format!("Segment {}", (b'A' + k as u8) as char), &format!("{:.1}", n("segmentWidth")), item == Some(Item::Segmenty)).0.clicked() {
                    cmds.push(Cmd::Item(Item::Segmenty));
                }
            }
            if segs > 6 && tree_row(ui, 2, None, None, &format!("… i {} więcej", segs - 6), "", false).0.clicked() {
                cmds.push(Cmd::Item(Item::Segmenty));
            }
            let (ll, lr) = (n("wingLength") * n("wingLeft"), n("wingLength") * n("wingRight"));
            let wings = [("Oficyna lewa", ll, ll >= 2.0), ("Oficyna prawa", lr, lr >= 2.0), ("Oficyna tylna", ll.min(lr), ll >= 2.0 && lr >= 2.0 && n("wingRear") >= 0.5)];
            for (label, len, on) in wings {
                if on && tree_row(ui, 2, None, None, label, &format!("{len:.1}"), item == Some(Item::Oficyny)).0.clicked() {
                    cmds.push(Cmd::Item(Item::Oficyny));
                }
            }
        }
        if tree_row(ui, 2, None, None, "Dach", &format!("{:.0}°", n("roofPitch")), matches!(item, Some(Item::Dach | Item::Lukarny))).0.clicked() {
            cmds.push(Cmd::Item(Item::Dach));
        }
    }
    if tree_row(ui, 1, None, None, "Kamera", "F", false).0.clicked() {
        cmds.push(Cmd::Frame);
    }
    let (r, eye) = tree_row(ui, 1, None, Some(!view.night), "Słońce", if view.night { "noc" } else { "dzień" }, false);
    if eye || r.clicked() {
        cmds.push(Cmd::Night);
    }
    ui.add_space(6.0);
}

fn building_inspector(ui: &mut egui::Ui, item: Item, doc: &mut Doc, der: &Derived) {
    let (_, _, keys) = item_info(item);
    for k in keys {
        param_field(ui, doc, der, k);
    }
    match item {
        Item::Typologia => {
            let d = data();
            let cur = match &doc.source {
                Source::Archetype(a) => d.archetype(a).map_or(a.clone(), |x| x.name.clone()),
                Source::Cursor(y, s) => format!("Mapa {y:.0} · {s:.2}"),
                Source::Weights(_) => "Mieszanka z działki".into(),
            };
            prop_row(ui, "Typ", false, None, |ui| {
                egui::ComboBox::from_id_salt("typ").width(124.0).selected_text(cur).height(420.0).show_ui(ui, |ui| {
                    for a in &d.archetypes {
                        if ui.selectable_label(doc.source == Source::Archetype(a.id.clone()), format!("{} ({}–{})", a.name, a.years[0], a.years[1])).on_hover_text(&a.description).clicked() {
                            doc.source = Source::Archetype(a.id.clone());
                        }
                    }
                });
            });
            prop_row(ui, "Ostrość mieszania", doc.sharpness != 3.0, Some("Wykładnik IDW na mapie typologii: większy = bliżej najbliższego archetypu"), |ui| { let m = doc.sharpness != 3.0; num_field(ui, &mut doc.sharpness, 1.0..=8.0, 0.1, 1, "×", m) });
            prop_row(ui, "Sąsiedzi", doc.neighbors != 3.0, Some("Ile najbliższych archetypów miesza kursor mapy"), |ui| { let m = doc.neighbors != 3.0; num_field(ui, &mut doc.neighbors, 1.0..=6.0, 1.0, 0, "szt.", m) });
            let mut seed = doc.seed as f64;
            if prop_row(ui, "Ziarno", false, None, |ui| num_field(ui, &mut seed, 0.0..=99_999.0, 1.0, 0, "#", false)).changed() {
                doc.seed = seed as u32;
            }
            prop_row(ui, "Wariacja", doc.variation > 0.0, Some("Losowy rozrzut parametrów wokół typologii"), |ui| { let m = doc.variation > 0.0; num_field(ui, &mut doc.variation, 0.0..=1.0, 0.01, 2, "×", m) });
            ui.add_space(6.0);
            ui.label(RichText::new(format!("Datowanie ≈ {} · {}", der.year, der.mix)).size(12.0).color(DIM));
        }
        Item::Wt2002 => {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                toggle(ui, &mut doc.regulations, "Stosuj WT 2002 (budynki od 1990)");
            });
            ui.add_space(4.0);
            match der.stats.as_ref().map(|s| &s.notes) {
                Some(n) if !n.is_empty() => n.iter().for_each(|t| _ = ui.label(RichText::new(format!("• {t}")).size(12.5).color(DIM))),
                _ => _ = ui.label(RichText::new("Brak uwag dla tego budynku.").size(12.5).color(DIM)),
            }
            if let Some(s) = &der.stats {
                readonly(ui, "Grupa wysokości", s.height_class.to_string());
            }
        }
        Item::Dzialka => {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                toggle(ui, &mut doc.use_poly, "Obrys wielokątny");
            });
            if doc.use_poly {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    egui::ComboBox::from_id_salt("szablon").selected_text("Szablon obrysu").width(150.0).show_ui(ui, |ui| {
                        for name in crate::PRESETS {
                            if ui.selectable_label(false, name).clicked() {
                                doc.poly = Poly::preset(name);
                            }
                        }
                    });
                });
                polygon_editor(ui, doc, 240.0);
            }
            if let Some(s) = &der.stats {
                readonly(ui, "Front × głębokość", format!("{:.1} × {:.1} m", s.frontage, s.depth));
                readonly(ui, "Zabudowa / PC", format!("{:.0} / {:.0} m²", s.footprint, s.gfa));
            }
        }
        Item::Kondygnacje => {
            if let Some(s) = &der.stats {
                readonly(ui, "Wysokość całkowita", format!("{:.1} m", s.height_total));
                readonly(ui, "Mieszkania ≈", format!("{}", s.flats_estimate));
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn street_panels(ctx: &egui::Context, docs: &mut Docs, der: &Derived, view: &mut View, left: Pos2, insp: Pos2, insp_w: f32, cmds: &mut Vec<Cmd>) {
    let doc = docs.cur_mut();
    let d = data();
    let tree = floating(ctx, "browser", left, 290.0, |ui| {
        let right = if doc.mode == Mode::Street { plural(der.parcels.len(), "działka", "działki", "działek") } else { doc.mode.label().into() };
        panel_header(ui, None, "Przeglądarka", |ui| {
            ui.label(RichText::new(format!("{right} {}", ic::CARET_DOWN)).size(12.5).color(DIM));
        });
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.y = 0.0;
        tree_row(ui, 0, Some(true), Some(view.building), &doc.name, "", false);
        if doc.mode == Mode::Street {
            tree_row(ui, 1, None, Some(view.ground), "Ulica", "15 m", false);
            let len = der.parcels.last().zip(der.parcels.first()).map_or(0.0, |(l, f)| l.x0 + l.width - f.x0);
            tree_row(ui, 1, Some(true), None, "Działki", &format!("{len:.0} m"), false);
            for (i, p) in der.parcels.iter().enumerate() {
                if tree_row(ui, 2, None, None, &format!("{} · {}", i + 1, p.name), &format!("{:.1} · {}k", p.height_walls, p.floors), view.parcel == Some(i)).0.clicked() {
                    cmds.push(Cmd::Parcel(i));
                }
            }
        } else {
            for line in der.info.lines().skip(1) {
                let (name, val) = line.trim_start_matches("• ").split_once(" — ").unwrap_or((line, ""));
                tree_row(ui, 1, None, None, name, val, false);
            }
        }
        let (r, eye) = tree_row(ui, 1, None, Some(!view.night), "Słońce", if view.night { "noc" } else { "dzień" }, false);
        if eye || r.clicked() {
            cmds.push(Cmd::Night);
        }
        ui.add_space(6.0);
    });
    let lower = egui::pos2(left.x, tree.response.rect.bottom() + 12.0);
    let prof = d.city(&doc.profile);
    floating(ctx, "street-params", lower, 290.0, |ui| {
        panel_header(ui, None, if doc.mode == Mode::Street { "Ulica" } else { doc.mode.label() }, |_| {});
        body(ui, |ui| {
            let profile_combo = |ui: &mut egui::Ui, doc: &mut Doc, cmds: &mut Vec<Cmd>| {
                prop_row(ui, "Profil", false, prof.map(|p| p.description.as_str()), |ui| {
                    egui::ComboBox::from_id_salt("profil").width(124.0).truncate().selected_text(prof.map_or(doc.profile.clone(), |p| p.name.clone())).show_ui(ui, |ui| {
                        for c in &d.city_profiles {
                            if ui.selectable_label(doc.profile == c.id, &c.name).on_hover_text(&c.description).clicked() {
                                cmds.push(Cmd::Profile(c.id.clone()));
                            }
                        }
                    });
                });
            };
            let seed_row = |ui: &mut egui::Ui, seed: &mut u32| {
                let mut s = *seed as f64;
                if prop_row(ui, "Ziarno", false, None, |ui| num_field(ui, &mut s, 0.0..=99_999.0, 1.0, 0, "#", false)).changed() {
                    *seed = s as u32;
                }
            };
            match doc.mode {
                Mode::Street => {
                    profile_combo(ui, doc, cmds);
                    let mut count = doc.count as f64;
                    let mc = prof.is_some_and(|p| p.count != doc.count);
                    if prop_row(ui, "Działki", mc, None, |ui| num_field(ui, &mut count, 1.0..=40.0, 1.0, 0, "szt.", mc)).changed() {
                        doc.count = count as u32;
                    }
                    let mv = doc.street_var != 0.35;
                    prop_row(ui, "Rozrzut epok", mv, Some("Wariacja parametrów każdego budynku"), |ui| num_field(ui, &mut doc.street_var, 0.0..=1.0, 0.01, 2, "×", mv));
                    let mm = prof.is_some_and(|p| (p.mixing - doc.mixing).abs() > 1e-9);
                    prop_row(ui, "Mieszanie typów", mm, Some("Szansa, że budynek jest mieszanką dwóch archetypów"), |ui| num_field(ui, &mut doc.mixing, 0.0..=1.0, 0.01, 2, "×", mm));
                    seed_row(ui, &mut doc.street_seed);
                }
                Mode::Estate => {
                    prop_row(ui, "Odstęp rzędów", doc.spacing != 1.5, Some("WT 2002 § 13: co najmniej wysokość budynku przesłaniającego; osiedla PRL zwykle 1,5–2 H"), |ui| { let m = doc.spacing != 1.5; num_field(ui, &mut doc.spacing, 0.8..=3.0, 0.05, 2, "× H", m) });
                    let mut rows = doc.rows as f64;
                    if prop_row(ui, "Rzędy bloków", doc.rows != 3, None, |ui| num_field(ui, &mut rows, 1.0..=6.0, 1.0, 0, "szt.", doc.rows != 3)).changed() {
                        doc.rows = rows as u32;
                    }
                    seed_row(ui, &mut doc.scene_seed);
                }
                _ => {
                    profile_combo(ui, doc, cmds);
                    prop_row(ui, "Długość ramion", doc.arm != 90.0, None, |ui| { let m = doc.arm != 90.0; num_field(ui, &mut doc.arm, 50.0..=200.0, 1.0, 0, "m", m) });
                    seed_row(ui, &mut doc.scene_seed);
                }
            }
        });
    });
    let Some(item) = view.item.filter(|i| matches!(i, Item::Profil | Item::Gzyms | Item::Parcel)) else { return };
    if doc.mode != Mode::Street && item != Item::Profil {
        return;
    }
    let (icon, title, _) = item_info(item);
    floating(ctx, "inspector", insp, insp_w, |ui| {
        panel_header(ui, Some(icon), title, |ui| {
            if icon_btn(ui, ic::X, false, "").clicked() {
                view.item = None;
            }
        });
        body(ui, |ui| match item {
            Item::Profil => {
                for c in &d.city_profiles {
                    let on = doc.profile == c.id;
                    let r = ui.add(egui::Button::selectable(on, RichText::new(&c.name).size(13.5)).min_size(Vec2::new(ui.available_width(), 28.0)));
                    if r.on_hover_text(&c.description).clicked() {
                        cmds.push(Cmd::Profile(c.id.clone()));
                    }
                }
            }
            Item::Gzyms => {
                readonly(ui, "Wysokość linii", cornice_line(der, view).map_or("—".into(), |h| format!("{h:.1} m")));
                prop_row(ui, "Tolerancja", false, Some("Odchyłka od linii, od której budynek jest oznaczany"), |ui| num_field(ui, &mut view.cornice_tol, 0.0..=6.0, 0.1, 1, "± m", false));
                let out = cornice_line(der, view).map_or(0, |h| der.parcels.iter().filter(|p| (cornice_h(p, view) - h).abs() > view.cornice_tol).count());
                readonly(ui, "Odstające", format!("{out} z {}", der.parcels.len()));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    toggle(ui, &mut view.cornice_show, "Pokaż linię w widoku");
                });
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.checkbox(&mut view.cornice_walls, "Szczyty i attyki mogą wystawać").on_hover_text("Porównuj wysokość ścian (gzyms), nie całkowitą");
                });
            }
            _ => match view.parcel.and_then(|i| der.parcels.get(i).map(|p| (i, p))) {
                Some((i, p)) => {
                    readonly(ui, "Działka", format!("{} · {}", i + 1, p.name));
                    readonly(ui, "Front", format!("{:.1} m", p.width));
                    readonly(ui, "Wysokość ścian", format!("{:.1} m", p.height_walls));
                    readonly(ui, "Wysokość całkowita", format!("{:.1} m", p.height_total));
                    readonly(ui, "Kondygnacje", p.floors.to_string());
                    readonly(ui, "Trójkąty", thousands(p.tris));
                    let mix = p.weights.iter().map(|(k, w)| format!("{} {:.0}%", d.archetype(k).map_or(k.as_str(), |a| a.short.as_str()), w * 100.0)).collect::<Vec<_>>().join(" + ");
                    ui.label(RichText::new(mix).size(12.0).color(DIM));
                }
                None => _ = ui.label(RichText::new("Kliknij działkę na liście albo na linijce.").color(DIM)),
            },
        });
        if doc.mode == Mode::Street
            && item != Item::Profil
            && let Some(i) = view.parcel
            && footer_link(ui, &format!("Otwórz działkę {} {}", i + 1, ic::ARROW_RIGHT), true)
        {
            cmds.push(Cmd::OpenParcel(i));
        }
    });
}

fn cornice_h(p: &Parcel, view: &View) -> f64 {
    if view.cornice_walls { p.height_walls } else { p.height_total }
}

/// Linia gzymsu ulicy = mediana wysokości budynków.
fn cornice_line(der: &Derived, view: &View) -> Option<f64> {
    let mut h: Vec<f64> = der.parcels.iter().map(|p| cornice_h(p, view)).collect();
    h.sort_by(f64::total_cmp);
    h.get(h.len() / 2).copied()
}

fn cornice_overlay(ctx: &egui::Context, der: &Derived, view: &View, proj: &dyn Fn(Vec3) -> Option<Pos2>) {
    let Some(h) = cornice_line(der, view) else { return };
    let (Some(f), Some(l)) = (der.parcels.first(), der.parcels.last()) else { return };
    let z = der.parcels.iter().map(|p| p.center.z).fold(f32::MIN, f32::max);
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, "cornice".into())).with_clip_rect(view.viewport);
    let (Some(a), Some(b)) = (proj(Vec3::new(f.x0 as f32 - 3.0, h as f32, z)), proj(Vec3::new((l.x0 + l.width) as f32 + 1.0, h as f32, z))) else { return };
    p.extend(egui::Shape::dashed_line(&[a, b], Stroke::new(1.5, ACCENT), 7.0, 5.0));
    p.circle_stroke(b, 6.0, Stroke::new(1.5, ACCENT));
    dim_label(&p, b + Vec2::new(-34.0, -10.0), &format!("{h:.1} m"), ACCENT);
    for (i, pc) in der.parcels.iter().enumerate() {
        let ph = cornice_h(pc, view);
        let top = proj(Vec3::new(pc.center.x, pc.height_total as f32, pc.center.z));
        if view.parcel == Some(i)
            && let (Some(q0), Some(q1)) = (proj(Vec3::new(pc.x0 as f32, 0.0, pc.center.z)), proj(Vec3::new((pc.x0 + pc.width) as f32, pc.height_total as f32, pc.center.z)))
        {
            p.rect_stroke(egui::Rect::from_two_pos(q0, q1), 2, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Outside);
        }
        if (ph - h).abs() > view.cornice_tol
            && let Some(t) = top
        {
            dim_label(&p, t - Vec2::new(0.0, 10.0), &format!("{:+.1} m", ph - h), ACCENT_DIM);
        }
    }
}

/// Gizmo osi w prawym górnym rogu (rzut osi świata przez obrót kamery) i przycisk „kadruj”.
fn gizmo(ctx: &egui::Context, vp: egui::Rect, g: &GlobalTransform, cmds: &mut Vec<Cmd>) {
    let c = egui::pos2(vp.right() - 60.0, vp.top() + 60.0);
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, "gizmo".into()));
    p.circle_filled(c, 44.0, Color32::from_rgba_unmultiplied(0x18, 0x18, 0x18, 235));
    let inv = g.rotation().inverse();
    let mut axes = [(Vec3::X, "X", Color32::from_rgb(0xe0, 0x5a, 0x50)), (Vec3::Y, "Y", Color32::from_rgb(0x78, 0xc2, 0x5a)), (Vec3::Z, "Z", Color32::from_rgb(0x5a, 0x8a, 0xe8))].map(|(a, l, col)| (inv * a, l, col));
    axes.sort_by(|a, b| a.0.z.total_cmp(&b.0.z));
    for (v, l, col) in axes {
        let e = c + Vec2::new(v.x, -v.y) * 30.0;
        p.line_segment([c, e], Stroke::new(2.0, col));
        p.circle_filled(e, 8.0, col);
        p.text(e, Align2::CENTER_CENTER, l, egui::FontId::new(10.0, bold()), Color32::WHITE);
    }
    egui::Area::new("home".into()).fixed_pos(egui::pos2(c.x - 92.0, vp.top() + 16.0)).order(egui::Order::Middle).show(ctx, |ui| {
        Frame::new().fill(Color32::from_rgb(0x18, 0x18, 0x18)).corner_radius(6).show(ui, |ui| {
            if icon_btn(ui, ic::HOUSE, false, "Kadruj (F)").clicked() {
                cmds.push(Cmd::Frame);
            }
        });
    });
}

fn viewport_toolbar(ctx: &egui::Context, docs: &mut Docs, view: &mut View, cmds: &mut Vec<Cmd>) {
    let vp = view.viewport;
    let mode = docs.cur().mode;
    egui::Area::new("vtoolbar".into()).pivot(Align2::CENTER_BOTTOM).fixed_pos(egui::pos2(vp.center().x, vp.bottom() - 18.0)).order(egui::Order::Middle).show(ctx, |ui| {
        panel_frame().inner_margin(Margin::symmetric(6, 5)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if mode == Mode::Street {
                    for (v, label) in [(SView::Elewacja, "Elewacja"), (SView::Perspektywa, "Perspektywa"), (SView::Plan, "Plan")] {
                        let on = view.sview == v;
                        let b = egui::Button::new(RichText::new(label).size(14.0).color(if on { TEXT } else { DIM })).fill(if on { ACTIVE } else { Color32::TRANSPARENT }).stroke(Stroke::NONE).min_size(Vec2::new(0.0, 30.0));
                        if ui.add(b).clicked() && !on {
                            view.sview = v;
                            view.reframe = true;
                        }
                    }
                    sep(ui);
                    if icon_btn(ui, ic::ARROWS_OUT_CARDINAL, false, "Przesuwanie: lewy przycisk").clicked() {
                        view.drag = Drag::Pan;
                    }
                } else {
                    for (d, icon, tip) in [(Drag::Orbit, ic::ARROWS_CLOCKWISE, "Obrót"), (Drag::Pan, ic::ARROWS_OUT_CARDINAL, "Przesuwanie"), (Drag::Zoom, ic::MAGNIFYING_GLASS, "Przybliżanie")] {
                        if icon_btn(ui, icon, view.drag == d, tip).clicked() {
                            view.drag = d;
                        }
                    }
                }
                if icon_btn(ui, ic::CORNERS_OUT, false, "Kadruj (F)").clicked() {
                    cmds.push(Cmd::Frame);
                }
                if matches!(mode, Mode::Vehicle | Mode::Human) {
                    for (pr, label) in [(Preset::Side, "Bok"), (Preset::Front, "Przód"), (Preset::Top, "Góra"), (Preset::Iso, "Perspektywa")] {
                        let b = egui::Button::new(RichText::new(label).size(14.0).color(DIM)).fill(Color32::TRANSPARENT).stroke(Stroke::NONE).min_size(Vec2::new(0.0, 30.0));
                        if ui.add(b).clicked() {
                            cmds.push(Cmd::Preset(pr));
                        }
                    }
                    sep(ui);
                }
                if !matches!(mode, Mode::Street | Mode::Vehicle | Mode::Human) {
                    for (p, icon, tip) in [(Preset::Front, ic::SQUARE, "Widok z frontu"), (Preset::Iso, ic::CUBE, "Izometria")] {
                        if icon_btn(ui, icon, false, tip).clicked() {
                            cmds.push(Cmd::Preset(p));
                        }
                    }
                }
                if icon_btn(ui, ic::CIRCLE_HALF, view.night, "Noc").clicked() {
                    cmds.push(Cmd::Night);
                }
                if icon_btn(ui, ic::SUN, view.shadows, "Cienie").clicked() {
                    cmds.push(Cmd::Shadows);
                }
                sep(ui);
                if matches!(mode, Mode::Building | Mode::Vehicle | Mode::Human) {
                    crate::gizmos::toolbar_button(ui, mode, view, cmds);
                    sep(ui);
                }
                let doc = docs.cur_mut();
                let (v, def) = if mode == Mode::Street { (&mut doc.street_var, 0.35) } else { (&mut doc.variation, 0.35) };
                let mut on = *v > 0.0;
                if mode.scene().is_none() && toggle(ui, &mut on, "Wariacje").on_hover_text("Losowy rozrzut parametrów wokół typologii").changed() {
                    *v = if on { def } else { 0.0 };
                }
                ui.add_space(6.0);
            });
        });
    });
}

fn sep(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    let r = ui.available_rect_before_wrap();
    ui.painter().vline(r.left(), r.top() + 4.0..=r.top() + 26.0, Stroke::new(1.0, BORDER_HI));
    ui.add_space(6.0);
}

// ------------------------------------------------------------------ Ctrl+K

fn palette(ctx: &egui::Context, docs: &Docs, view: &mut View, cmds: &mut Vec<Cmd>) {
    let Some(mut q) = view.palette.take() else { return };
    let d = data();
    let doc = docs.cur();
    let mut all: Vec<(String, Cmd)> = Vec::new();
    for (t, tl) in ribbon_tabs(doc.mode.fam()) {
        for (_, bs) in ribbon_def(t, doc, view) {
            for b in bs.into_iter().filter(|b| b.enabled) {
                all.push((format!("{tl} › {}", b.label), b.cmd));
            }
        }
    }
    for a in &d.archetypes {
        all.push((format!("Nowy budynek: {} ({}–{})", a.name, a.years[0], a.years[1]), Cmd::NewBuilding(a.id.clone())));
        if doc.mode == Mode::Building {
            all.push((format!("Typ: {}", a.name), Cmd::Archetype(a.id.clone())));
        }
    }
    for a in &tabor_core::data().archetypes {
        all.push((format!("Nowy pojazd: {} ({}–{})", a.name, a.years[0], a.years[1]), Cmd::NewVehicle(a.id.clone())));
        if doc.mode == Mode::Vehicle {
            all.push((format!("Typ pojazdu: {}", a.name), Cmd::Archetype(a.id.clone())));
        }
    }
    for a in &tlum_core::data().archetypes {
        all.push((format!("Nowy człowiek: {}", a.name), Cmd::NewHuman(a.id.clone())));
        if doc.mode == Mode::Human {
            all.push((format!("Typ postaci: {}", a.name), Cmd::Archetype(a.id.clone())));
        }
    }
    all.push(("Gizmo edycji (G)".into(), Cmd::Gizmos));
    if doc.mode.is_street() {
        for c in &d.city_profiles {
            all.push((format!("Profil ulicy: {}", c.name), Cmd::Profile(c.id.clone())));
        }
    }
    all.push(("Ekran startowy".into(), Cmd::Start));
    for (i, t) in docs.tabs.iter().enumerate() {
        all.push((format!("Karta: {}", t.doc.name), Cmd::Tab(i)));
    }
    let ql = q.to_lowercase();
    let hits: Vec<&(String, Cmd)> = all.iter().filter(|(l, _)| ql.split_whitespace().all(|w| l.to_lowercase().contains(w))).take(12).collect();
    let mut close = false;
    let mut te_id = egui::Id::NULL;
    let area = egui::Area::new("palette".into()).pivot(Align2::CENTER_TOP).fixed_pos(egui::pos2(ctx.content_rect().center().x, 50.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        panel_frame().inner_margin(Margin::same(8)).show(ui, |ui| {
            ui.set_width(520.0);
            let te = ui.add(egui::TextEdit::singleline(&mut q).hint_text(format!("{}  Szukaj poleceń, typów, profili…", ic::MAGNIFYING_GLASS)).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 6)));
            te.request_focus();
            te_id = te.id;
            ui.add_space(4.0);
            let sel_id = egui::Id::new("palette-sel");
            let mut sel: usize = ctx.data(|m| m.get_temp(sel_id)).unwrap_or(0).min(hits.len().saturating_sub(1));
            ctx.input(|i| {
                if i.key_pressed(egui::Key::ArrowDown) {
                    sel = (sel + 1).min(hits.len().saturating_sub(1));
                }
                if i.key_pressed(egui::Key::ArrowUp) {
                    sel = sel.saturating_sub(1);
                }
            });
            for (k, (label, cmd)) in hits.iter().enumerate() {
                let r = ui.add(egui::Button::selectable(k == sel, RichText::new(label).size(13.5)).min_size(Vec2::new(ui.available_width(), 26.0)));
                if r.clicked() {
                    cmds.push(cmd.clone());
                    close = true;
                }
            }
            if hits.is_empty() {
                ui.label(RichText::new("Brak wyników").color(DIM));
            }
            ctx.data_mut(|m| m.insert_temp(sel_id, sel));
            if ctx.input(|i| i.key_pressed(egui::Key::Enter))
                && let Some((_, cmd)) = hits.get(sel)
            {
                cmds.push(cmd.clone());
                close = true;
            }
        });
    });
    let clicked_out = ctx.input(|i| i.pointer.any_click()) && !area.response.contains_pointer();
    if close || clicked_out || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        ctx.memory_mut(|m| m.surrender_focus(te_id));
        return;
    }
    view.palette = Some(q);
}
