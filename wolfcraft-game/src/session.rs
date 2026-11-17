//! A running game: planet + chunk streaming + player + interaction.

use wolfcraft_engine::cubesphere::{along, radial};
use crate::loader::Loaded;
use glam::DVec3;
use std::path::PathBuf;
use std::sync::Arc;
use wolfcraft_common::platform;
use wolfcraft_engine::cubesphere::ChunkKey;
use wolfcraft_engine::chunk_manager::ChunkManager;
use wolfcraft_engine::config::{GameMode, WorldConfig};
use wolfcraft_engine::interaction::{Interaction, InteractionEvent};

/// Height above the baseplate the studio camera and players can rise to (no space there).
pub const BASEPLATE_CEILING: f64 = 160.0;
use wolfcraft_engine::physics::{Hit, Voxels, raycast};
use wolfcraft_engine::planet::{Planet, smoothstep};
use wolfcraft_engine::player::{Controls, Player, clock_string, local_sun};

/// A guardian's cut-scene: where, how long it has run, how long it lasts.
#[derive(Clone, Copy, Debug)]
struct BossFilm {
    at: DVec3,
    t: f32,
    dur: f32,
}

/// The remote's skills, in the order sneak + tap cycles them.
pub const STELLAR_NAMES: [&str; 11] = ["Рельсотрон", "Игла", "Семь звёзд", "Тессеракт", "Чёрная дыра", "Метеоритный дождь", "Солнечное копьё", "Ледяная комета", "Сверхновая", "Раскол планеты", "Орбитальный обстрел"];

/// A skill of «Звёздный пульт» being cast: where, how long ago it began, its own state.
#[derive(Clone, Debug)]
struct Stellar {
    mode: u8,
    at: DVec3,
    cell: wolfcraft_engine::cubesphere::Cell,
    t: f32,
    /// Seven stars: the points of the Big Dipper.
    pts: Vec<DVec3>,
    /// Seven stars: trenches burnt (1); tesseract: cells torn out so far.
    step: u32,
    /// Along the ground, the way the player looked (for the cut-scene camera).
    side: DVec3,
}

/// A rocket flight: lift-off (`phase` 0), in orbit choosing the planet (1), landing (2).
#[derive(Clone, Copy, Debug)]
pub struct RocketFlight {
    pub phase: u8,
    /// Seconds in this phase.
    pub t: f32,
    /// Speed along up (blocks/s).
    pub speed: f64,
}

/// Everything the player did this frame.
#[derive(Default, Clone, Debug)]
pub struct InputFrame {
    pub forward: f32,
    pub strafe: f32,
    pub jump: bool,
    pub jump_pressed: bool,
    pub sprint: bool,
    pub sneak: bool,
    pub look: (f64, f64),
    pub break_held: bool,
    pub use_pressed: bool,
    /// RMB is held (shield block).
    pub use_held: bool,
    /// F: swap main hand and second hand.
    pub swap_hands: bool,
    pub pick_pressed: bool,
    pub hotbar_delta: i32,
    pub hotbar_set: Option<usize>,
    /// Touch targeting: the direction under the finger (`None`: nothing is targeted).
    /// Without touch (`touch` false) the view centre is used.
    pub aim: Option<DVec3>,
    pub touch: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CameraMode {
    /// Eyes of the player.
    #[default]
    First,
    /// Behind the player, looking the same way.
    ThirdBack,
    /// In front of the player, looking back at the character.
    ThirdFront,
}

impl CameraMode {
    pub fn next(self) -> Self {
        match self {
            CameraMode::First => CameraMode::ThirdBack,
            CameraMode::ThirdBack => CameraMode::ThirdFront,
            CameraMode::ThirdFront => CameraMode::First,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            CameraMode::First => "Вид от первого лица",
            CameraMode::ThirdBack => "Вид сзади",
            CameraMode::ThirdFront => "Вид спереди",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: DVec3,
    pub forward: DVec3,
    pub up: DVec3,
}

/// Seconds to smelt one item in an electric furnace.
const FURNACE_TIME: f32 = 3.0;

/// A number or word that pops out of the world and floats up (damage, healing).
#[derive(Clone, Debug)]
pub struct Popup {
    pub at: DVec3,
    pub text: String,
    pub color: [u8; 3],
    pub age: f32,
    /// A heavy blow: bigger, with a shake.
    pub big: bool,
}

/// One part of a «Механика» network: how fast it turns (RPM), which way, the network's
/// stress (load / capacity), a machine's progress and a crank's turning time left.
#[derive(Clone, Copy, Debug, Default)]
pub struct KinNode {
    pub speed: f32,
    pub sign: i8,
    pub load: f32,
    pub cap: f32,
    pub progress: f32,
    pub crank: f32,
}

/// State of one WolfTech machine.
#[derive(Clone, Copy, Debug, Default)]
pub struct TechState {
    pub energy: f32,
    pub progress: f32,
    /// Generator: seconds of fuel left.
    pub burn: f32,
    /// ЭЕ/s made (generators) or wanted (machines) this frame.
    pub flow: f32,
    pub working: bool,
    /// Solar panel: in the sun right now / it is day.
    pub sunny: bool,
    pub day: bool,
    /// Wind / lava / water around a generator (its share of the full rate).
    pub env: f32,
}

pub struct GameSession {
    pub cfg: WorldConfig,
    pub dir: PathBuf,
    pub planet: Arc<Planet>,
    pub chunks: ChunkManager,
    pub player: Player,
    pub interaction: Interaction,
    /// Items lying in the world.
    pub drops: wolfcraft_engine::drops::Drops,
    pub hit: Option<Hit>,
    pub item_name: Option<(String, f32)>,
    pub near_table: bool,
    autosave: f32,
    last_jump_press: f32,
    clock: f32,
    last_selected: usize,
    pub saved_notice: f32,
    pub camera_mode: CameraMode,
    /// Camera mode to return to after resting.
    rest_camera: Option<CameraMode>,
    /// Vertical camera lag (blocks along up) that smooths automatic steps.
    cam_lag: f64,
    /// Camera feel: dip after a hard landing, last vertical speed (to spot the landing),
    /// third-person shoulder offset (smoothed, shrinks near walls).
    land_dip: f64,
    /// WolfTech machines: energy, progress, fuel left; the networks (rebuilt every second).
    pub tech: std::collections::HashMap<wolfcraft_engine::cubesphere::Cell, TechState>,
    tech_nets: Vec<Vec<wolfcraft_engine::cubesphere::Cell>>,
    /// «Механика»: every known shaft, cog, wheel, crank and machine, and when to re-solve.
    pub kin: std::collections::HashMap<wolfcraft_engine::cubesphere::Cell, KinNode>,
    kin_t: f32,
    kin_accum: f64,
    /// WolfTech item pipes placed in this world, and the timer of their next push.
    pub pipes: std::collections::HashSet<wolfcraft_engine::cubesphere::Cell>,
    pipe_t: f32,
    tech_t: f32,
    /// Seconds to the next entity-culling pass.
    cull_t: f32,
    /// Seconds to the next fire step (spreading, burning out).
    fire_t: f32,
    /// Rain, thunderstorms and snow.
    pub weather: crate::weather::Weather,
    /// Seconds to the next "am I stuck in blocks?" check.
    stuck_check: f32,
    /// View bobbing: step phase, smoothed strength, time off the ground.
    bob_phase: f64,
    bob_amp: f64,
    /// Camera leans into sideways movement (radians, smoothed).
    lean: f64,
    air_t: f64,
    prev_vy: f64,
    shoulder: f64,
    prev_radius: f64,
    /// Smoothed eye radius (filters tiny collision jitter).
    eye_r: f64,
    /// Smoothed third-person distance.
    tp_dist: f64,
    pub mobs: wolfcraft_engine::mobs::MobManager,
    /// Arrows in flight or stuck in blocks.
    pub arrows: wolfcraft_engine::projectiles::Arrows,
    /// Lit dynamite and grenades / rockets in flight.
    pub explosives: wolfcraft_engine::explosives::Explosives,
    /// Destruction physics: loose structures falling, blocks thrown by blasts.
    pub collapse: wolfcraft_engine::collapse::Collapse,
    /// «Питомник» mini-game: coins, hatched pets, the pets following the player.
    pub petsim: wolfcraft_engine::petsim::PetSim,
    /// Seconds to the next scan for coin blocks around the player.
    coin_scan: f32,
    /// Coin blocks near the player (cell, centre), refreshed by the scan.
    coin_cells: Vec<(wolfcraft_engine::cubesphere::Cell, DVec3)>,
    /// The lobby was built here (coin blocks keep appearing near it).
    pub petsim_home: Option<DVec3>,
    /// «Кнопки» mini-game: money, multiplier, rebirths.
    pub buttonsim: wolfcraft_engine::buttonsim::ButtonSim,
    /// «Кликер-симулятор» (WolfStudio template).
    pub clicker: wolfcraft_engine::clicker::Clicker,
    /// PvP templates (Бедварс, Скайварс, Эггварс, Мардер, Прятки): the running game.
    pub arena: Option<wolfcraft_engine::arenas::Arena>,
    arena_dead_seen: bool,
    /// Bow draw 0..1 while the use button is held with a bow.
    /// The player is asleep in a bed or sits on a chair (drawn in third person, cannot walk).
    pub rest: Option<Rest>,
    pub bow_charge: f32,
    /// The arrow the bow would shoot now (drawn on the string, tinted if tipped).
    pub bow_arrow: Option<wolfcraft_engine::items::ItemId>,
    /// Seed of the Чаровальня offers (changes after each enchanting).
    pub ench_seed: u32,
    prev_use_held: bool,
    /// The dragon the player rides (index into the creatures).
    pub riding: Option<usize>,
    /// «Звёздный пульт»: stars still to call down (where, seconds left) and its cooldown.
    star_queue: Vec<(DVec3, f32)>,
    star_cd: f32,
    /// The remote's menu: chosen skill, power, damage, speed, cut-scenes.
    pub stellar_ui: wolfcraft_ui::stellar_ui::StellarMenu,
    /// Camera of a cut-scene (overrides the player's), cinema bars 0..1, screen flash.
    pub cutscene: Option<Camera>,
    pub cinema: f32,
    pub flash: Option<([f32; 3], f32)>,
    /// «Пропустить»: no film for the rest of this cast.
    pub skip_film: bool,
    /// Epic touches: camera shake (0..1), slow motion left (s), the cast's title (name,
    /// motto, age), burning embers left in craters (where, seconds, radius), late booms
    /// (where, power, delay), blocks the last strike tore out (for flying chips).
    shake: f32,
    slowmo: f32,
    /// «Звёздный молот» cooldown.
    hammer_cd: f32,
    ship_hammer_mode: u8,
    /// A guardian's cut-scene (awakening, rage, defeat) and what was last seen of the
    /// guardian (health, phase) to notice hits and phase changes.
    boss_film: Option<BossFilm>,
    boss_seen: Option<(f32, u8)>,
    pub title: Option<(String, String, f32)>,
    embers: Vec<(DVec3, f32, f64)>,
    booms: Vec<(DVec3, f32, f32)>,
    stellar_removed: Vec<(wolfcraft_engine::cubesphere::Cell, wolfcraft_engine::block::Block)>,
    /// The skill being cast now.
    stellar: Option<Stellar>,
    /// A ritual of revival going on at an altar: (guardian, altar, seconds since it began).
    pub ritual: Option<(wolfcraft_engine::bosses::BossKind, wolfcraft_engine::cubesphere::Cell, f32)>,
    attack_cd: f32,
    death_burst_done: bool,
    /// Contents of chests (27 slots) and electric furnaces (0 input, 1 output) by block cell.
    /// Command typed into each command block.
    /// In the Hell dimension.
    pub hell: bool,
    /// Player name used in chat.
    pub nickname: String,
    /// Block changes from other players waiting for their chunk to load.
    pub remote_edits: Vec<(wolfcraft_engine::cubesphere::Cell, wolfcraft_engine::block::Block)>,
    /// Set when the dimension changed: the renderer must drop every chunk mesh.
    pub dimension_changed: bool,
    overworld_pos: Option<DVec3>,
    /// The planet being played (home or one the rocket flew to).
    pub planet_kind: wolfcraft_engine::planet::PlanetKind,
    /// A rocket flight in progress (lift-off, choosing the planet, landing).
    pub rocket: Option<RocketFlight>,
    /// The planet picked in the rocket menu (the app sets it).
    pub rocket_pick: Option<wolfcraft_engine::planet::PlanetKind>,
    portal_t: f32,
    pending_portal: Option<wolfcraft_engine::cubesphere::Cell>,
    pub block_commands: std::collections::HashMap<wolfcraft_engine::cubesphere::Cell, String>,
    pub storage_bank: wolfcraft_engine::storage_bank::StorageBank,
    pub boss_book: wolfcraft_engine::bosses::BossBook,
    pub containers: std::collections::HashMap<wolfcraft_engine::cubesphere::Cell, Vec<Option<wolfcraft_engine::items::ItemStack>>>,
    /// Smelting progress in seconds per furnace.
    furnace_t: std::collections::HashMap<wolfcraft_engine::cubesphere::Cell, f32>,
    /// Seconds of fuel still burning in each furnace.
    pub furnace_fuel: std::collections::HashMap<wolfcraft_engine::cubesphere::Cell, f32>,
    /// The start position was checked for blocks (a structure may have been built over it).
    spawn_checked: bool,
    /// Opened in the studio: creative rules without changing the world's saved mode.
    pub studio: bool,
    /// Mini-games («Питомник», «Кнопки») run only on servers and in the studio, not in
    /// ordinary single-player worlds. The app sets it every frame.
    pub minigames: bool,
    /// Pressed buttons and the seconds until they spring back.
    buttons: Vec<(wolfcraft_engine::cubesphere::Cell, f32)>,
    /// Pressed pressure plates and the seconds until they spring back up.
    pub plates: Vec<(wolfcraft_engine::cubesphere::Cell, f32)>,
    /// Random ticks owed (see `random_ticks`).
    tick_acc: f32,
    /// WorldEdit selection, clipboard and history.
    pub we: crate::worldedit::WorldEdit,
    /// Special effects: amount (the particles setting), light sources nearby (refreshed twice
    /// a second), timers and the player's state last frame (for splashes and landing dust).
    pub fx_amount: f32,
    fx_lights: Vec<(DVec3, wolfcraft_engine::block::Block)>,
    /// Reactor meltdowns still going up as a mushroom cloud: (ground point, seconds since).
    nukes: Vec<(DVec3, f32)>,
    fx_scan: f32,
    fx_dust: f32,
    fx_was_in_water: bool,
    fx_prev_vup: f64,
    fx_was_ground: bool,
    /// Biome around the player (biome, temperature, humidity), refreshed twice a second.
    env_col: Option<(wolfcraft_engine::biome::Biome, f32, f32)>,
    /// Direction of the current targeting ray (view centre, or under the finger on phones).
    aim_dir: Option<DVec3>,
    /// Blows that landed on creatures this frame (the app plays the swing and the sound).
    pub mob_hits: u32,
    /// Items picked up from the ground since the game last looked (pop sound).
    pub picked_up: u32,
    /// Pop-ups: damage numbers over creatures, and the pickup feed over the hotbar
    /// (item, how many, seconds shown).
    pub popups: Vec<Popup>,
    pub pickups: Vec<(wolfcraft_engine::items::ItemId, u32, f32)>,
    /// Achievements of this world, and the toasts showing now (index in the list, seconds).
    pub achieved: std::collections::HashSet<String>,
    pub ach_toasts: Vec<(usize, f32)>,
    /// Achievements granted since the game last played their chime.
    pub ach_new: u32,
    ach_t: f32,
    /// Radiation dose from uranium (seconds of exposure); every 3 s of it costs half a heart.
    radiation: f32,
    /// Flowing water and lava around edits.
    fluids: wolfcraft_engine::fluids::Fluids,
    /// Block changes made by the game itself (a button springing back) for the server.
    pub net_out: Vec<(wolfcraft_engine::cubesphere::Cell, wolfcraft_engine::block::Block)>,
}

impl GameSession {
    pub fn new(l: Loaded) -> Self {
        wolfcraft_engine::cubesphere::set_world_grid(l.planet.grid);
        // Home gravity until the saved place says otherwise (a world played before may have
        // been on the Moon).
        wolfcraft_engine::physics::set_gravity_scale(1.0);
        let chunks = ChunkManager::new(l.planet.clone(), l.storage.clone());
        let seed = l.cfg.seed;
        let mut s = Self {
            cfg: l.cfg,
            dir: l.dir,
            planet: l.planet,
            chunks,
            player: l.player,
            interaction: { let mut i = Interaction::default(); i.spill = true; i },
            drops: wolfcraft_engine::drops::Drops::new(),
            hit: None,
            item_name: None,
            near_table: false,
            autosave: 0.0,
            last_jump_press: -10.0,
            clock: 0.0,
            last_selected: usize::MAX,
            saved_notice: 0.0,
            camera_mode: CameraMode::First,
            rest_camera: None,
            cam_lag: 0.0,
            land_dip: 0.0,
            tech: std::collections::HashMap::new(),
            tech_nets: Vec::new(),
            kin: std::collections::HashMap::new(),
            kin_t: 0.0,
            kin_accum: 0.0,
            pipes: Default::default(),
            pipe_t: 0.0,
            tech_t: 0.0,
            cull_t: 0.0,
            fire_t: 0.0,
            weather: crate::weather::Weather::new(seed),
            stuck_check: 0.0,
            bob_phase: 0.0,
            bob_amp: 0.0,
            lean: 0.0,
            air_t: 0.0,
            prev_vy: 0.0,
            shoulder: 0.0,
            prev_radius: 0.0,
            eye_r: 0.0,
            tp_dist: 4.2,
            mobs: wolfcraft_engine::mobs::MobManager::new(seed),
            riding: None,
            ritual: None,
            star_queue: Vec::new(),
            star_cd: 0.0,
            stellar_ui: Default::default(),
            cutscene: None,
            cinema: 0.0,
            flash: None,
            skip_film: false,
            shake: 0.0,
            slowmo: 0.0,
            hammer_cd: 0.0,
            ship_hammer_mode: 0,
            boss_film: None,
            boss_seen: None,
            title: None,
            embers: Vec::new(),
            booms: Vec::new(),
            stellar_removed: Vec::new(),
            stellar: None,
            attack_cd: 0.0,
            death_burst_done: false,
            arrows: Default::default(),
            explosives: wolfcraft_engine::explosives::Explosives::new(seed),
            collapse: wolfcraft_engine::collapse::Collapse::new(seed),
            petsim: Default::default(),
            coin_scan: 0.0,
            coin_cells: Vec::new(),
            petsim_home: None,
            buttonsim: Default::default(),
            clicker: Default::default(),
            arena: None,
            arena_dead_seen: false,
            rest: None,
            bow_charge: 0.0,
            bow_arrow: None,
            ench_seed: 1,
            prev_use_held: false,
            containers: Default::default(),
            storage_bank: Default::default(),
            boss_book: Default::default(),
            block_commands: Default::default(),
            hell: false,
            nickname: "Игрок".to_string(),
            remote_edits: Vec::new(),
            dimension_changed: false,
            overworld_pos: None,
            planet_kind: wolfcraft_engine::planet::PlanetKind::Home,
            rocket: None,
            rocket_pick: None,
            portal_t: -2.0,
            pending_portal: None,
            furnace_t: Default::default(),
            furnace_fuel: Default::default(),
            spawn_checked: false,
            studio: false,
            minigames: false,
            buttons: Vec::new(),
            plates: Vec::new(),
            tick_acc: 0.0,
            we: Default::default(),
            net_out: Vec::new(),
            fluids: Default::default(),
            radiation: 0.0,
            mob_hits: 0,
            picked_up: 0,
            popups: Vec::new(),
            pickups: Vec::new(),
            achieved: std::collections::HashSet::new(),
            ach_toasts: Vec::new(),
            ach_new: 0,
            ach_t: 0.0,
            aim_dir: None,
            fx_amount: 1.0,
            fx_lights: Vec::new(),
            nukes: Vec::new(),
            fx_scan: 0.0,
            fx_dust: 0.0,
            fx_was_in_water: false,
            fx_prev_vup: 0.0,
            fx_was_ground: true,
            env_col: None,
        };
        // Studio baseplates stay empty: no creatures appear.
        s.mobs.spawning = s.cfg.world_type != wolfcraft_engine::config::WorldType::Baseplate;
        s.storage_bank=wolfcraft_engine::storage_bank::StorageBank::load(&s.dir.join("storage_bank.bin"));
        s.boss_book=wolfcraft_engine::bosses::BossBook::load(&s.dir.join("bosses.txt"));
        s.load_containers();
        s.load_tech();
        if let Ok(t) = std::fs::read_to_string(s.dir.join("kinetic.txt")) {
            for line in t.lines() {
                let n: Vec<i32> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                if n.len() == 4 {
                    s.kin.insert(wolfcraft_engine::cubesphere::Cell::new(n[0] as u8, n[1], n[2], n[3]), KinNode::default());
                }
            }
        }
        if let Ok(t) = std::fs::read_to_string(s.dir.join("pipes.txt")) {
            for line in t.lines() {
                let n: Vec<i32> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                if n.len() == 4 {
                    s.pipes.insert(wolfcraft_engine::cubesphere::Cell::new(n[0] as u8, n[1], n[2], n[3]));
                }
            }
        }
        if let Ok(t) = std::fs::read_to_string(s.dir.join("achievements.txt")) {
            s.achieved = t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        }
        s.petsim = wolfcraft_engine::petsim::PetSim::from_text(&std::fs::read_to_string(s.dir.join("petsim.txt")).unwrap_or_default());
        // Old saves kept the pets in petsim.txt: they move into the inventory once.
        for k in std::mem::take(&mut s.petsim.owned) {
            s.player.inventory.add(wolfcraft_engine::items::ItemStack::new(wolfcraft_engine::items::ids::PET_BASE + k as u16, 1));
        }
        s.buttonsim = wolfcraft_engine::buttonsim::ButtonSim::from_text(&std::fs::read_to_string(s.dir.join("buttonsim.txt")).unwrap_or_default());
        s.clicker = wolfcraft_engine::clicker::Clicker::from_text(&std::fs::read_to_string(s.dir.join("clicker.txt")).unwrap_or_default());
        s.arena = wolfcraft_engine::arenas::Arena::from_text(&std::fs::read_to_string(s.dir.join("arena.txt")).unwrap_or_default());
        s.petsim_home = std::fs::read_to_string(s.dir.join("petsim_home.txt")).ok().and_then(|t| {
            let v: Vec<f64> = t.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            (v.len() == 3).then(|| DVec3::new(v[0], v[1], v[2]))
        });
        s.load_block_commands();
        s.load_dimension();
        s.refresh_globe();
        s.player.align_heading();
        s.prev_radius = radial(s.player.body.pos);
        s
    }

    pub fn player_chunk(&self) -> ChunkKey {
        ChunkKey::of(self.chunks.grid().world_to_cell(self.player.body.pos))
    }

    /// True when the chunk under the player and its neighbours exist (safe to simulate).
    pub fn ground_ready(&self) -> bool {
        let c = self.player_chunk();
        let g = self.chunks.grid();
        let loaded = |k: Option<ChunkKey>| k.is_none_or(|k| self.chunks.is_loaded(k));
        (-1..=1).all(|d| loaded(g.chunk_neighbor(c, d, 0, 0)) && loaded(g.chunk_neighbor(c, 0, d, 0)) && loaded(g.chunk_neighbor(c, 0, 0, d)))
    }

    pub fn update(&mut self, input: &InputFrame, dt: f32, view: i32, frozen: bool) -> Vec<InteractionEvent> {
        // Slow motion right after a big strike.
        let dt = if self.slowmo > 0.0 {
            self.slowmo -= dt;
            dt * 0.35
        } else {
            dt
        };
        self.clock += dt;
        self.chunks.update(self.player_chunk(), view, 6);
        let mut events = Vec::new();
        let mut used_stack=None;
        if frozen {
            return events;
        }

        // Portal: stand inside it for a moment.
        let feet_cell = self.player.body.pos + self.player.up() * 0.6;
        if self.chunks.block_at_world(feet_cell) == wolfcraft_engine::block::Block::NetherPortal {
            self.portal_t += dt;
            if self.portal_t > 1.5 {
                self.switch_dimension();
                return events;
            }
        } else {
            self.portal_t = self.portal_t.min(0.0) + (dt).min(-self.portal_t.min(0.0));
            if self.portal_t > -0.0001 {
                self.portal_t = 0.0;
            }
        }
        if !self.remote_edits.is_empty() {
            let chunks = &mut self.chunks;
            let mut applied = Vec::new();
            self.remote_edits.retain(|(c, b)| {
                // Keep the edit until its chunk exists; an already equal block needs nothing.
                match chunks.get_block(*c) {
                    Some(cur) if cur == *b => false,
                    Some(_) => {
                        let done = chunks.set_block(*c, *b);
                        if done {
                            applied.push((*c, *b));
                        }
                        !done
                    }
                    None => true,
                }
            });
            // Only the edit itself comes over the network: charge the iskrit dust, light the
            // lamps and release pressed buttons here as well.
            for (c, b) in applied {
                self.fluids.changed(&self.chunks, c);
                wolfcraft_engine::power::update_around(&mut self.chunks, c);
                wolfcraft_engine::power::update_iskrit(&mut self.chunks, c);
                if b.unwall() == wolfcraft_engine::block::Block::ButtonOn && !self.buttons.iter().any(|(p, _)| *p == c) {
                    self.buttons.push((c, 1.0));
                }
            }
        }
        if let Some(c) = self.pending_portal {
            if self.ground_ready() {
                let up = Some(wolfcraft_engine::cubesphere::Cell::new(c.f, c.i, c.j, c.k + 1));
                let ok1 = self.chunks.set_block(c, wolfcraft_engine::block::Block::NetherPortal);
                let ok2 = up.is_some_and(|u| self.chunks.set_block(u, wolfcraft_engine::block::Block::NetherPortal));
                if ok1 && ok2 {
                    self.pending_portal = None;
                }
            }
        }

        // Look + hotbar.
        self.player.look(input.look.0, input.look.1);
        if input.swap_hands && !self.player.survival.dead {
            self.player.inventory.swap_offhand();
        }
        // The rocket: a tap with it in hand (and fuel in the bag) lifts off.
        if input.use_pressed && self.rocket.is_none() && !self.player.survival.dead && self.player.inventory.held().is_some_and(|h| h.id == wolfcraft_engine::items::ids::SPACE_ROCKET) {
            self.start_rocket();
        }
        // WolfTech mining laser: a tap burns a tunnel of up to 8 blocks ahead.
        if input.use_pressed && !self.player.survival.dead && self.player.inventory.held().is_some_and(|h| h.id == wolfcraft_engine::items::ids::MINING_LASER) {
            self.fire_laser();
        }
        // Shield up while RMB is held, unless the main hand has its own right-click use.
        let holding_bow = self.player.inventory.held().is_some_and(|s| s.id == wolfcraft_engine::items::ids::BOW);
        let main_use = holding_bow
            || self.player.inventory.held().is_some_and(|s| {
                let i = s.info();
                i.place.is_some() || i.food.is_some() || wolfcraft_engine::explosives::weapon(s.id).is_some() || s.id == wolfcraft_engine::items::ids::LIGHTER
            });
        // Bow: hold to draw (one second for full power), release to shoot.
        // The arrow the bow will shoot: the first arrow in the bag (tipped ones included).
        let arrow_id = self.next_arrow();
        self.bow_arrow = arrow_id;
        let has_arrows = self.player.is_creative() || arrow_id.is_some();
        if holding_bow && input.use_held && has_arrows && !self.player.survival.dead {
            self.bow_charge = (self.bow_charge + dt).min(1.0);
        } else {
            if holding_bow && self.prev_use_held && !input.use_held && self.bow_charge > 0.12 && has_arrows {
                let power = self.bow_charge;
                let creative = self.player.is_creative();
                let id = arrow_id.unwrap_or(wolfcraft_engine::items::ids::ARROW);
                let power_ench = self.player.inventory.held().map(|s| s.ench_level(wolfcraft_engine::magic::Ench::Power)).unwrap_or(0);
                if !creative {
                    self.player.inventory.take(id, 1);
                    self.player.inventory.wear_selected();
                }
                let effect = tipped_effect(id);
                self.arrows.shoot_with(self.player.eye_pos(), self.player.look_dir(), power, !creative && effect.is_none(), power_ench, effect);
            }
            self.bow_charge = 0.0;
        }
        self.prev_use_held = input.use_held;
        self.player.blocking = input.use_held && self.player.inventory.has_shield() && !main_use && !self.player.survival.dead;
        if let Some(i) = input.hotbar_set {
            self.player.inventory.selected = i.min(8);
        }
        if input.hotbar_delta != 0 {
            let s = self.player.inventory.selected as i32 - input.hotbar_delta.signum();
            self.player.inventory.selected = s.rem_euclid(9) as usize;
        }
        if self.last_selected != self.player.inventory.selected {
            self.last_selected = self.player.inventory.selected;
            self.item_name = self.player.inventory.held().map(|s| (s.display_name(), 2.0));
        }
        if let Some((_, t)) = &mut self.item_name {
            *t -= dt;
            if *t <= 0.0 {
                self.item_name = None;
            }
        }

        // Double-tap jump toggles flight in creative.
        if input.jump_pressed {
            if self.clock - self.last_jump_press < 0.3 {
                self.player.toggle_flight();
                self.last_jump_press = -10.0;
            } else {
                self.last_jump_press = self.clock;
            }
        }

        // Never start inside blocks: climb out to the first spot with room for the body.
        if !self.spawn_checked && self.ground_ready() {
            self.spawn_checked = true;
            let mut p = self.player.body.pos;
            for _ in 0..256 {
                let up = wolfcraft_engine::cubesphere::up(p);
                let blocked = |h: f64| self.chunks.block_at_world(p + up * h).is_solid();
                if !blocked(0.5) && !blocked(1.5) {
                    break;
                }
                p += up;
            }
            if p != self.player.body.pos {
                log::warn!("start position was inside blocks: moved up {:.0} blocks", (p - self.player.body.pos).length());
                self.player.body.pos = p;
                self.player.body.vel = DVec3::ZERO;
            }
        }

        // Riding a dragon: the stick steers it, «Прыжок» climbs, «Присесть» dives.
        self.tick_ride(input, dt);
        // Resting: asleep (the night passes in a few seconds) or sitting until you move.
        let resting = self.tick_rest(input, dt);
        // Physics, split into steps of at most 1/30 s.
        if self.rocket.is_some() {
            self.tick_rocket(dt);
        } else if self.ground_ready() && !self.player.survival.dead && !resting && self.riding.is_none() {
            let controls = Controls { forward: input.forward, strafe: input.strafe, jump: input.jump, sprint: input.sprint, sneak: input.sneak };
            let mut left = dt.min(0.25) as f64;
            while left > 1e-6 {
                let step = left.min(1.0 / 30.0);
                self.player.update(&self.chunks, &controls, self.cfg.difficulty, step);
                left -= step;
            }
        }
        // Server places (baseplate): a flat world with a sky ceiling — no flying off into space.
        if self.cfg.world_type == wolfcraft_engine::config::WorldType::Baseplate && !self.hell {
            let top = self.planet.radius + BASEPLATE_CEILING;
            if radial(self.player.body.pos) > top {
                self.player.body.pos = along(wolfcraft_engine::cubesphere::col_dir(self.player.body.pos), top);
                let up = self.player.up();
                let v = self.player.body.vel.dot(up);
                if v > 0.0 {
                    self.player.body.vel -= up * v;
                }
            }
        }

        // Smooth automatic steps: the camera lags behind sudden upward jumps of the feet.
        let r = radial(self.player.body.pos);
        let dr = r - self.prev_radius;
        self.prev_radius = r;
        if dr > 0.3 && dr < 2.0 && self.player.body.on_ground {
            self.cam_lag -= dr;
        }
        self.cam_lag = (self.cam_lag * (-16.0 * dt as f64).exp()).clamp(-1.6, 0.6);
        // A hard landing: the view sinks a little and springs back (like Minecraft/Halo).
        {
            let up = self.player.up();
            let vy = self.player.body.vel.dot(up);
            if self.player.body.on_ground && self.prev_vy < -7.0 && !self.player.body.flying {
                self.land_dip = ((-self.prev_vy - 7.0) * 0.035).clamp(0.05, 0.45);
            }
            self.prev_vy = if self.player.body.on_ground { 0.0 } else { vy };
            self.land_dip *= (-7.0 * dt as f64).exp();
        }
        // Buried in blocks (a broken save, a block placed on top, a big edit around)? Climb out,
        // like the game "pushing out of blocks" in Minecraft. Only once the area is loaded.
        self.stuck_check -= dt;
        if self.stuck_check <= 0.0 && self.riding.is_none() {
            self.stuck_check = 0.5;
            let feet = self.player.body.pos;
            let cell = self.chunks.grid().world_to_cell(feet);
            if self.chunks.get_block(cell).is_some() {
                if let Some(free) = wolfcraft_engine::physics::free_spot_above(&self.chunks, feet, 64) {
                    if free != feet {
                        log::warn!("player was stuck in blocks at {feet:?}: lifted by {:.1}", (free - feet).length());
                        self.player.body.pos = free;
                        self.player.body.vel = DVec3::ZERO;
                    }
                }
            }
        }
        // View bobbing that does not twitch: "on the ground" flickers on the cube planet, so
        // a short time in the air still counts as walking, and the strength eases in and out.
        {
            let up = self.player.up();
            let v = self.player.body.vel;
            let speed = (v - up * v.dot(up)).length();
            self.air_t = if self.player.body.on_ground { 0.0 } else { self.air_t + dt as f64 };
            let walking = self.air_t < 0.25 && !self.player.body.flying && self.riding.is_none() && self.cutscene.is_none();
            let target = if walking { (speed / 4.3).min(1.2) } else { 0.0 };
            self.bob_amp += (target - self.bob_amp) * (1.0 - (-6.0 * dt as f64).exp());
            self.bob_phase += speed.min(8.0) * dt as f64 * 1.9;
            // Lean into a sideways run a little (like a body turning into the curve).
            let side = if walking { v.dot(self.player.right()) / 5.0 } else { 0.0 };
            let want = (-side * 0.035).clamp(-0.05, 0.05);
            self.lean += (want - self.lean) * (1.0 - (-5.0 * dt as f64).exp());
        }
        // Third person behind: over the right shoulder (Fortnite/GTA), closer to the middle
        // when a wall is on the right.
        {
            let want = if self.camera_mode == CameraMode::ThirdBack {
                let eye = self.player.eye_pos();
                let right = self.player.right();
                let mut free = 0.0;
                while free < 0.8 && !self.chunks.block_at_world(eye + right * (free + 0.35)).is_solid() {
                    free += 0.05;
                }
                free.min(0.75)
            } else {
                0.0
            };
            self.shoulder += (want - self.shoulder) * (1.0 - (-8.0 * dt as f64).exp());
        }
        // Eye height: follow small changes smoothly, jump on big ones (teleport, falling).
        let er = radial(self.player.eye_pos());
        if (er - self.eye_r).abs() > 0.35 || !self.player.body.on_ground {
            self.eye_r = er;
        } else {
            self.eye_r += (er - self.eye_r) * (1.0 - (-30.0 * dt as f64).exp());
        }
        // Third person: back away smoothly, come closer at once (never inside a wall).
        let want = self.third_person_room();
        self.tp_dist = if want < self.tp_dist { want } else { self.tp_dist + (want - self.tp_dist) * (1.0 - (-6.0 * dt as f64).exp()) };

        // Targeting and interaction.
        let eye = self.player.eye_pos();
        let reach = Interaction::reach(&self.player);
        // Phones pick what is under the finger; with no finger on the world nothing is picked.
        self.aim_dir = if input.touch { input.aim } else { Some(self.player.look_dir()) };
        self.hit = self.aim_dir.and_then(|d| raycast(&self.chunks, eye, d, reach));
        if !self.player.survival.dead {
            let hit = self.hit;
            // A creature in front of the block takes the blow: holding to hit it never digs the
            // block behind it.
            let creature_first = self.aim_dir.is_some_and(|d| {
                let reach = Interaction::reach(&self.player) - 1.5;
                self.mobs.raycast(eye, d, reach).is_some_and(|(_, t)| self.hit.is_none_or(|h| t < h.dist + 0.5))
            });
            // Firearms and launchers never dig: the screen is for aiming.
            let gun = self.player.inventory.held().and_then(|s| wolfcraft_engine::explosives::weapon(s.id).map(|_| s.id)).filter(|id| *id != wolfcraft_engine::items::ids::GRENADE);
            events.extend(self.interaction.update_break(&mut self.chunks, &mut self.player, hit, input.break_held && !creature_first && gun.is_none(), dt, self.cfg.seed));
            // Automatic weapons fire while the trigger (or the finger) is held.
            if let Some(id) = gun.filter(|id| wolfcraft_engine::explosives::firearm(*id).is_some_and(|f| f.auto)) {
                if (input.use_held || input.break_held) && !input.use_pressed && self.explosives.ready() {
                    events.push(InteractionEvent::Fire { weapon: id });
                }
            }
            // A tap on a dragon: put a saddle on it, climb on or get off.
            let mounted = input.use_pressed && self.try_mount();
            // A tap on a creature in the crosshair hits it (touch screens: a short tap is "use").
            if input.use_pressed && !mounted && !self.try_attack(0.25) {
                used_stack=self.player.inventory.held();
                events.extend(self.interaction.use_item(&mut self.chunks, &mut self.player, hit));
            }
            if input.pick_pressed {
                self.interaction.pick_block(&mut self.player, hit);
            }
        }
        // Creatures: AI, melee attacks by the player, damage to the player.
        if self.ground_ready() && !self.player.survival.dead {
            let up = self.player.up();
            let day = local_sun(self.player.world_time, up).dot(up) > 0.05;
            let peaceful = self.cfg.difficulty == wolfcraft_engine::config::Difficulty::Peaceful;
            let dealt = self.mobs.update(&self.chunks, self.player.body.pos, day, peaceful, !self.player.is_creative(), dt.min(0.1) as f64);
            // What the guardians' blows do besides damage.
            for (e, secs) in std::mem::take(&mut self.mobs.boss_effects) {
                if !self.player.is_creative() && e != wolfcraft_engine::magic::Effect::Harm {
                    self.player.apply_effect(e, 1, secs);
                }
            }
            let knock = std::mem::replace(&mut self.mobs.boss_knock, DVec3::ZERO);
            if knock.length_squared() > 0.0 && !self.player.body.flying {
                self.player.body.vel += knock;
                self.player.body.on_ground = false;
            }
            if dealt > 0.0 {
                // A guardian's blow lands: red flash and a jolt.
                if self.mobs.mobs.iter().any(|m| m.boss.is_some()) {
                    self.flash = Some(([1.0, 0.15, 0.1], 0.35));
                    self.shake = self.shake.max(0.6);
                }
                let armor = self.player.armor_points();
                // «Защита» on worn pieces: another 3 % per level.
                let prot: u32 = self.player.inventory.armor.iter().flatten().map(|s| s.ench_level(wolfcraft_engine::magic::Ench::Protection) as u32).sum();
                let reduced = dealt * (1.0 - (armor as f32 * 0.04 + prot as f32 * 0.03).min(0.85));
                self.player.damage(reduced, self.cfg.difficulty);
            }
            if self.player.survival.dead && !self.death_burst_done {
                self.death_burst_done = true;
                let at = self.player.body.pos + wolfcraft_engine::physics::up_at(self.player.body.pos) * 0.5;
                self.leave_bones(at);
                self.mobs.burst(self.player.body.pos, &wolfcraft_engine::renderer::model::player_palette(), 1.8);
            } else if !self.player.survival.dead {
                self.death_burst_done = false;
            }
            self.attack_cd = (self.attack_cd - dt).max(0.0);
            if input.break_held {
                self.try_attack(0.45);
            }
        }
        // Sparkles round fresh ore loot.
        for i in 0..self.drops.list.len() {
            let d = &mut self.drops.list[i];
            if d.shine <= 0.0 {
                continue;
            }
            d.shine -= dt;
            let (pos, k) = (d.pos, d.shine);
            if self.mobs.rand() < (dt as f64 * 14.0) {
                let up = wolfcraft_engine::physics::up_at(pos);
                let o = DVec3::new(self.mobs.rand() - 0.5, self.mobs.rand() - 0.5, self.mobs.rand() - 0.5) * 0.35;
                let col = if self.mobs.rand() < 0.5 { [1.0, 1.0, 0.85] } else { [1.0, 0.92, 0.55] };
                self.mobs.fx_glow(pos + up * 0.2 + o, up * 0.6, col, 0.028 + 0.01 * k.min(1.0), 0.6, wolfcraft_engine::mobs::FxKind::Rise);
            }
        }
        // Pop-ups age and float away.
        for p in &mut self.popups {
            p.age += dt;
        }
        self.popups.retain(|p| p.age < 1.1);
        self.popups.truncate(24);
        for p in &mut self.pickups {
            p.2 += dt;
        }
        self.pickups.retain(|p| p.2 < 3.0);
        // Items in the world: fall, merge, fly to the player.
        if self.ground_ready() && !self.drops.list.is_empty() {
            let centre = self.player.body.pos + wolfcraft_engine::physics::up_at(self.player.body.pos) * 0.8;
            let can = !self.player.survival.dead;
            self.drops.magnet_bonus = self.player.inventory.held().map(|s| s.ench_level(wolfcraft_engine::magic::Ench::Magnet) as f64 * 2.5).unwrap_or(0.0);
            let inv = &mut self.player.inventory;
            let mut got: Vec<(wolfcraft_engine::items::ItemId, u32)> = Vec::new();
            let n = self.drops.step_log(&self.chunks, centre, can, dt as f64, |s| inv.add(s), |id, k| match got.iter_mut().find(|g| g.0 == id) {
                Some(g) => g.1 += k,
                None => got.push((id, k)),
            });
            self.picked_up += n;
            // The pickup feed: each kind of item gets its line, new ones pop in, the same one
            // counts up and pops again.
            for (id, k) in got {
                match self.pickups.iter_mut().find(|p| p.0 == id) {
                    Some(p) => {
                        p.1 += k;
                        p.2 = 0.0;
                    }
                    None => {
                        self.pickups.insert(0, (id, k, 0.0));
                        self.pickups.truncate(5);
                    }
                }
            }
        }
        // Arrows: flight, hits, pickup.
        if self.ground_ready() {
            let feet = self.player.body.pos;
            let can_pick = !self.player.survival.dead;
            let ev = self.arrows.step(&self.chunks, &mut self.mobs, feet, can_pick, dt.min(0.1) as f64);
            for (kind, at) in ev.killed {
                if kind != wolfcraft_engine::mobs::MobKind::Slime {
                    self.leave_bones(at);
                }
                // The loot lands where the creature fell.
                for drop in wolfcraft_engine::items::mob_drops(kind, self.cfg.seed ^ self.clock.to_bits() as u64) {
                    self.drops.pop_from_block(drop, at);
                }
                self.player.survival.add_xp(if kind.hostile() { 5 } else { 1 });
            }
            // Tipped arrows leave a trail of their colour; potion effects swirl round the player.
            let trails: Vec<(DVec3, [f32; 3])> = self.arrows.list.iter().filter(|a| !a.stuck).filter_map(|a| a.effect.map(|e| (a.pos, e.color()))).collect();
            for (p, c) in trails {
                if self.mobs.rand() < (dt as f64 * 30.0) {
                    self.mobs.fx_glow(p, DVec3::ZERO, c, 0.03, 0.5, wolfcraft_engine::mobs::FxKind::Rise);
                }
            }
            let colors: Vec<[f32; 3]> = self.player.effects.iter().map(|a| a.effect.color()).collect();
            for c in colors {
                if self.mobs.rand() < (dt as f64 * 4.0) {
                    let up = wolfcraft_engine::physics::up_at(feet);
                    let a = self.mobs.rand() * std::f64::consts::TAU;
                    let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
                    let t1 = up.cross(helper).normalize();
                    let t2 = up.cross(t1);
                    let p = feet + (t1 * a.cos() + t2 * a.sin()) * 0.45 + up * (0.2 + self.mobs.rand() * 1.0);
                    self.mobs.fx_glow(p, up * 0.6, c, 0.035, 0.9, wolfcraft_engine::mobs::FxKind::Rise);
                }
            }
            // Tipped arrows burst into the colour of their potion where they hit.
            for (at, e) in &ev.effect_hits {
                let c = e.color();
                for k in 0..12 {
                    let a = k as f64 * 0.52;
                    let up = wolfcraft_engine::physics::up_at(*at);
                    let side = DVec3::new(a.cos(), (a * 1.7).sin(), a.sin());
                    self.mobs.fx_glow(*at, (side - up * side.dot(up)) * 1.6 + up * 1.2, c, 0.05, 0.8, wolfcraft_engine::mobs::FxKind::Rise);
                }
            }
            if ev.picked > 0 {
                self.player.inventory.add(wolfcraft_engine::items::ItemStack::new(wolfcraft_engine::items::ids::ARROW, ev.picked as u8));
            }
        }
        // Explosives: light dynamite, fire weapons, then fuses, flight and blasts.
        self.explosives_step(&mut events, dt);
        // Destruction physics: what lost its support falls, thrown blocks come down.
        self.collapse_step(&mut events, dt);
        // «Питомник»: eggs, pets, coin blocks.
        if self.minigames || self.studio {
            self.petsim_step(&mut events, dt);
        } else {
            events.retain(|e| !matches!(e, InteractionEvent::BuyEgg { .. } | InteractionEvent::Hatch { .. }));
        }
        // «Кнопки»: income, and buying by standing on a floor button.
        if self.minigames || self.studio {
            self.buttonsim.tick(dt);
            self.clicker.tick(dt);
        }
        // WolfTech battery: one in the inventory tops up the electric tool in the hand.
        {
            use wolfcraft_engine::items::{ids, rechargeable};
            let inv = &mut self.player.inventory;
            let sel = inv.selected;
            let need = inv.slots[sel].filter(|h| h.id != ids::BATTERY && rechargeable(h.id)).map(|h| h.damage).unwrap_or(0);
            if need > 0 {
                let mut want = need.min((30.0 * dt).ceil() as u16);
                for i in 0..inv.slots.len() {
                    if want == 0 || i == sel {
                        continue;
                    }
                    if let Some(b) = inv.slots[i].as_mut().filter(|b| b.id == ids::BATTERY) {
                        let left = b.info().durability.saturating_sub(b.damage);
                        let n = left.min(want);
                        b.damage += n;
                        want -= n;
                        if let Some(h) = inv.slots[sel].as_mut() {
                            h.damage -= n;
                        }
                    }
                }
            }
        }
        // The clicker kit: a double jump.
        self.player.double_jump = self.clicker.active && (self.minigames || self.studio);
        self.tick_arena(dt);
        if (self.minigames || self.studio) && !self.player.survival.dead && self.player.body.on_ground {
            let under = self.chunks.block_at_world(self.player.body.pos - self.player.up() * 0.3);
            if let Some(pad) = wolfcraft_engine::buttonsim::pad(under) {
                match self.buttonsim.stand_on(under) {
                    Some(msg) => {
                        self.item_name = Some((msg, 1.5));
                        self.picked_up += 1;
                    }
                    None => {
                        if self.item_name.as_ref().is_none_or(|(_, t)| *t < 0.4) {
                            self.item_name = Some((wolfcraft_engine::buttonsim::describe(pad), 0.6));
                        }
                    }
                }
            }
        }
        // Spawn souls bring creatures to life (standing on the bottom of the cell).
        for e in &events {
            if let InteractionEvent::SpawnMob { pos, kind } = e {
                let g = self.chunks.grid();
                let c = g.cell_center(*pos);
                let feet = c - wolfcraft_engine::physics::up_at(c) * 0.48;
                let up = wolfcraft_engine::physics::up_at(feet);
                let to = self.player.body.pos - feet;
                let heading = (to - up * to.dot(up)).normalize_or_zero();
                let heading = if heading.length_squared() > 0.5 { heading } else { self.player.heading };
                self.mobs.mobs.push(wolfcraft_engine::mobs::Mob::new(*kind, feet, heading));
            }
        }
        // Broken blocks drop their items into the world; broken chests spill their contents.
        self.handle_building_events(&mut events, used_stack);
        // Boss kills pay once per altar, even when delivered by a projectile.
        for death in std::mem::take(&mut self.mobs.boss_deaths) {
            // The guardian falls: slow motion, a burst of its colour, «ПОВЕРЖЕН».
            {
                let c = death.kind.color();
                self.epic_impact(death.at, 8.0, c, 0.8);
                self.slowmo = 1.4;
                self.skip_film = false;
                self.boss_film = Some(BossFilm { at: death.at, t: 0.0, dur: 4.0 });
                self.title = Some((format!("{} ПОВЕРЖЕН", death.kind.name().to_uppercase()), "Победа!".into(), 0.0));
                self.boss_seen = None;
            }
            self.grant("boss");
            if self.boss_book.finish(death.kind,death.home) {
                for reward in death.kind.rewards() {if let Some(rest)=self.player.inventory.add(reward){self.drops.pop_shiny(rest,death.at);}}
                self.player.survival.add_xp(100);
                self.item_name=Some((format!("{} побеждён — редкая награда получена",death.kind.name()),4.0));
                let _=self.boss_book.save(&self.dir.join("bosses.txt"));
            }
        }
        self.tick_ritual(dt);
        self.tick_boss_film(dt);
        for mob in &self.mobs.mobs {if let Some(role)=&mob.boss {if let Some(record)=self.boss_book.records.get_mut(&(role.kind,role.home)){if !record.defeated{record.hp=mob.hp.max(1.0);}}}}
        // Ore loot jumps out higher and sparkles.
        let ores: Vec<_> = events.iter().filter_map(|e| match e {
            InteractionEvent::Broken { pos, block } if wolfcraft_engine::items::ore_glow(*block).is_some() => Some(*pos),
            _ => None,
        }).collect();
        for e in &events {
            if let InteractionEvent::Dropped { pos, stack } = e {
                let c = self.chunks.grid().cell_center(*pos);
                if ores.contains(pos) {
                    self.drops.pop_shiny(*stack, c);
                } else {
                    self.drops.pop_from_block(*stack, c);
                }
            }
            if let InteractionEvent::Broken { pos, .. } = e {
                if let Some(slots) = self.containers.remove(pos) {
                    let c = self.chunks.grid().cell_center(*pos);
                    for s in slots.into_iter().flatten() {
                        self.drops.pop_from_block(s, c);
                    }
                }
                self.furnace_t.remove(pos);
            }
        }
        self.tick_furnaces(dt);
        self.tick_tech(dt);
        self.tick_achievements(&events, dt);
        self.tick_kinetic(&events, dt);
        self.tick_pipes(&events, dt);
        // Broken blocks fall apart: textured pieces of the block fly out and tumble down
        // (a wide drill dig shares one budget so a 13×13 square stays smooth).
        if self.fx_amount > 0.0 {
            let broken: Vec<(wolfcraft_engine::cubesphere::Cell, wolfcraft_engine::block::Block)> = events.iter().filter_map(|e| if let InteractionEvent::Broken { pos, block } = e { Some((*pos, *block)) } else { None }).collect();
            let per = if broken.len() <= 1 { 8.0 } else if broken.len() <= 9 { 6.0 } else { (96.0 / broken.len() as f32).max(1.0) };
            let n = ((per * self.fx_amount).round() as usize).max(1);
            for (pos, block) in broken {
                let info = block.info();
                if block == wolfcraft_engine::block::Block::Air || block.is_fluid() || info.render == wolfcraft_engine::block::RenderKind::Invisible {
                    continue;
                }
                let tile = info.tiles[0];
                if tile == 0 {
                    continue;
                }
                let c = self.chunks.grid().cell_center(pos);
                // Plants and torches: a few small bits; blocks: eighths of the cube.
                let small = info.render == wolfcraft_engine::block::RenderKind::Cross || !block.boxes().is_empty();
                if small {
                    self.mobs.shards(c - wolfcraft_engine::physics::up_at(c) * 0.2, tile, n.min(4), 0.14);
                } else {
                    self.mobs.shards(c, tile, n, 0.34);
                }
            }
        }
        // Redstone-style power: re-evaluate the network around every edit.
        for e in &events {
            if let InteractionEvent::Broken { pos, block } = e {
                wolfcraft_engine::power::piston_broken(&mut self.chunks, *pos, *block);
            }
            if let InteractionEvent::Placed { pos, .. } | InteractionEvent::Broken { pos, .. } | InteractionEvent::Blasted { pos, .. } = e {
                self.fluids.changed(&self.chunks, *pos);
                wolfcraft_engine::power::update_around(&mut self.chunks, *pos);
                wolfcraft_engine::power::update_iskrit(&mut self.chunks, *pos);
            }
            if let InteractionEvent::OpenContainer { pos, block } = e {
                if wolfcraft_engine::wolftech::kind(*block).is_some() && !self.tech.contains_key(pos) {
                    self.tech.insert(*pos, TechState::default());
                    self.tech_t = 0.0;
                }
            }
            if let InteractionEvent::Placed { pos, block } = e {
                // A little puff of dust where a block lands.
                if !block.is_fluid() && self.fx_amount > 0.0 {
                    let c = self.chunks.grid().cell_center(*pos);
                    self.mobs.chips(c, &[[0.92, 0.90, 0.85], [0.80, 0.78, 0.72]], (6.0 * self.fx_amount) as usize, 1.2);
                }
                if wolfcraft_engine::wolftech::kind(*block).is_some() {
                    self.tech.entry(*pos).or_default();
                    self.tech_t = 0.0;
                }
                if *block == wolfcraft_engine::block::Block::Cable {
                    self.tech_t = 0.0;
                }
            }
            if let InteractionEvent::Placed { pos, block } = e
                && block.unwall() == wolfcraft_engine::block::Block::ButtonOn
            {
                self.buttons.retain(|(p, _)| p != pos);
                self.buttons.push((*pos, 1.0));
            }
        }
        self.special_effects(dt);
        self.weather_tick(dt);
        // Entity culling 5 times a second: creatures behind walls are not drawn.
        self.cull_t -= dt;
        if self.cull_t <= 0.0 {
            self.cull_t = 0.2;
            let eye = self.camera().eye;
            self.mobs.cull_hidden(&self.chunks, eye);
        }
        self.fire_tick(dt);
        // Radiation: uranium carried or within two blocks hurts slowly. A full set of armour
        // keeps it out.
        if !self.player.survival.dead && !self.player.is_creative() {
            use wolfcraft_engine::items::ids;
            let inv = &self.player.inventory;
            let carried = inv.slots.iter().flatten().any(|s| matches!(s.id, ids::RAW_URANIUM | ids::URANIUM_INGOT) || s.id == wolfcraft_engine::block::Block::UraniumBlock as u16);
            let feet = self.chunks.grid().world_to_cell(self.player.body.pos);
            let mut near = false;
            'scan: for dk in -1..=2 {
                for dj in -2..=2 {
                    for di in -2..=2 {
                        if let Some(c) = self.chunks.grid().offset(feet, di, dj, dk) {
                            if self.chunks.get_block(c).is_some_and(|b| b.is_radioactive()) {
                                near = true;
                                break 'scan;
                            }
                        }
                    }
                }
            }
            let shielded = self.player.inventory.armor.iter().all(|a| a.is_some());
            if (carried || near) && !shielded {
                self.radiation += dt;
                if self.radiation >= 3.0 {
                    self.radiation = 0.0;
                    self.player.damage(1.0, self.cfg.difficulty);
                }
            } else {
                self.radiation = (self.radiation - dt).max(0.0);
            }
        }
        // Water and lava flow; the current carries the player along.
        if self.fluids.busy() {
            self.fluids.update(&mut self.chunks, dt);
        }
        // A block that appeared inside the player (water turning lava to obsidian around
        // them, a piston pushing a block in): lift them out instead of letting the physics
        // push them down through the floor.
        self.unstuck();
        if !self.player.body.flying {
            let push = wolfcraft_engine::fluids::Fluids::current_at(&self.chunks, self.player.body.pos + self.player.up() * 0.3);
            self.player.body.vel += push * dt as f64;
        }
        // Pressed buttons spring back after a second.
        let mut released = Vec::new();
        self.buttons.retain_mut(|(pos, t)| {
            *t -= dt;
            if *t <= 0.0 {
                released.push(*pos);
                false
            } else {
                true
            }
        });
        self.step_plates(dt);
        self.random_ticks(dt);
        // Plants crushed by pistons drop as items (a cane column falls apart above the cut).
        for (c, b) in wolfcraft_engine::power::take_crushed() {
            let mut cells = vec![(c, b)];
            let mut above = self.chunks.grid().neighbor(c, 2, 1);
            while let Some(a) = above {
                if !matches!(self.chunks.get_block(a), Some(wolfcraft_engine::block::Block::SugarCane)) || b != wolfcraft_engine::block::Block::SugarCane {
                    break;
                }
                self.chunks.set_block(a, wolfcraft_engine::block::Block::Air);
                cells.push((a, wolfcraft_engine::block::Block::SugarCane));
                above = self.chunks.grid().neighbor(a, 2, 1);
            }
            for (cell, blk) in cells {
                let centre = self.chunks.grid().cell_center(cell);
                for d in wolfcraft_engine::items::drops(blk, true, glam::IVec3::new(cell.i, cell.j, cell.k), self.cfg.seed) {
                    self.drops.pop_from_block(d, centre);
                }
            }
        }
        for pos in released {
            if let Some(b) = self.chunks.get_block(pos)
                && b.unwall() == wolfcraft_engine::block::Block::ButtonOn
            {
                let up = b.same_mount(wolfcraft_engine::block::Block::Button);
                self.chunks.set_block(pos, up);
                wolfcraft_engine::power::update_iskrit(&mut self.chunks, pos);
                self.net_out.push((pos, up));
            }
        }
        self.near_table = false;
        for e in &events {
            if matches!(e, InteractionEvent::OpenCrafting) {
                self.near_table = true;
            }
        }

        // Autosave.
        self.autosave += dt;
        self.saved_notice = (self.saved_notice - dt).max(0.0);
        if self.autosave > 90.0 {
            self.autosave = 0.0;
            self.save();
        }
        events
    }

    /// Lighting, firing, fuses, projectiles and blast damage. Request events (`Ignite`, `Fire`)
    /// stay in `events` only if they actually happened; blasts add `Exploded` and `Blasted`.
    fn collapse_step(&mut self, events: &mut Vec<InteractionEvent>, dt: f32) {
        use wolfcraft_engine::block::Block;
        use wolfcraft_engine::collapse::{Collapse, Landing};
        use wolfcraft_engine::interaction::WorldEdit;
        use wolfcraft_engine::physics::Voxels;
        // Holes made this frame: broken blocks and blast craters.
        let mut holes = Vec::new();
        let mut blast = None;
        let mut thrown = Vec::new();
        for e in events.iter() {
            match *e {
                // Builders in creative and the studio make floating things on purpose: only
                // blasts bring those down. In survival a pickaxe can too.
                InteractionEvent::Broken { pos, .. } if !self.studio && !self.player.is_creative() => holes.push(pos),
                InteractionEvent::Blasted { pos, block } => {
                    holes.push(pos);
                    thrown.push((pos, block));
                }
                InteractionEvent::Exploded { at, power } => blast = Some((at, power)),
                _ => {}
            }
        }
        if !holes.is_empty() {
            let grid = self.chunks.grid();
            // Some blocks of a crater fly out as real blocks (they shatter where they land).
            if let Some((at, power)) = blast {
                let every = (thrown.len() / 30).max(3);
                for (i, (c, b)) in thrown.iter().enumerate() {
                    if i % every == 0 {
                        self.collapse.throw(*b, grid.cell_center(*c), at, power);
                    }
                }
            }
            let chunks = &self.chunks;
            let planet = &self.planet;
            // Untouched terrain holds whatever stands on it.
            let groups = Collapse::loose_groups(&grid, &holes, |c| chunks.get_block(c), |c, b| planet.block_at(c) == b);
            for g in groups {
                let blocks: Vec<(wolfcraft_engine::cubesphere::Cell, Block)> = g.iter().filter_map(|c| self.chunks.get_block(*c).map(|b| (*c, b))).collect();
                let mut removed = Vec::new();
                self.collapse.drop_group(
                    &grid,
                    &blocks,
                    |c| {
                        let ok = self.chunks.set_block(c, Block::Air);
                        if ok {
                            removed.push(c);
                        }
                        ok
                    },
                    blast.map(|b| b.0),
                );
                // Containers in a falling building spill their contents.
                for c in &removed {
                    if let Some(slots) = self.containers.remove(c) {
                        let centre = grid.cell_center(*c);
                        for st in slots.into_iter().flatten() {
                            self.drops.pop_from_block(st, centre);
                        }
                    }
                    events.push(InteractionEvent::WorldSet { pos: *c, block: Block::Air });
                }
            }
        }
        if self.collapse.falling.is_empty() {
            return;
        }
        // A falling block that hits the player or a creature hurts them and breaks.
        let up = self.player.up();
        let me = self.player.body.pos + up * 0.9;
        let mut hits = Vec::new();
        for (i, f) in self.collapse.falling.iter().enumerate() {
            let fall = -f.vel.dot(wolfcraft_engine::physics::up_at(f.pos));
            if fall < 6.0 {
                continue;
            }
            if !self.player.survival.dead && (f.pos - me).length_squared() < 0.9 * 0.9 {
                hits.push((i, None, fall));
            } else if let Some(m) = self.mobs.mobs.iter().position(|m| m.hp > 0.0 && (m.centre() - f.pos).length_squared() < 0.9 * 0.9) {
                hits.push((i, Some(m), fall));
            }
        }
        for &(i, mob, fall) in hits.iter().rev() {
            let f = self.collapse.falling.swap_remove(i);
            let dmg = (fall as f32 * 0.35).min(14.0);
            match mob {
                None if !self.player.is_creative() => self.player.damage(dmg, self.cfg.difficulty),
                Some(m) => {
                    let _ = self.mobs.hurt(m, dmg, f.pos + up * 2.0);
                }
                None => {}
            }
            events.push(InteractionEvent::Shattered { at: f.pos, block: f.block });
        }
        let grid = self.chunks.grid();
        let chunks = &self.chunks;
        let landings = self.collapse.update(&grid, dt, |p| chunks.block_at_world(p).is_solid(), |c| chunks.get_block(c).is_some_and(|b| b == Block::Air || b.is_fluid() || b.info().render == wolfcraft_engine::block::RenderKind::Cross));
        for l in landings {
            match l {
                Landing::Placed { cell, block } => {
                    if self.chunks.set_block(cell, block) {
                        events.push(InteractionEvent::WorldSet { pos: cell, block });
                        events.push(InteractionEvent::Landed { pos: cell, block });
                    }
                }
                Landing::Shattered { pos, block, .. } => events.push(InteractionEvent::Shattered { at: pos, block }),
            }
        }
    }

    fn explosives_step(&mut self, events: &mut Vec<InteractionEvent>, dt: f32) {
        use wolfcraft_engine::explosives::{DYNAMITE_FUSE, blast_damage, weapon};
        let creative = self.player.is_creative();
        let eye = self.player.eye_pos();
        // Weapons shoot at the centre of the screen (the weapon sight), on phones too: a tap
        // anywhere fires.
        let look = self.player.look_dir();
        let mut keep = Vec::with_capacity(events.len());
        for e in events.drain(..) {
            match e {
                InteractionEvent::Ignite { pos } => {
                    if self.explosives.light(&mut self.chunks, pos, DYNAMITE_FUSE) {
                        keep.push(e);
                    }
                }
                InteractionEvent::Fire { weapon: id } if id == wolfcraft_engine::items::ids::SHIP_HAMMER => {
                    self.use_ship_hammer(eye, look);
                }
                InteractionEvent::Fire { weapon: id } if id == wolfcraft_engine::items::ids::STAR_HAMMER => {
                    self.hammer_slam(eye, look);
                    keep.push(e);
                }
                InteractionEvent::Fire { weapon: id } if id == wolfcraft_engine::items::ids::STAR_REMOTE => {
                    // Sneak + tap: next skill. Tap: cast it on the spot under the crosshair.
                    if self.player.sneaking {
                        self.stellar_ui.open = true;
                        continue;
                    }
                    let mode = self.stellar_ui.mode.min(10);
                    if self.star_cd > 0.0 || self.stellar.is_some() {
                        self.item_name = Some((format!("Пульт перезаряжается: {:.0} с", self.star_cd.ceil().max(1.0)), 1.5));
                        continue;
                    }
                    let Some(h) = wolfcraft_engine::physics::raycast(&self.chunks, eye, look, 160.0) else {
                        self.item_name = Some(("Наведите пульт на землю".into(), 1.5));
                        continue;
                    };
                    let at = self.chunks.grid().cell_center(h.cell);
                    let up = wolfcraft_engine::physics::up_at(at);
                    let side = (look - up * look.dot(up)).normalize_or_zero();
                    let side = if side.length_squared() < 0.5 { self.player.heading } else { side };
                    let across = up.cross(side).normalize_or_zero();
                    if mode == 2 {
                        // Seven stars in the shape of the Big Dipper, one after another; then the
                        // lines between them burn into the ground.
                        const DIPPER: [(f64, f64); 7] = [(-9.0, 3.0), (-5.5, 2.0), (-2.5, 0.5), (0.5, -0.5), (1.0, -4.5), (6.0, -5.0), (6.5, 0.0)];
                        let pts: Vec<DVec3> = DIPPER.iter().map(|(a, b)| at + side * *b + across * *a).collect();
                        for (k, p) in pts.iter().enumerate() {
                            self.star_queue.push((*p, 0.8 + k as f32 * 0.7));
                        }
                        self.stellar = Some(Stellar { mode: 2, at, cell: h.cell, t: 0.0, pts, step: 0, side });
                    } else {
                        self.stellar = Some(Stellar { mode, at, cell: h.cell, t: 0.0, pts: Vec::new(), step: 0, side });
                    }
                    self.skip_film = false;
                    const MOTTO: [&str; 11] = ["Огонь с края галактики", "Тишина, потом гром", "Созвездие падает на землю", "Мир режется на клетки", "Всё падает внутрь", "Небо горит", "Луч солнца режет землю", "Вечная зима за секунду", "Рождение и смерть звезды", "Сквозь кору — к самому ядру", "Небо стреляет туда, куда смотришь ты"];
                    self.title = Some((format!("SS-0{} · {}", mode + 1, STELLAR_NAMES[mode as usize].to_uppercase()), MOTTO[mode as usize].to_string(), 0.0));
                    self.star_cd = 12.0;
                    self.item_name = Some((format!("{}!", STELLAR_NAMES[mode as usize]), 2.5));
                    keep.push(e);
                }
                InteractionEvent::Fire { weapon: id } => {
                    let Some((ammo, kind, speed, cooldown)) = weapon(id) else { continue };
                    if !self.explosives.ready() {
                        continue;
                    }
                    // Guns that are their own ammo run on charge (WolfTech) or on nothing (the star blaster).
                    let self_powered = ammo == id;
                    if self_powered && wolfcraft_engine::items::rechargeable(id) && !creative {
                        let sel = self.player.inventory.selected;
                        let Some(g) = self.player.inventory.slots[sel].as_mut().filter(|g| g.id == id) else { continue };
                        let dur = g.info().durability;
                        if g.damage >= dur {
                            self.item_name = Some(("Разряжено — зарядите в энергоячейке".into(), 1.5));
                            continue;
                        }
                        g.damage += 1;
                    }
                    if !creative && !self_powered && self.player.inventory.count(ammo) == 0 {
                        self.item_name = Some((format!("Нет боеприпасов: {}", wolfcraft_engine::items::info(ammo).name), 1.5));
                        continue;
                    }
                    if !creative && !self_powered {
                        self.player.inventory.take(ammo, 1);
                    }
                    if let Some(gun) = wolfcraft_engine::explosives::firearm(id) {
                        self.explosives.fire_gun(gun, eye, look);
                        // Muzzle flash and a wisp of smoke in front of the eye.
                        let up = wolfcraft_engine::physics::up_at(eye);
                        let muzzle = eye + look * 0.9 - up * 0.15;
                        self.mobs.fx_glow(muzzle, look * 2.0, [1.0, 0.80, 0.35], 0.08, 0.06, wolfcraft_engine::mobs::FxKind::Rise);
                        self.mobs.fx(muzzle, look * 0.6 + up * 0.3, [0.70, 0.70, 0.68], 0.08, 0.5, wolfcraft_engine::mobs::FxKind::Rise);
                    } else {
                        let _ = kind;
                        self.explosives.fire(kind, eye, look, speed, cooldown);
                    }
                    keep.push(e);
                }
                other => keep.push(other),
            }
        }
        *events = keep;

        // Stars due now appear high in the sky; a beam of light marks where each will land.
        self.star_cd = (self.star_cd - dt).max(0.0);
        let mut due = Vec::new();
        let speed = self.stellar_ui.speed.clamp(0.3, 3.0);
        for s in &mut self.star_queue {
            s.1 -= dt * speed;
            if s.1 <= 0.0 {
                due.push(s.0);
            }
        }
        self.star_queue.retain(|s| s.1 > 0.0);
        for (at, left) in self.star_queue.clone() {
            if left < 1.2 && self.mobs.rand() < 0.5 {
                let up = wolfcraft_engine::physics::up_at(at);
                let h = self.mobs.rand() * 30.0;
                self.mobs.fx_glow(at + up * h, up * 2.0, [0.75, 0.88, 1.0], 0.12, 0.4, wolfcraft_engine::mobs::FxKind::Rise);
            }
        }
        for at in due {
            self.explosives.call_star(at, 70.0);
        }
        let mut stellar_events = Vec::new();
        self.tick_stellar(dt, &mut stellar_events);
        self.tick_epic(dt, &mut stellar_events);
        events.extend(stellar_events);
        // Meteors smoke and burn, plasma bolts leave a cyan streak.
        let glowing: Vec<(DVec3, wolfcraft_engine::explosives::ShotKind)> = self.explosives.glowing().collect();
        for (p, kind) in glowing {
            let up = wolfcraft_engine::physics::up_at(p);
            if kind == wolfcraft_engine::explosives::ShotKind::Meteor {
                self.mobs.fx_glow(p, up * 1.0, [1.0, 0.55, 0.15], 0.35, 0.5, wolfcraft_engine::mobs::FxKind::Rise);
                self.mobs.fx(p, up * 1.5, [0.25, 0.22, 0.2], 0.5, 1.5, wolfcraft_engine::mobs::FxKind::Rise);
            } else {
                self.mobs.fx_glow(p, DVec3::ZERO, [0.45, 0.95, 1.0], 0.12, 0.2, wolfcraft_engine::mobs::FxKind::Rise);
            }
        }
        // Their glowing tails.
        let stars: Vec<DVec3> = self.explosives.stars().collect();
        for p in stars {
            let up = wolfcraft_engine::physics::up_at(p);
            for k in 0..3 {
                let jitter = DVec3::new(self.mobs.rand() - 0.5, self.mobs.rand() - 0.5, self.mobs.rand() - 0.5) * 0.8;
                let col = [[0.85, 0.92, 1.0], [0.55, 0.75, 1.0], [1.0, 0.95, 0.75]][k];
                self.mobs.fx_glow(p + up * (1.0 + k as f64) + jitter, up * 3.0 + jitter, col, 0.35 - k as f32 * 0.08, 0.7, wolfcraft_engine::mobs::FxKind::Rise);
            }
        }

        let targets: Vec<DVec3> = self.mobs.mobs.iter().map(|m| m.centre()).collect();
        let (blasts, broken) = self.explosives.update(&mut self.chunks, &targets, dt);
        // Bullets: creatures take the damage, walls get a puff of dust and sparks.
        for h in std::mem::take(&mut self.explosives.hits) {
            match h.target {
                Some(i) if i < self.mobs.mobs.len() && self.mobs.mobs[i].hp > 0.0 => {
                    let from = h.pos - (self.mobs.mobs[i].centre() - h.pos).normalize_or_zero();
                    events.push(InteractionEvent::BulletHit { at: h.pos, creature: true });
                    self.damage_popup(h.pos, h.damage);
                    if let Some(kind) = self.mobs.hurt(i, h.damage, from) {
                        let at = self.mobs.mobs[i].body.pos;
                        for drop in wolfcraft_engine::items::mob_drops(kind, self.cfg.seed ^ self.clock.to_bits() as u64 ^ i as u64) {
                            self.drops.pop_from_block(drop, at);
                        }
                        self.player.survival.add_xp(if kind.hostile() { 5 } else { 1 });
                    }
                }
                _ => {
                    events.push(InteractionEvent::BulletHit { at: h.pos, creature: false });
                    let up = wolfcraft_engine::physics::up_at(h.pos);
                    for k in 0..4 {
                        let a = k as f64 * 1.57 + self.clock as f64;
                        let side = DVec3::new(a.cos(), (a * 1.3).sin(), a.sin());
                        self.mobs.fx_glow(h.pos, side * 2.5 + up * 1.5, [1.0, 0.85, 0.45], 0.04, 0.18, wolfcraft_engine::mobs::FxKind::Rise);
                    }
                    self.mobs.fx(h.pos, up * 0.5, [0.55, 0.53, 0.50], 0.10, 0.6, wolfcraft_engine::mobs::FxKind::Rise);
                }
            }
        }
        // Smoke behind rockets.
        let rockets: Vec<DVec3> = self.explosives.rockets().collect();
        for p in rockets {
            let up = wolfcraft_engine::physics::up_at(p);
            self.mobs.fx(p, up * 0.4, [0.62, 0.62, 0.60], 0.12, 0.9, wolfcraft_engine::mobs::FxKind::Rise);
            self.mobs.fx_glow(p, DVec3::ZERO, [1.0, 0.65, 0.20], 0.06, 0.2, wolfcraft_engine::mobs::FxKind::Rise);
        }
        let mut n_broken = 0u32;
        for (pos, block) in broken {
            let centre = self.chunks.grid().cell_center(pos);
            // Chests and furnaces caught in a blast spill what they held.
            if let Some(slots) = self.containers.remove(&pos) {
                for st in slots.into_iter().flatten() {
                    self.drops.pop_from_block(st, centre);
                }
            }
            self.furnace_t.remove(&pos);
            // About a third of the blocks fall out as loot (not in creative).
            if !creative && block != wolfcraft_engine::block::Block::Dynamite && self.mobs.rand() < 0.33 {
                let ip = glam::IVec3::new(pos.i, pos.j, pos.k ^ ((pos.f as i32) << 24));
                for d in wolfcraft_engine::items::drops(block, true, ip, self.cfg.seed) {
                    self.drops.pop_from_block(d, centre);
                }
            }
            n_broken += 1;
            events.push(InteractionEvent::Blasted { pos, block });
        }
        let _ = n_broken;
        for b in blasts {
            events.push(InteractionEvent::Exploded { at: b.pos, power: b.power });
            // A fallen star leaves its glowing core at the bottom of the crater.
            if b.power >= wolfcraft_engine::explosives::STAR_POWER {
                self.shake = self.shake.max(0.7);
                self.flash = Some(([0.8, 0.9, 1.0], 0.35));
                self.embers.push((b.pos, 6.0, 4.0));
                let up = wolfcraft_engine::physics::up_at(b.pos);
                let core = self.chunks.grid().world_to_cell(b.pos - up * 3.5);
                if self.chunks.get_block(core).is_some_and(|x| !x.is_fluid()) && self.chunks.set_block(core, wolfcraft_engine::block::Block::Glowstone) {
                    self.net_out.push((core, wolfcraft_engine::block::Block::Glowstone));
                }
            }
            // A 3D blast: a chunky fireball that swells and fades, a ring of dust racing out
            // along the ground, and a mushroom of smoke rising over it (bigger for bigger blasts).
            {
                use wolfcraft_engine::mobs::FxKind;
                let up = wolfcraft_engine::physics::up_at(b.pos);
                let k = (b.power / 4.0).clamp(0.6, 1.8) as f64;
                let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
                let ta = up.cross(helper).normalize();
                let tb = up.cross(ta);
                let n_ball = (26.0 * k) as usize;
                for i in 0..n_ball {
                    let a = i as f64 * 2.399;
                    let e = (i as f64 / n_ball as f64) * 2.0 - 1.0;
                    let r = (1.0 - e * e).sqrt();
                    let dir = ta * (a.cos() * r) + tb * (a.sin() * r) + up * e.abs();
                    let col = [[1.0, 0.95, 0.70], [1.0, 0.70, 0.22], [0.98, 0.42, 0.10], [0.85, 0.22, 0.06]][i % 4];
                    self.mobs.fx_glow(b.pos + dir * 0.3, dir * (4.0 + (i % 3) as f64) * k, col, (0.20 + (i % 4) as f32 * 0.07) * k as f32, 0.45 + (i % 3) as f32 * 0.12, FxKind::Rise);
                }
                let n_ring = (20.0 * k) as usize;
                for i in 0..n_ring {
                    let a = i as f64 * std::f64::consts::TAU / n_ring as f64;
                    let dir = ta * a.cos() + tb * a.sin();
                    self.mobs.fx(b.pos + dir * 0.5, dir * 9.0 * k + up * 0.6, [0.62, 0.58, 0.52], 0.18 * k as f32, 0.9, FxKind::Rise);
                }
                for i in 0..(14.0 * k) as usize {
                    let a = i as f64 * 1.9;
                    let dir = ta * a.cos() + tb * a.sin();
                    let shade = 0.18 + (i % 4) as f32 * 0.06;
                    // Stem, then the cap spreading out at the top.
                    let cap = i % 2 == 0;
                    let v = if cap { up * 5.0 * k + dir * 2.5 * k } else { up * 3.0 * k + dir * 0.4 };
                    self.mobs.fx(b.pos + up * 0.5, v, [shade, shade * 0.97, shade * 0.95], (0.32 + (i % 3) as f32 * 0.12) * k as f32, 2.4, FxKind::Rise);
                }
                for i in 0..36 {
                    let a = i as f64 * 2.399;
                    let side = DVec3::new(a.cos(), (a * 1.3).sin(), a.sin());
                    let speed = 3.0 + (i % 5) as f64 * 1.6;
                    let col = [[1.0, 0.92, 0.55], [1.0, 0.62, 0.18], [0.95, 0.35, 0.10]][i % 3];
                    self.mobs.fx_glow(b.pos, side * speed + up * 2.0, col, 0.10 + (i % 3) as f32 * 0.05, 0.7, FxKind::Rise);
                }
                for i in 0..20 {
                    let a = i as f64 * 1.7;
                    let side = DVec3::new(a.cos(), (a * 0.9).sin(), a.sin());
                    let shade = 0.25 + (i % 4) as f32 * 0.08;
                    self.mobs.fx(b.pos + side * 0.8, side * 1.2 + up * 1.4, [shade, shade, shade * 0.95], 0.35 + (i % 3) as f32 * 0.12, 2.2, FxKind::Rise);
                }
                self.mobs.chips(b.pos, &[[0.35, 0.33, 0.32], [0.20, 0.19, 0.19]], 20, 9.0);
            }
            // The player: damage through armor, and a push away from the blast.
            if !self.player.survival.dead {
                let centre = self.player.body.pos + self.player.up() * 0.9;
                let dist = (centre - b.pos).length();
                let dmg = blast_damage(b.power, dist);
                if dmg > 0.0 {
                    let armor = self.player.armor_points();
                    self.player.damage(dmg * (1.0 - (armor as f32 * 0.04).min(0.8)), self.cfg.difficulty);
                    let push = (centre - b.pos).normalize_or_zero() * (dmg as f64 * 0.8).min(14.0);
                    if !self.player.body.flying {
                        self.player.body.vel += push;
                        self.player.body.on_ground = false;
                    }
                }
            }
            for i in 0..self.mobs.mobs.len() {
                if self.mobs.mobs[i].hp <= 0.0 {
                    continue;
                }
                let dist = (self.mobs.mobs[i].centre() - b.pos).length();
                let dmg = blast_damage(b.power, dist);
                if dmg <= 0.0 {
                    continue;
                }
                let pos = self.mobs.mobs[i].centre();
                self.damage_popup(pos, dmg);
                if let Some(kind) = self.mobs.hurt(i, dmg, b.pos) {
                    let at = self.mobs.mobs[i].body.pos;
                    for drop in wolfcraft_engine::items::mob_drops(kind, self.cfg.seed ^ self.clock.to_bits() as u64 ^ i as u64) {
                        self.drops.pop_from_block(drop, at);
                    }
                    self.player.survival.add_xp(if kind.hostile() { 5 } else { 1 });
                }
            }
        }
    }

    /// «Питомник»: buying eggs at incubators, coins from coin blocks, pets that follow the
    /// player and dig coin blocks, and new coin blocks appearing round the lobby.
    fn petsim_step(&mut self, events: &mut Vec<InteractionEvent>, dt: f32) {
        use wolfcraft_engine::block::Block;
        use wolfcraft_engine::petsim::{COIN_REWARD, EGGS, PETS, Rarity, hatch};
        let creative = self.player.is_creative();
        use wolfcraft_engine::items::{ItemStack, ids};
        // The pets are the pet items in the inventory (and the second hand).
        let inv = &self.player.inventory;
        self.petsim.owned = inv.slots.iter().chain(std::iter::once(&inv.offhand)).flatten().filter(|s| (ids::PET_BASE..=ids::PET_LAST).contains(&s.id)).map(|s| (s.id - ids::PET_BASE) as usize).collect();
        let mut keep = Vec::with_capacity(events.len());
        for e in events.drain(..) {
            match e {
                InteractionEvent::BuyEgg { tier } => {
                    // The incubator sells a capsule egg into the inventory.
                    let egg = &EGGS[(tier as usize).min(EGGS.len() - 1)];
                    if !creative && self.petsim.coins < egg.price {
                        self.item_name = Some((format!("{}: нужно {} монет (у вас {})", egg.name, egg.price, self.petsim.coins), 2.5));
                        continue;
                    }
                    if !creative {
                        self.petsim.coins -= egg.price;
                    }
                    let stack = ItemStack::new(ids::EGG_BASE + tier as u16, 1);
                    if let Some(rest) = self.player.inventory.add(stack) {
                        let at = self.player.body.pos + self.player.up() * 1.0;
                        self.drops.pop_from_block(rest, at);
                    }
                    self.item_name = Some((format!("+ {} — возьмите её в руку и тапните, чтобы вылупить", egg.name), 3.0));
                    keep.push(e);
                }
                InteractionEvent::Hatch { tier } => {
                    // The capsule egg in the hand opens.
                    if self.petsim.hatching.is_some() {
                        continue;
                    }
                    if !creative {
                        self.player.inventory.consume_selected();
                    }
                    let kind = hatch(tier as usize, self.mobs.rand(), self.mobs.rand());
                    self.petsim.hatching = Some((0.0, kind));
                    keep.push(e);
                }
                InteractionEvent::Broken { block: Block::CoinBlock, .. } => {
                    self.petsim.coins += COIN_REWARD;
                    self.item_name = Some((format!("+{COIN_REWARD} монет · всего {}", self.petsim.coins), 1.5));
                    keep.push(e);
                }
                other => keep.push(other),
            }
        }
        *events = keep;
        // The hatch animation runs 3.2 s; the pet joins the team when the egg opens.
        if let Some((t, kind)) = &mut self.petsim.hatching {
            let before = *t;
            *t += dt;
            if before < 1.8 && *t >= 1.8 {
                let k = *kind;
                self.item_name = Some((format!("{} {}!", PETS[k].rarity.name(), PETS[k].name), 3.0));
                self.picked_up += 1;
                // The pet joins the inventory (or falls at the feet if it is full).
                if let Some(rest) = self.player.inventory.add(ItemStack::new(ids::PET_BASE + k as u16, 1)) {
                    let at = self.player.body.pos + self.player.up() * 1.0;
                    self.drops.pop_from_block(rest, at);
                }
            }
            if *t > 3.2 {
                self.petsim.hatching = None;
            }
        }
        if self.petsim.owned.is_empty() && self.petsim_home.is_none() {
            return;
        }
        let feet = self.player.body.pos;
        let heading = self.player.heading;
        if self.petsim.pets.len() != self.petsim.best().len() || self.petsim.pets.iter().map(|p| p.kind).ne(self.petsim.best()) {
            self.petsim.refresh(feet, heading);
        }
        // Scan for coin blocks around the player twice a second.
        self.coin_scan -= dt;
        if self.coin_scan <= 0.0 {
            self.coin_scan = 0.5;
            self.coin_cells.clear();
            let g = self.chunks.grid();
            let at = g.world_to_cell(feet);
            for di in -12..=12 {
                for dj in -12..=12 {
                    for dk in -4..=5 {
                        if let Some(c) = g.offset(at, di, dj, dk) {
                            if self.chunks.get_block(c) == Some(Block::CoinBlock) {
                                self.coin_cells.push((c, g.cell_center(c)));
                            }
                        }
                    }
                }
            }
            // Keep about ten coin blocks round the lobby.
            if let Some(home) = self.petsim_home {
                if (home - feet).length() < 40.0 && self.coin_cells.len() < 10 {
                    let a = self.mobs.rand() * std::f64::consts::TAU;
                    let r = 4.0 + self.mobs.rand() * 9.0;
                    let (di, dj) = ((a.cos() * r) as i32, (a.sin() * r) as i32);
                    let home_cell = g.world_to_cell(home);
                    if let Some(col) = g.offset(home_cell, di, dj, 0) {
                        // First solid block from above with air over it.
                        for dk in (-6..=6).rev() {
                            let (Some(c), Some(up)) = (g.offset(col, 0, 0, dk), g.offset(col, 0, 0, dk + 1)) else { continue };
                            let (Some(b), Some(ub)) = (self.chunks.get_block(c), self.chunks.get_block(up)) else { break };
                            if b.is_solid() && ub == Block::Air {
                                if self.chunks.set_block(up, Block::CoinBlock) {
                                    events.push(InteractionEvent::WorldSet { pos: up, block: Block::CoinBlock });
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
        let dug = self.petsim.update(feet, heading, &self.coin_cells, dt);
        for c in dug {
            if self.chunks.get_block(c) == Some(Block::CoinBlock) && self.chunks.set_block(c, Block::Air) {
                self.coin_cells.retain(|(x, _)| *x != c);
                self.petsim.coins += COIN_REWARD;
                self.item_name = Some((format!("+{COIN_REWARD} монет · всего {}", self.petsim.coins), 1.5));
                self.picked_up += 1;
                events.push(InteractionEvent::Blasted { pos: c, block: Block::CoinBlock });
            }
        }
        // Rare pets glitter in their colour; digging pets throw gold sparks.
        let sparkle: Vec<(DVec3, [f32; 3], bool)> = self.petsim.pets.iter().map(|p| (p.pos, PETS[p.kind].rarity.color(), PETS[p.kind].rarity >= Rarity::Rare || p.dig > 0.0)).collect();
        for (p, c, on) in sparkle {
            if on && self.mobs.rand() < dt as f64 * 6.0 {
                let up = wolfcraft_engine::physics::up_at(p);
                let o = DVec3::new(self.mobs.rand() - 0.5, self.mobs.rand() - 0.5, self.mobs.rand() - 0.5) * 0.7;
                self.mobs.fx_glow(p + up * 0.5 + o, up * 0.5, c, 0.035, 0.8, wolfcraft_engine::mobs::FxKind::Rise);
            }
        }
    }

    /// Pets as creatures to draw (the renderer only knows creatures).
    pub fn pet_models(&self) -> Vec<wolfcraft_engine::mobs::Mob> {
        use wolfcraft_engine::petsim::{PETS, PetSim};
        self.petsim
            .pets
            .iter()
            .map(|p| {
                let up = wolfcraft_engine::physics::up_at(p.pos);
                let mut m = wolfcraft_engine::mobs::Mob::new(PETS[p.kind].model, p.pos + up * PetSim::bounce(p), p.heading);
                m.anim = p.anim;
                m.amount = if p.moving { 1.0 } else { 0.0 };
                m
            })
            .collect()
    }

    /// Builds the «Питомник» lobby ahead of the player: a floor, three incubators and coin
    /// blocks round it. Returns the number of blocks set.
    pub fn build_petsim_lobby(&mut self) -> Result<usize, String> {
        use wolfcraft_engine::block::Block as B;
        if self.hell {
            return Err("Питомник строится только на родной планете".into());
        }
        let g = self.chunks.grid();
        let feet = g.world_to_cell(self.player.body.pos);
        let centre = g.cell_center(feet);
        let ax = |di: i32, dj: i32| g.offset(feet, di, dj, 0).map(|c| (g.cell_center(c) - centre).normalize()).unwrap_or(DVec3::ZERO);
        let (ei, ej) = (ax(1, 0), ax(0, 1));
        let h = self.player.heading;
        let fwd = if h.dot(ei).abs() >= h.dot(ej).abs() { (h.dot(ei).signum() as i32, 0) } else { (0, h.dot(ej).signum() as i32) };
        let side = (fwd.1, fwd.0);
        let mid = g.offset(feet, fwd.0 * 9, fwd.1 * 9, 0).ok_or("Слишком близко к краю грани планеты, отойдите чуть в сторону")?;
        const R: i32 = 8;
        let mut cells = Vec::new();
        for di in -R..=R {
            for dj in -R..=R {
                if di * di + dj * dj > R * R + R {
                    continue;
                }
                let Some(col) = g.offset(mid, di, dj, 0) else { continue };
                for dk in -2..=5 {
                    let Some(cell) = g.offset(col, 0, 0, dk) else { continue };
                    let b = if dk == -2 { B::StoneBricks } else if dk == -1 { if (di + dj).rem_euclid(2) == 0 { B::PolishedMarble } else { B::SmoothStone } } else { B::Air };
                    cells.push((cell, b));
                }
            }
        }
        // Incubators in a row at the far side, lanterns between them.
        for (n, b) in [(-3, B::IncubatorCommon), (0, B::IncubatorRare), (3, B::IncubatorLegendary)] {
            if let Some(c) = g.offset(mid, fwd.0 * 4 + side.0 * n, fwd.1 * 4 + side.1 * n, 0) {
                cells.push((c, b));
            }
        }
        for n in [-5, 5] {
            if let Some(c) = g.offset(mid, fwd.0 * 4 + side.0 * n, fwd.1 * 4 + side.1 * n, 0) {
                cells.push((c, B::Lantern));
            }
        }
        // A ring of coin blocks.
        for k in 0..12 {
            let a = k as f64 / 12.0 * std::f64::consts::TAU;
            let (di, dj) = ((a.cos() * 6.0).round() as i32, (a.sin() * 6.0).round() as i32);
            if di * fwd.0 + dj * fwd.1 >= 3 {
                continue; // keep the incubator side clear
            }
            if let Some(c) = g.offset(mid, di, dj, 0) {
                cells.push((c, B::CoinBlock));
            }
        }
        let (n, missed) = crate::worldedit::apply(self, cells);
        if n == 0 || missed > 0 {
            return Err("Мир вокруг ещё не загрузился, подойдите ближе или попробуйте через пару секунд".into());
        }
        self.petsim_home = Some(g.cell_center(mid));
        self.coin_scan = 0.0;
        Ok(n)
    }

    /// Builds the «Кнопки» lobby ahead of the player: a floor with a row of buttons (four
    /// multiplier buttons and the rebirth button) and lanterns.
    pub fn build_button_lobby(&mut self) -> Result<usize, String> {
        use wolfcraft_engine::block::Block as B;
        let g = self.chunks.grid();
        let feet = g.world_to_cell(self.player.body.pos);
        let centre = g.cell_center(feet);
        let ax = |di: i32, dj: i32| g.offset(feet, di, dj, 0).map(|c| (g.cell_center(c) - centre).normalize()).unwrap_or(DVec3::ZERO);
        let (ei, ej) = (ax(1, 0), ax(0, 1));
        let h = self.player.heading;
        let fwd = if h.dot(ei).abs() >= h.dot(ej).abs() { (h.dot(ei).signum() as i32, 0) } else { (0, h.dot(ej).signum() as i32) };
        let side = (fwd.1, fwd.0);
        let mid = g.offset(feet, fwd.0 * 7, fwd.1 * 7, 0).ok_or("Слишком близко к краю грани планеты, отойдите чуть в сторону")?;
        let mut cells = Vec::new();
        for a in -6..=6 {
            for b in -4..=4 {
                let Some(col) = g.offset(mid, side.0 * a + fwd.0 * b, side.1 * a + fwd.1 * b, 0) else { continue };
                for dk in -2..=4 {
                    let Some(cell) = g.offset(col, 0, 0, dk) else { continue };
                    let bl = if dk == -2 { B::StoneBricks } else if dk == -1 { if (a + b).rem_euclid(2) == 0 { B::SmoothStone } else { B::PolishedMarble } } else { B::Air };
                    cells.push((cell, bl));
                }
            }
        }
        // The buttons sit in the floor in a row, two blocks apart, cheapest on the left.
        let pads = [B::ButtonMult1, B::ButtonMult2, B::ButtonMult3, B::ButtonMult4, B::ButtonRebirth];
        for (n, pad) in pads.iter().enumerate() {
            let a = -4 + n as i32 * 2;
            if let Some(c) = g.offset(mid, side.0 * a, side.1 * a, -1) {
                cells.push((c, *pad));
            }
        }
        for a in [-6, 6] {
            if let Some(c) = g.offset(mid, side.0 * a + fwd.0 * 4, side.1 * a + fwd.1 * 4, 0) {
                cells.push((c, B::Lantern));
            }
        }
        let (n, missed) = crate::worldedit::apply(self, cells);
        if n == 0 || missed > 0 {
            return Err("Мир вокруг ещё не загрузился, подойдите ближе или попробуйте через пару секунд".into());
        }
        self.buttonsim.active = true;
        Ok(n)
    }

    /// Builds the «Кликер» lobby: a round start island ahead of the player and four floating
    /// islands further on and higher up (forest, ice, volcano, sky), each in its own blocks.
    pub fn build_clicker_lobby(&mut self) -> Result<usize, String> {
        use wolfcraft_engine::block::Block as B;
        use wolfcraft_engine::clicker::ISLANDS;
        if self.hell {
            return Err("Кликер строится только на родной планете".into());
        }
        let g = self.chunks.grid();
        let feet = g.world_to_cell(self.player.body.pos);
        let centre = g.cell_center(feet);
        let ax = |di: i32, dj: i32| g.offset(feet, di, dj, 0).map(|c| (g.cell_center(c) - centre).normalize()).unwrap_or(DVec3::ZERO);
        let (ei, ej) = (ax(1, 0), ax(0, 1));
        let h = self.player.heading;
        let fwd = if h.dot(ei).abs() >= h.dot(ej).abs() { (h.dot(ei).signum() as i32, 0) } else { (0, h.dot(ej).signum() as i32) };
        let mut cells = Vec::new();
        let mut tops = Vec::new();
        for (k, isle) in ISLANDS.iter().enumerate() {
            let k = k as i32;
            let Some(mid) = g.offset(feet, fwd.0 * (8 + 16 * k), fwd.1 * (8 + 16 * k), 4 * k) else { continue };
            const R: i32 = 6;
            for di in -R..=R {
                for dj in -R..=R {
                    let d2 = di * di + dj * dj;
                    if d2 > R * R + R {
                        continue;
                    }
                    let Some(col) = g.offset(mid, di, dj, 0) else { continue };
                    // A floating island: floor on top, a rocky cone under it; the start island
                    // sits on the ground.
                    let depth = if k == 0 { 2 } else { 1 + (R * R + R - d2) / 9 };
                    for dk in -depth..=4 {
                        let Some(cell) = g.offset(col, 0, 0, dk) else { continue };
                        let b = if dk == -1 { isle.floor } else if dk < -1 { if k == 0 { B::StoneBricks } else { B::Stone } } else { B::Air };
                        cells.push((cell, b));
                    }
                    // A ring of the island's accent at the edge.
                    if d2 > (R - 1) * (R - 1) && (di + dj).rem_euclid(3) == 0 {
                        if let Some(c) = g.offset(col, 0, 0, 0) {
                            cells.push((c, isle.accent));
                        }
                    }
                }
            }
            // A lantern in the middle of every island, the sell spot (gold) on the start one.
            if let Some(c) = g.offset(mid, 0, 0, 0) {
                cells.push((c, B::Lantern));
            }
            if k == 0 {
                if let Some(c) = g.offset(mid, 2, 0, -1) {
                    cells.push((c, B::GoldBlock));
                }
            }
            if let Some(top) = g.offset(mid, fwd.1 * 2, fwd.0 * 2, 0) {
                let c = g.cell_center(top);
                tops.push([c.x, c.y, c.z]);
            }
        }
        let (n, missed) = crate::worldedit::apply(self, cells);
        if n == 0 {
            return Err("Мир вокруг ещё не загрузился, подойдите ближе или попробуйте через пару секунд".into());
        }
        let _ = missed;
        self.clicker.island_pos = tops;
        self.clicker.active = true;
        Ok(n)
    }

    /// The next block the quarry at `c` digs: its 5×5 shaft from the layer under it down to
    /// 64 deep (`cursor` counts cells already looked at). Air, water and bedrock are skipped.
    fn quarry_next(&self, c: wolfcraft_engine::cubesphere::Cell, cursor: &mut f32) -> Option<(wolfcraft_engine::cubesphere::Cell, wolfcraft_engine::block::Block)> {
        use wolfcraft_engine::block::Block;
        let g = self.chunks.grid();
        for _ in 0..80 {
            let n = *cursor as i32;
            let layer = n / 25;
            if layer >= 64 {
                return None;
            }
            let (di, dj) = (n % 25 % 5 - 2, n % 25 / 5 - 2);
            let Some(cell) = g.offset(c, di, dj, -1 - layer) else {
                *cursor += 1.0;
                continue;
            };
            let Some(b) = self.chunks.get_block(cell) else { return None };
            *cursor += 1.0;
            if matches!(b, Block::Air | Block::Water | Block::Bedrock | Block::InnerCore) || b.info().hardness < 0.0 || b.is_water() {
                continue;
            }
            return Some((cell, b));
        }
        None
    }

    /// The mining laser: up to 8 solid blocks along the look ray break with their loot; costs
    /// charge.
    fn fire_laser(&mut self) {
        use wolfcraft_engine::block::Block;
        let Some(h) = self.player.inventory.held() else { return };
        if !self.player.is_creative() && h.damage + 20 > h.info().durability {
            self.item_name = Some(("Горный лазер разряжен — зарядите в энергоячейке".into(), 2.0));
            return;
        }
        let eye = self.player.eye_pos();
        let dir = self.player.look_dir();
        let g = self.chunks.grid();
        let mut cells = Vec::new();
        let mut last = None;
        let mut t = 0.5;
        while t < 24.0 && cells.len() < 8 {
            let cell = g.world_to_cell(eye + dir * t);
            t += 0.25;
            if last == Some(cell) {
                continue;
            }
            last = Some(cell);
            let Some(b) = self.chunks.get_block(cell) else { break };
            if b == Block::Air || b.is_water() {
                continue;
            }
            if b.info().hardness < 0.0 || matches!(b, Block::Bedrock | Block::InnerCore) {
                break;
            }
            cells.push((cell, b));
        }
        if cells.is_empty() {
            return;
        }
        let creative = self.player.is_creative();
        for (cell, b) in &cells {
            if !creative {
                let seed = (self.mobs.rand() * 1e9) as u64;
                let centre = g.cell_center(*cell);
                for d in wolfcraft_engine::items::drops(*b, true, glam::IVec3::new(cell.i, cell.j, cell.k), seed) {
                    self.drops.pop_from_block(d, centre);
                }
            }
        }
        let n = cells.len();
        crate::worldedit::apply(self, cells.into_iter().map(|(c, _)| (c, Block::Air)).collect::<Vec<_>>());
        if !creative {
            if let Some(s) = self.player.inventory.slots[self.player.inventory.selected].as_mut() {
                s.damage = (s.damage + 20).min(s.info().durability);
            }
        }
        self.item_name = Some((format!("Лазер: {n} бл."), 0.8));
    }

    /// PvP templates: generators, the bed / egg, death and the round.
    fn tick_arena(&mut self, dt: f32) {
        use wolfcraft_engine::{block::Block, items::{ItemStack, ids}};
        let Some(a) = &mut self.arena else { return };
        for (id, p) in a.tick(dt) {
            self.drops.spawn(ItemStack::new(id, 1), DVec3::new(p[0], p[1], p[2]), DVec3::ZERO, 0.0);
        }
        // The bed / egg: broken → no coming back.
        if let (Some((f, i, j, k)), true) = (a.heart, a.heart_alive) {
            let want = if a.mode == wolfcraft_engine::arenas::Mode::EggWars { Block::TeamEgg } else { Block::TeamBed };
            if let Some(b) = self.chunks.get_block(wolfcraft_engine::cubesphere::Cell::new(f, i, j, k)) {
                if b != want {
                    a.heart_alive = false;
                    self.title = Some((if want == Block::TeamEgg { "Ваше яйцо разбито!".into() } else { "Ваша кровать разрушена!".into() }, "Больше вы не возродитесь".into(), 3.0));
                }
            }
        }
        // Death: out of the game unless the bed / egg stands.
        if self.player.survival.dead && !self.arena_dead_seen {
            self.arena_dead_seen = true;
            a.on_death();
            if a.eliminated {
                self.title = Some(("Вы выбыли".into(), "Наблюдайте за игрой".into(), 3.0));
            }
        } else if !self.player.survival.dead {
            self.arena_dead_seen = false;
        }
        let a = self.arena.as_ref().unwrap();
        self.player.spawn = DVec3::new(a.base[0], a.base[1], a.base[2]);
        // Мардер: 10 gold → a bow.
        if a.mode == wolfcraft_engine::arenas::Mode::Murder && a.role != wolfcraft_engine::arenas::Role::Murderer && self.player.inventory.count(ids::GOLD_INGOT) >= 10 {
            self.player.inventory.take(ids::GOLD_INGOT, 10);
            self.player.inventory.add(ItemStack::new(ids::BOW, 1));
            self.player.inventory.add(ItemStack::new(ids::ARROW, 4));
            self.item_name = Some(("10 золота → лук и стрелы!".into(), 2.5));
        }
    }

    /// Shop of Бед/Эггварс: pay with iron, gold or diamonds.
    pub fn arena_buy(&mut self, k: usize) -> Result<String, String> {
        use wolfcraft_engine::{arenas::SHOP, items::ItemStack};
        let (name, (gid, gn), (cid, cn)) = SHOP[k.min(SHOP.len() - 1)];
        // Studio playtests (creative) get it free when they can't pay.
        if !self.is_creative() && self.player.inventory.count(cid) < cn as u32 {
            return Err(format!("{name}: нужно {} × {}", cn, ItemStack::new(cid, 1).info().name));
        }
        if self.player.inventory.count(cid) >= cn as u32 {
            self.player.inventory.take(cid, cn as u32);
        }
        if let Some(rest) = self.player.inventory.add(ItemStack::new(gid, gn)) {
            self.drops.spawn(rest, self.player.body.pos, DVec3::ZERO, 0.5);
        }
        Ok(format!("Куплено: {name}"))
    }

    /// The cell under the player, the forward and side grid directions (for the builders).
    fn arena_axes(&self) -> Result<(wolfcraft_engine::cubesphere::Cell, (i32, i32), (i32, i32)), String> {
        if self.hell {
            return Err("Строится только на родной планете".into());
        }
        let g = self.chunks.grid();
        let feet = g.world_to_cell(self.player.body.pos);
        let centre = g.cell_center(feet);
        let ax = |di: i32, dj: i32| g.offset(feet, di, dj, 0).map(|c| (g.cell_center(c) - centre).normalize()).unwrap_or(DVec3::ZERO);
        let (ei, ej) = (ax(1, 0), ax(0, 1));
        let h = self.player.heading;
        let fwd = if h.dot(ei).abs() >= h.dot(ej).abs() { (h.dot(ei).signum() as i32, 0) } else { (0, h.dot(ej).signum() as i32) };
        Ok((feet, fwd, (fwd.1, fwd.0)))
    }

    /// Builds a PvP template ahead of the player and starts it.
    pub fn build_pvp(&mut self, mode: wolfcraft_engine::arenas::Mode) -> Result<String, String> {
        use wolfcraft_engine::arenas::{Arena, Gen, Mode, Role, chest_loot};
        use wolfcraft_engine::block::Block as B;
        use wolfcraft_engine::items::{ItemStack, ids};
        let (feet, fwd, side) = self.arena_axes()?;
        let g = self.chunks.grid();
        // (a along the side, b forward, k up) → cell.
        let at = |a: i32, b: i32, k: i32| g.offset(feet, side.0 * a + fwd.0 * b, side.1 * a + fwd.1 * b, k);
        let mut cells: Vec<(wolfcraft_engine::cubesphere::Cell, B)> = Vec::new();
        let mut chests: Vec<(wolfcraft_engine::cubesphere::Cell, bool)> = Vec::new();
        let roll = self.mobs.rand();
        let mut arena = Arena::new(mode, Role::roll(mode, roll));
        let centre_of = |c: wolfcraft_engine::cubesphere::Cell| {
            let p = g.cell_center(c);
            [p.x, p.y, p.z]
        };
        // A square slab (floor block on top, a layer under it) with air above.
        let slab = |cells: &mut Vec<_>, a0: i32, b0: i32, r: i32, k: i32, top: B, under: B, air: i32| {
            for a in -r..=r {
                for b in -r..=r {
                    for dk in -1..=air {
                        if let Some(c) = at(a0 + a, b0 + b, k + dk) {
                            cells.push((c, if dk == -1 { top } else { B::Air }));
                        }
                    }
                    if let Some(c) = at(a0 + a, b0 + b, k - 2) {
                        cells.push((c, under));
                    }
                }
            }
        };
        let lift = 8;
        match mode {
            Mode::BedWars | Mode::EggWars => {
                let teams = [(0, 18, B::RedWool, "Красные"), (18, 30, B::BlueWool, "Синие"), (-18, 30, B::YellowWool, "Жёлтые"), (0, 42, B::Wool, "Белые")];
                for (n, (a0, b0, wool, _)) in teams.iter().enumerate() {
                    slab(&mut cells, *a0, *b0, 4, lift, *wool, B::Stone, 4);
                    // Bed / egg at the back of the island, a shop chest and the generators.
                    let back = if n == 0 { -3 } else if n == 3 { 3 } else { 0 };
                    let sidew = if n == 1 { 3 } else if n == 2 { -3 } else { 0 };
                    if let Some(c) = at(a0 + sidew, b0 + back, lift) {
                        cells.push((c, if mode == Mode::EggWars { B::TeamEgg } else { B::TeamBed }));
                        if n == 0 {
                            arena.heart = Some((c.f, c.i, c.j, c.k));
                        }
                        // Protection ring.
                        for (da, db) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                            if let Some(c) = at(a0 + sidew + da, b0 + back + db, lift) {
                                cells.push((c, if mode == Mode::EggWars { B::Glass } else { B::Wool }));
                            }
                        }
                    }
                    if n == 0 {
                        if let Some(c) = at(*a0 + 3, *b0, lift) {
                            cells.push((c, B::Chest));
                        }
                        for (dx, item, every) in [(-2, ids::IRON_INGOT, 1.5f32), (2, ids::GOLD_INGOT, 6.0)] {
                            if let Some(c) = at(a0 + dx, b0 + 2, lift) {
                                cells.push((g.offset(c, 0, 0, -1).unwrap_or(c), if item == ids::IRON_INGOT { B::IronBlock } else { B::GoldBlock }));
                                arena.gens.push(Gen { pos: centre_of(c), item, every, t: 0.0 });
                            }
                        }
                        if let Some(c) = at(*a0, *b0, lift) {
                            arena.base = centre_of(c);
                        }
                    }
                }
                // The middle island with diamonds.
                slab(&mut cells, 0, 30, 5, lift + 2, B::Obsidian, B::Stone, 4);
                for da in [-3, 3] {
                    if let Some(c) = at(da, 30, lift + 2) {
                        cells.push((g.offset(c, 0, 0, -1).unwrap_or(c), B::DiamondBlock));
                        arena.gens.push(Gen { pos: centre_of(c), item: ids::DIAMOND, every: 20.0, t: 0.0 });
                    }
                }
                self.player.inventory.add(ItemStack::new(B::RedWool as u16, 32));
                self.player.inventory.add(ItemStack::new(ids::tool(0, 3), 1));
            }
            Mode::SkyWars => {
                // Eight islands round the middle, each with a chest; two rich chests in the middle.
                for n in 0..8 {
                    let ang = n as f64 / 8.0 * std::f64::consts::TAU;
                    let (a0, b0) = ((ang.sin() * 20.0).round() as i32, 28 + (-ang.cos() * 20.0).round() as i32);
                    slab(&mut cells, a0, b0, 2, lift, B::Grass, B::Dirt, 4);
                    if let Some(c) = at(a0 + 1, b0 + 1, lift) {
                        cells.push((c, B::Chest));
                        chests.push((c, false));
                    }
                    if n == 0 {
                        if let Some(c) = at(a0, b0, lift) {
                            arena.base = centre_of(c);
                        }
                    }
                }
                slab(&mut cells, 0, 28, 5, lift + 1, B::StoneBricks, B::Stone, 5);
                for da in [-2, 2] {
                    if let Some(c) = at(da, 28, lift + 1) {
                        cells.push((c, B::Chest));
                        chests.push((c, true));
                    }
                }
            }
            Mode::Murder => {
                // A mansion: plank floor, brick walls with windows, rooms, a roof; gold spawns.
                let (w, d) = (10i32, 7i32);
                let b0 = 12;
                for a in -w..=w {
                    for b in -d..=d {
                        let wall = a.abs() == w || b.abs() == d;
                        let inner = (a == 0 && b.abs() != 2) || (b == 0 && a.abs() != 5 && a.abs() != 6);
                        for k in -1..=5 {
                            let bl = if k == -1 {
                                if (a + b).rem_euclid(2) == 0 { B::OakPlanks } else { B::SprucePlanks }
                            } else if k == 5 {
                                B::AcaciaPlanks
                            } else if wall {
                                if k == 2 && (a + b).rem_euclid(4) == 0 && !(b == -d && a.abs() <= 1) { B::Glass } else if b == -d && a.abs() <= 1 && k < 2 { B::Air } else { B::Bricks }
                            } else if inner && k <= 3 {
                                B::OakPlanks
                            } else {
                                B::Air
                            };
                            if let Some(c) = at(a, b0 + b, k) {
                                cells.push((c, bl));
                            }
                        }
                    }
                }
                for (a, b) in [(-7, -4), (7, -4), (-7, 4), (7, 4), (-3, 2), (3, -2)] {
                    if let Some(c) = at(a, b0 + b, 0) {
                        arena.gens.push(Gen { pos: centre_of(c), item: ids::GOLD_INGOT, every: 10.0, t: 0.0 });
                    }
                    if let Some(c) = at(a, b0 + b, 3) {
                        cells.push((c, B::Lantern));
                    }
                }
                for (a, b) in [(-9, -6), (9, 6), (-9, 6), (9, -6)] {
                    if let Some(c) = at(a, b0 + b, 0) {
                        cells.push((c, B::Bookshelf));
                    }
                }
                if let Some(c) = at(0, b0 - 4, 0) {
                    arena.base = centre_of(c);
                }
            }
            Mode::HideSeek => {
                // A little village: grass, houses with doorways, bushes and barrels to hide behind.
                slab(&mut cells, 0, 18, 14, 0, B::Grass, B::Dirt, 5);
                for (a0, b0) in [(-8, 10), (8, 10), (-8, 26), (8, 26), (0, 18)] {
                    for a in -2i32..=2 {
                        for b in -2i32..=2 {
                            if a.abs() < 2 && b.abs() < 2 {
                                continue;
                            }
                            for k in 0..=3 {
                                let door = b == -2 && a == 0 && k < 2;
                                if let Some(c) = at(a0 + a, b0 + b, k) {
                                    cells.push((c, if door { B::Air } else if k == 3 { B::SprucePlanks } else { B::OakPlanks }));
                                }
                            }
                        }
                    }
                    for a in -1..=1 {
                        for b in -1..=1 {
                            if let Some(c) = at(a0 + a, b0 + b, 3) {
                                cells.push((c, B::SprucePlanks));
                            }
                        }
                    }
                }
                for (a, b, bl) in [(-12, 4, B::OakLeaves), (12, 6, B::OakLeaves), (-3, 30, B::OakLeaves), (5, 22, B::Pumpkin), (-5, 14, B::Pumpkin), (11, 30, B::Chest), (-11, 20, B::Chest)] {
                    if let Some(c) = at(a, b, 0) {
                        cells.push((c, bl));
                    }
                }
                if let Some(c) = at(0, 6, 0) {
                    arena.base = centre_of(c);
                }
            }
        }
        let (n, missed) = crate::worldedit::apply(self, cells);
        if n == 0 {
            return Err("Мир вокруг ещё не загрузился, подойдите ближе или попробуйте через пару секунд".into());
        }
        // Chest loot (Скайварс).
        for (c, rich) in chests {
            let rolls: Vec<f64> = (0..5).map(|_| self.mobs.rand()).collect();
            let loot = chest_loot(rich, &rolls);
            let slots = self.container_slots(c, B::Chest);
            slots.iter_mut().for_each(|x| *x = None);
            let len = slots.len().max(1);
            for (i, (id, cnt)) in loot.into_iter().enumerate() {
                slots[(i * 5 + 2) % len] = Some(ItemStack::new(id, cnt));
            }
        }
        for (id, cnt) in arena.role.kit() {
            self.player.inventory.add(ItemStack::new(id, cnt));
        }
        let base = DVec3::new(arena.base[0], arena.base[1], arena.base[2]);
        let mut msg = format!("{} готов{}", mode.name(), if missed > 0 { " (часть мира ещё грузилась — подойдите и постройте ещё раз, если чего-то не хватает)" } else { "" });
        if arena.role != Role::None {
            msg = format!("{msg}. Ваша роль: {} — {}", arena.role.name(), arena.role.goal());
            self.title = Some((arena.role.name().into(), arena.role.goal().into(), 4.0));
        }
        self.arena = Some(arena);
        self.arena_dead_seen = false;
        self.teleport(base);
        Ok(msg)
    }

    /// «Кликер»: to an island (opens it for money the first time) — the player is taken there.
    pub fn clicker_go(&mut self, i: usize) -> Result<(), String> {
        self.clicker.go_to(i)?;
        if let Some(p) = self.clicker.island_pos.get(i).copied() {
            self.teleport(DVec3::new(p[0], p[1], p[2]));
        }
        Ok(())
    }

    fn handle_building_events(&mut self, events:&mut Vec<InteractionEvent>, used_stack:Option<wolfcraft_engine::items::ItemStack>) {
        use wolfcraft_engine::{construction as c, items::{ItemStack,ids}, block::Block};
        let broken:Vec<_>=events.iter().filter_map(|e|if let InteractionEvent::Broken{pos,block}=e{Some((*pos,*block))}else{None}).collect();
        for (pos,block) in broken {
            if let Some(item)=self.storage_bank.frames.remove(&(self.hell,pos)){events.push(InteractionEvent::Dropped{pos,stack:item});}
            if c::is_portable(block) {
                if let Some(slots)=self.containers.remove(&pos) {
                    if slots.iter().any(Option::is_some) {
                        match self.storage_bank.pack(slots) {
                            Ok(tag)=>{let mut tagged=false;for e in events.iter_mut(){if let InteractionEvent::Dropped{pos:p,stack}=e{if *p==pos&&stack.id==block.base_item() as u16{stack.damage=tag;tagged=true;}}}if !tagged{let mut stack=ItemStack::of_block(block.base_item(),1);stack.damage=tag;events.push(InteractionEvent::Dropped{pos,stack});}},
                            Err(slots)=>for stack in slots.into_iter().flatten(){events.push(InteractionEvent::Dropped{pos,stack});}
                        }
                    }
                }
            }
        }
        let copy=events.clone();
        for event in copy {match event {
            InteractionEvent::Placed{pos,block} if c::is_portable(block)=>{if let Some(item)=used_stack.filter(|s|s.damage>0&&s.info().place.is_some_and(c::is_portable)){if let Some(slots)=self.storage_bank.unpack(item.damage){self.containers.insert(pos,slots);}}},
            InteractionEvent::Slept { night, pos } => self.lie_down(pos, night),
            InteractionEvent::Sat { pos } => {
                let c = self.chunks.grid().cell_center(pos);
                let up = wolfcraft_engine::physics::up_at(c);
                // Face away from where the player came from? Simpler and natural: keep looking
                // the way you look.
                self.start_rest(Rest::Sit { at: c - up * 0.02, dir: self.player.heading, back: self.player.body.pos });
                self.item_name = Some(("Вы сели. Двиньтесь или прыгните, чтобы встать".into(), 2.5));
            }
            InteractionEvent::FrameUse{pos}=>{
                let old=self.storage_bank.frames.remove(&(self.hell,pos));
                if let Some(mut item)=self.player.inventory.held() {
                    item.count=1;
                    if !self.player.is_creative(){let index=self.player.inventory.selected;if let Some(stack)=&mut self.player.inventory.slots[index]{stack.count-=1;if stack.count==0{self.player.inventory.slots[index]=None;}}}
                    self.storage_bank.frames.insert((self.hell,pos),item);
                }
                if let Some(item)=old{if let Some(rest)=self.player.inventory.add(item){self.drops.pop_from_block(rest,self.chunks.grid().cell_center(pos));}}
            },
            InteractionEvent::ActivateBoss{pos,kind}=>{
                if self.cfg.difficulty==wolfcraft_engine::config::Difficulty::Peaceful {self.item_name=Some(("Для боя отключите мирную сложность".into(),3.));continue;}
                if self.hell {self.item_name=Some(("Алтари действуют только на родной планете".into(),3.));continue;}
                if self.mobs.mobs.iter().any(|m|m.boss.is_some()){self.item_name=Some(("Сначала завершите текущий бой".into(),3.));continue;}
                if self.ritual.is_some(){self.item_name=Some(("Ритуал уже идёт".into(),2.));continue;}
                let hp=match self.boss_book.begin(kind,pos){Ok(hp)=>hp,Err(_)=>{
                    // Defeated: a ritual of revival brings the guardian back (seal + resonance rune).
                    let creative=self.player.is_creative();
                    if !creative&&(self.player.inventory.count(ids::RITUAL_SEAL)==0||self.player.inventory.count(ids::RESONANCE_RUNE)==0){
                        self.item_name=Some((format!("{} повержен. Ритуал возрождения: Печать древних + Руна резонанса",kind.name()),5.));continue;
                    }
                    if !creative{self.player.inventory.take(ids::RITUAL_SEAL,1);self.player.inventory.take(ids::RESONANCE_RUNE,1);}
                    self.ritual=Some((kind,pos,0.0));
                    self.item_name=Some(("Ритуал возрождения начался… отойдите от алтаря".into(),4.));
                    continue;
                }};
                if !self.player.is_creative()&&self.player.inventory.count(ids::RITUAL_SEAL)==0{self.item_name=Some(("Для вызова нужна Печать древних из подземелья".into(),4.));continue;}
                if !self.player.is_creative(){self.player.inventory.take(ids::RITUAL_SEAL,1);}
                let anchor=self.chunks.grid().cell_center(pos);let up=wolfcraft_engine::physics::up_at(anchor);let at=anchor+up*1.2;
                let mut mob=wolfcraft_engine::mobs::Mob::new(wolfcraft_engine::mobs::MobKind::Ghoul,at,self.player.heading);
                mob.hp=hp;mob.boss=Some(wolfcraft_engine::bosses::BossRole::new(kind,pos,at));self.mobs.mobs.push(mob);
                self.item_name=Some((format!("{} пробуждается",kind.name()),3.));
            }, _=>{}
        }}
        let _=Block::Air;
    }

    /// Slots for a container block, created on first use: the furnace has input, fuel and
    /// output, the forge item, material and result, chests and barrels 27.
    pub fn container_slots(&mut self, pos: wolfcraft_engine::cubesphere::Cell, block: wolfcraft_engine::block::Block) -> &mut Vec<Option<wolfcraft_engine::items::ItemStack>> {
        use wolfcraft_engine::block::Block as B;
        if wolfcraft_engine::construction::is_shared(block) {return &mut self.storage_bank.shared;}
        let block=block.base_kind();
        let tech_machine = wolfcraft_engine::wolftech::kind(block).is_some_and(wolfcraft_engine::wolftech::is_machine);
        let reactor = wolfcraft_engine::wolftech::kind(block) == Some(wolfcraft_engine::wolftech::Kind::Reactor);
        let kin_machine = wolfcraft_engine::kinetic::part(block).is_some_and(wolfcraft_engine::kinetic::is_machine);
        let n = if reactor { wolfcraft_engine::wolftech::REACTOR_SLOTS } else if tech_machine { 7 } else if kin_machine { 3 } else if matches!(block, B::ElectricFurnace | B::MachineCasing) || wolfcraft_engine::wolftech::kind(block).is_some() { 3 } else if block == B::EnchantTable { 2 } else { 27 };
        let v = self.containers.entry(pos).or_insert_with(|| {
            let mut slots=vec![None;n];
            // Player-placed chests use construction IDs; legacy chest IDs identify generated loot.
            if block==B::Chest {
                use wolfcraft_engine::items::{ids,ItemStack};
                slots[0]=Some(ItemStack::new(ids::RITUAL_SEAL,1));
                slots[1]=Some(ItemStack::of_block(B::IronOre,3));
                slots[2]=Some(ItemStack::of_block(B::Torch,8));
            } slots
        });
        // WolfTech machines from before the upgrade slots: four more on the right.
        if tech_machine && v.len() < 7 {
            v.resize(7, None);
        }
        // Reactors from before the core grid: 18 slots.
        if reactor && v.len() < wolfcraft_engine::wolftech::REACTOR_SLOTS {
            v.resize(wolfcraft_engine::wolftech::REACTOR_SLOTS, None);
        }
        // Furnaces from before the fuel slot: [input, output] → [input, fuel, output].
        if block == B::ElectricFurnace && v.len() == 2 {
            let out = v.pop().flatten();
            v.push(None);
            v.push(out);
        }
        v
    }

    /// WolfTech: (progress 0..1, working, energy, capacity) of a machine.
    pub fn tech_status(&self, pos: wolfcraft_engine::cubesphere::Cell, block: wolfcraft_engine::block::Block) -> Option<(f32, bool, f32, f32)> {
        let k = wolfcraft_engine::wolftech::kind(block)?;
        let sp = wolfcraft_engine::wolftech::spec(k);
        let st = self.tech.get(&pos).copied().unwrap_or_default();
        let (tm, _, extra) = self.tech_mods(pos, block);
        let time = sp.time * tm;
        let prog = if time > 0.0 { st.progress / time } else if st.burn > 0.0 { 1.0 } else { 0.0 };
        Some((prog.clamp(0.0, 1.0), st.working, st.energy, sp.capacity + extra))
    }

    /// WolfTech upgrades in the machine at `pos`: (time ×, energy use ×, extra store).
    pub fn tech_mods(&self, pos: wolfcraft_engine::cubesphere::Cell, block: wolfcraft_engine::block::Block) -> (f32, f32, f32) {
        if !wolfcraft_engine::wolftech::kind(block).is_some_and(wolfcraft_engine::wolftech::is_machine) {
            return (1.0, 1.0, 0.0);
        }
        self.containers.get(&pos).map(|v| wolfcraft_engine::wolftech::upgrades(v.get(3..).unwrap_or(&[]))).unwrap_or((1.0, 1.0, 0.0))
    }

    /// WolfTech: what the network of the machine at `pos` makes (ЭЕ/s), holds and how many
    /// machines it has.
    pub fn tech_net(&self, pos: wolfcraft_engine::cubesphere::Cell) -> (f32, f32, usize) {
        let Some(net) = self.tech_nets.iter().find(|n| n.contains(&pos)) else { return (0.0, 0.0, 1) };
        let mut make = 0.0;
        let mut held = 0.0;
        for c in net {
            if let Some(st) = self.tech.get(c) {
                held += st.energy;
                if self.chunks.get_block(*c).and_then(wolfcraft_engine::wolftech::kind).is_some_and(wolfcraft_engine::wolftech::is_source) {
                    make += st.flow;
                }
            }
        }
        (make, held, net.len())
    }

    /// WolfTech: generators burn and shine, machines work, energy flows through the cables.
    fn tick_tech(&mut self, dt: f32) {
        use wolfcraft_engine::items::ItemStack;
        use wolfcraft_engine::wolftech::{self as wt, Kind};
        if self.tech.is_empty() {
            return;
        }
        let g = self.chunks.grid();
        self.tech_t -= dt;
        if self.tech_t <= 0.0 {
            self.tech_t = 1.0;
            // Forget machines that were broken; find the networks (machines joined by cables
            // or touching each other).
            let cells: Vec<_> = self.tech.keys().copied().collect();
            for c in &cells {
                if let Some(b) = self.chunks.get_block(*c) {
                    if wt::kind(b).is_none() {
                        self.tech.remove(c);
                    }
                }
            }
            let mut seen = std::collections::HashSet::new();
            self.tech_nets.clear();
            for c in self.tech.keys().copied().collect::<Vec<_>>() {
                if seen.contains(&c) {
                    continue;
                }
                let mut net = Vec::new();
                let mut queue = vec![c];
                seen.insert(c);
                while let Some(x) = queue.pop() {
                    if self.tech.contains_key(&x) {
                        net.push(x);
                    }
                    if seen.len() > 4096 {
                        break;
                    }
                    for (a, b, k) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                        let Some(n) = g.offset(x, a, b, k) else { continue };
                        if seen.contains(&n) {
                            continue;
                        }
                        if let Some(nb) = self.chunks.get_block(n).filter(|b| wt::conducts(*b)) {
                            // A machine nobody registered yet (placed by a script, an older
                            // version, another player): it joins the network now.
                            if wt::kind(nb).is_some() {
                                self.tech.entry(n).or_default();
                            }
                            seen.insert(n);
                            queue.push(n);
                        }
                    }
                }
                self.tech_nets.push(net);
            }
            // Solar panels: daylight and open sky, checked once a second.
            let day = self.daylight() > 0.45 && self.weather.rain < 0.5;
            let night = self.daylight() <= 0.45;
            for (c, st) in self.tech.iter_mut() {
                let here = self.chunks.get_block(*c);
                if here == Some(wolfcraft_engine::block::Block::SolarPanel) {
                    st.day = day;
                    st.sunny = day && (1..40).all(|k| g.offset(*c, 0, 0, k).and_then(|n| self.chunks.get_block(n)).is_none_or(|b| !b.is_opaque()));
                } else if here == Some(wolfcraft_engine::block::Block::LunarPanel) {
                    // The lunar panel: `sunny` means open sky here (it works at night).
                    st.day = !night;
                    st.sunny = (1..40).all(|k| g.offset(*c, 0, 0, k).and_then(|n| self.chunks.get_block(n)).is_none_or(|b| !b.is_opaque()));
                }
            }
            // Wind, lava heat and water: what is around them, checked once a second.
            let rain = self.weather.rain;
            let sea = self.planet.radius - 1.0;
            for (c, st) in self.tech.iter_mut() {
                let Some(k) = self.chunks.get_block(*c).and_then(wt::kind) else { continue };
                let around = |f: fn(wolfcraft_engine::block::Block) -> bool| {
                    [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].iter().filter(|(a, b, kk)| g.offset(*c, *a, *b, *kk).and_then(|n| self.chunks.get_block(n)).is_some_and(f)).count() as f32
                };
                st.env = match k {
                    Kind::Wind => {
                        let open = (1..20).all(|kk| g.offset(*c, 0, 0, kk).and_then(|n| self.chunks.get_block(n)).is_none_or(|b| !b.is_opaque()));
                        let alt = (wolfcraft_engine::cubesphere::radial(g.cell_center(*c)) - sea).max(0.0) as f32;
                        if open { (0.25 + (alt / 40.0).min(1.0) * 0.75) * (1.0 + 1.5 * rain) } else { 0.1 }
                    }
                    Kind::Geo => around(|b| b.is_lava()),
                    Kind::Water => around(|b| b.is_water()),
                    Kind::Extruder => (around(|b| b.is_lava()) > 0.0 && around(|b| b.is_water()) > 0.0) as u8 as f32,
                    Kind::Glacier => around(|b| b.is_water()),
                    Kind::Smeltery => around(|b| b.is_lava()),
                    _ => st.env,
                };
            }
        }
        // Each machine on its own: fuel, recipes, progress.
        let creative = self.player.is_creative();
        let mut meltdowns = Vec::new();
        let cells: Vec<_> = self.tech.keys().copied().collect();
        for c in cells {
            let Some(b) = self.chunks.get_block(c) else { continue };
            let Some(k) = wt::kind(b) else { continue };
            let sp = wt::spec(k);
            let mut st = self.tech[&c];
            st.working = false;
            match k {
                Kind::Wind | Kind::Geo | Kind::Water => {
                    st.flow = sp.rate * st.env;
                    st.working = st.flow > 0.0;
                }
                // Quarry: one block of its 5×5 shaft every `time` seconds, into its slots.
                Kind::Quarry => {
                    st.flow = sp.rate;
                    st.burn += dt;
                    let cost = 40.0;
                    if st.burn >= sp.time && st.energy >= cost {
                        st.burn = 0.0;
                        let full = self.containers.get(&c).is_some_and(|v| v.iter().all(|s| s.is_some_and(|s| s.count >= s.info().max_stack)));
                        if !full {
                            if let Some((cell, block)) = self.quarry_next(c, &mut st.progress) {
                                let seed = (self.mobs.rand() * 1e9) as u64;
                                let loot = wolfcraft_engine::items::drops(block, true, glam::IVec3::new(cell.i, cell.j, cell.k), seed);
                                crate::worldedit::apply(self, vec![(cell, wolfcraft_engine::block::Block::Air)]);
                                let centre = self.chunks.grid().cell_center(c);
                                for d in loot {
                                    let v = self.container_slots(c, b);
                                    let mut left = d.count;
                                    for i in wolfcraft_engine::pipes::order(v, (0..v.len()).collect(), d.id) {
                                        left -= wolfcraft_engine::pipes::insert(v, i, d, left);
                                        if left == 0 {
                                            break;
                                        }
                                    }
                                    if left > 0 {
                                        self.drops.pop_from_block(wolfcraft_engine::items::ItemStack { count: left, ..d }, centre);
                                    }
                                }
                                st.energy -= cost;
                                st.working = true;
                            }
                        }
                    }
                    st.burn = st.burn.min(sp.time);
                }
                // Magnet: twice a second, dropped items within 8 blocks fly into its slots.
                Kind::Magnet => {
                    st.flow = sp.rate;
                    st.burn += dt;
                    if st.burn >= sp.time {
                        st.burn = 0.0;
                        let centre = self.chunks.grid().cell_center(c);
                        let mut i = 0;
                        while i < self.drops.list.len() && st.energy >= 2.0 {
                            let d = &self.drops.list[i];
                            if d.delay > 0.0 || (d.pos - centre).length_squared() > 64.0 {
                                i += 1;
                                continue;
                            }
                            let stack = d.stack;
                            let v = self.container_slots(c, b);
                            let mut left = stack.count;
                            for k in wolfcraft_engine::pipes::order(v, (0..v.len()).collect(), stack.id) {
                                left -= wolfcraft_engine::pipes::insert(v, k, stack, left);
                                if left == 0 {
                                    break;
                                }
                            }
                            st.energy -= 2.0;
                            st.working = true;
                            if left == 0 {
                                self.drops.list.swap_remove(i);
                            } else {
                                self.drops.list[i].stack.count = left;
                                i += 1;
                            }
                        }
                    }
                }
                // «Механика»: the dynamo gives what the shaft turns into it (env, set by the
                // kinetic tick); the motor spends energy and turns its shafts while it has it.
                Kind::Dynamo => {
                    st.flow = st.env;
                    st.working = st.flow > 0.0;
                }
                Kind::Motor => {
                    // The 20 Hz kinetic simulation pays for rotation, including catch-up steps.
                    // Never mark a motor powered merely because it coasted without energy.
                    st.flow = if st.working { sp.rate } else { 0.0 };
                }
                Kind::Reactor => {
                    // Once a second: the rods pulse, the hull heats or cools (heat = progress).
                    st.burn += dt;
                    if st.burn >= 1.0 {
                        st.burn -= 1.0;
                        let v = self.container_slots(c, b);
                        let t = wt::reactor_second(v, st.progress);
                        st.progress = (st.progress + t.heat).max(0.0);
                        st.env = t.power;
                        if st.progress >= t.max_heat {
                            meltdowns.push(c);
                        }
                    }
                    st.flow = st.env;
                    st.working = st.flow > 0.0;
                }
                Kind::Generator | Kind::Magma => {
                    if st.burn <= 0.0 && st.energy < sp.capacity * 0.95 {
                        let v = self.container_slots(c, b);
                        if let Some(f) = v.get(1).copied().flatten().filter(|f| wt::fuel_for(k, f.id) > 0.0) {
                            st.burn = wt::fuel_for(k, f.id);
                            v[1] = if f.count > 1 { Some(ItemStack { count: f.count - 1, ..f }) } else { None };
                        }
                    }
                    // Creative: the generator runs without fuel.
                    if creative && st.burn <= 0.0 {
                        st.burn = 1.0;
                    }
                    if st.burn > 0.0 {
                        st.burn -= dt;
                        st.flow = sp.rate;
                        st.working = true;
                    } else {
                        st.flow = 0.0;
                    }
                }
                Kind::Solar => {
                    // Full power in the open sun, a quarter in the shade by day, nothing at night.
                    st.flow = if st.sunny { sp.rate } else if st.day { sp.rate * 0.25 } else { 0.0 };
                    st.working = st.flow > 0.0;
                }
                Kind::Lunar => {
                    st.flow = if st.day { 0.0 } else if st.sunny { sp.rate } else { sp.rate * 0.25 };
                    st.working = st.flow > 0.0;
                }
                // Tesla coil: once a second, a lightning bolt at the nearest hostile creature.
                Kind::Tesla => {
                    st.flow = sp.rate;
                    st.burn += dt;
                    if st.burn >= sp.time {
                        st.burn = 0.0;
                        let centre = self.chunks.grid().cell_center(c);
                        let target = self.mobs.mobs.iter().enumerate().filter(|(_, m)| m.kind.hostile() && m.hp > 0.0 && (m.body.pos - centre).length_squared() < 49.0).min_by(|a, b| (a.1.body.pos - centre).length_squared().total_cmp(&(b.1.body.pos - centre).length_squared())).map(|(i, m)| (i, m.body.pos));
                        if let (Some((i, at)), true) = (target, st.energy >= 60.0) {
                            st.energy -= 60.0;
                            st.working = true;
                            self.mobs.hurt(i, 8.0, centre);
                            // The bolt: bright sparks along the line from the coil to the target.
                            for k in 0..6 {
                                let p = centre + (at - centre) * (k as f64 / 5.0);
                                self.mobs.burst(p, &[[0.55, 0.85, 1.0], [0.95, 0.98, 1.0]], 0.25);
                            }
                        }
                    }
                }
                // Charging pad: the player standing on it gets the held tool and electric armour
                // charged, 5 ЭЕ per point.
                Kind::ChargePad => {
                    st.flow = sp.rate;
                    let g = self.chunks.grid();
                    let on = g.offset(c, 0, 0, 1) == Some(g.world_to_cell(self.player.body.pos));
                    if on && st.energy >= 5.0 {
                        let mut budget = ((st.energy / 5.0).floor() as u16).min((60.0 * dt).ceil() as u16);
                        let inv = &mut self.player.inventory;
                        let sel = inv.selected;
                        let gear = inv.slots.get_mut(sel).into_iter().flatten().chain(inv.armor.iter_mut().flatten());
                        for s in gear {
                            if budget == 0 {
                                break;
                            }
                            if wolfcraft_engine::items::rechargeable(s.id) && s.damage > 0 {
                                let pts = budget.min(s.damage);
                                s.damage -= pts;
                                budget -= pts;
                                st.energy -= pts as f32 * 5.0;
                                st.working = true;
                            }
                        }
                    }
                }
                Kind::Cell | Kind::BigCell | Kind::DracoCore => {
                    st.flow = 0.0;
                    // Charges WolfTech gear lying in its first slot: 5 ЭЕ per point of charge.
                    let v = self.container_slots(c, b);
                    if let Some(g) = v.first_mut().and_then(|g| g.as_mut()).filter(|g| wolfcraft_engine::items::rechargeable(g.id) && g.damage > 0) {
                        let pts = ((st.energy / 5.0).floor() as u16).min(g.damage).min((40.0 * dt).ceil() as u16);
                        g.damage -= pts;
                        st.energy -= pts as f32 * 5.0;
                        st.working = pts > 0;
                    }
                }
                _ => {
                    let v = self.container_slots(c, b).clone();
                    // Upgrades: faster and hungrier with «Ускорители».
                    let (tm, em, _) = wt::upgrades(v.get(3..).unwrap_or(&[]));
                    let sp = wt::Spec { time: sp.time * tm, rate: sp.rate * em, ..sp };
                    let inp = v.first().copied().flatten();
                    let out = match inp {
                        Some(i) => Some(i).filter(|i| i.count >= wt::need(k, i.id)).and_then(|i| wt::recipe(k, i.id)),
                        None => wt::idle_recipe(k),
                    };
                    let take = if wt::keeps(k) { 0 } else { inp.map(|i| wt::need(k, i.id)).unwrap_or(1) };
                    let sp = wt::Spec { time: sp.time * inp.map(|i| wt::slow(k, i.id)).unwrap_or(1.0), ..sp };
                    let room = wt::env_ok(k, st.env) && out.is_some_and(|(o, n)| match v.get(2).copied().flatten() {
                        None => true,
                        Some(s) => s.id == o && s.count as u32 + n as u32 <= 64,
                    });
                    let need = sp.rate * dt;
                    if room && st.energy >= need {
                        st.energy -= need;
                        st.progress += dt;
                        st.flow = sp.rate;
                        st.working = true;
                        if st.progress >= sp.time {
                            st.progress = 0.0;
                            let (o, n) = out.unwrap();
                            let v = self.container_slots(c, b);
                            if let Some(i) = v[0].as_mut() {
                                i.count = i.count.saturating_sub(take);
                                if i.count == 0 {
                                    v[0] = None;
                                }
                            }
                            match v[2].as_mut() {
                                Some(s) => s.count += n,
                                None => v[2] = Some(ItemStack::new(o, n)),
                            }
                        }
                    } else {
                        st.flow = if room { sp.rate } else { 0.0 };
                        if !room {
                            st.progress = 0.0;
                        }
                    }
                }
            }
            self.tech.insert(c, st);
        }
        // Reactors: smoke and sparks when hot, a meltdown when the hull gives way.
        self.reactor_fx(meltdowns, dt);
        // Working machines show it: smoke, sparks, dust, glow.
        self.tech_fx(dt);
        // Energy flows inside each network.
        for net in &self.tech_nets {
            let mut nodes = Vec::with_capacity(net.len());
            let mut keep = Vec::with_capacity(net.len());
            for c in net {
                let (Some(b), Some(st)) = (self.chunks.get_block(*c), self.tech.get(c)) else { continue };
                let Some(k) = wt::kind(b) else { continue };
                let cap = wt::spec(k).capacity + self.tech_mods(*c, b).2;
                nodes.push(wt::Node { kind: k, cap, energy: st.energy, flow: st.flow });
                keep.push(*c);
            }
            let e = wt::share(&nodes, dt);
            for (c, v) in keep.iter().zip(e) {
                if let Some(st) = self.tech.get_mut(c) {
                    st.energy = v;
                }
            }
        }
    }

    /// The next goal for a newcomer in survival: the first starter achievement not yet got
    /// (wood → workbench → pickaxe → stone → iron → diamond → WolfTech), shown at the top.
    pub fn goal(&self) -> Option<&'static crate::achievements::Achievement> {
        if self.player.is_creative() || self.studio {
            return None;
        }
        ["wood", "bench", "pick", "stone", "iron", "diamond", "circuit", "power"]
            .iter()
            .find(|id| !self.achieved.contains(**id))
            .and_then(|id| crate::achievements::find(id))
            .map(|(_, a)| a)
    }

    /// Controls for the first half minute in a brand-new world.
    pub fn first_steps(&self) -> bool {
        self.achieved.is_empty() && self.clock < 30.0
    }

    /// «Механика»: solves the turning networks twenty times a second (sources → shafts and cogs
    /// → machines, overload stops everything), runs the machines and the cranks.
    fn tick_kinetic(&mut self, events: &[InteractionEvent], dt: f32) {
        use wolfcraft_engine::kinetic as kn;
        for e in events {
            match e {
                InteractionEvent::Placed { pos, block } if kn::part(*block).is_some() => {
                    self.kin.entry(*pos).or_default();
                    self.kin_t = 0.0;
                }
                InteractionEvent::OpenContainer { pos, block } if kn::part(*block).is_some() => {
                    self.kin.entry(*pos).or_default();
                    self.kin_t = 0.0;
                }
                InteractionEvent::Crank { pos } => {
                    self.kin.entry(*pos).or_default().crank = 6.0;
                    self.kin_t = 0.0;
                }
                InteractionEvent::Broken { .. } | InteractionEvent::Blasted { .. } => self.kin_t = 0.0,
                _ => {}
            }
        }
        if !dt.is_finite() || dt <= 0.0 { return; }
        self.kin_accum += dt as f64;
        // Keep time debt rather than discarding simulation time on a slow frame.
        let steps = (((self.kin_accum + 0.000001) / 0.05).floor() as usize).min(100);
        for _ in 0..steps {
            self.kin_accum -= 0.05;
            self.step_kinetic(0.05);
        }
    }

    fn step_kinetic(&mut self, dt: f32) {
        use wolfcraft_engine::items::ItemStack;
        use wolfcraft_engine::kinetic::{self as kn, Part};
        if self.kin.is_empty() {
            return;
        }
        let motors: Vec<_> = self.kin.keys().copied().filter(|c| self.chunks.get_block(*c) == Some(wolfcraft_engine::block::Block::ElectricMotor)).collect();
        for c in motors {
            let t = self.tech.entry(c).or_default();
            let need = wolfcraft_engine::wolftech::spec(wolfcraft_engine::wolftech::Kind::Motor).rate * dt;
            t.working = t.energy + 0.00001 >= need;
            if t.working { t.energy = (t.energy - need).max(0.0); }
        }
        for n in self.kin.values_mut() {
            n.crank = (n.crank - dt).max(0.0);
        }
        self.kin_t -= dt;
        if self.kin_t <= 0.0 {
            self.kin_t = 0.05;
            let g = self.chunks.grid();
            let cells: Vec<_> = self.kin.keys().copied().collect();
            for c in &cells {
                if self.chunks.get_block(*c).is_some_and(|b| kn::part(b).is_none()) {
                    self.kin.remove(c);
                }
            }
            let mut seen: std::collections::HashMap<wolfcraft_engine::cubesphere::Cell, i8> = std::collections::HashMap::new();
            let mut start: Vec<_> = self.kin.keys().copied().collect();
            start.sort_by_key(|c| (c.f, c.i, c.j, c.k));
            for c0 in start {
                if seen.contains_key(&c0) {
                    continue;
                }
                let Some(p0) = self.chunks.get_block(c0).and_then(kn::part) else { continue };
                let _ = p0;
                let mut net = vec![(c0, 1i8)];
                seen.insert(c0, 1);
                let mut i = 0;
                let mut jammed = false;
                while i < net.len() {
                    let (c, sg) = net[i];
                    i += 1;
                    let Some(pc) = self.chunks.get_block(c).and_then(kn::part) else { continue };
                    for (ax, s) in [(0u8, 1i8), (0, -1), (1, 1), (1, -1), (2, 1), (2, -1)] {
                        let d = s as i32;
                        let (a, b, k) = match ax {
                            0 => (d, 0, 0),
                            1 => (0, d, 0),
                            _ => (0, 0, d),
                        };
                        let Some(n) = g.offset(c, a, b, k) else { continue };
                        let Some(pn) = self.chunks.get_block(n).and_then(kn::part) else { continue };
                        if let Some(flip) = kn::link(pc, pn, (ax, s)) {
                            let ns = if flip { -sg } else { sg };
                            if let Some(previous) = seen.get(&n) {
                                if *previous != ns { jammed = true; }
                            } else {
                                seen.insert(n, ns);
                                net.push((n, ns));
                            }
                        }
                    }
                }
                // Sources and load of this network.
                let (mut speed, mut cap, mut load) = (0.0f32, 0.0f32, 0.0f32);
                let mut motor_input = 0.0f32;
                let mut motor_cap = 0.0f32;
                for (c, _) in &net {
                    let Some(p) = self.chunks.get_block(*c).and_then(kn::part) else { continue };
                    match p {
                        Part::Wheel(_) => {
                            let water = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].iter().filter(|(a, b, k)| g.offset(*c, *a, *b, *k).and_then(|n| self.chunks.get_block(n)).is_some_and(|b| b.is_water())).count();
                            if water > 0 {
                                speed = speed.max((8.0 + 4.0 * water as f32).min(32.0));
                                cap += 256.0;
                            }
                        }
                        Part::Crank => {
                            if self.kin.get(c).is_some_and(|n| n.crank > 0.0) {
                                speed = speed.max(32.0);
                                cap += 96.0;
                            }
                        }
                        Part::Sail => {
                            // Open sky above: the sails catch the wind (a storm doubles it).
                            let open = (1..12).all(|kk| g.offset(*c, 0, 0, kk).and_then(|n| self.chunks.get_block(n)).is_none_or(|b| !b.is_opaque()));
                            if open {
                                speed = speed.max(if self.weather.rain > 0.5 { 32.0 } else { 16.0 });
                                cap += 384.0;
                            }
                        }
                        Part::Motor => {
                            if self.tech.get(c).is_some_and(|t| t.working) {
                                speed = speed.max(64.0);
                                cap += 512.0;
                                motor_cap += 512.0;
                                motor_input += wolfcraft_engine::wolftech::spec(wolfcraft_engine::wolftech::Kind::Motor).rate;
                            }
                        }
                        Part::Gearbox => {}
                        p => load += kn::load(p),
                    }
                }
                let over = load > cap && speed > 0.0;
                let run = if over || jammed { 0.0 } else { speed };
                let dynamos = net.iter().filter(|(c, _)| self.chunks.get_block(*c) == Some(wolfcraft_engine::block::Block::KineticDynamo)).count();
                let output = wolfcraft_engine::kinetic::dynamo_output(run, cap - motor_cap, motor_input, dynamos);
                // Dynamos share the energy budget of the drive, with conversion losses.
                for (c, _) in &net {
                    if self.chunks.get_block(*c) == Some(wolfcraft_engine::block::Block::KineticDynamo) {
                        self.tech.entry(*c).or_default().env = output;
                    }
                }
                for (c, sg) in &net {
                    let n = self.kin.entry(*c).or_default();
                    n.speed = run;
                    n.sign = *sg;
                    n.load = load;
                    n.cap = cap;
                }
            }
        }
        // Machines: two seconds an item at 16 RPM, faster when turning faster.
        let machines: Vec<_> = self.kin.iter().filter(|(_, n)| n.speed > 0.0).map(|(c, _)| *c).collect();
        for c in machines {
            let Some(b) = self.chunks.get_block(c) else { continue };
            let Some(p) = kn::part(b).filter(|p| kn::is_machine(*p)) else { continue };
            let speed = self.kin[&c].speed;
            self.kin.get_mut(&c).unwrap().progress += dt * speed / 16.0;
            loop {
                let v = self.container_slots(c, b).clone();
                let out = v.first().copied().flatten().and_then(|i| kn::recipe(p, i.id));
                let room = out.is_some_and(|(o, n)| match v.get(2).copied().flatten() {
                    None => n <= wolfcraft_engine::items::info(o).max_stack,
                    Some(s) => s.id == o && s.count as u32 + n as u32 <= s.info().max_stack as u32,
                });
                if !room { self.kin.get_mut(&c).unwrap().progress = 0.0; break; }
                if self.kin[&c].progress + 0.00001 < 2.0 { break; }
                self.kin.get_mut(&c).unwrap().progress = (self.kin[&c].progress - 2.0).max(0.0);
                let (o, n) = out.unwrap();
                let v = self.container_slots(c, b);
                if let Some(i) = v[0].as_mut() {
                    i.count -= 1;
                    if i.count == 0 { v[0] = None; }
                }
                match v[2].as_mut() {
                    Some(s) => s.count += n,
                    None => v[2] = Some(ItemStack::new(o, n)),
                }
            }
        }
    }

    /// The spinning parts near the player, for the renderer: centre, the axis and two
    /// directions across it (world), the kind (0 shaft, 1 cog, 2 wheel, 3 crank) and the angle.
    pub fn kinetic_models(&self) -> Vec<(DVec3, DVec3, DVec3, DVec3, u8, f32)> {
        use wolfcraft_engine::kinetic::{self as kn, Part};
        let g = self.chunks.grid();
        let me = self.player.body.pos;
        let mut out = Vec::new();
        for (c, n) in &self.kin {
            let Some(p) = self.chunks.get_block(*c).and_then(kn::part) else { continue };
            let centre = g.cell_center(*c);
            if (centre - me).length() > 64.0 {
                continue;
            }
            let (kind, ax) = match p {
                Part::Shaft(a) => (0u8, a),
                Part::Cog(a) => (1, a),
                Part::Wheel(a) => (2, a),
                Part::Crank => (3, 2),
                _ => continue,
            };
            let dir = |a: u8| {
                let (x, y, z) = match a {
                    0 => (1, 0, 0),
                    1 => (0, 1, 0),
                    _ => (0, 0, 1),
                };
                g.offset(*c, x, y, z).map(|o| (g.cell_center(o) - centre).normalize()).unwrap_or(DVec3::Y)
            };
            let (ea, eu, ev) = (dir(ax), dir((ax + 1) % 3), dir((ax + 2) % 3));
            // Turns per second = RPM / 60; cogs side by side get half a tooth of offset.
            let phase = if kind == 1 { ((c.i + c.j + c.k).rem_euclid(2)) as f32 * 0.39 } else { 0.0 };
            let angle = n.sign as f32 * self.clock * n.speed / 60.0 * std::f32::consts::TAU + phase;
            out.push((centre, ea, eu, ev, kind, angle));
        }
        out
    }

    fn save_pipes(&self) {
        let list: Vec<String> = self.pipes.iter().map(|c| format!("{} {} {} {}", c.f, c.i, c.j, c.k)).collect();
        let _ = wolfcraft_engine::storage::write_safe(&self.dir.join("pipes.txt"), list.join("\n").as_bytes());
    }

    /// WolfTech item pipes, twice a second: each network takes what its machines made (and
    /// what extracting pipes pull out of chests) and hands it on — to a machine that can work
    /// it or burn it first, then into a chest — up to 4 items per source each push.
    fn tick_pipes(&mut self, events: &[InteractionEvent], dt: f32) {
        use wolfcraft_engine::block::Block;
        use wolfcraft_engine::pipes::{self as pp, Port};
        for e in events {
            if let InteractionEvent::Placed { pos, block } = e {
                if pp::is_pipe(*block) {
                    self.pipes.insert(*pos);
                }
            }
        }
        if self.pipes.is_empty() {
            return;
        }
        self.pipe_t -= dt;
        if self.pipe_t > 0.0 {
            return;
        }
        self.pipe_t = 0.5;
        let g = self.chunks.grid();
        let gone: Vec<_> = self.pipes.iter().copied().filter(|c| !self.chunks.get_block(*c).is_some_and(pp::is_pipe)).collect();
        for c in gone {
            self.pipes.remove(&c);
        }
        let dirs = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];
        let mut seen = std::collections::HashSet::new();
        for start in self.pipes.iter().copied().collect::<Vec<_>>() {
            if !seen.insert(start) {
                continue;
            }
            // One network: its pipes and the inventories round them.
            let mut ports: Vec<(wolfcraft_engine::cubesphere::Cell, Block, Port, bool)> = Vec::new();
            let mut queue = vec![start];
            while let Some(x) = queue.pop() {
                let extract = self.chunks.get_block(x) == Some(Block::PipeExtractor);
                for (a, b, k) in dirs {
                    let Some(n) = g.offset(x, a, b, k) else { continue };
                    let Some(nb) = self.chunks.get_block(n) else { continue };
                    if pp::is_pipe(nb) {
                        if seen.insert(n) {
                            self.pipes.insert(n);
                            queue.push(n);
                        }
                    } else if let Some(p) = pp::port(nb) {
                        match ports.iter_mut().find(|q| q.0 == n) {
                            Some(q) => q.3 |= extract,
                            None => ports.push((n, nb, p, extract)),
                        }
                    }
                }
                if seen.len() > 4096 {
                    break;
                }
            }
            if ports.len() < 2 {
                continue;
            }
            // Machines and furnaces before chests.
            ports.sort_by_key(|q| matches!(q.2, Port::Store));
            for si in 0..ports.len() {
                let (sc, sb, sp, ex) = ports[si];
                let src = self.container_slots(sc, sb).clone();
                let mut budget = 4u8;
                for slot in pp::outputs(sp, ex, src.len()) {
                    let Some(st) = src[slot] else { continue };
                    let mut left = st.count.min(budget);
                    for (di, &(dc, db, dp, _)) in ports.iter().enumerate() {
                        if di == si || left == 0 || (matches!(sp, Port::Store) && matches!(dp, Port::Store)) {
                            continue;
                        }
                        let dst = self.container_slots(dc, db);
                        let cand = pp::order(dst, pp::inputs(dp, st.id, dst.len()), st.id);
                        for i in cand {
                            left -= pp::insert(dst, i, st, left);
                            if left == 0 {
                                break;
                            }
                        }
                    }
                    let moved = st.count.min(budget) - left;
                    if moved > 0 {
                        budget -= moved;
                        let v = self.container_slots(sc, sb);
                        v[slot] = if st.count > moved { Some(wolfcraft_engine::items::ItemStack { count: st.count - moved, ..st }) } else { None };
                    }
                    if budget == 0 {
                        break;
                    }
                }
            }
        }
    }

    fn save_kinetic(&self) {
        let list: Vec<String> = self.kin.keys().map(|c| format!("{} {} {} {}", c.f, c.i, c.j, c.k)).collect();
        let _ = wolfcraft_engine::storage::write_safe(&self.dir.join("kinetic.txt"), list.join("\n").as_bytes());
    }

    /// Grants an achievement once per world: a golden toast pops on the left.
    pub fn grant(&mut self, id: &str) {
        if self.achieved.contains(id) {
            return;
        }
        let Some((i, _)) = crate::achievements::find(id) else { return };
        self.achieved.insert(id.to_string());
        self.ach_toasts.push((i, 0.0));
        self.ach_new += 1;
        let list: Vec<&str> = self.achieved.iter().map(|s| s.as_str()).collect();
        let _ = wolfcraft_engine::storage::write_safe(&self.dir.join("achievements.txt"), list.join("\n").as_bytes());
    }

    fn tick_achievements(&mut self, events: &[InteractionEvent], dt: f32) {
        for t in &mut self.ach_toasts {
            t.1 += dt;
        }
        // One toast at a time: the next waits until the first has slid away.
        if self.ach_toasts.first().is_some_and(|t| t.1 > 4.5) {
            self.ach_toasts.remove(0);
        }
        for e in events {
            if let InteractionEvent::Placed { block, .. } = e {
                if let Some(id) = crate::achievements::for_placed(*block) {
                    self.grant(id);
                }
            }
        }
        self.ach_t -= dt;
        if self.ach_t > 0.0 {
            return;
        }
        self.ach_t = 1.0;
        let held: Vec<_> = self.player.inventory.slots.iter().flatten().filter_map(|s| crate::achievements::for_item(s.id)).collect();
        for id in held {
            self.grant(id);
        }
        if self.player.jetting {
            self.grant("jetpack");
        }
        if self.player.inventory.armor[1].is_some_and(|s| s.id == wolfcraft_engine::items::ids::NANO_BASE + 1) {
            self.grant("nano");
        }
        if self.riding.is_some() {
            self.grant("dragon");
        }
        if self.hell {
            self.grant("hell");
        }
    }

    /// Hull heat and its limit of the reactor at `pos`.
    pub fn reactor_heat(&self, pos: wolfcraft_engine::cubesphere::Cell) -> (f32, f32) {
        let plating = self.containers.get(&pos).map(|v| v.iter().flatten().filter(|s| s.id == wolfcraft_engine::items::ids::REACTOR_PLATING).count()).unwrap_or(0);
        (self.tech.get(&pos).map(|t| t.progress).unwrap_or(0.0), wolfcraft_engine::wolftech::REACTOR_MAX_HEAT + 1_000.0 * plating as f32)
    }

    fn reactor_fx(&mut self, meltdowns: Vec<wolfcraft_engine::cubesphere::Cell>, dt: f32) {
        use wolfcraft_engine::mobs::FxKind;
        let hot: Vec<_> = self
            .tech
            .keys()
            .copied()
            .filter(|c| self.chunks.get_block(*c) == Some(wolfcraft_engine::block::Block::Reactor))
            .map(|c| (c, self.reactor_heat(c)))
            .filter(|(_, (h, m))| *h > m * 0.6)
            .collect();
        for (c, (h, m)) in hot {
            let f = h / m;
            let p = self.chunks.grid().cell_center(c);
            let up = wolfcraft_engine::physics::up_at(p);
            if self.mobs.rand() < (f as f64 - 0.5) * 6.0 * dt as f64 * 10.0 {
                self.mobs.fx(p + up * 0.6, up * 1.4, [0.30, 0.30, 0.32], 0.25, 2.0, FxKind::Rise);
            }
            if f > 0.85 {
                if self.mobs.rand() < 0.3 {
                    self.mobs.fx_glow(p + up * 0.55, up * 2.0, [1.0, 0.55, 0.15], 0.10, 0.6, FxKind::Rise);
                }
                if (self.player.body.pos - p).length() < 40.0 && self.item_name.as_ref().is_none_or(|n| !n.0.starts_with("⚠")) {
                    self.item_name = Some((format!("⚠ Реактор перегревается: {:.0} %", f * 100.0), 1.5));
                }
            }
        }
        for c in meltdowns {
            // Meltdown: the reactor goes up in a chain of blasts, its core is lost.
            let p = self.chunks.grid().cell_center(c);
            if self.chunks.set_block(c, wolfcraft_engine::block::Block::Air) {
                self.net_out.push((c, wolfcraft_engine::block::Block::Air));
            }
            self.tech.remove(&c);
            self.containers.remove(&c);
            let up = wolfcraft_engine::physics::up_at(p);
            let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
            let ta = up.cross(helper).normalize();
            let tb = up.cross(ta);
            for (k, (a, b)) in [(0.0, 0.0), (2.5, 0.0), (-2.5, 0.0), (0.0, 2.5), (0.0, -2.5), (1.8, 1.8), (-1.8, -1.8), (1.8, -1.8), (-1.8, 1.8)].iter().enumerate() {
                self.explosives.lit.push(wolfcraft_engine::explosives::Lit { pos: p + ta * *a + tb * *b, fuse: 0.05 + k as f32 * 0.06 });
            }
            self.shake = self.shake.max(1.0);
            self.flash = Some(([1.0, 1.0, 0.85], 0.9));
            self.nuke_burst(p);
            self.nukes.push((p, 0.0));
            self.item_name = Some(("☢ Реактор взорвался!".into(), 3.0));
            self.grant("meltdown");
        }
    }

    /// The first instant of a meltdown: a white-hot fireball bursting out in every direction
    /// and a ring of dust racing along the ground (the shock wave).
    fn nuke_burst(&mut self, p: DVec3) {
        use wolfcraft_engine::mobs::FxKind;
        let up = wolfcraft_engine::physics::up_at(p);
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let t1 = up.cross(helper).normalize();
        let t2 = up.cross(t1);
        let amount = self.fx_amount.max(0.3) as f64;
        // Fireball: glowing chunks flying out of a sphere, hotter at the core.
        for _ in 0..(140.0 * amount) as usize {
            let (a, z) = (self.mobs.rand() * std::f64::consts::TAU, self.mobs.rand() * 2.0 - 1.0);
            let r = (1.0 - z * z).sqrt();
            let dir = (t1 * a.cos() + t2 * a.sin()) * r + up * z.abs();
            let speed = 6.0 + self.mobs.rand() * 14.0;
            let hot = self.mobs.rand();
            let col = if hot < 0.3 { [1.0, 0.97, 0.80] } else if hot < 0.7 { [1.0, 0.70, 0.18] } else { [0.95, 0.35, 0.08] };
            let (size, life) = (0.35 + self.mobs.rand() as f32 * 0.45, 1.4 + self.mobs.rand() as f32 * 0.8);
            self.mobs.fx_glow(p + dir * 0.8, dir * speed, col, size, life, FxKind::Rise);
        }
        // Shock wave: a flat ring of dust along the ground.
        for i in 0..(72.0 * amount) as usize {
            let a = i as f64 / (72.0 * amount) * std::f64::consts::TAU;
            let dir = t1 * a.cos() + t2 * a.sin();
            self.mobs.fx(p + dir * 1.5 + up * 0.3, dir * 22.0 + up * 0.4, [0.82, 0.80, 0.76], 0.7, 1.6, FxKind::Rise);
        }
        // Green-white radiation sparks.
        for _ in 0..(40.0 * amount) as usize {
            let v = (t1 * (self.mobs.rand() - 0.5) + t2 * (self.mobs.rand() - 0.5) + up * self.mobs.rand()) * 18.0;
            self.mobs.fx_glow(p, v, [0.70, 1.0, 0.55], 0.12, 1.2, FxKind::Fall);
        }
    }

    /// A meltdown's mushroom cloud, eight seconds long: a glowing stem climbs, a rolling cap
    /// spreads at its top, the fire cools to smoke.
    fn tick_nukes(&mut self, dt: f32) {
        use wolfcraft_engine::mobs::FxKind;
        if self.nukes.is_empty() {
            return;
        }
        let amount = self.fx_amount.max(0.3) as f64;
        let mut nukes = std::mem::take(&mut self.nukes);
        for (p, age) in nukes.iter_mut() {
            *age += dt;
            let t = *age as f64;
            let up = wolfcraft_engine::physics::up_at(*p);
            let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
            let t1 = up.cross(helper).normalize();
            let t2 = up.cross(t1);
            // Cooling: white-orange early, then dark orange, then grey smoke.
            let heat = (1.0 - t / 6.0).clamp(0.0, 1.0);
            let top = (t * 7.0).min(24.0);
            let spread = 3.0 + (t * 1.6).min(9.0);
            let n = ((70.0 * amount) * dt as f64).ceil() as usize;
            for _ in 0..n {
                let a = self.mobs.rand() * std::f64::consts::TAU;
                let ring = t1 * a.cos() + t2 * a.sin();
                // Stem: a column of rising fire and smoke.
                let h = self.mobs.rand() * top;
                let pos = *p + up * h + ring * (1.2 + self.mobs.rand() * 1.0);
                let col = if heat > 0.5 { [1.0, 0.55 + 0.3 * heat as f32, 0.15] } else { [0.45 + 0.4 * heat as f32, 0.40 + 0.2 * heat as f32, 0.38] };
                if heat > 0.35 {
                    self.mobs.fx_glow(pos, up * 5.0 + ring * 0.6, col, 0.6, 1.2, FxKind::Rise);
                } else {
                    self.mobs.fx(pos, up * 4.0 + ring * 0.6, col, 0.7, 1.6, FxKind::Rise);
                }
                // Cap: a rolling doughnut at the top, rolling outward.
                let r = spread * (0.6 + self.mobs.rand() * 0.5);
                let cap = *p + up * (top + self.mobs.rand() * 3.0) + ring * r;
                let roll = ring * 2.5 + up * (self.mobs.rand() * 2.0 - 0.5);
                let ccol = if heat > 0.4 { [1.0, 0.45 + 0.4 * heat as f32, 0.12] } else { [0.55, 0.50, 0.48] };
                let grow = self.mobs.rand() as f32;
                if heat > 0.4 && self.mobs.rand() < 0.6 {
                    self.mobs.fx_glow(cap, roll, ccol, 0.9 + grow * 0.6, 1.8, FxKind::Rise);
                } else {
                    self.mobs.fx(cap, roll, ccol, 1.0 + grow * 0.7, 2.2, FxKind::Rise);
                }
            }
            // A low rumble: the ground keeps shaking for the first seconds.
            if t < 3.0 {
                self.shake = self.shake.max((0.6 * (1.0 - t / 3.0)) as f32);
            }
        }
        nukes.retain(|(_, a)| *a < 8.0);
        self.nukes = nukes;
    }

    /// Machines at work, near the player: each kind shows it in its own way.
    fn tech_fx(&mut self, dt: f32) {
        use wolfcraft_engine::mobs::FxKind;
        use wolfcraft_engine::wolftech::Kind;
        if self.fx_amount <= 0.0 {
            return;
        }
        let me = self.player.body.pos;
        let g = self.chunks.grid();
        let busy: Vec<(DVec3, Kind)> = self
            .tech
            .iter()
            .filter(|(_, st)| st.working)
            .filter_map(|(c, _)| {
                let p = g.cell_center(*c);
                ((p - me).length_squared() < 28.0 * 28.0).then_some(())?;
                Some((p, self.chunks.get_block(*c).and_then(wolfcraft_engine::wolftech::kind)?))
            })
            .take(48)
            .collect();
        let k = self.fx_amount as f64 * dt as f64;
        for (p, kind) in busy {
            let up = wolfcraft_engine::physics::up_at(p);
            let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
            let t1 = up.cross(helper).normalize();
            let t2 = up.cross(t1);
            let r = self.mobs.rand();
            let side = |m: &mut wolfcraft_engine::mobs::MobManager| {
                let a = m.rand() * std::f64::consts::TAU;
                t1 * a.cos() + t2 * a.sin()
            };
            let top = p + up * 0.55;
            match kind {
                // Fire inside: smoke from the top and embers.
                Kind::Generator | Kind::Magma | Kind::CokeOven | Kind::BlastFurnace => {
                    if r < k * 3.0 {
                        let s = side(&mut self.mobs);
                        self.mobs.fx(top + s * 0.2, up * 1.2 + s * 0.2, [0.55, 0.54, 0.56], 0.22, 2.2, FxKind::Rise);
                    }
                    if r < k * 2.0 {
                        let s = side(&mut self.mobs);
                        let col = if kind == Kind::BlastFurnace { [1.0, 0.80, 0.30] } else { [1.0, 0.50, 0.12] };
                        let h = self.mobs.rand();
                        self.mobs.fx_glow(p + s * 0.52 + up * (h - 0.6) * 0.5, s * 0.8 + up * 1.4, col, 0.05, 0.8, FxKind::Rise);
                    }
                }
                // Grinding: dust and chips spat out of the sides.
                Kind::Crusher | Kind::Centrifuge | Kind::Recycler | Kind::Quarry | Kind::Enricher | Kind::Compressor => {
                    if r < k * 5.0 {
                        let s = side(&mut self.mobs);
                        self.mobs.fx(p + s * 0.55, s * 2.0 + up * 1.5, [0.62, 0.60, 0.57], 0.06, 0.9, FxKind::Fall);
                    }
                }
                Kind::Sawmill => {
                    if r < k * 6.0 {
                        let s = side(&mut self.mobs);
                        self.mobs.fx(p + s * 0.55, s * 2.5 + up * 1.8, [0.85, 0.68, 0.42], 0.05, 0.9, FxKind::Fall);
                    }
                }
                // Electric heat: white-blue sparks jumping out.
                Kind::Induction | Kind::ArcFurnace | Kind::Tesla | Kind::ChargePad => {
                    if r < k * 6.0 {
                        let s = side(&mut self.mobs);
                        let col = if self.mobs.rand() < 0.5 { [0.70, 0.90, 1.0] } else { [1.0, 1.0, 1.0] };
                        self.mobs.fx_glow(top + s * 0.3, s * 3.0 + up * 2.5, col, 0.04, 0.5, FxKind::Fall);
                    }
                }
                // Draconic: orange and purple motes spiralling up around the block.
                Kind::Fusion | Kind::DracoCore | Kind::MatterFab => {
                    if r < k * 8.0 {
                        let a = self.mobs.rand() * std::f64::consts::TAU;
                        let s = t1 * a.cos() + t2 * a.sin();
                        let swirl = t1 * (a + 1.57).cos() + t2 * (a + 1.57).sin();
                        let col = if self.mobs.rand() < 0.5 { [1.0, 0.55, 0.15] } else { [0.65, 0.30, 1.0] };
                        self.mobs.fx_glow(p + s * 0.75, swirl * 1.5 + up * 1.2, col, 0.07, 1.4, FxKind::Rise);
                    }
                }
                // Lava heat: embers.
                Kind::Geo | Kind::Smeltery | Kind::Extruder => {
                    if r < k * 2.5 {
                        let s = side(&mut self.mobs);
                        self.mobs.fx_glow(top + s * 0.3, up * 1.0 + s * 0.3, [1.0, 0.45, 0.10], 0.05, 1.0, FxKind::Rise);
                    }
                }
                // The reactor at work: green motes; the cells sparkle cyan while charging.
                Kind::Reactor => {
                    if r < k * 3.0 {
                        let s = side(&mut self.mobs);
                        self.mobs.fx_glow(p + s * 0.6, up * 0.8, [0.55, 1.0, 0.45], 0.05, 1.2, FxKind::Rise);
                    }
                }
                Kind::Cell | Kind::BigCell => {
                    if r < k * 3.0 {
                        let s = side(&mut self.mobs);
                        let h = self.mobs.rand();
                        self.mobs.fx_glow(p + s * 0.56 + up * (h - 0.5) * 0.8, up * 0.6, [0.40, 0.95, 1.0], 0.04, 0.8, FxKind::Rise);
                    }
                }
                Kind::Glacier => {
                    if r < k * 3.0 {
                        let s = side(&mut self.mobs);
                        self.mobs.fx(top + s * 0.3, up * 0.5 + s * 0.4, [0.92, 0.96, 1.0], 0.06, 1.2, FxKind::Rise);
                    }
                }
                Kind::Insolator => {
                    if r < k * 1.5 {
                        let s = side(&mut self.mobs);
                        self.mobs.fx(top + s * 0.3, up * 0.6, [0.45, 0.85, 0.35], 0.05, 1.5, FxKind::Leaf);
                    }
                }
                _ => {}
            }
        }
    }

    fn tech_path(&self) -> PathBuf {
        self.dir.join(if self.hell { "tech_hell.txt".to_string() } else if self.planet_kind != wolfcraft_engine::planet::PlanetKind::Home { format!("tech_{}.txt", self.planet_kind.slug()) } else { "tech.txt".to_string() })
    }

    fn save_tech(&self) {
        let mut out = String::new();
        for (c, st) in &self.tech {
            out += &format!("{} {} {} {} {} {} {}\n", c.f, c.i, c.j, c.k, st.energy, st.progress, st.burn);
        }
        if let Err(e) = wolfcraft_engine::storage::write_safe(&self.tech_path(), out.as_bytes()) {
            log::error!("saving tech: {e}");
        }
    }

    fn load_tech(&mut self) {
        let Some(text) = wolfcraft_engine::storage::read_with_backup(&self.tech_path(), |b| String::from_utf8(b.to_vec()).ok()) else { return };
        for line in text.lines() {
            let n: Vec<f64> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if n.len() == 7 {
                let c = wolfcraft_engine::cubesphere::Cell::new(n[0] as u8, n[1] as i32, n[2] as i32, n[3] as i32);
                self.tech.insert(c, TechState { energy: n[4] as f32, progress: n[5] as f32, burn: n[6] as f32, ..Default::default() });
            }
        }
    }

    /// Smelting progress 0..1 and whether the furnace is working (burning or powered).
    pub fn furnace_status(&self, pos: wolfcraft_engine::cubesphere::Cell) -> (f32, bool) {
        let t = self.furnace_t.get(&pos).copied().unwrap_or(0.0);
        let burning = self.furnace_fuel.get(&pos).is_some_and(|f| *f > 0.0);
        (t / FURNACE_TIME, burning || wolfcraft_engine::power::is_powered(&self.chunks, pos))
    }

    fn tick_furnaces(&mut self, dt: f32) {
        use wolfcraft_engine::block::Block as B;
        use wolfcraft_engine::interaction::{forge_result, fuel_time, smelt_result};
        use wolfcraft_engine::items::ItemStack;
        let cells: Vec<_> = self.containers.keys().copied().collect();
        for c in cells {
            match self.chunks.get_block(c).map(|b|b.base_kind()) {
                Some(B::ElectricFurnace) => {
                    self.container_slots(c, B::ElectricFurnace);
                    let v = &self.containers[&c];
                    let out = v[0].and_then(|s| smelt_result(s.id));
                    let room = out.is_some_and(|o| match v[2] {
                        None => true,
                        Some(s) => s.id == o && s.count < 64,
                    });
                    let powered = wolfcraft_engine::power::is_powered(&self.chunks, c);
                    let mut fuel = self.furnace_fuel.get(&c).copied().unwrap_or(0.0);
                    // Light a new piece of fuel when there is something to smelt.
                    if room && !powered && fuel <= 0.0 {
                        let v = self.containers.get_mut(&c).unwrap();
                        if let Some(f) = v[1].filter(|f| fuel_time(f.id) > 0.0) {
                            fuel = fuel_time(f.id);
                            v[1] = if f.count > 1 { Some(ItemStack { count: f.count - 1, ..f }) } else { None };
                        }
                    }
                    if fuel > 0.0 {
                        fuel -= dt;
                    }
                    if fuel > 0.0 {
                        self.furnace_fuel.insert(c, fuel);
                    } else {
                        self.furnace_fuel.remove(&c);
                    }
                    let Some(out) = out.filter(|_| room) else {
                        self.furnace_t.remove(&c);
                        continue;
                    };
                    if !(powered || fuel > 0.0) {
                        continue;
                    }
                    let t = self.furnace_t.entry(c).or_insert(0.0);
                    *t += dt;
                    if *t >= FURNACE_TIME {
                        *t = 0.0;
                        let v = self.containers.get_mut(&c).unwrap();
                        if let Some(i) = v[0].as_mut() {
                            i.count -= 1;
                            if i.count == 0 {
                                v[0] = None;
                            }
                        }
                        match v[2].as_mut() {
                            Some(o) => o.count += 1,
                            None => v[2] = Some(ItemStack::new(out, 1)),
                        }
                    }
                }
                Some(B::MachineCasing) => {
                    // Forge: the result appears as soon as the two inputs fit together.
                    let v = self.containers.get_mut(&c).unwrap();
                    if v.len() == 3 && v[2].is_none() {
                        if let (Some(a), Some(b)) = (v[0], v[1]) {
                            if let Some((res, used)) = forge_result(a, b) {
                                v[2] = Some(res);
                                v[0] = None;
                                v[1] = if b.count > used { Some(ItemStack { count: b.count - used, ..b }) } else { None };
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn dimension_path(&self) -> PathBuf {
        self.dir.join("dimension.txt")
    }

    fn load_dimension(&mut self) {
        let Ok(text) = std::fs::read_to_string(self.dimension_path()) else { return };
        let mut it = text.split_whitespace();
        let first = it.next();
        if first == Some("planet") {
            let Some(kind) = it.next().and_then(wolfcraft_engine::planet::PlanetKind::from_slug) else { return };
            let n: Vec<f64> = it.filter_map(|x| x.parse().ok()).collect();
            if n.len() == 3 {
                self.overworld_pos = Some(DVec3::new(n[0], n[1], n[2]));
            }
            self.enter_place(false, kind);
            return;
        }
        if first != Some("hell") {
            return;
        }
        let n: Vec<f64> = it.filter_map(|x| x.parse().ok()).collect();
        if n.len() == 3 {
            self.overworld_pos = Some(DVec3::new(n[0], n[1], n[2]));
        }
        self.enter_dimension(true);
    }

    /// Swaps planet + chunk streaming for the other dimension (the player is not moved).
    fn enter_dimension(&mut self, hell: bool) {
        self.enter_place(hell, wolfcraft_engine::planet::PlanetKind::Home);
    }

    /// Swaps planet + chunk streaming: home, its hell, or another planet of the world.
    fn enter_place(&mut self, hell: bool, kind: wolfcraft_engine::planet::PlanetKind) {
        use wolfcraft_engine::planet::PlanetKind;
        self.save_containers();
        self.save_tech();
        self.containers.clear();
        self.kin.clear();
        self.tech.clear();
        self.tech_nets.clear();
        let planet = Arc::new(if hell { Planet::new_hell(&self.cfg) } else { Planet::new_world(&self.cfg, kind) });
        let dir = if hell {
            self.dir.join("hell")
        } else if kind != PlanetKind::Home {
            self.dir.join(kind.slug())
        } else {
            self.dir.clone()
        };
        let Ok(storage) = wolfcraft_engine::storage::Storage::open(&dir) else {
            log::error!("cannot open dimension storage {}", dir.display());
            return;
        };
        self.planet = planet.clone();
        self.chunks = ChunkManager::new(planet, Arc::new(storage));
        self.mobs = wolfcraft_engine::mobs::MobManager::new(self.cfg.seed);
        self.mobs.planet = if hell { 0 } else { kind.index() };
        self.explosives = wolfcraft_engine::explosives::Explosives::new(self.cfg.seed);
        self.hell = hell;
        self.planet_kind = if hell { PlanetKind::Home } else { kind };
        wolfcraft_engine::physics::set_gravity_scale(self.planet_kind.gravity());
        self.load_containers();
        self.load_tech();
        self.refresh_globe();
        self.dimension_changed = true;
    }

    /// What the planet looks like from orbit: its real surface colours, 20 × 20 per face.
    fn refresh_globe(&self) {
        const N: i32 = 20;
        let sampler = wolfcraft_engine::cubesphere::Grid { n: N, cube: self.planet.grid.cube };
        let mut cols = Vec::with_capacity((6 * N * N) as usize);
        for f in 0..6u8 {
            for b in 0..N {
                for a in 0..N {
                    let col = self.planet.column(sampler.cell_dir(f, a, b));
                    cols.push(crate::preview::biome_color(&col));
                }
            }
        }
        wolfcraft_engine::renderer::sky::set_globe(N as u32, cols);
    }

    /// Lift-off: needs open sky and 4 rocket fuel (free in creative).
    fn start_rocket(&mut self) {
        use wolfcraft_engine::items::ids;
        if self.hell {
            self.item_name = Some(("Из Ада ракета не взлетит".into(), 2.5));
            return;
        }
        let up = self.player.up();
        let p = self.player.body.pos;
        let blocked = (2..40).any(|h| self.chunks.block_at_world(p + up * h as f64).is_solid());
        if blocked {
            self.item_name = Some(("Ракете нужно открытое небо над головой".into(), 2.5));
            return;
        }
        if !self.player.is_creative() {
            if self.player.inventory.count(ids::ROCKET_FUEL) < 4 {
                self.item_name = Some(("Нужно 4 ракетного топлива".into(), 2.5));
                return;
            }
            self.player.inventory.take(ids::ROCKET_FUEL, 4);
        }
        self.player.body.flying = true;
        self.player.body.vel = DVec3::ZERO;
        self.rocket = Some(RocketFlight { phase: 0, t: 0.0, speed: 0.0 });
        self.rocket_pick = None;
        self.shake = self.shake.max(0.5);
        self.item_name = Some(("🚀 Старт!".into(), 2.0));
    }

    /// The flight: thrust up through the sky, wait in orbit for the planet, come down slowly on
    /// the new world's surface with the engine braking.
    fn tick_rocket(&mut self, dt: f32) {
        use wolfcraft_engine::mobs::FxKind;
        use wolfcraft_engine::planet::PlanetKind;
        let Some(mut r) = self.rocket else { return };
        r.t += dt;
        let up = self.player.up();
        let pos = self.player.body.pos;
        let flame = |s: &mut Self, power: f64| {
            let amount = (s.fx_amount.max(0.3) * 6.0 * power as f32) as usize + 1;
            for _ in 0..amount {
                let j = DVec3::new(s.mobs.rand() - 0.5, s.mobs.rand() - 0.5, s.mobs.rand() - 0.5) * 0.6;
                let hot = s.mobs.rand();
                let col = if hot < 0.35 { [1.0, 0.95, 0.70] } else if hot < 0.75 { [1.0, 0.60, 0.15] } else { [0.95, 0.30, 0.08] };
                s.mobs.fx_glow(pos - up * 3.4 + j, -up * (8.0 + hot * 6.0) + j * 3.0, col, 0.18, 0.5, FxKind::Rise);
            }
            if s.mobs.rand() < 0.6 * power {
                let j = DVec3::new(s.mobs.rand() - 0.5, s.mobs.rand() - 0.5, s.mobs.rand() - 0.5);
                s.mobs.fx(pos - up * 4.0 + j, -up * 2.0 + j * 2.0, [0.80, 0.80, 0.82], 0.6, 2.4, FxKind::Rise);
            }
        };
        match r.phase {
            0 => {
                r.speed = (r.speed + dt as f64 * (6.0 + r.t as f64 * 10.0)).min(140.0);
                self.player.body.pos += up * r.speed * dt as f64;
                self.player.body.vel = DVec3::ZERO;
                flame(self, 1.0);
                self.shake = self.shake.max((0.4 * (1.0 - r.t / 6.0)).max(0.05));
                if r.t > 6.0 {
                    r = RocketFlight { phase: 1, t: 0.0, speed: 0.0 };
                    self.item_name = Some(("Орбита: выберите планету".into(), 3.0));
                }
            }
            1 => {
                self.player.body.vel = DVec3::ZERO;
                if let Some(kind) = self.rocket_pick.take() {
                    self.travel_to(kind);
                    r = RocketFlight { phase: 2, t: 0.0, speed: 60.0 };
                }
            }
            _ => {
                // Down to the ground, braking: fast up high, slow near the surface. Waits in
                // the air while the ground is still being generated.
                let below = (1..60).find(|h| self.chunks.get_block(self.chunks.grid().world_to_cell(pos - up * *h as f64)).is_some_and(|b| b.is_solid()));
                let loaded = self.ground_ready();
                let target = match below {
                    Some(h) => (h as f64 * 1.4).clamp(3.0, 60.0),
                    None if loaded => 60.0,
                    None => 0.0,
                };
                r.speed += (target - r.speed) * (1.0 - (-3.0 * dt as f64).exp());
                self.player.body.pos -= up * r.speed * dt as f64;
                self.player.body.vel = DVec3::ZERO;
                flame(self, 0.6);
                if below.is_some_and(|h| h <= 1) {
                    self.player.body.flying = false;
                    self.rocket = None;
                    self.shake = self.shake.max(0.4);
                    self.item_name = Some((format!("Посадка: {}", self.planet_kind.name()), 3.0));
                    if self.planet_kind != PlanetKind::Home {
                        self.grant("space");
                    }
                    return;
                }
            }
        }
        self.rocket = Some(r);
    }

    /// Moves the world to `kind` (from orbit): the player comes down above its surface — at
    /// home where they took off from it, elsewhere at a safe spot.
    fn travel_to(&mut self, kind: wolfcraft_engine::planet::PlanetKind) {
        use wolfcraft_engine::planet::PlanetKind;
        let here = self.player.body.pos;
        if kind == self.planet_kind {
            return;
        }
        self.save();
        if self.planet_kind == PlanetKind::Home {
            self.overworld_pos = Some(here);
        }
        self.enter_place(false, kind);
        let ground = if kind == PlanetKind::Home { self.overworld_pos } else { Some(self.planet.find_spawn()) };
        let ground = ground.unwrap_or_else(|| self.planet.find_spawn());
        let up = wolfcraft_engine::physics::up_at(ground);
        self.teleport(ground + up * 90.0);
        self.player.body.flying = true;
        self.spawn_checked = true;
    }

    /// Hell <-> Overworld through a portal (or `/hell`).
    pub fn switch_dimension(&mut self) {
        self.save();
        let to_hell = !self.hell;
        let here = self.player.body.pos;
        if to_hell {
            self.overworld_pos = Some(here);
        }
        self.enter_dimension(to_hell);
        let d = if to_hell { self.planet.grid.col_dir(here) } else { self.planet.grid.col_dir(self.overworld_pos.unwrap_or(here)) };
        let target = if !to_hell { self.overworld_pos } else { None };
        let pos = target.or_else(|| {
            // Look for a safe spot around the same direction.
            let up = if d.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
            let a = d.cross(up).normalize();
            let b = d.cross(a);
            (0..40).find_map(|i| {
                let off = (i as f64) * 0.004;
                let ang = i as f64 * 2.4;
                self.planet.stand_position((d + a * ang.cos() * off + b * ang.sin() * off).normalize())
            })
        });
        let pos = pos.unwrap_or_else(|| self.planet.grid.along(d, self.planet.radius + 6.0));
        self.teleport(pos);
        self.player.body.flying = false;
        // Exit portal two blocks aside, placed once its chunk exists.
        let cell = self.chunks.grid().world_to_cell(pos);
        self.pending_portal = Some(wolfcraft_engine::cubesphere::Cell::new(cell.f, cell.i + 2, cell.j, cell.k));
        self.portal_t = -4.0;
    }

    fn block_commands_path(&self) -> PathBuf {
        self.dir.join("commands.txt")
    }

    fn load_block_commands(&mut self) {
        let Ok(text) = std::fs::read_to_string(self.block_commands_path()) else { return };
        for line in text.lines() {
            let mut it = line.splitn(5, ' ');
            let n: Vec<i64> = (0..4).filter_map(|_| it.next().and_then(|x| x.parse().ok())).collect();
            if n.len() == 4 {
                let cell = wolfcraft_engine::cubesphere::Cell::new(n[0] as u8, n[1] as i32, n[2] as i32, n[3] as i32);
                self.block_commands.insert(cell, it.next().unwrap_or("").to_string());
            }
        }
    }

    fn save_block_commands(&self) {
        let mut out = String::new();
        for (c, cmd) in &self.block_commands {
            if !cmd.is_empty() {
                out += &format!("{} {} {} {} {}\n", c.f, c.i, c.j, c.k, cmd.replace('\n', " "));
            }
        }
        if let Err(e) = std::fs::write(self.block_commands_path(), out) {
            log::error!("saving commands.txt: {e}");
        }
    }

    fn containers_path(&self) -> PathBuf {
        self.dir.join(if self.hell { "containers_hell.txt".to_string() } else if self.planet_kind != wolfcraft_engine::planet::PlanetKind::Home { format!("containers_{}.txt", self.planet_kind.slug()) } else { "containers.txt".to_string() })
    }

    fn load_containers(&mut self) {
        let Ok(text) = std::fs::read_to_string(self.containers_path()) else { return };
        for line in text.lines() {
            let n: Vec<i64> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if n.len() == 9 || n.len() == 10 {
                let cell = wolfcraft_engine::cubesphere::Cell::new(n[0] as u8, n[1] as i32, n[2] as i32, n[3] as i32);
                let size = n[4] as usize;
                let slot = n[5] as usize;
                let v = self.containers.entry(cell).or_insert_with(|| vec![None; size]);
                if slot < v.len() && wolfcraft_engine::items::is_valid(n[6] as u16) {
                    v[slot] = Some(wolfcraft_engine::items::ItemStack { id: n[6] as u16, count: n[7] as u8, damage: n[8] as u16, ench: n.get(9).copied().unwrap_or(0) as u32 });
                }
            }
        }
    }

    fn save_containers(&self) {
        let mut out = String::new();
        for (c, v) in &self.containers {
            for (i, s) in v.iter().enumerate() {
                if let Some(s) = s {
                    out += &format!("{} {} {} {} {} {} {} {} {} {}\n", c.f, c.i, c.j, c.k, v.len(), i, s.id, s.count, s.damage, s.ench);
                }
            }
        }
        if let Err(e) = wolfcraft_engine::storage::write_safe(&self.containers_path(), out.as_bytes()) {
            log::error!("saving containers.txt: {e}");
        }
    }

    /// Writes player.dat, edited chunks and world.toml.
    pub fn save(&mut self) {
        if !self.hell {
            self.save_kinetic();
            self.save_pipes();
        }
        if let Err(e)=self.storage_bank.save(&self.dir.join("storage_bank.bin")){log::error!("storage bank: {e}");}
        if let Err(e)=self.boss_book.save(&self.dir.join("bosses.txt")){log::error!("bosses: {e}");}
        self.save_containers();
        self.save_block_commands();
        match &self.arena {
            Some(a) => {
                let _ = wolfcraft_engine::storage::write_safe(&self.dir.join("arena.txt"), a.to_text().as_bytes());
            }
            None => {
                let _ = std::fs::remove_file(self.dir.join("arena.txt"));
            }
        }
        if self.clicker.active {
            if let Err(e) = wolfcraft_engine::storage::write_safe(&self.dir.join("clicker.txt"), self.clicker.to_text().as_bytes()) {
                log::error!("clicker: {e}");
            }
        }
        if self.buttonsim.active {
            if let Err(e) = wolfcraft_engine::storage::write_safe(&self.dir.join("buttonsim.txt"), self.buttonsim.to_text().as_bytes()) {
                log::error!("buttonsim: {e}");
            }
        }
        if !self.petsim.owned.is_empty() || self.petsim.coins > 0 {
            if let Err(e) = wolfcraft_engine::storage::write_safe(&self.dir.join("petsim.txt"), self.petsim.to_text().as_bytes()) {
                log::error!("petsim: {e}");
            }
        }
        if let Some(h) = self.petsim_home {
            let _ = std::fs::write(self.dir.join("petsim_home.txt"), format!("{} {} {}", h.x, h.y, h.z));
        }
        let dim = if self.hell {
            let p = self.overworld_pos.unwrap_or(DVec3::ZERO);
            format!("hell {} {} {}", p.x, p.y, p.z)
        } else if self.planet_kind != wolfcraft_engine::planet::PlanetKind::Home {
            let p = self.overworld_pos.unwrap_or(DVec3::ZERO);
            format!("planet {} {} {} {}", self.planet_kind.slug(), p.x, p.y, p.z)
        } else {
            "overworld".to_string()
        };
        let _ = std::fs::write(self.dimension_path(), dim);
        if let Err(e) = wolfcraft_engine::storage::write_safe(&self.dir.join("player.dat"), &self.player.to_bytes()) {
            log::error!("saving player.dat: {e}");
        }
        match self.chunks.save_all() {
            Ok(n) => log::info!("saved {n} region file(s)"),
            Err(e) => log::error!("saving regions: {e}"),
        }
        self.cfg.last_played = platform::unix_now();
        if let Err(e) = self.cfg.save(&self.dir) {
            log::error!("saving world.toml: {e}");
        }
        self.saved_notice = 2.0;
    }

    /// Camera for rendering. First person sits at the eye level; third person orbits behind or
    /// in front of the character and never clips into blocks.
    pub fn camera(&self) -> Camera {
        let mut c = self.camera_base();
        if self.shake > 0.0 {
            let t = self.clock as f64;
            let k = (self.shake * self.shake) as f64 * 0.45;
            c.eye += DVec3::new((t * 53.0).sin(), (t * 71.0).sin(), (t * 37.0).sin()) * k;
        }
        c
    }

    /// Walk cycle of the view (phase, strength 0..1): the hands in front bob with it.
    pub fn view_bob(&self) -> (f32, f32) {
        (self.bob_phase as f32, self.bob_amp.min(1.0) as f32)
    }

    fn camera_base(&self) -> Camera {
        if let Some(c) = self.cutscene {
            return c;
        }
        let up = self.player.up();
        let raw_eye = self.player.eye_pos();
        let eye_r = if self.eye_r > 0.0 { self.eye_r } else { radial(raw_eye) };
        let eye = along(wolfcraft_engine::cubesphere::col_dir(raw_eye), eye_r) + up * self.cam_lag;
        let look = self.player.look_dir();
        let eye = eye - up * self.land_dip;
        let right0 = look.cross(up).normalize_or_zero();
        let first = self.camera_mode == CameraMode::First;
        // The view rocks from side to side with the steps (more when running), leans into a
        // sideways run, and rolls when hurt (Minecraft's damage tilt).
        let a = self.bob_amp;
        let ph = self.bob_phase;
        let roll = self.player.survival.hurt_flash as f64 * 0.35 + if first { ph.sin() * 0.011 * a + self.lean } else { 0.0 };
        let up = if roll.abs() > 1e-5 && right0.length_squared() > 0.5 { (up * roll.cos() + right0 * roll.sin()).normalize() } else { up };
        match self.camera_mode {
            // Gentle view bobbing (Minecraft-like): a small dip on every step and a sway
            // from side to side, scaled by the smoothed walking strength.
            CameraMode::First => {
                // Steps: a smooth dip on each footfall (no kink), a sway to the side, and a
                // slight nod; standing still, a slow breath.
                let dip = (1.0 - (2.0 * ph).cos()) * 0.5;
                let breath = (self.clock as f64 * 1.6).sin() * 0.008 * (1.0 - a.min(1.0));
                let eye = eye + right0 * (ph.sin() * 0.035 * a) - up * (dip * 0.055 * a) + up * breath;
                let nod = ((2.0 * ph).cos() * 0.006 * a) as f64;
                let forward = (look + up * nod).normalize();
                Camera { eye, forward, up }
            }
            CameraMode::ThirdBack | CameraMode::ThirdFront => {
                let sign = if self.camera_mode == CameraMode::ThirdBack { -1.0 } else { 1.0 };
                let want = 4.2;
                let dir = look * sign;
                let _ = want;
                let dist = self.tp_dist.min(self.room_along(eye, dir));
                let pos = eye + dir * dist + right0 * self.shoulder;
                Camera { eye: pos, forward: if sign < 0.0 { look } else { -look }, up }
            }
        }
    }

    /// Free distance from the eye along `dir` up to the third-person distance: only solid blocks
    /// stop the camera (grass and flowers do not), with a margin for the near plane.
    fn room_along(&self, eye: DVec3, dir: DVec3) -> f64 {
        let want = 4.2;
        let mut t = 0.0;
        while t < want + 0.35 {
            t += 0.05;
            if self.chunks.block_at_world(eye + dir * t).is_solid() {
                return (t - 0.35).clamp(0.2, want);
            }
        }
        want
    }

    fn third_person_room(&self) -> f64 {
        let look = self.player.look_dir();
        let sign = if self.camera_mode == CameraMode::ThirdFront { 1.0 } else { -1.0 };
        let eye = self.player.eye_pos() + self.player.up() * self.cam_lag;
        self.room_along(eye, look * sign)
    }

    /// Weather: the spell changes now and then; drops or flakes fall around the player (not
    /// under a roof, not in deserts); thunderstorms strike lightning nearby. Rain puts out
    /// burning creatures and keeps the sun from setting ghouls on fire.
    fn weather_tick(&mut self, dt: f32) {
        use wolfcraft_engine::biome::Biome as B;
        use wolfcraft_engine::mobs::FxKind;
        let flat = self.cfg.world_type == wolfcraft_engine::config::WorldType::Baseplate;
        if self.hell || flat {
            self.mobs.sun_burns = true;
            return;
        }
        if self.weather.step(dt) {
            self.item_name = Some((
                match self.weather.sky {
                    crate::weather::Sky::Clear => "Распогодилось",
                    crate::weather::Sky::Rain => "Начинается дождь",
                    crate::weather::Sky::Thunder => "Надвигается гроза",
                }
                .into(),
                2.5,
            ));
        }
        let (biome, _, _) = self.env_col.unwrap_or((B::Plains, 0.5, 0.5));
        let dry = matches!(biome, B::Desert | B::Badlands | B::Volcano | B::Savanna | B::MoonMare | B::MoonHighlands | B::AshWastes | B::LavaFields);
        let rain = if dry { 0.0 } else { self.weather.rain };
        self.mobs.sun_burns = rain < 0.3;
        if rain <= 0.01 {
            return;
        }
        let snowy = self.environment().1 > 0.5;
        let pos = self.player.body.pos;
        let up = self.player.up();
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let t1 = (helper - up * helper.dot(up)).normalize();
        let t2 = up.cross(t1);
        let k = self.fx_amount.clamp(0.2, 2.0) as f64;
        let n = (rain as f64 * k * if snowy { 90.0 } else { 260.0 } * dt as f64) + self.mobs.rand();
        for _ in 0..n as usize {
            let a = self.mobs.rand() * std::f64::consts::TAU;
            let d = 1.0 + self.mobs.rand().sqrt() * 13.0;
            let h = 6.0 + self.mobs.rand() * 10.0;
            let at = pos + (t1 * a.cos() + t2 * a.sin()) * d + up * h;
            if snowy {
                self.mobs.fx(at, -up * 1.6, [0.95, 0.97, 1.0], 0.07, 3.0, FxKind::Snow);
            } else {
                // Slanted a little by the wind.
                let wind = t1 * 1.2;
                self.mobs.fx(at, -up * 16.0 + wind, [0.62, 0.72, 0.9], 0.03, 1.4, FxKind::Rain);
            }
        }
        // Rain puts out burning creatures.
        if !snowy && rain > 0.5 {
            for m in &mut self.mobs.mobs {
                m.burning = 0.0;
            }
        }
        if self.weather.bolt(dt) {
            // Lightning somewhere 15–45 blocks away: a white flash, a bright column of sparks.
            let a = self.mobs.rand() * std::f64::consts::TAU;
            let d = 15.0 + self.mobs.rand() * 30.0;
            let dir = (pos + (t1 * a.cos() + t2 * a.sin()) * d).normalize();
            if let Some(ground) = self.planet.stand_position(dir) {
                let gup = wolfcraft_engine::physics::up_at(ground);
                for i in 0..40 {
                    let y = i as f64 * 1.2;
                    let wob = (t1 * (self.mobs.rand() - 0.5) + t2 * (self.mobs.rand() - 0.5)) * 0.8;
                    self.mobs.fx_glow(ground + gup * y + wob, DVec3::ZERO, [0.85, 0.9, 1.0], 0.3, 0.35, FxKind::Rise);
                }
                self.flash = Some(([0.9, 0.92, 1.0], 0.25));
                self.shake = self.shake.max(0.25);
                // Sometimes the strike sets the spot on fire (grass, a tree).
                if self.mobs.rand() < 0.4 {
                    let g = self.chunks.grid();
                    let c = g.world_to_cell(ground + gup * 0.5);
                    let below = g.offset(c, 0, 0, -1).and_then(|b| self.chunks.get_block(b));
                    if self.chunks.get_block(c) == Some(wolfcraft_engine::block::Block::Air) && below.is_some_and(|b| b.is_solid() || b.flammable()) {
                        self.chunks.set_block(c, wolfcraft_engine::block::Block::Fire);
                    }
                }
            }
        }
    }

    /// Fire, like Minecraft's: it burns the block under it away, spreads to air next to wood,
    /// leaves and plants, dies out on stone, goes out in the rain; it hurts what stands in it.
    fn fire_tick(&mut self, dt: f32) {
        use wolfcraft_engine::block::Block;
        let g = self.chunks.grid();
        // Standing in fire hurts and sets creatures alight (every frame).
        let feet = g.world_to_cell(self.player.body.pos + self.player.up() * 0.3);
        if self.chunks.get_block(feet) == Some(Block::Fire) && !self.player.is_creative() {
            self.player.damage(2.0 * dt, self.cfg.difficulty);
        }
        self.fire_t -= dt;
        if self.fire_t > 0.0 {
            return;
        }
        self.fire_t = 0.5;
        for m in &mut self.mobs.mobs {
            let c = g.world_to_cell(m.body.pos + wolfcraft_engine::physics::up_at(m.body.pos) * 0.3);
            if self.chunks.get_block(c) == Some(Block::Fire) {
                m.burning = m.burning.max(4.0);
            }
        }
        let fires: Vec<_> = self.fx_lights.iter().filter(|(_, b)| *b == Block::Fire).map(|(p, _)| g.world_to_cell(*p)).take(64).collect();
        let raining = self.rain_amount() > 0.4 && self.environment().1 < 0.5;
        for c in fires {
            if self.chunks.get_block(c) != Some(Block::Fire) {
                continue;
            }
            let roll = self.mobs.rand();
            // Rain puts out fires under the open sky.
            if raining && roll < 0.4 {
                let p = g.cell_center(c);
                let up = wolfcraft_engine::physics::up_at(p);
                let open = (1..30).all(|k| !self.chunks.block_at_world(p + up * k as f64).is_solid());
                if open {
                    self.chunks.set_block(c, Block::Air);
                    self.mobs.fx(p, up * 2.0, [0.6, 0.6, 0.62], 0.25, 2.0, wolfcraft_engine::mobs::FxKind::Rise);
                    continue;
                }
            }
            let below = g.offset(c, 0, 0, -1);
            let fuel = below.and_then(|b| self.chunks.get_block(b)).is_some_and(|b| b.flammable());
            if !fuel {
                // Nothing to burn: dies out after a while.
                if roll < 0.12 {
                    self.chunks.set_block(c, Block::Air);
                }
            } else if roll < 0.06 {
                // The block underneath burns away; the fire moves down onto what is left.
                if let Some(b) = below {
                    self.chunks.set_block(b, Block::Fire);
                    self.chunks.set_block(c, Block::Air);
                    let p = g.cell_center(b);
                    self.mobs.chips(p, &[[0.25, 0.18, 0.12], [0.1, 0.08, 0.06]], 6, 3.0);
                }
                continue;
            }
            // Spread: a random neighbour (sideways, up, diagonals) that is air and touches fuel.
            let (di, dj, dk) = {
                let r = (self.mobs.rand() * 26.0) as i32;
                let (a, b, k) = (r % 3 - 1, (r / 3) % 3 - 1, r / 9 - 1);
                (a, b, k)
            };
            if (di, dj, dk) == (0, 0, 0) || self.mobs.rand() > 0.35 {
                continue;
            }
            let Some(n) = g.offset(c, di, dj, dk) else { continue };
            if self.chunks.get_block(n) != Some(Block::Air) {
                continue;
            }
            let touches_fuel = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].iter().any(|&(a, b, k)| g.offset(n, a, b, k).and_then(|x| self.chunks.get_block(x)).is_some_and(|x| x.flammable()));
            if touches_fuel {
                self.chunks.set_block(n, Block::Fire);
            }
        }
    }

    /// How rainy it is where the player stands (0..1): the sky and fog go grey.
    pub fn rain_amount(&self) -> f32 {
        use wolfcraft_engine::biome::Biome as B;
        if self.hell {
            return 0.0;
        }
        let (biome, _, _) = self.env_col.unwrap_or((B::Plains, 0.5, 0.5));
        if matches!(biome, B::Desert | B::Badlands | B::Volcano | B::Savanna | B::MoonMare | B::MoonHighlands | B::AshWastes | B::LavaFields) { 0.0 } else { self.weather.rain }
    }

    /// Particles that bring the world to life: embers and smoke over torches, bubbles and
    /// sparks over lava, glowing motes around uranium, falling leaves, a splash when the
    /// player jumps into water, dust on landing and behind a sprinting player.
    fn special_effects(&mut self, dt: f32) {
        // A meltdown's mushroom cloud keeps rising (also when no machine is left).
        self.tick_nukes(dt);
        use wolfcraft_engine::block::Block;
        use wolfcraft_engine::mobs::FxKind;
        let k = self.fx_amount.clamp(0.0, 2.0);
        if self.env_col.is_none() || self.fx_scan <= dt {
            let col = self.planet.column(self.player.up());
            self.env_col = Some((col.biome, col.temp, col.humid));
        }
        if k <= 0.0 || self.player.survival.dead {
            return;
        }
        let g = self.chunks.grid();
        let pos = self.player.body.pos;
        let up = self.player.up();
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let t1 = (helper - up * helper.dot(up)).normalize();
        let t2 = up.cross(t1);
        let jitter = |m: &mut wolfcraft_engine::mobs::MobManager, s: f64| (t1 * (m.rand() - 0.5) + t2 * (m.rand() - 0.5)) * s;
        // Light sources within 16 blocks, from the chunks' emitter lists (cheap).
        self.fx_scan -= dt;
        if self.fx_scan <= 0.0 {
            self.fx_scan = 0.5;
            self.fx_lights.clear();
            let centre = ChunkKey::of(g.world_to_cell(pos));
            for dk in -1..=1 {
                for dj in -1..=1 {
                    for di in -1..=1 {
                        let Some(key) = g.chunk_neighbor(centre, di, dj, dk) else { continue };
                        let Some(ch) = self.chunks.chunk(key) else { continue };
                        let o = key.origin();
                        for &(idx, _) in ch.emitters() {
                            let (li, lj, lk) = wolfcraft_engine::cubesphere::idx_to_local(idx as usize);
                            let c = wolfcraft_engine::cubesphere::Cell::new(key.f, o.x + li, o.y + lj, o.z + lk);
                            let mut p = g.cell_center(c);
                            if (p - pos).length() < 16.0 {
                                let mut b = ch.get(li, lj, lk);
                                // A wall torch: its flame is near the wall and a bit lower.
                                if let Some((base, d)) = b.wall_mount() {
                                    let (axis, s) = [(0usize, 1i32), (0, -1), (1, 1), (1, -1)][d as usize];
                                    if let Some(w) = g.neighbor(c, axis, s) {
                                        let toward = (g.cell_center(w) - p).normalize_or_zero();
                                        p += toward * 0.2 - wolfcraft_engine::physics::up_at(p) * 0.06;
                                    }
                                    b = base;
                                }
                                self.fx_lights.push((p, b));
                            }
                        }
                    }
                }
            }
        }
        let lights = self.fx_lights.clone();
        for (p, b) in lights {
            let r = self.mobs.rand();
            let rate = k as f64 * dt as f64;
            let lup = wolfcraft_engine::physics::up_at(p);
            match b {
                Block::Torch | Block::Lantern | Block::IskritTorch => {
                    if r < rate * 2.5 {
                        let col = if b == Block::IskritTorch { [1.0, 0.35, 0.75] } else if self.mobs.rand() < 0.5 { [1.0, 0.62, 0.15] } else { [1.0, 0.85, 0.35] };
                        let v = lup * (0.4 + self.mobs.rand() * 0.5) + jitter(&mut self.mobs, 0.3);
                        let o = jitter(&mut self.mobs, 0.08);
                        self.mobs.fx_glow(p + lup * 0.36 + o, v, col, 0.035, 0.9, FxKind::Rise);
                    } else if r < rate * 3.3 && b == Block::Torch {
                        let v = lup * 0.5 + jitter(&mut self.mobs, 0.15);
                        // Light grey puffs (dark ones read as black squares against the walls).
                        self.mobs.fx(p + lup * 0.45, v, [0.78, 0.78, 0.80], 0.05, 1.2, FxKind::Rise);
                    }
                }
                Block::Lava => {
                    if r < rate * 0.6 {
                        // A bubble pops: a few sparks fly up and fall back.
                        for _ in 0..3 {
                            let v = lup * (2.5 + self.mobs.rand() * 2.5) + jitter(&mut self.mobs, 2.0);
                            self.mobs.fx_glow(p + lup * 0.55, v, [1.0, 0.45, 0.08], 0.05, 1.2, FxKind::Fall);
                        }
                    }
                }
                Block::UraniumOre | Block::UraniumBlock => {
                    if r < rate * 1.2 {
                        let o = jitter(&mut self.mobs, 1.2) + lup * (self.mobs.rand() - 0.3);
                        let v = lup * 0.25 + jitter(&mut self.mobs, 0.2);
                        self.mobs.fx_glow(p + o, v, [0.5, 1.0, 0.35], 0.03, 1.5, FxKind::Rise);
                    }
                }
                _ => {}
            }
        }
        // Falling leaves: try a few random cells around the player each frame.
        let tries = (6.0 * k) as usize;
        let feet = g.world_to_cell(pos);
        for _ in 0..tries {
            let (di, dj, dk) = ((self.mobs.rand() * 25.0) as i32 - 12, (self.mobs.rand() * 25.0) as i32 - 12, (self.mobs.rand() * 14.0) as i32 - 2);
            let Some(c) = g.offset(feet, di, dj, dk) else { continue };
            let b = self.chunks.get_block(c).unwrap_or(Block::Air);
            if !matches!(b, Block::OakLeaves | Block::BirchLeaves | Block::SpruceLeaves | Block::AcaciaLeaves | Block::CherryLeaves) {
                continue;
            }
            let below = g.neighbor(c, 2, -1).and_then(|n| self.chunks.get_block(n)).unwrap_or(Block::Stone);
            // Autumn: leaves fall much more often, in autumn colours; winter: hardly any.
            let (season, _, _) = self.environment();
            let autumn = (2.0..3.0).contains(&season);
            let rate = if autumn { 0.45 } else if season >= 3.0 { 0.02 } else { 0.12 };
            if below != Block::Air || self.mobs.rand() > rate {
                continue;
            }
            let col = if autumn && b != Block::SpruceLeaves {
                [[0.95, 0.55, 0.15], [0.80, 0.24, 0.12], [0.95, 0.80, 0.25]][(self.mobs.rand() * 3.0) as usize % 3]
            } else {
                match b {
                    Block::BirchLeaves => [0.55, 0.72, 0.25],
                    Block::SpruceLeaves => [0.20, 0.42, 0.24],
                    Block::AcaciaLeaves => [0.62, 0.70, 0.22],
                    _ => if self.mobs.rand() < 0.2 { [0.85, 0.62, 0.18] } else { [0.32, 0.62, 0.20] },
                }
            };
            let p = g.cell_center(c) - wolfcraft_engine::physics::up_at(g.cell_center(c)) * 0.55;
            self.mobs.fx(p, DVec3::ZERO, col, 0.06, 2.9, FxKind::Leaf);
        }
        // The biome's own nature around the player.
        self.biome_ambience(dt, k, pos, up, t1, t2);
        // Splash on entering water, dust on landing and while sprinting.
        let vup = self.player.body.vel.dot(up);
        let in_water = self.player.body.in_water;
        if in_water && !self.fx_was_in_water && self.fx_prev_vup < -3.0 {
            let n = ((-self.fx_prev_vup).min(20.0) * 2.0 * k as f64) as usize;
            for _ in 0..n {
                let v = up * (3.0 + self.mobs.rand() * 4.0) + jitter(&mut self.mobs, 5.0);
                let col = if self.mobs.rand() < 0.5 { [0.75, 0.88, 1.0] } else { [0.35, 0.60, 0.95] };
                self.mobs.fx(pos + up * 0.6, v, col, 0.07, 1.1, FxKind::Fall);
            }
        }
        let ground = self.player.body.on_ground;
        let under = g.neighbor(feet, 2, -1).and_then(|c| self.chunks.get_block(c)).unwrap_or(Block::Dirt);
        let dust = match under {
            Block::Sand | Block::Sandstone => [0.86, 0.80, 0.58],
            Block::Snow | Block::SnowyGrass => [0.95, 0.97, 1.0],
            Block::Grass | Block::Dirt | Block::Podzol => [0.55, 0.42, 0.30],
            _ => [0.62, 0.60, 0.58],
        };
        if ground && !self.fx_was_ground && self.fx_prev_vup < -7.0 {
            let n = ((-self.fx_prev_vup).min(25.0) * 0.8 * k as f64) as usize;
            for _ in 0..n {
                let v = jitter(&mut self.mobs, 4.0) + up * (0.5 + self.mobs.rand());
                self.mobs.fx(pos + up * 0.1, v, dust, 0.08, 0.8, FxKind::Rise);
            }
        }
        if ground && self.player.sprinting {
            self.fx_dust -= dt * k;
            if self.fx_dust <= 0.0 {
                self.fx_dust = 0.07;
                let back = -self.player.body.vel.normalize_or_zero();
                let v = back * 0.8 + up * 0.4 + jitter(&mut self.mobs, 0.6);
                let o = jitter(&mut self.mobs, 0.4);
                self.mobs.fx(pos + up * 0.08 + o, v, dust, 0.07, 0.6, FxKind::Rise);
            }
        }
        self.fx_was_in_water = in_water;
        self.fx_was_ground = ground;
        self.fx_prev_vup = vup;
    }

    /// Hits the creature in the crosshair if there is one within reach and nothing solid in
    /// front of it. Returns true when a creature was targeted (even during the cooldown, so a
    /// tap on a mob never places a block on it).
    fn start_rest(&mut self, r: Rest) {
        if self.rest.is_none() {
            self.rest_camera = Some(self.camera_mode);
        }
        self.camera_mode = CameraMode::ThirdBack;
        self.player.body.vel = DVec3::ZERO;
        self.rest = Some(r);
    }

    fn end_rest(&mut self) {
        if let Some(r) = self.rest.take() {
            let back = match r {
                Rest::Sleep { back, .. } | Rest::Sit { back, .. } => back,
            };
            self.player.body.pos = back;
            self.player.body.vel = DVec3::ZERO;
        }
        if let Some(m) = self.rest_camera.take() {
            self.camera_mode = m;
        }
    }

    /// Lies the player down on the bed at `pos`: the spawn point is already set; at night
    /// the sleep runs the clock to the morning.
    fn lie_down(&mut self, pos: wolfcraft_engine::cubesphere::Cell, night: bool) {
        if !night {
            self.item_name = Some(("Точка возрождения установлена. Спать можно ночью".into(), 3.0));
            return;
        }
        let g = self.chunks.grid();
        let c = g.cell_center(pos);
        let up = wolfcraft_engine::physics::up_at(c);
        // The bed reaches one cell towards its pillow (see the bed's geometry and rotation).
        let rot = wolfcraft_engine::construction::entry(self.chunks.get_block(pos).map(|b| b as u16).unwrap_or(0)).map(|e| e.1).unwrap_or(0);
        let (di, dj) = [(0, 1), (-1, 0), (0, -1), (1, 0)][rot as usize % 4];
        let head = g.offset(pos, di, dj, 0).map(|n| g.cell_center(n)).unwrap_or(c);
        let dir = (head - c).normalize_or_zero();
        let dir = if dir.length_squared() > 0.5 { dir } else { self.player.heading };
        let len = wolfcraft_engine::player::DAY_LENGTH;
        let t0 = self.player.world_time;
        let f = wolfcraft_engine::player::day_fraction(t0);
        let day = (t0 / len).floor() + if f >= 0.76 { 1.0 } else { 0.0 };
        let at = (c + head) * 0.5 + up * 0.1;
        self.start_rest(Rest::Sleep { at, dir, back: self.player.body.pos, t: 0.0, t0, t1: (day + 0.27) * len });
    }

    /// Seconds a night's sleep takes on screen.
    pub const SLEEP_SECS: f32 = 5.0;

    /// How dark the screen is while sleeping (0..1): fades in, stays, fades out.
    pub fn sleep_fade(&self) -> f32 {
        match self.rest {
            Some(Rest::Sleep { t, .. }) => {
                let k = t / Self::SLEEP_SECS;
                ((k / 0.25).min(1.0).min((1.0 - k) / 0.2)).clamp(0.0, 1.0) * 0.92
            }
            _ => 0.0,
        }
    }

    /// Advances sleeping and sitting. Returns true while the player rests (no walking).
    fn tick_rest(&mut self, input: &InputFrame, dt: f32) -> bool {
        let Some(r) = self.rest else { return false };
        let moved = input.jump || input.jump_pressed || input.forward.abs() > 0.3 || input.strafe.abs() > 0.3;
        match r {
            Rest::Sit { at, .. } => {
                if moved || self.player.survival.dead {
                    self.end_rest();
                    return false;
                }
                self.player.body.pos = at;
                self.player.body.vel = DVec3::ZERO;
            }
            Rest::Sleep { at, dir, back, t, t0, t1 } => {
                let t = t + dt;
                let hurt = self.player.survival.hurt_flash > 0.0;
                if t >= Self::SLEEP_SECS || self.player.survival.dead || hurt || (moved && t > 0.6) {
                    let done = t >= Self::SLEEP_SECS;
                    self.end_rest();
                    if done {
                        self.player.world_time = t1;
                        self.player.survival.health = (self.player.survival.health + 4.0).min(wolfcraft_engine::player::MAX_HEALTH);
                        self.item_name = Some(("Доброе утро!".into(), 3.0));
                    } else {
                        self.item_name = Some(("Сон прерван".into(), 2.0));
                    }
                    return false;
                }
                // The night runs by while the eyes are closed (eased in and out).
                let k = (t / Self::SLEEP_SECS).clamp(0.0, 1.0);
                let e = k * k * (3.0 - 2.0 * k);
                self.player.world_time = t0 + (t1 - t0) * e as f64;
                self.player.body.pos = at;
                self.player.body.vel = DVec3::ZERO;
                self.rest = Some(Rest::Sleep { at, dir, back, t, t0, t1 });
            }
        }
        true
    }

    /// First arrow in the bag (plain or tipped), in slot order.
    fn next_arrow(&self) -> Option<wolfcraft_engine::items::ItemId> {
        use wolfcraft_engine::items::ids;
        self.player.inventory.slots.iter().flatten().map(|s| s.id).find(|id| *id == ids::ARROW || (ids::TIPPED_BASE..=ids::TIPPED_LAST).contains(id))
    }

    /// Everything within `r` (along the ground) of `at`: the player takes `dmg` (creative is
    /// lifted out of the way instead), creatures too.
    fn stellar_hurt(&mut self, at: DVec3, r: f64, dmg: f32) {
        let up = wolfcraft_engine::physics::up_at(at);
        let flat = |p: DVec3| {
            let d = p - at;
            (d - up * d.dot(up)).length()
        };
        if !self.player.survival.dead && flat(self.player.body.pos) < r {
            if self.player.is_creative() {
                self.player.body.pos += up * 25.0;
                self.player.body.vel = DVec3::ZERO;
            } else {
                self.player.damage(dmg, self.cfg.difficulty);
            }
        }
        for i in 0..self.mobs.mobs.len() {
            if self.mobs.mobs[i].hp > 0.0 && flat(self.mobs.mobs[i].body.pos) < r {
                self.mobs.hurt(i, dmg, at);
            }
        }
    }

    /// Sets blocks for the remote's skills (and sends them to the server).
    fn stellar_set(&mut self, cells: Vec<(wolfcraft_engine::cubesphere::Cell, wolfcraft_engine::block::Block)>) {
        use wolfcraft_engine::block::Block as B;
        for (c, b) in cells {
            if c.k < 1 {
                continue;
            }
            let Some(old) = self.chunks.get_block(c) else { continue };
            if old == b || old == B::Bedrock {
                continue;
            }
            if self.chunks.set_block(c, b) {
                self.net_out.push((c, b));
                if b == B::Air && self.stellar_removed.len() < 600 {
                    self.stellar_removed.push((c, old));
                }
            }
        }
    }

    /// Guardians in cut-scenes: when one wakes (summoned, revived, or you come into its fight)
    /// the camera circles it under its name — «Бой начался!»; every new phase (rage) gets a
    /// short scene too; a hit on it throws sparks of its colour and jolts the view; while it
    /// winds up an attack, energy gathers around it.
    fn tick_boss_film(&mut self, dt: f32) {
        use wolfcraft_engine::mobs::FxKind;
        let boss = self.mobs.mobs.iter().find(|m| m.boss.is_some() && m.hp > 0.0).map(|m| {
            let r = m.boss.as_ref().unwrap();
            (m.centre(), m.hp, r.kind, r.casting)
        });
        if let Some((at, hp, kind, casting)) = boss {
            let phase = kind.phase(hp);
            match self.boss_seen {
                None => {
                    self.skip_film = false;
                    self.boss_film = Some(BossFilm { at, t: 0.0, dur: 3.6 });
                    self.title = Some((kind.name().to_uppercase(), "Бой начался!".into(), 0.0));
                    self.flash = Some((kind.color(), 0.5));
                    self.shake = 0.7;
                    self.booms.push((at, 5.0, 0.3));
                }
                Some((seen_hp, seen_phase)) => {
                    if phase > seen_phase {
                        self.skip_film = false;
                        self.boss_film = Some(BossFilm { at, t: 0.0, dur: 2.4 });
                        self.title = Some((format!("ЯРОСТЬ · ФАЗА {phase}"), kind.name().into(), 0.0));
                        self.slowmo = 0.8;
                        self.shake = 0.9;
                        self.flash = Some(([1.0, 0.3, 0.1], 0.45));
                    } else if hp < seen_hp - 0.01 {
                        // Hit: sparks and a jolt, bigger for bigger blows.
                        let k = ((seen_hp - hp) / 12.0).clamp(0.15, 1.0);
                        self.shake = self.shake.max(0.25 * k);
                        let c = kind.color();
                        for i in 0..(8.0 + 16.0 * k) as usize {
                            let a = i as f64 * 2.399;
                            let v = DVec3::new(a.cos(), 0.6 + (i % 3) as f64 * 0.3, a.sin()) * 7.0;
                            self.mobs.fx_glow(at, v, if i % 2 == 0 { [1.0, 1.0, 0.9] } else { c }, 0.18, 0.5, FxKind::Fall);
                        }
                    }
                }
            }
            self.boss_seen = Some((hp, phase));
            // Winding up an attack: energy pulled into the guardian.
            if casting > 0.0 {
                let up = wolfcraft_engine::physics::up_at(at);
                for _ in 0..3 {
                    let v = DVec3::new(self.mobs.rand() - 0.5, self.mobs.rand() - 0.5, self.mobs.rand() - 0.5).normalize_or_zero();
                    let from = at + v * 4.0 + up * 0.5;
                    self.mobs.fx_glow(from, (at - from) * 3.0, kind.color(), 0.14, 0.35, FxKind::Rise);
                }
            }
        } else if self.boss_film.as_ref().is_none_or(|f| f.dur < 3.9) {
            // Gone without a death scene (left, despawned): forget it.
            self.boss_seen = None;
        }
        // The film's camera: circling the guardian, slowly pulling away.
        if let Some(mut f) = self.boss_film.take() {
            f.t += dt;
            if f.t < f.dur {
                if self.stellar_ui.cutscenes && !self.skip_film && self.stellar.is_none() {
                    let at = boss.map(|b| b.0).unwrap_or(f.at);
                    let up = wolfcraft_engine::physics::up_at(at);
                    let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
                    let ta = up.cross(helper).normalize();
                    let tb = up.cross(ta);
                    let a = f.t as f64 * 0.55;
                    let r = 9.0 + f.t as f64 * 1.8;
                    let eye = at + up * (3.5 + f.t as f64 * 0.8) + (ta * a.cos() + tb * a.sin()) * r;
                    let target = at + up * 1.5;
                    self.cutscene = Some(Camera { eye, forward: (target - eye).normalize_or_zero(), up });
                }
                self.boss_film = Some(f);
            } else if self.stellar.is_none() {
                self.cutscene = None;
            }
        }
    }

    /// A quick ring of light on the ground (each shot of the orbital barrage).
    fn explosives_flash_ring(&mut self, at: DVec3, r: f64) {
        let up = wolfcraft_engine::physics::up_at(at);
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let ta = up.cross(helper).normalize();
        let tb = up.cross(ta);
        for k in 0..16 {
            let a = k as f64 * std::f64::consts::TAU / 16.0;
            let d = ta * a.cos() + tb * a.sin();
            self.mobs.fx_glow(at + up * 0.5 + d * 0.5, d * r * 3.0, [0.5, 1.0, 0.6], 0.2, 0.4, wolfcraft_engine::mobs::FxKind::Rise);
        }
    }

    /// «Звёздный молот»: a slam into the ground — a shock ring throws creatures up into the
    pub fn ship_hammer_label(&self) -> &'static str {
        ["Монтаж", "Демонтаж", "Диагностика"][self.ship_hammer_mode as usize % 3]
    }

    pub fn next_ship_hammer_mode(&mut self) {
        self.ship_hammer_mode = (self.ship_hammer_mode + 1) % 3;
        self.item_name = Some((format!("Корабельный молот: {}", self.ship_hammer_label()), 2.0));
    }

    // Never destroy storage, machines, scripts or unbreakable world layers with a shockwave.
    fn hammer_breakable(&self, c: wolfcraft_engine::cubesphere::Cell, b: wolfcraft_engine::block::Block) -> bool {
        use wolfcraft_engine::block::Block as B;
        b.is_solid() && b.info().hardness >= 0.0
            && !matches!(b.base_kind(), B::Bedrock | B::InnerCore | B::Chest | B::ElectricFurnace | B::MachineCasing | B::EnchantTable | B::Battery | B::CommandBlock | B::Terminal | B::Sign | B::SignB)
            && !wolfcraft_engine::construction::is_container(b)
            && wolfcraft_engine::wolftech::kind(b).is_none()
            && wolfcraft_engine::kinetic::part(b).is_none()
            && !self.containers.contains_key(&c) && !self.tech.contains_key(&c)
            && !self.block_commands.contains_key(&c)
    }

    fn use_ship_hammer(&mut self, eye: DVec3, look: DVec3) {
        use wolfcraft_engine::{block::Block as B, kinetic as kn, wolftech as wt};
        if self.player.sneaking { self.next_ship_hammer_mode(); return; }
        if self.hammer_cd > 0.0 { return; }
        let Some(h) = raycast(&self.chunks, eye, look, 7.0) else { return; };
        let (c, b) = (h.cell, h.block);
        if kn::part(b).is_none() && wt::kind(b).is_none() {
            self.item_name = Some(("Нужен механизм WolfTech или Механики".into(), 2.0));
            return;
        }
        let message = match self.ship_hammer_mode {
            0 => {
                let axis = kn::part(b).and_then(kn::axis).unwrap_or(0);
                let next = kn::with_axis(b, (axis + 1) % 3);
                if next == b { "Этот механизм не имеет поворотной оси".into() }
                else if self.chunks.set_block(c, next) {
                    self.net_out.push((c, next)); self.kin_t = 0.0;
                    if !self.player.is_creative() { self.player.inventory.wear_selected(); }
                    "Ось механизма повёрнута".into()
                } else { "Не удалось повернуть механизм".into() }
            }
            1 => {
                let occupied = self.containers.get(&c).is_some_and(|v| v.iter().any(Option::is_some))
                    || self.tech.get(&c).is_some_and(|t| t.energy > 0.0 || t.burn > 0.0 || t.progress > 0.0 || t.working)
                    || self.kin.get(&c).is_some_and(|n| n.progress > 0.0 || n.crank > 0.0)
                    || self.furnace_fuel.get(&c).is_some_and(|v| *v > 0.0)
                    || self.block_commands.contains_key(&c);
                if occupied { "Сначала опустошите и остановите механизм (включая энергию)".into() }
                else if self.chunks.set_block(c, B::Air) {
                    self.net_out.push((c, B::Air));
                    self.containers.remove(&c); self.tech.remove(&c); self.kin.remove(&c);
                    self.furnace_fuel.remove(&c); self.kin_t = 0.0;
                    self.drops.pop_from_block(wolfcraft_engine::items::ItemStack::of_block(b, 1), self.chunks.grid().cell_center(c));
                    if !self.player.is_creative() { self.player.inventory.wear_selected(); }
                    "Механизм демонтирован без потери блока".into()
                } else { "Не удалось демонтировать механизм".into() }
            }
            _ => {
                let mut text = b.info().name.to_string();
                if let Some(n) = self.kin.get(&c) {
                    text.push_str(&format!(" · {:.0} RPM · {:.0}/{:.0} SU{}", n.speed, n.load, n.cap, if n.load > n.cap { " · Перегрузка" } else { "" }));
                }
                if let Some(t) = self.tech.get(&c) {
                    text.push_str(&format!(" · {:.0} ЭЕ · {:.1} ЭЕ/с", t.energy, t.flow));
                }
                text
            }
        };
        self.hammer_cd = 0.35;
        self.item_name = Some((message, 3.0));
    }

    /// air, a crater opens in front, dust races out.
    fn hammer_slam(&mut self, eye: DVec3, look: DVec3) {
        use wolfcraft_engine::block::Block as B;
        use wolfcraft_engine::mobs::FxKind;
        if self.hammer_cd > 0.0 {
            return;
        }
        let Some(hit) = raycast(&self.chunks, eye, look, 7.0) else {
            self.item_name = Some(("Наведите молот на блок в пределах 7 м".into(), 1.5));
            return;
        };
        self.hammer_cd = 1.4;
        let at = self.chunks.grid().cell_center(hit.cell);
        let up = wolfcraft_engine::physics::up_at(at);
        let feet = at;
        // Creatures around are flung up and away.
        for i in 0..self.mobs.mobs.len() {
            let m = &self.mobs.mobs[i];
            let d = m.body.pos - feet;
            let flat = d.length();
            if m.hp > 0.0 && flat < 9.0 {
                self.mobs.hurt(i, 18.0 * (1.0 - flat as f32 / 12.0), feet);
                let m = &mut self.mobs.mobs[i];
                m.body.vel += up * 14.0 + (d - up * d.dot(up)).normalize_or_zero() * 8.0;
                m.body.on_ground = false;
            }
        }
        // A crater in front.
        let g = self.chunks.grid();
        let c0 = g.world_to_cell(at - up * 0.5);
        let mut cells = Vec::new();
        for di in -2..=2i32 {
            for dj in -2..=2i32 {
                for dk in -2..=1i32 {
                    if di * di + dj * dj + dk * dk <= 5 {
                        if let Some(c) = g.offset(c0, di, dj, dk) {
                            if self.chunks.get_block(c).is_some_and(|b| self.hammer_breakable(c, b)) {
                                cells.push((c, B::Air));
                            }
                        }
                    }
                }
            }
        }
        self.stellar_set(cells);
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let ta = up.cross(helper).normalize();
        let tb = up.cross(ta);
        for k in 0..36 {
            let a = k as f64 * std::f64::consts::TAU / 36.0;
            let d = ta * a.cos() + tb * a.sin();
            self.mobs.fx(feet + d + up * 0.2, d * 14.0 + up * 1.5, [0.6, 0.55, 0.48], 0.45, 1.0, FxKind::Rise);
            if k % 3 == 0 {
                self.mobs.fx_glow(feet + d + up * 0.4, d * 18.0, [0.75, 0.85, 1.0], 0.25, 0.5, FxKind::Rise);
            }
        }
        self.shake = 0.8;
        self.flash = Some(([0.8, 0.9, 1.0], 0.25));
        self.booms.push((at, 5.0, 0.01));
    }

    /// The big moment of a strike: flash, shake, slow motion, a dome of light racing out,
    /// embers left burning and the rumble rolling on after.
    fn epic_impact(&mut self, at: DVec3, radius: f64, color: [f32; 3], flash: f32) {
        use wolfcraft_engine::mobs::FxKind;
        self.flash = Some((color, flash));
        self.shake = 1.0;
        self.slowmo = 0.7;
        let up = wolfcraft_engine::physics::up_at(at);
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let ta = up.cross(helper).normalize();
        let tb = up.cross(ta);
        for k in 0..90 {
            let a = k as f64 * 2.399;
            let e = (k as f64 / 90.0).sqrt();
            let dir = (ta * a.cos() + tb * a.sin()) * (1.0 - e * e).sqrt() + up * e;
            let bright = [(color[0] + 0.4).min(1.0), (color[1] + 0.4).min(1.0), (color[2] + 0.4).min(1.0)];
            self.mobs.fx_glow(at + up * 0.5, dir * (radius * 2.2 + 10.0), if k % 2 == 0 { bright } else { color }, 0.45, 0.9, FxKind::Rise);
        }
        self.embers.push((at, 9.0, radius.max(3.0)));
        if self.embers.len() > 8 {
            self.embers.remove(0);
        }
        self.booms.push((at, 6.0, 0.45));
        self.booms.push((at, 4.0, 1.1));
        self.booms.push((at, 2.5, 2.0));
    }

    /// Embers and smoke over fresh craters, late rolling booms, chips of the torn-out blocks,
    /// the shake dying down.
    fn tick_epic(&mut self, dt: f32, events: &mut Vec<InteractionEvent>) {
        use wolfcraft_engine::mobs::FxKind;
        self.shake = (self.shake - dt * 1.4).max(0.0);
        self.hammer_cd = (self.hammer_cd - dt).max(0.0);
        if let Some(t) = &mut self.title {
            t.2 += dt;
            if t.2 > 4.0 {
                self.title = None;
            }
        }
        for e in &mut self.embers {
            e.1 -= dt;
        }
        self.embers.retain(|e| e.1 > 0.0);
        for (at, left, r) in self.embers.clone() {
            let up = wolfcraft_engine::physics::up_at(at);
            let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
            let ta = up.cross(helper).normalize();
            let tb = up.cross(ta);
            let k = (left / 9.0) as f64;
            for _ in 0..3 {
                let a = self.mobs.rand() * std::f64::consts::TAU;
                let d = self.mobs.rand() * r;
                let p = at + (ta * a.cos() + tb * a.sin()) * d;
                if self.mobs.rand() < 0.5 {
                    let g = 0.4 + self.mobs.rand() as f32 * 0.4;
                    self.mobs.fx_glow(p, up * (1.0 + 2.0 * k), [1.0, g, 0.1], 0.12, 0.8, FxKind::Rise);
                } else {
                    let s = 0.15 + self.mobs.rand() as f32 * 0.15;
                    self.mobs.fx(p, up * 3.0 * k, [s, s, s], 0.6, 3.0, FxKind::Rise);
                }
            }
        }
        let mut due = Vec::new();
        for b in &mut self.booms {
            b.2 -= dt;
            if b.2 <= 0.0 {
                due.push((b.0, b.1));
            }
        }
        self.booms.retain(|b| b.2 > 0.0);
        for (at, power) in due {
            events.push(InteractionEvent::Exploded { at, power });
            self.shake = self.shake.max(0.4);
        }
        // Chunks of the world fly off where a strike tore it out (a sample, not every block).
        let removed = std::mem::take(&mut self.stellar_removed);
        let step = (removed.len() / 90).max(1);
        for (pos, block) in removed.into_iter().step_by(step) {
            events.push(InteractionEvent::Blasted { pos, block });
        }
    }

    /// The remote's skills, step by step (marker → strike → aftermath):
    /// * Рельсотрон: a red laser marks the spot, then a white beam from space wipes out a
    ///   round column of the world, from high above down deep into the ground.
    /// * Игла: an ember ring, a black needle falls without a sound; a shock ring flattens the
    ///   land into a molten crater, the boom comes after; the needle stays standing.
    /// * Семь звёзд: the stars fall (see `star_queue`), then the lines of the Big Dipper burn
    ///   into the ground as trenches of magma.
    /// * Тессеракт: a rainbow cage grids the land into cells; one by one they are torn out and
    ///   lifted to the sky, leaving a stepped pit with a rainbow grid; a wireframe hypercube
    ///   hangs above it for a while.
    fn tick_stellar(&mut self, dt: f32, events: &mut Vec<InteractionEvent>) {
        use wolfcraft_engine::block::Block as B;
        use wolfcraft_engine::mobs::FxKind;
        // The flash fades, the cinema bars slide in and out.
        if let Some((c, a)) = self.flash {
            self.flash = (a > 0.02).then_some((c, a - dt * 1.6));
        }
        // The orbital barrage is aimed by hand: no film for it.
        let film = (self.stellar.as_ref().is_some_and(|s| s.mode != 10) || self.boss_film.is_some()) && self.stellar_ui.cutscenes && !self.skip_film;
        self.cinema = (self.cinema + if film { dt * 3.0 } else { -dt * 3.0 }).clamp(0.0, 1.0);
        if !film {
            self.cutscene = None;
        }
        let Some(mut s) = self.stellar.take() else { return };
        let pw = self.stellar_ui.power.clamp(0.5, 2.5) as f64;
        let dmg_k = self.stellar_ui.damage.clamp(0.0, 5.0);
        let t0 = s.t;
        s.t += dt * self.stellar_ui.speed.clamp(0.3, 3.0);
        let t = s.t;
        let crossed = |x: f32| t0 < x && t >= x;
        let at = s.at;
        let up = wolfcraft_engine::physics::up_at(at);
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let ta = up.cross(helper).normalize();
        let tb = up.cross(ta);
        let ring = |a: f64| ta * a.cos() + tb * a.sin();
        let g = self.chunks.grid();
        let finished;
        // The film: where the camera is during each skill.
        if film {
            let look_at = |eye: DVec3, target: DVec3| Camera { eye, forward: (target - eye).normalize_or_zero(), up };
            let side = s.side;
            self.cutscene = Some(match s.mode {
                0 if t < 2.2 => {
                    // Out to space above the target…
                    let h = 20.0 + (t as f64 / 2.2).powi(2) * 160.0;
                    look_at(at + up * h - side * h * 0.35, at)
                }
                0 => {
                    // …and riding the beam back down.
                    let k = ((t as f64 - 2.2) / 1.2).clamp(0.0, 1.0);
                    let h = 180.0 * (1.0 - k) + 18.0;
                    look_at(at + up * h - side * (h * 0.35 + 12.0), at)
                }
                1 => look_at(at - side * 48.0 + up * 16.0, at + up * 8.0),
                2 => look_at(at + up * 60.0 + ring(t as f64 * 0.15) * 32.0, at),
                3 => look_at(at + up * 30.0 + ring(t as f64 * 0.35) * 48.0, at + up * 6.0),
                5 => look_at(at + up * 45.0 - side * 55.0, at),
                6 => look_at(at - side * 70.0 * self.stellar_ui.power as f64 + up * 25.0 + up.cross(side) * 20.0, at),
                8 => look_at(at + up * 20.0 - side * 60.0 + ring(t as f64 * 0.2) * 10.0, at + up * 8.0),
                _ => look_at(at + up * 14.0 + ring(t as f64 * 0.7) * 26.0, at + up * 6.0),
            });
        }
        match s.mode {
            0 => {
                #[allow(non_snake_case)]
                let R = (13.0 * pw).round() as i32;
                if t < 2.2 {
                    // Red laser from the sky and a shrinking red ring on the ground.
                    for _ in 0..4 {
                        let h = self.mobs.rand() * 70.0;
                        self.mobs.fx_glow(at + up * h, DVec3::ZERO, [1.0, 0.15, 0.12], 0.10, 0.12, FxKind::Rise);
                    }
                    let rr = R as f64 * (1.0 - t as f64 / 2.6);
                    for k in 0..6 {
                        let a = t as f64 * 3.0 + k as f64 * 1.047;
                        self.mobs.fx_glow(at + up * 0.6 + ring(a) * rr, DVec3::ZERO, [1.0, 0.2, 0.15], 0.14, 0.2, FxKind::Rise);
                    }
                    // Energy gathering: sparks pulled in from all around, faster and faster.
                    let n = (2.0 + t * 4.0) as usize;
                    for _ in 0..n {
                        let a = self.mobs.rand() * std::f64::consts::TAU;
                        let from = at + ring(a) * (18.0 + self.mobs.rand() * 10.0) + up * (self.mobs.rand() * 14.0);
                        self.mobs.fx_glow(from, (at + up * 2.0 - from) * (1.2 + t as f64), [1.0, 0.35, 0.3], 0.1, 0.6, FxKind::Rise);
                    }
                    self.shake = self.shake.max(t / 2.2 * 0.35);
                }
                if crossed(2.2) {
                    let c0 = s.cell;
                    let mut cells = Vec::new();
                    for di in -R..=R {
                        for dj in -R..=R {
                            if di * di + dj * dj > R * R {
                                continue;
                            }
                            for dk in -28..=40 {
                                if let Some(c) = g.offset(c0, di, dj, dk) {
                                    cells.push((c, B::Air));
                                }
                            }
                        }
                    }
                    self.stellar_set(cells);
                    self.stellar_hurt(at, R as f64 + 0.5, 60.0 * dmg_k);
                    self.epic_impact(at, R as f64, [1.0, 0.95, 0.95], 0.95);
                    events.push(InteractionEvent::Exploded { at, power: 14.0 });
                }
                if (2.2..3.4).contains(&t) {
                    // The beam: a blinding white-blue column.
                    for _ in 0..14 {
                        let h = self.mobs.rand() * 90.0 - 28.0;
                        let a = self.mobs.rand() * std::f64::consts::TAU;
                        let rr = self.mobs.rand() * R as f64;
                        let col = if self.mobs.rand() < 0.5 { [1.0, 1.0, 1.0] } else { [0.70, 0.85, 1.0] };
                        self.mobs.fx_glow(at + up * h + ring(a) * rr, up * 6.0, col, 0.5, 0.3, FxKind::Rise);
                    }
                }
                finished = t > 3.6;
            }
            1 => {
                #[allow(non_snake_case)]
                let R = (22.0 * pw).round() as i32;
                if t < 1.8 {
                    for k in 0..8 {
                        let a = t as f64 * 2.0 + k as f64 * 0.785;
                        let rr = 6.0 * (1.0 - t as f64 / 2.2);
                        self.mobs.fx_glow(at + up * 0.6 + ring(a) * rr, up * 0.4, [1.0, 0.45, 0.10], 0.16, 0.25, FxKind::Rise);
                    }
                } else if t < 2.6 {
                    // The needle drops: a long black streak racing down.
                    let h = 80.0 * (1.0 - (t as f64 - 1.8) / 0.8);
                    for k in 0..10 {
                        self.mobs.fx(at + up * (h + k as f64 * 2.0), -up * 40.0, [0.05, 0.05, 0.07], 0.45, 0.2, FxKind::Rise);
                    }
                }
                if crossed(2.6) {
                    let c0 = s.cell;
                    let mut cells = Vec::new();
                    for di in -R..=R {
                        for dj in -R..=R {
                            let d2 = di * di + dj * dj;
                            if d2 > R * R {
                                continue;
                            }
                            let r = (d2 as f64).sqrt();
                            // Land above the crater floor is flattened away.
                            for dk in 1..=14 {
                                if let Some(c) = g.offset(c0, di, dj, dk) {
                                    cells.push((c, B::Air));
                                }
                            }
                            // The bowl: deeper in the middle, molten floor.
                            let depth = ((1.0 - r / R as f64) * 5.0).round() as i32;
                            for dk in (1 - depth)..=0 {
                                if let Some(c) = g.offset(c0, di, dj, dk) {
                                    cells.push((c, B::Air));
                                }
                            }
                            if let Some(c) = g.offset(c0, di, dj, -depth) {
                                cells.push((c, if r < R as f64 * 0.6 { B::Magma } else { B::Basalt }));
                            }
                        }
                    }
                    // The needle stays, sticking out of the crater.
                    for dk in -8..=22 {
                        if let Some(c) = g.offset(c0, 0, 0, dk) {
                            cells.push((c, B::Obsidian));
                        }
                    }
                    self.stellar_set(cells);
                    self.stellar_hurt(at, R as f64, 40.0 * dmg_k);
                    self.epic_impact(at, R as f64, [1.0, 0.55, 0.15], 0.75);
                    // A silent hit: a ring of dust and heat racing out.
                    for k in 0..48 {
                        let a = k as f64 * std::f64::consts::TAU / 48.0;
                        self.mobs.fx_glow(at + up * 0.8 + ring(a) * 2.0, ring(a) * 22.0, [1.0, 0.55, 0.15], 0.5, 0.9, FxKind::Rise);
                        self.mobs.fx(at + up * 0.4 + ring(a) * 2.0, ring(a) * 16.0 + up, [0.55, 0.50, 0.45], 0.6, 1.4, FxKind::Rise);
                    }
                }
                // …and the boom comes after.
                if crossed(3.4) {
                    events.push(InteractionEvent::Exploded { at, power: 12.0 });
                }
                finished = t > 3.6;
            }
            2 => {
                // After the last star: the constellation's lines burn as trenches.
                if self.star_queue.is_empty() && t > 6.5 && s.step == 0 {
                    s.step = 1;
                    let mut cells = Vec::new();
                    for w in s.pts.windows(2) {
                        let n = ((w[1] - w[0]).length() * 2.0).ceil() as i32;
                        for k in 0..=n {
                            let p = w[0].lerp(w[1], k as f64 / n.max(1) as f64);
                            let pu = wolfcraft_engine::physics::up_at(p);
                            if let Some(h) = wolfcraft_engine::physics::raycast(&self.chunks, p + pu * 12.0, -pu, 30.0) {
                                cells.push((h.cell, B::Air));
                                if let Some(c) = g.offset(h.cell, 0, 0, -1) {
                                    cells.push((c, B::Magma));
                                }
                            }
                        }
                    }
                    self.stellar_set(cells);
                    for p in s.pts.clone() {
                        self.stellar_hurt(p, 2.0, 20.0 * dmg_k);
                    }
                    let mid = s.pts[s.pts.len() / 2];
                    self.epic_impact(mid, 8.0, [0.6, 0.75, 1.0], 0.55);
                }
                if s.step == 1 && s.pts.len() > 1 {
                    // Flames along the trenches.
                    for _ in 0..6 {
                        let i = ((self.mobs.rand() * (s.pts.len() - 1) as f64) as usize).min(s.pts.len() - 2);
                        let p = s.pts[i].lerp(s.pts[i + 1], self.mobs.rand());
                        let pu = wolfcraft_engine::physics::up_at(p);
                        let gcol = 0.5 + self.mobs.rand() as f32 * 0.3;
                        self.mobs.fx_glow(p + pu * 0.2, pu * 2.0, [1.0, gcol, 0.1], 0.18, 0.7, FxKind::Rise);
                    }
                }
                finished = t > 11.0;
            }
            4 => {
                // Black hole: hangs a little above the ground, pulls everything in, eats a ball
                // of the world shell by shell, then collapses in a violet flash.
                let c = at + up * 6.0;
                let r_max = 11.0 * pw;
                if t < 3.5 {
                    for k in 0..10 {
                        let a = t as f64 * 5.0 + k as f64 * 0.628;
                        let rr = r_max * 1.4 * (1.0 - ((t as f64 * 1.3 + k as f64 * 0.1) % 1.0));
                        let col = if k % 3 == 0 { [0.75, 0.45, 1.0] } else { [0.08, 0.05, 0.12] };
                        let p = c + ring(a) * rr + up * ((k as f64 - 5.0) * 0.3);
                        let toward = (c - p).normalize_or_zero();
                        if k % 3 == 0 {
                            self.mobs.fx_glow(p, toward * 8.0 + ring(a + 1.57) * 6.0, col, 0.18, 0.5, FxKind::Rise);
                        } else {
                            self.mobs.fx(p, toward * 8.0 + ring(a + 1.57) * 6.0, col, 0.3, 0.5, FxKind::Rise);
                        }
                    }
                    // The core: a ball of black cubes.
                    for _ in 0..4 {
                        let v = DVec3::new(self.mobs.rand() - 0.5, self.mobs.rand() - 0.5, self.mobs.rand() - 0.5).normalize_or_zero();
                        self.mobs.fx(c + v * 1.2, DVec3::ZERO, [0.02, 0.02, 0.03], 0.8, 0.15, FxKind::Rise);
                    }
                    // The pull.
                    let pull = |p: DVec3| (c - p).normalize_or_zero() * (12.0 / (1.0 + (c - p).length() * 0.15));
                    for m in &mut self.mobs.mobs {
                        if (m.body.pos - c).length() < r_max * 3.0 {
                            m.body.vel += pull(m.body.pos) * dt as f64;
                            m.body.on_ground = false;
                        }
                    }
                    if !self.player.is_creative() && (self.player.body.pos - c).length() < r_max * 3.0 {
                        self.player.body.vel += pull(self.player.body.pos) * dt as f64 * 0.6;
                    }
                    // Eats the world from the inside out.
                    let r_now = r_max * (t as f64 / 3.5);
                    let r_was = r_max * (t0 as f64 / 3.5).max(0.0);
                    if r_now.ceil() > r_was.ceil() {
                        let n = r_now.ceil() as i32;
                        let mut cells = Vec::new();
                        if let Some(cc) = g.offset(s.cell, 0, 0, 6) {
                            for di in -n..=n {
                                for dj in -n..=n {
                                    for dk in -n..=n {
                                        let d = ((di * di + dj * dj + dk * dk) as f64).sqrt();
                                        if d <= r_now && d > r_was - 1.0 {
                                            if let Some(cl) = g.offset(cc, di, dj, dk) {
                                                cells.push((cl, B::Air));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        self.stellar_set(cells);
                    }
                }
                if crossed(3.5) {
                    self.stellar_hurt(c, r_max * 1.5, 50.0 * dmg_k);
                    self.epic_impact(c, r_max, [0.70, 0.40, 1.0], 0.9);
                    for k in 0..60 {
                        let a = k as f64 * 2.399;
                        let e = (k as f64 / 60.0) * 2.0 - 1.0;
                        let dir = ring(a) * (1.0 - e * e).sqrt() + up * e;
                        self.mobs.fx_glow(c, dir * 18.0, [0.85, 0.6, 1.0], 0.35, 0.8, FxKind::Rise);
                    }
                    events.push(InteractionEvent::Exploded { at: c, power: 10.0 });
                }
                finished = t > 4.6;
            }
            5 => {
                // Meteor shower: burning rocks rain on the area for six seconds.
                let r = 18.0 * pw;
                if t < 6.0 {
                    let n = ((t / 0.11) as u32).saturating_sub(s.step);
                    for _ in 0..n.min(4) {
                        s.step += 1;
                        let a = self.mobs.rand() * std::f64::consts::TAU;
                        let d = self.mobs.rand().sqrt() * r;
                        let p = at + ring(a) * d;
                        self.explosives.call_meteor(p, s.side * -0.8 + ring(a + 1.0) * 0.3);
                    }
                    self.shake = self.shake.max(0.25);
                }
                if crossed(6.0) {
                    self.epic_impact(at, r * 0.5, [1.0, 0.5, 0.15], 0.4);
                }
                finished = t > 8.0;
            }
            6 => {
                // Sun spear: a golden line marks the path, then a beam from the sky sweeps
                // along it and cuts a burning trench.
                let len = 60.0 * pw;
                let half = len / 2.0;
                let w = (3.0 * pw).round() as i32;
                let side = s.side;
                if t < 1.5 {
                    for _ in 0..6 {
                        let k = self.mobs.rand() * len - half;
                        self.mobs.fx_glow(at + side * k + up * 0.6, up * 0.3, [1.0, 0.85, 0.3], 0.12, 0.3, FxKind::Rise);
                    }
                } else if t < 4.0 {
                    let k = ((t - 1.5) / 2.5) as f64;
                    let target = (k * len) as u32;
                    while s.step <= target {
                        let p = at + side * (s.step as f64 - half);
                        let pu = wolfcraft_engine::physics::up_at(p);
                        if let Some(h) = wolfcraft_engine::physics::raycast(&self.chunks, p + pu * 30.0, -pu, 60.0) {
                            let mut cells = Vec::new();
                            for di in -w..=w {
                                for dj in -w..=w {
                                    if di * di + dj * dj > w * w {
                                        continue;
                                    }
                                    for dk in -w..=6 {
                                        if let Some(c) = g.offset(h.cell, di, dj, dk) {
                                            cells.push((c, B::Air));
                                        }
                                    }
                                    if let Some(c) = g.offset(h.cell, di, dj, -w - 1) {
                                        cells.push((c, B::Magma));
                                    }
                                }
                            }
                            self.stellar_set(cells);
                            self.stellar_hurt(p, w as f64 + 1.0, 30.0 * dmg_k);
                        }
                        s.step += 1;
                    }
                    let p = at + side * (k * len - half);
                    for _ in 0..10 {
                        let h = self.mobs.rand() * 80.0;
                        let col = if self.mobs.rand() < 0.5 { [1.0, 0.95, 0.6] } else { [1.0, 0.75, 0.2] };
                        self.mobs.fx_glow(p + up * h, -up * 20.0, col, 0.45, 0.25, FxKind::Rise);
                    }
                    self.shake = self.shake.max(0.5);
                    self.flash = Some(([1.0, 0.85, 0.4], 0.15));
                }
                if crossed(4.0) {
                    self.epic_impact(at + side * half, 6.0, [1.0, 0.8, 0.3], 0.5);
                }
                finished = t > 5.0;
            }
            7 => {
                // Ice comet: falls in a cold blue streak and freezes everything around.
                let r = (14.0 * pw).round() as i32;
                if t < 1.6 {
                    let h = 90.0 * (1.0 - t as f64 / 1.6);
                    for k in 0..8 {
                        let col = if k % 2 == 0 { [0.7, 0.95, 1.0] } else { [1.0, 1.0, 1.0] };
                        self.mobs.fx_glow(at + up * (h + k as f64 * 1.5), up * 4.0, col, 0.4, 0.4, FxKind::Rise);
                    }
                }
                if crossed(1.6) {
                    let mut cells = Vec::new();
                    for di in -r..=r {
                        for dj in -r..=r {
                            let d2 = di * di + dj * dj;
                            if d2 > r * r {
                                continue;
                            }
                            let rr = (d2 as f64).sqrt();
                            // A small crater in the middle.
                            if rr < 4.0 {
                                for dk in -2..=3 {
                                    if let Some(c) = g.offset(s.cell, di, dj, dk) {
                                        cells.push((c, B::Air));
                                    }
                                }
                                if let Some(c) = g.offset(s.cell, di, dj, -3) {
                                    cells.push((c, B::PackedIce));
                                }
                                continue;
                            }
                            // The ground freezes: snow, ice, ice over water; ice spikes here and there.
                            let hsh = ((di * 73 + dj * 151) as u32).wrapping_mul(2654435761);
                            if let Some(c) = g.offset(s.cell, di, dj, 0) {
                                let top = self.chunks.get_block(c);
                                let b = if top.is_some_and(|b| b.is_water()) { B::Ice } else if hsh % 3 == 0 { B::PackedIce } else { B::Snow };
                                if top.is_some_and(|b| b != B::Air) {
                                    cells.push((c, b));
                                }
                            }
                            if hsh % 17 == 0 {
                                let tall = 2 + (hsh / 17 % 6) as i32;
                                for dk in 1..=tall {
                                    if let Some(c) = g.offset(s.cell, di, dj, dk) {
                                        cells.push((c, B::PackedIce));
                                    }
                                }
                            }
                        }
                    }
                    self.stellar_set(cells);
                    self.stellar_hurt(at, r as f64, 25.0 * dmg_k);
                    for m in &mut self.mobs.mobs {
                        if (m.body.pos - at).length() < r as f64 * 1.5 {
                            m.slow = 12.0;
                        }
                    }
                    self.epic_impact(at, r as f64, [0.6, 0.9, 1.0], 0.8);
                    for k in 0..40 {
                        let a = k as f64 * 2.399;
                        self.mobs.fx(at + up, ring(a) * 10.0 + up * 6.0, [0.95, 0.98, 1.0], 0.3, 2.5, FxKind::Leaf);
                    }
                }
                finished = t > 2.6;
            }
            8 => {
                // Supernova: a little sun swells over the ground, pulsing, the world shakes
                // harder and harder… then everything around is blown away.
                let r = (13.0 * pw).round().min(26.0) as i32;
                let c = at + up * 8.0;
                if t < 3.2 {
                    let k = t as f64 / 3.2;
                    let rr = 1.0 + k * 5.0 + (t as f64 * 12.0).sin() * 0.3;
                    for _ in 0..16 {
                        let v = DVec3::new(self.mobs.rand() - 0.5, self.mobs.rand() - 0.5, self.mobs.rand() - 0.5).normalize_or_zero();
                        let col = if self.mobs.rand() < 0.5 { [1.0, 1.0, 0.85] } else { [1.0, 0.7, 0.2] };
                        self.mobs.fx_glow(c + v * rr, v * 1.5, col, 0.5, 0.2, FxKind::Rise);
                    }
                    self.shake = self.shake.max(k as f32 * 0.8);
                    self.flash = Some(([1.0, 0.95, 0.7], (k * 0.25) as f32));
                }
                if crossed(3.2) {
                    let mut cells = Vec::new();
                    for di in -r..=r {
                        for dj in -r..=r {
                            for dk in -r..=r {
                                if di * di + dj * dj + dk * dk <= r * r {
                                    if let Some(cl) = g.offset(s.cell, di, dj, dk) {
                                        cells.push((cl, B::Air));
                                    }
                                }
                            }
                        }
                    }
                    self.stellar_set(cells);
                    self.stellar_hurt(at, r as f64 * 1.3, 80.0 * dmg_k);
                    self.epic_impact(c, r as f64 * 1.4, [1.0, 1.0, 0.9], 1.0);
                    self.slowmo = 1.2;
                }
                finished = t > 4.5;
            }
            9 => {
                // Planet crack: a drill of light bores straight down towards the core, cracks of
                // magma run out across the land, then lava bursts out of the hole as a geyser.
                let depth = (60.0 * pw) as i32;
                let w = 3;
                if t < 2.5 {
                    // Boring: the hole goes deeper and deeper.
                    let to = (depth as f32 * (t / 2.5)) as i32;
                    while (s.step as i32) < to {
                        let dk = -(s.step as i32);
                        let mut cells = Vec::new();
                        for di in -w..=w {
                            for dj in -w..=w {
                                if di * di + dj * dj <= w * w {
                                    if let Some(c) = g.offset(s.cell, di, dj, dk) {
                                        cells.push((c, B::Air));
                                    }
                                }
                            }
                        }
                        self.stellar_set(cells);
                        s.step += 1;
                    }
                    for _ in 0..10 {
                        let h = self.mobs.rand() * 100.0 - to as f64;
                        let col = if self.mobs.rand() < 0.5 { [1.0, 0.3, 0.2] } else { [1.0, 0.9, 0.8] };
                        self.mobs.fx_glow(at + up * h, -up * 30.0, col, 0.5, 0.3, FxKind::Rise);
                    }
                    self.shake = self.shake.max(0.4 + t * 0.15);
                }
                if crossed(2.5) {
                    // Cracks: eight lines of magma running out from the hole.
                    let len = (26.0 * pw) as i32;
                    let mut cells = Vec::new();
                    for k in 0..8 {
                        let a = k as f64 * std::f64::consts::FRAC_PI_4 + 0.3;
                        let (ca, sa) = (a.cos(), a.sin());
                        for d in w + 1..len {
                            let wob = ((d as f64 * 0.7 + k as f64).sin() * 1.5) as i32;
                            let di = (ca * d as f64) as i32 + if sa.abs() > 0.5 { wob } else { 0 };
                            let dj = (sa * d as f64) as i32 + if ca.abs() > 0.5 { wob } else { 0 };
                            for dk in -3..=2 {
                                if let Some(c) = g.offset(s.cell, di, dj, dk) {
                                    cells.push((c, if dk == -3 { B::Magma } else { B::Air }));
                                }
                            }
                        }
                    }
                    // The bottom of the hole fills with lava that wells up.
                    for di in -w..=w {
                        for dj in -w..=w {
                            if let Some(c) = g.offset(s.cell, di, dj, -depth + 1) {
                                cells.push((c, B::Lava));
                            }
                        }
                    }
                    self.stellar_set(cells);
                    self.stellar_hurt(at, len as f64, 50.0 * dmg_k);
                    self.epic_impact(at, len as f64 * 0.6, [1.0, 0.35, 0.1], 0.9);
                }
                if (2.5..6.5).contains(&t) {
                    // The lava geyser.
                    for _ in 0..12 {
                        let v = ring(self.mobs.rand() * std::f64::consts::TAU) * (self.mobs.rand() * 4.0) + up * (18.0 + self.mobs.rand() * 14.0);
                        let col = [[1.0, 0.45, 0.05], [1.0, 0.75, 0.2], [0.9, 0.2, 0.05]][(self.mobs.rand() * 3.0) as usize % 3];
                        self.mobs.fx_glow(at + up * 0.5, v, col, 0.4, 1.4, FxKind::Fall);
                    }
                    self.shake = self.shake.max(0.3);
                }
                finished = t > 7.0;
            }
            10 => {
                // Orbital barrage: for eight seconds a laser from orbit strikes wherever you aim,
                // four times a second.
                if t < 8.0 {
                    let shots = (t / 0.25) as u32;
                    while s.step < shots {
                        s.step += 1;
                        let eye = self.player.eye_pos();
                        let look = self.aim_dir.unwrap_or_else(|| self.player.look_dir());
                        let Some(h) = wolfcraft_engine::physics::raycast(&self.chunks, eye, look, 200.0) else { continue };
                        let p = g.cell_center(h.cell);
                        let pu = wolfcraft_engine::physics::up_at(p);
                        let r = (2.0 * pw).round() as i32 + 1;
                        let mut cells = Vec::new();
                        for di in -r..=r {
                            for dj in -r..=r {
                                for dk in -r..=r {
                                    if di * di + dj * dj + dk * dk <= r * r {
                                        if let Some(c) = g.offset(h.cell, di, dj, dk) {
                                            cells.push((c, B::Air));
                                        }
                                    }
                                }
                            }
                        }
                        self.stellar_set(cells);
                        self.stellar_hurt(p, r as f64 + 1.5, 25.0 * dmg_k);
                        for k in 0..18 {
                            let col = if k % 2 == 0 { [0.4, 1.0, 0.6] } else { [0.9, 1.0, 0.9] };
                            self.mobs.fx_glow(p + pu * (k as f64 * 5.0), -pu * 60.0, col, 0.35, 0.2, FxKind::Rise);
                        }
                        self.explosives_flash_ring(p, r as f64 + 2.0);
                        events.push(InteractionEvent::Exploded { at: p, power: 3.0 });
                        self.shake = self.shake.max(0.35);
                        self.flash = Some(([0.6, 1.0, 0.7], 0.15));
                    }
                }
                finished = t > 8.3;
            }
            _ => {
                // 8×8 cells of 3×3 blocks.
                const CELLS: i32 = 8;
                const CELL: i32 = 3;
                const HALF: i32 = CELLS * CELL / 2;
                let rainbow = [[1.0, 0.2, 0.2], [1.0, 0.6, 0.1], [1.0, 0.95, 0.2], [0.3, 0.95, 0.3], [0.2, 0.8, 1.0], [0.3, 0.4, 1.0], [0.75, 0.35, 1.0]];
                let base = s.cell;
                let corner = |di: i32, dj: i32| g.offset(base, di, dj, 0).map(|c| g.cell_center(c));
                if t < 2.0 {
                    // The light cage: grid lines on the ground.
                    for _ in 0..10 {
                        let line = (self.mobs.rand() * (CELLS + 1) as f64) as i32;
                        let along = self.mobs.rand();
                        let pos = (-HALF as f64 + along * (2 * HALF) as f64) as i32;
                        let (di, dj) = if self.mobs.rand() < 0.5 { (line * CELL - HALF, pos) } else { (pos, line * CELL - HALF) };
                        if let Some(p) = corner(di, dj) {
                            let col = rainbow[(line as usize) % 7];
                            self.mobs.fx_glow(p + up * 0.7, DVec3::ZERO, col, 0.14, 0.4, FxKind::Rise);
                        }
                    }
                }
                // From 2 s: a cell every 0.08 s is torn out and lifted away.
                let total = (CELLS * CELLS) as u32;
                while s.step < total && t >= 2.0 + s.step as f32 * 0.08 {
                    let ci = (s.step as i32) % CELLS;
                    let cj = (s.step as i32) / CELLS;
                    // Stepped pit: deeper towards the middle.
                    let ring_n = (ci * 2 + 1 - CELLS).abs().max((cj * 2 + 1 - CELLS).abs()) / 2;
                    let depth = 1 + (CELLS / 2 - 1 - ring_n).max(0) * 2;
                    let mut cells = Vec::new();
                    for a in 0..CELL {
                        for b in 0..CELL {
                            let (di, dj) = (ci * CELL - HALF + a, cj * CELL - HALF + b);
                            for dk in (1 - depth)..=10 {
                                if let Some(c) = g.offset(base, di, dj, dk) {
                                    cells.push((c, B::Air));
                                }
                            }
                            // Rainbow grid on the floor along the cell's edges.
                            if a == 0 || b == 0 {
                                let wool = [B::RedWool, B::YellowWool, B::BlueWool, B::Wool][((ci + cj) % 4) as usize];
                                if let Some(c) = g.offset(base, di, dj, -depth) {
                                    cells.push((c, wool));
                                }
                            }
                        }
                    }
                    self.stellar_set(cells);
                    if let Some(p) = corner(ci * CELL - HALF + 1, cj * CELL - HALF + 1) {
                        // The cube rises to the sky in a burst of earth.
                        for k in 0..6 {
                            let col = [[0.45, 0.33, 0.22], [0.40, 0.62, 0.30], [0.50, 0.50, 0.52]][k % 3];
                            self.mobs.fx(p + up * (k as f64 * 0.4), up * (14.0 + k as f64), col, 0.9, 1.6, FxKind::Rise);
                        }
                        self.mobs.fx_glow(p, up * 18.0, rainbow[(s.step % 7) as usize], 0.5, 1.2, FxKind::Rise);
                        self.stellar_hurt(p, 2.2, 30.0 * dmg_k);
                    }
                    s.step += 1;
                }
                // The hypercube: two nested wire cubes hanging over the pit, turning.
                if t > 2.0 {
                    let c = at + up * 18.0;
                    let rot = t as f64 * 0.6;
                    let (ra, rb) = (ta * rot.cos() + tb * rot.sin(), tb * rot.cos() - ta * rot.sin());
                    for size in [6.0, 3.0] {
                        for _ in 0..8 {
                            // A random point on a random edge of the cube.
                            let e = (self.mobs.rand() * 12.0) as usize % 12;
                            let s1 = if e % 2 == 0 { -1.0 } else { 1.0 };
                            let s2 = if (e / 2) % 2 == 0 { -1.0 } else { 1.0 };
                            let u = self.mobs.rand() * 2.0 - 1.0;
                            let v = match e / 4 {
                                0 => ra * u + rb * s1 + up * s2,
                                1 => ra * s1 + rb * u + up * s2,
                                _ => ra * s1 + rb * s2 + up * u,
                            };
                            self.mobs.fx_glow(c + v * size, DVec3::ZERO, rainbow[e % 7], 0.16, 0.25, FxKind::Rise);
                        }
                    }
                }
                finished = t > 12.0;
            }
        }
        if !finished {
            self.stellar = Some(s);
        } else {
            self.cutscene = None;
        }
    }

    /// The ritual of revival at an altar, about six seconds:
    /// 1) four flames circle the altar, spiralling in and up;
    /// 2) a pillar of light rises from it and the ground trembles with sparks;
    /// 3) a shock ring bursts out — and the guardian stands up again at full health.
    fn tick_ritual(&mut self, dt: f32) {
        use wolfcraft_engine::mobs::FxKind;
        let Some((kind, pos, t)) = self.ritual else { return };
        let t1 = t + dt;
        self.ritual = Some((kind, pos, t1));
        let g = self.chunks.grid();
        let at = g.cell_center(pos);
        let up = wolfcraft_engine::physics::up_at(at);
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let a = up.cross(helper).normalize();
        let b = up.cross(a);
        let col = kind.color();
        let bright = [(col[0] + 0.6).min(1.0), (col[1] + 0.6).min(1.0), (col[2] + 0.6).min(1.0)];
        let ring = |ang: f64| a * ang.cos() + b * ang.sin();
        if t1 < 2.5 {
            // Flames spiral in.
            let k = (t1 / 2.5) as f64;
            for n in 0..4 {
                let ang = n as f64 * std::f64::consts::FRAC_PI_2 + t1 as f64 * 3.0;
                let p = at + ring(ang) * (4.0 - 3.0 * k) + up * (0.5 + 2.0 * k);
                self.mobs.fx_glow(p, up * 0.2, if n % 2 == 0 { col } else { bright }, 0.12, 0.6, FxKind::Rise);
            }
        } else if t1 < 5.0 {
            // A pillar of light and sparks from the ground.
            for n in 0..3 {
                let h = (t1 as f64 * 7.0 + n as f64 * 3.3) % 12.0;
                let ang = t1 as f64 * 9.0 + n as f64 * 2.1;
                self.mobs.fx_glow(at + up * (0.6 + h) + ring(ang) * 0.4, up * 2.5, bright, 0.16, 0.8, FxKind::Rise);
            }
            let ang = t1 as f64 * 17.0;
            self.mobs.fx(at + ring(ang) * 2.5, up * 1.5 + ring(ang) * 0.5, [0.45, 0.42, 0.40], 0.1, 0.7, FxKind::Fall);
        }
        // Shock ring and the guardian's return.
        if t < 5.0 && t1 >= 5.0 {
            for n in 0..32 {
                let ang = n as f64 * std::f64::consts::TAU / 32.0;
                self.mobs.fx_glow(at + up * 0.6 + ring(ang) * 0.8, ring(ang) * 6.0 + up * 0.8, bright, 0.18, 1.0, FxKind::Rise);
            }
            self.ritual = None;
            if self.boss_book.revive(kind, pos) {
                let _ = self.boss_book.save(&self.dir.join("bosses.txt"));
                let spot = at + up * 1.2;
                let mut mob = wolfcraft_engine::mobs::Mob::new(wolfcraft_engine::mobs::MobKind::Ghoul, spot, self.player.heading);
                mob.hp = kind.max_hp();
                mob.boss = Some(wolfcraft_engine::bosses::BossRole::new(kind, pos, spot));
                self.mobs.mobs.push(mob);
                self.item_name = Some((format!("{} возрождён!", kind.name()), 4.0));
            }
        }
    }

    /// A damage number pops out where the creature was hit (heavy blows bigger and golden).
    fn damage_popup(&mut self, at: DVec3, dmg: f32) {
        let big = dmg >= 10.0;
        let jitter = DVec3::new(self.mobs.rand() - 0.5, self.mobs.rand() - 0.5, self.mobs.rand() - 0.5) * 0.4;
        self.popups.push(Popup { at: at + jitter, text: format!("{:.0}", dmg.max(1.0)), color: if big { [255, 200, 60] } else { [255, 255, 255] }, age: 0.0, big });
    }

    /// Saddles, mounts or leaves the dragon in the crosshair. True when the tap was used.
    fn try_mount(&mut self) -> bool {
        use wolfcraft_engine::items::ids;
        use wolfcraft_engine::mobs::MobKind;
        let Some(look) = self.aim_dir else { return false };
        let eye = self.player.eye_pos();
        let Some((i, _)) = self.mobs.raycast(eye, look, Interaction::reach(&self.player)) else { return false };
        // In the saddle: a tap on your dragon gets you off.
        if self.riding.is_some() {
            if self.riding == Some(i) {
                self.dismount();
                return true;
            }
            return false;
        }
        let Some(m) = self.mobs.mobs.get_mut(i) else { return false };
        if m.kind != MobKind::Dragon || m.hp <= 0.0 {
            return false;
        }
        if !m.saddled {
            if self.player.inventory.held().is_some_and(|s| s.id == ids::SADDLE) {
                m.saddled = true;
                if !self.player.is_creative() {
                    self.player.inventory.consume_selected();
                }
                return true;
            }
            return false;
        }
        m.ridden = true;
        m.body.vel = DVec3::ZERO;
        self.riding = Some(i);
        true
    }

    pub fn dismount(&mut self) {
        if let Some(m) = self.riding.take().and_then(|i| self.mobs.mobs.get_mut(i)) {
            m.ridden = false;
        }
        self.player.body.vel = DVec3::ZERO;
    }

    fn tick_ride(&mut self, input: &InputFrame, dt: f32) {
        use wolfcraft_engine::mobs::MobKind;
        let Some(i) = self.riding else { return };
        let ok = self.mobs.mobs.get(i).is_some_and(|m| m.kind == MobKind::Dragon && m.ridden && m.hp > 0.0);
        if !ok || self.player.survival.dead {
            self.riding = None;
            for m in &mut self.mobs.mobs {
                m.ridden = false;
            }
            return;
        }
        let up = self.player.up();
        let fwd = self.player.heading;
        let right = self.player.right();
        let mut wish = fwd * input.forward as f64 + right * input.strafe as f64;
        if wish.length_squared() > 1.0 {
            wish = wish.normalize();
        }
        let speed = if input.sprint { 22.0 } else { 13.0 };
        let m = &mut self.mobs.mobs[i];
        m.body.flying = true;
        let mut left = dt.min(0.25) as f64;
        while left > 1e-6 {
            let step = left.min(1.0 / 30.0);
            wolfcraft_engine::physics::step(&self.chunks, &mut m.body, &wolfcraft_engine::physics::MoveInput { wish, speed, jump: false, up: input.jump, down: input.sneak }, step);
            left -= step;
        }
        // The dragon looks where it flies (or where the rider looks while hovering).
        let dir = if wish.length_squared() > 0.01 { wish } else { fwd };
        let h = dir - up * dir.dot(up);
        if h.length_squared() > 1e-6 {
            m.heading = h.normalize();
        }
        // The rider sits in the saddle.
        let seat = m.body.pos + wolfcraft_engine::physics::up_at(m.body.pos) * 1.9;
        self.player.body.pos = seat;
        self.player.body.vel = m.body.vel;
        self.player.body.on_ground = false;
    }

    fn try_attack(&mut self, cooldown: f32) -> bool {
        if self.player.survival.dead || !self.ground_ready() {
            return false;
        }
        let eye = self.player.eye_pos();
        let Some(look) = self.aim_dir else { return false };
        let reach = Interaction::reach(&self.player) - 1.5;
        let Some((i, t)) = self.mobs.raycast(eye, look, reach) else { return false };
        // Never your own mount.
        if self.riding == Some(i) {
            return false;
        }
        // A block in front of the creature protects it.
        if self.hit.is_some_and(|h| h.dist <= t) {
            return false;
        }
        if self.attack_cd > 0.0 {
            return true;
        }
        let held = self.player.inventory.held();
        // Weapon, «Острота» (+1.5 per level) and the Сила potion (+3 per level).
        let sharp = held.map(|s| s.ench_level(wolfcraft_engine::magic::Ench::Sharpness)).unwrap_or(0);
        let steal = held.map(|s| s.ench_level(wolfcraft_engine::magic::Ench::Lifesteal)).unwrap_or(0);
        let strength = self.player.effect_level(wolfcraft_engine::magic::Effect::Strength);
        let dmg = held.map(|s| s.info().attack).unwrap_or(1.0).max(1.0) + 1.5 * sharp as f32 + 3.0 * strength as f32;
        // «Жажда жизни»: each blow that lands gives back a little health.
        if steal > 0 && !self.player.survival.dead {
            self.player.survival.health = (self.player.survival.health + 0.5 * steal as f32).min(wolfcraft_engine::player::MAX_HEALTH);
        }
        let at = self.mobs.mobs.get(i).map(|m| m.centre()).unwrap_or(eye + look * t);
        // Hit feedback: a burst of red and white sparks, more for strong weapons.
        let n = ((4.0 + dmg * 1.5) * self.fx_amount) as usize;
        self.mobs.chips(at - look * 0.3, &[[0.95, 0.15, 0.12], [1.0, 1.0, 1.0], [0.75, 0.05, 0.05]], n, 2.2 + dmg as f64 * 0.15);
        self.damage_popup(at, dmg);
        if let Some(kind) = self.mobs.hurt(i, dmg, self.player.body.pos) {
            self.grant("hunter");
            // Finishing blow: a puff of smoke and a ring of little stars.
            {
                use wolfcraft_engine::mobs::FxKind;
                let up = wolfcraft_engine::physics::up_at(at);
                let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
                let t1 = up.cross(helper).normalize();
                let t2 = up.cross(t1);
                for k in 0..10 {
                    let a = k as f64 * std::f64::consts::TAU / 10.0;
                    let side = t1 * a.cos() + t2 * a.sin();
                    self.mobs.fx(at + side * 0.25, side * 1.2 + up * 0.6, [0.92, 0.92, 0.94], 0.16 + (k % 3) as f32 * 0.04, 0.9, FxKind::Rise);
                }
                for k in 0..8 {
                    let a = k as f64 * std::f64::consts::TAU / 8.0;
                    let side = t1 * a.cos() + t2 * a.sin();
                    let col = if k % 2 == 0 { [1.0, 0.92, 0.35] } else { [1.0, 1.0, 0.85] };
                    self.mobs.fx_glow(at + up * 0.5 + side * 0.4, side * 1.8 + up * 1.4, col, 0.05, 0.8, FxKind::Rise);
                }
            }
            for drop in wolfcraft_engine::items::mob_drops(kind, self.cfg.seed ^ self.clock.to_bits() as u64) {
                self.drops.pop_from_block(drop, at);
            }
            if kind != wolfcraft_engine::mobs::MobKind::Slime {
                self.leave_bones(at);
            }
            self.player.survival.add_xp(if kind.hostile() { 5 } else { 1 });
        }
        self.attack_cd = cooldown;
        self.mob_hits += 1;
        // A sword wears once per blow, other tools twice (they are not weapons).
        for _ in 0..wolfcraft_engine::tools::wear_for_hit(self.player.inventory.held().map(|s| s.id)) {
            self.player.inventory.wear_selected();
        }
        true
    }

    /// A heap of bones where a creature or the player died: on the ground under `at`, in a
    /// free cell (air or grass), never inside blocks. Mined with a pickaxe for bones.
    fn leave_bones(&mut self, at: DVec3) {
        use wolfcraft_engine::block::Block as B;
        let g = self.chunks.grid();
        let up = wolfcraft_engine::physics::up_at(at);
        // From a little above the body down to two blocks under it: the first free cell
        // standing on something solid.
        for step in 0..5 {
            let p = at + up * (0.6 - step as f64 * 0.7);
            let c = g.world_to_cell(p);
            let Some(below) = g.neighbor(c, 2, -1) else { continue };
            let (Some(here), Some(under)) = (self.chunks.get_block(c), self.chunks.get_block(below)) else { continue };
            let free = here == B::Air || (here.is_replaceable() && !here.is_fluid() && here.crop().is_none());
            if free && under.is_solid() && under.boxes().is_empty() {
                if self.chunks.set_block(c, B::Bones) {
                    self.net_out.push((c, B::Bones));
                }
                return;
            }
        }
    }

    /// Random ticks around the player: sugar cane and cactus grow a block now and then, up
    /// to three tall (about one block a minute each).
    fn random_ticks(&mut self, dt: f32) {
        use wolfcraft_engine::block::Block as B;
        self.tick_acc += dt * 128.0;
        let n = self.tick_acc as usize;
        self.tick_acc -= n as f32;
        if n == 0 || !self.ground_ready() {
            return;
        }
        let g = self.chunks.grid();
        let base = g.world_to_cell(self.player.body.pos);
        for _ in 0..n.min(64) {
            let r = |s: &mut Self, span: i32| (s.mobs.rand() * span as f64) as i32 - span / 2;
            let (di, dj, dk) = (r(self, 32), r(self, 32), r(self, 8));
            let Some(c) = g.offset(base, di, dj, dk) else { continue };
            let Some(b) = self.chunks.get_block(c) else { continue };
            if b.crop().is_some() {
                self.tick_crop(c, b);
                continue;
            }
            if !matches!(b, B::SugarCane | B::Cactus) {
                continue;
            }
            let Some(top) = g.neighbor(c, 2, 1) else { continue };
            if self.chunks.get_block(top) != Some(B::Air) {
                continue;
            }
            let mut h = 1;
            let mut below = g.neighbor(c, 2, -1);
            while let Some(d) = below {
                if self.chunks.get_block(d) != Some(b) || h >= 3 {
                    break;
                }
                h += 1;
                below = g.neighbor(d, 2, -1);
            }
            if h < 3 && self.chunks.set_block(top, b) {
                self.net_out.push((top, b));
            }
        }
    }

    /// A random tick on a crop: it needs farmland below (else it falls off as an item); a
    /// young crop ripens, twice as likely with water within four blocks of its farmland.
    fn tick_crop(&mut self, c: wolfcraft_engine::cubesphere::Cell, b: wolfcraft_engine::block::Block) {
        use wolfcraft_engine::block::Block as B;
        let g = self.chunks.grid();
        let Some(soil) = g.neighbor(c, 2, -1) else { return };
        if self.chunks.get_block(soil) != Some(B::Farmland) {
            if self.chunks.set_block(c, B::Air) {
                self.net_out.push((c, B::Air));
                let centre = g.cell_center(c);
                for d in wolfcraft_engine::items::drops(b, true, glam::IVec3::new(c.i, c.j, c.k), self.cfg.seed) {
                    self.drops.pop_from_block(d, centre);
                }
            }
            return;
        }
        let Some(next) = b.grown() else { return };
        let wet = (-4..=4).any(|di| (-4..=4).any(|dj| g.offset(soil, di, dj, 0).and_then(|n| self.chunks.get_block(n)).is_some_and(|x| x.is_water())));
        let chance = if wet { 0.5 } else { 0.22 };
        if self.mobs.rand() < chance && self.chunks.set_block(c, next) {
            self.net_out.push((c, next));
        }
    }

    /// Pressure plates: anyone standing on one (the player or a creature) presses it; it
    /// springs back half a second after they step off.
    fn step_plates(&mut self, dt: f32) {
        use wolfcraft_engine::block::Block as B;
        let g = self.chunks.grid();
        let mut feet: Vec<DVec3> = vec![self.player.body.pos];
        feet.extend(self.mobs.mobs.iter().map(|m| m.body.pos));
        for p in feet {
            let up = wolfcraft_engine::physics::up_at(p);
            let c = g.world_to_cell(p + up * 0.02);
            match self.chunks.get_block(c) {
                Some(B::PressurePlate) => {
                    if self.chunks.set_block(c, B::PressurePlateOn) {
                        wolfcraft_engine::power::update_iskrit(&mut self.chunks, c);
                        self.net_out.push((c, B::PressurePlateOn));
                        self.plates.push((c, 0.5));
                    }
                }
                Some(B::PressurePlateOn) => match self.plates.iter_mut().find(|(q, _)| *q == c) {
                    Some(e) => e.1 = 0.5,
                    None => self.plates.push((c, 0.5)),
                },
                _ => {}
            }
        }
        let mut up = Vec::new();
        self.plates.retain_mut(|(c, t)| {
            *t -= dt;
            if *t <= 0.0 {
                up.push(*c);
                false
            } else {
                true
            }
        });
        for c in up {
            if self.chunks.get_block(c) == Some(B::PressurePlateOn) {
                self.chunks.set_block(c, B::PressurePlate);
                wolfcraft_engine::power::update_iskrit(&mut self.chunks, c);
                self.net_out.push((c, B::PressurePlate));
            }
        }
    }

    /// Throws a stack out in front of the player (Bedrock style): it flies along the view
    /// and cannot be picked up again for a moment.
    pub fn throw(&mut self, stack: wolfcraft_engine::items::ItemStack) {
        let eye = self.player.eye_pos();
        let look = self.player.look_dir();
        let up = wolfcraft_engine::physics::up_at(eye);
        self.drops.spawn(stack, eye - up * 0.3 + look * 0.4, look * 6.5 + up * 1.8, 1.5);
    }

    /// Throws the held item: one, or the whole stack.
    pub fn throw_held(&mut self, all: bool) {
        let inv = &mut self.player.inventory;
        let sel = inv.selected;
        let Some(s) = inv.slots[sel] else { return };
        let n = if all { s.count } else { 1 };
        let thrown = wolfcraft_engine::items::ItemStack { count: n, ..s };
        inv.slots[sel] = if s.count > n { Some(wolfcraft_engine::items::ItemStack { count: s.count - n, ..s }) } else { None };
        self.throw(thrown);
    }

    /// True when the centre of one of the body's spheres is inside a solid block's box.
    fn body_stuck(&self, feet: DVec3) -> bool {
        let up = wolfcraft_engine::physics::up_at(feet);
        let g = self.chunks.grid();
        [0.3, 0.9, 1.5].iter().any(|h| {
            let p = feet + up * *h;
            let c = g.world_to_cell(p);
            let Some(b) = self.chunks.get_block(c) else { return false };
            b.is_solid() && wolfcraft_engine::physics::block_obbs_in(&self.chunks, c, b).iter().any(|o| {
                let d = p - o.center;
                (0..3).all(|a| d.dot(o.axes[a]).abs() < o.half[a] - 0.05)
            })
        })
    }

    /// True when the whole body fits at `feet`: none of its spheres touches a solid block.
    fn body_clear(&self, feet: DVec3) -> bool {
        let up = wolfcraft_engine::physics::up_at(feet);
        let g = self.chunks.grid();
        [0.3, 0.9, 1.5].iter().all(|h| {
            let base = g.world_to_cell(feet + up * *h);
            (-1..=1).all(|dk| {
                (-1..=1).all(|dj| {
                    (-1..=1).all(|di| {
                        g.offset(base, di, dj, dk).is_none_or(|c| !self.chunks.get_block(c).is_some_and(|b| b.is_solid()) || !wolfcraft_engine::physics::body_overlaps_cell(&g, feet, c))
                    })
                })
            })
        })
    }

    /// Moves the player to the nearest place where the body fits (up to 10 blocks up, or a
    /// block to a side).
    fn unstuck(&mut self) {
        if self.player.body.flying || self.player.survival.dead || !self.body_stuck(self.player.body.pos) {
            return;
        }
        let up = self.player.up();
        let helper = if up.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
        let t1 = (helper - up * helper.dot(up)).normalize();
        let t2 = up.cross(t1);
        // Nearest free place: the middle of the cell at each height first (a narrow shaft),
        // then straight up, then one block to a side.
        let g = self.chunks.grid();
        let feet_cell = g.world_to_cell(self.player.body.pos);
        for step in 0..=10 {
            if let Some(c) = g.offset(feet_cell, 0, 0, step) {
                let p = g.cell_center(c) - wolfcraft_engine::physics::up_at(g.cell_center(c)) * 0.45;
                if self.body_clear(p) {
                    self.player.body.pos = p;
                    self.player.body.vel = DVec3::ZERO;
                    return;
                }
            }
            for side in [DVec3::ZERO, t1, -t1, t2, -t2] {
                if step == 0 && side == DVec3::ZERO {
                    continue;
                }
                let p = self.player.body.pos + up * step as f64 + side;
                if self.body_clear(p) {
                    self.player.body.pos = p;
                    self.player.body.vel = DVec3::ZERO;
                    return;
                }
            }
        }
        // Buried deeper: the first free cell further up the column, else the surface.
        for step in 11..=48 {
            if let Some(c) = g.offset(feet_cell, 0, 0, step) {
                let p = g.cell_center(c) - wolfcraft_engine::physics::up_at(g.cell_center(c)) * 0.45;
                if self.body_clear(p) {
                    self.player.body.pos = p;
                    self.player.body.vel = DVec3::ZERO;
                    return;
                }
            }
        }
        if let Some(p) = self.planet.stand_position(up) {
            self.player.body.pos = p;
            self.player.body.vel = DVec3::ZERO;
        }
    }

    /// Season (0..4: spring, summer, autumn, winter), snow cover 0..1 here, humidity 0..1.
    /// Snow lies only where it is cold enough: never in deserts, always on glaciers.
    pub fn environment(&self) -> (f32, f32, f32) {
        let season = wolfcraft_engine::player::season(self.player.world_time);
        let (biome, temp, humid) = self.env_col.unwrap_or((wolfcraft_engine::biome::Biome::Plains, 0.5, 0.5));
        use wolfcraft_engine::biome::Biome as B;
        let winter = if season >= 3.0 { ((season - 3.0) * 4.0).min(1.0).min((4.0 - season) * 4.0) } else { 0.0 };
        let cold = ((0.8 - temp) * 2.0).clamp(0.0, 1.0);
        let snow = match biome {
            B::Desert | B::Badlands | B::Volcano | B::Jungle | B::Savanna => 0.0,
            B::Tundra | B::SnowyPlains | B::SnowyPeaks | B::FrozenOcean => 1.0,
            _ => winter * cold,
        };
        (season, snow, humid)
    }

    /// Date and season for the HUD clock: "2 марта · Весна".
    pub fn season_label(&self) -> String {
        use wolfcraft_engine::player as p;
        let (name, _) = p::season_label(self.player.world_time);
        let (m, day) = p::month(self.player.world_time);
        format!("{day} {} · {name}", p::MONTH_GENITIVE[m])
    }

    /// Fog colour tint and how much thicker the fog is in this biome (swamps are misty).
    pub fn biome_fog(&self) -> ([f32; 3], f32, f32) {
        use wolfcraft_engine::biome::Biome as B;
        match self.env_col.map(|e| e.0) {
            Some(B::Swamp) => ([0.45, 0.55, 0.40], 0.35, 1.8),
            Some(B::Jungle) => ([0.55, 0.70, 0.55], 0.20, 1.3),
            Some(B::Desert | B::Badlands) => ([0.90, 0.78, 0.55], 0.30, 1.0),
            Some(B::Volcano) => ([0.45, 0.38, 0.36], 0.45, 1.6),
            Some(B::Tundra | B::SnowyPlains | B::SnowyPeaks) => ([0.85, 0.90, 0.98], 0.30, 1.2),
            _ => ([1.0, 1.0, 1.0], 0.0, 1.0),
        }
    }

    /// Ambient particles of the biome and season: fireflies over swamps and jungles at
    /// night, blowing sand in deserts, snowfall where it is cold in winter, pollen in spring
    /// and summer meadows and forests, ash and embers over volcanoes.
    fn biome_ambience(&mut self, dt: f32, k: f32, pos: DVec3, up: DVec3, t1: DVec3, t2: DVec3) {
        use wolfcraft_engine::biome::Biome as B;
        use wolfcraft_engine::mobs::FxKind;
        let Some((biome, _, _)) = self.env_col else { return };
        let (season, snow, _) = self.environment();
        let day = local_sun(self.player.world_time, up).dot(up) > 0.05;
        let rate = (k * dt) as f64;
        let around = |m: &mut wolfcraft_engine::mobs::MobManager, r: f64, h0: f64, h1: f64| {
            let a = m.rand() * std::f64::consts::TAU;
            let d = m.rand().sqrt() * r;
            pos + (t1 * a.cos() + t2 * a.sin()) * d + up * (h0 + m.rand() * (h1 - h0))
        };
        // Snowfall (cold biomes, or any cold place in winter).
        if snow > 0.05 && !matches!(biome, B::Desert | B::Badlands | B::Volcano) && (season >= 3.0 || matches!(biome, B::Tundra | B::SnowyPlains | B::SnowyPeaks)) {
            for _ in 0..((rate * 40.0 * snow as f64) as usize + (self.mobs.rand() < (rate * 40.0 * snow as f64).fract()) as usize) {
                let p = around(&mut self.mobs, 12.0, 6.0, 12.0);
                self.mobs.fx(p, -up * 1.2, [0.96, 0.97, 1.0], 0.045, 2.9, FxKind::Leaf);
            }
        }
        match biome {
            B::Swamp | B::Jungle if !day => {
                if self.mobs.rand() < rate * 6.0 {
                    let p = around(&mut self.mobs, 9.0, 0.5, 3.0);
                    let v = (t1 * (self.mobs.rand() - 0.5) + t2 * (self.mobs.rand() - 0.5)) * 0.6 - up * 0.3;
                    self.mobs.fx_glow(p, v, [0.85, 1.0, 0.35], 0.04, 2.6, FxKind::Rise);
                }
            }
            B::Desert | B::Badlands if day => {
                if self.mobs.rand() < rate * 14.0 {
                    let p = around(&mut self.mobs, 10.0, 0.1, 2.0);
                    let col = if biome == B::Badlands { [0.80, 0.45, 0.25] } else { [0.88, 0.80, 0.58] };
                    self.mobs.fx(p, t1 * 3.0 + up * 0.2, col, 0.035, 1.8, FxKind::Rise);
                }
            }
            B::Volcano => {
                if self.mobs.rand() < rate * 12.0 {
                    let p = around(&mut self.mobs, 12.0, 5.0, 10.0);
                    self.mobs.fx(p, -up * 0.8, [0.35, 0.33, 0.32], 0.05, 2.9, FxKind::Leaf);
                }
                if self.mobs.rand() < rate * 3.0 {
                    let p = around(&mut self.mobs, 8.0, 0.0, 2.0);
                    self.mobs.fx_glow(p, up * 1.2, [1.0, 0.5, 0.12], 0.035, 1.5, FxKind::Rise);
                }
            }
            B::Plains | B::Forest | B::BirchForest | B::Savanna if day && season < 2.0 => {
                if self.mobs.rand() < rate * 5.0 {
                    let p = around(&mut self.mobs, 8.0, 0.3, 3.0);
                    let v = (t1 * (self.mobs.rand() - 0.5) + t2 * (self.mobs.rand() - 0.5)) * 0.5;
                    self.mobs.fx(p, v, [1.0, 0.96, 0.70], 0.025, 2.5, FxKind::Rise);
                }
            }
            _ => {}
        }
    }

    /// Builds an iskrit showcase in front of the player: a stone floor with signed stands —
    /// torch → dust → lamp, a button, a switch, an iskrit block, the 15-block reach (15 dust
    /// lights the lamp, 16 does not) and dust climbing a staircase. Returns a chat message.
    /// Builds a boss arena 16 blocks ahead of the player, so the guardians need not be
    /// searched for: a round floor 11 blocks in radius, a low wall with four gates, four
    /// lantern pillars and the guardian's altar in the middle. One `//undo` removes it.
    /// Returns the altar's cell.
    pub fn build_arena(&mut self, kind: wolfcraft_engine::bosses::BossKind) -> Result<wolfcraft_engine::cubesphere::Cell, String> {
        use wolfcraft_engine::block::Block as B;
        use wolfcraft_engine::bosses::BossKind;
        use wolfcraft_engine::construction::{self as c, Form};
        if self.hell {
            return Err("Арены строятся только на родной планете".into());
        }
        let g = self.chunks.grid();
        let feet = g.world_to_cell(self.player.body.pos);
        let centre = g.cell_center(feet);
        let ax = |di: i32, dj: i32| g.offset(feet, di, dj, 0).map(|c| (g.cell_center(c) - centre).normalize()).unwrap_or(DVec3::ZERO);
        let (ei, ej) = (ax(1, 0), ax(0, 1));
        let h = self.player.heading;
        let fwd = if h.dot(ei).abs() >= h.dot(ej).abs() { (h.dot(ei).signum() as i32, 0) } else { (0, h.dot(ej).signum() as i32) };
        const R: i32 = 11;
        let (floor, wall, altar) = match kind {
            BossKind::Stone => (B::StoneBricks, B::Cobblestone, c::block(Form::StoneAltar, 3, 0)),
            BossKind::Root => (B::MossBlock, B::OakLog, c::block(Form::RootAltar, 0, 0)),
            BossKind::Ember => (B::Obsidian, B::Netherrack, c::block(Form::EmberAltar, 6, 0)),
        };
        let altar = altar.ok_or("Алтарь не найден в каталоге")?;
        let mid = g.offset(feet, fwd.0 * (R + 5), fwd.1 * (R + 5), 0).ok_or("Слишком близко к краю грани планеты, отойдите чуть в сторону")?;
        let mut cells = Vec::new();
        for di in -R..=R {
            for dj in -R..=R {
                let d2 = di * di + dj * dj;
                if d2 > R * R + R {
                    continue;
                }
                let Some(col) = g.offset(mid, di, dj, 0) else { return Err("Слишком близко к краю грани планеты, отойдите чуть в сторону".into()) };
                let rim = d2 >= (R - 1) * (R - 1);
                // Floor at the player's feet level − 1, a few blocks of fill under it, air above.
                for dk in -3..=7 {
                    let Some(cell) = g.offset(col, 0, 0, dk) else { continue };
                    let gate = di.abs() <= 1 || dj.abs() <= 1;
                    let b = if dk < -1 {
                        match self.chunks.get_block(cell) {
                            Some(b) if b.is_solid() && !b.is_fluid() => continue,
                            _ => wall,
                        }
                    } else if dk == -1 {
                        floor
                    } else if rim && dk <= 1 && !gate {
                        wall
                    } else {
                        B::Air
                    };
                    cells.push((cell, b));
                }
            }
        }
        // Four pillars with lanterns half-way out, the altar in the centre.
        for (di, dj) in [(6, 6), (6, -6), (-6, 6), (-6, -6)] {
            for dk in 0..=3 {
                if let Some(cell) = g.offset(mid, di, dj, dk) {
                    cells.push((cell, if dk == 3 { B::Lantern } else { wall }));
                }
            }
        }
        cells.push((mid, altar));
        let (n, missed) = crate::worldedit::apply(self, cells);
        if n == 0 || missed > 0 && self.chunks.get_block(mid) != Some(altar) {
            return Err("Мир вокруг ещё не загрузился, подойдите ближе или попробуйте через пару секунд".into());
        }
        if !self.is_creative() && self.player.inventory.count(wolfcraft_engine::items::ids::RITUAL_SEAL) == 0 {
            self.player.inventory.add(wolfcraft_engine::items::ItemStack::new(wolfcraft_engine::items::ids::RITUAL_SEAL, 1));
        }
        Ok(mid)
    }

    pub fn build_iskrit_demo(&mut self) -> Result<String, String> {
        use wolfcraft_engine::block::Block as B;
        let g = self.chunks.grid();
        let feet = g.world_to_cell(self.player.body.pos);
        // Local axes: forward along the cell axis closest to the view, right beside it.
        let centre = g.cell_center(feet);
        let ax = |di: i32, dj: i32| g.offset(feet, di, dj, 0).map(|c| (g.cell_center(c) - centre).normalize()).unwrap_or(DVec3::ZERO);
        let (ei, ej) = (ax(1, 0), ax(0, 1));
        let h = self.player.heading;
        let fwd = if h.dot(ei).abs() >= h.dot(ej).abs() { (h.dot(ei).signum() as i32, 0) } else { (0, h.dot(ej).signum() as i32) };
        let right_dir = self.player.right();
        let perp = if fwd.0 != 0 { (0, 1) } else { (1, 0) };
        let rs = right_dir.dot(ei * perp.0 as f64 + ej * perp.1 as f64).signum() as i32;
        let rgt = (perp.0 * rs, perp.1 * rs);
        // (x right, y up, z forward) → cell, starting 3 blocks ahead and 2 to the left.
        let at = |x: i32, y: i32, z: i32| {
            let (x, z) = (x - 2, z + 3);
            g.offset(feet, fwd.0 * z + rgt.0 * x, fwd.1 * z + rgt.1 * x, y)
        };
        let mut placed: Vec<wolfcraft_engine::cubesphere::Cell> = Vec::new();
        let mut set = |s: &mut Self, x: i32, y: i32, z: i32, b: B| -> Result<(), String> {
            let c = at(x, y, z).ok_or("Слишком близко к краю грани планеты, отойдите чуть в сторону")?;
            if !s.chunks.set_block(c, b) {
                return Err("Мир вокруг ещё не загрузился, попробуйте через пару секунд".into());
            }
            placed.push(c);
            Ok(())
        };
        // Floor and clear air above it (two rows of stands).
        for x in -1..=39 {
            for z in -2..=42 {
                set(self, x, -1, z, B::SmoothStone)?;
                for y in 0..=4 {
                    set(self, x, y, z, B::Air)?;
                }
            }
        }
        let mut signs: Vec<((i32, i32, i32), &str)> = Vec::new();
        // Stand 1: a torch powers a line of dust, the dust lights a lamp.
        set(self, 0, 0, 1, B::IskritTorch)?;
        for z in 2..=5 {
            set(self, 0, 0, z, B::IskritWire)?;
        }
        set(self, 0, 0, 6, B::IskritLamp)?;
        signs.push(((0, 0, -1), "Искритовый факел всегда даёт сигнал. Провод несёт его к лампе"));
        // Stand 2: a button gives a short pulse.
        set(self, 4, 0, 1, B::Button)?;
        for z in 2..=5 {
            set(self, 4, 0, z, B::IskritWire)?;
        }
        set(self, 4, 0, 6, B::IskritLamp)?;
        signs.push(((4, 0, -1), "Нажмите кнопку: лампа горит секунду"));
        // Stand 3: a switch turns the line on and off.
        set(self, 8, 0, 1, B::SwitchOff)?;
        for z in 2..=5 {
            set(self, 8, 0, z, B::IskritWire)?;
        }
        set(self, 8, 0, 6, B::IskritLamp)?;
        signs.push(((8, 0, -1), "Рычаг: нажмите, чтобы включить и выключить"));
        // Stand 4: an iskrit block powers what touches it.
        set(self, 12, 0, 1, B::IskritBlock)?;
        set(self, 12, 0, 2, B::IskritLamp)?;
        set(self, 13, 0, 1, B::IskritWire)?;
        set(self, 14, 0, 1, B::IskritWire)?;
        set(self, 14, 0, 2, B::IskritLamp)?;
        signs.push(((12, 0, -1), "Блок искрита питает всё, чего касается"));
        // Stand 5: the reach. 15 dust still lights the lamp, 16 do not.
        set(self, 18, 0, 1, B::IskritTorch)?;
        for z in 2..=16 {
            set(self, 18, 0, z, B::IskritWire)?;
        }
        set(self, 18, 0, 17, B::IskritLamp)?;
        set(self, 20, 0, 1, B::IskritTorch)?;
        for z in 2..=17 {
            set(self, 20, 0, z, B::IskritWire)?;
        }
        set(self, 20, 0, 18, B::IskritLamp)?;
        signs.push(((18, 0, -1), "Сигнал слабеет: 15 блоков провода хватает (слева), 16 уже нет (справа)"));
        // Stand 6: dust climbs a staircase.
        set(self, 24, 0, 1, B::IskritTorch)?;
        set(self, 24, 0, 2, B::IskritWire)?;
        set(self, 24, 0, 3, B::Stone)?;
        set(self, 24, 1, 3, B::IskritWire)?;
        set(self, 24, 0, 4, B::Stone)?;
        set(self, 24, 1, 4, B::Stone)?;
        set(self, 24, 2, 4, B::IskritWire)?;
        set(self, 24, 0, 5, B::Stone)?;
        set(self, 24, 1, 5, B::Stone)?;
        set(self, 24, 2, 5, B::IskritLamp)?;
        signs.push(((24, 0, -1), "Провод поднимается по ступенькам на блок вверх"));
        // Stand 7: pistons on switches, an ordinary one and a sticky one, each with a block.
        use wolfcraft_engine::block::Part;
        let fdir = match fwd { (1, 0) => 0, (-1, 0) => 1, (0, 1) => 2, _ => 3 };
        for (x, sticky) in [(28, false), (31, true)] {
            set(self, x, 0, 1, B::SwitchOff)?;
            set(self, x, 0, 2, Part::Piston { sticky, extended: false, dir: fdir }.block())?;
            set(self, x, 0, 3, B::OakPlanks)?;
        }
        signs.push(((28, 0, -1), "Поршни: переключите рычаг. Обычный толкает блок, липкий ещё и тянет назад"));
        // Stand 8: a repeater renews the signal: torch, 13 dust, repeater, 6 dust, lamp.
        set(self, 35, 0, 1, B::IskritTorch)?;
        for z in 2..=14 {
            set(self, 35, 0, z, B::IskritWire)?;
        }
        set(self, 35, 0, 15, Part::Repeater { on: false, dir: fdir }.block())?;
        for z in 16..=21 {
            set(self, 35, 0, z, B::IskritWire)?;
        }
        set(self, 35, 0, 22, B::IskritLamp)?;
        signs.push(((35, 0, -1), "Повторитель восстанавливает сигнал до полной силы и пропускает его только вперёд"));
        // ---- Second row: simple everyday mechanisms ----
        let rdir: u8 = match rgt { (1, 0) => 0, (-1, 0) => 1, (0, 1) => 2, _ => 3 };
        let wall = |s: &mut Self, set: &mut dyn FnMut(&mut Self, i32, i32, i32, B) -> Result<(), String>, x0: i32, x1: i32, z: i32| -> Result<(), String> {
            let _ = s;
            for x in x0..=x1 {
                for y in 0..=2 {
                    set(s, x, y, z, B::OakPlanks)?;
                }
            }
            Ok(())
        };
        // Doors stand across the walk (the wall runs sideways): the closed state for that way.
        let (fi, fj) = (fwd.0 as f64, fwd.1 as f64);
        let door = |s: &mut Self, set: &mut dyn FnMut(&mut Self, i32, i32, i32, B) -> Result<(), String>, x: i32, z: i32, kind: B, top: B| -> Result<(), String> {
            let lower = kind.oriented(fi, fj);
            let upper = if lower == kind { top } else { top.door_toggle().unwrap_or(top) };
            set(s, x, 0, z, lower)?;
            set(s, x, 1, z, upper)
        };
        // Stand 9: automatic door, a pressure plate on both sides.
        wall(self, &mut set, -1, 3, 33)?;
        door(self, &mut set, 1, 33, B::Door, B::DoorTop)?;
        set(self, 1, 0, 32, B::PressurePlate)?;
        set(self, 1, 0, 34, B::PressurePlate)?;
        signs.push(((1, 0, 30), "Автодверь: встаньте на нажимную плиту — дверь откроется сама и закроется за вами"));
        // Stand 10: door on a button, through a short wire.
        wall(self, &mut set, 5, 9, 33)?;
        door(self, &mut set, 7, 33, B::IronDoor, B::IronDoorTop)?;
        set(self, 9, 0, 31, B::Button)?;
        set(self, 8, 0, 31, B::IskritWire)?;
        set(self, 7, 0, 31, B::IskritWire)?;
        set(self, 7, 0, 32, B::IskritWire)?;
        signs.push(((7, 0, 30), "Железная дверь рукой не открывается: нажмите кнопку справа, сигнал пройдёт по проводу"));
        // Stand 11: double door on a switch.
        wall(self, &mut set, 11, 16, 33)?;
        door(self, &mut set, 13, 33, B::SpruceDoor, B::SpruceDoorTop)?;
        door(self, &mut set, 14, 33, B::SpruceDoor, B::SpruceDoorTop)?;
        set(self, 12, 0, 31, B::SwitchOff)?;
        for x in 12..=14 {
            set(self, x, 0, 32, B::IskritWire)?;
        }
        signs.push(((13, 0, 30), "Двойная автодверь: рычаг открывает обе створки, они расходятся в стороны"));
        // Stand 12: sugar cane farm. Water, a row of cane on dirt, pistons above that cut it.
        for z in 33..=37 {
            set(self, 19, -1, z, B::Water)?;
            set(self, 20, -1, z, B::Dirt)?;
            set(self, 20, 0, z, B::SugarCane)?;
            set(self, 20, 1, z, B::SugarCane)?;
            set(self, 22, 0, z, B::Stone)?;
            set(self, 21, 1, z, Part::Piston { sticky: false, extended: false, dir: rdir ^ 1 }.block())?;
            set(self, 22, 1, z, B::IskritWire)?;
        }
        set(self, 22, 0, 32, B::Stone)?;
        set(self, 22, 1, 32, B::Button)?;
        signs.push(((20, 0, 30), "Ферма тростника: он сам растёт у воды. Нажмите кнопку — поршни срежут верх, тростник выпадет"));
        // Stand 13: secret passage. Two sticky pistons hold wall blocks; switch off opens it.
        for x in 25..=29 {
            for y in 0..=2 {
                if x != 27 || y == 2 {
                    set(self, x, y, 34, B::StoneBricks)?;
                }
            }
        }
        let back = fdir ^ 1;
        for y in 0..=1 {
            set(self, 27, y, 34, B::StoneBricks)?;
            set(self, 27, y, 35, Part::Head { sticky: true, dir: back }.block())?;
            set(self, 27, y, 36, Part::Piston { sticky: true, extended: true, dir: back }.block())?;
        }
        set(self, 28, 0, 36, B::SwitchOn)?;
        set(self, 28, 1, 36, B::IskritWire)?;
        signs.push(((27, 0, 30), "Потайной проход: обойдите стену и опустите рычаг — липкие поршни утянут камни"));
        // Stand 14: light on a sensor. A plate lights a row of lamps.
        set(self, 33, 0, 33, B::PressurePlate)?;
        for z in 34..=37 {
            set(self, 33, 0, z, B::IskritWire)?;
            set(self, 34, 0, z, B::IskritLamp)?;
        }
        signs.push(((33, 0, 30), "Свет по датчику: встаньте на плиту — загорятся лампы"));
        // Signs with the explanations, in front of every stand.
        for ((x, y, z), text) in signs {
            set(self, x, y, z, B::Sign)?;
            if let Some(c) = at(x, y, z) {
                self.block_commands.insert(c, text.to_string());
            }
        }
        self.save_block_commands();
        // Work out every network once.
        for c in placed {
            if matches!(self.chunks.get_block(c).map(|b| b.unwall()), Some(B::IskritWire | B::IskritTorch | B::IskritBlock | B::IskritLamp | B::SwitchOn)) || self.chunks.get_block(c).is_some_and(|b| b.part().is_some()) {
                wolfcraft_engine::power::update_iskrit(&mut self.chunks, c);
            }
        }
        Ok("Выставка искрита построена перед вами: 14 стендов в два ряда — схемы, автодвери, ферма, потайной проход. Подойдите к табличке, чтобы прочитать".into())
    }

    pub fn cycle_camera(&mut self) {
        self.camera_mode = self.camera_mode.next();
    }

    pub fn teleport(&mut self, pos: DVec3) {
        self.player.body.pos = pos;
        self.player.body.vel = DVec3::ZERO;
        self.player.align_heading();
        self.prev_radius = radial(pos);
        self.cam_lag = 0.0;
    }

    pub fn is_creative(&self) -> bool {
        self.studio || self.cfg.mode == GameMode::Creative
    }

    // ---- values for rendering / HUD ----
    pub fn sun(&self) -> DVec3 {
        local_sun(self.player.world_time, self.player.up())
    }

    /// Daylight at the player's position (0 night … 1 day).
    pub fn daylight(&self) -> f32 {
        smoothstep(-0.10, 0.14, self.player.up().dot(self.sun())) as f32
    }

    pub fn clock(&self) -> String {
        clock_string(self.player.world_time)
    }

    pub fn eye_in_water(&self) -> bool {
        self.chunks.block_at_world(self.player.eye_pos()).is_water()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wolfcraft_engine::items::ItemStack;
    use crate::loader::{LoadRequest, start};
    use std::time::{Duration, Instant};
    use wolfcraft_engine::block::Block;
    use wolfcraft_engine::config::PlanetSize;

    fn load(name: &str, seed: u64, creative: bool) -> GameSession {
        let dir = std::env::temp_dir().join(format!("wc_session_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = WorldConfig::new("Сессия", seed);
        cfg.size = PlanetSize::Small;
        cfg.structures = true;
        cfg.mode = if creative { GameMode::Creative } else { GameMode::Survival };
        let sh = start(LoadRequest { cfg, worlds_dir: dir, is_new: true });
        while !sh.is_done() {
            std::thread::sleep(Duration::from_millis(2));
        }
        GameSession::new(sh.take_result().unwrap().unwrap())
    }

    fn settle(s: &mut GameSession) {
        let t = Instant::now();
        loop {
            s.update(&InputFrame::default(), 0.016, 4, true);
            let (done, total) = s.chunks.progress(s.player_chunk(), 3);
            if done == total && s.chunks.stats.gen_inflight == 0 {
                break;
            }
            assert!(t.elapsed().as_secs() < 90, "chunks never finished: {done}/{total}, inflight {}, chunk {:?}", s.chunks.stats.gen_inflight, s.player_chunk());
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn hammer_mechanics_regression() {
        let mut s = load("hammer_mechanics", 1837, true);
        settle(&mut s);
        let g = s.chunks.grid();
        let c = g.world_to_cell(s.player.body.pos);
        // A saw processes identically under smooth and irregular frame schedules.
        assert!(s.chunks.set_block(c, Block::MechSaw));
        for schedule in [vec![0.016; 250], vec![0.13, 0.17, 0.7, 1.1, 0.9, 1.0]] {
            s.kin.clear(); s.kin_accum = 0.0; s.kin_t = 100.0;
            s.kin.insert(c, KinNode { speed: 24.0, ..Default::default() });
            s.containers.insert(c, vec![Some(ItemStack::of_block(Block::OakLog, 10)), None, None]);
            for dt in schedule { s.tick_kinetic(&[], dt); }
            let v = &s.containers[&c];
            assert_eq!(v[0].unwrap().count, 7);
            assert_eq!(v[2].unwrap().count, 18);
            assert!(s.kin[&c].progress < 0.01);
        }
        assert!(s.chunks.set_block(c, Block::ElectricMotor));
        s.kin.clear(); s.kin_accum = 0.0; s.kin_t = 100.0;
        s.kin.insert(c, KinNode::default());
        s.tech.insert(c, TechState { energy: 16.0, ..Default::default() });
        s.tick_kinetic(&[], 1.0);
        assert!(s.tech[&c].energy < 0.001);
        s.tick_kinetic(&[], 0.05);
        assert!(!s.tech[&c].working, "an unpaid motor must not power dynamos through coasting");
        s.tech.clear();
        assert!(!s.hammer_breakable(c, Block::MechSaw));
        assert!(!s.hammer_breakable(c, Block::Bedrock));
        s.containers.remove(&c); s.kin.clear();
        assert!(s.hammer_breakable(c, Block::Stone));
        // The utility hammer rotates a shaft, refuses occupied machines, then returns a block.
        let eye = g.cell_center(c) + wolfcraft_engine::physics::up_at(g.cell_center(c)) * 3.0;
        let look = (g.cell_center(c) - eye).normalize();
        assert!(s.chunks.set_block(c, Block::Shaft));
        s.use_ship_hammer(eye, look);
        assert_eq!(s.chunks.get_block(c), Some(Block::ShaftJ));
        s.hammer_cd = 0.0; s.ship_hammer_mode = 1;
        s.containers.insert(c, vec![Some(ItemStack::new(wolfcraft_engine::items::ids::IRON_INGOT, 3))]);
        s.use_ship_hammer(eye, look);
        assert_eq!(s.chunks.get_block(c), Some(Block::ShaftJ));
        s.containers.remove(&c); s.hammer_cd = 0.0;
        let drops = s.drops.list.len();
        s.use_ship_hammer(eye, look);
        assert_eq!(s.chunks.get_block(c), Some(Block::Air));
        assert_eq!(s.drops.list.len(), drops + 1);
        assert!(s.chunks.set_block(c, Block::Stone));
        let chest = g.offset(c, 1, 0, 0).unwrap();
        assert!(s.chunks.set_block(chest, Block::Chest));
        s.hammer_cd = 0.0;
        s.hammer_slam(eye, look);
        assert_eq!(s.chunks.get_block(c), Some(Block::Air), "shockwave follows the aimed block");
        assert_eq!(s.chunks.get_block(chest), Some(Block::Chest), "shockwave preserves nearby storage");
    }

    /// A bot plays for a few game minutes in three worlds (survival, creative, studio
    /// baseplate): walks, sprints, sneaks, jumps, looks around, breaks and places blocks,
    /// pours water and lava, builds the iskrit showcase and flips its switches, places
    /// pistons, changes seasons and the time of day. Checks: no panic, the player never ends
    /// up inside a solid block, frame time. Run: `cargo test -p wolfcraft-game soak -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn soak_bot_plays_without_crashing() {
        use wolfcraft_engine::block::Part;
        use wolfcraft_engine::config::WorldType;
        use wolfcraft_engine::items::ItemStack;
        let mut report = Vec::new();
        for (name, world_type, creative) in [("soak_survival", WorldType::Standard, false), ("soak_creative", WorldType::Standard, true), ("soak_studio", WorldType::Baseplate, true)] {
            let dir = std::env::temp_dir().join(format!("wc_session_{name}"));
            let _ = std::fs::remove_dir_all(&dir);
            let mut cfg = WorldConfig::new(name, 31337);
            cfg.size = PlanetSize::Small;
            cfg.world_type = world_type;
            cfg.mode = if creative { GameMode::Creative } else { GameMode::Survival };
            let sh = start(LoadRequest { cfg, worlds_dir: dir, is_new: true });
            while !sh.is_done() {
                std::thread::sleep(Duration::from_millis(2));
            }
            let mut s = GameSession::new(sh.take_result().unwrap().unwrap());
            settle(&mut s);
            if creative {
                let _ = crate::commands::run(&mut s, "/demo");
            }
            // Hotbar: things worth trying.
            let kit = [
                ItemStack::of_block(Block::Cobblestone, 64),
                ItemStack::of_block(Block::Water, 8),
                ItemStack::of_block(Block::Lava, 8),
                ItemStack::of_block(Part::Piston { sticky: true, extended: false, dir: 2 }.block(), 8),
                ItemStack::of_block(Block::IskritTorch, 16),
                ItemStack::of_block(Block::Door, 8),
                ItemStack::of_block(Block::GlassPaneA, 16),
                ItemStack::new(wolfcraft_engine::items::ids::tool(3, 0), 1),
                ItemStack::new(wolfcraft_engine::items::ids::tool(3, 3), 1),
            ];
            for (i, k) in kit.iter().enumerate() {
                s.player.inventory.slots[i] = Some(*k);
            }
            let mut rng = 0x5eedu64;
            let mut rnd = || {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                (rng >> 11) as f64 / (1u64 << 53) as f64
            };
            let (mut worst, mut total, mut inside, mut slow) = (0.0f64, 0.0f64, 0usize, 0usize);
            let mut input = InputFrame::default();
            let frames = 60 * 60 * 3; // three game minutes per world
            for f in 0..frames {
                if f % 45 == 0 {
                    input = InputFrame {
                        forward: if rnd() < 0.75 { 1.0 } else { -0.5 },
                        strafe: (rnd() as f32 - 0.5) * 2.0,
                        jump: rnd() < 0.2,
                        sprint: rnd() < 0.4,
                        sneak: rnd() < 0.15,
                        look: ((rnd() - 0.5) * 0.15, (rnd() - 0.5) * 0.06),
                        break_held: rnd() < 0.3,
                        ..Default::default()
                    };
                    s.player.inventory.selected = (rnd() * 9.0) as usize % 9;
                }
                let mut frame = input.clone();
                frame.use_pressed = f % 50 == 25 && rnd() < 0.6;
                frame.jump_pressed = frame.jump && f % 45 == 0;
                if f % 1800 == 900 {
                    let _ = crate::commands::run(&mut s, ["/season осень", "/season зима", "/time set ночь", "/time set день"][(f / 1800) % 4]);
                }
                let before = s.player.body.pos;
                let t = Instant::now();
                s.update(&frame, 1.0 / 60.0, 4, false);
                let _ = before;
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                if ms > 12.0 {
                    slow += 1;
                }
                worst = worst.max(ms);
                total += ms;
                // Never inside a solid block (centre of a body sphere in a block's box).
                let up = s.player.up();
                for h in [0.3, 0.9, 1.5] {
                    let p = s.player.body.pos + up * h;
                    let c = s.chunks.grid().world_to_cell(p);
                    if let Some(b) = s.chunks.get_block(c) {
                        if b.is_solid() && !s.player.body.flying {
                            let obbs = wolfcraft_engine::physics::block_obbs(&s.chunks.grid(), c, b);
                            if obbs.iter().any(|o| {
                                let d = p - o.center;
                                (0..3).all(|a| d.dot(o.axes[a]).abs() < o.half[a] - 0.05)
                            }) {
                                inside += 1;
                                if inside <= 5 {
                                    eprintln!("{name} frame {f}: inside {b:?} at sphere {h}, sneaking {}, held {:?}", s.player.sneaking, s.player.inventory.held().map(|h| h.id));
                                }
                            }
                        }
                    }
                }
                if s.player.survival.dead {
                    s.player.respawn();
                }
            }
            report.push(format!("{name}: {frames} frames, avg {:.2} ms, worst {:.1} ms, over 12 ms: {slow}, frames with the body inside a block: {inside}, mobs {}, particles {}", total / frames as f64, worst, s.mobs.mobs.len(), s.mobs.debris.len()));
            assert!(inside < frames / 200, "{name}: the body was inside blocks in {inside} frames");
        }
        for r in report {
            eprintln!("{r}");
        }
    }

    #[test]
    fn studio_baseplate_world_is_an_empty_plate() {
        use wolfcraft_engine::config::WorldType;
        let dir = std::env::temp_dir().join("wc_session_baseplate");
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = WorldConfig::new("Проект", 9);
        cfg.size = PlanetSize::Small;
        cfg.world_type = WorldType::Baseplate;
        cfg.mode = GameMode::Creative;
        cfg.structures = false;
        cfg.studio_project = true;
        let sh = start(LoadRequest { cfg, worlds_dir: dir, is_new: true });
        while !sh.is_done() {
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut s = GameSession::new(sh.take_result().unwrap().unwrap());
        settle(&mut s);
        for _ in 0..240 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        }
        let feet = s.chunks.grid().world_to_cell(s.player.body.pos);
        let below = s.chunks.grid().neighbor(feet, 2, -1).and_then(|c| s.chunks.get_block(c));
        assert_eq!(below, Some(Block::Baseplate), "standing on the baseplate");
        // Nothing else around: no trees, water or terrain above the plate.
        for di in -10..=10 {
            for dj in -10..=10 {
                for dk in 0..6 {
                    if let Some(c) = s.chunks.grid().offset(feet, di, dj, dk) {
                        assert_eq!(s.chunks.get_block(c), Some(Block::Air));
                    }
                }
            }
        }
        assert!(s.mobs.mobs.is_empty(), "no creatures on a studio baseplate");
    }

    #[test]
    fn iskrit_demo_lights_the_right_lamps() {
        let mut s = load("iskrit_demo", 77, true);
        settle(&mut s);
        let msg = s.build_iskrit_demo().expect("demo builds");
        assert!(msg.contains("14 стендов"));
        // Count lamps: torch, block (×2), 15-dust line and staircase lamps are lit; the
        // button, the switch and the 16-dust line are not.
        let feet = s.chunks.grid().world_to_cell(s.player.body.pos);
        let (mut on, mut off) = (0, 0);
        for di in -45..=45 {
            for dj in -45..=45 {
                for dk in -2..=4 {
                    if let Some(c) = s.chunks.grid().offset(feet, di, dj, dk) {
                        match s.chunks.get_block(c) {
                            Some(Block::IskritLampOn) => on += 1,
                            Some(Block::IskritLamp) => off += 1,
                            _ => {}
                        }
                    }
                }
            }
        }
        assert_eq!((on, off), (6, 7), "lit and unlit lamps (the repeater line lights its lamp; the sensor lamps wait)");
        assert!(s.block_commands.values().any(|t| t.contains("15 блоков")), "signs carry the explanations");
        // Automatic door: stepping on a plate beside a door swings it, stepping off swings it back.
        let g = s.chunks.grid();
        let mut found = None;
        'scan: for di in -45..=45 {
            for dj in -45..=45 {
                if let Some(c) = g.offset(feet, di, dj, 0) {
                    if s.chunks.get_block(c) == Some(Block::PressurePlate) {
                        for (a, sg) in [(0, 1), (0, -1), (1, 1), (1, -1)] {
                            if let Some(n) = g.neighbor(c, a, sg) {
                                if s.chunks.get_block(n).is_some_and(|b| b.door_toggle().is_some()) {
                                    found = Some((c, n));
                                    break 'scan;
                                }
                            }
                        }
                    }
                }
            }
        }
        let (plate, door) = found.expect("a plate next to a door");
        let closed = s.chunks.get_block(door);
        s.player.body.flying = false;
        s.teleport(g.cell_center(plate) - wolfcraft_engine::physics::up_at(g.cell_center(plate)) * 0.45);
        for _ in 0..3 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 3, false);
        }
        assert_eq!(s.chunks.get_block(plate), Some(Block::PressurePlateOn), "pressed");
        assert_ne!(s.chunks.get_block(door), closed, "the door swung open");
        let away = g.offset(plate, 0, 0, 6).unwrap();
        s.teleport(g.cell_center(away));
        for _ in 0..60 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 3, false);
        }
        assert_eq!(s.chunks.get_block(door), closed, "and shut after we left");
    }

    /// Cube planet end to end: spawn, stand, then walk over a cube edge onto the next face.
    /// Ignored by default because it switches the process-wide planet shape; run it alone:
    /// `cargo test -p wolfcraft-game cube_planet -- --ignored`.
    #[test]
    #[ignore]
    fn cube_planet_spawn_and_walk_over_an_edge() {
        use wolfcraft_engine::config::{PlanetShape, WorldType};
        let dir = std::env::temp_dir().join("wc_session_cube");
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = WorldConfig::new("Куб", 5);
        cfg.size = PlanetSize::Small;
        cfg.world_type = WorldType::Flat;
        cfg.structures = false;
        cfg.shape = PlanetShape::Cube;
        let sh = start(LoadRequest { cfg, worlds_dir: dir, is_new: true });
        while !sh.is_done() {
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut s = GameSession::new(sh.take_result().unwrap().unwrap());
        assert!(s.planet.grid.cube);
        settle(&mut s);
        for _ in 0..180 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        }
        assert!(s.player.body.on_ground, "stands on a cube face");
        // Go close to the edge between +X and +Y and walk toward +Y.
        let d = DVec3::new(1.0, 0.93, 0.05).normalize();
        let start_pos = s.planet.stand_position(d).expect("ground near the edge");
        s.teleport(start_pos);
        settle(&mut s);
        let f0 = s.planet.grid.world_to_cell(s.player.body.pos).f;
        let walk = InputFrame { forward: 1.0, ..Default::default() };
        let mut crossed = false;
        let mut last = s.player.body.pos;
        let mut stuck = 0;
        for step in 0..60 * 120 {
            let up = s.player.up();
            s.player.heading = (DVec3::Y - up * DVec3::Y.dot(up)).normalize_or_zero();
            s.update(&walk, 1.0 / 60.0, 4, step % 30 == 0);
            if step % 60 == 0 {
                settle(&mut s);
            }
            let c = s.planet.grid.world_to_cell(s.player.body.pos);
            assert!(s.planet.grid.radial(s.player.body.pos) > s.planet.radius - 20.0, "fell into the planet at step {step}");
            if c.f != f0 {
                crossed = true;
            }
            if crossed && s.player.up().y > 0.95 {
                break;
            }
            if step % 60 == 59 {
                let moved = (s.player.body.pos - last).length();
                stuck = if moved < 1.0 { stuck + 1 } else { 0 };
                if stuck >= 1 {
                    // Trees grow on the flat world: chop whatever blocks the way.
                    let g = s.planet.grid;
                    let (feet, up, h) = (s.player.body.pos, s.player.up(), s.player.heading);
                    for a in 1..5 {
                        for dy in 0..3 {
                            let c = g.world_to_cell(feet + h * (a as f64 * 0.5) + up * (dy as f64 + 0.5));
                            s.chunks.set_block(c, Block::Air);
                        }
                    }
                }
                assert!(stuck < 3, "stuck at {:?} (up {:?})", s.player.body.pos, s.player.up());
                last = s.player.body.pos;
            }
        }
        assert!(crossed, "never reached the next face");
        for _ in 0..60 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        }
        assert!(s.player.body.on_ground, "stands on the new face");
        assert!(s.player.up().y > 0.95, "up is now the +Y face normal: {:?}", s.player.up());
        wolfcraft_engine::cubesphere::set_cube_world(false);
    }

    #[test]
    fn arena_summons_a_guardian_that_fights_dies_once_and_pays_once() {
        use wolfcraft_engine::bosses::BossKind;
        use wolfcraft_engine::items::ids;
        let mut s = load("arena", 33, false);
        settle(&mut s);
        for _ in 0..60 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        }
        s.cfg.difficulty = wolfcraft_engine::config::Difficulty::Normal;
        let altar = s.build_arena(BossKind::Stone).expect("arena builds on loaded ground");
        assert!(wolfcraft_engine::construction::altar(s.chunks.get_block(altar).unwrap()).is_some(), "the altar stands in the middle");
        assert_eq!(s.player.inventory.count(ids::RITUAL_SEAL), 1, "a seal is handed out");
        let g = s.chunks.grid();
        // The floor is solid and the air above it is clear.
        let under = g.offset(altar, 3, 0, -1).unwrap();
        assert!(s.chunks.get_block(under).unwrap().is_solid());
        assert_eq!(s.chunks.get_block(g.offset(altar, 3, 0, 2).unwrap()), Some(wolfcraft_engine::block::Block::Air));
        // Stand four blocks from the altar and wake the guardian.
        let stand = g.cell_center(g.offset(altar, 4, 0, 0).unwrap());
        s.player.body.pos = stand - wolfcraft_engine::physics::up_at(stand) * 0.45;
        s.player.body.vel = DVec3::ZERO;
        let mut ev = vec![InteractionEvent::ActivateBoss { pos: altar, kind: BossKind::Stone }];
        s.handle_building_events(&mut ev, None);
        let boss_hp = |s: &GameSession| s.mobs.mobs.iter().find(|m| m.boss.is_some()).map(|m| m.hp);
        assert_eq!(boss_hp(&s), Some(180.0), "the guardian wakes with full health");
        assert_eq!(s.player.inventory.count(ids::RITUAL_SEAL), 0, "the seal is spent");
        // A second tap does not wake another one.
        let mut ev = vec![InteractionEvent::ActivateBoss { pos: altar, kind: BossKind::Stone }];
        s.handle_building_events(&mut ev, None);
        assert_eq!(s.mobs.mobs.iter().filter(|m| m.boss.is_some()).count(), 1);
        // It attacks: standing next to it costs health within a quarter of a minute.
        let mut hurt = false;
        for _ in 0..900 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
            if s.player.survival.health < 19.5 {
                hurt = true;
                break;
            }
        }
        assert!(hurt, "the guardian's blow reaches a player standing near it");
        s.player.survival.health = 20.0;
        s.player.survival.dead = false;
        // Phases follow its health; the first phase shrugs off part of the damage.
        let i = s.mobs.mobs.iter().position(|m| m.boss.is_some()).unwrap();
        let from = s.player.body.pos;
        s.mobs.hurt(i, 10.0, from);
        assert_eq!(boss_hp(&s), Some(173.0), "phase one takes 30 % less");
        while boss_hp(&s).is_some_and(|hp| hp > 0.0) {
            let i = s.mobs.mobs.iter().position(|m| m.boss.is_some()).unwrap();
            s.mobs.hurt(i, 40.0, from);
        }
        s.player.survival.health = 20.0;
        s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        assert_eq!(s.player.inventory.count(ids::REVIVAL_HEART), 1, "the rare reward is paid");
        assert_eq!(s.player.inventory.count(ids::RESONANCE_RUNE), 1);
        assert!(s.boss_book.records.values().any(|r| r.defeated));
        // Defeated for good: the altar stays quiet, no second reward.
        for _ in 0..30 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        }
        s.player.inventory.add(wolfcraft_engine::items::ItemStack::new(ids::RITUAL_SEAL, 1));
        let mut ev = vec![InteractionEvent::ActivateBoss { pos: altar, kind: BossKind::Stone }];
        s.handle_building_events(&mut ev, None);
        assert!(s.mobs.mobs.iter().all(|m| m.boss.is_none()), "a defeated guardian does not come back at once");
        assert_eq!(s.player.inventory.count(ids::REVIVAL_HEART), 1);
        // With a seal and a resonance rune the ritual of revival begins and, after a few
        // seconds of light and sparks, the guardian stands up again at full health.
        assert!(s.ritual.is_some(), "the ritual begins");
        assert_eq!(s.player.inventory.count(ids::RITUAL_SEAL), 0, "the seal is spent");
        assert_eq!(s.player.inventory.count(ids::RESONANCE_RUNE), 0, "and the rune");
        for _ in 0..(10 * 60) {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
            if s.ritual.is_none() {
                break;
            }
        }
        assert!(s.ritual.is_none());
        assert!(s.mobs.mobs.iter().any(|m| m.boss.is_some()), "the guardian is back");
        assert!(s.boss_book.records.values().all(|r| !r.defeated));
    }

    #[test]
    fn a_bed_sleeps_through_the_night_and_a_chair_seats_the_player() {
        use wolfcraft_engine::construction::{self as c, Form};
        use wolfcraft_engine::player::{DAY_LENGTH, day_fraction};
        let mut s = load("sleep", 34, false);
        settle(&mut s);
        for _ in 0..60 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        }
        let g = s.chunks.grid();
        let feet = g.world_to_cell(s.player.body.pos + s.player.up() * 0.5);
        let bed_cell = g.offset(feet, 2, 0, 0).unwrap();
        assert!(s.chunks.set_block(bed_cell, c::block(Form::Bed, 0, 0).unwrap()));
        // Midnight: a tap on the bed lays the player down and the night runs by.
        s.player.world_time = 3.0 * DAY_LENGTH;
        let stood = s.player.body.pos;
        let mut ev = vec![InteractionEvent::Slept { night: true, pos: bed_cell }];
        s.handle_building_events(&mut ev, None);
        assert!(matches!(s.rest, Some(Rest::Sleep { .. })));
        assert_eq!(s.camera_mode, CameraMode::ThirdBack);
        let mut darkest = 0.0f32;
        for _ in 0..(GameSession::SLEEP_SECS * 60.0) as usize + 30 {
            s.update(&InputFrame { forward: 0.0, ..Default::default() }, 1.0 / 60.0, 4, false);
            darkest = darkest.max(s.sleep_fade());
        }
        assert!(darkest > 0.8, "the screen goes dark while asleep");
        assert!(s.rest.is_none(), "awake again");
        assert_eq!(s.camera_mode, CameraMode::First, "the camera is back");
        let f = day_fraction(s.player.world_time);
        assert!((0.26..0.30).contains(&f), "it is morning: {f}");
        assert!((s.player.body.pos - stood).length() < 1.5, "back beside the bed");
        // By day a bed only sets the spawn point.
        let mut ev = vec![InteractionEvent::Slept { night: false, pos: bed_cell }];
        s.handle_building_events(&mut ev, None);
        assert!(s.rest.is_none());
        // A chair: sit, then stand up by moving.
        let mut ev = vec![InteractionEvent::Sat { pos: bed_cell }];
        s.handle_building_events(&mut ev, None);
        assert!(matches!(s.rest, Some(Rest::Sit { .. })));
        s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        assert!(s.rest.is_some(), "stays seated while still");
        s.update(&InputFrame { forward: 1.0, ..Default::default() }, 1.0 / 60.0, 4, false);
        assert!(s.rest.is_none(), "moving stands up");
    }

    #[test]
    fn bow_draws_shoots_and_the_arrow_sticks() {
        use wolfcraft_engine::items::{ItemStack, ids};
        let mut s = load("bow", 21, false);
        settle(&mut s);
        for _ in 0..120 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        }
        s.player.inventory.slots[0] = Some(ItemStack::new(ids::BOW, 1));
        s.player.inventory.slots[1] = Some(ItemStack::new(ids::ARROW, 5));
        s.player.inventory.selected = 0;
        s.player.pitch = 0.2;
        let hold = InputFrame { use_held: true, ..Default::default() };
        for _ in 0..70 {
            s.update(&hold, 1.0 / 60.0, 4, false);
        }
        assert!(s.bow_charge >= 1.0, "fully drawn after a second: {}", s.bow_charge);
        s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        assert_eq!(s.arrows.list.len(), 1, "release shoots");
        assert_eq!(s.player.inventory.count(ids::ARROW), 4, "one arrow used");
        let start = s.arrows.list[0].pos;
        for _ in 0..600 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
            if s.arrows.list.first().is_some_and(|a| a.stuck) {
                break;
            }
        }
        let a = &s.arrows.list[0];
        assert!(a.stuck, "the arrow lands and sticks");
        assert!((a.pos - start).length() > 10.0, "it flew some distance: {}", (a.pos - start).length());
    }

    #[test]
    fn player_spawns_stands_on_the_ground_and_walks() {
        let mut s = load("walk", 17, false);
        settle(&mut s);
        for _ in 0..180 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        }
        assert!(s.player.body.on_ground, "player should be standing after spawning");
        assert!(s.player.survival.health == 20.0, "no damage from spawning");
        let before = s.player.body.pos;
        let walk = InputFrame { forward: 1.0, ..Default::default() };
        for _ in 0..180 {
            s.update(&walk, 1.0 / 60.0, 4, false);
            for _ in 0..0 {}
        }
        assert!((s.player.body.pos - before).length() > 3.0, "player must move forward");
        // Still on the surface layer (didn't fall through the world).
        assert!(s.player.body.pos.length() > s.planet.radius - 20.0);
    }

    #[test]
    fn breaking_and_placing_a_block_works_end_to_end_and_persists() {
        let mut s = load("edit", 18, true);
        settle(&mut s);
        for _ in 0..120 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        }
        // Look straight down and break the ground under the feet.
        s.player.pitch = -0.6; // look at the ground ahead, not under the feet
        s.update(&InputFrame::default(), 0.016, 4, false);
        let hit = s.hit.expect("looking down must hit the ground");
        let pos = hit.cell;
        let was = hit.block;
        assert!(was != Block::Air);
        s.update(&InputFrame { break_held: true, ..Default::default() }, 0.016, 4, false);
        assert_eq!(s.chunks.get_block(pos), Some(Block::Air), "creative breaks instantly");
        // Place a lamp where it was broken.
        s.player.inventory.slots[0] = Some(ItemStack::of_block(Block::Lamp, 1));
        s.player.inventory.selected = 0;
        s.update(&InputFrame::default(), 0.016, 4, false);
        if let Some(h) = s.hit {
            s.update(&InputFrame { use_pressed: true, ..Default::default() }, 0.016, 4, false);
            let placed = h.front_cell(&s.chunks.grid()).expect("cell in front of the hit face");
            assert_eq!(s.chunks.get_block(placed), Some(Block::Lamp));
            s.save();
            // Reload the same world: edit and player must come back.
            let dir = s.dir.clone();
            let cfg = s.cfg.clone();
            let player_pos = s.player.body.pos;
            drop(s);
            let sh = start(LoadRequest { cfg, worlds_dir: dir.parent().unwrap().to_path_buf(), is_new: false });
            while !sh.is_done() {
                std::thread::sleep(Duration::from_millis(2));
            }
            let mut s2 = GameSession::new(sh.take_result().unwrap().unwrap());
            assert_eq!(s2.player.body.pos, player_pos, "position restored");
            settle(&mut s2);
            assert_eq!(s2.chunks.get_block(placed), Some(Block::Lamp), "placed block restored");
            if pos != placed {
                assert_eq!(s2.chunks.get_block(pos), Some(Block::Air), "broken block stays broken after reload");
            }
        }
    }

    #[test]
    fn standing_player_does_not_drift_on_real_terrain() {
        for (seed, name) in [(3u64, "drift3"), (31, "drift31"), (57, "drift57")] {
            let mut s = load(name, seed, false);
            settle(&mut s);
            for _ in 0..120 {
                s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
            }
            let rest = s.player.body.pos;
            for _ in 0..(60 * 20) {
                s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
            }
            let drift = (s.player.body.pos - rest).length();
            assert!(drift < 0.05, "seed {seed}: player crept {drift} blocks while standing still");
        }
    }

    #[test]
    fn third_person_camera_stays_outside_blocks_and_behind() {
        let mut s = load("cam", 21, false);
        settle(&mut s);
        for _ in 0..120 {
            s.update(&InputFrame::default(), 1.0 / 60.0, 4, false);
        }
        let first = s.camera();
        assert!((first.eye - (s.player.body.pos + s.player.up() * 1.62)).length() < 0.05, "first person = eye level");
        s.camera_mode = CameraMode::ThirdBack;
        let back = s.camera();
        let d = (back.eye - first.eye).length();
        assert!(d > 0.2 && d <= 4.3, "orbit distance {d}");
        assert!((back.forward - first.forward).length() < 1e-9);
        assert!(!s.chunks.block_at_world(back.eye).is_solid(), "camera inside a block");
        s.camera_mode = CameraMode::ThirdFront;
        let front = s.camera();
        assert!((front.forward + first.forward).length() < 1e-9, "front camera looks back at the player");
        assert_eq!(CameraMode::First.next().next().next(), CameraMode::First);
    }

    #[test]
    fn day_cycle_changes_daylight() {
        let mut s = load("day", 19, true);
        s.player.world_time = 600.0; // noon
        let noon = s.daylight();
        s.player.world_time = 0.0;
        let midnight = s.daylight();
        // The player sits somewhere on the planet: at least the sun position differs.
        assert!((noon - midnight).abs() > 0.5 || s.player.up().dot(s.sun()).abs() < 0.3);
    }
}

/// The player rests: asleep in a bed or sitting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rest {
    /// `at`: middle of the bed top, `dir`: towards the pillow, `back`: where the player
    /// stood, `t`: seconds asleep, `t0..t1`: the world time the sleep runs through.
    Sleep { at: DVec3, dir: DVec3, back: DVec3, t: f32, t0: f64, t1: f64 },
    /// `at`: the seat, `dir`: where the sitter faces, `back`: where the player stood.
    Sit { at: DVec3, dir: DVec3, back: DVec3 },
}

/// Effect of a tipped arrow item.
pub fn tipped_effect(id: wolfcraft_engine::items::ItemId) -> Option<wolfcraft_engine::magic::Effect> {
    use wolfcraft_engine::items::ids;
    use wolfcraft_engine::magic::Effect;
    (ids::TIPPED_BASE..=ids::TIPPED_LAST).contains(&id).then(|| [Effect::Poison, Effect::Slowness, Effect::Harm][(id - ids::TIPPED_BASE) as usize])
}

