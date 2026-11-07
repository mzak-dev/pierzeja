//! Pierzeja — natywna aplikacja: Bevy (podgląd 3D z cieniami) + egui (archetyp
//! albo punkt na mapie typologii, 71 parametrów w 5 wektorach, obrys wielokątny,
//! ulica, osiedle, skrzyżowanie i rondo, eksport GLB).
//! Mysz: lewy/prawy — obrót, środkowy — przesunięcie, kółko — przybliżenie, F — kadruj.

use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{CameraOutputMode, Viewport};
use bevy::render::render_resource::BlendState;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext};
use pierzeja_core::footprint::{self as fp, Footprint, P2};
use pierzeja_core::scene::{generate_scene, SceneKind, SceneSpec};
use pierzeja_core::{self as pc, data, BuildingSpec, MatKey, MeshData, ParamKind, StreetSpec};
use serde_json::{json, Map, Value};
use std::path::PathBuf;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Pierzeja — generator budynków".into(), resolution: (1600, 960).into(), ..default() }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.62, 0.72, 0.78)))
        .insert_resource(bevy::light::DirectionalLightShadowMap { size: 4096 })
        .insert_resource({
            // PIERZEJA_MODE=wielokat|ulica|osiedle|skrzyzowanie|rondo — tryb startowy (zrzuty, testy)
            let mut st = State::default();
            if let Ok(m) = std::env::var("PIERZEJA_MODE") {
                st.mode = match m.as_str() {
                    "wielokat" => Mode::Polygon,
                    "ulica" => Mode::Street,
                    "osiedle" => Mode::Estate,
                    "skrzyzowanie" => Mode::Crossroads,
                    "rondo" => Mode::Roundabout,
                    _ => Mode::Building,
                };
            }
            st
        })
        .insert_resource(Orbit::default())
        .add_systems(Startup, setup)
        .add_systems(EguiPrimaryContextPass, ui)
        .add_systems(Update, (regenerate, orbit, auto_screenshot).chain())
        .run();
}

/// `PIERZEJA_SCREENSHOT=plik.png` — zrzut okna po starcie i wyjście (podgląd w CI/testach).
fn auto_screenshot(mut commands: Commands, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>) {
    let Some(path) = std::env::var_os("PIERZEJA_SCREENSHOT") else { return };
    *frame += 1;
    if *frame == 90 {
        commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(PathBuf::from(path)));
    }
    if *frame == 130 {
        exit.write(AppExit::Success);
    }
}

// ------------------------------------------------------------------ stan

#[derive(PartialEq, Clone)]
enum Source {
    Archetype(String),
    Cursor(f64, f64),
}

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Building,
    Polygon,
    Street,
    Estate,
    Crossroads,
    Roundabout,
}

