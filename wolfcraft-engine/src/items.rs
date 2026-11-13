//! Items, stacks, drops and mining rules.
//!
//! Legacy blocks keep IDs 0..255; plain items keep 256..350; construction blocks use 1024+.

use crate::block::{Block, RenderKind, ToolKind};
use crate::planet::{hash3, to01};
use glam::IVec3;

pub type ItemId = u16;
pub const ITEM_BASE: ItemId = 256;

pub mod ids {
    use super::ItemId;
    pub const STICK: ItemId = 256;
    pub const APPLE: ItemId = 257;
    pub const COAL: ItemId = 258;
    pub const RAW_IRON: ItemId = 259;
    pub const IRON_INGOT: ItemId = 260;
    pub const DIAMOND: ItemId = 261;
    pub const BREAD: ItemId = 262;
    pub const GOLD_INGOT: ItemId = 263;
    pub const RAW_GOLD: ItemId = 264;
    pub const MUSHROOM_STEW: ItemId = 265;
    /// 16 tools: TOOLS_BASE + material * 4 + kind (kind: pickaxe, axe, shovel, sword).
    pub const TOOLS_BASE: ItemId = 266;
    pub const TOOLS_LAST: ItemId = 281;
    pub const SHIELD: ItemId = 282;
    pub const RAW_COPPER: ItemId = 283;
    pub const RAW_TIN: ItemId = 284;
    pub const RAW_SILVER: ItemId = 285;
    pub const COPPER_INGOT: ItemId = 286;
    pub const TIN_INGOT: ItemId = 287;
    pub const BRONZE_INGOT: ItemId = 288;
    pub const SILVER_INGOT: ItemId = 289;
    pub const RAW_MEAT: ItemId = 290;
    pub const COOKED_MEAT: ItemId = 291;
    pub const ROTTEN_FLESH: ItemId = 292;
    /// Armor: ARMOR_BASE + material * 4 + slot (helmet, chestplate, leggings, boots); materials
    /// 0 leather, 1 bronze, 2 iron, 3 diamond.
    pub const ARMOR_BASE: ItemId = 293;
    pub const ARMOR_LAST: ItemId = 308;
    pub const BOW: ItemId = 309;
    pub const ARROW: ItemId = 310;
    pub const STRING: ItemId = 311;
    pub const FLINT: ItemId = 312;
    pub const LEATHER: ItemId = 313;
    pub const RAW_URANIUM: ItemId = 314;
    pub const URANIUM_INGOT: ItemId = 315;
    /// Hoes: HOE_BASE + material (wood, stone, iron, diamond).
    pub const HOE_BASE: ItemId = 316;
    pub const HOE_LAST: ItemId = 319;
    pub const WHEAT_SEEDS: ItemId = 320;
    pub const WHEAT: ItemId = 321;
    pub const CARROT: ItemId = 322;
    pub const POTATO: ItemId = 323;
    pub const BAKED_POTATO: ItemId = 324;
    pub const SLIME: ItemId = 325;
    /// Spawn souls: SOUL_BASE + index in `mobs::ALL_MOBS`.
    pub const SOUL_BASE: ItemId = 326;
    pub const SOUL_LAST: ItemId = 333;
    pub const BONE: ItemId = 334;
    pub const BONE_MEAL: ItemId = 335;
    /// A blank book: the Чаровальня turns it into a «Книга чар».
    pub const BOOK: ItemId = 336;
    /// A book holding enchantments (in the stack's `ench`), put on items at the forge.
    pub const ENCH_BOOK: ItemId = 337;
    pub const BOTTLE: ItemId = 338;
    /// Potions: POTION_BASE + effect id − 1 (see `magic::Effect`).
    pub const POTION_BASE: ItemId = 339;
    pub const POTION_LAST: ItemId = 347;
    /// Tipped arrows: poison, slowness, harm.
    pub const TIPPED_BASE: ItemId = 348;
    pub const TIPPED_LAST: ItemId = 350;
    pub const REVIVAL_HEART: ItemId = 351;
    pub const WINGS: ItemId = 352;
    pub const RITUAL_SEAL: ItemId = 353;
    pub const RESONANCE_RUNE: ItemId = 354;
    pub const VOLCANIC_CORE: ItemId = 355;
    // Explosives and weapons.
    pub const GUNPOWDER: ItemId = 356;
    pub const LIGHTER: ItemId = 357;
    pub const GRENADE: ItemId = 358;
    pub const GRENADE_LAUNCHER: ItemId = 359;
    pub const ROCKET: ItemId = 360;
    pub const BAZOOKA: ItemId = 361;
    pub const LAST: ItemId = 361;

    pub const fn tool(material: u16, kind: u16) -> ItemId {
        TOOLS_BASE + material * 4 + kind
    }
}

