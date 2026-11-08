//! Pierzeja — natywna aplikacja: Bevy (podgląd 3D z cieniami) + egui (UI wg makiety
//! `docs/ui-mock`: pasek tytułu z kartami dokumentów, wstążka, pływające panele, inspektor).
//! Mysz: lewy — narzędzie z paska widoku (obrót/przesunięcie/zoom), prawy — obrót,
//! środkowy — przesunięcie, kółko — przybliżenie, F — kadruj, Ctrl+Z/Ctrl+Y — cofnij/ponów,
//! Ctrl+K — szukaj poleceń.

mod editors;
mod screens;
mod theme;
mod ui;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::CameraOutputMode;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::BlendState;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{egui, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext};
use pierzeja_core::footprint::{Footprint, P2};
use pierzeja_core::scene::{generate_scene, SceneKind, SceneSpec};
use pierzeja_core::{self as pc, data, BuildingSpec, MatKey, MeshData, StreetSpec};
use serde_json::{json, Map, Value};
use std::path::PathBuf;

fn main() {
    let (docs, view) = initial_state();
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Pierzeja".into(), resolution: (1560, 975).into(), decorations: false, ..default() }),
            ..default()
        }))
        .add_plugins((EguiPlugin::default(), FrameTimeDiagnosticsPlugin::default()))
        .insert_resource(ClearColor(SKY))
        .insert_resource(bevy::light::DirectionalLightShadowMap { size: 4096 })
        .insert_resource(docs)
        .insert_resource(view)
        .init_resource::<Derived>()
        .insert_resource(Orbit::default())
        .add_systems(Startup, setup)
        .add_systems(EguiPrimaryContextPass, ui::ui)
        .add_systems(Update, (regenerate, orbit, screenshots).chain())
        .add_systems(Update, window_icon)
        .run();
}

const SKY: Color = Color::srgb(0.80, 0.85, 0.88);
const NIGHT_SKY: Color = Color::srgb(0.04, 0.06, 0.11);

/// `PIERZEJA_MODE=start|mcp|budynek|wielokat|ulica|osiedle|skrzyzowanie|rondo` — ekran startowy (zrzuty, testy).
fn initial_state() -> (Docs, View) {
    let mut view = View::default();
    let mut doc = Doc::building("familok");
    let m = std::env::var("PIERZEJA_MODE").unwrap_or_default();
    match m.as_str() {
        "" | "loading" => view.screen = Screen::Loading,
        "start" => view.screen = Screen::Start,
        "mcp" => view.screen = Screen::Mcp,
        "wielokat" => {
            doc.use_poly = true;
            view.item = Some(Item::Dzialka);
        }
        "pojazd" => {
            doc = Doc::street(Mode::Vehicle);
            view.ribbon = RibbonTab::Ulica;
            view.item = Some(Item::Scena);
        }
        "ulica" | "osiedle" | "skrzyzowanie" | "rondo" => {
            doc = Doc::street(match m.as_str() {
                "osiedle" => Mode::Estate,
                "skrzyzowanie" => Mode::Crossroads,
                "rondo" => Mode::Roundabout,
                _ => Mode::Street,
            });
            view.ribbon = RibbonTab::Ulica;
            view.item = Some(if doc.mode == Mode::Street { Item::Gzyms } else { Item::Scena });
            view.parcel = Some(2);
        }
        _ => {}
    }
    (Docs { tabs: vec![Tab::new(doc)], active: 0 }, view)
}

/// `PIERZEJA_SCREENSHOT=plik.png` — zrzut okna po starcie i wyjście; poza tym zrzuty na żądanie (wstążka Render).
fn screenshots(mut commands: Commands, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>, mut view: ResMut<View>) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    if view.screenshot {
        view.screenshot = false;
        let dir = out_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = unique(&dir, "zrzut", "png");
        view.msg = format!("Zrzut: {}", path.display());
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    let Some(path) = std::env::var_os("PIERZEJA_SCREENSHOT") else { return };
    *frame += 1;
    if *frame == 90 {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(PathBuf::from(path)));
    }
    if *frame == 130 {
        exit.write(AppExit::Success);
    }
}

// ------------------------------------------------------------------ dokumenty