impl Mode {
    const ALL: [(Mode, &'static str); 6] = [
        (Mode::Building, "Budynek"),
        (Mode::Polygon, "Wielokąt"),
        (Mode::Street, "Ulica"),
        (Mode::Estate, "Osiedle"),
        (Mode::Crossroads, "Skrzyżowanie"),
        (Mode::Roundabout, "Rondo"),
    ];
    fn building(self) -> bool {
        matches!(self, Mode::Building | Mode::Polygon)
    }
    fn scene(self) -> Option<SceneKind> {
        match self {
            Mode::Estate => Some(SceneKind::Estate),
            Mode::Crossroads => Some(SceneKind::Crossroads),
            Mode::Roundabout => Some(SceneKind::Roundabout),
            _ => None,
        }
    }
}

/// Rola krawędzi obrysu w edytorze: 0 zwykła, 1 front (ulica), 2 ślepa (styk z sąsiadem).
#[derive(Clone)]
struct Poly {
    pts: Vec<P2>,
    roles: Vec<u8>,
}

impl Poly {
    fn footprint(&self) -> Footprint {
        let by = |r: u8| (0..self.pts.len()).filter(|&i| self.roles[i] == r).collect();
        Footprint { points: self.pts.clone(), front: by(1), blind: by(2) }
    }
    fn preset(name: &str) -> Poly {
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

const PRESETS: [&str; 8] = ["Prostokąt", "Narożnik", "L", "U (podwórze)", "T", "Trapez", "Łuk", "Sześciokąt"];

#[derive(Resource)]
struct State {
    mode: Mode,
    poly: Poly,
    drag: Option<usize>,
    spacing: f64,
    rows: u32,
    arm: f64,
    scene_seed: u32,
    source: Source,
    overrides: Map<String, Value>,
    seed: u32,
    variation: f64,
    night: bool,
    profile: String,
    count: u32,
    street_seed: u32,
    /// Wynik ostatniego mieszania (wartości suwaków) i wagi wektora bryły.
    resolved: Map<String, Value>,
    weights: Vec<(String, f64)>,
    info: String,
    dirty: bool,
    reframe: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            mode: Mode::Building,
            poly: Poly::preset("Narożnik"),
            drag: None,
            spacing: 1.5,
            rows: 3,
            arm: 90.0,
            scene_seed: 11,
            source: Source::Archetype(pc::DEFAULT_ARCHETYPE.into()),
            overrides: Map::new(),
            seed: 1,
            variation: 0.0,
            night: false,
            profile: data().city_profiles[0].id.clone(),
            count: 9,
            street_seed: 7,
            resolved: Map::new(),
            weights: Vec::new(),
            info: String::new(),
            dirty: true,
            reframe: true,
        }
    }
}

impl State {
    fn spec(&self) -> BuildingSpec {
        let mut s = BuildingSpec { seed: Some(self.seed as f64), ..Default::default() };
        match &self.source {
            Source::Archetype(a) => s.archetype = Some(a.clone()),
            Source::Cursor(year, scale) => s.cursor = Some(pc::MapPoint { year: *year, scale: *scale }),
        }
        if self.variation > 0.0 {
            s.variation = Some(self.variation);
        }
        if !self.overrides.is_empty() {
            s.overrides = Some(self.overrides.clone());
        }
        if self.mode == Mode::Polygon {
            s.footprint = Some(self.poly.footprint());
        }
        s
    }
    fn street_spec(&self) -> StreetSpec {
        StreetSpec { profile: Some(self.profile.clone()), count: Some(self.count as f64), seed: Some(self.street_seed as f64), ..Default::default() }
    }
    fn scene_spec(&self, kind: SceneKind) -> SceneSpec {
        SceneSpec {
            kind,
            seed: Some(self.scene_seed as f64),
            spacing: Some(self.spacing),
            rows: Some(self.rows),
            profile: Some(self.profile.clone()),
            arm: Some(self.arm),
            variation: None,
        }
    }
}

#[derive(Resource)]
struct Orbit {
    yaw: f32,
    pitch: f32,
    dist: f32,
    target: Vec3,
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
struct MainCamera;

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut egui_settings: ResMut<EguiGlobalSettings>) {
    commands.spawn((Camera3d::default(), MainCamera, Transform::from_xyz(40.0, 20.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y)));
    // osobna kamera dla egui: kamera 3D dostaje viewport na prawo od panelu
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
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.75, 0.82, 0.9), brightness: 600.0, ..default() });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(1500.0, 1500.0))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb_u8(0x8f, 0x9a, 0x80), perceptual_roughness: 1.0, ..default() })),
        Transform::from_xyz(0.0, -0.03, 0.0),
    ));
}

// ------------------------------------------------------------------ budowa sceny

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

fn spawn_mesh(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, mesh: &MeshData, offset: Vec3, night: bool) {
    for part in &mesh.parts {
        let Some(spec) = part.material.and_then(|k| mesh.material(k)) else { continue };
        commands.spawn((Generated, Mesh3d(meshes.add(to_bevy_mesh(part))), MeshMaterial3d(materials.add(to_material(spec, night))), Transform::from_translation(offset)));
    }
}

