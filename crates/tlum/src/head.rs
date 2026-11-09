//! Głowa: rzeźbiona siatka (8 pierścieni × 10 kolumn, wzorzec dorosłego mężczyzny w metrach,
//! y od podbródka), twarz z płatów położonych na powierzchni (białko + tęczówka, powieka, brwi,
//! usta), nos i uszy jako bryły, włosy i nakrycia głowy jako skorupy nad czaszką, zarost jako
//! cienka warstwa na żuchwie. Wszystko w kości `Head`, w pozie spoczynkowej.

use crate::skel::{idx, Joint, Scale};
use crate::P;
use procedura_kit::mesh::{add, cross, len, norm, scale, sub, MatKey, MeshBuilder, V3};
use procedura_kit::random::hash01_seeded;

const H0: f64 = 0.237;
const ROWS: usize = 8;
const COLS: usize = 10;
/// Pół-pierścienie (kolumny C0 przód … C5 tył) dla x ≥ 0: (x, y, z). Druga połowa to lustro C4…C1.
/// Wzorzec dorosłego mężczyzny: szeroka żuchwa z wyraźnym kątem, pełny podbródek.
const TPL: [[V3; 6]; ROWS] = [
    // R0 dolny obrys żuchwy (podbródek → kąt żuchwy → kark)
    [[0.0, 0.0, 0.078], [0.028, 0.004, 0.073], [0.05, 0.02, 0.052], [0.06, 0.045, 0.006], [0.05, 0.05, -0.054], [0.0, 0.05, -0.07]],
    // R1 podbródek / warga dolna
    [[0.0, 0.028, 0.095], [0.024, 0.03, 0.09], [0.052, 0.042, 0.062], [0.066, 0.068, 0.01], [0.062, 0.075, -0.072], [0.0, 0.075, -0.09]],
    // R2 usta
    [[0.0, 0.058, 0.103], [0.024, 0.058, 0.094], [0.054, 0.064, 0.066], [0.069, 0.088, 0.01], [0.068, 0.1, -0.082], [0.0, 0.1, -0.1]],
    // R3 podstawa nosa, policzki
    [[0.0, 0.08, 0.1], [0.025, 0.084, 0.093], [0.059, 0.088, 0.068], [0.073, 0.1, 0.008], [0.073, 0.124, -0.085], [0.0, 0.124, -0.104]],
    // R4 oczy (oczodół cofnięty)
    [[0.0, 0.116, 0.095], [0.031, 0.117, 0.079], [0.062, 0.115, 0.06], [0.078, 0.122, 0.004], [0.077, 0.148, -0.083], [0.0, 0.152, -0.102]],
    // R5 łuki brwiowe
    [[0.0, 0.138, 0.098], [0.032, 0.143, 0.092], [0.066, 0.14, 0.064], [0.08, 0.146, 0.002], [0.078, 0.172, -0.078], [0.0, 0.177, -0.1]],
    // R6 czoło
    [[0.0, 0.182, 0.088], [0.035, 0.182, 0.083], [0.069, 0.182, 0.056], [0.081, 0.184, -0.002], [0.074, 0.198, -0.072], [0.0, 0.201, -0.09]],
    // R7 sklepienie
    [[0.0, 0.216, 0.06], [0.031, 0.217, 0.056], [0.058, 0.218, 0.04], [0.066, 0.219, -0.004], [0.057, 0.225, -0.052], [0.0, 0.227, -0.066]],
];
/// Wzorzec dziecka (ok. 5 lat): duża okrągła czaszka, twarz niżej i krótsza, pełne policzki, mała żuchwa.
const TPL_CHILD: [[V3; 6]; ROWS] = [
    [[0.0, 0.0, 0.064], [0.03, 0.005, 0.062], [0.052, 0.02, 0.048], [0.06, 0.042, 0.01], [0.052, 0.05, -0.05], [0.0, 0.05, -0.07]],
    [[0.0, 0.024, 0.084], [0.03, 0.026, 0.082], [0.06, 0.036, 0.066], [0.072, 0.06, 0.014], [0.066, 0.07, -0.07], [0.0, 0.072, -0.092]],
    [[0.0, 0.048, 0.092], [0.026, 0.048, 0.088], [0.059, 0.054, 0.071], [0.074, 0.078, 0.014], [0.073, 0.096, -0.084], [0.0, 0.098, -0.104]],
    [[0.0, 0.068, 0.094], [0.028, 0.07, 0.09], [0.063, 0.074, 0.073], [0.078, 0.09, 0.012], [0.078, 0.12, -0.088], [0.0, 0.122, -0.108]],
    [[0.0, 0.1, 0.092], [0.032, 0.102, 0.08], [0.065, 0.1, 0.064], [0.081, 0.108, 0.006], [0.081, 0.145, -0.088], [0.0, 0.15, -0.108]],
    [[0.0, 0.124, 0.094], [0.034, 0.128, 0.09], [0.068, 0.126, 0.066], [0.084, 0.132, 0.002], [0.083, 0.17, -0.084], [0.0, 0.176, -0.106]],
    [[0.0, 0.175, 0.094], [0.037, 0.175, 0.089], [0.072, 0.176, 0.06], [0.085, 0.178, -0.002], [0.078, 0.198, -0.078], [0.0, 0.2, -0.096]],
    [[0.0, 0.214, 0.072], [0.033, 0.216, 0.066], [0.061, 0.217, 0.044], [0.07, 0.218, -0.004], [0.061, 0.225, -0.056], [0.0, 0.227, -0.07]],
];
const APEX: V3 = [0.0, H0, -0.002];
const CENTER: V3 = [0.0, 0.118, -0.004];

