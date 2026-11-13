//! A C64 bitmap image stored exactly as the hardware sees it
//! (bitmap bytes + screen RAM + colour RAM + background), with the colour
//! limits enforced on every write.

use crate::palette::Palette;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// 320x200, 2 colours per 8x8 cell (screen RAM hi/lo nibble).
    Hires,
    /// 160x200 (double-wide pixels), 4 colours per 4x8 cell:
    /// global background + screen hi + screen lo + colour RAM.
    Multicolor,
}

impl Mode {
    /// Logical pixels per cell horizontally.
    pub fn cell_px_w(self) -> usize {
        match self {
            Mode::Hires => 8,
            Mode::Multicolor => 4,
        }
    }
    /// Number of pixel codes (hardware colour slots) per cell.
    pub fn codes(self) -> u8 {
        match self {
            Mode::Hires => 2,
            Mode::Multicolor => 4,
        }
    }
    /// Screen pixels per logical pixel horizontally (multicolor pixels are double wide).
    pub fn aspect_x(self) -> usize {
        match self {
            Mode::Hires => 1,
            Mode::Multicolor => 2,
        }
    }
}

/// How an automatic-mode paint request was satisfied.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaintOutcome {
    /// Exact colour was placed (existing slot or a free one).
    Exact,
    /// An in-use slot was overwritten (force placement); other pixels changed colour.
    Forced,
    /// Colour could not fit; nearest colour already in the cell was used instead.
    Nearest,
    /// Position outside the image.
    OutOfBounds,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Image {
    pub mode: Mode,
    pub cells_w: usize,
    pub cells_h: usize,
    /// 8 bytes per cell, cell-major (the standard C64 bitmap layout for 40 columns).
    pub bitmap: Vec<u8>,
    pub screen: Vec<u8>,
    pub color: Vec<u8>,
    pub background: u8,
}

impl Image {
    pub fn new(mode: Mode, cells_w: usize, cells_h: usize) -> Self {
        let cells_w = cells_w.max(1);
        let cells_h = cells_h.max(1);
        let n = cells_w * cells_h;
        Image {
            mode,
            cells_w,
            cells_h,
            bitmap: vec![0; n * 8],
            screen: vec![0; n],
            color: vec![0; n],
            background: 0,
        }
    }

    /// Width in logical pixels (160 for a full multicolor screen, 320 for hires).
    pub fn width(&self) -> usize {
        self.cells_w * self.mode.cell_px_w()
    }
    pub fn height(&self) -> usize {
        self.cells_h * 8
    }

    fn locate(&self, x: i32, y: i32) -> Option<(usize, usize, usize)> {
        if x < 0 || y < 0 || x as usize >= self.width() || y as usize >= self.height() {
            return None;
        }
        let (x, y) = (x as usize, y as usize);
        let pw = self.mode.cell_px_w();
        let cell = (y / 8) * self.cells_w + x / pw;
        Some((cell, cell * 8 + y % 8, x % pw))
    }

    /// Pixel code (hardware colour slot index) at a pixel.
    pub fn code_at(&self, x: i32, y: i32) -> Option<u8> {
        let (_, byte, px) = self.locate(x, y)?;
        let b = self.bitmap[byte];
        Some(match self.mode {
            Mode::Hires => (b >> (7 - px)) & 1,
            Mode::Multicolor => (b >> (6 - 2 * px)) & 3,
        })
    }

    pub fn set_code(&mut self, x: i32, y: i32, code: u8) {
        let Some((_, byte, px)) = self.locate(x, y) else { return };
        let b = &mut self.bitmap[byte];
        match self.mode {
            Mode::Hires => {
                let s = 7 - px;
                *b = (*b & !(1 << s)) | ((code & 1) << s);
            }
            Mode::Multicolor => {
                let s = 6 - 2 * px;
                *b = (*b & !(3 << s)) | ((code & 3) << s);
            }
        }
    }

    /// Colour (0..15) that `code` currently maps to in `cell`.
    pub fn slot_color(&self, cell: usize, code: u8) -> u8 {
        match (self.mode, code) {
            (Mode::Hires, 0) => self.screen[cell] & 15,
            (Mode::Hires, _) => self.screen[cell] >> 4,
            (Mode::Multicolor, 0) => self.background,
            (Mode::Multicolor, 1) => self.screen[cell] >> 4,
            (Mode::Multicolor, 2) => self.screen[cell] & 15,
            (Mode::Multicolor, _) => self.color[cell] & 15,
        }
    }

