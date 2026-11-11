//! Inventory (36 slots: 0..9 hotbar) and crafting recipes.

use crate::block::Block;
use crate::items::{ItemId, ItemStack, ids, info};
use std::sync::OnceLock;

pub const SLOTS: usize = 36;
pub const HOTBAR: usize = 9;

#[derive(Clone, Debug, PartialEq)]
pub struct Inventory {
    pub slots: [Option<ItemStack>; SLOTS],
    pub selected: usize,
    /// Second hand (shield). Swapped with the selected slot by F.
    pub offhand: Option<ItemStack>,
}

impl Default for Inventory {
    fn default() -> Self {
        Self { slots: [None; SLOTS], selected: 0, offhand: None }
    }
}

impl Inventory {
    pub fn held(&self) -> Option<ItemStack> {
        self.slots[self.selected]
    }

    /// Swaps the selected hotbar slot with the second hand.
    pub fn swap_offhand(&mut self) {
        std::mem::swap(&mut self.slots[self.selected], &mut self.offhand);
    }

    pub fn has_shield(&self) -> bool {
        self.offhand.is_some_and(|s| s.id == ids::SHIELD)
    }

    /// Adds a stack; returns what did not fit.
    pub fn add(&mut self, mut s: ItemStack) -> Option<ItemStack> {
        let max = s.info().max_stack;
        // Fill existing stacks first (hotbar first, then main).
        for slot in self.slots.iter_mut().flatten() {
            if s.count == 0 {
                break;
            }
            if slot.can_stack_with(&s) && slot.count < max {
                let n = (max - slot.count).min(s.count);
                slot.count += n;
                s.count -= n;
            }
        }
        for slot in self.slots.iter_mut() {
            if s.count == 0 {
                break;
            }
            if slot.is_none() {
                let n = max.min(s.count);
                *slot = Some(ItemStack { count: n, ..s });
                s.count -= n;
            }
        }
        (s.count > 0).then_some(s)
    }

    pub fn count(&self, id: ItemId) -> u32 {
        self.slots.iter().flatten().filter(|s| s.id == id).map(|s| s.count as u32).sum()
    }

    /// Removes up to `n` items of `id`; returns how many were removed.
    pub fn take(&mut self, id: ItemId, mut n: u32) -> u32 {
        let want = n;
        for slot in self.slots.iter_mut() {
            if n == 0 {
                break;
            }
            if let Some(s) = slot {
                if s.id == id {
                    let k = n.min(s.count as u32);
                    s.count -= k as u8;
                    n -= k;
                    if s.count == 0 {
                        *slot = None;
                    }
                }
            }
        }
        want - n
    }

    /// Uses one item from the selected slot (placing a block, eating). Returns the used stack.
    pub fn consume_selected(&mut self) -> Option<ItemStack> {
        let s = self.slots[self.selected].as_mut()?;
        let used = ItemStack { count: 1, ..*s };
        s.count -= 1;
        if s.count == 0 {
            self.slots[self.selected] = None;
        }
        Some(used)
    }

    /// Wears the held tool by one use; breaks it when exhausted.
    pub fn wear_selected(&mut self) {
        if let Some(s) = self.slots[self.selected].as_mut() {
            let dur = s.info().durability;
            if dur > 0 {
                s.damage += 1;
                if s.damage >= dur {
                    self.slots[self.selected] = None;
                }
            }
        }
    }

    /// Swaps/merges slot `a` into slot `b` (drag and drop).
    pub fn move_slot(&mut self, a: usize, b: usize) {
        if a == b || a >= SLOTS || b >= SLOTS {
            return;
        }
        match (self.slots[a], self.slots[b]) {
            (Some(x), Some(y)) if x.can_stack_with(&y) => {
                let max = y.info().max_stack;
                let n = (max - y.count).min(x.count);
                self.slots[b] = Some(ItemStack { count: y.count + n, ..y });
                self.slots[a] = if x.count > n { Some(ItemStack { count: x.count - n, ..x }) } else { None };
            }
            _ => self.slots.swap(a, b),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_none())
    }

