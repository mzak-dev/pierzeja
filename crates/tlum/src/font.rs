//! Font bitmapowy 5×7 (wzór: `tabor/src/sluzba.rs`) — napisy na odzieży jako prostokąty
//! scalonych ciągów pikseli.

use procedura_kit::mesh::V3;

fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        _ => [0; 7],
    }
}

/// Szerokość napisu w komórkach (znak 5 + odstęp 1).
pub(crate) fn text_cols(s: &str) -> usize {
    6 * s.chars().count() - 1
}

/// Napis: `map(kolumna, wiersz od dołu)` → punkt 3D, `emit(czworokąt, środek kolumny, środek wiersza)`.
pub(crate) fn text(s: &str, map: impl Fn(f64, f64) -> V3, mut emit: impl FnMut([V3; 4], f64, f64)) {
    for (i, ch) in s.chars().enumerate() {
        let g = glyph(ch);
        let px = |c: usize, r: usize| g[r] >> (4 - c) & 1 == 1;
        // prostokąty (c0, c1, wiersz od góry, wysokość): ciągi w wierszach scalane w dół
        // albo ciągi w kolumnach scalane w bok — wybór mniejszej liczby
        let mut rows: Vec<(usize, usize, usize, usize)> = vec![];
        for r in 0..7 {
            let mut c = 0;
            while c < 5 {
                if !px(c, r) {
                    c += 1;
                    continue;
                }
                let c0 = c;
                while c < 5 && px(c, r) {
                    c += 1;
                }
                match rows.iter_mut().find(|b| b.0 == c0 && b.1 == c && b.2 + b.3 == r) {
                    Some(b) => b.3 += 1,
                    None => rows.push((c0, c, r, 1)),
                }
            }
        }
        let mut cols: Vec<(usize, usize, usize, usize)> = vec![];
        for c in 0..5 {
            let mut r = 0;
            while r < 7 {
                if !px(c, r) {
                    r += 1;
                    continue;
                }
                let r0 = r;
                while r < 7 && px(c, r) {
                    r += 1;
                }
                match cols.iter_mut().find(|b| b.2 == r0 && b.3 == r - r0 && b.1 == c) {
                    Some(b) => b.1 += 1,
                    None => cols.push((c, c + 1, r0, r - r0)),
                }
            }
        }
        let boxes = if cols.len() < rows.len() { cols } else { rows };
        for (c0, c1, r, hgt) in boxes {
            let (x0, x1) = ((6 * i + c0) as f64, (6 * i + c1) as f64);
            let (y0, y1) = ((7 - r - hgt) as f64, (7 - r) as f64);
            emit([map(x0, y0), map(x1, y0), map(x1, y1), map(x0, y1)], (x0 + x1) / 2.0, (y0 + y1) / 2.0);
        }
    }
}
