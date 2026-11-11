//! Nadwozie osobowe jako powłoka parametryczna: profil boczny z krzywych Hermite'a, przekrój
//! z rzędów na liniach cech (próg, bark, linia okien, krawędź szyby, rynna, oś dachu), nadkola
//! jako okrąg próbkowany gęstymi stacjami, końce zaokrąglone w planie i zamknięte czołem
//! (profil czoła wg wysokości). Lewa połowa (+X) jest siatką stacje × rzędy, prawa to odbicie;
//! normalne gładkie z progiem załamania (`Powloka`). Gęstość próbkowania = parametr (LOD).

use crate::sluzba::Half;
use crate::P;
use procedura_kit::krzywe::{g, lerp, o, smoothstep, Krzywa};
use procedura_kit::mesh::{MatKey, V3};
use procedura_kit::powloka::Powloka;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rodzaj {
    Hatch,
    Liftback,
    Sedan,
    Kombi,
    Suv,
    Coupe,
    /// Furgon / bus z krótką maską (T5, Sprinter) albo półmaską (Lublin).
    Van,
}

impl Rodzaj {
    pub fn of(body_type: &str) -> Option<Rodzaj> {
        Some(match body_type {
            "hatchback" => Rodzaj::Hatch,
            "liftback" => Rodzaj::Liftback,
            "sedan" => Rodzaj::Sedan,
            "kombi" => Rodzaj::Kombi,
            "suv" => Rodzaj::Suv,
            "coupe" => Rodzaj::Coupe,
            "van" => Rodzaj::Van,
            _ => return None,
        })
    }
}

/// Gęstość próbkowania powłoki: odstęp stacji w środku nadwozia i liczba podziałów łuków.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Gestosc {
    /// Maksymalny odstęp stacji wzdłuż nadwozia (m).
    pub ds: f64,
    /// Kąt między stacjami na łuku nadkola (stopnie).
    pub arch_deg: f64,
    /// Stacje na łuku naroża w planie i w poprzek czoła.
    pub end: usize,
    /// Dodatkowe rzędy w długich odcinkach przekroju (bok, dach).
    pub sub: usize,
}

impl Gestosc {
    pub const LOD0: Gestosc = Gestosc { ds: 0.07, arch_deg: 7.5, end: 9, sub: 2 };
}

/// Kluczowe rzędy przekroju (lewa połowa, od osi spodu do osi dachu).
pub(crate) const R_FLOOR: usize = 1; // krawędź podłogi = dolna krawędź wewnętrznej ściany wnęki koła
pub(crate) const R_WELL: usize = 2; // górna krawędź ściany wnęki (strop wnęki do rantu)
pub(crate) const R_LIP: usize = 3; // rant nadkola / dolna krawędź progu
pub(crate) const R_LOW: usize = 4; // dolna część boku (przetłoczenie drzwi)
pub(crate) const R_SH: usize = 5; // bark — najszersze miejsce
pub(crate) const R_BELT: usize = 6; // linia okien (na masce: krawędź błotnika)
pub(crate) const R_GT: usize = 7; // górna krawędź szyby bocznej / słupek A / szczelina maski
pub(crate) const R_RAIL: usize = 8; // rynna dachu / krawędź szyby czołowej
pub(crate) const R_MID: usize = 9; // pół dachu
pub(crate) const R_TOP: usize = 10; // oś dachu
pub(crate) const N_KEY: usize = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Strefa {
    Maska,
    Szyba,
    Dach,
    TylnaSzyba,
    Pokrywa,
}

/// Jawny układ osi i końców powłoki (kabina ciężarówki).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Rama {
    pub zaf: f64,
    /// Tylna oś; daleko za powłoką, gdy kabina nie ma tylnego nadkola.
    pub zar: f64,
    pub zf: f64,
    pub zr: f64,
}

/// Wymiary i linie nadwozia wyliczone z parametrów (wszystko w metrach, układ pojazdu).
pub(crate) struct Ksztalt {
    pub kind: Rodzaj,
    pub hw: f64,
    pub h: f64,
    pub clr: f64,
    pub r: f64,
    pub ra: f64,
    pub track: f64,
    pub zaf: f64,
    pub zar: f64,
    pub zf: f64,
    pub zr: f64,
    /// Krawędź maski (z, y) i podstawa szyby czołowej w osi (z, y).
    pub z_he: f64,
    pub y_he: f64,
    pub z_c: f64,
    /// Górna krawędź szyby czołowej, szczyt dachu, koniec dachu, dolna krawędź szyby tylnej.
    pub z_wt: f64,
    pub z_rr: f64,
    pub z_gb: f64,
    /// Koniec górnej powierzchni z tyłu (dalej profil tyłu w dół do zderzaka).
    pub y_tail: f64,
    pub z_te: f64,
    /// Nachylenie tylnej szyby (tangens kąta od poziomu).
    pub rear_tan: f64,
    /// Górna krawędź tylnego zderzaka (m).
    pub rear_bumper_top: f64,
    top: Krzywa,
    pub belt_f: f64,
    pub belt_r: f64,
    pub shoulder_drop: f64,
    pub tumble: f64,
    pub crown: f64,
    pub sill_tuck: f64,
    /// Okna boczne: od słupka A (wyznacza go wysokość szyby) do `z_win_r`; słupki B i C.
    pub z_win_r: f64,
    pub z_b: f64,
    pub z_c_pillar: Option<f64>,
    /// Słupki między oknami ładowni vana.
    pub extra_pillars: Vec<f64>,
    /// Przednia krawędź dachu (nad szybą; w wysokim vanie przed daszkiem).
    pub z_rf: f64,
    /// Górna krawędź okien bocznych przy daszku (wysokie vany, kabiny ciężarówek); bez daszka ∞.
    pub glass_top: f64,
    /// Kabina pojazdu ciężkiego (bez tylnych drzwi, zderzaków i tablic — te ma rama).
    pub cab_only: bool,
    pub x_inner: f64,
    nose: Krzywa,
    tail: Krzywa,
    pub r_front: f64,
    pub r_rear: f64,
}

