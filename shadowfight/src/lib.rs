//! «Бой с Тенью» — a first-person duel game with smooth (non-voxel) graphics.

pub mod fight;
pub mod game;
pub mod gfx;
pub mod story;
pub mod world;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};
use wolfcraft_common::gpu::GpuContext;

struct Running {
    gpu: GpuContext,
    gfx: gfx::Gfx,
    egui: egui::Context,
    state: egui_winit::State,
    rend: egui_wgpu::Renderer,
}

struct App {
    game: game::Game,
    run: Option<Running>,
    last: Instant,
    cursor: (f64, f64),
    mouse_down: bool,
}

impl App {
    fn new(data: PathBuf) -> Self {
        Self { game: game::Game::new(data), run: None, last: Instant::now(), cursor: (0.0, 0.0), mouse_down: false }
    }

    fn create(&mut self, el: &ActiveEventLoop) -> Result<Running, String> {
        let attrs = Window::default_attributes().with_title("Бой с Тенью").with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        let window = Arc::new(el.create_window(attrs).map_err(|e| format!("window: {e}"))?);
        let gpu = GpuContext::new(window.clone(), true, false)?;
        let gfx = gfx::Gfx::new(&gpu.adapter, &gpu.device, gpu.config.format, gpu.size());
        let egui = egui::Context::default();
        let mut style = (*egui.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgba_unmultiplied(20, 14, 18, 200);
        style.visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(90, 40, 30);
        style.visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(150, 60, 40);
        style.spacing.button_padding = egui::vec2(14.0, 8.0);
        egui.set_style(style);
        let state = egui_winit::State::new(egui.clone(), egui::ViewportId::ROOT, &*window, Some(window.scale_factor() as f32), None, None);
        let rend = egui_wgpu::Renderer::new(&gpu.device, gpu.config.format, None, 1, false);
        Ok(Running { gpu, gfx, egui, state, rend })
    }

    fn frame(&mut self, el: &ActiveEventLoop) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.05);
        self.last = now;
        let Some(r) = &mut self.run else { return };
        let Some(frame) = r.gpu.acquire() else { return };
        let size = (frame.texture.width(), frame.texture.height());
        self.game.update(dt);
        self.game.build_scene();
        let view = frame.texture.create_view(&Default::default());
        let mut enc = r.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        r.gfx.render(&r.gpu.device, &r.gpu.queue, &mut enc, &view, size, &self.game.scene);
        let input = r.state.take_egui_input(&r.gpu.window);
        let game = &mut self.game;
        let out = r.egui.run(input, |ctx| game.ui(ctx));
        r.state.handle_platform_output(&r.gpu.window, out.platform_output);
        let jobs = r.egui.tessellate(out.shapes, out.pixels_per_point);
        for (id, d) in &out.textures_delta.set {
            r.rend.update_texture(&r.gpu.device, &r.gpu.queue, *id, d);
        }
        let sd = egui_wgpu::ScreenDescriptor { size_in_pixels: [size.0, size.1], pixels_per_point: out.pixels_per_point };
        let extra = r.rend.update_buffers(&r.gpu.device, &r.gpu.queue, &mut enc, &jobs, &sd);
        {
            let mut pass = enc
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            r.rend.render(&mut pass, &jobs, &sd);
        }
        for id in &out.textures_delta.free {
            r.rend.free_texture(id);
        }
        r.gpu.queue.submit(extra.into_iter().chain(std::iter::once(enc.finish())));
        frame.present();
        if self.game.quit {
            el.exit();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if let Some(r) = &mut self.run {
            r.gpu.resume();
            r.gpu.window.request_redraw();
            return;
        }
        match self.create(el) {
            Ok(r) => {
                r.gpu.window.request_redraw();
                self.run = Some(r);
            }
            Err(e) => {
                log::error!("startup failed: {e}");
                el.exit();
            }
        }
    }

    fn suspended(&mut self, _el: &ActiveEventLoop) {
        if self.game.screen == game::Screen::Fight {
            self.game.screen = game::Screen::Paused;
        }
        if let Some(r) = &mut self.run {
            r.gpu.suspend();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(r) = &mut self.run else { return };
        let ppp = r.egui.pixels_per_point();
        let _ = r.state.on_window_event(&r.gpu.window, &event);
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => r.gpu.resize(s.width, s.height),
            WindowEvent::Touch(t) => self.game.touch(t.id, t.phase, (t.location.x, t.location.y), ppp),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                if self.mouse_down {
                    self.game.touch(game::MOUSE, TouchPhase::Moved, self.cursor, ppp);
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                self.mouse_down = state == ElementState::Pressed;
                let phase = if self.mouse_down { TouchPhase::Started } else { TouchPhase::Ended };
                self.game.touch(game::MOUSE, phase, self.cursor, ppp);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.game.key(code, event.state == ElementState::Pressed, event.repeat);
                }
            }
            WindowEvent::RedrawRequested => self.frame(el),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(r) = &self.run {
            r.gpu.window.request_redraw();
        }
    }
}

fn run(data: PathBuf, el: EventLoop<()>) {
    let mut app = App::new(data);
    if let Err(e) = el.run_app(&mut app) {
        log::error!("event loop: {e}");
    }
}

/// Desktop: the window version (keys WASD, J/K/L, I or Shift — block, Space — dodge, U — shadow).
pub fn run_desktop() {
    let data = std::env::current_dir().unwrap_or_default().join("shadowfight-data");
    match EventLoop::new() {
        Ok(el) => run(data, el),
        Err(e) => eprintln!("event loop: {e}"),
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::event_loop::EventLoopBuilder;
    use winit::platform::android::EventLoopBuilderExtAndroid;
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("ShadowFight"));
    let data = app.internal_data_path().unwrap_or_else(|| PathBuf::from("/data/local/tmp/ShadowFight"));
    match EventLoopBuilder::default().with_android_app(app).build() {
        Ok(el) => run(data, el),
        Err(e) => log::error!("event loop: {e}"),
    }
}
