//! Serwer MCP (stdio) generatora Pierzeja — te same 9 narzędzi co prototyp.
//! Uruchomienie: `claude mcp add pierzeja -- <ścieżka>/pierzeja-mcp.exe`.
//! Katalog wyjściowy: `PIERZEJA_OUT` (domyślnie `~/pierzeja-out`).
//!
//! ponytail: JSON-RPC po stdio ręcznie (initialize, tools/list, tools/call, ping)
//! zamiast SDK `rmcp` + tokio; przejdź na rmcp, gdy potrzebne będą zasoby,
//! prompty, powiadomienia o postępie albo transport HTTP.

use base64::Engine;
use pierzeja_core::scene::{generate_scene, SceneSpec};
use pierzeja_core::{data, describe_mix, describe_street, generate_from_spec, generate_street, overall_weights, to_glb, to_obj, unknown_archetypes, BuildingModel, BuildingSpec, ExportNode, StreetSpec, GROUP_IDS};
use pierzeja_render::{render_scene, RenderItem, RenderOptions, View};
use serde_json::{json, Map, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn out_dir() -> PathBuf {
    std::env::var_os("PIERZEJA_OUT").map(PathBuf::from).unwrap_or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default();
        Path::new(&home).join("pierzeja-out")
    })
}

// ------------------------------------------------------------------ schematy wejścia
fn spec_props() -> Map<String, Value> {
    let ids: Vec<&str> = data().archetypes.iter().map(|a| a.id.as_str()).collect();
    let map_point = json!({ "type": "object", "properties": {
        "year": { "type": "number", "minimum": 1800, "maximum": 2015, "description": "Rok 1800–2015 (oś X mapy typologii)" },
        "scale": { "type": "number", "minimum": 0, "maximum": 1, "description": "Skala zabudowy: 0 = dom, ~0.6 = kamienica, 1 = wielki blok" } },
        "required": ["year", "scale"] });
    let weights = json!({ "type": "object", "additionalProperties": { "type": "number", "minimum": 0 }, "description": "Wagi archetypów, np. {\"kamienica_eklektyczna\": 0.6, \"plomba\": 0.4} (normalizowane do sumy 1)" });
    let per_group = |v: &Value| Value::Object(GROUP_IDS.iter().map(|g| (g.to_string(), v.clone())).collect());
    match json!({
        "archetype": { "type": "string", "enum": ids, "description": "Jeden archetyp w 100%" },
        "weights": weights,
        "cursor": map_point,
        "groupWeights": { "type": "object", "properties": per_group(&weights), "additionalProperties": false, "description": "Osobne wagi dla wektorów: massing (bryła), storeys (kondygnacje), roof (dach), facade (elewacja), material (materiał). Mają pierwszeństwo." },
        "groupCursors": { "type": "object", "properties": per_group(&map_point), "additionalProperties": false, "description": "Osobne punkty na mapie dla wybranych wektorów" },
        "overrides": { "type": "object", "additionalProperties": { "type": ["number", "string"] }, "description": "Ręczne wartości parametrów (klucze z get_parameter_schema), np. {\"floors\": 7, \"wallColor\": \"#c9b48d\"}" },
        "seed": { "type": "integer", "minimum": 0, "description": "Ziarno losowości (domyślnie 1)" },
        "variation": { "type": "number", "minimum": 0, "maximum": 1, "description": "Siła losowej wariacji 0–1 (domyślnie 0)" },
        "sharpness": { "type": "number", "minimum": 0.5, "maximum": 8, "description": "Wykładnik IDW (domyślnie 3)" },
        "neighbors": { "type": "integer", "minimum": 1, "maximum": 8, "description": "Liczba archetypów w mieszance z mapy (domyślnie 3)" },
        "regulations": { "type": "boolean", "description": "Reguły z warunków technicznych, np. loggie zamiast balkonów powyżej 25 m (domyślnie true)" },
        "footprint": { "type": "object", "description": "Obrys wielokątny zamiast prostokąta (np. z OSM). Punkty [x, z] w metrach, ulica po stronie +z. Krawędź i = points[i] → points[i+1].",
            "properties": {
                "points": { "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }, "minItems": 3 },
                "front": { "type": "array", "items": { "type": "integer", "minimum": 0 }, "description": "Krawędzie frontowe (od ulicy): wejście, witryny, pełny detal. Domyślnie krawędź zwrócona ku +z." },
                "blind": { "type": "array", "items": { "type": "integer", "minimum": 0 }, "description": "Ściany graniczne (styk z sąsiadem) bez okien." }
            }, "required": ["points"] }
    }) {
        Value::Object(m) => m,
        _ => unreachable!(),
    }
}

fn view_prop() -> Value {
    json!({ "type": "string", "enum": ["iso", "front", "street", "aerial", "back"], "description": "Ujęcie: iso = izometria, front = elewacja, street = z ulicy, aerial = z góry, back = od podwórza" })
}

fn tool(name: &str, title: &str, desc: &str, props: Map<String, Value>, required: &[&str], read_only: bool) -> Value {
    json!({ "name": name, "title": title, "description": desc,
        "inputSchema": { "type": "object", "properties": props, "required": required },
        "annotations": { "readOnlyHint": read_only, "destructiveHint": false, "openWorldHint": false } })
}

fn with(mut base: Map<String, Value>, extra: Value) -> Map<String, Value> {
    if let Value::Object(e) = extra {
        base.extend(e);
    }
    base
}

fn tools() -> Value {
    let groups: Vec<&str> = GROUP_IDS.to_vec();
    let profiles: Vec<&str> = data().city_profiles.iter().map(|c| c.id.as_str()).collect();
    let fmt = json!({ "type": "string", "enum": ["glb", "obj", "both"], "description": "Format modelu (domyślnie glb)" });
    json!([
        tool("list_archetypes", "Lista archetypów", "Lista 19 archetypów budynków (id, nazwa, lata, położenie na mapie typologii rok × skala, opis). Id służą jako klucze wag w pozostałych narzędziach.", Map::new(), &[], true),
        tool("describe_archetype", "Opis archetypu", "Szczegóły archetypu: opis, cechy, źródła oraz pełny wektor parametrów.", with(Map::new(), json!({ "id": { "type": "string", "description": "Id archetypu" } })), &["id"], true),
        tool("get_parameter_schema", "Schemat parametrów", "Definicje parametrów w 5 wektorach (massing, storeys, roof, facade, material): klucz, typ, zakres, jednostka, domyślna, opcje. Klucze można nadpisywać w overrides.", with(Map::new(), json!({ "group": { "type": "string", "enum": groups } })), &[], true),
        tool("list_city_profiles", "Profile pierzei", "Profile ulic (proporcje typów zabudowy) dla generate_street.", Map::new(), &[], true),
        tool("resolve_parameters", "Mieszanie wektorów", "Interpolacja bez geometrii: wagi archetypów dla każdego wektora, datowanie, typ dominujący i pełny wektor parametrów.", spec_props(), &[], true),
        tool("generate_building", "Generuj budynek", "Generuje budynek ze specyfikacji, zapisuje .glb/.obj i .json, zwraca metrykę i podgląd PNG.", with(spec_props(), json!({
            "format": fmt, "preview": { "type": "boolean", "description": "Dołącz podgląd PNG (domyślnie true)" }, "view": view_prop(), "name": { "type": "string", "description": "Nazwa plików (bez rozszerzenia)" } })), &[], false),
        tool("render_building", "Podgląd budynku", "Renderuje budynek do PNG (renderer programowy z cieniami), także nocą z oświetlonymi oknami.", with(spec_props(), json!({
            "view": view_prop(), "width": { "type": "integer", "minimum": 160, "maximum": 2000 }, "height": { "type": "integer", "minimum": 120, "maximum": 2000 },
            "night": { "type": "boolean" }, "azimuth": { "type": "number", "description": "Azymut kamery w stopniach (0 = na wprost frontu)" }, "elevation": { "type": "number", "minimum": -5, "maximum": 89 } })), &[], false),
        tool("morph_sequence", "Interpolacja A → B", "Szereg budynków interpolowanych od archetypu A do B na jednym obrazie; można morfować tylko wybrane wektory.", with(Map::new(), json!({
            "from": { "type": "string" }, "to": { "type": "string" }, "steps": { "type": "integer", "minimum": 2, "maximum": 9 },
            "groups": { "type": "array", "items": { "type": "string", "enum": groups } }, "seed": { "type": "integer", "minimum": 0 }, "view": view_prop(), "save_glb": { "type": "boolean" } })), &["from", "to"], false),
        tool("generate_scene", "Generuj zespół zabudowy", "Przykładowe zespoły: osiedle bloków (odstępy między rzędami × H, drogi dojazdowe, parkingi, chodniki, plac zabaw), skrzyżowanie i rondo z budynkami narożnymi na obrysach wielokątnych i pierzejami na ramionach. Zapisuje GLB (węzeł na budynek) i zwraca podgląd PNG.", with(Map::new(), json!({
            "kind": { "type": "string", "enum": ["estate", "crossroads", "roundabout"], "description": "estate = osiedle bloków, crossroads = skrzyżowanie, roundabout = rondo" },
            "seed": { "type": "integer", "minimum": 0 }, "spacing": { "type": "number", "minimum": 0.8, "maximum": 3, "description": "Osiedle: odstęp rzędów jako wielokrotność wysokości (domyślnie 1,5; WT 2002 § 13: min. 1)" },
            "rows": { "type": "integer", "minimum": 1, "maximum": 6 }, "profile": { "type": "string", "enum": profiles.clone() }, "arm": { "type": "number", "minimum": 40, "maximum": 300, "description": "Długość ramion [m]" },
            "variation": { "type": "number", "minimum": 0, "maximum": 1 }, "format": fmt.clone(), "preview": { "type": "boolean" }, "view": view_prop(), "night": { "type": "boolean" },
            "azimuth": { "type": "number" }, "elevation": { "type": "number" }, "name": { "type": "string" } })), &["kind"], false),
        tool("generate_street", "Generuj ulicę", "Pierzeja: szereg budynków losowanych z profilu miasta albo własnych wag, z hybrydami. Zapisuje model (węzeł na budynek) i podgląd PNG.", with(Map::new(), json!({
            "profile": { "type": "string", "enum": profiles }, "weights": { "type": "object", "additionalProperties": { "type": "number", "minimum": 0 } },
            "count": { "type": "integer", "minimum": 1, "maximum": 40 }, "seed": { "type": "integer", "minimum": 0 }, "mixing": { "type": "number", "minimum": 0, "maximum": 1 },
            "variation": { "type": "number", "minimum": 0, "maximum": 1 }, "format": fmt, "preview": { "type": "boolean" }, "view": view_prop(), "night": { "type": "boolean" }, "name": { "type": "string" } })), &[], false),
    ])
}

// ------------------------------------------------------------------ pomocnicze
type ToolResult = Result<Vec<Value>, String>;

fn text(t: impl Into<String>) -> Value {
    json!({ "type": "text", "text": t.into() })
}
fn image(png: &[u8]) -> Value {
    json!({ "type": "image", "data": base64::engine::general_purpose::STANDARD.encode(png), "mimeType": "image/png" })
}
fn pretty(v: &impl serde::Serialize) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}
fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn to_spec(args: &Value) -> Result<(BuildingSpec, Vec<String>), String> {
    let keys = ["archetype", "weights", "cursor", "groupWeights", "groupCursors", "overrides", "seed", "variation", "sharpness", "neighbors", "regulations", "footprint"];
    let m: Map<String, Value> = keys.iter().filter_map(|k| args.get(*k).map(|v| (k.to_string(), v.clone()))).collect();
    let spec: BuildingSpec = serde_json::from_value(Value::Object(m)).map_err(|e| format!("Błędna specyfikacja: {e}"))?;
    let bad = unknown_archetypes(&spec);
    if !bad.is_empty() {
        return Err(format!("Nieznane archetypy: {}. Dostępne: {}.", bad.join(", "), arch_ids()));
    }
    let bad_keys: Vec<&String> = spec.mix.overrides.iter().flat_map(|o| o.keys()).filter(|k| data().param(k).is_none()).collect();
    let warnings = if bad_keys.is_empty() { vec![] } else { vec![format!("Pominięto nieznane parametry: {} (lista: get_parameter_schema).", bad_keys.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))] };
    Ok((spec, warnings))
}

