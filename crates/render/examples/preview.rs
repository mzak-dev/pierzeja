//! Podgląd PNG ze spec JSON: `cargo run -p pierzeja-render --example preview --release -- spec.json out.png [view]`
use pierzeja_core::{generate_from_spec, BuildingSpec};
use pierzeja_render::{render_scene, RenderItem, RenderOptions, View};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let spec: BuildingSpec = serde_json::from_str(&std::fs::read_to_string(&a[0]).expect("spec")).expect("json");
    let view: View = a.get(2).map(|v| serde_json::from_value(serde_json::json!(v)).expect("view")).unwrap_or_default();
    let m = generate_from_spec(&spec);
    let img = render_scene(&[RenderItem { mesh: &m.mesh, offset: [0.0; 3] }], &RenderOptions { width: 900, height: 600, view, ..Default::default() });
    std::fs::write(&a[1], img.to_png()).expect("png");
    println!("{}", serde_json::to_string(&m.stats).unwrap());
}
