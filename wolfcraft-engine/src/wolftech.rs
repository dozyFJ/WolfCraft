//! WolfTech — the tech mod built into WolfCraft (ideas from IC2, Thermal Expansion, Mekanism).
//! Part 1: energy and machines. Energy is counted in ЭЕ (энергоединицы). Generators make it,
//! energy cells store it, copper cables carry it, machines spend it to crush, smelt and enrich.

use crate::block::Block;
use crate::items::{ItemId, ItemStack, ids};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Kind {
    /// Burns fuel from its slot.
    Generator,
    /// Makes energy in daylight under the open sky.
    Solar,
    /// Stores energy for the network.
    Cell,
    /// Ore → 2 dust; stone → cobble → gravel → sand.
    Crusher,
    /// Smelts like a furnace, faster, on energy.
    Induction,
    /// Ore → 3 dust (the best yield).
    Enricher,
    /// Spins in the wind: more up high under the open sky, much more in a storm.
    Wind,
    /// Draws heat from lava around it (each lava side adds).
    Geo,
    /// Turned by water around it (each water side adds).
    Water,
    /// Splits uranium: a lot of energy for a long time.
    Reactor,
}

pub struct Spec {
    pub name: &'static str,
    /// Energy it can hold.
    pub capacity: f32,
    /// ЭЕ/s made (generators) or spent while working (machines).
    pub rate: f32,
    /// Seconds per operation (machines).
    pub time: f32,
    pub hint: &'static str,
}

pub fn kind(b: Block) -> Option<Kind> {
    let b = b.machine_dir().map_or(b, |(base, _)| base);
    Some(match b {
        Block::TechGenerator => Kind::Generator,
        Block::SolarPanel => Kind::Solar,
        Block::EnergyCell => Kind::Cell,
        Block::Crusher => Kind::Crusher,
        Block::InductionFurnace => Kind::Induction,
        Block::Enricher => Kind::Enricher,
        Block::WindTurbine => Kind::Wind,
        Block::GeoGenerator => Kind::Geo,
        Block::WaterMill => Kind::Water,
        Block::Reactor => Kind::Reactor,
        _ => return None,
    })
}

pub fn spec(k: Kind) -> Spec {
    match k {
        Kind::Generator => Spec {
            name: "Генератор",
            capacity: 2_000.0,
            rate: 20.0,
            time: 0.0,
            hint: "Положите уголь или дрова",
        },
        Kind::Solar => Spec {
            name: "Солнечная панель",
            capacity: 500.0,
            rate: 8.0,
            time: 0.0,
            hint: "Работает днём под открытым небом",
        },
        Kind::Cell => Spec {
            name: "Энергоячейка",
            capacity: 40_000.0,
            rate: 0.0,
            time: 0.0,
            hint: "Хранит энергию, заряжает снаряжение (положите его слева)",
        },
        Kind::Crusher => Spec {
            name: "Дробитель",
            capacity: 200.0,
            rate: 10.0,
            time: 4.0,
            hint: "Руда → 2 пыли, камень → булыжник → гравий → песок",
        },
        Kind::Induction => Spec {
            name: "Индукционная печь",
            capacity: 160.0,
            rate: 8.0,
            time: 2.0,
            hint: "Плавит быстро, на энергии",
        },
        Kind::Wind => Spec {
            name: "Ветрогенератор",
            capacity: 1_000.0,
            rate: 12.0,
            time: 0.0,
            hint: "Чем выше и чем сильнее непогода, тем больше энергии",
        },
        Kind::Geo => Spec {
            name: "Геотермальный генератор",
            capacity: 2_000.0,
            rate: 8.0,
            time: 0.0,
            hint: "Поставьте рядом с лавой: каждая сторона с лавой — +8 ЭЕ/с",
        },
        Kind::Water => Spec {
            name: "Гидрогенератор",
            capacity: 1_000.0,
            rate: 3.0,
            time: 0.0,
            hint: "Поставьте в воду: каждая сторона с водой — +3 ЭЕ/с",
        },
        Kind::Reactor => Spec {
            name: "Ядерный реактор",
            capacity: 10_000.0,
            rate: 120.0,
            time: 0.0,
            hint: "Положите урановый слиток (10 минут) или сырой уран",
        },
        Kind::Enricher => Spec {
            name: "Обогатитель",
            capacity: 320.0,
            rate: 16.0,
            time: 6.0,
            hint: "Руда → 3 пыли",
        },
    }
}

