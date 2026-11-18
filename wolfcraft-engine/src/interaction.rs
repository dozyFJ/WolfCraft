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
    /// Armour (or a shield) was put on from the hand.
    Equipped,
    /// A hand crank was turned.
    Crank { pos: Cell },
    /// The «Конструктор» block: building pieces (forms × materials).
    OpenBuilder,
    FrameUse {pos:Cell},
    /// A bed was used: the spawn point is set; `night` = slept through to the morning.
    Slept { night: bool, pos: Cell },
    /// A chair or sofa was used: the player sits down on it.
    Sat { pos: Cell },
    ActivateBoss {pos:Cell,kind:crate::bosses::BossKind},
    /// Right click on a chest or electric furnace.
    OpenContainer { pos: Cell, block: Block },
    /// A broken block dropped an item into the world (only with `Interaction::spill`).
    Dropped { pos: Cell, stack: ItemStack },
    /// A spawn soul was used: a creature appears in this cell.
    SpawnMob { pos: Cell, kind: crate::mobs::MobKind },
    /// Lighter used on dynamite. The session lights it and keeps the event if that worked.
    Ignite { pos: Cell },
    /// Use with a grenade launcher, bazooka or grenade. Kept only if a shot was fired.
    Fire { weapon: items::ItemId },
    /// A bullet struck a creature or a surface (for the hit sound and marker).
    BulletHit { at: glam::DVec3, creature: bool },
    /// A blast at a world point (sound, particles); `power` is its radius in blocks.
    Exploded { at: glam::DVec3, power: f32 },
    /// A block destroyed by a blast (sync, chips), without the usual break sound.
    Blasted { pos: Cell, block: Block },
    /// Tap on an incubator: buy a capsule egg of this tier (0 common … 2 legendary) into the
    /// inventory. The session keeps the event only if the egg was bought.
    BuyEgg { tier: u8 },
    /// A capsule egg used from the hand: it hatches (the session takes the egg and adds the pet).
    Hatch { tier: u8 },
    /// A block set by the game itself (coin blocks appearing): only needs syncing.
    WorldSet { pos: Cell, block: Block },
    /// A falling block broke apart here (chips and a sound; nothing to sync).
    Shattered { at: glam::DVec3, block: Block },
    /// A falling block settled into this cell (a thud; synced by its `WorldSet`).
    Landed { pos: Cell, block: Block },
}

