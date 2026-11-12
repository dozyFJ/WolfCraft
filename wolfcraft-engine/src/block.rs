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
    // Was the machine casing: now the forge (repair and upgrade tools).
    75 MachineCasing => needs_pick(info("Кузница", Opaque, 5.0, Pickaxe, column(t::MACHINE_CASING, t::GENERATOR_FRONT, t::COBBLESTONE))),
    76 Generator => needs_pick(info("Генератор", Opaque, 8.0, Pickaxe, [t::MACHINE_CASING, t::MACHINE_CASING, t::MACHINE_CASING, t::MACHINE_CASING, t::GENERATOR_FRONT, t::MACHINE_CASING])),
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
        const WIRE: [f32; 6] = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0625];
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
            b => b,
        }
    }

    /// The item a placed variant comes from (open doors, turned stairs and panes, ...).
    pub fn base_item(self) -> Block {
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
        use Block::*;
        match self {
            Lava | LavaFall | Glowstone | Lamp | Lantern | ElectricLampOn | IskritLampOn | InnerCore => 15,
            LavaFlow1 | LavaFlow2 | LavaFlow3 => 13,
            Torch => 14,
            UraniumBlock => 10,
            UraniumOre => 6,
            NetherPortal => 11,
            IskritTorch => 7,
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
