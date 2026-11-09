//! Tłum — proceduralny generator ludzi (ProceduraSuite, ADR 0022).
//! `TlumSpec` → wektor parametrów → `HumanModel`: ciało z pierścieni na kość, głowa z twarzą
//! i włosami, poza statyczna przez FK na szkielecie `rr_human`. Układ glTF: Y w górę, przód +Z,
//! lewa strona postaci = +X; origin = grunt pod środkiem między stopami (poza siedząca: pod biodrami).

mod body;
mod font;
mod head;
mod skel;

pub use skel::{Joint, Pose};

use indexmap::IndexMap;
use procedura_kit::color::mix_hex;
use procedura_kit::data::{Dataset, ParamMap, WeightMap};
use procedura_kit::export::{to_glb as kit_to_glb, ExportNode};
use procedura_kit::mesh::{Bounds, MatKey, MaterialSpec, MeshBuilder, MeshData, Xform, V3};
use procedura_kit::mix::{resolve_map, MixSpec};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;

const JSON: &str = include_str!("../../../data/tlum-dane.json");

/// Dane Tłumu: wektory, parametry, archetypy (year = rok urodzenia, scale = formalność stroju).
pub fn data() -> &'static Dataset {
    static D: OnceLock<Dataset> = OnceLock::new();
    D.get_or_init(|| Dataset::load(JSON))
}

/// Cały dokument jako JSON: sekcje spoza `Dataset` (`szkielet`, `proporcje`, `palety`, `mundury`, …).
// ponytail: surowy Value zamiast struktur serde — typuj sekcje dopiero, gdy ich użyje kolejny krok.
pub fn extra() -> &'static Value {
    static V: OnceLock<Value> = OnceLock::new();
    V.get_or_init(|| serde_json::from_str(JSON).expect("tlum-dane.json"))
}

/// Kontrakt wymiany: pola mieszania wspólne z ProceduraSuite + poza statyczna.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlumSpec {
    #[serde(flatten)]
    pub mix: MixSpec,
    /// `rest` (T-pose rr_human), `stand` (domyślnie), `walk`, `sit` (krzesło), `drive` (kierowca), `ride` (pasażer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pose: Option<String>,
    /// Faza kroku 0–1 (tylko `walk`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Resolved {
    pub params: ParamMap,
    pub weights: IndexMap<String, WeightMap>,
    pub seed: u32,
}

pub fn resolve(spec: &TlumSpec) -> Resolved {
    let (params, weights, seed) = resolve_map(data(), &spec.mix);
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HumanStats {
    pub pose: Pose,
    pub triangles: usize,
    /// Wysokość siatki (czubek głowy z włosami minus najniższy punkt).
    pub height: f64,
}

pub struct HumanModel {
    /// Całe ciało w jednej siatce (jeden prymityw na slot materiału).
    pub body: MeshData,
    /// Kości w pozie (head/tail w przestrzeni modelu); pomocnicze IK podążają za dłonią/stopą.
    /// Kolejność: 29 kości `rr_human`, potem palce (Unity Humanoid: `LeftThumbProximal` … `RightLittleDistal`).
    pub joints: Vec<Joint>,
    /// Kości w T-pose przeskalowane parametrami (baza pod skin).
    pub rest: Vec<Joint>,
    /// Punkt H (środek stawów biodrowych) w pozie — do osadzenia w Miejscu Tabora.
    pub hip: V3,
    pub stats: HumanStats,
    pub params: ParamMap,
}

fn mat(key: MatKey, name: &'static str, color: &str, roughness: f64) -> MaterialSpec {
    MaterialSpec { key, name, color: color.into(), roughness, metalness: 0.0, opacity: 1.0, double_sided: false, texture: "none", texture_scale: 1.0, emissive: None }
}

fn vest_color(kind: &str) -> &'static str {
    match kind {
        "kam_niebieska" => "#1f4aa8",
        "kdr_bialo_niebieska" => "#e8eef6",
        "odblask_pomaranczowa" => "#ff6a13",
        "kamizelka_policji" => "#17191f",
        _ => "#dff020",
    }
}

