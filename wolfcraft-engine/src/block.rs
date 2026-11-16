//! Block registry. Ids are persisted in save files: append new blocks, never reorder.

use wolfcraft_assets::tiles as t;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderKind {
    Invisible,
    /// Full opaque cube; hides neighbour faces.
    Opaque,
    /// Full cube with alpha-tested holes (leaves, glass).
    Cutout,
    /// Alpha-blended (water, ice), separate pass.
    Translucent,
    /// Two crossed quads (grass, flowers).
    Cross,
    /// A box smaller than the cell (slabs, panes, fences, doors): see `Block::shape`.
    Shape,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolKind {
    Hand,
    Pickaxe,
    Axe,
    Shovel,
}

#[derive(Clone, Copy, Debug)]
pub struct BlockInfo {
    pub name: &'static str,
    pub render: RenderKind,
    pub solid: bool,
    /// Seconds to break by hand; negative = unbreakable.
    pub hardness: f32,
    pub tool: ToolKind,
    /// Drops nothing unless mined with `tool` of at least `min_tier`.
    pub needs_tool: bool,
    /// 0 wood, 1 stone, 2 iron, 3 diamond.
    pub min_tier: u8,
    /// Texture per face: +X, -X, +Y, -Y, +Z, -Z (blocks atlas sprite ids).
    pub tiles: [u16; 6],
    pub emissive: bool,
}

const fn all(tile: u16) -> [u16; 6] {
    [tile; 6]
}
const fn column(side: u16, top: u16, bottom: u16) -> [u16; 6] {
    [side, side, top, bottom, side, side]
}

const fn info(name: &'static str, render: RenderKind, hardness: f32, tool: ToolKind, tiles: [u16; 6]) -> BlockInfo {
    BlockInfo {
        name,
        render,
        // Shaped blocks (slabs, stairs, panes, fences, walls, doors) collide with their own
        // boxes; the few that should be walked through (dust, buttons, signs) say not_solid.
        solid: matches!(render, RenderKind::Opaque | RenderKind::Cutout | RenderKind::Shape),
        hardness,
        tool,
        needs_tool: false,
        min_tier: 0,
        tiles,
        emissive: false,
    }
}
const fn ore(name: &'static str, tile: u16, tier: u8) -> BlockInfo {
    let mut i = info(name, RenderKind::Opaque, 15.0, ToolKind::Pickaxe, all(tile));
    i.needs_tool = true;
    i.min_tier = tier;
    i
}
const fn needs_pick(mut i: BlockInfo) -> BlockInfo {
    i.needs_tool = true;
    i
}
/// Only a pickaxe of at least `tier` (0 wood … 3 diamond) gets anything out of it.
const fn tier(mut i: BlockInfo, tier: u8) -> BlockInfo {
    i.needs_tool = true;
    i.min_tier = tier;
    i
}
const fn emissive(mut i: BlockInfo) -> BlockInfo {
    i.emissive = true;
    i
}
const fn not_solid(mut i: BlockInfo) -> BlockInfo {
    i.solid = false;
    i
}
const fn solid(mut i: BlockInfo) -> BlockInfo {
    i.solid = true;
    i
}

/// Tiles of a block facing direction `d` (0 +i, 1 -i, 2 +j, 3 -j, 4 up, 5 down): `front` on
/// that face, `back` on the opposite one, `side` elsewhere. Tile slots are (+j, -j, up, down,
/// +i, -i), as the mesher reads them.
const fn ptiles(front: u16, back: u16, side: u16, d: usize) -> [u16; 6] {
    const SLOT: [usize; 6] = [4, 5, 0, 1, 2, 3];
    let mut t = [side; 6];
    t[SLOT[d]] = front;
    t[SLOT[d ^ 1]] = back;
    t
}

/// WolfTech machine casing with its `front` facing `d` (0 +i, 1 -i, 2 +j, 3 -j).
const fn mtiles(front: u16, d: usize) -> [u16; 6] {
    const SLOT: [usize; 4] = [4, 5, 0, 1];
    let mut r = [t::TECH_SIDE; 6];
    r[2] = t::TECH_TOP;
    r[SLOT[d]] = front;
    r
}

macro_rules! blocks {
    ($( $id:literal $var:ident => $info:expr ),* $(,)?) => {
        #[repr(u16)]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
        pub enum Block {
            #[default]
            $( $var = $id ),*
        }
        pub const BLOCK_INFO: &[BlockInfo] = &[ $( $info ),* ];
        impl Block {
            pub const ALL: &'static [Block] = &[ $( Block::$var ),* ];
        }
    };
}

use RenderKind::*;
use ToolKind::*;

blocks! {
    0 Air => not_solid(info("Воздух", Invisible, 0.0, Hand, all(0))),
    1 Stone => needs_pick(info("Камень", Opaque, 7.5, Pickaxe, all(t::STONE))),
    2 Dirt => info("Земля", Opaque, 0.75, Shovel, all(t::DIRT)),
    3 Grass => info("Дёрн", Opaque, 0.9, Shovel, column(t::GRASS_SIDE, t::GRASS_TOP, t::DIRT)),
    4 Cobblestone => needs_pick(info("Булыжник", Opaque, 10.0, Pickaxe, all(t::COBBLESTONE))),
    5 Sand => info("Песок", Opaque, 0.75, Shovel, all(t::SAND)),
    6 Gravel => info("Гравий", Opaque, 0.9, Shovel, all(t::GRAVEL)),
    7 OakLog => info("Дубовое бревно", Opaque, 3.0, Axe, column(t::LOG_SIDE, t::LOG_TOP, t::LOG_TOP)),
    8 OakPlanks => info("Дубовые доски", Opaque, 3.0, Axe, all(t::PLANKS)),
    9 OakLeaves => info("Дубовая листва", Cutout, 0.35, Hand, all(t::LEAVES)),
    10 Water => not_solid(info("Вода", Translucent, -1.0, Hand, all(t::WATER))),
    11 Glass => info("Стекло", Cutout, 0.45, Hand, all(t::GLASS)),
    12 Snow => info("Снег", Opaque, 0.3, Shovel, all(t::SNOW)),
    13 SnowyGrass => info("Заснеженный дёрн", Opaque, 0.9, Shovel, column(t::SNOW_SIDE, t::SNOW, t::DIRT)),
    14 CoalOre => ore("Угольная руда", t::COAL_ORE, 0),
    15 IronOre => ore("Железная руда", t::IRON_ORE, 1),
    16 GoldOre => ore("Золотая руда", t::GOLD_ORE, 2),
    17 DiamondOre => ore("Алмазная руда", t::DIAMOND_ORE, 2),
    18 Bedrock => info("Коренная порода", Opaque, -1.0, Hand, all(t::BEDROCK)),
    19 Sandstone => needs_pick(info("Песчаник", Opaque, 4.0, Pickaxe, all(t::SANDSTONE))),
    20 CraftingTable => info("Верстак", Opaque, 3.75, Axe, column(t::TABLE_SIDE, t::TABLE_TOP, t::PLANKS)),
    21 Cactus => info("Кактус", Opaque, 0.6, Hand, column(t::CACTUS_SIDE, t::CACTUS_TOP, t::CACTUS_TOP)),
    22 TallGrass => info("Высокая трава", Cross, 0.0, Hand, all(t::TALL_GRASS)),
    23 FlowerRed => info("Красный цветок", Cross, 0.0, Hand, all(t::FLOWER_RED)),
    24 FlowerYellow => info("Жёлтый цветок", Cross, 0.0, Hand, all(t::FLOWER_YELLOW)),
    25 Ice => solid(info("Лёд", Translucent, 0.75, Pickaxe, all(t::ICE))),
    26 Clay => info("Глина", Opaque, 0.9, Shovel, all(t::CLAY)),
    27 Bricks => needs_pick(info("Кирпич", Opaque, 10.0, Pickaxe, all(t::BRICKS))),
    28 Lamp => emissive(info("Лампа", Opaque, 0.45, Hand, all(t::LAMP))),
    29 SpruceLog => info("Еловое бревно", Opaque, 3.0, Axe, column(t::SPRUCE_LOG_SIDE, t::LOG_TOP, t::LOG_TOP)),
    30 SpruceLeaves => info("Еловая хвоя", Cutout, 0.35, Hand, all(t::SPRUCE_LEAVES)),
    31 DeadBush => info("Сухой куст", Cross, 0.0, Hand, all(t::DEAD_BUSH)),
    32 StoneBricks => needs_pick(info("Каменные кирпичи", Opaque, 7.5, Pickaxe, all(t::STONE_BRICKS))),
    33 Lava => emissive(not_solid(info("Лава", Opaque, -1.0, Hand, all(t::LAVA)))),
    34 Basalt => needs_pick(info("Базальт", Opaque, 12.0, Pickaxe, column(t::BASALT_SIDE, t::BASALT_TOP, t::BASALT_TOP))),
    35 Obsidian => tier(info("Обсидиан", Opaque, 40.0, Pickaxe, all(t::OBSIDIAN)), 3),
    36 Mud => info("Грязь", Opaque, 0.9, Shovel, all(t::MUD)),
    37 MossyCobblestone => needs_pick(info("Замшелый булыжник", Opaque, 10.0, Pickaxe, all(t::MOSSY_COBBLESTONE))),
    38 BirchLog => info("Берёзовое бревно", Opaque, 3.0, Axe, column(t::BIRCH_LOG_SIDE, t::LOG_TOP, t::LOG_TOP)),
    39 BirchLeaves => info("Берёзовая листва", Cutout, 0.35, Hand, all(t::BIRCH_LEAVES)),
    40 AcaciaLog => info("Акациевое бревно", Opaque, 3.0, Axe, column(t::ACACIA_LOG_SIDE, t::LOG_TOP, t::LOG_TOP)),
    41 AcaciaLeaves => info("Акациевая листва", Cutout, 0.35, Hand, all(t::ACACIA_LEAVES)),
    42 MushroomRed => info("Красный гриб", Cross, 0.0, Hand, all(t::MUSHROOM_RED)),
    43 MushroomBrown => info("Бурый гриб", Cross, 0.0, Hand, all(t::MUSHROOM_BROWN)),
    44 Seagrass => info("Морская трава", Cross, 0.0, Hand, all(t::SEAGRASS)),
    45 Kelp => info("Ламинария", Cross, 0.0, Hand, all(t::KELP)),
    46 Shrub => info("Кустарник", Cross, 0.0, Hand, all(t::SHRUB)),
    47 DryGrass => info("Сухой дёрн", Opaque, 0.9, Shovel, column(t::DRY_GRASS_SIDE, t::DRY_GRASS_TOP, t::DIRT)),
    48 MossyStoneBricks => needs_pick(info("Замшелые кирпичи", Opaque, 7.5, Pickaxe, all(t::MOSSY_STONE_BRICKS))),
    49 CrackedStoneBricks => needs_pick(info("Треснувшие кирпичи", Opaque, 7.5, Pickaxe, all(t::CRACKED_STONE_BRICKS))),
    50 Chest => info("Сундук", Opaque, 3.0, Axe, [t::CHEST_SIDE, t::CHEST_SIDE, t::CHEST_TOP, t::CHEST_TOP, t::CHEST_FRONT, t::CHEST_SIDE]),
    51 Ash => info("Пепел", Opaque, 0.6, Shovel, all(t::ASH)),
    52 PackedIce => solid(info("Плотный лёд", Opaque, 1.2, Pickaxe, all(t::PACKED_ICE))),
    53 GoldBlock => needs_pick(info("Золотой блок", Opaque, 15.0, Pickaxe, all(t::GOLD_BLOCK))),
    54 RedSand => info("Красный песок", Opaque, 0.75, Shovel, all(t::RED_SAND)),
    55 Terracotta => needs_pick(info("Терракота", Opaque, 6.0, Pickaxe, all(t::TERRACOTTA))),
    56 SwampGrass => info("Болотный дёрн", Opaque, 0.9, Shovel, column(t::SWAMP_GRASS_SIDE, t::SWAMP_GRASS_TOP, t::DIRT)),
    57 Magma => emissive(info("Магма", Opaque, -1.0, Hand, all(t::MAGMA))),
    58 InnerCore => emissive(info("Внутреннее ядро", Opaque, -1.0, Hand, all(t::GOLD_BLOCK))),
    // Stones
    59 Granite => needs_pick(info("Гранит", Opaque, 8.0, Pickaxe, all(t::GRANITE))),
    60 Marble => needs_pick(info("Мрамор", Opaque, 8.0, Pickaxe, all(t::MARBLE))),
    61 Slate => needs_pick(info("Сланец", Opaque, 8.0, Pickaxe, all(t::SLATE))),
    62 Limestone => needs_pick(info("Известняк", Opaque, 6.0, Pickaxe, all(t::LIMESTONE))),
    // Ores (copper/tin: stone pickaxe, silver: iron, cobalt: iron)
    63 CopperOre => ore("Медная руда", t::COPPER_ORE, 1),
    64 TinOre => ore("Оловянная руда", t::TIN_ORE, 1),
    65 SilverOre => ore("Серебряная руда", t::SILVER_ORE, 2),
    66 CobaltOre => ore("Кобальтовая руда", t::COBALT_ORE, 2),
    // Metal blocks
    67 CopperBlock => needs_pick(info("Медный блок", Opaque, 15.0, Pickaxe, all(t::COPPER_BLOCK))),
    68 IronBlock => needs_pick(info("Железный блок", Opaque, 18.0, Pickaxe, all(t::IRON_BLOCK))),
    69 DiamondBlock => needs_pick(info("Алмазный блок", Opaque, 18.0, Pickaxe, all(t::DIAMOND_BLOCK))),
    70 CoalBlock => needs_pick(info("Угольный блок", Opaque, 8.0, Pickaxe, all(t::COAL_BLOCK))),
    71 BronzeBlock => needs_pick(info("Бронзовый блок", Opaque, 16.0, Pickaxe, all(t::BRONZE_BLOCK))),
    72 SaltBlock => needs_pick(info("Соляной блок", Opaque, 3.0, Pickaxe, all(t::SALT))),
    73 Peat => info("Торф", Opaque, 0.8, Shovel, all(t::PEAT)),
    74 Glowstone => emissive(info("Светокамень", Opaque, 0.6, Hand, all(t::GLOWSTONE))),
    // Tech
    // Was the machine casing: now the forge (repair and upgrade tools).
    75 MachineCasing => needs_pick(info("Кузница", Opaque, 5.0, Pickaxe, column(t::MACHINE_CASING, t::GENERATOR_FRONT, t::COBBLESTONE))),
    // The Чаровальня (enchanting station; was the generator): items + iskrit + levels → чары.
    76 EnchantTable => emissive(needs_pick(info("Чаровальня", Opaque, 5.0, Pickaxe, column(t::ENCHANT_SIDE, t::ENCHANT_TOP, t::OBSIDIAN)))),
    // A heap of bones left where a creature or a player died (was the copper cable).
    77 Bones => needs_pick(info("Кости", Shape, 1.5, Pickaxe, all(t::BONES))),
    // Was the battery: now a wooden barrel (storage like a chest).
    78 Battery => info("Бочка", Opaque, 2.5, Axe, column(t::BATTERY_SIDE, t::BATTERY_TOP, t::BATTERY_TOP)),
    79 ElectricLamp => emissive(info("Электролампа", Opaque, 0.5, Hand, all(t::ELECTRIC_LAMP))),
    // The furnace: burns fuel (coal, wood) or runs on power.
    80 ElectricFurnace => needs_pick(info("Печь", Opaque, 3.5, Pickaxe, [t::COBBLESTONE, t::COBBLESTONE, t::SMOOTH_STONE, t::COBBLESTONE, t::FURNACE_FRONT, t::COBBLESTONE])),
    81 ElectricLampOn => emissive(info("Электролампа (вкл.)", Opaque, 0.5, Hand, all(t::ELECTRIC_LAMP_ON))),
    82 SwitchOff => not_solid(info("Рычаг", Shape, 0.5, Hand, column(t::SWITCH_OFF, t::SWITCH_ON, t::COBBLESTONE))),
    83 SwitchOn => not_solid(info("Рычаг (вкл.)", Shape, 0.5, Hand, column(t::SWITCH_OFF, t::SWITCH_ON, t::COBBLESTONE))),
    84 CommandBlock => needs_pick(info("Командный блок", Opaque, 8.0, Pickaxe, all(t::COMMAND_BLOCK))),
    85 Terminal => needs_pick(info("Терминал", Opaque, 8.0, Pickaxe, all(t::TERMINAL))),
    86 Netherrack => needs_pick(info("Адский камень", Opaque, 2.0, Pickaxe, all(t::NETHERRACK))),
    87 NetherPortal => emissive(not_solid(info("Адский портал", Translucent, 1.0, Hand, all(t::PORTAL)))),
    88 StoneSlab => needs_pick(info("Каменная плита", Shape, 5.0, Pickaxe, all(t::STONE))),
    89 PlankSlab => info("Дощатая плита", Shape, 2.0, Axe, all(t::PLANKS)),
    90 StoneSlabVerticalA => needs_pick(info("Вертикальная плита (вдоль)", Shape, 5.0, Pickaxe, all(t::STONE))),
    91 StoneSlabVerticalB => needs_pick(info("Вертикальная плита (поперёк)", Shape, 5.0, Pickaxe, all(t::STONE))),
    92 GlassPaneA => info("Стеклянная панель (вдоль)", Shape, 0.4, Hand, all(t::GLASS)),
    93 GlassPaneB => info("Стеклянная панель (поперёк)", Shape, 0.4, Hand, all(t::GLASS)),
    94 Fence => info("Забор", Shape, 2.0, Axe, all(t::PLANKS)),
    95 StoneWall => needs_pick(info("Каменная стена", Shape, 5.0, Pickaxe, all(t::COBBLESTONE))),
    96 Door => info("Дверь", Shape, 2.0, Axe, all(t::DOOR_OAK)),
    97 DoorOpen => info("Дверь (открыта)", Shape, 2.0, Axe, all(t::DOOR_OAK)),
    // Biome and building blocks
    98 Wool => info("Белая шерсть", Opaque, 1.2, Hand, all(t::WOOL)),
    99 BirchPlanks => info("Берёзовые доски", Opaque, 3.0, Axe, all(t::BIRCH_PLANKS)),
    100 SprucePlanks => info("Еловые доски", Opaque, 3.0, Axe, all(t::SPRUCE_PLANKS)),
    101 AcaciaPlanks => info("Акациевые доски", Opaque, 3.0, Axe, all(t::ACACIA_PLANKS)),
    102 Torch => emissive(info("Факел", Cross, 0.0, Hand, all(t::TORCH))),
    103 Pumpkin => info("Тыква", Opaque, 1.5, Axe, column(t::PUMPKIN_SIDE, t::PUMPKIN_TOP, t::PUMPKIN_TOP)),
    104 Melon => info("Арбуз", Opaque, 1.5, Axe, column(t::MELON_SIDE, t::MELON_TOP, t::MELON_TOP)),
    105 SugarCane => info("Тростник", Cross, 0.0, Hand, all(t::SUGAR_CANE)),
    106 Fern => info("Папоротник", Cross, 0.0, Hand, all(t::FERN)),
    107 FlowerBlue => info("Синий цветок", Cross, 0.0, Hand, all(t::FLOWER_BLUE)),
    108 Podzol => info("Подзол", Opaque, 0.9, Shovel, column(t::PODZOL_SIDE, t::PODZOL_TOP, t::DIRT)),
    109 MossBlock => info("Мох", Opaque, 0.6, Hand, all(t::MOSS)),
    110 PolishedGranite => needs_pick(info("Полированный гранит", Opaque, 8.0, Pickaxe, all(t::POLISHED_GRANITE))),
    111 PolishedMarble => needs_pick(info("Полированный мрамор", Opaque, 8.0, Pickaxe, all(t::POLISHED_MARBLE))),
    112 SmoothStone => needs_pick(info("Гладкий камень", Opaque, 8.0, Pickaxe, all(t::SMOOTH_STONE))),
    113 SandstoneBricks => needs_pick(info("Песчаниковые кирпичи", Opaque, 4.0, Pickaxe, all(t::SANDSTONE_BRICKS))),
    114 Bookshelf => info("Книжная полка", Opaque, 2.0, Axe, column(t::BOOKSHELF, t::PLANKS, t::PLANKS)),
    115 Lantern => emissive(info("Фонарь", Opaque, 1.5, Pickaxe, all(t::LANTERN))),
    116 RedWool => info("Красная шерсть", Opaque, 1.2, Hand, all(t::WOOL_RED)),
    117 BlueWool => info("Синяя шерсть", Opaque, 1.2, Hand, all(t::WOOL_BLUE)),
    118 YellowWool => info("Жёлтая шерсть", Opaque, 1.2, Hand, all(t::WOOL_YELLOW)),
    // Slabs
    119 CobbleSlab => needs_pick(info("Булыжная плита", Shape, 5.0, Pickaxe, all(t::COBBLESTONE))),
    120 StoneBrickSlab => needs_pick(info("Плита из каменных кирпичей", Shape, 5.0, Pickaxe, all(t::STONE_BRICKS))),
    121 SandstoneSlab => needs_pick(info("Песчаниковая плита", Shape, 3.0, Pickaxe, all(t::SANDSTONE))),
    122 BrickSlab => needs_pick(info("Кирпичная плита", Shape, 5.0, Pickaxe, all(t::BRICKS))),
    123 BirchSlab => info("Берёзовая плита", Shape, 2.0, Axe, all(t::BIRCH_PLANKS)),
    124 SpruceSlab => info("Еловая плита", Shape, 2.0, Axe, all(t::SPRUCE_PLANKS)),
    125 PlankSlabVerticalA => info("Вертикальная дощатая плита", Shape, 2.0, Axe, all(t::PLANKS)),
    126 PlankSlabVerticalB => info("Вертикальная дощатая плита (поперёк)", Shape, 2.0, Axe, all(t::PLANKS)),
    127 BrickSlabVerticalA => needs_pick(info("Вертикальная кирпичная плита", Shape, 5.0, Pickaxe, all(t::BRICKS))),
    128 BrickSlabVerticalB => needs_pick(info("Вертикальная кирпичная плита (поперёк)", Shape, 5.0, Pickaxe, all(t::BRICKS))),
    // Stairs: four directions each (the base item is the first one)
    129 CobbleStairs => needs_pick(info("Булыжные ступени", Shape, 5.0, Pickaxe, all(t::COBBLESTONE))),
    130 CobbleStairs1 => needs_pick(info("Булыжные ступени", Shape, 5.0, Pickaxe, all(t::COBBLESTONE))),
    131 CobbleStairs2 => needs_pick(info("Булыжные ступени", Shape, 5.0, Pickaxe, all(t::COBBLESTONE))),
    132 CobbleStairs3 => needs_pick(info("Булыжные ступени", Shape, 5.0, Pickaxe, all(t::COBBLESTONE))),
    133 PlankStairs => info("Деревянные ступени", Shape, 2.0, Axe, all(t::PLANKS)),
    134 PlankStairs1 => info("Деревянные ступени", Shape, 2.0, Axe, all(t::PLANKS)),
    135 PlankStairs2 => info("Деревянные ступени", Shape, 2.0, Axe, all(t::PLANKS)),
    136 PlankStairs3 => info("Деревянные ступени", Shape, 2.0, Axe, all(t::PLANKS)),
    137 StoneBrickStairs => needs_pick(info("Ступени из каменных кирпичей", Shape, 5.0, Pickaxe, all(t::STONE_BRICKS))),
    138 StoneBrickStairs1 => needs_pick(info("Ступени из каменных кирпичей", Shape, 5.0, Pickaxe, all(t::STONE_BRICKS))),
    139 StoneBrickStairs2 => needs_pick(info("Ступени из каменных кирпичей", Shape, 5.0, Pickaxe, all(t::STONE_BRICKS))),
    140 StoneBrickStairs3 => needs_pick(info("Ступени из каменных кирпичей", Shape, 5.0, Pickaxe, all(t::STONE_BRICKS))),
    141 SandstoneStairs => needs_pick(info("Песчаниковые ступени", Shape, 3.0, Pickaxe, all(t::SANDSTONE))),
    142 SandstoneStairs1 => needs_pick(info("Песчаниковые ступени", Shape, 3.0, Pickaxe, all(t::SANDSTONE))),
    143 SandstoneStairs2 => needs_pick(info("Песчаниковые ступени", Shape, 3.0, Pickaxe, all(t::SANDSTONE))),
    144 SandstoneStairs3 => needs_pick(info("Песчаниковые ступени", Shape, 3.0, Pickaxe, all(t::SANDSTONE))),
    // Doors
    145 BirchDoor => info("Берёзовая дверь", Shape, 2.0, Axe, all(t::DOOR_BIRCH)),
    146 BirchDoorOpen => info("Берёзовая дверь (открыта)", Shape, 2.0, Axe, all(t::DOOR_BIRCH)),
    147 SpruceDoor => info("Еловая дверь", Shape, 2.0, Axe, all(t::DOOR_SPRUCE)),
    148 SpruceDoorOpen => info("Еловая дверь (открыта)", Shape, 2.0, Axe, all(t::DOOR_SPRUCE)),
    149 IronDoor => needs_pick(info("Железная дверь", Shape, 8.0, Pickaxe, all(t::DOOR_IRON))),
    150 IronDoorOpen => needs_pick(info("Железная дверь (открыта)", Shape, 8.0, Pickaxe, all(t::DOOR_IRON))),
    // Stained glass and panes
    151 RedGlass => info("Красное стекло", Cutout, 0.45, Hand, all(t::GLASS_RED)),
    152 BlueGlass => info("Синее стекло", Cutout, 0.45, Hand, all(t::GLASS_BLUE)),
    153 GreenGlass => info("Зелёное стекло", Cutout, 0.45, Hand, all(t::GLASS_GREEN)),
    154 YellowGlass => info("Жёлтое стекло", Cutout, 0.45, Hand, all(t::GLASS_YELLOW)),
    155 RedPaneA => info("Красная стеклянная панель", Shape, 0.4, Hand, all(t::GLASS_RED)),
    156 RedPaneB => info("Красная стеклянная панель (поперёк)", Shape, 0.4, Hand, all(t::GLASS_RED)),
    157 BluePaneA => info("Синяя стеклянная панель", Shape, 0.4, Hand, all(t::GLASS_BLUE)),
    158 BluePaneB => info("Синяя стеклянная панель (поперёк)", Shape, 0.4, Hand, all(t::GLASS_BLUE)),
    159 GreenPaneA => info("Зелёная стеклянная панель", Shape, 0.4, Hand, all(t::GLASS_GREEN)),
    160 GreenPaneB => info("Зелёная стеклянная панель (поперёк)", Shape, 0.4, Hand, all(t::GLASS_GREEN)),
    161 YellowPaneA => info("Жёлтая стеклянная панель", Shape, 0.4, Hand, all(t::GLASS_YELLOW)),
    162 YellowPaneB => info("Жёлтая стеклянная панель (поперёк)", Shape, 0.4, Hand, all(t::GLASS_YELLOW)),
    // Fences, walls, signs
    163 BirchFence => info("Берёзовый забор", Shape, 2.0, Axe, all(t::BIRCH_PLANKS)),
    164 SpruceFence => info("Еловый забор", Shape, 2.0, Axe, all(t::SPRUCE_PLANKS)),
    165 StoneBrickWall => needs_pick(info("Стена из каменных кирпичей", Shape, 5.0, Pickaxe, all(t::STONE_BRICKS))),
    166 SandstoneWall => needs_pick(info("Песчаниковая стена", Shape, 3.0, Pickaxe, all(t::SANDSTONE))),
    167 Sign => not_solid(info("Табличка", Shape, 1.0, Axe, all(t::PLANKS))),
    168 SignB => not_solid(info("Табличка (поперёк)", Shape, 1.0, Axe, all(t::PLANKS))),
    // Iskrit: the signal network (dust wire, sources, lamps).
    169 IskritOre => ore("Искритовая руда", t::ISKRIT_ORE, 2),
    170 IskritWire => not_solid(info("Искрит", Shape, 0.0, Hand, all(t::ISKRIT_WIRE))),
    171 IskritWireOn => emissive(not_solid(info("Искрит (заряжен)", Shape, 0.0, Hand, all(t::ISKRIT_WIRE_ON)))),
    172 IskritBlock => emissive(needs_pick(info("Блок искрита", Opaque, 5.0, Pickaxe, all(t::ISKRIT_BLOCK)))),
    173 IskritTorch => emissive(info("Искритовый факел", Cross, 0.0, Hand, all(t::ISKRIT_TORCH))),
    174 IskritLamp => info("Искритовая лампа", Opaque, 0.6, Hand, all(t::ISKRIT_LAMP)),
    175 IskritLampOn => emissive(info("Искритовая лампа (вкл.)", Opaque, 0.6, Hand, all(t::ISKRIT_LAMP_ON))),
    176 Button => not_solid(info("Кнопка", Shape, 0.5, Hand, all(t::BUTTON))),
    177 ButtonOn => not_solid(info("Кнопка (нажата)", Shape, 0.5, Hand, all(t::BUTTON))),
    // Flowing fluids (see `fluids.rs`): level 1..7 for water, 1..3 for lava, and falling columns.
    178 WaterFlow1 => not_solid(info("Вода (течёт)", Translucent, -1.0, Hand, all(t::WATER))),
    179 WaterFlow2 => not_solid(info("Вода (течёт)", Translucent, -1.0, Hand, all(t::WATER))),
    180 WaterFlow3 => not_solid(info("Вода (течёт)", Translucent, -1.0, Hand, all(t::WATER))),
    181 WaterFlow4 => not_solid(info("Вода (течёт)", Translucent, -1.0, Hand, all(t::WATER))),
    182 WaterFlow5 => not_solid(info("Вода (течёт)", Translucent, -1.0, Hand, all(t::WATER))),
    183 WaterFlow6 => not_solid(info("Вода (течёт)", Translucent, -1.0, Hand, all(t::WATER))),
    184 WaterFlow7 => not_solid(info("Вода (течёт)", Translucent, -1.0, Hand, all(t::WATER))),
    185 WaterFall => not_solid(info("Вода (падает)", Translucent, -1.0, Hand, all(t::WATER))),
    186 LavaFlow1 => emissive(not_solid(info("Лава (течёт)", Shape, -1.0, Hand, all(t::LAVA)))),
    187 LavaFlow2 => emissive(not_solid(info("Лава (течёт)", Shape, -1.0, Hand, all(t::LAVA)))),
    188 LavaFlow3 => emissive(not_solid(info("Лава (течёт)", Shape, -1.0, Hand, all(t::LAVA)))),
    189 LavaFall => emissive(not_solid(info("Лава (падает)", Opaque, -1.0, Hand, all(t::LAVA)))),
    // Upper halves of the two-block doors (window), same states as the lower halves.
    190 DoorTop => info("Дверь", Shape, 2.0, Axe, all(t::DOOR_OAK_TOP)),
    191 DoorTopOpen => info("Дверь (открыта)", Shape, 2.0, Axe, all(t::DOOR_OAK_TOP)),
    192 BirchDoorTop => info("Берёзовая дверь", Shape, 2.0, Axe, all(t::DOOR_BIRCH_TOP)),
    193 BirchDoorTopOpen => info("Берёзовая дверь (открыта)", Shape, 2.0, Axe, all(t::DOOR_BIRCH_TOP)),
    194 SpruceDoorTop => info("Еловая дверь", Shape, 2.0, Axe, all(t::DOOR_SPRUCE_TOP)),
    195 SpruceDoorTopOpen => info("Еловая дверь (открыта)", Shape, 2.0, Axe, all(t::DOOR_SPRUCE_TOP)),
    196 IronDoorTop => needs_pick(info("Железная дверь", Shape, 8.0, Pickaxe, all(t::DOOR_IRON_TOP))),
    197 IronDoorTopOpen => needs_pick(info("Железная дверь (открыта)", Shape, 8.0, Pickaxe, all(t::DOOR_IRON_TOP))),
    // Uranium: a glowing, radioactive ore deep underground.
    198 UraniumOre => emissive(ore("Урановая руда", t::URANIUM_ORE, 2)),
    199 UraniumBlock => emissive(needs_pick(info("Блок урана", Opaque, 12.0, Pickaxe, all(t::URANIUM_BLOCK)))),
    // Iskrit mechanisms (see `power.rs`). Directions: 0 +i, 1 -i, 2 +j, 3 -j, 4 up, 5 down.
    200 Piston0 => info("Поршень", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_FRONT, t::PISTON_BACK, t::PISTON_SIDE, 0)),
    201 Piston1 => info("Поршень", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_FRONT, t::PISTON_BACK, t::PISTON_SIDE, 1)),
    202 Piston2 => info("Поршень", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_FRONT, t::PISTON_BACK, t::PISTON_SIDE, 2)),
    203 Piston3 => info("Поршень", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_FRONT, t::PISTON_BACK, t::PISTON_SIDE, 3)),
    204 Piston4 => info("Поршень", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_FRONT, t::PISTON_BACK, t::PISTON_SIDE, 4)),
    205 Piston5 => info("Поршень", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_FRONT, t::PISTON_BACK, t::PISTON_SIDE, 5)),
    206 PistonOn0 => info("Поршень (выдвинут)", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_INNER, t::PISTON_BACK, t::PISTON_SIDE, 0)),
    207 PistonOn1 => info("Поршень (выдвинут)", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_INNER, t::PISTON_BACK, t::PISTON_SIDE, 1)),
    208 PistonOn2 => info("Поршень (выдвинут)", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_INNER, t::PISTON_BACK, t::PISTON_SIDE, 2)),
    209 PistonOn3 => info("Поршень (выдвинут)", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_INNER, t::PISTON_BACK, t::PISTON_SIDE, 3)),
    210 PistonOn4 => info("Поршень (выдвинут)", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_INNER, t::PISTON_BACK, t::PISTON_SIDE, 4)),
    211 PistonOn5 => info("Поршень (выдвинут)", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_INNER, t::PISTON_BACK, t::PISTON_SIDE, 5)),
    212 StickyPiston0 => info("Липкий поршень", Opaque, 1.5, Pickaxe, ptiles(t::STICKY_FRONT, t::PISTON_BACK, t::PISTON_SIDE, 0)),
    213 StickyPiston1 => info("Липкий поршень", Opaque, 1.5, Pickaxe, ptiles(t::STICKY_FRONT, t::PISTON_BACK, t::PISTON_SIDE, 1)),
    214 StickyPiston2 => info("Липкий поршень", Opaque, 1.5, Pickaxe, ptiles(t::STICKY_FRONT, t::PISTON_BACK, t::PISTON_SIDE, 2)),
    215 StickyPiston3 => info("Липкий поршень", Opaque, 1.5, Pickaxe, ptiles(t::STICKY_FRONT, t::PISTON_BACK, t::PISTON_SIDE, 3)),
    216 StickyPiston4 => info("Липкий поршень", Opaque, 1.5, Pickaxe, ptiles(t::STICKY_FRONT, t::PISTON_BACK, t::PISTON_SIDE, 4)),
    217 StickyPiston5 => info("Липкий поршень", Opaque, 1.5, Pickaxe, ptiles(t::STICKY_FRONT, t::PISTON_BACK, t::PISTON_SIDE, 5)),
    218 StickyPistonOn0 => info("Липкий поршень (выдвинут)", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_INNER, t::PISTON_BACK, t::PISTON_SIDE, 0)),
    219 StickyPistonOn1 => info("Липкий поршень (выдвинут)", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_INNER, t::PISTON_BACK, t::PISTON_SIDE, 1)),
    220 StickyPistonOn2 => info("Липкий поршень (выдвинут)", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_INNER, t::PISTON_BACK, t::PISTON_SIDE, 2)),
    221 StickyPistonOn3 => info("Липкий поршень (выдвинут)", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_INNER, t::PISTON_BACK, t::PISTON_SIDE, 3)),
    222 StickyPistonOn4 => info("Липкий поршень (выдвинут)", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_INNER, t::PISTON_BACK, t::PISTON_SIDE, 4)),
    223 StickyPistonOn5 => info("Липкий поршень (выдвинут)", Opaque, 1.5, Pickaxe, ptiles(t::PISTON_INNER, t::PISTON_BACK, t::PISTON_SIDE, 5)),
    224 PistonHead0 => info("Головка поршня", Shape, 1.5, Pickaxe, ptiles(t::PISTON_FRONT, t::PISTON_FRONT, t::PISTON_SIDE, 0)),
    225 PistonHead1 => info("Головка поршня", Shape, 1.5, Pickaxe, ptiles(t::PISTON_FRONT, t::PISTON_FRONT, t::PISTON_SIDE, 1)),
    226 PistonHead2 => info("Головка поршня", Shape, 1.5, Pickaxe, ptiles(t::PISTON_FRONT, t::PISTON_FRONT, t::PISTON_SIDE, 2)),
    227 PistonHead3 => info("Головка поршня", Shape, 1.5, Pickaxe, ptiles(t::PISTON_FRONT, t::PISTON_FRONT, t::PISTON_SIDE, 3)),
    228 PistonHead4 => info("Головка поршня", Shape, 1.5, Pickaxe, ptiles(t::PISTON_FRONT, t::PISTON_FRONT, t::PISTON_SIDE, 4)),
    229 PistonHead5 => info("Головка поршня", Shape, 1.5, Pickaxe, ptiles(t::PISTON_FRONT, t::PISTON_FRONT, t::PISTON_SIDE, 5)),
    // Ids 230..=235 were the sticky piston heads (one head now serves both pistons).
    230 Farmland => info("Грядка", Opaque, 0.6, Shovel, column(t::DIRT, t::FARMLAND, t::DIRT)),
    231 WheatYoung => not_solid(info("Пшеница (растёт)", Cross, 0.0, Hand, all(t::WHEAT_YOUNG))),
    232 Wheat => not_solid(info("Пшеница", Cross, 0.0, Hand, all(t::WHEAT_RIPE))),
    233 CarrotYoung => not_solid(info("Морковь (растёт)", Cross, 0.0, Hand, all(t::CARROT_YOUNG))),
    234 Carrots => not_solid(info("Морковь", Cross, 0.0, Hand, all(t::CARROT_RIPE))),
    235 PotatoYoung => not_solid(info("Картофель (растёт)", Cross, 0.0, Hand, all(t::POTATO_YOUNG))),
    236 Repeater0 => not_solid(info("Повторитель", Shape, 0.0, Hand, all(t::REPEATER + 0))),
    237 Repeater1 => not_solid(info("Повторитель", Shape, 0.0, Hand, all(t::REPEATER + 1))),
    238 Repeater2 => not_solid(info("Повторитель", Shape, 0.0, Hand, all(t::REPEATER + 2))),
    239 Repeater3 => not_solid(info("Повторитель", Shape, 0.0, Hand, all(t::REPEATER + 3))),
    240 RepeaterOn0 => emissive(not_solid(info("Повторитель (вкл.)", Shape, 0.0, Hand, all(t::REPEATER_ON + 0)))),
    241 RepeaterOn1 => emissive(not_solid(info("Повторитель (вкл.)", Shape, 0.0, Hand, all(t::REPEATER_ON + 1)))),
    242 RepeaterOn2 => emissive(not_solid(info("Повторитель (вкл.)", Shape, 0.0, Hand, all(t::REPEATER_ON + 2)))),
    243 RepeaterOn3 => emissive(not_solid(info("Повторитель (вкл.)", Shape, 0.0, Hand, all(t::REPEATER_ON + 3)))),
    244 Comparator0 => not_solid(info("Сравниватель", Shape, 0.0, Hand, all(t::COMPARATOR + 0))),
    245 Comparator1 => not_solid(info("Сравниватель", Shape, 0.0, Hand, all(t::COMPARATOR + 1))),
    246 Comparator2 => not_solid(info("Сравниватель", Shape, 0.0, Hand, all(t::COMPARATOR + 2))),
    247 Comparator3 => not_solid(info("Сравниватель", Shape, 0.0, Hand, all(t::COMPARATOR + 3))),
    248 ComparatorOn0 => emissive(not_solid(info("Сравниватель (вкл.)", Shape, 0.0, Hand, all(t::COMPARATOR_ON + 0)))),
    249 ComparatorOn1 => emissive(not_solid(info("Сравниватель (вкл.)", Shape, 0.0, Hand, all(t::COMPARATOR_ON + 1)))),
    250 ComparatorOn2 => emissive(not_solid(info("Сравниватель (вкл.)", Shape, 0.0, Hand, all(t::COMPARATOR_ON + 2)))),
    251 ComparatorOn3 => emissive(not_solid(info("Сравниватель (вкл.)", Shape, 0.0, Hand, all(t::COMPARATOR_ON + 3)))),
    252 Baseplate => needs_pick(info("Основа", Opaque, 7.5, Pickaxe, all(t::BASEPLATE))),
    253 PressurePlate => not_solid(info("Нажимная плита", Shape, 0.5, Pickaxe, all(t::SMOOTH_STONE))),
    254 PressurePlateOn => not_solid(info("Нажимная плита (нажата)", Shape, 0.5, Pickaxe, all(t::SMOOTH_STONE))),
    255 Potatoes => not_solid(info("Картофель", Cross, 0.0, Hand, all(t::POTATO_RIPE))),
    1024 Build0R0 => crate::construction::block_info("Дуб · Нижняя плита", 0, crate::construction::Form::LowerSlab),
    1025 Build0R1 => crate::construction::block_info("Дуб · Нижняя плита", 0, crate::construction::Form::LowerSlab),
    1026 Build0R2 => crate::construction::block_info("Дуб · Нижняя плита", 0, crate::construction::Form::LowerSlab),
    1027 Build0R3 => crate::construction::block_info("Дуб · Нижняя плита", 0, crate::construction::Form::LowerSlab),
    1028 Build1R0 => crate::construction::block_info("Берёза · Нижняя плита", 1, crate::construction::Form::LowerSlab),
    1029 Build1R1 => crate::construction::block_info("Берёза · Нижняя плита", 1, crate::construction::Form::LowerSlab),
    1030 Build1R2 => crate::construction::block_info("Берёза · Нижняя плита", 1, crate::construction::Form::LowerSlab),
    1031 Build1R3 => crate::construction::block_info("Берёза · Нижняя плита", 1, crate::construction::Form::LowerSlab),
    1032 Build2R0 => crate::construction::block_info("Ель · Нижняя плита", 2, crate::construction::Form::LowerSlab),
    1033 Build2R1 => crate::construction::block_info("Ель · Нижняя плита", 2, crate::construction::Form::LowerSlab),
    1034 Build2R2 => crate::construction::block_info("Ель · Нижняя плита", 2, crate::construction::Form::LowerSlab),
    1035 Build2R3 => crate::construction::block_info("Ель · Нижняя плита", 2, crate::construction::Form::LowerSlab),
    1036 Build3R0 => crate::construction::block_info("Камень · Нижняя плита", 3, crate::construction::Form::LowerSlab),
    1037 Build3R1 => crate::construction::block_info("Камень · Нижняя плита", 3, crate::construction::Form::LowerSlab),
    1038 Build3R2 => crate::construction::block_info("Камень · Нижняя плита", 3, crate::construction::Form::LowerSlab),
    1039 Build3R3 => crate::construction::block_info("Камень · Нижняя плита", 3, crate::construction::Form::LowerSlab),
    1040 Build4R0 => crate::construction::block_info("Кирпич · Нижняя плита", 4, crate::construction::Form::LowerSlab),
    1041 Build4R1 => crate::construction::block_info("Кирпич · Нижняя плита", 4, crate::construction::Form::LowerSlab),
    1042 Build4R2 => crate::construction::block_info("Кирпич · Нижняя плита", 4, crate::construction::Form::LowerSlab),
    1043 Build4R3 => crate::construction::block_info("Кирпич · Нижняя плита", 4, crate::construction::Form::LowerSlab),
    1044 Build5R0 => crate::construction::block_info("Мрамор · Нижняя плита", 5, crate::construction::Form::LowerSlab),
    1045 Build5R1 => crate::construction::block_info("Мрамор · Нижняя плита", 5, crate::construction::Form::LowerSlab),
    1046 Build5R2 => crate::construction::block_info("Мрамор · Нижняя плита", 5, crate::construction::Form::LowerSlab),
    1047 Build5R3 => crate::construction::block_info("Мрамор · Нижняя плита", 5, crate::construction::Form::LowerSlab),
    1048 Build6R0 => crate::construction::block_info("Железо · Нижняя плита", 6, crate::construction::Form::LowerSlab),
    1049 Build6R1 => crate::construction::block_info("Железо · Нижняя плита", 6, crate::construction::Form::LowerSlab),
    1050 Build6R2 => crate::construction::block_info("Железо · Нижняя плита", 6, crate::construction::Form::LowerSlab),
    1051 Build6R3 => crate::construction::block_info("Железо · Нижняя плита", 6, crate::construction::Form::LowerSlab),
    1052 Build7R0 => crate::construction::block_info("Медь · Нижняя плита", 7, crate::construction::Form::LowerSlab),
    1053 Build7R1 => crate::construction::block_info("Медь · Нижняя плита", 7, crate::construction::Form::LowerSlab),
    1054 Build7R2 => crate::construction::block_info("Медь · Нижняя плита", 7, crate::construction::Form::LowerSlab),
    1055 Build7R3 => crate::construction::block_info("Медь · Нижняя плита", 7, crate::construction::Form::LowerSlab),
    1056 Build8R0 => crate::construction::block_info("Дуб · Верхняя плита", 0, crate::construction::Form::UpperSlab),
    1057 Build8R1 => crate::construction::block_info("Дуб · Верхняя плита", 0, crate::construction::Form::UpperSlab),
    1058 Build8R2 => crate::construction::block_info("Дуб · Верхняя плита", 0, crate::construction::Form::UpperSlab),
    1059 Build8R3 => crate::construction::block_info("Дуб · Верхняя плита", 0, crate::construction::Form::UpperSlab),
    1060 Build9R0 => crate::construction::block_info("Берёза · Верхняя плита", 1, crate::construction::Form::UpperSlab),
    1061 Build9R1 => crate::construction::block_info("Берёза · Верхняя плита", 1, crate::construction::Form::UpperSlab),
    1062 Build9R2 => crate::construction::block_info("Берёза · Верхняя плита", 1, crate::construction::Form::UpperSlab),
    1063 Build9R3 => crate::construction::block_info("Берёза · Верхняя плита", 1, crate::construction::Form::UpperSlab),
    1064 Build10R0 => crate::construction::block_info("Ель · Верхняя плита", 2, crate::construction::Form::UpperSlab),
    1065 Build10R1 => crate::construction::block_info("Ель · Верхняя плита", 2, crate::construction::Form::UpperSlab),
    1066 Build10R2 => crate::construction::block_info("Ель · Верхняя плита", 2, crate::construction::Form::UpperSlab),
    1067 Build10R3 => crate::construction::block_info("Ель · Верхняя плита", 2, crate::construction::Form::UpperSlab),
    1068 Build11R0 => crate::construction::block_info("Камень · Верхняя плита", 3, crate::construction::Form::UpperSlab),
    1069 Build11R1 => crate::construction::block_info("Камень · Верхняя плита", 3, crate::construction::Form::UpperSlab),
    1070 Build11R2 => crate::construction::block_info("Камень · Верхняя плита", 3, crate::construction::Form::UpperSlab),
    1071 Build11R3 => crate::construction::block_info("Камень · Верхняя плита", 3, crate::construction::Form::UpperSlab),
    1072 Build12R0 => crate::construction::block_info("Кирпич · Верхняя плита", 4, crate::construction::Form::UpperSlab),
    1073 Build12R1 => crate::construction::block_info("Кирпич · Верхняя плита", 4, crate::construction::Form::UpperSlab),
    1074 Build12R2 => crate::construction::block_info("Кирпич · Верхняя плита", 4, crate::construction::Form::UpperSlab),
    1075 Build12R3 => crate::construction::block_info("Кирпич · Верхняя плита", 4, crate::construction::Form::UpperSlab),
    1076 Build13R0 => crate::construction::block_info("Мрамор · Верхняя плита", 5, crate::construction::Form::UpperSlab),
    1077 Build13R1 => crate::construction::block_info("Мрамор · Верхняя плита", 5, crate::construction::Form::UpperSlab),
    1078 Build13R2 => crate::construction::block_info("Мрамор · Верхняя плита", 5, crate::construction::Form::UpperSlab),
    1079 Build13R3 => crate::construction::block_info("Мрамор · Верхняя плита", 5, crate::construction::Form::UpperSlab),
    1080 Build14R0 => crate::construction::block_info("Железо · Верхняя плита", 6, crate::construction::Form::UpperSlab),
    1081 Build14R1 => crate::construction::block_info("Железо · Верхняя плита", 6, crate::construction::Form::UpperSlab),
    1082 Build14R2 => crate::construction::block_info("Железо · Верхняя плита", 6, crate::construction::Form::UpperSlab),
    1083 Build14R3 => crate::construction::block_info("Железо · Верхняя плита", 6, crate::construction::Form::UpperSlab),
    1084 Build15R0 => crate::construction::block_info("Медь · Верхняя плита", 7, crate::construction::Form::UpperSlab),
    1085 Build15R1 => crate::construction::block_info("Медь · Верхняя плита", 7, crate::construction::Form::UpperSlab),
    1086 Build15R2 => crate::construction::block_info("Медь · Верхняя плита", 7, crate::construction::Form::UpperSlab),
    1087 Build15R3 => crate::construction::block_info("Медь · Верхняя плита", 7, crate::construction::Form::UpperSlab),
    1088 Build16R0 => crate::construction::block_info("Дуб · Вертикальная плита", 0, crate::construction::Form::VerticalSlab),
    1089 Build16R1 => crate::construction::block_info("Дуб · Вертикальная плита", 0, crate::construction::Form::VerticalSlab),
    1090 Build16R2 => crate::construction::block_info("Дуб · Вертикальная плита", 0, crate::construction::Form::VerticalSlab),
    1091 Build16R3 => crate::construction::block_info("Дуб · Вертикальная плита", 0, crate::construction::Form::VerticalSlab),
    1092 Build17R0 => crate::construction::block_info("Берёза · Вертикальная плита", 1, crate::construction::Form::VerticalSlab),
    1093 Build17R1 => crate::construction::block_info("Берёза · Вертикальная плита", 1, crate::construction::Form::VerticalSlab),
    1094 Build17R2 => crate::construction::block_info("Берёза · Вертикальная плита", 1, crate::construction::Form::VerticalSlab),
    1095 Build17R3 => crate::construction::block_info("Берёза · Вертикальная плита", 1, crate::construction::Form::VerticalSlab),
    1096 Build18R0 => crate::construction::block_info("Ель · Вертикальная плита", 2, crate::construction::Form::VerticalSlab),
    1097 Build18R1 => crate::construction::block_info("Ель · Вертикальная плита", 2, crate::construction::Form::VerticalSlab),
    1098 Build18R2 => crate::construction::block_info("Ель · Вертикальная плита", 2, crate::construction::Form::VerticalSlab),
    1099 Build18R3 => crate::construction::block_info("Ель · Вертикальная плита", 2, crate::construction::Form::VerticalSlab),
    1100 Build19R0 => crate::construction::block_info("Камень · Вертикальная плита", 3, crate::construction::Form::VerticalSlab),
    1101 Build19R1 => crate::construction::block_info("Камень · Вертикальная плита", 3, crate::construction::Form::VerticalSlab),
    1102 Build19R2 => crate::construction::block_info("Камень · Вертикальная плита", 3, crate::construction::Form::VerticalSlab),
    1103 Build19R3 => crate::construction::block_info("Камень · Вертикальная плита", 3, crate::construction::Form::VerticalSlab),
    1104 Build20R0 => crate::construction::block_info("Кирпич · Вертикальная плита", 4, crate::construction::Form::VerticalSlab),
    1105 Build20R1 => crate::construction::block_info("Кирпич · Вертикальная плита", 4, crate::construction::Form::VerticalSlab),
    1106 Build20R2 => crate::construction::block_info("Кирпич · Вертикальная плита", 4, crate::construction::Form::VerticalSlab),
    1107 Build20R3 => crate::construction::block_info("Кирпич · Вертикальная плита", 4, crate::construction::Form::VerticalSlab),
    1108 Build21R0 => crate::construction::block_info("Мрамор · Вертикальная плита", 5, crate::construction::Form::VerticalSlab),
    1109 Build21R1 => crate::construction::block_info("Мрамор · Вертикальная плита", 5, crate::construction::Form::VerticalSlab),
    1110 Build21R2 => crate::construction::block_info("Мрамор · Вертикальная плита", 5, crate::construction::Form::VerticalSlab),
    1111 Build21R3 => crate::construction::block_info("Мрамор · Вертикальная плита", 5, crate::construction::Form::VerticalSlab),
    1112 Build22R0 => crate::construction::block_info("Железо · Вертикальная плита", 6, crate::construction::Form::VerticalSlab),
    1113 Build22R1 => crate::construction::block_info("Железо · Вертикальная плита", 6, crate::construction::Form::VerticalSlab),
    1114 Build22R2 => crate::construction::block_info("Железо · Вертикальная плита", 6, crate::construction::Form::VerticalSlab),
    1115 Build22R3 => crate::construction::block_info("Железо · Вертикальная плита", 6, crate::construction::Form::VerticalSlab),
    1116 Build23R0 => crate::construction::block_info("Медь · Вертикальная плита", 7, crate::construction::Form::VerticalSlab),
    1117 Build23R1 => crate::construction::block_info("Медь · Вертикальная плита", 7, crate::construction::Form::VerticalSlab),
    1118 Build23R2 => crate::construction::block_info("Медь · Вертикальная плита", 7, crate::construction::Form::VerticalSlab),
    1119 Build23R3 => crate::construction::block_info("Медь · Вертикальная плита", 7, crate::construction::Form::VerticalSlab),
    1120 Build24R0 => crate::construction::block_info("Дуб · Ступени", 0, crate::construction::Form::Stairs),
    1121 Build24R1 => crate::construction::block_info("Дуб · Ступени", 0, crate::construction::Form::Stairs),
    1122 Build24R2 => crate::construction::block_info("Дуб · Ступени", 0, crate::construction::Form::Stairs),
    1123 Build24R3 => crate::construction::block_info("Дуб · Ступени", 0, crate::construction::Form::Stairs),
    1124 Build25R0 => crate::construction::block_info("Берёза · Ступени", 1, crate::construction::Form::Stairs),
    1125 Build25R1 => crate::construction::block_info("Берёза · Ступени", 1, crate::construction::Form::Stairs),
    1126 Build25R2 => crate::construction::block_info("Берёза · Ступени", 1, crate::construction::Form::Stairs),
    1127 Build25R3 => crate::construction::block_info("Берёза · Ступени", 1, crate::construction::Form::Stairs),
    1128 Build26R0 => crate::construction::block_info("Ель · Ступени", 2, crate::construction::Form::Stairs),
    1129 Build26R1 => crate::construction::block_info("Ель · Ступени", 2, crate::construction::Form::Stairs),
    1130 Build26R2 => crate::construction::block_info("Ель · Ступени", 2, crate::construction::Form::Stairs),
    1131 Build26R3 => crate::construction::block_info("Ель · Ступени", 2, crate::construction::Form::Stairs),
    1132 Build27R0 => crate::construction::block_info("Камень · Ступени", 3, crate::construction::Form::Stairs),
    1133 Build27R1 => crate::construction::block_info("Камень · Ступени", 3, crate::construction::Form::Stairs),
    1134 Build27R2 => crate::construction::block_info("Камень · Ступени", 3, crate::construction::Form::Stairs),
    1135 Build27R3 => crate::construction::block_info("Камень · Ступени", 3, crate::construction::Form::Stairs),
    1136 Build28R0 => crate::construction::block_info("Кирпич · Ступени", 4, crate::construction::Form::Stairs),
    1137 Build28R1 => crate::construction::block_info("Кирпич · Ступени", 4, crate::construction::Form::Stairs),
    1138 Build28R2 => crate::construction::block_info("Кирпич · Ступени", 4, crate::construction::Form::Stairs),
    1139 Build28R3 => crate::construction::block_info("Кирпич · Ступени", 4, crate::construction::Form::Stairs),
    1140 Build29R0 => crate::construction::block_info("Мрамор · Ступени", 5, crate::construction::Form::Stairs),
    1141 Build29R1 => crate::construction::block_info("Мрамор · Ступени", 5, crate::construction::Form::Stairs),
    1142 Build29R2 => crate::construction::block_info("Мрамор · Ступени", 5, crate::construction::Form::Stairs),
    1143 Build29R3 => crate::construction::block_info("Мрамор · Ступени", 5, crate::construction::Form::Stairs),
    1144 Build30R0 => crate::construction::block_info("Железо · Ступени", 6, crate::construction::Form::Stairs),
    1145 Build30R1 => crate::construction::block_info("Железо · Ступени", 6, crate::construction::Form::Stairs),
    1146 Build30R2 => crate::construction::block_info("Железо · Ступени", 6, crate::construction::Form::Stairs),
    1147 Build30R3 => crate::construction::block_info("Железо · Ступени", 6, crate::construction::Form::Stairs),
    1148 Build31R0 => crate::construction::block_info("Медь · Ступени", 7, crate::construction::Form::Stairs),
    1149 Build31R1 => crate::construction::block_info("Медь · Ступени", 7, crate::construction::Form::Stairs),
    1150 Build31R2 => crate::construction::block_info("Медь · Ступени", 7, crate::construction::Form::Stairs),
    1151 Build31R3 => crate::construction::block_info("Медь · Ступени", 7, crate::construction::Form::Stairs),
    1152 Build32R0 => crate::construction::block_info("Дуб · Внутренний угол ступеней", 0, crate::construction::Form::InnerStairs),
    1153 Build32R1 => crate::construction::block_info("Дуб · Внутренний угол ступеней", 0, crate::construction::Form::InnerStairs),
    1154 Build32R2 => crate::construction::block_info("Дуб · Внутренний угол ступеней", 0, crate::construction::Form::InnerStairs),
    1155 Build32R3 => crate::construction::block_info("Дуб · Внутренний угол ступеней", 0, crate::construction::Form::InnerStairs),
    1156 Build33R0 => crate::construction::block_info("Берёза · Внутренний угол ступеней", 1, crate::construction::Form::InnerStairs),
    1157 Build33R1 => crate::construction::block_info("Берёза · Внутренний угол ступеней", 1, crate::construction::Form::InnerStairs),
    1158 Build33R2 => crate::construction::block_info("Берёза · Внутренний угол ступеней", 1, crate::construction::Form::InnerStairs),
    1159 Build33R3 => crate::construction::block_info("Берёза · Внутренний угол ступеней", 1, crate::construction::Form::InnerStairs),
    1160 Build34R0 => crate::construction::block_info("Ель · Внутренний угол ступеней", 2, crate::construction::Form::InnerStairs),
    1161 Build34R1 => crate::construction::block_info("Ель · Внутренний угол ступеней", 2, crate::construction::Form::InnerStairs),
    1162 Build34R2 => crate::construction::block_info("Ель · Внутренний угол ступеней", 2, crate::construction::Form::InnerStairs),
    1163 Build34R3 => crate::construction::block_info("Ель · Внутренний угол ступеней", 2, crate::construction::Form::InnerStairs),
    1164 Build35R0 => crate::construction::block_info("Камень · Внутренний угол ступеней", 3, crate::construction::Form::InnerStairs),
    1165 Build35R1 => crate::construction::block_info("Камень · Внутренний угол ступеней", 3, crate::construction::Form::InnerStairs),
    1166 Build35R2 => crate::construction::block_info("Камень · Внутренний угол ступеней", 3, crate::construction::Form::InnerStairs),
    1167 Build35R3 => crate::construction::block_info("Камень · Внутренний угол ступеней", 3, crate::construction::Form::InnerStairs),
    1168 Build36R0 => crate::construction::block_info("Кирпич · Внутренний угол ступеней", 4, crate::construction::Form::InnerStairs),
    1169 Build36R1 => crate::construction::block_info("Кирпич · Внутренний угол ступеней", 4, crate::construction::Form::InnerStairs),
    1170 Build36R2 => crate::construction::block_info("Кирпич · Внутренний угол ступеней", 4, crate::construction::Form::InnerStairs),
    1171 Build36R3 => crate::construction::block_info("Кирпич · Внутренний угол ступеней", 4, crate::construction::Form::InnerStairs),
    1172 Build37R0 => crate::construction::block_info("Мрамор · Внутренний угол ступеней", 5, crate::construction::Form::InnerStairs),
    1173 Build37R1 => crate::construction::block_info("Мрамор · Внутренний угол ступеней", 5, crate::construction::Form::InnerStairs),
    1174 Build37R2 => crate::construction::block_info("Мрамор · Внутренний угол ступеней", 5, crate::construction::Form::InnerStairs),
    1175 Build37R3 => crate::construction::block_info("Мрамор · Внутренний угол ступеней", 5, crate::construction::Form::InnerStairs),
    1176 Build38R0 => crate::construction::block_info("Железо · Внутренний угол ступеней", 6, crate::construction::Form::InnerStairs),
    1177 Build38R1 => crate::construction::block_info("Железо · Внутренний угол ступеней", 6, crate::construction::Form::InnerStairs),
    1178 Build38R2 => crate::construction::block_info("Железо · Внутренний угол ступеней", 6, crate::construction::Form::InnerStairs),
    1179 Build38R3 => crate::construction::block_info("Железо · Внутренний угол ступеней", 6, crate::construction::Form::InnerStairs),
    1180 Build39R0 => crate::construction::block_info("Медь · Внутренний угол ступеней", 7, crate::construction::Form::InnerStairs),
    1181 Build39R1 => crate::construction::block_info("Медь · Внутренний угол ступеней", 7, crate::construction::Form::InnerStairs),
    1182 Build39R2 => crate::construction::block_info("Медь · Внутренний угол ступеней", 7, crate::construction::Form::InnerStairs),
    1183 Build39R3 => crate::construction::block_info("Медь · Внутренний угол ступеней", 7, crate::construction::Form::InnerStairs),
    1184 Build40R0 => crate::construction::block_info("Дуб · Внешний угол ступеней", 0, crate::construction::Form::OuterStairs),
    1185 Build40R1 => crate::construction::block_info("Дуб · Внешний угол ступеней", 0, crate::construction::Form::OuterStairs),
    1186 Build40R2 => crate::construction::block_info("Дуб · Внешний угол ступеней", 0, crate::construction::Form::OuterStairs),
    1187 Build40R3 => crate::construction::block_info("Дуб · Внешний угол ступеней", 0, crate::construction::Form::OuterStairs),
    1188 Build41R0 => crate::construction::block_info("Берёза · Внешний угол ступеней", 1, crate::construction::Form::OuterStairs),
    1189 Build41R1 => crate::construction::block_info("Берёза · Внешний угол ступеней", 1, crate::construction::Form::OuterStairs),
    1190 Build41R2 => crate::construction::block_info("Берёза · Внешний угол ступеней", 1, crate::construction::Form::OuterStairs),
    1191 Build41R3 => crate::construction::block_info("Берёза · Внешний угол ступеней", 1, crate::construction::Form::OuterStairs),
    1192 Build42R0 => crate::construction::block_info("Ель · Внешний угол ступеней", 2, crate::construction::Form::OuterStairs),
    1193 Build42R1 => crate::construction::block_info("Ель · Внешний угол ступеней", 2, crate::construction::Form::OuterStairs),
    1194 Build42R2 => crate::construction::block_info("Ель · Внешний угол ступеней", 2, crate::construction::Form::OuterStairs),
    1195 Build42R3 => crate::construction::block_info("Ель · Внешний угол ступеней", 2, crate::construction::Form::OuterStairs),
    1196 Build43R0 => crate::construction::block_info("Камень · Внешний угол ступеней", 3, crate::construction::Form::OuterStairs),
    1197 Build43R1 => crate::construction::block_info("Камень · Внешний угол ступеней", 3, crate::construction::Form::OuterStairs),
    1198 Build43R2 => crate::construction::block_info("Камень · Внешний угол ступеней", 3, crate::construction::Form::OuterStairs),
    1199 Build43R3 => crate::construction::block_info("Камень · Внешний угол ступеней", 3, crate::construction::Form::OuterStairs),
    1200 Build44R0 => crate::construction::block_info("Кирпич · Внешний угол ступеней", 4, crate::construction::Form::OuterStairs),
    1201 Build44R1 => crate::construction::block_info("Кирпич · Внешний угол ступеней", 4, crate::construction::Form::OuterStairs),
    1202 Build44R2 => crate::construction::block_info("Кирпич · Внешний угол ступеней", 4, crate::construction::Form::OuterStairs),
    1203 Build44R3 => crate::construction::block_info("Кирпич · Внешний угол ступеней", 4, crate::construction::Form::OuterStairs),
    1204 Build45R0 => crate::construction::block_info("Мрамор · Внешний угол ступеней", 5, crate::construction::Form::OuterStairs),
    1205 Build45R1 => crate::construction::block_info("Мрамор · Внешний угол ступеней", 5, crate::construction::Form::OuterStairs),
    1206 Build45R2 => crate::construction::block_info("Мрамор · Внешний угол ступеней", 5, crate::construction::Form::OuterStairs),
    1207 Build45R3 => crate::construction::block_info("Мрамор · Внешний угол ступеней", 5, crate::construction::Form::OuterStairs),
    1208 Build46R0 => crate::construction::block_info("Железо · Внешний угол ступеней", 6, crate::construction::Form::OuterStairs),
    1209 Build46R1 => crate::construction::block_info("Железо · Внешний угол ступеней", 6, crate::construction::Form::OuterStairs),
    1210 Build46R2 => crate::construction::block_info("Железо · Внешний угол ступеней", 6, crate::construction::Form::OuterStairs),
    1211 Build46R3 => crate::construction::block_info("Железо · Внешний угол ступеней", 6, crate::construction::Form::OuterStairs),
    1212 Build47R0 => crate::construction::block_info("Медь · Внешний угол ступеней", 7, crate::construction::Form::OuterStairs),
    1213 Build47R1 => crate::construction::block_info("Медь · Внешний угол ступеней", 7, crate::construction::Form::OuterStairs),
    1214 Build47R2 => crate::construction::block_info("Медь · Внешний угол ступеней", 7, crate::construction::Form::OuterStairs),
    1215 Build47R3 => crate::construction::block_info("Медь · Внешний угол ступеней", 7, crate::construction::Form::OuterStairs),
    1216 Build48R0 => crate::construction::block_info("Дуб · Скат крыши", 0, crate::construction::Form::Wedge),
    1217 Build48R1 => crate::construction::block_info("Дуб · Скат крыши", 0, crate::construction::Form::Wedge),
    1218 Build48R2 => crate::construction::block_info("Дуб · Скат крыши", 0, crate::construction::Form::Wedge),
    1219 Build48R3 => crate::construction::block_info("Дуб · Скат крыши", 0, crate::construction::Form::Wedge),
    1220 Build49R0 => crate::construction::block_info("Берёза · Скат крыши", 1, crate::construction::Form::Wedge),
    1221 Build49R1 => crate::construction::block_info("Берёза · Скат крыши", 1, crate::construction::Form::Wedge),
    1222 Build49R2 => crate::construction::block_info("Берёза · Скат крыши", 1, crate::construction::Form::Wedge),
    1223 Build49R3 => crate::construction::block_info("Берёза · Скат крыши", 1, crate::construction::Form::Wedge),
    1224 Build50R0 => crate::construction::block_info("Ель · Скат крыши", 2, crate::construction::Form::Wedge),
    1225 Build50R1 => crate::construction::block_info("Ель · Скат крыши", 2, crate::construction::Form::Wedge),
    1226 Build50R2 => crate::construction::block_info("Ель · Скат крыши", 2, crate::construction::Form::Wedge),
    1227 Build50R3 => crate::construction::block_info("Ель · Скат крыши", 2, crate::construction::Form::Wedge),
    1228 Build51R0 => crate::construction::block_info("Камень · Скат крыши", 3, crate::construction::Form::Wedge),
    1229 Build51R1 => crate::construction::block_info("Камень · Скат крыши", 3, crate::construction::Form::Wedge),
    1230 Build51R2 => crate::construction::block_info("Камень · Скат крыши", 3, crate::construction::Form::Wedge),
    1231 Build51R3 => crate::construction::block_info("Камень · Скат крыши", 3, crate::construction::Form::Wedge),
    1232 Build52R0 => crate::construction::block_info("Кирпич · Скат крыши", 4, crate::construction::Form::Wedge),
    1233 Build52R1 => crate::construction::block_info("Кирпич · Скат крыши", 4, crate::construction::Form::Wedge),
    1234 Build52R2 => crate::construction::block_info("Кирпич · Скат крыши", 4, crate::construction::Form::Wedge),
    1235 Build52R3 => crate::construction::block_info("Кирпич · Скат крыши", 4, crate::construction::Form::Wedge),
    1236 Build53R0 => crate::construction::block_info("Мрамор · Скат крыши", 5, crate::construction::Form::Wedge),
    1237 Build53R1 => crate::construction::block_info("Мрамор · Скат крыши", 5, crate::construction::Form::Wedge),
    1238 Build53R2 => crate::construction::block_info("Мрамор · Скат крыши", 5, crate::construction::Form::Wedge),
    1239 Build53R3 => crate::construction::block_info("Мрамор · Скат крыши", 5, crate::construction::Form::Wedge),
    1240 Build54R0 => crate::construction::block_info("Железо · Скат крыши", 6, crate::construction::Form::Wedge),
    1241 Build54R1 => crate::construction::block_info("Железо · Скат крыши", 6, crate::construction::Form::Wedge),
    1242 Build54R2 => crate::construction::block_info("Железо · Скат крыши", 6, crate::construction::Form::Wedge),
    1243 Build54R3 => crate::construction::block_info("Железо · Скат крыши", 6, crate::construction::Form::Wedge),
    1244 Build55R0 => crate::construction::block_info("Медь · Скат крыши", 7, crate::construction::Form::Wedge),
    1245 Build55R1 => crate::construction::block_info("Медь · Скат крыши", 7, crate::construction::Form::Wedge),
    1246 Build55R2 => crate::construction::block_info("Медь · Скат крыши", 7, crate::construction::Form::Wedge),
    1247 Build55R3 => crate::construction::block_info("Медь · Скат крыши", 7, crate::construction::Form::Wedge),
    1248 Build56R0 => crate::construction::block_info("Дуб · Балка", 0, crate::construction::Form::Beam),
    1249 Build56R1 => crate::construction::block_info("Дуб · Балка", 0, crate::construction::Form::Beam),
    1250 Build56R2 => crate::construction::block_info("Дуб · Балка", 0, crate::construction::Form::Beam),
    1251 Build56R3 => crate::construction::block_info("Дуб · Балка", 0, crate::construction::Form::Beam),
    1252 Build57R0 => crate::construction::block_info("Берёза · Балка", 1, crate::construction::Form::Beam),
    1253 Build57R1 => crate::construction::block_info("Берёза · Балка", 1, crate::construction::Form::Beam),
    1254 Build57R2 => crate::construction::block_info("Берёза · Балка", 1, crate::construction::Form::Beam),
    1255 Build57R3 => crate::construction::block_info("Берёза · Балка", 1, crate::construction::Form::Beam),
    1256 Build58R0 => crate::construction::block_info("Ель · Балка", 2, crate::construction::Form::Beam),
    1257 Build58R1 => crate::construction::block_info("Ель · Балка", 2, crate::construction::Form::Beam),
    1258 Build58R2 => crate::construction::block_info("Ель · Балка", 2, crate::construction::Form::Beam),
    1259 Build58R3 => crate::construction::block_info("Ель · Балка", 2, crate::construction::Form::Beam),
    1260 Build59R0 => crate::construction::block_info("Камень · Балка", 3, crate::construction::Form::Beam),
    1261 Build59R1 => crate::construction::block_info("Камень · Балка", 3, crate::construction::Form::Beam),
    1262 Build59R2 => crate::construction::block_info("Камень · Балка", 3, crate::construction::Form::Beam),
    1263 Build59R3 => crate::construction::block_info("Камень · Балка", 3, crate::construction::Form::Beam),
    1264 Build60R0 => crate::construction::block_info("Кирпич · Балка", 4, crate::construction::Form::Beam),
    1265 Build60R1 => crate::construction::block_info("Кирпич · Балка", 4, crate::construction::Form::Beam),
    1266 Build60R2 => crate::construction::block_info("Кирпич · Балка", 4, crate::construction::Form::Beam),
    1267 Build60R3 => crate::construction::block_info("Кирпич · Балка", 4, crate::construction::Form::Beam),
    1268 Build61R0 => crate::construction::block_info("Мрамор · Балка", 5, crate::construction::Form::Beam),
    1269 Build61R1 => crate::construction::block_info("Мрамор · Балка", 5, crate::construction::Form::Beam),
    1270 Build61R2 => crate::construction::block_info("Мрамор · Балка", 5, crate::construction::Form::Beam),
    1271 Build61R3 => crate::construction::block_info("Мрамор · Балка", 5, crate::construction::Form::Beam),
    1272 Build62R0 => crate::construction::block_info("Железо · Балка", 6, crate::construction::Form::Beam),
    1273 Build62R1 => crate::construction::block_info("Железо · Балка", 6, crate::construction::Form::Beam),
    1274 Build62R2 => crate::construction::block_info("Железо · Балка", 6, crate::construction::Form::Beam),
    1275 Build62R3 => crate::construction::block_info("Железо · Балка", 6, crate::construction::Form::Beam),
    1276 Build63R0 => crate::construction::block_info("Медь · Балка", 7, crate::construction::Form::Beam),
    1277 Build63R1 => crate::construction::block_info("Медь · Балка", 7, crate::construction::Form::Beam),
    1278 Build63R2 => crate::construction::block_info("Медь · Балка", 7, crate::construction::Form::Beam),
    1279 Build63R3 => crate::construction::block_info("Медь · Балка", 7, crate::construction::Form::Beam),
    1280 Build64R0 => crate::construction::block_info("Дуб · Столб", 0, crate::construction::Form::Post),
    1281 Build64R1 => crate::construction::block_info("Дуб · Столб", 0, crate::construction::Form::Post),
    1282 Build64R2 => crate::construction::block_info("Дуб · Столб", 0, crate::construction::Form::Post),
    1283 Build64R3 => crate::construction::block_info("Дуб · Столб", 0, crate::construction::Form::Post),
    1284 Build65R0 => crate::construction::block_info("Берёза · Столб", 1, crate::construction::Form::Post),
    1285 Build65R1 => crate::construction::block_info("Берёза · Столб", 1, crate::construction::Form::Post),
    1286 Build65R2 => crate::construction::block_info("Берёза · Столб", 1, crate::construction::Form::Post),
    1287 Build65R3 => crate::construction::block_info("Берёза · Столб", 1, crate::construction::Form::Post),
    1288 Build66R0 => crate::construction::block_info("Ель · Столб", 2, crate::construction::Form::Post),
    1289 Build66R1 => crate::construction::block_info("Ель · Столб", 2, crate::construction::Form::Post),
    1290 Build66R2 => crate::construction::block_info("Ель · Столб", 2, crate::construction::Form::Post),
    1291 Build66R3 => crate::construction::block_info("Ель · Столб", 2, crate::construction::Form::Post),
    1292 Build67R0 => crate::construction::block_info("Камень · Столб", 3, crate::construction::Form::Post),
    1293 Build67R1 => crate::construction::block_info("Камень · Столб", 3, crate::construction::Form::Post),
    1294 Build67R2 => crate::construction::block_info("Камень · Столб", 3, crate::construction::Form::Post),
    1295 Build67R3 => crate::construction::block_info("Камень · Столб", 3, crate::construction::Form::Post),
    1296 Build68R0 => crate::construction::block_info("Кирпич · Столб", 4, crate::construction::Form::Post),
    1297 Build68R1 => crate::construction::block_info("Кирпич · Столб", 4, crate::construction::Form::Post),
    1298 Build68R2 => crate::construction::block_info("Кирпич · Столб", 4, crate::construction::Form::Post),
    1299 Build68R3 => crate::construction::block_info("Кирпич · Столб", 4, crate::construction::Form::Post),
    1300 Build69R0 => crate::construction::block_info("Мрамор · Столб", 5, crate::construction::Form::Post),
    1301 Build69R1 => crate::construction::block_info("Мрамор · Столб", 5, crate::construction::Form::Post),
    1302 Build69R2 => crate::construction::block_info("Мрамор · Столб", 5, crate::construction::Form::Post),
    1303 Build69R3 => crate::construction::block_info("Мрамор · Столб", 5, crate::construction::Form::Post),
    1304 Build70R0 => crate::construction::block_info("Железо · Столб", 6, crate::construction::Form::Post),
    1305 Build70R1 => crate::construction::block_info("Железо · Столб", 6, crate::construction::Form::Post),
    1306 Build70R2 => crate::construction::block_info("Железо · Столб", 6, crate::construction::Form::Post),
    1307 Build70R3 => crate::construction::block_info("Железо · Столб", 6, crate::construction::Form::Post),
    1308 Build71R0 => crate::construction::block_info("Медь · Столб", 7, crate::construction::Form::Post),
    1309 Build71R1 => crate::construction::block_info("Медь · Столб", 7, crate::construction::Form::Post),
    1310 Build71R2 => crate::construction::block_info("Медь · Столб", 7, crate::construction::Form::Post),
    1311 Build71R3 => crate::construction::block_info("Медь · Столб", 7, crate::construction::Form::Post),
    1312 Build72R0 => crate::construction::block_info("Дуб · Четверть блока", 0, crate::construction::Form::Quarter),
    1313 Build72R1 => crate::construction::block_info("Дуб · Четверть блока", 0, crate::construction::Form::Quarter),
    1314 Build72R2 => crate::construction::block_info("Дуб · Четверть блока", 0, crate::construction::Form::Quarter),
    1315 Build72R3 => crate::construction::block_info("Дуб · Четверть блока", 0, crate::construction::Form::Quarter),
    1316 Build73R0 => crate::construction::block_info("Берёза · Четверть блока", 1, crate::construction::Form::Quarter),
    1317 Build73R1 => crate::construction::block_info("Берёза · Четверть блока", 1, crate::construction::Form::Quarter),
    1318 Build73R2 => crate::construction::block_info("Берёза · Четверть блока", 1, crate::construction::Form::Quarter),
    1319 Build73R3 => crate::construction::block_info("Берёза · Четверть блока", 1, crate::construction::Form::Quarter),
    1320 Build74R0 => crate::construction::block_info("Ель · Четверть блока", 2, crate::construction::Form::Quarter),
    1321 Build74R1 => crate::construction::block_info("Ель · Четверть блока", 2, crate::construction::Form::Quarter),
    1322 Build74R2 => crate::construction::block_info("Ель · Четверть блока", 2, crate::construction::Form::Quarter),
    1323 Build74R3 => crate::construction::block_info("Ель · Четверть блока", 2, crate::construction::Form::Quarter),
    1324 Build75R0 => crate::construction::block_info("Камень · Четверть блока", 3, crate::construction::Form::Quarter),
    1325 Build75R1 => crate::construction::block_info("Камень · Четверть блока", 3, crate::construction::Form::Quarter),
    1326 Build75R2 => crate::construction::block_info("Камень · Четверть блока", 3, crate::construction::Form::Quarter),
    1327 Build75R3 => crate::construction::block_info("Камень · Четверть блока", 3, crate::construction::Form::Quarter),
    1328 Build76R0 => crate::construction::block_info("Кирпич · Четверть блока", 4, crate::construction::Form::Quarter),
    1329 Build76R1 => crate::construction::block_info("Кирпич · Четверть блока", 4, crate::construction::Form::Quarter),
    1330 Build76R2 => crate::construction::block_info("Кирпич · Четверть блока", 4, crate::construction::Form::Quarter),
    1331 Build76R3 => crate::construction::block_info("Кирпич · Четверть блока", 4, crate::construction::Form::Quarter),
    1332 Build77R0 => crate::construction::block_info("Мрамор · Четверть блока", 5, crate::construction::Form::Quarter),
    1333 Build77R1 => crate::construction::block_info("Мрамор · Четверть блока", 5, crate::construction::Form::Quarter),
    1334 Build77R2 => crate::construction::block_info("Мрамор · Четверть блока", 5, crate::construction::Form::Quarter),
    1335 Build77R3 => crate::construction::block_info("Мрамор · Четверть блока", 5, crate::construction::Form::Quarter),
    1336 Build78R0 => crate::construction::block_info("Железо · Четверть блока", 6, crate::construction::Form::Quarter),
    1337 Build78R1 => crate::construction::block_info("Железо · Четверть блока", 6, crate::construction::Form::Quarter),
    1338 Build78R2 => crate::construction::block_info("Железо · Четверть блока", 6, crate::construction::Form::Quarter),
    1339 Build78R3 => crate::construction::block_info("Железо · Четверть блока", 6, crate::construction::Form::Quarter),
    1340 Build79R0 => crate::construction::block_info("Медь · Четверть блока", 7, crate::construction::Form::Quarter),
    1341 Build79R1 => crate::construction::block_info("Медь · Четверть блока", 7, crate::construction::Form::Quarter),
    1342 Build79R2 => crate::construction::block_info("Медь · Четверть блока", 7, crate::construction::Form::Quarter),
    1343 Build79R3 => crate::construction::block_info("Медь · Четверть блока", 7, crate::construction::Form::Quarter),
    1344 Build80R0 => crate::construction::block_info("Дуб · Арка", 0, crate::construction::Form::Arch),
    1345 Build80R1 => crate::construction::block_info("Дуб · Арка", 0, crate::construction::Form::Arch),
    1346 Build80R2 => crate::construction::block_info("Дуб · Арка", 0, crate::construction::Form::Arch),
    1347 Build80R3 => crate::construction::block_info("Дуб · Арка", 0, crate::construction::Form::Arch),
    1348 Build81R0 => crate::construction::block_info("Берёза · Арка", 1, crate::construction::Form::Arch),
    1349 Build81R1 => crate::construction::block_info("Берёза · Арка", 1, crate::construction::Form::Arch),
    1350 Build81R2 => crate::construction::block_info("Берёза · Арка", 1, crate::construction::Form::Arch),
    1351 Build81R3 => crate::construction::block_info("Берёза · Арка", 1, crate::construction::Form::Arch),
    1352 Build82R0 => crate::construction::block_info("Ель · Арка", 2, crate::construction::Form::Arch),
    1353 Build82R1 => crate::construction::block_info("Ель · Арка", 2, crate::construction::Form::Arch),
    1354 Build82R2 => crate::construction::block_info("Ель · Арка", 2, crate::construction::Form::Arch),
    1355 Build82R3 => crate::construction::block_info("Ель · Арка", 2, crate::construction::Form::Arch),
    1356 Build83R0 => crate::construction::block_info("Камень · Арка", 3, crate::construction::Form::Arch),
    1357 Build83R1 => crate::construction::block_info("Камень · Арка", 3, crate::construction::Form::Arch),
    1358 Build83R2 => crate::construction::block_info("Камень · Арка", 3, crate::construction::Form::Arch),
    1359 Build83R3 => crate::construction::block_info("Камень · Арка", 3, crate::construction::Form::Arch),
    1360 Build84R0 => crate::construction::block_info("Кирпич · Арка", 4, crate::construction::Form::Arch),
    1361 Build84R1 => crate::construction::block_info("Кирпич · Арка", 4, crate::construction::Form::Arch),
    1362 Build84R2 => crate::construction::block_info("Кирпич · Арка", 4, crate::construction::Form::Arch),
    1363 Build84R3 => crate::construction::block_info("Кирпич · Арка", 4, crate::construction::Form::Arch),
    1364 Build85R0 => crate::construction::block_info("Мрамор · Арка", 5, crate::construction::Form::Arch),
    1365 Build85R1 => crate::construction::block_info("Мрамор · Арка", 5, crate::construction::Form::Arch),
    1366 Build85R2 => crate::construction::block_info("Мрамор · Арка", 5, crate::construction::Form::Arch),
    1367 Build85R3 => crate::construction::block_info("Мрамор · Арка", 5, crate::construction::Form::Arch),
    1368 Build86R0 => crate::construction::block_info("Железо · Арка", 6, crate::construction::Form::Arch),
    1369 Build86R1 => crate::construction::block_info("Железо · Арка", 6, crate::construction::Form::Arch),
    1370 Build86R2 => crate::construction::block_info("Железо · Арка", 6, crate::construction::Form::Arch),
    1371 Build86R3 => crate::construction::block_info("Железо · Арка", 6, crate::construction::Form::Arch),
    1372 Build87R0 => crate::construction::block_info("Медь · Арка", 7, crate::construction::Form::Arch),
    1373 Build87R1 => crate::construction::block_info("Медь · Арка", 7, crate::construction::Form::Arch),
    1374 Build87R2 => crate::construction::block_info("Медь · Арка", 7, crate::construction::Form::Arch),
    1375 Build87R3 => crate::construction::block_info("Медь · Арка", 7, crate::construction::Form::Arch),
    1376 Build88R0 => crate::construction::block_info("Дуб · Полуарка", 0, crate::construction::Form::HalfArch),
    1377 Build88R1 => crate::construction::block_info("Дуб · Полуарка", 0, crate::construction::Form::HalfArch),
    1378 Build88R2 => crate::construction::block_info("Дуб · Полуарка", 0, crate::construction::Form::HalfArch),
    1379 Build88R3 => crate::construction::block_info("Дуб · Полуарка", 0, crate::construction::Form::HalfArch),
    1380 Build89R0 => crate::construction::block_info("Берёза · Полуарка", 1, crate::construction::Form::HalfArch),
    1381 Build89R1 => crate::construction::block_info("Берёза · Полуарка", 1, crate::construction::Form::HalfArch),
    1382 Build89R2 => crate::construction::block_info("Берёза · Полуарка", 1, crate::construction::Form::HalfArch),
    1383 Build89R3 => crate::construction::block_info("Берёза · Полуарка", 1, crate::construction::Form::HalfArch),
    1384 Build90R0 => crate::construction::block_info("Ель · Полуарка", 2, crate::construction::Form::HalfArch),
    1385 Build90R1 => crate::construction::block_info("Ель · Полуарка", 2, crate::construction::Form::HalfArch),
    1386 Build90R2 => crate::construction::block_info("Ель · Полуарка", 2, crate::construction::Form::HalfArch),
    1387 Build90R3 => crate::construction::block_info("Ель · Полуарка", 2, crate::construction::Form::HalfArch),
    1388 Build91R0 => crate::construction::block_info("Камень · Полуарка", 3, crate::construction::Form::HalfArch),
    1389 Build91R1 => crate::construction::block_info("Камень · Полуарка", 3, crate::construction::Form::HalfArch),
    1390 Build91R2 => crate::construction::block_info("Камень · Полуарка", 3, crate::construction::Form::HalfArch),
    1391 Build91R3 => crate::construction::block_info("Камень · Полуарка", 3, crate::construction::Form::HalfArch),
    1392 Build92R0 => crate::construction::block_info("Кирпич · Полуарка", 4, crate::construction::Form::HalfArch),
    1393 Build92R1 => crate::construction::block_info("Кирпич · Полуарка", 4, crate::construction::Form::HalfArch),
    1394 Build92R2 => crate::construction::block_info("Кирпич · Полуарка", 4, crate::construction::Form::HalfArch),
    1395 Build92R3 => crate::construction::block_info("Кирпич · Полуарка", 4, crate::construction::Form::HalfArch),
    1396 Build93R0 => crate::construction::block_info("Мрамор · Полуарка", 5, crate::construction::Form::HalfArch),
    1397 Build93R1 => crate::construction::block_info("Мрамор · Полуарка", 5, crate::construction::Form::HalfArch),
    1398 Build93R2 => crate::construction::block_info("Мрамор · Полуарка", 5, crate::construction::Form::HalfArch),
    1399 Build93R3 => crate::construction::block_info("Мрамор · Полуарка", 5, crate::construction::Form::HalfArch),
    1400 Build94R0 => crate::construction::block_info("Железо · Полуарка", 6, crate::construction::Form::HalfArch),
    1401 Build94R1 => crate::construction::block_info("Железо · Полуарка", 6, crate::construction::Form::HalfArch),
    1402 Build94R2 => crate::construction::block_info("Железо · Полуарка", 6, crate::construction::Form::HalfArch),
    1403 Build94R3 => crate::construction::block_info("Железо · Полуарка", 6, crate::construction::Form::HalfArch),
    1404 Build95R0 => crate::construction::block_info("Медь · Полуарка", 7, crate::construction::Form::HalfArch),
    1405 Build95R1 => crate::construction::block_info("Медь · Полуарка", 7, crate::construction::Form::HalfArch),
    1406 Build95R2 => crate::construction::block_info("Медь · Полуарка", 7, crate::construction::Form::HalfArch),
    1407 Build95R3 => crate::construction::block_info("Медь · Полуарка", 7, crate::construction::Form::HalfArch),
    1408 Build96R0 => crate::construction::block_info("Дуб · Панель", 0, crate::construction::Form::Panel),
    1409 Build96R1 => crate::construction::block_info("Дуб · Панель", 0, crate::construction::Form::Panel),
    1410 Build96R2 => crate::construction::block_info("Дуб · Панель", 0, crate::construction::Form::Panel),
    1411 Build96R3 => crate::construction::block_info("Дуб · Панель", 0, crate::construction::Form::Panel),
    1412 Build97R0 => crate::construction::block_info("Берёза · Панель", 1, crate::construction::Form::Panel),
    1413 Build97R1 => crate::construction::block_info("Берёза · Панель", 1, crate::construction::Form::Panel),
    1414 Build97R2 => crate::construction::block_info("Берёза · Панель", 1, crate::construction::Form::Panel),
    1415 Build97R3 => crate::construction::block_info("Берёза · Панель", 1, crate::construction::Form::Panel),
    1416 Build98R0 => crate::construction::block_info("Ель · Панель", 2, crate::construction::Form::Panel),
    1417 Build98R1 => crate::construction::block_info("Ель · Панель", 2, crate::construction::Form::Panel),
    1418 Build98R2 => crate::construction::block_info("Ель · Панель", 2, crate::construction::Form::Panel),
    1419 Build98R3 => crate::construction::block_info("Ель · Панель", 2, crate::construction::Form::Panel),
    1420 Build99R0 => crate::construction::block_info("Камень · Панель", 3, crate::construction::Form::Panel),
    1421 Build99R1 => crate::construction::block_info("Камень · Панель", 3, crate::construction::Form::Panel),
    1422 Build99R2 => crate::construction::block_info("Камень · Панель", 3, crate::construction::Form::Panel),
    1423 Build99R3 => crate::construction::block_info("Камень · Панель", 3, crate::construction::Form::Panel),
    1424 Build100R0 => crate::construction::block_info("Кирпич · Панель", 4, crate::construction::Form::Panel),
    1425 Build100R1 => crate::construction::block_info("Кирпич · Панель", 4, crate::construction::Form::Panel),
    1426 Build100R2 => crate::construction::block_info("Кирпич · Панель", 4, crate::construction::Form::Panel),
    1427 Build100R3 => crate::construction::block_info("Кирпич · Панель", 4, crate::construction::Form::Panel),
    1428 Build101R0 => crate::construction::block_info("Мрамор · Панель", 5, crate::construction::Form::Panel),
    1429 Build101R1 => crate::construction::block_info("Мрамор · Панель", 5, crate::construction::Form::Panel),
    1430 Build101R2 => crate::construction::block_info("Мрамор · Панель", 5, crate::construction::Form::Panel),
    1431 Build101R3 => crate::construction::block_info("Мрамор · Панель", 5, crate::construction::Form::Panel),
    1432 Build102R0 => crate::construction::block_info("Железо · Панель", 6, crate::construction::Form::Panel),
    1433 Build102R1 => crate::construction::block_info("Железо · Панель", 6, crate::construction::Form::Panel),
    1434 Build102R2 => crate::construction::block_info("Железо · Панель", 6, crate::construction::Form::Panel),
    1435 Build102R3 => crate::construction::block_info("Железо · Панель", 6, crate::construction::Form::Panel),
    1436 Build103R0 => crate::construction::block_info("Медь · Панель", 7, crate::construction::Form::Panel),
    1437 Build103R1 => crate::construction::block_info("Медь · Панель", 7, crate::construction::Form::Panel),
    1438 Build103R2 => crate::construction::block_info("Медь · Панель", 7, crate::construction::Form::Panel),
    1439 Build103R3 => crate::construction::block_info("Медь · Панель", 7, crate::construction::Form::Panel),
    1440 Build104R0 => crate::construction::block_info("Дуб · Решётка", 0, crate::construction::Form::Grille),
    1441 Build104R1 => crate::construction::block_info("Дуб · Решётка", 0, crate::construction::Form::Grille),
    1442 Build104R2 => crate::construction::block_info("Дуб · Решётка", 0, crate::construction::Form::Grille),
    1443 Build104R3 => crate::construction::block_info("Дуб · Решётка", 0, crate::construction::Form::Grille),
    1444 Build105R0 => crate::construction::block_info("Берёза · Решётка", 1, crate::construction::Form::Grille),
    1445 Build105R1 => crate::construction::block_info("Берёза · Решётка", 1, crate::construction::Form::Grille),
    1446 Build105R2 => crate::construction::block_info("Берёза · Решётка", 1, crate::construction::Form::Grille),
    1447 Build105R3 => crate::construction::block_info("Берёза · Решётка", 1, crate::construction::Form::Grille),
    1448 Build106R0 => crate::construction::block_info("Ель · Решётка", 2, crate::construction::Form::Grille),
    1449 Build106R1 => crate::construction::block_info("Ель · Решётка", 2, crate::construction::Form::Grille),
    1450 Build106R2 => crate::construction::block_info("Ель · Решётка", 2, crate::construction::Form::Grille),
    1451 Build106R3 => crate::construction::block_info("Ель · Решётка", 2, crate::construction::Form::Grille),
    1452 Build107R0 => crate::construction::block_info("Камень · Решётка", 3, crate::construction::Form::Grille),
    1453 Build107R1 => crate::construction::block_info("Камень · Решётка", 3, crate::construction::Form::Grille),
    1454 Build107R2 => crate::construction::block_info("Камень · Решётка", 3, crate::construction::Form::Grille),
    1455 Build107R3 => crate::construction::block_info("Камень · Решётка", 3, crate::construction::Form::Grille),
    1456 Build108R0 => crate::construction::block_info("Кирпич · Решётка", 4, crate::construction::Form::Grille),
    1457 Build108R1 => crate::construction::block_info("Кирпич · Решётка", 4, crate::construction::Form::Grille),
    1458 Build108R2 => crate::construction::block_info("Кирпич · Решётка", 4, crate::construction::Form::Grille),
    1459 Build108R3 => crate::construction::block_info("Кирпич · Решётка", 4, crate::construction::Form::Grille),
    1460 Build109R0 => crate::construction::block_info("Мрамор · Решётка", 5, crate::construction::Form::Grille),
    1461 Build109R1 => crate::construction::block_info("Мрамор · Решётка", 5, crate::construction::Form::Grille),
    1462 Build109R2 => crate::construction::block_info("Мрамор · Решётка", 5, crate::construction::Form::Grille),
    1463 Build109R3 => crate::construction::block_info("Мрамор · Решётка", 5, crate::construction::Form::Grille),
    1464 Build110R0 => crate::construction::block_info("Железо · Решётка", 6, crate::construction::Form::Grille),
    1465 Build110R1 => crate::construction::block_info("Железо · Решётка", 6, crate::construction::Form::Grille),
    1466 Build110R2 => crate::construction::block_info("Железо · Решётка", 6, crate::construction::Form::Grille),
    1467 Build110R3 => crate::construction::block_info("Железо · Решётка", 6, crate::construction::Form::Grille),
    1468 Build111R0 => crate::construction::block_info("Медь · Решётка", 7, crate::construction::Form::Grille),
    1469 Build111R1 => crate::construction::block_info("Медь · Решётка", 7, crate::construction::Form::Grille),
    1470 Build111R2 => crate::construction::block_info("Медь · Решётка", 7, crate::construction::Form::Grille),
    1471 Build111R3 => crate::construction::block_info("Медь · Решётка", 7, crate::construction::Form::Grille),
    1472 Build112R0 => crate::construction::block_info("Стекло · Дуговая секция 15° R½", 8, crate::construction::Form::Arc15Small),
    1473 Build112R1 => crate::construction::block_info("Стекло · Дуговая секция 15° R½", 8, crate::construction::Form::Arc15Small),
    1474 Build112R2 => crate::construction::block_info("Стекло · Дуговая секция 15° R½", 8, crate::construction::Form::Arc15Small),
    1475 Build112R3 => crate::construction::block_info("Стекло · Дуговая секция 15° R½", 8, crate::construction::Form::Arc15Small),
    1476 Build113R0 => crate::construction::block_info("Красное стекло · Дуговая секция 15° R½", 9, crate::construction::Form::Arc15Small),
    1477 Build113R1 => crate::construction::block_info("Красное стекло · Дуговая секция 15° R½", 9, crate::construction::Form::Arc15Small),
    1478 Build113R2 => crate::construction::block_info("Красное стекло · Дуговая секция 15° R½", 9, crate::construction::Form::Arc15Small),
    1479 Build113R3 => crate::construction::block_info("Красное стекло · Дуговая секция 15° R½", 9, crate::construction::Form::Arc15Small),
    1480 Build114R0 => crate::construction::block_info("Синее стекло · Дуговая секция 15° R½", 10, crate::construction::Form::Arc15Small),
    1481 Build114R1 => crate::construction::block_info("Синее стекло · Дуговая секция 15° R½", 10, crate::construction::Form::Arc15Small),
    1482 Build114R2 => crate::construction::block_info("Синее стекло · Дуговая секция 15° R½", 10, crate::construction::Form::Arc15Small),
    1483 Build114R3 => crate::construction::block_info("Синее стекло · Дуговая секция 15° R½", 10, crate::construction::Form::Arc15Small),
    1484 Build115R0 => crate::construction::block_info("Зелёное стекло · Дуговая секция 15° R½", 11, crate::construction::Form::Arc15Small),
    1485 Build115R1 => crate::construction::block_info("Зелёное стекло · Дуговая секция 15° R½", 11, crate::construction::Form::Arc15Small),
    1486 Build115R2 => crate::construction::block_info("Зелёное стекло · Дуговая секция 15° R½", 11, crate::construction::Form::Arc15Small),
    1487 Build115R3 => crate::construction::block_info("Зелёное стекло · Дуговая секция 15° R½", 11, crate::construction::Form::Arc15Small),
    1488 Build116R0 => crate::construction::block_info("Стекло · Дуговая секция 30° R½", 8, crate::construction::Form::Arc30Small),
    1489 Build116R1 => crate::construction::block_info("Стекло · Дуговая секция 30° R½", 8, crate::construction::Form::Arc30Small),
    1490 Build116R2 => crate::construction::block_info("Стекло · Дуговая секция 30° R½", 8, crate::construction::Form::Arc30Small),
    1491 Build116R3 => crate::construction::block_info("Стекло · Дуговая секция 30° R½", 8, crate::construction::Form::Arc30Small),
    1492 Build117R0 => crate::construction::block_info("Красное стекло · Дуговая секция 30° R½", 9, crate::construction::Form::Arc30Small),
    1493 Build117R1 => crate::construction::block_info("Красное стекло · Дуговая секция 30° R½", 9, crate::construction::Form::Arc30Small),
    1494 Build117R2 => crate::construction::block_info("Красное стекло · Дуговая секция 30° R½", 9, crate::construction::Form::Arc30Small),
    1495 Build117R3 => crate::construction::block_info("Красное стекло · Дуговая секция 30° R½", 9, crate::construction::Form::Arc30Small),
    1496 Build118R0 => crate::construction::block_info("Синее стекло · Дуговая секция 30° R½", 10, crate::construction::Form::Arc30Small),
    1497 Build118R1 => crate::construction::block_info("Синее стекло · Дуговая секция 30° R½", 10, crate::construction::Form::Arc30Small),
    1498 Build118R2 => crate::construction::block_info("Синее стекло · Дуговая секция 30° R½", 10, crate::construction::Form::Arc30Small),
    1499 Build118R3 => crate::construction::block_info("Синее стекло · Дуговая секция 30° R½", 10, crate::construction::Form::Arc30Small),
    1500 Build119R0 => crate::construction::block_info("Зелёное стекло · Дуговая секция 30° R½", 11, crate::construction::Form::Arc30Small),
    1501 Build119R1 => crate::construction::block_info("Зелёное стекло · Дуговая секция 30° R½", 11, crate::construction::Form::Arc30Small),
    1502 Build119R2 => crate::construction::block_info("Зелёное стекло · Дуговая секция 30° R½", 11, crate::construction::Form::Arc30Small),
    1503 Build119R3 => crate::construction::block_info("Зелёное стекло · Дуговая секция 30° R½", 11, crate::construction::Form::Arc30Small),
    1504 Build120R0 => crate::construction::block_info("Стекло · Дуговая секция 45° R½", 8, crate::construction::Form::Arc45Small),
    1505 Build120R1 => crate::construction::block_info("Стекло · Дуговая секция 45° R½", 8, crate::construction::Form::Arc45Small),
    1506 Build120R2 => crate::construction::block_info("Стекло · Дуговая секция 45° R½", 8, crate::construction::Form::Arc45Small),
    1507 Build120R3 => crate::construction::block_info("Стекло · Дуговая секция 45° R½", 8, crate::construction::Form::Arc45Small),
    1508 Build121R0 => crate::construction::block_info("Красное стекло · Дуговая секция 45° R½", 9, crate::construction::Form::Arc45Small),
    1509 Build121R1 => crate::construction::block_info("Красное стекло · Дуговая секция 45° R½", 9, crate::construction::Form::Arc45Small),
    1510 Build121R2 => crate::construction::block_info("Красное стекло · Дуговая секция 45° R½", 9, crate::construction::Form::Arc45Small),
    1511 Build121R3 => crate::construction::block_info("Красное стекло · Дуговая секция 45° R½", 9, crate::construction::Form::Arc45Small),
    1512 Build122R0 => crate::construction::block_info("Синее стекло · Дуговая секция 45° R½", 10, crate::construction::Form::Arc45Small),
    1513 Build122R1 => crate::construction::block_info("Синее стекло · Дуговая секция 45° R½", 10, crate::construction::Form::Arc45Small),
    1514 Build122R2 => crate::construction::block_info("Синее стекло · Дуговая секция 45° R½", 10, crate::construction::Form::Arc45Small),
    1515 Build122R3 => crate::construction::block_info("Синее стекло · Дуговая секция 45° R½", 10, crate::construction::Form::Arc45Small),
    1516 Build123R0 => crate::construction::block_info("Зелёное стекло · Дуговая секция 45° R½", 11, crate::construction::Form::Arc45Small),
    1517 Build123R1 => crate::construction::block_info("Зелёное стекло · Дуговая секция 45° R½", 11, crate::construction::Form::Arc45Small),
    1518 Build123R2 => crate::construction::block_info("Зелёное стекло · Дуговая секция 45° R½", 11, crate::construction::Form::Arc45Small),
    1519 Build123R3 => crate::construction::block_info("Зелёное стекло · Дуговая секция 45° R½", 11, crate::construction::Form::Arc45Small),
    1520 Build124R0 => crate::construction::block_info("Стекло · Дуговая секция 90° R½", 8, crate::construction::Form::Arc90Small),
    1521 Build124R1 => crate::construction::block_info("Стекло · Дуговая секция 90° R½", 8, crate::construction::Form::Arc90Small),
    1522 Build124R2 => crate::construction::block_info("Стекло · Дуговая секция 90° R½", 8, crate::construction::Form::Arc90Small),
    1523 Build124R3 => crate::construction::block_info("Стекло · Дуговая секция 90° R½", 8, crate::construction::Form::Arc90Small),
    1524 Build125R0 => crate::construction::block_info("Красное стекло · Дуговая секция 90° R½", 9, crate::construction::Form::Arc90Small),
    1525 Build125R1 => crate::construction::block_info("Красное стекло · Дуговая секция 90° R½", 9, crate::construction::Form::Arc90Small),
    1526 Build125R2 => crate::construction::block_info("Красное стекло · Дуговая секция 90° R½", 9, crate::construction::Form::Arc90Small),
    1527 Build125R3 => crate::construction::block_info("Красное стекло · Дуговая секция 90° R½", 9, crate::construction::Form::Arc90Small),
    1528 Build126R0 => crate::construction::block_info("Синее стекло · Дуговая секция 90° R½", 10, crate::construction::Form::Arc90Small),
    1529 Build126R1 => crate::construction::block_info("Синее стекло · Дуговая секция 90° R½", 10, crate::construction::Form::Arc90Small),
    1530 Build126R2 => crate::construction::block_info("Синее стекло · Дуговая секция 90° R½", 10, crate::construction::Form::Arc90Small),
    1531 Build126R3 => crate::construction::block_info("Синее стекло · Дуговая секция 90° R½", 10, crate::construction::Form::Arc90Small),
    1532 Build127R0 => crate::construction::block_info("Зелёное стекло · Дуговая секция 90° R½", 11, crate::construction::Form::Arc90Small),
    1533 Build127R1 => crate::construction::block_info("Зелёное стекло · Дуговая секция 90° R½", 11, crate::construction::Form::Arc90Small),
    1534 Build127R2 => crate::construction::block_info("Зелёное стекло · Дуговая секция 90° R½", 11, crate::construction::Form::Arc90Small),
    1535 Build127R3 => crate::construction::block_info("Зелёное стекло · Дуговая секция 90° R½", 11, crate::construction::Form::Arc90Small),
    1536 Build128R0 => crate::construction::block_info("Стекло · Дуговая секция 15° R¾", 8, crate::construction::Form::Arc15Medium),
    1537 Build128R1 => crate::construction::block_info("Стекло · Дуговая секция 15° R¾", 8, crate::construction::Form::Arc15Medium),
    1538 Build128R2 => crate::construction::block_info("Стекло · Дуговая секция 15° R¾", 8, crate::construction::Form::Arc15Medium),
    1539 Build128R3 => crate::construction::block_info("Стекло · Дуговая секция 15° R¾", 8, crate::construction::Form::Arc15Medium),
    1540 Build129R0 => crate::construction::block_info("Красное стекло · Дуговая секция 15° R¾", 9, crate::construction::Form::Arc15Medium),
    1541 Build129R1 => crate::construction::block_info("Красное стекло · Дуговая секция 15° R¾", 9, crate::construction::Form::Arc15Medium),
    1542 Build129R2 => crate::construction::block_info("Красное стекло · Дуговая секция 15° R¾", 9, crate::construction::Form::Arc15Medium),
    1543 Build129R3 => crate::construction::block_info("Красное стекло · Дуговая секция 15° R¾", 9, crate::construction::Form::Arc15Medium),
    1544 Build130R0 => crate::construction::block_info("Синее стекло · Дуговая секция 15° R¾", 10, crate::construction::Form::Arc15Medium),
    1545 Build130R1 => crate::construction::block_info("Синее стекло · Дуговая секция 15° R¾", 10, crate::construction::Form::Arc15Medium),
    1546 Build130R2 => crate::construction::block_info("Синее стекло · Дуговая секция 15° R¾", 10, crate::construction::Form::Arc15Medium),
    1547 Build130R3 => crate::construction::block_info("Синее стекло · Дуговая секция 15° R¾", 10, crate::construction::Form::Arc15Medium),
    1548 Build131R0 => crate::construction::block_info("Зелёное стекло · Дуговая секция 15° R¾", 11, crate::construction::Form::Arc15Medium),
    1549 Build131R1 => crate::construction::block_info("Зелёное стекло · Дуговая секция 15° R¾", 11, crate::construction::Form::Arc15Medium),
    1550 Build131R2 => crate::construction::block_info("Зелёное стекло · Дуговая секция 15° R¾", 11, crate::construction::Form::Arc15Medium),
    1551 Build131R3 => crate::construction::block_info("Зелёное стекло · Дуговая секция 15° R¾", 11, crate::construction::Form::Arc15Medium),
    1552 Build132R0 => crate::construction::block_info("Стекло · Дуговая секция 30° R¾", 8, crate::construction::Form::Arc30Medium),
    1553 Build132R1 => crate::construction::block_info("Стекло · Дуговая секция 30° R¾", 8, crate::construction::Form::Arc30Medium),
    1554 Build132R2 => crate::construction::block_info("Стекло · Дуговая секция 30° R¾", 8, crate::construction::Form::Arc30Medium),
    1555 Build132R3 => crate::construction::block_info("Стекло · Дуговая секция 30° R¾", 8, crate::construction::Form::Arc30Medium),
    1556 Build133R0 => crate::construction::block_info("Красное стекло · Дуговая секция 30° R¾", 9, crate::construction::Form::Arc30Medium),
    1557 Build133R1 => crate::construction::block_info("Красное стекло · Дуговая секция 30° R¾", 9, crate::construction::Form::Arc30Medium),
    1558 Build133R2 => crate::construction::block_info("Красное стекло · Дуговая секция 30° R¾", 9, crate::construction::Form::Arc30Medium),
    1559 Build133R3 => crate::construction::block_info("Красное стекло · Дуговая секция 30° R¾", 9, crate::construction::Form::Arc30Medium),
    1560 Build134R0 => crate::construction::block_info("Синее стекло · Дуговая секция 30° R¾", 10, crate::construction::Form::Arc30Medium),
    1561 Build134R1 => crate::construction::block_info("Синее стекло · Дуговая секция 30° R¾", 10, crate::construction::Form::Arc30Medium),
    1562 Build134R2 => crate::construction::block_info("Синее стекло · Дуговая секция 30° R¾", 10, crate::construction::Form::Arc30Medium),
    1563 Build134R3 => crate::construction::block_info("Синее стекло · Дуговая секция 30° R¾", 10, crate::construction::Form::Arc30Medium),
    1564 Build135R0 => crate::construction::block_info("Зелёное стекло · Дуговая секция 30° R¾", 11, crate::construction::Form::Arc30Medium),
    1565 Build135R1 => crate::construction::block_info("Зелёное стекло · Дуговая секция 30° R¾", 11, crate::construction::Form::Arc30Medium),
    1566 Build135R2 => crate::construction::block_info("Зелёное стекло · Дуговая секция 30° R¾", 11, crate::construction::Form::Arc30Medium),
    1567 Build135R3 => crate::construction::block_info("Зелёное стекло · Дуговая секция 30° R¾", 11, crate::construction::Form::Arc30Medium),
    1568 Build136R0 => crate::construction::block_info("Стекло · Дуговая секция 45° R¾", 8, crate::construction::Form::Arc45Medium),
    1569 Build136R1 => crate::construction::block_info("Стекло · Дуговая секция 45° R¾", 8, crate::construction::Form::Arc45Medium),
    1570 Build136R2 => crate::construction::block_info("Стекло · Дуговая секция 45° R¾", 8, crate::construction::Form::Arc45Medium),
    1571 Build136R3 => crate::construction::block_info("Стекло · Дуговая секция 45° R¾", 8, crate::construction::Form::Arc45Medium),
    1572 Build137R0 => crate::construction::block_info("Красное стекло · Дуговая секция 45° R¾", 9, crate::construction::Form::Arc45Medium),
    1573 Build137R1 => crate::construction::block_info("Красное стекло · Дуговая секция 45° R¾", 9, crate::construction::Form::Arc45Medium),
    1574 Build137R2 => crate::construction::block_info("Красное стекло · Дуговая секция 45° R¾", 9, crate::construction::Form::Arc45Medium),
    1575 Build137R3 => crate::construction::block_info("Красное стекло · Дуговая секция 45° R¾", 9, crate::construction::Form::Arc45Medium),
    1576 Build138R0 => crate::construction::block_info("Синее стекло · Дуговая секция 45° R¾", 10, crate::construction::Form::Arc45Medium),
    1577 Build138R1 => crate::construction::block_info("Синее стекло · Дуговая секция 45° R¾", 10, crate::construction::Form::Arc45Medium),
    1578 Build138R2 => crate::construction::block_info("Синее стекло · Дуговая секция 45° R¾", 10, crate::construction::Form::Arc45Medium),
    1579 Build138R3 => crate::construction::block_info("Синее стекло · Дуговая секция 45° R¾", 10, crate::construction::Form::Arc45Medium),
    1580 Build139R0 => crate::construction::block_info("Зелёное стекло · Дуговая секция 45° R¾", 11, crate::construction::Form::Arc45Medium),
    1581 Build139R1 => crate::construction::block_info("Зелёное стекло · Дуговая секция 45° R¾", 11, crate::construction::Form::Arc45Medium),
    1582 Build139R2 => crate::construction::block_info("Зелёное стекло · Дуговая секция 45° R¾", 11, crate::construction::Form::Arc45Medium),
    1583 Build139R3 => crate::construction::block_info("Зелёное стекло · Дуговая секция 45° R¾", 11, crate::construction::Form::Arc45Medium),
    1584 Build140R0 => crate::construction::block_info("Стекло · Дуговая секция 90° R¾", 8, crate::construction::Form::Arc90Medium),
    1585 Build140R1 => crate::construction::block_info("Стекло · Дуговая секция 90° R¾", 8, crate::construction::Form::Arc90Medium),
    1586 Build140R2 => crate::construction::block_info("Стекло · Дуговая секция 90° R¾", 8, crate::construction::Form::Arc90Medium),
    1587 Build140R3 => crate::construction::block_info("Стекло · Дуговая секция 90° R¾", 8, crate::construction::Form::Arc90Medium),
    1588 Build141R0 => crate::construction::block_info("Красное стекло · Дуговая секция 90° R¾", 9, crate::construction::Form::Arc90Medium),
    1589 Build141R1 => crate::construction::block_info("Красное стекло · Дуговая секция 90° R¾", 9, crate::construction::Form::Arc90Medium),
    1590 Build141R2 => crate::construction::block_info("Красное стекло · Дуговая секция 90° R¾", 9, crate::construction::Form::Arc90Medium),
    1591 Build141R3 => crate::construction::block_info("Красное стекло · Дуговая секция 90° R¾", 9, crate::construction::Form::Arc90Medium),
    1592 Build142R0 => crate::construction::block_info("Синее стекло · Дуговая секция 90° R¾", 10, crate::construction::Form::Arc90Medium),
    1593 Build142R1 => crate::construction::block_info("Синее стекло · Дуговая секция 90° R¾", 10, crate::construction::Form::Arc90Medium),
    1594 Build142R2 => crate::construction::block_info("Синее стекло · Дуговая секция 90° R¾", 10, crate::construction::Form::Arc90Medium),
    1595 Build142R3 => crate::construction::block_info("Синее стекло · Дуговая секция 90° R¾", 10, crate::construction::Form::Arc90Medium),
    1596 Build143R0 => crate::construction::block_info("Зелёное стекло · Дуговая секция 90° R¾", 11, crate::construction::Form::Arc90Medium),
    1597 Build143R1 => crate::construction::block_info("Зелёное стекло · Дуговая секция 90° R¾", 11, crate::construction::Form::Arc90Medium),
    1598 Build143R2 => crate::construction::block_info("Зелёное стекло · Дуговая секция 90° R¾", 11, crate::construction::Form::Arc90Medium),
    1599 Build143R3 => crate::construction::block_info("Зелёное стекло · Дуговая секция 90° R¾", 11, crate::construction::Form::Arc90Medium),
    1600 Build144R0 => crate::construction::block_info("Стекло · Дуговая секция 15° R1", 8, crate::construction::Form::Arc15Large),
    1601 Build144R1 => crate::construction::block_info("Стекло · Дуговая секция 15° R1", 8, crate::construction::Form::Arc15Large),
    1602 Build144R2 => crate::construction::block_info("Стекло · Дуговая секция 15° R1", 8, crate::construction::Form::Arc15Large),
    1603 Build144R3 => crate::construction::block_info("Стекло · Дуговая секция 15° R1", 8, crate::construction::Form::Arc15Large),
    1604 Build145R0 => crate::construction::block_info("Красное стекло · Дуговая секция 15° R1", 9, crate::construction::Form::Arc15Large),
    1605 Build145R1 => crate::construction::block_info("Красное стекло · Дуговая секция 15° R1", 9, crate::construction::Form::Arc15Large),
    1606 Build145R2 => crate::construction::block_info("Красное стекло · Дуговая секция 15° R1", 9, crate::construction::Form::Arc15Large),
    1607 Build145R3 => crate::construction::block_info("Красное стекло · Дуговая секция 15° R1", 9, crate::construction::Form::Arc15Large),
    1608 Build146R0 => crate::construction::block_info("Синее стекло · Дуговая секция 15° R1", 10, crate::construction::Form::Arc15Large),
    1609 Build146R1 => crate::construction::block_info("Синее стекло · Дуговая секция 15° R1", 10, crate::construction::Form::Arc15Large),
    1610 Build146R2 => crate::construction::block_info("Синее стекло · Дуговая секция 15° R1", 10, crate::construction::Form::Arc15Large),
    1611 Build146R3 => crate::construction::block_info("Синее стекло · Дуговая секция 15° R1", 10, crate::construction::Form::Arc15Large),
    1612 Build147R0 => crate::construction::block_info("Зелёное стекло · Дуговая секция 15° R1", 11, crate::construction::Form::Arc15Large),
    1613 Build147R1 => crate::construction::block_info("Зелёное стекло · Дуговая секция 15° R1", 11, crate::construction::Form::Arc15Large),
    1614 Build147R2 => crate::construction::block_info("Зелёное стекло · Дуговая секция 15° R1", 11, crate::construction::Form::Arc15Large),
    1615 Build147R3 => crate::construction::block_info("Зелёное стекло · Дуговая секция 15° R1", 11, crate::construction::Form::Arc15Large),
    1616 Build148R0 => crate::construction::block_info("Стекло · Дуговая секция 30° R1", 8, crate::construction::Form::Arc30Large),
    1617 Build148R1 => crate::construction::block_info("Стекло · Дуговая секция 30° R1", 8, crate::construction::Form::Arc30Large),
    1618 Build148R2 => crate::construction::block_info("Стекло · Дуговая секция 30° R1", 8, crate::construction::Form::Arc30Large),
    1619 Build148R3 => crate::construction::block_info("Стекло · Дуговая секция 30° R1", 8, crate::construction::Form::Arc30Large),
    1620 Build149R0 => crate::construction::block_info("Красное стекло · Дуговая секция 30° R1", 9, crate::construction::Form::Arc30Large),
    1621 Build149R1 => crate::construction::block_info("Красное стекло · Дуговая секция 30° R1", 9, crate::construction::Form::Arc30Large),
    1622 Build149R2 => crate::construction::block_info("Красное стекло · Дуговая секция 30° R1", 9, crate::construction::Form::Arc30Large),
    1623 Build149R3 => crate::construction::block_info("Красное стекло · Дуговая секция 30° R1", 9, crate::construction::Form::Arc30Large),
    1624 Build150R0 => crate::construction::block_info("Синее стекло · Дуговая секция 30° R1", 10, crate::construction::Form::Arc30Large),
    1625 Build150R1 => crate::construction::block_info("Синее стекло · Дуговая секция 30° R1", 10, crate::construction::Form::Arc30Large),
    1626 Build150R2 => crate::construction::block_info("Синее стекло · Дуговая секция 30° R1", 10, crate::construction::Form::Arc30Large),
    1627 Build150R3 => crate::construction::block_info("Синее стекло · Дуговая секция 30° R1", 10, crate::construction::Form::Arc30Large),
    1628 Build151R0 => crate::construction::block_info("Зелёное стекло · Дуговая секция 30° R1", 11, crate::construction::Form::Arc30Large),
    1629 Build151R1 => crate::construction::block_info("Зелёное стекло · Дуговая секция 30° R1", 11, crate::construction::Form::Arc30Large),
    1630 Build151R2 => crate::construction::block_info("Зелёное стекло · Дуговая секция 30° R1", 11, crate::construction::Form::Arc30Large),
    1631 Build151R3 => crate::construction::block_info("Зелёное стекло · Дуговая секция 30° R1", 11, crate::construction::Form::Arc30Large),
    1632 Build152R0 => crate::construction::block_info("Стекло · Дуговая секция 45° R1", 8, crate::construction::Form::Arc45Large),
    1633 Build152R1 => crate::construction::block_info("Стекло · Дуговая секция 45° R1", 8, crate::construction::Form::Arc45Large),
    1634 Build152R2 => crate::construction::block_info("Стекло · Дуговая секция 45° R1", 8, crate::construction::Form::Arc45Large),
    1635 Build152R3 => crate::construction::block_info("Стекло · Дуговая секция 45° R1", 8, crate::construction::Form::Arc45Large),
    1636 Build153R0 => crate::construction::block_info("Красное стекло · Дуговая секция 45° R1", 9, crate::construction::Form::Arc45Large),
    1637 Build153R1 => crate::construction::block_info("Красное стекло · Дуговая секция 45° R1", 9, crate::construction::Form::Arc45Large),
    1638 Build153R2 => crate::construction::block_info("Красное стекло · Дуговая секция 45° R1", 9, crate::construction::Form::Arc45Large),
    1639 Build153R3 => crate::construction::block_info("Красное стекло · Дуговая секция 45° R1", 9, crate::construction::Form::Arc45Large),
    1640 Build154R0 => crate::construction::block_info("Синее стекло · Дуговая секция 45° R1", 10, crate::construction::Form::Arc45Large),
    1641 Build154R1 => crate::construction::block_info("Синее стекло · Дуговая секция 45° R1", 10, crate::construction::Form::Arc45Large),
    1642 Build154R2 => crate::construction::block_info("Синее стекло · Дуговая секция 45° R1", 10, crate::construction::Form::Arc45Large),
    1643 Build154R3 => crate::construction::block_info("Синее стекло · Дуговая секция 45° R1", 10, crate::construction::Form::Arc45Large),
    1644 Build155R0 => crate::construction::block_info("Зелёное стекло · Дуговая секция 45° R1", 11, crate::construction::Form::Arc45Large),
    1645 Build155R1 => crate::construction::block_info("Зелёное стекло · Дуговая секция 45° R1", 11, crate::construction::Form::Arc45Large),
    1646 Build155R2 => crate::construction::block_info("Зелёное стекло · Дуговая секция 45° R1", 11, crate::construction::Form::Arc45Large),
    1647 Build155R3 => crate::construction::block_info("Зелёное стекло · Дуговая секция 45° R1", 11, crate::construction::Form::Arc45Large),
    1648 Build156R0 => crate::construction::block_info("Стекло · Дуговая секция 90° R1", 8, crate::construction::Form::Arc90Large),
    1649 Build156R1 => crate::construction::block_info("Стекло · Дуговая секция 90° R1", 8, crate::construction::Form::Arc90Large),
    1650 Build156R2 => crate::construction::block_info("Стекло · Дуговая секция 90° R1", 8, crate::construction::Form::Arc90Large),
    1651 Build156R3 => crate::construction::block_info("Стекло · Дуговая секция 90° R1", 8, crate::construction::Form::Arc90Large),
    1652 Build157R0 => crate::construction::block_info("Красное стекло · Дуговая секция 90° R1", 9, crate::construction::Form::Arc90Large),
    1653 Build157R1 => crate::construction::block_info("Красное стекло · Дуговая секция 90° R1", 9, crate::construction::Form::Arc90Large),
    1654 Build157R2 => crate::construction::block_info("Красное стекло · Дуговая секция 90° R1", 9, crate::construction::Form::Arc90Large),
    1655 Build157R3 => crate::construction::block_info("Красное стекло · Дуговая секция 90° R1", 9, crate::construction::Form::Arc90Large),
    1656 Build158R0 => crate::construction::block_info("Синее стекло · Дуговая секция 90° R1", 10, crate::construction::Form::Arc90Large),
    1657 Build158R1 => crate::construction::block_info("Синее стекло · Дуговая секция 90° R1", 10, crate::construction::Form::Arc90Large),
    1658 Build158R2 => crate::construction::block_info("Синее стекло · Дуговая секция 90° R1", 10, crate::construction::Form::Arc90Large),
    1659 Build158R3 => crate::construction::block_info("Синее стекло · Дуговая секция 90° R1", 10, crate::construction::Form::Arc90Large),
    1660 Build159R0 => crate::construction::block_info("Зелёное стекло · Дуговая секция 90° R1", 11, crate::construction::Form::Arc90Large),
    1661 Build159R1 => crate::construction::block_info("Зелёное стекло · Дуговая секция 90° R1", 11, crate::construction::Form::Arc90Large),
    1662 Build159R2 => crate::construction::block_info("Зелёное стекло · Дуговая секция 90° R1", 11, crate::construction::Form::Arc90Large),
    1663 Build159R3 => crate::construction::block_info("Зелёное стекло · Дуговая секция 90° R1", 11, crate::construction::Form::Arc90Large),
    1664 Build160R0 => crate::construction::block_info("Стекло · Купол", 8, crate::construction::Form::Dome),
    1665 Build160R1 => crate::construction::block_info("Стекло · Купол", 8, crate::construction::Form::Dome),
    1666 Build160R2 => crate::construction::block_info("Стекло · Купол", 8, crate::construction::Form::Dome),
    1667 Build160R3 => crate::construction::block_info("Стекло · Купол", 8, crate::construction::Form::Dome),
    1668 Build161R0 => crate::construction::block_info("Красное стекло · Купол", 9, crate::construction::Form::Dome),
    1669 Build161R1 => crate::construction::block_info("Красное стекло · Купол", 9, crate::construction::Form::Dome),
    1670 Build161R2 => crate::construction::block_info("Красное стекло · Купол", 9, crate::construction::Form::Dome),
    1671 Build161R3 => crate::construction::block_info("Красное стекло · Купол", 9, crate::construction::Form::Dome),
    1672 Build162R0 => crate::construction::block_info("Синее стекло · Купол", 10, crate::construction::Form::Dome),
    1673 Build162R1 => crate::construction::block_info("Синее стекло · Купол", 10, crate::construction::Form::Dome),
    1674 Build162R2 => crate::construction::block_info("Синее стекло · Купол", 10, crate::construction::Form::Dome),
    1675 Build162R3 => crate::construction::block_info("Синее стекло · Купол", 10, crate::construction::Form::Dome),
    1676 Build163R0 => crate::construction::block_info("Зелёное стекло · Купол", 11, crate::construction::Form::Dome),
    1677 Build163R1 => crate::construction::block_info("Зелёное стекло · Купол", 11, crate::construction::Form::Dome),
    1678 Build163R2 => crate::construction::block_info("Зелёное стекло · Купол", 11, crate::construction::Form::Dome),
    1679 Build163R3 => crate::construction::block_info("Зелёное стекло · Купол", 11, crate::construction::Form::Dome),
    1680 Build164R0 => crate::construction::block_info("Стекло · Половина купола", 8, crate::construction::Form::DomeHalf),
    1681 Build164R1 => crate::construction::block_info("Стекло · Половина купола", 8, crate::construction::Form::DomeHalf),
    1682 Build164R2 => crate::construction::block_info("Стекло · Половина купола", 8, crate::construction::Form::DomeHalf),
    1683 Build164R3 => crate::construction::block_info("Стекло · Половина купола", 8, crate::construction::Form::DomeHalf),
    1684 Build165R0 => crate::construction::block_info("Красное стекло · Половина купола", 9, crate::construction::Form::DomeHalf),
    1685 Build165R1 => crate::construction::block_info("Красное стекло · Половина купола", 9, crate::construction::Form::DomeHalf),
    1686 Build165R2 => crate::construction::block_info("Красное стекло · Половина купола", 9, crate::construction::Form::DomeHalf),
    1687 Build165R3 => crate::construction::block_info("Красное стекло · Половина купола", 9, crate::construction::Form::DomeHalf),
    1688 Build166R0 => crate::construction::block_info("Синее стекло · Половина купола", 10, crate::construction::Form::DomeHalf),
    1689 Build166R1 => crate::construction::block_info("Синее стекло · Половина купола", 10, crate::construction::Form::DomeHalf),
    1690 Build166R2 => crate::construction::block_info("Синее стекло · Половина купола", 10, crate::construction::Form::DomeHalf),
    1691 Build166R3 => crate::construction::block_info("Синее стекло · Половина купола", 10, crate::construction::Form::DomeHalf),
    1692 Build167R0 => crate::construction::block_info("Зелёное стекло · Половина купола", 11, crate::construction::Form::DomeHalf),
    1693 Build167R1 => crate::construction::block_info("Зелёное стекло · Половина купола", 11, crate::construction::Form::DomeHalf),
    1694 Build167R2 => crate::construction::block_info("Зелёное стекло · Половина купола", 11, crate::construction::Form::DomeHalf),
    1695 Build167R3 => crate::construction::block_info("Зелёное стекло · Половина купола", 11, crate::construction::Form::DomeHalf),
    1696 Build168R0 => crate::construction::block_info("Стекло · Четверть купола", 8, crate::construction::Form::DomeQuarter),
    1697 Build168R1 => crate::construction::block_info("Стекло · Четверть купола", 8, crate::construction::Form::DomeQuarter),
    1698 Build168R2 => crate::construction::block_info("Стекло · Четверть купола", 8, crate::construction::Form::DomeQuarter),
    1699 Build168R3 => crate::construction::block_info("Стекло · Четверть купола", 8, crate::construction::Form::DomeQuarter),
    1700 Build169R0 => crate::construction::block_info("Красное стекло · Четверть купола", 9, crate::construction::Form::DomeQuarter),
    1701 Build169R1 => crate::construction::block_info("Красное стекло · Четверть купола", 9, crate::construction::Form::DomeQuarter),
    1702 Build169R2 => crate::construction::block_info("Красное стекло · Четверть купола", 9, crate::construction::Form::DomeQuarter),
    1703 Build169R3 => crate::construction::block_info("Красное стекло · Четверть купола", 9, crate::construction::Form::DomeQuarter),
    1704 Build170R0 => crate::construction::block_info("Синее стекло · Четверть купола", 10, crate::construction::Form::DomeQuarter),
    1705 Build170R1 => crate::construction::block_info("Синее стекло · Четверть купола", 10, crate::construction::Form::DomeQuarter),
    1706 Build170R2 => crate::construction::block_info("Синее стекло · Четверть купола", 10, crate::construction::Form::DomeQuarter),
    1707 Build170R3 => crate::construction::block_info("Синее стекло · Четверть купола", 10, crate::construction::Form::DomeQuarter),
    1708 Build171R0 => crate::construction::block_info("Зелёное стекло · Четверть купола", 11, crate::construction::Form::DomeQuarter),
    1709 Build171R1 => crate::construction::block_info("Зелёное стекло · Четверть купола", 11, crate::construction::Form::DomeQuarter),
    1710 Build171R2 => crate::construction::block_info("Зелёное стекло · Четверть купола", 11, crate::construction::Form::DomeQuarter),
    1711 Build171R3 => crate::construction::block_info("Зелёное стекло · Четверть купола", 11, crate::construction::Form::DomeQuarter),
    1712 Build172R0 => crate::construction::block_info("Дуб · Дверь", 0, crate::construction::Form::Door),
    1713 Build172R1 => crate::construction::block_info("Дуб · Дверь", 0, crate::construction::Form::Door),
    1714 Build172R2 => crate::construction::block_info("Дуб · Дверь", 0, crate::construction::Form::Door),
    1715 Build172R3 => crate::construction::block_info("Дуб · Дверь", 0, crate::construction::Form::Door),
    1716 Build173R0 => crate::construction::block_info("Берёза · Дверь", 1, crate::construction::Form::Door),
    1717 Build173R1 => crate::construction::block_info("Берёза · Дверь", 1, crate::construction::Form::Door),
    1718 Build173R2 => crate::construction::block_info("Берёза · Дверь", 1, crate::construction::Form::Door),
    1719 Build173R3 => crate::construction::block_info("Берёза · Дверь", 1, crate::construction::Form::Door),
    1720 Build174R0 => crate::construction::block_info("Ель · Дверь", 2, crate::construction::Form::Door),
    1721 Build174R1 => crate::construction::block_info("Ель · Дверь", 2, crate::construction::Form::Door),
    1722 Build174R2 => crate::construction::block_info("Ель · Дверь", 2, crate::construction::Form::Door),
    1723 Build174R3 => crate::construction::block_info("Ель · Дверь", 2, crate::construction::Form::Door),
    1724 Build175R0 => crate::construction::block_info("Железо · Дверь", 6, crate::construction::Form::Door),
    1725 Build175R1 => crate::construction::block_info("Железо · Дверь", 6, crate::construction::Form::Door),
    1726 Build175R2 => crate::construction::block_info("Железо · Дверь", 6, crate::construction::Form::Door),
    1727 Build175R3 => crate::construction::block_info("Железо · Дверь", 6, crate::construction::Form::Door),
    1728 Build176R0 => crate::construction::block_info("Медь · Дверь", 7, crate::construction::Form::Door),
    1729 Build176R1 => crate::construction::block_info("Медь · Дверь", 7, crate::construction::Form::Door),
    1730 Build176R2 => crate::construction::block_info("Медь · Дверь", 7, crate::construction::Form::Door),
    1731 Build176R3 => crate::construction::block_info("Медь · Дверь", 7, crate::construction::Form::Door),
    1732 Build177R0 => crate::construction::block_info("Стекло · Дверь", 8, crate::construction::Form::Door),
    1733 Build177R1 => crate::construction::block_info("Стекло · Дверь", 8, crate::construction::Form::Door),
    1734 Build177R2 => crate::construction::block_info("Стекло · Дверь", 8, crate::construction::Form::Door),
    1735 Build177R3 => crate::construction::block_info("Стекло · Дверь", 8, crate::construction::Form::Door),
    1736 Build178R0 => crate::construction::block_info("Дуб · Дверь (открыта)", 0, crate::construction::Form::DoorOpen),
    1737 Build178R1 => crate::construction::block_info("Дуб · Дверь (открыта)", 0, crate::construction::Form::DoorOpen),
    1738 Build178R2 => crate::construction::block_info("Дуб · Дверь (открыта)", 0, crate::construction::Form::DoorOpen),
    1739 Build178R3 => crate::construction::block_info("Дуб · Дверь (открыта)", 0, crate::construction::Form::DoorOpen),
    1740 Build179R0 => crate::construction::block_info("Берёза · Дверь (открыта)", 1, crate::construction::Form::DoorOpen),
    1741 Build179R1 => crate::construction::block_info("Берёза · Дверь (открыта)", 1, crate::construction::Form::DoorOpen),
    1742 Build179R2 => crate::construction::block_info("Берёза · Дверь (открыта)", 1, crate::construction::Form::DoorOpen),
    1743 Build179R3 => crate::construction::block_info("Берёза · Дверь (открыта)", 1, crate::construction::Form::DoorOpen),
    1744 Build180R0 => crate::construction::block_info("Ель · Дверь (открыта)", 2, crate::construction::Form::DoorOpen),
    1745 Build180R1 => crate::construction::block_info("Ель · Дверь (открыта)", 2, crate::construction::Form::DoorOpen),
    1746 Build180R2 => crate::construction::block_info("Ель · Дверь (открыта)", 2, crate::construction::Form::DoorOpen),
    1747 Build180R3 => crate::construction::block_info("Ель · Дверь (открыта)", 2, crate::construction::Form::DoorOpen),
    1748 Build181R0 => crate::construction::block_info("Железо · Дверь (открыта)", 6, crate::construction::Form::DoorOpen),
    1749 Build181R1 => crate::construction::block_info("Железо · Дверь (открыта)", 6, crate::construction::Form::DoorOpen),
    1750 Build181R2 => crate::construction::block_info("Железо · Дверь (открыта)", 6, crate::construction::Form::DoorOpen),
    1751 Build181R3 => crate::construction::block_info("Железо · Дверь (открыта)", 6, crate::construction::Form::DoorOpen),
    1752 Build182R0 => crate::construction::block_info("Медь · Дверь (открыта)", 7, crate::construction::Form::DoorOpen),
    1753 Build182R1 => crate::construction::block_info("Медь · Дверь (открыта)", 7, crate::construction::Form::DoorOpen),
    1754 Build182R2 => crate::construction::block_info("Медь · Дверь (открыта)", 7, crate::construction::Form::DoorOpen),
    1755 Build182R3 => crate::construction::block_info("Медь · Дверь (открыта)", 7, crate::construction::Form::DoorOpen),
    1756 Build183R0 => crate::construction::block_info("Стекло · Дверь (открыта)", 8, crate::construction::Form::DoorOpen),
    1757 Build183R1 => crate::construction::block_info("Стекло · Дверь (открыта)", 8, crate::construction::Form::DoorOpen),
    1758 Build183R2 => crate::construction::block_info("Стекло · Дверь (открыта)", 8, crate::construction::Form::DoorOpen),
    1759 Build183R3 => crate::construction::block_info("Стекло · Дверь (открыта)", 8, crate::construction::Form::DoorOpen),
    1760 Build184R0 => crate::construction::block_info("Дуб · Дверь (верх)", 0, crate::construction::Form::DoorTop),
    1761 Build184R1 => crate::construction::block_info("Дуб · Дверь (верх)", 0, crate::construction::Form::DoorTop),
    1762 Build184R2 => crate::construction::block_info("Дуб · Дверь (верх)", 0, crate::construction::Form::DoorTop),
    1763 Build184R3 => crate::construction::block_info("Дуб · Дверь (верх)", 0, crate::construction::Form::DoorTop),
    1764 Build185R0 => crate::construction::block_info("Берёза · Дверь (верх)", 1, crate::construction::Form::DoorTop),
    1765 Build185R1 => crate::construction::block_info("Берёза · Дверь (верх)", 1, crate::construction::Form::DoorTop),
    1766 Build185R2 => crate::construction::block_info("Берёза · Дверь (верх)", 1, crate::construction::Form::DoorTop),
    1767 Build185R3 => crate::construction::block_info("Берёза · Дверь (верх)", 1, crate::construction::Form::DoorTop),
    1768 Build186R0 => crate::construction::block_info("Ель · Дверь (верх)", 2, crate::construction::Form::DoorTop),
    1769 Build186R1 => crate::construction::block_info("Ель · Дверь (верх)", 2, crate::construction::Form::DoorTop),
    1770 Build186R2 => crate::construction::block_info("Ель · Дверь (верх)", 2, crate::construction::Form::DoorTop),
    1771 Build186R3 => crate::construction::block_info("Ель · Дверь (верх)", 2, crate::construction::Form::DoorTop),
    1772 Build187R0 => crate::construction::block_info("Железо · Дверь (верх)", 6, crate::construction::Form::DoorTop),
    1773 Build187R1 => crate::construction::block_info("Железо · Дверь (верх)", 6, crate::construction::Form::DoorTop),
    1774 Build187R2 => crate::construction::block_info("Железо · Дверь (верх)", 6, crate::construction::Form::DoorTop),
    1775 Build187R3 => crate::construction::block_info("Железо · Дверь (верх)", 6, crate::construction::Form::DoorTop),
    1776 Build188R0 => crate::construction::block_info("Медь · Дверь (верх)", 7, crate::construction::Form::DoorTop),
    1777 Build188R1 => crate::construction::block_info("Медь · Дверь (верх)", 7, crate::construction::Form::DoorTop),
    1778 Build188R2 => crate::construction::block_info("Медь · Дверь (верх)", 7, crate::construction::Form::DoorTop),
    1779 Build188R3 => crate::construction::block_info("Медь · Дверь (верх)", 7, crate::construction::Form::DoorTop),
    1780 Build189R0 => crate::construction::block_info("Стекло · Дверь (верх)", 8, crate::construction::Form::DoorTop),
    1781 Build189R1 => crate::construction::block_info("Стекло · Дверь (верх)", 8, crate::construction::Form::DoorTop),
    1782 Build189R2 => crate::construction::block_info("Стекло · Дверь (верх)", 8, crate::construction::Form::DoorTop),
    1783 Build189R3 => crate::construction::block_info("Стекло · Дверь (верх)", 8, crate::construction::Form::DoorTop),
    1784 Build190R0 => crate::construction::block_info("Дуб · Дверь (верх открыт)", 0, crate::construction::Form::DoorTopOpen),
    1785 Build190R1 => crate::construction::block_info("Дуб · Дверь (верх открыт)", 0, crate::construction::Form::DoorTopOpen),
    1786 Build190R2 => crate::construction::block_info("Дуб · Дверь (верх открыт)", 0, crate::construction::Form::DoorTopOpen),
    1787 Build190R3 => crate::construction::block_info("Дуб · Дверь (верх открыт)", 0, crate::construction::Form::DoorTopOpen),
    1788 Build191R0 => crate::construction::block_info("Берёза · Дверь (верх открыт)", 1, crate::construction::Form::DoorTopOpen),
    1789 Build191R1 => crate::construction::block_info("Берёза · Дверь (верх открыт)", 1, crate::construction::Form::DoorTopOpen),
    1790 Build191R2 => crate::construction::block_info("Берёза · Дверь (верх открыт)", 1, crate::construction::Form::DoorTopOpen),
    1791 Build191R3 => crate::construction::block_info("Берёза · Дверь (верх открыт)", 1, crate::construction::Form::DoorTopOpen),
    1792 Build192R0 => crate::construction::block_info("Ель · Дверь (верх открыт)", 2, crate::construction::Form::DoorTopOpen),
    1793 Build192R1 => crate::construction::block_info("Ель · Дверь (верх открыт)", 2, crate::construction::Form::DoorTopOpen),
    1794 Build192R2 => crate::construction::block_info("Ель · Дверь (верх открыт)", 2, crate::construction::Form::DoorTopOpen),
    1795 Build192R3 => crate::construction::block_info("Ель · Дверь (верх открыт)", 2, crate::construction::Form::DoorTopOpen),
    1796 Build193R0 => crate::construction::block_info("Железо · Дверь (верх открыт)", 6, crate::construction::Form::DoorTopOpen),
    1797 Build193R1 => crate::construction::block_info("Железо · Дверь (верх открыт)", 6, crate::construction::Form::DoorTopOpen),
    1798 Build193R2 => crate::construction::block_info("Железо · Дверь (верх открыт)", 6, crate::construction::Form::DoorTopOpen),
    1799 Build193R3 => crate::construction::block_info("Железо · Дверь (верх открыт)", 6, crate::construction::Form::DoorTopOpen),
    1800 Build194R0 => crate::construction::block_info("Медь · Дверь (верх открыт)", 7, crate::construction::Form::DoorTopOpen),
    1801 Build194R1 => crate::construction::block_info("Медь · Дверь (верх открыт)", 7, crate::construction::Form::DoorTopOpen),
    1802 Build194R2 => crate::construction::block_info("Медь · Дверь (верх открыт)", 7, crate::construction::Form::DoorTopOpen),
    1803 Build194R3 => crate::construction::block_info("Медь · Дверь (верх открыт)", 7, crate::construction::Form::DoorTopOpen),
    1804 Build195R0 => crate::construction::block_info("Стекло · Дверь (верх открыт)", 8, crate::construction::Form::DoorTopOpen),
    1805 Build195R1 => crate::construction::block_info("Стекло · Дверь (верх открыт)", 8, crate::construction::Form::DoorTopOpen),
    1806 Build195R2 => crate::construction::block_info("Стекло · Дверь (верх открыт)", 8, crate::construction::Form::DoorTopOpen),
    1807 Build195R3 => crate::construction::block_info("Стекло · Дверь (верх открыт)", 8, crate::construction::Form::DoorTopOpen),
    1808 Build196R0 => crate::construction::block_info("Дуб · Раздвижная дверь", 0, crate::construction::Form::SlidingDoor),
    1809 Build196R1 => crate::construction::block_info("Дуб · Раздвижная дверь", 0, crate::construction::Form::SlidingDoor),
    1810 Build196R2 => crate::construction::block_info("Дуб · Раздвижная дверь", 0, crate::construction::Form::SlidingDoor),
    1811 Build196R3 => crate::construction::block_info("Дуб · Раздвижная дверь", 0, crate::construction::Form::SlidingDoor),
    1812 Build197R0 => crate::construction::block_info("Берёза · Раздвижная дверь", 1, crate::construction::Form::SlidingDoor),
    1813 Build197R1 => crate::construction::block_info("Берёза · Раздвижная дверь", 1, crate::construction::Form::SlidingDoor),
    1814 Build197R2 => crate::construction::block_info("Берёза · Раздвижная дверь", 1, crate::construction::Form::SlidingDoor),
    1815 Build197R3 => crate::construction::block_info("Берёза · Раздвижная дверь", 1, crate::construction::Form::SlidingDoor),
    1816 Build198R0 => crate::construction::block_info("Ель · Раздвижная дверь", 2, crate::construction::Form::SlidingDoor),
    1817 Build198R1 => crate::construction::block_info("Ель · Раздвижная дверь", 2, crate::construction::Form::SlidingDoor),
    1818 Build198R2 => crate::construction::block_info("Ель · Раздвижная дверь", 2, crate::construction::Form::SlidingDoor),
    1819 Build198R3 => crate::construction::block_info("Ель · Раздвижная дверь", 2, crate::construction::Form::SlidingDoor),
    1820 Build199R0 => crate::construction::block_info("Железо · Раздвижная дверь", 6, crate::construction::Form::SlidingDoor),
    1821 Build199R1 => crate::construction::block_info("Железо · Раздвижная дверь", 6, crate::construction::Form::SlidingDoor),
    1822 Build199R2 => crate::construction::block_info("Железо · Раздвижная дверь", 6, crate::construction::Form::SlidingDoor),
    1823 Build199R3 => crate::construction::block_info("Железо · Раздвижная дверь", 6, crate::construction::Form::SlidingDoor),
    1824 Build200R0 => crate::construction::block_info("Медь · Раздвижная дверь", 7, crate::construction::Form::SlidingDoor),
    1825 Build200R1 => crate::construction::block_info("Медь · Раздвижная дверь", 7, crate::construction::Form::SlidingDoor),
    1826 Build200R2 => crate::construction::block_info("Медь · Раздвижная дверь", 7, crate::construction::Form::SlidingDoor),
    1827 Build200R3 => crate::construction::block_info("Медь · Раздвижная дверь", 7, crate::construction::Form::SlidingDoor),
    1828 Build201R0 => crate::construction::block_info("Стекло · Раздвижная дверь", 8, crate::construction::Form::SlidingDoor),
    1829 Build201R1 => crate::construction::block_info("Стекло · Раздвижная дверь", 8, crate::construction::Form::SlidingDoor),
    1830 Build201R2 => crate::construction::block_info("Стекло · Раздвижная дверь", 8, crate::construction::Form::SlidingDoor),
    1831 Build201R3 => crate::construction::block_info("Стекло · Раздвижная дверь", 8, crate::construction::Form::SlidingDoor),
    1832 Build202R0 => crate::construction::block_info("Дуб · Раздвижная дверь (открыта)", 0, crate::construction::Form::SlidingDoorOpen),
    1833 Build202R1 => crate::construction::block_info("Дуб · Раздвижная дверь (открыта)", 0, crate::construction::Form::SlidingDoorOpen),
    1834 Build202R2 => crate::construction::block_info("Дуб · Раздвижная дверь (открыта)", 0, crate::construction::Form::SlidingDoorOpen),
    1835 Build202R3 => crate::construction::block_info("Дуб · Раздвижная дверь (открыта)", 0, crate::construction::Form::SlidingDoorOpen),
    1836 Build203R0 => crate::construction::block_info("Берёза · Раздвижная дверь (открыта)", 1, crate::construction::Form::SlidingDoorOpen),
    1837 Build203R1 => crate::construction::block_info("Берёза · Раздвижная дверь (открыта)", 1, crate::construction::Form::SlidingDoorOpen),
    1838 Build203R2 => crate::construction::block_info("Берёза · Раздвижная дверь (открыта)", 1, crate::construction::Form::SlidingDoorOpen),
    1839 Build203R3 => crate::construction::block_info("Берёза · Раздвижная дверь (открыта)", 1, crate::construction::Form::SlidingDoorOpen),
    1840 Build204R0 => crate::construction::block_info("Ель · Раздвижная дверь (открыта)", 2, crate::construction::Form::SlidingDoorOpen),
    1841 Build204R1 => crate::construction::block_info("Ель · Раздвижная дверь (открыта)", 2, crate::construction::Form::SlidingDoorOpen),
    1842 Build204R2 => crate::construction::block_info("Ель · Раздвижная дверь (открыта)", 2, crate::construction::Form::SlidingDoorOpen),
    1843 Build204R3 => crate::construction::block_info("Ель · Раздвижная дверь (открыта)", 2, crate::construction::Form::SlidingDoorOpen),
    1844 Build205R0 => crate::construction::block_info("Железо · Раздвижная дверь (открыта)", 6, crate::construction::Form::SlidingDoorOpen),
    1845 Build205R1 => crate::construction::block_info("Железо · Раздвижная дверь (открыта)", 6, crate::construction::Form::SlidingDoorOpen),
    1846 Build205R2 => crate::construction::block_info("Железо · Раздвижная дверь (открыта)", 6, crate::construction::Form::SlidingDoorOpen),
    1847 Build205R3 => crate::construction::block_info("Железо · Раздвижная дверь (открыта)", 6, crate::construction::Form::SlidingDoorOpen),
    1848 Build206R0 => crate::construction::block_info("Медь · Раздвижная дверь (открыта)", 7, crate::construction::Form::SlidingDoorOpen),
    1849 Build206R1 => crate::construction::block_info("Медь · Раздвижная дверь (открыта)", 7, crate::construction::Form::SlidingDoorOpen),
    1850 Build206R2 => crate::construction::block_info("Медь · Раздвижная дверь (открыта)", 7, crate::construction::Form::SlidingDoorOpen),
    1851 Build206R3 => crate::construction::block_info("Медь · Раздвижная дверь (открыта)", 7, crate::construction::Form::SlidingDoorOpen),
    1852 Build207R0 => crate::construction::block_info("Стекло · Раздвижная дверь (открыта)", 8, crate::construction::Form::SlidingDoorOpen),
    1853 Build207R1 => crate::construction::block_info("Стекло · Раздвижная дверь (открыта)", 8, crate::construction::Form::SlidingDoorOpen),
    1854 Build207R2 => crate::construction::block_info("Стекло · Раздвижная дверь (открыта)", 8, crate::construction::Form::SlidingDoorOpen),
    1855 Build207R3 => crate::construction::block_info("Стекло · Раздвижная дверь (открыта)", 8, crate::construction::Form::SlidingDoorOpen),
    1856 Build208R0 => crate::construction::block_info("Дуб · Раздвижная дверь (верх)", 0, crate::construction::Form::SlidingDoorTop),
    1857 Build208R1 => crate::construction::block_info("Дуб · Раздвижная дверь (верх)", 0, crate::construction::Form::SlidingDoorTop),
    1858 Build208R2 => crate::construction::block_info("Дуб · Раздвижная дверь (верх)", 0, crate::construction::Form::SlidingDoorTop),
    1859 Build208R3 => crate::construction::block_info("Дуб · Раздвижная дверь (верх)", 0, crate::construction::Form::SlidingDoorTop),
    1860 Build209R0 => crate::construction::block_info("Берёза · Раздвижная дверь (верх)", 1, crate::construction::Form::SlidingDoorTop),
    1861 Build209R1 => crate::construction::block_info("Берёза · Раздвижная дверь (верх)", 1, crate::construction::Form::SlidingDoorTop),
    1862 Build209R2 => crate::construction::block_info("Берёза · Раздвижная дверь (верх)", 1, crate::construction::Form::SlidingDoorTop),
    1863 Build209R3 => crate::construction::block_info("Берёза · Раздвижная дверь (верх)", 1, crate::construction::Form::SlidingDoorTop),
    1864 Build210R0 => crate::construction::block_info("Ель · Раздвижная дверь (верх)", 2, crate::construction::Form::SlidingDoorTop),
    1865 Build210R1 => crate::construction::block_info("Ель · Раздвижная дверь (верх)", 2, crate::construction::Form::SlidingDoorTop),
    1866 Build210R2 => crate::construction::block_info("Ель · Раздвижная дверь (верх)", 2, crate::construction::Form::SlidingDoorTop),
    1867 Build210R3 => crate::construction::block_info("Ель · Раздвижная дверь (верх)", 2, crate::construction::Form::SlidingDoorTop),
    1868 Build211R0 => crate::construction::block_info("Железо · Раздвижная дверь (верх)", 6, crate::construction::Form::SlidingDoorTop),
    1869 Build211R1 => crate::construction::block_info("Железо · Раздвижная дверь (верх)", 6, crate::construction::Form::SlidingDoorTop),
    1870 Build211R2 => crate::construction::block_info("Железо · Раздвижная дверь (верх)", 6, crate::construction::Form::SlidingDoorTop),
    1871 Build211R3 => crate::construction::block_info("Железо · Раздвижная дверь (верх)", 6, crate::construction::Form::SlidingDoorTop),
    1872 Build212R0 => crate::construction::block_info("Медь · Раздвижная дверь (верх)", 7, crate::construction::Form::SlidingDoorTop),
    1873 Build212R1 => crate::construction::block_info("Медь · Раздвижная дверь (верх)", 7, crate::construction::Form::SlidingDoorTop),
    1874 Build212R2 => crate::construction::block_info("Медь · Раздвижная дверь (верх)", 7, crate::construction::Form::SlidingDoorTop),
    1875 Build212R3 => crate::construction::block_info("Медь · Раздвижная дверь (верх)", 7, crate::construction::Form::SlidingDoorTop),
    1876 Build213R0 => crate::construction::block_info("Стекло · Раздвижная дверь (верх)", 8, crate::construction::Form::SlidingDoorTop),
    1877 Build213R1 => crate::construction::block_info("Стекло · Раздвижная дверь (верх)", 8, crate::construction::Form::SlidingDoorTop),
    1878 Build213R2 => crate::construction::block_info("Стекло · Раздвижная дверь (верх)", 8, crate::construction::Form::SlidingDoorTop),
    1879 Build213R3 => crate::construction::block_info("Стекло · Раздвижная дверь (верх)", 8, crate::construction::Form::SlidingDoorTop),
    1880 Build214R0 => crate::construction::block_info("Дуб · Раздвижная дверь (верх открыт)", 0, crate::construction::Form::SlidingDoorTopOpen),
    1881 Build214R1 => crate::construction::block_info("Дуб · Раздвижная дверь (верх открыт)", 0, crate::construction::Form::SlidingDoorTopOpen),
    1882 Build214R2 => crate::construction::block_info("Дуб · Раздвижная дверь (верх открыт)", 0, crate::construction::Form::SlidingDoorTopOpen),
    1883 Build214R3 => crate::construction::block_info("Дуб · Раздвижная дверь (верх открыт)", 0, crate::construction::Form::SlidingDoorTopOpen),
    1884 Build215R0 => crate::construction::block_info("Берёза · Раздвижная дверь (верх открыт)", 1, crate::construction::Form::SlidingDoorTopOpen),
    1885 Build215R1 => crate::construction::block_info("Берёза · Раздвижная дверь (верх открыт)", 1, crate::construction::Form::SlidingDoorTopOpen),
    1886 Build215R2 => crate::construction::block_info("Берёза · Раздвижная дверь (верх открыт)", 1, crate::construction::Form::SlidingDoorTopOpen),
    1887 Build215R3 => crate::construction::block_info("Берёза · Раздвижная дверь (верх открыт)", 1, crate::construction::Form::SlidingDoorTopOpen),
    1888 Build216R0 => crate::construction::block_info("Ель · Раздвижная дверь (верх открыт)", 2, crate::construction::Form::SlidingDoorTopOpen),
    1889 Build216R1 => crate::construction::block_info("Ель · Раздвижная дверь (верх открыт)", 2, crate::construction::Form::SlidingDoorTopOpen),
    1890 Build216R2 => crate::construction::block_info("Ель · Раздвижная дверь (верх открыт)", 2, crate::construction::Form::SlidingDoorTopOpen),
    1891 Build216R3 => crate::construction::block_info("Ель · Раздвижная дверь (верх открыт)", 2, crate::construction::Form::SlidingDoorTopOpen),
    1892 Build217R0 => crate::construction::block_info("Железо · Раздвижная дверь (верх открыт)", 6, crate::construction::Form::SlidingDoorTopOpen),
    1893 Build217R1 => crate::construction::block_info("Железо · Раздвижная дверь (верх открыт)", 6, crate::construction::Form::SlidingDoorTopOpen),
    1894 Build217R2 => crate::construction::block_info("Железо · Раздвижная дверь (верх открыт)", 6, crate::construction::Form::SlidingDoorTopOpen),
    1895 Build217R3 => crate::construction::block_info("Железо · Раздвижная дверь (верх открыт)", 6, crate::construction::Form::SlidingDoorTopOpen),
    1896 Build218R0 => crate::construction::block_info("Медь · Раздвижная дверь (верх открыт)", 7, crate::construction::Form::SlidingDoorTopOpen),
    1897 Build218R1 => crate::construction::block_info("Медь · Раздвижная дверь (верх открыт)", 7, crate::construction::Form::SlidingDoorTopOpen),
    1898 Build218R2 => crate::construction::block_info("Медь · Раздвижная дверь (верх открыт)", 7, crate::construction::Form::SlidingDoorTopOpen),
    1899 Build218R3 => crate::construction::block_info("Медь · Раздвижная дверь (верх открыт)", 7, crate::construction::Form::SlidingDoorTopOpen),
    1900 Build219R0 => crate::construction::block_info("Стекло · Раздвижная дверь (верх открыт)", 8, crate::construction::Form::SlidingDoorTopOpen),
    1901 Build219R1 => crate::construction::block_info("Стекло · Раздвижная дверь (верх открыт)", 8, crate::construction::Form::SlidingDoorTopOpen),
    1902 Build219R2 => crate::construction::block_info("Стекло · Раздвижная дверь (верх открыт)", 8, crate::construction::Form::SlidingDoorTopOpen),
    1903 Build219R3 => crate::construction::block_info("Стекло · Раздвижная дверь (верх открыт)", 8, crate::construction::Form::SlidingDoorTopOpen),
    1904 Build220R0 => crate::construction::block_info("Дуб · Складная дверь", 0, crate::construction::Form::FoldingDoor),
    1905 Build220R1 => crate::construction::block_info("Дуб · Складная дверь", 0, crate::construction::Form::FoldingDoor),
    1906 Build220R2 => crate::construction::block_info("Дуб · Складная дверь", 0, crate::construction::Form::FoldingDoor),
    1907 Build220R3 => crate::construction::block_info("Дуб · Складная дверь", 0, crate::construction::Form::FoldingDoor),
    1908 Build221R0 => crate::construction::block_info("Берёза · Складная дверь", 1, crate::construction::Form::FoldingDoor),
    1909 Build221R1 => crate::construction::block_info("Берёза · Складная дверь", 1, crate::construction::Form::FoldingDoor),
    1910 Build221R2 => crate::construction::block_info("Берёза · Складная дверь", 1, crate::construction::Form::FoldingDoor),
    1911 Build221R3 => crate::construction::block_info("Берёза · Складная дверь", 1, crate::construction::Form::FoldingDoor),
    1912 Build222R0 => crate::construction::block_info("Ель · Складная дверь", 2, crate::construction::Form::FoldingDoor),
    1913 Build222R1 => crate::construction::block_info("Ель · Складная дверь", 2, crate::construction::Form::FoldingDoor),
    1914 Build222R2 => crate::construction::block_info("Ель · Складная дверь", 2, crate::construction::Form::FoldingDoor),
    1915 Build222R3 => crate::construction::block_info("Ель · Складная дверь", 2, crate::construction::Form::FoldingDoor),
    1916 Build223R0 => crate::construction::block_info("Железо · Складная дверь", 6, crate::construction::Form::FoldingDoor),
    1917 Build223R1 => crate::construction::block_info("Железо · Складная дверь", 6, crate::construction::Form::FoldingDoor),
    1918 Build223R2 => crate::construction::block_info("Железо · Складная дверь", 6, crate::construction::Form::FoldingDoor),
    1919 Build223R3 => crate::construction::block_info("Железо · Складная дверь", 6, crate::construction::Form::FoldingDoor),
    1920 Build224R0 => crate::construction::block_info("Медь · Складная дверь", 7, crate::construction::Form::FoldingDoor),
    1921 Build224R1 => crate::construction::block_info("Медь · Складная дверь", 7, crate::construction::Form::FoldingDoor),
    1922 Build224R2 => crate::construction::block_info("Медь · Складная дверь", 7, crate::construction::Form::FoldingDoor),
    1923 Build224R3 => crate::construction::block_info("Медь · Складная дверь", 7, crate::construction::Form::FoldingDoor),
    1924 Build225R0 => crate::construction::block_info("Стекло · Складная дверь", 8, crate::construction::Form::FoldingDoor),
    1925 Build225R1 => crate::construction::block_info("Стекло · Складная дверь", 8, crate::construction::Form::FoldingDoor),
    1926 Build225R2 => crate::construction::block_info("Стекло · Складная дверь", 8, crate::construction::Form::FoldingDoor),
    1927 Build225R3 => crate::construction::block_info("Стекло · Складная дверь", 8, crate::construction::Form::FoldingDoor),
    1928 Build226R0 => crate::construction::block_info("Дуб · Складная дверь (открыта)", 0, crate::construction::Form::FoldingDoorOpen),
    1929 Build226R1 => crate::construction::block_info("Дуб · Складная дверь (открыта)", 0, crate::construction::Form::FoldingDoorOpen),
    1930 Build226R2 => crate::construction::block_info("Дуб · Складная дверь (открыта)", 0, crate::construction::Form::FoldingDoorOpen),
    1931 Build226R3 => crate::construction::block_info("Дуб · Складная дверь (открыта)", 0, crate::construction::Form::FoldingDoorOpen),
    1932 Build227R0 => crate::construction::block_info("Берёза · Складная дверь (открыта)", 1, crate::construction::Form::FoldingDoorOpen),
    1933 Build227R1 => crate::construction::block_info("Берёза · Складная дверь (открыта)", 1, crate::construction::Form::FoldingDoorOpen),
    1934 Build227R2 => crate::construction::block_info("Берёза · Складная дверь (открыта)", 1, crate::construction::Form::FoldingDoorOpen),
    1935 Build227R3 => crate::construction::block_info("Берёза · Складная дверь (открыта)", 1, crate::construction::Form::FoldingDoorOpen),
    1936 Build228R0 => crate::construction::block_info("Ель · Складная дверь (открыта)", 2, crate::construction::Form::FoldingDoorOpen),
    1937 Build228R1 => crate::construction::block_info("Ель · Складная дверь (открыта)", 2, crate::construction::Form::FoldingDoorOpen),
    1938 Build228R2 => crate::construction::block_info("Ель · Складная дверь (открыта)", 2, crate::construction::Form::FoldingDoorOpen),
    1939 Build228R3 => crate::construction::block_info("Ель · Складная дверь (открыта)", 2, crate::construction::Form::FoldingDoorOpen),
    1940 Build229R0 => crate::construction::block_info("Железо · Складная дверь (открыта)", 6, crate::construction::Form::FoldingDoorOpen),
    1941 Build229R1 => crate::construction::block_info("Железо · Складная дверь (открыта)", 6, crate::construction::Form::FoldingDoorOpen),
    1942 Build229R2 => crate::construction::block_info("Железо · Складная дверь (открыта)", 6, crate::construction::Form::FoldingDoorOpen),
    1943 Build229R3 => crate::construction::block_info("Железо · Складная дверь (открыта)", 6, crate::construction::Form::FoldingDoorOpen),
    1944 Build230R0 => crate::construction::block_info("Медь · Складная дверь (открыта)", 7, crate::construction::Form::FoldingDoorOpen),
    1945 Build230R1 => crate::construction::block_info("Медь · Складная дверь (открыта)", 7, crate::construction::Form::FoldingDoorOpen),
    1946 Build230R2 => crate::construction::block_info("Медь · Складная дверь (открыта)", 7, crate::construction::Form::FoldingDoorOpen),
    1947 Build230R3 => crate::construction::block_info("Медь · Складная дверь (открыта)", 7, crate::construction::Form::FoldingDoorOpen),
    1948 Build231R0 => crate::construction::block_info("Стекло · Складная дверь (открыта)", 8, crate::construction::Form::FoldingDoorOpen),
    1949 Build231R1 => crate::construction::block_info("Стекло · Складная дверь (открыта)", 8, crate::construction::Form::FoldingDoorOpen),
    1950 Build231R2 => crate::construction::block_info("Стекло · Складная дверь (открыта)", 8, crate::construction::Form::FoldingDoorOpen),
    1951 Build231R3 => crate::construction::block_info("Стекло · Складная дверь (открыта)", 8, crate::construction::Form::FoldingDoorOpen),
    1952 Build232R0 => crate::construction::block_info("Дуб · Складная дверь (верх)", 0, crate::construction::Form::FoldingDoorTop),
    1953 Build232R1 => crate::construction::block_info("Дуб · Складная дверь (верх)", 0, crate::construction::Form::FoldingDoorTop),
    1954 Build232R2 => crate::construction::block_info("Дуб · Складная дверь (верх)", 0, crate::construction::Form::FoldingDoorTop),
    1955 Build232R3 => crate::construction::block_info("Дуб · Складная дверь (верх)", 0, crate::construction::Form::FoldingDoorTop),
    1956 Build233R0 => crate::construction::block_info("Берёза · Складная дверь (верх)", 1, crate::construction::Form::FoldingDoorTop),
    1957 Build233R1 => crate::construction::block_info("Берёза · Складная дверь (верх)", 1, crate::construction::Form::FoldingDoorTop),
    1958 Build233R2 => crate::construction::block_info("Берёза · Складная дверь (верх)", 1, crate::construction::Form::FoldingDoorTop),
    1959 Build233R3 => crate::construction::block_info("Берёза · Складная дверь (верх)", 1, crate::construction::Form::FoldingDoorTop),
    1960 Build234R0 => crate::construction::block_info("Ель · Складная дверь (верх)", 2, crate::construction::Form::FoldingDoorTop),
    1961 Build234R1 => crate::construction::block_info("Ель · Складная дверь (верх)", 2, crate::construction::Form::FoldingDoorTop),
    1962 Build234R2 => crate::construction::block_info("Ель · Складная дверь (верх)", 2, crate::construction::Form::FoldingDoorTop),
    1963 Build234R3 => crate::construction::block_info("Ель · Складная дверь (верх)", 2, crate::construction::Form::FoldingDoorTop),
    1964 Build235R0 => crate::construction::block_info("Железо · Складная дверь (верх)", 6, crate::construction::Form::FoldingDoorTop),
    1965 Build235R1 => crate::construction::block_info("Железо · Складная дверь (верх)", 6, crate::construction::Form::FoldingDoorTop),
    1966 Build235R2 => crate::construction::block_info("Железо · Складная дверь (верх)", 6, crate::construction::Form::FoldingDoorTop),
    1967 Build235R3 => crate::construction::block_info("Железо · Складная дверь (верх)", 6, crate::construction::Form::FoldingDoorTop),
    1968 Build236R0 => crate::construction::block_info("Медь · Складная дверь (верх)", 7, crate::construction::Form::FoldingDoorTop),
    1969 Build236R1 => crate::construction::block_info("Медь · Складная дверь (верх)", 7, crate::construction::Form::FoldingDoorTop),
    1970 Build236R2 => crate::construction::block_info("Медь · Складная дверь (верх)", 7, crate::construction::Form::FoldingDoorTop),
    1971 Build236R3 => crate::construction::block_info("Медь · Складная дверь (верх)", 7, crate::construction::Form::FoldingDoorTop),
    1972 Build237R0 => crate::construction::block_info("Стекло · Складная дверь (верх)", 8, crate::construction::Form::FoldingDoorTop),
    1973 Build237R1 => crate::construction::block_info("Стекло · Складная дверь (верх)", 8, crate::construction::Form::FoldingDoorTop),
    1974 Build237R2 => crate::construction::block_info("Стекло · Складная дверь (верх)", 8, crate::construction::Form::FoldingDoorTop),
    1975 Build237R3 => crate::construction::block_info("Стекло · Складная дверь (верх)", 8, crate::construction::Form::FoldingDoorTop),
    1976 Build238R0 => crate::construction::block_info("Дуб · Складная дверь (верх открыт)", 0, crate::construction::Form::FoldingDoorTopOpen),
    1977 Build238R1 => crate::construction::block_info("Дуб · Складная дверь (верх открыт)", 0, crate::construction::Form::FoldingDoorTopOpen),
    1978 Build238R2 => crate::construction::block_info("Дуб · Складная дверь (верх открыт)", 0, crate::construction::Form::FoldingDoorTopOpen),
    1979 Build238R3 => crate::construction::block_info("Дуб · Складная дверь (верх открыт)", 0, crate::construction::Form::FoldingDoorTopOpen),
    1980 Build239R0 => crate::construction::block_info("Берёза · Складная дверь (верх открыт)", 1, crate::construction::Form::FoldingDoorTopOpen),
    1981 Build239R1 => crate::construction::block_info("Берёза · Складная дверь (верх открыт)", 1, crate::construction::Form::FoldingDoorTopOpen),
    1982 Build239R2 => crate::construction::block_info("Берёза · Складная дверь (верх открыт)", 1, crate::construction::Form::FoldingDoorTopOpen),
    1983 Build239R3 => crate::construction::block_info("Берёза · Складная дверь (верх открыт)", 1, crate::construction::Form::FoldingDoorTopOpen),
    1984 Build240R0 => crate::construction::block_info("Ель · Складная дверь (верх открыт)", 2, crate::construction::Form::FoldingDoorTopOpen),
    1985 Build240R1 => crate::construction::block_info("Ель · Складная дверь (верх открыт)", 2, crate::construction::Form::FoldingDoorTopOpen),
    1986 Build240R2 => crate::construction::block_info("Ель · Складная дверь (верх открыт)", 2, crate::construction::Form::FoldingDoorTopOpen),
    1987 Build240R3 => crate::construction::block_info("Ель · Складная дверь (верх открыт)", 2, crate::construction::Form::FoldingDoorTopOpen),
    1988 Build241R0 => crate::construction::block_info("Железо · Складная дверь (верх открыт)", 6, crate::construction::Form::FoldingDoorTopOpen),
    1989 Build241R1 => crate::construction::block_info("Железо · Складная дверь (верх открыт)", 6, crate::construction::Form::FoldingDoorTopOpen),
    1990 Build241R2 => crate::construction::block_info("Железо · Складная дверь (верх открыт)", 6, crate::construction::Form::FoldingDoorTopOpen),
    1991 Build241R3 => crate::construction::block_info("Железо · Складная дверь (верх открыт)", 6, crate::construction::Form::FoldingDoorTopOpen),
    1992 Build242R0 => crate::construction::block_info("Медь · Складная дверь (верх открыт)", 7, crate::construction::Form::FoldingDoorTopOpen),
    1993 Build242R1 => crate::construction::block_info("Медь · Складная дверь (верх открыт)", 7, crate::construction::Form::FoldingDoorTopOpen),
    1994 Build242R2 => crate::construction::block_info("Медь · Складная дверь (верх открыт)", 7, crate::construction::Form::FoldingDoorTopOpen),
    1995 Build242R3 => crate::construction::block_info("Медь · Складная дверь (верх открыт)", 7, crate::construction::Form::FoldingDoorTopOpen),
    1996 Build243R0 => crate::construction::block_info("Стекло · Складная дверь (верх открыт)", 8, crate::construction::Form::FoldingDoorTopOpen),
    1997 Build243R1 => crate::construction::block_info("Стекло · Складная дверь (верх открыт)", 8, crate::construction::Form::FoldingDoorTopOpen),
    1998 Build243R2 => crate::construction::block_info("Стекло · Складная дверь (верх открыт)", 8, crate::construction::Form::FoldingDoorTopOpen),
    1999 Build243R3 => crate::construction::block_info("Стекло · Складная дверь (верх открыт)", 8, crate::construction::Form::FoldingDoorTopOpen),
    2000 Build244R0 => crate::construction::block_info("Дуб · Стул", 0, crate::construction::Form::Chair),
    2001 Build244R1 => crate::construction::block_info("Дуб · Стул", 0, crate::construction::Form::Chair),
    2002 Build244R2 => crate::construction::block_info("Дуб · Стул", 0, crate::construction::Form::Chair),
    2003 Build244R3 => crate::construction::block_info("Дуб · Стул", 0, crate::construction::Form::Chair),
    2004 Build245R0 => crate::construction::block_info("Берёза · Стул", 1, crate::construction::Form::Chair),
    2005 Build245R1 => crate::construction::block_info("Берёза · Стул", 1, crate::construction::Form::Chair),
    2006 Build245R2 => crate::construction::block_info("Берёза · Стул", 1, crate::construction::Form::Chair),
    2007 Build245R3 => crate::construction::block_info("Берёза · Стул", 1, crate::construction::Form::Chair),
    2008 Build246R0 => crate::construction::block_info("Ель · Стул", 2, crate::construction::Form::Chair),
    2009 Build246R1 => crate::construction::block_info("Ель · Стул", 2, crate::construction::Form::Chair),
    2010 Build246R2 => crate::construction::block_info("Ель · Стул", 2, crate::construction::Form::Chair),
    2011 Build246R3 => crate::construction::block_info("Ель · Стул", 2, crate::construction::Form::Chair),
    2012 Build247R0 => crate::construction::block_info("Железо · Стул", 6, crate::construction::Form::Chair),
    2013 Build247R1 => crate::construction::block_info("Железо · Стул", 6, crate::construction::Form::Chair),
    2014 Build247R2 => crate::construction::block_info("Железо · Стул", 6, crate::construction::Form::Chair),
    2015 Build247R3 => crate::construction::block_info("Железо · Стул", 6, crate::construction::Form::Chair),
    2016 Build248R0 => crate::construction::block_info("Медь · Стул", 7, crate::construction::Form::Chair),
    2017 Build248R1 => crate::construction::block_info("Медь · Стул", 7, crate::construction::Form::Chair),
    2018 Build248R2 => crate::construction::block_info("Медь · Стул", 7, crate::construction::Form::Chair),
    2019 Build248R3 => crate::construction::block_info("Медь · Стул", 7, crate::construction::Form::Chair),
    2020 Build249R0 => crate::construction::block_info("Дуб · Стол", 0, crate::construction::Form::Table),
    2021 Build249R1 => crate::construction::block_info("Дуб · Стол", 0, crate::construction::Form::Table),
    2022 Build249R2 => crate::construction::block_info("Дуб · Стол", 0, crate::construction::Form::Table),
    2023 Build249R3 => crate::construction::block_info("Дуб · Стол", 0, crate::construction::Form::Table),
    2024 Build250R0 => crate::construction::block_info("Берёза · Стол", 1, crate::construction::Form::Table),
    2025 Build250R1 => crate::construction::block_info("Берёза · Стол", 1, crate::construction::Form::Table),
    2026 Build250R2 => crate::construction::block_info("Берёза · Стол", 1, crate::construction::Form::Table),
    2027 Build250R3 => crate::construction::block_info("Берёза · Стол", 1, crate::construction::Form::Table),
    2028 Build251R0 => crate::construction::block_info("Ель · Стол", 2, crate::construction::Form::Table),
    2029 Build251R1 => crate::construction::block_info("Ель · Стол", 2, crate::construction::Form::Table),
    2030 Build251R2 => crate::construction::block_info("Ель · Стол", 2, crate::construction::Form::Table),
    2031 Build251R3 => crate::construction::block_info("Ель · Стол", 2, crate::construction::Form::Table),
    2032 Build252R0 => crate::construction::block_info("Железо · Стол", 6, crate::construction::Form::Table),
    2033 Build252R1 => crate::construction::block_info("Железо · Стол", 6, crate::construction::Form::Table),
    2034 Build252R2 => crate::construction::block_info("Железо · Стол", 6, crate::construction::Form::Table),
    2035 Build252R3 => crate::construction::block_info("Железо · Стол", 6, crate::construction::Form::Table),
    2036 Build253R0 => crate::construction::block_info("Медь · Стол", 7, crate::construction::Form::Table),
    2037 Build253R1 => crate::construction::block_info("Медь · Стол", 7, crate::construction::Form::Table),
    2038 Build253R2 => crate::construction::block_info("Медь · Стол", 7, crate::construction::Form::Table),
    2039 Build253R3 => crate::construction::block_info("Медь · Стол", 7, crate::construction::Form::Table),
    2040 Build254R0 => crate::construction::block_info("Дуб · Стол со стеклянной столешницей", 0, crate::construction::Form::GlassTable),
    2041 Build254R1 => crate::construction::block_info("Дуб · Стол со стеклянной столешницей", 0, crate::construction::Form::GlassTable),
    2042 Build254R2 => crate::construction::block_info("Дуб · Стол со стеклянной столешницей", 0, crate::construction::Form::GlassTable),
    2043 Build254R3 => crate::construction::block_info("Дуб · Стол со стеклянной столешницей", 0, crate::construction::Form::GlassTable),
    2044 Build255R0 => crate::construction::block_info("Берёза · Стол со стеклянной столешницей", 1, crate::construction::Form::GlassTable),
    2045 Build255R1 => crate::construction::block_info("Берёза · Стол со стеклянной столешницей", 1, crate::construction::Form::GlassTable),
    2046 Build255R2 => crate::construction::block_info("Берёза · Стол со стеклянной столешницей", 1, crate::construction::Form::GlassTable),
    2047 Build255R3 => crate::construction::block_info("Берёза · Стол со стеклянной столешницей", 1, crate::construction::Form::GlassTable),
    2048 Build256R0 => crate::construction::block_info("Ель · Стол со стеклянной столешницей", 2, crate::construction::Form::GlassTable),
    2049 Build256R1 => crate::construction::block_info("Ель · Стол со стеклянной столешницей", 2, crate::construction::Form::GlassTable),
    2050 Build256R2 => crate::construction::block_info("Ель · Стол со стеклянной столешницей", 2, crate::construction::Form::GlassTable),
    2051 Build256R3 => crate::construction::block_info("Ель · Стол со стеклянной столешницей", 2, crate::construction::Form::GlassTable),
    2052 Build257R0 => crate::construction::block_info("Железо · Стол со стеклянной столешницей", 6, crate::construction::Form::GlassTable),
    2053 Build257R1 => crate::construction::block_info("Железо · Стол со стеклянной столешницей", 6, crate::construction::Form::GlassTable),
    2054 Build257R2 => crate::construction::block_info("Железо · Стол со стеклянной столешницей", 6, crate::construction::Form::GlassTable),
    2055 Build257R3 => crate::construction::block_info("Железо · Стол со стеклянной столешницей", 6, crate::construction::Form::GlassTable),
    2056 Build258R0 => crate::construction::block_info("Медь · Стол со стеклянной столешницей", 7, crate::construction::Form::GlassTable),
    2057 Build258R1 => crate::construction::block_info("Медь · Стол со стеклянной столешницей", 7, crate::construction::Form::GlassTable),
    2058 Build258R2 => crate::construction::block_info("Медь · Стол со стеклянной столешницей", 7, crate::construction::Form::GlassTable),
    2059 Build258R3 => crate::construction::block_info("Медь · Стол со стеклянной столешницей", 7, crate::construction::Form::GlassTable),
    2060 Build259R0 => crate::construction::block_info("Дуб · Диван", 0, crate::construction::Form::Sofa),
    2061 Build259R1 => crate::construction::block_info("Дуб · Диван", 0, crate::construction::Form::Sofa),
    2062 Build259R2 => crate::construction::block_info("Дуб · Диван", 0, crate::construction::Form::Sofa),
    2063 Build259R3 => crate::construction::block_info("Дуб · Диван", 0, crate::construction::Form::Sofa),
    2064 Build260R0 => crate::construction::block_info("Берёза · Диван", 1, crate::construction::Form::Sofa),
    2065 Build260R1 => crate::construction::block_info("Берёза · Диван", 1, crate::construction::Form::Sofa),
    2066 Build260R2 => crate::construction::block_info("Берёза · Диван", 1, crate::construction::Form::Sofa),
    2067 Build260R3 => crate::construction::block_info("Берёза · Диван", 1, crate::construction::Form::Sofa),
    2068 Build261R0 => crate::construction::block_info("Ель · Диван", 2, crate::construction::Form::Sofa),
    2069 Build261R1 => crate::construction::block_info("Ель · Диван", 2, crate::construction::Form::Sofa),
    2070 Build261R2 => crate::construction::block_info("Ель · Диван", 2, crate::construction::Form::Sofa),
    2071 Build261R3 => crate::construction::block_info("Ель · Диван", 2, crate::construction::Form::Sofa),
    2072 Build262R0 => crate::construction::block_info("Железо · Диван", 6, crate::construction::Form::Sofa),
    2073 Build262R1 => crate::construction::block_info("Железо · Диван", 6, crate::construction::Form::Sofa),
    2074 Build262R2 => crate::construction::block_info("Железо · Диван", 6, crate::construction::Form::Sofa),
    2075 Build262R3 => crate::construction::block_info("Железо · Диван", 6, crate::construction::Form::Sofa),
    2076 Build263R0 => crate::construction::block_info("Медь · Диван", 7, crate::construction::Form::Sofa),
    2077 Build263R1 => crate::construction::block_info("Медь · Диван", 7, crate::construction::Form::Sofa),
    2078 Build263R2 => crate::construction::block_info("Медь · Диван", 7, crate::construction::Form::Sofa),
    2079 Build263R3 => crate::construction::block_info("Медь · Диван", 7, crate::construction::Form::Sofa),
    2080 Build264R0 => crate::construction::block_info("Дуб · Угловой диван", 0, crate::construction::Form::SofaCorner),
    2081 Build264R1 => crate::construction::block_info("Дуб · Угловой диван", 0, crate::construction::Form::SofaCorner),
    2082 Build264R2 => crate::construction::block_info("Дуб · Угловой диван", 0, crate::construction::Form::SofaCorner),
    2083 Build264R3 => crate::construction::block_info("Дуб · Угловой диван", 0, crate::construction::Form::SofaCorner),
    2084 Build265R0 => crate::construction::block_info("Берёза · Угловой диван", 1, crate::construction::Form::SofaCorner),
    2085 Build265R1 => crate::construction::block_info("Берёза · Угловой диван", 1, crate::construction::Form::SofaCorner),
    2086 Build265R2 => crate::construction::block_info("Берёза · Угловой диван", 1, crate::construction::Form::SofaCorner),
    2087 Build265R3 => crate::construction::block_info("Берёза · Угловой диван", 1, crate::construction::Form::SofaCorner),
    2088 Build266R0 => crate::construction::block_info("Ель · Угловой диван", 2, crate::construction::Form::SofaCorner),
    2089 Build266R1 => crate::construction::block_info("Ель · Угловой диван", 2, crate::construction::Form::SofaCorner),
    2090 Build266R2 => crate::construction::block_info("Ель · Угловой диван", 2, crate::construction::Form::SofaCorner),
    2091 Build266R3 => crate::construction::block_info("Ель · Угловой диван", 2, crate::construction::Form::SofaCorner),
    2092 Build267R0 => crate::construction::block_info("Железо · Угловой диван", 6, crate::construction::Form::SofaCorner),
    2093 Build267R1 => crate::construction::block_info("Железо · Угловой диван", 6, crate::construction::Form::SofaCorner),
    2094 Build267R2 => crate::construction::block_info("Железо · Угловой диван", 6, crate::construction::Form::SofaCorner),
    2095 Build267R3 => crate::construction::block_info("Железо · Угловой диван", 6, crate::construction::Form::SofaCorner),
    2096 Build268R0 => crate::construction::block_info("Медь · Угловой диван", 7, crate::construction::Form::SofaCorner),
    2097 Build268R1 => crate::construction::block_info("Медь · Угловой диван", 7, crate::construction::Form::SofaCorner),
    2098 Build268R2 => crate::construction::block_info("Медь · Угловой диван", 7, crate::construction::Form::SofaCorner),
    2099 Build268R3 => crate::construction::block_info("Медь · Угловой диван", 7, crate::construction::Form::SofaCorner),
    2100 Build269R0 => crate::construction::block_info("Дуб · Кровать", 0, crate::construction::Form::Bed),
    2101 Build269R1 => crate::construction::block_info("Дуб · Кровать", 0, crate::construction::Form::Bed),
    2102 Build269R2 => crate::construction::block_info("Дуб · Кровать", 0, crate::construction::Form::Bed),
    2103 Build269R3 => crate::construction::block_info("Дуб · Кровать", 0, crate::construction::Form::Bed),
    2104 Build270R0 => crate::construction::block_info("Берёза · Кровать", 1, crate::construction::Form::Bed),
    2105 Build270R1 => crate::construction::block_info("Берёза · Кровать", 1, crate::construction::Form::Bed),
    2106 Build270R2 => crate::construction::block_info("Берёза · Кровать", 1, crate::construction::Form::Bed),
    2107 Build270R3 => crate::construction::block_info("Берёза · Кровать", 1, crate::construction::Form::Bed),
    2108 Build271R0 => crate::construction::block_info("Ель · Кровать", 2, crate::construction::Form::Bed),
    2109 Build271R1 => crate::construction::block_info("Ель · Кровать", 2, crate::construction::Form::Bed),
    2110 Build271R2 => crate::construction::block_info("Ель · Кровать", 2, crate::construction::Form::Bed),
    2111 Build271R3 => crate::construction::block_info("Ель · Кровать", 2, crate::construction::Form::Bed),
    2112 Build272R0 => crate::construction::block_info("Железо · Кровать", 6, crate::construction::Form::Bed),
    2113 Build272R1 => crate::construction::block_info("Железо · Кровать", 6, crate::construction::Form::Bed),
    2114 Build272R2 => crate::construction::block_info("Железо · Кровать", 6, crate::construction::Form::Bed),
    2115 Build272R3 => crate::construction::block_info("Железо · Кровать", 6, crate::construction::Form::Bed),
    2116 Build273R0 => crate::construction::block_info("Медь · Кровать", 7, crate::construction::Form::Bed),
    2117 Build273R1 => crate::construction::block_info("Медь · Кровать", 7, crate::construction::Form::Bed),
    2118 Build273R2 => crate::construction::block_info("Медь · Кровать", 7, crate::construction::Form::Bed),
    2119 Build273R3 => crate::construction::block_info("Медь · Кровать", 7, crate::construction::Form::Bed),
    2120 Build274R0 => crate::construction::block_info("Дуб · Шкаф", 0, crate::construction::Form::Wardrobe),
    2121 Build274R1 => crate::construction::block_info("Дуб · Шкаф", 0, crate::construction::Form::Wardrobe),
    2122 Build274R2 => crate::construction::block_info("Дуб · Шкаф", 0, crate::construction::Form::Wardrobe),
    2123 Build274R3 => crate::construction::block_info("Дуб · Шкаф", 0, crate::construction::Form::Wardrobe),
    2124 Build275R0 => crate::construction::block_info("Берёза · Шкаф", 1, crate::construction::Form::Wardrobe),
    2125 Build275R1 => crate::construction::block_info("Берёза · Шкаф", 1, crate::construction::Form::Wardrobe),
    2126 Build275R2 => crate::construction::block_info("Берёза · Шкаф", 1, crate::construction::Form::Wardrobe),
    2127 Build275R3 => crate::construction::block_info("Берёза · Шкаф", 1, crate::construction::Form::Wardrobe),
    2128 Build276R0 => crate::construction::block_info("Ель · Шкаф", 2, crate::construction::Form::Wardrobe),
    2129 Build276R1 => crate::construction::block_info("Ель · Шкаф", 2, crate::construction::Form::Wardrobe),
    2130 Build276R2 => crate::construction::block_info("Ель · Шкаф", 2, crate::construction::Form::Wardrobe),
    2131 Build276R3 => crate::construction::block_info("Ель · Шкаф", 2, crate::construction::Form::Wardrobe),
    2132 Build277R0 => crate::construction::block_info("Железо · Шкаф", 6, crate::construction::Form::Wardrobe),
    2133 Build277R1 => crate::construction::block_info("Железо · Шкаф", 6, crate::construction::Form::Wardrobe),
    2134 Build277R2 => crate::construction::block_info("Железо · Шкаф", 6, crate::construction::Form::Wardrobe),
    2135 Build277R3 => crate::construction::block_info("Железо · Шкаф", 6, crate::construction::Form::Wardrobe),
    2136 Build278R0 => crate::construction::block_info("Медь · Шкаф", 7, crate::construction::Form::Wardrobe),
    2137 Build278R1 => crate::construction::block_info("Медь · Шкаф", 7, crate::construction::Form::Wardrobe),
    2138 Build278R2 => crate::construction::block_info("Медь · Шкаф", 7, crate::construction::Form::Wardrobe),
    2139 Build278R3 => crate::construction::block_info("Медь · Шкаф", 7, crate::construction::Form::Wardrobe),
    2140 Build279R0 => crate::construction::block_info("Дуб · Полка", 0, crate::construction::Form::Shelf),
    2141 Build279R1 => crate::construction::block_info("Дуб · Полка", 0, crate::construction::Form::Shelf),
    2142 Build279R2 => crate::construction::block_info("Дуб · Полка", 0, crate::construction::Form::Shelf),
    2143 Build279R3 => crate::construction::block_info("Дуб · Полка", 0, crate::construction::Form::Shelf),
    2144 Build280R0 => crate::construction::block_info("Берёза · Полка", 1, crate::construction::Form::Shelf),
    2145 Build280R1 => crate::construction::block_info("Берёза · Полка", 1, crate::construction::Form::Shelf),
    2146 Build280R2 => crate::construction::block_info("Берёза · Полка", 1, crate::construction::Form::Shelf),
    2147 Build280R3 => crate::construction::block_info("Берёза · Полка", 1, crate::construction::Form::Shelf),
    2148 Build281R0 => crate::construction::block_info("Ель · Полка", 2, crate::construction::Form::Shelf),
    2149 Build281R1 => crate::construction::block_info("Ель · Полка", 2, crate::construction::Form::Shelf),
    2150 Build281R2 => crate::construction::block_info("Ель · Полка", 2, crate::construction::Form::Shelf),
    2151 Build281R3 => crate::construction::block_info("Ель · Полка", 2, crate::construction::Form::Shelf),
    2152 Build282R0 => crate::construction::block_info("Железо · Полка", 6, crate::construction::Form::Shelf),
    2153 Build282R1 => crate::construction::block_info("Железо · Полка", 6, crate::construction::Form::Shelf),
    2154 Build282R2 => crate::construction::block_info("Железо · Полка", 6, crate::construction::Form::Shelf),
    2155 Build282R3 => crate::construction::block_info("Железо · Полка", 6, crate::construction::Form::Shelf),
    2156 Build283R0 => crate::construction::block_info("Медь · Полка", 7, crate::construction::Form::Shelf),
    2157 Build283R1 => crate::construction::block_info("Медь · Полка", 7, crate::construction::Form::Shelf),
    2158 Build283R2 => crate::construction::block_info("Медь · Полка", 7, crate::construction::Form::Shelf),
    2159 Build283R3 => crate::construction::block_info("Медь · Полка", 7, crate::construction::Form::Shelf),
    2160 Build284R0 => crate::construction::block_info("Дуб · Тумба", 0, crate::construction::Form::Nightstand),
    2161 Build284R1 => crate::construction::block_info("Дуб · Тумба", 0, crate::construction::Form::Nightstand),
    2162 Build284R2 => crate::construction::block_info("Дуб · Тумба", 0, crate::construction::Form::Nightstand),
    2163 Build284R3 => crate::construction::block_info("Дуб · Тумба", 0, crate::construction::Form::Nightstand),
    2164 Build285R0 => crate::construction::block_info("Берёза · Тумба", 1, crate::construction::Form::Nightstand),
    2165 Build285R1 => crate::construction::block_info("Берёза · Тумба", 1, crate::construction::Form::Nightstand),
    2166 Build285R2 => crate::construction::block_info("Берёза · Тумба", 1, crate::construction::Form::Nightstand),
    2167 Build285R3 => crate::construction::block_info("Берёза · Тумба", 1, crate::construction::Form::Nightstand),
    2168 Build286R0 => crate::construction::block_info("Ель · Тумба", 2, crate::construction::Form::Nightstand),
    2169 Build286R1 => crate::construction::block_info("Ель · Тумба", 2, crate::construction::Form::Nightstand),
    2170 Build286R2 => crate::construction::block_info("Ель · Тумба", 2, crate::construction::Form::Nightstand),
    2171 Build286R3 => crate::construction::block_info("Ель · Тумба", 2, crate::construction::Form::Nightstand),
    2172 Build287R0 => crate::construction::block_info("Железо · Тумба", 6, crate::construction::Form::Nightstand),
    2173 Build287R1 => crate::construction::block_info("Железо · Тумба", 6, crate::construction::Form::Nightstand),
    2174 Build287R2 => crate::construction::block_info("Железо · Тумба", 6, crate::construction::Form::Nightstand),
    2175 Build287R3 => crate::construction::block_info("Железо · Тумба", 6, crate::construction::Form::Nightstand),
    2176 Build288R0 => crate::construction::block_info("Медь · Тумба", 7, crate::construction::Form::Nightstand),
    2177 Build288R1 => crate::construction::block_info("Медь · Тумба", 7, crate::construction::Form::Nightstand),
    2178 Build288R2 => crate::construction::block_info("Медь · Тумба", 7, crate::construction::Form::Nightstand),
    2179 Build288R3 => crate::construction::block_info("Медь · Тумба", 7, crate::construction::Form::Nightstand),
    2180 Build289R0 => crate::construction::block_info("Дуб · Кухонный шкаф", 0, crate::construction::Form::KitchenBase),
    2181 Build289R1 => crate::construction::block_info("Дуб · Кухонный шкаф", 0, crate::construction::Form::KitchenBase),
    2182 Build289R2 => crate::construction::block_info("Дуб · Кухонный шкаф", 0, crate::construction::Form::KitchenBase),
    2183 Build289R3 => crate::construction::block_info("Дуб · Кухонный шкаф", 0, crate::construction::Form::KitchenBase),
    2184 Build290R0 => crate::construction::block_info("Берёза · Кухонный шкаф", 1, crate::construction::Form::KitchenBase),
    2185 Build290R1 => crate::construction::block_info("Берёза · Кухонный шкаф", 1, crate::construction::Form::KitchenBase),
    2186 Build290R2 => crate::construction::block_info("Берёза · Кухонный шкаф", 1, crate::construction::Form::KitchenBase),
    2187 Build290R3 => crate::construction::block_info("Берёза · Кухонный шкаф", 1, crate::construction::Form::KitchenBase),
    2188 Build291R0 => crate::construction::block_info("Ель · Кухонный шкаф", 2, crate::construction::Form::KitchenBase),
    2189 Build291R1 => crate::construction::block_info("Ель · Кухонный шкаф", 2, crate::construction::Form::KitchenBase),
    2190 Build291R2 => crate::construction::block_info("Ель · Кухонный шкаф", 2, crate::construction::Form::KitchenBase),
    2191 Build291R3 => crate::construction::block_info("Ель · Кухонный шкаф", 2, crate::construction::Form::KitchenBase),
    2192 Build292R0 => crate::construction::block_info("Железо · Кухонный шкаф", 6, crate::construction::Form::KitchenBase),
    2193 Build292R1 => crate::construction::block_info("Железо · Кухонный шкаф", 6, crate::construction::Form::KitchenBase),
    2194 Build292R2 => crate::construction::block_info("Железо · Кухонный шкаф", 6, crate::construction::Form::KitchenBase),
    2195 Build292R3 => crate::construction::block_info("Железо · Кухонный шкаф", 6, crate::construction::Form::KitchenBase),
    2196 Build293R0 => crate::construction::block_info("Медь · Кухонный шкаф", 7, crate::construction::Form::KitchenBase),
    2197 Build293R1 => crate::construction::block_info("Медь · Кухонный шкаф", 7, crate::construction::Form::KitchenBase),
    2198 Build293R2 => crate::construction::block_info("Медь · Кухонный шкаф", 7, crate::construction::Form::KitchenBase),
    2199 Build293R3 => crate::construction::block_info("Медь · Кухонный шкаф", 7, crate::construction::Form::KitchenBase),
    2200 Build294R0 => crate::construction::block_info("Дуб · Кухонная столешница", 0, crate::construction::Form::Counter),
    2201 Build294R1 => crate::construction::block_info("Дуб · Кухонная столешница", 0, crate::construction::Form::Counter),
    2202 Build294R2 => crate::construction::block_info("Дуб · Кухонная столешница", 0, crate::construction::Form::Counter),
    2203 Build294R3 => crate::construction::block_info("Дуб · Кухонная столешница", 0, crate::construction::Form::Counter),
    2204 Build295R0 => crate::construction::block_info("Берёза · Кухонная столешница", 1, crate::construction::Form::Counter),
    2205 Build295R1 => crate::construction::block_info("Берёза · Кухонная столешница", 1, crate::construction::Form::Counter),
    2206 Build295R2 => crate::construction::block_info("Берёза · Кухонная столешница", 1, crate::construction::Form::Counter),
    2207 Build295R3 => crate::construction::block_info("Берёза · Кухонная столешница", 1, crate::construction::Form::Counter),
    2208 Build296R0 => crate::construction::block_info("Ель · Кухонная столешница", 2, crate::construction::Form::Counter),
    2209 Build296R1 => crate::construction::block_info("Ель · Кухонная столешница", 2, crate::construction::Form::Counter),
    2210 Build296R2 => crate::construction::block_info("Ель · Кухонная столешница", 2, crate::construction::Form::Counter),
    2211 Build296R3 => crate::construction::block_info("Ель · Кухонная столешница", 2, crate::construction::Form::Counter),
    2212 Build297R0 => crate::construction::block_info("Железо · Кухонная столешница", 6, crate::construction::Form::Counter),
    2213 Build297R1 => crate::construction::block_info("Железо · Кухонная столешница", 6, crate::construction::Form::Counter),
    2214 Build297R2 => crate::construction::block_info("Железо · Кухонная столешница", 6, crate::construction::Form::Counter),
    2215 Build297R3 => crate::construction::block_info("Железо · Кухонная столешница", 6, crate::construction::Form::Counter),
    2216 Build298R0 => crate::construction::block_info("Медь · Кухонная столешница", 7, crate::construction::Form::Counter),
    2217 Build298R1 => crate::construction::block_info("Медь · Кухонная столешница", 7, crate::construction::Form::Counter),
    2218 Build298R2 => crate::construction::block_info("Медь · Кухонная столешница", 7, crate::construction::Form::Counter),
    2219 Build298R3 => crate::construction::block_info("Медь · Кухонная столешница", 7, crate::construction::Form::Counter),
    2220 Build299R0 => crate::construction::block_info("Дуб · Мойка", 0, crate::construction::Form::Sink),
    2221 Build299R1 => crate::construction::block_info("Дуб · Мойка", 0, crate::construction::Form::Sink),
    2222 Build299R2 => crate::construction::block_info("Дуб · Мойка", 0, crate::construction::Form::Sink),
    2223 Build299R3 => crate::construction::block_info("Дуб · Мойка", 0, crate::construction::Form::Sink),
    2224 Build300R0 => crate::construction::block_info("Берёза · Мойка", 1, crate::construction::Form::Sink),
    2225 Build300R1 => crate::construction::block_info("Берёза · Мойка", 1, crate::construction::Form::Sink),
    2226 Build300R2 => crate::construction::block_info("Берёза · Мойка", 1, crate::construction::Form::Sink),
    2227 Build300R3 => crate::construction::block_info("Берёза · Мойка", 1, crate::construction::Form::Sink),
    2228 Build301R0 => crate::construction::block_info("Ель · Мойка", 2, crate::construction::Form::Sink),
    2229 Build301R1 => crate::construction::block_info("Ель · Мойка", 2, crate::construction::Form::Sink),
    2230 Build301R2 => crate::construction::block_info("Ель · Мойка", 2, crate::construction::Form::Sink),
    2231 Build301R3 => crate::construction::block_info("Ель · Мойка", 2, crate::construction::Form::Sink),
    2232 Build302R0 => crate::construction::block_info("Железо · Мойка", 6, crate::construction::Form::Sink),
    2233 Build302R1 => crate::construction::block_info("Железо · Мойка", 6, crate::construction::Form::Sink),
    2234 Build302R2 => crate::construction::block_info("Железо · Мойка", 6, crate::construction::Form::Sink),
    2235 Build302R3 => crate::construction::block_info("Железо · Мойка", 6, crate::construction::Form::Sink),
    2236 Build303R0 => crate::construction::block_info("Медь · Мойка", 7, crate::construction::Form::Sink),
    2237 Build303R1 => crate::construction::block_info("Медь · Мойка", 7, crate::construction::Form::Sink),
    2238 Build303R2 => crate::construction::block_info("Медь · Мойка", 7, crate::construction::Form::Sink),
    2239 Build303R3 => crate::construction::block_info("Медь · Мойка", 7, crate::construction::Form::Sink),
    2240 Build304R0 => crate::construction::block_info("Дуб · Торшер", 0, crate::construction::Form::FloorLamp),
    2241 Build304R1 => crate::construction::block_info("Дуб · Торшер", 0, crate::construction::Form::FloorLamp),
    2242 Build304R2 => crate::construction::block_info("Дуб · Торшер", 0, crate::construction::Form::FloorLamp),
    2243 Build304R3 => crate::construction::block_info("Дуб · Торшер", 0, crate::construction::Form::FloorLamp),
    2244 Build305R0 => crate::construction::block_info("Берёза · Торшер", 1, crate::construction::Form::FloorLamp),
    2245 Build305R1 => crate::construction::block_info("Берёза · Торшер", 1, crate::construction::Form::FloorLamp),
    2246 Build305R2 => crate::construction::block_info("Берёза · Торшер", 1, crate::construction::Form::FloorLamp),
    2247 Build305R3 => crate::construction::block_info("Берёза · Торшер", 1, crate::construction::Form::FloorLamp),
    2248 Build306R0 => crate::construction::block_info("Ель · Торшер", 2, crate::construction::Form::FloorLamp),
    2249 Build306R1 => crate::construction::block_info("Ель · Торшер", 2, crate::construction::Form::FloorLamp),
    2250 Build306R2 => crate::construction::block_info("Ель · Торшер", 2, crate::construction::Form::FloorLamp),
    2251 Build306R3 => crate::construction::block_info("Ель · Торшер", 2, crate::construction::Form::FloorLamp),
    2252 Build307R0 => crate::construction::block_info("Железо · Торшер", 6, crate::construction::Form::FloorLamp),
    2253 Build307R1 => crate::construction::block_info("Железо · Торшер", 6, crate::construction::Form::FloorLamp),
    2254 Build307R2 => crate::construction::block_info("Железо · Торшер", 6, crate::construction::Form::FloorLamp),
    2255 Build307R3 => crate::construction::block_info("Железо · Торшер", 6, crate::construction::Form::FloorLamp),
    2256 Build308R0 => crate::construction::block_info("Дуб · Штора", 0, crate::construction::Form::Curtain),
    2257 Build308R1 => crate::construction::block_info("Дуб · Штора", 0, crate::construction::Form::Curtain),
    2258 Build308R2 => crate::construction::block_info("Дуб · Штора", 0, crate::construction::Form::Curtain),
    2259 Build308R3 => crate::construction::block_info("Дуб · Штора", 0, crate::construction::Form::Curtain),
    2260 Build309R0 => crate::construction::block_info("Берёза · Штора", 1, crate::construction::Form::Curtain),
    2261 Build309R1 => crate::construction::block_info("Берёза · Штора", 1, crate::construction::Form::Curtain),
    2262 Build309R2 => crate::construction::block_info("Берёза · Штора", 1, crate::construction::Form::Curtain),
    2263 Build309R3 => crate::construction::block_info("Берёза · Штора", 1, crate::construction::Form::Curtain),
    2264 Build310R0 => crate::construction::block_info("Ель · Штора", 2, crate::construction::Form::Curtain),
    2265 Build310R1 => crate::construction::block_info("Ель · Штора", 2, crate::construction::Form::Curtain),
    2266 Build310R2 => crate::construction::block_info("Ель · Штора", 2, crate::construction::Form::Curtain),
    2267 Build310R3 => crate::construction::block_info("Ель · Штора", 2, crate::construction::Form::Curtain),
    2268 Build311R0 => crate::construction::block_info("Железо · Штора", 6, crate::construction::Form::Curtain),
    2269 Build311R1 => crate::construction::block_info("Железо · Штора", 6, crate::construction::Form::Curtain),
    2270 Build311R2 => crate::construction::block_info("Железо · Штора", 6, crate::construction::Form::Curtain),
    2271 Build311R3 => crate::construction::block_info("Железо · Штора", 6, crate::construction::Form::Curtain),
    2272 Build312R0 => crate::construction::block_info("Дуб · Ковёр", 0, crate::construction::Form::Rug),
    2273 Build312R1 => crate::construction::block_info("Дуб · Ковёр", 0, crate::construction::Form::Rug),
    2274 Build312R2 => crate::construction::block_info("Дуб · Ковёр", 0, crate::construction::Form::Rug),
    2275 Build312R3 => crate::construction::block_info("Дуб · Ковёр", 0, crate::construction::Form::Rug),
    2276 Build313R0 => crate::construction::block_info("Берёза · Ковёр", 1, crate::construction::Form::Rug),
    2277 Build313R1 => crate::construction::block_info("Берёза · Ковёр", 1, crate::construction::Form::Rug),
    2278 Build313R2 => crate::construction::block_info("Берёза · Ковёр", 1, crate::construction::Form::Rug),
    2279 Build313R3 => crate::construction::block_info("Берёза · Ковёр", 1, crate::construction::Form::Rug),
    2280 Build314R0 => crate::construction::block_info("Ель · Ковёр", 2, crate::construction::Form::Rug),
    2281 Build314R1 => crate::construction::block_info("Ель · Ковёр", 2, crate::construction::Form::Rug),
    2282 Build314R2 => crate::construction::block_info("Ель · Ковёр", 2, crate::construction::Form::Rug),
    2283 Build314R3 => crate::construction::block_info("Ель · Ковёр", 2, crate::construction::Form::Rug),
    2284 Build315R0 => crate::construction::block_info("Железо · Ковёр", 6, crate::construction::Form::Rug),
    2285 Build315R1 => crate::construction::block_info("Железо · Ковёр", 6, crate::construction::Form::Rug),
    2286 Build315R2 => crate::construction::block_info("Железо · Ковёр", 6, crate::construction::Form::Rug),
    2287 Build315R3 => crate::construction::block_info("Железо · Ковёр", 6, crate::construction::Form::Rug),
    2288 Build316R0 => crate::construction::block_info("Дуб · Зеркало", 0, crate::construction::Form::Mirror),
    2289 Build316R1 => crate::construction::block_info("Дуб · Зеркало", 0, crate::construction::Form::Mirror),
    2290 Build316R2 => crate::construction::block_info("Дуб · Зеркало", 0, crate::construction::Form::Mirror),
    2291 Build316R3 => crate::construction::block_info("Дуб · Зеркало", 0, crate::construction::Form::Mirror),
    2292 Build317R0 => crate::construction::block_info("Берёза · Зеркало", 1, crate::construction::Form::Mirror),
    2293 Build317R1 => crate::construction::block_info("Берёза · Зеркало", 1, crate::construction::Form::Mirror),
    2294 Build317R2 => crate::construction::block_info("Берёза · Зеркало", 1, crate::construction::Form::Mirror),
    2295 Build317R3 => crate::construction::block_info("Берёза · Зеркало", 1, crate::construction::Form::Mirror),
    2296 Build318R0 => crate::construction::block_info("Ель · Зеркало", 2, crate::construction::Form::Mirror),
    2297 Build318R1 => crate::construction::block_info("Ель · Зеркало", 2, crate::construction::Form::Mirror),
    2298 Build318R2 => crate::construction::block_info("Ель · Зеркало", 2, crate::construction::Form::Mirror),
    2299 Build318R3 => crate::construction::block_info("Ель · Зеркало", 2, crate::construction::Form::Mirror),
    2300 Build319R0 => crate::construction::block_info("Железо · Зеркало", 6, crate::construction::Form::Mirror),
    2301 Build319R1 => crate::construction::block_info("Железо · Зеркало", 6, crate::construction::Form::Mirror),
    2302 Build319R2 => crate::construction::block_info("Железо · Зеркало", 6, crate::construction::Form::Mirror),
    2303 Build319R3 => crate::construction::block_info("Железо · Зеркало", 6, crate::construction::Form::Mirror),
    2304 Build320R0 => crate::construction::block_info("Дуб · Горшок с растением", 0, crate::construction::Form::FlowerPot),
    2305 Build320R1 => crate::construction::block_info("Дуб · Горшок с растением", 0, crate::construction::Form::FlowerPot),
    2306 Build320R2 => crate::construction::block_info("Дуб · Горшок с растением", 0, crate::construction::Form::FlowerPot),
    2307 Build320R3 => crate::construction::block_info("Дуб · Горшок с растением", 0, crate::construction::Form::FlowerPot),
    2308 Build321R0 => crate::construction::block_info("Берёза · Горшок с растением", 1, crate::construction::Form::FlowerPot),
    2309 Build321R1 => crate::construction::block_info("Берёза · Горшок с растением", 1, crate::construction::Form::FlowerPot),
    2310 Build321R2 => crate::construction::block_info("Берёза · Горшок с растением", 1, crate::construction::Form::FlowerPot),
    2311 Build321R3 => crate::construction::block_info("Берёза · Горшок с растением", 1, crate::construction::Form::FlowerPot),
    2312 Build322R0 => crate::construction::block_info("Ель · Горшок с растением", 2, crate::construction::Form::FlowerPot),
    2313 Build322R1 => crate::construction::block_info("Ель · Горшок с растением", 2, crate::construction::Form::FlowerPot),
    2314 Build322R2 => crate::construction::block_info("Ель · Горшок с растением", 2, crate::construction::Form::FlowerPot),
    2315 Build322R3 => crate::construction::block_info("Ель · Горшок с растением", 2, crate::construction::Form::FlowerPot),
    2316 Build323R0 => crate::construction::block_info("Железо · Горшок с растением", 6, crate::construction::Form::FlowerPot),
    2317 Build323R1 => crate::construction::block_info("Железо · Горшок с растением", 6, crate::construction::Form::FlowerPot),
    2318 Build323R2 => crate::construction::block_info("Железо · Горшок с растением", 6, crate::construction::Form::FlowerPot),
    2319 Build323R3 => crate::construction::block_info("Железо · Горшок с растением", 6, crate::construction::Form::FlowerPot),
    2320 MoreBuild0R0 => crate::construction::block_info("Дуб · Сундук", 0, crate::construction::Form::StorageChest),
    2321 MoreBuild0R1 => crate::construction::block_info("Дуб · Сундук", 0, crate::construction::Form::StorageChest),
    2322 MoreBuild0R2 => crate::construction::block_info("Дуб · Сундук", 0, crate::construction::Form::StorageChest),
    2323 MoreBuild0R3 => crate::construction::block_info("Дуб · Сундук", 0, crate::construction::Form::StorageChest),
    2324 MoreBuild1R0 => crate::construction::block_info("Берёза · Сундук", 1, crate::construction::Form::StorageChest),
    2325 MoreBuild1R1 => crate::construction::block_info("Берёза · Сундук", 1, crate::construction::Form::StorageChest),
    2326 MoreBuild1R2 => crate::construction::block_info("Берёза · Сундук", 1, crate::construction::Form::StorageChest),
    2327 MoreBuild1R3 => crate::construction::block_info("Берёза · Сундук", 1, crate::construction::Form::StorageChest),
    2328 MoreBuild2R0 => crate::construction::block_info("Ель · Сундук", 2, crate::construction::Form::StorageChest),
    2329 MoreBuild2R1 => crate::construction::block_info("Ель · Сундук", 2, crate::construction::Form::StorageChest),
    2330 MoreBuild2R2 => crate::construction::block_info("Ель · Сундук", 2, crate::construction::Form::StorageChest),
    2331 MoreBuild2R3 => crate::construction::block_info("Ель · Сундук", 2, crate::construction::Form::StorageChest),
    2332 MoreBuild3R0 => crate::construction::block_info("Железо · Сундук", 6, crate::construction::Form::StorageChest),
    2333 MoreBuild3R1 => crate::construction::block_info("Железо · Сундук", 6, crate::construction::Form::StorageChest),
    2334 MoreBuild3R2 => crate::construction::block_info("Железо · Сундук", 6, crate::construction::Form::StorageChest),
    2335 MoreBuild3R3 => crate::construction::block_info("Железо · Сундук", 6, crate::construction::Form::StorageChest),
    2336 MoreBuild4R0 => crate::construction::block_info("Дуб · Бочка", 0, crate::construction::Form::Barrel),
    2337 MoreBuild4R1 => crate::construction::block_info("Дуб · Бочка", 0, crate::construction::Form::Barrel),
    2338 MoreBuild4R2 => crate::construction::block_info("Дуб · Бочка", 0, crate::construction::Form::Barrel),
    2339 MoreBuild4R3 => crate::construction::block_info("Дуб · Бочка", 0, crate::construction::Form::Barrel),
    2340 MoreBuild5R0 => crate::construction::block_info("Берёза · Бочка", 1, crate::construction::Form::Barrel),
    2341 MoreBuild5R1 => crate::construction::block_info("Берёза · Бочка", 1, crate::construction::Form::Barrel),
    2342 MoreBuild5R2 => crate::construction::block_info("Берёза · Бочка", 1, crate::construction::Form::Barrel),
    2343 MoreBuild5R3 => crate::construction::block_info("Берёза · Бочка", 1, crate::construction::Form::Barrel),
    2344 MoreBuild6R0 => crate::construction::block_info("Ель · Бочка", 2, crate::construction::Form::Barrel),
    2345 MoreBuild6R1 => crate::construction::block_info("Ель · Бочка", 2, crate::construction::Form::Barrel),
    2346 MoreBuild6R2 => crate::construction::block_info("Ель · Бочка", 2, crate::construction::Form::Barrel),
    2347 MoreBuild6R3 => crate::construction::block_info("Ель · Бочка", 2, crate::construction::Form::Barrel),
    2348 MoreBuild7R0 => crate::construction::block_info("Дуб · Путевой ящик", 0, crate::construction::Form::PortableCrate),
    2349 MoreBuild7R1 => crate::construction::block_info("Дуб · Путевой ящик", 0, crate::construction::Form::PortableCrate),
    2350 MoreBuild7R2 => crate::construction::block_info("Дуб · Путевой ящик", 0, crate::construction::Form::PortableCrate),
    2351 MoreBuild7R3 => crate::construction::block_info("Дуб · Путевой ящик", 0, crate::construction::Form::PortableCrate),
    2352 MoreBuild8R0 => crate::construction::block_info("Берёза · Путевой ящик", 1, crate::construction::Form::PortableCrate),
    2353 MoreBuild8R1 => crate::construction::block_info("Берёза · Путевой ящик", 1, crate::construction::Form::PortableCrate),
    2354 MoreBuild8R2 => crate::construction::block_info("Берёза · Путевой ящик", 1, crate::construction::Form::PortableCrate),
    2355 MoreBuild8R3 => crate::construction::block_info("Берёза · Путевой ящик", 1, crate::construction::Form::PortableCrate),
    2356 MoreBuild9R0 => crate::construction::block_info("Ель · Путевой ящик", 2, crate::construction::Form::PortableCrate),
    2357 MoreBuild9R1 => crate::construction::block_info("Ель · Путевой ящик", 2, crate::construction::Form::PortableCrate),
    2358 MoreBuild9R2 => crate::construction::block_info("Ель · Путевой ящик", 2, crate::construction::Form::PortableCrate),
    2359 MoreBuild9R3 => crate::construction::block_info("Ель · Путевой ящик", 2, crate::construction::Form::PortableCrate),
    2360 MoreBuild10R0 => crate::construction::block_info("Железо · Путевой ящик", 6, crate::construction::Form::PortableCrate),
    2361 MoreBuild10R1 => crate::construction::block_info("Железо · Путевой ящик", 6, crate::construction::Form::PortableCrate),
    2362 MoreBuild10R2 => crate::construction::block_info("Железо · Путевой ящик", 6, crate::construction::Form::PortableCrate),
    2363 MoreBuild10R3 => crate::construction::block_info("Железо · Путевой ящик", 6, crate::construction::Form::PortableCrate),
    2364 MoreBuild11R0 => crate::construction::block_info("Железо · Резонансный сундук", 6, crate::construction::Form::SharedChest),
    2365 MoreBuild11R1 => crate::construction::block_info("Железо · Резонансный сундук", 6, crate::construction::Form::SharedChest),
    2366 MoreBuild11R2 => crate::construction::block_info("Железо · Резонансный сундук", 6, crate::construction::Form::SharedChest),
    2367 MoreBuild11R3 => crate::construction::block_info("Железо · Резонансный сундук", 6, crate::construction::Form::SharedChest),
    2368 MoreBuild12R0 => crate::construction::block_info("Дуб · Рамка для предмета", 0, crate::construction::Form::ItemFrame),
    2369 MoreBuild12R1 => crate::construction::block_info("Дуб · Рамка для предмета", 0, crate::construction::Form::ItemFrame),
    2370 MoreBuild12R2 => crate::construction::block_info("Дуб · Рамка для предмета", 0, crate::construction::Form::ItemFrame),
    2371 MoreBuild12R3 => crate::construction::block_info("Дуб · Рамка для предмета", 0, crate::construction::Form::ItemFrame),
    2372 MoreBuild13R0 => crate::construction::block_info("Берёза · Рамка для предмета", 1, crate::construction::Form::ItemFrame),
    2373 MoreBuild13R1 => crate::construction::block_info("Берёза · Рамка для предмета", 1, crate::construction::Form::ItemFrame),
    2374 MoreBuild13R2 => crate::construction::block_info("Берёза · Рамка для предмета", 1, crate::construction::Form::ItemFrame),
    2375 MoreBuild13R3 => crate::construction::block_info("Берёза · Рамка для предмета", 1, crate::construction::Form::ItemFrame),
    2376 MoreBuild14R0 => crate::construction::block_info("Ель · Рамка для предмета", 2, crate::construction::Form::ItemFrame),
    2377 MoreBuild14R1 => crate::construction::block_info("Ель · Рамка для предмета", 2, crate::construction::Form::ItemFrame),
    2378 MoreBuild14R2 => crate::construction::block_info("Ель · Рамка для предмета", 2, crate::construction::Form::ItemFrame),
    2379 MoreBuild14R3 => crate::construction::block_info("Ель · Рамка для предмета", 2, crate::construction::Form::ItemFrame),
    2380 MoreBuild15R0 => crate::construction::block_info("Дуб · Картина «Чаща»", 0, crate::construction::Form::PaintingForest),
    2381 MoreBuild15R1 => crate::construction::block_info("Дуб · Картина «Чаща»", 0, crate::construction::Form::PaintingForest),
    2382 MoreBuild15R2 => crate::construction::block_info("Дуб · Картина «Чаща»", 0, crate::construction::Form::PaintingForest),
    2383 MoreBuild15R3 => crate::construction::block_info("Дуб · Картина «Чаща»", 0, crate::construction::Form::PaintingForest),
    2384 MoreBuild16R0 => crate::construction::block_info("Берёза · Картина «Чаща»", 1, crate::construction::Form::PaintingForest),
    2385 MoreBuild16R1 => crate::construction::block_info("Берёза · Картина «Чаща»", 1, crate::construction::Form::PaintingForest),
    2386 MoreBuild16R2 => crate::construction::block_info("Берёза · Картина «Чаща»", 1, crate::construction::Form::PaintingForest),
    2387 MoreBuild16R3 => crate::construction::block_info("Берёза · Картина «Чаща»", 1, crate::construction::Form::PaintingForest),
    2388 MoreBuild17R0 => crate::construction::block_info("Ель · Картина «Чаща»", 2, crate::construction::Form::PaintingForest),
    2389 MoreBuild17R1 => crate::construction::block_info("Ель · Картина «Чаща»", 2, crate::construction::Form::PaintingForest),
    2390 MoreBuild17R2 => crate::construction::block_info("Ель · Картина «Чаща»", 2, crate::construction::Form::PaintingForest),
    2391 MoreBuild17R3 => crate::construction::block_info("Ель · Картина «Чаща»", 2, crate::construction::Form::PaintingForest),
    2392 MoreBuild18R0 => crate::construction::block_info("Дуб · Картина «Небо»", 0, crate::construction::Form::PaintingSky),
    2393 MoreBuild18R1 => crate::construction::block_info("Дуб · Картина «Небо»", 0, crate::construction::Form::PaintingSky),
    2394 MoreBuild18R2 => crate::construction::block_info("Дуб · Картина «Небо»", 0, crate::construction::Form::PaintingSky),
    2395 MoreBuild18R3 => crate::construction::block_info("Дуб · Картина «Небо»", 0, crate::construction::Form::PaintingSky),
    2396 MoreBuild19R0 => crate::construction::block_info("Берёза · Картина «Небо»", 1, crate::construction::Form::PaintingSky),
    2397 MoreBuild19R1 => crate::construction::block_info("Берёза · Картина «Небо»", 1, crate::construction::Form::PaintingSky),
    2398 MoreBuild19R2 => crate::construction::block_info("Берёза · Картина «Небо»", 1, crate::construction::Form::PaintingSky),
    2399 MoreBuild19R3 => crate::construction::block_info("Берёза · Картина «Небо»", 1, crate::construction::Form::PaintingSky),
    2400 MoreBuild20R0 => crate::construction::block_info("Ель · Картина «Небо»", 2, crate::construction::Form::PaintingSky),
    2401 MoreBuild20R1 => crate::construction::block_info("Ель · Картина «Небо»", 2, crate::construction::Form::PaintingSky),
    2402 MoreBuild20R2 => crate::construction::block_info("Ель · Картина «Небо»", 2, crate::construction::Form::PaintingSky),
    2403 MoreBuild20R3 => crate::construction::block_info("Ель · Картина «Небо»", 2, crate::construction::Form::PaintingSky),
    2404 MoreBuild21R0 => crate::construction::block_info("Дуб · Картина «Искрит»", 0, crate::construction::Form::PaintingAbstract),
    2405 MoreBuild21R1 => crate::construction::block_info("Дуб · Картина «Искрит»", 0, crate::construction::Form::PaintingAbstract),
    2406 MoreBuild21R2 => crate::construction::block_info("Дуб · Картина «Искрит»", 0, crate::construction::Form::PaintingAbstract),
    2407 MoreBuild21R3 => crate::construction::block_info("Дуб · Картина «Искрит»", 0, crate::construction::Form::PaintingAbstract),
    2408 MoreBuild22R0 => crate::construction::block_info("Берёза · Картина «Искрит»", 1, crate::construction::Form::PaintingAbstract),
    2409 MoreBuild22R1 => crate::construction::block_info("Берёза · Картина «Искрит»", 1, crate::construction::Form::PaintingAbstract),
    2410 MoreBuild22R2 => crate::construction::block_info("Берёза · Картина «Искрит»", 1, crate::construction::Form::PaintingAbstract),
    2411 MoreBuild22R3 => crate::construction::block_info("Берёза · Картина «Искрит»", 1, crate::construction::Form::PaintingAbstract),
    2412 MoreBuild23R0 => crate::construction::block_info("Ель · Картина «Искрит»", 2, crate::construction::Form::PaintingAbstract),
    2413 MoreBuild23R1 => crate::construction::block_info("Ель · Картина «Искрит»", 2, crate::construction::Form::PaintingAbstract),
    2414 MoreBuild23R2 => crate::construction::block_info("Ель · Картина «Искрит»", 2, crate::construction::Form::PaintingAbstract),
    2415 MoreBuild23R3 => crate::construction::block_info("Ель · Картина «Искрит»", 2, crate::construction::Form::PaintingAbstract),
    2416 MoreBuild24R0 => crate::construction::block_info("Дуб · Верстак", 0, crate::construction::Form::Workbench),
    2417 MoreBuild24R1 => crate::construction::block_info("Дуб · Верстак", 0, crate::construction::Form::Workbench),
    2418 MoreBuild24R2 => crate::construction::block_info("Дуб · Верстак", 0, crate::construction::Form::Workbench),
    2419 MoreBuild24R3 => crate::construction::block_info("Дуб · Верстак", 0, crate::construction::Form::Workbench),
    2420 MoreBuild25R0 => crate::construction::block_info("Железо · Печь", 6, crate::construction::Form::Furnace),
    2421 MoreBuild25R1 => crate::construction::block_info("Железо · Печь", 6, crate::construction::Form::Furnace),
    2422 MoreBuild25R2 => crate::construction::block_info("Железо · Печь", 6, crate::construction::Form::Furnace),
    2423 MoreBuild25R3 => crate::construction::block_info("Железо · Печь", 6, crate::construction::Form::Furnace),
    2424 MoreBuild26R0 => crate::construction::block_info("Железо · Терминал", 6, crate::construction::Form::Terminal),
    2425 MoreBuild26R1 => crate::construction::block_info("Железо · Терминал", 6, crate::construction::Form::Terminal),
    2426 MoreBuild26R2 => crate::construction::block_info("Железо · Терминал", 6, crate::construction::Form::Terminal),
    2427 MoreBuild26R3 => crate::construction::block_info("Железо · Терминал", 6, crate::construction::Form::Terminal),
    2428 MoreBuild27R0 => crate::construction::block_info("Железо · Командный блок", 6, crate::construction::Form::Command),
    2429 MoreBuild27R1 => crate::construction::block_info("Железо · Командный блок", 6, crate::construction::Form::Command),
    2430 MoreBuild27R2 => crate::construction::block_info("Железо · Командный блок", 6, crate::construction::Form::Command),
    2431 MoreBuild27R3 => crate::construction::block_info("Железо · Командный блок", 6, crate::construction::Form::Command),
    2432 MoreBuild28R0 => crate::construction::block_info("Камень · Каменный алтарь", 3, crate::construction::Form::StoneAltar),
    2433 MoreBuild28R1 => crate::construction::block_info("Камень · Каменный алтарь", 3, crate::construction::Form::StoneAltar),
    2434 MoreBuild28R2 => crate::construction::block_info("Камень · Каменный алтарь", 3, crate::construction::Form::StoneAltar),
    2435 MoreBuild28R3 => crate::construction::block_info("Камень · Каменный алтарь", 3, crate::construction::Form::StoneAltar),
    2436 MoreBuild29R0 => crate::construction::block_info("Дуб · Лесной алтарь", 0, crate::construction::Form::RootAltar),
    2437 MoreBuild29R1 => crate::construction::block_info("Дуб · Лесной алтарь", 0, crate::construction::Form::RootAltar),
    2438 MoreBuild29R2 => crate::construction::block_info("Дуб · Лесной алтарь", 0, crate::construction::Form::RootAltar),
    2439 MoreBuild29R3 => crate::construction::block_info("Дуб · Лесной алтарь", 0, crate::construction::Form::RootAltar),
    2440 MoreBuild30R0 => crate::construction::block_info("Железо · Пепельный алтарь", 6, crate::construction::Form::EmberAltar),
    2441 MoreBuild30R1 => crate::construction::block_info("Железо · Пепельный алтарь", 6, crate::construction::Form::EmberAltar),
    2442 MoreBuild30R2 => crate::construction::block_info("Железо · Пепельный алтарь", 6, crate::construction::Form::EmberAltar),
    2443 MoreBuild30R3 => crate::construction::block_info("Железо · Пепельный алтарь", 6, crate::construction::Form::EmberAltar),
    // Lit with a lighter (or by another blast): see `explosives`. Not a catalogue piece:
    // `construction::entry` has nothing for this id.
    2444 Dynamite => info("Динамит", Opaque, 0.1, Hand, column(t::DYNAMITE_SIDE, t::DYNAMITE_TOP, t::DYNAMITE_TOP)),
    // «Питомник» mini-game (see `petsim`).
    2445 CoinBlock => emissive(info("Монетный блок", Opaque, 1.2, Hand, all(t::COIN_BLOCK))),
    2446 IncubatorCommon => info("Инкубатор: обычное яйцо", Opaque, 2.0, Hand, column(t::INCUBATOR_SIDE, t::INCUBATOR_TOP, t::INCUBATOR_TOP)),
    2447 IncubatorRare => info("Инкубатор: редкое яйцо", Opaque, 2.0, Hand, column(t::INCUBATOR_SIDE + 1, t::INCUBATOR_TOP, t::INCUBATOR_TOP)),
    2448 IncubatorLegendary => info("Инкубатор: легендарное яйцо", Opaque, 2.0, Hand, column(t::INCUBATOR_SIDE + 2, t::INCUBATOR_TOP, t::INCUBATOR_TOP)),
    // «Кнопки» mini-game (see `buttonsim`): floor buttons, stand on them to buy.
    2449 ButtonMult1 => emissive(info("Кнопка: +1 множитель", Opaque, 1.0, Hand, column(t::BUTTON_PAD_SIDE, t::BUTTON_PAD, t::BUTTON_PAD_SIDE))),
    2450 ButtonMult2 => emissive(info("Кнопка: +10 множитель", Opaque, 1.0, Hand, column(t::BUTTON_PAD_SIDE, t::BUTTON_PAD + 1, t::BUTTON_PAD_SIDE))),
    2451 ButtonMult3 => emissive(info("Кнопка: +120 множитель", Opaque, 1.0, Hand, column(t::BUTTON_PAD_SIDE, t::BUTTON_PAD + 2, t::BUTTON_PAD_SIDE))),
    2452 ButtonMult4 => emissive(info("Кнопка: +1500 множитель", Opaque, 1.0, Hand, column(t::BUTTON_PAD_SIDE, t::BUTTON_PAD + 3, t::BUTTON_PAD_SIDE))),
    2453 ButtonRebirth => emissive(info("Кнопка перерождения", Opaque, 1.0, Hand, column(t::BUTTON_PAD_SIDE, t::BUTTON_PAD + 4, t::BUTTON_PAD_SIDE))),
    // Fire: burns on top of a block, spreads over wood and leaves, goes out in the rain.
    2454 Fire => emissive(not_solid(info("Огонь", Cross, 0.0, Hand, all(t::FIRE)))),
    // WolfTech (the built-in tech mod): energy and machines. Tile slots: +j, -j, up, down, +i, -i.
    2455 TechGenerator => needs_pick(info("Генератор", Opaque, 4.0, Pickaxe, [t::TECH_SIDE, t::TECH_SIDE, t::TECH_TOP, t::TECH_SIDE, t::TECH_GEN_FRONT, t::TECH_SIDE])),
    2456 SolarPanel => needs_pick(info("Солнечная панель", Shape, 3.0, Pickaxe, column(t::TECH_SIDE, t::SOLAR_TOP, t::TECH_SIDE))),
    2457 EnergyCell => needs_pick(info("Энергоячейка", Opaque, 4.0, Pickaxe, column(t::CELL_SIDE, t::TECH_TOP, t::TECH_TOP))),
    2458 Cable => info("Медный кабель", Shape, 0.5, Hand, all(t::CABLE)),
    2459 Crusher => needs_pick(info("Дробитель", Opaque, 4.0, Pickaxe, [t::TECH_SIDE, t::TECH_SIDE, t::TECH_TOP, t::TECH_SIDE, t::CRUSHER_FRONT, t::TECH_SIDE])),
    2460 InductionFurnace => needs_pick(info("Индукционная печь", Opaque, 4.0, Pickaxe, [t::TECH_SIDE, t::TECH_SIDE, t::TECH_TOP, t::TECH_SIDE, t::INDUCTION_FRONT, t::TECH_SIDE])),
    2461 Enricher => needs_pick(info("Обогатитель", Opaque, 4.0, Pickaxe, [t::TECH_SIDE, t::TECH_SIDE, t::TECH_TOP, t::TECH_SIDE, t::ENRICHER_FRONT, t::TECH_SIDE])),
    2462 BuilderTable => info("Конструктор", Opaque, 3.75, Axe, column(t::BUILDER_SIDE, t::BUILDER_TOP, t::PLANKS)),
    2463 TechGeneratorD1 => needs_pick(info("Генератор", Opaque, 4.0, Pickaxe, mtiles(t::TECH_GEN_FRONT, 1))),
    2464 TechGeneratorD2 => needs_pick(info("Генератор", Opaque, 4.0, Pickaxe, mtiles(t::TECH_GEN_FRONT, 2))),
    2465 TechGeneratorD3 => needs_pick(info("Генератор", Opaque, 4.0, Pickaxe, mtiles(t::TECH_GEN_FRONT, 3))),
    2466 CrusherD1 => needs_pick(info("Дробитель", Opaque, 4.0, Pickaxe, mtiles(t::CRUSHER_FRONT, 1))),
    2467 CrusherD2 => needs_pick(info("Дробитель", Opaque, 4.0, Pickaxe, mtiles(t::CRUSHER_FRONT, 2))),
    2468 CrusherD3 => needs_pick(info("Дробитель", Opaque, 4.0, Pickaxe, mtiles(t::CRUSHER_FRONT, 3))),
    2469 InductionFurnaceD1 => needs_pick(info("Индукционная печь", Opaque, 4.0, Pickaxe, mtiles(t::INDUCTION_FRONT, 1))),
    2470 InductionFurnaceD2 => needs_pick(info("Индукционная печь", Opaque, 4.0, Pickaxe, mtiles(t::INDUCTION_FRONT, 2))),
    2471 InductionFurnaceD3 => needs_pick(info("Индукционная печь", Opaque, 4.0, Pickaxe, mtiles(t::INDUCTION_FRONT, 3))),
    2472 EnricherD1 => needs_pick(info("Обогатитель", Opaque, 4.0, Pickaxe, mtiles(t::ENRICHER_FRONT, 1))),
    2473 EnricherD2 => needs_pick(info("Обогатитель", Opaque, 4.0, Pickaxe, mtiles(t::ENRICHER_FRONT, 2))),
    2474 EnricherD3 => needs_pick(info("Обогатитель", Opaque, 4.0, Pickaxe, mtiles(t::ENRICHER_FRONT, 3))),
    2475 WindTurbine => needs_pick(info("Ветрогенератор", Opaque, 4.0, Pickaxe, column(t::TECH_SIDE, t::WIND_TOP, t::TECH_TOP))),
    2476 GeoGenerator => needs_pick(info("Геотермальный генератор", Opaque, 4.0, Pickaxe, column(t::GEO_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2477 WaterMill => needs_pick(info("Гидрогенератор", Opaque, 4.0, Pickaxe, column(t::WATER_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2478 Reactor => needs_pick(info("Ядерный реактор", Opaque, 6.0, Pickaxe, column(t::REACTOR_SIDE, t::TECH_TOP, t::TECH_TOP))),
    2479 Compressor => needs_pick(info("Компрессор", Opaque, 4.0, Pickaxe, column(t::COMPRESSOR_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2480 Sawmill => needs_pick(info("Пилорама", Opaque, 4.0, Pickaxe, column(t::SAWMILL_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2481 Extractor => needs_pick(info("Экстрактор", Opaque, 4.0, Pickaxe, column(t::EXTRACTOR_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2482 Shaft => info("Вал", Shape, 1.5, Pickaxe, all(t::KIN_SHAFT)),
    2483 ShaftJ => info("Вал", Shape, 1.5, Pickaxe, all(t::KIN_SHAFT)),
    2484 ShaftK => info("Вал", Shape, 1.5, Pickaxe, all(t::KIN_SHAFT)),
    2485 Cogwheel => info("Шестерня", Shape, 1.5, Axe, all(t::KIN_COG)),
    2486 CogwheelJ => info("Шестерня", Shape, 1.5, Axe, all(t::KIN_COG)),
    2487 CogwheelK => info("Шестерня", Shape, 1.5, Axe, all(t::KIN_COG)),
    2488 WaterWheel => info("Водяное колесо", Shape, 2.0, Axe, all(t::KIN_WHEEL)),
    2489 WaterWheelJ => info("Водяное колесо", Shape, 2.0, Axe, all(t::KIN_WHEEL)),
    2490 HandCrank => info("Ручной кривошип", Shape, 1.0, Axe, all(t::KIN_CRANK)),
    2491 MechPress => needs_pick(info("Механический пресс", Opaque, 3.5, Pickaxe, column(t::KIN_PRESS_SIDE, t::KIN_PRESS_TOP, t::KIN_PRESS_TOP))),
    2492 Millstone => needs_pick(info("Жернова", Opaque, 3.5, Pickaxe, column(t::KIN_MILL_SIDE, t::KIN_MILL_TOP, t::KIN_MILL_TOP))),
    2493 Extruder => needs_pick(info("Магмовый экструдер", Opaque, 4.0, Pickaxe, column(t::EXTRUDER_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2494 Insolator => needs_pick(info("Теплица", Opaque, 3.0, Pickaxe, column(t::INSOLATOR_FRONT, t::INSOLATOR_TOP, t::TECH_TOP))),
    2495 Glacier => needs_pick(info("Ледогенератор", Opaque, 4.0, Pickaxe, column(t::GLACIER_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2496 MagmaGenerator => needs_pick(info("Магматический генератор", Opaque, 4.0, Pickaxe, column(t::MAGMA_GEN_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2497 ItemPipe => info("Труба для предметов", Shape, 0.8, Pickaxe, all(t::PIPE)),
    2498 PipeExtractor => info("Извлекающая труба", Shape, 0.8, Pickaxe, all(t::PIPE_EXTRACT)),
    2499 WindSail => info("Ветряная мельница", Opaque, 2.0, Axe, column(t::KIN_SAIL, t::KIN_GEARBOX, t::KIN_GEARBOX)),
    2500 ElectricMotor => needs_pick(info("Электромотор", Opaque, 3.5, Pickaxe, column(t::KIN_MOTOR, t::TECH_TOP, t::TECH_TOP))),
    2501 KineticDynamo => needs_pick(info("Кинетическое динамо", Opaque, 3.5, Pickaxe, column(t::KIN_DYNAMO, t::TECH_TOP, t::TECH_TOP))),
    2502 MechSaw => needs_pick(info("Механическая пила", Opaque, 3.0, Pickaxe, column(t::KIN_SAW, t::KIN_PRESS_TOP, t::KIN_PRESS_TOP))),
    2503 CrushingWheel => needs_pick(info("Дробильные колёса", Opaque, 3.5, Pickaxe, column(t::KIN_CRUSH, t::KIN_PRESS_TOP, t::KIN_PRESS_TOP))),
    2504 Gearbox => needs_pick(info("Редуктор", Opaque, 3.0, Pickaxe, all(t::KIN_GEARBOX))),
    2505 TeamBed => info("Кровать команды", Shape, 0.6, Hand, column(t::WOOL_RED, t::WOOL_RED, t::PLANKS)),
    2506 TeamEgg => info("Яйцо команды", Shape, 2.5, Hand, all(t::DIAMOND_BLOCK)),
    2507 Quarry => needs_pick(info("Буровая установка", Opaque, 5.0, Pickaxe, column(t::QUARRY_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2508 Recycler => needs_pick(info("Утилизатор", Opaque, 4.0, Pickaxe, column(t::RECYCLER_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2509 MatterFab => needs_pick(info("Фабрикатор материи", Opaque, 6.0, Pickaxe, column(t::FAB_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2510 ItemMagnet => needs_pick(info("Магнит предметов", Opaque, 4.0, Pickaxe, column(t::MAGNET_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2511 LunarPanel => needs_pick(info("Лунная панель", Shape, 3.0, Pickaxe, column(t::TECH_SIDE, t::LUNAR_TOP, t::TECH_SIDE))),
    2512 BigCell => needs_pick(info("Энергохранилище", Opaque, 6.0, Pickaxe, column(t::BIGCELL_SIDE, t::TECH_TOP, t::TECH_TOP))),
    2513 TeslaCoil => needs_pick(info("Тесла-катушка", Opaque, 4.0, Pickaxe, column(t::TESLA_SIDE, t::TESLA_TOP, t::TECH_TOP))),
    2514 ChargePad => needs_pick(info("Зарядная платформа", Opaque, 3.5, Pickaxe, column(t::TECH_SIDE, t::PAD_TOP, t::TECH_TOP))),
    2515 Centrifuge => needs_pick(info("Центрифуга", Opaque, 4.0, Pickaxe, column(t::CENTRIFUGE_FRONT, t::TECH_TOP, t::TECH_TOP))),
    // Hung on a wall (wall at +i, -i, +j, -j): torches lean out of it, buttons and levers sit on it.
    2516 WallTorch0 => emissive(info("Факел", Cross, 0.0, Hand, all(t::TORCH))),
    2517 WallTorch1 => emissive(info("Факел", Cross, 0.0, Hand, all(t::TORCH))),
    2518 WallTorch2 => emissive(info("Факел", Cross, 0.0, Hand, all(t::TORCH))),
    2519 WallTorch3 => emissive(info("Факел", Cross, 0.0, Hand, all(t::TORCH))),
    2520 IskritWallTorch0 => emissive(info("Искритовый факел", Cross, 0.0, Hand, all(t::ISKRIT_TORCH))),
    2521 IskritWallTorch1 => emissive(info("Искритовый факел", Cross, 0.0, Hand, all(t::ISKRIT_TORCH))),
    2522 IskritWallTorch2 => emissive(info("Искритовый факел", Cross, 0.0, Hand, all(t::ISKRIT_TORCH))),
    2523 IskritWallTorch3 => emissive(info("Искритовый факел", Cross, 0.0, Hand, all(t::ISKRIT_TORCH))),
    2524 WallButton0 => not_solid(info("Кнопка", Shape, 0.5, Hand, all(t::BUTTON))),
    2525 WallButton1 => not_solid(info("Кнопка", Shape, 0.5, Hand, all(t::BUTTON))),
    2526 WallButton2 => not_solid(info("Кнопка", Shape, 0.5, Hand, all(t::BUTTON))),
    2527 WallButton3 => not_solid(info("Кнопка", Shape, 0.5, Hand, all(t::BUTTON))),
    2528 WallButtonOn0 => not_solid(info("Кнопка (нажата)", Shape, 0.5, Hand, all(t::BUTTON))),
    2529 WallButtonOn1 => not_solid(info("Кнопка (нажата)", Shape, 0.5, Hand, all(t::BUTTON))),
    2530 WallButtonOn2 => not_solid(info("Кнопка (нажата)", Shape, 0.5, Hand, all(t::BUTTON))),
    2531 WallButtonOn3 => not_solid(info("Кнопка (нажата)", Shape, 0.5, Hand, all(t::BUTTON))),
    2532 WallSwitchOff0 => not_solid(info("Рычаг", Shape, 0.5, Hand, column(t::SWITCH_OFF, t::SWITCH_ON, t::COBBLESTONE))),
    2533 WallSwitchOff1 => not_solid(info("Рычаг", Shape, 0.5, Hand, column(t::SWITCH_OFF, t::SWITCH_ON, t::COBBLESTONE))),
    2534 WallSwitchOff2 => not_solid(info("Рычаг", Shape, 0.5, Hand, column(t::SWITCH_OFF, t::SWITCH_ON, t::COBBLESTONE))),
    2535 WallSwitchOff3 => not_solid(info("Рычаг", Shape, 0.5, Hand, column(t::SWITCH_OFF, t::SWITCH_ON, t::COBBLESTONE))),
    2536 WallSwitchOn0 => not_solid(info("Рычаг (вкл.)", Shape, 0.5, Hand, column(t::SWITCH_OFF, t::SWITCH_ON, t::COBBLESTONE))),
    2537 WallSwitchOn1 => not_solid(info("Рычаг (вкл.)", Shape, 0.5, Hand, column(t::SWITCH_OFF, t::SWITCH_ON, t::COBBLESTONE))),
    2538 WallSwitchOn2 => not_solid(info("Рычаг (вкл.)", Shape, 0.5, Hand, column(t::SWITCH_OFF, t::SWITCH_ON, t::COBBLESTONE))),
    2539 WallSwitchOn3 => not_solid(info("Рычаг (вкл.)", Shape, 0.5, Hand, column(t::SWITCH_OFF, t::SWITCH_ON, t::COBBLESTONE))),
    // Mod packs in WolfTech: Draconic Evolution, Immersive Engineering, Tinkers' Construct.
    2540 DraconiumOre => emissive(ore("Дракониевая руда", t::DRACONIUM_ORE, 3)),
    2541 DraconiumBlock => needs_pick(info("Блок дракония", Opaque, 20.0, Pickaxe, all(t::DRACONIUM_BLOCK))),
    2542 DraconicCore => emissive(needs_pick(info("Драконье энергоядро", Opaque, 12.0, Pickaxe, all(t::DRACO_CORE)))),
    2543 FusionCrafter => needs_pick(info("Термоядерный крафтер", Opaque, 8.0, Pickaxe, column(t::FUSION_FRONT, t::DRACONIUM_BLOCK, t::TECH_TOP))),
    2544 CokeOven => needs_pick(info("Коксовая печь", Opaque, 4.0, Pickaxe, column(t::COKE_FRONT, t::BRICKS, t::BRICKS))),
    2545 BlastFurnace => needs_pick(info("Доменная печь", Opaque, 5.0, Pickaxe, column(t::BLAST_FRONT, t::BRICKS, t::BRICKS))),
    2546 ArcFurnace => needs_pick(info("Дуговая печь", Opaque, 5.0, Pickaxe, column(t::ARC_FRONT, t::TECH_TOP, t::TECH_TOP))),
    2547 Smeltery => needs_pick(info("Плавильня", Opaque, 5.0, Pickaxe, column(t::SMELTERY_FRONT, t::SEARED_BRICKS, t::SEARED_BRICKS))),
    2548 SearedBricks => needs_pick(info("Обожжённый кирпич", Opaque, 4.0, Pickaxe, all(t::SEARED_BRICKS))),
}

/// An iskrit mechanism: a piston (base, possibly sticky and extended), a piston head, a
/// repeater or a comparator, with its direction (0 +i, 1 -i, 2 +j, 3 -j, 4 up, 5 down;
/// repeaters and comparators only 0..3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Piston { sticky: bool, extended: bool, dir: u8 },
    Head { sticky: bool, dir: u8 },
    Repeater { on: bool, dir: u8 },
    Comparator { on: bool, dir: u8 },
}

impl Part {
    pub fn dir(self) -> u8 {
        match self {
            Part::Piston { dir, .. } | Part::Head { dir, .. } | Part::Repeater { dir, .. } | Part::Comparator { dir, .. } => dir,
        }
    }
    /// The block for this state.
    pub fn block(self) -> Block {
        let id = match self {
            Part::Piston { sticky, extended, dir } => 200 + (sticky as u8) * 12 + (extended as u8) * 6 + dir.min(5),
            // One head for both kinds of piston: stickiness lives in the base behind it.
            Part::Head { dir, .. } => 224 + dir.min(5),
            Part::Repeater { on, dir } => 236 + (on as u8) * 4 + dir.min(3),
            Part::Comparator { on, dir } => 244 + (on as u8) * 4 + dir.min(3),
        };
        Block::from_u8(id)
    }
}

/// Liquid state of a block: lava or water, flow level (1 = thinnest, `max_level` = beside a
/// source), source block, falling column.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fluid {
    pub lava: bool,
    pub level: u8,
    pub source: bool,
    pub falling: bool,
}

impl Fluid {
    pub fn max_level(lava: bool) -> u8 {
        if lava { 3 } else { 7 }
    }
    /// Height of the liquid surface inside the cell (0..1).
    pub fn height(self) -> f32 {
        if self.source || self.falling {
            return 1.0;
        }
        let max = Self::max_level(self.lava) as f32;
        (self.level as f32 / max) * 0.78 + 0.1
    }
    /// The block for this state.
    pub fn block(self) -> Block {
        use Block::*;
        match (self.lava, self.source, self.falling) {
            (false, true, _) => Water,
            (true, true, _) => Lava,
            (false, _, true) => WaterFall,
            (true, _, true) => LavaFall,
            (false, ..) => [WaterFlow1, WaterFlow2, WaterFlow3, WaterFlow4, WaterFlow5, WaterFlow6, WaterFlow7][(self.level.clamp(1, 7) - 1) as usize],
            (true, ..) => [LavaFlow1, LavaFlow2, LavaFlow3][(self.level.clamp(1, 3) - 1) as usize],
        }
    }
}

impl Block {
    pub fn base_kind(self)->Block { crate::construction::legacy_kind(self) }
    #[inline]
    pub fn info(self) -> &'static BlockInfo {
        &BLOCK_INFO[if (self as u16) < 256 { self as usize } else { 256 + self as usize - 1024 }]
    }
    #[inline]
    pub fn from_u8(v: u8) -> Block { Self::from_id(v as u16) }
    #[inline]
    pub fn from_id(v: u16) -> Block {
        let i = if v < 256 { v as usize } else if v >= 1024 { 256 + v as usize - 1024 } else { return Self::Air };
        Block::ALL.get(i).copied().unwrap_or(Block::Air)
    }
    #[inline]
    pub fn is_air(self) -> bool {
        self == Block::Air
    }
    /// Box of a `Shape` block inside its cell: `[i0, j0, k0, i1, j1, k1]` in 0..1
    /// (i and j run along the ground, k is up). `None` for full cubes. For blocks made of
    /// several boxes (stairs, signs) this is the first one; see [`Block::boxes`].
    pub fn shape(self) -> Option<[f32; 6]> {
        self.boxes().first().copied()
    }

    /// A block hung on a wall: its floor block and the wall side (0 +i, 1 -i, 2 +j, 3 -j).
    pub fn wall_mount(self) -> Option<(Block, u8)> {
        let id = self as u16;
        let base = match id {
            2516..=2519 => Block::Torch,
            2520..=2523 => Block::IskritTorch,
            2524..=2527 => Block::Button,
            2528..=2531 => Block::ButtonOn,
            2532..=2535 => Block::SwitchOff,
            2536..=2539 => Block::SwitchOn,
            _ => return None,
        };
        Some((base, ((id - 2516) % 4) as u8))
    }

    /// The floor block itself (a wall torch is a torch, a wall lever a lever).
    pub fn unwall(self) -> Block {
        self.wall_mount().map_or(self, |m| m.0)
    }

    /// This floor block hung on the wall at side `dir` (as in `wall_mount`); itself when it
    /// can't hang.
    pub fn on_wall(self, dir: u8) -> Block {
        let start = match self {
            Block::Torch => 2516,
            Block::IskritTorch => 2520,
            Block::Button => 2524,
            Block::ButtonOn => 2528,
            Block::SwitchOff => 2532,
            Block::SwitchOn => 2536,
            _ => return self,
        };
        Block::from_id(start + (dir % 4) as u16)
    }

    /// Same kind of mount (floor or wall side) for another floor block: a pressed wall button
    /// stays on its wall.
    pub fn same_mount(self, floor: Block) -> Block {
        match self.wall_mount() {
            Some((_, d)) => floor.on_wall(d),
            None => floor,
        }
    }

    /// A wall torch leans out of its wall: maps a point of the standing torch (i, j, k = up)
    /// to where it is drawn. The foot touches the wall a little above the cell's bottom.
    pub fn wall_torch_point(dir: u8, p: [f32; 3]) -> [f32; 3] {
        let (axis, s) = match dir % 4 {
            0 => (0, 1.0f32),
            1 => (0, -1.0),
            2 => (1, 1.0),
            _ => (1, -1.0),
        };
        let mut q = p;
        let h = p[2];
        q[2] = h * 0.92 + 0.22;
        // Toward the wall: the foot sits at the wall, the top 0.24 out of it.
        let u = (p[axis] - 0.5) * s;
        q[axis] = 0.5 + s * (u + 0.4375 - h * 0.38);
        q
    }

    /// 3D model of a torch, boxes as in `boxes` (i, j, k = up): the stick and, for the iskrit
    /// torch, its glowing head. They match the torch pictures (stick in columns 7–8).
    pub fn torch_boxes(self) -> Option<&'static [[f32; 6]]> {
        const P: f32 = 1.0 / 16.0;
        match self.unwall() {
            Block::Torch => Some(&[[7.0 * P, 7.0 * P, 0.0, 9.0 * P, 9.0 * P, 10.0 * P]]),
            Block::IskritTorch => Some(&[[7.0 * P, 7.0 * P, 0.0, 9.0 * P, 9.0 * P, 7.0 * P], [5.0 * P, 5.0 * P, 6.0 * P, 11.0 * P, 11.0 * P, 12.0 * P]]),
            _ => None,
        }
    }

    /// The selection outline of a block in its cell (i, j, k = up): the union of what is drawn.
    pub fn outline_box(self) -> [f32; 6] {
        let union = |list: &[[f32; 6]]| list.iter().fold([1.0f32, 1.0, 1.0, 0.0, 0.0, 0.0], |a, s| [a[0].min(s[0]), a[1].min(s[1]), a[2].min(s[2]), a[3].max(s[3]), a[4].max(s[4]), a[5].max(s[5])]);
        if let Some((e, r)) = crate::construction::entry(self as u16) {
            // Construction pieces: around everything drawn (an arch or a wardrobe is taller
            // than one cell, a bed longer), not just the part that collides.
            let g = crate::construction::geometry::model(e.form, r, 0);
            let mut o = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
            for p in g.faces.iter().flat_map(|q| q.points.iter()) {
                for k in 0..3 {
                    o[k] = o[k].min(p[k]);
                    o[k + 3] = o[k + 3].max(p[k]);
                }
            }
            if o[0] <= o[3] { o } else { union(self.boxes()) }
        } else if let Some(t) = self.torch_boxes() {
            match self.wall_mount() {
                // The leaning stick: around its moved corners.
                Some((_, d)) => {
                    let mut o = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
                    for b in t {
                        for n in 0..8 {
                            let p = [b[if n & 1 == 0 { 0 } else { 3 }], b[if n & 2 == 0 { 1 } else { 4 }], b[if n & 4 == 0 { 2 } else { 5 }]];
                            let q = Self::wall_torch_point(d, p);
                            for k in 0..3 {
                                o[k] = o[k].min(q[k]);
                                o[k + 3] = o[k + 3].max(q[k]);
                            }
                        }
                    }
                    o
                }
                None => union(t),
            }
        } else if !self.boxes().is_empty() {
            union(self.boxes())
        } else if self.info().render == RenderKind::Cross {
            [0.15, 0.15, 0.0, 0.85, 0.85, 0.85]
        } else {
            [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]
        }
    }

    /// All boxes of a `Shape` block (empty for full cubes).
    pub fn boxes(self) -> &'static [[f32; 6]] {
        if let Some((e, r)) = crate::construction::entry(self as u16) { return &crate::construction::geometry::model(e.form, r, 0).boxes; }
        if let Some((base, d)) = self.wall_mount() {
            return wall_boxes(base, d);
        }
        use Block::*;
        const SLAB: [f32; 6] = [0.0, 0.0, 0.0, 1.0, 1.0, 0.5];
        const VERT_A: [f32; 6] = [0.0, 0.0, 0.0, 0.5, 1.0, 1.0];
        const VERT_B: [f32; 6] = [0.0, 0.0, 0.0, 1.0, 0.5, 1.0];
        const PANE_A: [f32; 6] = [0.44, 0.0, 0.0, 0.56, 1.0, 1.0];
        const PANE_B: [f32; 6] = [0.0, 0.44, 0.0, 1.0, 0.56, 1.0];
        const FENCE: [f32; 6] = [0.375, 0.375, 0.0, 0.625, 0.625, 1.0];
        const WALL: [f32; 6] = [0.25, 0.25, 0.0, 0.75, 0.75, 1.0];
        const DOOR: [f32; 6] = [0.0, 0.0, 0.0, 0.19, 1.0, 1.0];
        const DOOR_OPEN: [f32; 6] = [0.0, 0.0, 0.0, 1.0, 0.19, 1.0];
        const WIRE: [f32; 6] = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0625];
        const CABLE: [f32; 6] = [0.34, 0.34, 0.34, 0.66, 0.66, 0.66];
        const PIPE: [f32; 6] = [0.30, 0.30, 0.30, 0.70, 0.70, 0.70];
        const SOLAR: [f32; 6] = [0.0, 0.0, 0.0, 1.0, 1.0, 0.375];
        const BUTTON: [f32; 6] = [0.32, 0.32, 0.0, 0.68, 0.68, 0.14];
        // A heap of bones: two long bones crossed, a skull and a small piece.
        const BONES: [[f32; 6]; 4] = [
            [0.08, 0.40, 0.0, 0.84, 0.52, 0.11],
            [0.32, 0.08, 0.0, 0.44, 0.82, 0.10],
            [0.54, 0.54, 0.0, 0.88, 0.88, 0.30],
            [0.14, 0.62, 0.0, 0.30, 0.80, 0.08],
        ];
        // Repeater / comparator: a thin slab like the dust, a little thicker.
        const GATE: [f32; 6] = [0.0, 0.0, 0.0, 1.0, 1.0, 0.125];
        // Piston heads per direction: a plate on the far side and a rod back to the base.
        const HEADS: [[[f32; 6]; 2]; 6] = [
            [[0.75, 0.0, 0.0, 1.0, 1.0, 1.0], [0.0, 0.375, 0.375, 0.75, 0.625, 0.625]],
            [[0.0, 0.0, 0.0, 0.25, 1.0, 1.0], [0.25, 0.375, 0.375, 1.0, 0.625, 0.625]],
            [[0.0, 0.75, 0.0, 1.0, 1.0, 1.0], [0.375, 0.0, 0.375, 0.625, 0.75, 0.625]],
            [[0.0, 0.0, 0.0, 1.0, 0.25, 1.0], [0.375, 0.25, 0.375, 0.625, 1.0, 0.625]],
            [[0.0, 0.0, 0.75, 1.0, 1.0, 1.0], [0.375, 0.375, 0.0, 0.625, 0.625, 0.75]],
            [[0.0, 0.0, 0.0, 1.0, 1.0, 0.25], [0.375, 0.375, 0.25, 0.625, 0.625, 1.0]],
        ];
        const BUTTON_ON: [f32; 6] = [0.32, 0.32, 0.0, 0.68, 0.68, 0.06];
        // Lever like the classic one: a cobblestone base and a wooden handle that leans one
        // way when off and the other when on (a little staircase of boxes makes the slope).
        const LEVER_OFF: [[f32; 6]; 5] = [
            [0.30, 0.22, 0.0, 0.70, 0.78, 0.19],
            [0.44, 0.48, 0.19, 0.56, 0.60, 0.33],
            [0.44, 0.55, 0.33, 0.56, 0.67, 0.46],
            [0.44, 0.62, 0.46, 0.56, 0.74, 0.59],
            [0.43, 0.68, 0.59, 0.57, 0.82, 0.73],
        ];
        const LEVER_ON: [[f32; 6]; 5] = [
            [0.30, 0.22, 0.0, 0.70, 0.78, 0.19],
            [0.44, 0.40, 0.19, 0.56, 0.52, 0.33],
            [0.44, 0.33, 0.33, 0.56, 0.45, 0.46],
            [0.44, 0.26, 0.46, 0.56, 0.38, 0.59],
            [0.43, 0.18, 0.59, 0.57, 0.32, 0.73],
        ];
        const PLATE: [f32; 6] = [0.06, 0.06, 0.0, 0.94, 0.94, 0.0625];
        const PLATE_ON: [f32; 6] = [0.06, 0.06, 0.0, 0.94, 0.94, 0.03];
        // Stairs: the high half is toward +i, -i, +j or -j.
        const ST0: [[f32; 6]; 2] = [SLAB, [0.5, 0.0, 0.5, 1.0, 1.0, 1.0]];
        const ST1: [[f32; 6]; 2] = [SLAB, [0.0, 0.0, 0.5, 0.5, 1.0, 1.0]];
        const ST2: [[f32; 6]; 2] = [SLAB, [0.0, 0.5, 0.5, 1.0, 1.0, 1.0]];
        const ST3: [[f32; 6]; 2] = [SLAB, [0.0, 0.0, 0.5, 1.0, 0.5, 1.0]];
        // Sign: post and board.
        const SIGN_A: [[f32; 6]; 2] = [[0.46, 0.46, 0.0, 0.54, 0.54, 0.55], [0.05, 0.44, 0.5, 0.95, 0.56, 0.95]];
        const SIGN_B: [[f32; 6]; 2] = [[0.46, 0.46, 0.0, 0.54, 0.54, 0.55], [0.44, 0.05, 0.5, 0.56, 0.95, 0.95]];
        match self {
            StoneSlab | PlankSlab | CobbleSlab | StoneBrickSlab | SandstoneSlab | BrickSlab | BirchSlab | SpruceSlab => std::slice::from_ref(&SLAB),
            StoneSlabVerticalA | PlankSlabVerticalA | BrickSlabVerticalA => std::slice::from_ref(&VERT_A),
            StoneSlabVerticalB | PlankSlabVerticalB | BrickSlabVerticalB => std::slice::from_ref(&VERT_B),
            GlassPaneA | RedPaneA | BluePaneA | GreenPaneA | YellowPaneA => std::slice::from_ref(&PANE_A),
            GlassPaneB | RedPaneB | BluePaneB | GreenPaneB | YellowPaneB => std::slice::from_ref(&PANE_B),
            Fence | BirchFence | SpruceFence => std::slice::from_ref(&FENCE),
            StoneWall | StoneBrickWall | SandstoneWall => std::slice::from_ref(&WALL),
            Door | BirchDoor | SpruceDoor | IronDoor | DoorTop | BirchDoorTop | SpruceDoorTop | IronDoorTop => std::slice::from_ref(&DOOR),
            DoorOpen | BirchDoorOpen | SpruceDoorOpen | IronDoorOpen | DoorTopOpen | BirchDoorTopOpen | SpruceDoorTopOpen | IronDoorTopOpen => std::slice::from_ref(&DOOR_OPEN),
            CobbleStairs | PlankStairs | StoneBrickStairs | SandstoneStairs => &ST0,
            CobbleStairs1 | PlankStairs1 | StoneBrickStairs1 | SandstoneStairs1 => &ST1,
            CobbleStairs2 | PlankStairs2 | StoneBrickStairs2 | SandstoneStairs2 => &ST2,
            CobbleStairs3 | PlankStairs3 | StoneBrickStairs3 | SandstoneStairs3 => &ST3,
            Sign => &SIGN_A,
            SignB => &SIGN_B,
            IskritWire | IskritWireOn => std::slice::from_ref(&WIRE),
            b if b.part().is_some() => match b.part() {
                Some(Part::Head { dir, .. }) => &HEADS[dir as usize % 6],
                Some(Part::Repeater { .. } | Part::Comparator { .. }) => std::slice::from_ref(&GATE),
                _ => &[],
            },
            Button => std::slice::from_ref(&BUTTON),
            Bones => &BONES,
            Cable => &[CABLE],
            ItemPipe | PipeExtractor => &[PIPE],
            TeamBed => &[[0.0, 0.0, 0.0, 1.0, 0.56, 1.0]],
            TeamEgg => &[[0.25, 0.0, 0.25, 0.75, 0.85, 0.75]],
            // «Механика»: thin shafts, cog and wheel discs, a crank on top of a block.
            Shaft => &[[0.0, 0.375, 0.375, 1.0, 0.625, 0.625]],
            ShaftJ => &[[0.375, 0.0, 0.375, 0.625, 1.0, 0.625]],
            ShaftK => &[[0.375, 0.375, 0.0, 0.625, 0.625, 1.0]],
            Cogwheel => &[[0.375, 0.06, 0.06, 0.625, 0.94, 0.94]],
            CogwheelJ => &[[0.06, 0.375, 0.06, 0.94, 0.625, 0.94]],
            CogwheelK => &[[0.06, 0.06, 0.375, 0.94, 0.94, 0.625]],
            WaterWheel => &[[0.25, 0.0, 0.0, 0.75, 1.0, 1.0]],
            WaterWheelJ => &[[0.0, 0.25, 0.0, 1.0, 0.75, 1.0]],
            HandCrank => &[[0.3, 0.3, 0.0, 0.7, 0.7, 0.6]],
            SolarPanel | LunarPanel => &[SOLAR],
            ButtonOn => std::slice::from_ref(&BUTTON_ON),
            SwitchOff => &LEVER_OFF,
            SwitchOn => &LEVER_ON,
            PressurePlate => std::slice::from_ref(&PLATE),
            PressurePlateOn => std::slice::from_ref(&PLATE_ON),
            _ => &[],
        }
    }

    /// Block placed for this item, turned to face `dir_i`/`dir_j` (how much the player looks
    /// along the cell's i and j axes): stairs rise away from the player, panes, doors,
    /// vertical slabs and signs stand across the view.
    pub fn oriented(self, dir_i: f64, dir_j: f64) -> Block {
        use crate::construction::{self,Form};
        // Our doors, crafting table and furnace stay our own blocks (own textures, double doors
        // that part like in the classic game); only these use the catalogue models.
        let legacy = match self { Self::Chest=>Some((Form::StorageChest,0)),Self::Terminal=>Some((Form::Terminal,6)),Self::CommandBlock=>Some((Form::Command,6)),_=>None };
        if let Some((f,m))=legacy {return construction::block(f,m,0).unwrap().oriented(dir_i,dir_j);}
        if let Some((e, _)) = crate::construction::entry(self as u16) {
            let r = if dir_i.abs() >= dir_j.abs() { if dir_i > 0.0 { 3 } else { 1 } } else if dir_j > 0.0 { 0 } else { 2 };
            return Block::from_id(e.id + r);
        }
        use Block::*;
        let along_i = dir_i.abs() >= dir_j.abs();
        let stairs = |base: Block| -> Block {
            let k = if along_i { if dir_i > 0.0 { 0 } else { 1 } } else if dir_j > 0.0 { 2 } else { 3 };
            Block::from_u8(base as u8 + k)
        };
        let ab = |a: Block, b: Block| if along_i { a } else { b };
        // Repeaters and comparators send the signal the way the player looks.
        if let Some(Part::Repeater { on, .. } | Part::Comparator { on, .. }) = self.part() {
            let dir = if along_i { if dir_i > 0.0 { 0 } else { 1 } } else if dir_j > 0.0 { 2 } else { 3 };
            return match self.part() {
                Some(Part::Repeater { .. }) => Part::Repeater { on, dir }.block(),
                _ => Part::Comparator { on, dir }.block(),
            };
        }
        match self {
            CobbleStairs => stairs(CobbleStairs),
            PlankStairs => stairs(PlankStairs),
            StoneBrickStairs => stairs(StoneBrickStairs),
            SandstoneStairs => stairs(SandstoneStairs),
            GlassPaneA => ab(GlassPaneA, GlassPaneB),
            RedPaneA => ab(RedPaneA, RedPaneB),
            BluePaneA => ab(BluePaneA, BluePaneB),
            GreenPaneA => ab(GreenPaneA, GreenPaneB),
            YellowPaneA => ab(YellowPaneA, YellowPaneB),
            Door => ab(Door, DoorOpen),
            BirchDoor => ab(BirchDoor, BirchDoorOpen),
            SpruceDoor => ab(SpruceDoor, SpruceDoorOpen),
            IronDoor => ab(IronDoor, IronDoorOpen),
            Sign => ab(SignB, Sign),
            StoneSlabVerticalA => ab(StoneSlabVerticalA, StoneSlabVerticalB),
            PlankSlabVerticalA => ab(PlankSlabVerticalA, PlankSlabVerticalB),
            BrickSlabVerticalA => ab(BrickSlabVerticalA, BrickSlabVerticalB),
            // «Механика»: shafts, cogs and wheels lie along the way the player looks.
            Shaft | ShaftJ | ShaftK | Cogwheel | CogwheelJ | CogwheelK | WaterWheel | WaterWheelJ => crate::kinetic::with_axis(self, if along_i { 0 } else { 1 }),
            // WolfTech machines turn their front to the player (like a furnace).
            TechGenerator | Crusher | InductionFurnace | Enricher => {
                let d = if along_i { if dir_i > 0.0 { 1 } else { 0 } } else if dir_j > 0.0 { 3 } else { 2 };
                self.machine_facing(d)
            }
            b => b,
        }
    }

    /// WolfTech machine turned to face `d` (0 +i, 1 -i, 2 +j, 3 -j); `self` is the base block.
    pub fn machine_facing(self, d: usize) -> Block {
        if d == 0 {
            return self;
        }
        use Block::*;
        let v = match self {
            TechGenerator => [TechGeneratorD1, TechGeneratorD2, TechGeneratorD3],
            Crusher => [CrusherD1, CrusherD2, CrusherD3],
            InductionFurnace => [InductionFurnaceD1, InductionFurnaceD2, InductionFurnaceD3],
            Enricher => [EnricherD1, EnricherD2, EnricherD3],
            b => return b,
        };
        v[(d - 1).min(2)]
    }

    /// (base machine, facing) of a WolfTech machine with a front.
    pub fn machine_dir(self) -> Option<(Block, usize)> {
        use Block::*;
        Some(match self {
            TechGenerator => (TechGenerator, 0),
            TechGeneratorD1 => (TechGenerator, 1),
            TechGeneratorD2 => (TechGenerator, 2),
            TechGeneratorD3 => (TechGenerator, 3),
            Crusher => (Crusher, 0),
            CrusherD1 => (Crusher, 1),
            CrusherD2 => (Crusher, 2),
            CrusherD3 => (Crusher, 3),
            InductionFurnace => (InductionFurnace, 0),
            InductionFurnaceD1 => (InductionFurnace, 1),
            InductionFurnaceD2 => (InductionFurnace, 2),
            InductionFurnaceD3 => (InductionFurnace, 3),
            Enricher => (Enricher, 0),
            EnricherD1 => (Enricher, 1),
            EnricherD2 => (Enricher, 2),
            EnricherD3 => (Enricher, 3),
            _ => return None,
        })
    }

    /// The item a placed variant comes from (open doors, turned stairs and panes, ...).
    pub fn base_item(self) -> Block {
        if let Some((e, _)) = crate::construction::entry(self as u16) { return Block::from_id(e.base); }
        if let Some((b, _)) = self.wall_mount() {
            return match b {
                Block::ButtonOn => Block::Button,
                Block::SwitchOn => Block::SwitchOff,
                b => b,
            };
        }
        use Block::*;
        match self {
            b if b.part().is_some() => match b.part() {
                Some(Part::Piston { sticky, .. }) => Part::Piston { sticky, extended: false, dir: 2 }.block(),
                Some(Part::Head { sticky, .. }) => Part::Piston { sticky, extended: false, dir: 2 }.block(),
                Some(Part::Repeater { .. }) => Part::Repeater { on: false, dir: 0 }.block(),
                Some(Part::Comparator { .. }) => Part::Comparator { on: false, dir: 0 }.block(),
                None => b,
            },
            WaterFlow1 | WaterFlow2 | WaterFlow3 | WaterFlow4 | WaterFlow5 | WaterFlow6 | WaterFlow7 | WaterFall => Water,
            LavaFlow1 | LavaFlow2 | LavaFlow3 | LavaFall => Lava,
            DoorOpen | DoorTop | DoorTopOpen => Door,
            BirchDoorOpen | BirchDoorTop | BirchDoorTopOpen => BirchDoor,
            SpruceDoorOpen | SpruceDoorTop | SpruceDoorTopOpen => SpruceDoor,
            IronDoorOpen | IronDoorTop | IronDoorTopOpen => IronDoor,
            CobbleStairs1 | CobbleStairs2 | CobbleStairs3 => CobbleStairs,
            PlankStairs1 | PlankStairs2 | PlankStairs3 => PlankStairs,
            StoneBrickStairs1 | StoneBrickStairs2 | StoneBrickStairs3 => StoneBrickStairs,
            SandstoneStairs1 | SandstoneStairs2 | SandstoneStairs3 => SandstoneStairs,
            GlassPaneB => GlassPaneA,
            RedPaneB => RedPaneA,
            BluePaneB => BluePaneA,
            GreenPaneB => GreenPaneA,
            YellowPaneB => YellowPaneA,
            SignB => Sign,
            StoneSlabVerticalB => StoneSlabVerticalA,
            PlankSlabVerticalB => PlankSlabVerticalA,
            BrickSlabVerticalB => BrickSlabVerticalA,
            IskritWireOn => IskritWire,
            IskritLampOn => IskritLamp,
            ButtonOn => Button,
            PressurePlateOn => PressurePlate,
            b if b.machine_dir().is_some() => b.machine_dir().unwrap().0,
            ShaftJ | ShaftK => Shaft,
            CogwheelJ | CogwheelK => Cogwheel,
            WaterWheelJ => WaterWheel,
            b => b,
        }
    }

    /// Door states: (closed, open) pairs toggle on use.
    /// Fences (1) and walls (2) join their neighbours with rails.
    pub fn post_family(self) -> Option<u8> {
        use Block::*;
        match self {
            Fence | BirchFence | SpruceFence => Some(1),
            StoneWall | StoneBrickWall | SandstoneWall => Some(2),
            _ => None,
        }
    }

    /// Does a fence or wall reach out to `n`? Same family, or a full solid block.
    pub fn post_joins(self, n: Block) -> bool {
        let Some(f) = self.post_family() else { return false };
        n.post_family() == Some(f) || (n.info().render == RenderKind::Opaque && n.is_solid())
    }

    /// The rails from a fence or wall post towards the neighbour along `axis` (0 i, 1 j) on
    /// side `sign`: two thin rails for a fence, one thick low wall for walls.
    pub fn post_arms(self, axis: usize, sign: i32) -> Vec<[f32; 6]> {
        let (lo, hi) = if sign > 0 { (0.5, 1.0) } else { (0.0, 0.5) };
        let bar = |w0: f32, w1: f32, k0: f32, k1: f32| if axis == 0 { [lo, w0, k0, hi, w1, k1] } else { [w0, lo, k0, w1, hi, k1] };
        match self.post_family() {
            Some(1) => vec![bar(0.4375, 0.5625, 0.38, 0.56), bar(0.4375, 0.5625, 0.72, 0.9)],
            Some(2) => vec![bar(0.3125, 0.6875, 0.0, 0.8125)],
            _ => vec![],
        }
    }

    /// A crop and whether it is ripe.
    pub fn crop(self) -> Option<(Block, bool)> {
        use Block::*;
        Some(match self {
            WheatYoung => (Wheat, false),
            Wheat => (Wheat, true),
            CarrotYoung => (Carrots, false),
            Carrots => (Carrots, true),
            PotatoYoung => (Potatoes, false),
            Potatoes => (Potatoes, true),
            _ => return None,
        })
    }

    /// The next growth stage of a young crop.
    pub fn grown(self) -> Option<Block> {
        self.crop().filter(|c| !c.1).map(|c| c.0)
    }

    pub fn door_toggle(self) -> Option<Block> {
        if self as u16 >= 1024 { return crate::construction::door_toggle(self); }
        use Block::*;
        Some(match self {
            Door => DoorOpen,
            DoorOpen => Door,
            BirchDoor => BirchDoorOpen,
            BirchDoorOpen => BirchDoor,
            SpruceDoor => SpruceDoorOpen,
            SpruceDoorOpen => SpruceDoor,
            IronDoor => IronDoorOpen,
            IronDoorOpen => IronDoor,
            DoorTop => DoorTopOpen,
            DoorTopOpen => DoorTop,
            BirchDoorTop => BirchDoorTopOpen,
            BirchDoorTopOpen => BirchDoorTop,
            SpruceDoorTop => SpruceDoorTopOpen,
            SpruceDoorTopOpen => SpruceDoorTop,
            IronDoorTop => IronDoorTopOpen,
            IronDoorTopOpen => IronDoorTop,
            _ => return None,
        })
    }

    /// Iskrit mechanism this block is, if any.
    pub fn part(self) -> Option<Part> {
        if self as u16 >= 256 { return None; }
        let id = self as u8;
        Some(match id {
            200..=205 => Part::Piston { sticky: false, extended: false, dir: id - 200 },
            206..=211 => Part::Piston { sticky: false, extended: true, dir: id - 206 },
            212..=217 => Part::Piston { sticky: true, extended: false, dir: id - 212 },
            218..=223 => Part::Piston { sticky: true, extended: true, dir: id - 218 },
            224..=229 => Part::Head { sticky: false, dir: id - 224 },
            236..=239 => Part::Repeater { on: false, dir: id - 236 },
            240..=243 => Part::Repeater { on: true, dir: id - 240 },
            244..=247 => Part::Comparator { on: false, dir: id - 244 },
            248..=251 => Part::Comparator { on: true, dir: id - 248 },
            _ => return None,
        })
    }

    /// Door leaf moved to the far side of its cell, and the axis (0 i, 1 j) along which the
    /// neighbour that decides it lies (at -axis). Double doors: the second leaf of a pair
    /// hangs on the other jamb, so both swing out of the doorway like in Minecraft.
    pub fn door_mirror(self) -> Option<(usize, [f32; 6])> {
        if self as u16 >= 1024 { return None; }
        self.door_toggle()?;
        let b = *self.boxes().first()?;
        if b[3] < 0.5 {
            Some((0, [1.0 - b[3], b[1], b[2], 1.0, b[4], b[5]]))
        } else if b[4] < 0.5 {
            Some((1, [b[0], 1.0 - b[4], b[2], b[3], 1.0, b[5]]))
        } else {
            None
        }
    }

    /// True when `neighbor` (the cell at -axis of `door_mirror`) is the other leaf of a
    /// double door: a door leaf of the same shape.
    pub fn door_mirrored_by(self, neighbor: Block) -> bool {
        neighbor.door_toggle().is_some() && neighbor.boxes() == self.boxes()
    }

    /// Upper half that goes above this lower door half (same open state).
    pub fn door_top(self) -> Option<Block> {
        if self as u16 >= 1024 { return crate::construction::door_top(self); }
        use Block::*;
        Some(match self {
            Door => DoorTop,
            DoorOpen => DoorTopOpen,
            BirchDoor => BirchDoorTop,
            BirchDoorOpen => BirchDoorTopOpen,
            SpruceDoor => SpruceDoorTop,
            SpruceDoorOpen => SpruceDoorTopOpen,
            IronDoor => IronDoorTop,
            IronDoorOpen => IronDoorTopOpen,
            _ => return None,
        })
    }

    /// True for the upper half of a door.
    pub fn is_door_top(self) -> bool {
        if self as u16 >= 1024 { return crate::construction::is_door_top(self); }
        use Block::*;
        matches!(self, DoorTop | DoorTopOpen | BirchDoorTop | BirchDoorTopOpen | SpruceDoorTop | SpruceDoorTopOpen | IronDoorTop | IronDoorTopOpen)
    }

    /// Radioactive (uranium ore and blocks).
    pub fn is_radioactive(self) -> bool {
        matches!(self, Block::UraniumOre | Block::UraniumBlock)
    }

    /// Collides with the player.
    #[inline]
    pub fn is_solid(self) -> bool {
        self.info().solid
    }
    /// Block light it gives off (0..15), like Minecraft: a torch lights 13 blocks around it.
    #[inline]
    pub fn light_emission(self) -> u8 {
        if let Some((b, _)) = self.wall_mount() {
            return b.light_emission();
        }
        if crate::construction::entry(self as u16).is_some_and(|(e,_)| matches!(e.form, crate::construction::Form::FloorLamp|crate::construction::Form::SharedChest|crate::construction::Form::StoneAltar|crate::construction::Form::RootAltar|crate::construction::Form::EmberAltar)) { return 15; }
        use Block::*;
        match self {
            Lava | LavaFall | Glowstone | Lamp | Lantern | ElectricLampOn | IskritLampOn | InnerCore => 15,
            LavaFlow1 | LavaFlow2 | LavaFlow3 => 13,
            Torch | Fire => 14,
            UraniumBlock => 10,
            UraniumOre => 6,
            NetherPortal => 11,
            IskritTorch => 7,
            DraconicCore => 12,
            DraconiumOre => 5,
            Magma => 4,
            _ => 0,
        }
    }

    /// Fully hides neighbouring faces.
    #[inline]
    pub fn is_opaque(self) -> bool {
        matches!(self.info().render, RenderKind::Opaque)
    }
    /// Blocks skylight.
    #[inline]
    pub fn blocks_light(self) -> bool {
        matches!(self.info().render, RenderKind::Opaque | RenderKind::Cutout) && self != Block::Glass
    }
    #[inline]
    pub fn occludes_ao(self) -> bool {
        self.is_opaque() || self == Block::OakLeaves
    }
    #[inline]
    pub fn is_replaceable(self) -> bool {
        matches!(self, Block::Air | Block::TallGrass | Block::DeadBush) || self.is_fluid() || self.info().render == RenderKind::Cross
    }
    #[inline]
    pub fn is_fluid(self) -> bool {
        self.fluid().is_some()
    }
    #[inline]
    /// Catches fire: wood (logs, planks, wooden furniture), leaves, plants, wool and the like.
    pub fn flammable(self) -> bool {
        let i = self.info();
        if self == Block::Fire || i.hardness < 0.0 {
            return false;
        }
        i.tool == ToolKind::Axe || i.name.contains("листв") || i.name.contains("хвоя") || i.name.contains("Шерсть") || i.name.contains("шерсть") || (i.render == RenderKind::Cross && !i.emissive)
    }

    pub fn is_water(self) -> bool {
        self.fluid().is_some_and(|f| !f.lava)
    }
    #[inline]
    pub fn is_lava(self) -> bool {
        self.fluid().is_some_and(|f| f.lava)
    }
    /// Liquid state, `None` for everything that is not water or lava.
    pub fn fluid(self) -> Option<Fluid> {
        use Block::*;
        let f = |lava: bool, level: u8, source: bool, falling: bool| Some(Fluid { lava, level, source, falling });
        match self {
            Water => f(false, 8, true, false),
            Lava => f(true, 4, true, false),
            WaterFall => f(false, 7, false, true),
            LavaFall => f(true, 3, false, true),
            WaterFlow1 | WaterFlow2 | WaterFlow3 | WaterFlow4 | WaterFlow5 | WaterFlow6 | WaterFlow7 => f(false, self as u8 - WaterFlow1 as u8 + 1, false, false),
            LavaFlow1 | LavaFlow2 | LavaFlow3 => f(true, self as u8 - LavaFlow1 as u8 + 1, false, false),
            _ => None,
        }
    }
    #[inline]
    pub fn is_breakable(self) -> bool {
        self.info().hardness >= 0.0
    }
    pub fn name(self) -> &'static str {
        self.info().name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_roundtrip() {
        for (i, b) in Block::ALL.iter().enumerate() {
            assert_eq!(*b as usize, if i < 256 { i } else { 1024 + i - 256 });
            assert_eq!(Block::from_id(*b as u16), *b);
        }
        assert_eq!(BLOCK_INFO.len(), Block::ALL.len());
    }

    #[test]
    fn properties() {
        assert!(Block::Stone.is_solid() && Block::Stone.is_opaque());
        assert!(!Block::Water.is_solid() && !Block::Lava.is_solid());
        assert!(!Block::TallGrass.is_solid());
        assert!(Block::OakLeaves.is_solid() && !Block::OakLeaves.is_opaque());
        assert!(Block::Ice.is_solid());
        // Every id is a block of its own (all 256 are in use), declared in id order.
        for id in 0..=255u8 {
            assert_eq!(Block::from_u8(id) as u8, id, "id {id}");
        }
        assert_eq!(Block::WheatYoung.grown(), Some(Block::Wheat));
        assert_eq!(Block::Wheat.grown(), None);
    }

    #[test]
    fn tiles_exist_in_atlas() {
        let n = wolfcraft_assets::procedural::sprites(wolfcraft_assets::AtlasKind::Blocks).len() as u16;
        for b in Block::ALL {
            for tile in b.info().tiles {
                assert!(tile < n, "{:?} uses missing tile {}", b, tile);
            }
        }
    }
}


#[cfg(test)]mod construction_facing_tests {
 use super::*;
 #[test]fn fronts_face_the_player_on_all_four_sides(){
  for (i,j,r) in [(1.,0.,3),(-1.,0.,1),(0.,1.,0),(0.,-1.,2)] {
   // Our own doors, crafting table and furnace stay our blocks (textures, double doors).
   for b in [Block::Door,Block::BirchDoor,Block::SpruceDoor,Block::IronDoor,Block::CraftingTable,Block::ElectricFurnace] {
    let placed=b.oriented(i,j);
    assert!(crate::construction::entry(placed as u16).is_none(),"{:?} must not turn into a catalogue piece",b);
    assert_eq!(placed.base_item(),b);
    if b!=Block::CraftingTable&&b!=Block::ElectricFurnace {assert!(placed.door_toggle().is_some()&&placed.door_top().is_some(),"{:?} is still a two-block door",b);}
   }
   for b in [Block::Chest] {
    let placed=b.oriented(i,j);let (e,rotation)=crate::construction::entry(placed as u16).unwrap();assert_eq!(rotation,r);
    assert_eq!(crate::items::info(placed.base_item() as u16).place.unwrap().oriented(i,j),placed,"{}",e.name);
   }
  }
 }
}

/// Boxes of a button or a lever on a wall: the floor boxes stood up against the wall at side
/// `dir` (the height off the floor becomes the distance out of the wall; the lever leans up
/// and down instead of sideways).
fn wall_boxes(base: Block, dir: u8) -> &'static [[f32; 6]] {
    use std::sync::OnceLock;
    static T: [OnceLock<Vec<[f32; 6]>>; 24] = [const { OnceLock::new() }; 24];
    let kind = match base {
        Block::Button => 0,
        Block::ButtonOn => 1,
        Block::SwitchOff => 2,
        Block::SwitchOn => 3,
        _ => return &[],
    };
    T[kind * 4 + (dir % 4) as usize].get_or_init(|| {
        base.boxes()
            .iter()
            .map(|b| {
                let (o0, o1) = (b[2], b[5]);
                match dir % 4 {
                    0 => [1.0 - o1, b[0], b[1], 1.0 - o0, b[3], b[4]],
                    1 => [o0, b[0], b[1], o1, b[3], b[4]],
                    2 => [b[0], 1.0 - o1, b[1], b[3], 1.0 - o0, b[4]],
                    _ => [b[0], o0, b[1], b[3], o1, b[4]],
                }
            })
            .collect()
    })
}
