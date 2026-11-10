//! Kolory: mieszanie w OKLab (Ottosson 2020), port 1:1 z prototypu.

use crate::random::jround;

pub type Rgb = [f64; 3];

pub fn hex_to_rgb(hex: &str) -> Rgb {
    let h = hex.trim_start_matches('#');
    let full: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
    let v = u32::from_str_radix(&full, 16).unwrap_or(0x808080);
    [((v >> 16) & 255) as f64 / 255.0, ((v >> 8) & 255) as f64 / 255.0, (v & 255) as f64 / 255.0]
}

pub fn rgb_to_hex(c: Rgb) -> String {
    let to = |x: f64| jround(x.clamp(0.0, 1.0) * 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", to(c[0]), to(c[1]), to(c[2]))
}

pub fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

pub fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

pub fn hex_to_linear(hex: &str) -> Rgb {
    hex_to_rgb(hex).map(srgb_to_linear)
}

fn linear_to_oklab([r, g, b]: Rgb) -> Rgb {
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
    ]
}

fn oklab_to_linear([ll, a, b]: Rgb) -> Rgb {
    let l = (ll + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (ll - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (ll - 0.0894841775 * a - 1.291485548 * b).powi(3);
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
    ]
}

pub fn hex_to_oklab(hex: &str) -> Rgb {
    linear_to_oklab(hex_to_linear(hex))
}

pub fn oklab_to_hex(lab: Rgb) -> String {
    let [r, g, b] = oklab_to_linear(lab);
    rgb_to_hex([linear_to_srgb(r.max(0.0)), linear_to_srgb(g.max(0.0)), linear_to_srgb(b.max(0.0))])
}

/// Średnia ważona kolorów w OKLab.
pub fn blend_hex<'a>(colors: impl IntoIterator<Item = (&'a str, f64)>) -> String {
    let (mut l, mut a, mut b, mut sum) = (0.0, 0.0, 0.0, 0.0);
    let mut first: Option<&str> = None;
    for (hex, w) in colors {
        first.get_or_insert(hex);
        if !(w > 0.0) {
            continue;
        }
        let [x, y, z] = hex_to_oklab(hex);
        l += x * w;
        a += y * w;
        b += z * w;
        sum += w;
    }
    if sum <= 0.0 {
        return first.unwrap_or("#808080").to_string();
    }
    oklab_to_hex([l / sum, a / sum, b / sum])
}

/// Przesunięcie jasności, chromy i odcienia (w OKLab).
pub fn adjust_hex(hex: &str, d_l: f64, chroma_scale: f64, d_hue: f64) -> String {
    let [l, a, b] = hex_to_oklab(hex);
    let c = a.hypot(b) * chroma_scale;
    let h = b.atan2(a) + d_hue;
    oklab_to_hex([(l + d_l).clamp(0.0, 1.0), c * h.cos(), c * h.sin()])
}

pub fn mix_hex(a: &str, b: &str, t: f64) -> String {
    blend_hex([(a, 1.0 - t), (b, t)])
}
