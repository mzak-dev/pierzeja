//! Szkielet `rr_human` skalowany parametrami + pozy statyczne przez FK (`Xform`) z prostym IK rąk.
//! Układ: Y w górę, przód +Z, lewa strona postaci = +X. Rotacja kości działa wokół jej `head`
//! w układzie rodzica, więc pozę liczy `world[i] = world[rodzic] ∘ obrót_wokół(head_i)`.
//! Kości palców (konwencja Unity Humanoid) są dopisane na końcu listy `szkielet.jointy`.

use crate::{extra, P};
use procedura_kit::mesh::{add, cross, dot, len, norm, scale, sub, Xform, V3};
use serde::Serialize;

const REF_H: f64 = 1.78;
const REF_SHOULDER: f64 = 0.294;
const REF_HIP: f64 = 0.215;
const REF_CROTCH: f64 = 0.516;
/// Wysokości granic odcinków `rr_human`: staw biodrowy (nogi | tułów), podstawa szyi, staw głowy, czubek.
const HIP_Y: f64 = 0.98;
const NECK_Y: f64 = 1.46;
const HEAD_Y: f64 = 1.56;
/// Staw głowy leży 0,928 wysokości głowy pod czubkiem (`rr_human`: 0,22 m przy głowie 0,237 m).
const PIVOT: f64 = 0.928;

#[derive(Debug, Clone, Serialize)]
pub struct Joint {
    pub name: String,
    pub parent: Option<usize>,
    pub head: V3,
    pub tail: V3,
}

pub type Skeleton = Vec<Joint>;

pub fn idx(sk: &[Joint], name: &str) -> usize {
    sk.iter().position(|j| j.name == name).unwrap_or_else(|| panic!("brak kości {name}"))
}

pub(crate) const FINGERS: [&str; 5] = ["Thumb", "Index", "Middle", "Ring", "Little"];
pub(crate) const PHALANGES: [&str; 3] = ["Proximal", "Intermediate", "Distal"];

fn is_arm(name: &str) -> bool {
    ["Arm", "Hand", "Shoulder"].iter().chain(FINGERS.iter()).any(|k| name.contains(k))
}

/// Głów we wzroście wg wieku (dziecko ma większą głowę) i wzrostu (niski dorosły: głowa względnie większa).
fn heads(age: f64, h: f64) -> f64 {
    const T: [(f64, f64, f64); 6] = [(3.0, 5.0, 0.96), (6.0, 5.7, 1.17), (9.0, 6.2, 1.34), (12.0, 6.7, 1.52), (15.0, 7.2, 1.68), (18.0, 7.5, 1.78)];
    let i = T.iter().position(|t| age < t.0).unwrap_or(T.len());
    let (n, rh) = match i {
        0 => (T[0].1, T[0].2),
        i if i == T.len() => (7.5, REF_H),
        i => {
            let (a, b) = (T[i - 1], T[i]);
            let u = (age - a.0) / (b.0 - a.0);
            (a.1 + (b.1 - a.1) * u, a.2 + (b.2 - a.2) * u)
        }
    };
    n * (h / rh).powf(0.4)
}

/// Skala sylwetki: wzrost, barki, biodra, długość nóg (krocze), wiek → odwzorowanie `rr_human`.
/// `shoulderWidth`, `hipWidth`, `crotchHeight` są ułamkami wzrostu, więc szerokości skalują się też wzrostem.
pub(crate) struct Scale {
    pub sv: f64,
    pub kw: f64,
    pub kh: f64,
    /// Wysokość głowy (podbródek → czubek).
    pub hh: f64,
    /// Dziecięcość 0 (≥ 14 lat) … 1 (≤ 4 lata).
    pub child: f64,
    kc: f64,
    ts: f64,
    yn: f64,
    tn: f64,
    yp: f64,
    th: f64,
    arm: f64,
}

