//! Pierzeja — natywna aplikacja: Bevy (podgląd 3D z cieniami) + egui (archetyp
//! albo punkt na mapie typologii, 71 parametrów w 5 wektorach, tryb ulicy, eksport).
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
        .insert_resource(State::default())
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

#[derive(Resource)]
struct State {
    street: bool,
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
            street: false,
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
        s
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
    let bounds = if st.street {
        let ss = StreetSpec { profile: Some(st.profile.clone()), count: Some(st.count as f64), seed: Some(st.street_seed as f64), ..Default::default() };
        let street = pc::generate_street(&ss);
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
    let (name, glb) = if st.street {
        let ss = StreetSpec { profile: Some(st.profile.clone()), count: Some(st.count as f64), seed: Some(st.street_seed as f64), ..Default::default() };
        let street = pc::generate_street(&ss);
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
            ui.horizontal(|ui| {
                if ui.selectable_label(!st.street, "Budynek").clicked() && st.street {
                    st.street = false;
                    st.dirty = true;
                    st.reframe = true;
                }
                if ui.selectable_label(st.street, "Ulica").clicked() && !st.street {
                    st.street = true;
                    st.dirty = true;
                    st.reframe = true;
                }
                if ui.checkbox(&mut st.night, "Noc").changed() {
                    st.dirty = true;
                }
            });
            ui.separator();
            if st.street {
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
                typology_map(ui, &mut st);
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
                if !st.street && ui.button("Kopiuj spec").clicked() {
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
            if !st.street {
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