fn arch_ids() -> String {
    data().archetypes.iter().map(|a| a.id.as_str()).collect::<Vec<_>>().join(", ")
}

fn unique_base(dir: &Path, base: &str) -> String {
    let mut clean: String = base.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '-' }).collect();
    clean = clean.trim_matches('-').chars().take(80).collect();
    if clean.is_empty() {
        clean = "model".into();
    }
    let taken = |n: &str| ["glb", "obj", "png"].iter().any(|e| dir.join(format!("{n}.{e}")).exists());
    let mut name = clean.clone();
    let mut i = 2;
    while taken(&name) {
        name = format!("{clean}-{i}");
        i += 1;
    }
    name
}

fn write_model(dir: &Path, nodes: &[ExportNode], base: &str, format: &str, spec: &Value) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut files = Vec::new();
    if format == "glb" || format == "both" {
        let p = dir.join(format!("{base}.glb"));
        std::fs::write(&p, to_glb(nodes))?;
        files.push(p);
    }
    if format == "obj" || format == "both" {
        let (obj, mtl) = to_obj(nodes, &format!("{base}.mtl"));
        let p = dir.join(format!("{base}.obj"));
        std::fs::write(&p, obj)?;
        std::fs::write(dir.join(format!("{base}.mtl")), mtl)?;
        files.extend([p, dir.join(format!("{base}.mtl"))]);
    }
    let sp = dir.join(format!("{base}.json"));
    std::fs::write(&sp, pretty(spec))?;
    files.push(sp);
    Ok(files)
}

