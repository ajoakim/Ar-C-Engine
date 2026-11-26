//! Integer-only terrain palette texture (x = height band, y = mask in v1; the dry-to-rock ramp in
//! v2).

pub const PALETTE_W: u32 = 64;
pub const PALETTE_H: u32 = 4;
/// Rows of the generator-v2 palette: texel row `r` is ramp value `q = r / 7`.
pub const PALETTE_H_V2: u32 = 8;
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

/// Generator-v2 bands (start x, colour): sand, green, olive-brown, rock, snow. Sand to green is a
/// wide blend (14 columns) so that sub-metre height changes never flip a hard edge.
const BANDS_V2: [(u32, [i32; 3]); 5] = [
    (0, [222, 200, 140]),
    (14, [58, 128, 52]),
    (26, [128, 104, 60]),
    (42, [120, 118, 116]),
    (56, [250, 250, 255]),
];

fn band_color_v2(x: u32) -> [i32; 3] {
    let mut i = 0;
    while i + 1 < BANDS_V2.len() && x >= BANDS_V2[i + 1].0 {
        i += 1;
    }
    if i + 1 >= BANDS_V2.len() {
        return BANDS_V2[BANDS_V2.len() - 1].1;
    }
    let (x0, c0) = BANDS_V2[i];
    let (x1, c1) = BANDS_V2[i + 1];
    let span = (x1 - x0) as i32;
    let t = (x - x0) as i32;
    [
        c0[0] + (c1[0] - c0[0]) * t / span,
        c0[1] + (c1[1] - c0[1]) * t / span,
        c0[2] + (c1[2] - c0[2]) * t / span,
    ]
}

/// Generator-v2 RGBA8 palette, `PALETTE_W * PALETTE_H_V2 * 4` bytes. Row `r` is ramp value
/// `q = r / 7`: rows 0..=3 go from lush to dry tint, rows 3..=7 blend toward rock grey with weight
/// `w = (q - 0.4) / 0.6 = (5 r - 14) / 21` (clamped at 0), reduced to 0 on the snow band so peaks
/// stay white. Integer-only.
#[must_use]
pub fn palette_rgba8_v2() -> Vec<u8> {
    let tan = [210i32, 180, 120];
    let lush = [60i32, 130, 50];
    let rock = [118i32, 116, 112];
    let mut out = Vec::with_capacity((PALETTE_W * PALETTE_H_V2 * 4) as usize);
    for y in 0..PALETTE_H_V2 {
        let ry = y as i32;
        // Lush (row 0) to dry (row 3 onwards).
        let dry = ry.min(3);
        // Rock weight over 21.
        let w = (5 * ry - 14).max(0);
        for x in 0..PALETTE_W {
            let band = band_color_v2(x);
            // Snow weight over 8 on columns 48..=56: tints and rock fade out toward the peaks.
            let snow = (x as i32 - 48).clamp(0, 8);
            for k in 0..3 {
                let tint = lush[k] + (tan[k] - lush[k]) * dry / 3;
                // A quarter tint keeps the bands strong while rows still differ.
                let tinted = band[k] + (tint - band[k]) * (8 - snow) / 32;
                // Rock shade lightens with height.
                let shade = rock[k] + (x as i32) * 40 / 63 - 20;
                let mix = w * (8 - snow) / 8;
                let v = (tinted * (21 - mix) + shade * mix) / 21;
                out.push(u8::try_from(v.clamp(0, 255)).unwrap_or(255));
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

    fn fnv(bytes: &[u8]) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    #[test]
    fn palette_v1_unchanged() {
        // Recorded from the plan 24 palette (`palette_rgba8`), which v2 must not touch.
        assert_eq!(palette_rgba8().len(), 64 * 4 * 4);
        assert_eq!(fnv(&palette_rgba8()), PALETTE_V1_FNV);
        assert_eq!(fnv(&debug_palette_rgba8()), DEBUG_PALETTE_FNV);
    }

    const PALETTE_V1_FNV: u64 = 1_864_388_963_668_788_041;
    const DEBUG_PALETTE_FNV: u64 = 9_034_359_614_131_302_828;

    fn px(p: &[u8], row: u32, x: u32) -> [i32; 3] {
        let o = ((row * PALETTE_W + x) * 4) as usize;
        [i32::from(p[o]), i32::from(p[o + 1]), i32::from(p[o + 2])]
    }

    fn dist(a: [i32; 3], b: [i32; 3]) -> i32 {
        (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
    }

    #[test]
    fn v2_length_and_opaque() {
        let p = palette_rgba8_v2();
        assert_eq!(p.len(), (PALETTE_W * 8 * 4) as usize);
        assert!(p.chunks(4).all(|c| c[3] == 255));
    }

    #[test]
    fn v2_band_contrast() {
        let p = palette_rgba8_v2();
        let cols = [0u32, 3, 20, 40, 56];
        let colours: Vec<_> = cols.iter().map(|&x| px(&p, 0, x)).collect();
        for i in 0..colours.len() {
            for j in i + 1..colours.len() {
                assert_ne!(colours[i], colours[j]);
            }
        }
        for i in 0..colours.len() - 1 {
            let d = dist(colours[i], colours[i + 1]);
            assert!(d >= 40, "columns {} and {}: {d}", cols[i], cols[i + 1]);
        }
    }

    #[test]
    fn v2_rock_row_differs_and_snow_stays_white() {
        let p = palette_rgba8_v2();
        for x in [3u32, 20] {
            assert!(dist(px(&p, 0, x), px(&p, 7, x)) >= 40, "column {x}");
        }
        assert_eq!(px(&p, 7, 56), px(&p, 0, 56));
        let snow = px(&p, 7, 63);
        assert!(snow.iter().all(|c| *c >= 245), "{snow:?}");
        // Row 7 on the rock band is grey (channels within 10 of each other).
        let g = px(&p, 7, 40);
        assert!(
            (g[0] - g[2]).abs() <= 10 && (g[0] - g[1]).abs() <= 10,
            "{g:?}"
        );
    }

    #[test]
    fn v2_sand_green_edge_is_a_wide_blend() {
        let p = palette_rgba8_v2();
        for row in 0..PALETTE_H_V2 {
            for x in 0..8 {
                let d = dist(px(&p, row, x), px(&p, row, x + 1));
                assert!(d <= 24, "row {row} columns {x}/{}: {d}", x + 1);
            }
        }
    }

    #[test]
    fn v2_rows_progress_from_lush_to_rock() {
        let p = palette_rgba8_v2();
        // Every ramp step on a land column changes the colour visibly.
        for row in 0..PALETTE_H_V2 - 1 {
            let d = dist(px(&p, row, 20), px(&p, row + 1, 20));
            assert!(d >= 8, "rows {row}/{}: {d}", row + 1);
        }
    }
}
