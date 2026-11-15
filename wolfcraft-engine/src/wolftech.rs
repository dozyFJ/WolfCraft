//! WolfTech — the tech mod built into WolfCraft (ideas from IC2, Thermal Expansion, Mekanism).
//! Part 1: energy and machines. Energy is counted in ЭЕ (энергоединицы). Generators make it,
//! energy cells store it, copper cables carry it, machines spend it to crush, smelt and enrich.

use crate::block::Block;
use crate::items::{ItemId, ids};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
        Kind::Generator => Spec { name: "Генератор", capacity: 2_000.0, rate: 20.0, time: 0.0, hint: "Положите уголь или дрова" },
        Kind::Solar => Spec { name: "Солнечная панель", capacity: 500.0, rate: 8.0, time: 0.0, hint: "Работает днём под открытым небом" },
        Kind::Cell => Spec { name: "Энергоячейка", capacity: 40_000.0, rate: 0.0, time: 0.0, hint: "Хранит энергию, заряжает снаряжение (положите его слева)" },
        Kind::Crusher => Spec { name: "Дробитель", capacity: 200.0, rate: 10.0, time: 4.0, hint: "Руда → 2 пыли, камень → булыжник → гравий → песок" },
        Kind::Induction => Spec { name: "Индукционная печь", capacity: 160.0, rate: 8.0, time: 2.0, hint: "Плавит быстро, на энергии" },
        Kind::Wind => Spec { name: "Ветрогенератор", capacity: 1_000.0, rate: 12.0, time: 0.0, hint: "Чем выше и чем сильнее непогода, тем больше энергии" },
        Kind::Geo => Spec { name: "Геотермальный генератор", capacity: 2_000.0, rate: 8.0, time: 0.0, hint: "Поставьте рядом с лавой: каждая сторона с лавой — +8 ЭЕ/с" },
        Kind::Water => Spec { name: "Гидрогенератор", capacity: 1_000.0, rate: 3.0, time: 0.0, hint: "Поставьте в воду: каждая сторона с водой — +3 ЭЕ/с" },
        Kind::Reactor => Spec { name: "Ядерный реактор", capacity: 10_000.0, rate: 0.0, time: 0.0, hint: "Урановые стержни дают энергию и жар; теплоотводы и капсулы охлаждают. Перегрев — взрыв!" },
        Kind::Enricher => Spec { name: "Обогатитель", capacity: 320.0, rate: 16.0, time: 6.0, hint: "Руда → 3 пыли" },
    }
}

/// Makes energy (generators of every kind).
pub fn is_source(k: Kind) -> bool {
    matches!(k, Kind::Generator | Kind::Solar | Kind::Wind | Kind::Geo | Kind::Water | Kind::Reactor)
}

/// Burns fuel from its slot (the coal generator and the reactor).
pub fn burns(k: Kind) -> bool {
    matches!(k, Kind::Generator | Kind::Reactor)
}

/// Slots in the reactor's core (6 × 3), and its hull heat limit before plating.
pub const REACTOR_SLOTS: usize = 18;
pub const REACTOR_COLS: usize = 6;
pub const REACTOR_MAX_HEAT: f32 = 5_000.0;

/// A part that goes into the reactor core.
pub fn is_reactor_part(id: ItemId) -> bool {
    matches!(id, ids::URANIUM_ROD | ids::HEAT_VENT | ids::COOLANT_CELL | ids::REACTOR_PLATING)
}

/// What one second of the reactor does (IC2-like).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReactorTick {
    /// ЭЕ/s made.
    pub power: f32,
    /// Heat added to the hull this second (after vents and capsules).
    pub heat: f32,
    /// Hull heat limit (more with plating).
    pub max_heat: f32,
    pub rods: u32,
}