#[allow(clippy::too_many_arguments)]
fn regenerate(
    mut commands: Commands,
    mut st: ResMut<State>,
    mut orbit: ResMut<Orbit>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old: Query<Entity, With<Generated>>,
    mut sun: Query<&mut DirectionalLight, With<Sun>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
) {
    if !st.dirty {
        return;
    }
    st.dirty = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    for mut s in &mut sun {
        s.illuminance = if st.night { 60.0 } else { 9000.0 };
    }
    ambient.brightness = if st.night { 40.0 } else { 600.0 };
    clear.0 = if st.night { Color::srgb(0.04, 0.06, 0.11) } else { Color::srgb(0.62, 0.72, 0.78) };

    let night = st.night;
    let bounds = if let Some(kind) = st.mode.scene() {
        let sc = generate_scene(&st.scene_spec(kind));
        for it in &sc.items {
            spawn_mesh(&mut commands, &mut meshes, &mut materials, &it.mesh, Vec3::ZERO, night);
        }
        st.info = sc.describe();
        sc.bounds
    } else if st.mode == Mode::Street {
        let street = pc::generate_street(&st.street_spec());
        for b in &street.buildings {
            let o = Vec3::new(b.offset[0] as f32, b.offset[1] as f32, b.offset[2] as f32);
            spawn_mesh(&mut commands, &mut meshes, &mut materials, &b.model.mesh, o, night);
        }
        spawn_mesh(&mut commands, &mut meshes, &mut materials, &street.context, Vec3::ZERO, night);
        st.info = format!("{} — {} budynków\n{}", street.profile.map_or("Ulica", |p| p.name.as_str()), street.buildings.len(), pc::describe_street(&street));
        street.bounds
    } else {
        let spec = st.spec();
        let m = pc::generate_from_spec(&spec);
        spawn_mesh(&mut commands, &mut meshes, &mut materials, &m.mesh, Vec3::ZERO, night);
        st.resolved = m.resolved.params.to_map();
        st.weights = m.resolved.weights["massing"].iter().map(|(k, v)| (k.clone(), *v)).collect();
        let s = &m.stats;
        let dom = data().archetype(&m.resolved.dominant).map_or(m.resolved.dominant.as_str(), |a| a.name.as_str());
        let mut info = format!(
            "{dom} · datowanie ≈ {}\n{}\nKondygnacje {} · wysokość {:.1} m (ściany {:.1} m) · grupa {}\nFront × głęb. {:.1} × {:.1} m · zabudowa {:.0} m² · PC {:.0} m²\nKlatki {} · mieszkania ≈ {} · okna {} · balkony {} · trójkąty {}",
            m.resolved.year,
            pc::describe_mix(&pc::overall_weights(&m.resolved.weights), 4),
            s.floors, s.height_total, s.height_walls, s.height_class, s.frontage, s.depth, s.footprint, s.gfa, s.staircases, s.flats_estimate, s.windows, s.balconies, s.triangles
        );
        for n in &s.notes {
            info.push_str(&format!("\n• {n}"));
        }
        st.info = info;
        m.mesh.bounds
    };
    if st.reframe {
        st.reframe = false;
        let (min, max) = (Vec3::from_array(bounds.min.map(|x| x as f32)), Vec3::from_array(bounds.max.map(|x| x as f32)));
        let size = max - min;
        orbit.target = Vec3::new((min.x + max.x) / 2.0, min.y + size.y * 0.42, (min.z + max.z) / 2.0);
        orbit.dist = size.length() * 0.5 / (std::f32::consts::FRAC_PI_4 * 0.5).sin() * 1.05;
        // sceny: ujęcie z góry na środek (skrzyżowanie/rondo w punkcie 0,0), bez dalekich końców ramion
        match st.mode {
            Mode::Crossroads | Mode::Roundabout => (orbit.target, orbit.dist, orbit.pitch) = (Vec3::new(0.0, 6.0, 0.0), 150.0, 38f32.to_radians()),
            Mode::Estate => (orbit.dist, orbit.pitch) = (size.x.max(size.z) * 0.95, 40f32.to_radians()),
            _ => {}
        }
    }
}

fn orbit(
    mut orbit: ResMut<Orbit>,
    mut st: ResMut<State>,
    wants: Res<EguiWantsInput>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
) {
    if !wants.wants_any_pointer_input() {
        let d = motion.delta;
        if buttons.pressed(MouseButton::Middle) {
            if let Ok(t) = cam.single() {
                let (r, u) = (t.right(), t.up());
                let k = orbit.dist * 0.0015;
                orbit.target += (-*r * d.x + *u * d.y) * k;
            }
        } else if buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right) {
            orbit.yaw -= d.x * 0.008;
            orbit.pitch = (orbit.pitch + d.y * 0.008).clamp(-5f32.to_radians(), 85f32.to_radians());
        }
        if scroll.delta.y != 0.0 {
            orbit.dist = (orbit.dist * 0.9f32.powf(scroll.delta.y.signum())).max(3.0);
        }
    }
    if !wants.wants_any_keyboard_input() && keys.just_pressed(KeyCode::KeyF) {
        st.reframe = true;
        st.dirty = true;
    }
    let dir = Vec3::new(orbit.yaw.sin() * orbit.pitch.cos(), orbit.pitch.sin(), orbit.yaw.cos() * orbit.pitch.cos());
    for mut t in &mut cam {
        *t = Transform::from_translation(orbit.target + dir * orbit.dist).looking_at(orbit.target, Vec3::Y);
    }
}

