//! Redstone-style power network.
//!
//! * `Generator` is a power source.
//! * `Cable` and `Battery` conduct power between face-adjacent blocks.
//! * `ElectricLamp` / `ElectricLampOn` are consumers: lit while touching a powered conductor
//!   or generator.
//!
//! The network is event-driven: call [`update_around`] after a block was placed or broken.

use crate::block::Block;
use crate::interaction::WorldEdit;
use crate::cubesphere::Cell;
use std::collections::{HashMap, HashSet, VecDeque};

/// Upper bound on the cells explored for one network (keeps edits cheap).
const MAX_NODES: usize = 4096;

fn conducts(b: Block) -> bool {
    matches!(b, Block::Cable | Block::Battery | Block::Generator | Block::SwitchOn)
}

fn is_lamp(b: Block) -> bool {
    matches!(b, Block::ElectricLamp | Block::ElectricLampOn)
}

fn neighbors(world: &impl WorldEdit, c: Cell) -> Vec<Cell> {
    let g = world.grid();
    (0..3).flat_map(|axis| [-1, 1].into_iter().filter_map(move |s| g.neighbor(c, axis, s))).collect()
}

/// Floods the conductor network containing `start`. Returns (powered, visited cells).
fn flood(world: &impl WorldEdit, start: Cell) -> (bool, Vec<Cell>) {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::new();
    let mut powered = false;
    seen.insert(start);
    queue.push_back(start);
    while let Some(c) = queue.pop_front() {
        if world.cell_block(c) == Block::Generator {
            powered = true;
        }
        if seen.len() >= MAX_NODES {
            break;
        }
        for n in neighbors(world, c) {
            if !seen.contains(&n) && conducts(world.cell_block(n)) {
                seen.insert(n);
                queue.push_back(n);
            }
        }
    }
    (powered, seen.into_iter().collect())
}

