//! # egui_speedy2d
//!
//! egui_speedy2d is a library that helps integrate [egui](https://crates.io/crates/egui),
//! an immediate mode GUI library, with [speedy2d](https://crates.io/crates/speedy2d),
//! a 2D rendering framework.
//!
//! ## Usage
//!
//! The easiest way to use egui_speedy2d is to make your main WindowHandler struct
//! implement egui_speedy2d's [`WindowHandler`] trait instead of
//! [speedy2d's](speedy2d::window::WindowHandler). This will
//! give you access to a [`egui_ctx`](egui::Context) context where you can do your GUI
//! rendering.
//!
//! ```
//! struct MyWindowHandler;
//!
//! impl egui_speedy2d::WindowHandler for MyWindowHandler {
//!     fn on_draw(
//!         &mut self,
//!         helper: &mut WindowHelper,
//!         graphics: &mut Graphics2D,
//!         egui_ctx: &egui::Context,
//!     ) {
//!         graphics.clear_screen(Color::WHITE);
//!         egui::Window::new("Hello").show(&egui_ctx, |ui| {
//!             ui.label("World !");
//!         });
//!     }
//! }
//! ```
//!
//! When running the speedy2d [`Window`](speedy2d::Window::run_loop), wrap your handler
//! struct in a [`WindowHandler`] to make it compatible with Speedy2d's
//! [`speedy2d::windows::WindowHandler` trait](speedy2d::window::WindowHandler).
//!
//! ```no_run
//! fn main() {
//!     let window = speedy2d::Window::new_centered("Speedy2D: Hello World", (640, 240)).unwrap();
//!     window.run_loop(egui_speedy2d::WindowWrapper::new(MyWindowHandler{}))
//! }
//! ```

pub use egui;
use egui::{Context, RawInput};
use speedy2d::{
    color::Color,
    dimen::{UVec2, Vec2},
    error::{BacktraceError, ErrorMessage},
    image::{ImageDataType, ImageHandle, ImageSmoothingMode},
    window::{
        KeyScancode, ModifiersState, MouseButton, MouseScrollDistance, VirtualKeyCode,
        WindowHelper, WindowStartupInfo,
    },
    Graphics2D,
};
use std::collections::HashMap;

/// Wraps an egui context with features that are useful
/// for integrating egui with Speedy2d.
pub struct WindowWrapper<UserEventType> {
    handler: Box<dyn WindowHandler<UserEventType>>,
    raw_input: RawInput,
    egui_ctx: Context,
    id_and_textures: HashMap<u64, (ImageHandle, RgbaImage)>,
    to_free_textures: Vec<u64>,
    last_mouse_position: Vec2,
    current_modifiers: ModifiersState,
}

impl<UserEventType> WindowWrapper<UserEventType> {
    /// Creates a new [`WindowWrapper`] and underlying egui context.
    pub fn new(handler: impl WindowHandler<UserEventType> + 'static) -> Self {
        Self {
            handler: Box::new(handler),
            raw_input: Default::default(),
            egui_ctx: Default::default(),
            id_and_textures: Default::default(),
            to_free_textures: Default::default(),
            last_mouse_position: Vec2::new(0., 0.),
            current_modifiers: Default::default(),
        }
    }