impl Scale {
    pub fn new(p: &P) -> Scale {
        let h = p.f("height");
        let age = p.f("age");
        let sv = h / REF_H;
        let kc = p.f("crotchHeight") / REF_CROTCH;
        let hh = h / heads(age, h);
        let yp = h - PIVOT * hh;
        let child = ((14.0 - age) / 10.0).clamp(0.0, 1.0);
        let yn = yp - (HEAD_Y - NECK_Y) * sv * (1.0 - 0.3 * child);
        let y0 = HIP_Y * sv * kc;
        Scale {
            sv,
            kw: p.f("shoulderWidth") / REF_SHOULDER,
            kh: p.f("hipWidth") / REF_HIP,
            hh,
            child,
            kc,
            ts: (yn - y0) / (NECK_Y - HIP_Y),
            yn,
            tn: (yp - yn) / (HEAD_Y - NECK_Y),
            yp,
            th: (h - yp) / (REF_H - HEAD_Y),
            arm: 1.0 - 0.05 * child,
        }
    }

    fn map(&self, name: &str, q: V3) -> V3 {
        let [x, y, z] = q;
        let y2 = if y <= HIP_Y {
            y * self.sv * self.kc
        } else if y <= NECK_Y {
            HIP_Y * self.sv * self.kc + (y - HIP_Y) * self.ts
        } else if y <= HEAD_Y {
            self.yn + (y - NECK_Y) * self.tn
        } else {
            self.yp + (y - HEAD_Y) * self.th
        };
        let ax = x.abs();
        let x2 = if is_arm(name) {
            // barki skalują się szerokością, długość ręki wzrostem
            if ax <= 0.17 { ax * self.kw * self.sv } else { 0.17 * self.kw * self.sv + (ax - 0.17) * self.sv * self.arm }
        } else if name.contains("Leg") || name.contains("Foot") || name.contains("Toes") {
            ax * self.kh * self.sv
        } else {
            ax * self.sv
        };
        [x2 * x.signum(), y2, z * self.sv]
    }
}