/// Indeks półkolumny (0…5) dla kolumny pełnego pierścienia i znak x.
fn half(j: usize) -> (usize, f64) {
    if j <= 5 { (j, 1.0) } else { (COLS - j, -1.0) }
}

struct Head {
    lat: [[V3; COLS]; ROWS],
    bx: f64,
    by: f64,
    bz: f64,
    chin: f64,
    tris: Vec<[V3; 3]>,
}

impl Head {
    fn to(&self, p: V3) -> V3 {
        [p[0] * self.bx, self.chin + p[1] * self.by, p[2] * self.bz]
    }
    fn dir(&self, v: V3) -> V3 {
        [v[0] / self.bx, v[1] / self.by, v[2] / self.bz]
    }
    fn poly(&self, mb: &mut MeshBuilder, mat: MatKey, pts: &[V3], want: V3) {
        let v: Vec<V3> = pts.iter().map(|&p| self.to(p)).collect();
        mb.poly(mat, &v, Some(self.dir(want)));
    }
    fn loft(&self, mb: &mut MeshBuilder, mat: MatKey, rings: &[Vec<V3>]) {
        let r: Vec<Vec<V3>> = rings.iter().map(|r| r.iter().map(|&p| self.to(p)).collect()).collect();
        mb.loft(mat, &r, false);
    }
    /// Najbardziej wysunięty punkt przedniej powierzchni głowy na (x, y): z.
    fn surf(&self, x: f64, y: f64) -> f64 {
        let mut best = f64::NEG_INFINITY;
        for t in &self.tris {
            let [a, b, c] = *t;
            let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
            if d.abs() < 1e-12 {
                continue;
            }
            let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / d;
            let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / d;
            let l3 = 1.0 - l1 - l2;
            if l1 >= -1e-6 && l2 >= -1e-6 && l3 >= -1e-6 {
                best = best.max(l1 * a[2] + l2 * b[2] + l3 * c[2]);
            }
        }
        if best.is_finite() { best } else { 0.06 }
    }
    fn on(&self, x: f64, y: f64, off: f64) -> V3 {
        [x, y, self.surf(x, y) + off]
    }
    /// Punkt kolumny `j` na wysokości `y` (powyżej R7 — ku wierzchołkowi).
    fn col(&self, j: usize, y: f64) -> V3 {
        let c: Vec<V3> = (0..ROWS).map(|r| self.lat[r][j]).chain([APEX]).collect();
        let i = c.windows(2).position(|w| y <= w[1][1]).unwrap_or(c.len() - 2);
        let (a, b) = (c[i], c[i + 1]);
        let t = ((y - a[1]) / (b[1] - a[1]).max(1e-9)).clamp(if i == 0 { -2.0 } else { 0.0 }, 1.0);
        add(a, scale(sub(b, a), t))
    }
    /// Punkt kolumny `j` w ułamku `t` długości łuku między wysokościami `y0` i `y1` (t < 0: liniowo poniżej `y0`).
    fn col_arc(&self, j: usize, y0: f64, y1: f64, t: f64) -> V3 {
        if t <= 0.0 {
            return self.col(j, y0 + (y1 - y0) * t);
        }
        let mut pts = vec![self.col(j, y0)];
        pts.extend((0..ROWS).map(|r| self.lat[r][j]).filter(|q| q[1] > y0 && q[1] < y1));
        pts.push(self.col(j, y1));
        let total: f64 = pts.windows(2).map(|w| len(sub(w[1], w[0]))).sum();
        let mut left = t.min(1.0) * total;
        for w in pts.windows(2) {
            let l = len(sub(w[1], w[0]));
            if left <= l && l > 0.0 {
                return add(w[0], scale(sub(w[1], w[0]), left / l));
            }
            left -= l;
        }
        *pts.last().unwrap()
    }
    fn radial(p: V3) -> V3 {
        norm(sub(p, CENTER))
    }
}

/// Skorupa nad czaszką: dolna linia (y na półkolumnę), wiersze (ułamek do wierzchołka, odsunięcie), grzebień.
struct Shell {
    lower: [f64; 6],
    rows: Vec<(f64, f64)>,
    apex: Option<f64>,
    comb: f64,
    /// Przesunięcie czubka do przodu (kaszkiet).
    fwd: f64,
}

