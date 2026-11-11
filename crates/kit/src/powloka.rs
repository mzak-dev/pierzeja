//! Powłoka: siatka indeksowana (wspólne wierzchołki) z materiałem na trójkąt i gładkimi normalnymi
//! liczonymi z progiem załamania („autosmooth”). Do nadwozi Tabora: siatkę buduje się z rzędów
//! punktów (`grid`), łączy (`weld`), odbija (`mirror_x`), a na końcu wypuszcza do `MeshBuilder`
//! jako części per materiał z normalnymi na wierzchołek.

use crate::mesh::{add, cross, dot, len, norm, scale, sub, MatKey, MeshBuilder, V3};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Powloka {
    pub pos: Vec<V3>,
    pub tris: Vec<[u32; 3]>,
    pub mats: Vec<MatKey>,
    /// Grupa wygładzania trójkąta: normalne uśrednia się tylko w obrębie tej samej grupy.
    pub groups: Vec<u16>,
    group: u16,
}

impl Powloka {
    pub fn vert(&mut self, p: V3) -> u32 {
        self.pos.push(p);
        (self.pos.len() - 1) as u32
    }

    /// Kolejne trójkąty dostają tę grupę wygładzania (np. osobna dla szyb, lamp, felgi).
    pub fn set_group(&mut self, g: u16) {
        self.group = g;
    }

    /// Trójkąt CCW patrząc z zewnątrz; zdegenerowane (pole ~0 albo powtórzony indeks) pomija.
    pub fn tri(&mut self, a: u32, b: u32, c: u32, mat: MatKey) {
        if a == b || b == c || a == c {
            return;
        }
        let (pa, pb, pc) = (self.pos[a as usize], self.pos[b as usize], self.pos[c as usize]);
        if len(cross(sub(pb, pa), sub(pc, pa))) < 1e-10 {
            return;
        }
        self.tris.push([a, b, c]);
        self.mats.push(mat);
        self.groups.push(self.group);
    }

    /// Czworokąt a-b-c-d (CCW); dzielony po krótszej przekątnej.
    pub fn quad(&mut self, a: u32, b: u32, c: u32, d: u32, mat: MatKey) {
        let p = |i: u32| self.pos[i as usize];
        if len(sub(p(a), p(c))) <= len(sub(p(b), p(d))) {
            self.tri(a, b, c, mat);
            self.tri(a, c, d, mat);
        } else {
            self.tri(a, b, d, mat);
            self.tri(b, c, d, mat);
        }
    }

    /// Siatka z rzędów punktów `rows[i][j]` (wszystkie rzędy tej samej długości). Czworokąty
    /// (i,j)-(i,j+1)-(i+1,j+1)-(i+1,j); zwrot normalnej wybiera kolejność: rzędy rosną w kierunku,
    /// przy którym `cross(d_j, d_i)` wskazuje na zewnątrz. `mat(i, j)` = materiał komórki.
    /// Zwraca indeksy wierzchołków (do zszywania z sąsiednimi łatami).
    pub fn grid(&mut self, rows: &[Vec<V3>], mat: impl Fn(usize, usize) -> MatKey) -> Vec<Vec<u32>> {
        let ids: Vec<Vec<u32>> = rows.iter().map(|r| r.iter().map(|&p| self.vert(p)).collect()).collect();
        self.grid_ids(&ids, mat);
        ids
    }

    /// Jak `grid`, ale na gotowych indeksach (wspólne wierzchołki z innymi łatami).
    pub fn grid_ids(&mut self, ids: &[Vec<u32>], mat: impl Fn(usize, usize) -> MatKey) {
        for i in 0..ids.len().saturating_sub(1) {
            let n = ids[i].len().min(ids[i + 1].len());
            for j in 0..n.saturating_sub(1) {
                self.quad(ids[i][j], ids[i][j + 1], ids[i + 1][j + 1], ids[i + 1][j], mat(i, j));
            }
        }
    }

