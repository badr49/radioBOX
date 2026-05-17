mod config;
mod gui;
mod player;
mod stream;

use eframe::NativeOptions;
use egui::ViewportBuilder;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("radioBOX")
            .with_inner_size([860.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        "radioBOX",
        options,
        Box::new(|cc| Ok(Box::new(gui::RadioApp::new(cc)))),
    )
}
