//! GDExtension Pierzei: generator budynków bezpośrednio w Godot.
//!
//! - `Pierzeja` — statyczne API (dane, mieszanie, budowa `ArrayMesh`, eksport GLB)
//!   dla GDScript i zakładki edytora (`addons/pierzeja`).
//! - `PierzejaBuilding` — węzeł `@tool`: zmiana `spec` w inspektorze przebudowuje budynek.
//! - `Tabor` / `TaborVehicle` — to samo dla pojazdów (Tabor): API statyczne i węzeł `@tool`.

use godot::classes::base_material_3d::{CullMode, Feature, Transparency};
use godot::classes::mesh::{ArrayType, PrimitiveType};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::{ArrayMesh, IMeshInstance3D, INode3D, MeshInstance3D, Node3D, ProjectSettings, StandardMaterial3D};
use godot::prelude::*;
use pierzeja_core::scene::{generate_scene, SceneSpec};
use pierzeja_core::{self as pc, BuildingSpec, MatKey, MeshData};
use serde_json::{json, Value};
use tabor_core::{self as tc, TaborSpec};

struct PierzejaExtension;

#[gdextension]
unsafe impl ExtensionLibrary for PierzejaExtension {}

fn gs(v: &impl serde::Serialize) -> GString {
    GString::from(serde_json::to_string(v).unwrap_or_default().as_str())
}

fn parse_spec(spec_json: &GString) -> Result<BuildingSpec, String> {
    let s = spec_json.to_string();
    let spec: BuildingSpec = if s.trim().is_empty() { BuildingSpec::default() } else { serde_json::from_str(&s).map_err(|e| format!("Błędny JSON spec: {e}"))? };
    let bad = pc::unknown_archetypes(&spec);
    if !bad.is_empty() {
        return Err(format!("Nieznane archetypy: {}", bad.join(", ")));
    }
    Ok(spec)
}

fn material(m: &pc::MaterialSpec, night: bool) -> Gd<StandardMaterial3D> {
    let mut mat = StandardMaterial3D::new_gd();
    let mut c = Color::from_html(m.color.as_str()).unwrap_or(Color::GRAY);
    // pręty balustrad bez tekstury alfa: półprzezroczyste jak w rendererze programowym
    let opacity = if m.key == MatKey::Railing { 0.62 } else { m.opacity };
    if opacity < 0.99 {
        c.a = opacity as f32;
        mat.set_transparency(Transparency::ALPHA);
    }
    mat.set_albedo(c);
    mat.set_roughness(m.roughness as f32);
    mat.set_metallic(m.metalness as f32);
    if m.double_sided {
        mat.set_cull_mode(CullMode::DISABLED);
    }
    if let (Some(e), true) = (m.emissive, night) {
        mat.set_feature(Feature::EMISSION, true);
        mat.set_emission(Color::from_html(e).unwrap_or(Color::WHITE));
        mat.set_emission_energy_multiplier(2.0);
    }
    mat.set_name(m.name);
    mat
}

/// `MeshData` → `ArrayMesh` (powierzchnia na materiał). Godot ma przednie ściany
/// zgodnie z ruchem wskazówek zegara, glTF/rdzeń — przeciwnie, więc odwracamy indeksy.
pub fn to_array_mesh(mesh: &MeshData, night: bool) -> Gd<ArrayMesh> {
    let mut am = ArrayMesh::new_gd();
    for part in &mesh.parts {
        let Some(key) = part.material else { continue };
        let v3 = |a: &[f32]| a.chunks_exact(3).map(|p| Vector3::new(p[0], p[1], p[2])).collect::<PackedVector3Array>();
        let uvs: PackedVector2Array = part.uvs.chunks_exact(2).map(|p| Vector2::new(p[0], p[1])).collect();
        let idx: PackedInt32Array = part.indices.chunks_exact(3).flat_map(|t| [t[0] as i32, t[2] as i32, t[1] as i32]).collect();
        let mut arrays = VarArray::new();
        arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
        arrays.set(ArrayType::VERTEX.ord() as usize, &v3(&part.positions).to_variant());
        arrays.set(ArrayType::NORMAL.ord() as usize, &v3(&part.normals).to_variant());
        arrays.set(ArrayType::TEX_UV.ord() as usize, &uvs.to_variant());
        arrays.set(ArrayType::INDEX.ord() as usize, &idx.to_variant());
        am.add_surface_from_arrays(PrimitiveType::TRIANGLES, &arrays);
        let s = am.get_surface_count() - 1;
        if let Some(spec) = mesh.material(key) {
            am.surface_set_material(s, &material(spec, night));
        }
        am.surface_set_name(s, key.as_str());
    }
    am
}

