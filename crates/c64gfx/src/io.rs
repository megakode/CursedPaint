//! File formats: Koala Painter, Art Studio, native project, KickAssembler ASM.

use crate::image::{Image, Mode};

#[derive(Debug)]
pub enum IoError {
    BadSize(usize),
    BadMagic,
    Unsupported(&'static str),
}

impl std::fmt::Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IoError::BadSize(n) => write!(f, "unexpected file size ({n} bytes)"),
            IoError::BadMagic => write!(f, "not a CursedPaint project file"),
            IoError::Unsupported(s) => write!(f, "{s}"),
        }
    }
}
impl std::error::Error for IoError {}

const KOALA_LOAD: u16 = 0x6000;
const ART_LOAD: u16 = 0x2000;

fn need_full_screen(img: &Image, what: &'static str) -> Result<(), IoError> {
    if img.cells_w != 40 || img.cells_h != 25 {
        return Err(IoError::Unsupported(what));
    }
    Ok(())
}

/// Koala Painter: load addr $6000, 8000 bitmap, 1000 screen, 1000 colour, 1 background.
pub fn save_koala(img: &Image) -> Result<Vec<u8>, IoError> {
    if img.mode != Mode::Multicolor {
        return Err(IoError::Unsupported("Koala export needs a multicolor image"));
    }
    need_full_screen(img, "Koala export needs a 40x25 image")?;
    let mut out = Vec::with_capacity(10003);
    out.extend_from_slice(&KOALA_LOAD.to_le_bytes());
    out.extend_from_slice(&img.bitmap);
    out.extend_from_slice(&img.screen);
    out.extend(img.color.iter().map(|c| c & 15));
    out.push(img.background);
    Ok(out)
}

pub fn load_koala(data: &[u8]) -> Result<Image, IoError> {
    if data.len() < 10002 || data.len() > 10003 {
        return Err(IoError::BadSize(data.len()));
    }
    let d = &data[2..];
    let mut img = Image::new(Mode::Multicolor, 40, 25);
    img.bitmap.copy_from_slice(&d[..8000]);
    img.screen.copy_from_slice(&d[8000..9000]);
    img.color.copy_from_slice(&d[9000..10000]);
    img.background = d.get(10000).copied().unwrap_or(0) & 15;
    Ok(img)
}

/// Art Studio (hires): load addr $2000, 8000 bitmap, 1000 screen, padded to 9009 bytes.
pub fn save_art_studio(img: &Image) -> Result<Vec<u8>, IoError> {
    if img.mode != Mode::Hires {
        return Err(IoError::Unsupported("Art Studio export needs a hires image"));
    }
    need_full_screen(img, "Art Studio export needs a 40x25 image")?;
    let mut out = Vec::with_capacity(9009);
    out.extend_from_slice(&ART_LOAD.to_le_bytes());
    out.extend_from_slice(&img.bitmap);
    out.extend_from_slice(&img.screen);
    out.resize(9009, 0);
    Ok(out)
}

pub fn load_art_studio(data: &[u8]) -> Result<Image, IoError> {
    if data.len() < 9002 || data.len() > 9009 {
        return Err(IoError::BadSize(data.len()));
    }
    let d = &data[2..];
    let mut img = Image::new(Mode::Hires, 40, 25);
    img.bitmap.copy_from_slice(&d[..8000]);
    img.screen.copy_from_slice(&d[8000..9000]);
    Ok(img)
}

/// Native project: "RPNT", version, mode, cells_w, cells_h, background, bitmap, screen, colour.
pub fn save_project(img: &Image) -> Vec<u8> {
    let mut out = b"RPNT".to_vec();
    out.push(1);
    out.push(match img.mode {
        Mode::Hires => 0,
        Mode::Multicolor => 1,
    });
    out.push(img.cells_w as u8);
    out.push(img.cells_h as u8);
    out.push(img.background);
    out.extend_from_slice(&img.bitmap);
    out.extend_from_slice(&img.screen);
    out.extend_from_slice(&img.color);
    out
}