pub fn is_machine(k: Kind) -> bool {
    matches!(k, Kind::Crusher | Kind::Induction | Kind::Enricher)
}

/// Blocks that carry energy between machines: cables and the machines themselves.
pub fn conducts(b: Block) -> bool {
    b == Block::Cable || kind(b).is_some()
}

fn raw_dust(id: ItemId) -> Option<ItemId> {
    Some(match id {
        ids::RAW_IRON => ids::IRON_DUST,
        ids::RAW_GOLD => ids::GOLD_DUST,
        ids::RAW_COPPER => ids::COPPER_DUST,
        ids::RAW_TIN => ids::TIN_DUST,
        ids::RAW_SILVER => ids::SILVER_DUST,
        x if x == Block::IronOre as ItemId => ids::IRON_DUST,
        x if x == Block::GoldOre as ItemId => ids::GOLD_DUST,
        x if x == Block::CopperOre as ItemId => ids::COPPER_DUST,
        x if x == Block::TinOre as ItemId => ids::TIN_DUST,
        x if x == Block::SilverOre as ItemId => ids::SILVER_DUST,
        _ => return None,
    })
}

/// What a machine makes from one `input` item: (item, count).
pub fn recipe(k: Kind, input: ItemId) -> Option<(ItemId, u8)> {
    match k {
        Kind::Crusher => {
            if let Some(d) = raw_dust(input) {
                return Some((d, 2));
            }
            let b = |x: Block| x as ItemId;
            Some(match input {
                x if x == b(Block::Stone) => (b(Block::Cobblestone), 1),
                x if x == b(Block::Cobblestone) => (b(Block::Gravel), 1),
                x if x == b(Block::Gravel) => (b(Block::Sand), 1),
                x if x == b(Block::CoalOre) => (ids::COAL, 3),
                _ => return None,
            })
        }
        Kind::Enricher => raw_dust(input).map(|d| (d, 3)),
        Kind::Induction => crate::interaction::smelt_result(input).map(|o| (o, 1)),
        _ => None,
    }
}

/// One block of the machine network for `share` (index-aligned).
#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub kind: Kind,
    /// Energy store of this node (the spec's, plus «Накопитель» upgrades).
    pub cap: f32,
    pub energy: f32,
    /// Generators: ЭЕ/s it makes now. Machines: ЭЕ/s it wants now (0 when idle).
    pub flow: f32,
}