/// Which atlas/sprite draws the item icon.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    /// Sprite id in the blocks atlas.
    Block(u16),
    /// Sprite id in the items atlas.
    Item(u16),
}

#[derive(Clone, Copy, Debug)]
pub struct ItemInfo {
    pub name: &'static str,
    pub max_stack: u8,
    pub icon: Icon,
    pub place: Option<Block>,
    /// (tool kind, tier 0..3)
    pub tool: Option<(ToolKind, u8)>,
    pub attack: f32,
    /// (hunger restored, saturation restored)
    pub food: Option<(f32, f32)>,
    pub durability: u16,
}

const TOOL_NAMES: [&str; 16] = [
    "Деревянная кирка", "Деревянный топор", "Деревянная лопата", "Деревянный меч",
    "Каменная кирка", "Каменный топор", "Каменная лопата", "Каменный меч",
    "Железная кирка", "Железный топор", "Железная лопата", "Железный меч",
    "Алмазная кирка", "Алмазный топор", "Алмазная лопата", "Алмазный меч",
];
const ARMOR_NAMES: [&str; 16] = [
    "Кожаный шлем", "Кожаный нагрудник", "Кожаные поножи", "Кожаные ботинки",
    "Бронзовый шлем", "Бронзовый нагрудник", "Бронзовые поножи", "Бронзовые ботинки",
    "Железный шлем", "Железный нагрудник", "Железные поножи", "Железные ботинки",
    "Алмазный шлем", "Алмазный нагрудник", "Алмазные поножи", "Алмазные ботинки",
];
/// Defence points per slot (helmet, chest, legs, boots) for each material.
const ARMOR_POINTS: [[u8; 4]; 4] = [[1, 3, 2, 1], [2, 4, 3, 2], [2, 6, 5, 2], [3, 8, 6, 3]];

/// Defence points of an armor item (0 for anything else).
pub fn armor_points(id: ItemId) -> u8 {
    if (ids::ARMOR_BASE..=ids::ARMOR_LAST).contains(&id) {
        let n = (id - ids::ARMOR_BASE) as usize;
        ARMOR_POINTS[n / 4][n % 4]
    } else {
        0
    }
}

/// Armor slot (0 helmet … 3 boots) of an armor item.
pub fn armor_slot(id: ItemId) -> Option<usize> {
    if id == ids::WINGS { return Some(1); }
    (ids::ARMOR_BASE..=ids::ARMOR_LAST).contains(&id).then(|| ((id - ids::ARMOR_BASE) % 4) as usize)
}

const DURABILITY: [u16; 4] = [59, 131, 250, 1561];

fn plain(name: &'static str, icon: u16) -> ItemInfo {
    ItemInfo { name, max_stack: 64, icon: Icon::Item(icon), place: None, tool: None, attack: 1.0, food: None, durability: 0 }
}

