//! Block breaking, placing and item use.

use crate::block::{Block, RenderKind};
use crate::chunk_manager::ChunkManager;
use crate::inventory::Inventory;
use crate::items::{self, ItemStack};
use crate::cubesphere::Cell;
use crate::physics::{Hit, Voxels, body_overlaps_cell};
use crate::player::Player;
use glam::IVec3;

pub trait WorldEdit: Voxels {
    fn set_block(&mut self, c: Cell, b: Block) -> bool;
}

impl WorldEdit for ChunkManager {
    fn set_block(&mut self, c: Cell, b: Block) -> bool {
        ChunkManager::set_block(self, c, b)
    }
}

pub const REACH_SURVIVAL: f64 = 5.0;
pub const REACH_CREATIVE: f64 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InteractionEvent {
    Broken { pos: Cell, block: Block },
    Placed { pos: Cell, block: Block },
    Ate { hunger: f32 },
    OpenCrafting,
    /// Right click on a chest or electric furnace.
    OpenContainer { pos: Cell, block: Block },
}

#[derive(Default)]
pub struct Interaction {
    target: Option<Cell>,
    pub progress: f32,
    cooldown: f32,
    /// Seconds since LMB was last held on the target (progress fades after a pause).
    idle: f32,
    was_holding: bool,
}

/// Mining progress is kept this long after releasing LMB, so repeated clicks also break blocks.
const CLICK_MEMORY: f32 = 0.8;
/// Extra mining time granted by each fresh click (a punch), in seconds.
const CLICK_BONUS: f32 = 0.12;

impl Interaction {
    /// Current mining progress 0..1 (for the HUD).
    pub fn mining_fraction(&self) -> f32 {
        self.progress.clamp(0.0, 1.0)
    }

    pub fn reach(player: &Player) -> f64 {
        if player.is_creative() { REACH_CREATIVE } else { REACH_SURVIVAL }
    }

    /// Call every frame. `holding` = left mouse held. Returns events that happened.
    pub fn update_break(&mut self, world: &mut impl WorldEdit, player: &mut Player, hit: Option<Hit>, holding: bool, dt: f32, seed: u64) -> Vec<InteractionEvent> {
        let mut events = Vec::new();
        self.cooldown = (self.cooldown - dt).max(0.0);
        let pressed = holding && !self.was_holding;
        self.was_holding = holding;
        let Some(hit) = hit else {
            self.target = None;
            self.progress = 0.0;
            return events;
        };
        if self.target != Some(hit.cell) {
            // Looking at another block: start over (but keep counting while not holding).
            self.target = if holding { Some(hit.cell) } else { None };
            self.progress = 0.0;
            self.idle = 0.0;
        }
        if !holding {
            self.idle += dt;
            if self.idle > CLICK_MEMORY {
                self.target = None;
                self.progress = 0.0;
            }
            return events;
        }
        self.idle = 0.0;
        let held = player.inventory.held().map(|s| s.info());
        let Some(time) = items::break_time(held, hit.block, player.is_creative()) else {
            self.progress = 0.0;
            return events;
        };
        if time <= 0.0 {
            if self.cooldown > 0.0 {
                return events;
            }
            self.cooldown = 0.18;
            self.progress = 1.0;
        } else {
            self.progress += (dt + if pressed { CLICK_BONUS } else { 0.0 }) / time;
        }
        if self.progress >= 1.0 {
            self.progress = 0.0;
            self.target = None;
            if self.break_block(world, player, hit.cell, hit.block, seed) {
                events.push(InteractionEvent::Broken { pos: hit.cell, block: hit.block });
            }
        }
        events
    }

    fn break_block(&mut self, world: &mut impl WorldEdit, player: &mut Player, pos: Cell, block: Block, seed: u64) -> bool {
        if !world.set_block(pos, Block::Air) {
            return false;
        }
        if !player.is_creative() {
            let held = player.inventory.held().map(|s| s.info());
            let ok = items::can_harvest(held, block);
            for d in items::drops(block, ok, IVec3::new(pos.i, pos.j, pos.k ^ ((pos.f as i32) << 24)), seed) {
                player.inventory.add(d);
            }
            player.survival.add_xp(items::xp_for(block));
            player.survival.exhaust(0.025);
            if block.info().hardness > 0.0 {
                player.inventory.wear_selected();
            }
        }
        // Plants lose their support: remove the cross-plant standing on top.
        if let Some(above) = world.grid().neighbor(pos, 2, 1) {
            if world.cell_block(above).info().render == RenderKind::Cross {
                world.set_block(above, Block::Air);
            }
        }
        true
    }