// ------------------------------------------------------------------ API statyczne

/// Statyczne API generatora dla GDScript: `Pierzeja.build(spec_json, night)` itd.
#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct Pierzeja;

#[godot_api]
impl Pierzeja {
    /// Lista archetypów (JSON): id, nazwa, krótka nazwa, lata, położenie na mapie, opis.
    #[func]
    fn archetypes_json() -> GString {
        let rows: Vec<Value> = pc::data().archetypes.iter().map(|a| json!({ "id": a.id, "name": a.name, "short": a.short, "years": a.years, "map": a.map, "description": a.description })).collect();
        gs(&rows)
    }

    /// Definicje 71 parametrów (JSON) — do budowy UI.
    #[func]
    fn params_json() -> GString {
        gs(&pc::data().params)
    }

    /// Grupy parametrów (5 wektorów) i zakres lat mapy typologii (JSON).
    #[func]
    fn groups_json() -> GString {
        let d = pc::data();
        gs(&json!({ "groups": d.groups, "mapYears": d.map_years, "eras": d.eras }))
    }

    #[func]
    fn city_profiles_json() -> GString {
        gs(&pc::data().city_profiles)
    }

    /// Mieszanie bez geometrii: `{params, weights, year, dominant}` albo `{error}`.
    #[func]
    fn resolve(spec_json: GString) -> GString {
        match parse_spec(&spec_json) {
            Ok(spec) => {
                let r = pc::resolve_spec(&spec);
                gs(&json!({ "params": r.params, "weights": r.weights, "year": r.year, "dominant": r.dominant, "mix": pc::describe_mix(&pc::overall_weights(&r.weights), 4) }))
            }
            Err(e) => gs(&json!({ "error": e })),
        }
    }

    /// Budowa: `{mesh: ArrayMesh, stats: String(JSON), error: String}`.
    #[func]
    fn build(spec_json: GString, night: bool) -> VarDictionary {
        let mut d = VarDictionary::new();
        match parse_spec(&spec_json) {
            Ok(spec) => {
                let m = pc::generate_from_spec(&spec);
                d.set("mesh", &to_array_mesh(&m.mesh, night));
                d.set("stats", &gs(&m.stats));
                d.set("error", &GString::new());
            }
            Err(e) => d.set("error", &GString::from(e.as_str())),
        }
        d
    }

    /// Zapis GLB (ścieżka `res://`, `user://` albo systemowa). Zwraca komunikat błędu albo pusty tekst.
    #[func]
    fn export_glb(spec_json: GString, path: GString) -> GString {
        let spec = match parse_spec(&spec_json) {
            Ok(s) => s,
            Err(e) => return e.as_str().into(),
        };
        let m = pc::generate_from_spec(&spec);
        let name = std::path::Path::new(&path.to_string()).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or("budynek".into());
        let glb = pc::to_glb(&[pc::ExportNode { name, mesh: &m.mesh, offset: None, parent: None, rotation: None, extras: Some(json!({ "spec": spec, "stats": m.stats })) }]);
        let real = ProjectSettings::singleton().globalize_path(&path).to_string();
        match std::fs::write(&real, glb) {
            Ok(()) => GString::new(),
            Err(e) => format!("{real}: {e}").as_str().into(),
        }
    }

    /// Zespół zabudowy (`{"kind":"estate|crossroads|roundabout", ...}`): `{items: [{name, mesh}], info, error}`.
    #[func]
    fn build_scene(scene_json: GString, night: bool) -> VarDictionary {
        let mut d = VarDictionary::new();
        match serde_json::from_str::<SceneSpec>(&scene_json.to_string()) {
            Ok(ss) => {
                let sc = generate_scene(&ss);
                let mut items = VarArray::new();
                for it in &sc.items {
                    let mut e = VarDictionary::new();
                    e.set("name", &GString::from(it.name.as_str()));
                    e.set("mesh", &to_array_mesh(&it.mesh, night));
                    items.push(&e.to_variant());
                }
                d.set("items", &items);
                d.set("info", &GString::from(sc.describe().as_str()));
                d.set("error", &GString::new());
            }
            Err(e) => d.set("error", &GString::from(format!("Błędny JSON sceny: {e}").as_str())),
        }
        d
    }
}

