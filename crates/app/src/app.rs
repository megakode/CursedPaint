use crate::icons::{self, Icon, ACCENT};
use c64gfx::palette::COLOR_NAMES;
use c64gfx::{brush, io, shapes, Dither, Image, Mode, PaintOutcome, Palette, SortMode, PALETTES};
use eframe::egui::{
    self, Color32, Key, Modifiers, PointerButton, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2,
};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Pencil,
    Line,
    Rect,
    RectFill,
    Ellipse,
    EllipseFill,
    Fill,
    Picker,
}

const TOOLS: [(Tool, Icon, &str, &str, Key); 8] = [
    (Tool::Pencil, Icon::Pencil, "Pencil", "B", Key::B),
    (Tool::Line, Icon::Line, "Line", "L", Key::L),
    (Tool::Rect, Icon::Rect, "Rectangle", "R", Key::R),
    (Tool::RectFill, Icon::RectFill, "Filled rectangle", "Shift+R", Key::T),
    (Tool::Ellipse, Icon::Ellipse, "Ellipse", "E", Key::E),
    (Tool::EllipseFill, Icon::EllipseFill, "Filled ellipse", "Shift+E", Key::Y),
    (Tool::Fill, Icon::Fill, "Fill", "G", Key::G),
    (Tool::Picker, Icon::Picker, "Colour picker", "I", Key::I),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorMode {
    /// Paint the colour you see; the editor finds a legal slot.
    Automatic,
    /// Write straight to a chosen hardware colour slot.
    Hardware,
}

struct Drag {
    button: PointerButton,
    start: (i32, i32),
    last: (i32, i32),
    before: Image,
    nearest: u32,
    forced: u32,
}

struct NewDialog {
    mode: Mode,
    w: usize,
    h: usize,
}

pub struct App {
    img: Image,
    undo: Vec<Image>,
    redo: Vec<Image>,
    sort: SortMode,
    tool: Tool,
    color_mode: ColorMode,
    force: bool,
    fg: u8,
    bg: u8,
    hw_code: u8,
    dither: Dither,
    brush: usize,
    zoom: f32,
    pan: Vec2,
    fit_next: bool,
    tex: Option<TextureHandle>,
    dirty: bool,
    cell_grid: bool,
    pixel_grid: bool,
    drag: Option<Drag>,
    hover_cell: Option<(usize, usize)>,
    status: String,
    new_dialog: Option<NewDialog>,
    path: Option<PathBuf>,
}

impl App {
    pub fn new(ctx: &egui::Context) -> Self {
        apply_theme(ctx);
        App {
            img: Image::new(Mode::Multicolor, 40, 25),
            undo: vec![],
            redo: vec![],
            sort: SortMode::C64,
            tool: Tool::Pencil,
            color_mode: ColorMode::Automatic,
            force: false,
            fg: 1,
            bg: 0,
            hw_code: 1,
            dither: Dither::default(),
            brush: 0,
            zoom: 3.0,
            pan: Vec2::ZERO,
            fit_next: true,
            tex: None,
            dirty: true,
            cell_grid: false,
            pixel_grid: false,
            drag: None,
            hover_cell: Some((0, 0)),
            status: "Ready".into(),
            new_dialog: None,
            path: None,
        }
    }

    /// The fixed palette (Pepto). Sorting only changes display order, never the colours.
    fn pal(&self) -> Palette {
        PALETTES[0]
    }

    /// Colour numbers in the order they are displayed.
    fn display_order(&self) -> [u8; 16] {
        self.sort.order(&self.pal())
    }

    fn set_image(&mut self, img: Image, path: Option<PathBuf>) {
        self.img = img;
        self.undo.clear();
        self.redo.clear();
        self.path = path;
        self.hw_code = 1;
        self.dirty = true;
        self.fit_next = true;
    }

    /// Select the previous (-1) or next (+1) palette colour, wrapping around.
    fn step_color(&mut self, delta: i32) {
        let order = self.display_order();
        let pos = order.iter().position(|&c| c == self.fg).unwrap_or(0) as i32;
        self.fg = order[(pos + delta).rem_euclid(16) as usize];
    }

    /// Select the previous (-1) or next (+1) preset brush, wrapping around.
    fn step_brush(&mut self, delta: i32) {
        self.brush = (self.brush as i32 + delta).rem_euclid(brush::COUNT as i32) as usize;
    }

    fn do_undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.img, prev));
            self.dirty = true;
        }
    }

    fn do_redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.img, next));
            self.dirty = true;
        }
    }

    fn put(&mut self, x: i32, y: i32, secondary: bool) -> PaintOutcome {
        match self.color_mode {
            ColorMode::Automatic => {
                let c = if secondary { self.bg } else { self.fg };
                let pal = self.pal();
                self.img.paint_auto(x, y, c, self.force, &pal)
            }
            ColorMode::Hardware => {
                if secondary {
                    self.img.set_code(x, y, 0);
                } else {
                    self.img.paint_hardware(x, y, self.hw_code, self.fg);
                }
                PaintOutcome::Exact
            }
        }
    }

    /// Pattern phase for the stroke: canvas origin, or the stroke's first pixel when anchored.
    fn dither_origin(&self, start: (i32, i32)) -> (i32, i32) {
        if self.dither.anchored { start } else { (0, 0) }
    }

    fn put_all(&mut self, pts: &[(i32, i32)], secondary: bool, d: &mut Drag) {
        let origin = self.dither_origin(d.start);
        let mode = self.img.mode;
        // Brushes stamp around every point of pencil and outline tools; fills and
        // filled shapes keep their exact area.
        let stamped;
        let pts: &[(i32, i32)] = if self.brush > 0 && matches!(self.tool, Tool::Pencil | Tool::Line | Tool::Rect | Tool::Ellipse) {
            let offsets = brush::offsets(mode, self.brush);
            let mut seen = std::collections::HashSet::new();
            let mut out = Vec::with_capacity(pts.len() * offsets.len());
            for &(x, y) in pts {
                for &(dx, dy) in &offsets {
                    let p = (x + dx, y + dy);
                    if seen.insert(p) {
                        out.push(p);
                    }
                }
            }
            stamped = out;
            &stamped
        } else {
            pts
        };
        for &(x, y) in pts {
            if !self.dither.allows(mode, x, y, origin) {
                continue;
            }
            match self.put(x, y, secondary) {
                PaintOutcome::Nearest => d.nearest += 1,
                PaintOutcome::Forced => d.forced += 1,
                _ => {}
            }
        }
        self.dirty = true;
    }

    fn finish(&mut self, d: Drag) {
        if self.img != d.before {
            self.undo.push(d.before);
            self.redo.clear();
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
        }
        self.status = if d.nearest > 0 {
            format!("{} pixel(s) could not fit the cell's colour limit; nearest colour used", d.nearest)
        } else if d.forced > 0 {
            format!("{} pixel(s) forced: other pixels in the cell changed colour", d.forced)
        } else {
            "Ready".into()
        };
    }

    fn shape_points(&self, a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
        match self.tool {
            Tool::Line => shapes::line(a, b),
            Tool::Rect => shapes::rect(a, b, false),
            Tool::RectFill => shapes::rect(a, b, true),
            Tool::Ellipse => shapes::ellipse(a, b, false),
            Tool::EllipseFill => shapes::ellipse(a, b, true),
            _ => vec![],
        }
    }

    fn is_shape_tool(&self) -> bool {
        matches!(self.tool, Tool::Line | Tool::Rect | Tool::RectFill | Tool::Ellipse | Tool::EllipseFill)
    }

    // ---- file operations ----

    fn open_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("C64 / CursedPaint", &["rpnt", "koa", "kla", "art", "aas", "ocp"])
            .pick_file()
        else {
            return;
        };
        let result = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|data| {
            let ext = ext_of(&path);
            match ext.as_str() {
                "rpnt" => io::load_project(&data),
                "koa" | "kla" => io::load_koala(&data),
                "art" | "aas" | "ocp" => io::load_art_studio(&data),
                _ => Err(io::IoError::Unsupported("unknown file type")),
            }
            .map_err(|e| e.to_string())
        });
        match result {
            Ok(img) => {
                let is_project = ext_of(&path) == "rpnt";
                self.status = format!("Opened {}", path.display());
                self.set_image(img, if is_project { Some(path) } else { None });
            }
            Err(e) => self.status = format!("Open failed: {e}"),
        }
    }

    fn save_project_as(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("CursedPaint project", &["rpnt"]).set_file_name("untitled.rpnt").save_file()
        else {
            return;
        };
        self.write(path.with_extension("rpnt"), io::save_project(&self.img), true);
    }

    fn save_project(&mut self) {
        match self.path.clone() {
            Some(p) => self.write(p, io::save_project(&self.img), true),
            None => self.save_project_as(),
        }
    }

    fn export(&mut self, kind: &str) {
        let (filter, ext): (&str, &str) = match kind {
            "koala" => ("Koala Painter", "koa"),
            "art" => ("Art Studio", "art"),
            _ => ("KickAssembler source", "asm"),
        };
        let result: Result<Vec<u8>, String> = match kind {
            "koala" => io::save_koala(&self.img).map_err(|e| e.to_string()),
            "art" => io::save_art_studio(&self.img).map_err(|e| e.to_string()),
            _ => Ok(io::export_kickass(&self.img, "picture").into_bytes()),
        };
        match result {
            Err(e) => self.status = format!("Export failed: {e}"),
            Ok(bytes) => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(filter, &[ext])
                    .set_file_name(format!("picture.{ext}"))
                    .save_file()
                {
                    self.write(path, bytes, false);
                }
            }
        }
    }

    fn write(&mut self, path: PathBuf, bytes: Vec<u8>, is_project: bool) {
        match std::fs::write(&path, bytes) {
            Ok(()) => {
                self.status = format!("Saved {}", path.display());
                if is_project {
                    self.path = Some(path);
                }
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    // ---- UI pieces ----

    fn menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New…").clicked() {
                    self.new_dialog = Some(NewDialog { mode: Mode::Multicolor, w: 40, h: 25 });
                    ui.close();
                }
                if ui.button("Open…").clicked() {
                    ui.close();
                    self.open_file();
                }
                if ui.button("Save project").clicked() {
                    ui.close();
                    self.save_project();
                }
                if ui.button("Save project as…").clicked() {
                    ui.close();
                    self.save_project_as();
                }
                ui.separator();
                ui.label("Export");
                if ui.add_enabled(self.img.mode == Mode::Multicolor, egui::Button::new("Koala Painter (40x25)")).clicked() {
                    ui.close();
                    self.export("koala");
                }
                if ui.add_enabled(self.img.mode == Mode::Hires, egui::Button::new("Art Studio (40x25)")).clicked() {
                    ui.close();
                    self.export("art");
                }
                if ui.button("KickAssembler source").clicked() {
                    ui.close();
                    self.export("asm");
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Undo")).clicked() {
                    self.do_undo();
                    ui.close();
                }
                if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Redo")).clicked() {
                    self.do_redo();
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.cell_grid, "Cell grid");
                ui.checkbox(&mut self.pixel_grid, "Pixel grid");
                if ui.button("Fit to window").clicked() {
                    self.fit_next = true;
                    ui.close();
                }
            });
        });
    }

    /// Fixed-position indicator of the hovered cell's colour slots. It always
    /// occupies four slots at the far left of the status bar, so it never moves
    /// when the text next to it changes length.
    fn cell_colors_indicator(&self, ui: &mut egui::Ui) {
        const SLOTS: u8 = 4;
        const SIZE: f32 = 16.0;
        let pal = self.pal();
        let (names, slots): (&[&str], u8) = match self.img.mode {
            Mode::Hires => (&["background (screen low)", "foreground (screen high)", "", ""], 2),
            Mode::Multicolor => (&["global background", "screen high", "screen low", "colour RAM"], 4),
        };
        let info = self.hover_cell.filter(|&(cx, cy)| cx < self.img.cells_w && cy < self.img.cells_h).map(|(cx, cy)| {
            (self.img.cell_code_usage(cx, cy), cy * self.img.cells_w + cx)
        });
        ui.spacing_mut().item_spacing.x = 4.0;
        for code in 0..SLOTS {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(SIZE), Sense::hover());
            let p = ui.painter();
            let outline = egui::Stroke::new(1.0_f32, Color32::from_gray(80));
            match info {
                Some((usage, cell)) if code < slots => {
                    let col = self.img.slot_color(cell, code);
                    let [r, g, b] = pal.rgb[col as usize];
                    // slot 0 of multicolor is global, so always "in use"
                    let used = usage[code as usize] > 0 || (self.img.mode == Mode::Multicolor && code == 0);
                    let fill = if used { Color32::from_rgb(r, g, b) } else { Color32::from_rgb(r, g, b).gamma_multiply(0.35) };
                    p.rect_filled(rect, 3.0, fill);
                    p.rect_stroke(rect, 3.0, outline, egui::StrokeKind::Inside);
                    if !used {
                        p.line_segment([rect.left_bottom() + Vec2::new(2.0, -2.0), rect.right_top() + Vec2::new(-2.0, 2.0)], egui::Stroke::new(1.0_f32, Color32::from_gray(150)));
                    }
                    resp.on_hover_text(format!(
                        "Slot {code} ({}): {} — {}",
                        names[code as usize],
                        COLOR_NAMES[col as usize],
                        if used { format!("{} px", usage[code as usize]) } else { "unused (free)".to_string() }
                    ));
                }
                _ => {
                    p.rect_stroke(rect, 3.0, outline, egui::StrokeKind::Inside);
                }
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        const S: f32 = 34.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            if icons::button(ui, Icon::New, false, true, S, "New image…").clicked() {
                self.new_dialog = Some(NewDialog { mode: Mode::Multicolor, w: 40, h: 25 });
            }
            if icons::button(ui, Icon::Open, false, true, S, "Open… (Cmd+O)").clicked() {
                self.open_file();
            }
            if icons::button(ui, Icon::Save, false, true, S, "Save project (Cmd+S)").clicked() {
                self.save_project();
            }
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);
            if icons::button(ui, Icon::Undo, false, !self.undo.is_empty(), S, "Undo (Cmd+Z)").clicked() {
                self.do_undo();
            }
            if icons::button(ui, Icon::Redo, false, !self.redo.is_empty(), S, "Redo (Cmd+Shift+Z)").clicked() {
                self.do_redo();
            }
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);
            if icons::button(ui, Icon::CellGrid, self.cell_grid, true, S, "Cell grid").clicked() {
                self.cell_grid = !self.cell_grid;
            }
            if icons::button(ui, Icon::PixelGrid, self.pixel_grid, true, S, "Pixel grid (when zoomed in)").clicked() {
                self.pixel_grid = !self.pixel_grid;
            }
            if icons::button(ui, Icon::Fit, false, true, S, "Fit to window (F)").clicked() {
                self.fit_next = true;
            }
        });
    }

    fn brush_section(&mut self, ui: &mut egui::Ui) {
        section(ui, "BRUSHES");
        let mode = self.img.mode;
        egui::Grid::new("brush_grid").spacing([4.0, 4.0]).show(ui, |ui| {
            for i in 0..brush::COUNT {
                let selected = self.brush == i;
                let cell = |x: usize, y: usize| brush::cell(mode, i, x, y);
                if mask_button(ui, &cell, brush::width(mode), selected, &format!("Brush {}", i + 1)).clicked() {
                    self.brush = i;
                }
                if i % 4 == 3 {
                    ui.end_row();
                }
            }
        });
        ui.small("- = previous / next brush");
    }

    fn dither_section(&mut self, ui: &mut egui::Ui) {
        section(ui, "DITHER");
        egui::Grid::new("dither_toggles").spacing([4.0, 4.0]).show(ui, |ui| {
            let w = 78.0;
            let toggle = |ui: &mut egui::Ui, on: &mut bool, off_label: &str, on_label: &str, tip: &str| {
                let label = if *on { on_label } else { off_label };
                let b = ui.add_sized([w, 24.0], egui::Button::new(label).selected(*on));
                if b.on_hover_text(tip).clicked() {
                    *on = !*on;
                }
            };
            toggle(ui, &mut self.dither.invert, "Orig.", "Invert", "Invert the pattern");
            toggle(ui, &mut self.dither.anchored, "Global", "Anchor", "Anchor the pattern to the first click of a stroke");
            ui.end_row();
            toggle(ui, &mut self.dither.flip_x, "FlipX", "FlipX", "Flip the pattern horizontally");
            toggle(ui, &mut self.dither.flip_y, "FlipY", "FlipY", "Flip the pattern vertically");
            ui.end_row();
        });
        ui.add_space(4.0);
        let mode = self.img.mode;
        egui::Grid::new("dither_grid").spacing([4.0, 4.0]).show(ui, |ui| {
            for i in 0..c64gfx::dither::COUNT {
                let shown = Dither { index: i, ..self.dither };
                let selected = self.dither.index == i;
                let cell = |x: usize, y: usize| shown.cell(mode, x, y);
                if mask_button(ui, &cell, Dither::width(mode), selected, &format!("Dither {}", i + 1)).clicked() {
                    self.dither.index = i;
                }
                if i % 4 == 3 {
                    ui.end_row();
                }
            }
        });
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        section(ui, "TOOLS");
        egui::Grid::new("tools").spacing([4.0, 4.0]).show(ui, |ui| {
            for (i, (tool, icon, name, key, _)) in TOOLS.into_iter().enumerate() {
                if icons::button(ui, icon, self.tool == tool, true, 40.0, &format!("{name} ({key})")).clicked() {
                    self.tool = tool;
                }
                if i % 4 == 3 {
                    ui.end_row();
                }
            }
        });
        if let Some((_, _, name, key, _)) = TOOLS.iter().find(|t| t.0 == self.tool) {
            ui.add_space(2.0);
            ui.label(egui::RichText::new(format!("{name}  ·  {key}")).weak().small());
        }
        self.brush_section(ui);
        section(ui, "COLOUR MODE");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.color_mode, ColorMode::Automatic, "Automatic")
                .on_hover_text("Paint the colour you want; the editor picks a legal hardware slot.");
            ui.selectable_value(&mut self.color_mode, ColorMode::Hardware, "Hardware")
                .on_hover_text("Write directly to a chosen colour slot of the cell.");
        });
        match self.color_mode {
            ColorMode::Automatic => {
                ui.checkbox(&mut self.force, "Force placement")
                    .on_hover_text("If a cell is full, recolour its least-used slot instead of using the nearest colour.");
            }
            ColorMode::Hardware => {
                let names: &[&str] = match self.img.mode {
                    Mode::Hires => &["0 background", "1 foreground"],
                    Mode::Multicolor => &["0 global background", "1 screen high", "2 screen low", "3 colour RAM"],
                };
                for (i, n) in names.iter().enumerate() {
                    ui.radio_value(&mut self.hw_code, i as u8, *n);
                }
                ui.small("Right-click sets the pixel to code 0 only.");
            }
        }
        section(ui, "PALETTE");
        egui::ComboBox::from_id_salt("palette_sort")
            .selected_text(format!("Sort: {}", self.sort.label()))
            .show_ui(ui, |ui| {
                for m in SortMode::ALL {
                    ui.selectable_value(&mut self.sort, m, m.label()).on_hover_text(m.description());
                }
            })
            .response
            .on_hover_text("Changes only how the swatches are arranged, not the colours.");
        let pal = self.pal();
        let order = self.display_order();
        egui::Grid::new("swatches").spacing([4.0, 4.0]).show(ui, |ui| {
            for (slot, &i) in order.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
                let [r, g, b] = pal.rgb[i as usize];
                ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(r, g, b));
                if self.fg == i {
                    ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(2.5_f32, Color32::WHITE), egui::StrokeKind::Inside);
                    ui.painter().rect_stroke(rect.shrink(2.5), 1.0, egui::Stroke::new(1.0_f32, Color32::BLACK), egui::StrokeKind::Inside);
                }
                if self.bg == i {
                    ui.painter().circle_filled(rect.right_bottom() - Vec2::splat(6.0), 3.5, Color32::from_gray(if i == 1 { 0 } else { 255 }));
                }
                if resp.clicked() {
                    self.fg = i;
                }
                if resp.secondary_clicked() {
                    self.bg = i;
                }
                resp.on_hover_text(format!("{} ({i}) — left: paint, right: erase colour", COLOR_NAMES[i as usize]));
                if slot % 4 == 3 {
                    ui.end_row();
                }
            }
        });
        ui.small(format!("Paint: {}   Right-click: {}", COLOR_NAMES[self.fg as usize], COLOR_NAMES[self.bg as usize]));
        ui.small("[ ] previous / next colour   X swap");
        self.dither_section(ui);
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        if !self.dirty && self.tex.is_some() {
            return;
        }
        let (w, h, rgba) = self.img.to_rgba(&self.pal());
        let image = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
        match &mut self.tex {
            Some(t) => t.set(image, TextureOptions::NEAREST),
            None => self.tex = Some(ctx.load_texture("canvas", image, TextureOptions::NEAREST)),
        }
        self.dirty = false;
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let avail = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(avail, Sense::click_and_drag());
        let painter = ui.painter_at(avail);
        painter.rect_filled(avail, 0.0, Color32::from_gray(38));

        let ax = self.img.mode.aspect_x() as f32;
        let img_w = self.img.width() as f32 * ax;
        let img_h = self.img.height() as f32;

        if self.fit_next {
            self.zoom = ((avail.width() - 40.0) / img_w).min((avail.height() - 40.0) / img_h).max(0.25);
            if self.zoom > 1.0 {
                self.zoom = self.zoom.floor();
            }
            self.pan = (avail.size() - Vec2::new(img_w, img_h) * self.zoom) / 2.0;
            self.fit_next = false;
        }

        let hovered = resp.hovered();
        let ctx = ui.ctx().clone();

        // zoom around the cursor
        if hovered {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let old = self.zoom;
                self.zoom = (old * (scroll * 0.004).exp()).clamp(0.25, 64.0);
                if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
                    let rel = p - avail.min;
                    self.pan = rel - (rel - self.pan) * (self.zoom / old);
                }
            }
        }
        // pan: middle button or space + drag
        let space = ctx.input(|i| i.key_down(Key::Space));
        let panning = resp.dragged_by(PointerButton::Middle) || (space && resp.dragged_by(PointerButton::Primary));
        if panning {
            self.pan += ctx.input(|i| i.pointer.delta());
        }

        let origin = avail.min + self.pan;
        let px_w = self.zoom * ax; // screen size of one logical pixel
        let px_h = self.zoom;
        let to_pixel = |p: Pos2| -> (i32, i32) {
            (((p.x - origin.x) / px_w).floor() as i32, ((p.y - origin.y) / px_h).floor() as i32)
        };
        let img_rect = Rect::from_min_size(origin, Vec2::new(img_w, img_h) * self.zoom);

        self.update_texture(&ctx);
        if let Some(t) = &self.tex {
            painter.image(t.id(), img_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }

        // ---- drawing input ----
        let pointer = ctx.input(|i| i.pointer.hover_pos().or(i.pointer.interact_pos()));
        let cur = pointer.map(to_pixel);
        if !panning {
            for (button, secondary) in [(PointerButton::Primary, false), (PointerButton::Secondary, true)] {
                let pressed = ctx.input(|i| i.pointer.button_pressed(button));
                let down = ctx.input(|i| i.pointer.button_down(button));
                let released = ctx.input(|i| i.pointer.button_released(button));

                if pressed && hovered && self.drag.is_none() && !space {
                    if let Some(pos) = cur {
                        self.begin(button, secondary, pos);
                    }
                }
                let Some(mut d) = self.drag.take() else { continue };
                if d.button != button {
                    self.drag = Some(d);
                    continue;
                }
                if down {
                    if let Some(pos) = cur {
                        if self.tool == Tool::Pencil && pos != d.last {
                            let pts = shapes::line(d.last, pos);
                            self.put_all(&pts, secondary, &mut d);
                            d.last = pos;
                        } else if self.is_shape_tool() {
                            d.last = pos;
                        }
                    }
                }
                if released || !down {
                    if self.is_shape_tool() {
                        let pts = self.shape_points(d.start, d.last);
                        self.put_all(&pts, secondary, &mut d);
                    }
                    self.finish(d);
                } else {
                    self.drag = Some(d);
                }
            }
        }

        // ---- overlays ----
        if self.pixel_grid && px_w.min(px_h) >= 6.0 {
            let c = Color32::from_white_alpha(28);
            for x in 0..=self.img.width() {
                let sx = origin.x + x as f32 * px_w;
                painter.line_segment([Pos2::new(sx, img_rect.top()), Pos2::new(sx, img_rect.bottom())], egui::Stroke::new(1.0_f32, c));
            }
            for y in 0..=self.img.height() {
                let sy = origin.y + y as f32 * px_h;
                painter.line_segment([Pos2::new(img_rect.left(), sy), Pos2::new(img_rect.right(), sy)], egui::Stroke::new(1.0_f32, c));
            }
        }
        if self.cell_grid {
            let c = Color32::from_rgba_unmultiplied(255, 255, 0, 70);
            let cw = self.img.mode.cell_px_w() as f32 * px_w;
            for x in 0..=self.img.cells_w {
                let sx = origin.x + x as f32 * cw;
                painter.line_segment([Pos2::new(sx, img_rect.top()), Pos2::new(sx, img_rect.bottom())], egui::Stroke::new(1.0_f32, c));
            }
            for y in 0..=self.img.cells_h {
                let sy = origin.y + y as f32 * 8.0 * px_h;
                painter.line_segment([Pos2::new(img_rect.left(), sy), Pos2::new(img_rect.right(), sy)], egui::Stroke::new(1.0_f32, c));
            }
        }
        if let Some(d) = &self.drag {
            if self.is_shape_tool() {
                let [r, g, b] = self.pal().rgb[(if d.button == PointerButton::Secondary { self.bg } else { self.fg }) as usize];
                let col = Color32::from_rgba_unmultiplied(r, g, b, 170);
                let dither_org = self.dither_origin(d.start);
                for (x, y) in self.shape_points(d.start, d.last) {
                    if !self.dither.allows(self.img.mode, x, y, dither_org) {
                        continue;
                    }
                    let r = Rect::from_min_size(origin + Vec2::new(x as f32 * px_w, y as f32 * px_h), Vec2::new(px_w, px_h));
                    painter.rect_filled(r, 0.0, col);
                }
            }
        }
        // Floating brush: the pixels the next click would paint, in the active colour.
        let shape_drag = self.drag.is_some() && self.tool != Tool::Pencil;
        if hovered && !panning && !space && !shape_drag {
            if let Some(pos) = cur {
                let pts = self.hover_preview_points(pos);
                if !pts.is_empty() {
                    ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                    let [r, g, b] = self.pal().rgb[self.fg as usize];
                    let fill = if self.tool == Tool::Picker { Color32::TRANSPARENT } else { Color32::from_rgba_unmultiplied(r, g, b, 215) };
                    let edge = egui::Stroke::new(1.0_f32, Color32::from_black_alpha(150));
                    let rim = egui::Stroke::new(1.0_f32, Color32::from_white_alpha(170));
                    for &(x, y) in &pts {
                        let r = Rect::from_min_size(origin + Vec2::new(x as f32 * px_w, y as f32 * px_h), Vec2::new(px_w, px_h));
                        painter.rect_filled(r, 0.0, fill);
                        if px_w.min(px_h) >= 4.0 {
                            painter.rect_stroke(r, 0.0, edge, egui::StrokeKind::Inside);
                        }
                    }
                    // outline the whole footprint so it stays visible over same-coloured art
                    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
                    for &(x, y) in &pts {
                        x0 = x0.min(x);
                        y0 = y0.min(y);
                        x1 = x1.max(x);
                        y1 = y1.max(y);
                    }
                    let bb = Rect::from_min_max(
                        origin + Vec2::new(x0 as f32 * px_w, y0 as f32 * px_h),
                        origin + Vec2::new((x1 + 1) as f32 * px_w, (y1 + 1) as f32 * px_h),
                    );
                    painter.rect_stroke(bb.expand(1.0), 0.0, rim, egui::StrokeKind::Outside);
                }
            }
        }
        painter.rect_stroke(img_rect.expand(1.0), 0.0, egui::Stroke::new(1.0_f32, Color32::from_gray(90)), egui::StrokeKind::Outside);

        // hover info
        if hovered {
            if let Some((x, y)) = cur {
                if let (Some(code), Some(col)) = (self.img.code_at(x, y), self.img.color_at(x, y)) {
                    let pw = self.img.mode.cell_px_w() as i32;
                    let (cx, cy) = ((x / pw) as usize, (y / 8) as usize);
                    self.hover_cell = Some((cx, cy));
                    let n = self.img.distinct_colors_in_cell(cx, cy);
                    ctx.data_mut(|d| {
                        d.insert_temp(
                            egui::Id::new("hover"),
                            format!("x {x}, y {y}  cell {cx},{cy}  {} (code {code})  {n} colour(s) in cell", COLOR_NAMES[col as usize]),
                        )
                    });
                } else {
                    ctx.data_mut(|d| d.remove::<String>(egui::Id::new("hover")));
                }
            }
        }
    }

    /// Pixels a click at `pos` would paint right now (brush footprint, minus dithered-out
    /// cells, clipped to the image). Used for the floating brush preview.
    fn hover_preview_points(&self, pos: (i32, i32)) -> Vec<(i32, i32)> {
        let mode = self.img.mode;
        let (w, h) = (self.img.width() as i32, self.img.height() as i32);
        let stamped = self.brush > 0 && matches!(self.tool, Tool::Pencil | Tool::Line | Tool::Rect | Tool::Ellipse);
        let offsets = if stamped { brush::offsets(mode, self.brush) } else { vec![(0, 0)] };
        let origin = self.dither_origin(pos);
        offsets
            .into_iter()
            .map(|(dx, dy)| (pos.0 + dx, pos.1 + dy))
            .filter(|&(x, y)| x >= 0 && y >= 0 && x < w && y < h)
            .filter(|&(x, y)| self.tool == Tool::Picker || self.dither.allows(mode, x, y, origin))
            .collect()
    }

    fn begin(&mut self, button: PointerButton, secondary: bool, pos: (i32, i32)) {
        let mut d = Drag { button, start: pos, last: pos, before: self.img.clone(), nearest: 0, forced: 0 };
        match self.tool {
            Tool::Pencil => {
                self.put_all(&[pos], secondary, &mut d);
                self.drag = Some(d);
            }
            Tool::Fill => {
                let pts = self.img.region_at(pos.0, pos.1);
                self.put_all(&pts, secondary, &mut d);
                self.finish(d);
            }
            Tool::Picker => {
                if let Some(c) = self.img.color_at(pos.0, pos.1) {
                    if secondary {
                        self.bg = c;
                    } else {
                        self.fg = c;
                    }
                }
            }
            _ => self.drag = Some(d),
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let redo = ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z));
        let undo = !redo && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z));
        if redo {
            self.do_redo();
        }
        if undo {
            self.do_undo();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
            self.save_project();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::O)) {
            self.open_file();
        }
        let plain = ctx.input(|i| !i.modifiers.command && !i.modifiers.alt);
        if plain {
            let shift = ctx.input(|i| i.modifiers.shift);
            for (tool, _, _, _, key) in TOOLS {
                // Shift+R / Shift+E select the filled variants; T / Y are not advertised.
                let pressed = ctx.input(|i| i.key_pressed(key));
                if pressed && !(shift && matches!(tool, Tool::Rect | Tool::Ellipse)) {
                    self.tool = tool;
                }
            }
            if shift && ctx.input(|i| i.key_pressed(Key::R)) {
                self.tool = Tool::RectFill;
            }
            if shift && ctx.input(|i| i.key_pressed(Key::E)) {
                self.tool = Tool::EllipseFill;
            }
            if ctx.input(|i| i.key_pressed(Key::Minus)) {
                self.step_brush(-1);
            }
            if ctx.input(|i| i.key_pressed(Key::Equals)) {
                self.step_brush(1);
            }
            if ctx.input(|i| i.key_pressed(Key::OpenBracket)) {
                self.step_color(-1);
            }
            if ctx.input(|i| i.key_pressed(Key::CloseBracket)) {
                self.step_color(1);
            }
            if ctx.input(|i| i.key_pressed(Key::X)) {
                std::mem::swap(&mut self.fg, &mut self.bg);
            }
            if ctx.input(|i| i.key_pressed(Key::F)) {
                self.fit_next = true;
            }
        }
    }

    fn new_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.new_dialog else { return };
        let mut create = false;
        let mut cancel = false;
        egui::Window::new("New image").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut dlg.mode, Mode::Multicolor, "Multicolor (160x200)");
                ui.radio_value(&mut dlg.mode, Mode::Hires, "Hires (320x200)");
            });
            ui.horizontal(|ui| {
                ui.label("Size in cells:");
                ui.add(egui::DragValue::new(&mut dlg.w).range(1..=40));
                ui.label("x");
                ui.add(egui::DragValue::new(&mut dlg.h).range(1..=25));
            });
            ui.small("40x25 is a full C64 screen (required for Koala / Art Studio export).");
            ui.horizontal(|ui| {
                create = ui.button("Create").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if create {
            let d = self.new_dialog.take().unwrap();
            self.set_image(Image::new(d.mode, d.w, d.h), None);
            self.status = "New image".into();
        } else if cancel {
            self.new_dialog = None;
        }
    }
}