// ------------------------------------------------------------------ UI

fn out_dir() -> PathBuf {
    std::env::var_os("PIERZEJA_OUT").map(PathBuf::from).unwrap_or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default();
        PathBuf::from(home).join("pierzeja-out")
    })
}

fn export_glb(st: &State) -> Result<PathBuf, String> {
    let dir = out_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let (name, glb) = if let Some(kind) = st.mode.scene() {
        let sc = generate_scene(&st.scene_spec(kind));
        let nodes: Vec<pc::ExportNode> = sc
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| pc::ExportNode { name: format!("{:02}-{}", i + 1, it.name), mesh: &it.mesh, offset: None, extras: it.spec.as_ref().map(|s| json!({ "spec": s, "stats": it.stats })) })
            .collect();
        (format!("{:?}-{}", kind, st.scene_seed).to_lowercase(), pc::to_glb(&nodes))
    } else if st.mode == Mode::Street {
        let street = pc::generate_street(&st.street_spec());
        let mut nodes: Vec<pc::ExportNode> = street
            .buildings
            .iter()
            .map(|b| pc::ExportNode { name: format!("{:02}-{}", b.index + 1, b.archetype), mesh: &b.model.mesh, offset: Some(b.offset), extras: Some(json!({ "spec": b.spec })) })
            .collect();
        nodes.push(pc::ExportNode { name: "ulica".into(), mesh: &street.context, offset: None, extras: None });
        (format!("ulica-{}-{}", st.profile, st.street_seed), pc::to_glb(&nodes))
    } else {
        let spec = st.spec();
        let m = pc::generate_from_spec(&spec);
        let name = format!("{}-{}", m.resolved.dominant, m.resolved.seed);
        let glb = pc::to_glb(&[pc::ExportNode { name: name.clone(), mesh: &m.mesh, offset: None, extras: Some(json!({ "spec": spec, "stats": m.stats })) }]);
        (name, glb)
    };
    let mut path = dir.join(format!("{name}.glb"));
    let mut i = 2;
    while path.exists() {
        path = dir.join(format!("{name}-{i}.glb"));
        i += 1;
    }
    std::fs::write(&path, glb).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Mapa typologii rok × skala: punkty archetypów, kursor IDW, linie wpływu.