    fn set_slot_color(&mut self, cell: usize, code: u8, c: u8) {
        let c = c & 15;
        match (self.mode, code) {
            (Mode::Hires, 0) => self.screen[cell] = (self.screen[cell] & 0xF0) | c,
            (Mode::Hires, _) => self.screen[cell] = (self.screen[cell] & 0x0F) | (c << 4),
            (Mode::Multicolor, 0) => self.background = c,
            (Mode::Multicolor, 1) => self.screen[cell] = (self.screen[cell] & 0x0F) | (c << 4),
            (Mode::Multicolor, 2) => self.screen[cell] = (self.screen[cell] & 0xF0) | c,
            (Mode::Multicolor, _) => self.color[cell] = c,
        }
    }

    /// Visible C64 colour at a pixel.
    pub fn color_at(&self, x: i32, y: i32) -> Option<u8> {
        let (cell, _, _) = self.locate(x, y)?;
        Some(self.slot_color(cell, self.code_at(x, y)?))
    }

    /// Number of pixels in the cell (excluding `skip`) that use each code.
    fn usage(&self, x: i32, y: i32, skip: (i32, i32)) -> [u32; 4] {
        let mut u = [0u32; 4];
        let pw = self.mode.cell_px_w() as i32;
        let (cx, cy) = ((x / pw) * pw, (y / 8) * 8);
        for py in cy..cy + 8 {
            for px in cx..cx + pw {
                if (px, py) != skip {
                    if let Some(c) = self.code_at(px, py) {
                        u[c as usize] += 1;
                    }
                }
            }
        }
        u
    }

    /// Hardware mode: write `code` into the pixel and set that slot's colour
    /// in the pixel's cell. Other pixels in the cell using the same code change
    /// colour too (that is how the hardware works). Multicolor code 0 sets the
    /// global background colour.
    pub fn paint_hardware(&mut self, x: i32, y: i32, code: u8, color: u8) {
        let Some((cell, _, _)) = self.locate(x, y) else { return };
        let code = code.min(self.mode.codes() - 1);
        self.set_code(x, y, code);
        self.set_slot_color(cell, code, color);
    }

    /// Automatic mode: paint the colour you want and let the editor pick a valid slot.
    /// With `force`, if every slot is in use by other pixels, the least-used slot is
    /// recoloured (changing those pixels); otherwise the nearest available colour is used.
    pub fn paint_auto(&mut self, x: i32, y: i32, color: u8, force: bool, pal: &Palette) -> PaintOutcome {
        let Some((cell, _, _)) = self.locate(x, y) else { return PaintOutcome::OutOfBounds };
        let color = color & 15;
        let n = self.mode.codes();

        // 1. Colour already available in this cell. Prefer slots that are in use.
        let usage = self.usage(x, y, (x, y));
        let mut found: Option<u8> = None;
        for code in 0..n {
            if self.slot_color(cell, code) == color {
                if found.is_none() || usage[code as usize] > 0 && usage[found.unwrap() as usize] == 0 {
                    found = Some(code);
                }
            }
        }
        if let Some(code) = found {
            self.set_code(x, y, code);
            return PaintOutcome::Exact;
        }

        // 2. A slot nobody else uses can be recoloured freely
        //    (multicolor code 0 is the global background and is never per-cell).
        let first_editable = if self.mode == Mode::Multicolor { 1 } else { 0 };
        if let Some(code) = (first_editable..n).find(|&c| usage[c as usize] == 0) {
            self.set_slot_color(cell, code, color);
            self.set_code(x, y, code);
            return PaintOutcome::Exact;
        }

        // 3. Everything is taken.
        if force {
            let code = (first_editable..n).min_by_key(|&c| usage[c as usize]).unwrap();
            self.set_slot_color(cell, code, color);
            self.set_code(x, y, code);
            return PaintOutcome::Forced;
        }
        let code = (0..n)
            .min_by_key(|&c| pal.distance(self.slot_color(cell, c), color))
            .unwrap();
        self.set_code(x, y, code);
        PaintOutcome::Nearest
    }

    /// How many pixels in a cell use each pixel code (hardware slot).
    pub fn cell_code_usage(&self, cell_x: usize, cell_y: usize) -> [u32; 4] {
        let pw = self.mode.cell_px_w();
        self.usage((cell_x * pw) as i32, (cell_y * 8) as i32, (-1, -1))
    }

