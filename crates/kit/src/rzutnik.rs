//! Rzutnik naklejek: wielokąt narysowany w płaszczyźnie widoku (przód, tył, bok, góra) dzielony
//! na drobną siatkę i rzutowany promieniem na powłokę. Wynik przylega do krzywizny i dziedziczy
//! jej gładkie normalne — lampy, grill, szczeliny, napisy i pasy oklejenia bez warstwy „w powietrzu”.

use crate::mesh::{add, cross, dot, len, norm, scale, sub, MatKey, MeshBuilder, V3};
use crate::powloka::Powloka;

/// Kierunek patrzenia: promień biegnie od strony `+os·znak` w głąb pojazdu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Widok {
    /// Od przodu (+Z): mapa (x, y).
    Przod,
    /// Od tyłu (−Z): mapa (x, y).
    Tyl,
    /// Z lewej (+X): mapa (z, y).
    Lewy,
    /// Z prawej (−X): mapa (z, y).
    Prawy,
    /// Z góry (+Y): mapa (x, z).
    Gora,
}

impl Widok {
    fn axis(self) -> (usize, f64) {
        match self {
            Widok::Przod => (2, 1.0),
            Widok::Tyl => (2, -1.0),
            Widok::Lewy => (0, 1.0),
            Widok::Prawy => (0, -1.0),
            Widok::Gora => (1, 1.0),
        }
    }
    /// Współrzędne mapy (a, b) punktu.
    fn chart(self, p: V3) -> [f64; 2] {
        match self.axis().0 {
            2 => [p[0], p[1]],
            0 => [p[2], p[1]],
            _ => [p[0], p[2]],
        }
    }
}

const NB: usize = 48;

#[derive(Debug, Clone)]
pub struct Rzutnik {
    widok: Widok,
    tris: Vec<[V3; 3]>,
    nrm: Vec<[V3; 3]>,
    lo: [f64; 2],
    cell: [f64; 2],
    buckets: Vec<Vec<u32>>,
}

/// Trafienie promienia: punkt na powłoce i gładka normalna.
#[derive(Debug, Clone, Copy)]
pub struct Trafienie {
    pub p: V3,
    pub n: V3,
}

impl Rzutnik {
    /// Rzutnik dla powłoki; `crease_deg` jak w `Powloka::emit` (te same normalne).
    pub fn new(pw: &Powloka, crease_deg: f64, widok: Widok) -> Rzutnik {
        Rzutnik::limited(pw, crease_deg, widok, f64::NEG_INFINITY)
    }

    /// Jak `new`, ale widzi tylko trójkąty sięgające głębokości `min_depth` (współrzędna osi widoku
    /// razy jej zwrot): promień, który minie zewnętrzną warstwę, nie trafi we wnękę koła czy podłogę.
    pub fn limited(pw: &Powloka, crease_deg: f64, widok: Widok, min_depth: f64) -> Rzutnik {
        let cn = pw.corner_normals(crease_deg);
        let (ax, sg) = widok.axis();
        let mut tris = Vec::new();
        let mut nrm = Vec::new();
        for (t, tri) in pw.tris.iter().enumerate() {
            let (fnrm, area) = pw.face(t);
            if area < 1e-12 || fnrm[ax] * sg < 0.02 || tri.iter().all(|&i| pw.pos[i as usize][ax] * sg < min_depth) {
                continue;
            }
            tris.push(tri.map(|i| pw.pos[i as usize]));
            nrm.push(cn[t]);
        }
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for t in &tris {
            for p in t {
                let c = widok.chart(*p);
                for k in 0..2 {
                    lo[k] = lo[k].min(c[k]);
                    hi[k] = hi[k].max(c[k]);
                }
            }
        }
        if !lo[0].is_finite() {
            (lo, hi) = ([0.0; 2], [1.0; 2]);
        }
        let cell = [((hi[0] - lo[0]) / NB as f64).max(1e-6), ((hi[1] - lo[1]) / NB as f64).max(1e-6)];
        let mut buckets = vec![Vec::new(); NB * NB];
        let idx = |v: f64, k: usize| (((v - lo[k]) / cell[k]).floor().max(0.0) as usize).min(NB - 1);
        for (i, t) in tris.iter().enumerate() {
            let c: Vec<[f64; 2]> = t.iter().map(|p| widok.chart(*p)).collect();
            let (a0, a1) = (c.iter().map(|q| q[0]).fold(f64::INFINITY, f64::min), c.iter().map(|q| q[0]).fold(f64::NEG_INFINITY, f64::max));
            let (b0, b1) = (c.iter().map(|q| q[1]).fold(f64::INFINITY, f64::min), c.iter().map(|q| q[1]).fold(f64::NEG_INFINITY, f64::max));
            for u in idx(a0, 0)..=idx(a1, 0) {
                for v in idx(b0, 1)..=idx(b1, 1) {
                    buckets[v * NB + u].push(i as u32);
                }
            }
        }
        Rzutnik { widok, tris, nrm, lo, cell, buckets }
    }