/// Moves energy inside one connected network for `dt` seconds: generators fill their own
/// buffer, then everything flows from those with energy to machines that work and on into
/// energy cells. Returns the new energy of each node.
pub fn share(nodes: &[Node], dt: f32) -> Vec<f32> {
    let mut e: Vec<f32> = nodes
        .iter()
        .map(|n| {
            if n.energy.is_finite() {
                n.energy.clamp(0.0, n.cap.max(0.0))
            } else {
                0.0
            }
        })
        .collect();
    for (i, n) in nodes.iter().enumerate() {
        if is_source(n.kind) {
            e[i] = (e[i] + n.flow.max(0.0) * dt.max(0.0)).min(n.cap);
        }
    }
    let givers: Vec<_> = (0..nodes.len())
        .filter(|&i| (is_source(nodes[i].kind) || nodes[i].kind == Kind::Cell))
        .collect();
    let mut pool: f32 = givers.iter().map(|&i| e[i]).sum();
    for &i in &givers {
        e[i] = 0.0;
    }
    // Fair active demand first. Idle machines never drain a factory merely to prefill.
    let active: Vec<_> = (0..nodes.len())
        .filter(|&i| is_machine(nodes[i].kind) && nodes[i].flow > 0.0)
        .collect();
    let mut distribute = |indices: &[usize], short: bool| {
        let wants: Vec<f32> = indices
            .iter()
            .map(|&i| {
                let cap = if short {
                    nodes[i].cap.min(nodes[i].flow * 3.0)
                } else {
                    nodes[i].cap
                };
                (cap - e[i]).max(0.0)
            })
            .collect();
        let total: f32 = wants.iter().sum();
        if total > 0.0 && pool > 0.0 {
            let used = pool.min(total);
            let ratio = used / total;
            for (&i, w) in indices.iter().zip(wants) {
                e[i] += w * ratio;
            }
            pool = (pool - used).max(0.0);
        }
    };
    distribute(&active, true);
    let cells: Vec<_> = givers
        .iter()
        .copied()
        .filter(|&i| nodes[i].kind == Kind::Cell)
        .collect();
    distribute(&cells, false);
    distribute(&active, false);
    let sources: Vec<_> = givers
        .iter()
        .copied()
        .filter(|&i| nodes[i].kind != Kind::Cell)
        .collect();
    distribute(&sources, false);
    e
}

/// IC2-style upgrades in a machine's right-hand slots: (time ×, energy use ×, extra store).
/// Each «Ускоритель» works 30 % faster and uses 60 % more power; each «Накопитель» adds
/// 2 000 ЭЕ of store.
pub fn upgrades(slots: &[Option<crate::items::ItemStack>]) -> (f32, f32, f32) {
    let mut oc = 0u32;
    let mut st = 0u32;
    for s in slots.iter().flatten() {
        match s.id {
            ids::OVERCLOCKER => oc += s.count as u32,
            ids::STORAGE_UPGRADE => st += s.count as u32,
            _ => {}
        }
    }
    let oc = oc.min(16) as i32;
    (
        0.7f32.powi(oc),
        1.6f32.powi(oc),
        st.min(64) as f32 * 2_000.0,
    )
}

/// An upgrade item for the machine slots.
pub fn is_upgrade(id: ItemId) -> bool {
    matches!(id, ids::OVERCLOCKER | ids::STORAGE_UPGRADE)
}

/// Fuel value in seconds of burning (same as the furnace).
pub fn fuel(id: ItemId) -> f32 {
    crate::interaction::fuel_time(id)
}

/// Shared simulation state: single-player and the server run exactly the same machine rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MachineState {
    pub energy: f32,
    pub progress: f32,
    pub burn: f32,
    pub flow: f32,
    pub consumption: f32,
    pub env: f32,
    pub working: bool,
    pub sunny: bool,
    pub day: bool,
    pub charge_fraction: f32,
}

pub const STEP: f32 = 0.05;
/// Rendering only contributes elapsed time. All energy networks run at 20 Hz.
#[derive(Default, Debug)]
pub struct Clock {
    remainder: f64,
}
impl Clock {
    pub fn advance(&mut self, dt: f32) -> usize {
        if !dt.is_finite() || dt <= 0.0 {
            return 0;
        }
        self.remainder += dt as f64;
        let steps = ((self.remainder + 1e-7) / 0.05).floor().min(100.0) as usize;
        self.remainder = (self.remainder - steps as f64 * 0.05).max(0.0);
        steps
    }
}

pub fn slot_count(k: Kind) -> usize {
    if is_machine(k) { 7 } else { 3 }
}
pub fn accepts(k: Kind, slot: usize, stack: ItemStack) -> bool {
    match k {
        Kind::Generator | Kind::Reactor => slot == 1 && fuel_for(k, stack.id) > 0.0,
        Kind::Cell => slot == 0 && crate::items::rechargeable(stack.id),
        Kind::Solar | Kind::Wind | Kind::Geo | Kind::Water => false,
        _ => {
            (slot == 0 && recipe(k, stack.id).is_some())
                || ((3..7).contains(&slot) && is_upgrade(stack.id))
        }
    }
}