fn view_of(args: &Value, default: View) -> View {
    args.get("view").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or(default)
}

fn render_png(items: &[RenderItem], view: View, width: u32, height: u32, night: bool, az: Option<f64>, el: Option<f64>) -> Vec<u8> {
    render_scene(items, &RenderOptions { width, height, view, night, azimuth: az, elevation: el, ssaa: 2, ..Default::default() }).to_png()
}

fn stats_text(m: &BuildingModel) -> String {
    let d = data();
    let (s, r) = (&m.stats, &m.resolved);
    let dom = d.archetype(&r.dominant).map_or(r.dominant.as_str(), |a| a.name.as_str());
    let mut lines = vec![format!("Typ dominujący: {dom} (datowanie ≈ {})", r.year), format!("Mieszanka: {}", describe_mix(&overall_weights(&r.weights), 4))];
    for g in &d.groups {
        lines.push(format!("  {:<12} {}", g.label, describe_mix(&r.weights[g.id.as_str()], 3)));
    }
    lines.push(format!("Kondygnacje: {} · wysokość {} m (ściany {} m) · grupa wysokości {}", s.floors, r2(s.height_total), r2(s.height_walls), s.height_class));
    lines.push(format!("Front × głębokość: {} × {} m · zabudowa {} m² · pow. całkowita {} m²", r2(s.frontage), r2(s.depth), s.footprint.round(), s.gfa.round()));
    lines.push(format!("Klatki: {} · mieszkania ≈ {} · okna {} · balkony {} · trójkąty {}", s.staircases, s.flats_estimate, s.windows, s.balconies, s.triangles));
    if !s.notes.is_empty() {
        lines.push("Uwagi:".into());
        lines.extend(s.notes.iter().map(|n| format!("  - {n}")));
    }
    lines.join("\n")
}