/// One second of the reactor: every uranium rod pulses once, plus once more for each rod next
/// to it (left, right, above, below in the 6×3 core) — packed rods give more energy, but heat
/// grows faster than energy. Heat vents carry 6 heat/s off the hull each; coolant capsules
/// soak up to 12 heat/s each into themselves (they fill up: their wear); plating raises the
/// hull's limit by 1 000. Rods and full capsules wear (`slots` is changed).
pub fn reactor_second(slots: &mut [Option<crate::items::ItemStack>], hull: f32) -> ReactorTick {
    let n = slots.len().min(REACTOR_SLOTS);
    let id_at = |s: &[Option<crate::items::ItemStack>], i: usize| s.get(i).copied().flatten().map(|x| x.id);
    let rod = |s: &[Option<crate::items::ItemStack>], i: usize| id_at(s, i) == Some(ids::URANIUM_ROD);
    let mut t = ReactorTick { max_heat: REACTOR_MAX_HEAT, ..Default::default() };
    let mut made_heat = 0.0;
    let mut vents = 0.0;
    let mut capsules = Vec::new();
    for i in 0..n {
        match id_at(slots, i) {
            Some(ids::URANIUM_ROD) => {
                let (x, y) = (i % REACTOR_COLS, i / REACTOR_COLS);
                let mut p = 1;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx >= 0 && ny >= 0 && (nx as usize) < REACTOR_COLS && rod(slots, ny as usize * REACTOR_COLS + nx as usize) {
                        p += 1;
                    }
                }
                t.power += 10.0 * p as f32;
                made_heat += 2.0 * (p * (p + 1)) as f32;
                t.rods += 1;
                if let Some(s) = slots[i].as_mut() {
                    s.damage += 1;
                    if s.damage >= 600 {
                        slots[i] = None;
                    }
                }
            }
            Some(ids::HEAT_VENT) => vents += 6.0,
            Some(ids::COOLANT_CELL) => capsules.push(i),
            Some(ids::REACTOR_PLATING) => t.max_heat += 1_000.0,
            _ => {}
        }
    }
    // Heat on the hull: what the rods made, minus the vents (they also cool a hot hull down).
    let mut excess = made_heat - vents;
    // Capsules take what the vents could not.
    for i in capsules {
        if excess <= 0.0 {
            break;
        }
        if let Some(s) = slots[i].as_mut() {
            let room = 2_000u16.saturating_sub(s.damage) as f32;
            let take = excess.min(12.0).min(room);
            s.damage += take.ceil() as u16;
            excess -= take;
        }
    }
    t.heat = if excess < 0.0 { excess.max(-hull) } else { excess };
    t
}

