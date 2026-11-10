//! Podgląd PNG sylwetek: `cargo run -p pierzeja-render --example tlum --release -- out.png [azymut] spec1.json spec2.json …`
//! (spec = TlumSpec; kilka spec ustawia postacie w rzędzie co 1,1 m, prostopadle do widoku).
use pierzeja_render::{render_scene, RenderItem, RenderOptions, View};
use tlum_core::{generate, TlumSpec};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let az: f64 = a[1].parse().unwrap_or(25.0);
    let mut models: Vec<_> = a[2..].iter().map(|f| generate(&serde_json::from_str::<TlumSpec>(&std::fs::read_to_string(f).expect("spec")).expect("json"))).collect();
    // HEAD=1: tylko głowa (trójkąty powyżej szyi) do oceny twarzy
    if std::env::var("HEAD").is_ok() {
        for m in &mut models {
            for part in &mut m.body.parts {
                let ys = part.positions.chunks_exact(3).map(|p| p[1]).collect::<Vec<_>>();
                let top = (m.body.bounds.max[1] - 0.24) as f32;
                part.indices = part.indices.chunks_exact(3).filter(|t| t.iter().all(|&i| ys[i as usize] > top)).flatten().copied().collect();
            }
            m.body.bounds.min[1] = m.body.bounds.max[1] - 0.24;
        }
    }
    let n = models.len() as f64;
    let items: Vec<RenderItem> = models.iter().enumerate().map(|(i, m)| RenderItem { mesh: &m.body, offset: { let t = (i as f64 - (n - 1.0) / 2.0) * 1.1; [t * az.to_radians().cos(), 0.0, t * az.to_radians().sin()] } }).collect();
    let (w, hgt) = if std::env::var("HEAD").is_ok() { (1600, 2400) } else { ((500.0 + 220.0 * n) as u32, 700) };
    let img = render_scene(&items, &RenderOptions { width: w, height: hgt, view: View::Iso, azimuth: Some(az), elevation: Some(8.0), ..Default::default() });
    std::fs::write(&a[0], img.to_png()).expect("png");
    for m in &models {
        println!("{}", serde_json::to_string(&m.stats).unwrap());
    }
}
