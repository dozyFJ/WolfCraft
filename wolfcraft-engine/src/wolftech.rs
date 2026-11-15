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
        _ => return None,
    })
}

pub fn spec(k: Kind) -> Spec {
    match k {
        Kind::Generator => Spec { name: "Генератор", capacity: 2_000.0, rate: 20.0, time: 0.0, hint: "Положите уголь или дрова" },
        Kind::Solar => Spec { name: "Солнечная панель", capacity: 500.0, rate: 8.0, time: 0.0, hint: "Работает днём под открытым небом" },
        Kind::Cell => Spec { name: "Энергоячейка", capacity: 40_000.0, rate: 0.0, time: 0.0, hint: "Хранит энергию сети" },
        Kind::Crusher => Spec { name: "Дробитель", capacity: 1_000.0, rate: 10.0, time: 4.0, hint: "Руда → 2 пыли, камень → булыжник → гравий → песок" },
        Kind::Induction => Spec { name: "Индукционная печь", capacity: 1_000.0, rate: 8.0, time: 2.0, hint: "Плавит быстро, на энергии" },
        Kind::Enricher => Spec { name: "Обогатитель", capacity: 2_000.0, rate: 16.0, time: 6.0, hint: "Руда → 3 пыли" },
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
        if matches!(n.kind, Kind::Generator | Kind::Solar) {
            e[i] = (e[i] + n.flow * dt).min(spec(n.kind).capacity);
        }
    }
    // 2. Pool everything that can be given: generators and cells.
    let givers: Vec<usize> = (0..nodes.len()).filter(|&i| matches!(nodes[i].kind, Kind::Generator | Kind::Solar | Kind::Cell)).collect();
    let mut pool: f32 = givers.iter().map(|&i| e[i]).sum();
    for &i in &givers {
        e[i] = 0.0;
    }
    // 3. Machines take what they need for this frame (filling their buffer up to a second's worth).
    for (i, n) in nodes.iter().enumerate() {
        if is_machine(n.kind) {
            // A machine keeps only a few seconds of work in its buffer: the rest of the
            // energy stays in the cells for the others.
            let sp = spec(n.kind);
            let target = sp.capacity.min(sp.rate * 3.0);
            let want = (target - e[i]).max(0.0);
            let got = want.min(pool);
            e[i] += got;
            pool -= got;
        }
    }
    // 4. The rest goes back: first to cells (storage), then to the generators' buffers.
    for &i in givers.iter().filter(|&&i| nodes[i].kind == Kind::Cell).chain(givers.iter().filter(|&&i| nodes[i].kind != Kind::Cell)) {
        let room = spec(nodes[i].kind).capacity - e[i];
        let put = room.min(pool);
        e[i] += put;
        pool -= put;
    }
    e
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
            Node { kind: Kind::Generator, energy: 0.0, flow: 20.0 },
            Node { kind: Kind::Crusher, energy: 0.0, flow: 10.0 },
            Node { kind: Kind::Cell, energy: 0.0, flow: 0.0 },
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
        assert!(nodes[2].energy > 100.0, "the surplus is stored: {}", nodes[2].energy);
        let held: f32 = nodes.iter().map(|n| n.energy).sum();
        assert!(held <= total_made + 1.0, "no energy from nowhere");
    }

    #[test]
    fn machines_have_recipes() {
        assert_eq!(recipe(Kind::Crusher, ids::RAW_IRON), Some((ids::IRON_DUST, 2)));
        assert_eq!(recipe(Kind::Enricher, ids::RAW_GOLD), Some((ids::GOLD_DUST, 3)));
        assert_eq!(recipe(Kind::Induction, ids::IRON_DUST), Some((ids::IRON_INGOT, 1)));
        assert_eq!(recipe(Kind::Crusher, Block::Cobblestone as ItemId), Some((Block::Gravel as ItemId, 1)));
    }
}