/// Seconds of work one item of fuel gives this generator.
pub fn fuel_for(k: Kind, id: ItemId) -> f32 {
    match k {
        Kind::Generator => fuel(id),
        Kind::Reactor => 0.0,
        _ => 0.0,
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
    let mut e: Vec<f32> = nodes.iter().map(|n| n.energy).collect();
    // 1. Generators produce into themselves (up to capacity).
    for (i, n) in nodes.iter().enumerate() {
        if is_source(n.kind) {
            e[i] = (e[i] + n.flow * dt).min(n.cap);
        }
    }
    // 2. Pool everything that can be given: generators and cells.
    let givers: Vec<usize> = (0..nodes.len()).filter(|&i| is_source(nodes[i].kind) || nodes[i].kind == Kind::Cell).collect();
    let mut pool: f32 = givers.iter().map(|&i| e[i]).sum();
    for &i in &givers {
        e[i] = 0.0;
    }
    // 3. Machines take what they need for this frame (filling their buffer up to a second's worth).
    for (i, n) in nodes.iter().enumerate() {
        if is_machine(n.kind) {
            // A machine keeps only a few seconds of work in its buffer: the rest of the
            // energy stays in the cells for the others.
            let want = (n.cap.min(n.flow.max(spec(n.kind).rate) * 3.0) - e[i]).max(0.0);
            let got = want.min(pool);
            e[i] += got;
            pool -= got;
        }
    }
    // 4. Machines fill their own small stores, the rest goes to the cells (storage) and
    //    then back into the generators' buffers.
    for (i, n) in nodes.iter().enumerate() {
        if is_machine(n.kind) && pool > 0.0 {
            let put = (n.cap - e[i]).max(0.0).min(pool);
            e[i] += put;
            pool -= put;
        }
    }
    for &i in givers.iter().filter(|&&i| nodes[i].kind == Kind::Cell).chain(givers.iter().filter(|&&i| nodes[i].kind != Kind::Cell)) {
        let room = nodes[i].cap - e[i];
        let put = room.min(pool);
        e[i] += put;
        pool -= put;
    }
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
    (0.7f32.powi(oc), 1.6f32.powi(oc), st.min(64) as f32 * 2_000.0)
}

/// An upgrade item for the machine slots.
pub fn is_upgrade(id: ItemId) -> bool {
    matches!(id, ids::OVERCLOCKER | ids::STORAGE_UPGRADE)
}

/// Fuel value in seconds of burning (same as the furnace).
pub fn fuel(id: ItemId) -> f32 {
    crate::interaction::fuel_time(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_flows_from_a_generator_through_to_a_machine_and_a_cell() {
        let mut nodes = vec![
            Node { kind: Kind::Generator, cap: spec(Kind::Generator).capacity, energy: 0.0, flow: 20.0 },
            Node { kind: Kind::Crusher, cap: spec(Kind::Crusher).capacity, energy: 0.0, flow: 10.0 },
            Node { kind: Kind::Cell, cap: spec(Kind::Cell).capacity, energy: 0.0, flow: 0.0 },
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
        assert!(nodes[1].energy > 5.0, "the machine is fed: {}", nodes[1].energy);
        assert!(nodes[2].energy > 50.0, "the surplus is stored: {}", nodes[2].energy);
        let held: f32 = nodes.iter().map(|n| n.energy).sum();
        assert!(held <= total_made + 1.0, "no energy from nowhere");
    }

    #[test]
    fn overclockers_speed_up_and_cost_more() {
        use crate::items::ItemStack;
        assert_eq!(upgrades(&[]), (1.0, 1.0, 0.0));
        let (t, e, s) = upgrades(&[Some(ItemStack::new(ids::OVERCLOCKER, 2)), Some(ItemStack::new(ids::STORAGE_UPGRADE, 1))]);
        assert!((t - 0.49).abs() < 1e-4 && (e - 2.56).abs() < 1e-4 && s == 2_000.0);
    }

    #[test]
    fn packed_rods_give_more_and_overheat_without_vents() {
        use crate::items::ItemStack;
        let rod = Some(ItemStack::new(ids::URANIUM_ROD, 1));
        let vent = Some(ItemStack::new(ids::HEAT_VENT, 1));
        let mut one = vec![None; REACTOR_SLOTS];
        one[0] = rod;
        one[1] = vent;
        let t = reactor_second(&mut one, 0.0);
        assert_eq!((t.power, t.rods), (10.0, 1));
        assert!(t.heat <= 0.0, "one rod, one vent: stays cool");
        let mut two = vec![None; REACTOR_SLOTS];
        two[0] = rod;
        two[1] = rod;
        let t = reactor_second(&mut two, 0.0);
        assert_eq!(t.power, 40.0, "neighbours pulse each other");
        assert!(t.heat > 20.0, "and heat up fast without vents");
        assert_eq!(two[0].unwrap().damage, 1, "rods wear");
    }

    #[test]
    fn machines_have_recipes() {
        assert_eq!(recipe(Kind::Crusher, ids::RAW_IRON), Some((ids::IRON_DUST, 2)));
        assert_eq!(recipe(Kind::Enricher, ids::RAW_GOLD), Some((ids::GOLD_DUST, 3)));
        assert_eq!(recipe(Kind::Induction, ids::IRON_DUST), Some((ids::IRON_INGOT, 1)));
        assert_eq!(recipe(Kind::Crusher, Block::Cobblestone as ItemId), Some((Block::Gravel as ItemId, 1)));
    }
}
