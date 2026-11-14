//! Redstone-style power network.
//!
//! * (The old generator block is now the Чаровальня: no power source any more.)
//! * `Battery` conducts power between face-adjacent blocks.
//! * `ElectricLamp` / `ElectricLampOn` are consumers: lit while touching a powered conductor
//!   or generator.
//!
//! The network is event-driven: call [`update_around`] after a block was placed or broken.

use crate::block::{Block, Part};
use crate::interaction::WorldEdit;
use crate::cubesphere::Cell;
use std::collections::{HashMap, HashSet, VecDeque};

/// Upper bound on the cells explored for one network (keeps edits cheap).
const MAX_NODES: usize = 4096;

fn conducts(b: Block) -> bool {
    matches!(b, Block::SwitchOn)
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
        if world.cell_block(c) == Block::SwitchOn {
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
    matches!(b, Block::IskritBlock | Block::IskritTorch | Block::SwitchOn | Block::ButtonOn | Block::PressurePlateOn)
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

/// Neighbour of `c` in direction `d` (0 +i, 1 -i, 2 +j, 3 -j, 4 up, 5 down).
pub fn dir_neighbor(world: &impl WorldEdit, c: Cell, d: u8) -> Option<Cell> {
    world.grid().neighbor(c, (d / 2) as usize, if d % 2 == 0 { 1 } else { -1 })
}

/// A repeater or comparator at `n` that is on and points into `c`.
fn gate_into(world: &impl WorldEdit, n: Cell, c: Cell) -> bool {
    match world.cell_block(n).part() {
        Some(Part::Repeater { on: true, dir }) | Some(Part::Comparator { on: true, dir }) => dir_neighbor(world, n, dir) == Some(c),
        _ => false,
    }
}

/// Does the block at `n` send a signal into `c`? Sources and charged dust power everything
/// around them; repeaters and comparators only the block in front of them.
fn feeds(world: &impl WorldEdit, n: Cell, c: Cell) -> bool {
    let b = world.cell_block(n);
    is_iskrit_source(b) || b == Block::IskritWireOn || gate_into(world, n, c)
}

/// Is `c` powered by any neighbour?
pub fn powered(world: &impl WorldEdit, c: Cell) -> bool {
    neighbors(world, c).into_iter().any(|n| feeds(world, n, c))
}

/// Recomputes the iskrit network around `origin` (an edited cell): charges or discharges the
/// dust, switches the lamps, and lets repeaters, comparators and pistons react; every change
/// of a mechanism carries on to what it touches. Returns how many blocks changed.
pub fn update_iskrit(world: &mut impl WorldEdit, origin: Cell) -> usize {
    let mut todo = vec![origin];
    let mut total = 0;
    let mut steps = 0;
    while let Some(o) = todo.pop() {
        steps += 1;
        if steps > 96 {
            break; // a clock or a loop: stop for this edit
        }
        let (changed, probe) = update_network(world, o);
        total += changed;
        let mut parts: Vec<Cell> = probe.into_iter().filter(|c| matches!(world.cell_block(*c).part(), Some(Part::Repeater { .. } | Part::Comparator { .. } | Part::Piston { .. }))).collect();
        parts.sort_by_key(|c| (c.f, c.i, c.j, c.k));
        parts.dedup();
        for c in parts {
            let next = eval_part(world, c);
            if !next.is_empty() {
                total += 1;
                todo.extend(next);
            }
        }
    }
    total
}

/// Blocks a piston cannot move: liquids, unbreakable blocks, obsidian, extended pistons and
/// their heads, doors, and blocks that keep extra data (chests, signs, command blocks…).
fn movable(b: Block) -> bool {
    use Block::*;
    if b.is_fluid() || !b.is_breakable() || b.door_toggle().is_some() {
        return false;
    }
    if matches!(b.part(), Some(Part::Piston { extended: true, .. } | Part::Head { .. })) {
        return false;
    }
    !matches!(b, Obsidian | Chest | Sign | SignB | CommandBlock | Terminal | ElectricFurnace | Bedrock)
}

/// Lets one mechanism react to its inputs. Returns the cells to re-evaluate (empty: no change).
fn eval_part(world: &mut impl WorldEdit, c: Cell) -> Vec<Cell> {
    let Some(part) = world.cell_block(c).part() else { return vec![] };
    let mut out = Vec::new();
    match part {
        Part::Repeater { on, dir } => {
            // Full-strength output in front when anything powers it from behind; one way only.
            let want = dir_neighbor(world, c, dir ^ 1).is_some_and(|b| feeds(world, b, c));
            if want != on {
                world.set_block(c, Part::Repeater { on: want, dir }.block());
                out.push(c);
                out.extend(dir_neighbor(world, c, dir));
            }
        }
        Part::Comparator { on, dir } => {
            // On when the back is powered and neither side is: a gate.
            let back = dir_neighbor(world, c, dir ^ 1).is_some_and(|b| feeds(world, b, c));
            let side_axis = if dir < 2 { 2u8 } else { 0 };
            let side = [side_axis, side_axis + 1].iter().any(|d| dir_neighbor(world, c, *d).is_some_and(|n| feeds(world, n, c)));
            let want = back && !side;
            if want != on {
                world.set_block(c, Part::Comparator { on: want, dir }.block());
                out.push(c);
                out.extend(dir_neighbor(world, c, dir));
            }
        }
        Part::Piston { sticky, extended, dir } => {
            let p = powered(world, c);
            if p && !extended {
                if let Some(cells) = extend(world, c, sticky, dir) {
                    out.extend(cells);
                }
            } else if !p && extended {
                out.extend(retract(world, c, sticky, dir));
            }
        }
        Part::Head { .. } => {}
    }
    out
}

/// Doors a signal holds swung (lower halves).
static POWERED_DOORS: std::sync::LazyLock<std::sync::Mutex<HashSet<Cell>>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(HashSet::new()));

/// Doors swung by a button: they stay as they are when the button pops back up, and the next
/// press swings them back (press — open, press again — shut).
static LATCHED_DOORS: std::sync::LazyLock<std::sync::Mutex<HashSet<Cell>>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(HashSet::new()));