fn typology_map(ui: &mut egui::Ui, st: &mut State) {
    let d = data();
    let [y0, y1] = d.map_years;
    let (resp, painter) = ui.allocate_painter(egui::vec2(ui.available_width(), 240.0), egui::Sense::click_and_drag());
    let r = resp.rect.shrink(10.0);
    let to_px = |year: f64, scale: f64| egui::pos2(r.left() + ((year - y0) / (y1 - y0)) as f32 * r.width(), r.top() + (1.0 - scale as f32) * r.height());
    painter.rect_filled(resp.rect, 4.0, egui::Color32::from_rgb(28, 31, 36));
    let font = egui::FontId::proportional(10.5);
    for (i, e) in d.eras.iter().enumerate() {
        let (a, b) = (to_px(e.from, 0.0).x, to_px(e.to, 0.0).x);
        painter.rect_filled(egui::Rect::from_x_y_ranges(a..=b, resp.rect.y_range()), 0.0, egui::Color32::from_white_alpha(if i % 2 == 0 { 6 } else { 14 }));
        painter.text(egui::pos2(a + 3.0, resp.rect.bottom() - 3.0), egui::Align2::LEFT_BOTTOM, &e.label, font.clone(), egui::Color32::from_white_alpha(90));
    }
    let cursor = match &st.source {
        Source::Cursor(y, s) => Some(to_px(*y, *s)),
        Source::Archetype(_) => None,
    };
    if let Some(cp) = cursor {
        for (id, w) in &st.weights {
            if let Some(a) = d.archetype(id) {
                painter.line_segment([cp, to_px(a.map.year, a.map.scale)], egui::Stroke::new(1.0 + 5.0 * *w as f32, egui::Color32::from_rgba_unmultiplied(242, 190, 76, 190)));
            }
        }
    }
    for a in &d.archetypes {
        let p = to_px(a.map.year, a.map.scale);
        let sel = st.source == Source::Archetype(a.id.clone());
        painter.circle_filled(p, if sel { 5.0 } else { 3.5 }, if sel { egui::Color32::from_rgb(242, 190, 76) } else { egui::Color32::from_rgb(184, 204, 230) });
        let (off, align) = if p.x > r.right() - 70.0 { (-6.0, egui::Align2::RIGHT_CENTER) } else { (6.0, egui::Align2::LEFT_CENTER) };
        painter.text(p + egui::vec2(off, 0.0), align, &a.short, egui::FontId::proportional(11.0), egui::Color32::from_white_alpha(if sel { 230 } else { 165 }));
    }
    if let Some(cp) = cursor {
        painter.circle_stroke(cp, 7.0, egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 216, 100)));
    }
    if let Some(mut p) = resp.interact_pointer_pos().filter(|_| resp.clicked() || resp.dragged()) {
        // magnes: blisko punktu archetypu = dokładnie ten archetyp (100%)
        if let Some(a) = d.archetypes.iter().find(|a| to_px(a.map.year, a.map.scale).distance(p) < 11.0) {
            p = to_px(a.map.year, a.map.scale);
        }
        let tx = ((p.x - r.left()) / r.width()).clamp(0.0, 1.0) as f64;
        let ty = ((p.y - r.top()) / r.height()).clamp(0.0, 1.0) as f64;
        st.source = Source::Cursor((y0 + tx * (y1 - y0)).round(), ((1.0 - ty) * 100.0).round() / 100.0);
        st.dirty = true;
    }
}

fn scene_controls(ui: &mut egui::Ui, st: &mut State, kind: SceneKind) {
    let d = data();
    let mut ch = false;
    match kind {
        SceneKind::Estate => {
            ui.label("Osiedle z wielkiej płyty: rzędy bloków, drogi dojazdowe z parkingiem, chodniki przy wejściach, przejście piesze, plac zabaw, pawilon.");
            ch |= ui.add(egui::Slider::new(&mut st.spacing, 0.8..=3.0).text("odstęp rzędów (× H)")).on_hover_text("WT 2002 § 13: co najmniej wysokość budynku przesłaniającego; osiedla PRL zwykle 1,5–2 H").changed();
            ch |= ui.add(egui::Slider::new(&mut st.rows, 1..=6).text("rzędy bloków")).changed();
        }
        _ => {
            ui.label(if kind == SceneKind::Crossroads { "Skrzyżowanie czterowlotowe: narożniki na obrysach wielokątnych (ścięty, z wieżyczką, obły, ostry) i pierzeje na ramionach." } else { "Rondo jednopasowe (Ø 35 m): narożniki z frontem po łuku i pierzeje na ramionach." });
            let cur = d.city(&st.profile).map_or(st.profile.clone(), |c| c.name.clone());
            egui::ComboBox::from_label("Profil pierzei").selected_text(cur).show_ui(ui, |ui| {
                for c in &d.city_profiles {
                    if ui.selectable_label(st.profile == c.id, &c.name).clicked() {
                        st.profile = c.id.clone();
                        ch = true;
                    }
                }
            });
            ch |= ui.add(egui::Slider::new(&mut st.arm, 50.0..=200.0).text("długość ramion [m]")).changed();
        }
    }
    ch |= ui.add(egui::DragValue::new(&mut st.scene_seed).prefix("ziarno ")).changed();
    if ch {
        st.dirty = true;
    }
}

