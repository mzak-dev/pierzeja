//! Parametry → materiały PBR (kolory sRGB, tekstury proceduralne tylko jako nazwa).

use crate::color::{adjust_hex, mix_hex};
use crate::mesh::{MatKey, MaterialSpec};
use crate::params::Params;

fn spec(key: MatKey, name: &'static str, color: String) -> MaterialSpec {
    MaterialSpec { key, name, color, roughness: 0.85, metalness: 0.0, opacity: 1.0, double_sided: false, texture: "none", texture_scale: 1.0, emissive: None }
}

pub fn build_materials(p: &Params) -> Vec<MaterialSpec> {
    let (wtex, wrough, wscale) = match p.wall_texture.as_str() {
        "brick" => ("brick", 0.88, 1.0),
        "concrete" => ("concrete", 0.9, 2.4),
        "stone" => ("stone", 0.7, 1.8),
        "wood" => ("wood", 0.8, 1.6),
        _ => ("plaster", 0.92, 2.0),
    };
    let (rtex, rrough, rmetal, rscale) = match p.roof_texture.as_str() {
        "tiles" => ("tiles", 0.75, 0.0, 1.2),
        "sheet" => ("sheet", 0.55, 0.35, 1.0),
        "seam" => ("seam", 0.45, 0.5, 1.0),
        "shingle" => ("shingle", 0.9, 0.0, 1.0),
        _ => ("tarpaper", 0.95, 0.0, 3.0),
    };
    // patyna: lekko ciemniej i mniej nasycenie
    let age = p.weathering;
    let brick = p.wall_texture == "brick";
    let slab = mix_hex(&adjust_hex(&p.wall_color, -0.12, 0.4, 0.0), "#9a9893", 0.6);
    vec![
        MaterialSpec { roughness: wrough, texture: wtex, texture_scale: wscale, ..spec(MatKey::Wall, "Ściany", adjust_hex(&p.wall_color, -0.05 * age, 1.0 - 0.25 * age, 0.0)) },
        MaterialSpec { roughness: 0.9, texture: if brick { "brick" } else { "plaster" }, texture_scale: 2.0, ..spec(MatKey::Accent, "Akcenty", adjust_hex(&p.accent_color, -0.04 * age, 1.0 - 0.2 * age, 0.0)) },
        MaterialSpec { roughness: 0.85, texture: "plaster", texture_scale: 3.0, ..spec(MatKey::Trim, "Detal", adjust_hex(&p.trim_color, -0.05 * age, 1.0 - 0.2 * age, 0.0)) },
        MaterialSpec { roughness: 0.9, texture: if brick { "brick" } else { "stone" }, texture_scale: 1.6, ..spec(MatKey::Plinth, "Cokół", adjust_hex(&p.plinth_color, -0.04 * age, 1.0, 0.0)) },
        MaterialSpec { roughness: rrough, metalness: rmetal, texture: rtex, texture_scale: rscale, double_sided: true, ..spec(MatKey::Roof, "Dach", p.roof_color.clone()) },
        MaterialSpec { roughness: 0.06, metalness: 0.4, ..spec(MatKey::Glass, "Szkło", p.glass_tint.clone()) },
        MaterialSpec { roughness: 0.06, metalness: 0.4, emissive: Some("#ffc27a"), ..spec(MatKey::GlassLit, "Szkło (oświetlone)", p.glass_tint.clone()) },
        MaterialSpec { roughness: 0.6, ..spec(MatKey::Frame, "Stolarka", p.frame_color.clone()) },
        MaterialSpec { roughness: 0.92, texture: "concrete", texture_scale: 2.0, ..spec(MatKey::Slab, "Płyty balkonów", slab) },
        MaterialSpec { roughness: 0.45, metalness: 0.6, ..spec(MatKey::Metal, "Metal", "#2e3133".into()) },
        MaterialSpec { roughness: 0.5, metalness: 0.5, texture: "bars", double_sided: true, opacity: 0.999, ..spec(MatKey::Railing, "Balustrada prętowa", "#2b2d2f".into()) },
        MaterialSpec { roughness: 0.05, metalness: 0.1, opacity: 0.32, double_sided: true, ..spec(MatKey::GlassRail, "Balustrada szklana", "#a9c3cc".into()) },
        MaterialSpec { roughness: 0.6, ..spec(MatKey::Door, "Drzwi", adjust_hex(&p.frame_color, -0.35, 0.8, 0.0)) },
        MaterialSpec { roughness: 0.95, ..spec(MatKey::Joint, "Styki i fugi", adjust_hex(&p.wall_color, -0.28, 0.6, 0.0)) },
        MaterialSpec { roughness: 1.0, ..spec(MatKey::Ground, "Teren", "#7d8a6a".into()) },
        MaterialSpec { roughness: 0.95, texture: "asphalt", texture_scale: 4.0, ..spec(MatKey::Road, "Jezdnia", "#45484b".into()) },
        MaterialSpec { roughness: 0.95, texture: "paving", texture_scale: 1.0, ..spec(MatKey::Sidewalk, "Chodnik", "#a6a49c".into()) },
    ]
}
