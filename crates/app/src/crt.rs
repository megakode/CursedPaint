//! CRT preview: runs a RetroArch slang shader preset (`.slangp`) over the picture using
//! librashader's OpenGL runtime, and hands the result to egui as a native texture.
//!
//! librashader uses glow 0.17 while eframe uses glow 0.16, so we make a second glow context
//! from the same GL loader. Both talk to the same underlying OpenGL context, so GL texture
//! names are shared between them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::glow as glow16;
use eframe::egui;
use glow17::HasContext;
use librashader::presets::ShaderFeatures;
use librashader::runtime::gl::{FilterChain, FilterChainOptions, GLImage};
use librashader::runtime::{Size, Viewport};

/// Size of the shaded output (4:3, like a PAL/NTSC screen). egui scales it to the window.
pub const OUT_W: u32 = 1280;
pub const OUT_H: u32 = 960;

pub struct CrtPreview {
    gl: Arc<glow17::Context>,
    chain: Option<FilterChain>,
    input: Option<glow17::Texture>,
    output: Option<glow17::Texture>,
    tex_id: Option<egui::TextureId>,
    frames: usize,
    /// Preset to load on the next render (set when the user picks one).
    pending_preset: Option<PathBuf>,
    pub preset: Option<PathBuf>,
    pub error: Option<String>,
}

impl CrtPreview {
    pub fn new(gl: glow17::Context) -> Self {
        let gl = Arc::new(gl);
        CrtPreview { gl, chain: None, input: None, output: None, tex_id: None, frames: 0, pending_preset: None, preset: None, error: None }
    }

    pub fn set_preset(&mut self, path: PathBuf) {
        self.pending_preset = Some(path.clone());
        self.preset = Some(path);
        self.error = None;
    }

    pub fn has_chain(&self) -> bool {
        self.chain.is_some()
    }

    pub fn texture_id(&self) -> Option<egui::TextureId> {
        self.tex_id
    }

    pub fn needs_load(&self) -> bool {
        self.pending_preset.is_some()
    }

    fn load_chain(&mut self, path: &Path) {
        let opts = FilterChainOptions { glsl_version: 0, use_dsa: false, force_no_mipmaps: false, disable_cache: true };
        // SAFETY: the GL context is current (called from `App::update`).
        let r = unsafe { FilterChain::load_from_path(path, ShaderFeatures::NONE, self.gl.clone(), Some(&opts)) };
        match r {
            Ok(c) => {
                self.chain = Some(c);
                self.error = None;
            }
            Err(e) => {
                self.chain = None;
                self.error = Some(e.to_string());
            }
        }
    }

    /// Run the preset over an RGBA image. Must be called with the GL context current.
    pub fn render(&mut self, frame: &mut eframe::Frame, w: usize, h: usize, rgba: &[u8]) {
        if let Some(p) = self.pending_preset.take() {
            self.load_chain(&p);
        }
        if self.chain.is_none() {
            return;
        }
        let gl = self.gl.clone();
        unsafe {
            // Output texture, created once and registered with egui.
            if self.output.is_none() {
                let t = gl.create_texture().expect("texture");
                gl.bind_texture(glow17::TEXTURE_2D, Some(t));
                gl.tex_image_2d(glow17::TEXTURE_2D, 0, glow17::RGBA8 as i32, OUT_W as i32, OUT_H as i32, 0, glow17::RGBA, glow17::UNSIGNED_BYTE, glow17::PixelUnpackData::Slice(None));
                for (p, v) in [
                    (glow17::TEXTURE_MIN_FILTER, glow17::LINEAR),
                    (glow17::TEXTURE_MAG_FILTER, glow17::LINEAR),
                    (glow17::TEXTURE_WRAP_S, glow17::CLAMP_TO_EDGE),
                    (glow17::TEXTURE_WRAP_T, glow17::CLAMP_TO_EDGE),
                ] {
                    gl.tex_parameter_i32(glow17::TEXTURE_2D, p, v as i32);
                }
                self.output = Some(t);
                // egui_glow takes ownership of the texture name; same GL context, so same name.
                self.tex_id = Some(frame.register_native_glow_texture(glow16::NativeTexture(t.0)));
            }
            let input = *self.input.get_or_insert_with(|| gl.create_texture().expect("texture"));
            gl.bind_texture(glow17::TEXTURE_2D, Some(input));
            gl.pixel_store_i32(glow17::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(glow17::TEXTURE_2D, 0, glow17::RGBA8 as i32, w as i32, h as i32, 0, glow17::RGBA, glow17::UNSIGNED_BYTE, glow17::PixelUnpackData::Slice(Some(rgba)));
            // librashader's samplers use mipmapped min filters, so a texture without a mip
            // chain is "incomplete" and samples as black.
            gl.generate_mipmap(glow17::TEXTURE_2D);
            for (p, v) in [
                (glow17::TEXTURE_MIN_FILTER, glow17::NEAREST_MIPMAP_NEAREST),
                (glow17::TEXTURE_MAG_FILTER, glow17::NEAREST),
                (glow17::TEXTURE_WRAP_S, glow17::CLAMP_TO_EDGE),
                (glow17::TEXTURE_WRAP_T, glow17::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow17::TEXTURE_2D, p, v as i32);
            }
            // eframe/egui leave GL state around; put back what an offscreen render expects.
            gl.disable(glow17::SCISSOR_TEST);
            gl.disable(glow17::BLEND);
            gl.disable(glow17::CULL_FACE);
            gl.disable(glow17::DEPTH_TEST);
            gl.disable(glow17::STENCIL_TEST);
            gl.color_mask(true, true, true, true);
            gl.active_texture(glow17::TEXTURE0);
            let in_img = GLImage { handle: Some(input), format: glow17::RGBA8, size: Size { width: w as u32, height: h as u32 } };
            let out_img = GLImage { handle: self.output, format: glow17::RGBA8, size: Size { width: OUT_W, height: OUT_H } };
            let viewport = Viewport { x: 0.0, y: 0.0, output: &out_img, mvp: None, size: out_img.size };
            let res = self.chain.as_mut().unwrap().frame(&in_img, &viewport, self.frames, None);
            self.frames += 1;
            if let Err(e) = res {
                self.error = Some(e.to_string());
                self.chain = None;
            }
        }
    }
}
