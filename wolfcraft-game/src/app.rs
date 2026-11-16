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
    GameMenu,
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

/// Old world script (chat commands, one per line), next to world.toml: read once and turned into Luau.
const STUDIO_SCRIPT: &str = "studio.txt";
/// World script in Luau, written in the studio, run when the world starts.
const SCRIPT_FILE: &str = "script.lua";

/// The world's Luau script (an old command list is translated on the way).
/// Worlds of the bundled WolfServer (`--data <files>/wolfserver`): the WolfStudio places.
fn servers_dir(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("wolfserver").join("worlds")
}

/// Studio projects made by older versions among the game worlds move to the servers folder.
fn move_studio_projects(old: &std::path::Path, servers: &std::path::Path) {
    let Ok(rd) = std::fs::read_dir(old) else { return };
    for e in rd.flatten() {
        let dir = e.path();
        if WorldConfig::load(&dir).is_ok_and(|c| c.studio_project) {
            let to = servers.join(e.file_name());
            if !to.exists() {
                if let Err(err) = std::fs::rename(&dir, &to) {
                    log::warn!("move {} -> {}: {err}", dir.display(), to.display());
                }
            }
        }
    }
}

fn read_world_script(dir: &std::path::Path) -> String {
    if let Ok(s) = std::fs::read_to_string(dir.join(SCRIPT_FILE)) {
        return s;
    }
    match std::fs::read_to_string(dir.join(STUDIO_SCRIPT)) {
        Ok(old) if !old.trim().is_empty() => crate::lua::from_command_list(&old),
        _ => String::new(),
    }
}

impl App {
    /// `/gui` lists the pages in the world's «gui» folder (an example is put there first);
    /// `/gui <name>` opens one, `/gui close` closes them all.
    fn open_web_page(&mut self, name: &str, now: f64) {
        use wolfcraft_ui::chat_ui::ChatKind;
        let Some(s) = &self.session else { return };
        let dir = s.dir.join("gui");
        let _ = std::fs::create_dir_all(&dir);
        let example = dir.join("пульт.html");
        if !example.exists() {
            let _ = std::fs::write(&example, wolfcraft_ui::webgui::EXAMPLE);
        }
        if name == "close" || name == "закрыть" {
            self.web_pages.clear();
            return;
        }
        let mut list: Vec<String> = std::fs::read_dir(&dir).map(|r| r.filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".html")).map(String::from)).collect()).unwrap_or_default();
        list.sort();
        if name.is_empty() {
            self.chat.push(ChatKind::System, format!("Окна модов (HTML/CSS/JS) в папке {}: {}. Открыть: /gui <имя>", dir.display(), list.join(", ")), now);
            return;
        }
        let Some(file) = list.iter().find(|n| n.to_lowercase() == name.to_lowercase()) else {
            self.chat.push(ChatKind::Error, format!("Нет окна «{name}». Есть: {}", list.join(", ")), now);
            return;
        };
        match std::fs::read_to_string(dir.join(format!("{file}.html"))) {
            Ok(src) => {
                self.web_pages.retain(|p| p.name != *file);
                let page = wolfcraft_ui::webgui::Page::load(file, &src, Some(dir.join(format!("{file}.save"))));
                self.web_pages.push(page);
                self.chat.open = false;
            }
            Err(e) => self.chat.push(ChatKind::Error, format!("Не открыть {file}.html: {e}"), now),
        }
    }
}

/// Draws the script's GUI (ScreenGui-like) over the game; returns the ids of pressed buttons.
fn draw_lua_gui(ctx: &egui::Context, icons:&wolfcraft_ui::GuiIcons, els: &[crate::lua::GuiElement]) -> Vec<String> {
    use crate::lua::GuiKind;
    let mut pressed = Vec::new();
    let sr = ctx.screen_rect();
    let c32 = |c: [u8; 3]| egui::Color32::from_rgb(c[0], c[1], c[2]);
    for (i, e) in els.iter().filter(|e| e.visible).enumerate() {
        let centre = egui::pos2(sr.left() + e.x * sr.width(), sr.top() + e.y * sr.height());
        let font = egui::FontId::proportional(e.size);
        let size = if e.w > 0.0 && e.h > 0.0 {
            egui::vec2(e.w * sr.width(), e.h * sr.height())
        } else {
            ctx.fonts(|f| f.layout_no_wrap(e.text.clone(), font.clone(), egui::Color32::WHITE)).size() + egui::vec2(16.0, 8.0)
        };
        let rect = egui::Rect::from_center_size(centre, size);
        egui::Area::new(egui::Id::new(("lua-gui", i, &e.id))).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
            let resp = ui.allocate_rect(egui::Rect::from_min_size(rect.min, size), if e.kind == GuiKind::Button { egui::Sense::click() } else { egui::Sense::hover() });
            let painter = ui.painter();
            let mut fill = e.color.map(c32);
            if e.kind == GuiKind::Button && resp.is_pointer_button_down_on() {
                fill = fill.map(|c| c.gamma_multiply(0.8));
            }
            let tint=fill.unwrap_or(egui::Color32::WHITE).gamma_multiply(e.opacity);
            if e.kind==GuiKind::Image {
                if let Some(sprite)=e.sprite {icons.paint(painter,rect,sprite,tint);}
            } else if e.kind!=GuiKind::Label {
                let sprite=e.sprite.unwrap_or(wolfcraft_assets::gui::PANEL_DARK);
                icons.paint_panel(painter,rect,sprite,tint);
            }
            if !e.text.is_empty() {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, &e.text, font, c32(e.text_color).gamma_multiply(e.opacity));
            }
            if resp.clicked() {
                pressed.push(e.id.clone());
            }
        });
    }
    pressed
}

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
    /// The world tick panicked this frame: save what can be saved and go back to the menu.
    world_panic: bool,
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
    /// Last touch, click or key: menus slow down to 30 FPS when nobody touches the phone.
    last_input: Instant,
    quit: bool,
    frames: u64,
    last_cpu: f32,
    near_table: bool,
    /// Orientation last asked of the phone (launcher upright, the rest in landscape).
    portrait: Option<bool>,
    /// Studio (world editor) panels while a world is open in the studio, and that world's id.
    studio: Option<wolfcraft_ui::studio_ui::StudioState>,
    studio_world: Option<String>,
    /// The world's Luau script while it runs (GUI, events, timers).
    lua: Option<crate::lua::LuaHost>,
    /// Open WolfHTML windows of mods (HTML + CSS + JS), opened with /gui <name>.
    web_pages: Vec<wolfcraft_ui::webgui::Page>,
    /// The world list was read for the launcher since it was last shown.
    main_worlds_fresh: bool,
    /// Listens for servers on the local network while the world list is open.
    discovery: Option<wolfcraft_net::lan::Discovery>,
    /// Open chest / furnace (block cell and kind).
    container: Option<(wolfcraft_engine::cubesphere::Cell, wolfcraft_engine::block::Block)>,
    pending_apply: bool,
    /// Seconds a tap with a tool or weapon still counts as a held "hit" (touch screens).
    tap_hit: f32,
    /// Seconds until the studio brush may dab again (while held).
    brush_cd: f32,
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
    /// Weapon kick after a shot: 1 → 0.
    gun_recoil: f32,
    /// Drill / chainsaw motor: 0 idle → 1 full, follows digging smoothly.
    tool_work: f32,
    /// Chat commands to send soon (the server's «Команды при запуске»).
    queued_chat: Vec<String>,
    /// Hit marker after a bullet struck a creature (seconds left).
    hit_marker: f32,
    /// Collapse sounds: at most one thud every few hundredths of a second.
    thud_cd: f32,
    /// Studio: the block last tapped or aimed at (shown in «Выбранный блок»).
    studio_target: Option<wolfcraft_engine::cubesphere::Cell>,
    /// Studio: where the edit camera was when «Играть» started (position, heading, pitch).
    studio_cam: Option<(glam::DVec3, glam::DVec3, f64)>,
    /// Voice chat on servers, and its menu (the microphone in the top bar).
    voice: crate::voice::Voice,
    voice_menu: bool,
    /// Block the player last stepped onto (for the script's «block.on_step»).
    lua_step: Option<wolfcraft_engine::cubesphere::Cell>,
    /// Fraction of a crumble particle owed while mining (see the mining effects).
    mine_dust: f32,
    /// Ink of sign texts, drawn on the signs in the world.
    sign_texts: crate::signs::SignTexts,
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
    /// The profile folder of what is hosted (server or mini-games) and the stats timer.
    host_profile: Option<std::path::PathBuf>,
    host_stat_t: f32,
    net_players: Vec<wolfcraft_net::PlayerState>,
    /// Every edit the server told us about, per dimension (`[overworld, hell]`).
    net_edits: [std::collections::HashMap<[i32; 4], u16>; 2],
    net_send_t: f32,
    net_hell: bool,
    last_step: i64,
    last_health: f32,
    /// Minimap texture and when it was last rebuilt.
    minimap: Option<egui::TextureHandle>,
    minimap_age: f32,
    /// Seconds since the sky (stars, planets, clouds) was last rebuilt: 20 times a second is
    /// plenty, it hardly moves.
    sky_age: f32,
    /// First-person animations: eating and putting armour on (1 → 0).
    eat_anim: f32,
    equip_anim: f32,
    /// Minimap pixels being rebuilt a few rows per frame, and the next row to do.
    minimap_px: Vec<egui::Color32>,
    minimap_row: usize,
    /// Average colour of each block's top texture (minimap).
    block_colors: Vec<[u8; 3]>,
}

impl App {
    pub fn new(args: LaunchArgs) -> Self {
        let data_dir = args.resolved_data_dir();
        wolfcraft_ui::launcher::set_data_dir(data_dir.clone());
        let crash_report = std::fs::read_to_string(data_dir.join("logs").join("crash.txt")).ok();
        // WolfStudio app: servers are places with their own worlds, apart from game worlds —
        // the same folder the bundled WolfServer runs them from, so every server made on the
        // «Сервер» tab is in the studio list and every place can be started as a server.
        let worlds_dir = if wolfcraft_ui::launcher::is_studio_app() { servers_dir(&data_dir) } else { data_dir.join("worlds") };
        let _ = std::fs::create_dir_all(&worlds_dir);
        if wolfcraft_ui::launcher::is_studio_app() {
            move_studio_projects(&data_dir.join("worlds"), &worlds_dir);
        }
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
            tap_hit: 0.0,
            brush_cd: 0.0,
            world_map: None,
            map_px: vec![egui::Color32::BLACK; MAP_W * MAP_H],
            map_marks: Vec::new(),
            map_grid: false,
            map_row: 0,
            map_view: MapView::default(),
            map_explored: HashMap::new(),
            crash_report,
            world_panic: false,
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
            last_input: Instant::now(),
            quit: false,
            frames: 0,
            last_cpu: 0.0,
            near_table: false,
            portrait: None,
            studio: None,
            studio_world: None,
            web_pages: Vec::new(),
            lua: None,
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
            gun_recoil: 0.0,
            tool_work: 0.0,
            queued_chat: Vec::new(),
            hit_marker: 0.0,
            thud_cd: 0.0,
            studio_target: None,
            studio_cam: None,
            lua_step: None,
            voice: crate::voice::Voice::default(),
            voice_menu: false,
            mine_dust: 0.0,
            sign_texts: Default::default(),
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
            host_profile: None,
            host_stat_t: 0.0,
            net_players: Vec::new(),
            net_edits: Default::default(),
            net_send_t: 0.0,
            net_hell: false,
            last_step: 0,
            last_health: 20.0,
            minimap: None,
            minimap_age: 10.0,
            sky_age: 10.0,
            eat_anim: 0.0,
            equip_anim: 0.0,
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
            world.set_render_scale(self.settings.render_scale);
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
        // A role key is never part of the address, world-cache ID or ordinary invite code.
        let (address,explicit_key)=addr.split_once('#').map(|(a,k)|(a,Some(k.to_string()))).unwrap_or((addr,None));
        let resolved = wolfcraft_net::lan::resolve_input(address);
        let local_access = if wolfcraft_ui::launcher::is_studio_app() {
            let path=self.data_dir.join("open_access.json");
            let data=std::fs::read_to_string(&path).ok();let _=std::fs::remove_file(path);
            data.and_then(|s|serde_json::from_str::<serde_json::Value>(&s).ok()).filter(|v|v["address"].as_str().is_some_and(|a|wolfcraft_net::with_port(a)==wolfcraft_net::with_port(&resolved))).and_then(|v|v["key"].as_str().map(str::to_owned))
        }else{None};
        let access=explicit_key.or(local_access);
        let addr = resolved.as_str();
        let nick = if self.settings.nickname.trim().is_empty() { "Игрок" } else { self.settings.nickname.trim() };
        let client = match wolfcraft_net::Client::connect_with_access(addr, nick, self.settings.skin,access) {
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
        // Use the selected world's profile, never the last visible launcher tab.
        let profile_data = wolfcraft_ui::server_page::profile_data(&self.data_dir, cfg.studio_project);
        let options = wolfcraft_net::server::ServerOptions { max_players: wolfcraft_ui::server_page::max_players(&profile_data), ..Default::default() };
        match wolfcraft_net::server::start_with_options(&cfg.name, toml, &dir, port, false, options) {
            Ok(h) => {
                self.host = Some(h);
                wolfcraft_ui::server_page::add_run(&profile_data, platform::unix_now());
                self.host_profile = Some(profile_data.clone());
                self.host_stat_t = 0.0;
            }
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
        // «Сервер → Консоль → Команды при запуске».
        self.queued_chat.extend(wolfcraft_ui::server_page::autoexec(&profile_data));
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
                            s.remote_edits.push((to_cell(c), Block::from_id(b)));
                        }
                    }
                }
                wolfcraft_net::S2C::Edit { cell, block, hell } => {
                    self.net_edits[hell as usize].insert(cell, block);
                    if hell == s.hell {
                        s.remote_edits.push((to_cell(cell), Block::from_id(block)));
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
                wolfcraft_net::S2C::Voice { id, data } => self.voice.receive(self.audio.handle(), id, &data, now),
                wolfcraft_net::S2C::Welcome { .. } | wolfcraft_net::S2C::Ready => {}
            }
        }
        // Voice chat: the settings, the microphone (push-to-talk: V or holding the 🎤),
        // frames out to the server, volumes by distance.
        {
            let v = &mut self.voice;
            v.enabled = self.settings.voice_enabled;
            v.push_to_talk = self.settings.voice_push_to_talk;
            v.threshold = self.settings.voice_threshold;
            v.volume = self.settings.voice_volume;
            v.proximity = self.settings.voice_proximity;
            v.ptt_held = self.touch.voice_held || (self.screen == Screen::Playing && self.input.keys.contains(&KeyCode::KeyV));
            let allowed = v.enabled && net.voice && crate::mic::granted();
            for frame in v.capture(allowed, dt) {
                net.send(&wolfcraft_net::C2S::Voice(frame));
            }
            v.update(s.player.body.pos, &self.net_players, now);
            self.touch.voice_state = if !v.enabled { 0 } else if v.talking { 2 } else { 1 };
        }
        // After a dimension change every known edit of the new dimension applies again.
        if s.hell != self.net_hell {
            self.net_hell = s.hell;
            s.remote_edits = self.net_edits[s.hell as usize].iter().map(|(c, b)| (to_cell(*c), Block::from_id(*b))).collect();
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
                swim: (p.body.in_water || p.body.in_lava) && !p.body.on_ground && !p.body.flying,
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

    /// What the studio shows about the place this frame.
    fn studio_view(&self) -> Option<wolfcraft_ui::studio_ui::StudioView> {
        let s = self.session.as_ref()?;
        use wolfcraft_engine::player as pl;
        let (m, day) = pl::month(s.player.world_time);
        let held_block = s.player.inventory.held().filter(|h| h.info().place.is_some()).map(|h| (h.id, h.info().name.to_string()));
        let mut players = vec![(s.nickname.clone(), s.player.body.pos.to_array(), s.player.survival.health)];
        for rp in &self.net_players {
            players.push((rp.nick.clone(), rp.pos, 20.0));
        }
        let cell = |c: Option<wolfcraft_engine::cubesphere::Cell>| c.map(|c| [c.i, c.j, c.k]);
        Some(wolfcraft_ui::studio_ui::StudioView {
            project: s.cfg.name.clone(),
            seed: s.cfg.seed as i64,
            creative: s.is_creative(),
            clock: s.clock(),
            date: format!("{day} {}", pl::MONTH_GENITIVE[m]),
            hour: (pl::day_fraction(s.player.world_time) * 24.0) as f32,
            spawning: s.mobs.spawning,
            players,
            mobs: s.mobs.mobs.iter().map(|mb| (mb.kind.name().to_string(), mb.hp, mb.kind.max_hp(), mb.body.pos.to_array())).collect(),
            drops: s.drops.list.len(),
            output: self.chat.lines.iter().rev().take(80).rev().map(|l| (l.kind, l.text.clone())).collect(),
            held_block,
            selection: (cell(s.we.pos1), cell(s.we.pos2)),
            target: self.studio_target.and_then(|c| {
                let b = s.chunks.get_block(c)?;
                (b != wolfcraft_engine::block::Block::Air).then(|| (b as u16, b.info().name.to_string(), [c.i, c.j, c.k]))
            }),
            script_running: self.lua.is_some(),
            models: s.we.models.iter().map(|m| (m.name.clone(), m.cells.len())).collect(),
            compact: cfg!(target_os = "android") || self.touch.seen,
            now: 0.0,
        })
    }

    /// Saves the studio's world script next to the world.
    fn save_studio_script(&self) {
        if let (Some(st), Some(id)) = (&self.studio, &self.studio_world) {
            if let Err(e) = std::fs::write(self.worlds_dir.join(id).join(SCRIPT_FILE), &st.script) {
                log::error!("studio script: {e}");
            }
        }
    }

    fn studio_action(&mut self, a: wolfcraft_ui::studio_ui::StudioAction, submitted: &mut Vec<String>, actions: &mut Vec<MenuAction>) {
        use wolfcraft_ui::studio_ui::StudioAction;
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
                if wolfcraft_ui::launcher::is_studio_app() {
                    // WolfStudio: back to «Мои серверы» in the app (like closing a place in
                    // Roblox Studio), with everything saved.
                    if let Some(s) = &mut self.session {
                        s.save();
                    }
                    self.quit = true;
                } else {
                    actions.push(MenuAction::SaveAndQuit);
                }
            }
            StudioAction::RunScript => {
                // A fresh Luau state each run: the old GUI, handlers and timers go away.
                let script = self.studio.as_ref().map(|s| s.script.clone()).unwrap_or_default();
                let _ = &submitted;
                let errors = self.start_lua(&script);
                status(self, if errors { "Скрипт: есть ошибки (см. чат)" } else { "Скрипт запущен" });
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
            StudioAction::StopScript => {
                self.lua = None;
                status(self, "Скрипт остановлен");
            }
            StudioAction::Command(c) => submitted.push(c),
            StudioAction::SetHour(h) => {
                if let Some(s) = &mut self.session {
                    let len = wolfcraft_engine::player::DAY_LENGTH;
                    let t = s.player.world_time;
                    s.player.world_time = (t / len).floor() * len + (h as f64 / 24.0) * len;
                }
            }
            StudioAction::SetCreative(c) => submitted.push(if c { "/gamemode creative".into() } else { "/gamemode survival".into() }),
            StudioAction::SetSpawning(on) => {
                if let Some(s) = &mut self.session {
                    s.mobs.spawning = on;
                }
            }
            StudioAction::RemoveMob(i) => {
                if let Some(s) = &mut self.session {
                    if i < s.mobs.mobs.len() {
                        s.mobs.mobs.remove(i);
                    }
                }
            }
            StudioAction::GoTo(p) => {
                if let Some(s) = &mut self.session {
                    let at = glam::DVec3::from_array(p);
                    s.player.body.pos = at + wolfcraft_engine::physics::up_at(at) * 1.5;
                    s.player.body.vel = glam::DVec3::ZERO;
                }
            }
            StudioAction::TargetCorner(n) => {
                if let (Some(s), Some(c)) = (&mut self.session, self.studio_target) {
                    if n == 1 {
                        s.we.pos1 = Some(c);
                    } else {
                        s.we.pos2 = Some(c);
                    }
                    status(self, if n == 1 { "Точка 1 выбрана" } else { "Точка 2 выбрана" });
                }
            }
            StudioAction::PickTarget => {
                if let (Some(s), Some(c)) = (&mut self.session, self.studio_target) {
                    if let Some(b) = s.chunks.get_block(c).filter(|b| *b != wolfcraft_engine::block::Block::Air) {
                        let id = b.base_item() as u16;
                        let sel = s.player.inventory.selected;
                        s.player.inventory.slots[sel] = Some(wolfcraft_engine::items::ItemStack::new(id, wolfcraft_engine::items::info(id).max_stack));
                    }
                }
            }
            StudioAction::RemoveTarget => {
                if let (Some(s), Some(c)) = (&mut self.session, self.studio_target) {
                    use wolfcraft_engine::interaction::WorldEdit;
                    if s.chunks.set_block(c, wolfcraft_engine::block::Block::Air) {
                        if let Some(net) = &mut self.net {
                            net.send(&wolfcraft_net::C2S::Edit { cell: [c.f as i32, c.i, c.j, c.k], block: 0, hell: s.hell });
                        }
                    }
                }
            }
            StudioAction::Paint(wolfcraft_ui::texture_editor::PaintAction::Save { name, size, rgba, item }) => {
                let Some(id) = self.studio_world.clone() else { return };
                let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
                let safe = if safe.is_empty() { "texture".to_string() } else { safe };
                let dir = self.worlds_dir.join(id).join("textures").join(if item { "items" } else { "blocks" });
                let _ = std::fs::create_dir_all(&dir);
                let bytes: Vec<u8> = rgba.iter().flatten().copied().collect();
                let path = dir.join(format!("{safe}.png"));
                match image::RgbaImage::from_raw(size, size, bytes).map(|img| img.save(&path)) {
                    Some(Ok(())) => status(self, &format!("Текстура сохранена: textures/{}/{safe}.png", if item { "items" } else { "blocks" })),
                    _ => status(self, "Не удалось сохранить текстуру"),
                }
            }
            StudioAction::Paint(wolfcraft_ui::texture_editor::PaintAction::ToBlocks { size, rgba }) => {
                let Some(s) = &mut self.session else { return };
                use wolfcraft_engine::interaction::WorldEdit;
                let up = s.player.up();
                let fwd = s.player.heading;
                let right = s.player.right();
                let grid = s.chunks.grid();
                let base = s.player.body.pos + fwd * (size as f64 * 0.5 + 4.0) - right * (size as f64 * 0.5);
                let mut n = 0;
                for (k, c) in rgba.iter().enumerate() {
                    if c[3] < 128 {
                        continue;
                    }
                    let x = (k as u32 % size) as f64;
                    let y = (size - 1 - k as u32 / size) as f64;
                    let cell = grid.world_to_cell(base + right * (x + 0.5) + up * (y + 0.5));
                    let b = nearest_block(*c);
                    if s.chunks.set_block(cell, b) {
                        n += 1;
                        if let Some(net) = &mut self.net {
                            net.send(&wolfcraft_net::C2S::Edit { cell: [cell.f as i32, cell.i, cell.j, cell.k], block: b as u16, hell: s.hell });
                        }
                    }
                }
                if let Some(st) = &mut self.studio {
                    st.paint.open = false;
                }
                status(self, &format!("Из текстуры построено блоков: {n}"));
            }
            StudioAction::Model(i, cmd) => {
                if let Some(s) = &mut self.session {
                    crate::worldedit::load_models(s);
                    let msg = crate::worldedit::model_cmd(s, i, cmd);
                    match msg {
                        Ok(t) | Err(t) => status(self, &t),
                    }
                }
            }
            StudioAction::SpawnHere => {
                if let Some(s) = &mut self.session {
                    s.player.spawn = s.player.body.pos;
                    status(self, "Точка появления — здесь (сюда встанет игрок по «Играть»)");
                }
            }
            StudioAction::Playtest(on) => {
                if let Some(st) = &mut self.studio {
                    st.playtest = on;
                    st.brush_on = false;
                }
                if let Some(s) = &mut self.session {
                    if on {
                        // Like Roblox Studio's Play: the camera waits here, a character appears at
                        // the spawn point and plays by the world's own rules.
                        self.studio_cam = Some((s.player.body.pos, s.player.heading, s.player.pitch));
                        s.player.noclip = false;
                        s.player.body.flying = false;
                        s.studio = false;
                        s.player.mode = s.cfg.mode;
                        s.player.survival = Default::default();
                        let spawn = s.player.spawn;
                        let dir = wolfcraft_engine::cubesphere::col_dir(spawn);
                        let pos = s.planet.stand_position(dir).unwrap_or(spawn);
                        s.teleport(pos);
                    } else {
                        // Back to the edit camera where it was; what happened in play stays
                        // (blocks broken in play stay broken, like a saved test run).
                        s.studio = true;
                        s.player.mode = wolfcraft_engine::config::GameMode::Creative;
                        s.player.survival = Default::default();
                        if let Some((pos, heading, pitch)) = self.studio_cam.take() {
                            s.teleport(pos);
                            s.player.heading = heading;
                            s.player.pitch = pitch;
                        }
                        s.player.noclip = true;
                        s.player.body.flying = true;
                    }
                }
                if on {
                    // Play the place as players will: its script runs fresh (saved first).
                    self.save_studio_script();
                    let script = self.studio.as_ref().map(|s| s.script.clone()).unwrap_or_default();
                    self.start_lua(&script);
                } else {
                    self.lua = None;
                    status(self, "Игра остановлена");
                }
            }
        }
    }

