//! Krzywe prowadzące (Tabor): łamane, monotoniczny Hermite z węzłami-załamaniami, zaokrąglanie
//! naroży łamanej, superelipsa. Wszystko w 2D `(x, y)`; `x` rośnie wzdłuż krzywej.

/// Węzeł krzywej: położenie i to, czy krzywa w nim się łamie (`ostry`), czy przechodzi gładko.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wezel {
    pub x: f64,
    pub y: f64,
    pub ostry: bool,
}

pub const fn g(x: f64, y: f64) -> Wezel {
    Wezel { x, y, ostry: false }
}
pub const fn o(x: f64, y: f64) -> Wezel {
    Wezel { x, y, ostry: true }
}

/// Łamana: interpolacja liniowa po `x` rosnącym, poza zakresem wartość skrajna.
pub fn pl(pts: &[(f64, f64)], x: f64) -> f64 {
    if pts.is_empty() {
        return 0.0;
    }
    if x <= pts[0].0 {
        return pts[0].1;
    }
    for w in pts.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x <= x1 {
            let t = if x1 - x0 < 1e-12 { 1.0 } else { (x - x0) / (x1 - x0) };
            return y0 + (y1 - y0) * t;
        }
    }
    pts[pts.len() - 1].1
}

/// Monotoniczna krzywa Hermite'a (Fritsch–Carlson) przez węzły o rosnącym `x`. Węzeł `ostry`
/// rozcina krzywą: styczne liczone osobno po obu stronach, więc w nim jest załamanie.
/// Bez przestrzeleń: między dwoma węzłami wartość nie wychodzi poza ich zakres.
#[derive(Debug, Clone)]
pub struct Krzywa {
    xs: Vec<f64>,
    ys: Vec<f64>,
    /// Styczne: lewa i prawa w każdym węźle (różne tylko w węzłach ostrych).
    ml: Vec<f64>,
    mr: Vec<f64>,
}

impl Krzywa {
    pub fn new(w: &[Wezel]) -> Krzywa {
        // węzły o (prawie) tym samym x: zostaje ostatni — chroni przed dzieleniem przez zero
        let mut pts: Vec<Wezel> = Vec::with_capacity(w.len());
        for &p in w {
            match pts.last_mut() {
                Some(q) if p.x - q.x < 1e-9 => *q = Wezel { x: q.x, ostry: q.ostry || p.ostry, ..p },
                _ => pts.push(p),
            }
        }
        let n = pts.len();
        let xs: Vec<f64> = pts.iter().map(|p| p.x).collect();
        let ys: Vec<f64> = pts.iter().map(|p| p.y).collect();
        if n < 2 {
            return Krzywa { xs, ys, ml: vec![0.0; n], mr: vec![0.0; n] };
        }
        let d: Vec<f64> = (0..n - 1).map(|i| (ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i])).collect();
        let (mut ml, mut mr) = (vec![0.0; n], vec![0.0; n]);
        for i in 0..n {
            let (left, right) = (i.checked_sub(1).map(|k| d[k]), d.get(i).copied());
            let ostry = pts[i].ostry || i == 0 || i == n - 1;
            if ostry {
                // jednostronnie: styczna = nachylenie sąsiedniego odcinka
                ml[i] = left.unwrap_or(0.0);
                mr[i] = right.unwrap_or(0.0);
            } else {
                let (a, b) = (left.unwrap(), right.unwrap());
                let m = if a * b <= 0.0 { 0.0 } else { (a + b) / 2.0 };
                ml[i] = m;
                mr[i] = m;
            }
        }
        // ograniczenie Fritscha–Carlsona na każdym odcinku (monotoniczność)
        for i in 0..n - 1 {
            if d[i].abs() < 1e-12 {
                mr[i] = 0.0;
                ml[i + 1] = 0.0;
                continue;
            }
            let (a, b) = (mr[i] / d[i], ml[i + 1] / d[i]);
            if a < 0.0 {
                mr[i] = 0.0;
            }
            if b < 0.0 {
                ml[i + 1] = 0.0;
            }
            let s = a * a + b * b;
            if s > 9.0 {
                let t = 3.0 / s.sqrt();
                mr[i] = t * a * d[i];
                ml[i + 1] = t * b * d[i];
            }
        }
        Krzywa { xs, ys, ml, mr }
    }

    pub fn x0(&self) -> f64 {
        self.xs[0]
    }
    pub fn x1(&self) -> f64 {
        self.xs[self.xs.len() - 1]
    }

    pub fn at(&self, x: f64) -> f64 {
        let n = self.xs.len();
        if n == 0 {
            return 0.0;
        }
        if n == 1 || x <= self.xs[0] {
            return self.ys[0];
        }
        if x >= self.xs[n - 1] {
            return self.ys[n - 1];
        }
        let i = self.xs.partition_point(|&v| v <= x).saturating_sub(1).min(n - 2);
        let (x0, x1) = (self.xs[i], self.xs[i + 1]);
        let h = x1 - x0;
        let t = (x - x0) / h;
        let (t2, t3) = (t * t, t * t * t);
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        h00 * self.ys[i] + h10 * h * self.mr[i] + h01 * self.ys[i + 1] + h11 * h * self.ml[i + 1]
    }

    /// Nachylenie dy/dx (numerycznie, środkowa różnica).
    pub fn slope(&self, x: f64) -> f64 {
        let e = 1e-4;
        (self.at(x + e) - self.at(x - e)) / (2.0 * e)
    }

    /// Położenia węzłów (do wstawiania żeber siatki na liniach cech).
    pub fn wezly(&self) -> &[f64] {
        &self.xs
    }
}

