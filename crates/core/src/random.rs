//! Deterministyczna losowość, port 1:1 z prototypu (arytmetyka 32-bitowa jak w JS).

/// ToUint32 z JS dla liczby już całkowitej (modulo 2^32).
pub fn js_u32(x: f64) -> u32 {
    if !x.is_finite() {
        return 0;
    }
    (x.trunc() as i64) as u32
}

/// `Math.round` z JS: połówki w stronę +∞.
pub fn jround(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

pub struct Mulberry32(u32);

impl Mulberry32 {
    pub fn new(seed: u32) -> Self {
        Self(seed)
    }
    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        (t ^ (t >> 14)) as f64 / 4_294_967_296.0
    }
}

fn mix(h: u32, x: f64) -> u32 {
    let mut k = js_u32((x * 1_000_003.0).floor());
    k = (k ^ (k >> 16)).wrapping_mul(0x045d_9f3b);
    k = (k ^ (k >> 16)).wrapping_mul(0x045d_9f3b);
    k ^= k >> 16;
    (h ^ k).wrapping_mul(0x0100_0193)
}

fn finish(mut h: u32) -> f64 {
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h as f64 / 4_294_967_296.0
}

/// Hash kilku liczb do [0, 1).
pub fn hash01(xs: &[f64]) -> f64 {
    finish(xs.iter().fold(0x811c_9dc5, |h, &x| mix(h, x)))
}

/// `hash01(seed, ...xs)` bez alokacji.
pub fn hash01_seeded(seed: u32, xs: &[f64]) -> f64 {
    finish(xs.iter().fold(mix(0x811c_9dc5, seed as f64), |h, &x| mix(h, x)))
}

pub fn hash_string(s: &str) -> u32 {
    // JS iteruje po jednostkach UTF-16
    s.encode_utf16().fold(2_166_136_261u32, |h, c| (h ^ c as u32).wrapping_mul(16_777_619))
}
