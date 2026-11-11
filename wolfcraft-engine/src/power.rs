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