/// Zaokrągla naroża łamanej (otwartej) łukami o promieniu do `r` (mniejszym, gdy odcinki są krótkie),
/// każdy łuk z `seg` odcinków. Końce łamanej zostają.
pub fn zaokraglij(pts: &[[f64; 2]], r: f64, seg: usize) -> Vec<[f64; 2]> {
    if pts.len() < 3 || r <= 0.0 || seg == 0 {
        return pts.to_vec();
    }
    let mut out = vec![pts[0]];
    for i in 1..pts.len() - 1 {
        let (a, b, c) = (pts[i - 1], pts[i], pts[i + 1]);
        let (u, v) = ([a[0] - b[0], a[1] - b[1]], [c[0] - b[0], c[1] - b[1]]);
        let (lu, lv) = (u[0].hypot(u[1]), v[0].hypot(v[1]));
        if lu < 1e-9 || lv < 1e-9 {
            out.push(b);
            continue;
        }
        let (u, v) = ([u[0] / lu, u[1] / lu], [v[0] / lv, v[1] / lv]);
        let cos = (u[0] * v[0] + u[1] * v[1]).clamp(-1.0, 1.0);
        let half = cos.acos() / 2.0; // połowa kąta wewnętrznego
        if half > 1.55 || half < 1e-3 {
            out.push(b);
            continue;
        }
        // odległość punktów styczności od wierzchołka, przycięta do połowy krótszego odcinka
        let mut t = r / half.tan();
        t = t.min(0.5 * lu).min(0.5 * lv);
        let rr = t * half.tan();
        let p0 = [b[0] + u[0] * t, b[1] + u[1] * t];
        let p1 = [b[0] + v[0] * t, b[1] + v[1] * t];
        let bis = [u[0] + v[0], u[1] + v[1]];
        let lb = bis[0].hypot(bis[1]);
        let dc = rr / half.sin();
        let cen = [b[0] + bis[0] / lb * dc, b[1] + bis[1] / lb * dc];
        let a0 = (p0[1] - cen[1]).atan2(p0[0] - cen[0]);
        let mut a1 = (p1[1] - cen[1]).atan2(p1[0] - cen[0]);
        let mut da = a1 - a0;
        if da > std::f64::consts::PI {
            a1 -= 2.0 * std::f64::consts::PI;
        } else if da < -std::f64::consts::PI {
            a1 += 2.0 * std::f64::consts::PI;
        }
        da = a1 - a0;
        for k in 0..=seg {
            let a = a0 + da * k as f64 / seg as f64;
            out.push([cen[0] + rr * a.cos(), cen[1] + rr * a.sin()]);
        }
    }
    out.push(pts[pts.len() - 1]);
    out
}

/// Superelipsa |x/a|^n + |y/b|^n = 1: dla `x` w [0, a] zwraca y ≥ 0 (n = 2 elipsa, n → ∞ prostokąt).
pub fn superelipsa(a: f64, b: f64, n: f64, x: f64) -> f64 {
    let t = (x / a).abs().min(1.0);
    b * (1.0 - t.powf(n)).max(0.0).powf(1.0 / n)
}

/// Wygładzony krok 0→1 na [e0, e1].
pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hermite_goes_through_knots_without_overshoot() {
        let k = Krzywa::new(&[g(0.0, 0.0), g(1.0, 1.0), g(2.0, 1.0), g(3.0, 0.2)]);
        for (x, y) in [(0.0, 0.0), (1.0, 1.0), (2.0, 1.0), (3.0, 0.2)] {
            assert!((k.at(x) - y).abs() < 1e-9);
        }
        for i in 0..=300 {
            let x = i as f64 / 100.0;
            let y = k.at(x);
            assert!((-1e-9..=1.0 + 1e-9).contains(&y), "przestrzelenie {y} przy {x}");
        }
        // płaski odcinek zostaje płaski
        assert!((k.at(1.5) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn sharp_knot_breaks_tangent() {
        let k = Krzywa::new(&[g(0.0, 0.0), o(1.0, 1.0), g(2.0, 0.0)]);
        let (l, r) = ((k.at(1.0) - k.at(0.999)) / 0.001, (k.at(1.001) - k.at(1.0)) / 0.001);
        assert!(l > 0.5 && r < -0.5, "{l} {r}");
    }

    #[test]
    fn fillet_keeps_ends_and_stays_inside() {
        let p = zaokraglij(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], 0.2, 4);
        assert_eq!(p[0], [0.0, 0.0]);
        assert_eq!(*p.last().unwrap(), [1.0, 1.0]);
        assert_eq!(p.len(), 2 + 5);
        for q in &p {
            assert!(q[0] <= 1.0 + 1e-9 && q[1] >= -1e-9);
        }
        // łuk o środku (0,8; 0,2): punkt środkowy ok. (0,941; 0,059)
        let m = p[3];
        assert!((m[0] - (0.8 + 0.2 * std::f64::consts::FRAC_1_SQRT_2)).abs() < 1e-6, "{m:?}");
    }
}