    /// Right click: use the targeted block, eat, or place the held block.
    pub fn use_item(&mut self, world: &mut impl WorldEdit, player: &mut Player, hit: Option<Hit>) -> Vec<InteractionEvent> {
        let mut events = Vec::new();
        if let Some(h) = hit {
            if h.block == Block::CraftingTable && !player.sneaking {
                events.push(InteractionEvent::OpenCrafting);
                return events;
            }
            // Switch: toggles; the caller re-evaluates the power network on `Placed`.
            if matches!(h.block, Block::SwitchOff | Block::SwitchOn) && !player.sneaking {
                let new = if h.block == Block::SwitchOff { Block::SwitchOn } else { Block::SwitchOff };
                if world.set_block(h.cell, new) {
                    events.push(InteractionEvent::Placed { pos: h.cell, block: new });
                }
                return events;
            }
            // Button: pressed for a moment (the session releases it).
            if h.block == Block::Button && !player.sneaking {
                if world.set_block(h.cell, Block::ButtonOn) {
                    events.push(InteractionEvent::Placed { pos: h.cell, block: Block::ButtonOn });
                }
                return events;
            }
            // Door: opens and closes.
            if let (Some(new), false) = (h.block.door_toggle(), player.sneaking) {
                if world.set_block(h.cell, new) {
                    events.push(InteractionEvent::Placed { pos: h.cell, block: new });
                }
                return events;
            }
            // Chest and electric furnace open their own window (sneak to place blocks instead).
            if matches!(h.block, Block::Chest | Block::ElectricFurnace | Block::CommandBlock | Block::Terminal | Block::Sign | Block::SignB) && !player.sneaking {
                events.push(InteractionEvent::OpenContainer { pos: h.cell, block: h.block });
                return events;
            }
        }
        let Some(stack) = player.inventory.held() else { return events };
        // Armor in hand: put it on.
        if player.inventory.equip_selected() {
            return events;
        }
        let info = stack.info();
        if let Some((hunger, sat)) = info.food {
            if player.survival.hunger < 20.0 && !player.is_creative() {
                player.inventory.consume_selected();
                player.survival.eat(hunger, sat);
                events.push(InteractionEvent::Ate { hunger });
            }
            return events;
        }
        let (Some(block), Some(h)) = (info.place, hit) else { return events };
        let Some(target) = h.front_cell(world.grid()) else { return events };
        // Turn stairs, panes, doors and signs to the player's view.
        let block = {
            let g = world.grid();
            let p0 = g.point(target.f, target.i as f64, target.j as f64, target.k as f64);
            let ei = (g.point(target.f, target.i as f64 + 1.0, target.j as f64, target.k as f64) - p0).normalize_or_zero();
            let ej = (g.point(target.f, target.i as f64, target.j as f64 + 1.0, target.k as f64) - p0).normalize_or_zero();
            block.oriented(player.heading.dot(ei), player.heading.dot(ej))
        };
        if !world.cell_block(target).is_replaceable() {
            return events;
        }
        if block.is_solid() && body_overlaps_cell(world.grid(), player.body.pos, target) {
            return events;
        }
        if world.set_block(target, block) {
            if !player.is_creative() {
                player.inventory.consume_selected();
            }
            events.push(InteractionEvent::Placed { pos: target, block });
        }
        events
    }