    /// Draws the latest finished GUI frame to the screen.
    pub fn draw(
        &mut self,
        full_output: egui::FullOutput,
        gfx: &mut Graphics2D,
    ) -> Result<(), BacktraceError<ErrorMessage>> {
        // free old textures
        self.free_textures();

        // get change
        let clipped_primitives = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

        // save textures to delete next frame
        self.to_free_textures = full_output
            .textures_delta
            .free
            .iter()
            .filter_map(|t| match t {
                egui::TextureId::Managed(id) => Some(*id),
                egui::TextureId::User(_) => None,
            })
            .collect();

        // set new textures
        self.set_textures(full_output.textures_delta, gfx)?;

        // draw
        for egui::ClippedPrimitive {
            clip_rect,
            primitive,
        } in clipped_primitives
        {
            gfx.set_clip(Some(rect_from_egui(clip_rect)));
            if let epaint::Primitive::Mesh(epaint::Mesh {
                indices,
                vertices,
                texture_id,
            }) = primitive
            {
                let texture_id = match texture_id {
                    egui::TextureId::Managed(id) => id,
                    egui::TextureId::User(_) => continue,
                };

                let handle = self.id_and_textures.get(&texture_id).unwrap().0.clone();
                for indices in indices.chunks_exact(3) {
                    let mut v = indices
                        .iter()
                        .map(|i| vertices[*i as usize])
                        .collect::<Vec<_>>();
                    let mut p = v.iter().map(|v| vec2_from_egui(v.pos)).collect::<Vec<_>>();

                    // dots must be in clockwise order
                    let cross_product = (p[1].x - p[0].x) * (p[2].y - p[0].y)
                        - (p[1].y - p[0].y) * (p[2].x - p[0].x);
                    if cross_product.is_sign_positive() {
                        v.swap(1, 2);
                        p.swap(1, 2);
                    }

                    let colors = v
                        .iter()
                        .map(|v| color_from_egui(v.color))
                        .collect::<Vec<_>>();
                    let uvs = v.iter().map(|v| vec2_from_egui(v.uv)).collect::<Vec<_>>();

                    gfx.draw_triangle_image_tinted_three_color(
                        p.try_into().unwrap(),
                        colors.try_into().unwrap(),
                        uvs.try_into().unwrap(),
                        &handle,
                    );
                }
            } else {
                todo!();
            }
        }

        // todo handle platform output

        Ok(())
    }

    fn set_textures(
        &mut self,
        textures_delta: egui::TexturesDelta,
        gfx: &mut Graphics2D,
    ) -> Result<(), BacktraceError<ErrorMessage>> {
        for (texture_id, image_delta) in textures_delta.set {
            let id = match texture_id {
                egui::TextureId::Managed(texture_id) => texture_id,
                egui::TextureId::User(_) => continue,
            };

            let image = RgbaImage::from(image_delta.image);
            if let Some(_pos) = image_delta.pos {
                todo!();
            } else {
                let handle = gfx.create_image_from_raw_pixels(
                    ImageDataType::RGBA,
                    match image_delta.options {
                        egui::TextureOptions::NEAREST => ImageSmoothingMode::NearestNeighbor,
                        egui::TextureOptions::LINEAR => ImageSmoothingMode::Linear,
                        _ => ImageSmoothingMode::Linear,
                    },
                    UVec2::new(image.size.0 as u32, image.size.1 as u32),
                    &image.pixels,
                )?;
                self.id_and_textures.insert(id, (handle, image));
            }
        }
        Ok(())
    }

    fn free_textures(&mut self) {
        for id in self.to_free_textures.drain(..) {
            self.id_and_textures.remove(&id);
        }
    }
}