pub fn info(id: ItemId) -> ItemInfo {
    use wolfcraft_assets::items as sp;
    if id < ITEM_BASE || id >= 1024 {
        let b = Block::from_id(id);
        let i = b.info();
        return ItemInfo { name: i.name, max_stack: if crate::construction::is_portable(b) {1}else{64}, icon: Icon::Block(i.tiles[0]), place: Some(b), tool: None, attack: 1.0, food: None, durability: 0 };
    }
    match id {
        ids::REVIVAL_HEART => ItemInfo {max_stack:1,..plain("Сердце возрождения",sp::REVIVAL_HEART)},
        ids::WINGS => ItemInfo {max_stack:1,durability:512,..plain("Крылья ветра",sp::WINGS)},
        ids::RITUAL_SEAL => plain("Печать древних",sp::RITUAL_SEAL),
        ids::RESONANCE_RUNE => plain("Руна резонанса",sp::RESONANCE_RUNE),
        ids::VOLCANIC_CORE => plain("Пепельное ядро",sp::VOLCANIC_CORE),
        ids::STICK => plain("Палка", sp::STICK),
        ids::APPLE => ItemInfo { food: Some((4.0, 2.4)), ..plain("Яблоко", sp::APPLE) },
        ids::COAL => plain("Уголь", sp::COAL),
        ids::RAW_IRON => plain("Сырое железо", sp::RAW_IRON),
        ids::IRON_INGOT => plain("Железный слиток", sp::IRON_INGOT),
        ids::DIAMOND => plain("Алмаз", sp::DIAMOND),
        ids::BREAD => ItemInfo { food: Some((5.0, 6.0)), ..plain("Хлеб", sp::BREAD) },
        ids::GOLD_INGOT => plain("Золотой слиток", sp::GOLD_INGOT),
        ids::RAW_GOLD => plain("Сырое золото", sp::RAW_GOLD),
        ids::MUSHROOM_STEW => ItemInfo { food: Some((6.0, 7.2)), max_stack: 1, ..plain("Грибное рагу", sp::MUSHROOM_STEW) },
        ids::RAW_COPPER => plain("Сырая медь", sp::RAW_COPPER),
        ids::RAW_TIN => plain("Сырое олово", sp::RAW_TIN),
        ids::RAW_SILVER => plain("Сырое серебро", sp::RAW_SILVER),
        ids::COPPER_INGOT => plain("Медный слиток", sp::COPPER_INGOT),
        ids::TIN_INGOT => plain("Оловянный слиток", sp::TIN_INGOT),
        ids::BRONZE_INGOT => plain("Бронзовый слиток", sp::BRONZE_INGOT),
        ids::SILVER_INGOT => plain("Серебряный слиток", sp::SILVER_INGOT),
        ids::RAW_MEAT => ItemInfo { food: Some((2.0, 1.2)), ..plain("Сырое мясо", sp::RAW_MEAT) },
        ids::COOKED_MEAT => ItemInfo { food: Some((8.0, 12.8)), ..plain("Жареное мясо", sp::COOKED_MEAT) },
        ids::ROTTEN_FLESH => ItemInfo { food: Some((2.0, 0.4)), ..plain("Гнилая плоть", sp::ROTTEN_FLESH) },
        a if (ids::ARMOR_BASE..=ids::ARMOR_LAST).contains(&a) => {
            let n = (a - ids::ARMOR_BASE) as usize;
            ItemInfo { name: ARMOR_NAMES[n], max_stack: 1, durability: 100 + (n / 4) as u16 * 120, ..plain(ARMOR_NAMES[n], sp::ARMOR_BASE + n as u16) }
        }
        ids::SHIELD => ItemInfo { max_stack: 1, durability: 336, ..plain("Щит", sp::SHIELD) },
        ids::GUNPOWDER => plain("Порох", sp::GUNPOWDER),
        ids::LIGHTER => ItemInfo { max_stack: 1, durability: 64, ..plain("Зажигалка", sp::LIGHTER) },
        ids::GRENADE => ItemInfo { max_stack: 16, ..plain("Граната", sp::GRENADE) },
        ids::GRENADE_LAUNCHER => ItemInfo { max_stack: 1, attack: 3.0, ..plain("Гранатомёт", sp::GRENADE_LAUNCHER) },
        ids::ROCKET => ItemInfo { max_stack: 16, ..plain("Ракета", sp::ROCKET) },
        ids::BAZOOKA => ItemInfo { max_stack: 1, attack: 3.0, ..plain("Базука", sp::BAZOOKA) },
        ids::BOW => ItemInfo { max_stack: 1, durability: 384, ..plain("Лук", sp::BOW) },
        ids::ARROW => plain("Стрела", sp::ARROW),
        ids::STRING => plain("Нить", sp::STRING),
        ids::FLINT => plain("Кремень", sp::FLINT),
        ids::LEATHER => plain("Кожа", sp::LEATHER),
        ids::RAW_URANIUM => plain("Сырой уран", sp::RAW_URANIUM),
        ids::URANIUM_INGOT => plain("Урановый слиток", sp::URANIUM_INGOT),
        h if (ids::HOE_BASE..=ids::HOE_LAST).contains(&h) => {
            let m = (h - ids::HOE_BASE) as usize;
            let name = ["Деревянная мотыга", "Каменная мотыга", "Железная мотыга", "Алмазная мотыга"][m];
            ItemInfo { max_stack: 1, durability: DURABILITY[m], ..plain(name, sp::HOE_BASE + m as u16) }
        }
        ids::WHEAT_SEEDS => ItemInfo { place: Some(Block::WheatYoung), ..plain("Семена пшеницы", sp::WHEAT_SEEDS) },
        ids::WHEAT => plain("Пшеница", sp::WHEAT),
        ids::CARROT => ItemInfo { place: Some(Block::CarrotYoung), food: Some((3.0, 3.6)), ..plain("Морковь", sp::CARROT) },
        ids::POTATO => ItemInfo { place: Some(Block::PotatoYoung), food: Some((1.0, 0.6)), ..plain("Картофель", sp::POTATO) },
        ids::BAKED_POTATO => ItemInfo { food: Some((5.0, 6.0)), ..plain("Печёный картофель", sp::BAKED_POTATO) },
        ids::SLIME => plain("Слизь", sp::SLIME),
        ids::BONE => plain("Кость", sp::BONE),
        ids::BONE_MEAL => plain("Костная мука", sp::BONE_MEAL),
        ids::BOOK => plain("Книга", sp::BOOK),
        ids::ENCH_BOOK => ItemInfo { max_stack: 1, ..plain("Книга чар", sp::ENCH_BOOK) },
        ids::BOTTLE => plain("Склянка", sp::BOTTLE),
        p if (ids::POTION_BASE..=ids::POTION_LAST).contains(&p) => {
            const NAMES: [&str; 9] = ["Зелье лечения", "Зелье регенерации", "Зелье скорости", "Зелье силы", "Зелье стойкости", "Зелье огнестойкости", "Зелье яда", "Зелье медлительности", "Зелье вреда"];
            let i = (p - ids::POTION_BASE) as usize;
            ItemInfo { max_stack: 16, ..plain(NAMES[i], sp::POTION_BASE + i as u16) }
        }
        a if (ids::TIPPED_BASE..=ids::TIPPED_LAST).contains(&a) => {
            const NAMES: [&str; 3] = ["Стрела яда", "Стрела медлительности", "Стрела вреда"];
            let i = (a - ids::TIPPED_BASE) as usize;
            plain(NAMES[i], sp::TIPPED_BASE + i as u16)
        }
        s if (ids::SOUL_BASE..=ids::SOUL_LAST).contains(&s) => {
            const NAMES: [&str; 8] = ["Душа спавна: Упырь", "Душа спавна: Ползун", "Душа спавна: Кабан", "Душа спавна: Овца", "Душа спавна: Корова", "Душа спавна: Курица", "Душа спавна: Волк", "Душа спавна: Слизень"];
            let i = (s - ids::SOUL_BASE) as usize;
            plain(NAMES[i], sp::SOUL_BASE + i as u16)
        }
        t if (ids::TOOLS_BASE..=ids::TOOLS_LAST).contains(&t) => {
            let n = (t - ids::TOOLS_BASE) as usize;
            let (mat, kind) = (n / 4, n % 4);
            let tool_kind = [ToolKind::Pickaxe, ToolKind::Axe, ToolKind::Shovel, ToolKind::Hand][kind];
            ItemInfo {
                name: TOOL_NAMES[n],
                max_stack: 1,
                icon: Icon::Item(sp::TOOLS_BASE + n as u16),
                place: None,
                tool: Some((tool_kind, mat as u8)),
                attack: if kind == 3 { 4.0 + mat as f32 } else { 2.0 + mat as f32 * 0.5 },
                food: None,
                durability: DURABILITY[mat],
            }
        }
        _ => plain("???", sp::STICK),
    }
}

