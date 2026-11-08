//! Podgląd PNG pojazdu ze spec JSON: `cargo run -p pierzeja-render --example tabor --release -- spec.json out.png [view]`
use pierzeja_render::{render_scene, RenderItem, RenderOptions, View};
use tabor_core::{generate, TaborSpec};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let spec: TaborSpec = serde_json::from_str(&std::fs::read_to_string(&a[0]).expect("spec")).expect("json");
    let view: View = a.get(2).map(|v| serde_json::from_value(serde_json::json!(v)).expect("view")).unwrap_or_default();
    let m = generate(&spec);
    let mut items = vec![RenderItem { mesh: &m.body, offset: [0.0; 3] }];
    items.extend(m.wheels.iter().map(|(_, pos)| RenderItem { mesh: &m.wheel, offset: *pos }));
    items.extend(m.lightbar.iter().map(|(l, o)| RenderItem { mesh: l, offset: *o }));
    let img = render_scene(&items, &RenderOptions { width: 1000, height: 600, view, ..Default::default() });
    std::fs::write(&a[1], img.to_png()).expect("png");
    println!("{}", serde_json::to_string(&m.stats).unwrap());
}
