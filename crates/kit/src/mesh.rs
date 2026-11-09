//! Budowa siatek: „flat shaded” wielokąty grupowane per materiał, UV w metrach.

use serde::Serialize;

pub type V3 = [f64; 3];

pub const UP: V3 = [0.0, 1.0, 0.0];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub fn len(a: V3) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}
pub fn norm(a: V3) -> V3 {
    let l = len(a);
    let l = if l == 0.0 || l.is_nan() { 1.0 } else { l };
    [a[0] / l, a[1] / l, a[2] / l]
}

/// Lokalny układ elewacji: u w prawo (patrząc z zewnątrz), v w górę, n na zewnątrz.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub o: V3,
    pub u: V3,
    pub v: V3,
    pub n: V3,
}

pub fn frame_at(o: V3, n: V3) -> Frame {
    Frame { o, n, v: UP, u: cross(UP, n) }
}

pub fn to_world(f: &Frame, a: f64, b: f64, c: f64) -> V3 {
    [
        f.o[0] + f.u[0] * a + f.v[0] * b + f.n[0] * c,
        f.o[1] + f.u[1] * a + f.v[1] * b + f.n[1] * c,
        f.o[2] + f.u[2] * a + f.v[2] * b + f.n[2] * c,
    ]
}

/// Przesunięcie układu wzdłuż normalnej (wcięcie loggii, podcienia, wykusz).
pub fn shift_frame(f: &Frame, c: f64) -> Frame {
    Frame { o: to_world(f, 0.0, 0.0, c), ..*f }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatKey {
    Wall,
    Accent,
    Trim,
    Plinth,
    Roof,
    Glass,
    GlassLit,
    Frame,
    Slab,
    Metal,
    Railing,
    GlassRail,
    Door,
    Joint,
    Ground,
    Road,
    Sidewalk,
    // Pojazdy (Tabor); `ALL` zostaje listą materiałów budynku.
    Paint,
    RoofPaint,
    Bumper,
    Plastic,
    Tyre,
    Rim,
    Headlight,
    Taillight,
    /// Kierunkowskazy (pomarańczowe, Regulamin EKG ONZ nr 48).
    Indicator,
    /// Tablica rejestracyjna: tło i niebieski pasek UE.
    Plate,
    PlateBand,
    // Ludzie (Tłum): sloty zgodne z CharacterLibrary (Skin/Top/Bottom/Shoes) + dodatki.
    Skin,
    Top,
    Bottom,
    Shoes,
    Hair,
    Outer,
    HiVis,
    Helmet,
    Face,
}

impl MatKey {
    pub const ALL: [MatKey; 17] = [
        MatKey::Wall, MatKey::Accent, MatKey::Trim, MatKey::Plinth, MatKey::Roof, MatKey::Glass, MatKey::GlassLit, MatKey::Frame, MatKey::Slab,
        MatKey::Metal, MatKey::Railing, MatKey::GlassRail, MatKey::Door, MatKey::Joint, MatKey::Ground, MatKey::Road, MatKey::Sidewalk,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            MatKey::Wall => "wall",
            MatKey::Accent => "accent",
            MatKey::Trim => "trim",
            MatKey::Plinth => "plinth",
            MatKey::Roof => "roof",
            MatKey::Glass => "glass",
            MatKey::GlassLit => "glassLit",
            MatKey::Frame => "frame",
            MatKey::Slab => "slab",
            MatKey::Metal => "metal",
            MatKey::Railing => "railing",
            MatKey::GlassRail => "glassRail",
            MatKey::Door => "door",
            MatKey::Joint => "joint",
            MatKey::Ground => "ground",
            MatKey::Road => "road",
            MatKey::Sidewalk => "sidewalk",
            MatKey::Paint => "paint",
            MatKey::RoofPaint => "roofPaint",
            MatKey::Bumper => "bumper",
            MatKey::Plastic => "plastic",
            MatKey::Tyre => "tyre",
            MatKey::Rim => "rim",
            MatKey::Headlight => "headlight",
            MatKey::Taillight => "taillight",
            MatKey::Indicator => "indicator",
            MatKey::Plate => "plate",
            MatKey::PlateBand => "plateBand",
            MatKey::Skin => "Skin",
            MatKey::Top => "Top",
            MatKey::Bottom => "Bottom",
            MatKey::Shoes => "Shoes",
            MatKey::Hair => "Hair",
            MatKey::Outer => "Outer",
            MatKey::HiVis => "HiVis",
            MatKey::Helmet => "Helmet",
            MatKey::Face => "Face",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialSpec {
    pub key: MatKey,
    pub name: &'static str,
    /// sRGB #rrggbb.
    pub color: String,
    pub roughness: f64,
    pub metalness: f64,
    pub opacity: f64,
    pub double_sided: bool,
    pub texture: &'static str,
    /// Rozmiar kafla tekstury w metrach.
    pub texture_scale: f64,
    pub emissive: Option<&'static str>,
}

#[derive(Debug, Clone, Default)]
pub struct MeshPart {
    pub material: Option<MatKey>,
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Vec<f32>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Bounds {
    pub min: V3,
    pub max: V3,
}

#[derive(Debug, Clone)]
pub struct MeshData {
    pub parts: Vec<MeshPart>,
    pub materials: Vec<MaterialSpec>,
    pub bounds: Bounds,
}

impl MeshData {
    /// Pusta siatka (węzły-kotwice w GLB).
    pub fn empty() -> MeshData {
        MeshData { parts: vec![], materials: vec![], bounds: Bounds { min: [0.0; 3], max: [0.0; 3] } }
    }
    pub fn material(&self, k: MatKey) -> Option<&MaterialSpec> {
        self.materials.iter().find(|m| m.key == k)
    }
    pub fn triangles(&self) -> usize {
        self.parts.iter().map(|p| p.indices.len() / 3).sum()
    }

    /// Kopia ustawiona w świecie: lokalne +x → `d`, lokalne +z (front, ulica) → `n`
    /// (`d` = (n.z, −n.x), czyli obrót wokół Y), potem przesunięcie o `origin`.
    pub fn placed(&self, n: [f64; 2], origin: V3) -> MeshData {
        let d = [n[1], -n[0]];
        let rot = |x: f64, z: f64| [x * d[0] + z * n[0], x * d[1] + z * n[1]];
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        let parts = self
            .parts
            .iter()
            .map(|p| {
                let mut q = p.clone();
                for v in q.positions.chunks_exact_mut(3) {
                    let [x, z] = rot(v[0] as f64, v[2] as f64);
                    let w = [x + origin[0], v[1] as f64 + origin[1], z + origin[2]];
                    for k in 0..3 {
                        lo[k] = lo[k].min(w[k]);
                        hi[k] = hi[k].max(w[k]);
                    }
                    v.copy_from_slice(&w.map(|c| c as f32));
                }
                for v in q.normals.chunks_exact_mut(3) {
                    let [x, z] = rot(v[0] as f64, v[2] as f64);
                    v[0] = x as f32;
                    v[2] = z as f32;
                }
                // ponytail: UV zostają w układzie lokalnym budynku (kafel tekstury i tak w metrach)
                q
            })
            .collect();
        MeshData { parts, materials: self.materials.clone(), bounds: Bounds { min: lo, max: hi } }
    }
}

/// Przekształcenie afiniczne 3×4: obrót (macierz wierszowa) + przesunięcie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xform {
    pub m: [[f64; 3]; 3],
    pub t: V3,
}

impl Xform {
    pub const fn identity() -> Xform {
        Xform { m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], t: [0.0; 3] }
    }
    pub fn translate(t: V3) -> Xform {
        Xform { t, ..Xform::identity() }
    }
    pub fn rot_x(a: f64) -> Xform {
        let (s, c) = a.sin_cos();
        Xform { m: [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]], t: [0.0; 3] }
    }
    pub fn rot_y(a: f64) -> Xform {
        let (s, c) = a.sin_cos();
        Xform { m: [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]], t: [0.0; 3] }
    }
    pub fn rot_z(a: f64) -> Xform {
        let (s, c) = a.sin_cos();
        Xform { m: [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]], t: [0.0; 3] }
    }
    /// Złożenie: najpierw `rhs`, potem `self` (`self ∘ rhs`).
    pub fn mul(&self, rhs: &Xform) -> Xform {
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| self.m[i][k] * rhs.m[k][j]).sum();
            }
        }
        Xform { m, t: self.point(rhs.t) }
    }
    /// Sam obrót (normalne, kierunki).
    pub fn dir(&self, v: V3) -> V3 {
        std::array::from_fn(|i| self.m[i][0] * v[0] + self.m[i][1] * v[1] + self.m[i][2] * v[2])
    }
    pub fn point(&self, v: V3) -> V3 {
        add(self.dir(v), self.t)
    }
}