// ------------------------------------------------------------------ węzeł zespołu zabudowy

/// Osiedle, skrzyżowanie albo rondo z budynkami; `spec` = JSON `SceneSpec`.
/// Siatki są dziećmi tworzonymi w locie (bez właściciela — nie trafiają do .tscn).
#[derive(GodotClass)]
#[class(tool, base=Node3D)]
pub struct PierzejaScene {
    #[export(multiline)]
    #[var(set = set_spec)]
    spec: GString,
    #[export]
    #[var(set = set_night)]
    night: bool,
    /// Opis ostatniej budowy albo komunikat błędu.
    #[var(no_set)]
    info: GString,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for PierzejaScene {
    fn init(base: Base<Node3D>) -> Self {
        Self { spec: r#"{"kind": "crossroads", "seed": 11}"#.into(), night: false, info: GString::new(), base }
    }
    fn ready(&mut self) {
        self.rebuild();
    }
}

#[godot_api]
impl PierzejaScene {
    #[func]
    fn set_spec(&mut self, spec: GString) {
        self.spec = spec;
        self.rebuild();
    }
    #[func]
    fn set_night(&mut self, night: bool) {
        self.night = night;
        self.rebuild();
    }
    #[func]
    fn rebuild(&mut self) {
        let d = Pierzeja::build_scene(self.spec.clone(), self.night);
        let err: GString = d.get("error").map(|v| v.to::<GString>()).unwrap_or_default();
        if !err.is_empty() {
            godot_warn!("PierzejaScene: {err}");
            self.info = err;
            return;
        }
        self.info = d.get("info").map(|v| v.to::<GString>()).unwrap_or_default();
        let mut base = self.base_mut();
        for mut c in base.get_children().iter_shared() {
            c.queue_free();
        }
        let items: VarArray = d.get("items").map(|v| v.to()).unwrap_or_default();
        for it in items.iter_shared() {
            let e: VarDictionary = it.to();
            let mut mi = MeshInstance3D::new_alloc();
            mi.set_name(&StringName::from(&e.get("name").map(|v| v.to::<GString>()).unwrap_or_default()));
            let mesh: Gd<ArrayMesh> = e.get("mesh").map(|v| v.to()).unwrap();
            mi.set_mesh(&mesh);
            mi.set_cast_shadows_setting(ShadowCastingSetting::DOUBLE_SIDED);
            base.add_child(&mi);
        }
    }
}

// ------------------------------------------------------------------ węzeł budynku

/// Budynek generowany na żywo w edytorze i w grze. `spec` = JSON `BuildingSpec`
/// (np. `{"archetype":"blok_plyta_5","seed":3}` albo `{"cursor":{"year":1965,"scale":0.6}}`).
#[derive(GodotClass)]
#[class(tool, base=MeshInstance3D)]
pub struct PierzejaBuilding {
    #[export(multiline)]
    #[var(set = set_spec)]
    spec: GString,
    /// Okna `glassLit` świecą (emisja).
    #[export]
    #[var(set = set_night)]
    night: bool,
    /// Metryka ostatniej budowy (JSON) albo komunikat błędu.
    #[var(no_set)]
    stats: GString,
    base: Base<MeshInstance3D>,
}

#[godot_api]
impl IMeshInstance3D for PierzejaBuilding {
    fn init(base: Base<MeshInstance3D>) -> Self {
        Self { spec: r#"{"archetype": "kamienica_eklektyczna", "seed": 1}"#.into(), night: false, stats: GString::new(), base }
    }

    fn ready(&mut self) {
        if self.base().get_mesh().is_none() {
            self.rebuild();
        }
    }

    /// Siatka powstaje ze `spec` — nie zapisuj jej w .tscn (tylko podgląd w inspektorze).
    fn on_validate_property(&self, property: &mut godot::register::info::PropertyInfo) {
        if property.property_name == StringName::from("mesh") {
            property.usage = godot::register::info::PropertyUsageFlags::EDITOR | godot::register::info::PropertyUsageFlags::READ_ONLY;
        }
    }
}

#[godot_api]
impl PierzejaBuilding {
    #[func]
    fn set_spec(&mut self, spec: GString) {
        self.spec = spec;
        self.rebuild();
    }

    #[func]
    fn set_night(&mut self, night: bool) {
        self.night = night;
        self.rebuild();
    }

    /// Przebudowuje siatkę z bieżącego `spec`.
    #[func]
    fn rebuild(&mut self) {
        let d = Pierzeja::build(self.spec.clone(), self.night);
        let err: GString = d.get("error").map(|v| v.to::<GString>()).unwrap_or_default();
        if !err.is_empty() {
            godot_warn!("PierzejaBuilding: {err}");
            self.stats = err;
            return;
        }
        self.stats = d.get("stats").map(|v| v.to::<GString>()).unwrap_or_default();
        if let Some(m) = d.get("mesh") {
            let mesh: Gd<ArrayMesh> = m.to();
            let mut base = self.base_mut();
            base.set_mesh(&mesh);
            // ściany bez grubości: cień z obu stron, inaczej oficyny rzucają tylko obrys
            base.set_cast_shadows_setting(ShadowCastingSetting::DOUBLE_SIDED);
        }
    }
}

// ------------------------------------------------------------------ Tabor (pojazdy)

fn parse_tabor(spec_json: &GString) -> Result<TaborSpec, String> {
    let s = spec_json.to_string();
    let spec: TaborSpec = if s.trim().is_empty() { TaborSpec::default() } else { serde_json::from_str(&s).map_err(|e| format!("Błędny JSON spec: {e}"))? };
    let td = tc::data();
    let bad: Vec<&str> = spec.mix.archetype.iter().map(String::as_str).chain(spec.mix.weights.iter().flat_map(|w| w.keys().map(String::as_str))).filter(|k| td.archetype(k).is_none()).collect();
    if bad.is_empty() { Ok(spec) } else { Err(format!("Nieznane archetypy: {}", bad.join(", "))) }
}

/// Statyczne API pojazdów — ten sam kształt co `Pierzeja` (panel edytora przełącza tylko nazwę klasy).
/// Pozycje w `build`: Godot-owe współrzędne rdzenia (przód +Z, bez przeliczenia na konwencję glTF).
#[derive(GodotClass)]
#[class(init, base=RefCounted)]
pub struct Tabor;

#[godot_api]
impl Tabor {
    #[func]
    fn archetypes_json() -> GString {
        let rows: Vec<Value> = tc::data().archetypes.iter().map(|a| json!({ "id": a.id, "name": a.name, "short": a.short, "years": a.years, "map": a.map, "description": a.description })).collect();
        gs(&rows)
    }

    #[func]
    fn params_json() -> GString {
        gs(&tc::data().params)
    }

    #[func]
    fn groups_json() -> GString {
        let d = tc::data();
        gs(&json!({ "groups": d.groups, "mapYears": d.map_years, "eras": d.eras }))
    }

    /// Mieszanie bez geometrii: `{params, weights, seed}` albo `{error}`.
    #[func]
    fn resolve(spec_json: GString) -> GString {
        match parse_tabor(&spec_json) {
            Ok(spec) => {
                let r = tc::resolve(&spec);
                gs(&json!({ "params": r.params, "weights": r.weights, "seed": r.seed }))
            }
            Err(e) => gs(&json!({ "error": e })),
        }
    }

    /// Budowa: `{body: ArrayMesh, wheel: ArrayMesh (wspólna), wheels: [{name, pos}], lightbar: ArrayMesh?, lightbar_pos: Vector3?, seats: [Vector3], stats: String(JSON), error: String}`.
    #[func]
    fn build(spec_json: GString, night: bool) -> VarDictionary {
        let mut d = VarDictionary::new();
        match parse_tabor(&spec_json) {
            Ok(spec) => {
                let m = tc::generate(&spec);
                d.set("body", &to_array_mesh(&m.body, night));
                d.set("wheel", &to_array_mesh(&m.wheel, night));
                let v3 = |p: &[f64; 3]| Vector3::new(p[0] as f32, p[1] as f32, p[2] as f32);
                let mut wheels = VarArray::new();
                for (name, pos) in &m.wheels {
                    let mut e = VarDictionary::new();
                    e.set("name", &GString::from(name.as_str()));
                    e.set("pos", v3(pos));
                    wheels.push(&e.to_variant());
                }
                d.set("wheels", &wheels);
                // belka świetlna służb (osobna siatka i jej położenie względem nadwozia); brak = cywilny
                if let Some((bar, o)) = &m.lightbar {
                    d.set("lightbar", &to_array_mesh(bar, night));
                    d.set("lightbar_pos", v3(o));
                }
                d.set("seats", &m.seats.iter().map(v3).collect::<PackedVector3Array>());
                d.set("stats", &gs(&m.stats));
                d.set("error", &GString::new());
            }
            Err(e) => d.set("error", &GString::from(e.as_str())),
        }
        d
    }

    /// Zapis GLB (ścieżka `res://`, `user://` albo systemowa). Zwraca komunikat błędu albo pusty tekst.
    #[func]
    fn export_glb(spec_json: GString, path: GString) -> GString {
        let spec = match parse_tabor(&spec_json) {
            Ok(s) => s,
            Err(e) => return e.as_str().into(),
        };
        let glb = tc::to_glb(&tc::generate(&spec));
        let real = ProjectSettings::singleton().globalize_path(&path).to_string();
        match std::fs::write(&real, glb) {
            Ok(()) => GString::new(),
            Err(e) => format!("{real}: {e}").as_str().into(),
        }
    }
}

/// Pojazd generowany na żywo (`spec` = JSON `TaborSpec`). Nadwozie i koła to dzieci tworzone
/// w locie (bez właściciela — nie trafiają do .tscn); koła współdzielą jedną siatkę.
#[derive(GodotClass)]
#[class(tool, base=Node3D)]
pub struct TaborVehicle {
    #[export(multiline)]
    #[var(set = set_spec)]
    spec: GString,
    /// Reflektory i światła tylne świecą (emisja).
    #[export]
    #[var(set = set_night)]
    night: bool,
    /// Metryka ostatniej budowy (JSON) albo komunikat błędu.
    #[var(no_set)]
    stats: GString,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for TaborVehicle {
    fn init(base: Base<Node3D>) -> Self {
        Self { spec: r#"{"archetype": "hatchback", "seed": 1}"#.into(), night: false, stats: GString::new(), base }
    }
    fn ready(&mut self) {
        self.rebuild();
    }
}

#[godot_api]
impl TaborVehicle {
    #[func]
    fn set_spec(&mut self, spec: GString) {
        self.spec = spec;
        self.rebuild();
    }
    #[func]
    fn set_night(&mut self, night: bool) {
        self.night = night;
        self.rebuild();
    }
    #[func]
    fn rebuild(&mut self) {
        let d = Tabor::build(self.spec.clone(), self.night);
        let err: GString = d.get("error").map(|v| v.to::<GString>()).unwrap_or_default();
        if !err.is_empty() {
            godot_warn!("TaborVehicle: {err}");
            self.stats = err;
            return;
        }
        self.stats = d.get("stats").map(|v| v.to::<GString>()).unwrap_or_default();
        let mut base = self.base_mut();
        for mut c in base.get_children().iter_shared() {
            c.queue_free();
        }
        let mut add = |name: &str, mesh: &Gd<ArrayMesh>, pos: Vector3| {
            let mut mi = MeshInstance3D::new_alloc();
            mi.set_name(name);
            mi.set_mesh(mesh);
            mi.set_position(pos);
            mi.set_cast_shadows_setting(ShadowCastingSetting::DOUBLE_SIDED);
            base.add_child(&mi);
        };
        let body: Gd<ArrayMesh> = d.get("body").unwrap().to();
        let wheel: Gd<ArrayMesh> = d.get("wheel").unwrap().to();
        add("Body", &body, Vector3::ZERO);
        let wheels: VarArray = d.get("wheels").map(|v| v.to()).unwrap_or_default();
        for w in wheels.iter_shared() {
            let e: VarDictionary = w.to();
            let name: GString = e.get("name").map(|v| v.to()).unwrap_or_default();
            add(&name.to_string(), &wheel, e.get("pos").map(|v| v.to()).unwrap_or_default());
        }
    }
}
