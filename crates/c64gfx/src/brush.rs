//! Preset brushes: eight stamp shapes per mode, as in ArcPaint64 (MIT licensed).
//!
//! Each brush is an 8-row bit mask stamped around a centre pixel:
//! hires masks are 8 pixels wide with centre (3, 3); multicolor masks use two bits per
//! logical pixel (4 wide) with centre (1, 3).

use crate::image::Mode;

pub const COUNT: usize = 8;

const HR: [[u8; 8]; COUNT] = [
    [0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x10, 0x38, 0x10, 0x00, 0x00, 0x00],
    [0x00, 0x38, 0x7c, 0x7c, 0x7c, 0x38, 0x00, 0x00],
    [0x38, 0x7c, 0xfe, 0xfe, 0xfe, 0x7c, 0x38, 0x00],
    [0x00, 0x00, 0x38, 0x38, 0x38, 0x00, 0x00, 0x00],
    [0x00, 0x7c, 0x7c, 0x7c, 0x7c, 0x7c, 0x00, 0x00],
    [0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0xfe, 0x00],
    [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
];

const MC: [[u8; 8]; COUNT] = [
    [0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x30, 0x30, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x30, 0xfc, 0xfc, 0xfc, 0x30, 0x00, 0x00],
    [0x30, 0xfc, 0xfc, 0xfc, 0xfc, 0xfc, 0x30, 0x00],
    [0x3c, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x3c],
    [0x00, 0x3c, 0x3c, 0x3c, 0x3c, 0x00, 0x00, 0x00],
    [0xfc, 0xfc, 0xfc, 0xfc, 0xfc, 0xfc, 0x00, 0x00],
    [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
];

/// Width of the brush mask in logical pixels.
pub fn width(mode: Mode) -> usize {
    match mode {
        Mode::Hires => 8,
        Mode::Multicolor => 4,
    }
}

/// True if mask cell (x, y) is set; `x` in 0..width(mode), `y` in 0..8.
pub fn cell(mode: Mode, index: usize, x: usize, y: usize) -> bool {
    let i = index.min(COUNT - 1);
    match mode {
        Mode::Hires => (HR[i][y] >> (7 - x)) & 1 == 1,
        Mode::Multicolor => (MC[i][y] >> (6 - 2 * x)) & 3 == 3,
    }
}

/// Pixel offsets (relative to the pointer pixel) covered by the brush.
pub fn offsets(mode: Mode, index: usize) -> Vec<(i32, i32)> {
    let centre = match mode {
        Mode::Hires => (3, 3),
        Mode::Multicolor => (1, 3),
    };
    let mut out = Vec::new();
    for y in 0..8 {
        for x in 0..width(mode) {
            if cell(mode, index, x, y) {
                out.push((x as i32 - centre.0, y as i32 - centre.1));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_brush_is_a_single_pixel() {
        assert_eq!(offsets(Mode::Hires, 0), vec![(0, 0)]);
        assert_eq!(offsets(Mode::Multicolor, 0), vec![(0, 0)]);
    }

    #[test]
    fn last_brush_is_a_full_square() {
        assert_eq!(offsets(Mode::Hires, 7).len(), 64);
        assert_eq!(offsets(Mode::Multicolor, 7).len(), 32);
    }

    #[test]
    fn every_brush_contains_its_centre() {
        for mode in [Mode::Hires, Mode::Multicolor] {
            for i in 0..COUNT {
                assert!(offsets(mode, i).contains(&(0, 0)), "brush {i} {mode:?}");
            }
        }
    }
}