fn fo(p: &P, k: &str, d: f64) -> f64 {
    p.0.get(k).and_then(|v| v.as_f64()).unwrap_or(d)
}

impl Ksztalt {
    /// Jak `new`, ale z jawnym położeniem osi i końców (kabina pojazdu ciężkiego: przednie nadkole
    /// w kabinie, tył kabiny ucięty przed ramą, bez tylnego nadkola).
    pub fn with_frame(p: &P, rama: Option<Rama>) -> Ksztalt {
        let kind = Rodzaj::of(p.s("bodyType")).unwrap_or(Rodzaj::Hatch);
        let (l, w, h, wb) = (p.f("length"), p.f("width"), p.f("height"), p.f("wheelbase"));
        let hw = w / 2.0;
        let clr = p.f("clearance");
        let (r, tw) = (p.f("wheelDiameter") / 2.0, p.f("tyreWidth"));
        let ra = r + p.f("wheelArchGap");
        // lico opony ok. 15 mm w głębi boku przy nadkolu
        let track = (2.0 * (hw - 0.015 - tw / 2.0)).min(p.f("trackRatio") * w + 0.06);
        let x_inner = track / 2.0 - tw / 2.0 - 0.035;
        let (fo_, ro_) = (p.f("frontOverhang"), p.f("rearOverhang"));
        let oh = (l - wb).max(0.8);
        let (zaf, zar, zf, zr) = match rama {
            Some(r) => (r.zaf, r.zar, r.zf, r.zr),
            None => {
                let (zaf, zar) = (wb / 2.0, -wb / 2.0);
                (zaf, zar, zaf + oh * fo_ / (fo_ + ro_), zar - oh * ro_ / (fo_ + ro_))
            }
        };
        let l = if rama.is_some() { zf - zr } else { l };

        // profil: krawędź maski, podstawa szyby, szyba, dach, tył
        let y_he = fo(p, "noseHeight", 0.52) * h;
        let z_he = zf - fo(p, "noseRake", 0.09);
        let z_c = zf - fo(p, "hoodLength", 0.31) * l;
        let belt_f = p.f("beltline") * h;
        // podstawa szyby w osi nad linią okien (krawędzie szyby są niżej przez wygięcie)
        let y_c = (p.f("hoodHeight") * h).max(belt_f + 0.04);
        let crown_f = 0.03;
        let ws = p.f("windshieldAngle").to_radians().tan();
        // górna krawędź szyby: do dachu (osobówki) albo niżej z daszkiem w kolorze nadwozia (wysokie vany)
        let y_wt = (fo(p, "windshieldTop", 1.0) * h).min(h - crown_f);
        let z_wt = z_c - (y_wt - y_c) / ws;
        let cap = y_wt < h - 0.06;
        let z_rf = if cap { z_wt - (h - 0.012 - y_wt) / 70f64.to_radians().tan() } else { z_wt };
        let belt_r = belt_f + fo(p, "beltRise", 0.05);
        let y_tail_top = fo(p, "tailHeight", 0.70) * h;
        let rg = p.f("rearGlassAngle").to_radians().tan();
        // tył: (z_gb, y_gb) dolna krawędź szyby tylnej, (z_te, y_te) koniec górnej powierzchni
        // (dalej w dół idzie już profil tyłu: klapa / pas tylny do zderzaka)
        let roof_drop = fo(p, "roofDrop", 0.04);
        let y_rr = h - roof_drop;
        let (z_gb, y_gb, z_te, y_te) = match kind {
            Rodzaj::Sedan | Rodzaj::Liftback | Rodzaj::Coupe => {
                // pokrywa bagażnika (sedan długa, liftback/coupé krótki kuferek ze spojlerem)
                let deck = fo(p, "deckLength", if kind == Rodzaj::Sedan { 0.2 } else { 0.06 }) * l;
                let z_te = zr + fo(p, "tailLean", 0.06);
                (z_te + deck, y_tail_top, z_te, y_tail_top - 0.015)
            }
            _ => {
                // klapa: szyba do `tailHeight`, niżej pochylona klapa do zderzaka
                let z_gb = zr + fo(p, "tailLean", 0.12);
                (z_gb, y_tail_top, z_gb, y_tail_top)
            }
        };
        let z_rr = (z_gb + (y_rr - y_gb) / rg).min(z_rf - 0.6);
        let y_tail = y_te;
        let z_peak = z_rf - fo(p, "roofPeak", 0.4) * (z_rf - z_rr);
        let hood_mid = lerp(y_he, y_c, 0.82);
        let mut knots = vec![o(z_te, y_te)];
        if z_gb > z_te + 0.02 {
            knots.push(o(z_gb, y_gb));
        }
        knots.extend([g(z_rr, y_rr), g(z_peak, h)]);
        if cap {
            knots.extend([g(z_rf, h - 0.012), o(z_wt, y_wt)]);
        } else {
            knots.push(g(z_wt, y_wt));
        }
        knots.extend([o(z_c, y_c), g(lerp(z_c, z_he, 0.55), hood_mid), o(z_he, y_he), o(zf, y_he - 0.02)]);
        let top = Krzywa::new(&knots);
        // czoło: z najbardziej wysuniętego punktu w zależności od wysokości (zderzak wypukły, maska cofnięta)
        let yb_max = (clr + 0.28).min(y_he - 0.1);
        let nose = Krzywa::new(&[g(clr, zf - 0.07), g(clr + 0.17, zf - 0.01), g(yb_max, zf), g(lerp(yb_max, y_he, 0.6), zf - 0.03), g(y_he, z_he)]);
        let bumper_top = fo(p, "rearBumperTop", 0.40) * h;
        let tail = Krzywa::new(&[g(clr, zr + 0.10), g(clr + 0.2, zr + 0.01), g(bumper_top, zr), g(y_te, z_te)]);

        // okna: tył kabiny (słupek C/D) i słupek B między drzwiami
        // bez szyby za drzwiami (Golf) okna kończą się nad przodem tylnego koła, za nimi szeroki słupek C
        let quarter = fo(p, "quarterWindow", 1.0) > 0.5;
        let cargo_windows = fo(p, "cargoWindows", 0.0) > 0.5;
        // van: drzwi kabiny ok. 0,95 m za podstawą szyby (słupek B), dalej ładownia
        let van_door = (0.33 * wb).clamp(0.85, 1.15);
        let z_win_r = match kind {
            Rodzaj::Sedan | Rodzaj::Liftback | Rodzaj::Coupe => lerp(z_rr, z_gb, 0.25),
            Rodzaj::Van if cargo_windows => z_rr + 0.25,
            Rodzaj::Van => z_c - van_door + 0.05,
            _ if !quarter => zar + 0.3 * ra,
            _ => z_rr + 0.06,
        };
        let z_b = match kind {
            Rodzaj::Van => z_c - van_door,
            Rodzaj::Coupe => lerp(z_c, z_win_r, 0.62),
            _ => lerp(z_c, z_win_r, if quarter { 0.42 } else { 0.5 }),
        };
        let z_c_pillar = (quarter && matches!(kind, Rodzaj::Hatch | Rodzaj::Kombi | Rodzaj::Suv)).then(|| lerp(z_b, z_win_r, if kind == Rodzaj::Kombi { 0.55 } else { 0.72 }));
        // van z oknami w ładowni: słupki co ok. 1 m
        let mut extra_pillars = Vec::new();
        if kind == Rodzaj::Van && cargo_windows {
            let n = ((z_b - z_win_r) / 1.0).round().max(1.0) as usize;
            extra_pillars.extend((1..n).map(|i| lerp(z_b, z_win_r, i as f64 / n as f64)));
        }

        Ksztalt {
            kind,
            hw,
            h,
            clr,
            r,
            ra,
            track,
            zaf,
            zar,
            zf,
            zr,
            z_he,
            y_he,
            z_c,
            z_wt,
            z_rr,
            z_gb,
            y_tail,
            z_te,
            rear_tan: rg,
            rear_bumper_top: bumper_top,
            top,
            belt_f,
            belt_r,
            shoulder_drop: fo(p, "shoulderDrop", 0.10),
            tumble: (200.0 * p.f("tumblehome").max(0.0)).min(35.0).to_radians(), // 0,1 → 20°, 0,03 → 6°
            crown: fo(p, "roofCrown", 0.045),
            sill_tuck: fo(p, "sillTuck", 0.035),
            z_win_r,
            z_b,
            z_c_pillar,
            extra_pillars,
            z_rf,
            // przy daszku okna boczne kończą się na wysokości górnej krawędzi szyby, wyżej blacha
            glass_top: if cap { y_wt } else { f64::INFINITY },
            cab_only: rama.is_some(),
            x_inner,
            nose,
            tail,
            r_front: fo(p, "cornerRadius", 0.32),
            r_rear: fo(p, "cornerRadius", 0.32) * 0.75,
        }
    }