#[derive(PartialEq, Clone, Debug)]
pub enum Source {
    Archetype(String),
    Cursor(f64, f64),
    /// Mieszanka z działki ulicy („Otwórz działkę”).
    Weights(Vec<(String, f64)>),
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Mode {
    Building,
    Street,
    Estate,
    Crossroads,
    Roundabout,
    /// Pojazd z Tabora (`Doc::vehicle` = archetyp, ziarno = `scene_seed`).
    Vehicle,
}

impl Mode {
    pub fn scene(self) -> Option<SceneKind> {
        match self {
            Mode::Estate => Some(SceneKind::Estate),
            Mode::Crossroads => Some(SceneKind::Crossroads),
            Mode::Roundabout => Some(SceneKind::Roundabout),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Mode::Building => "Budynek",
            Mode::Street => "Pierzeja",
            Mode::Estate => "Osiedle",
            Mode::Crossroads => "Skrzyżowanie",
            Mode::Roundabout => "Rondo",
            Mode::Vehicle => "Pojazd",
        }
    }
}

/// Obrys wielokątny; rola krawędzi: 0 zwykła, 1 front (ulica), 2 ślepa (styk z sąsiadem).
#[derive(Clone, PartialEq, Debug)]
pub struct Poly {
    pub pts: Vec<P2>,
    pub roles: Vec<u8>,
}

impl Poly {
    pub fn footprint(&self) -> Footprint {
        let by = |r: u8| (0..self.pts.len()).filter(|&i| self.roles[i] == r).collect();
        Footprint { points: self.pts.clone(), front: by(1), blind: by(2) }
    }
    pub fn preset(name: &str) -> Poly {
        let (pts, roles): (Vec<P2>, Vec<u8>) = match name {
            // narożnik ze ściętym rogiem, ulice od +z i od −x
            "Narożnik" => (vec![[-10.0, 12.0], [-13.0, 9.0], [-13.0, -10.0], [-1.0, -10.0], [-1.0, 0.0], [13.0, 0.0], [13.0, 12.0]], vec![1, 1, 2, 0, 0, 2, 1]),
            "L" => (vec![[-12.0, 8.0], [12.0, 8.0], [12.0, -4.0], [0.0, -4.0], [0.0, -16.0], [-12.0, -16.0]], vec![1, 0, 0, 0, 0, 0]),
            "U (podwórze)" => (
                vec![[-14.0, 6.0], [14.0, 6.0], [14.0, -22.0], [7.0, -22.0], [7.0, -6.0], [-7.0, -6.0], [-7.0, -22.0], [-14.0, -22.0]],
                vec![1, 2, 0, 0, 0, 0, 0, 2],
            ),
            "T" => (vec![[-30.0, 6.0], [30.0, 6.0], [30.0, -6.0], [6.0, -6.0], [6.0, -34.0], [-6.0, -34.0], [-6.0, -6.0], [-30.0, -6.0]], vec![1, 0, 0, 0, 0, 0, 0, 0]),
            "Trapez" => (vec![[-11.0, 7.0], [11.0, 7.0], [8.0, -7.0], [-6.0, -7.0]], vec![1, 0, 0, 0]),
            "Łuk" => {
                let mut v = vec![];
                for k in 0..=6 {
                    let t = std::f64::consts::PI * (0.25 + 0.5 * k as f64 / 6.0);
                    v.push([-30.0 * t.cos(), 30.0 * t.sin() - 18.0]);
                }
                for k in (0..=6).rev() {
                    let t = std::f64::consts::PI * (0.25 + 0.5 * k as f64 / 6.0);
                    v.push([-42.0 * t.cos(), 42.0 * t.sin() - 18.0]);
                }
                let mut r = vec![0u8; v.len()];
                (0..6).for_each(|i| r[i] = 1);
                (v, r)
            }
            "Sześciokąt" => (vec![[-5.0, 6.0], [5.0, 6.0], [8.0, 0.0], [5.0, -6.0], [-5.0, -6.0], [-8.0, 0.0]], vec![1, 0, 0, 0, 0, 0]),
            _ => (vec![[-10.0, 6.0], [10.0, 6.0], [10.0, -6.0], [-10.0, -6.0]], vec![1, 0, 0, 0]),
        };
        Poly { pts, roles }
    }
}

pub const PRESETS: [&str; 8] = ["Prostokąt", "Narożnik", "L", "U (podwórze)", "T", "Trapez", "Łuk", "Sześciokąt"];

/// Stan jednego dokumentu (karty). Porównywany co klatkę: zmiana = przebudowa sceny
/// i (po puszczeniu myszy) wpis do historii cofania.
#[derive(Clone, PartialEq, Debug)]
pub struct Doc {
    pub name: String,
    pub mode: Mode,
    // budynek
    pub source: Source,
    pub overrides: Map<String, Value>,
    pub seed: u32,
    pub variation: f64,
    pub regulations: bool,
    pub sharpness: f64,
    pub neighbors: f64,
    pub use_poly: bool,
    pub poly: Poly,
    // ulica
    pub profile: String,
    pub count: u32,
    pub street_seed: u32,
    pub street_var: f64,
    pub mixing: f64,
    // sceny
    pub spacing: f64,
    pub rows: u32,
    pub arm: f64,
    pub scene_seed: u32,
    // pojazd (Tabor)
    pub vehicle: String,
}

impl Doc {
    pub fn building(archetype: &str) -> Doc {
        let p = &data().city_profiles[0];
        Doc {
            name: archetype.into(),
            mode: Mode::Building,
            source: Source::Archetype(archetype.into()),
            overrides: Map::new(),
            seed: 1,
            variation: 0.0,
            regulations: true,
            sharpness: 3.0,
            neighbors: 3.0,
            use_poly: false,
            poly: Poly::preset("Narożnik"),
            profile: p.id.clone(),
            count: 6,
            street_seed: 7,
            street_var: 0.35,
            mixing: p.mixing,
            spacing: 1.5,
            rows: 3,
            arm: 90.0,
            scene_seed: 11,
            vehicle: tabor_core::data().default_archetype.clone(),
        }
    }
    pub fn street(mode: Mode) -> Doc {
        let mut d = Doc::building(pc::DEFAULT_ARCHETYPE);
        d.mode = mode;
        d.name = match mode {
            Mode::Street => format!("ulica_{}", d.profile.split('_').next().unwrap_or("")),
            m => m.label().to_lowercase(),
        };
        d
    }
    pub fn spec(&self) -> BuildingSpec {
        let mut s = BuildingSpec { mix: pc::MixSpec { seed: Some(self.seed as f64), ..Default::default() }, ..Default::default() };
        match &self.source {
            Source::Archetype(a) => s.mix.archetype = Some(a.clone()),
            Source::Cursor(year, scale) => s.mix.cursor = Some(pc::MapPoint { year: *year, scale: *scale }),
            Source::Weights(w) => s.mix.weights = Some(w.iter().cloned().collect()),
        }
        if self.variation > 0.0 {
            s.mix.variation = Some(self.variation);
        }
        if !self.overrides.is_empty() {
            s.mix.overrides = Some(self.overrides.clone());
        }
        if self.sharpness != 3.0 {
            s.mix.sharpness = Some(self.sharpness);
        }
        if self.neighbors != 3.0 {
            s.mix.neighbors = Some(self.neighbors);
        }
        if !self.regulations {
            s.regulations = Some(false);
        }
        if self.use_poly {
            s.footprint = Some(self.poly.footprint());
        }
        s
    }
    pub fn street_spec(&self) -> StreetSpec {
        StreetSpec {
            profile: Some(self.profile.clone()),
            count: Some(self.count as f64),
            seed: Some(self.street_seed as f64),
            variation: Some(self.street_var),
            mixing: Some(self.mixing),
            ..Default::default()
        }
    }
    pub fn vehicle_spec(&self) -> tabor_core::TaborSpec {
        serde_json::from_value(json!({ "archetype": self.vehicle, "seed": self.scene_seed })).expect("TaborSpec")
    }
    pub fn scene_spec(&self, kind: SceneKind) -> SceneSpec {
        SceneSpec { kind, seed: Some(self.scene_seed as f64), spacing: Some(self.spacing), rows: Some(self.rows), profile: Some(self.profile.clone()), arm: Some(self.arm), variation: None }
    }
}

/// Karta: dokument + historia (stos migawek; ostatnia zatwierdzona = `committed`).
pub struct Tab {
    pub doc: Doc,
    pub committed: Doc,
    pub undo: Vec<Doc>,
    pub redo: Vec<Doc>,
}

impl Tab {
    pub fn new(doc: Doc) -> Tab {
        Tab { committed: doc.clone(), doc, undo: Vec::new(), redo: Vec::new() }
    }
}

#[derive(Resource)]
pub struct Docs {
    pub tabs: Vec<Tab>,
    pub active: usize,
}

impl Docs {
    pub fn cur(&self) -> &Doc {
        &self.tabs[self.active].doc
    }
    pub fn cur_mut(&mut self) -> &mut Doc {
        &mut self.tabs[self.active].doc
    }
    pub fn open(&mut self, mut doc: Doc) {
        // nazwa unikalna wśród kart
        let base = doc.name.clone();
        let mut i = 2;
        while self.tabs.iter().any(|t| t.doc.name == doc.name) {
            doc.name = format!("{base}_{i}");
            i += 1;
        }
        // nietknięta jedyna karta startowa zostaje zastąpiona
        if self.tabs.len() == 1 && self.tabs[0].undo.is_empty() && self.tabs[0].doc == self.tabs[0].committed {
            self.tabs[0] = Tab::new(doc);
            self.active = 0;
        } else {
            self.tabs.push(Tab::new(doc));
            self.active = self.tabs.len() - 1;
        }
    }
    /// Zatwierdza zmianę do historii (wołane, gdy mysz nie jest wciśnięta — przeciąganie = jeden krok).
    pub fn commit(&mut self) {
        let t = &mut self.tabs[self.active];
        if t.doc != t.committed {
            t.undo.push(std::mem::replace(&mut t.committed, t.doc.clone()));
            t.redo.clear();
            // ponytail: limit 200 kroków na kartę, wystarcza dla migawek ~kB
            if t.undo.len() > 200 {
                t.undo.remove(0);
            }
        }
    }
    pub fn undo(&mut self) {
        self.commit();
        let t = &mut self.tabs[self.active];
        if let Some(prev) = t.undo.pop() {
            t.redo.push(std::mem::replace(&mut t.doc, prev.clone()));
            t.committed = prev;
        }
    }
    pub fn redo(&mut self) {
        let t = &mut self.tabs[self.active];
        if let Some(next) = t.redo.pop() {
            t.undo.push(std::mem::replace(&mut t.doc, next.clone()));
            t.committed = next;
        }
    }
}

// ------------------------------------------------------------------ stan widoku (poza historią)

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Screen {
    /// Animacja pierwszego ładowania (potem `Start`).
    Loading,
    Start,
    Editor,
    Mcp,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum RibbonTab {
    Plik,
    Budynek,
    Ulica,
    Render,
    Eksport,
}

/// Element wstążki otwierający inspektor.
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Item {
    Typologia,
    Segmenty,
    Oficyny,
    Uskoki,
    Kondygnacje,
    Dach,
    Lukarny,
    Okna,
    Balkony,
    Parter,
    Gzymsy,
    Material,
    Wt2002,
    Dzialka,
    Profil,
    Gzyms,
    Parcel,
    Scena,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Drag {
    Orbit,
    Pan,
    Zoom,
}

/// Widok ulicy: elewacja (rzut prostokątny z frontu), perspektywa, plan (z góry).
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum SView {
    Elewacja,
    Perspektywa,
    Plan,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Preset {
    Front,
    Iso,
    Top,
}

#[derive(Resource)]
pub struct View {
    pub screen: Screen,
    pub ribbon: RibbonTab,
    pub item: Option<Item>,
    pub parcel: Option<usize>,
    pub night: bool,
    pub shadows: bool,
    pub ground: bool,
    pub building: bool,
    pub drag: Drag,
    pub sview: SView,
    pub cornice_show: bool,
    pub cornice_tol: f64,
    pub cornice_walls: bool,
    pub palette: Option<String>,
    pub mcp_tab: u8,
    /// Obszar widoku 3D w punktach egui (ustawiany przez UI).
    pub viewport: egui::Rect,
    pub reframe: bool,
    pub preset: Option<Preset>,
    pub screenshot: bool,
    pub msg: String,
    pub maximized: bool,
}

impl Default for View {
    fn default() -> Self {
        Self {
            screen: Screen::Editor,
            ribbon: RibbonTab::Budynek,
            item: Some(Item::Kondygnacje),
            parcel: None,
            night: false,
            shadows: true,
            ground: true,
            building: true,
            drag: Drag::Orbit,
            sview: SView::Elewacja,
            cornice_show: true,
            cornice_tol: 1.0,
            cornice_walls: true,
            palette: None,
            mcp_tab: 0,
            viewport: egui::Rect::NOTHING,
            reframe: true,
            preset: None,
            screenshot: false,
            msg: String::new(),
            maximized: false,
        }
    }
}

// ------------------------------------------------------------------ wynik generacji

#[derive(Clone, Default)]
pub struct Parcel {
    pub name: String,
    pub x0: f64,
    pub width: f64,
    pub height_walls: f64,
    pub height_total: f64,
    pub floors: u32,
    pub color: [u8; 3],
    pub tris: usize,
    /// Środek budynku w świecie (x, z frontu).
    pub center: Vec3,
    pub weights: Vec<(String, f64)>,
    pub seed: u32,
    pub variation: f64,
}

#[derive(Resource, Default)]
pub struct Derived {
    pub last: Option<(usize, Doc, bool)>,
    pub bounds: (Vec3, Vec3),
    pub resolved: Map<String, Value>,
    pub weights: Vec<(String, f64)>,
    pub stats: Option<pc::BuildingStats>,
    pub dominant: String,
    pub year: i32,
    pub mix: String,
    pub parcels: Vec<Parcel>,
    pub info: String,
    pub tris: usize,
}

fn rgb(hex: &str) -> [u8; 3] {
    let c = Srgba::hex(hex.trim_start_matches('#')).unwrap_or(Srgba::gray(0.6));
    [c.red, c.green, c.blue].map(|x| (x * 255.0).round() as u8)
}

// ------------------------------------------------------------------ scena Bevy

#[derive(Resource)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub target: Vec3,
}

impl Default for Orbit {
    fn default() -> Self {
        Self { yaw: 34f32.to_radians(), pitch: 17f32.to_radians(), dist: 60.0, target: Vec3::new(0.0, 8.0, -6.0) }
    }
}

#[derive(Component)]
struct Generated;

#[derive(Component)]
struct Sun;

#[derive(Component)]
struct Ground;

#[derive(Component)]
pub struct MainCamera;

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut egui_settings: ResMut<EguiGlobalSettings>) {
    commands.spawn((Camera3d::default(), MainCamera, Transform::from_xyz(40.0, 20.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y)));
    // osobna kamera dla egui: kamera 3D dostaje viewport (obszar pod wstążką)
    egui_settings.auto_create_primary_context = false;
    commands.spawn((
        PrimaryEguiContext,
        Camera2d,
        RenderLayers::none(),
        Camera {
            order: 1,
            output_mode: CameraOutputMode::Write { blend_state: Some(BlendState::ALPHA_BLENDING), clear_color: ClearColorConfig::None },
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
    ));
    commands.spawn((
        Sun,
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        CascadeShadowConfigBuilder { maximum_distance: 600.0, first_cascade_far_bound: 60.0, ..default() }.build(),
        Transform::from_xyz(-0.5, 0.74, 0.46).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.82, 0.9), brightness: 650.0, ..default() });
    commands.spawn((
        Ground,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(1500.0, 1500.0))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb_u8(0xb9, 0xc2, 0xa4), perceptual_roughness: 1.0, ..default() })),
        Transform::from_xyz(0.0, -0.03, 0.0),
    ));
}