    /// Pierwsze trafienie (najbliżej obserwatora) promienia w punkcie mapy (a, b).
    pub fn hit(&self, a: f64, b: f64) -> Option<Trafienie> {
        let (ax, sg) = self.widok.axis();
        let (u, v) = (((a - self.lo[0]) / self.cell[0]).floor(), ((b - self.lo[1]) / self.cell[1]).floor());
        if u < 0.0 || v < 0.0 || u >= NB as f64 || v >= NB as f64 {
            return None;
        }
        let mut best: Option<(f64, Trafienie)> = None;
        for &ti in &self.buckets[v as usize * NB + u as usize] {
            let t = &self.tris[ti as usize];
            let c = t.map(|p| self.widok.chart(p));
            let d = (c[1][0] - c[0][0]) * (c[2][1] - c[0][1]) - (c[2][0] - c[0][0]) * (c[1][1] - c[0][1]);
            if d.abs() < 1e-14 {
                continue;
            }
            let l1 = ((a - c[0][0]) * (c[2][1] - c[0][1]) - (c[2][0] - c[0][0]) * (b - c[0][1])) / d;
            let l2 = ((c[1][0] - c[0][0]) * (b - c[0][1]) - (a - c[0][0]) * (c[1][1] - c[0][1])) / d;
            let l0 = 1.0 - l1 - l2;
            let e = -1e-9;
            if l0 < e || l1 < e || l2 < e {
                continue;
            }
            let p = add(add(scale(t[0], l0), scale(t[1], l1)), scale(t[2], l2));
            let depth = p[ax] * sg;
            if best.is_none_or(|(bd, _)| depth > bd) {
                let n = &self.nrm[ti as usize];
                let nn = norm(add(add(scale(n[0], l0), scale(n[1], l1)), scale(n[2], l2)));
                best = Some((depth, Trafienie { p, n: nn }));
            }
        }
        best.map(|b| b.1)
    }

