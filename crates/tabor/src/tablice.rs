//! Tablice rejestracyjne i kierunkowskazy wspólne dla osobówek, vana i ciężkich.
//! Tablica UE 520 × 114 mm: białe tło, niebieski pasek z „PL” po lewej stronie patrzącego,
//! czarne znaki (tu: prostokąty — z kamery gry i tak nieczytelne). Kierunkowskazy pomarańczowe
//! z przodu, z tyłu i boczne (Regulamin EKG ONZ nr 48).

use procedura_kit::mesh::{MatKey, MaterialSpec, MeshBuilder, V3};
use procedura_kit::random::hash01_seeded;

pub(crate) const PLATE_W: f64 = 0.52;
pub(crate) const PLATE_H: f64 = 0.114;

fn mat(key: MatKey, name: &'static str, color: &str, roughness: f64, emissive: Option<&'static str>) -> MaterialSpec {
    MaterialSpec { key, name, color: color.into(), roughness, metalness: 0.0, opacity: 1.0, double_sided: false, texture: "none", texture_scale: 1.0, emissive }
}

pub(crate) fn materials() -> Vec<MaterialSpec> {
    vec![
        mat(MatKey::Indicator, "Kierunkowskazy", "#f28a1a", 0.3, None),
        mat(MatKey::Plate, "Tablica", "#f3f3ef", 0.5, None),
        mat(MatKey::PlateBand, "Pasek UE", "#1f3fa3", 0.5, None),
    ]
}

/// Tablica w płaszczyźnie `z = c[2]`, środek (c[0], c[1]), zwrócona w stronę `nz` (+1 przód, −1 tył).
/// Znaki: 2–3 litery wyróżnika, odstęp, 4–5 znaków (razem 7), stałe dla `seed` i `salt`.
pub(crate) fn plate(mb: &mut MeshBuilder, seed: u32, salt: f64, c: V3, nz: f64) {
    let n = [0.0, 0.0, nz];
    // kierunek czytania (od lewej patrzącego): przód → +X, tył → −X
    let u = nz;
    let at = |s: f64, y: f64, dz: f64| -> V3 { [c[0] + u * s, c[1] + y, c[2] + nz * dz] };
    let rect = |mb: &mut MeshBuilder, m: MatKey, s0: f64, s1: f64, y0: f64, y1: f64, dz: f64| {
        mb.quad(m, at(s0, y0, dz), at(s1, y0, dz), at(s1, y1, dz), at(s0, y1, dz), Some(n));
    };
    let (hw, hh) = (PLATE_W / 2.0, PLATE_H / 2.0);
    // ciemna ramka (uchwyt tablicy), żeby tablica nie ginęła na białym zderzaku
    rect(mb, MatKey::Plastic, -hw - 0.012, hw + 0.012, -hh - 0.012, hh + 0.012, 0.0005);
    rect(mb, MatKey::Plate, -hw, hw, -hh, hh, 0.001);
    rect(mb, MatKey::PlateBand, -hw + 0.006, -hw + 0.046, -hh + 0.006, hh - 0.006, 0.002);
    // znaki: 0,042 × 0,075 m co 0,052 m, odstęp po wyróżniku
    let letters = 2 + usize::from(hash01_seeded(seed, &[salt, 1.0]) < 0.45);
    let mut s = -hw + 0.094;
    for i in 0..7 {
        if i == letters {
            s += 0.022;
        }
        rect(mb, MatKey::Plastic, s, s + 0.04, -0.037, 0.038, 0.002);
        s += 0.051;
    }
}