/// Recomputes power for everything touching `origin` (the changed cell) and lights or
/// darkens the lamps. Returns how many blocks were toggled.
pub fn update_around(world: &mut impl WorldEdit, origin: Cell) -> usize {
    // Conductor cells to examine: the origin and its neighbours.
    let mut starts: Vec<Cell> = vec![origin];
    starts.extend(neighbors(world, origin));
    let mut powered_of: HashMap<Cell, bool> = HashMap::new();
    let mut lamps: HashSet<Cell> = HashSet::new();

    for s in starts {
        match Some(world.cell_block(s)) {
            Some(b) if is_lamp(b) => {
                lamps.insert(s);
            }
            Some(b) if conducts(b) => {
                if powered_of.contains_key(&s) {
                    continue;
                }
                let (powered, cells) = flood(world, s);
                for c in &cells {
                    powered_of.insert(*c, powered);
                    // Lamps hanging on this network may need to change too.
                    for n in neighbors(world, *c) {
                        if is_lamp(world.cell_block(n)) {
                            lamps.insert(n);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut toggled = 0;
    for lamp in lamps {
        let mut on = false;
        let around = neighbors(world, lamp);
        for n in around {
            if !conducts(world.cell_block(n)) {
                continue;
            }
            let p = match powered_of.get(&n) {
                Some(p) => *p,
                None => {
                    let (p, cells) = flood(world, n);
                    for c in cells {
                        powered_of.insert(c, p);
                    }
                    p
                }
            };
            if p {
                on = true;
                break;
            }
        }
        let want = if on { Block::ElectricLampOn } else { Block::ElectricLamp };
        if world.cell_block(lamp) != want {
            world.set_block(lamp, want);
            toggled += 1;
        }
    }
    toggled
}

/// True when a conductor next to `c` belongs to a powered network (used by machines).
pub fn is_powered(world: &impl WorldEdit, c: Cell) -> bool {
    neighbors(world, c).into_iter().any(|n| conducts(world.cell_block(n)) && flood(world, n).0)
}
// ---------------------------------------------------------------------------------------------
// Iskrit: a signal network like redstone. Sources (iskrit block, iskrit torch, a switch that is
// on, a pressed button) power the iskrit dust next to them with strength 15; the signal drops
// by one per dust block, so a line reaches 15 blocks. Iskrit lamps light up next to a source or
// a charged dust block. Dust also climbs one block up or down, like a staircase.

/// Signal strength at a source.
pub const ISKRIT_MAX: u8 = 15;

fn is_iskrit_source(b: Block) -> bool {
    matches!(b, Block::IskritBlock | Block::IskritTorch | Block::SwitchOn | Block::ButtonOn)
}

fn is_wire(b: Block) -> bool {
    matches!(b, Block::IskritWire | Block::IskritWireOn)
}

fn is_iskrit_lamp(b: Block) -> bool {
    matches!(b, Block::IskritLamp | Block::IskritLampOn)
}

/// Dust neighbours: the six faces plus one step up or down beside it.
fn wire_links(world: &impl WorldEdit, c: Cell) -> Vec<Cell> {
    let g = world.grid();
    let mut v = neighbors(world, c);
    for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        for dk in [-1, 1] {
            if let Some(n) = g.offset(c, di, dj, dk) {
                v.push(n);
            }
        }
    }
    v
}

/// Recomputes the iskrit network around `origin` (an edited cell): charges or discharges the
/// dust and switches the lamps. Returns how many blocks changed.
pub fn update_iskrit(world: &mut impl WorldEdit, origin: Cell) -> usize {
    // Every dust block connected to the edit.
    let mut wires: HashSet<Cell> = HashSet::new();
    let mut queue: VecDeque<Cell> = VecDeque::new();
    let mut around = vec![origin];
    around.extend(wire_links(world, origin));
    for c in around {
        if is_wire(world.cell_block(c)) && wires.insert(c) {
            queue.push_back(c);
        }
    }
    while let Some(c) = queue.pop_front() {
        if wires.len() >= MAX_NODES {
            break;
        }
        for n in wire_links(world, c) {
            if !wires.contains(&n) && is_wire(world.cell_block(n)) {
                wires.insert(n);
                queue.push_back(n);
            }
        }
    }
    // Signal strength: from the sources touching the dust, minus one per step.
    let mut strength: HashMap<Cell, u8> = HashMap::new();
    let mut bfs: VecDeque<(Cell, u8)> = VecDeque::new();
    for w in &wires {
        if neighbors(world, *w).into_iter().any(|n| is_iskrit_source(world.cell_block(n))) {
            strength.insert(*w, ISKRIT_MAX);
            bfs.push_back((*w, ISKRIT_MAX));
        }
    }
    while let Some((c, s)) = bfs.pop_front() {
        if s <= 1 {
            continue;
        }
        for n in wire_links(world, c) {
            if wires.contains(&n) && strength.get(&n).copied().unwrap_or(0) < s - 1 {
                strength.insert(n, s - 1);
                bfs.push_back((n, s - 1));
            }
        }
    }
    let mut changed = 0;
    for w in &wires {
        let want = if strength.get(w).copied().unwrap_or(0) > 0 { Block::IskritWireOn } else { Block::IskritWire };
        if world.cell_block(*w) != want {
            world.set_block(*w, want);
            changed += 1;
        }
    }
    // Lamps next to the edit or the dust.
    let mut lamps: HashSet<Cell> = HashSet::new();
    let mut probe = vec![origin];
    probe.extend(neighbors(world, origin));
    for w in &wires {
        probe.extend(neighbors(world, *w));
    }
    for c in probe {
        if is_iskrit_lamp(world.cell_block(c)) {
            lamps.insert(c);
        }
    }
    for l in lamps {
        let on = neighbors(world, l).into_iter().any(|n| {
            let b = world.cell_block(n);
            is_iskrit_source(b) || b == Block::IskritWireOn
        });
        let want = if on { Block::IskritLampOn } else { Block::IskritLamp };
        if world.cell_block(l) != want {
            world.set_block(l, want);
            changed += 1;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cubesphere::Grid;
    use crate::physics::Voxels;

    struct Mock {
        grid: Grid,
        blocks: HashMap<Cell, Block>,
    }
    impl Voxels for Mock {
        fn grid(&self) -> &Grid {
            &self.grid
        }
        fn cell_block(&self, c: Cell) -> Block {
            *self.blocks.get(&c).unwrap_or(&Block::Air)
        }
    }
    impl WorldEdit for Mock {
        fn set_block(&mut self, c: Cell, b: Block) -> bool {
            self.blocks.insert(c, b);
            true
        }
    }

    #[test]
    fn iskrit_line_lights_a_lamp_within_15_blocks() {
        let mut w = Mock { grid: Grid::with_cells(64), blocks: HashMap::new() };
        let at = |i: i32| Cell::new(4, 10 + i, 20, 30);
        // Torch, then dust from 1 to `len`, then a lamp.
        let mut run = |len: i32| -> bool {
            w.blocks.clear();
            w.set_block(at(0), Block::IskritTorch);
            for i in 1..=len {
                w.set_block(at(i), Block::IskritWire);
            }
            w.set_block(at(len + 1), Block::IskritLamp);
            update_iskrit(&mut w, at(1));
            w.cell_block(at(len + 1)) == Block::IskritLampOn
        };
        assert!(run(3), "short line lights the lamp");
        assert!(run(15), "15 blocks of dust still carry the signal");
        assert!(!run(16), "the signal dies after 15 blocks");
        // Removing the torch turns everything off again.
        w.blocks.clear();
        w.set_block(at(0), Block::IskritTorch);
        for i in 1..=4 {
            w.set_block(at(i), Block::IskritWire);
        }
        w.set_block(at(5), Block::IskritLamp);
        update_iskrit(&mut w, at(0));
        assert_eq!(w.cell_block(at(2)), Block::IskritWireOn);
        w.set_block(at(0), Block::Air);
        update_iskrit(&mut w, at(0));
        assert_eq!(w.cell_block(at(2)), Block::IskritWire);
        assert_eq!(w.cell_block(at(5)), Block::IskritLamp);
    }
}