/// Plants a piston crushed (cell, block), for the game to drop as items.
static CRUSHED: std::sync::Mutex<Vec<(Cell, Block)>> = std::sync::Mutex::new(Vec::new());

/// Takes the plants pistons crushed since the last call.
pub fn take_crushed() -> Vec<(Cell, Block)> {
    CRUSHED.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default()
}

/// Pushes up to 12 blocks in front of the piston one cell on and puts the head out.
fn extend(world: &mut impl WorldEdit, c: Cell, sticky: bool, dir: u8) -> Option<Vec<Cell>> {
    let front = dir_neighbor(world, c, dir)?;
    let mut line = Vec::new();
    let mut cur = front;
    loop {
        let b = world.cell_block(cur);
        if b == Block::Air || b.is_replaceable() {
            break;
        }
        if !movable(b) || line.len() >= 12 {
            return None;
        }
        line.push(cur);
        cur = dir_neighbor(world, cur, dir)?;
    }
    let mut touched = vec![c, front, cur];
    // The free cell at the end may hold a plant: it gets crushed (and drops).
    let end = world.cell_block(cur);
    if end != Block::Air && !end.is_fluid() {
        if let Ok(mut v) = CRUSHED.lock() {
            v.push((cur, end));
        }
    }
    let mut dst = cur;
    for &src in line.iter().rev() {
        let b = world.cell_block(src);
        world.set_block(dst, b);
        touched.push(dst);
        dst = src;
    }
    world.set_block(front, Part::Head { sticky, dir }.block());
    world.set_block(c, Part::Piston { sticky, extended: true, dir }.block());
    Some(touched)
}

