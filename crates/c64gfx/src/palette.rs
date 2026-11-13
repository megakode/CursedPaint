/// A named 16-colour C64 palette (RGB), indexed by C64 colour number.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub name: &'static str,
    pub rgb: [[u8; 3]; 16],
}

pub const COLOR_NAMES: [&str; 16] = [
    "Black", "White", "Red", "Cyan", "Purple", "Green", "Blue", "Yellow", "Orange", "Brown",
    "Light red", "Dark grey", "Grey", "Light green", "Light blue", "Light grey",
];

const fn hex(v: u32) -> [u8; 3] {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

pub const PEPTO: Palette = Palette {
    name: "Pepto",
    rgb: [
        hex(0x000000), hex(0xFFFFFF), hex(0x68372B), hex(0x70A4B2),
        hex(0x6F3D86), hex(0x588D43), hex(0x352879), hex(0xB8C76F),
        hex(0x6F4F25), hex(0x433900), hex(0x9A6759), hex(0x444444),
        hex(0x6C6C6C), hex(0x9AD284), hex(0x6C5EB5), hex(0x959595),
    ],
};

pub const COLODORE: Palette = Palette {
    name: "Colodore",
    rgb: [
        hex(0x000000), hex(0xFFFFFF), hex(0x813338), hex(0x75CEC8),
        hex(0x8E3C97), hex(0x56AC4D), hex(0x2E2C9B), hex(0xEDF171),
        hex(0x8E5029), hex(0x553800), hex(0xC46C71), hex(0x4A4A4A),
        hex(0x7B7B7B), hex(0xA9FF9F), hex(0x706DEB), hex(0xB2B2B2),
    ],
};

pub const PALETTES: [Palette; 2] = [PEPTO, COLODORE];

impl Palette {
    /// Squared RGB distance between two palette entries.
    pub fn distance(&self, a: u8, b: u8) -> u32 {
        let (a, b) = (self.rgb[(a & 15) as usize], self.rgb[(b & 15) as usize]);
        (0..3)
            .map(|i| {
                let d = a[i] as i32 - b[i] as i32;
                (d * d) as u32
            })
            .sum()
    }
}

/// How the 16 colours are *displayed* in the UI. This never changes the colour
/// numbers, the palette itself or any image data; only the order of the swatches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortMode {
    /// The C64's own order, 0..15.
    C64,
    /// Dark to light by perceived brightness. Good for judging value/shading.
    Luminance,
    /// Greys first (dark to light), then the colour wheel from red round to purple.
    /// Neighbours are analogous colours; opposite ends are complements.
    Hue,
    /// Hand-grouped shading ramps, one per row, dark to light:
    /// greys / brown-orange-skin-yellow / blue-to-white / accents.
    Ramps,
}

impl SortMode {
    pub const ALL: [SortMode; 4] = [SortMode::C64, SortMode::Luminance, SortMode::Hue, SortMode::Ramps];

    pub fn label(self) -> &'static str {
        match self {
            SortMode::C64 => "C64 order",
            SortMode::Luminance => "Luminance",
            SortMode::Hue => "Hue wheel",
            SortMode::Ramps => "Gradient ramps",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            SortMode::C64 => "The C64's own colour numbers, 0-15.",
            SortMode::Luminance => "Dark to light by brightness. Easy to judge value, shadows and highlights.",
            SortMode::Hue => "Greys first, then around the colour wheel. Neighbours are analogous, far ends are complements.",
            SortMode::Ramps => "Rows of colours that shade into each other: greys, warm earth/skin, cool blues to white, accents.",
        }
    }

    /// Colour numbers in display order (always a permutation of 0..15).
    pub fn order(self, pal: &Palette) -> [u8; 16] {
        let mut idx: [u8; 16] = std::array::from_fn(|i| i as u8);
        match self {
            SortMode::C64 => {}
            SortMode::Luminance => {
                idx.sort_by(|&a, &b| luma(pal, a).total_cmp(&luma(pal, b)).then(a.cmp(&b)));
            }
            SortMode::Hue => {
                idx.sort_by(|&a, &b| {
                    let (na, nb) = (is_neutral(pal, a), is_neutral(pal, b));
                    nb.cmp(&na) // neutrals first
                        .then_with(|| {
                            if na {
                                luma(pal, a).total_cmp(&luma(pal, b))
                            } else {
                                hue(pal, a).total_cmp(&hue(pal, b)).then(luma(pal, a).total_cmp(&luma(pal, b)))
                            }
                        })
                        .then(a.cmp(&b))
                });
            }
            SortMode::Ramps => {
                // Colour numbers: 0 black, 1 white, 2 red, 3 cyan, 4 purple, 5 green, 6 blue,
                // 7 yellow, 8 orange, 9 brown, 10 light red, 11 dark grey, 12 grey,
                // 13 light green, 14 light blue, 15 light grey.
                idx = [0, 11, 12, 15, 9, 8, 10, 7, 6, 14, 3, 1, 5, 13, 2, 4];
            }
        }
        idx
    }
}

fn luma(pal: &Palette, c: u8) -> f32 {
    let [r, g, b] = pal.rgb[(c & 15) as usize];
    0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32
}

fn chroma(pal: &Palette, c: u8) -> f32 {
    let [r, g, b] = pal.rgb[(c & 15) as usize];
    let (mx, mn) = (r.max(g).max(b) as f32, r.min(g).min(b) as f32);
    mx - mn
}

fn is_neutral(pal: &Palette, c: u8) -> bool {
    chroma(pal, c) < 12.0
}

/// Hue angle in degrees, 0 = red, increasing through orange, yellow, green, cyan, blue, purple.
fn hue(pal: &Palette, c: u8) -> f32 {
    let [r, g, b] = pal.rgb[(c & 15) as usize];
    let (r, g, b) = (r as f32, g as f32, b as f32);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let d = mx - mn;
    if d == 0.0 {
        return 0.0;
    }
    let h = if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h * 60.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_permutation(o: [u8; 16]) -> bool {
        let mut seen = [false; 16];
        o.iter().all(|&c| c < 16 && !std::mem::replace(&mut seen[c as usize], true))
    }

    #[test]
    fn every_order_is_a_permutation() {
        for pal in PALETTES {
            for m in SortMode::ALL {
                assert!(is_permutation(m.order(&pal)), "{m:?} {}", pal.name);
            }
        }
    }

    #[test]
    fn c64_order_is_identity() {
        let o = SortMode::C64.order(&PEPTO);
        assert!(o.iter().enumerate().all(|(i, &c)| c as usize == i));
    }

    #[test]
    fn luminance_is_dark_to_light() {
        let o = SortMode::Luminance.order(&PEPTO);
        assert_eq!(o[0], 0);
        assert_eq!(o[15], 1);
        assert!(o.windows(2).all(|w| luma(&PEPTO, w[0]) <= luma(&PEPTO, w[1])));
    }

    #[test]
    fn hue_puts_greys_first_then_wheel() {
        let o = SortMode::Hue.order(&PEPTO);
        assert_eq!(&o[..5], &[0, 11, 12, 15, 1]);
        let hues: Vec<f32> = o[5..].iter().map(|&c| hue(&PEPTO, c)).collect();
        assert!(hues.windows(2).all(|w| w[0] <= w[1]), "{hues:?}");
    }

    #[test]
    fn ramps_rows_shade_dark_to_light() {
        let o = SortMode::Ramps.order(&PEPTO);
        for row in o.chunks(4).take(3) {
            assert!(row.windows(2).all(|w| luma(&PEPTO, w[0]) < luma(&PEPTO, w[1])), "{row:?}");
        }
    }
}