/// Advance one machine. Fractions and operation remainders survive any rendering FPS.
pub fn advance(
    k: Kind,
    st: &mut MachineState,
    slots: &mut Vec<Option<ItemStack>>,
    dt: f32,
    creative: bool,
) {
    slots.resize(slot_count(k), None);
    st.working = false;
    st.flow = 0.0;
    st.consumption = 0.0;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }
    let sp = spec(k);
    match k {
        Kind::Wind | Kind::Geo | Kind::Water => {
            st.flow = sp.rate * st.env;
            st.working = st.flow > 0.0;
        }
        Kind::Generator | Kind::Reactor => {
            let mut left = dt;
            let mut producing = 0.0;
            while left > 0.000001 {
                if st.burn <= 0.0 {
                    if st.energy >= sp.capacity * 0.95 {
                        break;
                    }
                    if creative {
                        st.burn = left;
                    } else if let Some(f) = slots[1].filter(|s| fuel_for(k, s.id) > 0.0) {
                        st.burn = fuel_for(k, f.id);
                        slots[1] = if f.count > 1 {
                            Some(ItemStack {
                                count: f.count - 1,
                                ..f
                            })
                        } else {
                            None
                        };
                    } else {
                        break;
                    }
                }
                let used = left.min(st.burn);
                st.burn -= used;
                left -= used;
                producing += used;
            }
            st.flow = sp.rate * producing / dt;
            st.working = producing > 0.0;
        }
        Kind::Solar => {
            st.flow = if st.sunny {
                sp.rate
            } else if st.day {
                sp.rate * 0.25
            } else {
                0.0
            };
            st.working = st.flow > 0.0;
        }
        Kind::Cell => {
            if let Some(g) = slots[0]
                .as_mut()
                .filter(|g| crate::items::rechargeable(g.id) && g.damage > 0)
            {
                st.charge_fraction += 40.0 * dt;
                let pts = (st.charge_fraction.floor() as u16)
                    .min((st.energy / 5.0).floor() as u16)
                    .min(g.damage);
                st.charge_fraction -= pts as f32;
                // Do not bank unlimited charging work while the cell has no power.
                st.charge_fraction = st.charge_fraction.fract();
                g.damage -= pts;
                st.energy -= pts as f32 * 5.0;
                st.consumption = pts as f32 * 5.0 / dt;
                st.working = pts > 0;
            } else {
                st.charge_fraction = 0.0;
            }
        }
        _ => {
            let (tm, em, _) = upgrades(&slots[3..]);
            let time = sp.time * tm;
            let rate = sp.rate * em;
            let Some(input) = slots[0] else {
                st.progress = 0.0;
                return;
            };
            let Some((item, count)) = recipe(k, input.id) else {
                st.progress = 0.0;
                return;
            };
            let room = match slots[2] {
                None => crate::items::info(item).max_stack as usize,
                Some(s) if s.id == item => s.info().max_stack.saturating_sub(s.count) as usize,
                _ => 0,
            };
            let max_ops = (room / count as usize).min(input.count as usize);
            if max_ops == 0 {
                st.progress = 0.0;
                return;
            }
            st.flow = rate;
            let work = dt
                .min(st.energy.max(0.0) / rate)
                .min((time * max_ops as f32 - st.progress).max(0.0));
            st.consumption = rate * work / dt;
            st.energy = (st.energy - rate * work).max(0.0);
            st.progress += work;
            st.working = work > 0.0;
            let done = ((st.progress / time + 0.00001).floor() as usize).min(max_ops);
            if done > 0 {
                st.progress = (st.progress - time * done as f32).max(0.0);
                let remain = input.count - done as u8;
                slots[0] = if remain > 0 {
                    Some(ItemStack {
                        count: remain,
                        ..input
                    })
                } else {
                    None
                };
                let n = count as usize * done;
                slots[2] = Some(ItemStack::new(
                    item,
                    slots[2].map_or(0, |s| s.count) + n as u8,
                ));
                if remain == 0 {
                    st.progress = 0.0;
                }
            }
        }
    }
}

