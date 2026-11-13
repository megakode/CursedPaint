//! C64 graphics core: palettes, bitmap modes with hardware colour-slot rules,
//! drawing primitives and file formats. No GUI dependencies.

pub mod brush;
pub mod dither;
pub mod image;
pub mod io;
pub mod palette;
pub mod shapes;

pub use dither::Dither;
pub use image::{Image, Mode, PaintOutcome};
pub use palette::{Palette, SortMode, PALETTES};
