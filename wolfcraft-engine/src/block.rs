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
        solid: matches!(render, RenderKind::Opaque | RenderKind::Cutout),
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

macro_rules! blocks {
    ($( $id:literal $var:ident => $info:expr ),* $(,)?) => {
        #[repr(u8)]
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
    35 Obsidian => needs_pick(info("Обсидиан", Opaque, 40.0, Pickaxe, all(t::OBSIDIAN))),
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
    75 MachineCasing => needs_pick(info("Корпус машины", Opaque, 8.0, Pickaxe, all(t::MACHINE_CASING))),
    76 Generator => needs_pick(info("Генератор", Opaque, 8.0, Pickaxe, [t::MACHINE_CASING, t::MACHINE_CASING, t::MACHINE_CASING, t::MACHINE_CASING, t::GENERATOR_FRONT, t::MACHINE_CASING])),
    77 Cable => needs_pick(info("Медный кабель", Opaque, 2.0, Pickaxe, all(t::CABLE))),
    78 Battery => needs_pick(info("Аккумулятор", Opaque, 8.0, Pickaxe, column(t::BATTERY_SIDE, t::BATTERY_TOP, t::BATTERY_TOP))),
    79 ElectricLamp => emissive(info("Электролампа", Opaque, 0.5, Hand, all(t::ELECTRIC_LAMP))),
    80 ElectricFurnace => needs_pick(info("Электропечь", Opaque, 8.0, Pickaxe, [t::MACHINE_CASING, t::MACHINE_CASING, t::MACHINE_CASING, t::MACHINE_CASING, t::FURNACE_FRONT, t::MACHINE_CASING])),
    81 ElectricLampOn => emissive(info("Электролампа (вкл.)", Opaque, 0.5, Hand, all(t::ELECTRIC_LAMP_ON))),
    82 SwitchOff => needs_pick(info("Рубильник", Opaque, 2.0, Pickaxe, all(t::SWITCH_OFF))),
    83 SwitchOn => emissive(needs_pick(info("Рубильник (вкл.)", Opaque, 2.0, Pickaxe, all(t::SWITCH_ON)))),
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
    96 Door => info("Дверь", Shape, 2.0, Axe, all(t::PLANKS)),
    97 DoorOpen => info("Дверь (открыта)", Shape, 2.0, Axe, all(t::PLANKS)),
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
    145 BirchDoor => info("Берёзовая дверь", Shape, 2.0, Axe, all(t::BIRCH_PLANKS)),
    146 BirchDoorOpen => info("Берёзовая дверь (открыта)", Shape, 2.0, Axe, all(t::BIRCH_PLANKS)),
    147 SpruceDoor => info("Еловая дверь", Shape, 2.0, Axe, all(t::SPRUCE_PLANKS)),
    148 SpruceDoorOpen => info("Еловая дверь (открыта)", Shape, 2.0, Axe, all(t::SPRUCE_PLANKS)),
    149 IronDoor => needs_pick(info("Железная дверь", Shape, 8.0, Pickaxe, all(t::IRON_BLOCK))),
    150 IronDoorOpen => needs_pick(info("Железная дверь (открыта)", Shape, 8.0, Pickaxe, all(t::IRON_BLOCK))),
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
}

impl Block {
    #[inline]
    pub fn info(self) -> &'static BlockInfo {
        &BLOCK_INFO[self as usize]
    }
    #[inline]
    pub fn from_u8(v: u8) -> Block {
        Block::ALL.get(v as usize).copied().unwrap_or(Block::Air)
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

    /// All boxes of a `Shape` block (empty for full cubes).
    pub fn boxes(self) -> &'static [[f32; 6]] {
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
            Door | BirchDoor | SpruceDoor | IronDoor => std::slice::from_ref(&DOOR),
            DoorOpen | BirchDoorOpen | SpruceDoorOpen | IronDoorOpen => std::slice::from_ref(&DOOR_OPEN),
            CobbleStairs | PlankStairs | StoneBrickStairs | SandstoneStairs => &ST0,
            CobbleStairs1 | PlankStairs1 | StoneBrickStairs1 | SandstoneStairs1 => &ST1,
            CobbleStairs2 | PlankStairs2 | StoneBrickStairs2 | SandstoneStairs2 => &ST2,
            CobbleStairs3 | PlankStairs3 | StoneBrickStairs3 | SandstoneStairs3 => &ST3,
            Sign => &SIGN_A,
            SignB => &SIGN_B,
            _ => &[],
        }
    }

    /// Block placed for this item, turned to face `dir_i`/`dir_j` (how much the player looks
    /// along the cell's i and j axes): stairs rise away from the player, panes, doors,
    /// vertical slabs and signs stand across the view.
    pub fn oriented(self, dir_i: f64, dir_j: f64) -> Block {
        use Block::*;
        let along_i = dir_i.abs() >= dir_j.abs();
        let stairs = |base: Block| -> Block {
            let k = if along_i { if dir_i > 0.0 { 0 } else { 1 } } else if dir_j > 0.0 { 2 } else { 3 };
            Block::from_u8(base as u8 + k)
        };
        let ab = |a: Block, b: Block| if along_i { a } else { b };
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
            b => b,
        }
    }

    /// The item a placed variant comes from (open doors, turned stairs and panes, ...).
    pub fn base_item(self) -> Block {
        use Block::*;
        match self {
            DoorOpen => Door,
            BirchDoorOpen => BirchDoor,
            SpruceDoorOpen => SpruceDoor,
            IronDoorOpen => IronDoor,
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
            b => b,
        }
    }

    /// Door states: (closed, open) pairs toggle on use.
    pub fn door_toggle(self) -> Option<Block> {
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
            _ => return None,
        })
    }

    /// Collides with the player.
    #[inline]
    pub fn is_solid(self) -> bool {
        self.info().solid
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
        matches!(self, Block::Air | Block::Water | Block::TallGrass | Block::DeadBush | Block::Lava) || self.info().render == RenderKind::Cross
    }
    #[inline]
    pub fn is_fluid(self) -> bool {
        matches!(self, Block::Water | Block::Lava)
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
            assert_eq!(*b as usize, i, "block table must be ordered by id");
            assert_eq!(Block::from_u8(i as u8), *b);
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
        assert_eq!(Block::from_u8(250), Block::Air);
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