    pub fn top(&self, z: f64) -> f64 {
        self.top.at(z)
    }

    fn strefa(&self, z: f64) -> Strefa {
        if z > self.z_c {
            Strefa::Maska
        } else if z > self.z_wt {
            Strefa::Szyba
        } else if z > self.z_rr {
            Strefa::Dach
        } else if z > self.z_gb {
            Strefa::TylnaSzyba
        } else {
            Strefa::Pokrywa
        }
    }

    pub fn belt(&self, z: f64) -> f64 {
        let t = ((self.z_c - z) / (self.z_c - self.zr)).clamp(0.0, 1.0);
        lerp(self.belt_f, self.belt_r, t)
    }

    /// Wysokość krawędzi otworu nadkola w `z` (None poza nadkolem).
    pub fn arch(&self, z: f64) -> Option<f64> {
        let d = (z - self.zaf).abs().min((z - self.zar).abs());
        (d < self.ra).then(|| self.r + (self.ra * self.ra - d * d).sqrt())
    }

    pub fn bottom(&self, z: f64) -> f64 {
        // spód podnosi się za osiami (kąty natarcia i zejścia)
        let f = smoothstep(self.zaf + 0.1, self.zf, z);
        // kabina ciężarówki: tył ucięty przed tylną osią — spód płaski
        let b = if self.zr > self.zar { 0.0 } else { smoothstep(self.zar - 0.1, self.zr, z) };
        self.clr + f * 0.08 + b * 0.12
    }