#[derive(Default)]
pub struct Counters {
    pub windows: u32,
    pub balconies: u32,
}

pub struct MeshBuilder {
    parts: Vec<MeshPart>,
    min: V3,
    max: V3,
    pub counters: Counters,
}

impl Default for MeshBuilder {
    fn default() -> Self {
        Self { parts: Vec::new(), min: [f64::INFINITY; 3], max: [f64::NEG_INFINITY; 3], counters: Counters::default() }
    }
}

fn uv_for(p: V3, n: V3) -> [f64; 2] {
    let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
    if ay > 0.72 {
        [p[0], p[2]]
    } else if ax > az {
        [if n[0] > 0.0 { -p[2] } else { p[2] }, p[1]]
    } else {
        [if n[2] > 0.0 { p[0] } else { -p[0] }, p[1]]
    }
}

impl MeshBuilder {
    fn buf(&mut self, mat: MatKey) -> &mut MeshPart {
        // kolejność części = kolejność pierwszego użycia (jak Map w JS)
        let i = match self.parts.iter().position(|p| p.material == Some(mat)) {
            Some(i) => i,
            None => {
                self.parts.push(MeshPart { material: Some(mat), ..Default::default() });
                self.parts.len() - 1
            }
        };
        &mut self.parts[i]
    }

