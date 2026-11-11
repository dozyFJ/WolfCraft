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
use wolfcraft_engine::renderer::model::{HandsPose, PlayerPose, build_first_person_hands, build_held_block, build_player_model, strike_moment, swing_speed};
use wolfcraft_ui::chat_ui::{self, ChatKind, ChatState};
use wolfcraft_ui::hud::{self, DebugInfo, HudState};
use wolfcraft_ui::inventory_ui::{self, InventoryUiState};
use wolfcraft_ui::main_menu::{self, CreateWorldState, LoadingView, MenuAction, PauseInfo, WorldEntry, WorldSelectState};
use wolfcraft_ui::{GuiRenderer, theme_manager};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Screen {
    /// Studio splash with a loading bar and tips, shown once at start.
    Splash,
    Main,
    WorldSelect,
    Create,
    Settings,
    Loading,
    Playing,
    Paused,
    Inventory,
    Dead,
    /// Full-screen world map (tap on the minimap).
    Map,
}

impl Screen {
    fn in_world(self) -> bool {
        matches!(self, Screen::Playing | Screen::Paused | Screen::Inventory | Screen::Dead | Screen::Map)
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
    /// Last chunk progress and when it last changed: a stalled load enters the world anyway.
    progress: (usize, usize),
    progress_at: Instant,
}

/// World script written in the studio (chat commands, one per line), next to world.toml.
const STUDIO_SCRIPT: &str = "studio.txt";

/// Phone folder for the player's resource packs (the app's own shared-storage folder).
const ANDROID_PACKS_DIR: &str = "/storage/emulated/0/Android/data/com.wolfcraft.mobile/files/resourcepacks";

/// Preview-channel id of the main menu backdrop planet.
const MENU_PLANET_ID: &str = "__menu_planet";

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
    /// Resource packs found on disk, the one currently loaded and the download in progress.
    packs: Vec<wolfcraft_assets::packs::PackInfo>,
    loaded_pack: String,
    pack_status: String,
    pack_dl: Option<Receiver<Result<String, String>>>,
    /// Text of the last crash (logs/crash.txt), shown once in the main menu.
    crash_report: Option<String>,
    /// World map screen: texture, pixels being rebuilt row by row, view and the colours of
    /// every column seen so far (so explored land stays on the map).
    world_map: Option<TextureHandle>,
    map_px: Vec<egui::Color32>,
    map_row: usize,
    map_view: MapView,
    map_explored: HashMap<(u8, i32, i32), egui::Color32>,
    /// Map markers placed by the player, and the chunk grid toggle.
    map_marks: Vec<DVec3>,
    map_grid: bool,
    /// Seconds the splash has been shown.
    splash_t: f32,
    /// Big planet drawn behind the main menu.
    menu_planet: Option<TextureHandle>,
    menu_planet_requested: bool,
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
    /// Orientation last asked of the phone (launcher upright, the rest in landscape).
    portrait: Option<bool>,
    /// Studio (world editor) panels while a world is open in the studio, and that world's id.
    studio: Option<wolfcraft_ui::studio_ui::StudioState>,
    studio_world: Option<String>,
    /// The world list was read for the launcher since it was last shown.
    main_worlds_fresh: bool,
    /// Listens for servers on the local network while the world list is open.
    discovery: Option<wolfcraft_net::lan::Discovery>,
    /// Open chest / furnace (block cell and kind).
    container: Option<(wolfcraft_engine::cubesphere::Cell, wolfcraft_engine::block::Block)>,
    pending_apply: bool,
    /// First-person arm swing progress 0..1 (0 = resting). A swing always plays to the end.
    hand_cycle: f32,
    /// Sprint and sneak blends of the view model (0..1).
    hand_sprint: f32,
    hand_sneak: f32,
    /// Vertical lag of the view model (spring position and velocity).
    hand_lift: f32,
    hand_lift_vel: f32,
    /// How far the hand trails behind camera turns (x right, y up).
    hand_sway: [f32; 2],
    /// Camera forward and vertical speed last frame, for sway and landing.
    hand_prev_fwd: Option<glam::Vec3>,
    hand_prev_vup: f32,
    hand_prev_ground: bool,
    /// Camera turn still to apply (smooth camera).
    look_pending: (f64, f64),
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
        let crash_report = std::fs::read_to_string(data_dir.join("logs").join("crash.txt")).ok();
        let worlds_dir = data_dir.join("worlds");
        let _ = std::fs::create_dir_all(&worlds_dir);
        let mut settings = Settings::load(&data_dir);
        let saved_servers = settings.servers.clone();
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
            screen: Screen::Splash,
            nav: Vec::new(),
            world_select: WorldSelectState { servers: saved_servers, ..Default::default() },
            create: CreateWorldState::default(),
            worlds: Vec::new(),
            preview_cache: HashMap::new(),
            preview_tx,
            preview_rx,
            preview_requested: HashSet::new(),
            create_preview: None,
            packs: Vec::new(),
            loaded_pack: String::new(),
            pack_status: String::new(),
            pack_dl: None,
            splash_t: 0.0,
            world_map: None,
            map_px: vec![egui::Color32::BLACK; MAP_W * MAP_H],
            map_marks: Vec::new(),
            map_grid: false,
            map_row: 0,
            map_view: MapView::default(),
            map_explored: HashMap::new(),
            crash_report,
            menu_planet: None,
            menu_planet_requested: false,
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
            portrait: None,
            studio: None,
            studio_world: None,
            main_worlds_fresh: false,
            discovery: None,
            container: None,
            pending_apply: false,
            hand_cycle: 0.0,
            hand_sprint: 0.0,
            hand_sneak: 0.0,
            hand_lift: 0.0,
            hand_lift_vel: 0.0,
            hand_sway: [0.0; 2],
            hand_prev_fwd: None,
            hand_prev_vup: 0.0,
            hand_prev_ground: true,
            look_pending: (0.0, 0.0),
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
        self.packs = wolfcraft_assets::packs::list(&self.pack_dirs());
        let atlases = self.load_atlases().or_else(|_| AtlasManager::new(None))?;
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
            world.set_render_scale(1.0);
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
        self.load = Some(LoadState { shared, title, error: None, started: Instant::now(), progress: (usize::MAX, 0), progress_at: Instant::now() });
        self.session = None;
        self.nav.clear();
        self.screen = Screen::Loading;
    }

    /// Connects to a WolfServer and loads its world (generated locally from the server settings).
    fn join_server(&mut self, addr: &str) {
        // An invite code ("ABCD-EFGH") or an address.
        let resolved = wolfcraft_net::lan::resolve_input(addr);
        let addr = resolved.as_str();
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
        self.load = Some(LoadState { shared, title, error: None, started: Instant::now(), progress: (usize::MAX, 0), progress_at: Instant::now() });
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
    fn host_server(&mut self, world: Option<String>) {
        self.host = None;
        // A private world: one of the player's own worlds; otherwise the shared network world.
        let dir = match &world {
            Some(id) => self.worlds_dir.join(id),
            None => self.data_dir.join("hosted").join("world"),
        };
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
        self.chat.push(ChatKind::System, "Сервер запущен: друзья увидят его в «Игра по сети → Рядом»", now);
        for line in wolfcraft_net::lan::host_lines(port) {
            self.chat.push(ChatKind::System, line, now);
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

    /// Saves the studio's world script next to the world.
    fn save_studio_script(&self) {
        if let (Some(st), Some(id)) = (&self.studio, &self.studio_world) {
            if let Err(e) = std::fs::write(self.worlds_dir.join(id).join(STUDIO_SCRIPT), &st.script) {
                log::error!("studio script: {e}");
            }
        }
    }

    fn studio_action(&mut self, a: wolfcraft_ui::studio_ui::StudioAction, submitted: &mut Vec<String>, actions: &mut Vec<MenuAction>) {
        use wolfcraft_ui::studio_ui::{StudioAction, script_commands};
        let now = self.time.elapsed() as f64;
        let status = |me: &mut Self, text: &str| {
            if let Some(st) = &mut me.studio {
                st.status = text.to_string();
                st.status_t = now;
            }
        };
        match a {
            StudioAction::Exit => {
                self.save_studio_script();
                actions.push(MenuAction::SaveAndQuit);
            }
            StudioAction::RunScript => {
                let script = self.studio.as_ref().map(|s| s.script.clone()).unwrap_or_default();
                let cmds = script_commands(&script);
                let n = cmds.len();
                submitted.extend(cmds);
                status(self, &format!("Скрипт выполнен: {n} команд"));
            }
            StudioAction::Save => {
                self.save_studio_script();
                if let Some(s) = &mut self.session {
                    s.save();
                }
                status(self, "Сохранено");
            }
            StudioAction::Publish => {
                self.save_studio_script();
                if let Some(id) = self.studio_world.clone() {
                    if let Some(s) = &mut self.session {
                        s.save();
                    }
                    actions.push(MenuAction::HostServer(Some(id)));
                }
            }
            StudioAction::DayNight => {
                let day = self.session.as_ref().is_some_and(|s| s.daylight() > 0.5);
                submitted.push(if day { "/time set ночь".into() } else { "/time set день".into() });
            }
        }
    }

    fn leave_world(&mut self) {
        self.studio = None;
        self.studio_world = None;
        self.touch.studio = false;
        self.touch.blocked.clear();
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
        // Show the planet of the world we just left.
        self.menu_planet_requested = false;
    }

    /// Folders scanned for resource packs. On a phone also the app's folder in shared storage,
    /// which a file manager or a computer over USB can reach.
    fn pack_dirs(&self) -> Vec<PathBuf> {
        let mut v = vec![self.data_dir.join("resourcepacks")];
        if cfg!(target_os = "android") {
            v.push(PathBuf::from(ANDROID_PACKS_DIR));
        }
        for d in &v {
            let _ = std::fs::create_dir_all(d);
        }
        v
    }

    /// Builds atlases for the selected pack (falls back to the default if it is gone).
    fn load_atlases(&mut self) -> Result<AtlasManager, String> {
        use wolfcraft_assets::packs::{DEFAULT_PACK, base_scale};
        let id = self.settings.texture_pack.clone();
        let user = self.packs.iter().find(|p| p.id == id).and_then(|p| p.path.clone());
        let base = if user.is_some() || base_scale(&id) > 1 || id == DEFAULT_PACK { id.clone() } else { DEFAULT_PACK.to_string() };
        if user.is_none() && base != id {
            self.settings.texture_pack = DEFAULT_PACK.into();
        }
        let m = AtlasManager::load(&base, user.as_deref(), Some(&self.data_dir.join("resourcepacks")))?;
        self.loaded_pack = self.settings.texture_pack.clone();
        Ok(m)
    }

    /// Switches the resource pack live: new atlases for the world, models and the GUI.
    fn reload_pack(&mut self, gfx: &mut Gfx) {
        match self.load_atlases() {
            Ok(a) => {
                gfx.atlases = a;
                gfx.world.reload_atlases(&gfx.gpu.device, &gfx.gpu.queue, &gfx.atlases);
                gfx.gui.icons = Arc::new(wolfcraft_ui::gui_renderer::GuiIcons::new(&gfx.gui.ctx, &gfx.atlases));
                self.block_colors.clear();
                self.pack_status = String::new();
            }
            Err(e) => {
                self.pack_status = format!("Не удалось загрузить пак: {e}");
                self.loaded_pack = self.settings.texture_pack.clone();
            }
        }
    }

    /// Downloads a .zip pack in the background into the packs folder.
    fn start_pack_download(&mut self, url: String) {
        let dir = self.pack_dirs()[0].clone();
        let (tx, rx) = channel();
        self.pack_dl = Some(rx);
        self.pack_status = "Загрузка…".into();
        std::thread::spawn(move || {
            let r = (|| -> Result<String, String> {
                let resp = ureq::get(&url).call().map_err(|e| e.to_string())?;
                let mut bytes = Vec::new();
                use std::io::Read;
                resp.into_reader().take(200 << 20).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                if !bytes.starts_with(b"PK") {
                    return Err("по ссылке не .zip файл".into());
                }
                let raw = url.split(['?', '#']).next().unwrap_or("").rsplit('/').next().unwrap_or("");
                let mut name: String = raw.trim_end_matches(".zip").chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
                if name.is_empty() {
                    name = "pack".into();
                }
                let file = format!("{name}.zip");
                std::fs::write(dir.join(&file), &bytes).map_err(|e| e.to_string())?;
                Ok(file)
            })();
            let _ = tx.send(r);
        });
    }

    fn poll_pack_download(&mut self) {
        let Some(rx) = &self.pack_dl else { return };
        if let Ok(r) = rx.try_recv() {
            self.pack_dl = None;
            self.packs = wolfcraft_assets::packs::list(&self.pack_dirs());
            self.pack_status = match r {
                Ok(f) => format!("Пак загружен: {f}. Выберите его в списке."),
                Err(e) => format!("Ошибка загрузки: {e}"),
            };
        }
    }

    fn apply_settings(&mut self, gfx: &mut Gfx) {
        gfx.gpu.set_refresh_rate(self.settings.refresh_rate);
        if self.settings.texture_pack != self.loaded_pack {
            self.reload_pack(gfx);
        }
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
            (Screen::Map, KeyCode::Escape | KeyCode::KeyM) => self.screen = Screen::Playing,
            (Screen::Playing, KeyCode::KeyM) => {
                self.screen = Screen::Map;
                self.map_row = 0;
                if let Some(s) = &self.session {
                    self.map_view.center = s.player.body.pos;
                    self.map_view.pan = (0.0, 0.0);
                }
            }
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

    /// Smooth camera: touch moves arrive unevenly (two in one frame, none in the next), so
    /// the turn is spread over a few frames instead of jumping. Nothing is lost: the rest of
    /// the turn is applied in the next frames.
    fn smooth_look(&mut self, raw: (f64, f64)) -> (f64, f64) {
        let s = self.settings.camera_smooth.clamp(0.0, 1.0) as f64;
        self.look_pending.0 += raw.0;
        self.look_pending.1 += raw.1;
        if s < 0.02 {
            return std::mem::take(&mut self.look_pending);
        }
        // Rate 40/s (barely smoothed) down to 9/s (soft glide).
        let rate = 40.0 + (9.0 - 40.0) * s;
        let k = 1.0 - (-rate * (self.time.delta() as f64).clamp(0.0, 0.1)).exp();
        let out = (self.look_pending.0 * k, self.look_pending.1 * k);
        self.look_pending.0 -= out.0;
        self.look_pending.1 -= out.1;
        if self.look_pending.0.abs() < 1e-5 && self.look_pending.1.abs() < 1e-5 {
            self.look_pending = (0.0, 0.0);
        }
        out
    }

    fn take_input_frame(&mut self) -> InputFrame {
        let k = &self.input.keys;
        let axis = |pos: KeyCode, neg: KeyCode| (k.contains(&pos) as i32 - k.contains(&neg) as i32) as f32;
        let sens = 0.0022 * self.settings.mouse_sensitivity as f64;
        self.touch.tick();
        let tl = self.touch.take_look();
        // With a bow in hand a long press on the screen draws it instead of breaking blocks.
        let bow = self.session.as_ref().is_some_and(|s| s.player.inventory.held().is_some_and(|h| h.id == wolfcraft_engine::items::ids::BOW));
        let k = &self.input.keys;
        let f = InputFrame {
            forward: (axis(KeyCode::KeyW, KeyCode::KeyS) + self.touch.move_vec.1).clamp(-1.0, 1.0),
            strafe: (axis(KeyCode::KeyD, KeyCode::KeyA) + self.touch.move_vec.0).clamp(-1.0, 1.0),
            jump: k.contains(&KeyCode::Space) || self.touch.jump_held,
            jump_pressed: std::mem::take(&mut self.input.jump_press),
            sprint: k.contains(&KeyCode::ShiftLeft) || k.contains(&KeyCode::ShiftRight) || self.touch.sprint,
            sneak: k.contains(&KeyCode::ControlLeft) || k.contains(&KeyCode::KeyC) || self.touch.sneak,
            look: self.smooth_look(((self.input.mouse_dx + tl.0 as f64 * 1.4) * sens, if self.settings.invert_y { 1.0 } else { -1.0 } * (self.input.mouse_dy + tl.1 as f64 * 1.4) * sens)),
            break_held: self.input.left || (self.touch.breaking && !bow),
            use_pressed: std::mem::take(&mut self.input.right_press),
            use_held: self.input.right || self.touch.place_held || (bow && self.touch.breaking),
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
        // Main menu backdrop: the planet of the last played world (or a fixed one).
        if matches!(self.screen, Screen::Main | Screen::Splash) && !self.menu_planet_requested {
            self.menu_planet_requested = true;
            let cfg = list_worlds(&self.worlds_dir).into_iter().max_by_key(|c| c.last_played).unwrap_or_else(|| WorldConfig::new("menu", 20_240_611));
            let tx = self.preview_tx.clone();
            rayon::spawn(move || {
                let img = preview::render_globe(&cfg, 512);
                let _ = tx.send((MENU_PLANET_ID.to_string(), img));
            });
        }
        while let Ok((id, img)) = self.preview_rx.try_recv() {
            if id == MENU_PLANET_ID {
                // Space around the globe becomes transparent so no square edge shows over the
                // menu's own starry background; the atmosphere glow fades out softly.
                let mut img = img;
                let n = img.size[0] as f32;
                for (i, px) in img.pixels.iter_mut().enumerate() {
                    let (x, y) = ((i % img.size[0]) as f32 + 0.5, (i / img.size[0]) as f32 + 0.5);
                    let r = ((x / n * 2.0 - 1.0).powi(2) + (y / n * 2.0 - 1.0).powi(2)).sqrt();
                    if r > 0.92 {
                        let a = ((0.98 - r) / 0.06).clamp(0.0, 1.0);
                        *px = egui::Color32::from_rgba_unmultiplied(px.r(), px.g(), px.b(), (a * 255.0 * 0.6) as u8);
                    }
                }
                self.menu_planet = Some(ctx.load_texture("menu-planet", img, egui::TextureOptions::LINEAR));
                continue;
            }
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
            h = splitmix(h ^ (st.size as u64 + 1) * 31 ^ (st.world_type as u64 + 1) * 131 ^ (st.continents as u64 + 1) * 1031 ^ (st.shape as u64 + 1) * 10007);
            if h != self.create_preview_key {
                self.create_preview_key = h;
            }
            if self.create_preview_inflight != Some(h) && (self.create.preview_dirty || self.create_preview.is_none() || self.create_preview_inflight.is_none()) {
                let mut cfg = WorldConfig::new("preview", st.effective_seed());
                cfg.size = st.size;
                cfg.world_type = st.world_type;
                cfg.continents = st.continents;
                cfg.shape = st.shape;
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
        let r = wolfcraft_engine::cubesphere::radial(p.body.pos);
        let surf = s.planet.surface_radius(wolfcraft_engine::cubesphere::col_dir(p.body.pos));
        let col = s.planet.column(wolfcraft_engine::cubesphere::col_dir(p.body.pos));
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
            gpu_errors: {
                let n = wolfcraft_common::gpu::GPU_ERRORS.load(std::sync::atomic::Ordering::Relaxed);
                if n == 0 { "Ошибок GPU: 0".into() } else { format!("Ошибок GPU: {n} · {}", wolfcraft_common::gpu::LAST_GPU_ERROR.lock().map(|l| l.clone()).unwrap_or_default()) }
            },
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
        // The launcher lists the worlds as studio projects: read them whenever it opens.
        if self.screen == Screen::Main {
            if !self.main_worlds_fresh {
                self.main_worlds_fresh = true;
                self.refresh_worlds(&gfx.gui.ctx);
            }
        } else {
            self.main_worlds_fresh = false;
        }
        // "Рядом": hear servers on Wi-Fi / hotspot while the world list is open.
        if self.screen == Screen::WorldSelect {
            if self.discovery.is_none() {
                crate::orientation::acquire_multicast_lock();
                self.discovery = wolfcraft_net::lan::Discovery::start();
            }
            if let Some(d) = &self.discovery {
                self.world_select.nearby = d.servers().into_iter().map(|f| (f.name, f.addr, f.players, f.max)).collect();
            }
        } else if self.discovery.is_some() {
            self.discovery = None;
            self.world_select.nearby.clear();
        }
        let portrait = matches!(self.screen, Screen::Splash | Screen::Main);
        if self.portrait != Some(portrait) {
            self.portrait = Some(portrait);
            crate::orientation::set_portrait(portrait);
        }
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
            Screen::Playing | Screen::Paused | Screen::Inventory | Screen::Dead | Screen::Map => {
                let input = if self.screen == Screen::Playing && !self.chat.open { self.take_input_frame() } else { InputFrame::default() };
                self.input.mouse_dx = 0.0;
                self.input.mouse_dy = 0.0;
                if let Some(s) = &mut self.session {
                    let frozen = self.screen != Screen::Playing;
                    let t = ScopeTimer::start();
                    let events = s.update(&input, dt, view, frozen);
                    self.profile.chunks_ms = t.ms();
                    // Changes the game made by itself (buttons springing back) go to the server too.
                    for (pos, block) in s.net_out.drain(..) {
                        if let Some(net) = &mut self.net {
                            net.send(&wolfcraft_net::C2S::Edit { cell: [pos.f as i32, pos.i, pos.j, pos.k], block: block as u8, hell: s.hell });
                        }
                    }
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
                                // A new sign asks for its text right away.
                                if matches!(block, wolfcraft_engine::block::Block::Sign | wolfcraft_engine::block::Block::SignB) {
                                    self.container = Some((pos, wolfcraft_engine::block::Block::Sign));
                                    self.inv_ui = InventoryUiState::default();
                                    self.screen = Screen::Inventory;
                                }
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
        let mut pack_refresh = false;
        let mut splash_skip = false;
        let mut crash_close = false;
        let mut map_result = MapUi::default();
        // Splash: about 2.8 s, a little longer if the menu planet is not rendered yet.
        if self.screen == Screen::Splash {
            self.splash_t += dt;
        }
        let splash_progress = (self.splash_t / 2.8).min(if self.menu_planet.is_some() { 1.0 } else { 0.9 + (self.splash_t / 6.0).min(0.1) });
        let mut pack_download: Option<String> = None;
        let mut close_inventory = false;
        let mut submitted: Vec<String> = Vec::new();
        let mut studio_actions: Vec<wolfcraft_ui::studio_ui::StudioAction> = Vec::new();
        let mut studio_cover: Vec<egui::Rect> = Vec::new();
        let icons = gfx.gui.icons.clone();
        let version = wolfcraft_common::VERSION;
        let data_dir_str = self.data_dir.display().to_string();
        let debug_info = if self.debug && self.screen.in_world() { self.session.as_ref().map(|s| self.debug_info(s, gfx)) } else { None };
        let screen = self.screen;
        let dark = self.settings.theme == ThemeMode::Dark;

        gfx.gui.hide_cursor = self.grabbed;
        // Minimap: rebuilt 4 times a second while in the world.
        self.minimap_age += dt;
        if self.screen == Screen::Map {
            if self.block_colors.is_empty() {
                self.block_colors = block_top_colors(&gfx.atlases);
            }
            if let Some(s) = &self.session {
                let rows = self.map_row..(self.map_row + 6).min(MAP_H);
                build_world_map_rows(s, &self.block_colors, rows, &mut self.map_px, &self.map_view, &mut self.map_explored);
                self.map_row += 6;
                let img = egui::ColorImage { size: [MAP_W, MAP_H], pixels: self.map_px.clone() };
                match &mut self.world_map {
                    Some(t) => t.set(img, egui::TextureOptions::NEAREST),
                    None => self.world_map = Some(gfx.gui.ctx.load_texture("world-map", img, egui::TextureOptions::NEAREST)),
                }
                if self.map_row >= MAP_H {
                    // Keep refreshing: the player moves and chunks stream in.
                    self.map_row = 0;
                }
            }
        }
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
        // Take the swapchain image before building the UI: if there is none this frame, skip
        // the UI too, so its texture updates are never lost and every shown frame has the UI.
        let t_render = ScopeTimer::start();
        let Some(output) = gfx.gpu.acquire() else { return };
        let minimap_tex = self.minimap.as_ref().map(|t| t.id());
        let frame = {
            let App { world_map, map_view, map_marks, map_grid, settings, world_select, create, worlds, create_preview, menu_planet, crash_report, packs, pack_status, session, inv_ui, load, near_table, time, chat, container, touch, studio, studio_world, .. } = self;
            let touch_size = gfx.gpu.size();
            let random_seed = create.preview_seed;
            let ctx_dt = dt;
            gfx.gui.run(&gfx.window, |ctx| match screen {
                Screen::Splash => {
                    if main_menu::splash_screen(ctx, &icons, splash_progress, version) {
                        splash_skip = true;
                    }
                    crash_close |= show_crash_report(ctx, &icons, crash_report.as_deref());
                }
                Screen::Main => {
                    actions.push(main_menu::main_menu(ctx, &icons, version, &data_dir_str, dark, menu_planet.as_ref(), settings.skin, worlds));
                    crash_close |= show_crash_report(ctx, &icons, crash_report.as_deref());
                }
                Screen::WorldSelect => actions.push(main_menu::world_select(ctx, &icons, world_select, worlds, version)),
                Screen::Create => actions.push(main_menu::create_world(ctx, &icons, create, create_preview.as_ref(), version, random_seed)),
                Screen::Settings => {
                    let folder = if cfg!(target_os = "android") { ANDROID_PACKS_DIR.to_string() } else { data_dir_str.clone() + "/resourcepacks" };
                    let r = main_menu::settings_screen(ctx, &icons, settings, version, &main_menu::PacksView { list: packs, status: pack_status, folder: &folder });
                    settings_changed |= r.changed;
                    settings_back |= r.back;
                    pack_refresh |= r.refresh_packs;
                    if r.download.is_some() {
                        pack_download = r.download;
                    }
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
                            let grid = session.as_ref().map(|s| s.chunks.progress_grid(s.player_chunk(), 7)).unwrap_or_default();
                            main_menu::loading_screen(ctx, &icons, &LoadingView { title: &format!("Мир {}", l.title), stages: &stages, current: cur, fraction: frac, detail: &detail, grid: &grid });
                        }
                    }
                }
                Screen::Playing | Screen::Paused | Screen::Inventory | Screen::Dead | Screen::Map => {
                    if touch.seen && screen == Screen::Playing && !chat.open {
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
                            bow_charge: s.bow_charge,
                            sign_text: s.hit.filter(|h| matches!(h.block, wolfcraft_engine::block::Block::Sign | wolfcraft_engine::block::Block::SignB)).and_then(|h| s.block_commands.get(&h.cell)).filter(|t| !t.trim().is_empty()).cloned(),
                            item_name: s.item_name.clone(),
                            show_fps: settings.show_fps,
                            debug: debug_info.as_ref(),
                            compass: Some(compass_bearing(p.heading, p.up()) as f32),
                            minimap: if studio.is_some() { None } else { minimap_tex },
                            compact: cfg!(target_os = "android"),
                        };
                        if screen != Screen::Map {
                            hud::draw(ctx, &icons, &hud);
                        }
                        if screen == Screen::Playing {
                            if let Some(line) = chat_ui::draw(ctx, &icons, chat, time.elapsed() as f64) {
                                submitted.push(line);
                            }
                        }
                        if let (Some(st), Screen::Playing) = (studio.as_mut(), screen) {
                            let project = s.cfg.name.clone();
                            let (a, cover) = wolfcraft_ui::studio_ui::draw(ctx, &icons, st, &project, &mut s.player.inventory, time.elapsed() as f64);
                            studio_actions = a;
                            studio_cover = cover;
                        }
                        let _ = &studio_world;
                        if s.saved_notice > 0.0 {
                            ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("saved")))
                                .text(egui::pos2(ctx.screen_rect().center().x, 28.0), egui::Align2::CENTER_CENTER, "Сохранено", egui::FontId::proportional(14.0), theme_manager::palette(ctx).success);
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
                        Screen::Map => {
                            if let Some(s) = session.as_ref() {
                                map_result = world_map_ui(ctx, &icons, world_map.as_ref(), map_view, s, map_marks, map_grid);
                            }
                        }
                        Screen::Inventory => {
                            if let Some(s) = session {
                                if let Some((pos, wolfcraft_engine::block::Block::Sign | wolfcraft_engine::block::Block::SignB)) = *container {
                                    // Sign text, kept with the command-block texts of the world.
                                    let mut text = s.block_commands.get(&pos).cloned().unwrap_or_default();
                                    let mut done = false;
                                    let dismissed = wolfcraft_ui::ui_components::modal(ctx, "sign", "Табличка", 460.0, |ui| {
                                        ui.label("Надпись (видна, когда смотришь на табличку):");
                                        let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY).char_limit(80));
                                        if !r.has_focus() && !r.lost_focus() {
                                            r.request_focus();
                                        }
                                        ui.add_space(8.0);
                                        if wolfcraft_ui::ui_components::pill_button(ui, &icons, Some(wolfcraft_assets::gui::CHECK), "Готово", wolfcraft_ui::ui_components::ButtonKind::Primary, 140.0, true).clicked() || r.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                                            done = true;
                                        }
                                    });
                                    s.block_commands.insert(pos, text);
                                    close_inventory |= dismissed || done;
                                } else if let Some((pos, wolfcraft_engine::block::Block::CommandBlock)) = *container {
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
        if map_result.close {
            self.screen = Screen::Playing;
        }
        if map_result.changed {
            self.map_row = 0;
        }
        if crash_close {
            self.crash_report = None;
            let _ = std::fs::remove_file(self.data_dir.join("logs").join("crash.txt"));
        }
        if self.screen == Screen::Splash && self.crash_report.is_none() && (splash_progress >= 1.0 || splash_skip && self.splash_t > 0.6) {
            self.screen = Screen::Main;
        }
        if pack_refresh {
            self.packs = wolfcraft_assets::packs::list(&self.pack_dirs());
            self.pack_status = format!("Найдено паков: {}", self.packs.iter().filter(|p| p.path.is_some()).count());
        }
        if let Some(url) = pack_download {
            self.start_pack_download(url);
        }
        self.poll_pack_download();
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
        if self.world_select.servers_changed {
            self.world_select.servers_changed = false;
            self.settings.servers = self.world_select.servers.clone();
            self.settings.save(&self.data_dir);
        }
        // Studio panels: keep the touch controls off them, then do what the ribbon asked.
        let ppp = gfx.gui.ctx.pixels_per_point();
        self.touch.studio = self.studio.is_some();
        if self.screen == Screen::Playing {
            self.touch.blocked = studio_cover.iter().map(|r| (r.left() * ppp, r.top() * ppp, r.right() * ppp, r.bottom() * ppp)).collect();
        }
        for a in studio_actions {
            self.studio_action(a, &mut submitted, &mut actions);
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
                let r = wolfcraft_engine::cubesphere::radial(s.player.eye_pos());
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
                let fx_target = if s.bow_charge > 0.0 { -14.0 * s.bow_charge } else if s.player.sprinting { 9.0 } else if s.player.body.flying { 4.0 } else if underwater { -4.0 } else { 0.0 };
                self.fov_fx += (fx_target - self.fov_fx) * (1.0 - (-dt as f32 * 7.0).exp());
                let params = FrameParams {
                    shaders: self.settings.shaders,
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
                let mut model_verts = wolfcraft_engine::renderer::model::build_mob_models(&s.mobs.mobs, cam.eye, cam.forward, (self.settings.render_distance * 16) as f64 * 1.15);
                let arrows: Vec<(glam::DVec3, glam::DVec3)> = s.arrows.list.iter().map(|a| (a.pos, a.dir)).collect();
                model_verts.extend(wolfcraft_engine::renderer::model::build_arrow_models(&arrows, cam.eye));
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
                        attack: self.hand_cycle,
                    };
                    model_verts.extend(build_player_model(&pose, cam.eye));
                }
                // Other players of the server (same dimension only).
                for rp in self.net_players.iter().filter(|rp| rp.hell == s.hell) {
                    let pos = glam::DVec3::from_array(rp.pos);
                    if pos.length_squared() < 1.0 {
                        continue;
                    }
                    let pose = PlayerPose { feet: pos, heading: glam::DVec3::from_array(rp.heading), up: wolfcraft_engine::physics::up_at(pos), pitch: rp.pitch, phase: rp.phase, amount: rp.amount, sneaking: rp.sneak, swimming: rp.swim, skin: rp.skin, attack: 0.0 };
                    model_verts.extend(build_player_model(&pose, cam.eye));
                }
                model_verts.extend(wolfcraft_engine::renderer::model::build_debris(&s.mobs.debris, cam.eye));
                gfx.world.set_model(&gfx.gpu.device, &gfx.gpu.queue, &model_verts);
                let sky_verts = wolfcraft_engine::renderer::sky::build_sky(&wolfcraft_engine::renderer::sky::SkyParams { eye: cam.eye, sun_dir: sun.as_vec3(), day, planet_radius: s.planet.radius, sky, time: self.time.elapsed(), underwater, cube: s.planet.grid.cube });
                gfx.world.set_sky(&gfx.gpu.device, &gfx.gpu.queue, &sky_verts);
                // First-person arms (Bedrock-style): one swing per click, a held button keeps
                // swinging, and a started swing always plays to the end.
                let first_person = s.camera_mode == crate::session::CameraMode::First && !s.player.survival.dead;
                let tool = s.player.inventory.held().and_then(|st| wolfcraft_engine::items::tool_shape(st.id));
                let want = self.screen == Screen::Playing && self.input.left && !s.player.survival.dead;
                let c0 = self.hand_cycle;
                if self.hand_cycle > 0.0 {
                    self.hand_cycle += dt * swing_speed(tool);
                    if self.hand_cycle >= 1.0 {
                        self.hand_cycle = if want { (self.hand_cycle - 1.0).max(1e-4) } else { 0.0 };
                    }
                } else if want {
                    self.hand_cycle = 1e-4;
                }
                // The moment the blow lands: chips and a hit sound while mining.
                let m = strike_moment(tool.is_some());
                let c1 = self.hand_cycle;
                let landed = (c1 > c0 && c0 < m && c1 >= m) || (c1 < c0 && c0 < m);
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
                // Body motion of the view model: sprint, sneak, turn inertia, jump and landing.
                let ease = |x: &mut f32, target: f32, rate: f32| *x += (target - *x) * (1.0 - (-rate * dt).exp());
                ease(&mut self.hand_sprint, if s.player.sprinting && s.player.body.on_ground { 1.0 } else { 0.0 }, 8.0);
                ease(&mut self.hand_sneak, if s.player.sneaking { 1.0 } else { 0.0 }, 10.0);
                {
                    let fwd = cam.forward.as_vec3();
                    let right = fwd.cross(cam.up.as_vec3()).normalize_or_zero();
                    let upv = right.cross(fwd).normalize_or_zero();
                    let (mut tx, mut ty) = (0.0, 0.0);
                    if let Some(prev) = self.hand_prev_fwd {
                        let d = fwd - prev;
                        // Turn speed in rad/s; the hand trails the opposite way.
                        tx = (-d.dot(right) / dt.max(1e-3) * 0.12).clamp(-1.0, 1.0);
                        ty = (-d.dot(upv) / dt.max(1e-3) * 0.12).clamp(-1.0, 1.0);
                    }
                    self.hand_prev_fwd = Some(fwd);
                    ease(&mut self.hand_sway[0], tx, 9.0);
                    ease(&mut self.hand_sway[1], ty, 9.0);
                }
                {
                    let vup = s.player.body.vel.dot(s.player.up()) as f32;
                    let ground = s.player.body.on_ground;
                    // Rising leaves the hand a little behind (lower), falling lets it float up.
                    let target = if ground || s.player.body.flying { 0.0 } else { (-vup * 0.006).clamp(-0.04, 0.05) };
                    if ground && !self.hand_prev_ground && self.hand_prev_vup < -2.0 {
                        // Landing: a dip that springs back.
                        self.hand_lift_vel -= (-self.hand_prev_vup * 0.09).min(1.4);
                    }
                    self.hand_prev_ground = ground;
                    self.hand_prev_vup = vup;
                    let acc = 140.0 * (target - self.hand_lift) - 14.0 * self.hand_lift_vel;
                    self.hand_lift_vel += acc * dt.min(0.05);
                    self.hand_lift = (self.hand_lift + self.hand_lift_vel * dt.min(0.05)).clamp(-0.09, 0.09);
                }
                self.hand_place = (self.hand_place - dt * 6.5).max(0.0);
                if s.player.inventory.selected != self.last_hotbar {
                    self.last_hotbar = s.player.inventory.selected;
                    self.hand_switch = 1.0;
                }
                self.hand_switch = (self.hand_switch - dt * 6.0).max(0.0);
                let block_target = if s.player.blocking { 1.0 } else { 0.0 };
                self.hand_block += (block_target - self.hand_block) * (1.0 - (-16.0 * dt).exp());
                if first_person {
                    let hand_pose = HandsPose {
                        forward: cam.forward.as_vec3(),
                        up: cam.up.as_vec3(),
                        punch_cycle: self.hand_cycle,
                        guard: self.settings.hand_anim.clamp(0.4, 1.0),
                        bob_phase: s.player.anim_phase,
                        bob_amount: if s.player.body.flying { 0.0 } else { s.player.anim_amount },
                        holding: s.player.inventory.held().is_some(),
                        tool,
                        icon: s.player.inventory.held().and_then(|st| match st.info().icon {
                            wolfcraft_engine::items::Icon::Item(i) if st.info().place.is_none() => Some(i),
                            _ => None,
                        }),
                        shield: s.player.inventory.has_shield(),
                        blocking: self.hand_block,
                        place: self.hand_place,
                        skin: self.settings.skin,
                        sprint: self.hand_sprint,
                        sneak: self.hand_sneak,
                        lift: self.hand_lift,
                        sway: self.hand_sway,
                        time: self.time.elapsed(),
                    };
                    let hands = build_first_person_hands(&hand_pose);
                    // View model placement: settings, item-switch dip, aspect ratio.
                    let aspect = w as f32 / h.max(1) as f32;
                    let fwd = cam.forward.as_vec3();
                    let upv0 = cam.up.as_vec3();
                    let right = fwd.cross(upv0).normalize_or_zero();
                    let upv = right.cross(fwd).normalize_or_zero();
                    let dip = (self.hand_switch * std::f32::consts::PI * 0.5).sin();
                    let ox = self.settings.vm_x + 0.03 - (1.6 - aspect).max(0.0) * 0.10;
                    let oy = self.settings.vm_y - 0.02 - dip * 0.40;
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
                        let pose = HandsPose { holding: true, tool: None, icon: None, ..hand_pose };
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
        // The world and the UI go to the GPU separately: an error in one never drops the other.
        gfx.gpu.queue.submit(std::iter::once(encoder.finish()));
        let mut gui_encoder = gfx.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("gui") });
        let extra = gfx.gui.render(&gfx.gpu.device, &gfx.gpu.queue, &mut gui_encoder, &view_tex, [w, h], frame);
        gfx.gpu.queue.submit(extra.into_iter().chain(std::iter::once(gui_encoder.finish())));
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
        if let (Some(s), Some(l)) = (&mut self.session, &mut self.load) {
            // Stream the starting area; enter the world once the ground around the player is meshed.
            let view = (self.settings.render_distance as i32).clamp(3, 6);
            s.update(&InputFrame::default(), self.time.delta(), view, true);
            let (done, total) = s.chunks.progress(s.player_chunk(), 3);
            if l.progress != (done, total) {
                l.progress = (done, total);
                l.progress_at = Instant::now();
            }
            let idle = s.chunks.stats.gen_inflight == 0 && s.chunks.stats.mesh_inflight == 0;
            let stalled = idle && l.progress_at.elapsed().as_secs_f32() > 4.0;
            if stalled {
                log::warn!("loading stalled at {done}/{total} (player {:?}, chunk {:?}); entering anyway", s.player.body.pos, s.player_chunk());
            }
            if (done >= total && s.ground_ready()) || stalled {
                self.load = None;
                self.screen = Screen::Playing;
                self.near_table = false;
                // Studio: build freely (creative, flying) whatever the world's mode.
                if self.studio.is_some() {
                    s.studio = true;
                    s.player.mode = wolfcraft_engine::config::GameMode::Creative;
                    s.player.body.flying = true;
                }
                // The world's script (written in the studio) runs every time the world starts.
                if self.net.is_none() {
                    if let Ok(script) = std::fs::read_to_string(self.worlds_dir.join(&s.cfg.id).join(STUDIO_SCRIPT)) {
                        let now = self.time.elapsed() as f64;
                        for line in wolfcraft_ui::studio_ui::script_commands(&script) {
                            for (kind, text) in crate::commands::run(s, &line) {
                                self.chat.push(kind, text, now);
                            }
                        }
                    }
                }
            }
        }
    }

    fn settings_world_hint(&self, s: &GameSession) {
        log::info!("session started: {} (seed {}, {:?}, {:?})", s.cfg.name, s.cfg.seed, s.cfg.size, s.cfg.world_type);
    }

    fn handle_action(&mut self, a: MenuAction, gfx: &mut Gfx) {
        match a {
            MenuAction::None => {}
            MenuAction::OpenNetwork => {
                self.world_select.mode = 1;
                self.world_select.net_tab = 0;
                self.refresh_worlds(&gfx.gui.ctx);
                self.push(Screen::WorldSelect);
            }
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
            MenuAction::SetSkin(n) => {
                self.settings.skin = n;
                self.settings.save(&self.data_dir);
            }
            MenuAction::ToggleTheme => {
                self.settings.theme = if self.settings.theme == ThemeMode::Dark { ThemeMode::Light } else { ThemeMode::Dark };
                self.apply_settings(gfx);
            }
            MenuAction::JoinServer(addr) => {
                self.host = None;
                self.join_server(&addr);
            }
            MenuAction::HostServer(world) => {
                // Publishing from the studio ends the studio: the server world is played normally.
                self.studio = None;
                self.studio_world = None;
                self.touch.studio = false;
                self.touch.blocked.clear();
                self.host_server(world)
            }
            MenuAction::OpenStudio(id) => {
                self.net = None;
                if let Some(w) = self.worlds.iter().find(|w| w.cfg.id == id) {
                    let cfg = w.cfg.clone();
                    let script = std::fs::read_to_string(self.worlds_dir.join(&id).join(STUDIO_SCRIPT)).unwrap_or_default();
                    self.studio = Some(wolfcraft_ui::studio_ui::StudioState { assets_open: true, script, ..Default::default() });
                    self.studio_world = Some(id);
                    self.start_load(cfg, false);
                }
            }
            MenuAction::PlayWorld(id) => {
                self.studio = None;
                self.studio_world = None;
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
        if let Some(gfx) = &mut self.gfx {
            // Back from the background (phone): the old surface is gone, make a new one.
            gfx.gpu.resume();
            gfx.gpu.set_refresh_rate(self.settings.refresh_rate);
            gfx.window.request_redraw();
            return;
        }
        match self.create_gfx(event_loop) {
            Ok(g) => {
                log::info!("window and GPU ready");
                g.gpu.set_refresh_rate(self.settings.refresh_rate);
                self.gfx = Some(g);
            }
            Err(e) => {
                log::error!("startup failed: {e}");
                platform::message_box("WolfCraft — ошибка запуска", &format!("{e}\n\nПодробности в logs/latest.log"));
                event_loop.exit();
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        // Phone: the app goes to the background. Save, and let go of the surface.
        if let Some(s) = &mut self.session {
            s.save();
        }
        if self.screen == Screen::Playing {
            self.screen = Screen::Paused;
        }
        if let Some(gfx) = &mut self.gfx {
            gfx.gpu.suspend();
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
                            TouchAction::Map => {
                                self.screen = Screen::Map;
                                self.map_row = 0;
                                if let Some(s) = &self.session {
                                    self.map_view.center = s.player.body.pos;
                                    self.map_view.pan = (0.0, 0.0);
                                }
                            }
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
        // Menus run at the screen rate on phones (vsync paces them), 60 FPS elsewhere;
        // in game honour VSync / the FPS limiter.
        let cap = if !self.screen.in_world() && !cfg!(target_os = "android") {
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
    let pk = g.radial(feet).floor() as i32;
    for zy in rows {
        for xx in 0..N {
            let off = right * (xx as f64 - N as f64 / 2.0 + 0.5) + fwd * (N as f64 / 2.0 - zy as f64 - 0.5);
            let dir = g.col_dir(feet + off);
            let mut found = None;
            px[zy * N + xx] = egui::Color32::from_rgb(12, 14, 18);
            for k in (pk - 18..=pk + 8).rev() {
                let c = g.world_to_cell(g.along(dir, k as f64 + 0.5));
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
                let base = if b.is_water() { [52, 96, 200] } else { colors.get(b as usize).copied().unwrap_or([128, 128, 128]) };
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

/// Shows the text of the last crash with copy / close buttons. Returns true when closed.
fn show_crash_report(ctx: &egui::Context, icons: &wolfcraft_ui::gui_renderer::GuiIcons, text: Option<&str>) -> bool {
    use wolfcraft_ui::ui_components::{ButtonKind, modal, pill_button};
    let Some(text) = text else { return false };
    let mut close = false;
    let dismissed = modal(ctx, "crash-report", "Игра упала в прошлый раз", 560.0, |ui| {
        ui.label("Сделайте скриншот или скопируйте текст и отправьте разработчику:");
        ui.add_space(6.0);
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(11.5));
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if pill_button(ui, icons, None, "Скопировать", ButtonKind::Primary, 0.0, true).clicked() {
                ctx.copy_text(text.to_string());
            }
            if pill_button(ui, icons, None, "Закрыть", ButtonKind::Secondary, 0.0, true).clicked() {
                close = true;
            }
        });
    });
    close || dismissed
}

/// World map texture size (wide, like a phone screen held sideways).
const MAP_W: usize = 352;
const MAP_H: usize = 160;

/// World map view: blocks per pixel, pan from the centre (east, north blocks), centre point.
#[derive(Clone, Copy)]
struct MapView {
    bpp: f64,
    pan: (f64, f64),
    center: DVec3,
}

impl Default for MapView {
    fn default() -> Self {
        Self { bpp: 1.0, pan: (0.0, 0.0), center: DVec3::Y }
    }
}

#[derive(Default)]
struct MapUi {
    close: bool,
    /// The view changed: redraw the map from the top.
    changed: bool,
}

/// North-up frame (east, north) at `p`.
fn map_axes(up: DVec3) -> (DVec3, DVec3) {
    let axis = if up.y.abs() > 0.995 { DVec3::Z } else { DVec3::Y };
    let north = (axis - up * axis.dot(up)).normalize_or_zero();
    (north.cross(up), north)
}

/// North-up map around `view.center`: top block colour shaded by height. Columns in unloaded
/// chunks come from the explored cache, so places you have been stay on the map.
fn build_world_map_rows(s: &GameSession, colors: &[[u8; 3]], rows: std::ops::Range<usize>, px: &mut [egui::Color32], view: &MapView, explored: &mut HashMap<(u8, i32, i32), egui::Color32>) {
    use wolfcraft_engine::block::{Block, RenderKind};
    let g = s.chunks.grid();
    let centre = view.center;
    let up = wolfcraft_engine::physics::up_at(centre);
    let (east, north) = map_axes(up);
    let pk = g.radial(s.player.body.pos).floor() as i32;
    let origin = centre + east * view.pan.0 + north * view.pan.1;
    for zy in rows {
        for xx in 0..MAP_W {
            let off = east * ((xx as f64 - MAP_W as f64 / 2.0 + 0.5) * view.bpp) + north * ((MAP_H as f64 / 2.0 - zy as f64 - 0.5) * view.bpp);
            let dir = g.col_dir(origin + off);
            let col_cell = g.world_to_cell(g.along(dir, pk as f64));
            let key = (col_cell.f, col_cell.i, col_cell.j);
            let mut found = None;
            let mut loaded = true;
            for k in (pk - 40..=pk + 24).rev() {
                let c = g.world_to_cell(g.along(dir, k as f64 + 0.5));
                match s.chunks.get_block(c) {
                    None => {
                        loaded = false;
                        break;
                    }
                    Some(Block::Air) => continue,
                    Some(b) if b.info().render == RenderKind::Invisible => continue,
                    Some(b) => {
                        found = Some((b, k));
                        break;
                    }
                }
            }
            let colour = if let Some((b, k)) = found {
                let base = if b.is_water() { [52, 96, 200] } else { colors.get(b as usize).copied().unwrap_or([128, 128, 128]) };
                let shade = (1.0 + (k - pk) as f32 * 0.03).clamp(0.55, 1.35);
                let ch = |v: u8| ((v as f32 * shade).clamp(0.0, 255.0)) as u8;
                let c = egui::Color32::from_rgb(ch(base[0]), ch(base[1]), ch(base[2]));
                if explored.len() < 2_000_000 {
                    explored.insert(key, c);
                }
                c
            } else if !loaded {
                explored.get(&key).copied().unwrap_or(egui::Color32::from_rgb(10, 12, 16))
            } else {
                egui::Color32::from_rgb(10, 12, 16)
            };
            px[zy * MAP_W + xx] = colour;
        }
    }
}

/// Round floating button for the map (like a phone maps app).
fn map_button(ui: &mut egui::Ui, c: egui::Pos2, r: f32, active: bool, draw: impl FnOnce(&egui::Painter, egui::Rect, egui::Color32)) -> bool {
    use egui::{Color32, Sense, Stroke};
    let p = theme_manager::palette(ui.ctx());
    let rect = egui::Rect::from_center_size(c, egui::Vec2::splat(r * 2.0));
    let resp = ui.interact(rect, ui.id().with(("map-btn", c.x as i32, c.y as i32)), Sense::click());
    let fill = if active { p.accent } else if resp.is_pointer_button_down_on() { Color32::from_rgb(52, 58, 72) } else { Color32::from_rgba_unmultiplied(24, 27, 36, 235) };
    ui.painter().circle_filled(c + egui::vec2(0.0, 2.0), r, Color32::from_black_alpha(90));
    ui.painter().circle_filled(c, r, fill);
    ui.painter().circle_stroke(c, r, Stroke::new(1.5_f32, Color32::from_white_alpha(40)));
    draw(ui.painter(), rect.shrink(r * 0.55), if active { p.on_accent } else { Color32::WHITE });
    resp.clicked()
}

/// Full-screen map that works like a phone maps app: drag with one finger, pinch to zoom,
/// round buttons floating over it ("назад", zoom, "где я", grid, markers).
fn world_map_ui(ctx: &egui::Context, icons: &wolfcraft_ui::gui_renderer::GuiIcons, tex: Option<&TextureHandle>, view: &mut MapView, s: &GameSession, marks: &mut Vec<DVec3>, grid: &mut bool) -> MapUi {
    use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
    let mut out = MapUi::default();
    let p = theme_manager::palette(ctx);
    egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::from_rgb(8, 10, 14))).show(ctx, |ui| {
        let screen = ui.max_rect();
        let k = (screen.width() / MAP_W as f32).max(screen.height() / MAP_H as f32);
        let map = Rect::from_center_size(screen.center(), Vec2::new(MAP_W as f32 * k, MAP_H as f32 * k));
        let resp = ui.interact(screen, ui.id().with("map-drag"), Sense::click_and_drag());
        let painter = ui.painter().clone();
        if let Some(t) = tex {
            painter.image(t.id(), map, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            view.pan.0 -= d.x as f64 / k as f64 * view.bpp;
            view.pan.1 += d.y as f64 / k as f64 * view.bpp;
            out.changed = true;
        }
        // Pinch (two fingers) and double tap zoom.
        let zoom = ctx.input(|i| i.zoom_delta());
        if (zoom - 1.0).abs() > 0.001 {
            view.bpp = (view.bpp / zoom as f64).clamp(0.25, 8.0);
            out.changed = true;
        }
        if resp.double_clicked() && view.bpp > 0.25 {
            view.bpp = (view.bpp / 2.0).max(0.25);
            out.changed = true;
        }
        let up = wolfcraft_engine::physics::up_at(view.center);
        let (east, north) = map_axes(up);
        let origin = view.center + east * view.pan.0 + north * view.pan.1;
        let scale = k as f64 / view.bpp;
        let to_screen = |w: DVec3| {
            let rel = w - origin;
            Pos2::new(map.center().x + (rel.dot(east) * scale) as f32, map.center().y - (rel.dot(north) * scale) as f32)
        };
        if *grid {
            let step = (16.0 * scale) as f32;
            if step > 6.0 {
                let ox = map.center().x - ((origin.dot(east) * scale) as f32).rem_euclid(step);
                let oy = map.center().y + ((origin.dot(north) * scale) as f32).rem_euclid(step);
                let line = Stroke::new(1.0_f32, Color32::from_black_alpha(70));
                let mut x = ox - (ox - screen.left()).div_euclid(step) * step;
                while x < screen.right() {
                    painter.line_segment([Pos2::new(x, screen.top()), Pos2::new(x, screen.bottom())], line);
                    x += step;
                }
                let mut y = oy - (oy - screen.top()).div_euclid(step) * step;
                while y < screen.bottom() {
                    painter.line_segment([Pos2::new(screen.left(), y), Pos2::new(screen.right(), y)], line);
                    y += step;
                }
            }
        }
        for (i, m) in marks.iter().enumerate() {
            let mp = to_screen(*m);
            if screen.contains(mp) {
                // Pin: a drop with a number.
                painter.circle_filled(mp - Vec2::new(0.0, 14.0), 10.0, Color32::from_rgb(235, 80, 60));
                painter.add(egui::Shape::convex_polygon(vec![mp - Vec2::new(7.0, 9.0), mp - Vec2::new(-7.0, 9.0), mp], Color32::from_rgb(235, 80, 60), Stroke::NONE));
                painter.text(mp - Vec2::new(0.0, 14.0), Align2::CENTER_CENTER, (i + 1).to_string(), wolfcraft_ui::theme_manager::bold_font(11.0), Color32::WHITE);
            }
        }
        // "You are here": a blue dot with a heading cone, like on a phone.
        let mp = to_screen(s.player.body.pos);
        if screen.contains(mp) {
            let b = compass_bearing(s.player.heading, s.player.up()).to_radians() as f32;
            let dir = Vec2::new(b.sin(), -b.cos());
            let side = Vec2::new(-dir.y, dir.x);
            painter.add(egui::Shape::convex_polygon(vec![mp, mp + dir * 34.0 + side * 16.0, mp + dir * 34.0 - side * 16.0], Color32::from_rgba_unmultiplied(66, 160, 255, 70), Stroke::NONE));
            painter.circle_filled(mp, 13.0, Color32::from_rgba_unmultiplied(66, 160, 255, 60));
            painter.circle_filled(mp, 8.0, Color32::WHITE);
            painter.circle_filled(mp, 6.0, Color32::from_rgb(40, 130, 245));
        }

        let r = 24.0;
        // Top-left: back.
        if map_button(ui, Pos2::new(screen.left() + 16.0 + r, screen.top() + 14.0 + r), r, false, |pa, rr, c| icons.paint(pa, rr, wolfcraft_assets::gui::ARROW_LEFT, c)) {
            out.close = true;
        }
        // Top: coordinates chip (like a search bar).
        let pos = s.player.body.pos;
        let info = format!("X {:.0}   Y {:.0}   Z {:.0}", pos.x, pos.y, pos.z);
        let g = painter.layout_no_wrap(info, FontId::proportional(14.0), Color32::WHITE);
        let chip = Rect::from_center_size(Pos2::new(screen.center().x, screen.top() + 14.0 + r), Vec2::new(g.size().x + 40.0, r * 2.0));
        painter.rect_filled(chip.translate(Vec2::new(0.0, 2.0)), r, Color32::from_black_alpha(90));
        painter.rect_filled(chip, r, Color32::from_rgba_unmultiplied(24, 27, 36, 235));
        painter.galley(Pos2::new(chip.left() + 20.0, chip.center().y - g.size().y / 2.0), g, Color32::WHITE);
        // Top-right: compass (north up).
        let cc = Pos2::new(screen.right() - 16.0 - r, screen.top() + 14.0 + r);
        if map_button(ui, cc, r, false, |pa, rr, _| {
            let c = rr.center();
            let h = rr.height() * 0.6;
            pa.add(egui::Shape::convex_polygon(vec![c - Vec2::new(0.0, h), c + Vec2::new(h * 0.4, 0.0), c - Vec2::new(h * 0.4, 0.0)], Color32::from_rgb(235, 70, 60), Stroke::NONE));
            pa.add(egui::Shape::convex_polygon(vec![c + Vec2::new(0.0, h), c - Vec2::new(h * 0.4, 0.0), c + Vec2::new(h * 0.4, 0.0)], Color32::from_gray(200), Stroke::NONE));
        }) {
            view.center = s.player.body.pos;
            view.pan = (0.0, 0.0);
            out.changed = true;
        }
        // Right side: zoom in / out in the middle.
        let rx = screen.right() - 16.0 - r;
        if map_button(ui, Pos2::new(rx, screen.center().y - r - 6.0), r, false, |pa, rr, c| {
            pa.line_segment([rr.left_center(), rr.right_center()], Stroke::new(3.0_f32, c));
            pa.line_segment([rr.center_top(), rr.center_bottom()], Stroke::new(3.0_f32, c));
        }) {
            view.bpp = (view.bpp / 2.0).max(0.25);
            out.changed = true;
        }
        if map_button(ui, Pos2::new(rx, screen.center().y + r + 6.0), r, false, |pa, rr, c| {
            pa.line_segment([rr.left_center(), rr.right_center()], Stroke::new(3.0_f32, c));
        }) {
            view.bpp = (view.bpp * 2.0).min(8.0);
            out.changed = true;
        }
        // Bottom-right: "where am I" (big), with grid and markers above to the left.
        let by = screen.bottom() - 22.0 - r;
        if map_button(ui, Pos2::new(rx, by), r, false, |pa, rr, c| {
            pa.circle_stroke(rr.center(), rr.width() * 0.38, Stroke::new(2.5_f32, c));
            pa.circle_filled(rr.center(), rr.width() * 0.16, c);
        }) {
            view.center = s.player.body.pos;
            view.pan = (0.0, 0.0);
            out.changed = true;
        }
        let bx = rx - 2.0 * r - 12.0;
        if map_button(ui, Pos2::new(bx, by), r, false, |pa, rr, c| icons.paint(pa, rr, wolfcraft_assets::gui::TRASH, c)) {
            marks.clear();
        }
        if map_button(ui, Pos2::new(bx - 2.0 * r - 12.0, by), r, false, |pa, rr, c| icons.paint(pa, rr, wolfcraft_assets::gui::STAR, c)) && marks.len() < 99 {
            marks.push(s.player.body.pos);
        }
        if map_button(ui, Pos2::new(bx - 4.0 * r - 24.0, by), r, *grid, |pa, rr, c| {
            for i in 0..4 {
                let f = i as f32 / 3.0;
                pa.line_segment([Pos2::new(rr.left() + rr.width() * f, rr.top()), Pos2::new(rr.left() + rr.width() * f, rr.bottom())], Stroke::new(1.5_f32, c));
                pa.line_segment([Pos2::new(rr.left(), rr.top() + rr.height() * f), Pos2::new(rr.right(), rr.top() + rr.height() * f)], Stroke::new(1.5_f32, c));
            }
        }) {
            *grid = !*grid;
        }
        // Scale hint bottom-left.
        painter.text(Pos2::new(screen.left() + 18.0, screen.bottom() - 18.0), Align2::LEFT_BOTTOM, format!("{} бл/пикс · север сверху", view.bpp), FontId::proportional(12.0), Color32::from_white_alpha(170));
    });
    out
}
