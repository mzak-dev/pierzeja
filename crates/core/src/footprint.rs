//! Obrysy wielokątne (np. z OSM): normalizacja, odsunięcia (inset), triangulacja.
//! Punkty to [x, z] w metrach; budynek stoi na y = 0, ulica domyślnie po stronie +z.

use serde::{Deserialize, Serialize};

pub type P2 = [f64; 2];

/// Obrys budynku podany w spec. Krawędź `i` = `points[i] → points[i+1]` (zamknięta).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Footprint {
    pub points: Vec<P2>,
    /// Elewacje frontowe (od ulicy): wejście, witryny, pełny detal. Puste = krawędź zwrócona najbardziej ku +z.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub front: Vec<usize>,
    /// Ściany graniczne (styk z sąsiadem w pierzei): bez okien i detalu.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blind: Vec<usize>,
}

/// Rola krawędzi po normalizacji.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EdgeKind {
    Front,
    Blind,
    Other,
}

/// Pole ze znakiem (shoelace w płaszczyźnie x–z).
pub fn signed_area(p: &[P2]) -> f64 {
    let n = p.len();
    (0..n).map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1]).sum::<f64>() / 2.0
}

pub fn area(p: &[P2]) -> f64 {
    signed_area(p).abs()
}

pub fn edge_len(p: &[P2], i: usize) -> f64 {
    let (a, b) = (p[i], p[(i + 1) % p.len()]);
    (b[0] - a[0]).hypot(b[1] - a[1])
}

/// Normalna zewnętrzna krawędzi (dla obiegu znormalizowanego: n = dir × UP).
pub fn edge_normal(p: &[P2], i: usize) -> P2 {
    let (a, b) = (p[i], p[(i + 1) % p.len()]);
    let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-12);
    [-(b[1] - a[1]) / l, (b[0] - a[0]) / l]
}

/// Obieg zgodny z resztą generatora (pole shoelace < 0, normalne na zewnątrz), bez
/// duplikatów i punktów współliniowych. Zwraca punkty i rolę każdej krawędzi.
pub fn normalize(fp: &Footprint) -> Option<(Vec<P2>, Vec<EdgeKind>)> {
    // role przypisane do krawędzi wejściowych
    let n0 = fp.points.len();
    if n0 < 3 {
        return None;
    }
    let mut pts: Vec<(P2, EdgeKind)> = (0..n0)
        .map(|i| {
            let k = if fp.blind.contains(&i) { EdgeKind::Blind } else if fp.front.contains(&i) { EdgeKind::Front } else { EdgeKind::Other };
            (fp.points[i], k)
        })
        .collect();
    // usuń duplikaty sąsiednich punktów
    let close = |a: P2, b: P2| (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6;
    let mut i = 0;
    while pts.len() > 2 && i < pts.len() {
        let j = (i + 1) % pts.len();
        if close(pts[i].0, pts[j].0) {
            pts.remove(j);
        } else {
            i += 1;
        }
    }
    // usuń punkty współliniowe (krawędź dalej ma rolę poprzedniej)
    let mut changed = true;
    while changed && pts.len() > 3 {
        changed = false;
        for i in 0..pts.len() {
            let n = pts.len();
            let (a, b, c) = (pts[(i + n - 1) % n].0, pts[i].0, pts[(i + 1) % n].0);
            let cr = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
            let la = (b[0] - a[0]).hypot(b[1] - a[1]) * (c[0] - b[0]).hypot(c[1] - b[1]);
            if cr.abs() <= 1e-9 * la.max(1e-12) && (b[0] - a[0]) * (c[0] - b[0]) + (b[1] - a[1]) * (c[1] - b[1]) > 0.0 {
                let k = pts[(i + n - 1) % n].1;
                pts.remove(i);
                let m = pts.len();
                pts[(i + m - 1) % m].1 = k;
                changed = true;
                break;
            }
        }
    }
    if pts.len() < 3 {
        return None;
    }
    let mut p: Vec<P2> = pts.iter().map(|x| x.0).collect();
    let mut k: Vec<EdgeKind> = pts.iter().map(|x| x.1).collect();
    if signed_area(&p).abs() < 1.0 || !is_simple(&p) {
        return None;
    }
    if signed_area(&p) > 0.0 {
        // odwrócenie: krawędź i (p_i→p_i+1) staje się krawędzią n−2−i (mod n)
        let n = p.len();
        p.reverse();
        let old = k.clone();
        for (i, kk) in k.iter_mut().enumerate() {
            *kk = old[(2 * n - 2 - i) % n];
        }
    }
    if !k.contains(&EdgeKind::Front) {
        // domyślnie front = krawędź zwrócona najbardziej ku ulicy (+z), przy remisie dłuższa
        let best = (0..p.len())
            .filter(|&i| k[i] != EdgeKind::Blind)
            .max_by(|&a, &b| (edge_normal(&p, a)[1] + 1e-3 * edge_len(&p, a)).total_cmp(&(edge_normal(&p, b)[1] + 1e-3 * edge_len(&p, b))));
        if let Some(b) = best {
            k[b] = EdgeKind::Front;
        }
    }
    Some((p, k))
}

fn seg_cross(a: P2, b: P2, c: P2, d: P2) -> bool {
    let o = |p: P2, q: P2, r: P2| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) && d1 != 0.0 && d2 != 0.0 && d3 != 0.0 && d4 != 0.0
}

/// Brak przecięć niesąsiednich krawędzi.
pub fn is_simple(p: &[P2]) -> bool {
    let n = p.len();
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            if seg_cross(p[i], p[(i + 1) % n], p[j], p[(j + 1) % n]) {
                return false;
            }
        }
    }
    true
}