    /// Wielokąt wypukły; `want` = zamierzony kierunek normalnej (odwraca kolejność, gdy trzeba).
    pub fn poly(&mut self, mat: MatKey, pts: &[V3], want: Option<V3>) {
        if pts.len() < 3 {
            return;
        }
        let mut nrm = norm(cross(sub(pts[1], pts[0]), sub(pts[2], pts[0])));
        if len(nrm) < 0.5 || !nrm[0].is_finite() {
            for k in 2..pts.len() {
                let c = cross(sub(pts[1], pts[0]), sub(pts[k], pts[0]));
                if len(c) > 1e-9 {
                    nrm = norm(c);
                    break;
                }
            }
        }
        let rev = want.is_some_and(|w| dot(nrm, w) < 0.0);
        if rev {
            nrm = scale(nrm, -1.0);
        }
        let (mut lo, mut hi) = (self.min, self.max);
        let b = self.buf(mat);
        let base = (b.positions.len() / 3) as u32;
        let n = pts.len();
        for i in 0..n {
            let p = pts[if rev { n - 1 - i } else { i }];
            b.positions.extend(p.map(|x| x as f32));
            b.normals.extend(nrm.map(|x| x as f32));
            b.uvs.extend(uv_for(p, nrm).map(|x| x as f32));
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        for k in 1..n as u32 - 1 {
            b.indices.extend([base, base + k, base + k + 1]);
        }
        (self.min, self.max) = (lo, hi);
    }

    pub fn quad(&mut self, mat: MatKey, a: V3, b: V3, c: V3, d: V3, want: Option<V3>) {
        self.poly(mat, &[a, b, c, d], want);
    }


    /// Pas między kolejnymi pierścieniami (zamkniętymi, tej samej długości) w „flat shading”.
    /// Pierścień to pętla CCW w płaszczyźnie XY patrząc z +Z, kolejne pierścienie rosną w +Z;
    /// wtedy normalne wychodzą na zewnątrz. `caps` = zaślepki pierwszego i ostatniego pierścienia.
    /// ponytail: zaślepka to wachlarz od pierwszego wierzchołka, więc pierścienie muszą być wypukłe
    pub fn loft(&mut self, mat: MatKey, rings: &[Vec<V3>], caps: bool) {
        self.loft_with(rings, caps.then_some(mat), |_, _| mat);
    }

    /// Jak `loft`, materiał pasa wybiera `pick(przedział, segment pierścienia)`; zaślepki (jeśli `Some`) mają własny.
    pub fn loft_with(&mut self, rings: &[Vec<V3>], caps: Option<MatKey>, pick: impl Fn(usize, usize) -> MatKey) {
        for (i, w) in rings.windows(2).enumerate() {
            let n = w[0].len();
            assert_eq!(n, w[1].len(), "pierścienie lofta muszą mieć tyle samo wierzchołków");
            for s in 0..n {
                let (a, b, c, d) = (w[0][s], w[0][(s + 1) % n], w[1][(s + 1) % n], w[1][s]);
                let mat = pick(i, s);
                // dwa trójkąty zamiast płaskiego czworokąta: niepłaskie ściany dostają własne normalne
                for tri in [[a, b, c], [a, c, d]] {
                    if len(cross(sub(tri[1], tri[0]), sub(tri[2], tri[0]))) > 1e-9 {
                        self.poly(mat, &tri, None);
                    }
                }
            }
        }
        if let (Some(cap), true) = (caps, rings.len() >= 2) {
            let centroid = |r: &Vec<V3>| scale(r.iter().fold([0.0; 3], |a, &p| add(a, p)), 1.0 / r.len() as f64);
            let (first, last) = (&rings[0], &rings[rings.len() - 1]);
            let (c0, c1) = (centroid(first), centroid(&rings[1]));
            let (e0, e1) = (centroid(last), centroid(&rings[rings.len() - 2]));
            self.poly(cap, first, Some(sub(c0, c1)));
            self.poly(cap, last, Some(sub(e0, e1)));
        }
    }

    /// Dołącza kopię gotowej siatki przekształconą `x` (materiały po `MatKey`).
    pub fn push(&mut self, m: &MeshData, x: &Xform) {
        let (mut lo, mut hi) = (self.min, self.max);
        for part in &m.parts {
            let mat = part.material.expect("materiał części");
            let b = self.buf(mat);
            let base = (b.positions.len() / 3) as u32;
            for v in part.positions.chunks_exact(3) {
                let p = x.point([v[0] as f64, v[1] as f64, v[2] as f64]);
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
                b.positions.extend(p.map(|c| c as f32));
            }
            for v in part.normals.chunks_exact(3) {
                b.normals.extend(x.dir([v[0] as f64, v[1] as f64, v[2] as f64]).map(|c| c as f32));
            }
            b.uvs.extend(&part.uvs);
            b.indices.extend(part.indices.iter().map(|i| i + base));
        }
        (self.min, self.max) = (lo, hi);
    }

    /// Prostopadłościan wyrównany do osi.
    pub fn bx(&mut self, mat: MatKey, min: V3, max: V3) {
        let [x0, y0, z0] = min;
        let [x1, y1, z1] = max;
        if x1 - x0 < 1e-6 || y1 - y0 < 1e-6 || z1 - z0 < 1e-6 {
            return;
        }
        self.quad(mat, [x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1], Some([1.0, 0.0, 0.0]));
        self.quad(mat, [x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0], Some([-1.0, 0.0, 0.0]));
        self.quad(mat, [x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0], Some([0.0, 1.0, 0.0]));
        self.quad(mat, [x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1], Some([0.0, -1.0, 0.0]));
        self.quad(mat, [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1], Some([0.0, 0.0, 1.0]));
        self.quad(mat, [x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0], Some([0.0, 0.0, -1.0]));
    }

    /// Prostopadłościan w układzie elewacji: [a0,a1] × [b0,b1] × [c0,c1].
    #[allow(clippy::too_many_arguments)]
    pub fn box_l(&mut self, f: &Frame, mat: MatKey, a0: f64, a1: f64, b0: f64, b1: f64, c0: f64, c1: f64, skip_back: bool) {
        if a1 - a0 < 1e-6 || b1 - b0 < 1e-6 || c1 - c0 < 1e-6 {
            return;
        }
        let p = |a, b, c| to_world(f, a, b, c);
        let (nu, nn) = (f.u, f.n);
        self.quad(mat, p(a0, b0, c1), p(a1, b0, c1), p(a1, b1, c1), p(a0, b1, c1), Some(nn));
        if !skip_back {
            self.quad(mat, p(a1, b0, c0), p(a0, b0, c0), p(a0, b1, c0), p(a1, b1, c0), Some(scale(nn, -1.0)));
        }
        self.quad(mat, p(a1, b0, c1), p(a1, b0, c0), p(a1, b1, c0), p(a1, b1, c1), Some(nu));
        self.quad(mat, p(a0, b0, c0), p(a0, b0, c1), p(a0, b1, c1), p(a0, b1, c0), Some(scale(nu, -1.0)));
        self.quad(mat, p(a0, b1, c1), p(a1, b1, c1), p(a1, b1, c0), p(a0, b1, c0), Some(UP));
        self.quad(mat, p(a0, b0, c0), p(a1, b0, c0), p(a1, b0, c1), p(a0, b0, c1), Some([0.0, -1.0, 0.0]));
    }

    /// Prostokąt w płaszczyźnie elewacji na głębokości c, zwrócony w stronę ±n.
    #[allow(clippy::too_many_arguments)]
    pub fn rect_n(&mut self, f: &Frame, mat: MatKey, a0: f64, a1: f64, b0: f64, b1: f64, c: f64, sign: f64) {
        if a1 - a0 < 1e-6 || b1 - b0 < 1e-6 {
            return;
        }
        let p = |a, b| to_world(f, a, b, c);
        self.quad(mat, p(a0, b0), p(a1, b0), p(a1, b1), p(a0, b1), Some(scale(f.n, sign)));
    }

    /// Prostokąt prostopadły do elewacji w pozycji a (ościeże boczne), zwrócony ±u.
    #[allow(clippy::too_many_arguments)]
    pub fn rect_u(&mut self, f: &Frame, mat: MatKey, a: f64, b0: f64, b1: f64, c0: f64, c1: f64, sign: f64) {
        if b1 - b0 < 1e-6 || (c1 - c0).abs() < 1e-6 {
            return;
        }
        let p = |b, c| to_world(f, a, b, c);
        self.quad(mat, p(b0, c0), p(b0, c1), p(b1, c1), p(b1, c0), Some(scale(f.u, sign)));
    }

    /// Prostokąt poziomy na wysokości b (nadproże / parapet), zwrócony ±v.
    #[allow(clippy::too_many_arguments)]
    pub fn rect_v(&mut self, f: &Frame, mat: MatKey, b: f64, a0: f64, a1: f64, c0: f64, c1: f64, sign: f64) {
        if a1 - a0 < 1e-6 || (c1 - c0).abs() < 1e-6 {
            return;
        }
        let p = |a, c| to_world(f, a, b, c);
        self.quad(mat, p(a0, c0), p(a1, c0), p(a1, c1), p(a0, c1), Some([0.0, sign, 0.0]));
    }

    /// Graniastosłup foremny (kolumna, wieżyczka).
    #[allow(clippy::too_many_arguments)]
    pub fn prism(&mut self, mat: MatKey, cx: f64, cz: f64, y0: f64, y1: f64, r: f64, seg: usize, rot: f64, caps: bool) {
        let tau = std::f64::consts::PI * 2.0;
        let pts: Vec<[f64; 2]> = (0..seg).map(|k| {
            let t = rot + (k as f64 / seg as f64) * tau;
            [cx + t.cos() * r, cz + t.sin() * r]
        }).collect();
        for k in 0..seg {
            let [ax, az] = pts[k];
            let [bx, bz] = pts[(k + 1) % seg];
            let mid = [(ax + bx) / 2.0 - cx, 0.0, (az + bz) / 2.0 - cz];
            self.quad(mat, [ax, y0, az], [bx, y0, bz], [bx, y1, bz], [ax, y1, az], Some(mid));
        }
        if caps {
            let top: Vec<V3> = pts.iter().map(|&[x, z]| [x, y1, z]).collect();
            let bot: Vec<V3> = pts.iter().map(|&[x, z]| [x, y0, z]).collect();
            self.poly(mat, &top, Some(UP));
            self.poly(mat, &bot, Some([0.0, -1.0, 0.0]));
        }
    }

    /// Ostrosłup (hełm wieżyczki).
    #[allow(clippy::too_many_arguments)]
    pub fn pyramid(&mut self, mat: MatKey, cx: f64, cz: f64, y0: f64, h: f64, r: f64, seg: usize, rot: f64) {
        let tau = std::f64::consts::PI * 2.0;
        let apex = [cx, y0 + h, cz];
        for k in 0..seg {
            let t0 = rot + (k as f64 / seg as f64) * tau;
            let t1 = rot + ((k + 1) as f64 / seg as f64) * tau;
            let a = [cx + t0.cos() * r, y0, cz + t0.sin() * r];
            let b = [cx + t1.cos() * r, y0, cz + t1.sin() * r];
            let out = [(a[0] + b[0]) / 2.0 - cx, r * 0.6, (a[2] + b[2]) / 2.0 - cz];
            self.poly(mat, &[a, b, apex], Some(out));
        }
    }

    pub fn bounds(&self) -> Bounds {
        if !self.min[0].is_finite() {
            return Bounds { min: [0.0; 3], max: [0.0; 3] };
        }
        Bounds { min: self.min, max: self.max }
    }

    pub fn build(self, materials: Vec<MaterialSpec>) -> MeshData {
        let bounds = self.bounds();
        MeshData { parts: self.parts.into_iter().filter(|p| !p.indices.is_empty()).collect(), materials, bounds }
    }
}
