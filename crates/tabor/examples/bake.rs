//! Spec JSON → GLB: `cargo run -p tabor-core --example bake --release -- spec.json out.glb`
use tabor_core::{generate, to_glb, TaborSpec};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let spec: TaborSpec = serde_json::from_str(&std::fs::read_to_string(&a[0]).expect("spec")).expect("json");
    let m = generate(&spec);
    std::fs::write(&a[1], to_glb(&m)).expect("glb");
    println!("{}", serde_json::to_string(&m.stats).unwrap());
}