fn to_bevy_mesh(part: &pc::MeshPart) -> Mesh {
    let v3 = |a: &[f32]| a.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect::<Vec<_>>();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, v3(&part.positions))
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, v3(&part.normals))
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, part.uvs.chunks_exact(2).map(|p| [p[0], p[1]]).collect::<Vec<_>>())
        .with_inserted_indices(Indices::U32(part.indices.clone()))
}

fn to_material(m: &pc::MaterialSpec, night: bool) -> StandardMaterial {
    let mut c = Srgba::hex(m.color.trim_start_matches('#')).unwrap_or(Srgba::gray(0.6));
    // pręty balustrad bez tekstury alfa: półprzezroczyste jak w rendererze programowym
    let opacity = if m.key == MatKey::Railing { 0.62 } else { m.opacity };
    c.alpha = opacity as f32;
    StandardMaterial {
        base_color: c.into(),
        perceptual_roughness: m.roughness as f32,
        metallic: m.metalness as f32,
        alpha_mode: if opacity < 0.99 { AlphaMode::Blend } else { AlphaMode::Opaque },
        double_sided: m.double_sided,
        // ściany to powierzchnie bez grubości: bez cullingu cienie oficyn i szczytów są pełne
        cull_mode: None,
        emissive: match (m.emissive, night) {
            (Some(e), true) => LinearRgba::from(Srgba::hex(e.trim_start_matches('#')).unwrap_or(Srgba::WHITE)) * 6.0,
            _ => LinearRgba::BLACK,
        },
        ..default()
    }
}

