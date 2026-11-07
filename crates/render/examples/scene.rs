//! Podgląd sceny: `cargo run -p pierzeja-render --example scene --release -- estate|crossroads|roundabout out.png [view] [az] [el]`
use pierzeja_core::scene::{generate_scene, SceneSpec};
use pierzeja_render::{render_scene, RenderItem, RenderOptions, View};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let spec: SceneSpec = serde_json::from_value(serde_json::json!({ "kind": a[0] })).expect("kind");
    let view: View = a.get(2).map(|v| serde_json::from_value(serde_json::json!(v)).expect("view")).unwrap_or(View::Aerial);
    let sc = generate_scene(&spec);
    let items: Vec<RenderItem> = sc.items.iter().map(|i| RenderItem { mesh: &i.mesh, offset: [0.0; 3] }).collect();
    let o = RenderOptions { width: 1400, height: 900, view, azimuth: a.get(3).map(|x| x.parse().unwrap()), elevation: a.get(4).map(|x| x.parse().unwrap()), ..Default::default() };
    std::fs::write(&a[1], render_scene(&items, &o).to_png()).expect("png");
    println!("{}", sc.describe());
}