/// A swatch showing a bit mask (dither tile or brush) as tiny pixels, `w` cells wide, 8 tall.
fn mask_button(ui: &mut egui::Ui, cell: &dyn Fn(usize, usize) -> bool, w: usize, selected: bool, tip: &str) -> egui::Response {
    const S: f32 = 38.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(S), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 5.0, Color32::from_rgb(22, 22, 28));
    let area = Rect::from_center_size(rect.center(), Vec2::splat(S - 12.0));
    let cw = area.width() / w as f32;
    let ch = area.height() / 8.0;
    for y in 0..8 {
        for x in 0..w {
            if cell(x, y) {
                let r = Rect::from_min_size(area.min + Vec2::new(x as f32 * cw, y as f32 * ch), Vec2::new(cw, ch));
                p.rect_filled(r, 0.0, Color32::from_gray(225));
            }
        }
    }
    if selected {
        p.rect_stroke(rect, 5.0, egui::Stroke::new(2.0_f32, ACCENT), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_stroke(rect, 5.0, egui::Stroke::new(1.0_f32, Color32::from_gray(120)), egui::StrokeKind::Inside);
    }
    resp.on_hover_text(tip)
}

fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(10.0);
    ui.label(egui::RichText::new(text).small().strong().color(Color32::from_gray(140)));
    ui.add_space(3.0);
}

