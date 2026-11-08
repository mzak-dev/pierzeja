//! Pierzeja: losowanie budynków z profilu miasta i ustawienie ich wzdłuż ulicy.

use crate::blend::{normalize_weights, BuildingSpec, MixSpec};
use crate::data::{data, CityProfile, WeightMap};
use crate::generator::{fixed1, generate_from_spec, BuildingModel};
use crate::materials::build_materials;
use crate::mesh::{Bounds, MatKey, MeshBuilder, MeshData, V3};
use crate::params::Params;
use crate::random::{hash01, jround, js_u32, Mulberry32};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreetSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<WeightMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variation: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mixing: Option<f64>,
}

pub struct StreetBuilding {
    pub index: usize,
    pub archetype: String,
    pub spec: BuildingSpec,
    pub model: BuildingModel,
    pub offset: V3,
    pub width: f64,
}

pub struct StreetModel {
    pub buildings: Vec<StreetBuilding>,
    pub context: MeshData,
    pub bounds: Bounds,
    pub profile: Option<&'static CityProfile>,
}

fn pick_weighted(w: &WeightMap, r: f64) -> String {
    let mut acc = 0.0;
    for (id, v) in w {
        acc += v;
        if r <= acc {
            return id.clone();
        }
    }
    w.keys().last().cloned().unwrap_or_default()
}

pub fn generate_street(s: &StreetSpec) -> StreetModel {
    let d = data();
    let profile = match (&s.profile, &s.weights) {
        (Some(id), _) => d.city(id),
        (None, Some(_)) => None,
        (None, None) => d.city_profiles.first(),
    };
    let weights = normalize_weights(s.weights.as_ref().or(profile.map(|p| &p.weights)));
    let count = jround(s.count.or(profile.map(|p| p.count as f64)).unwrap_or(8.0)).clamp(1.0, 40.0) as usize;
    let seed = js_u32(s.seed.unwrap_or(7.0));
    let mixing = s.mixing.or(profile.map(|p| p.mixing)).unwrap_or(0.2);
    let variation = s.variation.unwrap_or(0.35);
    let mut rnd = Mulberry32::new(seed);

    struct Item {
        archetype: String,
        spec: BuildingSpec,
        model: BuildingModel,
        width: f64,
        gap: f64,
        setback: f64,
    }
    let mut items: Vec<Item> = Vec::new();
    for i in 0..count {
        let a = pick_weighted(&weights, rnd.next());
        let mut w: WeightMap = IndexMap::from([(a.clone(), 1.0)]);
        if rnd.next() < mixing {
            let b = pick_weighted(&weights, rnd.next());
            if b != a {
                let t = 0.15 + rnd.next() * 0.3;
                w.insert(a.clone(), 1.0 - t);
                w.insert(b, t);
            }
        }
        let bseed = (hash01(&[seed as f64, i as f64, 3.0]) * 1e9).floor();
        let spec = BuildingSpec { mix: MixSpec { weights: Some(w), seed: Some(bseed), variation: Some(variation), ..Default::default() }, ..Default::default() };
        let model = generate_from_spec(&spec);
        let p = &model.resolved.params;
        let (width, gap, setback) = (p.segments * p.segment_width, p.side_gap, p.street_setback);
        items.push(Item { archetype: a, spec, model, width, gap, setback });
    }
    // rozstawienie wzdłuż ulicy (oś X), front przy z = −odsunięcie
    let mut buildings = Vec::new();
    let mut x = 0.0;
    let mut prev_gap: Option<f64> = None;
    for (index, it) in items.into_iter().enumerate() {
        if let Some(pg) = prev_gap {
            let continuous = pg < 0.5 && it.gap < 0.5;
            x += if continuous { 0.0 } else { pg.max(it.gap) };
        }
        prev_gap = Some(it.gap);
        buildings.push(StreetBuilding { index, archetype: it.archetype, spec: it.spec, model: it.model, offset: [x + it.width / 2.0, 0.0, -it.setback], width: it.width });
        x += it.width;
    }
    let shift = -x / 2.0;
    for b in &mut buildings {
        b.offset[0] += shift;
    }

    // kontekst: chodnik, jezdnia, linia środkowa
    let mut mb = MeshBuilder::default();
    let (x0, x1) = (shift - 20.0, -shift + 20.0);
    mb.bx(MatKey::Sidewalk, [x0, -0.02, 0.0], [x1, 0.12, 3.6]);
    mb.bx(MatKey::Road, [x0, -0.02, 3.6], [x1, 0.02, 11.4]);
    mb.bx(MatKey::Sidewalk, [x0, -0.02, 11.4], [x1, 0.12, 15.0]);
    let mut xx = x0 + 2.0;
    while xx < x1 - 2.0 {
        mb.bx(MatKey::Trim, [xx, 0.02, 7.42], [xx + 3.0, 0.025, 7.58]);
        xx += 6.0;
    }
    let mut dp = crate::params::default_params();
    dp.insert("trimColor".into(), "#e9e7df".into());
    let mats: Vec<_> = build_materials(&Params::from_map(&dp)).into_iter().filter(|m| matches!(m.key, MatKey::Sidewalk | MatKey::Road | MatKey::Trim)).collect();
    let context = mb.build(mats);

    let (mut min, mut max) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for b in &buildings {
        let bb = b.model.mesh.bounds;
        for k in 0..3 {
            min[k] = min[k].min(bb.min[k] + b.offset[k]);
            max[k] = max[k].max(bb.max[k] + b.offset[k]);
        }
    }
    for k in 0..3 {
        min[k] = min[k].min(context.bounds.min[k]);
        max[k] = max[k].max(context.bounds.max[k]);
    }
    StreetModel { buildings, context, bounds: Bounds { min, max }, profile }
}

pub fn describe_street(st: &StreetModel) -> String {
    st.buildings
        .iter()
        .map(|b| {
            let name = data().archetype(&b.archetype).map_or(b.archetype.as_str(), |a| a.name.as_str());
            let s = &b.model.stats;
            format!("{}. {} — {} kond., {} m, front {} m", b.index + 1, name, s.floors, fixed1(s.height_total, false), fixed1(b.width, false))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