fn files_text(files: &[PathBuf]) -> String {
    files.iter().map(|f| format!("  {}", f.display())).collect::<Vec<_>>().join("\n")
}

fn io<T>(r: std::io::Result<T>) -> Result<T, String> {
    r.map_err(|e| format!("Błąd zapisu: {e}"))
}

// ------------------------------------------------------------------ narzędzia
fn call(name: &str, a: &Value) -> ToolResult {
    let d = data();
    let dir = out_dir();
    let s = |k: &str| a.get(k).and_then(Value::as_str);
    let b = |k: &str| a.get(k).and_then(Value::as_bool);
    let n = |k: &str| a.get(k).and_then(Value::as_f64);
    match name {
        "list_archetypes" => {
            let rows: Vec<Value> = d.archetypes.iter().map(|x| json!({ "id": x.id, "name": x.name, "years": x.years, "map": x.map, "description": x.description })).collect();
            Ok(vec![text(pretty(&rows))])
        }
        "describe_archetype" => {
            let id = s("id").unwrap_or_default();
            let x = d.archetype(id).ok_or_else(|| format!("Nieznany archetyp „{id}”. Dostępne: {}.", arch_ids()))?;
            Ok(vec![text(pretty(x))])
        }
        "get_parameter_schema" => {
            let g = s("group");
            let rows: Vec<Value> = d
                .groups
                .iter()
                .filter(|x| g.is_none_or(|g| g == x.id))
                .map(|x| json!({ "id": x.id, "label": x.label, "description": x.description, "params": d.params.iter().filter(|p| p.group == x.id).collect::<Vec<_>>() }))
                .collect();
            Ok(vec![text(pretty(&rows))])
        }
        "list_city_profiles" => Ok(vec![text(pretty(&d.city_profiles))]),
        "resolve_parameters" => {
            let (spec, warnings) = to_spec(a)?;
            let r = pierzeja_core::resolve_spec(&spec);
            let mix: Map<String, Value> = GROUP_IDS.iter().map(|g| (g.to_string(), describe_mix(&r.weights[*g], 4).into())).collect();
            Ok(vec![text(pretty(&json!({ "dominant": r.dominant, "year": r.year, "mix": mix, "weights": r.weights, "params": r.params, "warnings": warnings })))])
        }
        "generate_building" => {
            let (spec, warnings) = to_spec(a)?;
            let m = generate_from_spec(&spec);
            let base = unique_base(&dir, s("name").unwrap_or(&format!("{}-{}", m.resolved.dominant, m.resolved.seed)));
            let node = ExportNode { name: base.clone(), mesh: &m.mesh, offset: None, extras: Some(json!({ "spec": spec, "stats": m.stats })) };
            let files = io(write_model(&dir, &[node], &base, s("format").unwrap_or("glb"), &serde_json::to_value(&spec).unwrap()))?;
            let warn = if warnings.is_empty() { String::new() } else { format!("\n\n{}", warnings.join("\n")) };
            let mut out = vec![text(format!("{}\n\nZapisano:\n{}{warn}", stats_text(&m), files_text(&files)))];
            if b("preview") != Some(false) {
                let png = render_png(&[RenderItem { mesh: &m.mesh, offset: [0.0; 3] }], view_of(a, View::Iso), 960, 640, false, None, None);
                io(std::fs::write(dir.join(format!("{base}.png")), &png))?;
                out.push(image(&png));
            }
            Ok(out)
        }
        "render_building" => {
            let (spec, _) = to_spec(a)?;
            let m = generate_from_spec(&spec);
            let view = view_of(a, View::Iso);
            let night = b("night") == Some(true);
            let png = render_png(
                &[RenderItem { mesh: &m.mesh, offset: [0.0; 3] }],
                view,
                n("width").unwrap_or(960.0) as u32,
                n("height").unwrap_or(640.0) as u32,
                night,
                n("azimuth"),
                n("elevation"),
            );
            io(std::fs::create_dir_all(&dir))?;
            let vname = s("view").unwrap_or("iso");
            let p = dir.join(format!("{}.png", unique_base(&dir, &format!("{}-{}-{vname}{}", m.resolved.dominant, m.resolved.seed, if night { "-noc" } else { "" }))));
            io(std::fs::write(&p, &png))?;
            Ok(vec![text(format!("{}\n\nPNG: {}", stats_text(&m), p.display())), image(&png)])
        }
        "morph_sequence" => {
            let (from, to) = (s("from").unwrap_or_default(), s("to").unwrap_or_default());
            for id in [from, to] {
                if d.archetype(id).is_none() {
                    return Err(format!("Nieznany archetyp „{id}”. Dostępne: {}.", arch_ids()));
                }
            }
            let steps = n("steps").unwrap_or(5.0).clamp(2.0, 9.0) as usize;
            let sel: Vec<String> = a.get("groups").and_then(|g| serde_json::from_value(g.clone()).ok()).filter(|g: &Vec<String>| !g.is_empty()).unwrap_or(GROUP_IDS.map(String::from).to_vec());
            let seed = n("seed").unwrap_or(3.0);
            let mut models = Vec::new();
            let mut lines = Vec::new();
            let mut x = 0.0;
            for i in 0..steps {
                let t = i as f64 / (steps - 1) as f64;
                let gw: Map<String, Value> = GROUP_IDS
                    .iter()
                    .map(|g| {
                        let w = if !sel.iter().any(|s| s == g) || t == 0.0 { json!({ from: 1 }) } else if t == 1.0 { json!({ to: 1 }) } else { json!({ from: 1.0 - t, to: t }) };
                        (g.to_string(), w)
                    })
                    .collect();
                let spec: BuildingSpec = serde_json::from_value(json!({ "groupWeights": gw, "seed": seed })).map_err(|e| e.to_string())?;
                let m = generate_from_spec(&spec);
                let bb = m.mesh.bounds;
                let offset = [x - bb.min[0], 0.0, 0.0];
                x += bb.max[0] - bb.min[0] + 8.0;
                lines.push(format!("t = {t:.2}: {} kond., {} m, front {} m — {}", m.stats.floors, r2(m.stats.height_total), r2(m.stats.frontage), describe_mix(&overall_weights(&m.resolved.weights), 2)));
                models.push((m, offset, t));
            }
            let items: Vec<RenderItem> = models.iter().map(|(m, o, _)| RenderItem { mesh: &m.mesh, offset: *o }).collect();
            let png = if a.get("view").is_some() { render_png(&items, view_of(a, View::Front), 1400, 620, false, None, None) } else { render_png(&items, View::Front, 1400, 620, false, Some(14.0), Some(11.0)) };
            io(std::fs::create_dir_all(&dir))?;
            let base = unique_base(&dir, &format!("morf-{from}-{to}"));
            let mut files = vec![dir.join(format!("{base}.png"))];
            io(std::fs::write(&files[0], &png))?;
            if b("save_glb") == Some(true) {
                let nodes: Vec<ExportNode> = models.iter().map(|(m, o, t)| ExportNode { name: format!("t{}", (t * 100.0).round()), mesh: &m.mesh, offset: Some(*o), extras: None }).collect();
                let p = dir.join(format!("{base}.glb"));
                io(std::fs::write(&p, to_glb(&nodes)))?;
                files.push(p);
            }
            let head = format!("Interpolacja {} → {} (wektory: {})", d.archetype(from).unwrap().name, d.archetype(to).unwrap().name, sel.join(", "));
            Ok(vec![text(format!("{head}\n{}\n\nZapisano:\n{}", lines.join("\n"), files_text(&files))), image(&png)])
        }
        "generate_scene" => {
            let keys = ["kind", "seed", "spacing", "rows", "profile", "arm", "variation"];
            let sm: Map<String, Value> = keys.iter().filter_map(|k| a.get(*k).map(|v| (k.to_string(), v.clone()))).collect();
            let ss: SceneSpec = serde_json::from_value(Value::Object(sm)).map_err(|e| format!("Błędne parametry sceny: {e}"))?;
            let sc = generate_scene(&ss);
            let nodes: Vec<ExportNode> = sc
                .items
                .iter()
                .enumerate()
                .map(|(i, it)| ExportNode { name: format!("{:02}-{}", i + 1, it.name), mesh: &it.mesh, offset: None, extras: it.spec.as_ref().map(|s| json!({ "spec": s, "stats": it.stats })) })
                .collect();
            let kind = s("kind").unwrap_or("estate");
            let base = unique_base(&dir, s("name").unwrap_or(&format!("{kind}-{}", n("seed").unwrap_or(0.0))));
            let files = io(write_model(&dir, &nodes, &base, s("format").unwrap_or("glb"), a))?;
            let mut out = vec![text(format!("{}\n\nZapisano:\n{}", sc.describe(), files_text(&files)))];
            if b("preview") != Some(false) {
                let items: Vec<RenderItem> = sc.items.iter().map(|it| RenderItem { mesh: &it.mesh, offset: [0.0; 3] }).collect();
                let png = render_png(&items, view_of(a, View::Aerial), 1400, 900, b("night") == Some(true), n("azimuth"), n("elevation"));
                io(std::fs::write(dir.join(format!("{base}.png")), &png))?;
                out.push(image(&png));
            }
            Ok(out)
        }
        "generate_street" => {
            if let Some(p) = s("profile")
                && d.city(p).is_none()
            {
                return Err(format!("Nieznany profil „{p}”. Dostępne: {}.", d.city_profiles.iter().map(|c| c.id.as_str()).collect::<Vec<_>>().join(", ")));
            }
            let keys = ["profile", "weights", "count", "seed", "mixing", "variation"];
            let sm: Map<String, Value> = keys.iter().filter_map(|k| a.get(*k).map(|v| (k.to_string(), v.clone()))).collect();
            let ss: StreetSpec = serde_json::from_value(Value::Object(sm)).map_err(|e| format!("Błędne parametry ulicy: {e}"))?;
            let bad: Vec<&String> = ss.weights.iter().flat_map(|w| w.keys()).filter(|k| d.archetype(k).is_none()).collect();
            if !bad.is_empty() {
                return Err(format!("Nieznane archetypy: {}.", bad.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
            }
            let st = generate_street(&ss);
            let mut nodes: Vec<ExportNode> = st
                .buildings
                .iter()
                .map(|b| ExportNode { name: format!("{:02}-{}", b.index + 1, b.archetype), mesh: &b.model.mesh, offset: Some(b.offset), extras: Some(json!({ "spec": b.spec, "stats": b.model.stats })) })
                .collect();
            nodes.push(ExportNode { name: "ulica".into(), mesh: &st.context, offset: None, extras: None });
            let base = unique_base(&dir, s("name").unwrap_or(&format!("ulica-{}-{}", s("profile").or(st.profile.map(|p| p.id.as_str())).unwrap_or("wlasna"), n("seed").unwrap_or(7.0))));
            let specs: Vec<&BuildingSpec> = st.buildings.iter().map(|b| &b.spec).collect();
            let files = io(write_model(&dir, &nodes, &base, s("format").unwrap_or("glb"), &json!({ "street": a, "buildings": specs })))?;
            let head = format!("{} — {} budynków", st.profile.map_or("Ulica (własne wagi)", |p| p.name.as_str()), st.buildings.len());
            let mut out = vec![text(format!("{head}\n{}\n\nZapisano:\n{}", describe_street(&st), files_text(&files)))];
            if b("preview") != Some(false) {
                let mut items: Vec<RenderItem> = st.buildings.iter().map(|b| RenderItem { mesh: &b.model.mesh, offset: b.offset }).collect();
                items.push(RenderItem { mesh: &st.context, offset: [0.0; 3] });
                let png = render_png(&items, view_of(a, View::Street), 1400, 700, b("night") == Some(true), None, None);
                io(std::fs::write(dir.join(format!("{base}.png")), &png))?;
                out.push(image(&png));
            }
            Ok(out)
        }
        _ => Err(format!("Nieznane narzędzie „{name}”.")),
    }
}

// ------------------------------------------------------------------ JSON-RPC
fn handle(msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone(); // powiadomienia (bez id) nie dostają odpowiedzi
    let method = msg.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => json!({
            "protocolVersion": params.get("protocolVersion").cloned().unwrap_or(json!("2025-06-18")),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "pierzeja", "version": VERSION },
            "instructions": format!("Proceduralny generator budynków typowych dla polskich miast 1800–2015 (dworek, kamienice, familok, wille, socrealizm, bloki z wielkiej płyty, falowiec, dom-kostka, plomby, apartamentowce, stodoła). Budynek opisują wagi 19 archetypów, osobno dla 5 wektorów (massing, storeys, roof, facade, material); wartości pomiędzy archetypami są interpolowane. Zacznij od list_archetypes, potem generate_building lub render_building. Modele zapisują się w {}.", out_dir().display()),
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools() }),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let res = std::panic::catch_unwind(|| call(name, &args)).unwrap_or_else(|_| Err("Błąd generatora (panic).".into()));
            match res {
                Ok(content) => json!({ "content": content }),
                Err(e) => json!({ "content": [text(e)], "isError": true }),
            }
        }
        _ => return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("Nieznana metoda: {method}") } })),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn main() {
    eprintln!("pierzeja MCP {VERSION} — katalog wyjściowy: {}", out_dir().display());
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&msg),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } })),
        };
        if let Some(r) = reply {
            let _ = writeln!(stdout, "{r}");
            let _ = stdout.flush();
        }
    }
}
