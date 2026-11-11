//! Application: window, GPU, screens and the main loop.

use crate::loader::{self, LoadRequest, LoadShared, STAGES};
use crate::preview;
use crate::session::{GameSession, InputFrame};
use crate::touch::{TouchAction, TouchState};
use egui::TextureHandle;
use glam::DVec3;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};
use wolfcraft_assets::AtlasManager;
use wolfcraft_common::gpu::GpuContext;
use wolfcraft_common::launch::LaunchArgs;
use wolfcraft_common::platform::{self, CpuMeter};
use wolfcraft_common::time::{CpuProfile, ScopeTimer, Time};
use wolfcraft_engine::config::*;
use wolfcraft_engine::interaction::InteractionEvent;
use wolfcraft_engine::cubesphere::ChunkKey;
use wolfcraft_engine::mesher::MeshData;
use wolfcraft_engine::planet::{Planet, splitmix};
use wolfcraft_engine::player::DAY_LENGTH;
use wolfcraft_engine::renderer::{FrameParams, WorldRenderer, sky_color};
use wolfcraft_engine::renderer::model::{HandsPose, PlayerPose, build_first_person_hands, build_held_block, build_player_model};
use wolfcraft_ui::chat_ui::{self, ChatKind, ChatState};
use wolfcraft_ui::hud::{self, DebugInfo, HudState};
use wolfcraft_ui::inventory_ui::{self, InventoryUiState};
use wolfcraft_ui::main_menu::{self, CreateWorldState, LoadingView, MenuAction, PauseInfo, WorldEntry, WorldSelectState};
use wolfcraft_ui::{GuiRenderer, theme_manager};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Screen {
    Main,
    WorldSelect,
    Create,
    Settings,
    Loading,
    Playing,
    Paused,
    Inventory,
    Dead,
}

impl Screen {
    fn in_world(self) -> bool {
        matches!(self, Screen::Playing | Screen::Paused | Screen::Inventory | Screen::Dead)
    }
}

struct Gfx {
    window: Arc<Window>,
    gpu: GpuContext,
    gui: GuiRenderer,
    atlases: AtlasManager,
    world: WorldRenderer,
}

#[derive(Default)]
struct InputState {
    keys: HashSet<KeyCode>,
    mouse_dx: f64,
    mouse_dy: f64,
    left: bool,
    right_press: bool,
    right: bool,
    swap_press: bool,
    middle_press: bool,
    scroll: f32,
    jump_press: bool,
    hotbar_set: Option<usize>,
}

struct LoadState {
    shared: Arc<LoadShared>,
    title: String,
    error: Option<String>,
    started: Instant,
}

pub struct App {
    args: LaunchArgs,
    data_dir: PathBuf,
    worlds_dir: PathBuf,
    settings: Settings,
    gfx: Option<Gfx>,
    screen: Screen,
    nav: Vec<Screen>,
    world_select: WorldSelectState,
    create: CreateWorldState,
    worlds: Vec<WorldEntry>,
    preview_cache: HashMap<String, TextureHandle>,
    preview_tx: Sender<(String, egui::ColorImage)>,
    preview_rx: Receiver<(String, egui::ColorImage)>,
    preview_requested: HashSet<String>,
    create_preview: Option<TextureHandle>,
    create_preview_key: u64,
    create_preview_inflight: Option<u64>,
    create_tx: Sender<(u64, egui::ColorImage)>,
    create_rx: Receiver<(u64, egui::ColorImage)>,
    random_seed: u64,
    load: Option<LoadState>,
    session: Option<GameSession>,
    inv_ui: InventoryUiState,
    chat: ChatState,
    fov_fx: f32,
    input: InputState,
    touch: TouchState,
    time: Time,
    cpu: CpuMeter,
    profile: CpuProfile,
    debug: bool,
    grabbed: bool,
    pending_meshes: VecDeque<(ChunkKey, MeshData)>,
    next_frame: Instant,
    quit: bool,
    frames: u64,
    last_cpu: f32,
    near_table: bool,
    /// Open chest / furnace (block cell and kind).
    container: Option<(wolfcraft_engine::cubesphere::Cell, wolfcraft_engine::block::Block)>,
    pending_apply: bool,
    /// First-person arms: punch cycle (1 = right + left jab) and guard blend (0 rest … 1 boxing).
    hand_cycle: f32,
    hand_guard: f32,
    /// Keeps a jab going briefly after a short click.
    hand_click: f32,
    /// Shield raise blend (0 down … 1 blocking).
    hand_block: f32,
    /// Block-placing arm motion, 1 -> 0.
    hand_place: f32,
    /// Item-switch dip of the view model: 1 right after changing the slot, fading to 0.
    hand_switch: f32,
    last_hotbar: usize,
    audio: crate::audio::Audio,
    /// Connection to a WolfServer while playing online.
    /// A few colours of every block tile, for break particles.
    tile_colors: Vec<[[f32; 3]; 3]>,
    prev_hand_cycle: f32,
    net: Option<wolfcraft_net::Client>,
    /// Server running inside the game ("host on this device").
    host: Option<wolfcraft_net::server::ServerHandle>,
    net_players: Vec<wolfcraft_net::PlayerState>,
    /// Every edit the server told us about, per dimension (`[overworld, hell]`).
    net_edits: [std::collections::HashMap<[i32; 4], u8>; 2],
    net_send_t: f32,
    net_hell: bool,
    last_step: i64,
    last_health: f32,
    /// Minimap texture and when it was last rebuilt.
    minimap: Option<egui::TextureHandle>,
    minimap_age: f32,
    /// Minimap pixels being rebuilt a few rows per frame, and the next row to do.
    minimap_px: Vec<egui::Color32>,
    minimap_row: usize,
    /// Average colour of each block's top texture (minimap).
    block_colors: Vec<[u8; 3]>,
}

impl App {
    pub fn new(args: LaunchArgs) -> Self {
        let data_dir = args.resolved_data_dir();
        let worlds_dir = data_dir.join("worlds");
        let _ = std::fs::create_dir_all(&worlds_dir);
        let mut settings = Settings::load(&data_dir);
        if cfg!(target_os = "android") {
            // Phones: a shorter view keeps the frame rate up.
            settings.render_distance = settings.render_distance.min(5);
        }
        let (preview_tx, preview_rx) = channel();
        let (create_tx, create_rx) = channel();
        let random_seed = splitmix(platform::unix_now() ^ (std::process::id() as u64) << 20) % 1_000_000_000;
        Self {
            args,
            data_dir,
            worlds_dir,
            settings,
            gfx: None,
            screen: Screen::Main,
            nav: Vec::new(),
            world_select: WorldSelectState::default(),
            create: CreateWorldState::default(),
            worlds: Vec::new(),
            preview_cache: HashMap::new(),
            preview_tx,
            preview_rx,
            preview_requested: HashSet::new(),
            create_preview: None,
            create_preview_key: 0,
            create_preview_inflight: None,
            create_tx,
            create_rx,
            random_seed,
            load: None,
            session: None,
            inv_ui: InventoryUiState::default(),
            chat: ChatState::default(),
            fov_fx: 0.0,
            input: InputState::default(),
            touch: TouchState::default(),
            time: Time::new(),
            cpu: CpuMeter::default(),
            profile: CpuProfile::default(),
            debug: false,
            grabbed: false,
            pending_meshes: VecDeque::new(),
            next_frame: Instant::now(),
            quit: false,
            frames: 0,
            last_cpu: 0.0,
            near_table: false,
            container: None,
            pending_apply: false,
            hand_cycle: 0.0,
            hand_guard: 0.0,
            hand_click: 0.0,
            hand_block: 0.0,
            hand_place: 0.0,
            hand_switch: 0.0,
            last_hotbar: 0,
            audio: crate::audio::Audio::new(),
            tile_colors: wolfcraft_assets::procedural::sprites(wolfcraft_assets::AtlasKind::Blocks)
                .iter()
                .map(|(_, img)| {
                    let px = |x: u32, y: u32| {
                        let p = img.get(x.min(img.width.saturating_sub(1)), y.min(img.height.saturating_sub(1)));
                        [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0]
                    };
                    [px(3, 4), px(9, 7), px(12, 12)]
                })
                .collect(),
            prev_hand_cycle: 0.0,
            net: None,
            host: None,
            net_players: Vec::new(),
            net_edits: Default::default(),
            net_send_t: 0.0,
            net_hell: false,
            last_step: 0,
            last_health: 20.0,
            minimap: None,
            minimap_age: 10.0,
            minimap_px: vec![egui::Color32::from_rgb(12, 14, 18); 64 * 64],
            minimap_row: 0,
            block_colors: Vec::new(),
        }
    }

    // ---------------------------------------------------------------- setup

