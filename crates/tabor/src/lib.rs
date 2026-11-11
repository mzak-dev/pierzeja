//! Tabor — proceduralny generator pojazdów (ProceduraSuite, ADR 0023).
//! `TaborSpec` → wagi archetypów → wektor parametrów → `VehicleModel` (nadwozie z loftu
//! stacji + jedna siatka koła + kotwice). Konwencja: przód +Z, Y w górę, lewa strona +X,
//! origin = środek między osiami na gruncie.

mod ciezkie;
mod detale;
mod karoseria;
mod sluzba;
mod tablice;
mod wheel;
mod zabudowa;

use indexmap::IndexMap;
use procedura_kit::color::adjust_hex;
use procedura_kit::data::{Dataset, ParamMap, WeightMap};
use procedura_kit::export::{to_glb as kit_to_glb, ExportNode};
use procedura_kit::mesh::{Bounds, MatKey, MaterialSpec, MeshBuilder, MeshData, V3};
use procedura_kit::mix::{resolve_map, MixSpec};
use procedura_kit::random::hash01_seeded;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::OnceLock;

/// Układ wyliczony przez budowniczego nadwozia (osobowe i vany: `karoseria`, ciężkie: `ciezkie`).
pub(crate) struct Layout {
    pub wheelbase: f64,
    /// Rozstaw środków opon.
    pub track: f64,
    /// Liczba stacji powłoki albo lofta kabiny.
    pub stations: usize,
    /// Szerokość nadwozia bez lusterek (tak podają producenci i dyrektywa 96/53/WE).
    pub body_width: f64,
    /// Belka świetlna służby (siatka, przesunięcie od origin); `None` dla cywilnego.
    pub lightbar: Option<(MeshData, V3)>,
    /// Piasty kół (nazwa węzła, pozycja); `None` = zwykłe 4 koła z rozstawu osi i kół.
    pub wheels: Option<Vec<(String, V3)>>,
    /// Kotwice (sprzęg, nogi naczepy): nazwa węzła, pozycja.
    pub anchors: Vec<(String, V3)>,
}

/// Dane Tabora: wektory, parametry, archetypy, mapa rok designu × klasa wielkości.
pub fn data() -> &'static Dataset {
    static D: OnceLock<Dataset> = OnceLock::new();
    D.get_or_init(|| Dataset::load(include_str!("../../../data/tabor-dane.json")))
}

/// Kontrakt wymiany: pola mieszania wspólne z ProceduraSuite + wersja formatu.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaborSpec {
    #[serde(flatten)]
    pub mix: MixSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Resolved {
    pub params: ParamMap,
    pub weights: IndexMap<String, WeightMap>,
    pub seed: u32,
}

pub fn resolve(spec: &TaborSpec) -> Resolved {
    let (mut params, weights, seed) = resolve_map(data(), &spec.mix);
    sluzba::defaults(&mut params);
    Resolved { params, weights, seed }
}

/// Odczyt parametrów po kluczu (wektor po `resolve_map` jest pełny i znormalizowany).
pub(crate) struct P<'a>(pub &'a ParamMap);

impl P<'_> {
    pub fn f(&self, k: &str) -> f64 {
        self.0[k].as_f64().unwrap_or_else(|| panic!("parametr {k} nie jest liczbą"))
    }
    pub fn s(&self, k: &str) -> &str {
        self.0[k].as_str().unwrap_or_else(|| panic!("parametr {k} nie jest tekstem"))
    }
}