    pub fn can_hold(&self, s: ItemStack) -> bool {
        let mut c = self.clone();
        c.add(s).is_none()
    }

    pub fn can_craft(&self, r: &Recipe, near_table: bool) -> bool {
        (!r.needs_table || near_table)
            && r.ingredients.iter().all(|(id, n)| self.count(*id) >= *n as u32)
            && {
                let mut c = self.clone();
                for (id, n) in &r.ingredients {
                    c.take(*id, *n as u32);
                }
                c.can_hold(ItemStack::new(r.output.0, r.output.1))
            }
    }

    pub fn craft(&mut self, r: &Recipe, near_table: bool) -> bool {
        if !self.can_craft(r, near_table) {
            return false;
        }
        for (id, n) in &r.ingredients {
            self.take(*id, *n as u32);
        }
        self.add(ItemStack::new(r.output.0, r.output.1));
        true
    }

    // ---- serialization (player.dat) ----
    pub fn to_bytes(&self, out: &mut Vec<u8>) {
        out.push(self.selected as u8);
        for s in &self.slots {
            match s {
                Some(s) => {
                    out.extend_from_slice(&s.id.to_le_bytes());
                    out.push(s.count);
                    out.extend_from_slice(&s.damage.to_le_bytes());
                }
                None => out.extend_from_slice(&[0xFF, 0xFF, 0, 0, 0]),
            }
        }
        // Off-hand, appended (older saves simply end before it).
        match &self.offhand {
            Some(s) => {
                out.extend_from_slice(&s.id.to_le_bytes());
                out.push(s.count);
                out.extend_from_slice(&s.damage.to_le_bytes());
            }
            None => out.extend_from_slice(&[0xFF, 0xFF, 0, 0, 0]),
        }
    }

    pub fn from_bytes(data: &[u8]) -> Option<(Inventory, usize)> {
        let need = 1 + SLOTS * 5;
        if data.len() < need {
            return None;
        }
        let mut inv = Inventory { selected: (data[0] as usize).min(HOTBAR - 1), ..Default::default() };
        for i in 0..SLOTS {
            let o = 1 + i * 5;
            let id = u16::from_le_bytes([data[o], data[o + 1]]);
            if id != 0xFFFF && crate::items::is_valid(id) && data[o + 2] > 0 {
                inv.slots[i] = Some(ItemStack { id, count: data[o + 2], damage: u16::from_le_bytes([data[o + 3], data[o + 4]]) });
            }
        }
        if let Some(o) = data.get(need..need + 5) {
            let id = u16::from_le_bytes([o[0], o[1]]);
            if id != 0xFFFF && crate::items::is_valid(id) && o[2] > 0 {
                inv.offhand = Some(ItemStack { id, count: o[2], damage: u16::from_le_bytes([o[3], o[4]]) });
            }
            return Some((inv, need + 5));
        }
        Some((inv, need))
    }
}

#[derive(Clone, Debug)]
pub struct Recipe {
    pub ingredients: Vec<(ItemId, u8)>,
    pub output: (ItemId, u8),
    pub needs_table: bool,
    pub category: &'static str,
    /// Exact pattern for the crafting grid (rows, `None` = empty). `None` = shapeless.
    pub shape: Option<Vec<Vec<Option<ItemId>>>>,
}

impl Recipe {
    pub fn name(&self) -> &'static str {
        info(self.output.0).name
    }
}

fn r(ing: &[(ItemId, u8)], out: (ItemId, u8), table: bool, cat: &'static str) -> Recipe {
    Recipe { ingredients: ing.to_vec(), output: out, needs_table: table, category: cat, shape: None }
}

/// Builds a pattern from strings: `M` = material, `S` = stick, anything else = empty.
fn pattern(rows: [&str; 3], mat: ItemId) -> Vec<Vec<Option<ItemId>>> {
    rows.iter().map(|r| r.chars().map(|c| match c { 'M' => Some(mat), 'S' => Some(ids::STICK), _ => None }).collect()).collect()
}