/// Szkielet w pozie spoczynkowej (T-pose `rr_human`) przeskalowany parametrami.
pub(crate) fn rest(sc: &Scale) -> Skeleton {
    let arr = extra()["szkielet"]["jointy"].as_array().expect("szkielet.jointy");
    let v3 = |v: &serde_json::Value| -> V3 { std::array::from_fn(|i| v[i].as_f64().unwrap()) };
    let names: Vec<&str> = arr.iter().map(|j| j["name"].as_str().unwrap()).collect();
    arr.iter()
        .map(|j| {
            let name = j["name"].as_str().unwrap();
            let parent = j["parent"].as_str().map(|p| names.iter().position(|n| *n == p).expect("rodzic"));
            assert!(parent.is_none_or(|p| p < names.iter().position(|n| *n == name).unwrap()), "rodzic przed dzieckiem");
            Joint { name: name.into(), parent, head: sc.map(name, v3(&j["head"])), tail: sc.map(name, v3(&j["tail"])) }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Pose {
    /// T-pose `rr_human` (baza pod skin, issue 16).
    Rest,
    Stand,
    Walk,
    /// Krzesło / ławka: udo poziomo, goleń pionowo.
    Sit,
    /// Kierowca: oparcie odchylone, stopy na pedałach, dłonie na kierownicy.
    Drive,
    /// Pasażer w pojeździe: jak `drive`, dłonie na udach.
    Ride,
}

impl Pose {
    pub fn parse(s: &str) -> Option<Pose> {
        Some(match s {
            "rest" => Pose::Rest,
            "stand" => Pose::Stand,
            "walk" => Pose::Walk,
            "sit" => Pose::Sit,
            "drive" => Pose::Drive,
            "ride" => Pose::Ride,
            _ => return None,
        })
    }
}

// ------------------------------------------------------------------ obroty 3×3

type M3 = [[f64; 3]; 3];
const I3: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn mx(m: M3) -> Xform {
    Xform { m, t: [0.0; 3] }
}
fn tr(a: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| a[j][i]))
}
fn mm(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}
fn mv(a: &M3, v: V3) -> V3 {
    std::array::from_fn(|i| a[i][0] * v[0] + a[i][1] * v[1] + a[i][2] * v[2])
}

/// Obrót o `deg` wokół osi `axis` (Rodrigues).
pub(crate) fn axis_angle(axis: V3, deg: f64) -> Xform {
    let k = norm(axis);
    let (s, c) = deg.to_radians().sin_cos();
    let t = 1.0 - c;
    let [x, y, z] = k;
    mx([[c + x * x * t, x * y * t - z * s, x * z * t + y * s], [y * x * t + z * s, c + y * y * t, y * z * t - x * s], [z * x * t - y * s, z * y * t + x * s, c + z * z * t]])
}

/// Najkrótszy obrót przeprowadzający kierunek `a` na `b`.
fn from_to(a: V3, b: V3) -> M3 {
    let (a, b) = (norm(a), norm(b));
    let ax = cross(a, b);
    let s = len(ax);
    let c = dot(a, b).clamp(-1.0, 1.0);
    if s < 1e-9 {
        if c > 0.0 {
            return I3;
        }
        let p = if a[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        return axis_angle(cross(a, p), 180.0).m;
    }
    axis_angle(ax, s.atan2(c).to_degrees()).m
}

/// Baza ortonormalna (kolumny: f, n ⟂ f, f × n).
fn basis(f: V3, n: V3) -> M3 {
    let f = norm(f);
    let n = norm(sub(n, scale(f, dot(n, f))));
    let b = cross(f, n);
    [[f[0], n[0], b[0]], [f[1], n[1], b[1]], [f[2], n[2], b[2]]]
}

fn eul(x: f64, y: f64, z: f64) -> Xform {
    let d = f64::to_radians;
    Xform::rot_z(d(z)).mul(&Xform::rot_y(d(y))).mul(&Xform::rot_x(d(x)))
}

/// Ręka wisząca: `hang` = kąt od T-pose w dół (80° ≈ pionowo), `swing` do przodu, `elbow` zgięcie łokcia.
fn arm(sx: f64, hang: f64, swing: f64, elbow: f64) -> (Xform, Xform) {
    (eul(-swing, 0.0, 0.0).mul(&eul(0.0, 0.0, -sx * hang)), eul(0.0, -sx * elbow, 0.0))
}

/// Kości w pozie: `world[i]` przekształca punkt z pozycji spoczynkowej do pozy.
pub(crate) fn fk(sk: &[Joint], rot: &[Xform]) -> Vec<Xform> {
    let mut w: Vec<Xform> = Vec::with_capacity(sk.len());
    for (i, b) in sk.iter().enumerate() {
        let local = Xform::translate(b.head).mul(&rot[i]).mul(&Xform::translate(b.head.map(|c| -c)));
        w.push(match b.parent {
            Some(p) => w[p].mul(&local),
            None => local,
        });
    }
    w
}

/// Pozowanie: lokalne obroty + bieżące FK (IK ustawia obroty w świecie przez rodzica).
struct Poser<'a> {
    sk: &'a [Joint],
    r: Vec<Xform>,
    w: Vec<Xform>,
}

impl Poser<'_> {
    fn i(&self, n: &str) -> usize {
        idx(self.sk, n)
    }
    fn set(&mut self, n: &str, x: Xform) {
        let i = self.i(n);
        self.r[i] = x;
    }
    fn pre(&mut self, n: &str, x: Xform) {
        let i = self.i(n);
        self.r[i] = x.mul(&self.r[i]);
    }
    fn update(&mut self) {
        self.w = fk(self.sk, &self.r);
    }
    fn parent_rot(&self, i: usize) -> M3 {
        self.sk[i].parent.map_or(I3, |p| self.w[p].m)
    }
    /// Obrót kości w świecie ustawiony wprost.
    fn set_world(&mut self, n: &str, world: M3) {
        let i = self.i(n);
        self.r[i] = mx(mm(&tr(&self.parent_rot(i)), &world));
        self.update();
    }
    /// Kość (od `head` do punktu `to` w spoczynku) celuje w kierunek `dir` w świecie — najmniejszym obrotem.
    fn aim(&mut self, n: &str, to: V3, dir: V3) {
        let i = self.i(n);
        let cur = self.w[i].m;
        let v = mv(&cur, sub(to, self.sk[i].head));
        self.set_world(n, mm(&from_to(v, dir), &cur));
    }
    fn head(&self, n: &str) -> V3 {
        let i = self.i(n);
        self.w[i].point(self.sk[i].head)
    }
    /// IK dwóch kości: punkt chwytu dłoni (`HandGrip`) w `grip`, dłoń zorientowana (`f` = ku palcom, `n` = wnętrze),
    /// łokieć w stronę `pole`.
    fn reach(&mut self, side: &str, sx: f64, grip: V3, f: V3, n: V3, pole: V3) {
        let (ua, la, hd, gp) = (format!("{side}UpperArm"), format!("{side}LowerArm"), format!("{side}Hand"), format!("{side}HandGrip"));
        let sk = self.sk;
        let (iu, il, ih, ig) = (self.i(&ua), self.i(&la), self.i(&hd), self.i(&gp));
        let hand = mm(&basis(f, n), &tr(&basis([sx, 0.0, 0.0], [0.0, -1.0, 0.0])));
        let wrist = sub(grip, mv(&hand, sub(sk[ig].head, sk[ih].head)));
        let s = self.head(&ua);
        let (a, b) = (len(sub(sk[il].head, sk[iu].head)), len(sub(sk[ih].head, sk[il].head)));
        let d = len(sub(wrist, s)).clamp((a - b).abs() + 1e-3, a + b - 1e-3);
        let u = norm(sub(wrist, s));
        let v = norm(sub(pole, scale(u, dot(pole, u))));
        let ca = ((a * a + d * d - b * b) / (2.0 * a * d)).clamp(-1.0, 1.0);
        let elbow = add(s, add(scale(u, a * ca), scale(v, a * (1.0 - ca * ca).sqrt())));
        let wr = add(s, scale(u, d));
        self.aim(&ua, sk[il].head, sub(elbow, s));
        self.aim(&la, sk[ih].head, sub(wr, elbow));
        self.set_world(&hd, hand);
    }
    /// Zgięcie palców: `mcp`/`pip`/`dip` w stopniach (kciuk osobno), oś = kierunek palca × wnętrze dłoni.
    fn curl(&mut self, side: &str, mcp: f64, pip: f64, dip: f64, thumb: f64) {
        let palm = [0.0, -1.0, 0.0];
        for f in FINGERS {
            for (k, ph) in PHALANGES.iter().enumerate() {
                let n = format!("{side}{f}{ph}");
                let i = self.i(&n);
                let dir = sub(self.sk[i].tail, self.sk[i].head);
                let (axis, deg) = if f == "Thumb" {
                    // kciuk zgina się ku małemu palcowi i wnętrzu dłoni
                    let inward = norm(add(palm, [0.0, 0.0, -0.8]));
                    (cross(dir, inward), thumb * [0.35, 0.8, 0.9][k])
                } else {
                    let spread = match f {
                        "Little" => 1.15,
                        "Ring" => 1.05,
                        _ => 1.0,
                    };
                    (cross(dir, palm), [mcp, pip, dip][k] * spread)
                };
                self.r[i] = axis_angle(axis, deg);
            }
        }
    }
}