    /// Kluczowe punkty przekroju (x, y) w `z`, bez zwężenia końców.
    pub fn key(&self, z: f64) -> [[f64; 2]; N_KEY] {
        let hw = self.hw;
        let top = self.top(z);
        let belt = self.belt(z);
        let strefa = self.strefa(z);
        let yb = self.bottom(z);
        let sill_y = yb + 0.07;
        let y_sh = belt - self.shoulder_drop;
        let y_low = lerp(sill_y, y_sh, 0.45);

        // górna część: maska/pokrywa i kabina liczone osobno, mieszane płynnie przy podstawie szyb
        let cr = self.crown;
        let tan = self.tumble.tan();
        let wc = smoothstep(self.z_c + 0.10, self.z_c - 0.04, z) * smoothstep(self.z_gb - 0.10, self.z_gb + 0.04, z);
        // „szybowość”: 1 na szybie czołowej i tylnej, 0 na dachu
        let wsz = smoothstep(self.z_wt - 0.06, self.z_wt + 0.06, z).max(smoothstep(self.z_rr + 0.06, self.z_rr - 0.06, z));
        let hood = [[hw - 0.05, belt.min(top - 0.05)], [hw - 0.11, top - 0.04], [hw - 0.26, top - 0.018], [0.5 * (hw - 0.26), top - 0.005]];
        let cabin = {
            let y6 = belt;
            let y8 = top - cr - 0.025 * wsz;
            let y7 = (y8 - lerp(0.055, 0.03, wsz)).min(self.glass_top).max(y6);
            let rear = smoothstep(self.z_wt - 0.3, self.z_rr, z); // szerszy słupek D niż A
            let x7 = (hw - 0.04) - (y7 - y6) * tan;
            let x8 = x7 - lerp(0.04, lerp(0.06, 0.09, rear), wsz);
            [[hw - 0.04, y6], [x7, y7], [x8, y8.max(y7)], [0.55 * x8, top - cr * 0.28]]
        };
        let mix = |a: [f64; 2], b: [f64; 2]| [lerp(a[0], b[0], wc), lerp(a[1], b[1], wc)];
        let (p6, p7, p8, p9, p10) = (mix(hood[0], cabin[0]), mix(hood[1], cabin[1]), mix(hood[2], cabin[2]), mix(hood[3], cabin[3]), [0.0, top]);
        let _ = strefa;
        // na masce bark niżej (błotnik zaokrąglony do maski), w kabinie tuż pod linią okien
        let y_sh = y_sh.min(p6[1] - lerp(0.11, 0.04, wc));
        let y_low = y_low.min(y_sh - 0.05);
        let x_lip = hw - self.sill_tuck;
        let (y2, y3, x3) = match self.arch(z) {
            Some(ay) => {
                let ay = ay.max(sill_y);
                (ay, ay, (hw + 0.008).min(hw + 0.002 + 0.01 * ((ay - sill_y) / 0.3).min(1.0)))
            }
            None => (yb, sill_y, x_lip),
        };
        let y4 = y_low.max(y3 + 0.03);
        let y5 = y_sh.max(y4 + 0.03);
        let y6 = p6[1].max(y5 + 0.02);
        [
            [0.0, yb],
            [self.x_inner, yb],
            [self.x_inner, y2],
            [x3, y3],
            [hw - 0.01, y4],
            [hw, y5],
            [p6[0], y6],
            [p7[0], p7[1].max(y6)],
            [p8[0], p8[1].max(p7[1].max(y6))],
            p9,
            p10,
        ]
    }