pub fn recipes() -> &'static [Recipe] {
    static R: OnceLock<Vec<Recipe>> = OnceLock::new();
    R.get_or_init(|| {
        let b = |x: Block| x as ItemId;
        let mut v = Vec::new();
        // Each log gives its own planks; any planks make sticks and a crafting table.
        for (log, planks) in [(Block::OakLog, Block::OakPlanks), (Block::BirchLog, Block::BirchPlanks), (Block::SpruceLog, Block::SprucePlanks), (Block::AcaciaLog, Block::AcaciaPlanks)] {
            v.push(r(&[(b(log), 1)], (b(planks), 4), false, "Дерево"));
        }
        for planks in [Block::BirchPlanks, Block::SprucePlanks, Block::AcaciaPlanks] {
            v.push(r(&[(b(planks), 2)], (ids::STICK, 4), false, "Дерево"));
            v.push(r(&[(b(planks), 4)], (b(Block::CraftingTable), 1), false, "Строительство"));
        }
        // Bow and arrows, wool, torches and new building blocks.
        v.push(r(&[(ids::STICK, 3), (ids::STRING, 3)], (ids::BOW, 1), true, "Инструменты"));
        v.push(r(&[(ids::FLINT, 1), (ids::STICK, 1)], (ids::ARROW, 4), false, "Инструменты"));
        v.push(r(&[(ids::STRING, 4)], (b(Block::Wool), 1), false, "Строительство"));
        v.push(r(&[(b(Block::Wool), 1), (b(Block::FlowerRed), 1)], (b(Block::RedWool), 1), false, "Строительство"));
        v.push(r(&[(b(Block::Wool), 1), (b(Block::FlowerBlue), 1)], (b(Block::BlueWool), 1), false, "Строительство"));
        v.push(r(&[(b(Block::Wool), 1), (b(Block::FlowerYellow), 1)], (b(Block::YellowWool), 1), false, "Строительство"));
        v.push(r(&[(ids::COAL, 1), (ids::STICK, 2)], (b(Block::Torch), 4), false, "Строительство"));
        v.push(r(&[(b(Block::Torch), 1), (ids::IRON_INGOT, 1)], (b(Block::Lantern), 1), true, "Строительство"));
        v.push(r(&[(b(Block::Granite), 4)], (b(Block::PolishedGranite), 4), true, "Строительство"));
        v.push(r(&[(b(Block::Marble), 4)], (b(Block::PolishedMarble), 4), true, "Строительство"));
        v.push(r(&[(b(Block::Stone), 1), (ids::COAL, 1)], (b(Block::SmoothStone), 1), true, "Плавка"));
        v.push(r(&[(b(Block::Sandstone), 4)], (b(Block::SandstoneBricks), 4), true, "Строительство"));
        v.push(r(&[(b(Block::OakPlanks), 6), (ids::LEATHER, 1)], (b(Block::Bookshelf), 1), true, "Строительство"));
        v.push(r(&[(ids::LEATHER, 5)], (ids::ARMOR_BASE + 1, 1), true, "Броня"));
        // Slabs, stairs, vertical slabs, doors, stained glass and panes, fences, walls, signs.
        for (mat, slab) in [(Block::Cobblestone, Block::CobbleSlab), (Block::StoneBricks, Block::StoneBrickSlab), (Block::Sandstone, Block::SandstoneSlab), (Block::Bricks, Block::BrickSlab), (Block::BirchPlanks, Block::BirchSlab), (Block::SprucePlanks, Block::SpruceSlab)] {
            v.push(r(&[(b(mat), 3)], (b(slab), 6), true, "Строительство"));
        }
        v.push(r(&[(b(Block::PlankSlab), 1)], (b(Block::PlankSlabVerticalA), 1), false, "Строительство"));
        v.push(r(&[(b(Block::BrickSlab), 1)], (b(Block::BrickSlabVerticalA), 1), false, "Строительство"));
        for (mat, stairs) in [(Block::Cobblestone, Block::CobbleStairs), (Block::OakPlanks, Block::PlankStairs), (Block::StoneBricks, Block::StoneBrickStairs), (Block::Sandstone, Block::SandstoneStairs)] {
            v.push(r(&[(b(mat), 6)], (b(stairs), 4), true, "Строительство"));
        }
        v.push(r(&[(b(Block::BirchPlanks), 3)], (b(Block::BirchDoor), 2), true, "Строительство"));
        v.push(r(&[(b(Block::SprucePlanks), 3)], (b(Block::SpruceDoor), 2), true, "Строительство"));
        v.push(r(&[(ids::IRON_INGOT, 3)], (b(Block::IronDoor), 1), true, "Строительство"));
        for (dye, glass, pane) in [(Block::FlowerRed, Block::RedGlass, Block::RedPaneA), (Block::FlowerBlue, Block::BlueGlass, Block::BluePaneA), (Block::Fern, Block::GreenGlass, Block::GreenPaneA), (Block::FlowerYellow, Block::YellowGlass, Block::YellowPaneA)] {
            v.push(r(&[(b(Block::Glass), 4), (b(dye), 1)], (b(glass), 4), false, "Строительство"));
            v.push(r(&[(b(glass), 3)], (b(pane), 8), true, "Строительство"));
        }
        v.push(r(&[(b(Block::BirchPlanks), 2), (ids::STICK, 2)], (b(Block::BirchFence), 3), true, "Строительство"));
        v.push(r(&[(b(Block::SprucePlanks), 2), (ids::STICK, 2)], (b(Block::SpruceFence), 3), true, "Строительство"));
        v.push(r(&[(b(Block::StoneBricks), 3)], (b(Block::StoneBrickWall), 6), true, "Строительство"));
        v.push(r(&[(b(Block::Sandstone), 3)], (b(Block::SandstoneWall), 6), true, "Строительство"));
        v.push(r(&[(b(Block::OakPlanks), 6), (ids::STICK, 1)], (b(Block::Sign), 3), true, "Строительство"));
        // Same material, different shapes: slabs are a row of three, doors a column of three,
        // walls two rows (shaped, so they never clash).
        for rec in v.iter_mut() {
            if rec.ingredients.len() != 1 || rec.output.0 >= crate::items::ITEM_BASE {
                continue;
            }
            let out = Block::from_u8(rec.output.0 as u8);
            let mat = rec.ingredients[0].0;
            let rows = match out {
                Block::StoneSlab | Block::PlankSlab | Block::CobbleSlab | Block::StoneBrickSlab | Block::SandstoneSlab | Block::BrickSlab | Block::BirchSlab | Block::SpruceSlab => ["MMM", "...", "..."],
                Block::Door | Block::BirchDoor | Block::SpruceDoor | Block::IronDoor => ["M..", "M..", "M.."],
                Block::StoneWall | Block::StoneBrickWall | Block::SandstoneWall => ["MMM", "MMM", "..."],
                _ => continue,
            };
            let n = rows.iter().map(|r| r.matches('M').count()).sum::<usize>() as u8;
            rec.ingredients = vec![(mat, n)];
            rec.shape = Some(pattern(rows, mat));
        }
        v.push(r(&[(b(Block::OakPlanks), 2)], (ids::STICK, 4), false, "Дерево"));
        v.push(r(&[(b(Block::OakPlanks), 4)], (b(Block::CraftingTable), 1), false, "Строительство"));
        v.push(r(&[(b(Block::OakPlanks), 8)], (b(Block::Chest), 1), true, "Строительство"));
        v.push(r(&[(b(Block::OakPlanks), 6), (ids::IRON_INGOT, 1)], (ids::SHIELD, 1), true, "Инструменты"));
        v.push(r(&[(b(Block::Stone), 4)], (b(Block::StoneBricks), 4), true, "Строительство"));
        v.push(r(&[(b(Block::Sand), 4)], (b(Block::Sandstone), 1), false, "Строительство"));
        v.push(r(&[(b(Block::Clay), 4)], (b(Block::Bricks), 1), true, "Строительство"));
        v.push(r(&[(ids::COAL, 1), (ids::STICK, 1)], (b(Block::Lamp), 2), false, "Строительство"));
        v.push(r(&[(ids::GOLD_INGOT, 9)], (b(Block::GoldBlock), 1), true, "Строительство"));
        // Metals. Bronze = copper + tin alloy.
        v.push(r(&[(ids::RAW_COPPER, 1), (ids::COAL, 1)], (ids::COPPER_INGOT, 1), true, "Плавка"));
        v.push(r(&[(ids::RAW_TIN, 1), (ids::COAL, 1)], (ids::TIN_INGOT, 1), true, "Плавка"));
        v.push(r(&[(ids::RAW_SILVER, 1), (ids::COAL, 1)], (ids::SILVER_INGOT, 1), true, "Плавка"));
        v.push(r(&[(ids::COPPER_INGOT, 3), (ids::TIN_INGOT, 1)], (ids::BRONZE_INGOT, 4), true, "Плавка"));
        v.push(r(&[(ids::COPPER_INGOT, 9)], (b(Block::CopperBlock), 1), true, "Строительство"));
        v.push(r(&[(ids::IRON_INGOT, 9)], (b(Block::IronBlock), 1), true, "Строительство"));
        v.push(r(&[(ids::DIAMOND, 9)], (b(Block::DiamondBlock), 1), true, "Строительство"));
        v.push(r(&[(ids::COAL, 9)], (b(Block::CoalBlock), 1), true, "Строительство"));
        v.push(r(&[(ids::BRONZE_INGOT, 9)], (b(Block::BronzeBlock), 1), true, "Строительство"));
        // Tech mod: casing, cable, generator, battery, lamp, electric furnace.
        v.push(r(&[(ids::IRON_INGOT, 4), (ids::TIN_INGOT, 2)], (b(Block::MachineCasing), 1), true, "Техника"));
        v.push(r(&[(ids::COPPER_INGOT, 1), (ids::COAL, 1)], (b(Block::Cable), 4), true, "Техника"));
        v.push(r(&[(b(Block::MachineCasing), 1), (ids::COPPER_INGOT, 3), (ids::COAL, 2)], (b(Block::Generator), 1), true, "Техника"));
        v.push(r(&[(b(Block::MachineCasing), 1), (ids::COPPER_INGOT, 2), (ids::SILVER_INGOT, 1)], (b(Block::Battery), 1), true, "Техника"));
        v.push(r(&[(b(Block::Glass), 1), (ids::SILVER_INGOT, 1), (b(Block::Cable), 1)], (b(Block::ElectricLamp), 2), true, "Техника"));
        v.push(r(&[(b(Block::Cable), 2), (ids::IRON_INGOT, 1), (b(Block::Cobblestone), 1)], (b(Block::SwitchOff), 1), true, "Техника"));
        v.push(r(&[(b(Block::MachineCasing), 1), (b(Block::Cable), 2), (ids::COAL, 4)], (b(Block::ElectricFurnace), 1), true, "Техника"));
        v.push(r(&[(b(Block::Obsidian), 4), (b(Block::Glowstone), 1)], (b(Block::NetherPortal), 1), true, "Строительство"));
        v.push(r(&[(b(Block::Stone), 3)], (b(Block::StoneSlab), 6), true, "Строительство"));
        v.push(r(&[(b(Block::OakPlanks), 3)], (b(Block::PlankSlab), 6), true, "Строительство"));
        v.push(r(&[(b(Block::StoneSlab), 1)], (b(Block::StoneSlabVerticalA), 1), false, "Строительство"));
        v.push(r(&[(b(Block::StoneSlabVerticalA), 1)], (b(Block::StoneSlabVerticalB), 1), false, "Строительство"));
        v.push(r(&[(b(Block::Glass), 3)], (b(Block::GlassPaneA), 8), true, "Строительство"));
        v.push(r(&[(b(Block::GlassPaneA), 1)], (b(Block::GlassPaneB), 1), false, "Строительство"));
        v.push(r(&[(b(Block::OakPlanks), 2), (ids::STICK, 2)], (b(Block::Fence), 3), true, "Строительство"));
        v.push(r(&[(b(Block::Cobblestone), 3)], (b(Block::StoneWall), 6), true, "Строительство"));
        v.push(r(&[(b(Block::OakPlanks), 3)], (b(Block::Door), 2), true, "Строительство"));
        // Food and armor.
        v.push(r(&[(ids::RAW_MEAT, 1), (ids::COAL, 1)], (ids::COOKED_MEAT, 1), true, "Плавка"));
        let armor_mats: [ItemId; 4] = [ids::STICK, ids::BRONZE_INGOT, ids::IRON_INGOT, ids::DIAMOND];
        let armor_units = [5u8, 8, 7, 4];
        for (mi, mat) in armor_mats.iter().enumerate() {
            for si in 0..4usize {
                let (cnt, mid) = if mi == 0 { (armor_units[si] * 2, b(Block::OakPlanks)) } else { (armor_units[si], *mat) };
                v.push(r(&[(mid, cnt)], (ids::ARMOR_BASE + (mi * 4 + si) as u16, 1), true, "Броня"));
            }
        }
        // Smelting (needs a crafting table nearby; stands in for a furnace).
        v.push(r(&[(ids::RAW_IRON, 1), (ids::COAL, 1)], (ids::IRON_INGOT, 1), true, "Плавка"));
        v.push(r(&[(ids::RAW_GOLD, 1), (ids::COAL, 1)], (ids::GOLD_INGOT, 1), true, "Плавка"));
        v.push(r(&[(b(Block::Cobblestone), 1), (ids::COAL, 1)], (b(Block::Stone), 1), true, "Плавка"));
        v.push(r(&[(b(Block::Sand), 1), (ids::COAL, 1)], (b(Block::Glass), 2), true, "Плавка"));
        v.push(r(&[(b(Block::MushroomRed), 1), (b(Block::MushroomBrown), 1), (ids::APPLE, 1)], (ids::MUSHROOM_STEW, 1), false, "Еда"));
        v.push(r(&[(ids::APPLE, 2), (b(Block::Sand), 1)], (ids::BREAD, 2), false, "Еда"));
        // Tools: pickaxe 3, axe 3, shovel 1, sword 2 material units + sticks.
        let mats: [ItemId; 4] = [b(Block::OakPlanks), b(Block::Cobblestone), ids::IRON_INGOT, ids::DIAMOND];
        let units = [3u8, 3, 1, 2];
        let sticks = [2u8, 2, 2, 1];
        // Pickaxe and axe use the same amounts, so tools are shaped.
        let shapes = [["MMM", ".S.", ".S."], ["MM.", "MS.", ".S."], [".M.", ".S.", ".S."], [".M.", ".M.", ".S."]];
        for (mi, mat) in mats.iter().enumerate() {
            for ki in 0..4usize {
                let mut rec = r(&[(*mat, units[ki]), (ids::STICK, sticks[ki])], (ids::tool(mi as u16, ki as u16), 1), true, "Инструменты");
                rec.shape = Some(pattern(shapes[ki], *mat));
                v.push(rec);
            }
        }
        v
    })
}

