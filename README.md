# egui_speedy2d

#### [crates.io](https://crates.io/crates/egui_speedy2d) | [docs](https://docs.rs/egui_speedy2d)

egui_speedy2d is a library that helps integrate [egui](https://crates.io/crates/egui),
an immediate mode GUI library, with [speeedy2d](https://crates.io/crates/speedy2d),
a 2D rendering framework.

## Warning

The current version has only been tested on linux platform. It should work on windows platform.
Not all features of egui has been tested. The work is still in progress. All merge-requests are
welcome.

## Basic example

```rust
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
        egui::Window::new("Hello").show(&egui_ctx, |ui| {
            ui.label("World !");
        });
        helper.request_redraw();
    }
}
```

## License

This project is licensed under
- [BSD-3-Clause](https://github.com/heretik31/egui_speedy2d/LICENSE)

Any contribution intentionally submitted for inclusion by you, shall be licensed as above, without
any additional terms or conditions.