fn spawn_mesh(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, mesh: &MeshData, offset: Vec3, night: bool) -> usize {
    let mut tris = 0;
    for part in &mesh.parts {
        let Some(spec) = part.material.and_then(|k| mesh.material(k)) else { continue };
        tris += part.indices.len() / 3;
        commands.spawn((Generated, Mesh3d(meshes.add(to_bevy_mesh(part))), MeshMaterial3d(materials.add(to_material(spec, night))), Transform::from_translation(offset)));
    }
    tris
}

#[allow(clippy::too_many_arguments)]
fn regenerate(
    mut commands: Commands,
    docs: Res<Docs>,
    mut der: ResMut<Derived>,
    mut view: ResMut<View>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old: Query<Entity, With<Generated>>,
    mut vis: ParamSet<(Query<&mut Visibility, With<Generated>>, Query<&mut Visibility, With<Ground>>)>,
    mut sun: Query<&mut DirectionalLight, With<Sun>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
) {
    let night = view.night;
    for mut s in &mut sun {
        let (ill, sh) = (if night { 60.0 } else { 9000.0 }, view.shadows);
        if s.illuminance != ill || s.shadow_maps_enabled != sh {
            (s.illuminance, s.shadow_maps_enabled) = (ill, sh);
        }
    }
    let amb = if night { 40.0 } else { 650.0 };
    if ambient.brightness != amb {
        ambient.brightness = amb;
        clear.0 = if night { NIGHT_SKY } else { SKY };
    }
    let show = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut vis.p0() {
        v.set_if_neq(show(view.building));
    }
    for mut v in &mut vis.p1() {
        v.set_if_neq(show(view.ground));
    }

    let doc = docs.cur();
    if der.last.as_ref().is_some_and(|(i, d, n)| *i == docs.active && d == doc && *n == night) {
        return;
    }
    let reframe = der.last.as_ref().is_none_or(|(i, d, _)| {
        *i != docs.active || d.mode != doc.mode || d.source != doc.source || d.vehicle != doc.vehicle || d.use_poly != doc.use_poly || (doc.mode != Mode::Building && (d.profile != doc.profile || d.count != doc.count || d.rows != doc.rows))
    });
    der.last = Some((docs.active, doc.clone(), night));
    view.reframe |= reframe;
    for e in &old {
        commands.entity(e).despawn();
    }
    let (b, c, m_) = (&mut commands, &mut *meshes, &mut *materials);
    let m = &mut *m_;
    der.parcels.clear();
    der.stats = None;
    let bounds = if let Some(kind) = doc.mode.scene() {
        let sc = generate_scene(&doc.scene_spec(kind));
        der.tris = sc.items.iter().map(|it| spawn_mesh(b, c, m, &it.mesh, Vec3::ZERO, night)).sum();
        der.info = sc.describe();
        sc.bounds
    } else if doc.mode == Mode::Vehicle {
        let m = tabor_core::generate(&doc.vehicle_spec());
        let mut tris = spawn_mesh(b, c, m_, &m.body, Vec3::ZERO, night);
        for (_, h) in &m.wheels {
            tris += spawn_mesh(b, c, m_, &m.wheel, Vec3::new(h[0] as f32, h[1] as f32, h[2] as f32), night);
        }
        der.tris = tris;
        let st = &m.stats;
        der.info = format!(
            "Pojazd\n• Nadwozie — {}\n• Wymiary — {:.2} × {:.2} × {:.2} m\n• Rozstaw osi — {:.2} m\n• Miejsca — {}",
            st.body_type, st.length, st.width, st.height, st.wheelbase, st.seats
        );
        m.bounds()
    } else if doc.mode == Mode::Street {
        let street = pc::generate_street(&doc.street_spec());
        let mut tris = 0;
        for bl in &street.buildings {
            let o = Vec3::new(bl.offset[0] as f32, bl.offset[1] as f32, bl.offset[2] as f32);
            let t = spawn_mesh(b, c, m, &bl.model.mesh, o, night);
            tris += t;
            let a = data().archetype(&bl.archetype);
            let p = &bl.model.resolved.params;
            der.parcels.push(Parcel {
                name: a.map_or(bl.archetype.clone(), |a| a.short.clone()),
                x0: bl.offset[0] - bl.width / 2.0,
                width: bl.width,
                height_walls: bl.model.stats.height_walls,
                height_total: bl.model.stats.height_total,
                floors: bl.model.stats.floors,
                color: rgb(&p.wall_color),
                tris: t,
                center: Vec3::new(bl.offset[0] as f32, 0.0, (bl.offset[2] + bl.model.mesh.bounds.max[2]) as f32),
                weights: bl.spec.mix.weights.as_ref().map(|w| w.iter().map(|(k, v)| (k.clone(), *v)).collect()).unwrap_or_default(),
                seed: bl.spec.mix.seed.unwrap_or(1.0) as u32,
                variation: bl.spec.mix.variation.unwrap_or(0.0),
            });
        }
        tris += spawn_mesh(b, c, m, &street.context, Vec3::ZERO, night);
        der.tris = tris;
        der.info = pc::describe_street(&street);
        street.bounds
    } else {
        let model = pc::generate_from_spec(&doc.spec());
        der.tris = spawn_mesh(b, c, m, &model.mesh, Vec3::ZERO, night);
        der.resolved = model.resolved.params.to_map();
        der.weights = model.resolved.weights["massing"].iter().map(|(k, v)| (k.clone(), *v)).collect();
        der.dominant = data().archetype(&model.resolved.dominant).map_or(model.resolved.dominant.clone(), |a| a.short.clone());
        der.year = model.resolved.year;
        der.mix = pc::describe_mix(&pc::overall_weights(&model.resolved.weights), 4);
        der.info = String::new();
        der.stats = Some(model.stats);
        model.mesh.bounds
    };
    der.bounds = (Vec3::from_array(bounds.min.map(|x| x as f32)), Vec3::from_array(bounds.max.map(|x| x as f32)));
}