/// Crafting grid shown in the survival inventory: 2×2 by hand, 3×3 at a crafting table.
/// Cells are stored row-major in a 3×3 array; a 2×2 grid uses the top-left corner.
#[derive(Clone, Debug, PartialEq)]
pub struct CraftGrid {
    pub cells: [Option<ItemStack>; 9],
    pub size: usize,
}

impl Default for CraftGrid {
    fn default() -> Self {
        Self { cells: [None; 9], size: 2 }
    }
}

impl CraftGrid {
    /// Indices of the cells in use for the current size.
    pub fn active(&self) -> Vec<usize> {
        (0..self.size).flat_map(|r| (0..self.size).map(move |c| r * 3 + c)).collect()
    }

    /// Changes the grid size; items in cells that disappear go back to the inventory.
    pub fn set_size(&mut self, size: usize, inv: &mut Inventory) {
        self.size = size.clamp(2, 3);
        let keep = self.active();
        for i in 0..9 {
            if !keep.contains(&i) {
                if let Some(s) = self.cells[i].take() {
                    inv.add(s);
                }
            }
        }
    }

    /// Moves everything back to the inventory. Returns what did not fit.
    pub fn return_all(&mut self, inv: &mut Inventory) -> Vec<ItemStack> {
        self.cells.iter_mut().filter_map(|c| c.take()).filter_map(|s| inv.add(s)).collect()
    }