    /// Flood-fill region of equal visible colour. Returns the pixels in the region.
    pub fn region_at(&self, x: i32, y: i32) -> Vec<(i32, i32)> {
        let Some(target) = self.color_at(x, y) else { return vec![] };
        let (w, h) = (self.width(), self.height());
        let mut seen = vec![false; w * h];
        let mut stack = vec![(x, y)];
        let mut out = Vec::new();
        while let Some((px, py)) = stack.pop() {
            if self.color_at(px, py) != Some(target) {
                continue;
            }
            let i = py as usize * w + px as usize;
            if seen[i] {
                continue;
            }
            seen[i] = true;
            out.push((px, py));
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (px + dx, py + dy);
                if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                    stack.push((nx, ny));
                }
            }
        }
        out
    }

    /// RGBA pixels at display resolution (multicolor pixels doubled horizontally).
    /// Returns (width, height, rgba).
    pub fn to_rgba(&self, pal: &Palette) -> (usize, usize, Vec<u8>) {
        let ax = self.mode.aspect_x();
        let (w, h) = (self.width() * ax, self.height());
        let mut out = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let c = self.color_at((x / ax) as i32, y as i32).unwrap_or(0);
                let [r, g, b] = pal.rgb[c as usize];
                out.extend_from_slice(&[r, g, b, 255]);
            }
        }
        (w, h, out)
    }

    /// Per-cell colour usage summary: how many distinct colours the cell shows.
    pub fn distinct_colors_in_cell(&self, cell_x: usize, cell_y: usize) -> usize {
        let pw = self.mode.cell_px_w();
        let mut seen = [false; 16];
        for py in 0..8 {
            for px in 0..pw {
                if let Some(c) = self.color_at((cell_x * pw + px) as i32, (cell_y * 8 + py) as i32) {
                    seen[c as usize] = true;
                }
            }
        }
        seen.iter().filter(|&&s| s).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::PEPTO;

    fn cell_colors(img: &Image, cx: usize, cy: usize) -> Vec<u8> {
        let pw = img.mode.cell_px_w();
        let mut v = Vec::new();
        for y in 0..8 {
            for x in 0..pw {
                let c = img.color_at((cx * pw + x) as i32, (cy * 8 + y) as i32).unwrap();
                if !v.contains(&c) {
                    v.push(c);
                }
            }
        }
        v
    }

    #[test]
    fn hires_never_exceeds_two_colors_per_cell() {
        let mut img = Image::new(Mode::Hires, 4, 4);
        for (i, c) in [2u8, 5, 7, 1, 3].iter().enumerate() {
            img.paint_auto(i as i32, 0, *c, false, &PEPTO);
        }
        assert!(cell_colors(&img, 0, 0).len() <= 2);
    }

    #[test]
    fn multicolor_allows_background_plus_three() {
        let mut img = Image::new(Mode::Multicolor, 2, 2);
        for (i, c) in [2u8, 5, 7].iter().enumerate() {
            assert_eq!(img.paint_auto(i as i32, 0, *c, false, &PEPTO), PaintOutcome::Exact);
        }
        // fourth non-background colour does not fit
        assert_eq!(img.paint_auto(3, 0, 1, false, &PEPTO), PaintOutcome::Nearest);
        assert!(cell_colors(&img, 0, 0).len() <= 4);
        // other cell unaffected
        assert_eq!(cell_colors(&img, 1, 0), vec![0]);
    }

    #[test]
    fn force_recolours_least_used_slot() {
        let mut img = Image::new(Mode::Multicolor, 1, 1);
        img.paint_auto(0, 0, 2, false, &PEPTO);
        img.paint_auto(1, 0, 5, false, &PEPTO);
        img.paint_auto(2, 0, 7, false, &PEPTO);
        assert_eq!(img.paint_auto(3, 0, 1, true, &PEPTO), PaintOutcome::Forced);
        assert_eq!(img.color_at(3, 0), Some(1));
    }

    #[test]
    fn repainting_own_pixel_frees_its_slot() {
        let mut img = Image::new(Mode::Hires, 1, 1);
        img.paint_auto(0, 0, 2, false, &PEPTO);
        img.paint_auto(0, 0, 5, false, &PEPTO); // only pixel using slot -> recolour fine
        assert_eq!(img.color_at(0, 0), Some(5));
    }

    #[test]
    fn hardware_slot_write_recolours_shared_pixels() {
        let mut img = Image::new(Mode::Multicolor, 1, 1);
        img.paint_hardware(0, 0, 3, 4);
        img.paint_hardware(1, 0, 3, 6);
        assert_eq!(img.color_at(0, 0), Some(6));
    }

    #[test]
    fn multicolor_background_is_global() {
        let mut img = Image::new(Mode::Multicolor, 2, 1);
        img.paint_hardware(0, 0, 0, 6);
        assert_eq!(img.color_at(7, 7), Some(6));
    }

    #[test]
    fn cell_usage_counts_pixels_per_code() {
        let mut img = Image::new(Mode::Multicolor, 2, 1);
        img.paint_auto(0, 0, 2, false, &PEPTO);
        img.paint_auto(1, 0, 2, false, &PEPTO);
        let u = img.cell_code_usage(0, 0);
        assert_eq!(u.iter().sum::<u32>(), 32);
        assert_eq!(u[0], 30);
        assert_eq!(img.cell_code_usage(1, 0)[0], 32);
    }

    #[test]
    fn flood_region() {
        let img = Image::new(Mode::Hires, 2, 1);
        assert_eq!(img.region_at(0, 0).len(), 16 * 8);
    }
}
