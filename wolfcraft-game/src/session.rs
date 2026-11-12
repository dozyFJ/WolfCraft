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
use wolfcraft_engine::physics::{Hit, Voxels, raycast};
use wolfcraft_engine::planet::{Planet, smoothstep};
use wolfcraft_engine::player::{Controls, Player, clock_string, local_sun};

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
    /// Vertical camera lag (blocks along up) that smooths automatic steps.
    cam_lag: f64,
    prev_radius: f64,
    /// Smoothed eye radius (filters tiny collision jitter).
    eye_r: f64,
    /// Smoothed third-person distance.
    tp_dist: f64,
    pub mobs: wolfcraft_engine::mobs::MobManager,
    /// Arrows in flight or stuck in blocks.
    pub arrows: wolfcraft_engine::projectiles::Arrows,
    /// Bow draw 0..1 while the use button is held with a bow.
    pub bow_charge: f32,
    prev_use_held: bool,
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
    portal_t: f32,
    pending_portal: Option<wolfcraft_engine::cubesphere::Cell>,
    pub block_commands: std::collections::HashMap<wolfcraft_engine::cubesphere::Cell, String>,
    pub containers: std::collections::HashMap<wolfcraft_engine::cubesphere::Cell, Vec<Option<wolfcraft_engine::items::ItemStack>>>,
    /// Smelting progress in seconds per furnace.
    furnace_t: std::collections::HashMap<wolfcraft_engine::cubesphere::Cell, f32>,
    /// The start position was checked for blocks (a structure may have been built over it).
    spawn_checked: bool,
    /// Opened in the studio: creative rules without changing the world's saved mode.
    pub studio: bool,
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
            cam_lag: 0.0,
            prev_radius: 0.0,
            eye_r: 0.0,
            tp_dist: 4.2,
            mobs: wolfcraft_engine::mobs::MobManager::new(seed),
            attack_cd: 0.0,
            death_burst_done: false,
            arrows: Default::default(),
            bow_charge: 0.0,
            prev_use_held: false,
            containers: Default::default(),
            block_commands: Default::default(),
            hell: false,
            nickname: "Игрок".to_string(),
            remote_edits: Vec::new(),
            dimension_changed: false,
            overworld_pos: None,
            portal_t: -2.0,
            pending_portal: None,
            furnace_t: Default::default(),
            spawn_checked: false,
            studio: false,
            buttons: Vec::new(),
            plates: Vec::new(),
            tick_acc: 0.0,
            we: Default::default(),
            net_out: Vec::new(),
            fluids: Default::default(),
            radiation: 0.0,
            mob_hits: 0,
            picked_up: 0,
            aim_dir: None,
            fx_amount: 1.0,
            fx_lights: Vec::new(),
            fx_scan: 0.0,
            fx_dust: 0.0,
            fx_was_in_water: false,
            fx_prev_vup: 0.0,
            fx_was_ground: true,
            env_col: None,
        };
        // Studio baseplates stay empty: no creatures appear.
        s.mobs.spawning = s.cfg.world_type != wolfcraft_engine::config::WorldType::Baseplate;
        s.load_containers();
        s.load_block_commands();
        s.load_dimension();
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
        self.clock += dt;
        self.chunks.update(self.player_chunk(), view, 6);
        let mut events = Vec::new();
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
                if b == wolfcraft_engine::block::Block::ButtonOn && !self.buttons.iter().any(|(p, _)| *p == c) {
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
        // Shield up while RMB is held, unless the main hand has its own right-click use.
        let holding_bow = self.player.inventory.held().is_some_and(|s| s.id == wolfcraft_engine::items::ids::BOW);
        let main_use = holding_bow
            || self.player.inventory.held().is_some_and(|s| {
                let i = s.info();
                i.place.is_some() || i.food.is_some()
            });
        // Bow: hold to draw (one second for full power), release to shoot.
        let has_arrows = self.player.is_creative() || self.player.inventory.count(wolfcraft_engine::items::ids::ARROW) > 0;
        if holding_bow && input.use_held && has_arrows && !self.player.survival.dead {
            self.bow_charge = (self.bow_charge + dt).min(1.0);
        } else {
            if holding_bow && self.prev_use_held && !input.use_held && self.bow_charge > 0.12 && has_arrows {
                let power = self.bow_charge;
                let creative = self.player.is_creative();
                if !creative {
                    self.player.inventory.take(wolfcraft_engine::items::ids::ARROW, 1);
                    self.player.inventory.wear_selected();
                }
                self.arrows.shoot(self.player.eye_pos(), self.player.look_dir(), power, !creative);
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
            self.item_name = self.player.inventory.held().map(|s| (s.info().name.to_string(), 2.0));
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

        // Physics, split into steps of at most 1/30 s.
        if self.ground_ready() && !self.player.survival.dead {
            let controls = Controls { forward: input.forward, strafe: input.strafe, jump: input.jump, sprint: input.sprint, sneak: input.sneak };
            let mut left = dt.min(0.25) as f64;
            while left > 1e-6 {
                let step = left.min(1.0 / 30.0);
                self.player.update(&self.chunks, &controls, self.cfg.difficulty, step);
                left -= step;
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
            events.extend(self.interaction.update_break(&mut self.chunks, &mut self.player, hit, input.break_held, dt, self.cfg.seed));
            // A tap on a creature in the crosshair hits it (touch screens: a short tap is "use").
            if input.use_pressed && !self.try_attack(0.25) {
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
            if dealt > 0.0 {
                let armor = self.player.armor_points();
                let reduced = dealt * (1.0 - (armor as f32 * 0.04).min(0.8));
                self.player.damage(reduced, self.cfg.difficulty);
            }
            if self.player.survival.dead && !self.death_burst_done {
                self.death_burst_done = true;
                self.mobs.burst(self.player.body.pos, &wolfcraft_engine::renderer::model::player_palette(), 1.8);
            } else if !self.player.survival.dead {
                self.death_burst_done = false;
            }
            self.attack_cd = (self.attack_cd - dt).max(0.0);
            if input.break_held {
                self.try_attack(0.45);
            }
        }
        // Items in the world: fall, merge, fly to the player.
        if self.ground_ready() && !self.drops.list.is_empty() {
            let centre = self.player.body.pos + wolfcraft_engine::physics::up_at(self.player.body.pos) * 0.8;
            let can = !self.player.survival.dead;
            let inv = &mut self.player.inventory;
            let mut got: Vec<(wolfcraft_engine::items::ItemId, u32)> = Vec::new();
            let n = self.drops.step_log(&self.chunks, centre, can, dt as f64, |s| inv.add(s), |id, k| match got.iter_mut().find(|g| g.0 == id) {
                Some(g) => g.1 += k,
                None => got.push((id, k)),
            });
            self.picked_up += n;
            // "+5 Земля" over the hotbar, like the item name when switching.
            if let Some((id, k)) = got.last() {
                self.item_name = Some((format!("+{k} {}", wolfcraft_engine::items::info(*id).name), 1.6));
            }
        }
        // Arrows: flight, hits, pickup.
        if self.ground_ready() {
            let feet = self.player.body.pos;
            let can_pick = !self.player.survival.dead;
            let ev = self.arrows.step(&self.chunks, &mut self.mobs, feet, can_pick, dt.min(0.1) as f64);
            for kind in ev.killed {
                // Shot from afar: the loot lands at the player's feet (the arrow event has no spot).
                for drop in wolfcraft_engine::items::mob_drops(kind, self.cfg.seed ^ self.clock.to_bits() as u64) {
                    let at = self.player.body.pos + wolfcraft_engine::physics::up_at(self.player.body.pos) * 0.5;
                    self.drops.pop_from_block(drop, at);
                }
                self.player.survival.add_xp(if kind.hostile() { 5 } else { 1 });
            }
            if ev.picked > 0 {
                self.player.inventory.add(wolfcraft_engine::items::ItemStack::new(wolfcraft_engine::items::ids::ARROW, ev.picked as u8));
            }
        }
        // Broken blocks drop their items into the world; broken chests spill their contents.
        for e in &events {
            if let InteractionEvent::Dropped { pos, stack } = e {
                let c = self.chunks.grid().cell_center(*pos);
                self.drops.pop_from_block(*stack, c);
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
        // Redstone-style power: re-evaluate the network around every edit.
        for e in &events {
            if let InteractionEvent::Broken { pos, block } = e {
                wolfcraft_engine::power::piston_broken(&mut self.chunks, *pos, *block);
            }
            if let InteractionEvent::Placed { pos, .. } | InteractionEvent::Broken { pos, .. } = e {
                self.fluids.changed(&self.chunks, *pos);
                wolfcraft_engine::power::update_around(&mut self.chunks, *pos);
                wolfcraft_engine::power::update_iskrit(&mut self.chunks, *pos);
            }
            if let InteractionEvent::Placed { pos, block: wolfcraft_engine::block::Block::ButtonOn } = e {
                self.buttons.retain(|(p, _)| p != pos);
                self.buttons.push((*pos, 1.0));
            }
        }
        self.special_effects(dt);
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
            if self.chunks.get_block(pos) == Some(wolfcraft_engine::block::Block::ButtonOn) {
                self.chunks.set_block(pos, wolfcraft_engine::block::Block::Button);
                wolfcraft_engine::power::update_iskrit(&mut self.chunks, pos);
                self.net_out.push((pos, wolfcraft_engine::block::Block::Button));
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

    /// Slots for a container block, created on first use.
    pub fn container_slots(&mut self, pos: wolfcraft_engine::cubesphere::Cell, block: wolfcraft_engine::block::Block) -> &mut Vec<Option<wolfcraft_engine::items::ItemStack>> {
        let n = if block == wolfcraft_engine::block::Block::ElectricFurnace { 2 } else { 27 };
        self.containers.entry(pos).or_insert_with(|| vec![None; n])
    }

    /// Smelting progress 0..1 and whether the furnace is powered.
    pub fn furnace_status(&self, pos: wolfcraft_engine::cubesphere::Cell) -> (f32, bool) {
        let t = self.furnace_t.get(&pos).copied().unwrap_or(0.0);
        (t / FURNACE_TIME, wolfcraft_engine::power::is_powered(&self.chunks, pos))
    }

    fn tick_furnaces(&mut self, dt: f32) {
        use wolfcraft_engine::interaction::smelt_result;
        use wolfcraft_engine::items::ItemStack;
        let cells: Vec<_> = self.containers.iter().filter(|(_, v)| v.len() == 2 && v[0].is_some()).map(|(c, _)| *c).collect();
        for c in cells {
            let Some(out) = self.containers[&c][0].and_then(|s| smelt_result(s.id)) else {
                self.furnace_t.remove(&c);
                continue;
            };
            let room = match self.containers[&c][1] {
                None => true,
                Some(o) => o.id == out && o.count < 64,
            };
            if !room || !wolfcraft_engine::power::is_powered(&self.chunks, c) {
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
                match v[1].as_mut() {
                    Some(o) => o.count += 1,
                    None => v[1] = Some(ItemStack::new(out, 1)),
                }
            }
        }
    }

    fn dimension_path(&self) -> PathBuf {
        self.dir.join("dimension.txt")
    }

    fn load_dimension(&mut self) {
        let Ok(text) = std::fs::read_to_string(self.dimension_path()) else { return };
        let mut it = text.split_whitespace();
        if it.next() != Some("hell") {
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
        let planet = Arc::new(if hell { Planet::new_hell(&self.cfg) } else { Planet::new(&self.cfg) });
        let dir = if hell { self.dir.join("hell") } else { self.dir.clone() };
        let Ok(storage) = wolfcraft_engine::storage::Storage::open(&dir) else {
            log::error!("cannot open dimension storage {}", dir.display());
            return;
        };
        self.planet = planet.clone();
        self.chunks = ChunkManager::new(planet, Arc::new(storage));
        self.mobs = wolfcraft_engine::mobs::MobManager::new(self.cfg.seed);
        self.hell = hell;
        self.dimension_changed = true;
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
        self.dir.join("containers.txt")
    }

    fn load_containers(&mut self) {
        let Ok(text) = std::fs::read_to_string(self.containers_path()) else { return };
        for line in text.lines() {
            let n: Vec<i64> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if n.len() == 9 {
                let cell = wolfcraft_engine::cubesphere::Cell::new(n[0] as u8, n[1] as i32, n[2] as i32, n[3] as i32);
                let size = n[4] as usize;
                let slot = n[5] as usize;
                let v = self.containers.entry(cell).or_insert_with(|| vec![None; size]);
                if slot < v.len() && wolfcraft_engine::items::is_valid(n[6] as u16) {
                    v[slot] = Some(wolfcraft_engine::items::ItemStack { id: n[6] as u16, count: n[7] as u8, damage: n[8] as u16 });
                }
            }
        }
    }

    fn save_containers(&self) {
        let mut out = String::new();
        for (c, v) in &self.containers {
            for (i, s) in v.iter().enumerate() {
                if let Some(s) = s {
                    out += &format!("{} {} {} {} {} {} {} {} {}\n", c.f, c.i, c.j, c.k, v.len(), i, s.id, s.count, s.damage);
                }
            }
        }
        if let Err(e) = std::fs::write(self.containers_path(), out) {
            log::error!("saving containers.txt: {e}");
        }
    }

    /// Writes player.dat, edited chunks and world.toml.
    pub fn save(&mut self) {
        self.save_containers();
        self.save_block_commands();
        let dim = if self.hell {
            let p = self.overworld_pos.unwrap_or(DVec3::ZERO);
            format!("hell {} {} {}", p.x, p.y, p.z)
        } else {
            "overworld".to_string()
        };
        let _ = std::fs::write(self.dimension_path(), dim);
        if let Err(e) = std::fs::write(self.dir.join("player.dat"), self.player.to_bytes()) {
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
        let up = self.player.up();
        let raw_eye = self.player.eye_pos();
        let eye_r = if self.eye_r > 0.0 { self.eye_r } else { radial(raw_eye) };
        let eye = along(wolfcraft_engine::cubesphere::col_dir(raw_eye), eye_r) + up * self.cam_lag;
        let look = self.player.look_dir();
        match self.camera_mode {
            CameraMode::First => Camera { eye, forward: look, up },
            CameraMode::ThirdBack | CameraMode::ThirdFront => {
                let sign = if self.camera_mode == CameraMode::ThirdBack { -1.0 } else { 1.0 };
                let want = 4.2;
                let dir = look * sign;
                let _ = want;
                let dist = self.tp_dist.min(self.room_along(eye, dir));
                let pos = eye + dir * dist;
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

    /// Particles that bring the world to life: embers and smoke over torches, bubbles and
    /// sparks over lava, glowing motes around uranium, falling leaves, a splash when the
    /// player jumps into water, dust on landing and behind a sprinting player.
    fn special_effects(&mut self, dt: f32) {
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
                            let p = g.cell_center(c);
                            if (p - pos).length() < 16.0 {
                                self.fx_lights.push((p, ch.get(li, lj, lk)));
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
                        self.mobs.fx_glow(p + lup * 0.42 + o, v, col, 0.035, 0.9, FxKind::Rise);
                    } else if r < rate * 3.3 && b == Block::Torch {
                        let v = lup * 0.5 + jitter(&mut self.mobs, 0.15);
                        self.mobs.fx(p + lup * 0.55, v, [0.35, 0.35, 0.37], 0.06, 1.6, FxKind::Rise);
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
            if !matches!(b, Block::OakLeaves | Block::BirchLeaves | Block::SpruceLeaves | Block::AcaciaLeaves) {
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
    fn try_attack(&mut self, cooldown: f32) -> bool {
        if self.player.survival.dead || !self.ground_ready() {
            return false;
        }
        let eye = self.player.eye_pos();
        let Some(look) = self.aim_dir else { return false };
        let reach = Interaction::reach(&self.player) - 1.5;
        let Some((i, t)) = self.mobs.raycast(eye, look, reach) else { return false };
        // A block in front of the creature protects it.
        if self.hit.is_some_and(|h| h.dist <= t) {
            return false;
        }
        if self.attack_cd > 0.0 {
            return true;
        }
        let dmg = self.player.inventory.held().map(|s| s.info().attack).unwrap_or(1.0).max(1.0);
        let at = self.mobs.mobs.get(i).map(|m| m.centre()).unwrap_or(eye + look * t);
        // Hit feedback: a burst of red and white sparks, more for strong weapons.
        let n = ((4.0 + dmg * 1.5) * self.fx_amount) as usize;
        self.mobs.chips(at - look * 0.3, &[[0.95, 0.15, 0.12], [1.0, 1.0, 1.0], [0.75, 0.05, 0.05]], n, 2.2 + dmg as f64 * 0.15);
        if let Some(kind) = self.mobs.hurt(i, dmg, self.player.body.pos) {
            for drop in wolfcraft_engine::items::mob_drops(kind, self.cfg.seed ^ self.clock.to_bits() as u64) {
                self.drops.pop_from_block(drop, at);
            }
            self.player.survival.add_xp(if kind.hostile() { 5 } else { 1 });
        }
        self.attack_cd = cooldown;
        self.mob_hits += 1;
        if let Some(st) = self.player.inventory.held() {
            if st.info().durability > 0 {
                self.player.inventory.wear_selected();
            }
        }
        true
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

    /// Name of the season and the day in it, for the HUD clock.
    pub fn season_label(&self) -> String {
        let (name, day) = wolfcraft_engine::player::season_label(self.player.world_time);
        format!("{name}, день {day}")
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
        signs.push(((8, 0, -1), "Рубильник: нажмите, чтобы включить и выключить"));
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
        signs.push(((28, 0, -1), "Поршни: включите рубильник. Обычный толкает блок, липкий ещё и тянет назад"));
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
        signs.push(((13, 0, 30), "Двойная автодверь: рубильник открывает обе створки, они расходятся в стороны"));
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
        signs.push(((27, 0, 30), "Потайной проход: обойдите стену и выключите рубильник — липкие поршни утянут камни"));
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
            if matches!(self.chunks.get_block(c), Some(B::IskritWire | B::IskritTorch | B::IskritBlock | B::IskritLamp | B::SwitchOn)) || self.chunks.get_block(c).is_some_and(|b| b.part().is_some()) {
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
        smoothstep(-0.12, 0.28, self.player.up().dot(self.sun())) as f32
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
