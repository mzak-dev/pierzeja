//! Szkielet `rr_human` skalowany parametrami + pozy statyczne przez FK (`Xform`).
//! Układ: Y w górę, przód +Z, lewa strona postaci = +X. Rotacja kości działa wokół jej `head`
//! w układzie rodzica, więc pozę liczy `world[i] = world[rodzic] ∘ obrót_wokół(head_i)`.

use crate::{extra, P};
use procedura_kit::mesh::{Xform, V3};
use serde::Serialize;

const REF_H: f64 = 1.78;
const REF_SHOULDER: f64 = 0.294;
const REF_HIP: f64 = 0.215;
const REF_CROTCH: f64 = 0.516;
/// Wysokość stawu biodrowego w `rr_human`: granica nóg i tułowia przy skalowaniu pionowym.
const HIP_Y: f64 = 0.98;

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

/// Skala sylwetki: wzrost, barki, biodra, długość nóg (kość krocza) → współczynniki względem `rr_human`.
pub(crate) struct Scale {
    pub sv: f64,
    pub kw: f64,
    pub kh: f64,
    kc: f64,
    ts: f64,
}

impl Scale {
    pub fn new(p: &P) -> Scale {
        let sv = p.f("height") / REF_H;
        let kc = p.f("crotchHeight") / REF_CROTCH;
        // tułów dopełnia wzrost, więc czubek głowy zawsze = `height`
        let ts = (REF_H - HIP_Y * kc) / (REF_H - HIP_Y);
        Scale { sv, kw: p.f("shoulderWidth") / REF_SHOULDER, kh: p.f("hipWidth") / REF_HIP, kc, ts }
    }

    fn map(&self, name: &str, q: V3) -> V3 {
        let [x, y, z] = q;
        let y2 = if y <= HIP_Y { y * self.sv * self.kc } else { HIP_Y * self.sv * self.kc + (y - HIP_Y) * self.sv * self.ts };
        let ax = x.abs();
        let x2 = if name.contains("Arm") || name.contains("Hand") || name.contains("Shoulder") {
            // barki skalują się szerokością, długość ręki wzrostem
            if ax <= 0.17 { ax * self.kw } else { 0.17 * self.kw + (ax - 0.17) * self.sv }
        } else if name.contains("Leg") || name.contains("Foot") || name.contains("Toes") {
            ax * self.kh
        } else {
            ax
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
    Sit,
    Drive,
}

impl Pose {
    pub fn parse(s: &str) -> Option<Pose> {
        Some(match s {
            "rest" => Pose::Rest,
            "stand" => Pose::Stand,
            "walk" => Pose::Walk,
            "sit" => Pose::Sit,
            "drive" => Pose::Drive,
            _ => return None,
        })
    }
}

fn eul(x: f64, y: f64, z: f64) -> Xform {
    let d = f64::to_radians;
    Xform::rot_z(d(z)).mul(&Xform::rot_y(d(y))).mul(&Xform::rot_x(d(x)))
}

/// Ręka wisząca: `hang` = kąt od T-pose w dół (80° ≈ pionowo), `swing` do przodu, `elbow` zgięcie łokcia.
fn arm(sx: f64, hang: f64, swing: f64, elbow: f64) -> (Xform, Xform) {
    (eul(-swing, 0.0, 0.0).mul(&eul(0.0, 0.0, -sx * hang)), eul(0.0, -sx * elbow, 0.0))
}

/// Obroty lokalne kości dla pozy (`phase` ∈ [0,1) tylko dla marszu) + postawa.
pub(crate) fn rotations(sk: &[Joint], pose: Pose, phase: f64, posture: &str) -> Vec<Xform> {
    let mut r = vec![Xform::identity(); sk.len()];
    let mut set = |n: &str, x: Xform| r[idx(sk, n)] = x;
    if pose != Pose::Rest {
        for (side, sx) in [("Left", 1.0), ("Right", -1.0)] {
            let th = phase * std::f64::consts::TAU + if sx > 0.0 { 0.0 } else { std::f64::consts::PI };
            // (udo do przodu, kolano, ramię do przodu, łokieć, ramię w dół)
            let (flex, knee, swing, elbow, hang) = match pose {
                Pose::Stand => (0.0, 0.0, 0.0, 8.0, 80.0),
                Pose::Walk => (20.0 * th.sin(), 6.0 + 42.0 * th.cos().max(0.0), -24.0 * th.sin(), 14.0 + 18.0 * (-th.sin()).max(0.0), 80.0),
                Pose::Sit => (89.0, 91.0, 6.0, 75.0, 76.0),
                Pose::Drive => (80.0, 82.0 - if sx < 0.0 { 6.0 } else { 0.0 }, 38.0, 48.0, 72.0),
                Pose::Rest => unreachable!(),
            };
            let (ua, la) = arm(sx, hang, swing, elbow);
            set(&format!("{side}UpperArm"), ua);
            set(&format!("{side}LowerArm"), la);
            set(&format!("{side}UpperLeg"), eul(-flex, 0.0, 0.0));
            set(&format!("{side}LowerLeg"), eul(knee, 0.0, 0.0));
            // stopa płasko względem gruntu (kompensacja uda i kolana)
            set(&format!("{side}Foot"), eul(flex - knee, 0.0, 0.0));
        }
        match pose {
            Pose::Walk => {
                let s = (phase * std::f64::consts::TAU).sin();
                set("Hips", eul(0.0, -6.0 * s, 0.0));
                set("Spine", eul(0.0, 4.0 * s, 0.0));
                set("Chest", eul(0.0, 4.0 * s, 0.0));
            }
            Pose::Drive => {
                set("Spine", eul(-6.0, 0.0, 0.0));
                set("Chest", eul(-3.0, 0.0, 0.0));
            }
            _ => {}
        }
    }
    // postawa: tułów w przód, szyja i głowa odginają się, żeby wzrok był poziomo
    let (sp, ch, uc) = match posture {
        "pochylona" => (7.0, 6.0, 3.0),
        "kifoza" => (12.0, 12.0, 8.0),
        _ => (0.0, 0.0, 0.0),
    };
    let neck = -(sp + ch + uc) * 0.7;
    for (n, a) in [("Spine", sp), ("Chest", ch), ("UpperChest", uc), ("Neck", neck * 0.5), ("Head", neck * 0.5)] {
        let i = idx(sk, n);
        r[i] = eul(a, 0.0, 0.0).mul(&r[i]);
    }
    r
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

/// Przesunięcie pionowe tak, by najniżej opuszczona względem spoczynku stopa stała na gruncie.
pub(crate) fn ground_shift(sk: &[Joint], w: &[Xform]) -> f64 {
    let mut worst = f64::INFINITY;
    for s in ["Left", "Right"] {
        let foot = idx(sk, &format!("{s}Foot"));
        let toes = idx(sk, &format!("{s}Toes"));
        for (i, p) in [(foot, sk[foot].head), (toes, sk[toes].head), (toes, sk[toes].tail)] {
            worst = worst.min(w[i].point(p)[1] - p[1]);
        }
    }
    -worst
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
