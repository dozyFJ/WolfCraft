//! Parametric building catalogue. IDs 1024+ preserve the legacy block and item ranges.
use crate::block::{Block, BlockInfo, RenderKind, ToolKind};
use wolfcraft_assets::tiles as t;
pub mod geometry;
pub const FIRST_ID: u16 = 1024;
pub const LAST_ID: u16 = 2443;
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form { LowerSlab, UpperSlab, VerticalSlab, Stairs, InnerStairs, OuterStairs, Wedge, Beam, Post, Quarter, Arch, HalfArch, Panel, Grille, Arc15Small, Arc30Small, Arc45Small, Arc90Small, Arc15Medium, Arc30Medium, Arc45Medium, Arc90Medium, Arc15Large, Arc30Large, Arc45Large, Arc90Large, Dome, DomeHalf, DomeQuarter, Door, DoorOpen, DoorTop, DoorTopOpen, SlidingDoor, SlidingDoorOpen, SlidingDoorTop, SlidingDoorTopOpen, FoldingDoor, FoldingDoorOpen, FoldingDoorTop, FoldingDoorTopOpen, Chair, Table, GlassTable, Sofa, SofaCorner, Bed, Wardrobe, Shelf, Nightstand, KitchenBase, Counter, Sink, FloorLamp, Curtain, Rug, Mirror, FlowerPot, StorageChest, Barrel, PortableCrate, SharedChest, ItemFrame, PaintingForest, PaintingSky, PaintingAbstract, Workbench, Furnace, Terminal, Command, StoneAltar, RootAltar, EmberAltar }
pub const FORM_COUNT: usize = 73;
pub const FORMS: &[(Form, &str)] = &[
    (Form::LowerSlab, "Нижняя плита"),
    (Form::UpperSlab, "Верхняя плита"),
    (Form::VerticalSlab, "Вертикальная плита"),
    (Form::Stairs, "Ступени"),
    (Form::InnerStairs, "Внутренний угол ступеней"),
    (Form::OuterStairs, "Внешний угол ступеней"),
    (Form::Wedge, "Скат крыши"),
    (Form::Beam, "Балка"),
    (Form::Post, "Столб"),
    (Form::Quarter, "Четверть блока"),
    (Form::Arch, "Арка"),
    (Form::HalfArch, "Полуарка"),
    (Form::Panel, "Панель"),
    (Form::Grille, "Решётка"),
    (Form::Arc15Small, "Дуговая секция 15° R½"),
    (Form::Arc30Small, "Дуговая секция 30° R½"),
    (Form::Arc45Small, "Дуговая секция 45° R½"),
    (Form::Arc90Small, "Дуговая секция 90° R½"),
    (Form::Arc15Medium, "Дуговая секция 15° R¾"),
    (Form::Arc30Medium, "Дуговая секция 30° R¾"),
    (Form::Arc45Medium, "Дуговая секция 45° R¾"),
    (Form::Arc90Medium, "Дуговая секция 90° R¾"),
    (Form::Arc15Large, "Дуговая секция 15° R1"),
    (Form::Arc30Large, "Дуговая секция 30° R1"),
    (Form::Arc45Large, "Дуговая секция 45° R1"),
    (Form::Arc90Large, "Дуговая секция 90° R1"),
    (Form::Dome, "Купол"),
    (Form::DomeHalf, "Половина купола"),
    (Form::DomeQuarter, "Четверть купола"),
    (Form::Door, "Дверь"),
    (Form::SlidingDoor, "Раздвижная дверь"),
    (Form::FoldingDoor, "Складная дверь"),
    (Form::Chair, "Стул"),
    (Form::Table, "Стол"),
    (Form::GlassTable, "Стол со стеклянной столешницей"),
    (Form::Sofa, "Диван"),
    (Form::SofaCorner, "Угловой диван"),
    (Form::Bed, "Кровать"),
    (Form::Wardrobe, "Шкаф"),
    (Form::Shelf, "Полка"),
    (Form::Nightstand, "Тумба"),
    (Form::KitchenBase, "Кухонный шкаф"),
    (Form::Counter, "Кухонная столешница"),
    (Form::Sink, "Мойка"),
    (Form::FloorLamp, "Торшер"),
    (Form::Curtain, "Штора"),
    (Form::Rug, "Ковёр"),
    (Form::Mirror, "Зеркало"),
    (Form::FlowerPot, "Горшок с растением"),
    (Form::StorageChest, "Сундук"),
    (Form::Barrel, "Бочка"),
    (Form::PortableCrate, "Путевой ящик"),
    (Form::SharedChest, "Резонансный сундук"),
    (Form::ItemFrame, "Рамка для предмета"),
    (Form::PaintingForest, "Картина «Чаща»"),
    (Form::PaintingSky, "Картина «Небо»"),
    (Form::PaintingAbstract, "Картина «Искрит»"),
    (Form::Workbench, "Верстак"),
    (Form::Furnace, "Печь"),
    (Form::Terminal, "Терминал"),
    (Form::Command, "Командный блок"),
    (Form::StoneAltar, "Каменный алтарь"),
    (Form::RootAltar, "Лесной алтарь"),
    (Form::EmberAltar, "Пепельный алтарь"),
];
#[derive(Clone, Copy)]
pub struct Material { pub name: &'static str, pub tile: u16, pub ingredient: u16, pub glass: bool }
pub const MATERIALS: &[Material] = &[
    Material { name: "Дуб", tile: t::PLANKS, ingredient: 8, glass: false },
    Material { name: "Берёза", tile: t::BIRCH_PLANKS, ingredient: 99, glass: false },
    Material { name: "Ель", tile: t::SPRUCE_PLANKS, ingredient: 100, glass: false },
    Material { name: "Камень", tile: t::STONE, ingredient: 1, glass: false },
    Material { name: "Кирпич", tile: t::BRICKS, ingredient: 27, glass: false },
    Material { name: "Мрамор", tile: t::MARBLE, ingredient: 60, glass: false },
    Material { name: "Железо", tile: t::IRON_BLOCK, ingredient: 68, glass: false },
    Material { name: "Медь", tile: t::COPPER_BLOCK, ingredient: 67, glass: false },
    Material { name: "Стекло", tile: t::BUILD_GLASS, ingredient: 11, glass: true },
    Material { name: "Красное стекло", tile: t::BUILD_GLASS_RED, ingredient: 151, glass: true },
    Material { name: "Синее стекло", tile: t::BUILD_GLASS_BLUE, ingredient: 152, glass: true },
    Material { name: "Зелёное стекло", tile: t::BUILD_GLASS_GREEN, ingredient: 153, glass: true },
];
#[derive(Clone, Copy)]
pub struct Entry { pub name: &'static str, pub form: Form, pub material: usize, pub category: &'static str, pub base: u16, pub id: u16 }
pub const ENTRIES: &[Entry] = &[
    Entry { name: "Дуб · Нижняя плита", form: Form::LowerSlab, material: 0, category: "Строительство", base: 1024, id: 1024 },
    Entry { name: "Берёза · Нижняя плита", form: Form::LowerSlab, material: 1, category: "Строительство", base: 1028, id: 1028 },
    Entry { name: "Ель · Нижняя плита", form: Form::LowerSlab, material: 2, category: "Строительство", base: 1032, id: 1032 },
    Entry { name: "Камень · Нижняя плита", form: Form::LowerSlab, material: 3, category: "Строительство", base: 1036, id: 1036 },
    Entry { name: "Кирпич · Нижняя плита", form: Form::LowerSlab, material: 4, category: "Строительство", base: 1040, id: 1040 },
    Entry { name: "Мрамор · Нижняя плита", form: Form::LowerSlab, material: 5, category: "Строительство", base: 1044, id: 1044 },
    Entry { name: "Железо · Нижняя плита", form: Form::LowerSlab, material: 6, category: "Строительство", base: 1048, id: 1048 },
    Entry { name: "Медь · Нижняя плита", form: Form::LowerSlab, material: 7, category: "Строительство", base: 1052, id: 1052 },
    Entry { name: "Дуб · Верхняя плита", form: Form::UpperSlab, material: 0, category: "Строительство", base: 1056, id: 1056 },
    Entry { name: "Берёза · Верхняя плита", form: Form::UpperSlab, material: 1, category: "Строительство", base: 1060, id: 1060 },
    Entry { name: "Ель · Верхняя плита", form: Form::UpperSlab, material: 2, category: "Строительство", base: 1064, id: 1064 },
    Entry { name: "Камень · Верхняя плита", form: Form::UpperSlab, material: 3, category: "Строительство", base: 1068, id: 1068 },
    Entry { name: "Кирпич · Верхняя плита", form: Form::UpperSlab, material: 4, category: "Строительство", base: 1072, id: 1072 },
    Entry { name: "Мрамор · Верхняя плита", form: Form::UpperSlab, material: 5, category: "Строительство", base: 1076, id: 1076 },
    Entry { name: "Железо · Верхняя плита", form: Form::UpperSlab, material: 6, category: "Строительство", base: 1080, id: 1080 },
    Entry { name: "Медь · Верхняя плита", form: Form::UpperSlab, material: 7, category: "Строительство", base: 1084, id: 1084 },
    Entry { name: "Дуб · Вертикальная плита", form: Form::VerticalSlab, material: 0, category: "Строительство", base: 1088, id: 1088 },
    Entry { name: "Берёза · Вертикальная плита", form: Form::VerticalSlab, material: 1, category: "Строительство", base: 1092, id: 1092 },
    Entry { name: "Ель · Вертикальная плита", form: Form::VerticalSlab, material: 2, category: "Строительство", base: 1096, id: 1096 },
    Entry { name: "Камень · Вертикальная плита", form: Form::VerticalSlab, material: 3, category: "Строительство", base: 1100, id: 1100 },
    Entry { name: "Кирпич · Вертикальная плита", form: Form::VerticalSlab, material: 4, category: "Строительство", base: 1104, id: 1104 },
    Entry { name: "Мрамор · Вертикальная плита", form: Form::VerticalSlab, material: 5, category: "Строительство", base: 1108, id: 1108 },
    Entry { name: "Железо · Вертикальная плита", form: Form::VerticalSlab, material: 6, category: "Строительство", base: 1112, id: 1112 },
    Entry { name: "Медь · Вертикальная плита", form: Form::VerticalSlab, material: 7, category: "Строительство", base: 1116, id: 1116 },
    Entry { name: "Дуб · Ступени", form: Form::Stairs, material: 0, category: "Строительство", base: 1120, id: 1120 },
    Entry { name: "Берёза · Ступени", form: Form::Stairs, material: 1, category: "Строительство", base: 1124, id: 1124 },
    Entry { name: "Ель · Ступени", form: Form::Stairs, material: 2, category: "Строительство", base: 1128, id: 1128 },
    Entry { name: "Камень · Ступени", form: Form::Stairs, material: 3, category: "Строительство", base: 1132, id: 1132 },
    Entry { name: "Кирпич · Ступени", form: Form::Stairs, material: 4, category: "Строительство", base: 1136, id: 1136 },
    Entry { name: "Мрамор · Ступени", form: Form::Stairs, material: 5, category: "Строительство", base: 1140, id: 1140 },
    Entry { name: "Железо · Ступени", form: Form::Stairs, material: 6, category: "Строительство", base: 1144, id: 1144 },
    Entry { name: "Медь · Ступени", form: Form::Stairs, material: 7, category: "Строительство", base: 1148, id: 1148 },
    Entry { name: "Дуб · Внутренний угол ступеней", form: Form::InnerStairs, material: 0, category: "Строительство", base: 1152, id: 1152 },
    Entry { name: "Берёза · Внутренний угол ступеней", form: Form::InnerStairs, material: 1, category: "Строительство", base: 1156, id: 1156 },
    Entry { name: "Ель · Внутренний угол ступеней", form: Form::InnerStairs, material: 2, category: "Строительство", base: 1160, id: 1160 },
    Entry { name: "Камень · Внутренний угол ступеней", form: Form::InnerStairs, material: 3, category: "Строительство", base: 1164, id: 1164 },
    Entry { name: "Кирпич · Внутренний угол ступеней", form: Form::InnerStairs, material: 4, category: "Строительство", base: 1168, id: 1168 },
    Entry { name: "Мрамор · Внутренний угол ступеней", form: Form::InnerStairs, material: 5, category: "Строительство", base: 1172, id: 1172 },
    Entry { name: "Железо · Внутренний угол ступеней", form: Form::InnerStairs, material: 6, category: "Строительство", base: 1176, id: 1176 },
    Entry { name: "Медь · Внутренний угол ступеней", form: Form::InnerStairs, material: 7, category: "Строительство", base: 1180, id: 1180 },
    Entry { name: "Дуб · Внешний угол ступеней", form: Form::OuterStairs, material: 0, category: "Строительство", base: 1184, id: 1184 },
    Entry { name: "Берёза · Внешний угол ступеней", form: Form::OuterStairs, material: 1, category: "Строительство", base: 1188, id: 1188 },
    Entry { name: "Ель · Внешний угол ступеней", form: Form::OuterStairs, material: 2, category: "Строительство", base: 1192, id: 1192 },
    Entry { name: "Камень · Внешний угол ступеней", form: Form::OuterStairs, material: 3, category: "Строительство", base: 1196, id: 1196 },
    Entry { name: "Кирпич · Внешний угол ступеней", form: Form::OuterStairs, material: 4, category: "Строительство", base: 1200, id: 1200 },
    Entry { name: "Мрамор · Внешний угол ступеней", form: Form::OuterStairs, material: 5, category: "Строительство", base: 1204, id: 1204 },
    Entry { name: "Железо · Внешний угол ступеней", form: Form::OuterStairs, material: 6, category: "Строительство", base: 1208, id: 1208 },
    Entry { name: "Медь · Внешний угол ступеней", form: Form::OuterStairs, material: 7, category: "Строительство", base: 1212, id: 1212 },
    Entry { name: "Дуб · Скат крыши", form: Form::Wedge, material: 0, category: "Строительство", base: 1216, id: 1216 },
    Entry { name: "Берёза · Скат крыши", form: Form::Wedge, material: 1, category: "Строительство", base: 1220, id: 1220 },
    Entry { name: "Ель · Скат крыши", form: Form::Wedge, material: 2, category: "Строительство", base: 1224, id: 1224 },
    Entry { name: "Камень · Скат крыши", form: Form::Wedge, material: 3, category: "Строительство", base: 1228, id: 1228 },
    Entry { name: "Кирпич · Скат крыши", form: Form::Wedge, material: 4, category: "Строительство", base: 1232, id: 1232 },
    Entry { name: "Мрамор · Скат крыши", form: Form::Wedge, material: 5, category: "Строительство", base: 1236, id: 1236 },
    Entry { name: "Железо · Скат крыши", form: Form::Wedge, material: 6, category: "Строительство", base: 1240, id: 1240 },
    Entry { name: "Медь · Скат крыши", form: Form::Wedge, material: 7, category: "Строительство", base: 1244, id: 1244 },
    Entry { name: "Дуб · Балка", form: Form::Beam, material: 0, category: "Строительство", base: 1248, id: 1248 },
    Entry { name: "Берёза · Балка", form: Form::Beam, material: 1, category: "Строительство", base: 1252, id: 1252 },
    Entry { name: "Ель · Балка", form: Form::Beam, material: 2, category: "Строительство", base: 1256, id: 1256 },
    Entry { name: "Камень · Балка", form: Form::Beam, material: 3, category: "Строительство", base: 1260, id: 1260 },
    Entry { name: "Кирпич · Балка", form: Form::Beam, material: 4, category: "Строительство", base: 1264, id: 1264 },
    Entry { name: "Мрамор · Балка", form: Form::Beam, material: 5, category: "Строительство", base: 1268, id: 1268 },
    Entry { name: "Железо · Балка", form: Form::Beam, material: 6, category: "Строительство", base: 1272, id: 1272 },
    Entry { name: "Медь · Балка", form: Form::Beam, material: 7, category: "Строительство", base: 1276, id: 1276 },
    Entry { name: "Дуб · Столб", form: Form::Post, material: 0, category: "Строительство", base: 1280, id: 1280 },
    Entry { name: "Берёза · Столб", form: Form::Post, material: 1, category: "Строительство", base: 1284, id: 1284 },
    Entry { name: "Ель · Столб", form: Form::Post, material: 2, category: "Строительство", base: 1288, id: 1288 },
    Entry { name: "Камень · Столб", form: Form::Post, material: 3, category: "Строительство", base: 1292, id: 1292 },
    Entry { name: "Кирпич · Столб", form: Form::Post, material: 4, category: "Строительство", base: 1296, id: 1296 },
    Entry { name: "Мрамор · Столб", form: Form::Post, material: 5, category: "Строительство", base: 1300, id: 1300 },
    Entry { name: "Железо · Столб", form: Form::Post, material: 6, category: "Строительство", base: 1304, id: 1304 },
    Entry { name: "Медь · Столб", form: Form::Post, material: 7, category: "Строительство", base: 1308, id: 1308 },
    Entry { name: "Дуб · Четверть блока", form: Form::Quarter, material: 0, category: "Строительство", base: 1312, id: 1312 },
    Entry { name: "Берёза · Четверть блока", form: Form::Quarter, material: 1, category: "Строительство", base: 1316, id: 1316 },
    Entry { name: "Ель · Четверть блока", form: Form::Quarter, material: 2, category: "Строительство", base: 1320, id: 1320 },
    Entry { name: "Камень · Четверть блока", form: Form::Quarter, material: 3, category: "Строительство", base: 1324, id: 1324 },
    Entry { name: "Кирпич · Четверть блока", form: Form::Quarter, material: 4, category: "Строительство", base: 1328, id: 1328 },
    Entry { name: "Мрамор · Четверть блока", form: Form::Quarter, material: 5, category: "Строительство", base: 1332, id: 1332 },
    Entry { name: "Железо · Четверть блока", form: Form::Quarter, material: 6, category: "Строительство", base: 1336, id: 1336 },
    Entry { name: "Медь · Четверть блока", form: Form::Quarter, material: 7, category: "Строительство", base: 1340, id: 1340 },
    Entry { name: "Дуб · Арка", form: Form::Arch, material: 0, category: "Строительство", base: 1344, id: 1344 },
    Entry { name: "Берёза · Арка", form: Form::Arch, material: 1, category: "Строительство", base: 1348, id: 1348 },
    Entry { name: "Ель · Арка", form: Form::Arch, material: 2, category: "Строительство", base: 1352, id: 1352 },
    Entry { name: "Камень · Арка", form: Form::Arch, material: 3, category: "Строительство", base: 1356, id: 1356 },
    Entry { name: "Кирпич · Арка", form: Form::Arch, material: 4, category: "Строительство", base: 1360, id: 1360 },
    Entry { name: "Мрамор · Арка", form: Form::Arch, material: 5, category: "Строительство", base: 1364, id: 1364 },
    Entry { name: "Железо · Арка", form: Form::Arch, material: 6, category: "Строительство", base: 1368, id: 1368 },
    Entry { name: "Медь · Арка", form: Form::Arch, material: 7, category: "Строительство", base: 1372, id: 1372 },
    Entry { name: "Дуб · Полуарка", form: Form::HalfArch, material: 0, category: "Строительство", base: 1376, id: 1376 },
    Entry { name: "Берёза · Полуарка", form: Form::HalfArch, material: 1, category: "Строительство", base: 1380, id: 1380 },
    Entry { name: "Ель · Полуарка", form: Form::HalfArch, material: 2, category: "Строительство", base: 1384, id: 1384 },
    Entry { name: "Камень · Полуарка", form: Form::HalfArch, material: 3, category: "Строительство", base: 1388, id: 1388 },
    Entry { name: "Кирпич · Полуарка", form: Form::HalfArch, material: 4, category: "Строительство", base: 1392, id: 1392 },
    Entry { name: "Мрамор · Полуарка", form: Form::HalfArch, material: 5, category: "Строительство", base: 1396, id: 1396 },
    Entry { name: "Железо · Полуарка", form: Form::HalfArch, material: 6, category: "Строительство", base: 1400, id: 1400 },
    Entry { name: "Медь · Полуарка", form: Form::HalfArch, material: 7, category: "Строительство", base: 1404, id: 1404 },
    Entry { name: "Дуб · Панель", form: Form::Panel, material: 0, category: "Строительство", base: 1408, id: 1408 },
    Entry { name: "Берёза · Панель", form: Form::Panel, material: 1, category: "Строительство", base: 1412, id: 1412 },
    Entry { name: "Ель · Панель", form: Form::Panel, material: 2, category: "Строительство", base: 1416, id: 1416 },
    Entry { name: "Камень · Панель", form: Form::Panel, material: 3, category: "Строительство", base: 1420, id: 1420 },
    Entry { name: "Кирпич · Панель", form: Form::Panel, material: 4, category: "Строительство", base: 1424, id: 1424 },
    Entry { name: "Мрамор · Панель", form: Form::Panel, material: 5, category: "Строительство", base: 1428, id: 1428 },
    Entry { name: "Железо · Панель", form: Form::Panel, material: 6, category: "Строительство", base: 1432, id: 1432 },
    Entry { name: "Медь · Панель", form: Form::Panel, material: 7, category: "Строительство", base: 1436, id: 1436 },
    Entry { name: "Дуб · Решётка", form: Form::Grille, material: 0, category: "Строительство", base: 1440, id: 1440 },
    Entry { name: "Берёза · Решётка", form: Form::Grille, material: 1, category: "Строительство", base: 1444, id: 1444 },
    Entry { name: "Ель · Решётка", form: Form::Grille, material: 2, category: "Строительство", base: 1448, id: 1448 },
    Entry { name: "Камень · Решётка", form: Form::Grille, material: 3, category: "Строительство", base: 1452, id: 1452 },
    Entry { name: "Кирпич · Решётка", form: Form::Grille, material: 4, category: "Строительство", base: 1456, id: 1456 },
    Entry { name: "Мрамор · Решётка", form: Form::Grille, material: 5, category: "Строительство", base: 1460, id: 1460 },
    Entry { name: "Железо · Решётка", form: Form::Grille, material: 6, category: "Строительство", base: 1464, id: 1464 },
    Entry { name: "Медь · Решётка", form: Form::Grille, material: 7, category: "Строительство", base: 1468, id: 1468 },
    Entry { name: "Стекло · Дуговая секция 15° R½", form: Form::Arc15Small, material: 8, category: "Стекло", base: 1472, id: 1472 },
    Entry { name: "Красное стекло · Дуговая секция 15° R½", form: Form::Arc15Small, material: 9, category: "Стекло", base: 1476, id: 1476 },
    Entry { name: "Синее стекло · Дуговая секция 15° R½", form: Form::Arc15Small, material: 10, category: "Стекло", base: 1480, id: 1480 },
    Entry { name: "Зелёное стекло · Дуговая секция 15° R½", form: Form::Arc15Small, material: 11, category: "Стекло", base: 1484, id: 1484 },
    Entry { name: "Стекло · Дуговая секция 30° R½", form: Form::Arc30Small, material: 8, category: "Стекло", base: 1488, id: 1488 },
    Entry { name: "Красное стекло · Дуговая секция 30° R½", form: Form::Arc30Small, material: 9, category: "Стекло", base: 1492, id: 1492 },
    Entry { name: "Синее стекло · Дуговая секция 30° R½", form: Form::Arc30Small, material: 10, category: "Стекло", base: 1496, id: 1496 },
    Entry { name: "Зелёное стекло · Дуговая секция 30° R½", form: Form::Arc30Small, material: 11, category: "Стекло", base: 1500, id: 1500 },
    Entry { name: "Стекло · Дуговая секция 45° R½", form: Form::Arc45Small, material: 8, category: "Стекло", base: 1504, id: 1504 },
    Entry { name: "Красное стекло · Дуговая секция 45° R½", form: Form::Arc45Small, material: 9, category: "Стекло", base: 1508, id: 1508 },
    Entry { name: "Синее стекло · Дуговая секция 45° R½", form: Form::Arc45Small, material: 10, category: "Стекло", base: 1512, id: 1512 },
    Entry { name: "Зелёное стекло · Дуговая секция 45° R½", form: Form::Arc45Small, material: 11, category: "Стекло", base: 1516, id: 1516 },
    Entry { name: "Стекло · Дуговая секция 90° R½", form: Form::Arc90Small, material: 8, category: "Стекло", base: 1520, id: 1520 },
    Entry { name: "Красное стекло · Дуговая секция 90° R½", form: Form::Arc90Small, material: 9, category: "Стекло", base: 1524, id: 1524 },
    Entry { name: "Синее стекло · Дуговая секция 90° R½", form: Form::Arc90Small, material: 10, category: "Стекло", base: 1528, id: 1528 },
    Entry { name: "Зелёное стекло · Дуговая секция 90° R½", form: Form::Arc90Small, material: 11, category: "Стекло", base: 1532, id: 1532 },
    Entry { name: "Стекло · Дуговая секция 15° R¾", form: Form::Arc15Medium, material: 8, category: "Стекло", base: 1536, id: 1536 },
    Entry { name: "Красное стекло · Дуговая секция 15° R¾", form: Form::Arc15Medium, material: 9, category: "Стекло", base: 1540, id: 1540 },
    Entry { name: "Синее стекло · Дуговая секция 15° R¾", form: Form::Arc15Medium, material: 10, category: "Стекло", base: 1544, id: 1544 },
    Entry { name: "Зелёное стекло · Дуговая секция 15° R¾", form: Form::Arc15Medium, material: 11, category: "Стекло", base: 1548, id: 1548 },
    Entry { name: "Стекло · Дуговая секция 30° R¾", form: Form::Arc30Medium, material: 8, category: "Стекло", base: 1552, id: 1552 },
    Entry { name: "Красное стекло · Дуговая секция 30° R¾", form: Form::Arc30Medium, material: 9, category: "Стекло", base: 1556, id: 1556 },
    Entry { name: "Синее стекло · Дуговая секция 30° R¾", form: Form::Arc30Medium, material: 10, category: "Стекло", base: 1560, id: 1560 },
    Entry { name: "Зелёное стекло · Дуговая секция 30° R¾", form: Form::Arc30Medium, material: 11, category: "Стекло", base: 1564, id: 1564 },
    Entry { name: "Стекло · Дуговая секция 45° R¾", form: Form::Arc45Medium, material: 8, category: "Стекло", base: 1568, id: 1568 },
    Entry { name: "Красное стекло · Дуговая секция 45° R¾", form: Form::Arc45Medium, material: 9, category: "Стекло", base: 1572, id: 1572 },
    Entry { name: "Синее стекло · Дуговая секция 45° R¾", form: Form::Arc45Medium, material: 10, category: "Стекло", base: 1576, id: 1576 },
    Entry { name: "Зелёное стекло · Дуговая секция 45° R¾", form: Form::Arc45Medium, material: 11, category: "Стекло", base: 1580, id: 1580 },
    Entry { name: "Стекло · Дуговая секция 90° R¾", form: Form::Arc90Medium, material: 8, category: "Стекло", base: 1584, id: 1584 },
    Entry { name: "Красное стекло · Дуговая секция 90° R¾", form: Form::Arc90Medium, material: 9, category: "Стекло", base: 1588, id: 1588 },
    Entry { name: "Синее стекло · Дуговая секция 90° R¾", form: Form::Arc90Medium, material: 10, category: "Стекло", base: 1592, id: 1592 },
    Entry { name: "Зелёное стекло · Дуговая секция 90° R¾", form: Form::Arc90Medium, material: 11, category: "Стекло", base: 1596, id: 1596 },
    Entry { name: "Стекло · Дуговая секция 15° R1", form: Form::Arc15Large, material: 8, category: "Стекло", base: 1600, id: 1600 },
    Entry { name: "Красное стекло · Дуговая секция 15° R1", form: Form::Arc15Large, material: 9, category: "Стекло", base: 1604, id: 1604 },
    Entry { name: "Синее стекло · Дуговая секция 15° R1", form: Form::Arc15Large, material: 10, category: "Стекло", base: 1608, id: 1608 },
    Entry { name: "Зелёное стекло · Дуговая секция 15° R1", form: Form::Arc15Large, material: 11, category: "Стекло", base: 1612, id: 1612 },
    Entry { name: "Стекло · Дуговая секция 30° R1", form: Form::Arc30Large, material: 8, category: "Стекло", base: 1616, id: 1616 },
    Entry { name: "Красное стекло · Дуговая секция 30° R1", form: Form::Arc30Large, material: 9, category: "Стекло", base: 1620, id: 1620 },
    Entry { name: "Синее стекло · Дуговая секция 30° R1", form: Form::Arc30Large, material: 10, category: "Стекло", base: 1624, id: 1624 },
    Entry { name: "Зелёное стекло · Дуговая секция 30° R1", form: Form::Arc30Large, material: 11, category: "Стекло", base: 1628, id: 1628 },
    Entry { name: "Стекло · Дуговая секция 45° R1", form: Form::Arc45Large, material: 8, category: "Стекло", base: 1632, id: 1632 },
    Entry { name: "Красное стекло · Дуговая секция 45° R1", form: Form::Arc45Large, material: 9, category: "Стекло", base: 1636, id: 1636 },
    Entry { name: "Синее стекло · Дуговая секция 45° R1", form: Form::Arc45Large, material: 10, category: "Стекло", base: 1640, id: 1640 },
    Entry { name: "Зелёное стекло · Дуговая секция 45° R1", form: Form::Arc45Large, material: 11, category: "Стекло", base: 1644, id: 1644 },
    Entry { name: "Стекло · Дуговая секция 90° R1", form: Form::Arc90Large, material: 8, category: "Стекло", base: 1648, id: 1648 },
    Entry { name: "Красное стекло · Дуговая секция 90° R1", form: Form::Arc90Large, material: 9, category: "Стекло", base: 1652, id: 1652 },
    Entry { name: "Синее стекло · Дуговая секция 90° R1", form: Form::Arc90Large, material: 10, category: "Стекло", base: 1656, id: 1656 },
    Entry { name: "Зелёное стекло · Дуговая секция 90° R1", form: Form::Arc90Large, material: 11, category: "Стекло", base: 1660, id: 1660 },
    Entry { name: "Стекло · Купол", form: Form::Dome, material: 8, category: "Стекло", base: 1664, id: 1664 },
    Entry { name: "Красное стекло · Купол", form: Form::Dome, material: 9, category: "Стекло", base: 1668, id: 1668 },
    Entry { name: "Синее стекло · Купол", form: Form::Dome, material: 10, category: "Стекло", base: 1672, id: 1672 },
    Entry { name: "Зелёное стекло · Купол", form: Form::Dome, material: 11, category: "Стекло", base: 1676, id: 1676 },
    Entry { name: "Стекло · Половина купола", form: Form::DomeHalf, material: 8, category: "Стекло", base: 1680, id: 1680 },
    Entry { name: "Красное стекло · Половина купола", form: Form::DomeHalf, material: 9, category: "Стекло", base: 1684, id: 1684 },
    Entry { name: "Синее стекло · Половина купола", form: Form::DomeHalf, material: 10, category: "Стекло", base: 1688, id: 1688 },
    Entry { name: "Зелёное стекло · Половина купола", form: Form::DomeHalf, material: 11, category: "Стекло", base: 1692, id: 1692 },
    Entry { name: "Стекло · Четверть купола", form: Form::DomeQuarter, material: 8, category: "Стекло", base: 1696, id: 1696 },
    Entry { name: "Красное стекло · Четверть купола", form: Form::DomeQuarter, material: 9, category: "Стекло", base: 1700, id: 1700 },
    Entry { name: "Синее стекло · Четверть купола", form: Form::DomeQuarter, material: 10, category: "Стекло", base: 1704, id: 1704 },
    Entry { name: "Зелёное стекло · Четверть купола", form: Form::DomeQuarter, material: 11, category: "Стекло", base: 1708, id: 1708 },
    Entry { name: "Дуб · Дверь", form: Form::Door, material: 0, category: "Двери", base: 1712, id: 1712 },
    Entry { name: "Берёза · Дверь", form: Form::Door, material: 1, category: "Двери", base: 1716, id: 1716 },
    Entry { name: "Ель · Дверь", form: Form::Door, material: 2, category: "Двери", base: 1720, id: 1720 },
    Entry { name: "Железо · Дверь", form: Form::Door, material: 6, category: "Двери", base: 1724, id: 1724 },
    Entry { name: "Медь · Дверь", form: Form::Door, material: 7, category: "Двери", base: 1728, id: 1728 },
    Entry { name: "Стекло · Дверь", form: Form::Door, material: 8, category: "Двери", base: 1732, id: 1732 },
    Entry { name: "Дуб · Дверь (открыта)", form: Form::DoorOpen, material: 0, category: "Двери", base: 1712, id: 1736 },
    Entry { name: "Берёза · Дверь (открыта)", form: Form::DoorOpen, material: 1, category: "Двери", base: 1716, id: 1740 },
    Entry { name: "Ель · Дверь (открыта)", form: Form::DoorOpen, material: 2, category: "Двери", base: 1720, id: 1744 },
    Entry { name: "Железо · Дверь (открыта)", form: Form::DoorOpen, material: 6, category: "Двери", base: 1724, id: 1748 },
    Entry { name: "Медь · Дверь (открыта)", form: Form::DoorOpen, material: 7, category: "Двери", base: 1728, id: 1752 },
    Entry { name: "Стекло · Дверь (открыта)", form: Form::DoorOpen, material: 8, category: "Двери", base: 1732, id: 1756 },
    Entry { name: "Дуб · Дверь (верх)", form: Form::DoorTop, material: 0, category: "Двери", base: 1712, id: 1760 },
    Entry { name: "Берёза · Дверь (верх)", form: Form::DoorTop, material: 1, category: "Двери", base: 1716, id: 1764 },
    Entry { name: "Ель · Дверь (верх)", form: Form::DoorTop, material: 2, category: "Двери", base: 1720, id: 1768 },
    Entry { name: "Железо · Дверь (верх)", form: Form::DoorTop, material: 6, category: "Двери", base: 1724, id: 1772 },
    Entry { name: "Медь · Дверь (верх)", form: Form::DoorTop, material: 7, category: "Двери", base: 1728, id: 1776 },
    Entry { name: "Стекло · Дверь (верх)", form: Form::DoorTop, material: 8, category: "Двери", base: 1732, id: 1780 },
    Entry { name: "Дуб · Дверь (верх открыт)", form: Form::DoorTopOpen, material: 0, category: "Двери", base: 1712, id: 1784 },
    Entry { name: "Берёза · Дверь (верх открыт)", form: Form::DoorTopOpen, material: 1, category: "Двери", base: 1716, id: 1788 },
    Entry { name: "Ель · Дверь (верх открыт)", form: Form::DoorTopOpen, material: 2, category: "Двери", base: 1720, id: 1792 },
    Entry { name: "Железо · Дверь (верх открыт)", form: Form::DoorTopOpen, material: 6, category: "Двери", base: 1724, id: 1796 },
    Entry { name: "Медь · Дверь (верх открыт)", form: Form::DoorTopOpen, material: 7, category: "Двери", base: 1728, id: 1800 },
    Entry { name: "Стекло · Дверь (верх открыт)", form: Form::DoorTopOpen, material: 8, category: "Двери", base: 1732, id: 1804 },
    Entry { name: "Дуб · Раздвижная дверь", form: Form::SlidingDoor, material: 0, category: "Двери", base: 1808, id: 1808 },
    Entry { name: "Берёза · Раздвижная дверь", form: Form::SlidingDoor, material: 1, category: "Двери", base: 1812, id: 1812 },
    Entry { name: "Ель · Раздвижная дверь", form: Form::SlidingDoor, material: 2, category: "Двери", base: 1816, id: 1816 },
    Entry { name: "Железо · Раздвижная дверь", form: Form::SlidingDoor, material: 6, category: "Двери", base: 1820, id: 1820 },
    Entry { name: "Медь · Раздвижная дверь", form: Form::SlidingDoor, material: 7, category: "Двери", base: 1824, id: 1824 },
    Entry { name: "Стекло · Раздвижная дверь", form: Form::SlidingDoor, material: 8, category: "Двери", base: 1828, id: 1828 },
    Entry { name: "Дуб · Раздвижная дверь (открыта)", form: Form::SlidingDoorOpen, material: 0, category: "Двери", base: 1808, id: 1832 },
    Entry { name: "Берёза · Раздвижная дверь (открыта)", form: Form::SlidingDoorOpen, material: 1, category: "Двери", base: 1812, id: 1836 },
    Entry { name: "Ель · Раздвижная дверь (открыта)", form: Form::SlidingDoorOpen, material: 2, category: "Двери", base: 1816, id: 1840 },
    Entry { name: "Железо · Раздвижная дверь (открыта)", form: Form::SlidingDoorOpen, material: 6, category: "Двери", base: 1820, id: 1844 },
    Entry { name: "Медь · Раздвижная дверь (открыта)", form: Form::SlidingDoorOpen, material: 7, category: "Двери", base: 1824, id: 1848 },
    Entry { name: "Стекло · Раздвижная дверь (открыта)", form: Form::SlidingDoorOpen, material: 8, category: "Двери", base: 1828, id: 1852 },
    Entry { name: "Дуб · Раздвижная дверь (верх)", form: Form::SlidingDoorTop, material: 0, category: "Двери", base: 1808, id: 1856 },
    Entry { name: "Берёза · Раздвижная дверь (верх)", form: Form::SlidingDoorTop, material: 1, category: "Двери", base: 1812, id: 1860 },
    Entry { name: "Ель · Раздвижная дверь (верх)", form: Form::SlidingDoorTop, material: 2, category: "Двери", base: 1816, id: 1864 },
    Entry { name: "Железо · Раздвижная дверь (верх)", form: Form::SlidingDoorTop, material: 6, category: "Двери", base: 1820, id: 1868 },
    Entry { name: "Медь · Раздвижная дверь (верх)", form: Form::SlidingDoorTop, material: 7, category: "Двери", base: 1824, id: 1872 },
    Entry { name: "Стекло · Раздвижная дверь (верх)", form: Form::SlidingDoorTop, material: 8, category: "Двери", base: 1828, id: 1876 },
    Entry { name: "Дуб · Раздвижная дверь (верх открыт)", form: Form::SlidingDoorTopOpen, material: 0, category: "Двери", base: 1808, id: 1880 },
    Entry { name: "Берёза · Раздвижная дверь (верх открыт)", form: Form::SlidingDoorTopOpen, material: 1, category: "Двери", base: 1812, id: 1884 },
    Entry { name: "Ель · Раздвижная дверь (верх открыт)", form: Form::SlidingDoorTopOpen, material: 2, category: "Двери", base: 1816, id: 1888 },
    Entry { name: "Железо · Раздвижная дверь (верх открыт)", form: Form::SlidingDoorTopOpen, material: 6, category: "Двери", base: 1820, id: 1892 },
    Entry { name: "Медь · Раздвижная дверь (верх открыт)", form: Form::SlidingDoorTopOpen, material: 7, category: "Двери", base: 1824, id: 1896 },
    Entry { name: "Стекло · Раздвижная дверь (верх открыт)", form: Form::SlidingDoorTopOpen, material: 8, category: "Двери", base: 1828, id: 1900 },
    Entry { name: "Дуб · Складная дверь", form: Form::FoldingDoor, material: 0, category: "Двери", base: 1904, id: 1904 },
    Entry { name: "Берёза · Складная дверь", form: Form::FoldingDoor, material: 1, category: "Двери", base: 1908, id: 1908 },
    Entry { name: "Ель · Складная дверь", form: Form::FoldingDoor, material: 2, category: "Двери", base: 1912, id: 1912 },
    Entry { name: "Железо · Складная дверь", form: Form::FoldingDoor, material: 6, category: "Двери", base: 1916, id: 1916 },
    Entry { name: "Медь · Складная дверь", form: Form::FoldingDoor, material: 7, category: "Двери", base: 1920, id: 1920 },
    Entry { name: "Стекло · Складная дверь", form: Form::FoldingDoor, material: 8, category: "Двери", base: 1924, id: 1924 },
    Entry { name: "Дуб · Складная дверь (открыта)", form: Form::FoldingDoorOpen, material: 0, category: "Двери", base: 1904, id: 1928 },
    Entry { name: "Берёза · Складная дверь (открыта)", form: Form::FoldingDoorOpen, material: 1, category: "Двери", base: 1908, id: 1932 },
    Entry { name: "Ель · Складная дверь (открыта)", form: Form::FoldingDoorOpen, material: 2, category: "Двери", base: 1912, id: 1936 },
    Entry { name: "Железо · Складная дверь (открыта)", form: Form::FoldingDoorOpen, material: 6, category: "Двери", base: 1916, id: 1940 },
    Entry { name: "Медь · Складная дверь (открыта)", form: Form::FoldingDoorOpen, material: 7, category: "Двери", base: 1920, id: 1944 },
    Entry { name: "Стекло · Складная дверь (открыта)", form: Form::FoldingDoorOpen, material: 8, category: "Двери", base: 1924, id: 1948 },
    Entry { name: "Дуб · Складная дверь (верх)", form: Form::FoldingDoorTop, material: 0, category: "Двери", base: 1904, id: 1952 },
    Entry { name: "Берёза · Складная дверь (верх)", form: Form::FoldingDoorTop, material: 1, category: "Двери", base: 1908, id: 1956 },
    Entry { name: "Ель · Складная дверь (верх)", form: Form::FoldingDoorTop, material: 2, category: "Двери", base: 1912, id: 1960 },
    Entry { name: "Железо · Складная дверь (верх)", form: Form::FoldingDoorTop, material: 6, category: "Двери", base: 1916, id: 1964 },
    Entry { name: "Медь · Складная дверь (верх)", form: Form::FoldingDoorTop, material: 7, category: "Двери", base: 1920, id: 1968 },
    Entry { name: "Стекло · Складная дверь (верх)", form: Form::FoldingDoorTop, material: 8, category: "Двери", base: 1924, id: 1972 },
    Entry { name: "Дуб · Складная дверь (верх открыт)", form: Form::FoldingDoorTopOpen, material: 0, category: "Двери", base: 1904, id: 1976 },
    Entry { name: "Берёза · Складная дверь (верх открыт)", form: Form::FoldingDoorTopOpen, material: 1, category: "Двери", base: 1908, id: 1980 },
    Entry { name: "Ель · Складная дверь (верх открыт)", form: Form::FoldingDoorTopOpen, material: 2, category: "Двери", base: 1912, id: 1984 },
    Entry { name: "Железо · Складная дверь (верх открыт)", form: Form::FoldingDoorTopOpen, material: 6, category: "Двери", base: 1916, id: 1988 },
    Entry { name: "Медь · Складная дверь (верх открыт)", form: Form::FoldingDoorTopOpen, material: 7, category: "Двери", base: 1920, id: 1992 },
    Entry { name: "Стекло · Складная дверь (верх открыт)", form: Form::FoldingDoorTopOpen, material: 8, category: "Двери", base: 1924, id: 1996 },
    Entry { name: "Дуб · Стул", form: Form::Chair, material: 0, category: "Мебель", base: 2000, id: 2000 },
    Entry { name: "Берёза · Стул", form: Form::Chair, material: 1, category: "Мебель", base: 2004, id: 2004 },
    Entry { name: "Ель · Стул", form: Form::Chair, material: 2, category: "Мебель", base: 2008, id: 2008 },
    Entry { name: "Железо · Стул", form: Form::Chair, material: 6, category: "Мебель", base: 2012, id: 2012 },
    Entry { name: "Медь · Стул", form: Form::Chair, material: 7, category: "Мебель", base: 2016, id: 2016 },
    Entry { name: "Дуб · Стол", form: Form::Table, material: 0, category: "Мебель", base: 2020, id: 2020 },
    Entry { name: "Берёза · Стол", form: Form::Table, material: 1, category: "Мебель", base: 2024, id: 2024 },
    Entry { name: "Ель · Стол", form: Form::Table, material: 2, category: "Мебель", base: 2028, id: 2028 },
    Entry { name: "Железо · Стол", form: Form::Table, material: 6, category: "Мебель", base: 2032, id: 2032 },
    Entry { name: "Медь · Стол", form: Form::Table, material: 7, category: "Мебель", base: 2036, id: 2036 },
    Entry { name: "Дуб · Стол со стеклянной столешницей", form: Form::GlassTable, material: 0, category: "Мебель", base: 2040, id: 2040 },
    Entry { name: "Берёза · Стол со стеклянной столешницей", form: Form::GlassTable, material: 1, category: "Мебель", base: 2044, id: 2044 },
    Entry { name: "Ель · Стол со стеклянной столешницей", form: Form::GlassTable, material: 2, category: "Мебель", base: 2048, id: 2048 },
    Entry { name: "Железо · Стол со стеклянной столешницей", form: Form::GlassTable, material: 6, category: "Мебель", base: 2052, id: 2052 },
    Entry { name: "Медь · Стол со стеклянной столешницей", form: Form::GlassTable, material: 7, category: "Мебель", base: 2056, id: 2056 },
    Entry { name: "Дуб · Диван", form: Form::Sofa, material: 0, category: "Мебель", base: 2060, id: 2060 },
    Entry { name: "Берёза · Диван", form: Form::Sofa, material: 1, category: "Мебель", base: 2064, id: 2064 },
    Entry { name: "Ель · Диван", form: Form::Sofa, material: 2, category: "Мебель", base: 2068, id: 2068 },
    Entry { name: "Железо · Диван", form: Form::Sofa, material: 6, category: "Мебель", base: 2072, id: 2072 },
    Entry { name: "Медь · Диван", form: Form::Sofa, material: 7, category: "Мебель", base: 2076, id: 2076 },
    Entry { name: "Дуб · Угловой диван", form: Form::SofaCorner, material: 0, category: "Мебель", base: 2080, id: 2080 },
    Entry { name: "Берёза · Угловой диван", form: Form::SofaCorner, material: 1, category: "Мебель", base: 2084, id: 2084 },
    Entry { name: "Ель · Угловой диван", form: Form::SofaCorner, material: 2, category: "Мебель", base: 2088, id: 2088 },
    Entry { name: "Железо · Угловой диван", form: Form::SofaCorner, material: 6, category: "Мебель", base: 2092, id: 2092 },
    Entry { name: "Медь · Угловой диван", form: Form::SofaCorner, material: 7, category: "Мебель", base: 2096, id: 2096 },
    Entry { name: "Дуб · Кровать", form: Form::Bed, material: 0, category: "Мебель", base: 2100, id: 2100 },
    Entry { name: "Берёза · Кровать", form: Form::Bed, material: 1, category: "Мебель", base: 2104, id: 2104 },
    Entry { name: "Ель · Кровать", form: Form::Bed, material: 2, category: "Мебель", base: 2108, id: 2108 },
    Entry { name: "Железо · Кровать", form: Form::Bed, material: 6, category: "Мебель", base: 2112, id: 2112 },
    Entry { name: "Медь · Кровать", form: Form::Bed, material: 7, category: "Мебель", base: 2116, id: 2116 },
    Entry { name: "Дуб · Шкаф", form: Form::Wardrobe, material: 0, category: "Мебель", base: 2120, id: 2120 },
    Entry { name: "Берёза · Шкаф", form: Form::Wardrobe, material: 1, category: "Мебель", base: 2124, id: 2124 },
    Entry { name: "Ель · Шкаф", form: Form::Wardrobe, material: 2, category: "Мебель", base: 2128, id: 2128 },
    Entry { name: "Железо · Шкаф", form: Form::Wardrobe, material: 6, category: "Мебель", base: 2132, id: 2132 },
    Entry { name: "Медь · Шкаф", form: Form::Wardrobe, material: 7, category: "Мебель", base: 2136, id: 2136 },
    Entry { name: "Дуб · Полка", form: Form::Shelf, material: 0, category: "Мебель", base: 2140, id: 2140 },
    Entry { name: "Берёза · Полка", form: Form::Shelf, material: 1, category: "Мебель", base: 2144, id: 2144 },
    Entry { name: "Ель · Полка", form: Form::Shelf, material: 2, category: "Мебель", base: 2148, id: 2148 },
    Entry { name: "Железо · Полка", form: Form::Shelf, material: 6, category: "Мебель", base: 2152, id: 2152 },
    Entry { name: "Медь · Полка", form: Form::Shelf, material: 7, category: "Мебель", base: 2156, id: 2156 },
    Entry { name: "Дуб · Тумба", form: Form::Nightstand, material: 0, category: "Мебель", base: 2160, id: 2160 },
    Entry { name: "Берёза · Тумба", form: Form::Nightstand, material: 1, category: "Мебель", base: 2164, id: 2164 },
    Entry { name: "Ель · Тумба", form: Form::Nightstand, material: 2, category: "Мебель", base: 2168, id: 2168 },
    Entry { name: "Железо · Тумба", form: Form::Nightstand, material: 6, category: "Мебель", base: 2172, id: 2172 },
    Entry { name: "Медь · Тумба", form: Form::Nightstand, material: 7, category: "Мебель", base: 2176, id: 2176 },
    Entry { name: "Дуб · Кухонный шкаф", form: Form::KitchenBase, material: 0, category: "Мебель", base: 2180, id: 2180 },
    Entry { name: "Берёза · Кухонный шкаф", form: Form::KitchenBase, material: 1, category: "Мебель", base: 2184, id: 2184 },
    Entry { name: "Ель · Кухонный шкаф", form: Form::KitchenBase, material: 2, category: "Мебель", base: 2188, id: 2188 },
    Entry { name: "Железо · Кухонный шкаф", form: Form::KitchenBase, material: 6, category: "Мебель", base: 2192, id: 2192 },
    Entry { name: "Медь · Кухонный шкаф", form: Form::KitchenBase, material: 7, category: "Мебель", base: 2196, id: 2196 },
    Entry { name: "Дуб · Кухонная столешница", form: Form::Counter, material: 0, category: "Мебель", base: 2200, id: 2200 },
    Entry { name: "Берёза · Кухонная столешница", form: Form::Counter, material: 1, category: "Мебель", base: 2204, id: 2204 },
    Entry { name: "Ель · Кухонная столешница", form: Form::Counter, material: 2, category: "Мебель", base: 2208, id: 2208 },
    Entry { name: "Железо · Кухонная столешница", form: Form::Counter, material: 6, category: "Мебель", base: 2212, id: 2212 },
    Entry { name: "Медь · Кухонная столешница", form: Form::Counter, material: 7, category: "Мебель", base: 2216, id: 2216 },
    Entry { name: "Дуб · Мойка", form: Form::Sink, material: 0, category: "Мебель", base: 2220, id: 2220 },
    Entry { name: "Берёза · Мойка", form: Form::Sink, material: 1, category: "Мебель", base: 2224, id: 2224 },
    Entry { name: "Ель · Мойка", form: Form::Sink, material: 2, category: "Мебель", base: 2228, id: 2228 },
    Entry { name: "Железо · Мойка", form: Form::Sink, material: 6, category: "Мебель", base: 2232, id: 2232 },
    Entry { name: "Медь · Мойка", form: Form::Sink, material: 7, category: "Мебель", base: 2236, id: 2236 },
    Entry { name: "Дуб · Торшер", form: Form::FloorLamp, material: 0, category: "Декор", base: 2240, id: 2240 },
    Entry { name: "Берёза · Торшер", form: Form::FloorLamp, material: 1, category: "Декор", base: 2244, id: 2244 },
    Entry { name: "Ель · Торшер", form: Form::FloorLamp, material: 2, category: "Декор", base: 2248, id: 2248 },
    Entry { name: "Железо · Торшер", form: Form::FloorLamp, material: 6, category: "Декор", base: 2252, id: 2252 },
    Entry { name: "Дуб · Штора", form: Form::Curtain, material: 0, category: "Декор", base: 2256, id: 2256 },
    Entry { name: "Берёза · Штора", form: Form::Curtain, material: 1, category: "Декор", base: 2260, id: 2260 },
    Entry { name: "Ель · Штора", form: Form::Curtain, material: 2, category: "Декор", base: 2264, id: 2264 },
    Entry { name: "Железо · Штора", form: Form::Curtain, material: 6, category: "Декор", base: 2268, id: 2268 },
    Entry { name: "Дуб · Ковёр", form: Form::Rug, material: 0, category: "Декор", base: 2272, id: 2272 },
    Entry { name: "Берёза · Ковёр", form: Form::Rug, material: 1, category: "Декор", base: 2276, id: 2276 },
    Entry { name: "Ель · Ковёр", form: Form::Rug, material: 2, category: "Декор", base: 2280, id: 2280 },
    Entry { name: "Железо · Ковёр", form: Form::Rug, material: 6, category: "Декор", base: 2284, id: 2284 },
    Entry { name: "Дуб · Зеркало", form: Form::Mirror, material: 0, category: "Декор", base: 2288, id: 2288 },
    Entry { name: "Берёза · Зеркало", form: Form::Mirror, material: 1, category: "Декор", base: 2292, id: 2292 },
    Entry { name: "Ель · Зеркало", form: Form::Mirror, material: 2, category: "Декор", base: 2296, id: 2296 },
    Entry { name: "Железо · Зеркало", form: Form::Mirror, material: 6, category: "Декор", base: 2300, id: 2300 },
    Entry { name: "Дуб · Горшок с растением", form: Form::FlowerPot, material: 0, category: "Декор", base: 2304, id: 2304 },
    Entry { name: "Берёза · Горшок с растением", form: Form::FlowerPot, material: 1, category: "Декор", base: 2308, id: 2308 },
    Entry { name: "Ель · Горшок с растением", form: Form::FlowerPot, material: 2, category: "Декор", base: 2312, id: 2312 },
    Entry { name: "Железо · Горшок с растением", form: Form::FlowerPot, material: 6, category: "Декор", base: 2316, id: 2316 },
    Entry { name: "Дуб · Сундук", form: Form::StorageChest, material: 0, category: "Хранилища", base: 2320, id: 2320 },
    Entry { name: "Берёза · Сундук", form: Form::StorageChest, material: 1, category: "Хранилища", base: 2324, id: 2324 },
    Entry { name: "Ель · Сундук", form: Form::StorageChest, material: 2, category: "Хранилища", base: 2328, id: 2328 },
    Entry { name: "Железо · Сундук", form: Form::StorageChest, material: 6, category: "Хранилища", base: 2332, id: 2332 },
    Entry { name: "Дуб · Бочка", form: Form::Barrel, material: 0, category: "Хранилища", base: 2336, id: 2336 },
    Entry { name: "Берёза · Бочка", form: Form::Barrel, material: 1, category: "Хранилища", base: 2340, id: 2340 },
    Entry { name: "Ель · Бочка", form: Form::Barrel, material: 2, category: "Хранилища", base: 2344, id: 2344 },
    Entry { name: "Дуб · Путевой ящик", form: Form::PortableCrate, material: 0, category: "Хранилища", base: 2348, id: 2348 },
    Entry { name: "Берёза · Путевой ящик", form: Form::PortableCrate, material: 1, category: "Хранилища", base: 2352, id: 2352 },
    Entry { name: "Ель · Путевой ящик", form: Form::PortableCrate, material: 2, category: "Хранилища", base: 2356, id: 2356 },
    Entry { name: "Железо · Путевой ящик", form: Form::PortableCrate, material: 6, category: "Хранилища", base: 2360, id: 2360 },
    Entry { name: "Железо · Резонансный сундук", form: Form::SharedChest, material: 6, category: "Хранилища", base: 2364, id: 2364 },
    Entry { name: "Дуб · Рамка для предмета", form: Form::ItemFrame, material: 0, category: "Декор", base: 2368, id: 2368 },
    Entry { name: "Берёза · Рамка для предмета", form: Form::ItemFrame, material: 1, category: "Декор", base: 2372, id: 2372 },
    Entry { name: "Ель · Рамка для предмета", form: Form::ItemFrame, material: 2, category: "Декор", base: 2376, id: 2376 },
    Entry { name: "Дуб · Картина «Чаща»", form: Form::PaintingForest, material: 0, category: "Декор", base: 2380, id: 2380 },
    Entry { name: "Берёза · Картина «Чаща»", form: Form::PaintingForest, material: 1, category: "Декор", base: 2384, id: 2384 },
    Entry { name: "Ель · Картина «Чаща»", form: Form::PaintingForest, material: 2, category: "Декор", base: 2388, id: 2388 },
    Entry { name: "Дуб · Картина «Небо»", form: Form::PaintingSky, material: 0, category: "Декор", base: 2392, id: 2392 },
    Entry { name: "Берёза · Картина «Небо»", form: Form::PaintingSky, material: 1, category: "Декор", base: 2396, id: 2396 },
    Entry { name: "Ель · Картина «Небо»", form: Form::PaintingSky, material: 2, category: "Декор", base: 2400, id: 2400 },
    Entry { name: "Дуб · Картина «Искрит»", form: Form::PaintingAbstract, material: 0, category: "Декор", base: 2404, id: 2404 },
    Entry { name: "Берёза · Картина «Искрит»", form: Form::PaintingAbstract, material: 1, category: "Декор", base: 2408, id: 2408 },
    Entry { name: "Ель · Картина «Искрит»", form: Form::PaintingAbstract, material: 2, category: "Декор", base: 2412, id: 2412 },
    Entry { name: "Дуб · Верстак", form: Form::Workbench, material: 0, category: "Строительство", base: 20, id: 2416 },
    Entry { name: "Железо · Печь", form: Form::Furnace, material: 6, category: "Техника", base: 80, id: 2420 },
    Entry { name: "Железо · Терминал", form: Form::Terminal, material: 6, category: "Техника", base: 85, id: 2424 },
    Entry { name: "Железо · Командный блок", form: Form::Command, material: 6, category: "Техника", base: 84, id: 2428 },
    Entry { name: "Камень · Каменный алтарь", form: Form::StoneAltar, material: 3, category: "Техника", base: 2432, id: 2432 },
    Entry { name: "Дуб · Лесной алтарь", form: Form::RootAltar, material: 0, category: "Техника", base: 2436, id: 2436 },
    Entry { name: "Железо · Пепельный алтарь", form: Form::EmberAltar, material: 6, category: "Техника", base: 2440, id: 2440 },
];
pub fn entry(id: u16) -> Option<(&'static Entry, u8)> {
    let n = id.checked_sub(FIRST_ID)? as usize;
    ENTRIES.get(n / 4).map(|e| (e, (n % 4) as u8))
}
pub fn block(form: Form, material: usize, rotation: u8) -> Option<Block> {
    ENTRIES.iter().find(|e| e.form == form && e.material == material).map(|e| Block::from_id(e.id + (rotation % 4) as u16))
}
pub const fn block_info(name: &'static str, material: usize, form: Form) -> BlockInfo {
    let m = MATERIALS[material];
    BlockInfo { name, render: RenderKind::Shape, solid: !matches!(form, Form::Curtain | Form::ItemFrame | Form::PaintingForest | Form::PaintingSky | Form::PaintingAbstract), hardness: if m.glass { 0.6 } else { 3.0 }, tool: if material < 3 { ToolKind::Axe } else { ToolKind::Pickaxe }, needs_tool: false, min_tier: 0, tiles: [m.tile; 6], emissive: matches!(form, Form::FloorLamp | Form::SharedChest | Form::StoneAltar | Form::RootAltar | Form::EmberAltar) }
}
pub fn door_toggle(b: Block) -> Option<Block> {
    let (e, r) = entry(b as u16)?;
    use Form::*;
    let form = match e.form {
        Door => DoorOpen, DoorOpen => Door, DoorTop => DoorTopOpen, DoorTopOpen => DoorTop,
        SlidingDoor => SlidingDoorOpen, SlidingDoorOpen => SlidingDoor, SlidingDoorTop => SlidingDoorTopOpen, SlidingDoorTopOpen => SlidingDoorTop,
        FoldingDoor => FoldingDoorOpen, FoldingDoorOpen => FoldingDoor, FoldingDoorTop => FoldingDoorTopOpen, FoldingDoorTopOpen => FoldingDoorTop,
        _ => return None
    };
    block(form, e.material, r)
}
pub fn door_top(b: Block) -> Option<Block> {
    let (e, r) = entry(b as u16)?;
    use Form::*;
    let form = match e.form { Door => DoorTop, DoorOpen => DoorTopOpen, SlidingDoor => SlidingDoorTop, SlidingDoorOpen => SlidingDoorTopOpen, FoldingDoor => FoldingDoorTop, FoldingDoorOpen => FoldingDoorTopOpen, _ => return None };
    block(form, e.material, r)
}
pub fn is_door_top(b: Block) -> bool { entry(b as u16).is_some_and(|(e,_)| matches!(e.form, Form::DoorTop|Form::DoorTopOpen|Form::SlidingDoorTop|Form::SlidingDoorTopOpen|Form::FoldingDoorTop|Form::FoldingDoorTopOpen)) }
/// Retired pieces keep their save IDs and geometry, but are no longer offered or crafted.
pub fn available(e: &Entry) -> bool {
    !(MATERIALS[e.material].glass && matches!(e.form,Form::Arch|Form::HalfArch|Form::Arc15Small|Form::Arc30Small|Form::Arc45Small|Form::Arc90Small|Form::Arc15Medium|Form::Arc30Medium|Form::Arc45Medium|Form::Arc90Medium|Form::Arc15Large|Form::Arc30Large|Form::Arc45Large|Form::Arc90Large|Form::Dome|Form::DomeHalf|Form::DomeQuarter))
}

pub fn recipe(id: u16) -> Option<crate::inventory::Recipe> {
    let (e, _) = entry(id)?;
    if !available(e) {return None;}
    if matches!(e.form,Form::SharedChest) { return Some(crate::inventory::Recipe {ingredients:vec![(Block::Obsidian as u16,8),(crate::items::ids::RESONANCE_RUNE,1)],output:(e.base,1),needs_table:true,category:e.category,shape:None}); }
    let mut ingredients = vec![(MATERIALS[e.material].ingredient, if e.category == "Мебель" || e.category == "Двери" { 4 } else { 2 })];
    if matches!(e.form, Form::GlassTable | Form::Mirror | Form::Sink) { ingredients.push((Block::Glass as u16, 1)); }
    if matches!(e.form, Form::Bed | Form::Sofa | Form::SofaCorner | Form::Curtain | Form::Rug) { ingredients.push((Block::Wool as u16, 2)); }
    if matches!(e.form, Form::FloorLamp) { ingredients.push((Block::Lamp as u16, 1)); }
    if matches!(e.form,Form::StoneAltar|Form::RootAltar|Form::EmberAltar) { ingredients.push((crate::items::ids::RITUAL_SEAL,1)); }
    Some(crate::inventory::Recipe { ingredients, output: (e.base, 1), needs_table: true, category: e.category, shape: None })
}

/// Tile, alpha-blended pass and emission for one part of a multi-material object.
pub fn surface(e: &Entry, s: geometry::Surface) -> (u16, bool, bool) {
    use geometry::Surface::*;
    match s {
        Main if e.form == Form::SharedChest => (t::OBSIDIAN,false,false),
        Main => (MATERIALS[e.material].tile, MATERIALS[e.material].glass, false),
        geometry::Surface::Front => (match e.form {Form::Furnace=>t::FURNACE_FRONT,Form::Terminal=>t::TERMINAL,Form::Command=>t::COMMAND_BLOCK,Form::Workbench=>t::TABLE_SIDE,_=>MATERIALS[e.material].tile},false,false),
        geometry::Surface::Top => (if e.form==Form::Workbench {t::TABLE_TOP}else{MATERIALS[e.material].tile},false,false),
        geometry::Surface::Picture => (match e.form {Form::PaintingForest=>t::BUILD_PICTURE_FOREST,Form::PaintingSky=>t::BUILD_PICTURE_SKY,_=>t::BUILD_PICTURE_ISKRIT},false,false),
        Fabric => (t::BUILD_FABRIC, false, false),
        Glass => (t::BUILD_GLASS, true, false),
        Metal => (t::IRON_BLOCK, false, false),
        Light => (t::LAMP, false, true),
        Mirror => (t::BUILD_MIRROR, false, false),
        Ceramic => (t::BUILD_CERAMIC, false, false),
        Plant => (t::LEAVES, false, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retired_curved_glass_is_not_offered_but_keeps_saved_ids() {
        let palette=crate::items::creative_palette();let mut retired=0;
        for e in ENTRIES.iter().filter(|e|!available(e)) {
            retired+=1;
            assert!(!palette.contains(&e.base));assert!(recipe(e.id).is_none());
            for r in 0..4 {assert_eq!(Block::from_id(e.id+r) as u16,e.id+r);}
        }
        assert!(retired>0);
        assert!(ENTRIES.iter().any(|e|MATERIALS[e.material].glass&&e.form==Form::Door&&available(e)));
    }

    #[test]
    fn catalogue_ids_items_material_costs_and_doors_roundtrip() {
        for e in ENTRIES {
            for r in 0..4 {let b=Block::from_id(e.id+r);assert_eq!(b as u16,e.id+r);assert_eq!(b.base_item() as u16,e.base);assert!(crate::items::is_valid(b as u16));assert_eq!(crate::items::info(b as u16).place,Some(b));}
            if available(e) {let recipe=recipe(e.id).unwrap();assert!(recipe.ingredients.iter().all(|(id,_)|crate::items::is_valid(*id)));} else {assert!(recipe(e.id).is_none());}
            let b=Block::from_id(e.id);
            if let Some(open)=b.door_toggle() {assert_eq!(open.door_toggle(),Some(b));}
            assert_eq!(b.part(),None,"new IDs must not wrap into old mechanisms");
        }
        assert_eq!(crate::items::info(crate::items::ids::STICK).name,"Палка");
        assert!(entry(crate::items::ids::STICK).is_none());
    }
    #[test]
    fn workbench_spends_real_materials() {
        let chair=block(Form::Chair,0,0).unwrap();let recipe=recipe(chair as u16).unwrap();
        let mut inv=crate::inventory::Inventory::default();
        assert!(!inv.craft(&recipe,true));
        for (id,n) in &recipe.ingredients {inv.add(crate::items::ItemStack::new(*id,*n));}
        assert!(!inv.craft(&recipe,false));assert!(inv.craft(&recipe,true));assert_eq!(inv.count(chair as u16),1);
        for (id,_) in &recipe.ingredients {assert_eq!(inv.count(*id),0);}
    }
}

pub fn legacy_kind(b:Block)->Block { match entry(b as u16).map(|(e,_)|e.form) { Some(Form::Workbench)=>Block::CraftingTable,Some(Form::Furnace)=>Block::ElectricFurnace,Some(Form::Terminal)=>Block::Terminal,Some(Form::Command)=>Block::CommandBlock,_=>b } }
pub fn is_container(b:Block)->bool { entry(b as u16).is_some_and(|(e,_)|matches!(e.form,Form::Wardrobe|Form::Nightstand|Form::KitchenBase|Form::StorageChest|Form::Barrel|Form::PortableCrate|Form::SharedChest)) }
pub fn is_portable(b:Block)->bool { entry(b as u16).is_some_and(|(e,_)|e.form==Form::PortableCrate) }
pub fn is_shared(b:Block)->bool { entry(b as u16).is_some_and(|(e,_)|e.form==Form::SharedChest) }
pub fn altar(b:Block)->Option<crate::bosses::BossKind> { match entry(b as u16)?.0.form {Form::StoneAltar=>Some(crate::bosses::BossKind::Stone),Form::RootAltar=>Some(crate::bosses::BossKind::Root),Form::EmberAltar=>Some(crate::bosses::BossKind::Ember),_=>None} }