/// Edytor obrysu 2D (widok z góry, ulica na dole = +z). Lewy klik przy krawędzi — nowy
/// punkt, przeciąganie — przesunięcie (siatka 0,5 m), prawy klik — usunięcie punktu,
/// klik w kwadrat na środku krawędzi — rola: zwykła → front → ślepa.
fn polygon_editor(ui: &mut egui::Ui, st: &mut State) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Szablon:");
        for name in PRESETS {
            if ui.small_button(name).clicked() {
                st.poly = Poly::preset(name);
                st.dirty = true;
                st.reframe = true;
            }
        }
    });
    let (resp, painter) = ui.allocate_painter(egui::vec2(ui.available_width(), 330.0), egui::Sense::click_and_drag());
    let r = resp.rect;
    painter.rect_filled(r, 4.0, egui::Color32::from_rgb(28, 31, 36));
    let pts = st.poly.pts.clone();
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in &pts {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    // stała skala, dopóki punkt jest przeciągany (kadr nie „ucieka”)
    let id = resp.id.with("view");
    let view: (f64, [f64; 2]) = if st.drag.is_some() {
        ui.ctx().data(|d| d.get_temp(id)).unwrap_or((5.0, [0.0, 0.0]))
    } else {
        let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(10.0) + 16.0;
        let v = ((r.width().min(r.height()) as f64) / span, [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0]);
        ui.ctx().data_mut(|d| d.insert_temp(id, v));
        v
    };
    let (sc, c) = view;
    let to_px = |p: P2| egui::pos2(r.center().x + ((p[0] - c[0]) * sc) as f32, r.center().y + ((p[1] - c[1]) * sc) as f32);
    let from_px = |q: egui::Pos2| [c[0] + (q.x - r.center().x) as f64 / sc, c[1] + (q.y - r.center().y) as f64 / sc];
    // siatka co 5 m
    let grid = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(14));
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
    painter.text(r.center_bottom() - egui::vec2(0.0, 4.0), egui::Align2::CENTER_BOTTOM, "ulica (+z) ↓   siatka 5 m", egui::FontId::proportional(10.5), egui::Color32::from_white_alpha(110));
    // wypełnienie (triangulacja) i krawędzie wg ról
    let valid = fp::normalize(&st.poly.footprint()).is_some();
    let mut mesh = egui::epaint::Mesh::default();
    let fill = if valid { egui::Color32::from_rgba_unmultiplied(120, 150, 190, 60) } else { egui::Color32::from_rgba_unmultiplied(220, 80, 80, 60) };
    for p in &pts {
        mesh.colored_vertex(to_px(*p), fill);
    }
    for t in fp::triangulate(&pts) {
        mesh.add_triangle(t[0] as u32, t[1] as u32, t[2] as u32);
    }
    painter.add(egui::Shape::mesh(mesh));
    let n = pts.len();
    let role_col = |r: u8| match r {
        1 => egui::Color32::from_rgb(242, 190, 76),
        2 => egui::Color32::from_rgb(130, 130, 130),
        _ => egui::Color32::from_rgb(170, 200, 235),
    };
    for i in 0..n {
        let (a, b) = (to_px(pts[i]), to_px(pts[(i + 1) % n]));
        painter.line_segment([a, b], egui::Stroke::new(if st.poly.roles[i] == 1 { 3.5 } else { 2.0 }, role_col(st.poly.roles[i])));
        let m = a + (b - a) * 0.5;
        painter.rect_filled(egui::Rect::from_center_size(m, egui::vec2(8.0, 8.0)), 1.0, role_col(st.poly.roles[i]));
        let len = fp::edge_len(&pts, i);
        painter.text(m + egui::vec2(6.0, -6.0), egui::Align2::LEFT_BOTTOM, format!("{len:.1}"), egui::FontId::proportional(10.0), egui::Color32::from_white_alpha(140));
    }
    for p in &pts {
        painter.circle_filled(to_px(*p), 4.5, egui::Color32::WHITE);
    }
    // interakcja
    let near = |q: egui::Pos2, tol: f32| (0..n).find(|&i| to_px(pts[i]).distance(q) < tol);
    let mid = |q: egui::Pos2| (0..n).find(|&i| (to_px(pts[i]) + (to_px(pts[(i + 1) % n]) - to_px(pts[i])) * 0.5).distance(q) < 7.0);
    let snap = |p: P2| [(p[0] * 2.0).round() / 2.0, (p[1] * 2.0).round() / 2.0];
    if resp.drag_started()
        && let Some(q) = resp.interact_pointer_pos()
    {
        st.drag = near(q, 10.0);
    }
    if let (Some(i), Some(q)) = (st.drag, resp.interact_pointer_pos())
        && resp.dragged()
    {
        let np = snap(from_px(q));
        if np != st.poly.pts[i] {
            st.poly.pts[i] = np;
            st.dirty = true;
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        st.reframe = true;
    }
    if resp.clicked()
        && let Some(q) = resp.interact_pointer_pos()
    {
        if let Some(i) = mid(q) {
            st.poly.roles[i] = (st.poly.roles[i] + 1) % 3;
            st.dirty = true;
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
                st.poly.pts.insert(i + 1, snap(from_px(q)));
                let role = st.poly.roles[i];
                st.poly.roles.insert(i + 1, role);
                st.dirty = true;
            }
        }
    }
    if resp.secondary_clicked()
        && let Some(q) = resp.interact_pointer_pos()
        && let Some(i) = near(q, 10.0)
        && n > 3
    {
        st.poly.pts.remove(i);
        st.poly.roles.remove(i);
        st.dirty = true;
    }
    let area = fp::area(&pts);
    ui.label(if valid {
        egui::RichText::new(format!("{n} punktów · pole {area:.0} m² · lewy klik przy krawędzi: punkt · prawy: usuń · kwadrat: rola (żółta = front, szara = ślepa)")).size(11.0)
    } else {
        egui::RichText::new("Obrys nieprawidłowy (samoprzecięcie albo zerowe pole) — generator użyje prostokąta.").color(egui::Color32::from_rgb(240, 110, 110))
    });
}