    /// Wachlarz (zamknięcie wielokąta wypukłego) wokół nowego wierzchołka w środku ciężkości.
    pub fn fan(&mut self, ring: &[u32], mat: MatKey) {
        if ring.len() < 3 {
            return;
        }
        let c = scale(ring.iter().fold([0.0; 3], |a, &i| add(a, self.pos[i as usize])), 1.0 / ring.len() as f64);
        let ci = self.vert(c);
        for k in 0..ring.len() {
            self.tri(ci, ring[k], ring[(k + 1) % ring.len()], mat);
        }
    }

    /// Dołącza drugą powłokę (indeksy przesunięte).
    pub fn append(&mut self, o: &Powloka) {
        let base = self.pos.len() as u32;
        self.pos.extend(&o.pos);
        self.tris.extend(o.tris.iter().map(|t| t.map(|i| i + base)));
        self.mats.extend(&o.mats);
        self.groups.extend(&o.groups);
    }

    /// Odbicie względem płaszczyzny x = 0 dopisane do powłoki (zwrot trójkątów odwrócony).
    pub fn mirror_x(&mut self) {
        let base = self.pos.len() as u32;
        let n = self.tris.len();
        self.pos.extend(self.pos.clone().into_iter().map(|p| [-p[0], p[1], p[2]]));
        for k in 0..n {
            let [a, b, c] = self.tris[k];
            self.tris.push([a + base, c + base, b + base]);
            self.mats.push(self.mats[k]);
            self.groups.push(self.groups[k]);
        }
    }