    /// Okno boczne w `z` (między słupkiem A a tyłem kabiny, bez słupków B i C).
    pub fn window(&self, z: f64) -> bool {
        let pillar = |zp: f64, half: f64| (z - zp).abs() < half;
        z < self.z_c && z > self.z_win_r && !pillar(self.z_b, 0.045) && !self.z_c_pillar.is_some_and(|zc| pillar(zc, 0.06)) && !self.extra_pillars.iter().any(|&zp| pillar(zp, 0.05))
    }
}

/// Stacja siatki: z w środku nadwozia albo parametr naroża / czoła na końcach.
#[derive(Debug, Clone, Copy)]
enum Stacja {
    Z(f64),
    /// Przód: t ∈ [0, 1] łuk naroża w planie, t ∈ (1, 2] w poprzek czoła do osi.
    Przod(f64),
    Tyl(f64),
}

pub(crate) struct Wynik {
    pub track: f64,
    pub body_width: f64,
    pub stations: usize,
    pub ksztalt: Ksztalt,
    /// Powłoka (lewa i prawa połowa, zespawana) — do rzutowania detali i oklejenia.
    pub powloka: Powloka,
    /// Stacje środka nadwozia (z rosnąco) — do próbkowania boku przez oklejenie.
    pub zs: Vec<f64>,
}

/// Próg załamania normalnych powłoki (stopnie); ten sam w rzutnikach detali.
pub(crate) const CREASE: f64 = 34.0;

/// Buduje powłokę nadwozia (bez wypuszczania do siatki: robi to wywołujący po detalach).
pub(crate) fn build(p: &P, roof_mat: MatKey, gest: Gestosc) -> Wynik {
    build_with(p, roof_mat, gest, None)
}

