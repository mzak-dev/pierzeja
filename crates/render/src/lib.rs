//! Programowy rasteryzator (bez GPU): z-bufor, Lambert, mapa cieni od słońca,
//! krawędzie z nieciągłości głębi, SSAA. Port `render/raster.ts` prototypu —
//! deterministyczne podglądy PNG dla MCP i testów obrazowych.

use pierzeja_core::color::{hex_to_linear, linear_to_srgb};
use pierzeja_core::{Bounds, MatKey, MeshData};
use serde::Deserialize;

pub type V3 = [f64; 3];

pub struct RenderItem<'a> {
    pub mesh: &'a MeshData,
    pub offset: V3,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum View {
    #[default]
    Iso,
    Front,
    Aerial,
    Back,
    Street,
}

impl View {
    fn angles(self) -> (f64, f64) {
        match self {
            View::Iso => (34.0, 17.0),
            View::Front => (0.0, 5.0),
            View::Aerial => (32.0, 50.0),
            View::Back => (214.0, 20.0),
            View::Street => (20.0, 9.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub width: u32,
    pub height: u32,
    pub view: View,
    pub ssaa: u32,
    pub shadows: bool,
    pub edges: bool,
    pub night: bool,
    pub ground: bool,
    pub azimuth: Option<f64>,
    pub elevation: Option<f64>,
    /// Przezroczyste tło (bez nieba i terenu) — np. miniatury.
    pub transparent: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { width: 1200, height: 800, view: View::Iso, ssaa: 2, shadows: true, edges: true, night: false, ground: true, azimuth: None, elevation: None, transparent: false }
    }
}

pub struct Image {
    pub width: u32,
    pub height: u32,
    /// RGBA8.
    pub data: Vec<u8>,
}

impl Image {
    pub fn to_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.width, self.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().expect("png header");
            w.write_image_data(&self.data).expect("png data");
        }
        out
    }
}

fn norm3(x: f64, y: f64, z: f64) -> V3 {
    let l = x.hypot(y).hypot(z);
    let l = if l == 0.0 { 1.0 } else { l };
    [x / l, y / l, z / l]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn scene_bounds(items: &[RenderItem]) -> Bounds {
    let (mut min, mut max) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for it in items {
        for k in 0..3 {
            min[k] = min[k].min(it.mesh.bounds.min[k] + it.offset[k]);
            max[k] = max[k].max(it.mesh.bounds.max[k] + it.offset[k]);
        }
    }
    if !min[0].is_finite() {
        return Bounds { min: [-1.0, 0.0, -1.0], max: [1.0, 1.0, 1.0] };
    }
    Bounds { min, max }
}

struct TransTri {
    p: [[f64; 3]; 3],
    rgb: V3,
    alpha: f64,
    depth: f64,
}

/// Bounding box trójkąta w pikselach, przycięty do obrazu.
fn bbox(p: &[[f64; 3]; 3], w: usize, h: usize) -> Option<(usize, usize, usize, usize)> {
    let (x0, x1) = (p[0][0].min(p[1][0]).min(p[2][0]).floor().max(0.0), p[0][0].max(p[1][0]).max(p[2][0]).ceil().min(w as f64 - 1.0));
    let (y0, y1) = (p[0][1].min(p[1][1]).min(p[2][1]).floor().max(0.0), p[0][1].max(p[1][1]).max(p[2][1]).ceil().min(h as f64 - 1.0));
    if !(x0 <= x1 && y0 <= y1) {
        return None;
    }
    Some((x0 as usize, x1 as usize, y0 as usize, y1 as usize))
}

/// Współrzędne barycentryczne środka piksela.
fn bary(p: &[[f64; 3]; 3], inv: f64, cx: f64, cy: f64) -> (f64, f64, f64) {
    let [x0, y0, _] = p[0];
    let [x1, y1, _] = p[1];
    let [x2, y2, _] = p[2];
    let l0 = ((x1 - cx) * (y2 - cy) - (x2 - cx) * (y1 - cy)) * inv;
    let l1 = ((x2 - cx) * (y0 - cy) - (x0 - cx) * (y2 - cy)) * inv;
    (l0, l1, 1.0 - l0 - l1)
}

fn area(p: &[[f64; 3]; 3]) -> f64 {
    (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1])
}