    /// Naklejka: wypukły wielokąt mapy wypełniony siatką o boku `step`, rzutowany na powłokę
    /// i odsunięty o `lift` wzdłuż normalnej. Komórki, które nie trafiają w powłokę, są pomijane.
    pub fn patch(&self, mb: &mut MeshBuilder, mat: MatKey, poly: &[[f64; 2]], step: f64, lift: f64) {
        if poly.len() < 3 {
            return;
        }
        let poly = ccw(poly);
        let (a0, a1) = (poly.iter().map(|q| q[0]).fold(f64::INFINITY, f64::min), poly.iter().map(|q| q[0]).fold(f64::NEG_INFINITY, f64::max));
        let (b0, b1) = (poly.iter().map(|q| q[1]).fold(f64::INFINITY, f64::min), poly.iter().map(|q| q[1]).fold(f64::NEG_INFINITY, f64::max));
        let (na, nb) = (((a1 - a0) / step).ceil().max(1.0) as usize, ((b1 - b0) / step).ceil().max(1.0) as usize);
        let (da, db) = ((a1 - a0) / na as f64, (b1 - b0) / nb as f64);
        let (ax, sg) = self.widok.axis();
        let mut toward = [0.0; 3];
        toward[ax] = sg;
        for i in 0..na {
            for j in 0..nb {
                let (x0, y0) = (a0 + i as f64 * da, b0 + j as f64 * db);
                let cell = [[x0, y0], [x0 + da, y0], [x0 + da, y0 + db], [x0, y0 + db]];
                let c = clip(&cell, &poly);
                if c.len() < 3 {
                    continue;
                }
                let hits: Option<Vec<Trafienie>> = c.iter().map(|q| self.hit(q[0], q[1])).collect();
                let Some(hits) = hits else { continue };
                let pos: Vec<V3> = hits.iter().map(|h| add(h.p, scale(h.n, lift))).collect();
                let nrm: Vec<V3> = hits.iter().map(|h| h.n).collect();
                // trójkąty CCW patrząc od obserwatora
                let mut idx = Vec::new();
                for k in 1..pos.len() as u32 - 1 {
                    let (p0, p1, p2) = (pos[0], pos[k as usize], pos[k as usize + 1]);
                    let f = cross(sub(p1, p0), sub(p2, p0));
                    if len(f) < 1e-14 {
                        continue;
                    }
                    if dot(f, toward) >= 0.0 {
                        idx.extend([0, k, k + 1]);
                    } else {
                        idx.extend([0, k + 1, k]);
                    }
                }
                mb.push_indexed(mat, &pos, &nrm, &idx);
            }
        }
    }
}

/// Wielokąt ustawiony przeciwnie do ruchu wskazówek zegara.
fn ccw(p: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let a: f64 = (0..p.len()).map(|i| {
        let (q, r) = (p[i], p[(i + 1) % p.len()]);
        q[0] * r[1] - r[0] * q[1]
    }).sum();
    if a < 0.0 { p.iter().rev().copied().collect() } else { p.to_vec() }
}

/// Sutherland–Hodgman: `subject` przycięty do wypukłego `clipper` (oba CCW).
pub fn clip(subject: &[[f64; 2]], clipper: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out = subject.to_vec();
    for i in 0..clipper.len() {
        if out.is_empty() {
            break;
        }
        let (a, b) = (clipper[i], clipper[(i + 1) % clipper.len()]);
        let inside = |p: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) >= -1e-12;
        let inter = |p: [f64; 2], q: [f64; 2]| {
            let (d1, d2) = ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]), (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]));
            let t = d1 / (d1 - d2);
            [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
        };
        let input = std::mem::take(&mut out);
        for k in 0..input.len() {
            let (p, q) = (input[k], input[(k + 1) % input.len()]);
            match (inside(p), inside(q)) {
                (true, true) => out.push(q),
                (true, false) => out.push(inter(p, q)),
                (false, true) => {
                    out.push(inter(p, q));
                    out.push(q);
                }
                (false, false) => {}
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_onto_front_face_of_a_box() {
        // płyta 2×2 w z = 1 zwrócona do +Z
        let mut pw = Powloka::default();
        pw.grid(&[vec![[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0]], vec![[-1.0, 1.0, 1.0], [1.0, 1.0, 1.0]]], |_, _| MatKey::Paint);
        let r = Rzutnik::new(&pw, 30.0, Widok::Przod);
        let h = r.hit(0.2, 0.3).expect("trafienie");
        assert!((h.p[2] - 1.0).abs() < 1e-9 && h.n[2] > 0.99);
        assert!(r.hit(3.0, 0.0).is_none());
        let mut mb = MeshBuilder::default();
        r.patch(&mut mb, MatKey::Headlight, &[[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]], 0.25, 0.01);
        let m = mb.build(vec![]);
        assert_eq!(m.triangles(), 32);
        assert!(m.parts[0].positions.chunks(3).all(|p| (p[2] - 1.01).abs() < 1e-6));
    }
}