/// Pozy w pojeździe: odchylenie oparcia i kąty nóg (stopnie).
const RECLINE: f64 = 27.0;
const THIGH_UP: f64 = 11.0;
const KNEE: f64 = 130.0;
const FOOT_UP: f64 = 30.0;

/// Transformacje kości w świecie dla pozy (`phase` ∈ [0,1) tylko dla marszu) i postawy.
pub(crate) fn pose(sk: &[Joint], sc: &Scale, pose: Pose, phase: f64, posture: &str) -> Vec<Xform> {
    let n = sk.len();
    let mut p = Poser { sk, r: vec![Xform::identity(); n], w: vec![] };
    if pose == Pose::Rest {
        p.update();
        return p.w;
    }
    let seated = matches!(pose, Pose::Drive | Pose::Ride);
    // postawa: tułów w przód, kolana lekko ugięte; szyja i głowa odginają się, żeby wzrok był poziomo
    let (sp, ch, uc, kn) = match posture {
        "pochylona" => (10.0, 9.0, 4.0, 7.0),
        "kifoza" => (14.0, 13.0, 9.0, 10.0),
        _ => (0.0, 0.0, 0.0, 0.0),
    };
    let stoop = if matches!(pose, Pose::Stand | Pose::Walk) { kn } else { 0.0 };
    for (side, sx) in [("Left", 1.0), ("Right", -1.0)] {
        let th = phase * std::f64::consts::TAU + if sx > 0.0 { 0.0 } else { std::f64::consts::PI };
        // (udo do przodu, kolano, ramię do przodu, łokieć, ramię w dół, pięta w górę)
        let (flex, knee, swing, elbow, hang, heel) = match pose {
            Pose::Stand => (0.0, 0.0, 0.0, 8.0, 80.0, 0.0),
            Pose::Walk => (20.0 * th.sin(), 6.0 + 42.0 * th.cos().max(0.0), -24.0 * th.sin() * (1.0 - stoop / 20.0), 14.0 + 18.0 * (-th.sin()).max(0.0), 80.0, 14.0 * (-th.sin()).max(0.0)),
            Pose::Sit => (89.0, 91.0, 6.0, 60.0, 76.0, 0.0),
            _ => (90.0, 90.0, 20.0, 40.0, 70.0, 0.0),
        };
        let (flex, knee) = (flex + stoop * 0.6, knee + stoop);
        let (ua, la) = arm(sx, hang, swing, elbow);
        p.set(&format!("{side}UpperArm"), ua);
        p.set(&format!("{side}LowerArm"), la);
        p.set(&format!("{side}UpperLeg"), eul(-flex, 0.0, 0.0));
        p.set(&format!("{side}LowerLeg"), eul(knee, 0.0, 0.0));
        // stopa płasko względem gruntu (kompensacja uda i kolana), w marszu tylna pięta odrywa się
        p.set(&format!("{side}Foot"), eul(flex - knee + heel, 0.0, 0.0));
        let (mcp, pip, dip, thumb) = if pose == Pose::Drive { (62.0, 80.0, 40.0, 38.0) } else { (14.0, 22.0, 12.0, 14.0) };
        p.curl(side, mcp, pip, dip, thumb);
    }
    match pose {
        Pose::Walk => {
            let s = (phase * std::f64::consts::TAU).sin();
            p.set("Hips", eul(0.0, -6.0 * s, 0.0));
            p.set("Spine", eul(3.0, 4.0 * s, 0.0));
            p.set("Chest", eul(0.0, 4.0 * s, 0.0));
        }
        Pose::Drive | Pose::Ride => {
            // oparcie odchylone: miednica + lędźwie + klatka; barki lekko wysunięte do kierownicy
            p.set("Hips", eul(-RECLINE * 0.5, 0.0, 0.0));
            p.set("Spine", eul(-RECLINE * 0.3, 0.0, 0.0));
            p.set("Chest", eul(-RECLINE * 0.2, 0.0, 0.0));
            if pose == Pose::Drive {
                p.set("UpperChest", eul(6.0, 0.0, 0.0));
                p.set("LeftShoulder", eul(0.0, -18.0, 0.0));
                p.set("RightShoulder", eul(0.0, 18.0, 0.0));
            }
        }
        _ => {}
    }
    for (nm, a) in [("Spine", sp), ("Chest", ch), ("UpperChest", uc)] {
        p.pre(nm, eul(a, 0.0, 0.0));
    }
    let neck = -(sp + ch + uc) * 0.7;
    p.pre("Neck", eul(neck * 0.5, 0.0, 0.0));
    p.pre("Head", eul(neck * 0.5, 0.0, 0.0));
    p.update();

    if seated {
        // głowa poziomo, szyja lekko do przodu
        let neck_tail = sk[p.i("Neck")].tail;
        p.aim("Neck", neck_tail, [0.0, 1.0, 0.18]);
        p.set_world("Head", eul(4.0, 0.0, 0.0).m);
        for (side, sx) in [("Left", 1.0), ("Right", -1.0)] {
            let (ul, ll, ft) = (format!("{side}UpperLeg"), format!("{side}LowerLeg"), format!("{side}Foot"));
            let (knee, ankle) = (sk[p.i(&ll)].head, sk[p.i(&ft)].head);
            let up = THIGH_UP.to_radians();
            p.aim(&ul, knee, [sx * 0.06, up.sin(), up.cos()]);
            let down = (KNEE - 180.0 + THIGH_UP).to_radians();
            p.aim(&ll, ankle, [sx * 0.04, down.sin(), down.cos()]);
            let foot = if sx > 0.0 { FOOT_UP - 4.0 } else { FOOT_UP };
            p.set_world(&ft, mm(&eul(0.0, -sx * 8.0, 0.0).m, &eul(-foot, 0.0, 0.0).m));
        }
    }
    let hip = scale(add(p.head("LeftUpperLeg"), p.head("RightUpperLeg")), 0.5);
    for (side, sx) in [("Left", 1.0), ("Right", -1.0)] {
        match pose {
            Pose::Drive => {
                // kierownica: chwyt „za kwadrans trzecia”, dłonie ok. 0,40 m nad i 0,45 m przed punktem H
                let grip = add(hip, scale([sx * 0.185, 0.40, 0.45], sc.sv));
                p.reach(side, sx, grip, [-sx * 0.15, 0.55, 1.0], [-sx, 0.15, 0.35], [sx * 0.8, -1.0, -0.2]);
            }
            Pose::Sit | Pose::Ride => {
                // dłonie płasko na udach, w ok. 60% długości uda
                let (ul, ll) = (format!("{side}UpperLeg"), format!("{side}LowerLeg"));
                let (a, b) = (p.head(&ul), p.head(&ll));
                let along = norm(sub(b, a));
                let top = add(add(a, scale(sub(b, a), 0.62)), scale([0.0, 1.0, 0.0], 0.095 * sc.sv));
                let grip = add(top, [-sx * 0.012 * sc.sv, 0.0, 0.0]);
                p.reach(side, sx, grip, add(along, [-sx * 0.25, 0.0, 0.0]), [0.0, -1.0, 0.0], [sx * 0.6, -0.2, -1.0]);
            }
            _ => {}
        }
    }
    p.w
}