    /// Grid contents trimmed to their bounding box.
    fn trimmed(&self) -> Vec<Vec<Option<ItemId>>> {
        let n = self.size;
        let id = |r: usize, c: usize| self.cells[r * 3 + c].map(|s| s.id);
        let rows: Vec<usize> = (0..n).filter(|&r| (0..n).any(|c| id(r, c).is_some())).collect();
        let cols: Vec<usize> = (0..n).filter(|&c| (0..n).any(|r| id(r, c).is_some())).collect();
        let (Some(&r0), Some(&r1), Some(&c0), Some(&c1)) = (rows.first(), rows.last(), cols.first(), cols.last()) else { return Vec::new() };
        (r0..=r1).map(|r| (c0..=c1).map(|c| id(r, c)).collect()).collect()
    }

    /// The recipe the grid currently forms, if any.
    pub fn matched(&self) -> Option<&'static Recipe> {
        let grid = self.trimmed();
        if grid.is_empty() {
            return None;
        }
        let mut counts: Vec<(ItemId, u8)> = Vec::new();
        for id in grid.iter().flatten().flatten() {
            match counts.iter_mut().find(|(i, _)| i == id) {
                Some((_, n)) => *n += 1,
                None => counts.push((*id, 1)),
            }
        }
        counts.sort();
        recipes().iter().find(|r| {
            if r.needs_table && self.size < 3 {
                return false;
            }
            match &r.shape {
                Some(shape) => {
                    let g = CraftGrid { cells: {
                        let mut c = [None; 9];
                        for (ri, row) in shape.iter().enumerate() {
                            for (ci, id) in row.iter().enumerate() {
                                c[ri * 3 + ci] = id.map(|id| ItemStack::new(id, 1));
                            }
                        }
                        c
                    }, size: 3 };
                    g.trimmed() == grid
                }
                None => {
                    let mut want = r.ingredients.clone();
                    want.sort();
                    want == counts
                }
            }
        })
    }

    /// Crafts once: uses one item from every filled cell and returns the result.
    pub fn craft_once(&mut self) -> Option<ItemStack> {
        let r = self.matched()?;
        for i in self.active() {
            if let Some(s) = &mut self.cells[i] {
                s.count -= 1;
                if s.count == 0 {
                    self.cells[i] = None;
                }
            }
        }
        Some(ItemStack::new(r.output.0, r.output.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_stacks_and_overflow() {
        let mut inv = Inventory::default();
        assert!(inv.add(ItemStack::of_block(Block::Dirt, 100)).is_none());
        assert_eq!(inv.count(Block::Dirt as ItemId), 100);
        assert_eq!(inv.slots[0].unwrap().count, 64);
        assert_eq!(inv.slots[1].unwrap().count, 36);
        for i in 0..SLOTS {
            inv.slots[i] = Some(ItemStack::of_block(Block::Stone, 64));
        }
        let rest = inv.add(ItemStack::of_block(Block::Dirt, 5));
        assert_eq!(rest.unwrap().count, 5);
    }

    #[test]
    fn crafting_consumes_and_produces() {
        let mut inv = Inventory::default();
        inv.add(ItemStack::of_block(Block::OakLog, 2));
        let planks = recipes().iter().find(|r| r.output.0 == Block::OakPlanks as ItemId && r.ingredients[0].0 == Block::OakLog as ItemId).unwrap();
        assert!(inv.craft(planks, false));
        assert_eq!(inv.count(Block::OakPlanks as ItemId), 4);
        assert_eq!(inv.count(Block::OakLog as ItemId), 1);
        let sticks = recipes().iter().find(|r| r.output.0 == ids::STICK && r.ingredients[0].0 == Block::OakPlanks as ItemId).unwrap();
        assert!(inv.craft(sticks, false));
        assert_eq!(inv.count(ids::STICK), 4);
        assert_eq!(inv.count(Block::OakPlanks as ItemId), 2);
        // A pickaxe needs the table.
        inv.add(ItemStack::of_block(Block::OakPlanks, 10));
        let pick = recipes().iter().find(|r| r.output.0 == ids::tool(0, 0)).unwrap();
        assert!(!inv.can_craft(pick, false));
        assert!(inv.craft(pick, true));
        assert_eq!(inv.count(ids::tool(0, 0)), 1);
    }

    #[test]
    fn all_recipes_reference_valid_items() {
        for r in recipes() {
            assert!(crate::items::is_valid(r.output.0), "{:?}", r);
            for (id, n) in &r.ingredients {
                assert!(crate::items::is_valid(*id) && *n > 0);
            }
        }
        assert!(recipes().len() > 30);
    }

    #[test]
    fn serialization_roundtrip() {
        let mut inv = Inventory::default();
        inv.add(ItemStack::of_block(Block::Lamp, 7));
        let mut tool = ItemStack::new(ids::tool(3, 0), 1);
        tool.damage = 12;
        inv.slots[5] = Some(tool);
        inv.selected = 5;
        let mut bytes = Vec::new();
        inv.to_bytes(&mut bytes);
        let (back, used) = Inventory::from_bytes(&bytes).unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(back, inv);
        assert!(Inventory::from_bytes(&bytes[..10]).is_none());
    }

    #[test]
    fn wear_breaks_tools() {
        let mut inv = Inventory::default();
        inv.slots[0] = Some(ItemStack::new(ids::tool(0, 0), 1));
        for _ in 0..58 {
            inv.wear_selected();
        }
        assert!(inv.slots[0].is_some());
        inv.wear_selected();
        assert!(inv.slots[0].is_none());
    }

    #[test]
    fn drag_and_drop_merges_and_swaps() {
        let mut inv = Inventory::default();
        inv.slots[0] = Some(ItemStack::of_block(Block::Dirt, 60));
        inv.slots[1] = Some(ItemStack::of_block(Block::Dirt, 10));
        inv.move_slot(1, 0);
        assert_eq!(inv.slots[0].unwrap().count, 64);
        assert_eq!(inv.slots[1].unwrap().count, 6);
        inv.slots[2] = Some(ItemStack::of_block(Block::Stone, 1));
        inv.move_slot(2, 1);
        assert_eq!(inv.slots[1].unwrap().id, Block::Stone as ItemId);
        assert_eq!(inv.slots[2].unwrap().id, Block::Dirt as ItemId);
    }

    #[test]
    fn grid_crafts_planks_by_hand_and_tools_by_shape() {
        let mut g = CraftGrid::default();
        g.cells[0] = Some(ItemStack::of_block(Block::OakLog, 2));
        assert_eq!(g.craft_once().unwrap(), ItemStack::of_block(Block::OakPlanks, 4));
        assert_eq!(g.cells[0].unwrap().count, 1);
        // Tools need the 3×3 grid.
        let mut inv = Inventory::default();
        g.cells[0] = None;
        let p = Some(ItemStack::of_block(Block::OakPlanks, 1));
        let s = Some(ItemStack::new(ids::STICK, 1));
        g.cells = [p, p, p, None, s, None, None, s, None];
        assert!(g.matched().is_none(), "2×2 grid hides the bottom rows");
        g.size = 3;
        assert_eq!(g.matched().unwrap().output.0, ids::tool(0, 0), "pickaxe");
        g.cells = [p, p, None, p, s, None, None, s, None];
        assert_eq!(g.matched().unwrap().output.0, ids::tool(0, 1), "axe");
        // Shapes may sit anywhere in the grid.
        g.cells = [None, None, p, None, None, p, None, None, s];
        assert_eq!(g.matched().unwrap().output.0, ids::tool(0, 3), "sword");
        g.set_size(2, &mut inv);
        assert_eq!(inv.count(Block::OakPlanks as ItemId), 2);
        assert_eq!(inv.count(ids::STICK), 1);
        g.return_all(&mut inv);
        assert!(g.cells.iter().all(|c| c.is_none()));
    }
}