fn materials(p: &P) -> Vec<MaterialSpec> {
    let pal = &extra()["palety"];
    let skin = pal["monk"][(p.f("skinTone") as usize).clamp(1, 10) - 1]["hex"].as_str().unwrap();
    let hair = mix_hex(p.s("hairColor"), pal["siwe"][1]["hex"].as_str().unwrap(), p.f("greyRatio").clamp(0.0, 1.0));
    let (mid, outer) = (p.s("upperMidType"), p.s("outerType"));
    // marynarka bez okrycia gra rolę okrycia (koszula widoczna w wycięciu)
    let top = if !matches!(mid, "brak" | "marynarka") { p.s("upperMidColor") } else { p.s("upperBaseColor") };
    let jacket = if mid == "marynarka" && matches!(outer, "brak" | "kamizelka") { p.s("upperMidColor") } else { p.s("outerColor") };
    let helmet = MaterialSpec { double_sided: true, ..mat(MatKey::Helmet, "Nakrycie głowy", p.s("headColor"), 0.6) };
    // slot `Stubble`: cień zarostu, a bez zarostu odcień warg
    let stubble = if p.s("beard") == "brak" { mix_hex(skin, "#9c4f4a", 0.32) } else { mix_hex(skin, &hair, 0.3) };
    vec![
        mat(MatKey::Skin, "Skóra", skin, 0.9),
        mat(MatKey::Top, "Góra", top, 0.9),
        mat(MatKey::Bottom, "Dół", p.s("lowerColor"), 0.9),
        mat(MatKey::Shoes, "Buty", p.s("feetColor"), 0.8),
        mat(MatKey::Hair, "Włosy", &hair, 0.9),
        mat(MatKey::Outer, "Okrycie", jacket, 0.85),
        mat(MatKey::HiVis, "Kamizelka", vest_color(p.s("vest")), 0.7),
        helmet,
        mat(MatKey::Face, "Twarz", "#241d1a", 0.9),
        MaterialSpec { metalness: 0.3, ..mat(MatKey::Reflective, "Taśma odblaskowa", "#dde0e3", 0.35) },
        mat(MatKey::Eye, "Białko oka", "#e9e4dc", 0.4),
        mat(MatKey::Detail, "Akcent", body::detail_color(p), 0.7),
        mat(MatKey::Stubble, "Zarost / wargi", &stubble, 0.95),
    ]
}

pub fn generate(spec: &TlumSpec) -> HumanModel {
    let r = resolve(spec);
    let p = P(&r.params);
    let pose = spec.pose.as_deref().map_or(Pose::Stand, |s| Pose::parse(s).unwrap_or_else(|| panic!("nieznana poza {s}")));
    let phase = spec.phase.unwrap_or(0.0).rem_euclid(1.0);

    let sc = skel::Scale::new(&p);
    let rest = skel::rest(&sc);
    let w = skel::pose(&rest, &sc, pose, phase, p.s("posture"));
    let dy = if pose == Pose::Rest { 0.0 } else { skel::ground_shift(&rest, &sc, &w) };
    let lift = Xform::translate([0.0, dy, 0.0]);
    let wl: Vec<Xform> = w.iter().map(|x| lift.mul(x)).collect();

    let mut mb = MeshBuilder::default();
    let b = body::Body::new(&rest, &sc, &p, &wl);
    b.emit(&mut mb);
    b.hands(&mut mb, &wl);
    let mut hb = MeshBuilder::default();
    head::build(&mut hb, &rest, &sc, &p, r.seed);
    mb.push(&hb.build(vec![]), &wl[skel::idx(&rest, "Head")]);
    let body = mb.build(materials(&p));

    let joints = skel::posed(&rest, &w, dy);
    let (hl, hr) = (joints[skel::idx(&joints, "LeftUpperLeg")].head, joints[skel::idx(&joints, "RightUpperLeg")].head);
    let hip = std::array::from_fn(|k| (hl[k] + hr[k]) / 2.0);
    let b = body.bounds;
    let stats = HumanStats { pose, triangles: body.triangles(), height: b.max[1] - b.min[1] };
    HumanModel { body, joints, rest, hip, stats, params: r.params }
}

impl HumanModel {
    pub fn bounds(&self) -> Bounds {
        self.body.bounds
    }
}

/// GLB: `Body` (jedna siatka w pozie, `extras.stats`) + hierarchia pustych węzłów kości
/// (`Root` … `RightHandGrip`, potem palce `LeftThumbProximal` … `RightLittleDistal`);
/// pozycja lokalna = różnica głów kości, więc węzły leżą w pozie.
pub fn to_glb(m: &HumanModel) -> Vec<u8> {
    let empty = MeshData::empty();
    let mut nodes = vec![ExportNode { name: "Body".into(), mesh: &m.body, offset: None, parent: None, rotation: None, extras: Some(json!({ "stats": m.stats })) }];
    for j in &m.joints {
        let (parent, offset) = match j.parent {
            Some(pi) => (Some(pi + 1), std::array::from_fn(|k| j.head[k] - m.joints[pi].head[k])),
            None => (None, j.head),
        };
        nodes.push(ExportNode { name: j.name.clone(), mesh: &empty, offset: Some(offset), parent, rotation: None, extras: None });
    }
    kit_to_glb(&nodes)
}