/// Jak `build`, z jawnym układem osi (kabina pojazdu ciężkiego).
pub(crate) fn build_with(p: &P, roof_mat: MatKey, gest: Gestosc, rama: Option<Rama>) -> Wynik {
    let k = Ksztalt::with_frame(p, rama);
    let zs_front = k.zf - k.r_front - 0.02;
    let zs_rear = k.zr + k.r_rear + 0.06;
    // stacje środka: równy odstęp + linie cech + łuki nadkoli
    let mut zs: Vec<f64> = Vec::new();
    let n = ((zs_front - zs_rear) / gest.ds).ceil() as usize;
    zs.extend((0..=n).map(|i| lerp(zs_rear, zs_front, i as f64 / n as f64)));
    for za in [k.zaf, k.zar] {
        let steps = (180.0 / gest.arch_deg).round() as i32;
        for i in 0..=steps {
            let a = (i as f64 / steps as f64 - 0.5) * std::f64::consts::PI;
            zs.push(za + k.ra * a.sin());
        }
        zs.extend([za - k.ra - 0.004, za + k.ra + 0.004]);
    }
    zs.extend([k.z_c, k.z_c - 0.02, k.z_wt, k.z_rr, k.z_gb, k.z_win_r, k.z_b - 0.045, k.z_b + 0.045]);
    if let Some(c) = k.z_c_pillar {
        zs.extend([c - 0.06, c + 0.06]);
    }
    zs.retain(|&z| z > zs_rear && z < zs_front);
    zs.sort_by(f64::total_cmp);
    zs.dedup_by(|a, b| (*a - *b).abs() < 0.008);

    let mut st: Vec<Stacja> = Vec::new();
    let ne = gest.end;
    st.extend((0..=2 * ne).rev().map(|i| Stacja::Tyl(i as f64 / ne as f64)));
    st.push(Stacja::Z(zs_rear));
    st.extend(zs.iter().map(|&z| Stacja::Z(z)));
    st.push(Stacja::Z(zs_front));
    st.extend((0..=2 * ne).map(|i| Stacja::Przod(i as f64 / ne as f64)));

    // punkt siatki: (x, y, z) dla stacji i kluczowego rzędu (po zwężeniu końców)
    let point = |s: Stacja, j: usize| -> V3 {
        match s {
            Stacja::Z(z) => {
                let q = k.key(z)[j];
                [q[0], q[1], z]
            }
            Stacja::Przod(t) | Stacja::Tyl(t) => {
                let front = matches!(s, Stacja::Przod(_));
                let z0 = if front { zs_front } else { zs_rear };
                let base = k.key(z0)[j];
                // czoło rzędu: najbardziej wysunięte z dla jego wysokości
                // górne rzędy (od linii okien) kończą się razem z górną powierzchnią: krawędź maski z przodu,
                // koniec klapy / pokrywy z tyłu — bez stopnia między górą a czołem
                let zend = match (front, j >= R_BELT) {
                    (true, true) => k.z_he,
                    (true, false) => k.nose.at(base[1]).min(k.zf),
                    (false, true) => k.z_te,
                    (false, false) => k.tail.at(base[1]).max(k.zr),
                };
                // naroże w planie: wyżej (maska, błotniki) większy promień niż przy zderzaku
                let rad = if front { k.r_front } else { k.r_rear } * match j {
                    R_BELT.. => 1.6,
                    R_SH => 1.25,
                    _ => 1.0,
                };
                let (zz, fx) = if t <= 1.0 {
                    let phi = t * std::f64::consts::FRAC_PI_2;
                    (lerp(z0, zend, phi.sin()), 1.0 - rad / k.hw * (1.0 - phi.cos()))
                } else {
                    (zend, (1.0 - rad / k.hw) * (2.0 - t))
                };
                let q = if t <= 1.0 { k.key(zz)[j] } else { k.key(zend)[j] };
                // wysokość: z przekroju w docelowym z (maska opada do krawędzi), szerokość: zwężenie planu
                [base[0].max(q[0]).min(k.hw) * fx.max(0.0), q[1], zz]
            }
        }
    };

    // rzędy z podziałami (dodatkowe rzędy w długich odcinkach przekroju)
    let mut rows: Vec<(usize, f64)> = Vec::new(); // (kluczowy odcinek, t w odcinku)
    for j in 0..N_KEY - 1 {
        let sub = match j {
            R_LIP | R_LOW | R_RAIL | R_MID => gest.sub,
            _ => 0,
        };
        for s in 0..=sub {
            rows.push((j, s as f64 / (sub + 1) as f64));
        }
    }
    rows.push((N_KEY - 1, 0.0));

    let grid: Vec<Vec<V3>> = st
        .iter()
        .map(|&s| {
            let key: Vec<V3> = (0..N_KEY).map(|j| point(s, j)).collect();
            rows.iter()
                .map(|&(j, t)| {
                    if t == 0.0 {
                        key[j]
                    } else {
                        // Catmull-Rom w przekroju: wypukłość boku i dachu
                        let pm = key[j.saturating_sub(1)];
                        let (p0, p1) = (key[j], key[j + 1]);
                        let p2 = key[(j + 2).min(N_KEY - 1)];
                        cr(pm, p0, p1, p2, t)
                    }
                })
                .collect()
        })
        .collect();

    // materiał komórki wg strefy w jej własnym z (na zaokrąglonych końcach rzędy mają różne z)
    let face = |s: Stacja| matches!(s, Stacja::Przod(t) | Stacja::Tyl(t) if t > 1.0 + 1e-9);
    let mats: Vec<Vec<MatKey>> = (0..st.len() - 1)
        .map(|i| {
            let across = face(st[i]) || face(st[i + 1]);
            (0..rows.len() - 1)
                .map(|jj| {
                    let j = rows[jj].0;
                    let zc = (grid[i][jj][2] + grid[i + 1][jj][2] + grid[i][jj + 1][2] + grid[i + 1][jj + 1][2]) / 4.0;
                    let strefa = k.strefa(zc);
                    match j {
                        0 | R_FLOOR | R_WELL => MatKey::Plastic,
                        R_BELT if !across && k.window(zc) => MatKey::Glass,
                        R_RAIL | R_MID if !across && matches!(strefa, Strefa::Szyba | Strefa::TylnaSzyba) => MatKey::Glass,
                        R_GT | R_RAIL | R_MID if !across && strefa == Strefa::Dach => roof_mat,
                        _ => MatKey::Paint,
                    }
                })
                .collect()
        })
        .collect();

    let mut pw = Powloka::default();
    let ids: Vec<Vec<u32>> = grid.iter().map(|r| r.iter().map(|&p| pw.vert(p)).collect()).collect();
    for i in 0..ids.len() - 1 {
        for jj in 0..rows.len() - 1 {
            let mat = mats[i][jj];
            pw.set_group(match mat {
                MatKey::Glass => 1,
                MatKey::Plastic => 2,
                _ => 0,
            });
            pw.quad(ids[i][jj], ids[i][jj + 1], ids[i + 1][jj + 1], ids[i + 1][jj], mat);
        }
    }
    pw.mirror_x();
    pw.weld(1e-7);
    pw.heal_t_junctions(1e-6);
    let zs_mid: Vec<f64> = st.iter().filter_map(|s| if let Stacja::Z(z) = s { Some(*z) } else { None }).collect();
    Wynik { track: k.track, body_width: 2.0 * k.hw, stations: st.len(), ksztalt: k, powloka: pw, zs: zs_mid }
}

fn cr(p0: V3, p1: V3, p2: V3, p3: V3, t: f64) -> V3 {
    let (t2, t3) = (t * t, t * t * t);
    std::array::from_fn(|i| 0.5 * ((2.0 * p1[i]) + (-p0[i] + p2[i]) * t + (2.0 * p0[i] - 5.0 * p1[i] + 4.0 * p2[i] - p3[i]) * t2 + (-p0[i] + 3.0 * p1[i] - 3.0 * p2[i] + p3[i]) * t3))
}

