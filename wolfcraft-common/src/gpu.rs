//! wgpu device/surface bootstrap shared by the launcher and the game (DX12 or Vulkan).

use std::sync::Arc;
use winit::window::Window;

pub struct GpuContext {
    pub window: Arc<Window>,
    /// `None` while the app is in the background (Android takes the window away).
    pub surface: Option<wgpu::Surface<'static>>,
    instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub adapter_info: wgpu::AdapterInfo,
    pub timestamps_supported: bool,
    vsync: bool,
}

impl GpuContext {
    pub fn new(window: Arc<Window>, vsync: bool, want_timestamps: bool) -> Result<Self, String> {
        // Phones flicker with Immediate/Mailbox: always vsync there.
        let vsync = vsync || cfg!(target_os = "android");
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12 | wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone()).map_err(|e| format!("create_surface: {e}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or("No compatible GPU adapter found (DirectX 12 or Vulkan required)")?;
        let adapter_info = adapter.get_info();
        log::info!("GPU: {} ({:?}, {:?})", adapter_info.name, adapter_info.backend, adapter_info.device_type);

        let timestamps_supported = want_timestamps && adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let mut features = wgpu::Features::empty();
        if timestamps_supported {
            features |= wgpu::Features::TIMESTAMP_QUERY;
        }
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("wolfcraft-device"),
                required_features: features,
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|e| format!("request_device: {e}"))?;

        // A GPU validation error must not kill the game: log it and go on.
        device.on_uncaptured_error(Box::new(|e| log::error!("wgpu error: {e}")));
        let caps = surface.get_capabilities(&adapter);
        // egui expects a non-sRGB target (it blends in gamma space); the 3D shader converts itself.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: Self::pick_present_mode(&caps, vsync),
            alpha_mode: if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) { wgpu::CompositeAlphaMode::Opaque } else { caps.alpha_modes[0] },
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        Ok(Self { window, surface: Some(surface), instance, adapter, device, queue, config, adapter_info, timestamps_supported, vsync })
    }

    fn pick_present_mode(caps: &wgpu::SurfaceCapabilities, vsync: bool) -> wgpu::PresentMode {
        if vsync {
            return wgpu::PresentMode::Fifo;
        }
        for m in [wgpu::PresentMode::Immediate, wgpu::PresentMode::Mailbox] {
            if caps.present_modes.contains(&m) {
                return m;
            }
        }
        wgpu::PresentMode::Fifo
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        if let Some(s) = &self.surface {
            s.configure(&self.device, &self.config);
        }
    }

    /// Asks the phone to run the screen at `hz` (Android 11+; ignored elsewhere). With vsync on,
    /// the game then renders at that rate.
    pub fn set_refresh_rate(&self, hz: u32) {
        #[cfg(target_os = "android")]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            // Looked up at run time: the function only exists on Android 11+.
            type SetFrameRate = unsafe extern "C" fn(*mut std::ffi::c_void, f32, i8) -> i32;
            unsafe extern "C" {
                fn dlopen(name: *const std::ffi::c_char, flags: i32) -> *mut std::ffi::c_void;
                fn dlsym(handle: *mut std::ffi::c_void, name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
            }
            let lib = unsafe { dlopen(c"libandroid.so".as_ptr(), 2) };
            if lib.is_null() {
                return;
            }
            let sym = unsafe { dlsym(lib, c"ANativeWindow_setFrameRate".as_ptr()) };
            if sym.is_null() {
                log::info!("refresh rate: not supported on this Android version");
                return;
            }
            let set: SetFrameRate = unsafe { std::mem::transmute(sym) };
            if let Ok(h) = self.window.window_handle() {
                if let RawWindowHandle::AndroidNdk(a) = h.as_raw() {
                    // compatibility 0 = default: the app renders at this rate.
                    let r = unsafe { set(a.a_native_window.as_ptr(), hz as f32, 0) };
                    log::info!("refresh rate {hz} Hz requested (result {r})");
                }
            }
        }
        #[cfg(not(target_os = "android"))]
        let _ = hz;
    }

    /// App went to the background: the window's native surface is about to go away.
    pub fn suspend(&mut self) {
        self.surface = None;
    }

    /// App is back: make a new surface for the (new) native window and configure it.
    pub fn resume(&mut self) {
        match self.instance.create_surface(self.window.clone()) {
            Ok(s) => {
                let size = self.window.inner_size();
                if size.width > 0 && size.height > 0 {
                    self.config.width = size.width;
                    self.config.height = size.height;
                }
                s.configure(&self.device, &self.config);
                self.surface = Some(s);
                log::info!("surface recreated {}x{}", self.config.width, self.config.height);
            }
            Err(e) => log::error!("surface recreate failed: {e}"),
        }
    }

    pub fn set_vsync(&mut self, vsync: bool) {
        if vsync == self.vsync {
            return;
        }
        let vsync = vsync || cfg!(target_os = "android");
        self.vsync = vsync;
        let Some(surface) = &self.surface else { return };
        let caps = surface.get_capabilities(&self.adapter);
        self.config.present_mode = Self::pick_present_mode(&caps, vsync);
        surface.configure(&self.device, &self.config);
    }

    pub fn vsync(&self) -> bool {
        self.vsync
    }

    /// Acquires the next swapchain image, reconfiguring on Lost/Outdated. `None` = skip this frame.
    pub fn acquire(&mut self) -> Option<wgpu::SurfaceTexture> {
        let surface = self.surface.as_ref()?;
        match surface.get_current_texture() {
            Ok(t) => {
                if t.suboptimal {
                    log::warn!("surface suboptimal");
                }
                Some(t)
            }
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                log::warn!("surface lost/outdated: reconfiguring");
                surface.configure(&self.device, &self.config);
                surface.get_current_texture().ok()
            }
            Err(wgpu::SurfaceError::Timeout) => {
                log::warn!("surface timeout");
                None
            }
            Err(e) => {
                log::error!("surface error: {e:?}");
                None
            }
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }
}
