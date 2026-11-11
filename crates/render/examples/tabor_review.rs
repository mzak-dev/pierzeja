//! Arkusz review pojazdu: `-- <spec.json | archetyp> out_prefix` → `<prefix>_{front,side,back,top,q}.png`.
use pierzeja_render::{render_scene, RenderItem, RenderOptions};
use tabor_core::{generate, TaborSpec};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let json = std::fs::read_to_string(&a[0]).unwrap_or_else(|_| format!(r#"{{"archetype":"{}","seed":1}}"#, a[0]));
    let spec: TaborSpec = serde_json::from_str(&json).expect("json");
    let m = generate(&spec);
    let mut items = vec![RenderItem { mesh: &m.body, offset: [0.0; 3] }];
    items.extend(m.wheels.iter().map(|(_, pos)| RenderItem { mesh: &m.wheel, offset: *pos }));
    items.extend(m.lightbar.iter().map(|(l, o)| RenderItem { mesh: l, offset: *o }));
    for (name, az, el) in [("front", 0.0, 3.0), ("side", 90.0, 3.0), ("back", 180.0, 3.0), ("top", 90.0, 89.0), ("q", 35.0, 18.0)] {
        let o = RenderOptions { width: 900, height: 540, azimuth: Some(az), elevation: Some(el), smooth: true, ..Default::default() };
        std::fs::write(format!("{}_{name}.png", a[1]), render_scene(&items, &o).to_png()).expect("png");
    }
    println!("{}", serde_json::to_string(&m.stats).unwrap());
}