/// A trait analogous to [`speedy2d::window::WindowHandler`], but with the
/// addition of a [`egui_ctx`](egui::Context) argument.
///
/// You can use a type implementing this trait as your main
/// window handler by wrapping it with a [`speedy2d::window::WindowHandler`] and passing the wrapper
/// to [`speedy2d::Window::run_loop`].
pub trait WindowHandler<UserEventType = ()> {
    /// Invoked once when the window first starts.
    #[allow(unused_variables)]
    #[inline]
    fn on_start(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        info: WindowStartupInfo,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked when a user-defined event is received, allowing you to wake up
    /// the event loop to handle events from other threads.
    ///
    /// See [WindowHelper::create_user_event_sender].
    #[allow(unused_variables)]
    #[inline]
    fn on_user_event(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        user_event: UserEventType,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked when the window is resized.
    #[allow(unused_variables)]
    #[inline]
    fn on_resize(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        size_pixels: UVec2,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked if the mouse cursor becomes grabbed or un-grabbed. See
    /// [WindowHelper::set_cursor_grab].
    ///
    /// Note: mouse movement events will behave differently depending on the
    /// current cursor grabbing status.
    #[allow(unused_variables)]
    #[inline]
    fn on_mouse_grab_status_changed(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        mouse_grabbed: bool,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked if the window enters or exits fullscreen mode. See
    /// [WindowHelper::set_fullscreen_mode].
    #[allow(unused_variables)]
    #[inline]
    fn on_fullscreen_status_changed(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        fullscreen: bool,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked when the window scale factor changes.
    #[allow(unused_variables)]
    #[inline]
    fn on_scale_factor_changed(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        scale_factor: f64,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked when the contents of the window needs to be redrawn.
    ///
    /// It is possible to request a redraw from any callback using
    /// [WindowHelper::request_redraw].
    #[allow(unused_variables)]
    #[inline]
    fn on_draw(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        graphics: &mut Graphics2D,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked when the mouse changes position.
    ///
    /// Normally, this provides the absolute  position of the mouse in the
    /// window/canvas. However, if the mouse cursor is grabbed, this will
    /// instead provide the amount of relative movement since the last move
    /// event.
    ///
    /// See [WindowHandler::on_mouse_grab_status_changed].
    #[allow(unused_variables)]
    #[inline]
    fn on_mouse_move(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        position: Vec2,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked when a mouse button is pressed.
    #[allow(unused_variables)]
    #[inline]
    fn on_mouse_button_down(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        button: MouseButton,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked when a mouse button is released.
    #[allow(unused_variables)]
    #[inline]
    fn on_mouse_button_up(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        button: MouseButton,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked when the mouse wheel moves.
    #[allow(unused_variables)]
    #[inline]
    fn on_mouse_wheel_scroll(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        distance: MouseScrollDistance,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked when a keyboard key is pressed.
    ///
    /// To detect when a character is typed, see the
    /// [WindowHandler::on_keyboard_char] callback.
    #[allow(unused_variables)]
    #[inline]
    fn on_key_down(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        virtual_key_code: Option<VirtualKeyCode>,
        scancode: KeyScancode,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked when a keyboard key is released.
    #[allow(unused_variables)]
    #[inline]
    fn on_key_up(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        virtual_key_code: Option<VirtualKeyCode>,
        scancode: KeyScancode,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked when a character is typed on the keyboard.
    ///
    /// This is invoked in addition to the [WindowHandler::on_key_up] and
    /// [WindowHandler::on_key_down] callbacks.
    #[allow(unused_variables)]
    #[inline]
    fn on_keyboard_char(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        unicode_codepoint: char,
        egui_ctx: &egui::Context,
    ) {
    }

    /// Invoked when the state of the modifier keys has changed.
    #[allow(unused_variables)]
    #[inline]
    fn on_keyboard_modifiers_changed(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        state: ModifiersState,
        egui_ctx: &egui::Context,
    ) {
    }
}

impl<UserEventType> speedy2d::window::WindowHandler<UserEventType>
    for WindowWrapper<UserEventType>
{
    /// Invoked once when the window first starts.
    #[allow(unused_variables)]
    #[inline]
    fn on_start(&mut self, helper: &mut WindowHelper<UserEventType>, info: WindowStartupInfo) {
        self.handler.on_start(helper, info, &self.egui_ctx);
    }

    /// Invoked when a user-defined event is received, allowing you to wake up
    /// the event loop to handle events from other threads.
    ///
    /// See [WindowHelper::create_user_event_sender].
    #[allow(unused_variables)]
    #[inline]
    fn on_user_event(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        user_event: UserEventType,
    ) {
        self.handler
            .on_user_event(helper, user_event, &self.egui_ctx);
    }

    /// Invoked when the window is resized.
    #[allow(unused_variables)]
    #[inline]
    fn on_resize(&mut self, helper: &mut WindowHelper<UserEventType>, size_pixels: UVec2) {
        self.raw_input.screen_rect = Some(egui::Rect::from_min_max(
            Default::default(),
            pos_from_uvec2(size_pixels),
        ));
        self.handler.on_resize(helper, size_pixels, &self.egui_ctx);
    }

    /// Invoked if the mouse cursor becomes grabbed or un-grabbed. See
    /// [WindowHelper::set_cursor_grab].
    ///
    /// Note: mouse movement events will behave differently depending on the
    /// current cursor grabbing status.
    #[allow(unused_variables)]
    #[inline]
    fn on_mouse_grab_status_changed(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        mouse_grabbed: bool,
    ) {
        self.handler
            .on_mouse_grab_status_changed(helper, mouse_grabbed, &self.egui_ctx);
    }

    /// Invoked if the window enters or exits fullscreen mode. See
    /// [WindowHelper::set_fullscreen_mode].
    #[allow(unused_variables)]
    #[inline]
    fn on_fullscreen_status_changed(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        fullscreen: bool,
    ) {
        self.handler
            .on_fullscreen_status_changed(helper, fullscreen, &self.egui_ctx);
    }

    /// Invoked when the window scale factor changes.
    #[allow(unused_variables)]
    #[inline]
    fn on_scale_factor_changed(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        scale_factor: f64,
    ) {
        self.egui_ctx.set_pixels_per_point(scale_factor as f32);
        self.handler
            .on_scale_factor_changed(helper, scale_factor, &self.egui_ctx);
    }

    /// Invoked when the contents of the window needs to be redrawn.
    ///
    /// It is possible to request a redraw from any callback using
    /// [WindowHelper::request_redraw].
    #[allow(unused_variables)]
    #[inline]
    fn on_draw(&mut self, helper: &mut WindowHelper<UserEventType>, graphics: &mut Graphics2D) {
        let ctx = &self.egui_ctx;
        // extract events and begin frame
        let raw_input = self.raw_input.take();
        ctx.begin_frame(raw_input);
        self.handler.on_draw(helper, graphics, ctx);
        let full_output = ctx.end_frame();
        // speedy2d doesn't authorize errors. So... panic.
        self.draw(full_output, graphics).unwrap();
    }

    /// Invoked when the mouse changes position.
    ///
    /// Normally, this provides the absolute  position of the mouse in the
    /// window/canvas. However, if the mouse cursor is grabbed, this will
    /// instead provide the amount of relative movement since the last move
    /// event.
    ///
    /// See [WindowHandler::on_mouse_grab_status_changed].
    #[allow(unused_variables)]
    #[inline]
    fn on_mouse_move(&mut self, helper: &mut WindowHelper<UserEventType>, position: Vec2) {
        self.last_mouse_position = position;
        self.raw_input
            .events
            .push(egui::Event::PointerMoved(pos2_from_speedy2d(position)));
        self.handler.on_mouse_move(helper, position, &self.egui_ctx);
    }

    /// Invoked when a mouse button is pressed.
    #[allow(unused_variables)]
    #[inline]
    fn on_mouse_button_down(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        button: MouseButton,
    ) {
        if let Some(button) = match button {
            MouseButton::Left => Some(egui::PointerButton::Primary),
            MouseButton::Right => Some(egui::PointerButton::Secondary),
            MouseButton::Middle => Some(egui::PointerButton::Middle),
            MouseButton::Other(btn) => None,
        } {
            self.raw_input.events.push(egui::Event::PointerButton {
                pos: pos2_from_speedy2d(self.last_mouse_position),
                button,
                pressed: true,
                modifiers: modifiers_from_speedy2d(&self.current_modifiers),
            });
        }
        self.handler
            .on_mouse_button_down(helper, button, &self.egui_ctx);
    }

    /// Invoked when a mouse button is released.
    #[allow(unused_variables)]
    #[inline]
    fn on_mouse_button_up(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        button: MouseButton,
    ) {
        if let Some(button) = match button {
            MouseButton::Left => Some(egui::PointerButton::Primary),
            MouseButton::Right => Some(egui::PointerButton::Secondary),
            MouseButton::Middle => Some(egui::PointerButton::Middle),
            MouseButton::Other(btn) => None,
        } {
            self.raw_input.events.push(egui::Event::PointerButton {
                pos: pos2_from_speedy2d(self.last_mouse_position),
                button,
                pressed: false,
                modifiers: modifiers_from_speedy2d(&self.current_modifiers),
            });
        }
        self.handler
            .on_mouse_button_up(helper, button, &self.egui_ctx);
    }

    /// Invoked when the mouse wheel moves.
    #[allow(unused_variables)]
    #[inline]
    fn on_mouse_wheel_scroll(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        distance: MouseScrollDistance,
    ) {
        self.handler
            .on_mouse_wheel_scroll(helper, distance, &self.egui_ctx);
    }

    /// Invoked when a keyboard key is pressed.
    ///
    /// To detect when a character is typed, see the
    /// [WindowHandler::on_keyboard_char] callback.
    #[allow(unused_variables)]
    #[inline]
    fn on_key_down(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        virtual_key_code: Option<VirtualKeyCode>,
        scancode: KeyScancode,
    ) {
        if let Some(key) = key_from_speedy2d(virtual_key_code) {
            self.raw_input.events.push(egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers: modifiers_from_speedy2d(&self.current_modifiers),
                physical_key: None,
            });
        }
        self.handler
            .on_key_down(helper, virtual_key_code, scancode, &self.egui_ctx);
    }

    /// Invoked when a keyboard key is released.
    #[allow(unused_variables)]
    #[inline]
    fn on_key_up(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        virtual_key_code: Option<VirtualKeyCode>,
        scancode: KeyScancode,
    ) {
        if let Some(key) = key_from_speedy2d(virtual_key_code) {
            self.raw_input.events.push(egui::Event::Key {
                key,
                pressed: false,
                repeat: false,
                modifiers: modifiers_from_speedy2d(&self.current_modifiers),
                physical_key: None,
            });
        }
        self.handler
            .on_key_up(helper, virtual_key_code, scancode, &self.egui_ctx);
    }

    /// Invoked when a character is typed on the keyboard.
    ///
    /// This is invoked in addition to the [WindowHandler::on_key_up] and
    /// [WindowHandler::on_key_down] callbacks.
    #[allow(unused_variables)]
    #[inline]
    fn on_keyboard_char(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        unicode_codepoint: char,
    ) {
        self.raw_input
            .events
            .push(egui::Event::Text(unicode_codepoint.to_string()));
        self.handler
            .on_keyboard_char(helper, unicode_codepoint, &self.egui_ctx);
    }

    /// Invoked when the state of the modifier keys has changed.
    #[allow(unused_variables)]
    #[inline]
    fn on_keyboard_modifiers_changed(
        &mut self,
        helper: &mut WindowHelper<UserEventType>,
        state: ModifiersState,
    ) {
        self.current_modifiers = state.clone();
        self.raw_input.modifiers = modifiers_from_speedy2d(&state);
        self.handler
            .on_keyboard_modifiers_changed(helper, state, &self.egui_ctx);
    }
}

fn rect_from_egui(rect: egui::Rect) -> speedy2d::shape::Rectangle<i32> {
    speedy2d::shape::Rectangle::new(ivec2_from_egui(rect.min), ivec2_from_egui(rect.max))
}

fn pos_from_uvec2(pos: UVec2) -> egui::Pos2 {
    egui::Pos2::new(pos.x as f32, pos.y as f32)
}

fn color_from_egui(color: epaint::Color32) -> Color {
    Color::from_int_rgba(color.r(), color.g(), color.b(), color.a())
}

fn vec2_from_egui(pos: egui::Pos2) -> speedy2d::dimen::Vec2 {
    speedy2d::dimen::Vec2::new(pos.x, pos.y)
}

fn ivec2_from_egui(pos: egui::Pos2) -> speedy2d::dimen::IVec2 {
    speedy2d::dimen::IVec2::new(pos.x.round() as i32, pos.y.round() as i32)
}

fn pos2_from_speedy2d(pos: Vec2) -> egui::Pos2 {
    egui::Pos2::new(pos.x, pos.y)
}

fn modifiers_from_speedy2d(modifiers: &ModifiersState) -> egui::Modifiers {
    egui::Modifiers {
        alt: modifiers.alt(),
        ctrl: modifiers.ctrl(),
        shift: modifiers.shift(),
        mac_cmd: false,
        command: modifiers.ctrl(),
    }
}

fn key_from_speedy2d(virtual_key_code: Option<VirtualKeyCode>) -> Option<egui::Key> {
    use VirtualKeyCode::*;
    match virtual_key_code {
        Some(A) => Some(egui::Key::A),
        Some(B) => Some(egui::Key::B),
        Some(C) => Some(egui::Key::C),
        Some(D) => Some(egui::Key::D),
        Some(E) => Some(egui::Key::E),
        Some(F) => Some(egui::Key::F),
        Some(G) => Some(egui::Key::G),
        Some(H) => Some(egui::Key::H),
        Some(I) => Some(egui::Key::I),
        Some(J) => Some(egui::Key::J),
        Some(K) => Some(egui::Key::K),
        Some(L) => Some(egui::Key::L),
        Some(M) => Some(egui::Key::M),
        Some(N) => Some(egui::Key::N),
        Some(O) => Some(egui::Key::O),
        Some(P) => Some(egui::Key::P),
        Some(Q) => Some(egui::Key::Q),
        Some(R) => Some(egui::Key::R),
        Some(S) => Some(egui::Key::S),
        Some(T) => Some(egui::Key::T),
        Some(U) => Some(egui::Key::U),
        Some(V) => Some(egui::Key::V),
        Some(W) => Some(egui::Key::W),
        Some(X) => Some(egui::Key::X),
        Some(Y) => Some(egui::Key::Y),
        Some(Z) => Some(egui::Key::Z),
        Some(Escape) => Some(egui::Key::Escape),
        Some(F1) => Some(egui::Key::F1),
        Some(F2) => Some(egui::Key::F2),
        Some(F3) => Some(egui::Key::F3),
        Some(F4) => Some(egui::Key::F4),
        Some(F5) => Some(egui::Key::F5),
        Some(F6) => Some(egui::Key::F6),
        Some(F7) => Some(egui::Key::F7),
        Some(F8) => Some(egui::Key::F8),
        Some(F9) => Some(egui::Key::F9),
        Some(F10) => Some(egui::Key::F10),
        Some(F11) => Some(egui::Key::F11),
        Some(F12) => Some(egui::Key::F12),
        Some(F13) => Some(egui::Key::F13),
        Some(F14) => Some(egui::Key::F14),
        Some(F15) => Some(egui::Key::F15),
        Some(F16) => Some(egui::Key::F16),
        Some(F17) => Some(egui::Key::F17),
        Some(F18) => Some(egui::Key::F18),
        Some(F19) => Some(egui::Key::F19),
        Some(F20) => Some(egui::Key::F20),
        Some(Insert) => Some(egui::Key::Insert),
        Some(Home) => Some(egui::Key::Home),
        Some(Delete) => Some(egui::Key::Delete),
        Some(End) => Some(egui::Key::End),
        Some(PageDown) => Some(egui::Key::PageDown),
        Some(PageUp) => Some(egui::Key::PageUp),
        Some(Left) => Some(egui::Key::ArrowLeft),
        Some(Up) => Some(egui::Key::ArrowUp),
        Some(Right) => Some(egui::Key::ArrowRight),
        Some(Down) => Some(egui::Key::ArrowDown),
        Some(Backspace) => Some(egui::Key::Backspace),
        Some(Return) => Some(egui::Key::Enter),
        Some(Space) => Some(egui::Key::Space),
        Some(Numpad0) => Some(egui::Key::Num0),
        Some(Numpad1) => Some(egui::Key::Num1),
        Some(Numpad2) => Some(egui::Key::Num2),
        Some(Numpad3) => Some(egui::Key::Num3),
        Some(Numpad4) => Some(egui::Key::Num4),
        Some(Numpad5) => Some(egui::Key::Num5),
        Some(Numpad6) => Some(egui::Key::Num6),
        Some(Numpad7) => Some(egui::Key::Num7),
        Some(Numpad8) => Some(egui::Key::Num8),
        Some(Numpad9) => Some(egui::Key::Num9),
        Some(Tab) => Some(egui::Key::Tab),
        Some(_) => None,
        None => None,
    }
}

struct RgbaImage {
    size: (usize, usize),
    pixels: Vec<u8>,
}

impl RgbaImage {
    fn from(image: egui::ImageData) -> Self {
        Self {
            size: {
                let size = image.size();
                (size[0], size[1])
            },
            pixels: match image {
                egui::ImageData::Font(font_image) => {
                    let mut pixels = vec![];
                    for color in font_image.srgba_pixels(None) {
                        pixels.push(color.r());
                        pixels.push(color.g());
                        pixels.push(color.b());
                        pixels.push(color.a());
                    }
                    pixels
                }
                egui::ImageData::Color(color_image) => {
                    let mut pixels = vec![];
                    for color in &color_image.pixels {
                        pixels.push(color.r());
                        pixels.push(color.g());
                        pixels.push(color.b());
                        pixels.push(color.a());
                    }
                    pixels
                }
            },
        }
    }
}
