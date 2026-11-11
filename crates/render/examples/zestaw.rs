//! Zestaw: ciągnik + naczepa sprzęgnięte kotwicami (sworzeń w punkcie siodła).
//! `cargo run -p pierzeja-render --example zestaw --release -- '<ciągnik>' '<naczepa>' <katalog>`
use pierzeja_core::MeshData;
use pierzeja_render::{render_scene, RenderItem, RenderOptions};
use tabor_core::{generate, TaborSpec, VehicleModel};

fn items<'a>(m: &'a VehicleModel, o: [f64; 3], out: &mut Vec<(&'a MeshData, [f64; 3])>) {
    out.push((&m.body, o));
    out.extend(m.wheels.iter().map(|(_, p)| (&m.wheel, [p[0] + o[0], p[1] + o[1], p[2] + o[2]])));
    if let Some((l, lo)) = &m.lightbar {
        out.push((l, [lo[0] + o[0], lo[1] + o[1], lo[2] + o[2]]));
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = generate(&serde_json::from_str::<TaborSpec>(&a[1]).expect("ciągnik"));
    let s = generate(&serde_json::from_str::<TaborSpec>(&a[2]).expect("naczepa"));
    let dir = &a[3];
    std::fs::create_dir_all(dir).expect("katalog");
    let fw = t.anchors.iter().find(|x| x.0 == "Anchor_FifthWheel").expect("siodło").1;
    let kp = s.anchors.iter().find(|x| x.0 == "Anchor_Kingpin").expect("sworzeń").1;
    let mut meshes = Vec::new();
    items(&t, [0.0; 3], &mut meshes);
    items(&s, [fw[0] - kp[0], fw[1] - kp[1], fw[2] - kp[2]], &mut meshes);
    let its: Vec<RenderItem> = meshes.iter().map(|(mesh, offset)| RenderItem { mesh, offset: *offset }).collect();
    for (name, az, el, ortho) in [("bok", 90.0, 0.0, true), ("przod-3-4", 35.0, 14.0, false), ("tyl-3-4", 215.0, 14.0, false)] {
        let o = RenderOptions { width: 1600, height: 600, azimuth: Some(az), elevation: Some(el), smooth: true, ortho, ..Default::default() };
        std::fs::write(format!("{dir}/{name}.png"), render_scene(&its, &o).to_png()).expect("zapis");
    }
}