fn apply_theme(ctx: &egui::Context) {
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(10.0, 5.0);
        let v = &mut s.visuals;
        v.panel_fill = Color32::from_rgb(30, 30, 36);
        v.window_fill = Color32::from_rgb(36, 36, 44);
        v.window_corner_radius = egui::CornerRadius::same(10);
        v.menu_corner_radius = egui::CornerRadius::same(8);
        v.selection.bg_fill = ACCENT;
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
            w.corner_radius = egui::CornerRadius::same(6);
        }
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(48, 48, 58);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(62, 62, 76);
    });
}

fn ext_of(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.shortcuts(ctx);
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            self.menu(ui);
            ui.add_space(2.0);
            self.toolbar(ui);
            ui.add_space(2.0);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let hover: Option<String> = ctx.data(|d| d.get_temp(egui::Id::new("hover")));
            ui.horizontal(|ui| {
                self.cell_colors_indicator(ui);
                ui.separator();
                ui.label(&self.status);
                if let Some(h) = hover {
                    ui.separator();
                    ui.label(h);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!(
                        "{} {}x{} cells  zoom {:.0}%",
                        match self.img.mode {
                            Mode::Hires => "Hires",
                            Mode::Multicolor => "Multicolor",
                        },
                        self.img.cells_w,
                        self.img.cells_h,
                        self.zoom * 100.0
                    ));
                });
            });
        });
        egui::SidePanel::left("tools").default_width(196.0).resizable(false).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.side_panel(ui));
        });
        egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx, |ui| self.canvas(ui));
        self.new_dialog_ui(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(app: &mut App, button: PointerButton, from: (i32, i32), to: (i32, i32)) {
        let secondary = button == PointerButton::Secondary;
        app.begin(button, secondary, from);
        if let Some(mut d) = app.drag.take() {
            if app.tool == Tool::Pencil {
                let pts = shapes::line(d.last, to);
                app.put_all(&pts, secondary, &mut d);
            }
            d.last = to;
            if app.is_shape_tool() {
                let pts = app.shape_points(d.start, d.last);
                app.put_all(&pts, secondary, &mut d);
            }
            app.finish(d);
        }
    }

    #[test]
    fn pencil_stroke_paints_and_undoes() {
        let mut app = App::new(&egui::Context::default());
        app.fg = 2;
        stroke(&mut app, PointerButton::Primary, (0, 0), (3, 0));
        assert_eq!(app.img.color_at(2, 0), Some(2));
        assert_eq!(app.undo.len(), 1);
        app.do_undo();
        assert_eq!(app.img.color_at(2, 0), Some(0));
        app.do_redo();
        assert_eq!(app.img.color_at(2, 0), Some(2));
    }

    #[test]
    fn right_button_paints_secondary_colour() {
        let mut app = App::new(&egui::Context::default());
        app.bg = 6;
        stroke(&mut app, PointerButton::Secondary, (1, 1), (1, 1));
        assert_eq!(app.img.color_at(1, 1), Some(6));
    }

    #[test]
    fn filled_rect_and_fill_tool() {
        let mut app = App::new(&egui::Context::default());
        app.tool = Tool::RectFill;
        app.fg = 5;
        stroke(&mut app, PointerButton::Primary, (0, 0), (3, 7)); // exactly one multicolor cell
        assert_eq!(app.img.region_at(0, 0).len(), 32);
        app.tool = Tool::Fill;
        app.fg = 7;
        stroke(&mut app, PointerButton::Primary, (5, 5), (5, 5)); // fill the empty remainder
        assert_eq!(app.img.color_at(5, 5), Some(7));
        assert_eq!(app.img.color_at(0, 0), Some(5));
    }

    #[test]
    fn colour_limit_reported_in_status() {
        let mut app = App::new(&egui::Context::default());
        app.tool = Tool::Pencil;
        for (i, c) in [2u8, 5, 7, 1].iter().enumerate() {
            app.fg = *c;
            stroke(&mut app, PointerButton::Primary, (i as i32, 0), (i as i32, 0));
        }
        assert!(app.status.contains("colour limit"), "{}", app.status);
        // no cell ever exceeds its limit
        assert!(app.img.distinct_colors_in_cell(0, 0) <= 4);
    }

    #[test]
    fn hardware_mode_writes_slot_colour() {
        let mut app = App::new(&egui::Context::default());
        app.color_mode = ColorMode::Hardware;
        app.hw_code = 3;
        app.fg = 4;
        stroke(&mut app, PointerButton::Primary, (0, 0), (1, 0));
        assert_eq!(app.img.code_at(1, 0), Some(3));
        assert_eq!(app.img.color_at(1, 0), Some(4));
    }

    #[test]
    fn dithered_stroke_paints_only_pattern_pixels() {
        let mut app = App::new(&egui::Context::default());
        app.img = Image::new(Mode::Hires, 2, 1);
        app.fg = 1;
        app.dither.index = 8; // checkerboard
        app.tool = Tool::RectFill;
        stroke(&mut app, PointerButton::Primary, (0, 0), (7, 7));
        let painted = (0..8).flat_map(|y| (0..8).map(move |x| (x, y))).filter(|&(x, y)| app.img.color_at(x, y) == Some(1)).count();
        assert_eq!(painted, 32);
        assert_ne!(app.img.color_at(0, 0), app.img.color_at(1, 0));
    }

    #[test]
    fn anchored_dither_phase_follows_first_pixel() {
        let mut app = App::new(&egui::Context::default());
        app.img = Image::new(Mode::Hires, 2, 1);
        app.dither.index = 8; // checkerboard whose (0,0) cell is clear
        app.fg = 1;
        app.tool = Tool::Line;
        app.dither.anchored = true;
        stroke(&mut app, PointerButton::Primary, (3, 3), (6, 3));
        let row: Vec<bool> = (3..=6).map(|x| app.img.color_at(x, 3) == Some(1)).collect();
        assert_eq!(row, vec![false, true, false, true]);
    }

    #[test]
    fn brush_stamps_around_pencil_pixels() {
        let mut app = App::new(&egui::Context::default());
        app.img = Image::new(Mode::Hires, 4, 4);
        app.fg = 1;
        app.brush = 7; // full 8x8 square, centre (3,3)
        stroke(&mut app, PointerButton::Primary, (10, 10), (10, 10));
        let n = (0..32).flat_map(|y| (0..32).map(move |x| (x, y))).filter(|&(x, y)| app.img.color_at(x, y) == Some(1)).count();
        assert_eq!(n, 64);
        assert_eq!(app.img.color_at(7, 7), Some(1));
        assert_eq!(app.img.color_at(14, 14), Some(1));
        assert_ne!(app.img.color_at(15, 15), Some(1));
    }

    #[test]
    fn brush_does_not_spill_out_of_fill() {
        let mut app = App::new(&egui::Context::default());
        app.img = Image::new(Mode::Hires, 2, 1);
        app.brush = 7;
        app.fg = 1;
        app.tool = Tool::Fill;
        stroke(&mut app, PointerButton::Primary, (0, 0), (0, 0));
        assert_eq!(app.img.region_at(0, 0).len(), 16 * 8);
    }

    #[test]
    fn hover_preview_matches_what_a_click_paints() {
        let mut app = App::new(&egui::Context::default());
        app.img = Image::new(Mode::Hires, 4, 4);
        app.fg = 1;
        app.brush = 3;
        app.dither.index = 8;
        let preview = app.hover_preview_points((10, 10));
        stroke(&mut app, PointerButton::Primary, (10, 10), (10, 10));
        let mut painted: Vec<(i32, i32)> = (0..32)
            .flat_map(|y| (0..32).map(move |x| (x, y)))
            .filter(|&(x, y)| app.img.color_at(x, y) == Some(1))
            .collect();
        let mut preview = preview;
        preview.sort();
        painted.sort();
        assert_eq!(preview, painted);
    }

    #[test]
    fn hover_preview_is_clipped_to_image() {
        let mut app = App::new(&egui::Context::default());
        app.img = Image::new(Mode::Hires, 1, 1);
        app.brush = 7;
        let pts = app.hover_preview_points((0, 0));
        assert!(pts.iter().all(|&(x, y)| (0..8).contains(&x) && (0..8).contains(&y)));
        assert!(app.hover_preview_points((50, 50)).is_empty());
    }

    #[test]
    fn step_brush_wraps() {
        let mut app = App::new(&egui::Context::default());
        app.step_brush(-1);
        assert_eq!(app.brush, brush::COUNT - 1);
        app.step_brush(1);
        assert_eq!(app.brush, 0);
    }

    #[test]
    fn step_color_wraps() {
        let mut app = App::new(&egui::Context::default());
        app.fg = 15;
        app.step_color(1);
        assert_eq!(app.fg, 0);
        app.step_color(-1);
        assert_eq!(app.fg, 15);
    }

    #[test]
    fn step_color_follows_displayed_order() {
        let mut app = App::new(&egui::Context::default());
        app.sort = SortMode::Luminance;
        let order = app.display_order();
        app.fg = order[3];
        app.step_color(1);
        assert_eq!(app.fg, order[4]);
        app.step_color(-2);
        assert_eq!(app.fg, order[2]);
        app.fg = order[15];
        app.step_color(1);
        assert_eq!(app.fg, order[0]); // wraps within the displayed order
    }

    #[test]
    fn sorting_never_changes_image_or_colours() {
        let mut app = App::new(&egui::Context::default());
        app.fg = 7;
        stroke(&mut app, PointerButton::Primary, (0, 0), (5, 0));
        let before = app.img.clone();
        let before_rgb = app.pal().rgb;
        for m in SortMode::ALL {
            app.sort = m;
            assert_eq!(app.img, before);
            assert_eq!(app.pal().rgb, before_rgb);
        }
    }

    #[test]
    fn picker_sets_colours() {
        let mut app = App::new(&egui::Context::default());
        app.fg = 9;
        stroke(&mut app, PointerButton::Primary, (0, 0), (0, 0));
        app.tool = Tool::Picker;
        app.fg = 1;
        stroke(&mut app, PointerButton::Primary, (0, 0), (0, 0));
        assert_eq!(app.fg, 9);
        assert_eq!(app.undo.len(), 1); // picking is not an undoable edit
    }
}