/// Shape of a tool for the hand model: (kind 0 pickaxe, 1 axe, 2 shovel, 3 sword; material 0..3).
pub fn tool_shape(id: ItemId) -> Option<(u8, u8)> {
    // Hoes are kind 4 (drawn and swung like the other tools).
    if (ids::HOE_BASE..=ids::HOE_LAST).contains(&id) {
        return Some((4, (id - ids::HOE_BASE) as u8));
    }
    (ids::TOOLS_BASE..=ids::TOOLS_LAST).contains(&id).then(|| {
        let n = id - ids::TOOLS_BASE;
        ((n % 4) as u8, (n / 4) as u8)
    })
}

pub fn is_valid(id: ItemId) -> bool {
    (id < ITEM_BASE || id >= 1024) && Block::from_id(id) as u16 == id || (ITEM_BASE..=ids::LAST).contains(&id)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ItemStack {
    pub id: ItemId,
    pub count: u8,
    /// Wear (tools).
    pub damage: u16,
    /// Enchantments (see `magic`): up to four, one per byte; 0 = none.
    pub ench: u32,
}

impl ItemStack {
    pub fn new(id: ItemId, count: u8) -> Self {
        Self { id, count, damage: 0, ench: 0 }
    }
    /// Has enchantments (glows red).
    pub fn enchanted(&self) -> bool {
        self.ench != 0 && !crate::magic::list(self.ench).is_empty()
    }
    pub fn ench_level(&self, e: crate::magic::Ench) -> u8 {
        crate::magic::level(self.ench, e)
    }
    /// Name with the enchantments, for tooltips: "Алмазный меч (Острота II)".
    pub fn display_name(&self) -> String {
        let base = self.info().name;
        let l = crate::magic::list(self.ench);
        if l.is_empty() {
            return base.to_string();
        }
        let parts: Vec<String> = l.iter().map(|(e, lv)| format!("{} {}", e.name(), crate::magic::roman(*lv))).collect();
        format!("{base} ({})", parts.join(", "))
    }
    pub fn info(&self) -> ItemInfo {
        info(self.id)
    }
    pub fn of_block(b: Block, count: u8) -> Self {
        Self::new(b as ItemId, count)
    }
    pub fn can_stack_with(&self, o: &ItemStack) -> bool {
        self.id == o.id && self.info().max_stack > 1 && self.damage == o.damage && self.ench == o.ench
    }
}

/// Items dropped when `block` is broken. `harvested` = the tool requirement was met.
pub fn drops(block: Block, harvested: bool, pos: IVec3, seed: u64) -> Vec<ItemStack> {
    if !harvested {
        return vec![];
    }
    let roll = to01(hash3(seed ^ 0xD20, pos.x, pos.y, pos.z));
    match block {
        Block::Air | Block::Bedrock | Block::Magma | Block::InnerCore => vec![],
        b if b.is_fluid() => vec![],
        Block::Stone => vec![ItemStack::of_block(Block::Cobblestone, 1)],
        Block::Grass | Block::SnowyGrass | Block::DryGrass | Block::SwampGrass => vec![ItemStack::of_block(Block::Dirt, 1)],
        Block::OakLeaves | Block::BirchLeaves | Block::SpruceLeaves | Block::AcaciaLeaves => {
            if roll < 0.06 {
                vec![ItemStack::new(ids::APPLE, 1)]
            } else if roll < 0.16 {
                vec![ItemStack::new(ids::STICK, 1)]
            } else {
                vec![]
            }
        }
        Block::CoalOre => vec![ItemStack::new(ids::COAL, 1)],
        Block::IronOre => vec![ItemStack::new(ids::RAW_IRON, 1)],
        Block::GoldOre => vec![ItemStack::new(ids::RAW_GOLD, 1)],
        Block::DiamondOre => vec![ItemStack::new(ids::DIAMOND, 1)],
        Block::CopperOre => vec![ItemStack::new(ids::RAW_COPPER, 1)],
        Block::TinOre => vec![ItemStack::new(ids::RAW_TIN, 1)],
        Block::SilverOre => vec![ItemStack::new(ids::RAW_SILVER, 1)],
        Block::IskritOre => vec![ItemStack::of_block(Block::IskritWire, 4 + (roll * 2.0) as u8)],
        Block::UraniumOre => vec![ItemStack::new(ids::RAW_URANIUM, 1)],
        b if b.is_door_top() => vec![],
        b if matches!(b.part(), Some(crate::block::Part::Head { .. })) => vec![],
        Block::ElectricLampOn => vec![ItemStack::of_block(Block::ElectricLamp, 1)],
        Block::SwitchOn => vec![ItemStack::of_block(Block::SwitchOff, 1)],
        b if b.base_item() != b => vec![ItemStack::of_block(b.base_item(), 1)],
        // Farming: ripe crops give the harvest and seeds back, young ones only a seed.
        Block::Wheat => vec![ItemStack::new(ids::WHEAT, 1), ItemStack::new(ids::WHEAT_SEEDS, 1 + (roll * 3.0) as u8)],
        Block::WheatYoung => vec![ItemStack::new(ids::WHEAT_SEEDS, 1)],
        Block::Carrots => vec![ItemStack::new(ids::CARROT, 2 + (roll * 3.0) as u8)],
        Block::CarrotYoung => vec![ItemStack::new(ids::CARROT, 1)],
        Block::Potatoes => vec![ItemStack::new(ids::POTATO, 2 + (roll * 3.0) as u8)],
        Block::PotatoYoung => vec![ItemStack::new(ids::POTATO, 1)],
        Block::Farmland => vec![ItemStack::of_block(Block::Dirt, 1)],
        Block::Bones => vec![ItemStack::new(ids::BONE, 2 + (roll * 3.0) as u8)],
        // Wild grass now and then hides seeds, a carrot or a potato.
        Block::TallGrass | Block::Fern => {
            if roll < 0.14 {
                vec![ItemStack::new(ids::WHEAT_SEEDS, 1)]
            } else if roll < 0.17 {
                vec![ItemStack::new(ids::CARROT, 1)]
            } else if roll < 0.20 {
                vec![ItemStack::new(ids::POTATO, 1)]
            } else {
                vec![]
            }
        }
        Block::Glass | Block::Ice | Block::PackedIce | Block::Seagrass | Block::Kelp => vec![],
        Block::Snow => vec![ItemStack::of_block(Block::Snow, 1)],
        Block::Gravel if roll < 0.12 => vec![ItemStack::new(ids::FLINT, 1)],
        Block::Podzol | Block::MossBlock => vec![ItemStack::of_block(Block::Dirt, 1)],
        Block::Basalt => vec![ItemStack::of_block(Block::Basalt, 1)],
        b => vec![ItemStack::of_block(b, 1)],
    }
}

/// The creature a spawn soul brings to life.
pub fn soul_mob(id: ItemId) -> Option<crate::mobs::MobKind> {
    (ids::SOUL_BASE..=ids::SOUL_LAST).contains(&id).then(|| crate::mobs::ALL_MOBS[(id - ids::SOUL_BASE) as usize])
}

/// Colour of the gems in an ore (its shards fly out when it is mined), `None` for other blocks.
pub fn ore_glow(b: Block) -> Option<[f32; 3]> {
    Some(match b {
        Block::CoalOre => [0.30, 0.30, 0.34],
        Block::IronOre => [0.92, 0.74, 0.58],
        Block::GoldOre => [1.0, 0.85, 0.25],
        Block::DiamondOre => [0.45, 1.0, 0.95],
        Block::CopperOre => [1.0, 0.58, 0.32],
        Block::TinOre => [0.85, 0.88, 0.92],
        Block::SilverOre => [0.92, 0.95, 1.0],
        Block::CobaltOre => [0.30, 0.50, 1.0],
        Block::IskritOre => [1.0, 0.32, 0.78],
        Block::UraniumOre => [0.50, 1.0, 0.32],
        _ => return None,
    })
}

/// Loot dropped by a creature.
pub fn mob_drops(kind: crate::mobs::MobKind, salt: u64) -> Vec<ItemStack> {
    use crate::mobs::MobKind;
    let roll = to01(hash3(salt, 7, 11, 13));
    match kind {
        // Boars dig up roots: sometimes one carries a carrot or a potato.
        MobKind::Boar if roll > 0.75 => vec![ItemStack::new(ids::RAW_MEAT, 1), ItemStack::new(ids::LEATHER, 1), ItemStack::new(if roll > 0.88 { ids::CARROT } else { ids::POTATO }, 1)],
        MobKind::Boar => vec![ItemStack::new(ids::RAW_MEAT, 1 + (roll * 2.0) as u8), ItemStack::new(ids::LEATHER, 1)],
        MobKind::Sheep => vec![ItemStack::new(ids::RAW_MEAT, 1), ItemStack::of_block(Block::Wool, 1)],
        // Ghouls live on radiation: now and then one carries a lump of uranium.
        MobKind::Ghoul if roll < 0.12 => vec![ItemStack::new(ids::ROTTEN_FLESH, 1), ItemStack::new(ids::RAW_URANIUM, 1)],
        MobKind::Ghoul => vec![ItemStack::new(ids::ROTTEN_FLESH, 1 + (roll * 2.0) as u8), ItemStack::new(ids::COAL, 1)],
        MobKind::Crawler => vec![ItemStack::new(ids::STRING, 1 + (roll * 2.0) as u8)],
        MobKind::Cow => vec![ItemStack::new(ids::RAW_MEAT, 1 + (roll * 3.0) as u8), ItemStack::new(ids::LEATHER, 1 + (roll * 2.0) as u8)],
        MobKind::Chicken => vec![ItemStack::new(ids::RAW_MEAT, 1)],
        MobKind::Wolf => vec![ItemStack::new(ids::LEATHER, 1 + (roll * 2.0) as u8)],
        MobKind::Slime => vec![ItemStack::new(ids::SLIME, 1 + (roll * 2.0) as u8)],
    }
}

pub fn xp_for(block: Block) -> u32 {
    match block {
        Block::CoalOre => 2,
        Block::IronOre => 3,
        Block::GoldOre => 4,
        Block::DiamondOre => 8,
        Block::CopperOre | Block::TinOre => 2,
        Block::SilverOre | Block::CobaltOre => 5,
        Block::IskritOre => 4,
        _ => 0,
    }
}

/// Mining speed multiplier for a held item against a block.
pub fn tool_speed(held: Option<ItemInfo>, block: Block) -> f32 {
    let bi = block.info();
    match held.and_then(|h| h.tool) {
        Some((kind, tier)) if kind == bi.tool && kind != ToolKind::Hand => [2.0, 4.0, 6.0, 8.0][tier as usize & 3],
        _ => 1.0,
    }
}

pub fn can_harvest(held: Option<ItemInfo>, block: Block) -> bool {
    let bi = block.info();
    if !bi.needs_tool {
        return true;
    }
    matches!(held.and_then(|h| h.tool), Some((kind, tier)) if kind == bi.tool && tier >= bi.min_tier)
}

/// Seconds to break the block with the held item. `None` = cannot be broken.
pub fn break_time(held: Option<ItemInfo>, block: Block, creative: bool) -> Option<f32> {
    let bi = block.info();
    if bi.hardness < 0.0 || block.is_fluid() {
        return None;
    }
    if creative {
        return Some(0.0);
    }
    let mut t = bi.hardness / tool_speed(held, block);
    if bi.needs_tool && !can_harvest(held, block) {
        t *= 3.3;
    }
    Some(t)
}

/// Seconds to break `block` with the held stack, by the tool rules (see `tools`): the
/// tool's own blocks are fast, others take a bare hand's time, too low a material 3.3×
/// slower; a sword breaks nothing in creative. `None` = cannot be broken.
pub fn break_time_for(held: Option<ItemStack>, block: Block, creative: bool) -> Option<f32> {
    let bi = block.info();
    let id = held.map(|s| s.id);
    if bi.hardness < 0.0 || block.is_fluid() || !crate::tools::may_break(id, block, creative) {
        return None;
    }
    if creative {
        return Some(0.0);
    }
    let mut t = bi.hardness / crate::tools::speed(id, block);
    if bi.needs_tool && !can_harvest(held.map(|s| s.info()), block) {
        t *= 3.3;
    }
    Some(t)
}

/// Added in the latest update (marked "NEW" and listed first in the creative inventory):
/// blocks from the biome/building set on, the bow and its materials.
pub fn is_new(id: ItemId) -> bool {
    id >= 1024 || (98..ITEM_BASE).contains(&id) || (ids::BOW..=ids::LAST).contains(&id)
}

/// All blocks that make sense in the creative palette.
pub fn creative_palette() -> Vec<ItemId> {
    let mut v: Vec<ItemId> = Block::ALL
        .iter()
        .filter(|b| b.crop().is_none())
        // The electric machines are put away for now.
        .filter(|b| !matches!(b, Block::ElectricLamp))
        .filter(|b| !matches!(b, Block::Air | Block::Water | Block::Magma | Block::InnerCore | Block::Bedrock | Block::ElectricLampOn | Block::SwitchOn | Block::GlassPaneB | Block::StoneSlabVerticalB | Block::PlankSlabVerticalB | Block::BrickSlabVerticalB) && b.base_item() == **b)
        .map(|b| *b as ItemId)
        .collect();
    v.extend(ITEM_BASE..=ids::LAST);
    v
}

/// Finds an item by id ("7"), exact name, or unique substring of its (Russian) name.
pub fn find_item(query: &str) -> Option<ItemId> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    let all = creative_palette();
    if let Ok(n) = q.parse::<u16>() {
        return all.iter().copied().find(|id| *id == n);
    }
    if let Some(id) = all.iter().copied().find(|id| info(*id).name.to_lowercase() == q) {
        return Some(id);
    }
    // Fall back to the first item whose name starts with / contains the query.
    all.iter().copied().find(|id| info(*id).name.to_lowercase().starts_with(&q)).or_else(|| all.iter().copied().find(|id| info(*id).name.to_lowercase().contains(&q)))
}

