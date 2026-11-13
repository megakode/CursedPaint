mod app;
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
            app.restore(cc.storage);
            Ok(Box::new(app))
        }),
    )
}
