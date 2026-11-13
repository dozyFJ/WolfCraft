//! A running game: planet + chunk streaming + player + interaction.

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
    /// Lit dynamite and grenades / rockets in flight.
    pub explosives: wolfcraft_engine::explosives::Explosives,
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
}

impl GameSession {
    pub fn new(l: Loaded) -> Self {
        let chunks = ChunkManager::new(l.planet.clone(), l.storage.clone());
        let seed = l.cfg.seed;
        let mut s = Self {
            cfg: l.cfg,
            dir: l.dir,
            planet: l.planet,
            chunks,
            player: l.player,
            interaction: Interaction::default(),
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
            explosives: wolfcraft_engine::explosives::Explosives::new(seed),
            attack_cd: 0.0,
            death_burst_done: false,
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
        };
        s.load_containers();
        s.load_block_commands();
        s.load_dimension();
        s.player.align_heading();
        s.prev_radius = s.player.body.pos.length();
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
            self.remote_edits.retain(|(c, b)| {
                // Keep the edit until its chunk exists; an already equal block needs nothing.
                match chunks.get_block(*c) {
                    Some(cur) if cur == *b => false,
                    Some(_) => !chunks.set_block(*c, *b),
                    None => true,
                }
            });
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
        let main_use = self.player.inventory.held().is_some_and(|s| {
            let i = s.info();
            i.place.is_some() || i.food.is_some() || wolfcraft_engine::explosives::weapon(s.id).is_some() || s.id == wolfcraft_engine::items::ids::LIGHTER
        });
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
        let r = self.player.body.pos.length();
        let dr = r - self.prev_radius;
        self.prev_radius = r;
        if dr > 0.3 && dr < 2.0 && self.player.body.on_ground {
            self.cam_lag -= dr;
        }
        self.cam_lag = (self.cam_lag * (-16.0 * dt as f64).exp()).clamp(-1.6, 0.6);
        // Eye height: follow small changes smoothly, jump on big ones (teleport, falling).
        let er = self.player.eye_pos().length();
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
        self.hit = raycast(&self.chunks, eye, self.player.look_dir(), reach);
        if !self.player.survival.dead {
            let hit = self.hit;
            events.extend(self.interaction.update_break(&mut self.chunks, &mut self.player, hit, input.break_held, dt, self.cfg.seed));
            if input.use_pressed {
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
            if input.break_held && self.attack_cd <= 0.0 {
                let eye = self.player.eye_pos();
                let look = self.player.look_dir();
                let reach = Interaction::reach(&self.player) - 1.5;
                if let Some((i, t)) = self.mobs.raycast(eye, look, reach) {
                    // A block in front of the creature protects it.
                    if self.hit.is_none_or(|h| h.dist > t) {
                        let dmg = self.player.inventory.held().map(|s| s.info().attack).unwrap_or(1.0).max(1.0);
                        if let Some(kind) = self.mobs.hurt(i, dmg, self.player.body.pos) {
                            for drop in wolfcraft_engine::items::mob_drops(kind, self.cfg.seed ^ self.clock.to_bits() as u64) {
                                self.player.inventory.add(drop);
                            }
                            self.player.survival.add_xp(if kind.hostile() { 5 } else { 1 });
                        }
                        self.attack_cd = 0.45;
                    }
                }
            }
        }
        // Explosives: light dynamite, fire weapons, then fuses, flight and blasts.
        self.explosives_step(&mut events, dt);
        // Containers: broken chests spill into the inventory; furnaces smelt while powered.
        for e in &events {
            if let InteractionEvent::Broken { pos, .. } = e {
                if let Some(slots) = self.containers.remove(pos) {
                    for s in slots.into_iter().flatten() {
                        self.player.inventory.add(s);
                    }
                }
                self.furnace_t.remove(pos);
            }
        }
        self.tick_furnaces(dt);
        // Redstone-style power: re-evaluate the network around every edit.
        for e in &events {
            match e {
                InteractionEvent::Placed { pos, .. } | InteractionEvent::Broken { pos, .. } => {
                    wolfcraft_engine::power::update_around(&mut self.chunks, *pos);
                }
                // A crater can cut a cable: re-check only blocks that are part of a network.
                InteractionEvent::Blasted { pos, block } if is_power_block(*block) => {
                    wolfcraft_engine::power::update_around(&mut self.chunks, *pos);
                }
                _ => {}
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
    fn explosives_step(&mut self, events: &mut Vec<InteractionEvent>, dt: f32) {
        use wolfcraft_engine::explosives::{DYNAMITE_FUSE, blast_damage, weapon};
        let creative = self.player.is_creative();
        let eye = self.player.eye_pos();
        let look = self.player.look_dir();
        let mut keep = Vec::with_capacity(events.len());
        for e in events.drain(..) {
            match e {
                InteractionEvent::Ignite { pos } => {
                    if self.explosives.light(&mut self.chunks, pos, DYNAMITE_FUSE) {
                        keep.push(e);
                    }
                }
                InteractionEvent::Fire { weapon: id } => {
                    let Some((ammo, kind, speed, cooldown)) = weapon(id) else { continue };
                    if !self.explosives.ready() {
                        continue;
                    }
                    let has_ammo = creative || self.player.inventory.count(ammo) > 0;
                    if !has_ammo {
                        self.item_name = Some((format!("Нет боеприпасов: {}", wolfcraft_engine::items::info(ammo).name), 1.5));
                        continue;
                    }
                    if !creative {
                        self.player.inventory.take(ammo, 1);
                    }
                    self.explosives.fire(kind, eye, look, speed, cooldown);
                    keep.push(e);
                }
                other => keep.push(other),
            }
        }
        *events = keep;

        let targets: Vec<glam::DVec3> = self.mobs.mobs.iter().map(|m| m.centre()).collect();
        let (blasts, broken) = self.explosives.update(&mut self.chunks, &targets, dt);
        // Smoke behind rockets.
        let rockets: Vec<glam::DVec3> = self.explosives.rockets().collect();
        for p in rockets {
            self.mobs.chips(p, &[[0.55, 0.55, 0.55], [0.75, 0.75, 0.72]], 1, 0.4);
        }
        for (pos, block) in broken {
            // Chests and furnaces caught in a blast lose their contents.
            self.containers.remove(&pos);
            self.furnace_t.remove(&pos);
            events.push(InteractionEvent::Blasted { pos, block });
        }
        for b in blasts {
            events.push(InteractionEvent::Exploded { at: b.pos, power: b.power });
            self.mobs.chips(b.pos, &[[1.0, 0.78, 0.25], [0.95, 0.45, 0.12], [0.35, 0.33, 0.32], [0.20, 0.19, 0.19]], 40, 9.0);
            // The player: damage through armor, and a push away from the blast.
            if !self.player.survival.dead {
                let centre = self.player.body.pos + self.player.up() * 0.9;
                let dist = (centre - b.pos).length();
                let dmg = blast_damage(b.power, dist);
                if dmg > 0.0 {
                    let armor = self.player.armor_points();
                    self.player.damage(dmg * (1.0 - (armor as f32 * 0.04).min(0.8)), self.cfg.difficulty);
                    let push = (centre - b.pos).normalize_or_zero() * (dmg as f64 * 0.8).min(14.0);
                    self.player.body.vel += push;
                    self.player.body.on_ground = false;
                }
            }
            for i in 0..self.mobs.mobs.len() {
                if self.mobs.mobs[i].hp <= 0.0 {
                    continue;
                }
                let dist = (self.mobs.mobs[i].centre() - b.pos).length();
                let dmg = blast_damage(b.power, dist);
                if dmg > 0.0 && self.mobs.hurt(i, dmg, b.pos).is_some() {
                    self.player.survival.add_xp(1);
                }
            }
        }
    }

    /// Slots for a container block, created on first use.    /// Slots for a container block, created on first use.
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
        self.explosives = wolfcraft_engine::explosives::Explosives::new(self.cfg.seed);
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
        let d = if to_hell { here.normalize_or_zero() } else { self.overworld_pos.unwrap_or(here).normalize_or_zero() };
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
        let pos = pos.unwrap_or_else(|| d * (self.planet.radius + 6.0));
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
        let eye_r = if self.eye_r > 0.0 { self.eye_r } else { raw_eye.length() };
        let eye = raw_eye.normalize_or_zero() * eye_r + up * self.cam_lag;
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

    pub fn cycle_camera(&mut self) {
        self.camera_mode = self.camera_mode.next();
    }

    pub fn teleport(&mut self, pos: DVec3) {
        self.player.body.pos = pos;
        self.player.body.vel = DVec3::ZERO;
        self.player.align_heading();
        self.prev_radius = pos.length();
        self.cam_lag = 0.0;
    }

    pub fn is_creative(&self) -> bool {
        self.cfg.mode == GameMode::Creative
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
        self.chunks.block_at_world(self.player.eye_pos()) == wolfcraft_engine::block::Block::Water
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
            assert!(t.elapsed().as_secs() < 90, "chunks never finished");
            std::thread::sleep(Duration::from_millis(2));
        }
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

/// Blocks that belong to an electric network.
fn is_power_block(b: wolfcraft_engine::block::Block) -> bool {
    use wolfcraft_engine::block::Block;
    matches!(b, Block::Cable | Block::Generator | Block::Battery | Block::ElectricLamp | Block::ElectricLampOn | Block::ElectricFurnace | Block::SwitchOff | Block::SwitchOn)
}
