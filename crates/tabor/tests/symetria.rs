//! Nadwozie cywilnego pojazdu musi być lustrzane względem płaszczyzny YZ (x → −x):
//! środek każdego trójkąta po odbiciu leży na powierzchni oryginału. Tolerancja 5 mm przepuszcza
//! nakładki z natury niesymetryczne (pasek UE tablicy), a 50 mm w vanie — drzwi przesuwne tylko
//! po prawej i listwa tylko po lewej (wystają 3 cm z pochylonej burty). Błąd lofta sprzed poprawki miał 64–105 mm.
//! Porównanie powierzchni, nie trójkątów: płaski czworokąt wolno podzielić inną przekątną.
use tabor_core::{generate, TaborSpec};

type V = [f64; 3];
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Odległość punktu od trójkąta (Ericson, „Real-Time Collision Detection” 5.1.5).
fn dist(p: V, [a, b, c]: [V; 3]) -> f64 {
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    let at = |v: f64, w: f64| [a[0] + ab[0] * v + ac[0] * w, a[1] + ab[1] * v + ac[1] * w, a[2] + ab[2] * v + ac[2] * w];
    let q = 'q: {
        if d1 <= 0.0 && d2 <= 0.0 {
            break 'q a;
        }
        let bp = sub(p, b);
        let (d3, d4) = (dot(ab, bp), dot(ac, bp));
        if d3 >= 0.0 && d4 <= d3 {
            break 'q b;
        }
        let vc = d1 * d4 - d3 * d2;
        if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
            break 'q at(d1 / (d1 - d3), 0.0);
        }
        let cp = sub(p, c);
        let (d5, d6) = (dot(ab, cp), dot(ac, cp));
        if d6 >= 0.0 && d5 <= d6 {
            break 'q c;
        }
        let vb = d5 * d2 - d1 * d6;
        if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
            break 'q at(0.0, d2 / (d2 - d6));
        }
        let va = d3 * d6 - d5 * d4;
        if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
            let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
            break 'q at(1.0 - w, w);
        }
        let den = 1.0 / (va + vb + vc);
        at(vb * den, vc * den)
    };
    let d = sub(p, q);
    dot(d, d).sqrt()
}

fn tris(m: &procedura_kit::mesh::MeshData) -> Vec<([V; 3], String)> {
    let mut out = Vec::new();
    for part in &m.parts {
        let v = |i: u32| -> V { std::array::from_fn(|k| part.positions[i as usize * 3 + k] as f64) };
        let mat = format!("{:?}", part.material);
        out.extend(part.indices.chunks_exact(3).map(|t| ([v(t[0]), v(t[1]), v(t[2])], mat.clone())));
    }
    out
}

#[test]
fn nadwozie_jest_lustrzane() {
    let mut fails = 0;
    for (a, tol) in [("hatchback", 0.005), ("sedan_liftback", 0.005), ("kombi", 0.005), ("suv", 0.005), ("van", 0.05)] {
        let spec: TaborSpec = serde_json::from_str(&format!(r#"{{"archetype":"{a}","seed":1,"overrides":{{"service":"cywilny"}}}}"#)).unwrap();
        let m = generate(&spec);
        let t = tris(&m.body);
        let off: Vec<f64> = t
            .iter()
            .map(|(tri, _)| {
                let c: V = std::array::from_fn(|k| (tri[0][k] + tri[1][k] + tri[2][k]) / 3.0);
                let c = [-c[0], c[1], c[2]];
                t.iter().map(|o| dist(c, o.0)).fold(f64::MAX, f64::min)
            })
            .collect();
        let bad = off.iter().filter(|&&d| d > tol).count();
        let (wi, worst) = off.iter().copied().enumerate().fold((0, 0.0), |b, (i, d)| if d > b.1 { (i, d) } else { b });
        let wc: V = std::array::from_fn(|k| (t[wi].0[0][k] + t[wi].0[1][k] + t[wi].0[2][k]) / 3.0);
        println!("{a}: nadwozie {} tr., koło {} tr. ×4, niesymetrycznych {bad}, największy błąd {:.0} mm ({} w {wc:.3?})", t.len(), m.wheel.triangles(), worst * 1000.0, t[wi].1);
        fails += (bad > 0) as usize;
    }
    assert_eq!(fails, 0, "nadwozia bez symetrii lustrzanej");
}
