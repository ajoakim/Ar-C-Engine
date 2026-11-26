//! Integer-only terrain palette texture (x = height band, y = mask).

pub const PALETTE_W: u32 = 64;
pub const PALETTE_H: u32 = 4;
/// Rows of the debug palette (one per cell/level colour selector value).
pub const DEBUG_PALETTE_H: u32 = 16;

/// (start x, colour): sand, green, brown, grey, white.
const BANDS: [(u32, [u32; 3]); 5] = [
    (0, [214, 196, 140]),
    (3, [80, 140, 60]),
    (20, [120, 96, 64]),
    (40, [128, 128, 132]),
    (56, [245, 245, 250]),
];

fn lerp(a: u32, b: u32, num: u32, den: u32) -> u32 {
    (a * (den - num) + b * num) / den
}

fn band_color(x: u32) -> [u32; 3] {
    let mut i = 0;
    while i + 1 < BANDS.len() && x >= BANDS[i + 1].0 {
        i += 1;
    }
    if i + 1 >= BANDS.len() {
        return BANDS[BANDS.len() - 1].1;
    }
    let (x0, c0) = BANDS[i];
    let (x1, c1) = BANDS[i + 1];
    let span = x1 - x0;
    let t = x - x0;
    [
        lerp(c0[0], c1[0], t, span),
        lerp(c0[1], c1[1], t, span),
        lerp(c0[2], c1[2], t, span),
    ]
}

/// RGBA8 palette, `PALETTE_W * PALETTE_H * 4` bytes.
#[must_use]
pub fn palette_rgba8() -> Vec<u8> {
    let tan = [210u32, 180, 120];
    let lush = [60u32, 130, 50];
    let mut out = Vec::with_capacity((PALETTE_W * PALETTE_H * 4) as usize);
    for y in 0..PALETTE_H {
        for x in 0..PALETTE_W {
            let c = band_color(x);
            for k in 0..3 {
                // Dry rows tint toward tan, lush rows toward green.
                let tint = lerp(tan[k], lush[k], y, PALETTE_H - 1);
                let v = (c[k] + tint) / 2;
                out.push(u8::try_from(v.min(255)).unwrap_or(255));
            }
            out.push(255);
        }
    }
    out
}

/// Integer hue ramp (6 sectors of 256 steps) to RGB at full saturation, value `v` (0..=255).
fn hue_rgb(h: u32, v: u32) -> [u32; 3] {
    let sector = (h / 256) % 6;
    let f = h % 256;
    let (r, g, b) = match sector {
        0 => (255, f, 0),
        1 => (255 - f, 255, 0),
        2 => (0, 255, f),
        3 => (0, 255 - f, 255),
        4 => (f, 0, 255),
        _ => (255, 0, 255 - f),
    };
    [r * v / 255, g * v / 255, b * v / 255]
}

/// Debug RGBA8 palette, `PALETTE_W * DEBUG_PALETTE_H * 4` bytes: 16 rows of distinct hues, the
/// columns shade each hue by height (dark at the low end, bright at the high end).
#[must_use]
pub fn debug_palette_rgba8() -> Vec<u8> {
    let mut out = Vec::with_capacity((PALETTE_W * DEBUG_PALETTE_H * 4) as usize);
    for y in 0..DEBUG_PALETTE_H {
        // 16 hues spread over 1536 steps; the stride 7 keeps neighbouring rows far apart.
        let hue = (y * 7 % DEBUG_PALETTE_H) * (6 * 256) / DEBUG_PALETTE_H;
        for x in 0..PALETTE_W {
            let v = 110 + 145 * x / (PALETTE_W - 1);
            for c in hue_rgb(hue, v) {
                out.push(u8::try_from(c.min(255)).unwrap_or(255));
            }
            out.push(255);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length() {
        assert_eq!(palette_rgba8().len(), 64 * 4 * 4);
    }

    #[test]
    fn debug_length_opaque_and_rows_distinct() {
        let p = debug_palette_rgba8();
        assert_eq!(p.len(), (PALETTE_W * DEBUG_PALETTE_H * 4) as usize);
        assert!(p.chunks(4).all(|c| c[3] == 255));
        let row = |y: usize| p[y * PALETTE_W as usize * 4..][..4].to_vec();
        for a in 0..DEBUG_PALETTE_H as usize {
            for b in a + 1..DEBUG_PALETTE_H as usize {
                assert_ne!(row(a), row(b), "rows {a} and {b}");
            }
        }
    }

    #[test]
    fn opaque() {
        assert!(palette_rgba8().chunks(4).all(|p| p[3] == 255));
    }
}