/// Components of the loaded conductor graph; a large component never truncates other ones.
pub fn networks(
    seeds: impl IntoIterator<Item = crate::cubesphere::Cell>,
    grid: crate::cubesphere::Grid,
    mut block: impl FnMut(crate::cubesphere::Cell) -> Option<Block>,
) -> Vec<Vec<crate::cubesphere::Cell>> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let mut seeds: Vec<_> = seeds.into_iter().collect();
    seeds.sort_by_key(|c| (c.f, c.i, c.j, c.k));
    for c in seeds {
        if !seen.insert(c) {
            continue;
        }
        let mut net = Vec::new();
        let mut queue = vec![c];
        while let Some(x) = queue.pop() {
            let Some(b) = block(x).filter(|b| conducts(*b)) else {
                continue;
            };
            if kind(b).is_some() {
                net.push(x);
            }
            for (i, j, k) in [
                (1, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ] {
                if let Some(n) = grid.offset(x, i, j, k) {
                    if !seen.contains(&n) && block(n).is_some_and(conducts) {
                        seen.insert(n);
                        queue.push(n);
                    }
                }
            }
        }
        if !net.is_empty() {
            out.push(net);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_flows_from_a_generator_through_to_a_machine_and_a_cell() {
        let mut nodes = vec![
            Node {
                kind: Kind::Generator,
                cap: spec(Kind::Generator).capacity,
                energy: 0.0,
                flow: 20.0,
            },
            Node {
                kind: Kind::Crusher,
                cap: spec(Kind::Crusher).capacity,
                energy: 0.0,
                flow: 10.0,
            },
            Node {
                kind: Kind::Cell,
                cap: spec(Kind::Cell).capacity,
                energy: 0.0,
                flow: 0.0,
            },
        ];
        let mut total_made = 0.0;
        for _ in 0..600 {
            let e = share(&nodes, 0.05);
            for (n, v) in nodes.iter_mut().zip(e) {
                n.energy = v;
            }
            total_made += 20.0 * 0.05;
            // The crusher works: spends its rate.
            nodes[1].energy = (nodes[1].energy - 10.0 * 0.05).max(0.0);
        }
        assert!(
            nodes[1].energy > 5.0,
            "the machine is fed: {}",
            nodes[1].energy
        );
        assert!(
            nodes[2].energy > 50.0,
            "the surplus is stored: {}",
            nodes[2].energy
        );
        let held: f32 = nodes.iter().map(|n| n.energy).sum();
        assert!(held <= total_made + 1.0, "no energy from nowhere");
    }

    #[test]
    fn overclockers_speed_up_and_cost_more() {
        use crate::items::ItemStack;
        assert_eq!(upgrades(&[]), (1.0, 1.0, 0.0));
        let (t, e, s) = upgrades(&[
            Some(ItemStack::new(ids::OVERCLOCKER, 2)),
            Some(ItemStack::new(ids::STORAGE_UPGRADE, 1)),
        ]);
        assert!((t - 0.49).abs() < 1e-4 && (e - 2.56).abs() < 1e-4 && s == 2_000.0);
    }

    #[test]
    fn machines_have_recipes() {
        assert_eq!(
            recipe(Kind::Crusher, ids::RAW_IRON),
            Some((ids::IRON_DUST, 2))
        );
        assert_eq!(
            recipe(Kind::Enricher, ids::RAW_GOLD),
            Some((ids::GOLD_DUST, 3))
        );
        assert_eq!(
            recipe(Kind::Induction, ids::IRON_DUST),
            Some((ids::IRON_INGOT, 1))
        );
        assert_eq!(
            recipe(Kind::Crusher, Block::Cobblestone as ItemId),
            Some((Block::Gravel as ItemId, 1))
        );
    }

    #[test]
    fn idle_machine_never_starves_active_work() {
        let nodes = [
            Node {
                kind: Kind::Cell,
                cap: 40000.0,
                energy: 20.0,
                flow: 0.0,
            },
            Node {
                kind: Kind::Crusher,
                cap: 200.0,
                energy: 0.0,
                flow: 0.0,
            },
            Node {
                kind: Kind::Crusher,
                cap: 200.0,
                energy: 0.0,
                flow: 10.0,
            },
        ];
        let e = share(&nodes, 1.0 / 60.0);
        assert_eq!(e[1], 0.0);
        assert_eq!(e[2], 20.0);
        let nodes = [
            nodes[0],
            nodes[2],
            Node {
                flow: 10.0,
                ..nodes[2]
            },
        ];
        let e = share(&nodes, 0.05);
        assert!((e[1] - e[2]).abs() < 0.001);
    }
    #[test]
    fn charging_is_independent_of_frame_rate() {
        for fps in [20, 30, 60, 120, 240] {
            let mut state = MachineState {
                energy: 40000.0,
                ..Default::default()
            };
            let mut gear = ItemStack::new(ids::JETPACK, 1);
            gear.damage = 400;
            let mut slots = vec![Some(gear), None, None];
            for _ in 0..fps * 5 {
                advance(Kind::Cell, &mut state, &mut slots, 1.0 / fps as f32, false);
            }
            let used = 400 - slots[0].unwrap().damage;
            assert!((used as i32 - 200).abs() <= 1, "{fps}: {used}");
            assert!((40000.0 - state.energy - used as f32 * 5.0).abs() < 0.01);
        }
    }
    #[test]
    fn overclocked_machine_keeps_remainder_and_finishes_multiple_operations() {
        let mut state = MachineState {
            energy: 8000.0,
            ..Default::default()
        };
        let mut slots = vec![
            Some(ItemStack::new(ids::RAW_IRON, 64)),
            None,
            None,
            Some(ItemStack::new(ids::OVERCLOCKER, 8)),
            None,
            None,
            None,
        ];
        advance(Kind::Crusher, &mut state, &mut slots, 1.0, false);
        assert_eq!(slots[2].unwrap().count, 8);
        assert!(state.progress > 0.0);
        assert!((8000.0 - state.energy - 10.0 * 1.6f32.powi(8)).abs() < 0.1);
    }
    #[test]
    fn a_large_component_does_not_break_the_next_independent_network() {
        use crate::cubesphere::{Cell, Grid};
        let grid = Grid::new_cube(4096.0);
        let mut blocks = std::collections::HashMap::new();
        let a = Cell::new(0, 10, 10, 200);
        let b = Cell::new(0, 4511, 10, 200);
        for i in 11..4511 {
            blocks.insert(Cell::new(0, i, 10, 200), Block::Cable);
        }
        let c = Cell::new(0, 6000, 10, 200);
        let d = Cell::new(0, 6001, 10, 200);
        for p in [a, b, c, d] {
            blocks.insert(p, Block::Crusher);
        }
        let nets = networks([a, b, c, d], grid, |p| blocks.get(&p).copied());
        assert_eq!(nets.len(), 2);
        assert!(nets.iter().all(|v| v.len() == 2));
    }

    #[test]
    fn network_tacts_and_total_energy_are_identical_at_different_render_fps() {
        let mut totals = Vec::new();
        for fps in [1, 5, 20, 30, 60, 120, 240] {
            let mut clock = Clock::default();
            let mut e = 0.0;
            let mut ticks = 0;
            for _ in 0..fps * 10 {
                for _ in 0..clock.advance(1.0 / fps as f32) {
                    ticks += 1;
                    e += 20.0 * STEP;
                }
            }
            assert_eq!(ticks, 200, "{fps} FPS");
            totals.push(e);
        }
        assert!(totals.iter().all(|v| *v == totals[0]));
    }
}

pub fn is_source(k: Kind) -> bool {
    matches!(
        k,
        Kind::Generator | Kind::Solar | Kind::Wind | Kind::Geo | Kind::Water | Kind::Reactor
    )
}

pub fn burns(k: Kind) -> bool {
    matches!(k, Kind::Generator | Kind::Reactor)
}

pub fn fuel_for(k: Kind, id: ItemId) -> f32 {
    match k {
        Kind::Generator => fuel(id),
        Kind::Reactor => match id {
            ids::URANIUM_INGOT => 600.0,
            ids::RAW_URANIUM => 200.0,
            _ => 0.0,
        },
        _ => 0.0,
    }
}

/// Ambient sources are measured once per network refresh, independently of render frames.
pub fn environment(
    k: Kind,
    c: crate::cubesphere::Cell,
    g: crate::cubesphere::Grid,
    sea: f64,
    rain: f32,
    mut block: impl FnMut(crate::cubesphere::Cell) -> Option<Block>,
) -> f32 {
    match k {
        Kind::Wind => {
            let open = (1..20).all(|h| {
                g.offset(c, 0, 0, h)
                    .and_then(&mut block)
                    .is_none_or(|b| !b.is_opaque())
            });
            let altitude = (g.radial(g.cell_center(c)) - sea).max(0.0) as f32;
            if open {
                (0.25 + (altitude / 40.0).min(1.0) * 0.75) * (1.0 + 1.5 * rain)
            } else {
                0.1
            }
        }
        Kind::Geo | Kind::Water => [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ]
        .into_iter()
        .filter(|(i, j, h)| {
            g.offset(c, *i, *j, *h)
                .and_then(&mut block)
                .is_some_and(|b| {
                    if k == Kind::Geo {
                        b.is_lava()
                    } else {
                        b.is_water()
                    }
                })
        })
        .count() as f32,
        _ => 0.0,
    }
}

#[cfg(test)]
mod ambient_regressions {
    use super::*;
    use crate::cubesphere::{Cell, Grid};
    #[test]
    fn all_new_sources_feed_the_same_network_and_reactor_consumes_uranium() {
        for kind in [Kind::Wind, Kind::Geo, Kind::Water, Kind::Reactor] {
            let mut state = MachineState {
                env: 2.0,
                ..Default::default()
            };
            let mut slots = vec![None; 3];
            if kind == Kind::Reactor {
                slots[1] = Some(ItemStack::new(ids::URANIUM_INGOT, 1));
            }
            advance(kind, &mut state, &mut slots, STEP, false);
            assert!(state.flow > 0.0);
            let e = share(
                &[
                    Node {
                        kind,
                        cap: spec(kind).capacity,
                        energy: 0.0,
                        flow: state.flow,
                    },
                    Node {
                        kind: Kind::Cell,
                        cap: 40000.0,
                        energy: 0.0,
                        flow: 0.0,
                    },
                ],
                STEP,
            );
            assert!((e[1] - state.flow * STEP).abs() < 0.01);
            if kind == Kind::Reactor {
                assert!(slots[1].is_none());
                assert!((state.burn - (600.0 - STEP)).abs() < 0.01);
            }
        }
    }
    #[test]
    fn wind_water_and_lava_are_measured_from_the_environment() {
        let grid = Grid::new_cube(512.0);
        let c = Cell::new(0, 100, 100, 600);
        let mut blocks = std::collections::HashMap::new();
        blocks.insert(grid.offset(c, 1, 0, 0).unwrap(), Block::Lava);
        blocks.insert(grid.offset(c, -1, 0, 0).unwrap(), Block::Water);
        assert_eq!(
            environment(Kind::Geo, c, grid, 511.0, 0.0, |n| blocks.get(&n).copied()),
            1.0
        );
        assert_eq!(
            environment(Kind::Water, c, grid, 511.0, 0.0, |n| blocks
                .get(&n)
                .copied()),
            1.0
        );
        let clear = environment(Kind::Wind, c, grid, 511.0, 0.0, |_| None);
        let storm = environment(Kind::Wind, c, grid, 511.0, 1.0, |_| None);
        assert!(storm > clear);
    }
}