#[allow(clippy::too_many_arguments)]
fn orbit(
    mut orbit: ResMut<Orbit>,
    mut view: ResMut<View>,
    docs: Res<Docs>,
    der: Res<Derived>,
    wants: Res<EguiWantsInput>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    mut cam: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    let mode = docs.cur().mode;
    let ortho = mode == Mode::Street && view.sview != SView::Perspektywa;
    let in_view = window.cursor_position().is_some_and(|p| view.viewport.contains(egui::pos2(p.x, p.y)));
    if view.screen == Screen::Editor && in_view && !wants.wants_any_pointer_input() {
        let d = motion.delta;
        let left = if ortho && view.drag == Drag::Orbit { Drag::Pan } else { view.drag };
        let pan = buttons.pressed(MouseButton::Middle) || (buttons.pressed(MouseButton::Left) && left == Drag::Pan);
        if pan {
            if let Ok((t, _)) = cam.single() {
                let (r, u) = (t.right(), t.up());
                let k = orbit.dist * 0.0015;
                orbit.target += (-*r * d.x + *u * d.y) * k;
            }
        } else if buttons.pressed(MouseButton::Left) && left == Drag::Zoom {
            orbit.dist = (orbit.dist * (1.0 + d.y * 0.005)).max(3.0);
        } else if !ortho && (buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right)) {
            orbit.yaw -= d.x * 0.008;
            orbit.pitch = (orbit.pitch + d.y * 0.008).clamp(-5f32.to_radians(), 85f32.to_radians());
        }
        if scroll.delta.y != 0.0 {
            orbit.dist = (orbit.dist * 0.9f32.powf(scroll.delta.y.signum())).max(3.0);
        }
    }
    if !wants.wants_any_keyboard_input() && keys.just_pressed(KeyCode::KeyF) {
        view.reframe = true;
    }
    if view.reframe && der.last.is_some() && view.viewport.is_positive() {
        view.reframe = false;
        let (min, max) = der.bounds;
        let size = max - min;
        orbit.target = Vec3::new((min.x + max.x) / 2.0, min.y + size.y * 0.42, (min.z + max.z) / 2.0);
        orbit.dist = size.length() * 0.5 / (std::f32::consts::FRAC_PI_4 * 0.5).sin() * 1.05;
        match mode {
            // sceny: ujęcie z góry na środek (skrzyżowanie/rondo w punkcie 0,0), bez dalekich końców ramion
            Mode::Crossroads | Mode::Roundabout => (orbit.target, orbit.dist, orbit.pitch) = (Vec3::new(0.0, 6.0, 0.0), 150.0, 38f32.to_radians()),
            Mode::Estate => (orbit.dist, orbit.pitch) = (size.x.max(size.z) * 0.95, 40f32.to_radians()),
            Mode::Street => {
                // elewacja: front całej pierzei (bez pasa jezdni z przodu), wysokość kadru z szerokości
                let aspect = (view.viewport.width() / view.viewport.height().max(1.0)).max(0.5);
                orbit.dist = ((max.x - min.x - 40.0) / (0.75 * aspect)).max(size.y * 2.2) * 1.75;
                orbit.target = Vec3::new((min.x + max.x) / 2.0, orbit.dist * 0.75 * 0.2, 0.0);
            }
            Mode::Building | Mode::Vehicle => {
                (orbit.yaw, orbit.pitch) = (34f32.to_radians(), 17f32.to_radians());
                orbit.dist *= 0.92;
            }
        }
    }
    match view.preset.take() {
        Some(Preset::Front) => (orbit.yaw, orbit.pitch) = (0.0, 4f32.to_radians()),
        Some(Preset::Iso) => (orbit.yaw, orbit.pitch) = (45f32.to_radians(), 30f32.to_radians()),
        Some(Preset::Top) => (orbit.yaw, orbit.pitch) = (0.0, 85f32.to_radians()),
        None => {}
    }
    let (yaw, pitch) = match (ortho, view.sview) {
        (true, SView::Plan) => (0.0, 89.9f32.to_radians()),
        (true, _) => (0.0, 0.0),
        _ => (orbit.yaw, orbit.pitch),
    };
    let dir = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos());
    for (mut t, mut proj) in &mut cam {
        *t = Transform::from_translation(orbit.target + dir * orbit.dist.max(if ortho { 400.0 } else { 0.0 })).looking_at(orbit.target, Vec3::Y);
        match (&mut *proj, ortho) {
            (Projection::Orthographic(o), true) => o.scaling_mode = bevy::camera::ScalingMode::FixedVertical { viewport_height: orbit.dist * 0.75 },
            (Projection::Perspective(_), false) => {}
            (p, true) => {
                *p = Projection::Orthographic(OrthographicProjection { scaling_mode: bevy::camera::ScalingMode::FixedVertical { viewport_height: orbit.dist * 0.75 }, far: 2000.0, ..OrthographicProjection::default_3d() })
            }
            (p, false) => *p = Projection::Perspective(default()),
        }
    }
}