/// Odsunięcie krawędzi do wnętrza o `d` (ujemne = na zewnątrz), wierzchołki z „miter”.
/// `None`, gdy któraś krawędź się odwraca/zanika albo obrys przestaje być prosty —
/// to pierwsze „zdarzenie” szkieletu prostego.
pub fn inset(p: &[P2], d: f64) -> Option<Vec<P2>> {
    let n = p.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let n0 = edge_normal(p, (i + n - 1) % n);
        let n1 = edge_normal(p, i);
        let k = 1.0 + n0[0] * n1[0] + n0[1] * n1[1];
        if k < 0.05 {
            return None; // szpic ~180°
        }
        out.push([p[i][0] - (n0[0] + n1[0]) / k * d, p[i][1] - (n0[1] + n1[1]) / k * d]);
    }
    for i in 0..n {
        let j = (i + 1) % n;
        let (od, nd) = ([p[j][0] - p[i][0], p[j][1] - p[i][1]], [out[j][0] - out[i][0], out[j][1] - out[i][1]]);
        if od[0] * nd[0] + od[1] * nd[1] <= 1e-6 {
            return None;
        }
    }
    if signed_area(&out) >= -1e-6 || !is_simple(&out) {
        return None;
    }
    Some(out)
}

/// Największe poprawne odsunięcie do wnętrza (bisekcja) — „połowa szerokości” obrysu.
pub fn max_inset(p: &[P2]) -> f64 {
    let (mut lo, mut hi) = (0.0, 0.0f64);
    let (xs, zs): (Vec<f64>, Vec<f64>) = p.iter().map(|q| (q[0], q[1])).unzip();
    let w = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let h = zs.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - zs.iter().cloned().fold(f64::INFINITY, f64::min);
    hi = hi.max(0.5 * w.min(h) + 0.01);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if inset(p, mid).is_some() { lo = mid } else { hi = mid }
    }
    lo
}

pub fn centroid(p: &[P2]) -> P2 {
    let a = signed_area(p);
    if a.abs() < 1e-9 {
        let n = p.len() as f64;
        return [p.iter().map(|q| q[0]).sum::<f64>() / n, p.iter().map(|q| q[1]).sum::<f64>() / n];
    }
    let n = p.len();
    let (mut cx, mut cz) = (0.0, 0.0);
    for i in 0..n {
        let (a0, b0) = (p[i], p[(i + 1) % n]);
        let c = a0[0] * b0[1] - b0[0] * a0[1];
        cx += (a0[0] + b0[0]) * c;
        cz += (a0[1] + b0[1]) * c;
    }
    [cx / (6.0 * a), cz / (6.0 * a)]
}

pub fn contains(p: &[P2], q: P2) -> bool {
    let n = p.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
    }
    inside
}

/// Triangulacja przez obcinanie uszu (wielokąt prosty, dowolny obieg). Indeksy trójkątów.
pub fn triangulate(p: &[P2]) -> Vec<[usize; 3]> {
    let n = p.len();
    let mut idx: Vec<usize> = (0..n).collect();
    let sign = if signed_area(p) > 0.0 { 1.0 } else { -1.0 };
    let cross = |a: P2, b: P2, c: P2| ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) * sign;
    let inside = |a: P2, b: P2, c: P2, q: P2| cross(a, b, q) > 1e-12 && cross(b, c, q) > 1e-12 && cross(c, a, q) > 1e-12;
    let mut tris = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut cut = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            if cross(p[ia], p[ib], p[ic]) <= 1e-12 {
                continue;
            }
            if idx.iter().any(|&j| j != ia && j != ib && j != ic && inside(p[ia], p[ib], p[ic], p[j])) {
                continue;
            }
            tris.push([ia, ib, ic]);
            idx.remove(i);
            cut = true;
            break;
        }
        if !cut {
            // zdegenerowane resztki: wachlarz (ponytail: wystarcza dla odcinków współliniowych)
            break;
        }
    }
    for k in 1..idx.len().saturating_sub(1) {
        tris.push([idx[0], idx[k], idx[k + 1]]);
    }
    tris
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_inset_and_triangles() {
        let r = vec![[0.0, 10.0], [20.0, 10.0], [20.0, 0.0], [0.0, 0.0]];
        let (p, k) = normalize(&Footprint { points: r, ..Default::default() }).unwrap();
        assert!(signed_area(&p) < 0.0);
        assert_eq!(k.iter().filter(|x| **x == EdgeKind::Front).count(), 1);
        assert_eq!(edge_normal(&p, k.iter().position(|x| *x == EdgeKind::Front).unwrap()), [0.0, 1.0]);
        assert!((max_inset(&p) - 5.0).abs() < 1e-3);
        assert_eq!(triangulate(&p).len(), 2);
    }

    #[test]
    fn l_shape_reversed_keeps_roles() {
        // obieg przeciwny, front = krawędź 0 (dolna, z = 0 → normalna −z po normalizacji)
        let l = vec![[0.0, 0.0], [20.0, 0.0], [20.0, 8.0], [8.0, 8.0], [8.0, 20.0], [0.0, 20.0]];
        let (p, k) = normalize(&Footprint { points: l, front: vec![0], blind: vec![1] }).unwrap();
        let f = k.iter().position(|x| *x == EdgeKind::Front).unwrap();
        let b = k.iter().position(|x| *x == EdgeKind::Blind).unwrap();
        assert!((edge_len(&p, f) - 20.0).abs() < 1e-9 && edge_normal(&p, f) == [0.0, -1.0]);
        assert!((edge_len(&p, b) - 8.0).abs() < 1e-9 && edge_normal(&p, b) == [1.0, 0.0]);
        assert_eq!(triangulate(&p).len(), 4);
        assert!((max_inset(&p) - 4.0).abs() < 0.05, "{}", max_inset(&p));
    }
}