    /// Starts the world's Luau script (replacing a running one). Returns true on errors.
    fn start_lua(&mut self, source: &str) -> bool {
        self.lua = None;
        if source.trim().is_empty() {
            return false;
        }
        let Some(s) = &mut self.session else { return false };
        let now = self.time.elapsed() as f64;
        match crate::lua::LuaHost::new() {
            Ok(mut l) => {
                l.run(s, source);
                let out = l.take_output();
                let errors = out.iter().any(|(k, _)| *k == wolfcraft_ui::chat_ui::ChatKind::Error);
                for (k, t) in out {
                    self.chat.push(k, t, now);
                }
                self.lua = Some(l);
                errors
            }
            Err(e) => {
                self.chat.push(wolfcraft_ui::chat_ui::ChatKind::Error, format!("Lua: {e}"), now);
                true
            }
        }
    }

    fn leave_world(&mut self) {
        self.lua = None;
        self.studio = None;
        self.studio_world = None;
        self.touch.studio = false;
        self.touch.blocked.clear();
        self.net = None;
        self.host = None;
        self.net_players.clear();
        self.voice.reset();
        self.voice_menu = false;
        if let Some(mut s) = self.session.take() {
            s.save();
        }
        self.pending_meshes.clear();
        if let Some(g) = &mut self.gfx {
            g.world.clear();
        }
        self.nav.clear();
        self.nav.push(Screen::Main);
        self.screen = Screen::GameMenu;
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
        let builtin = wolfcraft_assets::packs::BUILTIN.iter().any(|b| b.0 == id);
        let _ = base_scale;
        let base = if user.is_some() || builtin { id.clone() } else { DEFAULT_PACK.to_string() };
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
            (Screen::Playing, KeyCode::Escape) if !self.web_pages.is_empty() => self.web_pages.clear(),
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
            (Screen::Playing, KeyCode::KeyB) => self.voice_menu = !self.voice_menu,
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
            // WolfTech drill: R changes the digging square (1×1, 3×3 … 13×13).
            (Screen::Playing, KeyCode::KeyR) if self.session.as_ref().is_some_and(|s| s.player.inventory.held().is_some_and(|h| h.id == wolfcraft_engine::items::ids::DRILL)) => {
                if let Some(s) = &mut self.session {
                    s.player.drill_area = wolfcraft_engine::tools::next_drill_mode(s.player.drill_area);
                    let n = wolfcraft_engine::tools::drill_side(s.player.drill_area);
                    s.item_name = Some((format!("Бур: копает {n}×{n}"), 1.4));
                }
            }
            // «Кликер» on the keyboard: C taps, V sells.
            (Screen::Playing, KeyCode::KeyC | KeyCode::KeyV) if self.session.as_ref().is_some_and(|s| s.clicker.active && (s.minigames || s.studio)) => {
                if let Some(s) = &mut self.session {
                    if code == KeyCode::KeyC {
                        if s.clicker.tap() <= 0.0 {
                            s.item_name = Some(("Рюкзак полон — продайте клики (V)".into(), 1.5));
                        }
                    } else {
                        let m = s.clicker.sell();
                        if m > 0.0 {
                            s.item_name = Some((format!("Продано: +{} денег", wolfcraft_engine::buttonsim::short(m)), 1.5));
                        }
                    }
                }
            }
            (Screen::Playing, KeyCode::Space) => self.input.jump_press = true,
            (Screen::Playing, KeyCode::KeyF) => self.input.swap_press = true,
            // Q throws the held item (one; with Ctrl the whole stack), like the classic game.
            (Screen::Playing, KeyCode::KeyQ) => {
                let all = self.input.keys.contains(&KeyCode::ControlLeft);
                if let Some(s) = &mut self.session {
                    s.throw_held(all);
                }
            }
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
        // A mouse moves in jumpy steps too: on PC the view always glides a little.
        let mouse = !self.touch.seen;
        let s = self.settings.camera_smooth.clamp(0.0, 1.0) as f64;
        let s = if mouse { s.max(0.45) } else { s };
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

    /// Direction of the ray under the finger (touch targeting), from the camera, the field of
    /// view and the screen size. Third person still aims from the eyes, along the same ray.
    fn touch_aim(&self) -> Option<glam::DVec3> {
        let (px, py) = self.touch.aim_point()?;
        let s = self.session.as_ref()?;
        let gfx = self.gfx.as_ref()?;
        let (w, h) = gfx.gpu.size();
        let (w, h) = (w.max(1) as f64, h.max(1) as f64);
        let cam = s.camera();
        let fwd = cam.forward.normalize();
        let right = fwd.cross(cam.up).normalize();
        let up = right.cross(fwd).normalize();
        let tan = ((self.settings.fov + self.fov_fx) as f64).to_radians().mul_add(0.5, 0.0).tan();
        let (nx, ny) = (px as f64 / w * 2.0 - 1.0, 1.0 - py as f64 / h * 2.0);
        Some((fwd + right * (nx * tan * w / h) + up * (ny * tan)).normalize())
    }

    fn take_input_frame(&mut self) -> InputFrame {
        let k = &self.input.keys;
        let axis = |pos: KeyCode, neg: KeyCode| (k.contains(&pos) as i32 - k.contains(&neg) as i32) as f32;
        // The mouse turns slower than a finger drag (it was far too quick on PC).
        let sens = 0.0022 * self.settings.mouse_sensitivity as f64;
        let mouse_sens = if cfg!(target_os = "android") { sens } else { sens * 0.6 };
        self.touch.tick();
        let throws = std::mem::take(&mut self.touch.throws);
        if throws > 0 {
            if let Some(s) = &mut self.session {
                for _ in 0..throws {
                    s.throw_held(false);
                }
            }
        }
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
            sneak: k.contains(&KeyCode::ControlLeft) || k.contains(&KeyCode::KeyC) || if self.touch.flying { self.touch.down_held } else { self.touch.sneak },
            look: self.smooth_look((self.input.mouse_dx * mouse_sens + tl.0 as f64 * 1.4 * sens, if self.settings.invert_y { 1.0 } else { -1.0 } * (self.input.mouse_dy * mouse_sens + tl.1 as f64 * 1.4 * sens))),
            break_held: self.input.left || (self.touch.breaking && !bow) || self.tap_hit > 0.0,
            use_pressed: std::mem::take(&mut self.input.right_press),
            use_held: self.input.right || self.touch.place_held || (bow && self.touch.breaking),
            swap_hands: std::mem::take(&mut self.input.swap_press),
            pick_pressed: std::mem::take(&mut self.input.middle_press),
            hotbar_delta: std::mem::take(&mut self.input.scroll) as i32,
            hotbar_set: self.input.hotbar_set.take(),
            aim: if self.settings.tap_aim { self.touch_aim() } else { None },
            touch: self.touch.seen && self.settings.tap_aim,
        };
        // A tap blow aims at the tapped point for its whole short "hold".
        if self.tap_hit <= 0.0 {
            self.touch.tap_point = None;
        }
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
        // Like Java's "Saving world" on a crash: an error inside the world does not kill the
        // app — the world is saved, the report is shown, the player is back in the menu.
        if std::mem::take(&mut self.world_panic) {
            log::error!("world tick failed: saving and leaving the world");
            if let Some(s) = self.session.take() {
                let saved = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let mut s = s;
                    s.save();
                }));
                if saved.is_err() {
                    log::error!("saving after the crash failed too");
                }
            }
            self.leave_world();
            let report = std::fs::read_to_string(self.data_dir.join("logs").join("crash.txt")).unwrap_or_default();
            self.crash_report = Some(format!("Мир остановлен из-за ошибки. Прогресс сохранён, можно зайти снова.\n\n{report}"));
        }
        // The launcher lists the worlds as studio projects: read them whenever it opens.
        if self.screen == Screen::Main {
            if !self.main_worlds_fresh {
                self.main_worlds_fresh = true;
                self.refresh_worlds(&gfx.gui.ctx);
            }
        } else {
            self.main_worlds_fresh = false;
        }
        // WolfStudio app: «Открыть в студии» on the server dashboard leaves the address here.
        if wolfcraft_ui::launcher::is_studio_app() && self.screen == Screen::Main {
            // A server card tapped in the app's «Мои серверы»: straight into its editor scene.
            let f = self.data_dir.join("open_place.txt");
            if let Ok(id) = std::fs::read_to_string(&f) {
                let _ = std::fs::remove_file(&f);
                let id = id.trim().to_string();
                if !id.is_empty() {
                    self.refresh_worlds(&gfx.gui.ctx);
                    self.handle_action(MenuAction::OpenStudio(id), gfx);
                }
            }
            let f = self.data_dir.join("open_server.txt");
            if let Ok(addr) = std::fs::read_to_string(&f) {
                let _ = std::fs::remove_file(&f);
                let addr = addr.trim().to_string();
                if !addr.is_empty() {
                    self.handle_action(MenuAction::EditServerInStudio(addr), gfx);
                }
            }
        }
        // "Рядом": hear servers on Wi-Fi / hotspot while the world list is open.
        // The studio app also hears servers on its home screen (their worlds open in the studio).
        let studio_home = wolfcraft_ui::launcher::is_studio_app() && self.screen == Screen::Main;
        if self.screen == Screen::WorldSelect || studio_home {
            if self.discovery.is_none() {
                crate::orientation::acquire_multicast_lock();
                self.discovery = wolfcraft_net::lan::Discovery::start();
            }
            if let Some(d) = &self.discovery {
                self.world_select.nearby = d.servers().into_iter().map(|f| (f.name, f.addr, f.players, f.max)).collect();
                if studio_home {
                    wolfcraft_ui::launcher::set_nearby(self.world_select.nearby.clone());
                }
            }
        } else if self.discovery.is_some() {
            self.discovery = None;
            self.world_select.nearby.clear();
        }
        // Keep phone menu screens upright; gameplay and the editor use landscape.
        let portrait = matches!(self.screen, Screen::Splash | Screen::Main | Screen::GameMenu | Screen::WorldSelect | Screen::Create | Screen::Settings) && !wolfcraft_ui::launcher::is_studio_app() && self.studio.is_none();
        if self.portrait != Some(portrait) {
            self.portrait = Some(portrait);
            crate::orientation::set_portrait(portrait);
        }
        self.time.tick();
        let dt = self.time.delta();
        // Dashboard statistics: players online every 10 s while we host.
        if let (Some(h), Some(prof)) = (&self.host, &self.host_profile) {
            if h.is_running() {
                self.host_stat_t -= dt;
                if self.host_stat_t <= 0.0 {
                    self.host_stat_t = 10.0;
                    wolfcraft_ui::server_page::add_sample(prof, platform::unix_now(), self.net_players.len() as u32 + 1);
                }
            }
        }
        self.frames += 1;
        self.last_cpu = self.cpu.sample();
        let t_frame = ScopeTimer::start();

        // ---- non-UI logic ----
        let t_update = ScopeTimer::start();
        let view = self.settings.render_distance as i32;
        let mut deaths = false;
        match self.screen {
            Screen::Playing | Screen::Paused | Screen::Inventory | Screen::Dead | Screen::Map => {
                let mut input = if self.screen == Screen::Playing && !self.chat.open { self.take_input_frame() } else { InputFrame::default() };
                self.tap_hit = (self.tap_hit - dt as f32).max(0.0);
                // Studio brush: taps and holds on the world sculpt instead of placing/breaking.
                let brush = self.studio.as_ref().filter(|st| st.brush_on && !st.playtest).map(|st| (st.brush_op, st.brush_r, st.brush_cube));
                let brush_fire = brush.is_some() && (input.use_pressed || input.break_held || input.use_held);
                if brush.is_some() {
                    input.use_pressed = false;
                    input.use_held = false;
                    input.break_held = false;
                }
                self.input.mouse_dx = 0.0;
                self.input.mouse_dy = 0.0;
                if let Some(s) = &mut self.session {
                    let frozen = self.screen != Screen::Playing;
                    let t = ScopeTimer::start();
                    s.fx_amount = self.settings.particles;
                    // Mini-games only on a server (online) or in the studio.
                    s.minigames = s.cfg.studio_project && (self.net.is_some() || self.studio.is_some());
                    // Script: a tap on a block with a «block.on_interact» handler goes to the
                    // script instead of placing or opening.
                    if input.use_pressed && !frozen && !self.studio.as_ref().is_some_and(|st| !st.playtest) {
                        if let (Some(l), Some(h)) = (&mut self.lua, s.hit) {
                            if l.has_block_event("interact", h.block) && l.block_event(s, "interact", h.block, h.cell) {
                                input.use_pressed = false;
                            }
                        }
                    }
                    let events = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.update(&input, dt, view, frozen))) {
                        Ok(e) => e,
                        Err(_) => {
                            self.world_panic = true;
                            Vec::new()
                        }
                    };
                    // Script: «block.on_step» when the player steps onto a new block.
                    if let Some(l) = &mut self.lua {
                        let g = s.chunks.grid();
                        let under = g.world_to_cell(s.player.body.pos - s.player.up() * 0.3);
                        if self.lua_step != Some(under) && s.player.body.on_ground {
                            self.lua_step = Some(under);
                            if let Some(b) = s.chunks.get_block(under) {
                                if l.has_block_event("step", b) {
                                    l.block_event(s, "step", b, under);
                                }
                            }
                        } else if !s.player.body.on_ground {
                            self.lua_step = None;
                        }
                    }
                    if self.studio.is_some() {
                        crate::worldedit::load_models(s);
                        if let Some(h) = s.hit {
                            self.studio_target = Some(h.cell);
                        }
                    }
                    self.profile.chunks_ms = t.ms();
                    if let Some((op, r, cube)) = brush {
                        crate::worldedit::stroke(s, brush_fire);
                        self.brush_cd = (self.brush_cd - dt as f32).max(0.0);
                        if brush_fire && self.brush_cd <= 0.0 {
                            self.brush_cd = 0.12;
                            let block = s.player.inventory.held().and_then(|h| h.info().place).unwrap_or(wolfcraft_engine::block::Block::Stone);
                            let op = crate::worldedit::BrushOp::ALL[op.min(5)];
                            if crate::worldedit::brush(s, op, r, cube, block) > 0 {
                                self.audio.play(crate::audio::Sfx::Place(crate::audio::Mat::of(block)), 0.6);
                            }
                        }
                    }
                    // The world script: timers and "tick", plus block events.
                    if let Some(l) = &mut self.lua {
                        if !frozen {
                            l.tick(s, dt as f32);
                        }
                        for e in &events {
                            match e {
                                InteractionEvent::Broken { block, pos } => {
                                    l.event(s, "block_broken", block.info().name);
                                    l.block_event(s, "broken", *block, *pos);
                                }
                                InteractionEvent::Placed { block, pos } => {
                                    l.event(s, "block_placed", block.info().name);
                                    l.block_event(s, "placed", *block, *pos);
                                }
                                _ => {}
                            }
                        }
                        let now = self.time.elapsed() as f64;
                        for (k, t) in l.take_output() {
                            self.chat.push(k, t, now);
                        }
                    }
                    // Changes the game made by itself (buttons springing back) go to the server too.
                    for (pos, block) in s.net_out.drain(..) {
                        if let Some(net) = &mut self.net {
                            net.send(&wolfcraft_net::C2S::Edit { cell: [pos.f as i32, pos.i, pos.j, pos.k], block: block as u16, hell: s.hell });
                        }
                    }
                    self.audio.volume = self.settings.volume;
                    // Footsteps: one per half walk-cycle while on the ground.
                    let step_no = (s.player.anim_phase / std::f32::consts::PI).floor() as i64;
                    if step_no != self.last_step {
                        self.last_step = step_no;
                        if s.player.body.on_ground && s.player.anim_amount > 0.3 && !s.player.sneaking {
                            // The step sounds like what is underfoot.
                            let feet = s.player.body.pos;
                            let below = s.chunks.grid().world_to_cell(feet - wolfcraft_engine::physics::up_at(feet) * 0.3);
                            let mat = s.chunks.get_block(below).filter(|b| b.is_solid()).map(crate::audio::Mat::of).unwrap_or(crate::audio::Mat::Grass);
                            self.audio.play(crate::audio::Sfx::Step(mat), 0.8);
                        }
                    }
                    if s.player.survival.health < self.last_health - 0.01 {
                        self.audio.play(crate::audio::Sfx::Hurt, 1.0);
                    }
                    self.last_health = s.player.survival.health;
                    for e in events {
                        match e {
                            InteractionEvent::FrameUse{..}|InteractionEvent::ActivateBoss{..}=>{},
                            InteractionEvent::Slept { .. } | InteractionEvent::Sat { .. } => {}
                            InteractionEvent::OpenBuilder => {
                                self.inv_ui = InventoryUiState::default();
                                self.inv_ui.builder = true;
                                self.screen = Screen::Inventory;
                                self.near_table = false;
                            }
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
                            InteractionEvent::Placed { pos, block } if block == wolfcraft_engine::block::Block::Farmland => {
                                // Hoe: the turned soil flies up in clods with bits of grass.
                                let g = s.chunks.grid();
                                let c = g.cell_center(pos) + wolfcraft_engine::physics::up_at(g.cell_center(pos)) * 0.5;
                                let n = (12.0 * self.settings.particles) as usize;
                                s.mobs.chips(c, &[[0.40, 0.26, 0.14], [0.30, 0.19, 0.10], [0.36, 0.62, 0.22]], n, 2.6);
                                self.hand_place = 0.6;
                                self.audio.play(crate::audio::Sfx::Dig(crate::audio::Mat::Dirt), 1.0);
                                if let Some(net) = &mut self.net {
                                    net.send(&wolfcraft_net::C2S::Edit { cell: [pos.f as i32, pos.i, pos.j, pos.k], block: block as u16, hell: s.hell });
                                }
                            }
                            InteractionEvent::Placed { pos, block } => {
                                // A new sign asks for its text right away.
                                if matches!(block, wolfcraft_engine::block::Block::Sign | wolfcraft_engine::block::Block::SignB) {
                                    self.container = Some((pos, wolfcraft_engine::block::Block::Sign));
                                    self.inv_ui = InventoryUiState::default();
                                    self.screen = Screen::Inventory;
                                }
                                if let Some(net) = &mut self.net {
                                    net.send(&wolfcraft_net::C2S::Edit { cell: [pos.f as i32, pos.i, pos.j, pos.k], block: block as u16, hell: s.hell });
                                }
                                self.hand_place = 1.0;
                                self.audio.play(crate::audio::Sfx::Place(crate::audio::Mat::of(block)), 1.0);
                            }
                            InteractionEvent::Broken { pos, block } => {
                                // Final burst of chips in the colours of the block.
                                let n = (14.0 * self.settings.particles) as usize;
                                let corners = s.chunks.grid().cell_corners(pos);
                                let centre = corners.iter().fold(glam::DVec3::ZERO, |a, c| a + *c) / 8.0;
                                if let Some(cols) = self.tile_colors.get(block.info().tiles[0] as usize) {
                                    s.mobs.chips(centre, cols, n, 3.2);
                                }
                                // The last blow, by material: wood splinters, a cloud of dust from
                                // loose ground, a shower of leaves from foliage.
                                {
                                    use wolfcraft_engine::block::ToolKind;
                                    use wolfcraft_engine::mobs::FxKind;
                                    let up = wolfcraft_engine::physics::up_at(centre);
                                    let k = self.settings.particles.max(0.0);
                                    let cols = self.tile_colors.get(block.info().tiles[0] as usize).cloned().unwrap_or_default();
                                    let name = block.info().name;
                                    if name.contains("листв") || name.contains("хвоя") {
                                        for i in 0..(14.0 * k) as usize {
                                            let o = glam::DVec3::new((i as f64 * 1.3).sin(), (i as f64 * 2.1).cos(), (i as f64 * 0.7).sin()) * 0.45;
                                            s.mobs.fx(centre + o, up * 0.3, cols.get(i % cols.len().max(1)).copied().unwrap_or([0.3, 0.6, 0.2]), 0.07, 2.9, FxKind::Leaf);
                                        }
                                    } else if block.info().tool == ToolKind::Shovel && !cols.is_empty() {
                                        let dust = cols[0].map(|c| c * 0.7 + 0.25);
                                        for i in 0..(8.0 * k) as usize {
                                            let a = i as f64 * std::f64::consts::TAU / 8.0;
                                            let side = glam::DVec3::new(a.cos(), (a * 0.7).sin(), a.sin());
                                            let side = (side - up * side.dot(up)).normalize_or_zero();
                                            s.mobs.fx(centre + side * 0.3, side * 1.0 + up * 0.4, dust, 0.24, 1.1, FxKind::Rise);
                                        }
                                    } else if block.info().tool == ToolKind::Axe && !cols.is_empty() {
                                        for i in 0..(12.0 * k) as usize {
                                            let a = i as f64 * 2.399;
                                            let side = glam::DVec3::new(a.cos(), (a * 1.3).sin(), a.sin());
                                            let side = (side - up * side.dot(up)).normalize_or_zero();
                                            s.mobs.fx(centre, side * 3.0 + up * 3.0, cols[i % cols.len()], 0.04 + (i % 3) as f32 * 0.02, 1.2, FxKind::Fall);
                                        }
                                    }
                                }
                                // An ore bursts into glowing gem shards and a ring of sparkles.
                                if let (Some(gem), false) = (wolfcraft_engine::items::ore_glow(block), s.is_creative()) {
                                    use wolfcraft_engine::mobs::FxKind;
                                    let up = wolfcraft_engine::physics::up_at(centre);
                                    let k = self.settings.particles.max(0.3);
                                    for i in 0..(18.0 * k) as usize {
                                        let a = i as f64 * 2.399;
                                        let side = glam::DVec3::new(a.cos(), (a * 1.3).sin(), a.sin());
                                        let side = (side - up * side.dot(up)).normalize_or_zero();
                                        let vel = side * (2.0 + (i % 5) as f64 * 0.6) + up * (3.0 + (i % 3) as f64 * 1.2);
                                        let col = if i % 4 == 0 { [1.0, 1.0, 1.0] } else { gem };
                                        s.mobs.fx_glow(centre, vel, col, 0.05 + (i % 3) as f32 * 0.015, 1.1, FxKind::Fall);
                                    }
                                    for i in 0..(10.0 * k) as usize {
                                        let a = i as f64 * std::f64::consts::TAU / 10.0;
                                        let side = glam::DVec3::new(a.cos(), 0.3, a.sin());
                                        let side = (side - up * side.dot(up)).normalize_or_zero();
                                        s.mobs.fx_glow(centre + side * 0.3, side * 1.5 + up * 0.5, [1.0, 0.97, 0.80], 0.03, 0.5, FxKind::Rise);
                                    }
                                    self.audio.play(crate::audio::Sfx::Pop, 1.0);
                                }
                                if let Some(net) = &mut self.net {
                                    net.send(&wolfcraft_net::C2S::Edit { cell: [pos.f as i32, pos.i, pos.j, pos.k], block: 0, hell: s.hell });
                                }
                                self.audio.play(crate::audio::Sfx::Break(crate::audio::Mat::of(block)), 1.0);
                            }
                            InteractionEvent::Shattered { at, block } => {
                                // A falling or thrown block breaks apart where it lands.
                                if let Some(cols) = self.tile_colors.get(block.info().tiles[0] as usize) {
                                    s.mobs.chips(at, cols, (6.0 * self.settings.particles) as usize, 4.0);
                                }
                                if self.thud_cd <= 0.0 {
                                    let dist = (at - s.player.eye_pos()).length() as f32;
                                    self.audio.play(crate::audio::Sfx::Break(crate::audio::Mat::of(block)), (1.0 / (1.0 + dist / 10.0)).clamp(0.1, 1.0));
                                    self.thud_cd = 0.06;
                                }
                            }
                            InteractionEvent::Landed { pos, block } => {
                                if self.thud_cd <= 0.0 {
                                    let at = s.chunks.grid().cell_center(pos);
                                    let dist = (at - s.player.eye_pos()).length() as f32;
                                    self.audio.play(crate::audio::Sfx::Place(crate::audio::Mat::of(block)), (1.2 / (1.0 + dist / 10.0)).clamp(0.1, 1.0));
                                    self.thud_cd = 0.06;
                                }
                                if let Some(cols) = self.tile_colors.get(block.info().tiles[0] as usize) {
                                    let at = s.chunks.grid().cell_center(pos);
                                    s.mobs.chips(at, &[cols[0]], (2.0 * self.settings.particles) as usize, 2.0);
                                }
                            }
                            InteractionEvent::Ate { .. } => {
                                self.audio.play(crate::audio::Sfx::Eat, 1.0);
                                self.eat_anim = 1.0;
                                // Crumbs fly from the mouth.
                                let eye = s.player.eye_pos();
                                let look = s.player.look_dir();
                                s.mobs.chips(eye + look * 0.45 - s.player.up() * 0.15, &[[0.85, 0.62, 0.32], [0.95, 0.85, 0.6], [0.7, 0.45, 0.25]], (6.0 * self.settings.particles) as usize, 1.0);
                            }
                            InteractionEvent::Crank { .. } => self.audio.play(crate::audio::Sfx::Place(crate::audio::Mat::Wood), 0.6),
                            InteractionEvent::Equipped => {
                                self.audio.play(crate::audio::Sfx::Equip, 1.0);
                                self.equip_anim = 1.0;
                            }
                            InteractionEvent::Dropped { .. } => {}
                            InteractionEvent::Ignite { pos } => {
                                // The dynamite block turned into a burning charge.
                                if let Some(net) = &mut self.net {
                                    net.send(&wolfcraft_net::C2S::Edit { cell: [pos.f as i32, pos.i, pos.j, pos.k], block: 0, hell: s.hell });
                                }
                                self.audio.play(crate::audio::Sfx::Fuse, 1.0);
                            }
                            InteractionEvent::Fire { weapon } => {
                                use wolfcraft_engine::items::ids;
                                // Light guns kick less (an automatic would shake the view away).
                                let (kick, sfx) = match weapon {
                                    ids::PISTOL => (0.75, Some(0)),
                                    ids::RIFLE => (0.55, Some(1)),
                                    ids::SHOTGUN => (1.0, Some(2)),
                                    ids::SNIPER => (1.0, Some(3)),
                                    ids::RAILGUN => (1.0, Some(3)),
                                    ids::PLASMA_RIFLE => (0.4, Some(1)),
                                    _ => (1.0, None),
                                };
                                self.gun_recoil = self.gun_recoil.max(kick);
                                match sfx {
                                    Some(k) => self.audio.play(crate::audio::Sfx::Shot(k), 1.0),
                                    None => self.audio.play(crate::audio::Sfx::Launch, 1.0),
                                }
                            }
                            InteractionEvent::BulletHit { creature, .. } => {
                                if creature {
                                    self.audio.play(crate::audio::Sfx::BulletHit, 0.8);
                                    self.hit_marker = 0.25;
                                }
                            }
                            InteractionEvent::Exploded { at, power } => {
                                let dist = (at - s.player.eye_pos()).length() as f32;
                                let gain = (1.6 * power / 4.0 / (1.0 + dist / 12.0)).clamp(0.05, 1.6);
                                self.audio.play(crate::audio::Sfx::Explosion, gain);
                            }
                            InteractionEvent::Blasted { pos, block } => {
                                // A couple of chips per block: a crater has hundreds of them.
                                let n = (2.0 * self.settings.particles).round() as usize;
                                if n > 0 {
                                    if let Some(cols) = self.tile_colors.get(block.info().tiles[0] as usize) {
                                        let centre = s.chunks.grid().cell_center(pos);
                                        s.mobs.chips(centre, cols, n, 6.0);
                                    }
                                }
                                if let Some(net) = &mut self.net {
                                    net.send(&wolfcraft_net::C2S::Edit { cell: [pos.f as i32, pos.i, pos.j, pos.k], block: 0, hell: s.hell });
                                }
                            }
                            InteractionEvent::BuyEgg { .. } => self.audio.play(crate::audio::Sfx::Pop, 1.0),
                            InteractionEvent::Hatch { .. } => self.audio.play(crate::audio::Sfx::Launch, 0.6),
                            InteractionEvent::WorldSet { pos, block } => {
                                if let Some(net) = &mut self.net {
                                    net.send(&wolfcraft_net::C2S::Edit { cell: [pos.f as i32, pos.i, pos.j, pos.k], block: block as u16, hell: s.hell });
                                }
                            }
                            InteractionEvent::SpawnMob { pos, .. } => {
                                // The soul bursts into a puff where the creature appears.
                                let c = s.chunks.grid().cell_center(pos);
                                s.mobs.chips(c, &[[0.95, 0.95, 1.0], [0.80, 0.90, 1.0]], (14.0 * self.settings.particles) as usize, 2.0);
                                self.audio.play(crate::audio::Sfx::Pop, 1.0);
                            }
                        }
                    }
                    if std::mem::take(&mut s.picked_up) > 0 {
                        self.audio.play(crate::audio::Sfx::Pop, 0.8);
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
            // Relief height maps are built only once relief is switched on.
            if self.settings.relief_mode > 0 {
                gfx.world.ensure_relief(&gfx.gpu.device, &gfx.gpu.queue, &gfx.atlases);
            }
        }
        let upload_budget = if self.screen == Screen::Loading { 64 } else { 16 };
        self.pump_meshes(gfx, upload_budget);

        // PC: hold Alt to free the cursor and click the panels over the world (clicker, arena
        // shop, script GUIs); release it to look around again.
        let alt = self.input.keys.contains(&KeyCode::AltLeft) || self.input.keys.contains(&KeyCode::AltRight);
        let want_grab = self.screen == Screen::Playing && !self.chat.open && self.web_pages.is_empty() && !alt;
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
        let mut voice_changed = false;
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
        let mut ota_requested = false;
        let mut close_inventory = false;
        let mut submitted: Vec<String> = Vec::new();
        let lua_gui: Vec<crate::lua::GuiElement> = self.lua.as_ref().map(|l| l.gui()).unwrap_or_default();
        let mut lua_clicks: Vec<String> = Vec::new();
        let mut clicker_acts: Vec<ClickAct> = Vec::new();
        let mut arena_buys: Vec<usize> = Vec::new();
        let mut studio_actions: Vec<wolfcraft_ui::studio_ui::StudioAction> = Vec::new();
        let mut studio_cover: Vec<egui::Rect> = Vec::new();
        let mut stellar_cover: Vec<egui::Rect> = Vec::new();
        // Mini-game panels and mod windows over the world: touches there are theirs.
        let mut ui_cover: Vec<egui::Rect> = Vec::new();
        let icons = gfx.gui.icons.clone();
        let version = wolfcraft_common::VERSION;
        let data_dir_str = self.data_dir.display().to_string();
        let debug_info = if self.debug && self.screen.in_world() { self.session.as_ref().map(|s| self.debug_info(s, gfx)) } else { None };
        let screen = self.screen;
        let dark = self.settings.theme == ThemeMode::Dark;
        // What WolfHTML pages read through `wolf.*`.
        let web_host = self.session.as_ref().map(|s| {
            let p = s.player.body.pos;
            let f = wolfcraft_engine::player::day_fraction(s.player.world_time);
            let mins = (f * 24.0 * 60.0) as u32;
            wolfcraft_ui::webgui::Host { health: s.player.survival.health, max_health: 20.0, hunger: s.player.survival.hunger, pos: [p.x, p.y, p.z], time: format!("{:02}:{:02}", mins / 60 % 24, mins % 60), nick: s.nickname.clone(), creative: s.player.is_creative() }
        }).unwrap_or_default();
        let mut web_fx: Vec<wolfcraft_ui::webgui::Effect> = Vec::new();
        let mut web_log: Vec<String> = Vec::new();

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
        let studio_view = if self.studio.is_some() { self.studio_view() } else { None };
        let frame = {
            let App { world_map, map_view, map_marks, map_grid, settings, world_select, create, worlds, create_preview, menu_planet, crash_report, packs, pack_status, session, inv_ui, load, near_table, time, chat, container, touch, studio, studio_world, voice, voice_menu, net_players, net, web_pages, .. } = self;
            let touch_size = studio_touch_size(gfx.gpu.size(), studio.as_ref(), gfx.gui.ctx.pixels_per_point());
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
                    actions.push(main_menu::main_menu(ctx, &icons, version, &data_dir_str, dark, menu_planet.as_ref(), settings.skin, &settings.nickname, worlds));
                    crash_close |= show_crash_report(ctx, &icons, crash_report.as_deref());
                }
                Screen::GameMenu => {
                    actions.push(main_menu::game_menu(ctx, &icons, version, menu_planet.as_ref()));
                    crash_close |= show_crash_report(ctx, &icons, crash_report.as_deref());
                }
                Screen::WorldSelect => actions.push(main_menu::world_select(ctx, &icons, world_select, worlds, version)),
                Screen::Create => actions.push(main_menu::create_world(ctx, &icons, create, create_preview.as_ref(), version, random_seed)),
                Screen::Settings => {
                    let folder = if cfg!(target_os = "android") { ANDROID_PACKS_DIR.to_string() } else { data_dir_str.clone() + "/resourcepacks" };
                    let r = main_menu::settings_screen(ctx, &icons, settings, version, &main_menu::PacksView { list: packs, status: pack_status, folder: &folder });
                    if r.open_updates { ota_requested = true; }
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
                        if studio.as_ref().is_some_and(|st| !st.playtest) || touch.place_play {
                            touch.clear_hud();
                        } else {
                            touch.sync_hotbar(ctx.pixels_per_point());
                        }
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
                            clock: format!("{} · {}", s.clock(), s.season_label()),
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
                            weapon_reload: s.explosives.reload(),
                            hit_marker: self.hit_marker,
                            // Sign texts are drawn on the signs in the world now, not as a plate.
                            sign_text: None,
                            item_name: s.item_name.clone(),
                            show_fps: settings.show_fps,
                            debug: debug_info.as_ref(),
                            compass: Some(compass_bearing(p.heading, p.up()) as f32),
                            minimap: if studio.is_some() { None } else { minimap_tex },
                            compact: cfg!(target_os = "android"),
                            crosshair: !(touch.seen && settings.tap_aim),
                            effects: p.effects.iter().map(|a| (format!("{} {}", a.effect.name(), wolfcraft_engine::magic::roman(a.level)), a.effect.color(), a.left)).collect(),
                            look: if screen == Screen::Playing && settings.show_look { look_info(s) } else { None },
                            goal: s.goal().map(|a| (wolfcraft_engine::items::ItemStack::new(a.icon, 1), a.desc.to_string())),
                            tip: s.first_steps().then(|| {
                                if touch.seen {
                                    "Джойстик — идти · держите палец на блоке — ломать · тап — поставить".to_string()
                                } else {
                                    "WASD — идти · Пробел — прыжок · ЛКМ — ломать · ПКМ — поставить · E — инвентарь".to_string()
                                }
                            }),
                            achievement: s.ach_toasts.first().map(|(i, age)| {
                                let a = &crate::achievements::ALL[*i];
                                (wolfcraft_engine::items::ItemStack::new(a.icon, 1), a.name.to_string(), a.desc.to_string(), *age)
                            }),
                            pickups: s.pickups.iter().map(|(id, n, age)| (wolfcraft_engine::items::ItemStack::new(*id, (*n).min(255) as u8), *age)).collect(),
                            popups: {
                                // Damage numbers: from the world onto the screen.
                                let cam = s.camera();
                                let fwd = cam.forward.normalize();
                                let right = fwd.cross(cam.up).normalize();
                                let up = right.cross(fwd);
                                let scr = ctx.screen_rect();
                                let tan = (settings.fov.to_radians() * 0.5).tan() as f64;
                                let aspect = (scr.width() / scr.height().max(1.0)) as f64;
                                s.popups
                                    .iter()
                                    .filter_map(|pp| {
                                        let d = pp.at - cam.eye;
                                        let z = d.dot(fwd);
                                        if z < 0.3 {
                                            return None;
                                        }
                                        let x = d.dot(right) / z / (tan * aspect);
                                        let y = d.dot(up) / z / tan;
                                        Some((egui::Pos2::new(scr.center().x + x as f32 * scr.width() * 0.5, scr.center().y - y as f32 * scr.height() * 0.5), pp.text.clone(), pp.color, pp.age, pp.big))
                                    })
                                    .collect()
                            },
                        };
                        // Studio edit mode: no game GUI (hotbar, bars, minimap) — like Roblox
                        // Studio, the place's own GUI comes from its scripts; only the phone
                        // controls stay.
                        let studio_edit = studio.as_ref().is_some_and(|st| !st.playtest);
                        if screen != Screen::Map && !studio_edit {
                            // A place in play draws its own GUI: no hotbar, bars, minimap or
                            // crosshair from the game (its mini-game panels and voice stay).
                            if !touch.place_play {
                                hud::draw(ctx, &icons, &hud);
                            }
                            petsim_overlay(ctx, &s.petsim, s.petsim_home.is_some());
                            voice_changed |= voice_ui(ctx, &icons, voice, voice_menu, settings, net_players, net.as_ref().map(|n| n.voice), time.elapsed() as f64);
                            buttonsim_overlay(ctx, &s.buttonsim);
                            // Mini-game panels only over the world itself (not over the inventory,
                            // a chest or the pause menu).
                            if s.clicker.active && (s.minigames || s.studio) && !studio_edit && screen == Screen::Playing {
                                clicker_acts = clicker_overlay(ctx, &icons, &s.clicker, &mut ui_cover);
                            }
                            if let Some(a) = &s.arena {
                                if !studio_edit && matches!(screen, Screen::Playing | Screen::Dead) {
                                    arena_buys = arena_overlay(ctx, a, &s.player.inventory, &mut ui_cover);
                                }
                            }
                            // WolfTech drill: a glass pill with the digging square; tap to change.
                            if screen == Screen::Playing && !studio_edit && s.player.inventory.held().is_some_and(|h| h.id == wolfcraft_engine::items::ids::DRILL) && drill_overlay(ctx, s.player.drill_area, &mut ui_cover) {
                                s.player.drill_area = wolfcraft_engine::tools::next_drill_mode(s.player.drill_area);
                                let n = wolfcraft_engine::tools::drill_side(s.player.drill_area);
                                s.item_name = Some((format!("Бур: копает {n}×{n}"), 1.4));
                            }
                            // «Звёздный пульт»: its button and skill menu, cinema bars, flashes.
                            let holding = screen == Screen::Playing && s.player.inventory.held().is_some_and(|h| h.id == wolfcraft_engine::items::ids::STAR_REMOTE);
                            let flash = s.flash;
                            let cinema = s.cinema;
                            let title = s.title.clone();
                            let o = wolfcraft_ui::stellar_ui::draw(ctx, &icons, &mut s.stellar_ui, holding, flash, cinema, title.as_ref().map(|t| (t.0.as_str(), t.1.as_str(), t.2)));
                            if o.skip {
                                s.skip_film = true;
                                s.cutscene = None;
                            }
                            stellar_cover = o.cover;
                            if let Some(mob)=s.mobs.mobs.iter().find(|m|m.boss.is_some()&&m.hp>0.0) {
                                let role=mob.boss.as_ref().unwrap();
                                egui::Area::new(egui::Id::new("guardian_health")).anchor(egui::Align2::CENTER_TOP,egui::vec2(0.,10.)).interactable(false).show(ctx,|ui|{
                                    let width=(ctx.screen_rect().width()*0.46).clamp(160.,420.);
                                    ui.set_width(width);
                                    let c=role.kind.color();let color=egui::Color32::from_rgb((c[0]*255.) as u8,(c[1]*255.) as u8,(c[2]*255.) as u8);
                                    ui.add(egui::ProgressBar::new((mob.hp/role.kind.max_hp()).clamp(0.,1.)).desired_width(width).fill(color).text(format!("{} · фаза {}",role.kind.name(),role.kind.phase(mob.hp))));
                                    if role.casting>0. {ui.label(egui::RichText::new("Удар по отмеченной области — отойдите!").color(egui::Color32::YELLOW));}
                                });
                            }
                        }
                        // The world script's own GUI (Luau, like Roblox ScreenGui).
                        if screen == Screen::Playing && !lua_gui.is_empty() {
                            lua_clicks.extend(draw_lua_gui(ctx, &icons, &lua_gui));
                        }
                        // Mod windows in HTML + CSS + JS.
                        if screen == Screen::Playing {
                            for p in web_pages.iter_mut() {
                                web_fx.extend(p.show(ctx, &web_host, dt));
                                web_log.extend(p.take_log());
                                if let Some(r) = ctx.memory(|m| m.area_rect(egui::Id::new(("wolfhtml", &p.name)))) {
                                    ui_cover.push(r);
                                }
                            }
                            web_pages.retain(|p| p.open);
                        }
                        if screen == Screen::Playing {
                            if let Some(line) = chat_ui::draw(ctx, &icons, chat, time.elapsed() as f64) {
                                submitted.push(line);
                            }
                        }
                        if let (Some(st), Screen::Playing) = (studio.as_mut(), screen) {
                            let mut view = studio_view.clone().unwrap_or_default();
                            view.now = time.elapsed() as f64;
                            let (a, cover) = wolfcraft_ui::studio_ui::draw(ctx, &icons, st, &view, &mut s.player.inventory);
                            studio_actions = a;
                            studio_cover = cover;
                        }
                        let _ = &studio_world;
                        // Asleep: the screen goes dark and "Zzz" floats up, then the morning fades in.
                        let fade = s.sleep_fade();
                        if fade > 0.0 {
                            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("sleep")));
                            let r = ctx.screen_rect();
                            painter.rect_filled(r, 0.0, egui::Color32::from_rgba_unmultiplied(4, 6, 18, (fade * 255.0) as u8));
                            let t = time.elapsed();
                            for i in 0..3 {
                                let k = ((t * 0.5 + i as f32 * 0.33) % 1.0) as f32;
                                let pos = r.center() + egui::vec2(20.0 + k * 50.0 + i as f32 * 6.0, -10.0 - k * 80.0);
                                let a = ((1.0 - k) * fade * 255.0) as u8;
                                painter.text(pos, egui::Align2::CENTER_CENTER, "Z", egui::FontId::proportional(18.0 + k * 22.0), egui::Color32::from_rgba_unmultiplied(200, 215, 255, a));
                            }
                            painter.text(r.center() + egui::vec2(0.0, 60.0), egui::Align2::CENTER_CENTER, format!("Ночь проходит… {}", s.clock()), egui::FontId::proportional(16.0), egui::Color32::from_rgba_unmultiplied(220, 225, 240, (fade * 255.0) as u8));
                        }
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
                                    use wolfcraft_engine::block::Block as B;
                                    let furnace = block.base_kind() == B::ElectricFurnace;
                                    let forge = block == B::MachineCasing;
                                    let magic = block == B::EnchantTable;
                                    let tech = wolfcraft_engine::wolftech::kind(block);
                                    // «Механика» machines (press, millstone): they run while turning.
                                    let kin = wolfcraft_engine::kinetic::part(block).filter(|p| wolfcraft_engine::kinetic::is_machine(*p));
                                    let kin_node = s.kin.get(&pos).copied().unwrap_or_default();
                                    let (prog, working) = match (kin, s.tech_status(pos, block)) {
                                        (Some(_), _) => (kin_node.progress / 2.0, kin_node.speed > 0.0),
                                        (None, Some((p, w, _, _))) => (p, w),
                                        _ => s.furnace_status(pos),
                                    };
                                    let kin_text = if kin.is_none() {
                                        String::new()
                                    } else if kin_node.speed > 0.0 {
                                        format!("⚙ {:.0} об/мин · нагрузка {:.0} / {:.0} SU", kin_node.speed, kin_node.load, kin_node.cap)
                                    } else if kin_node.cap > 0.0 && kin_node.load > kin_node.cap {
                                        format!("⚠ Перегрузка: {:.0} / {:.0} SU — добавьте водяных колёс", kin_node.load, kin_node.cap)
                                    } else {
                                        "Не крутится: подведите вал от водяного колеса или кривошипа".to_string()
                                    };
                                    let tech_text = match (tech, s.tech_status(pos, block)) {
                                        (Some(k), Some((_, w, e, cap))) => {
                                            let sp = wolfcraft_engine::wolftech::spec(k);
                                            let what = if w {
                                                match k {
                                                    k if wolfcraft_engine::wolftech::is_source(k) => format!("даёт {:.0} ЭЕ/с", s.tech.get(&pos).map(|t| t.flow).unwrap_or(sp.rate)),
                                                    _ => format!("работает · {:.0} ЭЕ/с", sp.rate),
                                                }
                                            } else {
                                                sp.hint.to_string()
                                            };
                                            let (make, held, n) = s.tech_net(pos);
                                            let (tm, em, _) = s.tech_mods(pos, block);
                                            let what = if k == wolfcraft_engine::wolftech::Kind::Reactor {
                                                let (h, m) = s.reactor_heat(pos);
                                                format!("нагрев {:.0} / {:.0} · {what}", h, m)
                                            } else {
                                                what
                                            };
                                            let what = if wolfcraft_engine::wolftech::is_machine(k) && tm < 0.999 {
                                                format!("{what} · скорость ×{:.1}, расход ×{:.1}", 1.0 / tm, em)
                                            } else {
                                                what
                                            };
                                            format!("⚡ {:.0} / {:.0} ЭЕ · {} · сеть: {} шт., +{:.0} ЭЕ/с, всего {:.0}", e, cap, what, n, make, held)
                                        }
                                        _ => String::new(),
                                    };
                                    let status_text = if kin.is_some() { kin_text.as_str() } else if tech.is_some() { tech_text.as_str() } else if working { "Плавится…" } else { "Положите руду и топливо (уголь, дрова)" };
                                    let accepts = |slot: usize, st: &wolfcraft_engine::items::ItemStack| {
                                        if wolfcraft_engine::construction::is_portable(block)&&st.info().place.is_some_and(wolfcraft_engine::construction::is_portable){return false;}
                                        if let Some(p) = kin {
                                            return slot == 0 && wolfcraft_engine::kinetic::recipe(p, st.id).is_some();
                                        }
                                        if let Some(k) = tech {
                                            use wolfcraft_engine::wolftech::Kind;
                                            match k {
                                                Kind::Generator | Kind::Magma => slot == 1 && wolfcraft_engine::wolftech::fuel_for(k, st.id) > 0.0,
                                                Kind::Reactor => wolfcraft_engine::wolftech::is_reactor_part(st.id),
                                                k if wolfcraft_engine::wolftech::is_machine(k) => (slot == 0 && wolfcraft_engine::wolftech::recipe(k, st.id).is_some()) || (slot >= 3 && wolfcraft_engine::wolftech::is_upgrade(st.id)),
                                                Kind::Cell | Kind::BigCell => slot == 0 && wolfcraft_engine::items::rechargeable(st.id),
                                                // Quarry and magnet: their slots are only for taking out.
                                                _ => false,
                                            }
                                        } else if furnace {
                                            (slot == 0 && wolfcraft_engine::interaction::smelt_result(st.id).is_some()) || (slot == 1 && wolfcraft_engine::interaction::fuel_time(st.id) > 0.0)
                                        } else if forge {
                                            slot < 2
                                        } else if magic {
                                            (slot == 0 && st.count == 1 && wolfcraft_engine::magic::kind_of(st.id) != wolfcraft_engine::magic::Kind::Other) || (slot == 1 && st.id == B::IskritWire as u16)
                                        } else {
                                            true
                                        }
                                    };
                                    // WolfTech facts for the chips under the slots.
                                    let stats: Vec<String> = match tech {
                                        Some(k) => {
                                            let sp = wolfcraft_engine::wolftech::spec(k);
                                            let mut v = Vec::new();
                                            if let Some((_, _, e, cap)) = s.tech_status(pos, block) {
                                                v.push(format!("⚡ {:.0} / {:.0} ЭЕ", e, cap));
                                            }
                                            let (tm, em, _) = s.tech_mods(pos, block);
                                            let st = s.tech.get(&pos).copied().unwrap_or_default();
                                            if wolfcraft_engine::wolftech::is_source(k) {
                                                v.push(format!("+{:.1} ЭЕ/с", st.flow));
                                            } else if sp.rate > 0.0 {
                                                v.push(format!("−{:.0} ЭЕ/с", sp.rate * em));
                                            }
                                            if tm < 0.999 {
                                                v.push(format!("скорость ×{:.1}", 1.0 / tm.max(0.01)));
                                            }
                                            let (make, _, machines) = s.tech_net(pos);
                                            if machines > 1 {
                                                v.push(format!("сеть: {machines} бл. · +{make:.0} ЭЕ/с"));
                                            }
                                            if k == wolfcraft_engine::wolftech::Kind::Quarry {
                                                v.push(format!("глубина {}", st.progress as i32 / 25 + 1));
                                            }
                                            v
                                        }
                                        None => Vec::new(),
                                    };
                                    let view = wolfcraft_ui::container_ui::ContainerView {
                                        stats: &stats,
                                        skin:wolfcraft_ui::gui_renderer::block_panel(block),
                                        title: if let Some(p) = kin { wolfcraft_engine::kinetic::name(p) } else if let Some(k) = tech { wolfcraft_engine::wolftech::spec(k).name } else if furnace { "Печь" } else if forge { "Кузница" } else if magic { "Чаровальня" } else if block == B::Battery { "Бочка" } else if wolfcraft_engine::construction::entry(block as u16).is_some() { block.info().name } else { "Сундук" },
                                        columns: if furnace || forge || tech.is_some() || kin.is_some() { 3 } else if magic { 2 } else { 9 },
                                        accepts: &accepts,
                                        layout: if kin.is_some() {
                                            wolfcraft_ui::container_ui::Layout::Machine
                                        } else if let Some(k) = tech {
                                            use wolfcraft_engine::wolftech::Kind;
                                            match k {
                                                Kind::Generator | Kind::Magma => wolfcraft_ui::container_ui::Layout::Generator,
                                                Kind::Reactor => wolfcraft_ui::container_ui::Layout::Reactor,
                                                Kind::Solar | Kind::Wind | Kind::Geo | Kind::Water | Kind::Dynamo | Kind::Motor | Kind::Lunar | Kind::Tesla | Kind::ChargePad => wolfcraft_ui::container_ui::Layout::Panel,
                                                Kind::Cell | Kind::BigCell => wolfcraft_ui::container_ui::Layout::Charger,
                                                Kind::Quarry | Kind::Magnet => wolfcraft_ui::container_ui::Layout::Grid,
                                                _ => wolfcraft_ui::container_ui::Layout::Machine,
                                            }
                                        } else if furnace { wolfcraft_ui::container_ui::Layout::Furnace } else if forge { wolfcraft_ui::container_ui::Layout::Forge } else if magic { wolfcraft_ui::container_ui::Layout::Enchant } else { wolfcraft_ui::container_ui::Layout::Grid },
                                        working: (furnace || tech.is_some() || kin.is_some()) && working,
                                        energy: s.tech_status(pos, block).map(|(_, _, e, cap)| e / cap.max(1.0)),
                                        heat: (tech == Some(wolfcraft_engine::wolftech::Kind::Reactor)).then(|| {
                                            let (h, m) = s.reactor_heat(pos);
                                            h / m.max(1.0)
                                        }),
                                        status: if furnace || tech.is_some() || kin.is_some() {
                                            Some((status_text, prog))
                                        } else if forge {
                                            Some(("Предмет + его материал — ремонт · два одинаковых — склеить · +3 материала уровнем выше — улучшить · + книга чар — зачаровать", 0.0))
                                        } else if magic {
                                            Some(("Слева предмет или книга, справа искрит · чары стоят уровни опыта и искрит", 0.0))
                                        } else {
                                            None
                                        },
                                    };
                                    let mut slots = std::mem::take(s.container_slots(pos, block));
                                    let creative = s.is_creative();
                                    let seed = &mut s.ench_seed;
                                    let surv = &mut s.player.survival;
                                    let mut extra = |ui: &mut egui::Ui, slots: &mut [Option<wolfcraft_engine::items::ItemStack>], _inv: &mut wolfcraft_engine::inventory::Inventory| {
                                        if !magic {
                                            return;
                                        }
                                        enchant_offers(ui, &icons, slots, surv, seed, creative);
                                    };
                                    let (closed, thrown) = wolfcraft_ui::container_ui::show_with(ctx, &icons, &view, &mut slots, &mut s.player.inventory, &mut extra);
                                    *s.container_slots(pos, block) = slots;
                                    for st in thrown {
                                        s.throw(st);
                                    }
                                    close_inventory |= closed;
                                } else {
                                    let creative = s.is_creative();
                                    let r = inventory_ui::show(ctx, &icons, inv_ui, &mut s.player.inventory, creative, *near_table, ctx_dt);
                                    for st in r.thrown {
                                        s.throw(st);
                                    }
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
        // Phones: the game's own keyboard (Gboard cannot type into the game window).
        gfx.gui.osk_enabled = cfg!(target_os = "android");

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
        if ota_requested { self.check_updates(); }
        if let Some(url) = pack_download {
            self.start_pack_download(url);
        }
        self.poll_pack_download();
        if settings_changed {
            self.apply_settings(gfx);
        }
        if voice_changed {
            self.settings.save(&self.data_dir);
        }
        if settings_back {
            self.apply_settings(gfx);
            self.pop();
        }
        if close_inventory {
            self.screen = Screen::Playing;
            self.near_table = false;
            self.inv_ui.builder = false;
            self.container = None;
        }
        if std::mem::take(&mut self.world_select.paste_request) {
            // «Вставить» in «По коду»: the first line of the clipboard (a code or an address).
            match crate::clipboard::get().map(|t| t.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string()).filter(|t| !t.is_empty()) {
                Some(text) => {
                    let text = text.trim_start_matches("Код для входа:").trim_start_matches("Код:").trim().to_string();
                    self.world_select.address = text.chars().take(64).collect();
                    self.world_select.net_error = None;
                }
                None => {
                    self.world_select.net_error = Some(if cfg!(target_os = "android") { "Буфер обмена пуст — скопируйте код или адрес сервера".into() } else { "Вставьте код в поле: Ctrl+V".into() });
                }
            }
        }
        if self.world_select.servers_changed {
            self.world_select.servers_changed = false;
            self.settings.servers = self.world_select.servers.clone();
            self.settings.save(&self.data_dir);
        }
        // Studio panels: keep the touch controls off them, then do what the ribbon asked.
        let ppp = gfx.gui.ctx.pixels_per_point();
        // The studio's edit mode has its ribbon on top instead of the game's top bar; in
        // «Играть» the place runs as players see it, with the game's bar.
        self.touch.studio = self.studio.as_ref().is_some_and(|st| !st.playtest);
        // A place played like in Roblox: its GUI is its own (scripts). That is «Играть» in
        // the studio and a place joined over the network.
        self.touch.place_play = self.place_play();
        if let Some(s) = &self.session {
            self.touch.creative = s.player.is_creative();
            self.touch.flying = s.player.body.flying;
        }
        if self.screen == Screen::Playing {
            self.touch.blocked = studio_cover.iter().chain(stellar_cover.iter()).chain(ui_cover.iter()).map(|r| (r.left() * ppp, r.top() * ppp, r.right() * ppp, r.bottom() * ppp)).collect();
            // The studio's tools sidebar on the left: the stick moves out from under it.
            self.touch.left_inset = self.studio.as_ref().filter(|st| !st.playtest).and_then(|st| st.view_left).map(|x| x * ppp).unwrap_or(0.0);
        }
        for a in studio_actions {
            self.studio_action(a, &mut submitted, &mut actions);
        }
        for a in actions {
            self.handle_action(a, gfx);
        }
        let now = self.time.elapsed() as f64;
        if self.session.is_some() && self.screen == Screen::Playing && !self.queued_chat.is_empty() {
            submitted.extend(std::mem::take(&mut self.queued_chat));
        }
        // What the mod pages asked for: chat lines and commands.
        for l in web_log {
            self.chat.push(wolfcraft_ui::chat_ui::ChatKind::System, l, now);
        }
        for e in web_fx {
            match e {
                wolfcraft_ui::webgui::Effect::Chat(t) => submitted.push(t),
                wolfcraft_ui::webgui::Effect::Command(c) => submitted.push(c),
                wolfcraft_ui::webgui::Effect::Close => {}
            }
        }
        // /gui: WolfHTML pages from the world's «gui» folder.
        submitted.retain(|line| {
            let t = line.trim();
            let Some(arg) = t.strip_prefix("/gui").filter(|r| r.is_empty() || r.starts_with(' ')) else { return true };
            self.open_web_page(arg.trim(), now);
            false
        });
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
                // The world script hears plain chat messages.
                if let (Some(l), false) = (&mut self.lua, line.starts_with('/')) {
                    l.event(s, "chat", &line);
                    for (k, t) in l.take_output() {
                        self.chat.push(k, t, now);
                    }
                }
            }
        }
        // Бед/Эггварс shop purchases.
        if let Some(s) = &mut self.session {
            for k in arena_buys {
                match s.arena_buy(k) {
                    Ok(t) | Err(t) => s.item_name = Some((t, 2.0)),
                }
            }
        }
        // «Кликер»: what was pressed in its panels.
        if let Some(s) = &mut self.session {
            let now = self.time.elapsed() as f64;
            for a in clicker_acts {
                let res: Result<String, String> = match a {
                    ClickAct::Tap => {
                        let got = s.clicker.tap();
                        if got > 0.0 {
                            let id = egui::Id::new("clicker-pops");
                            let mut pops: Vec<(f64, String, f32)> = gfx.gui.ctx.data(|d| d.get_temp(id)).unwrap_or_default();
                            pops.retain(|p| now - p.0 < 1.0);
                            pops.push((now, format!("+{}", wolfcraft_engine::buttonsim::short(got)), (now * 7919.0).fract() as f32));
                            gfx.gui.ctx.data_mut(|d| d.insert_temp(id, pops));
                            Ok(String::new())
                        } else {
                            Err("Рюкзак полон — продайте клики!".into())
                        }
                    }
                    ClickAct::Sell => {
                        let m = s.clicker.sell();
                        Ok(if m > 0.0 { format!("Продано: +{} денег", wolfcraft_engine::buttonsim::short(m)) } else { String::new() })
                    }
                    ClickAct::Upgrade(k) => s.clicker.buy_upgrade(k).map(|_| format!("{} — уровень {}", wolfcraft_engine::clicker::UPGRADES[k].0, s.clicker.upgrades[k])),
                    ClickAct::Rebirth(n) => s.clicker.buy_rebirths(n).map(|_| format!("+{n} перерожд. — всего {}! Множитель ×{:.1}", s.clicker.rebirths, s.clicker.rebirth_mult())),
                    ClickAct::Hatch(e) => {
                        let (r1, r2) = (s.mobs.rand(), s.mobs.rand());
                        s.clicker.hatch(e, r1, r2).map(|k| {
                            gfx.gui.ctx.data_mut(|d| d.insert_temp(egui::Id::new("clicker-hatched"), Some((now, k))));
                            let pet = &wolfcraft_engine::clicker::PETS[k];
                            format!("{} {} ×{}", pet.rarity.name(), pet.name, pet.mult)
                        })
                    }
                    ClickAct::Island(i) => s.clicker_go(i).map(|_| format!("{} · клики ×{}", wolfcraft_engine::clicker::ISLANDS[i].name, wolfcraft_engine::clicker::ISLANDS[i].mult)),
                };
                match res {
                    Ok(t) if !t.is_empty() => s.item_name = Some((t, 2.0)),
                    Err(e) => s.item_name = Some((e, 2.0)),
                    _ => {}
                }
            }
        }
        // Buttons of the script's GUI.
        if let (Some(l), Some(s)) = (&mut self.lua, &mut self.session) {
            let now = self.time.elapsed() as f64;
            for id in lua_clicks {
                l.click(s, &id);
            }
            for (k, t) in l.take_output() {
                self.chat.push(k, t, now);
            }
        }

        // ---- render ----
        let view_tex = output.texture.create_view(&Default::default());
        let mut encoder = gfx.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        let (w, h) = gfx.gpu.size();
        gfx.world.set_render_scale(self.settings.render_scale);
        gfx.world.set_post(self.settings.fx_bloom || self.settings.fx_rays);
        gfx.world.resize(&gfx.gpu.device, w, h);
        // While loading, the world is not drawn: the loading screen covers everything (a
        // half-built world showed through the strips beside a notch).
        if self.screen.in_world() {
            if let Some(s) = &mut self.session {
                let up = s.player.up();
                let sun = s.sun();
                let day = s.daylight();
                let d = up.dot(sun);
                let twilight = (-(d / 0.18).powi(2)).exp() as f32;
                let r = wolfcraft_engine::cubesphere::radial(s.player.eye_pos());
                // Server places (baseplate) are flat worlds: always under the sky, never space.
                let flat = s.cfg.world_type == wolfcraft_engine::config::WorldType::Baseplate && !s.hell;
                let altitude = if flat { 0.0 } else { ((r - s.planet.radius - 300.0) / (s.planet.radius * 0.4)).clamp(0.0, 1.0) as f32 };
                let underwater = s.eye_in_water();
                let mut sky = sky_color(day, twilight, altitude);
                // Voxel look: creatures and items keep sharp cube edges (no rounded plastic).
                wolfcraft_engine::renderer::model::set_rounded(false);
                // "Fast leaves" switched: rebuild the chunk meshes with the new look.
                if wolfcraft_engine::mesher::FAST_LEAVES.load(std::sync::atomic::Ordering::Relaxed) != self.settings.fast_leaves {
                    wolfcraft_engine::mesher::set_fast_leaves(self.settings.fast_leaves);
                    s.chunks.remesh_all();
                }
                if self.settings.fx_toon {
                    // Cartoon sky: a clean candy blue by day.
                    let k = (day as f32 * (1.0 - altitude)).clamp(0.0, 1.0) * 0.7;
                    let toon = [0.42, 0.78, 1.0];
                    for i in 0..3 {
                        sky[i] += (toon[i] - sky[i]) * k;
                    }
                }
                // Rain and storms: a grey, darker sky.
                let rain = s.rain_amount();
                if rain > 0.0 {
                    let grey = (sky[0] * 0.3 + sky[1] * 0.5 + sky[2] * 0.2) * 0.75;
                    for c in &mut sky {
                        *c += (grey - *c) * rain * 0.8;
                    }
                }
                if underwater {
                    sky = [0.04, 0.18, 0.36];
                } else if s.hell {
                    sky = [0.30, 0.05, 0.02];
                }
                let cam = s.camera();
                // Smooth FOV kick: wider while sprinting or flying fast, a touch narrower underwater.
                let fx_target = if s.bow_charge > 0.0 { -14.0 * s.bow_charge } else if s.player.sprinting { 9.0 } else if s.player.body.flying { 4.0 } else if underwater { -4.0 } else { 0.0 };
                self.fov_fx += (fx_target - self.fov_fx) * (1.0 - (-dt as f32 * 7.0).exp());
                let (season, snow, humidity) = s.environment();
                let params = FrameParams {
                    shaders: self.settings.shaders,
                    relief_mode: self.settings.relief_mode,
                    relief_strength: self.settings.relief_strength,
                    fx_flags: self.settings.fx_flags(),
                    season,
                    snow,
                    humidity: humidity.max(s.rain_amount()),
                    rain: if s.hell { 0.0 } else { s.rain_amount() },
                    fog_density: s.biome_fog().2 * (1.0 + s.rain_amount() * 1.2),
                    eye: cam.eye,
                    forward: cam.forward.as_vec3(),
                    up: cam.up.as_vec3(),
                    fov_y: (self.settings.fov + self.fov_fx).to_radians(),
                    aspect: w as f32 / h.max(1) as f32,
                    sun_dir: sun.as_vec3(),
                    day: day * (1.0 - s.rain_amount() * 0.3),
                    underwater,
                    view_distance_blocks: (self.settings.render_distance * 16) as f32,
                    time: self.time.elapsed(),
                    // The outline hugs the block's shape (torches, plants, slabs, doors…).
                    outline: if self.screen == Screen::Playing { s.hit.map(|h| block_outline(&s.chunks, h.cell, h.block)) } else { None },
                    crack: if self.screen == Screen::Playing { s.interaction.mining_fraction() } else { 0.0 },
                    viewmodel_fov_y: self.settings.vm_fov.to_radians(),
                };
                // Character model is visible in third-person views only.
                let mut model_verts = wolfcraft_engine::renderer::model::build_mob_models(&s.mobs.mobs, cam.eye, cam.forward, (self.settings.render_distance * 16) as f64 * 1.15);
                model_verts.extend(wolfcraft_engine::renderer::model::build_mob_models(&s.pet_models(), cam.eye, cam.forward, 64.0));
                model_verts.extend(wolfcraft_engine::renderer::model::build_dropped_models(&s.drops.list, cam.eye, self.time.elapsed(), 48.0));
                let frames:Vec<_>=s.storage_bank.frames.iter().filter(|((hell,c),_)|*hell==s.hell&&(s.chunks.grid().cell_center(*c)-cam.eye).length_squared()<48.0*48.0).filter_map(|((_,c),item)|s.chunks.get_block(*c).map(|b|(*c,b,*item))).collect();
                model_verts.extend(wolfcraft_engine::renderer::model::build_frame_models(&s.chunks.grid(),&frames,cam.eye));
                // Sign texts, written on the boards themselves.
                {
                    let grid = s.chunks.grid();
                    let near: Vec<_> = s
                        .block_commands
                        .iter()
                        .filter(|(c, _)| (grid.cell_center(**c) - cam.eye).length() < crate::signs::TEXT_DISTANCE)
                        .filter_map(|(c, t)| s.chunks.get_block(*c).filter(|b| matches!(b, wolfcraft_engine::block::Block::Sign | wolfcraft_engine::block::Block::SignB)).map(|b| (*c, b, t.clone())))
                        .collect();
                    if !near.is_empty() {
                        model_verts.extend(self.sign_texts.build(&gfx.gui.ctx, &grid, &near, cam.eye));
                    }
                }
                let arrows: Vec<(glam::DVec3, glam::DVec3)> = s.arrows.list.iter().map(|a| (a.pos, a.dir)).collect();
                model_verts.extend(wolfcraft_engine::renderer::model::build_arrow_models(&arrows, cam.eye));
                if s.camera_mode != crate::session::CameraMode::First && !s.player.survival.dead {
                    let (rest_at, rest_dir) = match s.rest {
                        Some(crate::session::Rest::Sleep { at, dir, .. }) | Some(crate::session::Rest::Sit { at, dir, .. }) => (Some(at), Some(dir)),
                        None => (None, None),
                    };
                    let pose = PlayerPose {
                        feet: rest_at.unwrap_or(s.player.body.pos + s.player.up() * s.player.step_smooth),
                        heading: rest_dir.unwrap_or(s.player.heading),
                        up: s.player.up(),
                        pitch: s.player.pitch as f32,
                        phase: if s.rest.is_some() { self.time.elapsed() * 2.0 } else { s.player.anim_phase },
                        amount: s.player.anim_amount,
                        sneaking: s.player.sneaking,
                        swimming: s.rest.is_none() && (s.player.body.in_water || s.player.body.in_lava) && !s.player.body.on_ground && !s.player.body.flying,
                        rest: match s.rest { Some(crate::session::Rest::Sleep { .. }) => 1, Some(crate::session::Rest::Sit { .. }) => 2, None => 0 },
                        skin: self.settings.skin,
                        // Mining/attacking swings; placing a block is a short push of the arm.
                        attack: if self.hand_cycle > 0.0 { self.hand_cycle } else if self.hand_place > 0.0 { (1.0 - self.hand_place) * 0.5 } else { 0.0 },
                        held: held_visual(&s.player.inventory, &self.tile_colors),
                        armor: worn_armor(&s.player.inventory),
                        shield: s.player.inventory.has_shield(),
                    };
                    model_verts.extend(build_player_model(&pose, cam.eye));
                    if s.player.inventory.armor[1].is_some_and(|w|w.id==wolfcraft_engine::items::ids::WINGS) {model_verts.extend(wolfcraft_engine::renderer::model::build_wings(&pose,cam.eye,s.player.gliding));}
                }
                // Other players of the server (same dimension only).
                for rp in self.net_players.iter().filter(|rp| rp.hell == s.hell) {
                    let pos = glam::DVec3::from_array(rp.pos);
                    if pos.length_squared() < 1.0 {
                        continue;
                    }
                    let pose = PlayerPose { feet: pos, heading: glam::DVec3::from_array(rp.heading), up: wolfcraft_engine::physics::up_at(pos), pitch: rp.pitch, phase: rp.phase, amount: rp.amount, sneaking: rp.sneak, swimming: rp.swim, rest: 0, skin: rp.skin, attack: 0.0, held: Default::default(), armor: [None; 4], shield: false };
                    model_verts.extend(build_player_model(&pose, cam.eye));
                }
                model_verts.extend(wolfcraft_engine::renderer::model::build_kinetic(&s.kinetic_models(), cam.eye));
                model_verts.extend(wolfcraft_engine::renderer::model::build_debris(&s.mobs.debris, cam.eye));
                model_verts.extend(wolfcraft_engine::renderer::model::build_debris(&s.explosives.debris(self.time.elapsed()), cam.eye));
                // Falling and thrown blocks: cubes in their block's colours, tumbling.
                if !s.collapse.falling.is_empty() {
                    let cubes: Vec<wolfcraft_engine::mobs::Debris> = s
                        .collapse
                        .falling
                        .iter()
                        .map(|f| {
                            let c = self.tile_colors.get(f.block.info().tiles[0] as usize).map(|c| c[1]).unwrap_or([0.6, 0.6, 0.6]);
                            wolfcraft_engine::mobs::Debris { pos: f.pos, vel: f.vel, color: c, size: if f.debris { 0.7 } else { 0.98 }, age: 0.0, spin: f.spin * f.age, kind: wolfcraft_engine::mobs::FxKind::Fall, glow: false }
                        })
                        .collect();
                    model_verts.extend(wolfcraft_engine::renderer::model::build_debris(&cubes, cam.eye));
                }
                gfx.world.set_model(&gfx.gpu.device, &gfx.gpu.queue, &model_verts);
                self.sky_age += dt as f32;
                self.eat_anim = (self.eat_anim - dt as f32 / 0.9).max(0.0);
                self.equip_anim = (self.equip_anim - dt as f32 / 0.45).max(0.0);
                if s.ach_new > 0 {
                    s.ach_new = 0;
                    self.audio.play(crate::audio::Sfx::Achievement, 1.0);
                }
                if self.sky_age >= 0.05 {
                    self.sky_age = 0.0;
                    let sky_verts = wolfcraft_engine::renderer::sky::build_sky(&wolfcraft_engine::renderer::sky::SkyParams { eye: cam.eye, sun_dir: sun.as_vec3(), day, planet_radius: s.planet.radius, sky, time: self.time.elapsed(), underwater, cube: s.planet.grid.cube });
                    gfx.world.set_sky(&gfx.gpu.device, &gfx.gpu.queue, &sky_verts);
                }
                // First-person arms (Bedrock-style): one swing per click, a held button keeps
                // swinging, and a started swing always plays to the end.
                let first_person = s.camera_mode == crate::session::CameraMode::First && !s.player.survival.dead;
                let tool = s.player.inventory.held().and_then(|st| wolfcraft_engine::items::tool_shape(st.id));
                // Mouse button or a held finger (touch) keeps swinging; every blow that lands on a
                // creature starts a swing too (a tap on a mob).
                let bow = s.player.inventory.held().is_some_and(|h| h.id == wolfcraft_engine::items::ids::BOW);
                let hits = std::mem::take(&mut s.mob_hits);
                if hits > 0 {
                    self.audio.play(crate::audio::Sfx::Hit, 1.0);
                }
                let want = self.screen == Screen::Playing && (self.input.left || (self.touch.breaking && !bow) || self.tap_hit > 0.0 || hits > 0) && !s.player.survival.dead;
                let c0 = self.hand_cycle;
                if self.hand_cycle > 0.0 {
                    self.hand_cycle += dt * swing_speed(tool);
                    if self.hand_cycle >= 1.0 {
                        self.hand_cycle = if want { (self.hand_cycle - 1.0).max(1e-4) } else { 0.0 };
                    }
                } else if hits > 0 {
                    // The blow already landed: start the swing near its strike so the hand
                    // and the hit happen together.
                    self.hand_cycle = 0.18;
                } else if want {
                    self.hand_cycle = 1e-4;
                }
                // The moment the blow lands: chips and a hit sound while mining.
                let m = strike_moment(tool.is_some());
                let c1 = self.hand_cycle;
                let landed = (c1 > c0 && c0 < m && c1 >= m) || (c1 < c0 && c0 < m);
                // A sword leaves a white arc in the air on every swing.
                if landed && tool.is_some_and(|t| t.0 == 3) && self.settings.particles > 0.0 {
                    let fwd = cam.forward;
                    let up = wolfcraft_engine::physics::up_at(cam.eye);
                    let right = fwd.cross(up).normalize_or_zero();
                    for i in 0..11 {
                        let a = (i as f64 / 10.0) * 2.0 - 1.0; // right → left
                        let p = cam.eye + fwd * 1.35 + right * (-a * 0.75) + up * (a * 0.32 - 0.12);
                        let k = 1.0 - (a.abs() * 0.5) as f32;
                        s.mobs.fx_glow(p, right * (a * 0.6) + fwd * 0.4, [1.0, 1.0, 1.0], 0.035 * k + 0.01, 0.25, wolfcraft_engine::mobs::FxKind::Rise);
                    }
                }
                // Mining effects at the exact point the ray touches the block face.
                let mining = s.interaction.mining_fraction() > 0.0;
                if let (true, Some(h)) = (mining, s.hit) {
                    use wolfcraft_engine::block::ToolKind;
                    use wolfcraft_engine::mobs::FxKind;
                    let point = cam.eye + cam.forward * (h.dist - 0.04);
                    let back = -cam.forward;
                    let up = wolfcraft_engine::physics::up_at(point);
                    let pk = self.settings.particles.max(0.0);
                    let cols = self.tile_colors.get(h.block.info().tiles[0] as usize).cloned().unwrap_or_default();
                    let block_tool = h.block.info().tool;
                    let held_tool = s.player.inventory.held().and_then(|st| st.info().tool).map(|t| t.0);
                    if landed {
                        // The blow: a spray of chips off the face, a dusty puff, and by tool:
                        // sparks from a pickaxe on stone, splinters from an axe, clods from a shovel.
                        let speed = match held_tool { Some(ToolKind::Shovel) => 2.6, Some(_) => 2.2, None => 1.6 };
                        if !cols.is_empty() {
                            let n = ((8.0 + 4.0 * s.interaction.mining_fraction()) * pk) as usize;
                            for i in 0..n {
                                let r = |k: f64| ((i as f64 * 12.9898 + k * 78.233 + self.time.elapsed() as f64).sin() * 43758.5453).fract();
                                let side = cam.forward.cross(up).normalize_or_zero();
                                let vel = back * (speed * (0.5 + r(1.0))) + side * (r(2.0) - 0.5) * 3.0 + up * (1.0 + 2.0 * r(3.0)) * if held_tool == Some(ToolKind::Shovel) { 1.6 } else { 1.0 };
                                let size = 0.04 + 0.05 * r(4.0) as f32;
                                s.mobs.fx(point + side * (r(5.0) - 0.5) * 0.25, vel, cols[i % cols.len()], size, 0.9, FxKind::Fall);
                            }
                            let dust = cols[0].map(|c| c * 0.6 + 0.35);
                            for k in 0..(2.0 * pk) as usize {
                                s.mobs.fx(point + back * 0.1, back * 0.5 + up * (0.3 + k as f64 * 0.2), dust, 0.16, 0.7, FxKind::Rise);
                            }
                        }
                        // Tool by tool: splinters from an axe on wood, a dust cloud from a shovel,
                        // falling leaves from foliage.
                        if held_tool == Some(ToolKind::Axe) && block_tool == ToolKind::Axe && !cols.is_empty() {
                            for i in 0..(6.0 * pk) as usize {
                                let side = cam.forward.cross(up).normalize_or_zero();
                                let a = i as f64 * 1.7 + self.time.elapsed() as f64 * 3.0;
                                let vel = back * 3.2 + side * a.cos() * 2.2 + up * (2.0 + a.sin().abs() * 2.0);
                                s.mobs.fx(point, vel, cols[i % cols.len()].map(|c| (c * 1.15).min(1.0)), 0.035 + (i % 2) as f32 * 0.03, 1.0, FxKind::Fall);
                            }
                        }
                        if held_tool == Some(ToolKind::Shovel) && block_tool == ToolKind::Shovel && !cols.is_empty() {
                            let dust = cols[0].map(|c| c * 0.7 + 0.25);
                            for i in 0..(4.0 * pk) as usize {
                                let side = cam.forward.cross(up).normalize_or_zero();
                                let o = side * ((i as f64 - 1.5) * 0.18);
                                s.mobs.fx(point + o, back * 0.6 + up * 0.5 + o, dust, 0.2 + (i % 2) as f32 * 0.06, 1.0, FxKind::Rise);
                            }
                        }
                        if h.block.info().name.contains("листв") || h.block.info().name.contains("хвоя") {
                            for i in 0..(5.0 * pk) as usize {
                                let side = cam.forward.cross(up).normalize_or_zero();
                                let p = point + side * ((i as f64 - 2.0) * 0.2) + up * 0.2;
                                let col = cols.get(i % cols.len().max(1)).copied().unwrap_or([0.3, 0.6, 0.2]);
                                s.mobs.fx(p, back * 0.4, col, 0.06, 2.9, FxKind::Leaf);
                            }
                        }
                        // Ores: little gem shards fly off with every blow.
                        if let Some(gem) = wolfcraft_engine::items::ore_glow(h.block) {
                            for i in 0..(4.0 * pk) as usize {
                                let a = i as f64 * 2.1 + self.time.elapsed() as f64 * 5.0;
                                let side = cam.forward.cross(up).normalize_or_zero();
                                let vel = back * 2.4 + side * a.cos() * 1.8 + up * (1.5 + a.sin().abs() * 1.5);
                                s.mobs.fx_glow(point, vel, gem, 0.04, 0.8, FxKind::Fall);
                            }
                        }
                        if held_tool == Some(ToolKind::Pickaxe) && block_tool == ToolKind::Pickaxe {
                            for i in 0..(5.0 * pk) as usize {
                                let a = i as f64 * 2.4 + self.time.elapsed() as f64 * 7.0;
                                let side = cam.forward.cross(up).normalize_or_zero();
                                let vel = back * 3.0 + (side * a.cos() + up * a.sin()) * 2.6;
                                s.mobs.fx_glow(point, vel, [1.0, 0.85, 0.45], 0.035, 0.35, FxKind::Fall);
                            }
                        }
                        self.audio.play(crate::audio::Sfx::Dig(crate::audio::Mat::of(h.block)), 0.9);
                    } else if !cols.is_empty() && pk > 0.0 {
                        // Between blows a light crumble keeps falling from the face.
                        self.mine_dust += dt * 9.0 * pk;
                        while self.mine_dust >= 1.0 {
                            self.mine_dust -= 1.0;
                            let t = self.time.elapsed() as f64 * 13.0;
                            let side = cam.forward.cross(up).normalize_or_zero();
                            let p = point + side * (t.sin() * 0.3) + up * (t.cos() * 0.3);
                            s.mobs.fx(p, back * 0.4, cols[(t as usize) % cols.len()], 0.03, 0.6, FxKind::Fall);
                        }
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
                self.gun_recoil = (self.gun_recoil - dt * 3.2).max(0.0);
                let want = if self.hand_cycle > 0.0 { 1.0 } else { 0.0 };
                self.tool_work += (want - self.tool_work) * (dt * if want > self.tool_work { 7.0 } else { 2.5 }).min(1.0);
                self.hit_marker = (self.hit_marker - dt).max(0.0);
                self.thud_cd = (self.thud_cd - dt).max(0.0);
                if s.player.inventory.selected != self.last_hotbar {
                    self.last_hotbar = s.player.inventory.selected;
                    self.hand_switch = 1.0;
                }
                self.hand_switch = (self.hand_switch - dt * 6.0).max(0.0);
                let block_target = if s.player.blocking { 1.0 } else { 0.0 };
                self.hand_block += (block_target - self.hand_block) * (1.0 - (-16.0 * dt).exp());
                // The studio's edit camera has no hands (the character exists only in «Играть»).
                let studio_edit = self.studio.as_ref().is_some_and(|st| !st.playtest);
                if first_person && !studio_edit {
                    let hand_pose = HandsPose {
                        forward: cam.forward.as_vec3(),
                        up: cam.up.as_vec3(),
                        punch_cycle: self.hand_cycle,
                        guard: self.settings.hand_anim.clamp(0.4, 1.0),
                        // The hands bob in step with the camera (one walk cycle for both).
                        bob_phase: s.view_bob().0,
                        bob_amount: if s.player.body.flying { 0.0 } else { s.view_bob().1 },
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
                        sleeve: worn_armor(&s.player.inventory)[1].map(wolfcraft_engine::renderer::model::armor_color),
                        bow: bow.then_some(s.bow_charge),
                        glint: s.player.inventory.held().is_some_and(|h| h.enchanted()),
                        arrow_tip: s.bow_arrow.and_then(crate::session::tipped_effect).map(|e| e.color()),
                        eat: self.eat_anim,
                        equip: self.equip_anim,
                        gun: s.player.inventory.held().and_then(|st| {
                            use wolfcraft_engine::items::ids;
                            let kind = match st.id {
                                ids::GRENADE_LAUNCHER => 0,
                                ids::BAZOOKA => 1,
                                ids::PISTOL => 2,
                                ids::RIFLE => 3,
                                ids::SHOTGUN => 4,
                                ids::SNIPER => 5,
                                // Box-built 3D models too, not flat icons.
                                ids::STAR_BLASTER => 6,
                                ids::STAR_REMOTE => 7,
                                ids::STAR_HAMMER => 8,
                                ids::PLASMA_RIFLE => 9,
                                ids::RAILGUN => 10,
                                ids::DRILL => 11,
                                ids::CHAINSAW => 12,
                                ids::ENERGY_SWORD => 13,
                                ids::OMNI_TOOL => 14,
                                ids::MINING_LASER => 15,
                                _ => return None,
                            };
                            let ammo = wolfcraft_engine::explosives::weapon(st.id).map(|w| w.0).unwrap_or(0);
                            let reload = s.explosives.reload();
                            let loaded = if matches!(kind, 11 | 12 | 14 | 15) {
                                // Electric tools: charged unless worn down to zero.
                                s.player.is_creative() || st.damage < st.info().durability.max(1)
                            } else {
                                reload <= 0.0 && (s.player.is_creative() || s.player.inventory.count(ammo) > 0)
                            };
                            Some(wolfcraft_engine::renderer::model::GunPose { kind, recoil: self.gun_recoil, reload, loaded, work: self.tool_work })
                        }),
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
                        let mut vs = if (b as u16) >= 1024 && wolfcraft_engine::construction::entry(b as u16).is_some() { wolfcraft_engine::renderer::model::build_held_construction(&pose,b) } else { build_held_block(&pose, b.info().tiles, b.info().emissive, b.info().render == wolfcraft_engine::block::RenderKind::Cross) };
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
                // The biome tints the fog (green mist over swamps, warm haze in deserts).
                let (tint, amount, _) = s.biome_fog();
                let fog_c = [0, 1, 2].map(|i| sky[i] + (sky[i] * tint[i] * 1.1 - sky[i]) * amount);
                gfx.world.render(&gfx.gpu.device, &gfx.gpu.queue, &mut encoder, &view_tex, &params, sky, fog_c);
            }
        } else {
            let p = theme_manager::palette(&gfx.gui.ctx);
            // The interface keeps clear of the notch and rounded corners (safe insets); the
            // strips it leaves get the colour of the screen's own background, so the splash,
            // loading and start screens fill the whole display.
            let starry = matches!(self.screen, Screen::Splash | Screen::Loading | Screen::GameMenu);
            let c = if starry { egui::Color32::from_rgb(2, 7, 13) } else { p.bg };
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
            // Enter as soon as the nearest ground is ready; the rest streams in while playing.
            let (done, total) = s.chunks.progress(s.player_chunk(), 2);
            if l.progress != (done, total) {
                l.progress = (done, total);
                l.progress_at = Instant::now();
            }
            let idle = s.chunks.stats.gen_inflight == 0 && s.chunks.stats.mesh_inflight == 0;
            let stalled = idle && l.progress_at.elapsed().as_secs_f32() > 4.0;
            if stalled {
                log::warn!("loading stalled at {done}/{total} (player {:?}, chunk {:?}); entering anyway", s.player.body.pos, s.player_chunk());
            }
            if (done * 100 >= total * 90 && s.ground_ready()) || stalled {
                self.load = None;
                self.screen = Screen::Playing;
                self.near_table = false;
                // Studio: build freely (creative, flying) whatever the world's mode.
                if self.studio.is_some() {
                    s.studio = true;
                    s.player.mode = wolfcraft_engine::config::GameMode::Creative;
                    s.player.body.flying = true;
                    // Edit mode is a free camera, as in Roblox Studio: no character until «Играть».
                    s.player.noclip = true;
                }
                // The world's Luau script (written in the studio) runs every time the world starts.
                if self.net.is_none() {
                    let script = read_world_script(&self.worlds_dir.join(&s.cfg.id));
                    self.start_lua(&script);
                }
            }
        }
    }

    fn settings_world_hint(&self, s: &GameSession) {
        log::info!("session started: {} (seed {}, {:?}, {:?})", s.cfg.name, s.cfg.seed, s.cfg.size, s.cfg.world_type);
    }

    fn check_updates(&mut self) {
        self.save_studio_script();
        if let Some(session)=&mut self.session {session.save();}
        crate::updates::open_release();
    }

    fn handle_action(&mut self, a: MenuAction, gfx: &mut Gfx) {
        match a {
            MenuAction::None => {}
            MenuAction::LaunchVersion(version) => {
                if version == wolfcraft_common::VERSION { self.push(Screen::GameMenu); }
            }
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
            MenuAction::CheckUpdates => self.check_updates(),
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
            MenuAction::SetNickname(n) => {
                self.settings.nickname = n.trim().chars().take(16).collect();
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
            MenuAction::HostServer(Some(id)) if wolfcraft_ui::launcher::is_studio_app() => {
                // WolfStudio: «Запустить» starts this place on the bundled WolfServer (the
                // «Сервер» tab), with its own world from the servers folder.
                if let Some(s) = &mut self.session {
                    s.save();
                }
                let _ = std::fs::write(self.data_dir.join("start_server.txt"), &id);
                self.quit = true;
            }
            MenuAction::HostServer(world) => {
                // Publishing from the studio ends the studio: the server world is played normally.
                self.studio = None;
                self.studio_world = None;
                self.touch.studio = false;
                self.touch.blocked.clear();
                self.host_server(world)
            }
            MenuAction::NewStudioProject => {
                // An empty baseplate world, creative and peaceful, opened in the Studio.
                self.net = None;
                let n = self.worlds.iter().filter(|w| w.cfg.studio_project).count() + 1;
                let mut cfg = WorldConfig::new(&format!("Мини-игра {n}"), splitmix(self.random_seed ^ platform::unix_now()) % 1_000_000_000);
                cfg.world_type = wolfcraft_engine::config::WorldType::Baseplate;
                cfg.mode = wolfcraft_engine::config::GameMode::Creative;
                cfg.difficulty = wolfcraft_engine::config::Difficulty::Peaceful;
                cfg.structures = false;
                cfg.size = wolfcraft_engine::config::PlanetSize::Small;
                cfg.studio_project = true;
                cfg.id = wolfcraft_engine::config::unique_world_id(&self.worlds_dir, &cfg.name); // the loader picks the same id
                self.studio = Some(wolfcraft_ui::studio_ui::StudioState { assets_open: true, script_open: true, script: crate::lua::TEMPLATE.to_string(), ..Default::default() });
                self.studio_world = Some(cfg.id.clone());
                self.start_load(cfg, true);
            }
            MenuAction::EditServerInStudio(addr) => {
                // A server's world with the studio tools: creative, flying, brush and WorldEdit;
                // every edit is sent to the server, so all players see it at once.
                self.host = None;
                self.studio = Some(wolfcraft_ui::studio_ui::StudioState { assets_open: true, ..Default::default() });
                self.studio_world = None;
                self.join_server(&addr);
                if self.net.is_none() {
                    // Could not connect: no studio left behind.
                    self.studio = None;
                    if let Some(e) = self.world_select.net_error.clone() {
                        let now = self.time.elapsed() as f64;
                        self.chat.push(ChatKind::System, format!("Не удалось открыть мир сервера: {e}"), now);
                    }
                }
            }
            MenuAction::CreateStudioProject { name, template } => {
                // A new server (place) on disk only; it opens in the editor from the list.
                let mut cfg = WorldConfig::new(&name, splitmix(self.random_seed ^ platform::unix_now()) % 1_000_000_000);
                cfg.world_type = match template {
                    0 => wolfcraft_engine::config::WorldType::Baseplate,
                    1 => wolfcraft_engine::config::WorldType::Flat,
                    _ => wolfcraft_engine::config::WorldType::Standard,
                };
                cfg.mode = wolfcraft_engine::config::GameMode::Creative;
                // Empty flat place without structures (the planet template keeps them).
                cfg.structures = template == 2;
                if template != 2 {
                    cfg.difficulty = wolfcraft_engine::config::Difficulty::Peaceful;
                    cfg.size = wolfcraft_engine::config::PlanetSize::Small;
                }
                cfg.studio_project = true;
                cfg.id = wolfcraft_engine::config::unique_world_id(&self.worlds_dir, &cfg.name);
                let dir = self.worlds_dir.join(&cfg.id);
                let ok = std::fs::create_dir_all(&dir).map_err(|e| e.to_string()).and_then(|_| cfg.save(&dir));
                if let Err(e) = ok {
                    log::error!("new studio project: {e}");
                }
                let _ = std::fs::write(dir.join(SCRIPT_FILE), crate::lua::TEMPLATE);
                self.refresh_worlds(&gfx.gui.ctx);
            }
            MenuAction::OpenStudio(id) => {
                self.net = None;
                if let Some(w) = self.worlds.iter().find(|w| w.cfg.id == id) {
                    let cfg = w.cfg.clone();
                    let dir = self.worlds_dir.join(&id);
                    let script = read_world_script(&dir);
                    // Created in the menu and never opened: only world.toml (and the script).
                    let fresh = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).all(|e| { let n = e.file_name(); n == "world.toml" || n == SCRIPT_FILE })).unwrap_or(false);
                    self.studio = Some(wolfcraft_ui::studio_ui::StudioState { assets_open: true, script_open: fresh, script, ..Default::default() });
                    self.studio_world = Some(id);
                    // Not `is_new`: that would make another folder; an empty world loads fine.
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
        let (done, total) = s.chunks.progress(s.player_chunk(), 2);
        // 90 % of the nearest chunks is enough to enter (see the loading step).
        let f = (done as f32 / (total.max(1) as f32 * 0.9)).min(1.0);
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
            "q" | "й" => KeyCode::KeyQ,
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
                crate::updates::mark_ready();
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
        if matches!(event, WindowEvent::Touch(_) | WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } | WindowEvent::KeyboardInput { .. }) {
            self.last_input = Instant::now();
        }
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
                // A free cursor (Alt held) over a panel: the click is the panel's.
                let on_panel = !self.grabbed && gfx.gui.wants_pointer();
                if self.screen == Screen::Playing && !on_panel {
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
                    let size = studio_touch_size(gfx.gpu.size(), self.studio.as_ref(), gfx.gui.ctx.pixels_per_point());
                    for a in self.touch.event(&t, size) {
                        match a {
                            TouchAction::Fly => {
                                if let Some(s) = &mut self.session {
                                    s.player.toggle_flight();
                                    // Sneak toggled on would pull a fresh flight down.
                                    self.touch.sneak = false;
                                }
                            }
                            TouchAction::Place => {
                                // With a pickaxe, axe, shovel or sword in hand a tap is a blow
                                // (mines a little, hits a creature, swings the arm); a hoe and
                                // everything else keep the tap as "use".
                                let tool = self.session.as_ref().and_then(|s| s.player.inventory.held()).and_then(|h| wolfcraft_engine::items::tool_shape(h.id));
                                if tool.is_some_and(|t| t.0 < 4) {
                                    self.tap_hit = 0.2;
                                } else {
                                    self.input.right_press = true;
                                }
                            }
                            TouchAction::Hotbar(d) => self.input.scroll += d as f32,
                            TouchAction::Camera => self.handle_key(KeyCode::F5, true, false),
                            TouchAction::Chat => self.chat.open(""),
                            TouchAction::Voice => self.voice_menu = !self.voice_menu,
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
                // Android: a rotation (the studio turns to landscape) does not always send
                // `Resized`; the surface would stay upright. Follow the window's real size.
                let real = gfx.window.inner_size();
                if real.width > 0 && real.height > 0 && (real.width, real.height) != gfx.gpu.size() {
                    gfx.gpu.resize(real.width, real.height);
                    gfx.world.resize(&gfx.gpu.device, real.width, real.height);
                }
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
        let idle_menu = !self.screen.in_world() && self.last_input.elapsed() > Duration::from_secs(5);
        let cap = if idle_menu {
            // Live backgrounds keep moving, but at half the rate: the phone stays cool.
            30
        } else if !self.screen.in_world() && !cfg!(target_os = "android") {
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
    (0..=wolfcraft_engine::construction::LAST_ID)
        .map(|id| {
            let b = Block::from_id(id);
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
    // North-up (like a navigator): the map stays still while the view turns; only the
    // player's arrow on it rotates. A turning map was rebuilt a few rows at a time and tore.
    let p = &s.player;
    let up = p.up();
    let axis = if up.y.abs() > 0.995 { DVec3::Z } else { DVec3::Y };
    let fwd = (axis - up * axis.dot(up)).normalize_or_zero();
    let right = fwd.cross(up);
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
    let title = if text.starts_with("Мир остановлен") { "Ошибка в мире" } else { "Игра упала в прошлый раз" };
    let dismissed = modal(ctx, "crash-report", title, 560.0, |ui| {
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

/// What the character holds, for the third-person model.
fn held_visual(inv: &wolfcraft_engine::inventory::Inventory, tile_colors: &[[[f32; 3]; 3]]) -> wolfcraft_engine::renderer::model::Held {
    use wolfcraft_engine::renderer::model::Held;
    let Some(st) = inv.held() else { return Held::Nothing };
    // Weapons: box-built 3D models in the hand.
    {
        use wolfcraft_engine::items::ids;
        let gun = match st.id {
            ids::GRENADE_LAUNCHER => Some(0),
            ids::BAZOOKA => Some(1),
            ids::PISTOL => Some(2),
            ids::RIFLE => Some(3),
            ids::SHOTGUN => Some(4),
            ids::SNIPER => Some(5),
            ids::STAR_BLASTER => Some(6),
            ids::STAR_REMOTE => Some(7),
            ids::STAR_HAMMER => Some(8),
            ids::PLASMA_RIFLE => Some(9),
            ids::RAILGUN => Some(10),
            ids::DRILL => Some(11),
            ids::CHAINSAW => Some(12),
            ids::ENERGY_SWORD => Some(13),
            ids::OMNI_TOOL => Some(14),
            ids::MINING_LASER => Some(15),
            _ => None,
        };
        if let Some(k) = gun {
            return Held::Gun(k);
        }
    }
    if let Some((kind, mat)) = wolfcraft_engine::items::tool_shape(st.id) {
        return Held::Tool(kind, mat);
    }
    let info = st.info();
    if let Some(b) = info.place {
        let c = tile_colors.get(b.info().tiles[0] as usize).map(|c| c[1]).unwrap_or([0.6, 0.6, 0.6]);
        return Held::Block(c);
    }
    match info.icon {
        wolfcraft_engine::items::Icon::Item(i) => Held::Item(i),
        _ => Held::Nothing,
    }
}

/// Worn armour per slot as material index (leather, bronze, iron, diamond).
/// Corners of the box the outline is drawn around: the union of the block's shape boxes, a
/// slim box for plants and torches, the whole cell for full blocks.
fn block_outline(world: &impl wolfcraft_engine::physics::Voxels, c: wolfcraft_engine::cubesphere::Cell, b: wolfcraft_engine::block::Block) -> [glam::DVec3; 8] {
    use wolfcraft_engine::block::{Block, RenderKind};
    let g = &world.grid();
    // The second leaf of a double door hangs on the far jamb: outline it there (as drawn).
    let mirrored = b.door_mirror().filter(|(axis, _)| g.neighbor(c, *axis, -1).is_some_and(|n| b.door_mirrored_by(world.cell_block(n)))).map(|(_, mb)| mb);
    let bx: [f32; 6] = if let Some(mb) = mirrored {
        mb
    } else {
        b.outline_box()
    };
    let mut out = [glam::DVec3::ZERO; 8];
    for (n, p) in out.iter_mut().enumerate() {
        let pick = |bit: usize, lo: f32, hi: f32| if n >> bit & 1 == 1 { hi as f64 } else { lo as f64 };
        *p = g.point(c.f, c.i as f64 + pick(0, bx[0], bx[3]), c.j as f64 + pick(1, bx[1], bx[4]), c.k as f64 + pick(2, bx[2], bx[5]));
    }
    out
}

fn worn_armor(inv: &wolfcraft_engine::inventory::Inventory) -> [Option<u8>; 4] {
    let mut out = [None; 4];
    for (i, slot) in inv.armor.iter().enumerate() {
        out[i] = slot.and_then(|s| wolfcraft_engine::items::armor_material(s.id));
    }
    out
}

/// The Чаровальня's three offers for the item in its left slot: name and level, the cost in
/// levels and iskrit; a tap casts it (a blank book becomes a «Книга чар»).
fn enchant_offers(ui: &mut egui::Ui, icons: &wolfcraft_ui::GuiIcons, slots: &mut [Option<wolfcraft_engine::items::ItemStack>], surv: &mut wolfcraft_engine::player::Survival, seed: &mut u32, creative: bool) {
    use wolfcraft_engine::items::ids;
    use wolfcraft_engine::magic;
    use wolfcraft_ui::ui_components::{ButtonKind, pill_button};
    let Some(item) = slots.first().copied().flatten() else {
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Положите инструмент, оружие, броню, лук или книгу").color(egui::Color32::from_gray(170)));
        return;
    };
    let iskrit = slots.get(1).copied().flatten().map(|s| s.count as u32).unwrap_or(0);
    let level = surv.level();
    ui.add_space(6.0);
    ui.label(egui::RichText::new(format!("Ваш уровень: {level} · искрит: {iskrit}")).color(egui::Color32::from_rgb(255, 170, 160)));
    let mut cast: Option<(magic::Ench, u8, u32)> = None;
    for (e, lvl, cost) in magic::offers(item.id, *seed) {
        let can = creative || (level >= cost && iskrit >= cost);
        let text = format!("{} {} · {} ур. · {} искрита", e.name(), magic::roman(lvl), cost, cost);
        if pill_button(ui, icons, None, &text, if can { ButtonKind::Primary } else { ButtonKind::Secondary }, ui.available_width().min(420.0), can).clicked() {
            cast = Some((e, lvl, cost));
        }
    }
    if let Some((e, lvl, cost)) = cast {
        let mut out = item;
        if item.id == ids::BOOK {
            out = wolfcraft_engine::items::ItemStack::new(ids::ENCH_BOOK, 1);
        }
        out.ench = magic::with(out.ench, e, lvl);
        slots[0] = Some(out);
        if !creative {
            let new_level = level - cost;
            surv.xp = new_level * new_level;
            if let Some(s) = slots[1].as_mut() {
                s.count = s.count.saturating_sub(cost as u8);
                if s.count == 0 {
                    slots[1] = None;
                }
            }
        }
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    }
}

/// Size the touch controls lay themselves out in: the whole screen, or in the upright studio
/// only the world view above its dock (physical pixels).
fn studio_touch_size(size: (u32, u32), studio: Option<&wolfcraft_ui::studio_ui::StudioState>, ppp: f32) -> (u32, u32) {
    let Some(s) = studio.filter(|s| !s.playtest) else { return size };
    let w = s.view_right.map(|vr| ((vr * ppp) as u32).clamp(200, size.0)).unwrap_or(size.0);
    let h = s.view_bottom.map(|vb| ((vb * ppp) as u32).clamp(200, size.1)).unwrap_or(size.1);
    (w, h)
}

/// «Питомник» on screen: the coin counter with the pets in the team, and the egg hatch
/// animation (the capsule wobbles, cracks, bursts in the pet's rarity colour).
impl App {
    /// A studio place is being played (studio «Играть», or a place joined over the network):
    /// none of WolfCraft's own HUD, only the stick, jump and the top bar.
    fn place_play(&self) -> bool {
        let Some(s) = &self.session else { return false };
        match &self.studio {
            Some(st) => st.playtest,
            None => self.net.is_some() && s.cfg.studio_project,
        }
    }
}

/// Voice chat on screen: who is talking (top left, under the bar) and the menu of the
/// microphone button. Returns true when a setting changed (to save it).
fn voice_ui(ctx: &egui::Context, icons: &wolfcraft_ui::gui_renderer::GuiIcons, v: &mut crate::voice::Voice, open: &mut bool, st: &mut Settings, players: &[wolfcraft_net::PlayerState], server: Option<bool>, now: f64) -> bool {
    use egui::{Color32, Vec2};
    use wolfcraft_ui::ui_components::{ButtonKind, pill_button};
    let p = wolfcraft_ui::theme_manager::palette(ctx);
    // Speaking: a green dot and the nick for each voice heard right now.
    let speaking = v.speaking(now);
    if !speaking.is_empty() || v.talking {
        egui::Area::new(egui::Id::new("voice-speaking")).anchor(egui::Align2::LEFT_TOP, Vec2::new(14.0, 64.0)).interactable(false).show(ctx, |ui| {
            let mut names: Vec<String> = speaking.iter().filter_map(|id| players.iter().find(|p| p.id == *id).map(|p| p.nick.clone())).collect();
            if v.talking {
                names.insert(0, "Вы".into());
            }
            for n in names {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 5.0, Color32::from_rgb(90, 230, 120));
                    ui.label(egui::RichText::new(n).size(14.0).color(Color32::WHITE));
                });
            }
        });
    }
    if !*open {
        return false;
    }
    let before = (st.voice_enabled, st.voice_push_to_talk, st.voice_threshold, st.voice_volume, st.voice_proximity);
    let mut close = false;
    egui::Window::new("Голосовой чат").id(egui::Id::new("voice-menu")).collapsible(false).resizable(false).anchor(egui::Align2::LEFT_TOP, Vec2::new(16.0, 96.0)).default_width(300.0).show(ctx, |ui| {
        match server {
            None => {
                ui.label(egui::RichText::new("Голос работает на сервере: зайдите через «Игра по сети».").color(p.text_muted));
            }
            Some(false) => {
                ui.label(egui::RichText::new("Этот сервер старой версии и не передаёт голос.").color(p.text_muted));
            }
            Some(true) => {}
        }
        if let Some(e) = &v.error {
            ui.label(egui::RichText::new(format!("Микрофон: {e}")).color(p.danger));
        }
        ui.checkbox(&mut st.voice_enabled, "Микрофон включён");
        ui.horizontal(|ui| {
            ui.radio_value(&mut st.voice_push_to_talk, false, "Активация голосом");
            ui.radio_value(&mut st.voice_push_to_talk, true, "Нажми и говори");
        });
        if st.voice_push_to_talk {
            ui.label(egui::RichText::new("Держите кнопку 🎤 вверху (или V на клавиатуре), пока говорите.").size(12.5).color(p.text_muted));
        } else {
            ui.label(egui::RichText::new("Порог: голос громче отметки уходит в чат").size(12.5).color(p.text_muted));
            ui.add(egui::Slider::new(&mut st.voice_threshold, 0.003..=0.15).logarithmic(true).show_value(false));
        }
        // Level meter with the threshold mark.
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 10.0), egui::Sense::hover());
        ui.painter().rect_filled(r, 5.0, p.bg);
        let lv = (v.level / 0.15).clamp(0.0, 1.0).sqrt();
        ui.painter().rect_filled(egui::Rect::from_min_size(r.min, Vec2::new(r.width() * lv, r.height())), 5.0, if v.talking { Color32::from_rgb(90, 230, 120) } else { p.accent });
        if !st.voice_push_to_talk {
            let x = r.left() + r.width() * (st.voice_threshold / 0.15).clamp(0.0, 1.0).sqrt();
            ui.painter().line_segment([egui::pos2(x, r.top() - 2.0), egui::pos2(x, r.bottom() + 2.0)], egui::Stroke::new(2.0_f32, Color32::WHITE));
        }
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Громкость голосов").size(12.5).color(p.text_muted));
        ui.add(egui::Slider::new(&mut st.voice_volume, 0.0..=2.0).show_value(false));
        ui.checkbox(&mut st.voice_proximity, "Только рядом (тише с расстоянием)");
        if !players.is_empty() {
            ui.separator();
            ui.label(egui::RichText::new("Игроки").size(12.5).color(p.text_muted));
            for pl in players {
                let mut muted = v.muted.contains(&pl.id);
                if ui.checkbox(&mut muted, format!("Заглушить {}", pl.nick)).changed() {
                    if muted {
                        v.muted.insert(pl.id);
                    } else {
                        v.muted.remove(&pl.id);
                    }
                }
            }
        }
        ui.add_space(4.0);
        if pill_button(ui, icons, None, "Готово", ButtonKind::Primary, ui.available_width(), true).clicked() {
            close = true;
        }
    });
    if close {
        *open = false;
    }
    before != (st.voice_enabled, st.voice_push_to_talk, st.voice_threshold, st.voice_volume, st.voice_proximity)
}

fn petsim_overlay(ctx: &egui::Context, p: &wolfcraft_engine::petsim::PetSim, lobby: bool) {
    use egui::{Align2, Color32, FontId, Pos2, Shape, Stroke, Vec2};
    use wolfcraft_engine::petsim::PETS;
    if p.owned.is_empty() && p.coins == 0 && !lobby && p.hatching.is_none() {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("petsim")));
    let screen = ctx.screen_rect();
    let col = |c: [f32; 3], a: f32| Color32::from_rgba_unmultiplied((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, (a * 255.0) as u8);
    // Coins: a gold coin and the amount, top left; the team under it.
    let at = Pos2::new(screen.left() + 16.0, screen.top() + 70.0);
    painter.circle_filled(at + Vec2::new(10.0, 10.0), 10.0, Color32::from_rgb(180, 130, 20));
    painter.circle_filled(at + Vec2::new(10.0, 10.0), 7.5, Color32::from_rgb(250, 205, 60));
    let text = format!("{}", p.coins);
    painter.text(at + Vec2::new(27.0, 11.0), Align2::LEFT_CENTER, &text, FontId::proportional(20.0), Color32::from_black_alpha(160));
    painter.text(at + Vec2::new(26.0, 10.0), Align2::LEFT_CENTER, &text, FontId::proportional(20.0), Color32::from_rgb(255, 225, 90));
    for (i, k) in p.best().iter().enumerate() {
        let kind = &PETS[*k];
        let y = at.y + 34.0 + i as f32 * 20.0;
        painter.circle_filled(Pos2::new(at.x + 6.0, y), 5.0, col(kind.rarity.color(), 1.0));
        painter.text(Pos2::new(at.x + 16.0, y), Align2::LEFT_CENTER, format!("{} ×{:.0}", kind.name, kind.power), FontId::proportional(14.0), Color32::from_white_alpha(230));
    }
    if p.owned.len() > 3 {
        painter.text(Pos2::new(at.x, at.y + 34.0 + 3.0 * 20.0), Align2::LEFT_CENTER, format!("всего питомцев: {}", p.owned.len()), FontId::proportional(12.0), Color32::from_white_alpha(170));
    }
    // Hatching.
    let Some((t, k)) = p.hatching else { return };
    let kind = &PETS[k];
    let rc = kind.rarity.color();
    let c = screen.center() + Vec2::new(0.0, -20.0);
    let dim = (t / 0.3).min(1.0) * if t > 2.8 { ((3.2 - t) / 0.4).max(0.0) } else { 1.0 };
    painter.rect_filled(screen, 0.0, Color32::from_black_alpha((dim * 140.0) as u8));
    if t < 1.8 {
        // The capsule egg: wobbles harder and harder, cracks appear.
        let w = (t / 1.8).powi(2);
        let ang = (t * 22.0).sin() * 0.35 * w;
        let (sa, ca) = ang.sin_cos();
        let pts: Vec<Pos2> = (0..40)
            .map(|i| {
                let a = i as f32 / 40.0 * std::f32::consts::TAU;
                let (x, y) = (a.cos() * 48.0, a.sin() * if a.sin() < 0.0 { 66.0 } else { 54.0 });
                c + Vec2::new(x * ca - y * sa, x * sa + y * ca)
            })
            .collect();
        painter.add(Shape::convex_polygon(pts, Color32::from_rgb(245, 242, 232), Stroke::new(4.0_f32, col(rc, 1.0))));
        // Capsule band across the middle.
        painter.line_segment([c + Vec2::new(-46.0 * ca, -46.0 * sa), c + Vec2::new(46.0 * ca, 46.0 * sa)], Stroke::new(6.0_f32, col(rc, 0.9)));
        if t > 0.9 {
            let crack = [(-10.0, -30.0), (0.0, -14.0), (-8.0, 2.0), (6.0, 16.0), (-2.0, 30.0)];
            let n = (((t - 0.9) / 0.9) * crack.len() as f32).ceil() as usize;
            let pts: Vec<Pos2> = crack.iter().take(n.max(2)).map(|(x, y)| c + Vec2::new(x * ca - y * sa, x * sa + y * ca)).collect();
            painter.add(Shape::line(pts, Stroke::new(3.0_f32, Color32::from_rgb(60, 50, 40))));
        }
    } else {
        // Burst: rays and a ring in the rarity colour, then the name.
        let k2 = ((t - 1.8) / 0.5).min(1.0);
        for i in 0..16 {
            let a = i as f32 / 16.0 * std::f32::consts::TAU + t * 0.6;
            let r0 = 30.0 + 40.0 * k2;
            let r1 = r0 + 60.0 + 30.0 * (i % 2) as f32;
            painter.line_segment([c + Vec2::angled(a) * r0, c + Vec2::angled(a) * r1], Stroke::new(5.0_f32, col(rc, 0.85 * dim)));
        }
        painter.circle_stroke(c, 40.0 + 70.0 * k2, Stroke::new(6.0 * (1.0 - k2) + 1.0, col(rc, dim)));
        painter.circle_filled(c, 26.0, col(rc, dim));
        painter.text(c + Vec2::new(0.0, 96.0), Align2::CENTER_CENTER, kind.rarity.name(), FontId::proportional(18.0), col(rc, dim));
        painter.text(c + Vec2::new(0.0, 124.0), Align2::CENTER_CENTER, kind.name, FontId::proportional(30.0), Color32::from_white_alpha((dim * 255.0) as u8));
        painter.text(c + Vec2::new(0.0, 152.0), Align2::CENTER_CENTER, format!("сила копания ×{:.0}", kind.power), FontId::proportional(15.0), Color32::from_white_alpha((dim * 200.0) as u8));
    }
}

/// «Кнопки» on screen: money, income per second, multiplier and rebirths (top right).
/// PvP templates on screen: the mode, team or role, the timer, the bed / egg, the seeker's
/// count-down, the result; a «Магазин» button and window in Бед/Эггварс. Returns purchases.
fn arena_overlay(ctx: &egui::Context, a: &wolfcraft_engine::arenas::Arena, inv: &wolfcraft_engine::inventory::Inventory, cover: &mut Vec<egui::Rect>) -> Vec<usize> {
    use egui::{Align2, Color32, FontId, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
    use wolfcraft_engine::arenas::SHOP;
    let mut buys = Vec::new();
    let screen = ctx.screen_rect();
    let bold = |s: f32| egui::FontId::new(s, egui::FontFamily::Name("bold".into()));
    let shop_id = egui::Id::new("arena-shop");
    let mut shop: bool = ctx.data(|d| d.get_temp(shop_id)).unwrap_or(false);
    egui::Area::new(egui::Id::new("arena-hud")).order(egui::Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        let pt = ui.painter().clone();
        // The seeker counts with eyes closed.
        if a.counting() {
            pt.rect_filled(screen, 0.0, Color32::from_black_alpha(245));
            pt.text(screen.center() - Vec2::new(0.0, 30.0), Align2::CENTER_CENTER, format!("{}", 20 - a.age as i32), bold(64.0), Color32::WHITE);
            pt.text(screen.center() + Vec2::new(0.0, 30.0), Align2::CENTER_CENTER, "Считайте… прячущиеся прячутся", FontId::proportional(16.0), Color32::from_white_alpha(200));
        }
        let mut parts = vec![a.mode.name().to_string()];
        if !a.team.is_empty() {
            parts.push(a.team.to_string());
        }
        if a.role != wolfcraft_engine::arenas::Role::None {
            parts.push(a.role.name().to_string());
        }
        if a.mode.round() > 0.0 {
            parts.push(format!("{}:{:02}", a.timer as u32 / 60, a.timer as u32 % 60));
        }
        if a.mode.has_heart() {
            let what = if a.mode == wolfcraft_engine::arenas::Mode::EggWars { "Яйцо" } else { "Кровать" };
            parts.push(format!("{what} {}", if a.heart_alive { "✔" } else { "✖" }));
        }
        let text = parts.join("  ·  ");
        let g = pt.layout_no_wrap(text.clone(), bold(14.0), Color32::WHITE);
        let w = g.size().x + 28.0 + if a.mode.has_shop() { 100.0 } else { 0.0 };
        let bar = Rect::from_min_size(Pos2::new(screen.center().x - w / 2.0, screen.top() + 8.0), Vec2::new(w, 36.0));
        pt.rect_filled(bar.translate(Vec2::new(0.0, 3.0)), Rounding::same(18.0), Color32::from_black_alpha(60));
        pt.rect_filled(bar, Rounding::same(18.0), Color32::from_rgba_unmultiplied(18, 30, 52, 215));
        pt.rect_filled(Rect::from_min_max(bar.min + Vec2::new(10.0, 2.0), Pos2::new(bar.right() - 10.0, bar.center().y)), Rounding::same(9.0), Color32::from_white_alpha(18));
        pt.rect_stroke(bar, Rounding::same(18.0), Stroke::new(1.0_f32, Color32::from_white_alpha(90)));
        pt.galley(Pos2::new(bar.left() + 14.0, bar.center().y - g.size().y / 2.0), g, Color32::WHITE);
        cover.push(bar);
        if a.mode.has_shop() && !a.eliminated {
            let r = Rect::from_min_max(Pos2::new(bar.right() - 96.0, bar.top() + 4.0), Pos2::new(bar.right() - 4.0, bar.bottom() - 4.0));
            let resp = ui.interact(r, egui::Id::new("arena-shop-btn"), Sense::click());
            pt.rect_filled(r, Rounding::same(14.0), if shop || resp.hovered() { Color32::from_rgb(70, 180, 80) } else { Color32::from_rgb(50, 150, 60) });
            pt.text(r.center(), Align2::CENTER_CENTER, "Магазин", bold(13.0), Color32::WHITE);
            if resp.clicked() {
                shop = !shop;
            }
        }
        if a.eliminated || a.over.is_some() {
            let msg = a.over.clone().unwrap_or_else(|| "Вы выбыли — наблюдайте".into());
            let r = Rect::from_center_size(Pos2::new(screen.center().x, screen.top() + 80.0), Vec2::new(360.0f32.min(screen.width() - 20.0), 40.0));
            pt.rect_filled(r, Rounding::same(20.0), Color32::from_rgba_unmultiplied(120, 30, 40, 220));
            pt.text(r.center(), Align2::CENTER_CENTER, msg, bold(14.0), Color32::WHITE);
        }
    });
    if shop {
        let mut open = true;
        egui::Window::new(egui::RichText::new("Магазин").font(bold(17.0)))
            .id(egui::Id::new("arena-shop-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(egui::Frame::window(&ctx.style()).fill(Color32::from_rgba_unmultiplied(16, 26, 46, 240)).rounding(Rounding::same(16.0)).stroke(Stroke::new(1.0_f32, Color32::from_white_alpha(90))))
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(format!("У вас: железо {} · золото {} · алмазы {}", inv.count(wolfcraft_engine::items::ids::IRON_INGOT), inv.count(wolfcraft_engine::items::ids::GOLD_INGOT), inv.count(wolfcraft_engine::items::ids::DIAMOND))).color(Color32::from_white_alpha(210)));
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(screen.height() * 0.6).show(ui, |ui| {
                    egui::Grid::new("arena-shop-grid").num_columns(2).spacing(Vec2::new(10.0, 6.0)).show(ui, |ui| {
                        for (k, (name, _, (cid, cn))) in SHOP.iter().enumerate() {
                            let have = inv.count(*cid) >= *cn as u32;
                            let cost = format!("{} {}", cn, match *cid {
                                x if x == wolfcraft_engine::items::ids::IRON_INGOT => "жел.",
                                x if x == wolfcraft_engine::items::ids::GOLD_INGOT => "зол.",
                                _ => "алм.",
                            });
                            ui.label(egui::RichText::new(*name).color(Color32::WHITE));
                            let b = egui::Button::new(egui::RichText::new(cost).font(bold(13.0)).color(Color32::WHITE)).fill(if have { Color32::from_rgb(60, 170, 70) } else { Color32::from_rgb(70, 75, 90) }).rounding(Rounding::same(12.0)).min_size(Vec2::new(80.0, 28.0));
                            if ui.add(b).clicked() {
                                buys.push(k);
                            }
                            ui.end_row();
                        }
                    });
                });
            });
        if !open {
            shop = false;
        }
        if let Some(r) = ctx.memory(|m| m.area_rect(egui::Id::new("arena-shop-window"))) {
            cover.push(r);
        }
    }
    ctx.data_mut(|d| d.insert_temp(shop_id, shop));
    buys
}

/// What the «Кликер» overlay asks the game to do.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ClickAct {
    Tap,
    Sell,
    Upgrade(usize),
    Rebirth(u32),
    Hatch(usize),
    Island(usize),
}

/// The pet atlas: every pet drawn from its colours — a round body, ears (round, pointed or
/// long), a belly or mask in the accent colour, eyes and cheeks, a glow of its rarity.
fn paint_pet(painter: &egui::Painter, r: egui::Rect, k: usize, t: f32) {
    use egui::{Color32, Pos2, Stroke, Vec2};
    let pet = &wolfcraft_engine::clicker::PETS[k];
    let c = |v: [u8; 3]| Color32::from_rgb(v[0], v[1], v[2]);
    let s = r.width().min(r.height());
    let bob = (t * 3.0 + k as f32).sin() * s * 0.025;
    let o = r.center() + Vec2::new(0.0, s * 0.06 + bob);
    let rad = s * 0.30;
    // Rarity glow.
    let rc = pet.rarity.color();
    for g in 0..4 {
        painter.circle_filled(o, rad * (1.25 + g as f32 * 0.12), Color32::from_rgba_unmultiplied(rc[0], rc[1], rc[2], 26 - g as u8 * 5));
    }
    // Ears.
    for sx in [-1.0f32, 1.0] {
        let base = o + Vec2::new(sx * rad * 0.55, -rad * 0.70);
        match pet.ears {
            0 => {
                painter.circle_filled(base, rad * 0.32, c(pet.body));
                painter.circle_filled(base, rad * 0.17, c(pet.accent));
            }
            1 => {
                let pts = vec![base + Vec2::new(-rad * 0.30, rad * 0.2), base + Vec2::new(sx * rad * 0.15, -rad * 0.55), base + Vec2::new(rad * 0.30, rad * 0.2)];
                painter.add(egui::Shape::convex_polygon(pts, c(pet.body), Stroke::NONE));
            }
            _ => {
                let e = egui::Rect::from_center_size(base + Vec2::new(0.0, -rad * 0.35), Vec2::new(rad * 0.34, rad * 1.0));
                painter.rect_filled(e, egui::Rounding::same(rad * 0.17), c(pet.body));
                painter.rect_filled(e.shrink(rad * 0.08), egui::Rounding::same(rad * 0.1), c(pet.accent));
            }
        }
    }
    // Body and the accent pattern.
    painter.circle_filled(o, rad, c(pet.body));
    match pet.pattern {
        0 => {
            painter.circle_filled(o + Vec2::new(0.0, rad * 0.35), rad * 0.5, c(pet.accent));
        }
        1 => {
            painter.circle_filled(o + Vec2::new(0.0, rad * 0.25), rad * 0.62, c(pet.accent));
        }
        _ => {
            for sx in [-1.0f32, 1.0] {
                painter.circle_filled(o + Vec2::new(sx * rad * 0.38, -rad * 0.05), rad * 0.26, c(pet.accent));
            }
        }
    }
    painter.circle_stroke(o, rad, Stroke::new(1.5_f32, Color32::from_black_alpha(90)));
    // Face.
    for sx in [-1.0f32, 1.0] {
        let e = o + Vec2::new(sx * rad * 0.36, -rad * 0.05);
        painter.circle_filled(e, rad * 0.13, Color32::from_rgb(20, 20, 28));
        painter.circle_filled(e + Vec2::new(-rad * 0.04, -rad * 0.05), rad * 0.05, Color32::WHITE);
        painter.circle_filled(o + Vec2::new(sx * rad * 0.62, rad * 0.22), rad * 0.11, Color32::from_rgba_unmultiplied(255, 120, 140, 120));
    }
    painter.line_segment([o + Vec2::new(-rad * 0.12, rad * 0.25), o + Vec2::new(0.0, rad * 0.32)], Stroke::new(1.5_f32, Color32::from_rgb(40, 30, 30)));
    painter.line_segment([o + Vec2::new(0.0, rad * 0.32), o + Vec2::new(rad * 0.12, rad * 0.25)], Stroke::new(1.5_f32, Color32::from_rgb(40, 30, 30)));
    // Mythic pets sparkle.
    if pet.rarity == wolfcraft_engine::clicker::Rarity::Mythic {
        for i in 0..4 {
            let a = t * 1.5 + i as f32 * 1.57;
            let p = o + Vec2::new(a.cos(), a.sin()) * rad * 1.35;
            painter.circle_filled(Pos2::new(p.x, p.y), s * 0.025, Color32::WHITE);
        }
    }
}

/// A capsule egg: two halves in its colours with a shine.
fn paint_capsule(painter: &egui::Painter, r: egui::Rect, egg: usize) {
    use egui::{Color32, Rounding, Stroke, Vec2};
    let e = &wolfcraft_engine::clicker::EGGS[egg];
    let w = r.width().min(r.height()) * 0.55;
    let cap = egui::Rect::from_center_size(r.center(), Vec2::new(w, w * 1.45));
    let top = egui::Rect::from_min_max(cap.min, egui::pos2(cap.right(), cap.center().y));
    let bot = egui::Rect::from_min_max(egui::pos2(cap.left(), cap.center().y), cap.max);
    let c = |v: [u8; 3]| Color32::from_rgb(v[0], v[1], v[2]);
    painter.rect_filled(top, Rounding { nw: w / 2.0, ne: w / 2.0, sw: 0.0, se: 0.0 }, c(e.colors[0]));
    painter.rect_filled(bot, Rounding { nw: 0.0, ne: 0.0, sw: w / 2.0, se: w / 2.0 }, c(e.colors[1]));
    painter.line_segment([egui::pos2(cap.left(), cap.center().y), egui::pos2(cap.right(), cap.center().y)], Stroke::new(2.0_f32, Color32::from_black_alpha(80)));
    painter.rect_filled(egui::Rect::from_min_size(cap.min + Vec2::new(w * 0.2, w * 0.15), Vec2::new(w * 0.18, w * 0.45)), Rounding::same(w * 0.09), Color32::from_white_alpha(120));
    painter.rect_stroke(cap, Rounding::same(w / 2.0), Stroke::new(1.5_f32, Color32::from_black_alpha(100)));
}

/// «Кликер» on screen: the counters on top, a menu row (Улучшения, Капсулы, Питомцы,
/// Перерождение, Острова), the big glossy tap button with «+N» popping out, «Продать»,
/// and the open panel. Returns what was pressed.
fn clicker_overlay(ctx: &egui::Context, icons: &wolfcraft_ui::GuiIcons, c: &wolfcraft_engine::clicker::Clicker, cover: &mut Vec<egui::Rect>) -> Vec<ClickAct> {
    use egui::{Align2, Color32, FontId, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
    use wolfcraft_engine::buttonsim::short;
    use wolfcraft_engine::clicker::{EGGS, ISLANDS, PETS, UPGRADES};
    let _ = icons;
    let mut acts = Vec::new();
    let screen = ctx.screen_rect();
    let t = ctx.input(|i| i.time) as f32;
    let now = ctx.input(|i| i.time);
    let panel_id = egui::Id::new("clicker-panel");
    let mut panel: usize = ctx.data(|d| d.get_temp(panel_id)).unwrap_or(0);
    let bold = |s: f32| egui::FontId::new(s, egui::FontFamily::Name("bold".into()));
    let glass = |p: &egui::Painter, r: Rect, rad: f32| {
        p.rect_filled(r.translate(Vec2::new(0.0, 3.0)), Rounding::same(rad), Color32::from_black_alpha(60));
        p.rect_filled(r, Rounding::same(rad), Color32::from_rgba_unmultiplied(18, 30, 52, 205));
        p.rect_filled(Rect::from_min_max(r.min + Vec2::splat(1.0), Pos2::new(r.right() - 1.0, r.center().y)), Rounding { nw: rad, ne: rad, sw: 0.0, se: 0.0 }, Color32::from_white_alpha(16));
        p.rect_stroke(r, Rounding::same(rad), Stroke::new(1.0_f32, Color32::from_white_alpha(80)));
    };
    egui::Area::new(egui::Id::new("clicker-hud")).order(egui::Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        let pt = ui.painter().clone();
        // ---- Counters ----
        // Narrow enough to leave the minimap (top right) free.
        let w = (screen.width() - 2.0 * 170.0).clamp(300.0, 480.0).min(screen.width() - 16.0);
        let bar = Rect::from_min_size(Pos2::new(screen.center().x - w / 2.0, screen.top() + 10.0), Vec2::new(w, 54.0));
        glass(&pt, bar, 16.0);
        cover.push(Rect::from_min_max(bar.min, Pos2::new(bar.right(), bar.bottom() + 44.0)));
        let cells = [
            ("КЛИКИ", format!("{}/{}", short(c.clicks), short(c.backpack())), if c.full() { Color32::from_rgb(255, 120, 90) } else { Color32::from_rgb(140, 210, 255) }),
            ("ДЕНЬГИ", format!("${}", short(c.money)), Color32::from_rgb(120, 235, 120)),
            ("МНОЖИТЕЛЬ", format!("×{}", short(c.multiplier())), Color32::from_rgb(255, 210, 90)),
            ("ПЕРЕРОЖД.", c.rebirths.to_string(), Color32::from_rgb(200, 150, 255)),
        ];
        let cw = w / cells.len() as f32;
        for (i, (label, val, col)) in cells.iter().enumerate() {
            let x = bar.left() + cw * (i as f32 + 0.5);
            pt.text(Pos2::new(x, bar.top() + 14.0), Align2::CENTER_CENTER, *label, FontId::proportional(10.0), Color32::from_white_alpha(170));
            pt.text(Pos2::new(x, bar.top() + 35.0), Align2::CENTER_CENTER, val, bold(17.0), *col);
        }
        // ---- Menu row ----
        let names = ["Улучшения", "Капсулы", "Питомцы", "Перерождение", "Острова"];
        let gap = 6.0;
        let bw = ((w - gap * 4.0) / 5.0).max(60.0);
        for (i, n) in names.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(bar.left() + (bw + gap) * i as f32, bar.bottom() + 8.0), Vec2::new(bw, 34.0));
            let resp = ui.interact(r, egui::Id::new(("clicker-menu", i)), Sense::click());
            let on = panel == i + 1;
            pt.rect_filled(r, Rounding::same(17.0), if on { Color32::from_rgb(40, 110, 220) } else if resp.hovered() { Color32::from_rgba_unmultiplied(40, 60, 90, 220) } else { Color32::from_rgba_unmultiplied(18, 30, 52, 200) });
            pt.rect_filled(Rect::from_min_max(r.min + Vec2::new(8.0, 2.0), Pos2::new(r.right() - 8.0, r.center().y)), Rounding::same(8.0), Color32::from_white_alpha(if on { 50 } else { 14 }));
            pt.rect_stroke(r, Rounding::same(17.0), Stroke::new(1.0_f32, Color32::from_white_alpha(90)));
            let fs = if bw < 90.0 { 10.5 } else { 12.5 };
            pt.text(r.center(), Align2::CENTER_CENTER, *n, bold(fs), Color32::WHITE);
            if resp.clicked() {
                panel = if on { 0 } else { i + 1 };
            }
        }
        // PC hint (a mouse never set off a touch).
        if !ctx.input(|i| i.any_touches()) && !cfg!(target_os = "android") {
            pt.text(Pos2::new(bar.center().x, bar.bottom() + 52.0), Align2::CENTER_TOP, "ПК: C — клик · V — продать · держите Alt — курсор для меню", FontId::proportional(11.5), Color32::from_white_alpha(170));
        }
        // ---- The big tap button and «Продать» ----
        let rad = (screen.height() * 0.11).clamp(44.0, 70.0);
        let centre = Pos2::new(screen.center().x, screen.bottom() - rad - 96.0);
        let tap = Rect::from_center_size(centre, Vec2::splat(rad * 2.0));
        let resp = ui.interact(tap, egui::Id::new("clicker-tap"), Sense::click());
        cover.push(tap);
        let press = resp.is_pointer_button_down_on();
        let r2 = if press { rad * 0.93 } else { rad * (1.0 + (t * 3.0).sin() * 0.015) };
        pt.circle_filled(centre + Vec2::new(0.0, 4.0), r2, Color32::from_black_alpha(70));
        pt.circle_filled(centre, r2, if c.full() { Color32::from_rgb(170, 70, 60) } else { Color32::from_rgb(255, 160, 40) });
        pt.circle_filled(centre + Vec2::new(0.0, r2 * 0.12), r2 * 0.86, if c.full() { Color32::from_rgb(140, 50, 45) } else { Color32::from_rgb(235, 110, 20) });
        pt.circle_filled(centre - Vec2::new(0.0, r2 * 0.38), r2 * 0.5, Color32::from_white_alpha(55));
        pt.circle_stroke(centre, r2, Stroke::new(2.0_f32, Color32::from_white_alpha(150)));
        pt.text(centre + Vec2::new(0.0, 1.5), Align2::CENTER_CENTER, if c.full() { "ПОЛНО" } else { "КЛИК!" }, bold(rad * 0.38), Color32::from_black_alpha(110));
        pt.text(centre, Align2::CENTER_CENTER, if c.full() { "ПОЛНО" } else { "КЛИК!" }, bold(rad * 0.38), Color32::WHITE);
        if resp.clicked() {
            acts.push(ClickAct::Tap);
        }
        let sell = Rect::from_center_size(Pos2::new(centre.x + rad + 74.0, centre.y), Vec2::new(116.0, 44.0));
        let sresp = ui.interact(sell, egui::Id::new("clicker-sell"), Sense::click());
        cover.push(sell);
        pt.rect_filled(sell.translate(Vec2::new(0.0, 3.0)), Rounding::same(22.0), Color32::from_black_alpha(60));
        pt.rect_filled(sell, Rounding::same(22.0), if sresp.is_pointer_button_down_on() { Color32::from_rgb(40, 140, 50) } else { Color32::from_rgb(60, 180, 70) });
        pt.rect_filled(Rect::from_min_max(sell.min + Vec2::new(14.0, 2.0), Pos2::new(sell.right() - 14.0, sell.center().y)), Rounding::same(10.0), Color32::from_white_alpha(55));
        pt.rect_stroke(sell, Rounding::same(22.0), Stroke::new(1.0_f32, Color32::from_white_alpha(140)));
        pt.text(sell.center(), Align2::CENTER_CENTER, "Продать", bold(15.0), Color32::WHITE);
        if sresp.clicked() {
            acts.push(ClickAct::Sell);
        }
        if c.per_second() > 0.0 {
            pt.text(Pos2::new(centre.x - rad - 74.0, centre.y), Align2::CENTER_CENTER, format!("авто\n+{}/с", short(c.per_second())), FontId::proportional(12.5), Color32::from_white_alpha(220));
        }
        // «+N» popping out of the button.
        let pops: Vec<(f64, String, f32)> = ctx.data(|d| d.get_temp(egui::Id::new("clicker-pops"))).unwrap_or_default();
        for (at, txt, seed) in &pops {
            let k = ((now - at) as f32).clamp(0.0, 1.0);
            if k >= 1.0 {
                continue;
            }
            let p = centre + Vec2::new((seed - 0.5) * rad * 2.2, -rad - 10.0 - k * 70.0);
            let a = ((1.0 - k) * 255.0) as u8;
            pt.text(p + Vec2::new(0.0, 1.5), Align2::CENTER_CENTER, txt, bold(20.0), Color32::from_black_alpha(a / 2));
            pt.text(p, Align2::CENTER_CENTER, txt, bold(20.0), Color32::from_rgba_unmultiplied(255, 240, 120, a));
        }
        // The pet that just hatched, big in the middle for two seconds.
        if let Some((at, k)) = ctx.data(|d| d.get_temp::<Option<(f64, usize)>>(egui::Id::new("clicker-hatched"))).flatten() {
            let age = (now - at) as f32;
            if age < 2.2 {
                let s = 150.0 * (age * 4.0).min(1.0);
                let r = Rect::from_center_size(screen.center(), Vec2::splat(s));
                paint_pet(&pt, r, k, t);
                let pet = &PETS[k];
                let rc = pet.rarity.color();
                pt.text(r.center() + Vec2::new(0.0, s * 0.62), Align2::CENTER_CENTER, format!("{} · {}", pet.rarity.name(), pet.name), bold(18.0), Color32::from_rgb(rc[0], rc[1], rc[2]));
            }
        }
        ctx.request_repaint();
    });
    ctx.data_mut(|d| d.insert_temp(panel_id, panel));
    if panel == 0 {
        return acts;
    }
    // ---- The open panel ----
    let title = ["", "Улучшения", "Капсулы", "Питомцы", "Перерождение", "Острова"][panel];
    let mut open = true;
    egui::Window::new(egui::RichText::new(title).font(bold(17.0)))
        .id(egui::Id::new("clicker-window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 10.0))
        .default_width(340.0f32.min(screen.width() - 30.0))
        .frame(egui::Frame::window(&ctx.style()).fill(Color32::from_rgba_unmultiplied(16, 26, 46, 240)).rounding(Rounding::same(16.0)).stroke(Stroke::new(1.0_f32, Color32::from_white_alpha(90))))
        .show(ctx, |ui| {
            ui.set_width(340.0f32.min(screen.width() - 50.0));
            let row = |ui: &mut egui::Ui, draw: &dyn Fn(&egui::Painter, Rect), name: String, info: String, price: String, can: bool| -> bool {
                let mut clicked = false;
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(52.0), Sense::hover());
                    ui.painter().rect_filled(r, Rounding::same(12.0), Color32::from_white_alpha(14));
                    draw(ui.painter(), r);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(name).font(bold(14.5)).color(Color32::WHITE));
                        ui.label(egui::RichText::new(info).size(12.0).color(Color32::from_white_alpha(180)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let b = egui::Button::new(egui::RichText::new(price).font(bold(13.5)).color(Color32::WHITE)).fill(if can { Color32::from_rgb(60, 170, 70) } else { Color32::from_rgb(70, 75, 90) }).rounding(Rounding::same(14.0)).min_size(Vec2::new(92.0, 32.0));
                        clicked = ui.add(b).clicked();
                    });
                });
                ui.add_space(6.0);
                clicked
            };
            egui::ScrollArea::vertical().max_height(screen.height() * 0.6).show(ui, |ui| match panel {
                1 => {
                    for (k, (name, what, _, _)) in UPGRADES.iter().enumerate() {
                        let price = c.upgrade_price(k);
                        let lvl = c.upgrades[k];
                        let paint = move |p: &egui::Painter, r: Rect| {
                            p.text(r.center(), Align2::CENTER_CENTER, ["👆", "⚙", "🎒"][k], FontId::proportional(24.0), Color32::WHITE);
                        };
                        if row(ui, &paint, format!("{name} · ур. {lvl}"), what.to_string(), format!("${}", short(price)), c.money >= price) {
                            acts.push(ClickAct::Upgrade(k));
                        }
                    }
                }
                2 => {
                    for (k, e) in EGGS.iter().enumerate() {
                        let odds = ["об", "ред", "эп", "лег", "миф"].iter().zip(e.weights).filter(|(_, w)| *w > 0).map(|(n, w)| format!("{n} {w}%")).collect::<Vec<_>>().join(" · ");
                        if row(ui, &move |p: &egui::Painter, r: Rect| paint_capsule(p, r, k), e.name.to_string(), odds, format!("${}", short(e.price)), c.money >= e.price) {
                            acts.push(ClickAct::Hatch(k));
                        }
                    }
                }
                3 => {
                    let best = c.best();
                    ui.label(egui::RichText::new(format!("Работают {} лучших: ×{:.1} к кликам · всего {}", best.len(), c.pet_mult(), c.pets.len())).size(12.5).color(Color32::from_white_alpha(200)));
                    if c.pets.is_empty() {
                        ui.label(egui::RichText::new("Питомцев пока нет — откройте капсулу.").color(Color32::from_white_alpha(170)));
                    }
                    let mut sorted = c.pets.clone();
                    sorted.sort_by(|a, b| PETS[*b].mult.partial_cmp(&PETS[*a].mult).unwrap_or(std::cmp::Ordering::Equal));
                    ui.horizontal_wrapped(|ui| {
                        for (i, k) in sorted.iter().enumerate() {
                            let (r, _) = ui.allocate_exact_size(Vec2::new(76.0, 92.0), Sense::hover());
                            let pet = &PETS[*k];
                            let rc = pet.rarity.color();
                            ui.painter().rect_filled(r, Rounding::same(12.0), Color32::from_white_alpha(12));
                            ui.painter().rect_stroke(r, Rounding::same(12.0), Stroke::new(if i < best.len() { 2.0 } else { 1.0 }, Color32::from_rgb(rc[0], rc[1], rc[2])));
                            paint_pet(ui.painter(), Rect::from_min_size(r.min + Vec2::new(8.0, 4.0), Vec2::splat(60.0)), *k, t);
                            ui.painter().text(Pos2::new(r.center().x, r.bottom() - 22.0), Align2::CENTER_CENTER, pet.name, FontId::proportional(10.0), Color32::WHITE);
                            ui.painter().text(Pos2::new(r.center().x, r.bottom() - 9.0), Align2::CENTER_CENTER, format!("×{}", pet.mult), FontId::proportional(10.5), Color32::from_rgb(rc[0], rc[1], rc[2]));
                        }
                    });
                }
                4 => {
                    // The rebirth shop: one or a pack at once.
                    ui.label(egui::RichText::new(format!("Сейчас: перерождений {} · множитель ×{:.1}", c.rebirths, c.rebirth_mult())).color(Color32::WHITE));
                    ui.label(egui::RichText::new("Каждое перерождение: +×0.5 к кликам и рюкзак больше. Деньги, клики и улучшения обнулятся; питомцы и острова останутся.").size(12.5).color(Color32::from_white_alpha(190)));
                    ui.add_space(6.0);
                    for n in wolfcraft_engine::clicker::Clicker::PACKS {
                        let price = c.rebirths_price(n);
                        let paint = move |p: &egui::Painter, r: Rect| {
                            p.text(r.center() - Vec2::new(0.0, 6.0), Align2::CENTER_CENTER, "♻", FontId::proportional(22.0), Color32::from_rgb(200, 150, 255));
                            p.text(r.center() + Vec2::new(0.0, 14.0), Align2::CENTER_CENTER, format!("+{n}"), FontId::proportional(12.0), Color32::WHITE);
                        };
                        let after = 1.0 + (c.rebirths + n) as f64 * 0.5;
                        if row(ui, &paint, format!("{n} перерожд."), format!("станет ×{after:.1}"), format!("${}", short(price)), c.money >= price) {
                            acts.push(ClickAct::Rebirth(n));
                        }
                    }
                }
                _ => {
                    for (k, isle) in ISLANDS.iter().enumerate() {
                        let here = c.island == k;
                        let open = k <= c.unlocked;
                        let price = if open { if here { "Вы здесь".to_string() } else { "Перейти".to_string() } } else { format!("${}", short(isle.price)) };
                        let colour = [[90, 200, 90], [40, 140, 70], [170, 220, 255], [220, 80, 40], [250, 230, 140]][k];
                        let paint = move |p: &egui::Painter, r: Rect| {
                            let c = Color32::from_rgb(colour[0], colour[1], colour[2]);
                            p.circle_filled(r.center() + Vec2::new(0.0, 4.0), r.width() * 0.3, Color32::from_rgb(110, 90, 70));
                            p.circle_filled(r.center(), r.width() * 0.3, c);
                            p.circle_filled(r.center() - Vec2::new(r.width() * 0.08, r.width() * 0.1), r.width() * 0.1, Color32::from_white_alpha(80));
                        };
                        if row(ui, &paint, isle.name.to_string(), format!("клики ×{} · продажа ×{}", isle.mult, k + 1), price, !here && (open || (k == c.unlocked + 1 && c.money >= isle.price))) && !here {
                            acts.push(ClickAct::Island(k));
                        }
                    }
                }
            });
        });
    if !open {
        ctx.data_mut(|d| d.insert_temp(panel_id, 0usize));
    }
    if let Some(r) = ctx.memory(|m| m.area_rect(egui::Id::new("clicker-window"))) {
        cover.push(r);
    }
    acts
}

fn buttonsim_overlay(ctx: &egui::Context, b: &wolfcraft_engine::buttonsim::ButtonSim) {
    use egui::{Align2, Color32, FontId, Pos2, Rect, Rounding, Stroke, Vec2};
    use wolfcraft_engine::buttonsim::short;
    if !b.active {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("buttonsim")));
    let screen = ctx.screen_rect();
    let w = 210.0;
    let r = Rect::from_min_size(Pos2::new(screen.center().x - w / 2.0, screen.top() + 12.0), Vec2::new(w, 86.0));
    painter.rect_filled(r, Rounding::same(14.0), Color32::from_black_alpha(150));
    painter.rect_stroke(r, Rounding::same(14.0), Stroke::new(1.0_f32, Color32::from_white_alpha(40)));
    painter.text(Pos2::new(r.center().x, r.top() + 22.0), Align2::CENTER_CENTER, format!("$ {}", short(b.money)), FontId::proportional(24.0), Color32::from_rgb(120, 230, 120));
    painter.text(Pos2::new(r.center().x, r.top() + 46.0), Align2::CENTER_CENTER, format!("+{}/с", short(b.income())), FontId::proportional(13.0), Color32::from_white_alpha(200));
    painter.text(Pos2::new(r.left() + 14.0, r.bottom() - 16.0), Align2::LEFT_CENTER, format!("×{}", short(b.multiplier)), FontId::proportional(15.0), Color32::from_rgb(110, 170, 255));
    painter.text(Pos2::new(r.right() - 14.0, r.bottom() - 16.0), Align2::RIGHT_CENTER, format!("перерожд. {}", b.rebirths), FontId::proportional(15.0), Color32::from_rgb(190, 140, 255));
}

/// The building block whose colour is closest to a pixel (for «В блоки» in the texture editor).
fn nearest_block(c: [u8; 4]) -> wolfcraft_engine::block::Block {
    use wolfcraft_engine::block::Block::*;
    const PAL: [(wolfcraft_engine::block::Block, [i32; 3]); 34] = [
        (Wool, [235, 235, 235]),
        (RedWool, [180, 40, 40]),
        (BlueWool, [50, 70, 170]),
        (YellowWool, [225, 200, 50]),
        (Stone, [125, 125, 125]),
        (Cobblestone, [105, 105, 105]),
        (Dirt, [120, 85, 60]),
        (MossBlock, [90, 130, 50]),
        (OakPlanks, [170, 135, 85]),
        (SprucePlanks, [115, 85, 55]),
        (BirchPlanks, [205, 190, 140]),
        (Sand, [220, 205, 150]),
        (RedSand, [190, 105, 50]),
        (Terracotta, [160, 90, 65]),
        (Bricks, [150, 75, 60]),
        (Snow, [248, 250, 252]),
        (CoalBlock, [25, 25, 28]),
        (IronBlock, [210, 210, 214]),
        (GoldBlock, [240, 200, 60]),
        (DiamondBlock, [100, 220, 215]),
        (CopperBlock, [190, 110, 75]),
        (Obsidian, [30, 20, 45]),
        (Basalt, [70, 70, 75]),
        (Clay, [160, 165, 180]),
        (Marble, [230, 228, 222]),
        (Granite, [150, 100, 85]),
        (Pumpkin, [215, 125, 30]),
        (Melon, [110, 150, 40]),
        (Netherrack, [120, 40, 40]),
        (PackedIce, [160, 190, 240]),
        (Slate, [70, 80, 90]),
        (Limestone, [200, 195, 170]),
        (OakLog, [110, 85, 55]),
        (Mud, [80, 65, 55]),
    ];
    let px = [c[0] as i32, c[1] as i32, c[2] as i32];
    // Weighted distance (the eye sees green best).
    let d = |q: [i32; 3]| 3 * (px[0] - q[0]).pow(2) + 4 * (px[1] - q[1]).pow(2) + 2 * (px[2] - q[2]).pow(2);
    PAL.iter().min_by_key(|(_, q)| d(*q)).map(|(b, _)| *b).unwrap_or(Wool)
}



/// «Что это?» (like WAILA): what the crosshair is on — a creature or a block — with its name,
/// where it comes from, whether the held tool can mine it, and machine energy / health bars.
fn look_info(s: &crate::session::GameSession) -> Option<wolfcraft_ui::hud::LookInfo> {
    use egui::Color32;
    use wolfcraft_engine::block::ToolKind;
    use wolfcraft_engine::items::{self, ItemStack, ids};
    let green = Color32::from_rgb(120, 220, 120);
    let red = Color32::from_rgb(240, 110, 100);
    let grey = Color32::from_white_alpha(190);
    let eye = s.player.eye_pos();
    let look = s.player.look_dir();
    // A creature in front of the block wins.
    if let Some((i, t)) = s.mobs.raycast(eye, look, 8.0) {
        if s.hit.is_none_or(|h| t < h.dist) {
            let m = &s.mobs.mobs[i];
            use wolfcraft_engine::mobs::{ALL_MOBS, MobKind};
            let icon = match ALL_MOBS.iter().position(|k| *k == m.kind) {
                Some(n) => ids::SOUL_BASE + n as u16,
                None if m.kind == MobKind::Dragon => ids::DRAGON_SOUL,
                None => ids::RAW_FISH,
            };
            let (name, source, max) = match &m.boss {
                Some(b) => (b.kind.name().to_string(), "Страж · WolfCraft".to_string(), m.hp.max(1.0).max(b.kind.max_hp())),
                None => (m.kind.name().to_string(), "Существо · WolfCraft".to_string(), m.kind.max_hp()),
            };
            let mut lines = vec![if m.kind.hostile() || m.boss.is_some() { ("Враждебный".to_string(), red) } else { ("Мирный".to_string(), green) }];
            if m.saddled {
                lines.push(("Под седлом — можно оседлать".into(), grey));
            }
            let f = m.hp / max.max(1.0);
            return Some(wolfcraft_ui::hud::LookInfo { icon: ItemStack::new(icon, 1), name, source, lines, bar: Some((f, Color32::from_rgb(220, 60, 60), format!("♥ {:.0} / {:.0}", m.hp.max(0.0), max))) });
        }
    }
    let h = s.hit?;
    let block = h.block;
    let base = block.base_item();
    let id = base as items::ItemId;
    let info = block.info();
    let source = match items::category(id) {
        "WolfTech" => "WolfTech".to_string(),
        c => format!("WolfCraft · {c}"),
    };
    let mut lines = Vec::new();
    let mut bar = None;
    if info.hardness < 0.0 {
        lines.push(("Неразрушимый".to_string(), grey));
    } else {
        let tool = match info.tool {
            ToolKind::Pickaxe => Some("кирка"),
            ToolKind::Axe => Some("топор"),
            ToolKind::Shovel => Some("лопата"),
            ToolKind::Hand => None,
        };
        let held = s.player.inventory.held().map(|st| st.info());
        if info.needs_tool {
            if items::can_harvest(held, block) {
                lines.push(("✓ Добывается тем, что в руке".to_string(), green));
            } else {
                let tier = ["деревянная", "каменная", "железная", "алмазная"][(info.min_tier as usize).min(3)];
                lines.push((format!("✗ Нужна {} ({tier} или лучше)", tool.unwrap_or("кирка")), red));
            }
        } else if let Some(t) = tool {
            lines.push((format!("Быстрее: {t}"), grey));
        }
    }
    // WolfTech machines: energy, what they do now; the reactor's heat.
    if let Some((prog, working, e, cap)) = s.tech_status(h.cell, block) {
        let k = wolfcraft_engine::wolftech::kind(block).unwrap();
        if k == wolfcraft_engine::wolftech::Kind::Reactor {
            let (heat, max) = s.reactor_heat(h.cell);
            lines.push((format!("🌡 Нагрев {:.0} / {:.0}", heat, max), if heat > max * 0.7 { red } else { grey }));
        }
        if wolfcraft_engine::wolftech::is_machine(k) {
            lines.push((if working { format!("Работает · {:.0} %", prog * 100.0) } else { "Ждёт сырьё или энергию".to_string() }, if working { green } else { grey }));
        } else if wolfcraft_engine::wolftech::is_source(k) {
            let flow = s.tech.get(&h.cell).map(|t| t.flow).unwrap_or(0.0);
            lines.push((format!("Даёт {:.0} ЭЕ/с", flow), if flow > 0.0 { green } else { grey }));
        }
        bar = Some((e / cap.max(1.0), Color32::from_rgb(60, 190, 230), format!("⚡ {:.0} / {:.0} ЭЕ", e, cap)));
    } else if let Some(n) = wolfcraft_engine::kinetic::part(block).map(|_| s.kin.get(&h.cell).copied().unwrap_or_default()) {
        let line = if n.speed > 0.0 {
            (format!("⚙ {:.0} об/мин", n.speed), green)
        } else if n.cap > 0.0 && n.load > n.cap {
            ("⚠ Перегрузка — сеть стоит".to_string(), red)
        } else {
            ("Не крутится".to_string(), grey)
        };
        lines.push(line);
        if n.cap > 0.0 {
            bar = Some(((n.load / n.cap).min(1.0), if n.load > n.cap { Color32::from_rgb(230, 80, 60) } else { Color32::from_rgb(240, 180, 60) }, format!("Нагрузка {:.0} / {:.0} SU", n.load, n.cap)));
        }
    } else if let Some(v) = s.containers.get(&h.cell) {
        let n: u32 = v.iter().flatten().map(|x| x.count as u32).sum();
        lines.push((if n == 0 { "Пусто".to_string() } else { format!("Внутри: {n} предм.") }, grey));
    }
    Some(wolfcraft_ui::hud::LookInfo { icon: ItemStack::new(id, 1), name: base.info().name.to_string(), source, lines, bar })
}

/// The drill's mode pill on the right: the digging square drawn as a little grid and its
/// size; true when tapped (the caller moves to the next mode).
fn drill_overlay(ctx: &egui::Context, mode: u8, cover: &mut Vec<egui::Rect>) -> bool {
    use egui::{Align2, Color32, FontId, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
    let side = wolfcraft_engine::tools::drill_side(mode);
    let mut tapped = false;
    let screen = ctx.screen_rect();
    let size = Vec2::new(132.0, 46.0);
    let pos = Pos2::new(screen.right() - size.x - 14.0, screen.center().y - 110.0);
    egui::Area::new(egui::Id::new("drill-mode")).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(size, Sense::click());
        let p = ui.painter();
        let down = resp.is_pointer_button_down_on();
        p.rect_filled(r.translate(Vec2::new(0.0, 3.0)), Rounding::same(23.0), Color32::from_black_alpha(60));
        p.rect_filled(r, Rounding::same(23.0), if down { Color32::from_rgba_unmultiplied(40, 120, 200, 220) } else { Color32::from_rgba_unmultiplied(20, 40, 70, 170) });
        // Glass gloss on the top half and a light rim.
        p.rect_filled(Rect::from_min_max(r.min, Pos2::new(r.right(), r.center().y)).shrink2(Vec2::new(6.0, 2.0)), Rounding::same(18.0), Color32::from_white_alpha(28));
        p.rect_stroke(r, Rounding::same(23.0), Stroke::new(1.0_f32, Color32::from_white_alpha(110)));
        // The square: 1, 3, 5 … cells (at most 7 drawn, a wider one stays 7 small dots).
        let g = Rect::from_center_size(Pos2::new(r.left() + 26.0, r.center().y), Vec2::splat(28.0));
        let n = side.min(7) as usize;
        let cell = g.width() / n as f32;
        for y in 0..n {
            for x in 0..n {
                let c = Rect::from_min_size(Pos2::new(g.left() + x as f32 * cell, g.top() + y as f32 * cell), Vec2::splat(cell)).shrink((cell * 0.12).min(1.5));
                let mid = x == n / 2 && y == n / 2;
                p.rect_filled(c, Rounding::same(cell * 0.2), if mid { Color32::from_rgb(255, 196, 70) } else { Color32::from_rgb(120, 200, 255) });
            }
        }
        p.text(Pos2::new(r.left() + 50.0, r.center().y - 7.0), Align2::LEFT_CENTER, format!("{side}×{side}"), FontId::proportional(17.0), Color32::WHITE);
        p.text(Pos2::new(r.left() + 50.0, r.center().y + 10.0), Align2::LEFT_CENTER, "бур · режим", FontId::proportional(11.0), Color32::from_white_alpha(190));
        cover.push(r);
        if resp.clicked() {
            tapped = true;
        }
    });
    tapped
}
