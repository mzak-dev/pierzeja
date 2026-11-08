//! Podgląd PNG sylwetek: `cargo run -p pierzeja-render --example tlum --release -- out.png [azymut] spec1.json spec2.json …`
//! (spec = TlumSpec; kilka spec ustawia postacie w rzędzie co 1,1 m, prostopadle do widoku).
use pierzeja_render::{render_scene, RenderItem, RenderOptions, View};
use tlum_core::{generate, TlumSpec};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let az: f64 = a[1].parse().unwrap_or(25.0);
    let models: Vec<_> = a[2..].iter().map(|f| generate(&serde_json::from_str::<TlumSpec>(&std::fs::read_to_string(f).expect("spec")).expect("json"))).collect();
    let n = models.len() as f64;
    let items: Vec<RenderItem> = models.iter().enumerate().map(|(i, m)| RenderItem { mesh: &m.body, offset: { let t = (i as f64 - (n - 1.0) / 2.0) * 1.1; [t * az.to_radians().cos(), 0.0, t * az.to_radians().sin()] } }).collect();
    let w = (500.0 + 220.0 * n) as u32;
    let img = render_scene(&items, &RenderOptions { width: w, height: 700, view: View::Iso, azimuth: Some(az), elevation: Some(8.0), ..Default::default() });
    std::fs::write(&a[0], img.to_png()).expect("png");
    for m in &models {
        println!("{}", serde_json::to_string(&m.stats).unwrap());
    }
}