pub fn load_project(data: &[u8]) -> Result<Image, IoError> {
    if data.len() < 9 || &data[..4] != b"RPNT" || data[4] != 1 {
        return Err(IoError::BadMagic);
    }
    let mode = match data[5] {
        0 => Mode::Hires,
        1 => Mode::Multicolor,
        _ => return Err(IoError::BadMagic),
    };
    let (w, h) = (data[6] as usize, data[7] as usize);
    if w == 0 || h == 0 {
        return Err(IoError::BadMagic);
    }
    let n = w * h;
    if data.len() != 9 + n * 10 {
        return Err(IoError::BadSize(data.len()));
    }
    let mut img = Image::new(mode, w, h);
    img.background = data[8] & 15;
    let d = &data[9..];
    img.bitmap.copy_from_slice(&d[..n * 8]);
    img.screen.copy_from_slice(&d[n * 8..n * 9]);
    img.color.copy_from_slice(&d[n * 9..n * 10]);
    Ok(img)
}

/// KickAssembler source with labelled data blocks.
pub fn export_kickass(img: &Image, name: &str) -> String {
    fn block(label: &str, data: &[u8]) -> String {
        let mut s = format!("{label}:\n");
        for chunk in data.chunks(16) {
            let bytes: Vec<String> = chunk.iter().map(|b| format!("${b:02x}")).collect();
            s.push_str(&format!("    .byte {}\n", bytes.join(",")));
        }
        s
    }
    let mode = match img.mode {
        Mode::Hires => "hires",
        Mode::Multicolor => "multicolor",
    };
    let mut s = format!(
        "// {name}: {mode} bitmap, {}x{} cells\n.var backgroundColor = {}\n\n",
        img.cells_w, img.cells_h, img.background
    );
    s.push_str(&block(&format!("{name}_bitmap"), &img.bitmap));
    s.push_str(&block(&format!("{name}_screen"), &img.screen));
    if img.mode == Mode::Multicolor {
        let col: Vec<u8> = img.color.iter().map(|c| c & 15).collect();
        s.push_str(&block(&format!("{name}_color"), &col));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::PEPTO;

    fn painted(mode: Mode, w: usize, h: usize) -> Image {
        let mut img = Image::new(mode, w, h);
        for i in 0..20 {
            img.paint_auto(i, i % 8, (i % 7 + 1) as u8, true, &PEPTO);
        }
        img.background = 6;
        img
    }

    #[test]
    fn koala_roundtrip() {
        let img = painted(Mode::Multicolor, 40, 25);
        let bytes = save_koala(&img).unwrap();
        assert_eq!(bytes.len(), 10003);
        assert_eq!(&bytes[..2], &[0x00, 0x60]);
        assert_eq!(load_koala(&bytes).unwrap(), img);
    }

    #[test]
    fn art_studio_roundtrip() {
        let img = painted(Mode::Hires, 40, 25);
        let bytes = save_art_studio(&img).unwrap();
        assert_eq!(bytes.len(), 9009);
        let mut expect = img.clone();
        expect.background = 0; // Art Studio has no background byte
        assert_eq!(load_art_studio(&bytes).unwrap(), expect);
    }

    #[test]
    fn project_roundtrip_any_size() {
        let img = painted(Mode::Multicolor, 3, 2);
        assert_eq!(load_project(&save_project(&img)).unwrap(), img);
    }

    #[test]
    fn export_requires_full_screen() {
        assert!(save_koala(&painted(Mode::Multicolor, 3, 2)).is_err());
        assert!(save_koala(&painted(Mode::Hires, 40, 25)).is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(load_koala(&[0; 10]).is_err());
        assert!(load_project(b"nope").is_err());
    }

    #[test]
    fn kickass_has_blocks() {
        let s = export_kickass(&painted(Mode::Multicolor, 40, 25), "pic");
        assert!(s.contains("pic_bitmap:") && s.contains("pic_color:"));
    }
}
