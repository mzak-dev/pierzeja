//! Szybki podgląd pojazdu przy strojeniu kształtu: bok (rzut prostokątny), 3/4 przód, 3/4 tył, przód.
//! `cargo run -p pierzeja-render --example podglad_pojazdu --release -- '<TaborSpec json>' <katalog> [płasko]`
use pierzeja_core::MeshData;
use pierzeja_render::{render_scene, RenderItem, RenderOptions};
use tabor_core::{generate, TaborSpec};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let spec: TaborSpec = serde_json::from_str(&a[1]).expect("TaborSpec");
    let dir = &a[2];
    let smooth = a.get(3).is_none_or(|s| s != "plasko");
    std::fs::create_dir_all(dir).expect("katalog");
    let m = generate(&spec);
    let mut meshes: Vec<(&MeshData, [f64; 3])> = vec![(&m.body, [0.0; 3])];
    meshes.extend(m.wheels.iter().map(|(_, p)| (&m.wheel, *p)));
    if let Some((l, o)) = &m.lightbar {
        meshes.push((l, *o));
    }
    let items: Vec<RenderItem> = meshes.iter().map(|(mesh, offset)| RenderItem { mesh, offset: *offset }).collect();
    let views = [("bok", 90.0, 0.0, true), ("przod-3-4", 38.0, 16.0, false), ("tyl-3-4", 218.0, 14.0, false), ("przod", 0.0, 4.0, false), ("gora", 90.0, 89.0, true)];
    for (name, az, el, ortho) in views {
        let o = RenderOptions { width: 1200, height: 560, azimuth: Some(az), elevation: Some(el), smooth, ortho, ..Default::default() };
        std::fs::write(format!("{dir}/{name}.png"), render_scene(&items, &o).to_png()).expect("zapis");
    }
    println!("{}", serde_json::to_string(&m.stats).expect("stats"));
}