    fn create_gfx(&mut self, event_loop: &ActiveEventLoop) -> Result<Gfx, String> {
        let attrs = Window::default_attributes()
            .with_title("WolfCraft")
            .with_inner_size(LogicalSize::new(1280.0, 720.0))
            .with_min_inner_size(LogicalSize::new(900.0, 560.0));
        let window = Arc::new(event_loop.create_window(attrs).map_err(|e| format!("Не удалось создать окно: {e}"))?);
        if self.settings.fullscreen {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        let gpu = GpuContext::new(window.clone(), self.settings.vsync, false)?;
        let atlases = AtlasManager::new(Some(&self.data_dir.join("resourcepacks")))?;
        // Export the generated atlases once so artists can see and override them.
        let export_dir = self.data_dir.join("atlases");
        if !export_dir.join("blocks_atlas.png").exists() {
            if let Err(e) = atlases.export_all(&export_dir) {
                log::warn!("atlas export failed: {e}");
            }
        }
        let mut gui = GuiRenderer::new(&gpu.device, gpu.config.format, &window, &atlases);
        theme_manager::install(&gui.ctx);
        theme_manager::apply(&gui.ctx, self.settings.theme);
        gui.set_ui_scale(self.settings.ui_scale * ui_mult());
        if cfg!(target_os = "android") {
            if let Ok(mut i) = wolfcraft_ui::gui_renderer::SAFE_INSETS.write() {
                *i = [12.0, 0.0, 12.0, 6.0];
            }
        }
        let mut world = WorldRenderer::new(&gpu.device, &gpu.queue, gpu.config.format, &atlases, gpu.config.width, gpu.config.height);
        if cfg!(target_os = "android") {
            // The phone GPU is fill-rate bound: draw the world smaller and stretch it.
            world.set_render_scale(0.7);
        }
        Ok(Gfx { window, gpu, gui, atlases, world })
    }

    fn set_grab(&mut self, gfx_window: &Window, grab: bool) {
        if grab == self.grabbed {
            return;
        }
        if grab {
            let r = gfx_window.set_cursor_grab(CursorGrabMode::Confined).or_else(|_| gfx_window.set_cursor_grab(CursorGrabMode::Locked));
            if let Err(e) = r {
                log::warn!("cursor grab failed: {e}");
            }
            gfx_window.set_cursor_visible(false);
        } else {
            let _ = gfx_window.set_cursor_grab(CursorGrabMode::None);
            gfx_window.set_cursor_visible(true);
        }
        self.grabbed = grab;
        self.input.mouse_dx = 0.0;
        self.input.mouse_dy = 0.0;
    }

    fn go(&mut self, s: Screen) {
        self.screen = s;
    }

    fn push(&mut self, s: Screen) {
        self.nav.push(self.screen);
        self.screen = s;
    }

    fn pop(&mut self) {
        self.screen = self.nav.pop().unwrap_or(Screen::Main);
    }

    fn refresh_worlds(&mut self, ctx: &egui::Context) {
        let list = list_worlds(&self.worlds_dir);
        self.worlds = list
            .into_iter()
            .map(|cfg| {
                let preview = self.preview_cache.get(&cfg.id).cloned();
                if preview.is_none() && self.preview_requested.insert(cfg.id.clone()) {
                    let tx = self.preview_tx.clone();
                    let c = cfg.clone();
                    rayon::spawn(move || {
                        let img = preview::render_globe(&c, 128);
                        let _ = tx.send((c.id.clone(), img));
                    });
                }
                WorldEntry { cfg, preview }
            })
            .collect();
        let _ = ctx;
    }

    fn start_load(&mut self, cfg: WorldConfig, is_new: bool) {
        let title = format!("«{}»", cfg.name);
        let shared = loader::start(LoadRequest { cfg, worlds_dir: self.worlds_dir.clone(), is_new });
        self.load = Some(LoadState { shared, title, error: None, started: Instant::now() });
        self.session = None;
        self.nav.clear();
        self.screen = Screen::Loading;
    }

    /// Connects to a WolfServer and loads its world (generated locally from the server settings).
    fn join_server(&mut self, addr: &str) {
        let nick = if self.settings.nickname.trim().is_empty() { "Игрок" } else { self.settings.nickname.trim() };
        let client = match wolfcraft_net::Client::connect(addr, nick, self.settings.skin) {
            Ok(c) => c,
            Err(e) => {
                self.world_select.net_error = Some(e);
                return;
            }
        };
        let mut cfg = match WorldConfig::from_toml(&client.world_toml) {
            Ok(c) => c,
            Err(e) => {
                self.world_select.net_error = Some(format!("настройки мира сервера не прочитаны: {e}"));
                return;
            }
        };
        // Local cache of the server world: the terrain comes from the seed, the edits from the
        // server, so old cached regions are dropped (the inventory in player.dat stays).
        let online = self.data_dir.join("online");
        cfg.id = format!("srv_{}", wolfcraft_engine::config::slugify(&wolfcraft_net::with_port(addr)));
        let dir = online.join(&cfg.id);
        let _ = std::fs::remove_dir_all(dir.join("regions"));
        let _ = std::fs::remove_dir_all(dir.join("hell"));
        let _ = std::fs::remove_file(dir.join("dimension.txt"));
        let title = format!("«{}»", cfg.name);
        let shared = loader::start(LoadRequest { cfg, worlds_dir: online, is_new: false });
        self.load = Some(LoadState { shared, title, error: None, started: Instant::now() });
        self.session = None;
        self.nav.clear();
        self.screen = Screen::Loading;
        let now = self.time.elapsed() as f64;
        self.chat.push(ChatKind::System, client.motd.clone(), now);
        self.net = Some(client);
        self.net_players.clear();
        self.net_edits = Default::default();
        self.net_hell = false;
    }

    /// Starts a server on this device (its own world under `hosted/`) and joins it.
    fn host_server(&mut self) {
        self.host = None;
        let dir = self.data_dir.join("hosted").join("world");
        let cfg = match WorldConfig::load(&dir) {
            Ok(c) => c,
            Err(_) => {
                let mut c = WorldConfig::new("Сетевой мир", platform::unix_now().wrapping_mul(6364136223846793005));
                c.id = "world".into();
                if let Err(e) = c.save(&dir) {
                    self.world_select.net_error = Some(format!("не удалось создать мир сервера: {e}"));
                    return;
                }
                c
            }
        };
        let toml = match cfg.to_toml() {
            Ok(t) => t,
            Err(e) => {
                self.world_select.net_error = Some(e);
                return;
            }
        };
        let port = wolfcraft_net::DEFAULT_PORT;
        match wolfcraft_net::server::start(&cfg.name, toml, &dir, port, false) {
            Ok(h) => self.host = Some(h),
            Err(e) => {
                self.world_select.net_error = Some(format!("сервер не запущен: {e}"));
                return;
            }
        }
        self.join_server(&format!("127.0.0.1:{port}"));
        if self.net.is_none() {
            self.host = None;
            return;
        }
        let now = self.time.elapsed() as f64;
        for a in wolfcraft_net::server::lan_addresses(port) {
            self.chat.push(ChatKind::System, format!("Сервер запущен. Адрес для друзей: {a}"), now);
        }
    }

    /// Network step while a session exists: receive edits/players/chat, send our state.
    fn net_tick(&mut self, dt: f32) {
        let (Some(net), Some(s)) = (&mut self.net, &mut self.session) else { return };
        use wolfcraft_engine::block::Block;
        use wolfcraft_engine::cubesphere::Cell;
        let now = self.time.elapsed() as f64;
        let to_cell = |c: [i32; 4]| Cell::new(c[0] as u8, c[1], c[2], c[3]);
        let mut kicked = None;
        for m in net.poll() {
            match m {
                wolfcraft_net::S2C::Edits { hell, list } => {
                    for (c, b) in list {
                        self.net_edits[hell as usize].insert(c, b);
                        if hell == s.hell {
                            s.remote_edits.push((to_cell(c), Block::from_u8(b)));
                        }
                    }
                }
                wolfcraft_net::S2C::Edit { cell, block, hell } => {
                    self.net_edits[hell as usize].insert(cell, block);
                    if hell == s.hell {
                        s.remote_edits.push((to_cell(cell), Block::from_u8(block)));
                    }
                }
                wolfcraft_net::S2C::Players(list) => self.net_players = list.into_iter().filter(|p| p.id != net.id).collect(),
                wolfcraft_net::S2C::Chat { from, text } => {
                    if from.is_empty() {
                        self.chat.push(ChatKind::System, text, now);
                    } else {
                        self.chat.push(ChatKind::Player, format!("<{from}> {text}"), now);
                    }
                }
                wolfcraft_net::S2C::Kick(reason) => kicked = Some(reason),
                wolfcraft_net::S2C::Welcome { .. } | wolfcraft_net::S2C::Ready => {}
            }
        }
        // After a dimension change every known edit of the new dimension applies again.
        if s.hell != self.net_hell {
            self.net_hell = s.hell;
            s.remote_edits = self.net_edits[s.hell as usize].iter().map(|(c, b)| (to_cell(*c), Block::from_u8(*b))).collect();
        }
        self.net_send_t += dt;
        if self.net_send_t >= 0.05 {
            self.net_send_t = 0.0;
            let p = &s.player;
            net.send(&wolfcraft_net::C2S::State(wolfcraft_net::PlayerState {
                id: net.id,
                nick: String::new(),
                skin: self.settings.skin,
                pos: p.body.pos.to_array(),
                heading: p.heading.to_array(),
                pitch: p.pitch as f32,
                phase: p.anim_phase,
                amount: p.anim_amount,
                sneak: p.sneaking,
                swim: p.body.in_water && !p.body.on_ground && !p.body.flying,
                hell: s.hell,
            }));
        }
        if let Some(reason) = kicked {
            self.chat.push(ChatKind::Error, format!("Отключено от сервера: {reason}"), now);
            self.net = None;
            self.net_players.clear();
        } else if !net.is_alive() {
            self.chat.push(ChatKind::Error, "Соединение с сервером потеряно. Мир продолжает работать локально.", now);
            self.net = None;
            self.net_players.clear();
        }
    }

    fn leave_world(&mut self) {
        self.net = None;
        self.host = None;
        self.net_players.clear();
        if let Some(mut s) = self.session.take() {
            s.save();
        }
        self.pending_meshes.clear();
        if let Some(g) = &mut self.gfx {
            g.world.clear();
        }
        self.nav.clear();
        self.screen = Screen::Main;
    }

    fn apply_settings(&mut self, gfx: &mut Gfx) {
        gfx.gui.set_ui_scale(self.settings.ui_scale * ui_mult());
        theme_manager::apply(&gfx.gui.ctx, self.settings.theme);
        gfx.gpu.set_vsync(self.settings.vsync);
        let fs = gfx.window.fullscreen().is_some();
        if fs != self.settings.fullscreen {
            gfx.window.set_fullscreen(if self.settings.fullscreen { Some(Fullscreen::Borderless(None)) } else { None });
        }
        self.settings.save(&self.data_dir);
    }

    // ---------------------------------------------------------------- input

    fn handle_key(&mut self, code: KeyCode, pressed: bool, typing: bool) {
        let chat_open = self.chat.open && self.screen == Screen::Playing;
        if !pressed {
            self.input.keys.remove(&code);
            return;
        }
        // While typing (chat or a text field) only Esc and F11 act; movement keys must not stick.
        if chat_open || (typing && self.screen != Screen::Playing) {
            if code == KeyCode::Escape && chat_open {
                self.chat.close();
            } else if code == KeyCode::F11 {
                self.settings.fullscreen = !self.settings.fullscreen;
                self.pending_apply = true;
            }
            return;
        }
        self.input.keys.insert(code);
        match (self.screen, code) {
            (_, KeyCode::F11) => {
                self.settings.fullscreen = !self.settings.fullscreen;
                self.pending_apply = true;
            }
            (Screen::Playing, KeyCode::Escape) => self.screen = Screen::Paused,
            (Screen::Paused, KeyCode::Escape) => self.screen = Screen::Playing,
            (Screen::Playing, KeyCode::KeyE) => {
                self.inv_ui = InventoryUiState::default();
                self.screen = Screen::Inventory;
            }
            (Screen::Inventory, KeyCode::KeyE) => {
                if let Some(s) = &mut self.session {
                    inventory_ui::close(&mut self.inv_ui, &mut s.player.inventory);
                }
                self.container = None;
                self.screen = Screen::Playing;
            }
            (Screen::Playing, KeyCode::KeyT) => {
                self.input.keys.clear();
                self.chat.open("");
            }
            (Screen::Playing, KeyCode::Slash) => {
                self.input.keys.clear();
                self.chat.open("/");
            }
            (Screen::Playing, KeyCode::F5) => {
                if let Some(s) = &mut self.session {
                    s.cycle_camera();
                    let now = self.time.elapsed() as f64;
                    self.chat.push(ChatKind::System, s.camera_mode.label(), now);
                }
            }
            (Screen::Playing | Screen::Paused | Screen::Inventory, KeyCode::F3 | KeyCode::Tab) => self.debug = !self.debug,
            (Screen::Playing, KeyCode::Space) => self.input.jump_press = true,
            (Screen::Playing, KeyCode::KeyF) => self.input.swap_press = true,
            (Screen::Playing, c) => {
                let digit = match c {
                    KeyCode::Digit1 => Some(0),
                    KeyCode::Digit2 => Some(1),
                    KeyCode::Digit3 => Some(2),
                    KeyCode::Digit4 => Some(3),
                    KeyCode::Digit5 => Some(4),
                    KeyCode::Digit6 => Some(5),
                    KeyCode::Digit7 => Some(6),
                    KeyCode::Digit8 => Some(7),
                    KeyCode::Digit9 => Some(8),
                    _ => None,
                };
                if digit.is_some() {
                    self.input.hotbar_set = digit;
                }
            }
            _ => {}
        }
    }

    fn take_input_frame(&mut self) -> InputFrame {
        let k = &self.input.keys;
        let axis = |pos: KeyCode, neg: KeyCode| (k.contains(&pos) as i32 - k.contains(&neg) as i32) as f32;
        let sens = 0.0022 * self.settings.mouse_sensitivity as f64;
        self.touch.tick();
        let tl = self.touch.take_look();
        let k = &self.input.keys;
        let f = InputFrame {
            forward: (axis(KeyCode::KeyW, KeyCode::KeyS) + self.touch.move_vec.1).clamp(-1.0, 1.0),
            strafe: (axis(KeyCode::KeyD, KeyCode::KeyA) + self.touch.move_vec.0).clamp(-1.0, 1.0),
            jump: k.contains(&KeyCode::Space) || self.touch.jump_held,
            jump_pressed: std::mem::take(&mut self.input.jump_press),
            sprint: k.contains(&KeyCode::ShiftLeft) || k.contains(&KeyCode::ShiftRight),
            sneak: k.contains(&KeyCode::ControlLeft) || k.contains(&KeyCode::KeyC) || self.touch.sneak,
            look: ((self.input.mouse_dx + tl.0 as f64 * 1.4) * sens, if self.settings.invert_y { 1.0 } else { -1.0 } * (self.input.mouse_dy + tl.1 as f64 * 1.4) * sens),
            break_held: self.input.left || self.touch.breaking,
            use_pressed: std::mem::take(&mut self.input.right_press),
            use_held: self.input.right,
            swap_hands: std::mem::take(&mut self.input.swap_press),
            pick_pressed: std::mem::take(&mut self.input.middle_press),
            hotbar_delta: std::mem::take(&mut self.input.scroll) as i32,
            hotbar_set: self.input.hotbar_set.take(),
        };
        self.input.mouse_dx = 0.0;
        self.input.mouse_dy = 0.0;
        f
    }

    // ---------------------------------------------------------------- frame

    fn pump_meshes(&mut self, gfx: &mut Gfx, budget: usize) {
        if let Some(s) = &mut self.session {
            for (cc, m) in s.chunks.take_meshes() {
                self.pending_meshes.push_back((cc, m));
            }
            for cc in s.chunks.take_removed() {
                gfx.world.remove_chunk(cc);
            }
            if s.dimension_changed {
                s.dimension_changed = false;
                gfx.world.clear();
                self.pending_meshes.clear();
            }
        }
        for _ in 0..budget {
            if let Some(s) = &self.session {
                gfx.world.set_grid(s.chunks.grid());
            }
            let Some((cc, m)) = self.pending_meshes.pop_front() else { break };
            gfx.world.upload_mesh(&gfx.gpu.device, &gfx.gpu.queue, cc, m);
        }
    }

    fn poll_previews(&mut self, ctx: &egui::Context) {
        while let Ok((id, img)) = self.preview_rx.try_recv() {
            let tex = ctx.load_texture(format!("preview-{id}"), img, egui::TextureOptions::LINEAR);
            self.preview_cache.insert(id.clone(), tex.clone());
            for w in &mut self.worlds {
                if w.cfg.id == id {
                    w.preview = Some(tex.clone());
                }
            }
        }
        while let Ok((key, img)) = self.create_rx.try_recv() {
            if self.create_preview_inflight == Some(key) {
                self.create_preview_inflight = None;
            }
            if key == self.create_preview_key {
                self.create_preview = Some(ctx.load_texture("create-preview", img, egui::TextureOptions::LINEAR));
            }
        }
        // (Re)start the create-world preview when a planet setting changed.
        if self.screen == Screen::Create {
            let st = &self.create;
            let mut h = st.effective_seed();
            h = splitmix(h ^ (st.size as u64 + 1) * 31 ^ (st.world_type as u64 + 1) * 131 ^ (st.continents as u64 + 1) * 1031);
            if h != self.create_preview_key {
                self.create_preview_key = h;
            }
            if self.create_preview_inflight != Some(h) && (self.create.preview_dirty || self.create_preview.is_none() || self.create_preview_inflight.is_none()) {
                let mut cfg = WorldConfig::new("preview", st.effective_seed());
                cfg.size = st.size;
                cfg.world_type = st.world_type;
                cfg.continents = st.continents;
                self.create_preview_inflight = Some(h);
                self.create.preview_dirty = false;
                let tx = self.create_tx.clone();
                rayon::spawn(move || {
                    let img = preview::render_globe(&cfg, 300);
                    let _ = tx.send((h, img));
                });
            }
        }
    }

    fn debug_info(&self, s: &GameSession, gfx: &Gfx) -> DebugInfo {
        let p = &s.player;
        let up = p.up();
        let r = p.body.pos.length();
        let surf = s.planet.surface_radius(up);
        let col = s.planet.column(up);
        let north = (DVec3::Y - up * DVec3::Y.dot(up)).normalize_or_zero();
        let east = north.cross(up);
        let ang = p.heading.dot(east).atan2(p.heading.dot(north)).to_degrees().rem_euclid(360.0);
        let names = ["С", "СВ", "В", "ЮВ", "Ю", "ЮЗ", "З", "СЗ"];
        let facing = format!("{} ({:.0}°)", names[((ang + 22.5) / 45.0) as usize % 8], ang);
        DebugInfo {
            fps: self.time.fps(),
            frame_ms_avg: self.time.frame_ms_avg(),
            frame_ms_max: self.time.frame_ms_max(),
            cpu_percent: self.last_cpu,
            cpu_cores: self.cpu.cores(),
            ram_mb: platform::process_memory_bytes() as f32 / 1048576.0,
            gpu_name: gfx.gpu.adapter_info.name.clone(),
            backend: format!("{:?}", gfx.gpu.adapter_info.backend),
            pos: [p.body.pos.x, p.body.pos.y, p.body.pos.z],
            block: {
                let b = p.body.pos.floor().as_ivec3();
                [b.x, b.y, b.z]
            },
            chunk: {
                let c = s.player_chunk();
                [c.ci, c.cj, c.ck]
            },
            altitude: r - s.planet.radius,
            latitude: up.y.asin().to_degrees(),
            longitude: up.z.atan2(up.x).to_degrees(),
            biome: col.biome.label().to_string(),
            layer: s.planet.layer_at(r, surf).label().to_string(),
            facing,
            loaded_chunks: s.chunks.stats.loaded,
            meshed_chunks: s.chunks.stats.meshed,
            gen_inflight: s.chunks.stats.gen_inflight,
            mesh_inflight: s.chunks.stats.mesh_inflight,
            visible_chunks: gfx.world.stats.chunks_visible,
            gpu_chunks: gfx.world.chunk_count(),
            draw_calls: gfx.world.stats.draw_calls,
            triangles: gfx.world.stats.triangles,
            gpu_buffer_mb: gfx.world.stats.buffer_bytes as f32 / 1048576.0,
            gui_vertices: gfx.gui.last_vertices,
            vsync: gfx.gpu.vsync(),
            view_distance: self.settings.render_distance,
            clock: s.clock(),
            seed: s.cfg.seed,
            world_name: s.cfg.name.clone(),
            update_ms: self.profile.update_ms,
            chunks_ms: self.profile.chunks_ms,
            render_ms: self.profile.render_ms,
            ui_ms: self.profile.ui_ms,
        }
    }

    fn redraw(&mut self, gfx: &mut Gfx) {
        self.time.tick();
        let dt = self.time.delta();
        self.frames += 1;
        self.last_cpu = self.cpu.sample();
        let t_frame = ScopeTimer::start();

        // ---- non-UI logic ----
        let t_update = ScopeTimer::start();
        let view = self.settings.render_distance as i32;
        let mut deaths = false;
        match self.screen {
            Screen::Playing | Screen::Paused | Screen::Inventory | Screen::Dead => {
                let input = if self.screen == Screen::Playing && !self.chat.open { self.take_input_frame() } else { InputFrame::default() };
                self.input.mouse_dx = 0.0;
                self.input.mouse_dy = 0.0;
                if let Some(s) = &mut self.session {
                    let frozen = self.screen != Screen::Playing;
                    let t = ScopeTimer::start();
                    let events = s.update(&input, dt, view, frozen);
                    self.profile.chunks_ms = t.ms();
                    self.audio.volume = self.settings.volume;
                    // Footsteps: one per half walk-cycle while on the ground.
                    let step_no = (s.player.anim_phase / std::f32::consts::PI).floor() as i64;
                    if step_no != self.last_step {
                        self.last_step = step_no;
                        if s.player.body.on_ground && s.player.anim_amount > 0.3 && !s.player.sneaking {
                            self.audio.play(crate::audio::Sfx::Step, 0.8);
                        }
                    }
                    if s.player.survival.health < self.last_health - 0.01 {
                        self.audio.play(crate::audio::Sfx::Hurt, 1.0);
                    }
                    self.last_health = s.player.survival.health;
                    for e in events {
                        match e {
                            InteractionEvent::OpenCrafting => {
                                self.inv_ui = InventoryUiState::default();
                                self.screen = Screen::Inventory;
                                self.near_table = true;
                            }
                            InteractionEvent::OpenContainer { pos, block } if block == wolfcraft_engine::block::Block::Terminal => {
                                let _ = pos;
                                self.input.keys.clear();
                                self.chat.open("/");
                            }
                            InteractionEvent::OpenContainer { pos, block } => {
                                self.container = Some((pos, block));
                                self.inv_ui = InventoryUiState::default();
                                self.screen = Screen::Inventory;
                            }
                            InteractionEvent::Placed { pos, block } => {
                                if let Some(net) = &mut self.net {
                                    net.send(&wolfcraft_net::C2S::Edit { cell: [pos.f as i32, pos.i, pos.j, pos.k], block: block as u8, hell: s.hell });
                                }
                                self.hand_place = 1.0;
                                self.audio.play(crate::audio::Sfx::Place, 1.0);
                            }
                            InteractionEvent::Broken { pos, block } => {
                                // Final burst of chips in the colours of the block.
                                let n = (14.0 * self.settings.particles) as usize;
                                if let Some(cols) = self.tile_colors.get(block.info().tiles[0] as usize) {
                                    let corners = s.chunks.grid().cell_corners(pos);
                                    let centre = corners.iter().fold(glam::DVec3::ZERO, |a, c| a + *c) / 8.0;
                                    s.mobs.chips(centre, cols, n, 3.2);
                                }
                                if let Some(net) = &mut self.net {
                                    net.send(&wolfcraft_net::C2S::Edit { cell: [pos.f as i32, pos.i, pos.j, pos.k], block: 0, hell: s.hell });
                                }
                                self.audio.play(crate::audio::Sfx::Break, 1.0);
                            }
                            InteractionEvent::Ate { .. } => self.audio.play(crate::audio::Sfx::Eat, 1.0),
                        }
                    }
                    if s.player.survival.dead && self.screen == Screen::Playing {
                        deaths = true;
                    }
                }
            }
            Screen::Loading => self.poll_loading(),
            _ => {}
        }
        if deaths {
            self.screen = Screen::Dead;
        }
        self.net_tick(self.time.delta());
        self.profile.update_ms = t_update.ms();

        // Atlas animation (water) and mesh uploads.
        if gfx.atlases.animate(self.time.elapsed()) {
            gfx.world.sync_atlas(&gfx.gpu.device, &gfx.gpu.queue, &mut gfx.atlases);
        }
        let upload_budget = if self.screen == Screen::Loading { 64 } else { 16 };
        self.pump_meshes(gfx, upload_budget);

        let want_grab = self.screen == Screen::Playing && !self.chat.open;
        if want_grab != self.grabbed {
            let w = gfx.window.clone();
            self.set_grab(&w, want_grab);
        }
        self.poll_previews(&gfx.gui.ctx);
        if self.screen == Screen::WorldSelect && self.worlds.iter().any(|w| w.preview.is_none()) {
            // Attach freshly cached previews.
            for w in &mut self.worlds {
                if w.preview.is_none() {
                    w.preview = self.preview_cache.get(&w.cfg.id).cloned();
                }
            }
        }

        // ---- UI ----
        let t_ui = ScopeTimer::start();
        let mut actions: Vec<MenuAction> = Vec::new();
        let mut settings_changed = false;
        let mut settings_back = false;
        let mut close_inventory = false;
        let mut submitted: Vec<String> = Vec::new();
        let icons = gfx.gui.icons.clone();
        let version = wolfcraft_common::VERSION;
        let data_dir_str = self.data_dir.display().to_string();
        let debug_info = if self.debug && self.screen.in_world() { self.session.as_ref().map(|s| self.debug_info(s, gfx)) } else { None };
        let screen = self.screen;
        let dark = self.settings.theme == ThemeMode::Dark;

        gfx.gui.hide_cursor = self.grabbed;
        // Minimap: rebuilt 4 times a second while in the world.
        self.minimap_age += dt;
        if self.screen.in_world() && self.minimap_age > 0.05 {
            self.minimap_age = 0.0;
            if self.block_colors.is_empty() {
                self.block_colors = block_top_colors(&gfx.atlases);
            }
            if let Some(s) = &self.session {
                // 8 rows per step keeps each frame cheap; the texture updates when all 64 are done.
                build_minimap_rows(s, &self.block_colors, self.minimap_row..self.minimap_row + 8, &mut self.minimap_px);
                self.minimap_row += 8;
                if self.minimap_row >= 64 {
                    self.minimap_row = 0;
                    let img = egui::ColorImage { size: [64, 64], pixels: self.minimap_px.clone() };
                    match &mut self.minimap {
                        Some(t) => t.set(img, egui::TextureOptions::NEAREST),
                        None => self.minimap = Some(gfx.gui.ctx.load_texture("minimap", img, egui::TextureOptions::NEAREST)),
                    }
                }
            }
        }
        let minimap_tex = self.minimap.as_ref().map(|t| t.id());
        let frame = {
            let App { settings, world_select, create, worlds, create_preview, session, inv_ui, load, near_table, time, chat, container, touch, .. } = self;
            let touch_size = gfx.gpu.size();
            let random_seed = create.preview_seed;
            let ctx_dt = dt;
            gfx.gui.run(&gfx.window, |ctx| match screen {
                Screen::Main => actions.push(main_menu::main_menu(ctx, &icons, version, &data_dir_str, dark)),
                Screen::WorldSelect => actions.push(main_menu::world_select(ctx, &icons, world_select, worlds, version)),
                Screen::Create => actions.push(main_menu::create_world(ctx, &icons, create, create_preview.as_ref(), version, random_seed)),
                Screen::Settings => {
                    let r = main_menu::settings_screen(ctx, &icons, settings, version);
                    settings_changed |= r.changed;
                    settings_back |= r.back;
                }
                Screen::Loading => {
                    let stages: Vec<String> = STAGES.iter().map(|s| s.to_string()).collect();
                    if let Some(l) = load {
                        if let Some(err) = &l.error {
                            let mut back = false;
                            let p = theme_manager::palette(ctx);
                            let dismissed = wolfcraft_ui::ui_components::modal(ctx, "load-error", "Не удалось открыть мир", 460.0, |ui| {
                                ui.label(egui::RichText::new(err).color(p.danger));
                                ui.add_space(8.0);
                                if wolfcraft_ui::ui_components::pill_button(ui, &icons, Some(wolfcraft_assets::gui::ARROW_LEFT), "В меню", wolfcraft_ui::ui_components::ButtonKind::Primary, 0.0, true).clicked() {
                                    back = true;
                                }
                            });
                            if back || dismissed {
                                actions.push(MenuAction::Back);
                            }
                        } else {
                            let (cur, frac, detail) = loading_view(l, session.as_ref());
                            let grid = session.as_ref().map(|s| s.chunks.progress_grid(s.player_chunk(), 5)).unwrap_or_default();
                            main_menu::loading_screen(ctx, &icons, &LoadingView { title: &format!("Мир {}", l.title), stages: &stages, current: cur, fraction: frac, detail: &detail, grid: &grid });
                        }
                    }
                }
                Screen::Playing | Screen::Paused | Screen::Inventory | Screen::Dead => {
                    if touch.seen && screen == Screen::Playing {
                        touch.sync_hotbar(ctx.pixels_per_point());
                        touch.draw(&ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("touch"))), touch_size, ctx.pixels_per_point());
                    }
                    if let Some(s) = session {
                        let p = &s.player;
                        let hint = s.hit.map(|h| {
                            if s.is_creative() { format!("{} · ЛКМ — сломать · ПКМ — поставить", h.block.name()) } else { format!("{} · ЛКМ — копать · ПКМ — поставить", h.block.name()) }
                        });
                        let hud = HudState {
                            coords: [p.body.pos.x, p.body.pos.y, p.body.pos.z],
                            fps: time.fps(),
                            clock: s.clock(),
                            biome: s.planet.column(p.up()).biome.label().to_string(),
                            inventory: &p.inventory,
                            health: p.survival.health,
                            hunger: p.survival.hunger,
                            xp_level: p.survival.level(),
                            xp_progress: p.survival.level_progress(),
                            air: p.survival.air,
                            underwater: p.body.head_in_water,
                            hurt: p.survival.hurt_flash,
                            creative: s.is_creative(),
                            flying: p.body.flying,
                            hint: if screen == Screen::Playing { hint } else { None },
                            mining: s.interaction.mining_fraction(),
                            item_name: s.item_name.clone(),
                            show_fps: settings.show_fps,
                            debug: debug_info.as_ref(),
                            compass: Some(compass_bearing(p.heading, p.up()) as f32),
                            minimap: minimap_tex,
                            compact: cfg!(target_os = "android"),
                        };
                        hud::draw(ctx, &icons, &hud);
                        if screen == Screen::Playing {
                            if let Some(line) = chat_ui::draw(ctx, &icons, chat, time.elapsed() as f64) {
                                submitted.push(line);
                            }
                        }
                        if s.saved_notice > 0.0 {
                            ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("saved")))
                                .text(egui::pos2(ctx.screen_rect().right() - 18.0, 28.0), egui::Align2::RIGHT_CENTER, "Сохранено", egui::FontId::proportional(14.0), theme_manager::palette(ctx).success);
                        }
                    }
                    match screen {
                        Screen::Paused => {
                            if let Some(s) = session.as_ref() {
                                let info = PauseInfo {
                                    world_name: s.cfg.name.clone(),
                                    mode: if s.is_creative() { "Творческий".into() } else { "Выживание".into() },
                                    clock: s.clock(),
                                    biome: s.planet.column(s.player.up()).biome.label().to_string(),
                                    seed: s.cfg.seed as i64,
                                    coords: [s.player.body.pos.x, s.player.body.pos.y, s.player.body.pos.z],
                                    camera: s.camera_mode.label().to_string(),
                                };
                                actions.push(main_menu::pause_menu(ctx, &icons, &info));
                            }
                        }
                        Screen::Dead => actions.push(main_menu::death_screen(ctx, &icons)),
                        Screen::Inventory => {
                            if let Some(s) = session {
                                if let Some((pos, wolfcraft_engine::block::Block::CommandBlock)) = *container {
                                    let mut cmd = s.block_commands.get(&pos).cloned().unwrap_or_default();
                                    let mut run = false;
                                    let dismissed = wolfcraft_ui::ui_components::modal(ctx, "cmdblock", "Командный блок", 480.0, |ui| {
                                        ui.label("Команда (как в чате, например /time set день):");
                                        ui.add(egui::TextEdit::singleline(&mut cmd).desired_width(f32::INFINITY));
                                        ui.add_space(8.0);
                                        ui.horizontal(|ui| {
                                            if wolfcraft_ui::ui_components::pill_button(ui, &icons, None, "Выполнить", wolfcraft_ui::ui_components::ButtonKind::Primary, 140.0, true).clicked() {
                                                run = true;
                                            }
                                        });
                                    });
                                    s.block_commands.insert(pos, cmd.clone());
                                    if run && !cmd.trim().is_empty() {
                                        let c = cmd.trim();
                                        submitted.push(if c.starts_with('/') { c.to_string() } else { format!("/{c}") });
                                    }
                                    close_inventory |= dismissed || run;
                                } else if let Some((pos, block)) = *container {
                                    let furnace = block == wolfcraft_engine::block::Block::ElectricFurnace;
                                    let (prog, powered) = s.furnace_status(pos);
                                    let status_text = if powered { "Плавка" } else { "Нет питания" };
                                    let accepts = |slot: usize, st: &wolfcraft_engine::items::ItemStack| !furnace || (slot == 0 && wolfcraft_engine::interaction::smelt_result(st.id).is_some());
                                    let view = wolfcraft_ui::container_ui::ContainerView {
                                        title: if furnace { "Электропечь" } else { "Сундук" },
                                        columns: if furnace { 2 } else { 9 },
                                        accepts: &accepts,
                                        status: if furnace { Some((status_text, prog)) } else { None },
                                    };
                                    let mut slots = std::mem::take(s.container_slots(pos, block));
                                    let closed = wolfcraft_ui::container_ui::show(ctx, &icons, &view, &mut slots, &mut s.player.inventory);
                                    *s.container_slots(pos, block) = slots;
                                    close_inventory |= closed;
                                } else {
                                    let creative = s.is_creative();
                                    let r = inventory_ui::show(ctx, &icons, inv_ui, &mut s.player.inventory, creative, *near_table, ctx_dt);
                                    close_inventory |= r.close;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            })
        };
        self.profile.ui_ms = t_ui.ms();

        // ---- apply UI results ----
        if settings_changed {
            self.apply_settings(gfx);
        }
        if settings_back {
            self.apply_settings(gfx);
            self.pop();
        }
        if close_inventory {
            self.screen = Screen::Playing;
            self.near_table = false;
            self.container = None;
        }
        for a in actions {
            self.handle_action(a, gfx);
        }
        let now = self.time.elapsed() as f64;
        for line in submitted {
            // Online: plain chat lines go to the server, which sends them back to everyone.
            if let Some(net) = &mut self.net {
                if !line.starts_with('/') {
                    net.send(&wolfcraft_net::C2S::Chat(line.clone()));
                    continue;
                }
            }
            if let Some(s) = &mut self.session {
                s.nickname = if self.settings.nickname.trim().is_empty() { "Игрок".to_string() } else { self.settings.nickname.trim().to_string() };
                for (kind, text) in crate::commands::run(s, &line) {
                    self.chat.push(kind, text, now);
                }
            }
        }

        // ---- render ----
        let t_render = ScopeTimer::start();
        let Some(output) = gfx.gpu.acquire() else { return };
        let view_tex = output.texture.create_view(&Default::default());
        let mut encoder = gfx.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        let (w, h) = gfx.gpu.size();
        gfx.world.resize(&gfx.gpu.device, w, h);
        if self.screen.in_world() || (self.screen == Screen::Loading && self.session.is_some()) {
            if let Some(s) = &mut self.session {
                let up = s.player.up();
                let sun = s.sun();
                let day = s.daylight();
                let d = up.dot(sun);
                let twilight = (-(d / 0.18).powi(2)).exp() as f32;
                let r = s.player.eye_pos().length();
                let altitude = ((r - s.planet.radius - 300.0) / (s.planet.radius * 0.4)).clamp(0.0, 1.0) as f32;
                let underwater = s.eye_in_water();
                let mut sky = sky_color(day, twilight, altitude);
                if underwater {
                    sky = [0.04, 0.18, 0.36];
                } else if s.hell {
                    sky = [0.30, 0.05, 0.02];
                }
                let cam = s.camera();
                // Smooth FOV kick: wider while sprinting or flying fast, a touch narrower underwater.
                let fx_target = if s.player.sprinting { 9.0 } else if s.player.body.flying { 4.0 } else if underwater { -4.0 } else { 0.0 };
                self.fov_fx += (fx_target - self.fov_fx) * (1.0 - (-dt as f32 * 7.0).exp());
                let params = FrameParams {
                    eye: cam.eye,
                    forward: cam.forward.as_vec3(),
                    up: cam.up.as_vec3(),
                    fov_y: (self.settings.fov + self.fov_fx).to_radians(),
                    aspect: w as f32 / h.max(1) as f32,
                    sun_dir: sun.as_vec3(),
                    day,
                    underwater,
                    view_distance_blocks: (self.settings.render_distance * 16) as f32,
                    time: self.time.elapsed(),
                    outline: if self.screen == Screen::Playing { s.hit.map(|h| s.chunks.grid().cell_corners(h.cell)) } else { None },
                    crack: if self.screen == Screen::Playing { s.interaction.mining_fraction() } else { 0.0 },
                    viewmodel_fov_y: self.settings.vm_fov.to_radians(),
                };
                // Character model is visible in third-person views only.
                let mut model_verts = wolfcraft_engine::renderer::model::build_mob_models(&s.mobs.mobs, cam.eye);
                if s.camera_mode != crate::session::CameraMode::First && !s.player.survival.dead {
                    let pose = PlayerPose {
                        feet: s.player.body.pos,
                        heading: s.player.heading,
                        up: s.player.up(),
                        pitch: s.player.pitch as f32,
                        phase: s.player.anim_phase,
                        amount: s.player.anim_amount,
                        sneaking: s.player.sneaking,
                        swimming: s.player.body.in_water && !s.player.body.on_ground && !s.player.body.flying,
                        skin: self.settings.skin,
                    };
                    model_verts.extend(build_player_model(&pose, cam.eye));
                }
                // Other players of the server (same dimension only).
                for rp in self.net_players.iter().filter(|rp| rp.hell == s.hell) {
                    let pos = glam::DVec3::from_array(rp.pos);
                    if pos.length_squared() < 1.0 {
                        continue;
                    }
                    let pose = PlayerPose { feet: pos, heading: glam::DVec3::from_array(rp.heading), up: pos.normalize(), pitch: rp.pitch, phase: rp.phase, amount: rp.amount, sneaking: rp.sneak, swimming: rp.swim, skin: rp.skin };
                    model_verts.extend(build_player_model(&pose, cam.eye));
                }
                model_verts.extend(wolfcraft_engine::renderer::model::build_debris(&s.mobs.debris, cam.eye));
                gfx.world.set_model(&gfx.gpu.device, &gfx.gpu.queue, &model_verts);
                let sky_verts = wolfcraft_engine::renderer::sky::build_sky(&wolfcraft_engine::renderer::sky::SkyParams { eye: cam.eye, sun_dir: sun.as_vec3(), day, planet_radius: s.planet.radius, sky, time: self.time.elapsed(), underwater });
                gfx.world.set_sky(&gfx.gpu.device, &gfx.gpu.queue, &sky_verts);
                // First-person arms: raise into a boxing guard and jab left/right while LMB is held.
                let first_person = s.camera_mode == crate::session::CameraMode::First && !s.player.survival.dead;
                // A short click still plays a jab.
                if self.input.left {
                    self.hand_click = 0.34;
                } else {
                    self.hand_click = (self.hand_click - dt).max(0.0);
                }
                let punching = first_person && self.screen == Screen::Playing && (self.input.left || self.hand_click > 0.0);
                let target = if punching { 1.0 } else { 0.0 };
                self.hand_guard += (target - self.hand_guard) * (1.0 - (-14.0 * dt).exp());
                if punching {
                    // 1.5 cycles/s = 3 punches per second.
                    self.hand_cycle = (self.hand_cycle + dt * 1.5).rem_euclid(1.0);
                } else if self.hand_guard < 0.02 {
                    self.hand_cycle = 0.0;
                }
                // The moment a strike lands (fists: 0.25 and 0.75 of the cycle, tools: 0.5).
                let tool_in_hand = s.player.inventory.held().and_then(|st| wolfcraft_engine::items::tool_shape(st.id)).is_some();
                let marks: &[f32] = if tool_in_hand { &[0.5] } else { &[0.25, 0.75] };
                let (c0, c1) = (self.prev_hand_cycle, self.hand_cycle);
                let landed = punching && marks.iter().any(|m| (c0 < *m && c1 >= *m) || (c1 < c0 && (c0 < *m || c1 >= *m)));
                self.prev_hand_cycle = self.hand_cycle;
                if landed && s.interaction.mining_fraction() > 0.0 {
                    if let Some(h) = s.hit {
                        let corners = s.chunks.grid().cell_corners(h.cell);
                        let centre = corners.iter().fold(glam::DVec3::ZERO, |a, c| a + *c) / 8.0;
                        let towards = (cam.eye - centre).normalize_or_zero();
                        if let Some(cols) = self.tile_colors.get(h.block.info().tiles[0] as usize) {
                            s.mobs.chips(centre + towards * 0.55, cols, (4.0 * self.settings.particles) as usize, 1.8);
                        }
                        self.audio.play(crate::audio::Sfx::Hit, 0.7);
                    }
                }
                self.hand_place = (self.hand_place - dt * 5.0).max(0.0);
                if s.player.inventory.selected != self.last_hotbar {
                    self.last_hotbar = s.player.inventory.selected;
                    self.hand_switch = 1.0;
                }
                self.hand_switch = (self.hand_switch - dt * 4.5).max(0.0);
                let block_target = if s.player.blocking { 1.0 } else { 0.0 };
                self.hand_block += (block_target - self.hand_block) * (1.0 - (-16.0 * dt).exp());
                if first_person {
                    let hands = build_first_person_hands(&HandsPose {
                        forward: cam.forward.as_vec3(),
                        up: cam.up.as_vec3(),
                        punch_cycle: self.hand_cycle,
                        guard: self.hand_guard * self.settings.hand_anim.clamp(0.4, 1.0),
                        bob_phase: s.player.anim_phase,
                        bob_amount: s.player.anim_amount,
                        holding: s.player.inventory.held().is_some(),
                        tool: s.player.inventory.held().and_then(|st| wolfcraft_engine::items::tool_shape(st.id)),
                        icon: s.player.inventory.held().and_then(|st| match st.info().icon {
                            wolfcraft_engine::items::Icon::Item(i) if st.info().place.is_none() => Some(i),
                            _ => None,
                        }),
                        shield: s.player.inventory.has_shield(),
                        blocking: self.hand_block,
                        place: self.hand_place,
                        skin: self.settings.skin,
                    });
                    // View model placement: settings, idle breathing, item-switch dip, aspect ratio.
                    let aspect = w as f32 / h.max(1) as f32;
                    let fwd = cam.forward.as_vec3();
                    let upv0 = cam.up.as_vec3();
                    let right = fwd.cross(upv0).normalize_or_zero();
                    let upv = right.cross(fwd).normalize_or_zero();
                    let t_now = self.time.elapsed();
                    let dip = (self.hand_switch * std::f32::consts::PI * 0.5).sin();
                    let ox = self.settings.vm_x + 0.03 - (1.6 - aspect).max(0.0) * 0.10 + (t_now * 1.3).cos() * 0.004;
                    let oy = self.settings.vm_y - 0.02 - dip * 0.40 + (t_now * 1.9).sin() * 0.006;
                    let sc = self.settings.vm_scale;
                    let pivot = fwd * 0.45 + right * 0.30 - upv * 0.30;
                    let xf = move |p: [f32; 3]| -> [f32; 3] {
                        let v = glam::Vec3::from(p);
                        (pivot + (v - pivot) * sc + right * ox + upv * oy).to_array()
                    };
                    let mut hands = hands;
                    for v in hands.iter_mut() {
                        v.pos = xf(v.pos);
                    }
                    gfx.world.set_hands(&gfx.gpu.device, &gfx.gpu.queue, &hands);
                    // The selected block, textured, in the right fist.
                    let held = s.player.inventory.held().and_then(|st| st.info().place).map(|b| {
                        let pose = HandsPose {
                            forward: cam.forward.as_vec3(),
                            up: cam.up.as_vec3(),
                            punch_cycle: self.hand_cycle,
                            guard: self.hand_guard * self.settings.hand_anim.clamp(0.4, 1.0),
                            bob_phase: s.player.anim_phase,
                            bob_amount: s.player.anim_amount,
                            holding: true,
                            tool: None,
                            icon: None,
                            shield: s.player.inventory.has_shield(),
                            blocking: self.hand_block,
                            place: self.hand_place,
                            skin: self.settings.skin,
                        };
                        let mut vs = build_held_block(&pose, b.info().tiles, b.info().emissive);
                        for v in vs.iter_mut() {
                            v.pos = xf(v.pos);
                        }
                        vs
                    });
                    gfx.world.set_held(&gfx.gpu.queue, held.as_deref().unwrap_or(&[]));
                } else {
                    gfx.world.set_hands(&gfx.gpu.device, &gfx.gpu.queue, &[]);
                    gfx.world.set_held(&gfx.gpu.queue, &[]);
                }
                gfx.world.render(&gfx.gpu.device, &gfx.gpu.queue, &mut encoder, &view_tex, &params, sky, sky);
            }
        } else {
            let p = theme_manager::palette(&gfx.gui.ctx);
            let c = p.bg;
            let lin = |v: u8| v as f64 / 255.0;
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("menu-clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view_tex,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: lin(c.r()), g: lin(c.g()), b: lin(c.b()), a: 1.0 }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        let extra = gfx.gui.render(&gfx.gpu.device, &gfx.gpu.queue, &mut encoder, &view_tex, [w, h], frame);
        gfx.gpu.queue.submit(extra.into_iter().chain(std::iter::once(encoder.finish())));
        output.present();
        self.profile.render_ms = t_render.ms();
        thread_local! {
            static PERF: std::cell::RefCell<(Instant, u32, f32, f32)> = std::cell::RefCell::new((Instant::now(), 0, 0.0, 0.0));
        }
        PERF.with(|p| {
            let mut p = p.borrow_mut();
            p.1 += 1;
            p.2 += t_frame.ms();
            p.3 = p.3.max(t_frame.ms());
            if p.0.elapsed().as_secs_f32() >= 2.0 {
                log::info!("perf: {:.1} fps, cpu frame avg {:.1} ms max {:.1} ms (update {:.1} chunks {:.1} render {:.1}), view {} chunks, draw {}", p.1 as f32 / p.0.elapsed().as_secs_f32(), p.2 / p.1 as f32, p.3, self.profile.update_ms, self.profile.chunks_ms, self.profile.render_ms, self.settings.render_distance, gfx.world.stats.draw_calls);
                *p = (Instant::now(), 0, 0.0, 0.0);
            }
        });
    }

    fn poll_loading(&mut self) {
        let Some(l) = &mut self.load else { return };
        if l.error.is_some() {
            return;
        }
        if self.session.is_none() && l.shared.is_done() {
            match l.shared.take_result() {
                Some(Ok(loaded)) => {
                    log::info!("world ready after {:.2}s", l.started.elapsed().as_secs_f32());
                    self.pending_meshes.clear();
                    let session = GameSession::new(loaded);
                    self.settings_world_hint(&session);
                    self.session = Some(session);
                    if let Some(g) = &mut self.gfx {
                        g.world.clear();
                    }
                }
                Some(Err(e)) => l.error = Some(e),
                None => l.error = Some("Загрузка мира завершилась без результата".into()),
            }
        }
        if let Some(s) = &mut self.session {
            // Stream the starting area; enter the world once the ground around the player is meshed.
            let view = (self.settings.render_distance as i32).clamp(3, 6);
            s.update(&InputFrame::default(), self.time.delta(), view, true);
            let (done, total) = s.chunks.progress(s.player_chunk(), 3);
            if done >= total && s.ground_ready() {
                self.load = None;
                self.screen = Screen::Playing;
                self.near_table = false;
            }
        }
    }

    fn settings_world_hint(&self, s: &GameSession) {
        log::info!("session started: {} (seed {}, {:?}, {:?})", s.cfg.name, s.cfg.seed, s.cfg.size, s.cfg.world_type);
    }

    fn handle_action(&mut self, a: MenuAction, gfx: &mut Gfx) {
        match a {
            MenuAction::None => {}
            MenuAction::OpenWorldSelect => {
                self.refresh_worlds(&gfx.gui.ctx);
                self.push(Screen::WorldSelect);
            }
            MenuAction::OpenCreate => {
                self.random_seed = splitmix(self.random_seed ^ platform::unix_now()) % 1_000_000_000;
                self.create = CreateWorldState::default();
                self.create.preview_seed = self.random_seed;
                self.create_preview = None;
                self.create_preview_inflight = None;
                if self.screen == Screen::WorldSelect {
                    self.screen = Screen::Create; // replace, so "Back" returns to the menu below
                } else {
                    self.push(Screen::Create);
                }
            }
            MenuAction::OpenSettings => self.push(Screen::Settings),
            MenuAction::Back => {
                if self.screen == Screen::Loading {
                    self.load = None;
                    self.session = None;
                    self.screen = Screen::Main;
                } else {
                    self.pop();
                }
            }
            MenuAction::Quit => {
                if let Some(s) = &mut self.session {
                    s.save();
                }
                self.quit = true;
            }
            MenuAction::ToggleTheme => {
                self.settings.theme = if self.settings.theme == ThemeMode::Dark { ThemeMode::Light } else { ThemeMode::Dark };
                self.apply_settings(gfx);
            }
            MenuAction::JoinServer(addr) => {
                self.host = None;
                self.join_server(&addr);
            }
            MenuAction::HostServer => self.host_server(),
            MenuAction::PlayWorld(id) => {
                self.net = None;
                if let Some(w) = self.worlds.iter().find(|w| w.cfg.id == id) {
                    let cfg = w.cfg.clone();
                    self.start_load(cfg, false);
                }
            }
            MenuAction::CreateWorld(cfg) => self.start_load(*cfg, true),
            MenuAction::DeleteWorld(id) => {
                let dir = self.worlds_dir.join(&id);
                if let Err(e) = platform::move_to_recycle_bin(&dir) {
                    log::error!("delete world {id}: {e}");
                }
                self.preview_cache.remove(&id);
                self.refresh_worlds(&gfx.gui.ctx);
            }
            MenuAction::RenameWorld(id, name) => {
                let dir = self.worlds_dir.join(&id);
                if let Ok(mut cfg) = WorldConfig::load(&dir) {
                    cfg.name = name;
                    let _ = cfg.save(&dir);
                }
                self.refresh_worlds(&gfx.gui.ctx);
            }
            MenuAction::Resume => {
                if self.screen == Screen::Dead {
                    if let Some(s) = &mut self.session {
                        s.player.respawn();
                    }
                }
                self.screen = Screen::Playing;
            }
            MenuAction::SaveGame => {
                if let Some(s) = &mut self.session {
                    s.save();
                }
                let now = self.time.elapsed() as f64;
                self.chat.push(ChatKind::System, "Игра сохранена", now);
            }
            MenuAction::SaveAndQuit => {
                self.chat = ChatState::default();
                self.leave_world();
            }
        }
    }
}

fn loading_view(l: &LoadState, session: Option<&GameSession>) -> (usize, f32, String) {
    let sh = &l.shared;
    if let Some(s) = session {
        let (done, total) = s.chunks.progress(s.player_chunk(), 3);
        let f = done as f32 / total.max(1) as f32;
        (6, 0.40 + 0.60 * f, format!("Чанки вокруг игрока: {done}/{total} · в работе: {}", s.chunks.stats.gen_inflight))
    } else {
        (sh.stage().min(5), sh.fraction().min(0.4), sh.detail())
    }
}

/// Physical key if the OS gave a scan code, otherwise derived from the logical key
/// (synthetic input, remote desktops, unusual layouts).
fn key_code(ke: &KeyEvent) -> Option<KeyCode> {
    if let PhysicalKey::Code(c) = ke.physical_key {
        return Some(c);
    }
    match &ke.logical_key {
        Key::Named(n) => Some(match n {
            NamedKey::Escape => KeyCode::Escape,
            NamedKey::Enter => KeyCode::Enter,
            NamedKey::Space => KeyCode::Space,
            NamedKey::Shift => KeyCode::ShiftLeft,
            NamedKey::Control => KeyCode::ControlLeft,
            NamedKey::F3 => KeyCode::F3,
            NamedKey::Tab => KeyCode::Tab,
            NamedKey::F5 => KeyCode::F5,
            NamedKey::F11 => KeyCode::F11,
            NamedKey::ArrowUp => KeyCode::ArrowUp,
            NamedKey::ArrowDown => KeyCode::ArrowDown,
            _ => return None,
        }),
        Key::Character(c) => Some(match c.to_lowercase().as_str() {
            "w" | "ц" => KeyCode::KeyW,
            "a" | "ф" => KeyCode::KeyA,
            "s" | "ы" => KeyCode::KeyS,
            "d" | "в" => KeyCode::KeyD,
            "e" | "у" => KeyCode::KeyE,
            "f" | "а" => KeyCode::KeyF,
            "t" | "е" => KeyCode::KeyT,
            "c" | "с" => KeyCode::KeyC,
            "/" | "." => KeyCode::Slash,
            "1" => KeyCode::Digit1,
            "2" => KeyCode::Digit2,
            "3" => KeyCode::Digit3,
            "4" => KeyCode::Digit4,
            "5" => KeyCode::Digit5,
            "6" => KeyCode::Digit6,
            "7" => KeyCode::Digit7,
            "8" => KeyCode::Digit8,
            "9" => KeyCode::Digit9,
            _ => return None,
        }),
        _ => None,
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        match self.create_gfx(event_loop) {
            Ok(g) => {
                log::info!("window and GPU ready");
                self.gfx = Some(g);
            }
            Err(e) => {
                log::error!("startup failed: {e}");
                platform::message_box("WolfCraft — ошибка запуска", &format!("{e}\n\nПодробности в logs/latest.log"));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(mut gfx) = self.gfx.take() else { return };
        let resp = gfx.gui.on_event(&gfx.window, &event);
        let typing = gfx.gui.wants_keyboard();
        let over_ui = gfx.gui.wants_pointer() && self.screen != Screen::Playing;
        match event {
            WindowEvent::CloseRequested => {
                if let Some(s) = &mut self.session {
                    s.save();
                }
                self.quit = true;
            }
            WindowEvent::Resized(size) => {
                gfx.gpu.resize(size.width, size.height);
                gfx.world.resize(&gfx.gpu.device, size.width, size.height);
            }
            WindowEvent::Focused(false) => {
                self.input.keys.clear();
                self.input.left = false;
                if self.screen == Screen::Playing {
                    self.screen = Screen::Paused;
                }
            }
            WindowEvent::KeyboardInput { event: ke, .. } => {
                if let Some(code) = key_code(&ke) {
                    let pressed = ke.state == ElementState::Pressed;
                    if !ke.repeat {
                        self.handle_key(code, pressed, typing);
                        if self.pending_apply {
                            self.pending_apply = false;
                            self.apply_settings(&mut gfx);
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if self.screen == Screen::Playing && !resp.consumed || self.screen == Screen::Playing {
                    let down = state == ElementState::Pressed;
                    match button {
                        MouseButton::Left => self.input.left = down,
                        MouseButton::Right => {
                            if down {
                                self.input.right_press = true;
                            }
                            self.input.right = down;
                        }
                        MouseButton::Middle if down => self.input.middle_press = true,
                        _ => {}
                    }
                } else if state == ElementState::Released {
                    self.input.left = false;
                }
                let _ = over_ui;
            }
            WindowEvent::Touch(t) => {
                self.touch.seen = true;
                if self.screen == Screen::Playing && !self.chat.open {
                    let size = gfx.gpu.size();
                    for a in self.touch.event(&t, size) {
                        match a {
                            TouchAction::Jump => self.input.jump_press = true,
                            TouchAction::Place => self.input.right_press = true,
                            TouchAction::Hotbar(d) => self.input.scroll += d as f32,
                            TouchAction::Camera => self.handle_key(KeyCode::F5, true, false),
                            TouchAction::Chat => self.chat.open(""),
                            TouchAction::HotbarSet(i) => self.input.hotbar_set = Some(i),
                            TouchAction::Inventory => self.handle_key(KeyCode::KeyE, true, false),
                            TouchAction::Pause => self.handle_key(KeyCode::Escape, true, false),
                            TouchAction::Debug => self.handle_key(KeyCode::Tab, true, false),
                        }
                    }
                } else {
                    self.touch.reset();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.screen == Screen::Playing {
                    let y = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(p) => (p.y / 30.0) as f32,
                    };
                    self.input.scroll += y;
                }
            }
            WindowEvent::RedrawRequested => {
                self.redraw(&mut gfx);
            }
            _ => {}
        }
        if self.quit {
            self.settings.save(&self.data_dir);
            event_loop.exit();
        }
        self.gfx = Some(gfx);
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.grabbed {
                self.input.mouse_dx += delta.0;
                self.input.mouse_dy += delta.1;
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(gfx) = &self.gfx else { return };
        // Menus never need more than 60 FPS; in game honour VSync / the FPS limiter.
        let cap = if !self.screen.in_world() {
            60
        } else if gfx.gpu.vsync() {
            0
        } else {
            self.settings.max_fps
        };
        if cap == 0 {
            event_loop.set_control_flow(ControlFlow::Poll);
            gfx.window.request_redraw();
        } else {
            // WaitUntil uses the coarse OS timer on Windows (~15 ms steps), which makes frames
            // uneven and the camera stutter while turning. Sleep precisely instead.
            let now = Instant::now();
            if now < self.next_frame {
                std::thread::sleep(self.next_frame - now);
            }
            let now = Instant::now();
            self.next_frame = (self.next_frame + Duration::from_secs_f64(1.0 / cap as f64)).max(now - Duration::from_millis(5));
            gfx.window.request_redraw();
            event_loop.set_control_flow(ControlFlow::Poll);
        }
    }
}

#[allow(dead_code)]
fn _day() -> f64 {
    DAY_LENGTH
}

#[allow(dead_code)]
fn _planet(_: &Planet) {}

// ---------------------------------------------------------------- compass & minimap

/// Facing in degrees clockwise from north (towards the +Y pole), on the local tangent plane.
fn compass_bearing(heading: DVec3, up: DVec3) -> f64 {
    let axis = if up.y.abs() > 0.995 { DVec3::Z } else { DVec3::Y };
    let north = (axis - up * axis.dot(up)).normalize_or_zero();
    let east = north.cross(up);
    heading.dot(east).atan2(heading.dot(north)).to_degrees().rem_euclid(360.0)
}

/// Average colour of every block's top texture in the blocks atlas.
fn block_top_colors(atlases: &AtlasManager) -> Vec<[u8; 3]> {
    use wolfcraft_engine::block::Block;
    let atlas = atlases.get(wolfcraft_assets::AtlasKind::Blocks);
    let img = atlas.image();
    (0..=255u8)
        .map(|id| {
            let b = Block::from_u8(id);
            let tile = b.info().tiles[2] as usize;
            if tile >= atlas.len() {
                return [0, 0, 0];
            }
            let r = atlas.rect(tile);
            let (mut sum, mut n) = ([0u32; 3], 0u32);
            for y in r.y..r.y + r.h {
                for x in r.x..r.x + r.w {
                    let o = ((y * img.width + x) * 4) as usize;
                    if img.pixels[o + 3] > 127 {
                        for c in 0..3 {
                            sum[c] += img.pixels[o + c] as u32;
                        }
                        n += 1;
                    }
                }
            }
            if n == 0 { [0, 0, 0] } else { [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8] }
        })
        .collect()
}

/// 64x64 heading-up map around the player: top block colour, shaded by height.
fn build_minimap_rows(s: &GameSession, colors: &[[u8; 3]], rows: std::ops::Range<usize>, px: &mut [egui::Color32]) {
    use wolfcraft_engine::block::{Block, RenderKind};
    const N: usize = 64;
    let p = &s.player;
    let up = p.up();
    let fwd = p.heading;
    let right = p.right();
    let feet = p.body.pos;
    let g = s.chunks.grid();
    let pk = feet.length().floor() as i32;
    for zy in rows {
        for xx in 0..N {
            let off = right * (xx as f64 - N as f64 / 2.0 + 0.5) + fwd * (N as f64 / 2.0 - zy as f64 - 0.5);
            let dir = (feet + off).normalize();
            let mut found = None;
            px[zy * N + xx] = egui::Color32::from_rgb(12, 14, 18);
            for k in (pk - 18..=pk + 8).rev() {
                let c = g.world_to_cell(dir * (k as f64 + 0.5));
                match s.chunks.get_block(c) {
                    None => break,
                    Some(Block::Air) => continue,
                    Some(b) if b.info().render == RenderKind::Invisible => continue,
                    Some(b) => {
                        found = Some((b, k));
                        break;
                    }
                }
            }
            if let Some((b, k)) = found {
                let base = if b == Block::Water { [52, 96, 200] } else { colors.get(b as usize).copied().unwrap_or([128, 128, 128]) };
                let shade = (1.0 + (k - pk) as f32 * 0.035).clamp(0.55, 1.35);
                let ch = |v: u8| ((v as f32 * shade).clamp(0.0, 255.0)) as u8;
                px[zy * N + xx] = egui::Color32::from_rgb(ch(base[0]), ch(base[1]), ch(base[2]));
            }
        }
    }
    let _ = up;
}

/// Phones are small: shrink the interface a little.
fn ui_mult() -> f32 {
    if cfg!(target_os = "android") { 0.75 } else { 1.0 }
}
