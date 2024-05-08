#![deny(warnings)]

use {
    egui_speedy2d::{WindowHandler, WindowWrapper},
    speedy2d::{color::Color, window::WindowHelper, Graphics2D, Window},
};

fn main() {
    simple_logger::SimpleLogger::new().init().unwrap();
    let window = Window::new_centered("Basic sample", (640, 240)).unwrap();
    window.run_loop(WindowWrapper::new(MyWindowHandler {}))
}

struct MyWindowHandler {}

impl WindowHandler for MyWindowHandler {
    fn on_draw(
        &mut self,
        helper: &mut WindowHelper,
        graphics: &mut Graphics2D,
        egui_ctx: &egui::Context,
    ) {
        graphics.clear_screen(Color::WHITE);
        egui::Window::new("Hello").show(egui_ctx, |ui| {
            ui.label("World !");
        });
        helper.request_redraw();
    }
}
