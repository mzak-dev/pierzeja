//! Katalog spec JSON → GLB + kopia spec obok (np. do `RapidResponse/Assets/Vehicles`):
//! `cargo run -p tabor-core --example bake_dir --release -- crates/tabor/examples/gra <katalog_wyjściowy>`
//! Każdy `<nazwa>.json` daje `<nazwa>.glb` i `<nazwa>.json`; na wyjściu tabela statystyk.
use tabor_core::{generate, to_glb, TaborSpec};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (src, out) = (&a[0], &a[1]);
    std::fs::create_dir_all(out).expect("katalog wyjściowy");
    let mut files: Vec<_> = std::fs::read_dir(src).expect("katalog spec").filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    for p in files {
        let text = std::fs::read_to_string(&p).expect("spec");
        let spec: TaborSpec = serde_json::from_str(&text).expect("json");
        let m = generate(&spec);
        let name = p.file_stem().unwrap().to_string_lossy();
        std::fs::write(format!("{out}/{name}.glb"), to_glb(&m)).expect("glb");
        std::fs::write(format!("{out}/{name}.json"), text).expect("spec");
        let s = &m.stats;
        println!("{name:<10} {:<10} {:>5} tri  {:.2} × {:.2} × {:.2} m  rozstaw {:.2}  miejsca {}", s.body_type, s.triangles, s.length, s.width, s.height, s.wheelbase, s.seats);
    }
}
