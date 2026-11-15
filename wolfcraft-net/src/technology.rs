//! Server-owned WolfTech state. Player inventories retain the game's existing client-trust
//! model; exchanges validate item conservation and serialize changed machine slots.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use wolfcraft_engine::{
    block::Block,
    chunk::Chunk,
    config::{GameMode, WorldConfig},
    cubesphere::{Cell, ChunkKey},
    items::ItemStack,
    planet::Planet,
    storage::{self, Storage},
    wolftech::{self as wt, MachineState},
};

pub type Pos = [i32; 4];
pub fn cell(p: Pos) -> Cell {
    Cell::new(p[0] as u8, p[1], p[2], p[3])
}
pub fn pos(c: Cell) -> Pos {
    [c.f as i32, c.i, c.j, c.k]
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    pub pos: Pos,
    pub block: u16,
    pub state: MachineState,
    pub slots: Vec<Option<ItemStack>>,
    pub revision: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub world: String,
    pub id: String,
    pub hell: bool,
    pub pos: Pos,
    pub revision: u64,
    pub before: Vec<Option<ItemStack>>,
    pub after: Vec<Option<ItemStack>>,
    pub inventory_before: Vec<Option<ItemStack>>,
    pub inventory_after: Vec<Option<ItemStack>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    pub id: String,
    pub settled: bool,
    pub accepted: bool,
    pub error: String,
    pub inventory: Vec<Option<ItemStack>>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Receipt {
    change: Change,
    reply: Reply,
}
#[derive(Clone, Serialize, Deserialize)]
struct Saved {
    version: u32,
    machines: [Vec<Machine>; 2],
    #[serde(default)]
    known: [Vec<Pos>; 2],
    receipts: BTreeMap<String, Receipt>,
    sequences: BTreeMap<String, u64>,
    time: f64,
    #[serde(default)]
    weather: wolfcraft_engine::weather::Weather,
}
impl Default for Saved {
    fn default() -> Self {
        Self {
            version: 1,
            machines: Default::default(),
            known: Default::default(),
            receipts: Default::default(),
            sequences: Default::default(),
            weather: Default::default(),
            time: 300.0,
        }
    }
}
struct Terrain {
    planet: Planet,
    storage: Storage,
    cache: HashMap<ChunkKey, Option<Chunk>>,
}
impl Terrain {
    fn block(&mut self, p: Pos, edits: &HashMap<Pos, u16>) -> Block {
        if let Some(b) = edits.get(&p) {
            return Block::from_id(*b);
        }
        let c = cell(p);
        let k = ChunkKey::of(c);
        let stored = self
            .cache
            .entry(k)
            .or_insert_with(|| self.storage.load_chunk(k));
        stored.as_ref().map_or_else(
            || self.planet.block_at(c),
            |v| v.get(c.i & 15, c.j & 15, c.k & 15),
        )
    }
}
pub fn world_key(world: &str) -> String {
    let cfg = WorldConfig::from_toml(world).unwrap_or_else(|_| WorldConfig::new("Сервер", 1));
    serde_json::to_string(&(cfg.id, cfg.seed, cfg.size, cfg.shape, cfg.world_type)).unwrap()
}

pub struct Runtime {
    world: String,
    saved: Saved,
    terrain: [Terrain; 2],
    path: PathBuf,
    creative: bool,
    save_in: f32,
    nets: [Vec<Vec<Cell>>; 2],
    rebuild_in: f32,
    clock: wt::Clock,
}
fn valid(stack: Option<ItemStack>) -> bool {
    stack.is_none_or(|s| {
        wolfcraft_engine::items::is_valid(s.id)
            && s.count > 0
            && s.count <= s.info().max_stack
            && s.damage <= s.info().durability
    })
}
fn totals(a: &[Option<ItemStack>], b: &[Option<ItemStack>]) -> BTreeMap<(u16, u16, u32), u32> {
    let mut result = BTreeMap::new();
    for s in a.iter().chain(b).flatten() {
        *result.entry((s.id, s.damage, s.ench)).or_default() += s.count as u32;
    }
    result
}
fn id_parts(id: &str) -> Option<(&str, u64)> {
    let (c, s) = id.split_once(':')?;
    if c.len() != 32 || !c.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some((c, s.parse().ok()?))
}
impl Runtime {
    pub fn new(dir: &Path, world: &str, edits: &[HashMap<Pos, u16>; 2]) -> Result<Self, String> {
        let cfg = WorldConfig::from_toml(world).unwrap_or_else(|_| WorldConfig::new("Сервер", 1));
        let path = dir.join("wolftech-server.json");
        let loaded = storage::read_with_backup(&path, |b| {
            serde_json::from_slice::<Saved>(b)
                .ok()
                .filter(|s| s.version == 1)
        });
        if path.exists() && loaded.is_none() {
            return Err("WolfTech: повреждён файл состояния и резервная копия".into());
        }
        let saved = loaded.unwrap_or_else(|| Saved {
            weather: wolfcraft_engine::weather::Weather::new(cfg.seed),
            ..Default::default()
        });
        let mut rt = Self {
            world: world_key(world),
            saved,
            terrain: [
                Terrain {
                    planet: Planet::new(&cfg),
                    storage: Storage::open(dir)?,
                    cache: HashMap::new(),
                },
                Terrain {
                    planet: Planet::new_hell(&cfg),
                    storage: Storage::open(&dir.join("hell"))?,
                    cache: HashMap::new(),
                },
            ],
            path,
            creative: cfg.mode == GameMode::Creative,
            save_in: 1.0,
            nets: Default::default(),
            rebuild_in: 0.0,
            clock: Default::default(),
        };
        // Import pre-network saves once; old files remain as a migration backup.
        let offline = dir.join("wolftech-offline.flag");
        if !rt.path.exists() || offline.exists() {
            rt.saved.machines = Default::default();
            if let Some(time) = std::fs::read(dir.join("player.dat"))
                .ok()
                .and_then(|b| wolfcraft_engine::player::Player::from_bytes(&b))
                .map(|p| p.world_time)
            {
                rt.saved.time = time;
            }
            for hell in [false, true] {
                let ix = hell as usize;
                let file = dir.join(if hell { "tech_hell.txt" } else { "tech.txt" });
                if let Some(text) =
                    storage::read_with_backup(&file, |b| String::from_utf8(b.to_vec()).ok())
                {
                    for line in text.lines() {
                        let n: Vec<f64> = line
                            .split_whitespace()
                            .filter_map(|s| s.parse().ok())
                            .collect();
                        if n.len() >= 7 && n.iter().all(|x| x.is_finite()) {
                            let p = [n[0] as i32, n[1] as i32, n[2] as i32, n[3] as i32];
                            if rt.ensure(p, hell, edits) {
                                let m = rt.saved.machines[ix]
                                    .iter_mut()
                                    .find(|m| m.pos == p)
                                    .unwrap();
                                m.state.energy = n[4].max(0.0) as f32;
                                m.state.progress = n[5].max(0.0) as f32;
                                m.state.burn = n[6].max(0.0) as f32;
                                m.state.charge_fraction =
                                    n.get(7).copied().unwrap_or(0.0).max(0.0) as f32;
                            }
                        }
                    }
                }
                let file = dir.join(if hell {
                    "containers_hell.txt"
                } else {
                    "containers.txt"
                });
                if let Some(text) =
                    storage::read_with_backup(&file, |b| String::from_utf8(b.to_vec()).ok())
                {
                    for line in text.lines() {
                        let n: Vec<i64> = line
                            .split_whitespace()
                            .filter_map(|s| s.parse().ok())
                            .collect();
                        if n.len() >= 9 {
                            let p = [n[0] as i32, n[1] as i32, n[2] as i32, n[3] as i32];
                            if rt.ensure(p, hell, edits) {
                                let m = rt.saved.machines[ix]
                                    .iter_mut()
                                    .find(|m| m.pos == p)
                                    .unwrap();
                                let i = n[5] as usize;
                                let st = ItemStack {
                                    id: n[6] as u16,
                                    count: n[7] as u8,
                                    damage: n[8] as u16,
                                    ench: n.get(9).copied().unwrap_or(0) as u32,
                                };
                                if i < m.slots.len() && valid(Some(st)) {
                                    m.slots[i] = Some(st);
                                }
                            }
                        }
                    }
                }
                for p in edits[ix].keys().copied() {
                    rt.ensure(p, hell, edits);
                }
            }
            rt.save()?;
            if offline.exists() {
                std::fs::remove_file(&offline).map_err(|e| e.to_string())?;
            }
        }
        Ok(rt)
    }
    pub fn grid(&self, hell: bool) -> wolfcraft_engine::cubesphere::Grid {
        self.terrain[hell as usize].planet.grid
    }
    pub fn ensure(&mut self, p: Pos, hell: bool, edits: &[HashMap<Pos, u16>; 2]) -> bool {
        let ix = hell as usize;
        let g = self.grid(hell);
        if p[0] < 0
            || p[0] > 5
            || p[1] < 0
            || p[2] < 0
            || p[1] >= g.n
            || p[2] >= g.n
            || p[3] < 0
            || p[3] > 100000
        {
            return false;
        }
        let b = self.terrain[ix].block(p, &edits[ix]);
        if let Some(k) = wt::kind(b) {
            if !self.saved.known[ix].contains(&p) {
                self.saved.known[ix].push(p);
            }
            if let Some(m) = self.saved.machines[ix].iter_mut().find(|m| m.pos == p) {
                m.block = b as u16;
            } else {
                self.rebuild_in = 0.0;
                self.saved.machines[ix].push(Machine {
                    pos: p,
                    block: b as u16,
                    state: Default::default(),
                    slots: vec![None; wt::slot_count(k)],
                    revision: 0,
                });
            }
            true
        } else {
            if self.saved.machines[ix].iter().any(|m| m.pos == p) {
                self.rebuild_in = 0.0;
            }
            self.saved.machines[ix].retain(|m| m.pos != p);
            false
        }
    }
    pub fn time(&self) -> f64 {
        self.saved.time
    }
    pub fn climate(&self) -> (wolfcraft_engine::weather::Sky, f32, f32) {
        (
            self.saved.weather.sky,
            self.saved.weather.rain,
            self.saved.weather.timer,
        )
    }
    pub fn snapshot(&self, hell: bool) -> Vec<Machine> {
        self.saved.machines[hell as usize].clone()
    }
    pub fn save(&mut self) -> Result<(), String> {
        let bytes = serde_json::to_vec(&self.saved).map_err(|e| e.to_string())?;
        storage::write_safe(&self.path, &bytes)
    }
    /// Export only machine rows; unrelated chests remain intact. Original side files serve
    /// as a backwards-compatible handoff to single-player after a hosted game ends.
    pub fn export_singleplayer(&mut self) -> Result<(), String> {
        let dir = self.path.parent().unwrap();
        for ix in 0..2 {
            let tech = dir.join(if ix == 1 { "tech_hell.txt" } else { "tech.txt" });
            let containers = dir.join(if ix == 1 {
                "containers_hell.txt"
            } else {
                "containers.txt"
            });
            let old =
                storage::read_with_backup(&containers, |b| String::from_utf8(b.to_vec()).ok())
                    .unwrap_or_default();
            let mut rows: String = old
                .lines()
                .filter(|line| {
                    let v: Vec<i32> = line
                        .split_whitespace()
                        .take(4)
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    v.len() != 4 || !self.saved.known[ix].contains(&[v[0], v[1], v[2], v[3]])
                })
                .map(|s| format!("{s}\n"))
                .collect();
            let mut states = String::new();
            for m in &self.saved.machines[ix] {
                let p = m.pos;
                let st = m.state;
                states += &format!(
                    "{} {} {} {} {} {} {} {}\n",
                    p[0], p[1], p[2], p[3], st.energy, st.progress, st.burn, st.charge_fraction
                );
                for (i, s) in m.slots.iter().enumerate() {
                    if let Some(s) = s {
                        rows += &format!(
                            "{} {} {} {} {} {} {} {} {} {}\n",
                            p[0],
                            p[1],
                            p[2],
                            p[3],
                            m.slots.len(),
                            i,
                            s.id,
                            s.count,
                            s.damage,
                            s.ench
                        );
                    }
                }
            }
            storage::write_safe(&tech, states.as_bytes())?;
            storage::write_safe(&containers, rows.as_bytes())?;
        }
        Ok(())
    }
    pub fn invalidate(&mut self) {
        self.rebuild_in = 0.0;
    }
    pub fn tick(&mut self, dt: f32, edits: &[HashMap<Pos, u16>; 2]) {
        for _ in 0..self.clock.advance(dt) {
            self.tick_step(wt::STEP, edits);
        }
    }
    fn tick_step(&mut self, dt: f32, edits: &[HashMap<Pos, u16>; 2]) {
        self.saved.time += dt as f64;
        self.saved.weather.step(dt);
        self.rebuild_in -= dt;
        let rebuild = self.rebuild_in <= 0.0;
        for ix in 0..2 {
            let grid = self.terrain[ix].planet.grid;
            if rebuild {
                for p in edits[ix].keys().copied() {
                    if wt::kind(Block::from_id(edits[ix][&p])).is_some() {
                        self.ensure(p, ix == 1, edits);
                    }
                }
                let positions: Vec<_> = self.saved.machines[ix].iter().map(|m| m.pos).collect();
                for p in positions {
                    self.ensure(p, ix == 1, edits);
                }
                let seeds: Vec<_> = self.saved.machines[ix]
                    .iter()
                    .map(|m| cell(m.pos))
                    .collect();
                // Graph reads only explicit saved/edited conductor blocks. Natural terrain cannot conduct.
                self.nets[ix] = wt::networks(seeds, grid, |c| {
                    Some(self.terrain[ix].block(pos(c), &edits[ix]))
                });
            }
            for m in &mut self.saved.machines[ix] {
                let k = wt::kind(Block::from_id(m.block)).unwrap();
                if rebuild {
                    let sea = self.terrain[ix].planet.radius - 1.0;
                    let rain = self.saved.weather.rain;
                    m.state.env = wt::environment(k, cell(m.pos), grid, sea, rain, |c| {
                        Some(self.terrain[ix].block(pos(c), &edits[ix]))
                    });
                }
                if k == wt::Kind::Solar && rebuild {
                    let c = cell(m.pos);
                    let up = grid.up(grid.cell_center(c));
                    m.state.day = ix == 0
                        && self.saved.weather.rain < 0.5
                        && up.dot(wolfcraft_engine::player::local_sun(self.saved.time, up)) > 0.02;
                    m.state.sunny = m.state.day
                        && (1..40).all(|h| {
                            grid.offset(c, 0, 0, h).is_none_or(|n| {
                                !self.terrain[ix].block(pos(n), &edits[ix]).is_opaque()
                            })
                        });
                }
                let before = m.slots.clone();
                wt::advance(k, &mut m.state, &mut m.slots, dt, self.creative);
                if before != m.slots {
                    m.revision = m.revision.wrapping_add(1);
                }
            }
            let indices: HashMap<_, _> = self.saved.machines[ix]
                .iter()
                .enumerate()
                .map(|(i, m)| (m.pos, i))
                .collect();
            for net in &self.nets[ix] {
                let ids: Vec<_> = net
                    .iter()
                    .filter_map(|c| indices.get(&pos(*c)).copied())
                    .collect();
                let nodes: Vec<_> = ids
                    .iter()
                    .map(|&i| {
                        let m = &self.saved.machines[ix][i];
                        let k = wt::kind(Block::from_id(m.block)).unwrap();
                        wt::Node {
                            kind: k,
                            cap: wt::spec(k).capacity
                                + wt::upgrades(m.slots.get(3..).unwrap_or(&[])).2,
                            energy: m.state.energy,
                            flow: m.state.flow,
                        }
                    })
                    .collect();
                for (&i, e) in ids.iter().zip(wt::share(&nodes, dt)) {
                    self.saved.machines[ix][i].state.energy = e;
                }
            }
        }
        if rebuild {
            self.rebuild_in = 1.0;
        }
        self.save_in -= dt;
        if self.save_in <= 0.0 {
            self.save_in = 5.0;
            if let Err(e) = self.save() {
                log::error!("WolfTech: {e}");
            }
        }
    }
    pub fn recover(&mut self, c: &Change) -> Reply {
        if c.world != self.world {
            return Reply {
                id: c.id.clone(),
                settled: false,
                accepted: false,
                error: "Подключитесь к прежнему миру сервера для восстановления операции".into(),
                inventory: c.inventory_before.clone(),
            };
        }

        if let Some(r) = self.saved.receipts.get(&c.id).filter(|r| r.change == *c) {
            return r.reply.clone();
        }
        let mut reply = Reply {
            settled: false,
            id: c.id.clone(),
            accepted: false,
            error: "Операция не применялась".into(),
            inventory: c.inventory_before.clone(),
        };
        let Some((client, seq)) = id_parts(&c.id).map(|(s, n)| (s.to_owned(), n)) else {
            return reply;
        };
        if seq <= *self.saved.sequences.get(&client).unwrap_or(&0) {
            reply.error = "Операция уже подтверждена, проверьте сохранение игрока".into();
            return reply;
        }
        if c.inventory_before.len() != 36
            || c.inventory_after.len() != 36
            || c.before.len() > 7
            || c.after.len() > 7
            || self.saved.receipts.len() >= 4096
        {
            return reply;
        }
        let old = self.saved.clone();
        reply.settled = true;
        self.saved.sequences.insert(client, seq);
        self.saved.receipts.insert(
            c.id.clone(),
            Receipt {
                change: c.clone(),
                reply: reply.clone(),
            },
        );
        if let Err(e) = self.save() {
            self.saved = old;
            reply.settled = false;
            reply.error = e;
        }
        reply
    }
    pub fn confirm(&mut self, id: &str) {
        let old = self.saved.receipts.remove(id);
        if old.is_some() {
            if self.save().is_err() {
                self.saved.receipts.insert(id.into(), old.unwrap());
            }
        }
    }
    pub fn change(&mut self, c: Change, edits: &[HashMap<Pos, u16>; 2]) -> Reply {
        if c.world != self.world {
            return Reply {
                id: c.id,
                settled: false,
                accepted: false,
                error: "Другой мир сервера".into(),
                inventory: c.inventory_before,
            };
        }

        if let Some(r) = self.saved.receipts.get(&c.id) {
            return if r.change == c {
                r.reply.clone()
            } else {
                Reply {
                    settled: true,
                    id: c.id,
                    accepted: false,
                    error: "Повторный номер операции".into(),
                    inventory: c.inventory_before,
                }
            };
        }
        let mut reply = Reply {
            settled: true,
            id: c.id.clone(),
            accepted: false,
            error: "Состояние машины изменилось — повторите действие".into(),
            inventory: c.inventory_before.clone(),
        };
        let Some((client, seq)) = id_parts(&c.id).map(|(s, n)| (s.to_owned(), n)) else {
            return reply;
        };
        if seq <= *self.saved.sequences.get(&client).unwrap_or(&0)
            || c.inventory_before.len() != 36
            || c.inventory_after.len() != 36
            || !c
                .inventory_before
                .iter()
                .chain(&c.inventory_after)
                .chain(&c.before)
                .chain(&c.after)
                .all(|s| valid(*s))
        {
            return reply;
        }
        if !self.ensure(c.pos, c.hell, edits) {
            return reply;
        }
        let ix = c.hell as usize;
        let i = self.saved.machines[ix]
            .iter()
            .position(|m| m.pos == c.pos)
            .unwrap();
        let original = self.saved.clone();
        let m = &mut self.saved.machines[ix][i];
        let k = wt::kind(Block::from_id(m.block)).unwrap();
        let mut valid_change = c.before.len() == m.slots.len()
            && c.after.len() == m.slots.len()
            && c.revision <= m.revision
            && totals(&c.before, &c.inventory_before) == totals(&c.after, &c.inventory_after);
        if valid_change {
            for slot in 0..m.slots.len() {
                if c.before[slot] == c.after[slot] {
                    continue;
                }
                if m.slots[slot] != c.before[slot] {
                    valid_change = false;
                    break;
                }
                let removal = match (c.before[slot], c.after[slot]) {
                    (_, None) => true,
                    (Some(a), Some(b)) => {
                        a.id == b.id
                            && a.damage == b.damage
                            && a.ench == b.ench
                            && b.count <= a.count
                    }
                    _ => false,
                };
                if !removal && !c.after[slot].is_some_and(|s| wt::accepts(k, slot, s)) {
                    valid_change = false;
                    break;
                }
            }
        }
        if valid_change {
            for slot in 0..m.slots.len() {
                if c.before[slot] != c.after[slot] {
                    m.slots[slot] = c.after[slot];
                }
            }
            m.revision = m.revision.wrapping_add(1);
            reply.accepted = true;
            reply.error.clear();
            reply.inventory = c.inventory_after.clone();
        }
        let before_inventory = c.inventory_before.clone();
        self.saved.sequences.insert(client, seq);
        self.saved.receipts.insert(
            c.id.clone(),
            Receipt {
                change: c,
                reply: reply.clone(),
            },
        );
        if let Err(e) = self.save() {
            self.saved = original;
            reply.accepted = false;
            reply.settled = false;
            reply.inventory = before_inventory;
            reply.error = format!("Не удалось сохранить операцию: {e}");
        }
        reply
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wolfcraft_engine::{config::PlanetSize, items::ids};
    fn fixture(name: &str) -> (Runtime, [HashMap<Pos, u16>; 2], PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("wolftech_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = WorldConfig::new("Тест", 1);
        cfg.size = PlanetSize::Small;
        let world = cfg.to_toml().unwrap();
        let mut edits = [HashMap::new(), HashMap::new()];
        edits[0].insert([0, 10, 10, 700], Block::EnergyCell as u16);
        (
            Runtime::new(&dir, &world, &edits).unwrap(),
            edits,
            dir,
            world,
        )
    }
    fn put() -> Change {
        let gear = Some(ItemStack::new(ids::JETPACK, 1));
        let mut inv = vec![None; 36];
        inv[0] = gear;
        Change {
            world: {
                let mut cfg = WorldConfig::new("Тест", 1);
                cfg.size = PlanetSize::Small;
                world_key(&cfg.to_toml().unwrap())
            },
            id: format!("{}:1", "1".repeat(32)),
            hell: false,
            pos: [0, 10, 10, 700],
            revision: 0,
            before: vec![None; 3],
            after: vec![gear, None, None],
            inventory_before: inv,
            inventory_after: vec![None; 36],
        }
    }
    #[test]
    fn exchanges_survive_restart_and_retries_do_not_duplicate_items() {
        let (mut rt, edits, dir, world) = fixture("receipts");
        let c = put();
        let accepted = rt.change(c.clone(), &edits);
        assert!(accepted.accepted && accepted.settled);
        assert_eq!(rt.change(c.clone(), &edits), accepted);
        drop(rt);
        let mut rt = Runtime::new(&dir, &world, &edits).unwrap();
        assert_eq!(rt.recover(&c), accepted);
        assert_eq!(rt.snapshot(false)[0].slots[0], c.after[0]);
        rt.confirm(&c.id);
        assert!(!rt.change(c, &edits).accepted);
    }
    #[test]
    fn recovery_cancels_an_unreceived_operation_before_a_late_packet_can_apply_it() {
        let (mut rt, edits, _, _) = fixture("cancel");
        let c = put();
        let cancelled = rt.recover(&c);
        assert!(cancelled.settled && !cancelled.accepted);
        assert_eq!(rt.change(c, &edits), cancelled);
        assert!(rt.snapshot(false)[0].slots[0].is_none());
    }
    #[test]
    fn simultaneous_withdrawals_and_forged_output_are_rejected() {
        let (mut rt, edits, _, _) = fixture("conflict");
        let c = put();
        assert!(rt.change(c.clone(), &edits).accepted);
        let gear = c.after[0];
        let mut inv = vec![None; 36];
        inv[0] = gear;
        let take = Change {
            id: format!("{}:2", "1".repeat(32)),
            before: c.after.clone(),
            after: vec![None; 3],
            inventory_before: vec![None; 36],
            inventory_after: inv,
            revision: 1,
            ..c
        };
        assert!(rt.change(take.clone(), &edits).accepted);
        let other = Change {
            id: format!("{}:1", "2".repeat(32)),
            ..take
        };
        assert!(!rt.change(other, &edits).accepted);
        let mut forged = put();
        forged.id = format!("{}:1", "3".repeat(32));
        forged.inventory_before = vec![None; 36];
        assert!(!rt.change(forged, &edits).accepted);
    }
    #[test]
    fn persistence_failure_never_acknowledges_or_applies_an_exchange() {
        let (mut rt, edits, dir, _) = fixture("disk_error");
        std::fs::write(dir.join("blocked"), "x").unwrap();
        rt.path = dir.join("blocked/state.json");
        let reply = rt.change(put(), &edits);
        assert!(!reply.accepted && !reply.settled);
        assert!(rt.snapshot(false)[0].slots[0].is_none());
    }
    #[test]
    fn machines_work_without_players_and_reload_energy_and_output() {
        let (mut rt, mut edits, dir, world) = fixture("offline");
        let p = [0, 11, 10, 700];
        edits[0].insert(p, Block::InductionFurnace as u16);
        rt.ensure(p, false, &edits);
        let m = rt.saved.machines[0]
            .iter_mut()
            .find(|m| m.pos == p)
            .unwrap();
        m.state.energy = 100.0;
        m.slots[0] = Some(ItemStack::new(ids::RAW_IRON, 1));
        for _ in 0..50 {
            rt.tick(0.05, &edits);
        }
        rt.save().unwrap();
        let rt = Runtime::new(&dir, &world, &edits).unwrap();
        let m = rt.snapshot(false).into_iter().find(|m| m.pos == p).unwrap();
        assert_eq!(m.slots[2], Some(ItemStack::new(ids::IRON_INGOT, 1)));
        assert!(m.state.energy < 100.0);
    }
}

#[cfg(test)]
mod handoff_tests {
    use super::*;
    use wolfcraft_engine::{config::PlanetSize, items::ids};
    #[test]
    fn server_and_singleplayer_handoff_preserves_machine_state_and_other_chests() {
        let dir = std::env::temp_dir().join(format!("wolftech_handoff_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = WorldConfig::new("Мир", 2);
        cfg.size = PlanetSize::Small;
        let world = cfg.to_toml().unwrap();
        let p = [0, 10, 10, 700];
        let mut edits = [HashMap::new(), HashMap::new()];
        edits[0].insert(p, Block::EnergyCell as u16);
        let mut rt = Runtime::new(&dir, &world, &edits).unwrap();
        rt.saved.machines[0][0].state.energy = 123.0;
        rt.saved.machines[0][0].slots[0] = Some(ItemStack::new(ids::JETPACK, 1));
        std::fs::write(dir.join("containers.txt"), "0 20 20 700 27 0 1 3 0 0\n").unwrap();
        rt.save().unwrap();
        rt.export_singleplayer().unwrap();
        let text = std::fs::read_to_string(dir.join("containers.txt")).unwrap();
        assert!(text.contains("0 20 20 700"));
        assert!(text.contains("0 10 10 700"));
        std::fs::write(dir.join("tech.txt"), "0 10 10 700 321 0 0 0\n").unwrap();
        std::fs::write(dir.join("wolftech-offline.flag"), "offline").unwrap();
        let rt = Runtime::new(&dir, &world, &edits).unwrap();
        let m = &rt.saved.machines[0][0];
        assert_eq!(m.state.energy, 321.0);
        assert_eq!(m.slots[0], Some(ItemStack::new(ids::JETPACK, 1)));
        assert!(!dir.join("wolftech-offline.flag").exists());
    }
}