#[derive(Default)]
pub struct Interaction {
    target: Option<Cell>,
    pub progress: f32,
    cooldown: f32,
    /// Seconds since LMB was last held on the target (progress fades after a pause).
    idle: f32,
    was_holding: bool,
    /// Drops fall into the world as items (`InteractionEvent::Dropped`) instead of going
    /// straight into the inventory.
    pub spill: bool,
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
        let Some(time) = items::break_time_for(player.inventory.held(), hit.block, player.is_creative()) else {
            self.progress = 0.0;
            return events;
        };
        // «Сноровка»: +35 % mining speed per level.
        let eff = player.inventory.held().map(|s| s.ench_level(crate::magic::Ench::Efficiency)).unwrap_or(0);
        let mut time = time / (1.0 + 0.35 * eff as f32);
        // A tool digging a wide square (drill, draconic pickaxe, Tinkers' hammer) works a
        // little longer.
        let side = crate::tools::area_side(player.inventory.held().map(|s| s.id), player.drill_area);
        if side > 1 && time > 0.0 {
            time *= 1.0 + (side - 1) as f32 * 0.1;
        }
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
            let mut spilled = Vec::new();
            if self.break_block(world, player, hit.cell, hit.block, seed, &mut spilled) {
                events.extend(spilled.into_iter().map(|stack| InteractionEvent::Dropped { pos: hit.cell, stack }));
                events.push(InteractionEvent::Broken { pos: hit.cell, block: hit.block });
                if side > 1 {
                    self.drill_square(world, player, &hit, side, seed, &mut events);
                }
                // Lumber axe: the whole tree comes down.
                if player.inventory.held().is_some_and(|s| s.id == items::ids::LUMBER_AXE) && is_log(hit.block) {
                    self.fell_tree(world, player, hit.cell, seed, &mut events);
                }
                // A door is two blocks: the other half goes too (one door drops).
                if let Some((other, ob)) = door_partner(world, hit.cell, hit.block) {
                    if world.set_block(other, Block::Air) {
                        if hit.block.is_door_top() && !player.is_creative() {
                            let s = ItemStack::of_block(ob.base_item(), 1);
                            if self.spill {
                                events.push(InteractionEvent::Dropped { pos: other, stack: s });
                            } else {
                                player.inventory.add(s);
                            }
                        }
                        events.push(InteractionEvent::Broken { pos: other, block: ob });
                    }
                }
            }
        }
        events
    }

    /// The drill's wide mode: the square around the dug block, across the face that was hit
    /// (a wall is dug as a wall, a floor as a floor). Only what the drill can mine goes;
    /// the tool wears once per four extra blocks.
    fn drill_square(&mut self, world: &mut impl WorldEdit, player: &mut Player, hit: &Hit, side: i32, seed: u64, events: &mut Vec<InteractionEvent>) {
        let r = side / 2;
        let (a, b) = match hit.normal.0 {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let g = *world.grid();
        let step = |c: Cell, axis: usize, n: i32| -> Option<Cell> {
            let mut c = c;
            for _ in 0..n.abs() {
                c = g.neighbor(c, axis, n.signum())?;
            }
            Some(c)
        };
        let mut extra = 0u32;
        for da in -r..=r {
            for db in -r..=r {
                if da == 0 && db == 0 {
                    continue;
                }
                let Some(c) = step(hit.cell, a, da).and_then(|c| step(c, b, db)) else { continue };
                let blk = world.cell_block(c);
                if blk == Block::Air || blk == Block::Bedrock || blk.is_fluid() || blk.info().render == RenderKind::Invisible {
                    continue;
                }
                let held = player.inventory.held().map(|s| s.info());
                let Some(tool) = player.inventory.held().map(|s| s.id).filter(|&i| crate::tools::area_side(Some(i), player.drill_area) > 1) else { break };
                if !crate::tools::area_takes(tool, blk) || !items::can_harvest(held, blk) {
                    continue;
                }
                // Wear is paid below (per four blocks), not per block.
                let creative = player.is_creative();
                let mut spilled = Vec::new();
                let ok = self.break_block_ex(world, player, c, blk, seed, &mut spilled, false);
                if ok {
                    events.extend(spilled.into_iter().map(|stack| InteractionEvent::Dropped { pos: c, stack }));
                    events.push(InteractionEvent::Broken { pos: c, block: blk });
                    extra += 1;
                    if !creative && extra % 4 == 0 {
                        player.inventory.wear_selected();
                    }
                }
            }
        }
    }

    /// Lumber axe: every log joined to the cut one (also on the diagonals, for branches) goes,
    /// up to 160; the axe wears once per four logs.
    fn fell_tree(&mut self, world: &mut impl WorldEdit, player: &mut Player, start: Cell, seed: u64, events: &mut Vec<InteractionEvent>) {
        let g = *world.grid();
        let mut queue = std::collections::VecDeque::from([start]);
        let mut seen = std::collections::HashSet::from([start]);
        let mut cut = 0u32;
        while let Some(c) = queue.pop_front() {
            for di in -1..=1i32 {
                for dj in -1..=1i32 {
                    for dk in -1..=1i32 {
                        if di == 0 && dj == 0 && dk == 0 || cut >= 160 {
                            continue;
                        }
                        let mut n = Some(c);
                        for (axis, d) in [(0usize, di), (1, dj), (2, dk)] {
                            if d != 0 {
                                n = n.and_then(|x| g.neighbor(x, axis, d));
                            }
                        }
                        let Some(n) = n else { continue };
                        if !seen.insert(n) {
                            continue;
                        }
                        let b = world.cell_block(n);
                        if !is_log(b) {
                            continue;
                        }
                        let mut spilled = Vec::new();
                        if self.break_block_ex(world, player, n, b, seed, &mut spilled, false) {
                            events.extend(spilled.into_iter().map(|stack| InteractionEvent::Dropped { pos: n, stack }));
                            events.push(InteractionEvent::Broken { pos: n, block: b });
                            cut += 1;
                            if !player.is_creative() && cut % 4 == 0 {
                                player.inventory.wear_selected();
                            }
                            queue.push_back(n);
                        }
                    }
                }
            }
        }
    }

    fn break_block(&mut self, world: &mut impl WorldEdit, player: &mut Player, pos: Cell, block: Block, seed: u64, spilled: &mut Vec<ItemStack>) -> bool {
        self.break_block_ex(world, player, pos, block, seed, spilled, true)
    }

    #[allow(clippy::too_many_arguments)]
    fn break_block_ex(&mut self, world: &mut impl WorldEdit, player: &mut Player, pos: Cell, block: Block, seed: u64, spilled: &mut Vec<ItemStack>, wear: bool) -> bool {
        if !world.set_block(pos, Block::Air) {
            return false;
        }
        if !player.is_creative() {
            let held = player.inventory.held().map(|s| s.info());
            let ok = items::can_harvest(held, block);
            // «Удача» on a pickaxe: ore loot (not the ore block itself) sometimes doubles.
            let fortune = player.inventory.held().map(|s| s.ench_level(crate::magic::Ench::Fortune)).unwrap_or(0);
            let roll = ((pos.i.wrapping_mul(73856093) ^ pos.j.wrapping_mul(19349663) ^ pos.k.wrapping_mul(83492791)) as u32 % 100) as u8;
            for mut d in items::drops(block, ok, IVec3::new(pos.i, pos.j, pos.k ^ ((pos.f as i32) << 24)), seed) {
                if fortune > 0 && d.id != block as items::ItemId && items::xp_for(block) > 0 && roll < 30 * fortune {
                    d.count = d.count.saturating_add(1 + (roll % fortune.max(1)));
                }
                if self.spill {
                    spilled.push(d);
                } else {
                    player.inventory.add(d);
                }
            }
            player.survival.add_xp(items::xp_for(block));
            player.survival.exhaust(0.025);
            // Own blocks wear the tool once, other blocks twice, instant ones never.
            for _ in 0..if wear { crate::tools::wear_for_block(player.inventory.held().map(|s| s.id), block) } else { 0 } {
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
        let held_id = player.inventory.held().map(|s| s.id);
        // Utility tools must act before a targeted machine opens its container.
        if held_id == Some(items::ids::SHIP_HAMMER) {
            events.push(InteractionEvent::Fire { weapon: items::ids::SHIP_HAMMER });
            return events;
        }
        if let Some(h) = hit {
            // Lighter on dynamite: it starts to burn.
            if held_id == Some(items::ids::LIGHTER) && h.block == Block::Dynamite {
                if !player.is_creative() {
                    player.inventory.wear_selected();
                }
                events.push(InteractionEvent::Ignite { pos: h.cell });
                return events;
            }
            // Incubator: buy and hatch a capsule egg.
            if !player.sneaking {
                let tier = match h.block {
                    Block::IncubatorCommon => Some(0),
                    Block::IncubatorRare => Some(1),
                    Block::IncubatorLegendary => Some(2),
                    _ => None,
                };
                if let Some(tier) = tier {
                    events.push(InteractionEvent::BuyEgg { tier });
                    return events;
                }
            }
            if !player.sneaking && crate::construction::entry(h.block as u16).is_some_and(|(e,_)|e.form==crate::construction::Form::Bed) {
                // Bed: the spawn point moves here; at night you sleep till the morning.
                let c = world.grid().cell_center(h.cell);
                player.spawn = c + crate::physics::up_at(c) * 0.6;
                let f = crate::player::day_fraction(player.world_time);
                let night = !(0.24..0.76).contains(&f);
                // (The game lays the player down and lets the night pass.)
                events.push(InteractionEvent::Slept { night, pos: h.cell });
                return events;
            }
            if !player.sneaking && crate::construction::entry(h.block as u16).is_some_and(|(e,_)|matches!(e.form,crate::construction::Form::Chair|crate::construction::Form::Sofa|crate::construction::Form::SofaCorner)) {
                events.push(InteractionEvent::Sat { pos: h.cell });
                return events;
            }
            if !player.sneaking {
                if crate::construction::entry(h.block as u16).is_some_and(|(e,_)|e.form==crate::construction::Form::ItemFrame) {events.push(InteractionEvent::FrameUse{pos:h.cell});return events;}
                if let Some(kind)=crate::construction::altar(h.block){events.push(InteractionEvent::ActivateBoss{pos:h.cell,kind});return events;}
            }
            // A hand crank: every tap turns it for a few seconds.
            if h.block == Block::HandCrank && !player.sneaking {
                events.push(InteractionEvent::Crank { pos: h.cell });
                return events;
            }
            if h.block == Block::BuilderTable && !player.sneaking {
                events.push(InteractionEvent::OpenBuilder);
                return events;
            }
            if h.block.base_kind() == Block::CraftingTable && !player.sneaking {
                events.push(InteractionEvent::OpenCrafting);
                return events;
            }
            // Switch: toggles; the caller re-evaluates the power network on `Placed`.
            if matches!(h.block.unwall(), Block::SwitchOff | Block::SwitchOn) && !player.sneaking {
                let new = h.block.same_mount(if h.block.unwall() == Block::SwitchOff { Block::SwitchOn } else { Block::SwitchOff });
                if world.set_block(h.cell, new) {
                    events.push(InteractionEvent::Placed { pos: h.cell, block: new });
                }
                return events;
            }
            // Button: pressed for a moment (the session releases it).
            if h.block.unwall() == Block::Button && !player.sneaking {
                let on = h.block.same_mount(Block::ButtonOn);
                if world.set_block(h.cell, on) {
                    events.push(InteractionEvent::Placed { pos: h.cell, block: on });
                }
                return events;
            }
            // Door: opens and closes, both halves together. A double door (the same kind of
            // door in the same position right beside it) swings with it.
            if h.block.door_toggle().is_some() && !player.sneaking {
                let mut leaves = vec![(h.cell, h.block)];
                let g = *world.grid();
                for (axis, sign) in [(0, -1), (0, 1), (1, -1), (1, 1)] {
                    if let Some(n) = g.neighbor(h.cell, axis, sign) {
                        if world.cell_block(n) == h.block {
                            leaves.push((n, h.block));
                            break;
                        }
                    }
                }
                for (cell, b) in leaves {
                    let partner = door_partner(world, cell, b);
                    if let Some(new) = b.door_toggle() {
                        if world.set_block(cell, new) {
                            events.push(InteractionEvent::Placed { pos: cell, block: new });
                        }
                    }
                    if let Some((other, ob)) = partner {
                        if let Some(on) = ob.door_toggle() {
                            if world.set_block(other, on) {
                                events.push(InteractionEvent::Placed { pos: other, block: on });
                            }
                        }
                    }
                }
                return events;
            }
            // Chest and electric furnace open their own window (sneak to place blocks instead).
            let furniture_container = crate::construction::is_container(h.block);
            if (furniture_container || (matches!(h.block.base_kind(), Block::Chest | Block::ElectricFurnace | Block::MachineCasing | Block::EnchantTable | Block::Battery | Block::CommandBlock | Block::Terminal | Block::Sign | Block::SignB) || crate::wolftech::kind(h.block).is_some() || crate::kinetic::part(h.block).is_some_and(crate::kinetic::is_machine))) && !player.sneaking {
                events.push(InteractionEvent::OpenContainer { pos: h.cell, block: h.block.base_kind() });
                return events;
            }
        }
        let Some(stack) = player.inventory.held() else { return events };
        // Capsule egg in hand: hatch it.
        if (items::ids::EGG_BASE..=items::ids::EGG_LAST).contains(&stack.id) {
            events.push(InteractionEvent::Hatch { tier: (stack.id - items::ids::EGG_BASE) as u8 });
            return events;
        }
        // Grenade launcher, bazooka, grenade, the star remote: the session fires.
        if crate::explosives::weapon(stack.id).is_some() || matches!(stack.id, items::ids::STAR_REMOTE | items::ids::STAR_HAMMER) {
            events.push(InteractionEvent::Fire { weapon: stack.id });
            return events;
        }
        // Armor in hand: put it on.
        if player.inventory.equip_selected() {
            events.push(InteractionEvent::Equipped);
            return events;
        }
        let info = stack.info();
        // Spawn soul: the creature appears on the block you point at.
        if let Some(kind) = items::soul_mob(stack.id) {
            if let Some(target) = hit.and_then(|h| h.front_cell(world.grid())) {
                if world.cell_block(target).is_replaceable() {
                    if !player.is_creative() {
                        player.inventory.consume_selected();
                    }
                    events.push(InteractionEvent::SpawnMob { pos: target, kind });
                }
            }
            return events;
        }
        // Bone meal: a young crop ripens at once; on grass it sprouts grass and flowers around.
        if stack.id == items::ids::BONE_MEAL {
            if let Some(h) = hit {
                let mut used = false;
                if let Some(ripe) = h.block.grown() {
                    used = world.set_block(h.cell, ripe);
                    if used {
                        events.push(InteractionEvent::Placed { pos: h.cell, block: ripe });
                    }
                } else if matches!(h.block, Block::Grass | Block::DryGrass | Block::SwampGrass) {
                    let g = *world.grid();
                    let plants = [Block::TallGrass, Block::TallGrass, Block::FlowerRed, Block::FlowerYellow, Block::TallGrass, Block::FlowerBlue];
                    let mut n = 0usize;
                    for (di, dj) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
                        let Some(c) = g.offset(h.cell, di, dj, 0) else { continue };
                        let Some(above) = g.neighbor(c, 2, 1) else { continue };
                        if matches!(world.cell_block(c), Block::Grass | Block::DryGrass | Block::SwampGrass) && world.cell_block(above) == Block::Air {
                            let plant = plants[(n as i32 + h.cell.i + h.cell.j).rem_euclid(plants.len() as i32) as usize];
                            if (di + dj + h.cell.i + h.cell.j).rem_euclid(3) != 2 && world.set_block(above, plant) {
                                events.push(InteractionEvent::Placed { pos: above, block: plant });
                                used = true;
                            }
                            n += 1;
                        }
                    }
                }
                if used && !player.is_creative() {
                    player.inventory.consume_selected();
                }
            }
            return events;
        }
        // Potions: drink (instant ones work at once, the rest run for a while); the empty
        // bottle comes back.
        if (items::ids::POTION_BASE..=items::ids::POTION_LAST).contains(&stack.id) {
            if let Some(e) = crate::magic::Effect::from_id((stack.id - items::ids::POTION_BASE) as u8 + 1) {
                player.apply_effect(e, 1, e.duration());
                if !player.is_creative() {
                    player.inventory.consume_selected();
                    player.inventory.add(items::ItemStack::new(items::ids::BOTTLE, 1));
                }
                events.push(InteractionEvent::Ate { hunger: 0.0 });
            }
            return events;
        }
        // Hoe: till dirt or grass with free air above into farmland.
        if (items::ids::HOE_BASE..=items::ids::HOE_LAST).contains(&stack.id) {
            if let Some(h) = hit {
                let tillable = matches!(h.block, Block::Dirt | Block::Grass | Block::SnowyGrass | Block::DryGrass | Block::SwampGrass | Block::Podzol | Block::MossBlock);
                let above = world.grid().neighbor(h.cell, 2, 1);
                let free = above.is_some_and(|a| world.cell_block(a).is_replaceable() && !world.cell_block(a).is_fluid());
                if tillable && free {
                    if let Some(a) = above {
                        if world.cell_block(a) != Block::Air {
                            world.set_block(a, Block::Air);
                        }
                    }
                    if world.set_block(h.cell, Block::Farmland) {
                        if !player.is_creative() {
                            player.inventory.wear_selected();
                        }
                        events.push(InteractionEvent::Placed { pos: h.cell, block: Block::Farmland });
                    }
                }
            }
            return events;
        }
        // Seeds, carrots and potatoes are planted on top of farmland (elsewhere carrots and
        // potatoes are simply eaten).
        if let Some(crop) = info.place.filter(|b| b.crop().is_some()) {
            if let Some(h) = hit.filter(|h| h.block == Block::Farmland && h.normal == (2, 1)) {
                if let Some(target) = h.front_cell(world.grid()) {
                    if world.cell_block(target) == Block::Air && world.set_block(target, crop) {
                        if !player.is_creative() {
                            player.inventory.consume_selected();
                        }
                        events.push(InteractionEvent::Placed { pos: target, block: crop });
                    }
                }
                return events;
            }
            if info.food.is_none() {
                return events;
            }
        }
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
            // A log's end faces the clicked block. Across a planet face boundary, the
            // destination cell can have a different local axis from the hit cell.
            if block.log_on_axis(2).is_some() {
                let axis = (0..3).find(|&axis| {
                    [-1, 1].into_iter().any(|sign| g.neighbor(target, axis, sign) == Some(h.cell))
                }).unwrap_or(h.normal.0);
                block.log_on_axis(axis).unwrap_or(block)
            } else if let Some(crate::block::Part::Piston { sticky, .. }) = block.part() {
                // Pistons face the player, including up and down.
                let ek = (g.point(target.f, target.i as f64, target.j as f64, target.k as f64 + 1.0) - p0).normalize_or_zero();
                let l = player.look_dir();
                let (li, lj, lk) = (l.dot(ei), l.dot(ej), l.dot(ek));
                let dir = if lk.abs() >= li.abs() && lk.abs() >= lj.abs() {
                    if lk > 0.0 { 5 } else { 4 }
                } else if li.abs() >= lj.abs() {
                    if li > 0.0 { 1 } else { 0 }
                } else if lj > 0.0 {
                    3
                } else {
                    2
                };
                crate::block::Part::Piston { sticky, extended: false, dir }.block()
            } else if matches!(crate::kinetic::part(block), Some(crate::kinetic::Part::Shaft(_) | crate::kinetic::Part::Cog(_))) && {
                let ek = (g.point(target.f, target.i as f64, target.j as f64, target.k as f64 + 1.0) - p0).normalize_or_zero();
                player.look_dir().dot(ek).abs() > 0.75
            } {
                // Looking steeply up or down: a vertical shaft or a lying cogwheel.
                crate::kinetic::with_axis(block, 2)
            } else if h.normal.0 != 2 && (block.on_wall(0) != block || is_wall_decor(block)) {
                // Clicked the side of a block: torches, buttons, levers, pictures and mirrors
                // hang on that wall (it is on the opposite side of the new cell).
                let dir = match (h.normal.0, h.normal.1) {
                    (0, s) if s < 0 => 0,
                    (0, _) => 1,
                    (_, s) if s < 0 => 2,
                    _ => 3,
                };
                if block.on_wall(0) != block {
                    block.on_wall(dir)
                } else {
                    let (wi, wj) = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)][dir as usize];
                    block.oriented(wi, wj)
                }
            } else {
                block.oriented(player.heading.dot(ei), player.heading.dot(ej))
            }
        };
        let block = match h.normal {
            (2, -1) => block.inverted_stair().upper_slab(),
            (0 | 1, _) if h.height > 0.5 => block.upper_slab(),
            _ => block,
        };
        if !world.cell_block(target).is_replaceable() {
            return events;
        }
        if block.is_solid() && body_overlaps_cell(world.grid(), player.body.pos, target) {
            return events;
        }
        // A door needs a free block above for its upper half.
        let door_above = block.door_top().map(|top| (world.grid().neighbor(target, 2, 1), top));
        if let Some((above, _)) = door_above {
            if !above.is_some_and(|a| world.cell_block(a).is_replaceable()) {
                return events;
            }
        }
        if world.set_block(target, block) {
            if let Some((Some(above), top)) = door_above {
                if world.set_block(above, top) {
                    events.push(InteractionEvent::Placed { pos: above, block: top });
                }
            }
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
        Hit { cell, normal, dist: 1.0, height: 0.5, block: world.block(cell) }
    }

    fn setup() -> (Mock, Player) {
        let mut w = Mock { grid: Grid::with_cells(64), blocks: HashMap::new() };
        w.blocks.insert(c(0, 50, 0), Block::Stone);
        w.blocks.insert(c(1, 50, 0), Block::Dirt);
        let p = Player::new(wp(0.5, 80.0, 0.5), GameMode::Survival);
        (w, p)
    }

    #[test]
    fn placing_stairs_underneath_reflects_the_shape() {
        use crate::construction::{self, Form};
        for stair in [Block::CobbleStairs, Block::PlankStairs, construction::block(Form::InnerStairs, 0, 0).unwrap()] {
            let (mut world, mut player) = setup();
            player.inventory.selected = 0;
            player.inventory.slots[0] = Some(ItemStack::of_block(stair, 2));
            let support = c(0, 50, 0);
            for normal in [UP, (2, -1)] {
                let h = hit(&world, support, normal);
                Interaction::default().use_item(&mut world, &mut player, Some(h));
            }
            let upright = world.block(c(0, 51, 0));
            let inverted = world.block(c(0, 49, 0));
            assert_ne!(upright, inverted);
            assert_eq!(inverted, upright.inverted_stair());
            assert_eq!(inverted.base_item(), stair);
            for (above, below) in upright.boxes().iter().zip(inverted.boxes()) {
                assert_eq!((below[2], below[5]), (1.0 - above[5], 1.0 - above[2]));
            }
        }
    }

    #[test]
    fn slabs_follow_the_clicked_face_and_height() {
        use crate::construction::{self, Form};
        let catalogue_slab = construction::block(Form::LowerSlab, 0, 0).unwrap();
        for slab in [Block::CobbleSlab, Block::PlankSlab, catalogue_slab] {
            for (normal, height, upper) in [
                (UP, 1.0, false),
                ((2, -1), 0.0, true),
                ((0, -1), 0.25, false),
                ((0, -1), 0.75, true),
            ] {
                let (mut world, mut player) = setup();
                player.inventory.selected = 0;
                player.inventory.slots[0] = Some(ItemStack::of_block(slab, 1));
                let h = Hit { height, ..hit(&world, c(0, 50, 0), normal) };
                let target = h.front_cell(&world.grid).unwrap();
                Interaction::default().use_item(&mut world, &mut player, Some(h));
                let placed = world.block(target);
                let shape = placed.shape().unwrap();
                assert_eq!((shape[2], shape[5]), if upper { (0.5, 1.0) } else { (0.0, 0.5) });
                assert_eq!(Block::from_id(placed as u16), placed);
                if slab != catalogue_slab {
                    assert_eq!(placed.base_item(), slab);
                }
            }
        }
    }

    #[test]
    fn logs_follow_the_clicked_face_and_drop_the_original_item() {
        for log in [Block::OakLog, Block::SpruceLog, Block::BirchLog, Block::AcaciaLog, Block::AlienLog] {
            for (axis, sign) in [(0, -1), (1, 1), UP] {
                let (mut world, mut player) = setup();
                player.inventory.selected = 0;
                player.inventory.slots[0] = Some(ItemStack::of_block(log, 1));
                let h = hit(&world, c(0, 50, 0), (axis, sign));
                let target = h.front_cell(&world.grid).unwrap();
                Interaction::default().use_item(&mut world, &mut player, Some(h));
                let placed = world.block(target);
                assert_eq!(placed.horizontal_log_axis(), (axis != 2).then_some(axis));
                assert_eq!(placed.base_item(), log);
                assert_eq!(Block::from_id(placed as u16), placed);
            }
        }
    }

    #[test]
    fn ship_hammer_acts_on_machines_before_opening_their_gui() {
        let (mut w, mut p) = setup();
        p.inventory.selected = 0;
        p.inventory.slots[0] = Some(ItemStack::new(ids::SHIP_HAMMER, 1));
        let cell = c(0, 50, 0);
        w.blocks.insert(cell, Block::MechPress);
        let h = hit(&w, cell, UP);
        let mut interaction = Interaction::default();
        let events = interaction.use_item(&mut w, &mut p, Some(h));
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], InteractionEvent::Fire { weapon: ids::SHIP_HAMMER }));
        assert_eq!(w.block(cell), Block::MechPress);
    }

    #[test]
    fn the_forge_repairs_joins_and_upgrades_and_fuel_burns() {
        let pick = ids::tool(2, 0); // iron pickaxe
        let dur = items::info(pick).durability;
        let worn = ItemStack { damage: (dur / 4) * 2, ..ItemStack::new(pick, 1) };
        // Repair with iron ingots: a quarter each.
        let (r, used) = forge_result(worn, ItemStack::new(ids::IRON_INGOT, 5)).unwrap();
        assert_eq!(used, 2);
        assert_eq!(r.damage, 0);
        // Join two worn pickaxes.
        let (j, _) = forge_result(worn, worn).unwrap();
        assert!(j.damage < worn.damage);
        // Upgrade with three diamonds.
        let (u, used) = forge_result(worn, ItemStack::new(ids::DIAMOND, 3)).unwrap();
        assert_eq!((u.id, used), (ids::tool(3, 0), 3));
        assert!(forge_result(ItemStack::new(ids::STICK, 1), ItemStack::new(ids::STICK, 1)).is_none());
        assert!(fuel_time(ids::COAL) > fuel_time(Block::OakPlanks as items::ItemId));
        assert_eq!(fuel_time(ids::DIAMOND), 0.0);
    }

    #[test]
    fn a_hoe_tills_soil_and_seeds_grow_only_on_farmland() {
        let (mut w, mut p) = setup();
        let mut it = Interaction::default();
        // Seeds on plain dirt: nothing.
        p.inventory.slots[0] = Some(ItemStack::new(ids::WHEAT_SEEDS, 3));
        p.inventory.selected = 0;
        let h = hit(&w, c(1, 50, 0), UP);
        it.use_item(&mut w, &mut p, Some(h));
        assert_eq!(w.block(c(1, 51, 0)), Block::Air, "no farmland, no planting");
        // Till with a hoe, then plant.
        p.inventory.slots[1] = Some(ItemStack::new(ids::HOE_BASE, 1));
        p.inventory.selected = 1;
        let h = hit(&w, c(1, 50, 0), UP);
        it.use_item(&mut w, &mut p, Some(h));
        assert_eq!(w.block(c(1, 50, 0)), Block::Farmland);
        assert_eq!(p.inventory.slots[1].unwrap().damage, 1, "the hoe wears");
        p.inventory.selected = 0;
        let h = hit(&w, c(1, 50, 0), UP);
        it.use_item(&mut w, &mut p, Some(h));
        assert_eq!(w.block(c(1, 51, 0)), Block::WheatYoung, "planted");
        assert_eq!(p.inventory.count(ids::WHEAT_SEEDS), 2);
        // A carrot away from farmland is eaten instead.
        p.survival.hunger = 10.0;
        p.inventory.slots[2] = Some(ItemStack::new(ids::CARROT, 1));
        p.inventory.selected = 2;
        let h = hit(&w, c(0, 50, 0), UP);
        it.use_item(&mut w, &mut p, Some(h));
        assert!(p.survival.hunger > 10.0, "ate the carrot");
        // Ripe wheat gives wheat and seeds.
        let d = items::drops(Block::Wheat, true, glam::IVec3::ZERO, 1);
        assert!(d.iter().any(|s| s.id == ids::WHEAT) && d.iter().any(|s| s.id == ids::WHEAT_SEEDS));
        assert_eq!(smelt_result(ids::POTATO), Some(ids::BAKED_POTATO));
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
    fn doors_are_two_blocks_that_open_and_break_together() {
        let (mut w, mut p) = setup();
        let mut it = Interaction::default();
        p.body.pos = wp(5.5, 51.0, 5.5);
        p.inventory.add(ItemStack::of_block(Block::Door, 1));
        let h = hit(&w, c(0, 50, 0), UP);
        let ev = it.use_item(&mut w, &mut p, Some(h));
        assert_eq!(ev.len(), 2, "lower and upper half placed: {ev:?}");
        let (lo, hi) = (w.block(c(0, 51, 0)), w.block(c(0, 52, 0)));
        assert!(lo.door_toggle().is_some() && !lo.is_door_top(), "lower half {lo:?}");
        assert_eq!(lo.door_top(), Some(hi), "upper half matches");
        // Open from the upper half: both halves turn.
        let top = hit(&w, c(0, 52, 0), UP);
        it.use_item(&mut w, &mut p, Some(top));
        assert_eq!(w.block(c(0, 51, 0)), lo.door_toggle().unwrap());
        assert_eq!(w.block(c(0, 52, 0)), hi.door_toggle().unwrap());
        // A second door right beside the first: both swing together (double door).
        let (lo, _) = (w.block(c(0, 51, 0)), ());
        p.inventory.add(ItemStack::of_block(Block::Door, 1));
        let h2 = hit(&w, c(1, 50, 0), UP);
        it.use_item(&mut w, &mut p, Some(h2));
        // Put it in the same state as the first one.
        if w.block(c(1, 51, 0)) != lo {
            w.blocks.insert(c(1, 51, 0), lo);
            w.blocks.insert(c(1, 52, 0), lo.door_top().unwrap());
        }
        let first = w.block(c(0, 51, 0));
        let h3 = hit(&w, c(0, 51, 0), UP);
        it.use_item(&mut w, &mut p, Some(h3));
        assert_eq!(w.block(c(0, 51, 0)), first.door_toggle().unwrap());
        assert_eq!(w.block(c(1, 51, 0)), first.door_toggle().unwrap(), "the other leaf swings too");
        assert_eq!(w.block(c(1, 52, 0)), first.door_toggle().unwrap().door_top().unwrap());
        // No room above: no door.
        w.blocks.insert(c(3, 52, 0), Block::Stone);
        p.inventory.add(ItemStack::of_block(Block::Door, 1));
        let h3 = hit(&w, c(3, 50, 0), UP);
        assert!(it.use_item(&mut w, &mut p, Some(h3)).is_empty());
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

/// The other half of a two-block door at `c` (its cell and block), if it is there.
pub fn door_partner(world: &impl WorldEdit, c: Cell, b: Block) -> Option<(Cell, Block)> {
    b.door_toggle()?;
    let other = world.grid().neighbor(c, 2, if b.is_door_top() { -1 } else { 1 })?;
    let ob = world.cell_block(other);
    (ob.door_toggle().is_some() && ob.is_door_top() != b.is_door_top()).then_some((other, ob))
}

/// Seconds of burning a fuel item gives in the furnace (0 = not a fuel).
pub fn fuel_time(id: items::ItemId) -> f32 {
    use items::ids;
    let b = |x: Block| x as items::ItemId;
    match id {
        ids::COAL => 80.0,
        ids::COKE => 160.0,
        ids::FIRE_CRYSTAL => 600.0,
        x if x == b(Block::CoalBlock) => 800.0,
        x if [Block::OakLog, Block::BirchLog, Block::SpruceLog, Block::AcaciaLog].iter().any(|l| b(*l) == x) => 15.0,
        x if [Block::OakPlanks, Block::BirchPlanks, Block::SprucePlanks, Block::AcaciaPlanks, Block::CraftingTable, Block::Chest, Block::Battery, Block::Bookshelf].iter().any(|l| b(*l) == x) => 15.0,
        ids::STICK => 5.0,
        x if x == b(Block::Peat) => 40.0,
        x if items::tool_shape(x).is_some_and(|(_, m)| m == 0) => 10.0,
        _ => 0.0,
    }
}

/// Forge: what the tool or armour in slot 0 becomes with what lies in slot 1.
/// * its material repairs it, a quarter of its durability per item;
/// * a second identical item: the two are joined (durability added up, plus 10 %);
/// * three items of the next material up upgrade it a tier (wood → stone → iron → diamond),
///   keeping its wear.
/// Returns (result, how many items slot 1 gives up).
pub fn forge_result(item: items::ItemStack, with: items::ItemStack) -> Option<(items::ItemStack, u8)> {
    use items::ids;
    // A «Книга чар» puts its enchantments on a tool, weapon, armour or the bow.
    if with.id == ids::ENCH_BOOK && item.count == 1 {
        let (e, changed) = crate::magic::combine(item.ench, with.ench, crate::magic::kind_of(item.id));
        return changed.then_some((items::ItemStack { ench: e, ..item }, 1));
    }
    let info = item.info();
    let dur = info.durability;
    if dur == 0 || item.count != 1 {
        return None;
    }
    let b = |x: Block| x as items::ItemId;
    // Material tier of a tool, a hoe or armour, and the item of the same kind one tier up.
    let (tier, up): (usize, Option<items::ItemId>) = if let Some((kind, m)) = items::tool_shape(item.id) {
        let m = m as usize;
        let up = (m < 3).then(|| if kind == 4 { ids::HOE_BASE + m as u16 + 1 } else { ids::tool(m as u16 + 1, kind as u16) });
        (m, up)
    } else if let Some(slot) = items::armor_slot(item.id).filter(|_| (ids::ARMOR_BASE..=ids::ARMOR_LAST).contains(&item.id)) {
        let m = ((item.id - ids::ARMOR_BASE) / 4) as usize;
        (m, (m < 3).then(|| ids::ARMOR_BASE + (m as u16 + 1) * 4 + slot as u16))
    } else {
        return None;
    };
    let tool_mats = [b(Block::OakPlanks), b(Block::Cobblestone), ids::IRON_INGOT, ids::DIAMOND];
    let armor_mats = [ids::LEATHER, ids::BRONZE_INGOT, ids::IRON_INGOT, ids::DIAMOND];
    let mats = if items::armor_slot(item.id).is_some() { armor_mats } else { tool_mats };
    if with.id == item.id && with.count == 1 {
        let left = |s: items::ItemStack| (dur - s.damage.min(dur)) as u32;
        let total = (left(item) + left(with) + dur as u32 / 10).min(dur as u32);
        return Some((items::ItemStack { damage: dur - total as u16, ..item }, 1));
    }
    if with.id == mats[tier] && item.damage > 0 {
        let per = (dur / 4).max(1);
        let need = item.damage.div_ceil(per).min(with.count as u16) as u8;
        let fixed = item.damage.saturating_sub(per * need as u16);
        return Some((items::ItemStack { damage: fixed, ..item }, need));
    }
    if tier < 3 && with.id == mats[tier + 1] && with.count >= 3 {
        let up = up?;
        let new_dur = items::info(up).durability;
        let worn = item.damage as f32 / dur as f32;
        return Some((items::ItemStack { id: up, count: 1, damage: (worn * new_dur as f32) as u16, ench: item.ench }, 3));
    }
    None
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
        ids::RAW_URANIUM => ids::URANIUM_INGOT,
        ids::IRON_DUST => ids::IRON_INGOT,
        ids::GOLD_DUST => ids::GOLD_INGOT,
        ids::COPPER_DUST => ids::COPPER_INGOT,
        ids::TIN_DUST => ids::TIN_INGOT,
        ids::SILVER_DUST => ids::SILVER_INGOT,
        ids::DRACONIUM_DUST => ids::DRACONIUM_INGOT,
        x if x == Block::DraconiumOre as items::ItemId => ids::DRACONIUM_INGOT,
        x if x == Block::CobaltOre as items::ItemId => ids::COBALT_INGOT,
        ids::POTATO => ids::BAKED_POTATO,
        ids::RAW_MEAT => ids::COOKED_MEAT,
        ids::RAW_FISH => ids::COOKED_FISH,
        x if x == Block::Cobblestone as items::ItemId => Block::Stone as items::ItemId,
        x if x == Block::Sand as items::ItemId => Block::Glass as items::ItemId,
        _ => return None,
    })
}


/// Construction pieces that hang on a wall (pictures, mirrors, frames, curtains, panels).
fn is_wall_decor(b: Block) -> bool {
    use crate::construction::Form::*;
    crate::construction::entry(b as u16).is_some_and(|(e, _)| matches!(e.form, ItemFrame | PaintingForest | PaintingSky | PaintingAbstract | Mirror | Curtain | Panel | Shelf))
}

/// Tree trunks (the lumber axe fells what is joined).
fn is_log(b: Block) -> bool {
    b.log_on_axis(2).is_some()
}
