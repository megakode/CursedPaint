mod app;
mod crt;
mod icons;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_title("CursedPaint"),
        ..Default::default()
    };
    eframe::run_native(
        "CursedPaint",
        options,
        Box::new(|cc| {
            let mut app = app::App::new(&cc.egui_ctx);
            if let Some(get) = cc.get_proc_address {
                // SAFETY: eframe's GL context is current while the creation callback runs.
                let gl = unsafe { glow17::Context::from_loader_function_cstr(|name| get(name)) };
                app.set_crt(crt::CrtPreview::new(gl));
            }
            app.restore(cc.storage);
            if let Some(path) = std::env::args_os().nth(1) {
                app.open_path(path.into());
            }
            Ok(Box::new(app))
        }),
    )
}