/// Category used by the inventory UI tabs.
pub fn category(id: ItemId) -> &'static str {
    if let Some((e, _)) = crate::construction::entry(id) { return e.category; }
    if (ITEM_BASE..1024).contains(&id) {
        if armor_slot(id).is_some() {
            return "Доспехи";
        }
        if (ids::HOE_BASE..=ids::HOE_LAST).contains(&id) {
            return "Инструменты";
        }
        if (ids::SOUL_BASE..=ids::SOUL_LAST).contains(&id) {
            return "Мобы";
        }
        if matches!(id, ids::WHEAT_SEEDS | ids::WHEAT | ids::CARROT | ids::POTATO) {
            return "Растения";
        }
        if (ids::POTION_BASE..=ids::POTION_LAST).contains(&id) || matches!(id, ids::BOTTLE | ids::BOOK | ids::ENCH_BOOK) {
            return "Предметы";
        }
        if (ids::TIPPED_BASE..=ids::TIPPED_LAST).contains(&id) {
            return "Вооружение";
        }
        if matches!(id, ids::BOW | ids::ARROW | ids::SHIELD | ids::GUNPOWDER | ids::LIGHTER | ids::GRENADE | ids::GRENADE_LAUNCHER | ids::ROCKET | ids::BAZOOKA) || tool_shape(id).is_some_and(|(kind, _)| kind == 3) {
            return "Вооружение";
        }
        return if info(id).tool.is_some() { "Инструменты" } else { "Предметы" };
    }
    let b = Block::from_id(id);
    if b == Block::Dynamite {
        return "Вооружение";
    }
    if b.part().is_some() {
        return "Техника";
    }
    match b.info().render {
        RenderKind::Cross if b == Block::IskritTorch => "Техника",
        RenderKind::Cross if b != Block::Torch => "Растения",
        _ => match b {
            Block::IskritWire | Block::IskritBlock | Block::IskritLamp | Block::Button | Block::PressurePlate => "Техника",
            Block::BirchPlanks | Block::SprucePlanks | Block::AcaciaPlanks => "Дерево",
            b if !b.boxes().is_empty() || matches!(b, Block::RedGlass | Block::BlueGlass | Block::GreenGlass | Block::YellowGlass) => "Строительство",
            Block::Wool | Block::RedWool | Block::BlueWool | Block::YellowWool | Block::Torch | Block::PolishedGranite | Block::PolishedMarble | Block::SmoothStone | Block::SandstoneBricks | Block::Bookshelf | Block::Lantern => "Строительство",
            Block::Pumpkin | Block::Melon => "Растения",
            Block::EnchantTable => "Предметы",
            Block::Bones => "Мобы",
            Block::OakLog | Block::BirchLog | Block::SpruceLog | Block::AcaciaLog | Block::OakPlanks | Block::OakLeaves | Block::BirchLeaves | Block::SpruceLeaves | Block::AcaciaLeaves => "Дерево",
            Block::CoalOre | Block::IronOre | Block::GoldOre | Block::DiamondOre | Block::GoldBlock | Block::CopperOre | Block::TinOre | Block::SilverOre | Block::CobaltOre | Block::CopperBlock | Block::IronBlock | Block::DiamondBlock | Block::CoalBlock | Block::BronzeBlock => "Руды",
            Block::MachineCasing | Block::Battery | Block::ElectricFurnace => "Строительство",
            Block::ElectricLamp | Block::CommandBlock | Block::Terminal | Block::SwitchOff => "Техника",
            Block::IskritOre | Block::UraniumOre | Block::UraniumBlock => "Руды",
            Block::StoneBricks | Block::MossyStoneBricks | Block::CrackedStoneBricks | Block::Bricks | Block::Glass | Block::Lamp | Block::Chest | Block::CraftingTable => "Строительство",
            _ => "Природа",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapons_and_armour_have_their_own_tabs() {
        assert_eq!(category(ids::tool(2, 3)), "Вооружение", "sword");
        assert_eq!(category(ids::BOW), "Вооружение");
        assert_eq!(category(ids::ARROW), "Вооружение");
        assert_eq!(category(ids::SHIELD), "Вооружение");
        assert_eq!(category(ids::tool(2, 0)), "Инструменты", "pickaxe");
        assert_eq!(category(ids::ARMOR_BASE + 5), "Доспехи");
    }

    #[test]
    fn every_item_has_info_and_icon_in_range() {
        let blocks_n = wolfcraft_assets::procedural::sprites(wolfcraft_assets::AtlasKind::Blocks).len() as u16;
        let items_n = wolfcraft_assets::procedural::sprites(wolfcraft_assets::AtlasKind::Items).len() as u16;
        for id in creative_palette() {
            let i = info(id);
            match i.icon {
                Icon::Block(t) => assert!(t < blocks_n, "{} icon {}", i.name, t),
                Icon::Item(t) => assert!(t < items_n, "{} icon {}", i.name, t),
            }
            assert_ne!(i.name, "???");
        }
    }

    #[test]
    fn tools_need_right_tier() {
        let wood_pick = Some(info(ids::tool(0, 0)));
        let iron_pick = Some(info(ids::tool(2, 0)));
        assert!(can_harvest(wood_pick, Block::Stone));
        assert!(!can_harvest(wood_pick, Block::IronOre));
        assert!(can_harvest(iron_pick, Block::DiamondOre));
        assert!(!can_harvest(None, Block::Stone));
        assert!(can_harvest(None, Block::Dirt));
        assert!(break_time(iron_pick, Block::Stone, false).unwrap() < break_time(wood_pick, Block::Stone, false).unwrap());
        assert!(break_time(None, Block::Bedrock, false).is_none());
        assert_eq!(break_time(None, Block::Stone, true), Some(0.0));
    }

    #[test]
    fn find_item_by_name_id_and_prefix() {
        assert_eq!(find_item("камень"), Some(Block::Stone as ItemId));
        assert_eq!(find_item("  Лампа "), Some(Block::Lamp as ItemId));
        assert_eq!(find_item("алмаз"), Some(ids::DIAMOND));
        assert_eq!(find_item(&(Block::Dirt as u16).to_string()), Some(Block::Dirt as ItemId));
        assert_eq!(find_item("железная кир"), Some(ids::tool(2, 0)));
        assert_eq!(find_item("такого нет"), None);
        assert_eq!(find_item(""), None);
    }

    #[test]
    fn drops_follow_rules() {
        let p = IVec3::new(1, 2, 3);
        assert_eq!(drops(Block::Stone, true, p, 1)[0].id, Block::Cobblestone as ItemId);
        assert!(drops(Block::Stone, false, p, 1).is_empty());
        assert_eq!(drops(Block::DiamondOre, true, p, 1)[0].id, ids::DIAMOND);
        assert_eq!(drops(Block::Grass, true, p, 1)[0].id, Block::Dirt as ItemId);
    }
}