fn param_row(ui: &mut egui::Ui, st: &mut State, def: &pc::ParamDef) {
    let key = def.key.as_str();
    let cur = st.resolved.get(key).cloned().unwrap_or(Value::Null);
    let mut new: Option<Value> = None;
    ui.horizontal(|ui| {
        match def.kind {
            ParamKind::Number | ParamKind::Int => {
                let mut v = cur.as_f64().unwrap_or(0.0);
                let mut s = egui::Slider::new(&mut v, def.min.unwrap_or(0.0)..=def.max.unwrap_or(1.0)).text(&def.label);
                if def.kind == ParamKind::Int {
                    s = s.integer();
                } else if let Some(step) = def.step {
                    s = s.step_by(step);
                }
                if let Some(u) = &def.unit {
                    s = s.suffix(format!(" {u}"));
                }
                if ui.add(s).on_hover_text(def.hint.as_deref().unwrap_or("")).changed() {
                    new = Some(pc::params::num(v));
                }
            }
            ParamKind::Enum => {
                let opts = def.options.as_deref().unwrap_or(&[]);
                let cur_s = cur.as_str().unwrap_or("");
                let label = opts.iter().find(|o| o.value == cur_s).map_or(cur_s, |o| o.label.as_str());
                egui::ComboBox::from_label(&def.label).selected_text(label).show_ui(ui, |ui| {
                    for o in opts {
                        if ui.selectable_label(o.value == cur_s, &o.label).clicked() {
                            new = Some(Value::String(o.value.clone()));
                        }
                    }
                });
            }
            ParamKind::Color => {
                let c = Srgba::hex(cur.as_str().unwrap_or("#808080").trim_start_matches('#')).unwrap_or(Srgba::gray(0.5));
                let mut rgb = [c.red, c.green, c.blue].map(|x| (x * 255.0).round() as u8);
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    new = Some(Value::String(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])));
                }
                ui.label(&def.label);
            }
        }
        if st.overrides.contains_key(key) && ui.small_button("↺").on_hover_text("Wróć do wartości z mieszanki").clicked() {
            st.overrides.remove(key);
            st.dirty = true;
        }
    });
    if let Some(v) = new {
        st.overrides.insert(key.to_string(), v);
        st.dirty = true;
    }
}