fn emit_shell(h: &Head, mb: &mut MeshBuilder, s: &Shell, mat: MatKey) -> Vec<Vec<V3>> {
    let rings: Vec<Vec<V3>> = s
        .rows
        .iter()
        .map(|&(t, off)| {
            (0..COLS)
                .map(|j| {
                    let (hj, _) = half(j);
                    let top = h.lat[ROWS - 1][j][1] + 0.006;
                    let p = h.col_arc(j, s.lower[hj], top, t);
                    let mid = if hj == 0 || hj == 5 { s.comb * t * t } else { 0.0 };
                    add(add(p, scale(Head::radial(p), off + mid)), [0.0, 0.0, s.fwd * t])
                })
                .collect()
        })
        .collect();
    h.loft(mb, mat, &rings);
    if let Some(off) = s.apex {
        let a = add(APEX, [0.0, off + s.comb * 0.8, s.fwd]);
        let last = rings.last().unwrap();
        for j in 0..COLS {
            let (p, q) = (last[j], last[(j + 1) % COLS]);
            h.poly(mb, mat, &[p, q, a], sub(scale(add(add(p, q), a), 1.0 / 3.0), CENTER));
        }
    }
    rings
}

pub(crate) fn build(mb: &mut MeshBuilder, sk: &[Joint], sc: &Scale, p: &P, seed: u32) {
    let hd = &sk[idx(sk, "Head")];
    let crown = hd.tail[1];
    let hh = sc.hh;
    let age = p.f("age");
    let adult = ((age - 11.0) / 6.0).clamp(0.0, 1.0);
    let male = 0.5 + (p.f("sex").clamp(0.0, 1.0) - 0.5) * adult;
    let fem = 1.0 - male;
    let c = sc.child;
    let fat = ((p.f("bmi") - 22.5) / 8.0).clamp(-0.6, 1.5);
    // drobne różnice rysów z ziarna: szerokość żuchwy, nosa, rozstaw oczu, długość podbródka
    let var = |k: f64| hash01_seeded(seed, &[k]) * 2.0 - 1.0;
    let (v_jaw, v_nose, v_eye, v_chin) = (1.0 + 0.04 * var(1.0), 1.0 + 0.12 * var(2.0), 1.0 + 0.05 * var(3.0), 0.004 * var(4.0));
    let by = hh / H0;
    let bw = by * (1.0 + 0.08 * c) * (1.0 - 0.03 * fem);
    let mut lat = [[[0.0; 3]; COLS]; ROWS];
    for r in 0..ROWS {
        for j in 0..COLS {
            let (hj, sx) = half(j);
            let [mut x, mut y, mut z] = TPL[r][hj];
            // kobieta: węższa, łagodna żuchwa, krótszy podbródek, mniejsze łuki brwiowe
            if r <= 1 && (2..=3).contains(&hj) {
                x *= (1.0 - 0.1 * fem) * v_jaw;
            }
            if r <= 1 && hj <= 1 {
                z -= 0.005 * fem;
                y += 0.004 * fem * (1.0 - r as f64);
                y -= v_chin * (1.0 - r as f64);
            }
            if r == 5 && hj <= 1 {
                z -= 0.003 * fem;
            }
            // tusza: pełniejsze policzki i żuchwa, podgardle
            if r <= 3 && (1..=3).contains(&hj) {
                x += 0.006 * fat.max(0.0) * if r <= 1 { 1.0 } else { 0.8 };
            }
            if r == 0 && hj <= 2 {
                y -= 0.006 * fat.max(0.0);
            }
            // dziecko: osobny wzorzec
            let q = TPL_CHILD[r][hj];
            let l = |a: f64, b: f64| a + (b - a) * c;
            lat[r][j] = [sx * l(x, q[0]), l(y, q[1]), l(z, q[2])];
        }
    }
    // cechy twarzy podane we współrzędnych wzorca dorosłego → wysokość w siatce (wiersze kolumny C0)
    let rows_y: Vec<(f64, f64)> = (0..ROWS).map(|r| (TPL[r][0][1], lat[r][0][1])).chain([(H0, H0)]).collect();
    let fy = |y: f64| {
        let i = rows_y.windows(2).position(|w| y <= w[1].0).unwrap_or(rows_y.len() - 2);
        let (a, b) = (rows_y[i], rows_y[i + 1]);
        a.1 + (b.1 - a.1) * (y - a.0) / (b.0 - a.0)
    };
    let mut h = Head { lat, bx: bw, by, bz: bw, chin: crown - hh, tris: vec![] };
    for r in 0..ROWS - 1 {
        for j in 0..COLS {
            let k = (j + 1) % COLS;
            let (a, b, cc, d) = (h.lat[r][j], h.lat[r][k], h.lat[r + 1][k], h.lat[r + 1][j]);
            if a[2] > -0.02 || b[2] > -0.02 {
                h.tris.push([a, b, cc]);
                h.tris.push([a, cc, d]);
            }
        }
    }

    // --- czaszka i twarz
    let rings: Vec<Vec<V3>> = h.lat.iter().map(|r| r.to_vec()).collect();
    h.loft(mb, MatKey::Skin, &rings);
    h.poly(mb, MatKey::Skin, &rings[0], [0.0, -1.0, 0.2]);
    for j in 0..COLS {
        let (a, b) = (rings[ROWS - 1][j], rings[ROWS - 1][(j + 1) % COLS]);
        h.poly(mb, MatKey::Skin, &[a, b, APEX], sub(scale(add(add(a, b), APEX), 1.0 / 3.0), CENTER));
    }

    let style = p.s("hairStyle");
    let head_type = p.s("headType");
    let covers_ears = matches!(style, "medium" | "dlugie" | "bob") || matches!(head_type, "czapka_zimowa" | "kaptur" | "helm_psp");
    let big_eye = 1.14 + 0.14 * c + 0.04 * fem;
    for sx in [-1.0f64, 1.0] {
        // oko: białko (migdał), tęczówka, linia rzęs górnej powieki
        let (ex, ey) = (sx * 0.032 * v_eye, fy(0.117));
        let e = |dx: f64, dy: f64, off: f64| h.on(ex + sx * dx * big_eye, ey + dy * big_eye, off);
        let white = [(-0.0145, -0.0006), (-0.0068, 0.0052), (0.0045, 0.006), (0.0142, 0.0016), (0.0062, -0.0042), (-0.0062, -0.0046)];
        let wp: Vec<V3> = white.iter().map(|&(dx, dy)| e(dx, dy, 0.0016 + 0.0018 * (1.0 - (dx / 0.0145).powi(2)))).collect();
        h.poly(mb, MatKey::Eye, &wp, [0.0, 0.0, 1.0]);
        let ir = 0.0054;
        let iris = [(-0.8 * ir, 0.0049), (-0.8 * ir, -0.0041), (0.8 * ir, -0.0041), (0.8 * ir, 0.0049)];
        let ip: Vec<V3> = iris.iter().map(|&(dx, dy)| e(dx - 0.0004, dy, 0.0042)).collect();
        h.poly(mb, MatKey::Face, &ip, [0.0, 0.0, 1.0]);
        let lid = [(-0.0152, -0.0004), (-0.001, 0.0062), (0.0148, 0.0018)];
        for w in lid.windows(2) {
            let (a, b) = (w[0], w[1]);
            let t = 0.0017;
            let q = [e(a.0, a.1 - t * 0.4, 0.0034), e(b.0, b.1 - t * 0.4, 0.0034), e(b.0, b.1 + t, 0.0034), e(a.0, a.1 + t, 0.0034)];
            h.poly(mb, MatKey::Face, &q, [0.0, 0.0, 1.0]);
        }
        // brew: pas od nasady nosa łukiem ku skroni
        let arch = 0.002 * fem;
        let bt = 1.0 - 0.35 * fem;
        let brow = [(0.012, 0.134, 0.0062), (0.031, 0.1405 + arch, 0.005), (0.054, 0.137 + arch, 0.0028)];
        for w in brow.windows(2) {
            let (a, b) = (w[0], w[1]);
            let q = [h.on(sx * a.0, fy(a.1) - a.2 * bt * 0.5, 0.0014), h.on(sx * b.0, fy(b.1) - b.2 * bt * 0.5, 0.0014), h.on(sx * b.0, fy(b.1) + b.2 * bt * 0.5, 0.0014), h.on(sx * a.0, fy(a.1) + a.2 * bt * 0.5, 0.0014)];
            h.poly(mb, MatKey::Hair, &q, [0.0, 0.2, 1.0]);
        }
        // ucho: płaski płat odchylony do tyłu (przód przy czaszce), od oczu do podstawy nosa
        if !covers_ears {
            // ucho ok. 15% większe niż w pierwszej wersji, nasada przy powierzchni czaszki
            let ear = |x: f64, y: f64, z: f64| [sx * (0.077 + (x - 0.072) * 1.15), fy(0.118 + (y - 0.118) * 1.15), -0.01 + (z + 0.01) * 1.15];
            let (a, b) = (ear(0.073, 0.147, -0.008), ear(0.07, 0.09, -0.002));
            let (cc, d, ee) = (ear(0.085, 0.152, -0.03), ear(0.091, 0.122, -0.042), ear(0.08, 0.087, -0.018));
            let out = [sx, 0.0, 0.25];
            for t in [[a, cc, d], [a, d, ee], [a, ee, b]] {
                h.poly(mb, MatKey::Skin, &t, out);
            }
        }
    }
    // nos: nasada, grzbiet, czubek, skrzydełka, podstawa
    let ns = (1.0 - 0.28 * c) * (1.0 - 0.08 * fem);
    let nz = |x: f64, y: f64, out: f64| h.on(x * ns, fy(y), out * ns);
    let (root, mid, tip) = (nz(0.0, 0.128, 0.002), nz(0.0, 0.106, 0.012), [0.0, fy(0.089), h.surf(0.0, fy(0.089)) + 0.026 * ns]);
    let (sl, sr) = (nz(0.011, 0.109, 0.0), nz(-0.011, 0.109, 0.0));
    let (al, ar) = (nz(0.018 * v_nose, 0.085, 0.004), nz(-0.018 * v_nose, 0.085, 0.004));
    let base = nz(0.0, 0.08, 0.006);
    let inside = [0.0, fy(0.1), 0.06];
    for t in [vec![root, sl, mid], vec![root, mid, sr], vec![sl, al, tip, mid], vec![sr, mid, tip, ar], vec![al, base, tip], vec![ar, tip, base]] {
        let cen = scale(t.iter().fold([0.0; 3], |a, &q| add(a, q)), 1.0 / t.len() as f64);
        h.poly(mb, MatKey::Skin, &t, sub(cen, inside));
    }
    // usta: ciemna szczelina między dwiema wargami (górna z łukiem Kupidyna, dolna pełniejsza), lekko wysunięte
    let lw = 0.0245 * (1.0 + 0.04 * fem) * (1.0 - 0.12 * c);
    let my = fy(0.0576);
    let beard = p.s("beard");
    let boff = match beard {
        "krotka_broda" => 0.006,
        "pelna_broda" => 0.011,
        "kozia" => 0.006,
        _ => 0.0,
    };
    let (lp, lh) = ((1.0 + 0.25 * fem) * (1.0 - 0.2 * c), (1.0 + 0.15 * fem) * (1.0 - 0.15 * c));
    // wargi w slocie zarostu: bez zarostu ma on odcień warg (lib.rs), z kilkudniowym — cień zarostu
    let lips = if matches!(beard, "brak" | "kilkudniowy") { MatKey::Stubble } else { MatKey::Skin };
    let mo = |x: f64, y: f64, off: f64| h.on(x, my + y, off * lp + boff);
    h.poly(mb, MatKey::Face, &[mo(-lw, 0.0004, 0.002), mo(-0.011, -0.0012, 0.0042), mo(0.011, -0.0012, 0.0042), mo(lw, 0.0004, 0.002), mo(0.011, 0.0012, 0.0042), mo(-0.011, 0.0012, 0.0042)], [0.0, 0.0, 1.0]);
    for sx in [-1.0f64, 1.0] {
        let c0 = mo(sx * lw, 0.0004, 0.0012);
        let (b1, b0) = (mo(sx * 0.011, 0.0013, 0.0045), mo(0.0, 0.0013, 0.005));
        let (t0, t1) = (mo(0.0, 0.0066 * lh, 0.0026), mo(sx * 0.011, 0.0058 * lh, 0.0022));
        h.poly(mb, lips, &[c0, b1, b0, t0], [0.0, 0.3, 1.0]);
        h.poly(mb, lips, &[c0, t0, t1], [0.0, 0.5, 1.0]);
        let (d1, d0) = (mo(sx * 0.011, -0.0015, 0.0046), mo(0.0, -0.0015, 0.0052));
        let (e0, e1) = (mo(0.0, -0.0088 * lh, 0.0014), mo(sx * 0.012, -0.0076 * lh, 0.0012));
        h.poly(mb, lips, &[c0, d1, d0, e0], [0.0, -0.2, 1.0]);
        h.poly(mb, lips, &[c0, e0, e1], [0.0, -0.5, 1.0]);
    }

    // --- zarost: cienka warstwa na żuchwie (wierzchołki siatki odsunięte promieniowo)
    let region: Option<(MatKey, f64, Vec<(usize, usize)>)> = {
        let jaw: Vec<(usize, usize)> = (0..=2).flat_map(|r| (0..=3).map(move |c| (r, c))).collect();
        let stache = vec![(2, 0), (2, 1), (3, 0), (3, 1)];
        let mut all = jaw.clone();
        all.extend(stache.iter().copied());
        match beard {
            "kilkudniowy" => Some((MatKey::Stubble, 0.0018, all)),
            "krotka_broda" => Some((MatKey::Hair, 0.006, all)),
            "pelna_broda" => {
                let mut v = all;
                v.extend([(3, 2), (3, 3)]);
                Some((MatKey::Hair, 0.011, v))
            }
            "wasy" => Some((MatKey::Hair, 0.004, stache)),
            "kozia" => Some((MatKey::Hair, 0.006, vec![(0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (2, 1), (3, 0), (3, 1)])),
            _ => None,
        }
    };
    if let Some((mat, off, cells)) = region {
        let has = |r: usize, j: usize| cells.contains(&(r, half(j).0));
        let lift = |r: usize, j: usize| {
            let q = h.lat[r][j];
            // normalna wierzchołka siatki (styczna kolumny × w górę), żeby warstwa nie wchodziła w skórę
            let up = sub(h.lat[(r + 1).min(ROWS - 1)][j], h.lat[r.saturating_sub(1)][j]);
            let tang = sub(h.lat[r][(j + 1) % COLS], h.lat[r][(j + COLS - 1) % COLS]);
            let n = norm(cross(tang, up));
            // brzeg obszaru nieco niżej (miękkie przejście), ale bez współpłaszczyznowości ze skórą
            let edge = !(has(r + 1, j) || r == 3) || r == 3 && half(j).0 >= 2;
            let o = if edge { (off * 0.55).max(0.0016) } else { off };
            let drop = if beard == "pelna_broda" && r == 0 && half(j).0 <= 1 { 0.012 } else { 0.0 };
            add(add(q, scale(n, o)), [0.0, -drop, drop * 0.3])
        };
        for r in 0..3 {
            for j in 0..COLS {
                let k = (j + 1) % COLS;
                if has(r, j) && has(r, k) && has(r + 1, j) && has(r + 1, k) {
                    let q = [lift(r, j), lift(r, k), lift(r + 1, k), lift(r + 1, j)];
                    let n = Head::radial(scale(add(q[0], q[2]), 0.5));
                    h.poly(mb, mat, &[q[0], q[1], q[2]], n);
                    h.poly(mb, mat, &[q[0], q[2], q[3]], n);
                }
            }
        }
    }

    // --- okulary
    let g = p.s("glasses");
    if matches!(g, "korekcyjne" | "przeciwsloneczne") {
        let (ey, z) = (fy(0.117), h.surf(0.0, fy(0.117)).max(h.surf(0.032, fy(0.128))) + 0.004);
        let (x0, x1, y0, y1) = (0.013, 0.053, ey - 0.012, ey + 0.011);
        for sx in [-1.0f64, 1.0] {
            let q = |x: f64, y: f64| [sx * x, y, z - (x - 0.03).max(0.0) * 0.35];
            if g == "przeciwsloneczne" {
                h.poly(mb, MatKey::Face, &[q(x0, y0), q(x1, y0), q(x1, y1), q(x0, y1)], [0.0, 0.0, 1.0]);
            } else {
                let t = 0.0026;
                for (a0, a1, b0, b1) in [(x0, x1, y1 - t, y1), (x0, x1, y0, y0 + t), (x0, x0 + t, y0, y1), (x1 - t, x1, y0, y1)] {
                    h.poly(mb, MatKey::Face, &[q(a0, b0), q(a1, b0), q(a1, b1), q(a0, b1)], [0.0, 0.0, 1.0]);
                }
            }
            // zausznik
            let a = q(x1, y1);
            let b = [sx * 0.078, y1 + 0.004, -0.02];
            h.poly(mb, MatKey::Face, &[a, b, add(b, [0.0, -0.003, 0.0]), add(a, [0.0, -0.003, 0.0])], [sx, 0.0, 0.0]);
        }
        h.poly(mb, MatKey::Face, &[[-x0, y1 - 0.004, z], [x0, y1 - 0.004, z], [x0, y1 - 0.0015, z], [-x0, y1 - 0.0015, z]], [0.0, 0.0, 1.0]);
    }

    // --- włosy
    let short_line = [0.2, 0.197, 0.186, 0.145, 0.085, 0.066];
    let long_line = [0.2, 0.196, 0.17, 0.1, 0.045, 0.035];
    // (dolna linia, początek, grubość w połowie, grubość u góry); pod nakryciem tylko dolny pas
    let spec: Option<([f64; 6], f64, f64, f64)> = match style {
        "buzz" => Some((short_line, 0.0, 0.0035, 0.004)),
        "fade" => Some(([0.2, 0.197, 0.19, 0.16, 0.12, 0.105], 0.0, 0.004, 0.011)),
        "krotkie_klasyczne" => Some((short_line, 0.0, 0.009, 0.012)),
        "kok" | "kucyk" => Some(([0.205, 0.2, 0.18, 0.125, 0.1, 0.09], 0.0, 0.005, 0.007)),
        "medium" | "dlugie" => Some((long_line, 0.0, 0.013, 0.015)),
        "bob" => Some(([0.19, 0.185, 0.14, 0.06, 0.045, 0.045], -0.15, 0.017, 0.016)),
        "lysiejacy" => Some(([0.22, 0.215, 0.17, 0.125, 0.105, 0.095], 0.0, 0.006, 0.0)),
        _ => None,
    };
    let covered = head_type != "brak";
    let hair: Option<Shell> = spec.map(|(lower, t0, mid, top)| {
        let rows = if style == "lysiejacy" {
            vec![(t0, 0.0015), (0.35, mid), (0.55, 0.0015)]
        } else if covered {
            vec![(t0, 0.0015), (0.33, mid), (0.6, top)]
        } else {
            vec![(t0, 0.0015), (0.33, mid), (0.67, top), (1.0, top)]
        };
        let apex = (!covered && style != "lysiejacy").then_some(top);
        Shell { lower: lower.map(fy), rows, apex, comb: 0.0, fwd: 0.0 }
    });
    let hair_off = hair.as_ref().map_or(0.0, |s| s.rows.iter().map(|r| r.1).fold(0.0, f64::max)).max(spec.map_or(0.0, |s| s.3));
    if let Some(s) = &hair {
        emit_shell(&h, mb, s, MatKey::Hair);
    }
    let hat_low = matches!(head_type, "czapka_zimowa" | "kaptur" | "helm_psp");
    match style {
        "dlugie" | "bob" | "kucyk" => {
            let (len, w0, w1) = match style {
                "dlugie" => (0.2, 0.068, 0.05),
                "bob" => (0.035, 0.07, 0.066),
                _ => (0.17, 0.022, 0.012),
            };
            let top = if style == "kucyk" { h.col(5, fy(0.17)) } else { h.col(5, fy(0.11)) };
            let z0 = top[2] - if style == "kucyk" { 0.02 } else { 0.004 };
            let ring = |y: f64, w: f64, d: f64, z: f64| -> Vec<V3> {
                (0..6).map(|i| {
                    let a = -std::f64::consts::TAU * i as f64 / 6.0;
                    [w * a.sin(), y, z + d * a.cos()]
                }).collect()
            };
            if !(hat_low && style == "kucyk") {
                let d0 = if style == "kucyk" { w0 } else { 0.03 };
                let rings = vec![ring(top[1], w0, d0, z0 + if style == "kucyk" { 0.0 } else { 0.012 }), ring(top[1] - len * 0.5, (w0 + w1) * 0.55, d0 * 0.9, z0 - 0.008), ring(top[1] - len, w1, d0 * 0.6, z0 - 0.006)];
                h.loft(mb, MatKey::Hair, &rings);
                h.poly(mb, MatKey::Hair, rings.last().unwrap(), [0.0, -1.0, 0.0]);
            }
        }
        "kok" if head_type == "brak" => {
            let c0 = add(h.col(5, fy(0.19)), [0.0, 0.0, -0.022]);
            let ring = |dy: f64, r: f64| -> Vec<V3> {
                (0..6).map(|i| {
                    let a = std::f64::consts::TAU * i as f64 / 6.0;
                    [c0[0] + r * a.sin(), c0[1] + dy, c0[2] + r * a.cos()]
                }).collect()
            };
            let rings = vec![ring(-0.024, 0.02), ring(0.0, 0.032), ring(0.024, 0.02)];
            h.loft(mb, MatKey::Hair, &rings);
            h.poly(mb, MatKey::Hair, &rings[0], [0.0, -1.0, 0.0]);
            h.poly(mb, MatKey::Hair, &rings[2], [0.0, 1.0, 0.0]);
        }
        _ => {}
    }

    // --- nakrycia głowy
    let o = hair_off + 0.004;
    let line = |v: [f64; 6]| v.map(fy);
    let visor = |mb: &mut MeshBuilder, ring: &[V3], len: f64, drop: f64, mat: MatKey| {
        // daszek od dolnej krawędzi przodu (kolumny C2' … C2)
        let idx = [COLS - 2, COLS - 1, 0, 1, 2];
        let pts: Vec<V3> = idx.iter().map(|&j| ring[j]).collect();
        let tip: Vec<V3> = pts.iter().map(|&q| add(q, [q[0] * 0.15, -drop, len * (1.0 - q[0].abs() * 6.0).max(0.45)])).collect();
        for i in 0..4 {
            h.poly(mb, mat, &[pts[i], pts[i + 1], tip[i + 1], tip[i]], [0.0, 1.0, 0.1]);
        }
    };
    match head_type {
        "czapka_daszek" | "czapka_mundurowa" => {
            let s = Shell { lower: line([0.178, 0.176, 0.17, 0.152, 0.13, 0.122]), rows: vec![(0.0, o + 0.002), (0.4, o + 0.008), (0.72, o + 0.01), (1.0, o + 0.009)], apex: Some(o + 0.008), comb: 0.0, fwd: 0.0 };
            let r = emit_shell(&h, mb, &s, MatKey::Helmet);
            visor(mb, &r[0], 0.068, 0.012, MatKey::Helmet);
        }
        "czapka_policji_rd" => {
            // czapka garnizonowa: otok, szerokie denko, czarny daszek
            let lo = line([0.168, 0.167, 0.165, 0.16, 0.158, 0.158]);
            let band = Shell { lower: lo, rows: vec![(0.0, o + 0.004), (0.32, o + 0.007)], apex: None, comb: 0.0, fwd: 0.0 };
            let r = emit_shell(&h, mb, &band, MatKey::Outer);
            let top: Vec<V3> = r[1].iter().map(|&q| {
                let d = norm([q[0], 0.0, q[2] - 0.004]);
                add(q, [d[0] * 0.024, 0.016 + q[2].max(0.0) * 0.12, d[2] * 0.024 + 0.006])
            }).collect();
            h.loft(mb, MatKey::Helmet, &[r[1].clone(), top.clone()]);
            h.poly(mb, MatKey::Helmet, &top, [0.0, 1.0, 0.0]);
            visor(mb, &r[0], 0.06, 0.02, MatKey::Face);
        }
        "kaszkiet" => {
            // kaszkiet: niska główka nasunięta do przodu, krótki daszek
            let s = Shell { lower: line([0.176, 0.174, 0.168, 0.152, 0.132, 0.126]), rows: vec![(0.0, o + 0.003), (0.45, o + 0.012), (1.0, o + 0.004)], apex: Some(o), comb: 0.0, fwd: 0.022 };
            let r = emit_shell(&h, mb, &s, MatKey::Helmet);
            visor(mb, &r[0], 0.045, 0.012, MatKey::Helmet);
        }
        "czapka_zimowa" => {
            let lo = line([0.168, 0.164, 0.15, 0.11, 0.095, 0.095]);
            emit_shell(&h, mb, &Shell { lower: lo, rows: vec![(0.0, o + 0.012), (0.18, o + 0.014), (0.2, o + 0.009), (0.6, o + 0.011), (1.0, o + 0.01)], apex: Some(o + 0.014), comb: 0.0, fwd: 0.0 }, MatKey::Helmet);
        }
        "beret" => {
            let s = Shell { lower: line([0.185, 0.182, 0.176, 0.165, 0.158, 0.156]), rows: vec![(0.0, o + 0.003), (0.5, o + 0.022)], apex: None, comb: 0.0, fwd: 0.0 };
            let r = emit_shell(&h, mb, &s, MatKey::Helmet);
            let top: Vec<V3> = r[1].iter().map(|&q| add(q, [q[0] * 0.1 + 0.012, 0.016, q[2] * 0.06])).collect();
            h.loft(mb, MatKey::Helmet, &[r[1].clone(), top.clone()]);
            h.poly(mb, MatKey::Helmet, &top, [0.0, 1.0, 0.0]);
        }
        "kaptur" => {
            let lo = line([0.205, 0.19, 0.11, 0.03, 0.0, 0.0]);
            emit_shell(&h, mb, &Shell { lower: lo, rows: vec![(0.0, o + 0.012), (0.3, o + 0.022), (0.65, o + 0.024), (1.0, o + 0.024)], apex: Some(o + 0.024), comb: 0.0, fwd: 0.0 }, MatKey::Top);
        }
        "kask_budowlany" => {
            // kask EN 397: kopuła z grzebieniem, wąskie rondo dookoła, krótki daszek
            let lo = line([0.176, 0.174, 0.17, 0.162, 0.152, 0.148]);
            let s = Shell { lower: lo, rows: vec![(0.0, o + 0.018), (0.3, o + 0.026), (0.65, o + 0.028), (1.0, o + 0.028)], apex: Some(o + 0.022), comb: 0.008, fwd: 0.0 };
            let r = emit_shell(&h, mb, &s, MatKey::Helmet);
            let rim: Vec<V3> = r[0].iter().map(|&q| add(add(q, scale(norm([q[0], 0.0, q[2] - 0.002]), 0.016)), [0.0, -0.006, 0.0])).collect();
            h.loft(mb, MatKey::Helmet, &[rim, r[0].clone()]);
            visor(mb, &r[0], 0.04, 0.012, MatKey::Helmet);
        }
        "helm_psp" => {
            // hełm strażacki: wysoka kopuła z grzebieniem, osłona karku, krótki daszek
            let lo = line([0.172, 0.17, 0.158, 0.118, 0.078, 0.07]);
            let s = Shell { lower: lo, rows: vec![(0.0, o + 0.022), (0.22, o + 0.03), (0.6, o + 0.034), (1.0, o + 0.034)], apex: Some(o + 0.032), comb: 0.016, fwd: 0.0 };
            let r = emit_shell(&h, mb, &s, MatKey::Helmet);
            let guard: Vec<V3> = r[0].iter().map(|&q| {
                let back = (-q[2] / 0.1).clamp(0.0, 1.0);
                add(add(q, scale(norm([q[0], 0.0, q[2]]), 0.006 + 0.045 * back)), [0.0, -0.028 * back - 0.004, 0.0])
            }).collect();
            h.loft(mb, MatKey::Helmet, &[guard, r[0].clone()]);
            visor(mb, &r[0], 0.034, 0.018, MatKey::Helmet);
            // grzebień: płetwa wzdłuż osi hełmu od czoła po kark
            let apex = add(APEX, [0.0, o + 0.032 + 0.016 * 0.8, 0.0]);
            let spine = [r[1][0], r[2][0], r[3][0], apex, r[3][5], r[2][5], r[1][5]];
            let lift = |i: usize| 0.008 + 0.014 * (1.0 - ((i as f64 - 3.0) / 3.0).abs());
            for i in 0..spine.len() - 1 {
                let (a, b) = (spine[i], spine[i + 1]);
                let (ta, tb) = (add(a, scale(Head::radial(a), lift(i))), add(b, scale(Head::radial(b), lift(i + 1))));
                h.poly(mb, MatKey::Helmet, &[a, b, tb, ta], [1.0, 0.0, 0.0]);
            }
        }
        _ => {}
    }
}