    /// Scala wierzchołki bliższe niż `eps` (np. szew odbicia na x = 0, styki łat).
    pub fn weld(&mut self, eps: f64) {
        let q = |v: f64| (v / eps).round() as i64;
        let mut map: HashMap<[i64; 3], u32> = HashMap::new();
        let mut remap = vec![0u32; self.pos.len()];
        let mut pos = Vec::with_capacity(self.pos.len());
        for (i, p) in self.pos.iter().enumerate() {
            let k = [q(p[0]), q(p[1]), q(p[2])];
            let id = *map.entry(k).or_insert_with(|| {
                pos.push(*p);
                (pos.len() - 1) as u32
            });
            remap[i] = id;
        }
        self.pos = pos;
        let (mut tris, mut mats, mut groups) = (Vec::new(), Vec::new(), Vec::new());
        for (k, t) in self.tris.iter().enumerate() {
            let t = t.map(|i| remap[i as usize]);
            if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
                tris.push(t);
                mats.push(self.mats[k]);
                groups.push(self.groups[k]);
            }
        }
        (self.tris, self.mats, self.groups) = (tris, mats, groups);
    }

    /// Normalna ściany i pole.
    pub fn face(&self, t: usize) -> (V3, f64) {
        let [a, b, c] = self.tris[t].map(|i| self.pos[i as usize]);
        let n = cross(sub(b, a), sub(c, a));
        let l = len(n);
        (scale(n, 1.0 / l.max(1e-30)), l / 2.0)
    }

    /// Krawędzie nieszczelne: występujące w innej liczbie trójkątów niż 2 (otwory, T-złącza).
    /// Kolejność wierzchołków krawędzi nieistotna.
    pub fn open_edges(&self) -> Vec<(u32, u32, usize)> {
        let mut m: HashMap<(u32, u32), usize> = HashMap::new();
        for t in &self.tris {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                *m.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let mut v: Vec<(u32, u32, usize)> = m.into_iter().filter(|&(_, c)| c != 2).map(|((a, b), c)| (a, b, c)).collect();
        v.sort();
        v
    }

    /// Wypuszcza powłokę do `MeshBuilder`: normalne wierzchołków uśrednione (ważone kątem) po
    /// trójkątach tej samej grupy, których normalne różnią się o mniej niż `crease_deg`.
    pub fn emit(&self, mb: &mut MeshBuilder, crease_deg: f64) {
        let cos_lim = crease_deg.to_radians().cos();
        let faces: Vec<(V3, f64)> = (0..self.tris.len()).map(|t| self.face(t)).collect();
        // trójkąty przy wierzchołku z kątem w tym wierzchołku
        let mut inc: Vec<Vec<(usize, f64)>> = vec![Vec::new(); self.pos.len()];
        for (t, tri) in self.tris.iter().enumerate() {
            for k in 0..3 {
                let (p, a, b) = (self.pos[tri[k] as usize], self.pos[tri[(k + 1) % 3] as usize], self.pos[tri[(k + 2) % 3] as usize]);
                let (u, v) = (norm(sub(a, p)), norm(sub(b, p)));
                inc[tri[k] as usize].push((t, dot(u, v).clamp(-1.0, 1.0).acos()));
            }
        }
        // części per materiał; wierzchołek wyjściowy = (wierzchołek, zaokrąglona normalna)
        let mut parts: Vec<(MatKey, Vec<V3>, Vec<V3>, Vec<u32>, HashMap<(u32, [i32; 3]), u32>)> = Vec::new();
        for (t, tri) in self.tris.iter().enumerate() {
            let mat = self.mats[t];
            let pi = match parts.iter().position(|p| p.0 == mat) {
                Some(i) => i,
                None => {
                    parts.push((mat, Vec::new(), Vec::new(), Vec::new(), HashMap::new()));
                    parts.len() - 1
                }
            };
            let (nt, _) = faces[t];
            for &vi in tri {
                let mut acc = [0.0; 3];
                for &(u, ang) in &inc[vi as usize] {
                    if self.groups[u] != self.groups[t] {
                        continue;
                    }
                    let (nu, _) = faces[u];
                    if dot(nu, nt) >= cos_lim {
                        acc = add(acc, scale(nu, ang));
                    }
                }
                let n = if len(acc) < 1e-12 { nt } else { norm(acc) };
                let key = (vi, n.map(|c| (c * 4096.0).round() as i32));
                let part = &mut parts[pi];
                let id = match part.4.get(&key) {
                    Some(&id) => id,
                    None => {
                        part.1.push(self.pos[vi as usize]);
                        part.2.push(n);
                        let id = (part.1.len() - 1) as u32;
                        part.4.insert(key, id);
                        id
                    }
                };
                part.3.push(id);
            }
        }
        for (mat, pos, nrm, idx, _) in parts {
            mb.push_indexed(mat, &pos, &nrm, &idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_box_is_watertight_and_smooth_only_below_crease() {
        // sześcian z 6 czworokątów na wspólnych wierzchołkach
        let mut p = Powloka::default();
        let v: Vec<u32> = (0..8).map(|i| p.vert([(i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64])).collect();
        for f in [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]] {
            p.quad(v[f[0]], v[f[1]], v[f[2]], v[f[3]], MatKey::Paint);
        }
        assert!(p.open_edges().is_empty());
        let mut mb = MeshBuilder::default();
        p.emit(&mut mb, 40.0);
        let m = mb.build(vec![]);
        // 90° > 40°: każda ściana ma własne normalne, 6 ścian × 4 wierzchołki
        assert_eq!(m.parts[0].positions.len() / 3, 24);
        let mut mb = MeshBuilder::default();
        p.emit(&mut mb, 100.0);
        // wszystko gładkie: 8 wierzchołków
        assert_eq!(mb.build(vec![]).parts[0].positions.len() / 3, 8);
    }

    #[test]
    fn mirror_and_weld_close_the_seam() {
        let mut p = Powloka::default();
        let ids = p.grid(&[vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], vec![[0.0, 1.0, 0.0], [1.0, 1.0, 0.0]]], |_, _| MatKey::Paint);
        assert_eq!(ids.len(), 2);
        p.mirror_x();
        p.weld(1e-6);
        assert_eq!(p.pos.len(), 6);
        assert_eq!(p.tris.len(), 4);
    }
}