/// Punkty podeszwy w spoczynku (pięta, przodostopie, czubek) z kością, do której należą.
pub(crate) fn sole_points(sk: &[Joint], sc: &Scale, side: &str) -> [(usize, V3); 3] {
    let (fi, ti) = (idx(sk, &format!("{side}Foot")), idx(sk, &format!("{side}Toes")));
    let (f, t) = (sk[fi].head, sk[ti].head);
    [(fi, [f[0], 0.0, f[2] - 0.062 * sc.sv]), (ti, [t[0], 0.0, t[2]]), (ti, [t[0], 0.0, t[2] + 0.075 * sc.sv])]
}

/// Przesunięcie pionowe tak, by najniższy punkt podeszwy stał na gruncie.
pub(crate) fn ground_shift(sk: &[Joint], sc: &Scale, w: &[Xform]) -> f64 {
    let lowest = ["Left", "Right"].iter().flat_map(|s| sole_points(sk, sc, s)).map(|(i, p)| w[i].point(p)[1]).fold(f64::INFINITY, f64::min);
    -lowest
}

pub(crate) fn posed(sk: &[Joint], w: &[Xform], dy: f64) -> Vec<Joint> {
    let up = |p: V3| -> V3 { [p[0], p[1] + dy, p[2]] };
    sk.iter()
        .enumerate()
        .map(|(i, j)| {
            // pomocnicze IK podążają za dłonią/stopą
            let k = j.name.strip_suffix("IK").map_or(i, |n| idx(sk, n));
            Joint { name: j.name.clone(), parent: j.parent, head: up(w[k].point(j.head)), tail: up(w[k].point(j.tail)) }
        })
        .collect()
}