/// Losowanie „prawdopodobieństwa” z wektora (dachu dwukolorowego, okładzin…) — stałe dla seeda.
pub(crate) fn roll(seed: u32, salt: f64, prob: f64) -> bool {
    hash01_seeded(seed, &[salt]) < prob
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleStats {
    pub body_type: String,
    /// Trójkąty całego pojazdu (nadwozie + 4 koła).
    pub triangles: usize,
    pub length: f64,
    /// Szerokość nadwozia bez lusterek (jak w danych producentów i w limicie 2,55 m).
    pub width: f64,
    /// Szerokość z lusterkami.
    pub width_mirrors: f64,
    pub height: f64,
    pub wheelbase: f64,
    /// Rozstaw kół (środki opon).
    pub track: f64,
    pub wheel_radius: f64,
    pub seats: usize,
    /// Stacje lofta nadwozia (8–24).
    pub stations: usize,
}

pub struct VehicleModel {
    pub body: MeshData,
    /// Jedna siatka koła (oś = lokalne X, symetryczna), współdzielona przez 4 węzły.
    pub wheel: MeshData,
    /// Piasty: (nazwa węzła, pozycja). Zawsze `Wheel_FL`, `Wheel_FR`, `Wheel_RL`, `Wheel_RR`
    /// (pierwsza oś przednia i pierwsza tylna; przy bliźniakach — koło zewnętrzne), dalej kolejne
    /// osie `Wheel_F2*`, `Wheel_R2*`, `Wheel_R3*` i wewnętrzne bliźniaki z sufiksem `i` (`Wheel_RLi`).
    /// Naczepa nie ma kół `Wheel_F*`.
    pub wheels: Vec<(String, V3)>,
    /// Kotwice: `Anchor_FifthWheel` (siodło ciągnika), `Anchor_Kingpin` i `Anchor_LandingGear` (naczepa).
    pub anchors: Vec<(String, V3)>,
    /// Punkty biodrowe Miejsc; 0 = kierowca (lewa strona, +X).
    pub seats: Vec<V3>,
    /// Belka świetlna służby w gnieździe na dachu: (siatka, przesunięcie od origin).
    pub lightbar: Option<(MeshData, V3)>,
    pub stats: VehicleStats,
    pub params: ParamMap,
}

fn mat(key: MatKey, name: &'static str, color: &str, roughness: f64, metalness: f64) -> MaterialSpec {
    MaterialSpec { key, name, color: color.into(), roughness, metalness, opacity: 1.0, double_sided: false, texture: "none", texture_scale: 1.0, emissive: None }
}

fn materials(p: &P, seed: u32) -> Vec<MaterialSpec> {
    // ponytail: „weathering” tylko przygasza lakier; brud, rysy i rdza to osobny krok
    let age = p.f("weathering");
    let paint = |c: &str| adjust_hex(c, -0.06 * age, 1.0 - 0.2 * age, 0.0);
    let steel = roll(seed, 3.0, p.f("steelWheels"));
    vec![
        mat(MatKey::Paint, "Lakier", &paint(p.s("paintColor")), 0.42, 0.25),
        mat(MatKey::RoofPaint, "Dach", &paint(p.s("roofColor")), 0.42, 0.25),
        mat(MatKey::Bumper, "Zderzaki", &paint(p.s("bumperColor")), 0.5, 0.15),
        mat(MatKey::Plastic, "Plastik", p.s("plasticColor"), 0.8, 0.0),
        mat(MatKey::Glass, "Szyby", p.s("glassColor"), 0.08, 0.5),
        mat(MatKey::Tyre, "Opony", "#17181a", 0.95, 0.0),
        mat(MatKey::Rim, "Felgi", match (steel, p.s("bodyType") == "podwozie") { (true, true) => "#b4b9bd", (true, false) => "#5d6368", _ => "#bcc1c5" }, 0.4, if steel { 0.3 } else { 0.8 }),
        MaterialSpec { emissive: Some("#fff3c4"), ..mat(MatKey::Headlight, "Reflektory", "#f2efe2", 0.2, 0.2) },
        MaterialSpec { emissive: Some("#ff2a1a"), ..mat(MatKey::Taillight, "Światła tylne", "#a31a1a", 0.3, 0.0) },
    ]
}

/// Miejsca: kierowca z lewej (+X), pierwszy rząd 2 (van: 3 w ławce), dalej po 3 w rzędzie.
fn seat_points(p: &P) -> Vec<V3> {
    let (n, wb, w) = (p.f("seats") as usize, p.f("wheelbase"), p.f("width"));
    let van = p.s("bodyType") == "van";
    // punkt H: podłoga ok. 0,12 m nad prześwitem, siedzisko 0,3 m nad podłogą (poza `drive`/`ride`:
    // pięty ok. 0,3 m pod punktem H); van: kabina 0,25 m wyżej
    let y = p.f("clearance") + 0.12 + if van { 0.25 } else { 0.0 } + 0.30;
    let front_z = wb / 2.0 - wb * if van { 0.27 } else { 0.38 };
    let (front_cap, pitch) = (if van { 3 } else { 2 }, if van { 0.9 } else { 0.7 });
    let k = w / 1.8;
    let x = |i: usize, cnt: usize| match (cnt, i) {
        (1, _) | (2, 0) => 0.36 * k,
        (2, _) => -0.36 * k,
        (_, i) => [0.42, 0.0, -0.42][i] * k,
    };
    let (mut out, mut row) = (Vec::new(), 0usize);
    while out.len() < n {
        let cnt = (if row == 0 { front_cap } else { 3 }).min(n - out.len());
        let z = if row == 0 { front_z } else { wb / 2.0 - wb * 0.76 - (row as f64 - 1.0) * pitch };
        out.extend((0..cnt).map(|i| [x(i, cnt), y, z]));
        row += 1;
    }
    out
}

pub fn generate(spec: &TaborSpec) -> VehicleModel {
    let r = resolve(spec);
    let p = P(&r.params);
    let heavy = p.s("bodyType") == "podwozie";
    let mut mats = materials(&p, r.seed);
    if heavy {
        mats.extend(ciezkie::extra_materials(&p));
    }

    let mut mb = MeshBuilder::default();
    let layout = if heavy {
        ciezkie::build(&p, r.seed, &mut mb)
    } else {
        let roof_mat = if roll(r.seed, 1.0, p.f("twoToneRoof")) { MatKey::RoofPaint } else { MatKey::Paint };
        let w = karoseria::build(&p, roof_mat, karoseria::Gestosc::LOD0);
        w.powloka.emit(&mut mb, karoseria::CREASE);
        detale::build(&mut mb, &p, r.seed, &w.ksztalt, &w.powloka, roll(r.seed, 2.0, p.f("claddingBlack")));
        let lightbar = karoseria::livery(&mut mb, &p, r.seed, &w);
        Layout { wheelbase: p.f("wheelbase"), track: w.track, stations: w.stations, body_width: w.body_width, lightbar, wheels: None, anchors: Vec::new() }
    };
    let mut body_mats = mats.clone();
    body_mats.extend(tablice::materials());
    body_mats.extend(sluzba::materials(&p));
    let body = mb.build(body_mats);
    let wheel = wheel::build(&p, mats);

    let (rad, hub_x) = (p.f("wheelDiameter") / 2.0, layout.track / 2.0);
    let wheels = layout.wheels.unwrap_or_else(|| {
        vec![
            ("Wheel_FL".into(), [hub_x, rad, layout.wheelbase / 2.0]),
            ("Wheel_FR".into(), [-hub_x, rad, layout.wheelbase / 2.0]),
            ("Wheel_RL".into(), [hub_x, rad, -layout.wheelbase / 2.0]),
            ("Wheel_RR".into(), [-hub_x, rad, -layout.wheelbase / 2.0]),
        ]
    });
    let seats = if heavy { ciezkie::seat_points(&p) } else { seat_points(&p) };
    let mut m = VehicleModel {
        body,
        wheel,
        wheels,
        seats,
        lightbar: layout.lightbar,
        anchors: layout.anchors,
        stats: VehicleStats { body_type: p.s("bodyType").into(), triangles: 0, length: 0.0, width: layout.body_width, width_mirrors: 0.0, height: 0.0, wheelbase: layout.wheelbase, track: layout.track, wheel_radius: rad, seats: 0, stations: layout.stations },
        params: r.params.clone(),
    };
    let b = m.bounds();
    m.stats.triangles = m.body.triangles() + m.wheels.len() * m.wheel.triangles() + m.lightbar.as_ref().map_or(0, |l| l.0.triangles());
    m.stats.length = b.max[2] - b.min[2];
    m.stats.width_mirrors = b.max[0] - b.min[0];
    m.stats.height = b.max[1] - b.min[1];
    m.stats.seats = m.seats.len();
    m
}

impl VehicleModel {
    /// Granice całego pojazdu (nadwozie + koła).
    pub fn bounds(&self) -> Bounds {
        let (mut lo, mut hi) = (self.body.bounds.min, self.body.bounds.max);
        for (_, h) in &self.wheels {
            for k in 0..3 {
                lo[k] = lo[k].min(h[k] + self.wheel.bounds.min[k]);
                hi[k] = hi[k].max(h[k] + self.wheel.bounds.max[k]);
            }
        }
        if let Some((l, o)) = &self.lightbar {
            for k in 0..3 {
                lo[k] = lo[k].min(o[k] + l.bounds.min[k]);
                hi[k] = hi[k].max(o[k] + l.bounds.max[k]);
            }
        }
        Bounds { min: lo, max: hi }
    }
}

fn child(name: String, mesh: &MeshData, offset: V3) -> ExportNode<'_> {
    ExportNode { name, mesh, offset: Some(offset), parent: Some(0), rotation: None, extras: None }
}

/// GLB: korzeń `Body` (siatka nadwozia), dzieci `Wheel_FL/FR/RL/RR` (pivot w piaście, jedna
/// współdzielona siatka, toczenie = obrót wokół lokalnego X) i puste `Seat_0..N-1`.
pub fn to_glb(m: &VehicleModel) -> Vec<u8> {
    let empty = MeshData::empty();
    let mut nodes = vec![ExportNode { name: "Body".into(), mesh: &m.body, offset: None, parent: None, rotation: None, extras: Some(json!({ "stats": m.stats })) }];
    for (n, pos) in &m.wheels {
        nodes.push(child(n.clone(), &m.wheel, *pos));
    }
    if let Some((mesh, o)) = &m.lightbar {
        nodes.push(child("Lightbar".into(), mesh, *o));
    }
    for (n, pos) in &m.anchors {
        nodes.push(child(n.clone(), &empty, *pos));
    }
    for (i, s) in m.seats.iter().enumerate() {
        nodes.push(child(format!("Seat_{i}"), &empty, *s));
    }
    kit_to_glb(&nodes)
}