    /// Middle click in creative: copy the targeted block into the selected hotbar slot.
    pub fn pick_block(&self, player: &mut Player, hit: Option<Hit>) {
        if !player.is_creative() {
            return;
        }
        if let Some(h) = hit {
            let id = h.block as items::ItemId;
            let inv: &mut Inventory = &mut player.inventory;
            if let Some(i) = inv.slots.iter().position(|s| s.map(|s| s.id) == Some(id)) {
                if i < crate::inventory::HOTBAR {
                    inv.selected = i;
                    return;
                }
            }
            let sel = inv.selected;
            inv.slots[sel] = Some(ItemStack::new(id, 64));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Difficulty, GameMode};
    use crate::items::ids;
    use std::collections::HashMap;

    use crate::cubesphere::Grid;
    use glam::DVec3;

    struct Mock {
        grid: Grid,
        blocks: HashMap<Cell, Block>,
    }
    impl Mock {
        fn block(&self, c: Cell) -> Block {
            self.cell_block(c)
        }
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

    /// Old flat test coordinates (x, height, z) → a cell on face +Z of a 64-cell grid.
    fn c(x: i32, y: i32, z: i32) -> Cell {
        Cell::new(4, x + 20, z + 20, y)
    }
    /// World point at continuous flat coordinates (x, height, z).
    fn wp(x: f64, y: f64, z: f64) -> DVec3 {
        Grid::with_cells(64).point(4, x + 20.0, z + 20.0, y)
    }
    const UP: (usize, i32) = (2, 1);
    const EAST: (usize, i32) = (0, 1);

    fn hit(world: &Mock, cell: Cell, normal: (usize, i32)) -> Hit {
        Hit { cell, normal, dist: 1.0, block: world.block(cell) }
    }

    fn setup() -> (Mock, Player) {
        let mut w = Mock { grid: Grid::with_cells(64), blocks: HashMap::new() };
        w.blocks.insert(c(0, 50, 0), Block::Stone);
        w.blocks.insert(c(1, 50, 0), Block::Dirt);
        let p = Player::new(wp(0.5, 80.0, 0.5), GameMode::Survival);
        (w, p)
    }

    #[test]
    fn survival_mining_takes_time_and_drops_items() {
        let (mut w, mut p) = setup();
        let mut it = Interaction::default();
        let h = hit(&w, c(1, 50, 0), UP);
        // Dirt by hand: 0.75 s.
        for _ in 0..30 {
            it.update_break(&mut w, &mut p, Some(h), true, 0.016, 1);
        }
        assert_eq!(w.block(h.cell), Block::Dirt, "not yet broken");
        let mut broken = false;
        for _ in 0..30 {
            broken |= it.update_break(&mut w, &mut p, Some(h), true, 0.016, 1).iter().any(|e| matches!(e, InteractionEvent::Broken { .. }));
        }
        assert!(broken);
        assert_eq!(w.block(h.cell), Block::Air);
        assert_eq!(p.inventory.count(Block::Dirt as u16), 1);
    }

    #[test]
    fn stone_by_hand_drops_nothing_but_pickaxe_gives_cobblestone() {
        let (mut w, mut p) = setup();
        let mut it = Interaction::default();
        let h = hit(&w, c(0, 50, 0), UP);
        for _ in 0..1000 {
            it.update_break(&mut w, &mut p, Some(h), true, 0.05, 1);
            if w.block(h.cell) == Block::Air {
                break;
            }
        }
        assert_eq!(w.block(h.cell), Block::Air);
        assert_eq!(p.inventory.count(Block::Cobblestone as u16), 0, "no pickaxe → no drop");
        w.blocks.insert(h.cell, Block::Stone);
        p.inventory.slots[0] = Some(ItemStack::new(ids::tool(1, 0), 1));
        let t0 = std::time::Instant::now();
        let mut frames = 0;
        while w.block(h.cell) == Block::Stone && frames < 1000 {
            it.update_break(&mut w, &mut p, Some(h), true, 0.05, 1);
            frames += 1;
        }
        let _ = t0;
        assert_eq!(p.inventory.count(Block::Cobblestone as u16), 1);
        assert_eq!(p.inventory.slots[0].unwrap().damage, 1, "tool wears");
        assert!(frames as f32 * 0.05 < 7.5 / 3.9, "stone pickaxe must be faster than by hand");
    }

    #[test]
    fn creative_breaks_instantly_without_drops() {
        let (mut w, mut p) = setup();
        p.mode = GameMode::Creative;
        let mut it = Interaction::default();
        let h = hit(&w, c(0, 50, 0), UP);
        it.update_break(&mut w, &mut p, Some(h), true, 0.016, 1);
        assert_eq!(w.block(h.cell), Block::Air);
        assert!(p.inventory.is_empty());
    }

    #[test]
    fn bedrock_cannot_be_broken() {
        let (mut w, mut p) = setup();
        w.blocks.insert(c(5, 50, 5), Block::Bedrock);
        p.mode = GameMode::Creative;
        let mut it = Interaction::default();
        let h = hit(&w, c(5, 50, 5), UP);
        for _ in 0..10 {
            it.update_break(&mut w, &mut p, Some(h), true, 0.3, 1);
        }
        assert_eq!(w.block(h.cell), Block::Bedrock);
    }

    #[test]
    fn placing_consumes_items_and_respects_the_player_body() {
        let (mut w, mut p) = setup();
        let mut it = Interaction::default();
        p.inventory.add(ItemStack::of_block(Block::Cobblestone, 2));
        let h = hit(&w, c(0, 50, 0), UP);
        let ev = it.use_item(&mut w, &mut p, Some(h));
        assert_eq!(ev, vec![InteractionEvent::Placed { pos: c(0, 51, 0), block: Block::Cobblestone }]);
        assert_eq!(w.block(c(0, 51, 0)), Block::Cobblestone);
        assert_eq!(p.inventory.count(Block::Cobblestone as u16), 1);
        // Can't place into an occupied cell.
        let again = hit(&w, c(0, 50, 0), UP);
        assert!(it.use_item(&mut w, &mut p, Some(again)).is_empty());
        // Can't place inside the player's own body.
        p.body.pos = wp(2.5, 50.2, 0.5);
        let h2 = hit(&w, c(1, 50, 0), EAST);
        assert!(it.use_item(&mut w, &mut p, Some(h2)).is_empty(), "block would trap the player");
    }

    #[test]
    fn eating_restores_hunger_only_when_hungry() {
        let (mut w, mut p) = setup();
        let mut it = Interaction::default();
        p.inventory.slots[0] = Some(ItemStack::new(ids::BREAD, 2));
        assert!(it.use_item(&mut w, &mut p, None).is_empty(), "full → nothing happens");
        p.survival.hunger = 10.0;
        let ev = it.use_item(&mut w, &mut p, None);
        assert_eq!(ev, vec![InteractionEvent::Ate { hunger: 5.0 }]);
        assert_eq!(p.survival.hunger, 15.0);
        assert_eq!(p.inventory.count(ids::BREAD), 1);
    }

    #[test]
    fn breaking_support_removes_plant_above_and_table_opens_crafting() {
        let (mut w, mut p) = setup();
        p.mode = GameMode::Creative;
        w.blocks.insert(c(0, 51, 0), Block::TallGrass);
        let mut it = Interaction::default();
        let hh = hit(&w, c(0, 50, 0), UP);
        it.update_break(&mut w, &mut p, Some(hh), true, 0.016, 1);
        // dominant axis of (0,50,0) is +Y so the plant above sits at y+1.
        assert_eq!(w.block(c(0, 51, 0)), Block::Air);
        w.blocks.insert(c(3, 50, 3), Block::CraftingTable);
        let th = hit(&w, c(3, 50, 3), UP);
        let ev = it.use_item(&mut w, &mut p, Some(th));
        assert_eq!(ev, vec![InteractionEvent::OpenCrafting]);
        let _ = Difficulty::Normal;
    }
}

/// Electric furnace recipes: input item -> output item.
pub fn smelt_result(id: items::ItemId) -> Option<items::ItemId> {
    use items::ids;
    Some(match id {
        ids::RAW_IRON => ids::IRON_INGOT,
        ids::RAW_GOLD => ids::GOLD_INGOT,
        ids::RAW_COPPER => ids::COPPER_INGOT,
        ids::RAW_TIN => ids::TIN_INGOT,
        ids::RAW_SILVER => ids::SILVER_INGOT,
        ids::RAW_MEAT => ids::COOKED_MEAT,
        x if x == Block::Cobblestone as items::ItemId => Block::Stone as items::ItemId,
        x if x == Block::Sand as items::ItemId => Block::Glass as items::ItemId,
        _ => return None,
    })
}