pub fn render_scene(items: &[RenderItem], o: &RenderOptions) -> Image {
    let out_w = o.width.clamp(64, 2400) as usize;
    let out_h = o.height.clamp(64, 2400) as usize;
    let ss = o.ssaa.clamp(1, 3) as usize;
    let (w, h) = (out_w * ss, out_h * ss);
    let (wf, hf) = (w as f64, h as f64);
    let night = o.night;
    let (paz, pel) = o.view.angles();
    let az = o.azimuth.unwrap_or(paz).to_radians();
    let el = o.elevation.unwrap_or(pel).to_radians();

    let b = scene_bounds(items);
    let size = [b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]];
    let target = [(b.min[0] + b.max[0]) / 2.0, b.min[1] + size[1] * 0.42, (b.min[2] + b.max[2]) / 2.0];
    let dir = norm3(az.sin() * el.cos(), el.sin(), az.cos() * el.cos());
    let tan_y = (15f64).to_radians().tan();
    let tan_x = tan_y * (wf / hf);
    let fwd = [-dir[0], -dir[1], -dir[2]];
    let right = norm3(-fwd[2], 0.0, fwd[0]);
    let cam_up = [right[1] * fwd[2] - right[2] * fwd[1], right[2] * fwd[0] - right[0] * fwd[2], right[0] * fwd[1] - right[1] * fwd[0]];
    // dopasowanie odległości do narożników prostopadłościanu sceny
    let mut dist = 1.0f64;
    for x in [b.min[0], b.max[0]] {
        for y in [b.min[1], b.max[1]] {
            for z in [b.min[2], b.max[2]] {
                let r = [x - target[0], y - target[1], z - target[2]];
                let (xc, yc, zc) = (dot(r, right), dot(r, cam_up), dot(r, dir));
                dist = dist.max(zc + xc.abs() / (tan_x * 0.92)).max(zc + yc.abs() / (tan_y * 0.9));
            }
        }
    }
    let eye = [target[0] + dir[0] * dist, target[1] + dir[1] * dist, target[2] + dir[2] * dist];
    let near = 0.5;

    // słońce i mapa cieni
    let l = if night { norm3(0.3, 0.9, 0.35) } else { norm3(-0.5, 0.74, 0.46) };
    let shadows = o.shadows && !night;
    let s = 1600usize;
    let rr = size[0].hypot(size[1]).hypot(size[2]) / 2.0 + 2.0;
    let c = [(b.min[0] + b.max[0]) / 2.0, (b.min[1] + b.max[1]) / 2.0, (b.min[2] + b.max[2]) / 2.0];
    let lx = norm3(l[2], 0.0, -l[0]);
    let ly = [l[1] * lx[2] - l[2] * lx[1], l[2] * lx[0] - l[0] * lx[2], l[0] * lx[1] - l[1] * lx[0]];
    let mut sm = vec![f32::NEG_INFINITY; if shadows { s * s } else { 1 }];
    let sm_scale = s as f64 / (2.0 * rr);
    let bias = (2.0 * rr) / s as f64 * 2.2 + 0.04;
    let transparent_mat = |k: MatKey| matches!(k, MatKey::GlassRail | MatKey::Railing);
    let is_glass = |k: MatKey| matches!(k, MatKey::Glass | MatKey::GlassLit);

    // ---------------------------------------------------------- przebieg cieni
    if shadows {
        for it in items {
            for part in &it.mesh.parts {
                let mat = part.material.unwrap_or(MatKey::Wall);
                if transparent_mat(mat) {
                    continue;
                }
                let pts: Vec<[f64; 3]> = part
                    .positions
                    .chunks_exact(3)
                    .map(|p| {
                        let r = [p[0] as f64 + it.offset[0] - c[0], p[1] as f64 + it.offset[1] - c[1], p[2] as f64 + it.offset[2] - c[2]];
                        [(dot(r, lx) + rr) * sm_scale, (dot(r, ly) + rr) * sm_scale, dot(r, l)]
                    })
                    .collect();
                for t in part.indices.chunks_exact(3) {
                    let tri = [pts[t[0] as usize], pts[t[1] as usize], pts[t[2] as usize]];
                    let a = area(&tri);
                    if a.abs() < 1e-9 {
                        continue;
                    }
                    let Some((x0, x1, y0, y1)) = bbox(&tri, s, s) else { continue };
                    for py in y0..=y1 {
                        for px in x0..=x1 {
                            let (l0, l1, l2) = bary(&tri, 1.0 / a, px as f64 + 0.5, py as f64 + 0.5);
                            if l0 < -0.02 || l1 < -0.02 || l2 < -0.02 {
                                continue;
                            }
                            let d = (l0 * tri[0][2] + l1 * tri[1][2] + l2 * tri[2][2]) as f32;
                            let i = py * s + px;
                            if d > sm[i] {
                                sm[i] = d;
                            }
                        }
                    }
                }
            }
        }
    }
    let in_shadow = |x: f64, y: f64, z: f64| -> bool {
        if !shadows {
            return false;
        }
        let r = [x - c[0], y - c[1], z - c[2]];
        let u = ((dot(r, lx) + rr) * sm_scale).floor();
        let v = ((dot(r, ly) + rr) * sm_scale).floor();
        if u < 0.0 || v < 0.0 || u >= s as f64 || v >= s as f64 {
            return false;
        }
        sm[v as usize * s + u as usize] as f64 > dot(r, l) + bias
    };

    // ---------------------------------------------------------- bufory
    let mut col = vec![0f32; w * h * 3];
    let mut zb = vec![f64::INFINITY; w * h];
    let hx = |s: &str| hex_to_linear(s);
    let sky_top = if night { hx("#0b1220") } else { hx("#9fb7c8") };
    let sky_hor = if night { hx("#1d2738") } else { hx("#e4e9ea") };
    let hl0 = fwd[0].hypot(fwd[2]);
    let hl0 = if hl0 == 0.0 { 1.0 } else { hl0 };
    let hzc0 = (fwd[0] * fwd[0] + fwd[2] * fwd[2]) / hl0;
    let hy0 = (fwd[0] * cam_up[0] + fwd[2] * cam_up[2]) / hl0;
    let horizon_y = if hzc0 > 1e-6 { (1.0 - hy0 / (hzc0 * tan_y)) * 0.5 * hf } else { hf };
    let with_ground = o.ground && !o.transparent;
    for y in 0..h {
        let t = if with_ground { (y as f64 / horizon_y.max(1.0)).min(1.0) } else { y as f64 / hf };
        for x in 0..w {
            let k = (y * w + x) * 3;
            for ch in 0..3 {
                col[k + ch] = (sky_top[ch] + (sky_hor[ch] - sky_top[ch]) * t) as f32;
            }
        }
    }
    let sky_refl = if night { hx("#1b2333") } else { hx("#c9d6de") };
    let sun_i = if night { 0.12 } else { 1.0 };
    let amb_k = if night { 0.25 } else { 1.0 };
    let gcol = if night { hx("#1a2018") } else { hx("#8f9a80") };

    // maska terenu (krawędzie między polami terenu a tłem nie są obrysowywane)
    let mut gmask = vec![0u8; w * h];
    if with_ground {
        let lit = 0.66 * amb_k + l[1].max(0.0) * 0.95 * sun_i;
        for y in (horizon_y.ceil().max(0.0) as usize)..h {
            for x in 0..w {
                let i = y * w + x;
                for ch in 0..3 {
                    col[i * 3 + ch] = (gcol[ch] * lit) as f32;
                }
                gmask[i] = 1;
            }
        }
    }

    let project = |p: V3| -> [f64; 3] {
        let r = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
        let (xc, yc, zc) = (dot(r, right), dot(r, cam_up), dot(r, fwd));
        [(xc / (zc * tan_x) + 1.0) * 0.5 * wf, (1.0 - yc / (zc * tan_y)) * 0.5 * hf, zc]
    };

    let mut trans: Vec<TransTri> = Vec::new();

    // rysuje trójkąty części; (pr, wp) = rzut i pozycje w świecie
    let mut draw_part = |col: &mut Vec<f32>, zb: &mut Vec<f64>, gmask: &mut Vec<u8>, pr: &[[f64; 3]], wp: &[V3], idx: &[u32], mat: MatKey, base: V3, double_sided: bool, opacity: f64, emissive: bool, ground_pass: bool| {
        for t in idx.chunks_exact(3) {
            let (i0, i1, i2) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let tri = [pr[i0], pr[i1], pr[i2]];
            if tri.iter().any(|p| p[2] < near) {
                continue;
            }
            let a = [wp[i1][0] - wp[i0][0], wp[i1][1] - wp[i0][1], wp[i1][2] - wp[i0][2]];
            let bb = [wp[i2][0] - wp[i0][0], wp[i2][1] - wp[i0][1], wp[i2][2] - wp[i0][2]];
            let mut n = [a[1] * bb[2] - a[2] * bb[1], a[2] * bb[0] - a[0] * bb[2], a[0] * bb[1] - a[1] * bb[0]];
            let nl = n[0].hypot(n[1]).hypot(n[2]);
            if nl < 1e-12 {
                continue;
            }
            n = [n[0] / nl, n[1] / nl, n[2] / nl];
            let v = [eye[0] - wp[i0][0], eye[1] - wp[i0][1], eye[2] - wp[i0][2]];
            if dot(n, v) < 0.0 {
                if !double_sided {
                    continue;
                }
                n = [-n[0], -n[1], -n[2]];
            }
            let vl = v[0].hypot(v[1]).hypot(v[2]);
            let ndl = dot(n, l).max(0.0);
            let amb = (0.36 + 0.3 * (n[1] * 0.5 + 0.5)) * amb_k;
            let (mut rgb, mut sun_part);
            if is_glass(mat) {
                let ndv = (dot(n, v) / vl).abs();
                let fr = 0.22 + 0.6 * (1.0 - ndv).powf(2.2);
                rgb = [0, 1, 2].map(|k| base[k] * 0.5 + sky_refl[k] * fr * 0.6);
                sun_part = ndl * 0.18 * sun_i;
                if emissive && night {
                    rgb = [1.0, 0.7, 0.38];
                    sun_part = 0.0;
                }
            } else {
                rgb = base;
                sun_part = ndl * 0.95 * sun_i;
            }
            if opacity < 0.99 {
                let lit = amb + sun_part;
                trans.push(TransTri { p: tri, rgb: rgb.map(|x| x * lit), alpha: opacity, depth: (tri[0][2] + tri[1][2] + tri[2][2]) / 3.0 });
                continue;
            }
            let glow = emissive && night && is_glass(mat);
            let ar = area(&tri);
            if ar.abs() < 1e-9 {
                continue;
            }
            let Some((x0, x1, y0, y1)) = bbox(&tri, w, h) else { continue };
            let inv = 1.0 / ar;
            let ws = [1.0 / tri[0][2], 1.0 / tri[1][2], 1.0 / tri[2][2]];
            for py in y0..=y1 {
                for px in x0..=x1 {
                    let (l0, l1, l2) = bary(&tri, inv, px as f64 + 0.5, py as f64 + 0.5);
                    if l0 < -1e-6 || l1 < -1e-6 || l2 < -1e-6 {
                        continue;
                    }
                    let z = 1.0 / (l0 * ws[0] + l1 * ws[1] + l2 * ws[2]);
                    let i = py * w + px;
                    if z >= zb[i] {
                        continue;
                    }
                    zb[i] = z;
                    gmask[i] = ground_pass as u8;
                    let mut s = sun_part;
                    if s > 0.0 && shadows {
                        let wpt = [0, 1, 2].map(|k| (l0 * wp[i0][k] * ws[0] + l1 * wp[i1][k] * ws[1] + l2 * wp[i2][k] * ws[2]) * z);
                        if in_shadow(wpt[0], wpt[1], wpt[2]) {
                            s = 0.0;
                        }
                    }
                    let lit = if glow { 1.0 } else { amb + s };
                    for k in 0..3 {
                        col[i * 3 + k] = (rgb[k] * lit) as f32;
                    }
                }
            }
        }
    };

    // ---------------------------------------------------------- teren
    if with_ground {
        // rozległy teren (aż po horyzont) cięty na siatkę pól, żeby pominąć tylko pola za kamerą
        let ext = size[0].max(size[2]) * 2.5 + 160.0;
        let (gx0, gz0) = ((b.min[0] + b.max[0]) / 2.0 - ext, (b.min[2] + b.max[2]) / 2.0 - ext);
        let n = 40;
        let cell = (2.0 * ext) / n as f64;
        let (mut wp, mut idx) = (Vec::new(), Vec::new());
        for i in 0..n {
            for j in 0..n {
                let (xa, za, zbb) = (gx0 + i as f64 * cell, gz0 + (j + 1) as f64 * cell, gz0 + j as f64 * cell);
                let o = wp.len() as u32;
                wp.extend([[xa, -0.03, za], [xa + cell, -0.03, za], [xa + cell, -0.03, zbb], [xa, -0.03, zbb]]);
                idx.extend([o, o + 1, o + 2, o, o + 2, o + 3]);
            }
        }
        let pr: Vec<[f64; 3]> = wp.iter().map(|&p| project(p)).collect();
        draw_part(&mut col, &mut zb, &mut gmask, &pr, &wp, &idx, MatKey::Ground, gcol, true, 1.0, false, true);
    }

    // ---------------------------------------------------------- budynki
    for it in items {
        for part in &it.mesh.parts {
            let mat = part.material.unwrap_or(MatKey::Wall);
            let spec = it.mesh.material(mat);
            let base = hex_to_linear(spec.map_or("#999999", |s| s.color.as_str()));
            let wp: Vec<V3> = part.positions.chunks_exact(3).map(|p| [p[0] as f64 + it.offset[0], p[1] as f64 + it.offset[1], p[2] as f64 + it.offset[2]]).collect();
            let pr: Vec<[f64; 3]> = wp.iter().map(|&p| project(p)).collect();
            let opacity = if mat == MatKey::Railing { 0.62 } else { spec.map_or(1.0, |s| s.opacity) };
            draw_part(&mut col, &mut zb, &mut gmask, &pr, &wp, &part.indices, mat, base, spec.is_some_and(|s| s.double_sided), opacity, mat == MatKey::GlassLit, false);
        }
    }

    // ---------------------------------------------------------- przezroczyste (od tyłu)
    trans.sort_by(|a, c| c.depth.total_cmp(&a.depth));
    for t in &trans {
        let ar = area(&t.p);
        if ar.abs() < 1e-9 {
            continue;
        }
        let Some((x0, x1, y0, y1)) = bbox(&t.p, w, h) else { continue };
        let ws = [1.0 / t.p[0][2], 1.0 / t.p[1][2], 1.0 / t.p[2][2]];
        for py in y0..=y1 {
            for px in x0..=x1 {
                let (l0, l1, l2) = bary(&t.p, 1.0 / ar, px as f64 + 0.5, py as f64 + 0.5);
                if l0 < 0.0 || l1 < 0.0 || l2 < 0.0 {
                    continue;
                }
                let z = 1.0 / (l0 * ws[0] + l1 * ws[1] + l2 * ws[2]);
                let i = py * w + px;
                if z >= zb[i] {
                    continue;
                }
                for k in 0..3 {
                    col[i * 3 + k] = (col[i * 3 + k] as f64 * (1.0 - t.alpha) + t.rgb[k] * t.alpha) as f32;
                }
            }
        }
    }

    // ---------------------------------------------------------- mgiełka nad terenem
    if with_ground {
        let haze = if night { hx("#1d2738") } else { sky_hor };
        let dd = dist * 1.6;
        for y in 0..h {
            let sy = 1.0 - (2.0 * (y as f64 + 0.5)) / hf;
            for x in 0..w {
                let i = y * w + x;
                if gmask[i] == 0 {
                    continue;
                }
                let sx = (2.0 * (x as f64 + 0.5)) / wf - 1.0;
                let r = [0, 1, 2].map(|k| fwd[k] + right[k] * sx * tan_x + cam_up[k] * sy * tan_y);
                if r[1] >= -1e-6 {
                    continue;
                }
                let t = (-0.03 - eye[1]) / r[1];
                let d = t * r[0].hypot(r[1]).hypot(r[2]);
                let hz = 0.97f64.min(1.0 - (-(d - dist).max(0.0) / dd).exp());
                if hz <= 0.0 {
                    continue;
                }
                for k in 0..3 {
                    col[i * 3 + k] = (col[i * 3 + k] as f64 * (1.0 - hz) + haze[k] * hz) as f32;
                }
            }
        }
    }

    // ---------------------------------------------------------- krawędzie
    if o.edges {
        let mut dark = vec![false; w * h];
        for y in 0..h - 1 {
            for x in 0..w - 1 {
                let i = y * w + x;
                if gmask[i] != 0 && gmask[i + 1] != 0 && gmask[i + w] != 0 {
                    continue;
                }
                let (z, zr, zd) = (zb[i], zb[i + 1], zb[i + w]);
                let t = 0.018 * z.min(zr).min(zd);
                if (z.is_finite() || zr.is_finite() || zd.is_finite()) && ((z - zr).abs() > t || (z - zd).abs() > t) {
                    dark[i] = true;
                }
            }
        }
        for (i, _) in dark.iter().enumerate().filter(|(_, d)| **d) {
            for k in 0..3 {
                col[i * 3 + k] *= 0.6;
            }
        }
    }

    // ---------------------------------------------------------- SSAA i sRGB
    let mut out = vec![0u8; out_w * out_h * 4];
    let inv = 1.0 / (ss * ss) as f64;
    for y in 0..out_h {
        for x in 0..out_w {
            let (mut acc, mut cov) = ([0.0f64; 3], 0usize);
            for sy in 0..ss {
                for sx in 0..ss {
                    let i = (y * ss + sy) * w + (x * ss + sx);
                    if o.transparent && !zb[i].is_finite() {
                        continue;
                    }
                    for k in 0..3 {
                        acc[k] += col[i * 3 + k] as f64;
                    }
                    cov += 1;
                }
            }
            let oi = (y * out_w + x) * 4;
            if cov == 0 {
                continue;
            }
            let invc = if o.transparent { 1.0 / cov as f64 } else { inv };
            for k in 0..3 {
                let c = acc[k] * invc;
                out[oi + k] = (linear_to_srgb((c / (1.0 + c * 0.12) * 1.08).min(1.0)) * 255.0).round() as u8;
            }
            out[oi + 3] = if o.transparent { (cov as f64 * inv * 255.0).round() as u8 } else { 255 };
        }
    }
    Image { width: out_w as u32, height: out_h as u32, data: out }
}