/// Pulls the head back; a sticky piston brings the block in front of the head with it.
fn retract(world: &mut impl WorldEdit, c: Cell, sticky: bool, dir: u8) -> Vec<Cell> {
    let mut touched = vec![c];
    if let Some(front) = dir_neighbor(world, c, dir) {
        if matches!(world.cell_block(front).part(), Some(Part::Head { .. })) {
            let beyond = dir_neighbor(world, front, dir);
            let pulled = beyond.map(|b| world.cell_block(b)).filter(|b| sticky && *b != Block::Air && !b.is_replaceable() && movable(*b));
            match (pulled, beyond) {
                (Some(b), Some(from)) => {
                    world.set_block(front, b);
                    world.set_block(from, Block::Air);
                    touched.push(from);
                }
                _ => {
                    world.set_block(front, Block::Air);
                }
            }
            touched.push(front);
        }
    }
    world.set_block(c, Part::Piston { sticky, extended: false, dir }.block());
    touched
}

/// A piston or its head was broken: the other part goes too (the base returns to rest).
pub fn piston_broken(world: &mut impl WorldEdit, pos: Cell, block: Block) {
    match block.part() {
        Some(Part::Head { dir, .. }) => {
            if let Some(base) = dir_neighbor(world, pos, dir ^ 1) {
                for sticky in [false, true] {
                    if world.cell_block(base) == (Part::Piston { sticky, extended: true, dir }).block() {
                        world.set_block(base, Part::Piston { sticky, extended: false, dir }.block());
                    }
                }
            }
        }
        Some(Part::Piston { extended: true, dir, .. }) => {
            if let Some(front) = dir_neighbor(world, pos, dir) {
                if matches!(world.cell_block(front).part(), Some(Part::Head { .. })) {
                    world.set_block(front, Block::Air);
                }
            }
        }
        _ => {}
    }
}