fn ui(mut contexts: EguiContexts, mut st: ResMut<State>, mut camera: Query<&mut Camera, With<MainCamera>>, window: Single<&Window, With<PrimaryWindow>>) -> Result {
    let ctx = contexts.ctx_mut()?;
    let d = data();
    let mut root = egui::Ui::new(ctx.clone(), "root".into(), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.viewport_rect()));
    let panel = egui::Panel::left("pierzeja").resizable(true).default_size(430.0).show(&mut root, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Pierzeja");
            ui.horizontal_wrapped(|ui| {
                for (m, label) in Mode::ALL {
                    if ui.selectable_label(st.mode == m, label).clicked() && st.mode != m {
                        st.mode = m;
                        st.dirty = true;
                        st.reframe = true;
                    }
                }
                if ui.checkbox(&mut st.night, "Noc").changed() {
                    st.dirty = true;
                }
            });
            ui.separator();
            if let Some(kind) = st.mode.scene() {
                scene_controls(ui, &mut st, kind);
            } else if st.mode == Mode::Street {
                let cur = d.city(&st.profile).map_or(st.profile.clone(), |c| c.name.clone());
                egui::ComboBox::from_label("Profil").selected_text(cur).show_ui(ui, |ui| {
                    for c in &d.city_profiles {
                        if ui.selectable_label(st.profile == c.id, &c.name).on_hover_text(&c.description).clicked() {
                            st.profile = c.id.clone();
                            st.dirty = true;
                            st.reframe = true;
                        }
                    }
                });
                let (mut count, mut seed) = (st.count, st.street_seed);
                let ch = ui.add(egui::Slider::new(&mut count, 1..=40).text("Działki")).changed() | ui.add(egui::DragValue::new(&mut seed).prefix("ziarno ")).changed();
                if ch {
                    (st.count, st.street_seed) = (count, seed);
                    st.dirty = true;
                }
            } else {
                let cur = match &st.source {
                    Source::Archetype(a) => d.archetype(a).map_or(a.clone(), |x| x.name.clone()),
                    Source::Cursor(y, s) => format!("◎ Mapa: {y:.0} · {s:.2}"),
                };
                egui::ComboBox::from_label("Typ").selected_text(cur).width(300.0).show_ui(ui, |ui| {
                    for a in &d.archetypes {
                        let sel = st.source == Source::Archetype(a.id.clone());
                        if ui.selectable_label(sel, format!("{} ({}–{})", a.name, a.years[0], a.years[1])).on_hover_text(&a.description).clicked() {
                            st.source = Source::Archetype(a.id.clone());
                            st.dirty = true;
                            st.reframe = true;
                        }
                    }
                });
                if st.mode == Mode::Polygon {
                    polygon_editor(ui, &mut st);
                } else {
                    typology_map(ui, &mut st);
                }
                ui.horizontal(|ui| {
                    let (mut seed, mut var) = (st.seed, st.variation);
                    let ch = ui.add(egui::DragValue::new(&mut seed).prefix("ziarno ")).changed() | ui.add(egui::Slider::new(&mut var, 0.0..=1.0).text("wariacja")).changed();
                    if ch {
                        (st.seed, st.variation) = (seed, var);
                        st.dirty = true;
                    }
                });
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button("Eksport GLB").clicked() {
                    st.info = match export_glb(&st) {
                        Ok(p) => format!("Zapisano: {}\n\n{}", p.display(), st.info),
                        Err(e) => format!("Błąd zapisu: {e}"),
                    };
                }
                if st.mode.building() && ui.button("Kopiuj spec").clicked() {
                    ui.ctx().copy_text(serde_json::to_string_pretty(&st.spec()).unwrap_or_default());
                }
                if ui.button("Kadruj (F)").clicked() {
                    st.reframe = true;
                    st.dirty = true;
                }
                if !st.overrides.is_empty() && ui.button(format!("Wyczyść nadpisania ({})", st.overrides.len())).clicked() {
                    st.overrides.clear();
                    st.dirty = true;
                }
            });
            ui.separator();
            ui.label(egui::RichText::new(&st.info).monospace().size(11.5));
            if st.mode.building() {
                ui.separator();
                for g in &d.groups {
                    egui::CollapsingHeader::new(format!("{} — {}", g.label, g.description)).default_open(g.id == "storeys").show(ui, |ui| {
                        for def in d.params.iter().filter(|p| p.group == g.id) {
                            param_row(ui, &mut st, def);
                        }
                    });
                }
            }
        });
    });
    let left = (panel.response.rect.width() * window.scale_factor()) as u32;
    let (w, h) = (window.physical_width(), window.physical_height());
    if let Ok(mut cam) = camera.single_mut()
        && w > left + 16
    {
        cam.viewport = Some(Viewport { physical_position: UVec2::new(left, 0), physical_size: UVec2::new(w - left, h), ..default() });
    }
    Ok(())
}