impl Ksztalt {
    /// Półprzekrój w formacie starego `Half` (dla `sluzba`): dno, próg, talia, linia okien,
    /// róg szyby, krawędź dachu, oś dachu.
    pub fn half(&self, z: f64) -> Half {
        let q = self.key(z);
        let v = |p: [f64; 2]| [p[0], p[1], z];
        [v(q[0]), v(q[R_LIP]), v(q[R_LOW]), v(q[R_BELT]), v(q[R_GT]), v(q[R_RAIL]), v(q[R_TOP])]
    }
}

/// Oklejenie służb na powłoce: dotychczasowy `sluzba::apply` z bokiem próbkowanym rzutnikiem.
pub(crate) fn livery(mb: &mut procedura_kit::mesh::MeshBuilder, p: &P, seed: u32, w: &Wynik) -> Option<(procedura_kit::mesh::MeshData, V3)> {
    use procedura_kit::rzutnik::{Rzutnik, Widok};
    let k = &w.ksztalt;
    let side = Rzutnik::new(&w.powloka, CREASE, Widok::Lewy);
    let surf = |z: f64, y: f64| side.hit(z, y).map(|h| h.p[0]);
    let half = |z: f64| k.half(z);
    let top = |z: f64| k.top(z);
    let hull = crate::sluzba::Hull {
        zs: &w.zs,
        half: &half,
        top: &top,
        van: k.kind == Rodzaj::Van,
        h: k.h,
        yb: k.belt_f,
        clr: k.clr,
        ytail: k.y_tail,
        zf: k.zf,
        zn: k.z_he,
        zr: k.zr,
        zg: k.z_gb,
        cowl: k.z_c,
        roof_f: k.z_rf,
        roof_r: k.z_rr,
        // van: ściana ładowni bez okien zaczyna się za drzwiami kabiny
        z_dw: if k.kind == Rodzaj::Van { k.z_win_r } else { f64::NEG_INFINITY },
        surf: Some(&surf),
    };
    // oklejenie powstaje płasko na płaszczyźnie tyłu z = zr; tu jest rzutowane na klapę i zderzak
    let mut tmp = procedura_kit::mesh::MeshBuilder::default();
    let bar = crate::sluzba::apply(&mut tmp, p, seed, &hull);
    let mut m = tmp.build(vec![]);
    let rear = Rzutnik::limited(&w.powloka, CREASE, Widok::Tyl, -k.zr - 0.6);
    // punkt z płaszczyzny tyłu na powierzchnię (odstęp warstwy zachowany; poza powłoką — ku osi)
    let drape = |p: V3| -> V3 {
        let lift = p[2] - k.zr;
        let mut x = p[0];
        for _ in 0..40 {
            if let Some(h) = rear.hit(x, p[1]) {
                return [x, p[1], h.p[2] + lift];
            }
            x *= 0.97;
        }
        p
    };
    for part in &mut m.parts {
        let pos: Vec<V3> = part.positions.chunks_exact(3).map(|v| [v[0] as f64, v[1] as f64, v[2] as f64]).collect();
        let nrm: Vec<[f32; 3]> = part.normals.chunks_exact(3).map(|v| [v[0], v[1], v[2]]).collect();
        let on_rear = |i: usize| nrm[i][2] < -0.9 && pos[i][2] <= k.zr + 0.001 && pos[i][2] >= k.zr - 0.03;
        let (mut np, mut nn, mut nu, mut ni) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for t in part.indices.chunks_exact(3) {
            let ids = [t[0] as usize, t[1] as usize, t[2] as usize];
            let tri = ids.map(|i| pos[i]);
            // trójkąt z tyłu dzielony na drobną siatkę (≤ 4 cm), żeby szedł za krzywizną klapy/drzwi
            let n = if ids.iter().all(|&i| on_rear(i)) {
                let e = (0..3).map(|j| procedura_kit::mesh::len(procedura_kit::mesh::sub(tri[(j + 1) % 3], tri[j]))).fold(0.0, f64::max);
                (e / 0.04).ceil().max(1.0) as usize
            } else {
                1
            };
            let draped = n > 1 || ids.iter().all(|&i| on_rear(i));
            let base = (np.len() / 3) as u32;
            let mut idx_of = std::collections::HashMap::new();
            for i in 0..=n {
                for j in 0..=(n - i) {
                    let (u, v) = (i as f64 / n as f64, j as f64 / n as f64);
                    let wgt = [1.0 - u - v, u, v];
                    let p: V3 = std::array::from_fn(|c| wgt[0] * tri[0][c] + wgt[1] * tri[1][c] + wgt[2] * tri[2][c]);
                    let p = if draped { drape(p) } else { p };
                    idx_of.insert((i, j), base + (np.len() / 3) as u32 - base);
                    np.extend(p.map(|c| c as f32));
                    nn.extend(nrm[ids[0]]);
                    nu.extend([p[0] as f32, p[1] as f32]);
                }
            }
            let id = |i: usize, j: usize| idx_of[&(i, j)];
            for i in 0..n {
                for j in 0..(n - i) {
                    ni.extend([id(i, j), id(i + 1, j), id(i, j + 1)]);
                    if j + 1 < n - i {
                        ni.extend([id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)]);
                    }
                }
            }
        }
        (part.positions, part.normals, part.uvs, part.indices) = (np, nn, nu, ni);
    }
    mb.push(&m, &procedura_kit::mesh::Xform::identity());
    bar
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{resolve, TaborSpec};
    use procedura_kit::mesh::{cross, dot, sub};
    use serde_json::json;

    fn shell(spec: serde_json::Value) -> (Wynik, f64) {
        let spec: TaborSpec = serde_json::from_value(spec).expect("spec");
        let r = resolve(&spec);
        let p = P(&r.params);
        let w = build(&p, MatKey::Paint, Gestosc::LOD0);
        (w, p.f("length") * p.f("width") * p.f("height"))
    }

    /// Objętość ze wzoru dywergencji: dodatnia, gdy normalne trójkątów wskazują na zewnątrz.
    fn volume(pw: &Powloka) -> f64 {
        pw.tris.iter().map(|t| {
            let [a, b, c] = t.map(|i| pw.pos[i as usize]);
            dot(a, cross(sub(b, a), sub(c, a))) / 6.0
        }).sum()
    }

    fn check(name: &str, w: &Wynik, box_vol: f64) {
        let pw = &w.powloka;
        assert!(pw.pos.iter().all(|p| p.iter().all(|c| c.is_finite())), "{name}: NaN w powłoce");
        let open = pw.open_edges();
        assert!(open.is_empty(), "{name}: powłoka nieszczelna, {} krawędzi, np. {:?}", open.len(), open.iter().take(3).map(|e| (pw.pos[e.0 as usize], pw.pos[e.1 as usize], e.2)).collect::<Vec<_>>());
        for t in 0..pw.tris.len() {
            assert!(pw.face(t).1 > 1e-12, "{name}: zdegenerowany trójkąt");
        }
        let v = volume(pw);
        assert!(v > 0.45 * box_vol && v < 0.95 * box_vol, "{name}: objętość {v:.2} m³ przy prostopadłościanie {box_vol:.2} (normalne do środka?)");
    }

    #[test]
    fn car_shells_are_watertight_and_face_outward() {
        for a in ["hatchback", "hatch_miejski", "sedan_liftback", "coupe", "kombi", "suv", "suv_duzy", "van_sredni", "van_polmaska", "van"] {
            let (w, bv) = shell(json!({ "archetype": a, "seed": 3 }));
            check(a, &w, bv);
        }
    }

    #[test]
    fn random_mixes_and_overrides_stay_valid() {
        let cars = ["hatchback", "hatch_miejski", "sedan_liftback", "coupe", "kombi", "suv", "suv_duzy", "van_sredni", "van_polmaska", "van"];
        for seed in 0..40u32 {
            let (a, b) = (cars[seed as usize % cars.len()], cars[(seed as usize * 3 + 1) % cars.len()]);
            let t = (seed as f64 * 0.137).fract();
            let (w, bv) = shell(json!({ "weights": { a: t, b: 1.0 - t }, "seed": seed, "variation": 1.0 }));
            check(&format!("{a}/{b} {t:.2} seed {seed}"), &w, bv);
        }
        // kabiny ciężarówek: nad silnikiem (brygadowa) i z półmaską — jawny układ osi
        for (top, ycb, len, nose) in [(3.0, 0.88, 3.27, 0.0), (2.45, 0.51, 2.2, 0.6)] {
            let mut m = resolve(&serde_json::from_value::<TaborSpec>(json!({ "archetype": "van" })).expect("spec")).params;
            for (key, v) in [("height", top), ("clearance", ycb), ("length", len), ("windshieldAngle", if nose > 0.0 { 60.0 } else { 82.0 }), ("windshieldTop", (top - 0.3) / top), ("hoodLength", (nose + 0.06) / len), ("cargoWindows", 1.0)] {
                m.insert(key.into(), json!(v));
            }
            let w = build_with(&P(&m), MatKey::Paint, Gestosc::LOD0, Some(Rama { zaf: 1.2, zar: -1.0e3, zf: 2.6, zr: 2.6 - len }));
            check(&format!("kabina {len} m"), &w, len * 2.4 * (top - ycb));
        }
        for o in [json!({ "wheelbase": 3.4, "length": 4.3 }), json!({ "height": 1.35, "windshieldAngle": 20 }), json!({ "rearGlassAngle": 90, "tailLean": 0.0 }), json!({ "hoodLength": 0.45, "length": 3.6, "wheelbase": 2.3 })] {
            let (w, bv) = shell(json!({ "archetype": "hatchback", "overrides": o }));
            check(&format!("hatchback {o}"), &w, bv);
        }
    }
}