// ------------------------------------------------------------------ pliki

pub fn out_dir() -> PathBuf {
    std::env::var_os("PIERZEJA_OUT").map(PathBuf::from).unwrap_or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default();
        PathBuf::from(home).join("pierzeja-out")
    })
}

fn unique(dir: &std::path::Path, name: &str, ext: &str) -> PathBuf {
    let mut path = dir.join(format!("{name}.{ext}"));
    let mut i = 2;
    while path.exists() {
        path = dir.join(format!("{name}-{i}.{ext}"));
        i += 1;
    }
    path
}

pub fn export_glb(doc: &Doc) -> Result<PathBuf, String> {
    let dir = out_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let glb = if let Some(kind) = doc.mode.scene() {
        let sc = generate_scene(&doc.scene_spec(kind));
        let nodes: Vec<pc::ExportNode> = sc
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| pc::ExportNode { name: format!("{:02}-{}", i + 1, it.name), mesh: &it.mesh, offset: None, parent: None, rotation: None, extras: it.spec.as_ref().map(|s| json!({ "spec": s, "stats": it.stats })) })
            .collect();
        pc::to_glb(&nodes)
    } else if doc.mode == Mode::Vehicle {
        tabor_core::to_glb(&tabor_core::generate(&doc.vehicle_spec()))
    } else if doc.mode == Mode::Street {
        let street = pc::generate_street(&doc.street_spec());
        let mut nodes: Vec<pc::ExportNode> = street
            .buildings
            .iter()
            .map(|b| pc::ExportNode { name: format!("{:02}-{}", b.index + 1, b.archetype), mesh: &b.model.mesh, offset: Some(b.offset), parent: None, rotation: None, extras: Some(json!({ "spec": b.spec })) })
            .collect();
        nodes.push(pc::ExportNode { name: "ulica".into(), mesh: &street.context, offset: None, parent: None, rotation: None, extras: None });
        pc::to_glb(&nodes)
    } else {
        let spec = doc.spec();
        let m = pc::generate_from_spec(&spec);
        pc::to_glb(&[pc::ExportNode { name: doc.name.clone(), mesh: &m.mesh, offset: None, parent: None, rotation: None, extras: Some(json!({ "spec": spec, "stats": m.stats })) }])
    };
    let path = unique(&dir, &doc.name, "glb");
    std::fs::write(&path, glb).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_coalesces_and_restores() {
        let mut d = Docs { tabs: vec![Tab::new(Doc::building("familok"))], active: 0 };
        d.cur_mut().seed = 2;
        d.cur_mut().seed = 3; // jedna zmiana „w trakcie przeciągania”
        d.commit();
        d.cur_mut().variation = 0.5;
        d.commit();
        assert_eq!(d.tabs[0].undo.len(), 2);
        d.undo();
        assert_eq!((d.cur().seed, d.cur().variation), (3, 0.0));
        d.undo();
        assert_eq!(d.cur().seed, 1);
        d.undo(); // pusty stos: bez zmian
        d.redo();
        d.redo();
        assert_eq!((d.cur().seed, d.cur().variation), (3, 0.5));
        d.cur_mut().seed = 9;
        d.commit();
        assert!(d.tabs[0].redo.is_empty());
    }

    #[test]
    fn open_replaces_pristine_tab_and_dedupes_names() {
        let mut d = Docs { tabs: vec![Tab::new(Doc::building("familok"))], active: 0 };
        d.open(Doc::building("dworek"));
        assert_eq!(d.tabs.len(), 1);
        d.cur_mut().seed = 5;
        d.commit();
        d.open(Doc::building("dworek"));
        assert_eq!((d.tabs.len(), d.active, d.cur().name.as_str()), (2, 1, "dworek_2"));
    }
}

/// Ikona okna i paska zadań. winit pozwala ją ustawić tylko z wątku głównego (stąd NonSendMarker),
/// a okno powstaje dopiero po starcie pętli zdarzeń, więc próbujemy co klatkę aż się uda.
fn window_icon(_main: bevy::ecs::system::NonSendMarker, mut done: Local<bool>) {
    if *done {
        return;
    }
    bevy::winit::WINIT_WINDOWS.with_borrow(|ww| {
        let Some(w) = ww.windows.values().next() else { return };
        let img = image::load_from_memory(include_bytes!("../assets/icon.png")).expect("icon.png").into_rgba8();
        let (iw, ih) = img.dimensions();
        w.set_window_icon(winit::window::Icon::from_rgba(img.into_raw(), iw, ih).ok());
        *done = true;
    });
}