/// Dust, lamps and the cells around them for one edit. Returns (blocks changed, cells looked at).
fn update_network(world: &mut impl WorldEdit, origin: Cell) -> (usize, HashSet<Cell>) {
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
        if neighbors(world, *w).into_iter().any(|n| is_iskrit_source(world.cell_block(n)) || gate_into(world, n, *w)) {
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
    for c in &probe {
        if is_iskrit_lamp(world.cell_block(*c)) {
            lamps.insert(*c);
        }
    }
    for l in lamps {
        let on = powered(world, l);
        let want = if on { Block::IskritLampOn } else { Block::IskritLamp };
        if world.cell_block(l) != want {
            world.set_block(l, want);
            changed += 1;
        }
    }
    // Doors: open while either half is powered, shut when the signal goes (automatic doors).
    let mut doors: Vec<Cell> = Vec::new();
    for c in &probe {
        let b = world.cell_block(*c);
        if b.door_toggle().is_some() {
            // Work on the lower half.
            let bottom = if b.is_door_top() { world.grid().neighbor(*c, 2, -1).unwrap_or(*c) } else { *c };
            if !doors.contains(&bottom) {
                doors.push(bottom);
            }
        }
    }
    for d in doors {
        let b = world.cell_block(d);
        if b.door_toggle().is_none() {
            continue;
        }
        let top = crate::interaction::door_partner(world, d, b);
        // A door the player just used by hand is left alone, and so is any door without iskrit
        // beside it (ordinary house doors stay manual).
        if d == origin || top.is_some_and(|(t, _)| t == origin) {
            continue;
        }
        let wired = |c: Cell| neighbors(world, c).into_iter().any(|n| {
            let nb = world.cell_block(n);
            is_iskrit_source(nb) || is_wire(nb) || nb.part().is_some() || matches!(nb, Block::SwitchOff | Block::Button | Block::PressurePlate)
        });
        if !(wired(d) || top.is_some_and(|(t, _)| wired(t))) {
            continue;
        }
        let on = powered(world, d) || top.is_some_and(|(t, _)| powered(world, t));
        // "Open" is only a turn of the leaf (a door placed the other way is closed in the open
        // state), so the signal swings the door when it comes and swings it back when it goes;
        // the doors held by a signal are remembered.
        let held = POWERED_DOORS.lock().map(|s| s.contains(&d)).unwrap_or(false);
        if on != held {
            if let Ok(mut s) = POWERED_DOORS.lock() {
                if on {
                    s.insert(d);
                } else {
                    s.remove(&d);
                }
            }
            // A button is a press, not a hold: the door turns on the press and keeps its new
            // position when the button pops back up.
            let button = on && (probe.iter().any(|c| world.cell_block(*c) == Block::ButtonOn) || neighbors(world, d).into_iter().chain(top.map(|(t, _)| neighbors(world, t)).unwrap_or_default()).any(|n| world.cell_block(n) == Block::ButtonOn));
            let swing = if on {
                if button {
                    if let Ok(mut l) = LATCHED_DOORS.lock() {
                        l.insert(d);
                    }
                }
                true
            } else {
                // Released: a door turned by a button stays where it is.
                !LATCHED_DOORS.lock().map(|mut l| l.remove(&d)).unwrap_or(false)
            };
            if !swing {
                continue;
            }
            if let Some(nb) = b.door_toggle() {
                world.set_block(d, nb);
                changed += 1;
            }
            if let Some((t, tb)) = top {
                if let Some(nt) = tb.door_toggle() {
                    world.set_block(t, nt);
                }
            }
        }
    }
    (changed, probe.into_iter().collect())
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

    fn mock() -> Mock {
        Mock { grid: Grid::with_cells(64), blocks: HashMap::new() }
    }
    fn at(i: i32, j: i32) -> Cell {
        Cell::new(4, 10 + i, 20 + j, 30)
    }

    #[test]
    fn a_pressure_plate_opens_a_door_and_house_doors_stay_manual() {
        let mut w = mock();
        let up = |c: Cell| Cell::new(c.f, c.i, c.j, c.k + 1);
        // Door at 0 with a plate in front of it.
        w.set_block(at(0, 0), Block::Door);
        w.set_block(up(at(0, 0)), Block::DoorTop);
        w.set_block(at(-1, 0), Block::PressurePlate);
        w.set_block(at(-1, 0), Block::PressurePlateOn);
        update_iskrit(&mut w, at(-1, 0));
        assert_eq!(w.cell_block(at(0, 0)), Block::DoorOpen, "stepping on the plate opens the door");
        assert_eq!(w.cell_block(up(at(0, 0))), Block::DoorTopOpen, "both halves");
        w.set_block(at(-1, 0), Block::PressurePlate);
        update_iskrit(&mut w, at(-1, 0));
        assert_eq!(w.cell_block(at(0, 0)), Block::Door, "and it shuts again");
        // A door with no iskrit around keeps whatever the player did to it.
        w.set_block(at(5, 0), Block::DoorOpen);
        w.set_block(up(at(5, 0)), Block::DoorTopOpen);
        w.set_block(at(6, 0), Block::Stone);
        update_iskrit(&mut w, at(6, 0));
        assert_eq!(w.cell_block(at(5, 0)), Block::DoorOpen, "manual door untouched");
    }

    #[test]
    fn a_button_opens_a_door_and_the_next_press_shuts_it() {
        let mut w = mock();
        let up = |c: Cell| Cell::new(c.f, c.i, c.j, c.k + 1);
        w.set_block(at(20, 0), Block::Door);
        w.set_block(up(at(20, 0)), Block::DoorTop);
        w.set_block(at(20, 1), Block::Button);
        let press = |w: &mut Mock| {
            w.set_block(at(20, 1), Block::ButtonOn);
            update_iskrit(w, at(20, 1));
            w.set_block(at(20, 1), Block::Button);
            update_iskrit(w, at(20, 1));
        };
        press(&mut w);
        assert_eq!(w.cell_block(at(20, 0)), Block::DoorOpen, "a press opens the door and it stays open");
        assert_eq!(w.cell_block(up(at(20, 0))), Block::DoorTopOpen);
        press(&mut w);
        assert_eq!(w.cell_block(at(20, 0)), Block::Door, "the next press shuts it");
        assert_eq!(w.cell_block(up(at(20, 0))), Block::DoorTop);
    }

    #[test]
    fn a_piston_pushes_a_row_and_a_sticky_one_pulls_a_block_back() {
        for sticky in [false, true] {
            let mut w = mock();
            // Piston at 0 facing +i, two stone blocks in front, a switch behind it.
            w.set_block(at(0, 0), Part::Piston { sticky, extended: false, dir: 0 }.block());
            w.set_block(at(1, 0), Block::Stone);
            w.set_block(at(2, 0), Block::Cobblestone);
            w.set_block(at(-1, 0), Block::SwitchOn);
            update_iskrit(&mut w, at(-1, 0));
            assert_eq!(w.cell_block(at(0, 0)), Part::Piston { sticky, extended: true, dir: 0 }.block(), "extended");
            assert_eq!(w.cell_block(at(1, 0)), Part::Head { sticky, dir: 0 }.block());
            assert_eq!((w.cell_block(at(2, 0)), w.cell_block(at(3, 0))), (Block::Stone, Block::Cobblestone), "the row moved one cell");
            // Switch off: the head comes back; only the sticky piston brings the stone with it.
            w.set_block(at(-1, 0), Block::SwitchOff);
            update_iskrit(&mut w, at(-1, 0));
            assert_eq!(w.cell_block(at(0, 0)), Part::Piston { sticky, extended: false, dir: 0 }.block(), "retracted");
            if sticky {
                assert_eq!(w.cell_block(at(1, 0)), Block::Stone, "pulled back");
                assert_eq!(w.cell_block(at(2, 0)), Block::Air);
            } else {
                assert_eq!(w.cell_block(at(1, 0)), Block::Air);
                assert_eq!(w.cell_block(at(2, 0)), Block::Stone, "left where it was pushed");
            }
        }
        // Obsidian cannot be pushed: the piston stays in.
        let mut w = mock();
        w.set_block(at(0, 0), Part::Piston { sticky: false, extended: false, dir: 0 }.block());
        w.set_block(at(1, 0), Block::Obsidian);
        w.set_block(at(-1, 0), Block::SwitchOn);
        update_iskrit(&mut w, at(-1, 0));
        assert_eq!(w.cell_block(at(1, 0)), Block::Obsidian);
    }

    #[test]
    fn a_repeater_renews_the_signal_and_only_passes_it_forward() {
        let mut w = mock();
        // Torch, 14 dust, a repeater (+i), 10 more dust and a lamp: far beyond 15 blocks.
        w.set_block(at(0, 0), Block::IskritTorch);
        for i in 1..=14 {
            w.set_block(at(i, 0), Block::IskritWire);
        }
        w.set_block(at(15, 0), Part::Repeater { on: false, dir: 0 }.block());
        for i in 16..=25 {
            w.set_block(at(i, 0), Block::IskritWire);
        }
        w.set_block(at(26, 0), Block::IskritLamp);
        update_iskrit(&mut w, at(1, 0));
        assert_eq!(w.cell_block(at(15, 0)), Part::Repeater { on: true, dir: 0 }.block());
        assert_eq!(w.cell_block(at(26, 0)), Block::IskritLampOn, "the repeater carries the signal on");
        // Backwards: a source in front of a repeater does not reach behind it.
        let mut w = mock();
        w.set_block(at(0, 0), Block::IskritLamp);
        w.set_block(at(1, 0), Part::Repeater { on: false, dir: 0 }.block());
        w.set_block(at(2, 0), Block::IskritBlock);
        update_iskrit(&mut w, at(2, 0));
        assert_eq!(w.cell_block(at(1, 0)), Part::Repeater { on: false, dir: 0 }.block());
        assert_eq!(w.cell_block(at(0, 0)), Block::IskritLamp, "one way only");
    }

    #[test]
    fn a_comparator_passes_the_back_signal_unless_a_side_is_powered() {
        let mut w = mock();
        // Comparator facing +i: input behind at -i, lamp in front, a side input at +j.
        w.set_block(at(-1, 0), Block::IskritBlock);
        w.set_block(at(0, 0), Part::Comparator { on: false, dir: 0 }.block());
        w.set_block(at(1, 0), Block::IskritLamp);
        update_iskrit(&mut w, at(-1, 0));
        assert_eq!(w.cell_block(at(1, 0)), Block::IskritLampOn, "back powered, sides quiet: on");
        w.set_block(at(0, 1), Block::IskritBlock);
        update_iskrit(&mut w, at(0, 1));
        assert_eq!(w.cell_block(at(0, 0)), Part::Comparator { on: false, dir: 0 }.block(), "a powered side blocks it");
        assert_eq!(w.cell_block(at(1, 0)), Block::IskritLamp);
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
