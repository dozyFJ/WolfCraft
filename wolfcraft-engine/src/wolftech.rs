//! WolfTech — the tech mod built into WolfCraft (ideas from IC2, Thermal Expansion, Mekanism).
//! Part 1: energy and machines. Energy is counted in ЭЕ (энергоединицы). Generators make it,
//! energy cells store it, copper cables carry it, machines spend it to crush, smelt and enrich.

use crate::block::Block;
use crate::items::{ItemId, ids};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Burns fuel from its slot.
    Generator,
    /// Makes energy in daylight under the open sky.
    Solar,
    /// Stores energy for the network.
    Cell,
    /// Ore → 2 dust; stone → cobble → gravel → sand.
    Crusher,
    /// Smelts like a furnace, faster, on energy.
    Induction,
    /// Ore → 3 dust (the best yield).
    Enricher,
    /// Spins in the wind: more up high under the open sky, much more in a storm.
    Wind,
    /// Draws heat from lava around it (each lava side adds).
    Geo,
    /// Turned by water around it (each water side adds).
    Water,
    /// Splits uranium: a lot of energy for a long time.
    Reactor,
    /// Squeezes: sand → sandstone, snow → ice → packed ice, 32 coal → a diamond.
    Compressor,
    /// Saws: log → 6 planks, planks → 4 sticks.
    Sawmill,
    /// Extracts: bone → 4 bone meal, gravel → flint, leaves → sapling-free mulch (bone meal).
    Extractor,
    /// Thermal-style: lava and water on its sides → cobblestone forever (stone or obsidian
    /// when one lies in the slot as a pattern).
    Extruder,
    /// Thermal-style farm in a box: grows more of the seed or flower in its slot.
    Insolator,
    /// Thermal-style: water on a side → snow (or ice with ice in the slot).
    Glacier,
    /// Thermal-style dynamo: burns magma and netherrack.
    Magma,
    /// «Механика» link: turns rotation into energy.
    Dynamo,
    /// «Механика» link: spends energy to turn shafts.
    Motor,
    /// Digs a 5×5 shaft down under itself, block by block, into its slots.
    Quarry,
    /// Any 4 items → 1 scrap.
    Recycler,
    /// 8 scrap → 1 UU-matter (slow, hungry).
    MatterFab,
    /// Pulls dropped items within 8 blocks into its slots.
    Magnet,
    /// Works at night under the open sky.
    Lunar,
    /// A huge store (and charges gear like the cell).
    BigCell,
    /// Zaps hostile creatures within 7 blocks.
    Tesla,
    /// Charges the player's electric gear while they stand on it.
    ChargePad,
    /// Gravel → flint, netherrack → gunpowder, bones → bone meal, scrap → iron dust…
    Centrifuge,
    /// Draconic Evolution: a huge energy store (50 million) that also charges gear.
    DracoCore,
    /// Draconic Evolution: fusion crafting — draconium → awakened, crystals → cores (hungry).
    Fusion,
    /// Immersive Engineering: coal → coke, logs → coal, with no energy (slow).
    CokeOven,
    /// Immersive Engineering: iron → steel, with no energy (slow).
    BlastFurnace,
    /// Immersive Engineering: ore → 2 ingots straight away, on energy.
    ArcFurnace,
    /// Tinkers' Construct: melts ore into 2 ingots on the heat of lava beside it (no energy);
    /// cobalt → manyullyn with gold.
    Smeltery,
}

/// Energy stores that give back to the net and charge gear in their first slot.
pub fn is_store(k: Kind) -> bool {
    matches!(k, Kind::Cell | Kind::BigCell | Kind::DracoCore)
}

pub struct Spec {
    pub name: &'static str,
    /// Energy it can hold.
    pub capacity: f32,
    /// ЭЕ/s made (generators) or spent while working (machines).
    pub rate: f32,
    /// Seconds per operation (machines).
    pub time: f32,
    pub hint: &'static str,
}

pub fn kind(b: Block) -> Option<Kind> {
    let b = b.machine_dir().map_or(b, |(base, _)| base);
    Some(match b {
        Block::TechGenerator => Kind::Generator,
        Block::SolarPanel => Kind::Solar,
        Block::EnergyCell => Kind::Cell,
        Block::Crusher => Kind::Crusher,
        Block::InductionFurnace => Kind::Induction,
        Block::Enricher => Kind::Enricher,
        Block::WindTurbine => Kind::Wind,
        Block::GeoGenerator => Kind::Geo,
        Block::WaterMill => Kind::Water,
        Block::Reactor => Kind::Reactor,
        Block::Compressor => Kind::Compressor,
        Block::Sawmill => Kind::Sawmill,
        Block::Extractor => Kind::Extractor,
        Block::Extruder => Kind::Extruder,
        Block::Insolator => Kind::Insolator,
        Block::Glacier => Kind::Glacier,
        Block::MagmaGenerator => Kind::Magma,
        Block::KineticDynamo => Kind::Dynamo,
        Block::ElectricMotor => Kind::Motor,
        Block::Quarry => Kind::Quarry,
        Block::Recycler => Kind::Recycler,
        Block::MatterFab => Kind::MatterFab,
        Block::ItemMagnet => Kind::Magnet,
        Block::LunarPanel => Kind::Lunar,
        Block::BigCell => Kind::BigCell,
        Block::TeslaCoil => Kind::Tesla,
        Block::ChargePad => Kind::ChargePad,
        Block::Centrifuge => Kind::Centrifuge,
        Block::DraconicCore => Kind::DracoCore,
        Block::FusionCrafter => Kind::Fusion,
        Block::CokeOven => Kind::CokeOven,
        Block::BlastFurnace => Kind::BlastFurnace,
        Block::ArcFurnace => Kind::ArcFurnace,
        Block::Smeltery => Kind::Smeltery,
        _ => return None,
    })
}

pub fn spec(k: Kind) -> Spec {
    match k {
        Kind::Generator => Spec { name: "Генератор", capacity: 2_000.0, rate: 20.0, time: 0.0, hint: "Положите уголь или дрова" },
        Kind::Solar => Spec { name: "Солнечная панель", capacity: 500.0, rate: 8.0, time: 0.0, hint: "Работает днём под открытым небом" },
        Kind::Cell => Spec { name: "Энергоячейка", capacity: 40_000.0, rate: 0.0, time: 0.0, hint: "Хранит энергию, заряжает снаряжение (положите его слева)" },
        Kind::Crusher => Spec { name: "Дробитель", capacity: 200.0, rate: 10.0, time: 4.0, hint: "Руда → 2 пыли, камень → булыжник → гравий → песок" },
        Kind::Induction => Spec { name: "Индукционная печь", capacity: 160.0, rate: 8.0, time: 2.0, hint: "Плавит быстро, на энергии" },
        Kind::Wind => Spec { name: "Ветрогенератор", capacity: 1_000.0, rate: 12.0, time: 0.0, hint: "Чем выше и чем сильнее непогода, тем больше энергии" },
        Kind::Geo => Spec { name: "Геотермальный генератор", capacity: 2_000.0, rate: 8.0, time: 0.0, hint: "Поставьте рядом с лавой: каждая сторона с лавой — +8 ЭЕ/с" },
        Kind::Water => Spec { name: "Гидрогенератор", capacity: 1_000.0, rate: 3.0, time: 0.0, hint: "Поставьте в воду: каждая сторона с водой — +3 ЭЕ/с" },
        Kind::Reactor => Spec { name: "Ядерный реактор", capacity: 10_000.0, rate: 0.0, time: 0.0, hint: "Урановые стержни дают энергию и жар; теплоотводы и капсулы охлаждают. Перегрев — взрыв!" },
        Kind::Compressor => Spec { name: "Компрессор", capacity: 240.0, rate: 12.0, time: 4.0, hint: "Песок → песчаник, снег → лёд → плотный лёд, 32 угля → алмаз" },
        Kind::Sawmill => Spec { name: "Пилорама", capacity: 160.0, rate: 6.0, time: 1.5, hint: "Бревно → 6 досок, доски → 4 палки" },
        Kind::Extractor => Spec { name: "Экстрактор", capacity: 200.0, rate: 8.0, time: 3.0, hint: "Кость → 4 костной муки, гравий → кремень, листва → костная мука" },
        Kind::Extruder => Spec { name: "Магмовый экструдер", capacity: 200.0, rate: 6.0, time: 1.0, hint: "Нужны лава и вода по бокам → булыжник. Камень или обсидиан в слоте — делает их" },
        Kind::Insolator => Spec { name: "Теплица", capacity: 400.0, rate: 10.0, time: 8.0, hint: "Семена, морковь, картофель или цветок в слоте — растит ещё (сам образец не тратится)" },
        Kind::Glacier => Spec { name: "Ледогенератор", capacity: 200.0, rate: 8.0, time: 2.0, hint: "Нужна вода сбоку → снег. Лёд в слоте — делает лёд" },
        Kind::Magma => Spec { name: "Магматический генератор", capacity: 4_000.0, rate: 40.0, time: 0.0, hint: "Положите магму или адский камень" },
        Kind::Quarry => Spec { name: "Буровая установка", capacity: 6_000.0, rate: 50.0, time: 0.8, hint: "Копает шахту 5×5 вниз под собой; добыча — в слоты (заберите или подведите трубу)" },
        Kind::Recycler => Spec { name: "Утилизатор", capacity: 400.0, rate: 6.0, time: 2.0, hint: "Любые 4 предмета → 1 лом" },
        Kind::MatterFab => Spec { name: "Фабрикатор материи", capacity: 20_000.0, rate: 80.0, time: 12.0, hint: "8 лома → 1 материя (из материи крафтятся алмазы и металлы)" },
        Kind::Magnet => Spec { name: "Магнит предметов", capacity: 400.0, rate: 4.0, time: 0.5, hint: "Притягивает выпавшие предметы в радиусе 8 блоков в свои слоты" },
        Kind::Lunar => Spec { name: "Лунная панель", capacity: 1_000.0, rate: 3.0, time: 0.0, hint: "Даёт энергию ночью под открытым небом (днём спит)" },
        Kind::BigCell => Spec { name: "Энергохранилище", capacity: 1_000_000.0, rate: 0.0, time: 0.0, hint: "Хранит миллион ЭЕ и заряжает снаряжение (положите его слева)" },
        Kind::Tesla => Spec { name: "Тесла-катушка", capacity: 2_000.0, rate: 12.0, time: 1.0, hint: "Бьёт молнией враждебных существ в радиусе 7 блоков (60 ЭЕ за разряд)" },
        Kind::ChargePad => Spec { name: "Зарядная платформа", capacity: 4_000.0, rate: 20.0, time: 0.0, hint: "Встаньте сверху — заряжает электроинструменты и броню" },
        Kind::Centrifuge => Spec { name: "Центрифуга", capacity: 600.0, rate: 10.0, time: 4.0, hint: "Гравий → кремень, адский камень → порох, кости → костная мука, лом → железная пыль" },
        Kind::Dynamo => Spec { name: "Кинетическое динамо", capacity: 1_000.0, rate: 0.0, time: 0.0, hint: "Подведите вал «Механики»: вращение → энергия (быстрее крутится — больше ЭЕ)" },
        Kind::Motor => Spec { name: "Электромотор", capacity: 400.0, rate: 16.0, time: 0.0, hint: "Подключите кабель: на энергии крутит валы «Механики» (64 об/мин)" },
        Kind::DracoCore => Spec { name: "Драконье энергоядро", capacity: 50_000_000.0, rate: 0.0, time: 0.0, hint: "Draconic Evolution: хранит 50 миллионов ЭЕ и заряжает снаряжение (положите его слева)" },
        Kind::Fusion => Spec { name: "Термоядерный крафтер", capacity: 40_000.0, rate: 400.0, time: 10.0, hint: "Draconic Evolution: драконий → пробуждённый, энергокристалл → драконье ядро, алмаз → дракониевая пыль. Очень голоден" },
        Kind::CokeOven => Spec { name: "Коксовая печь", capacity: 0.0, rate: 0.0, time: 12.0, hint: "Immersive Engineering: уголь → кокс, бревно → уголь. Без энергии, медленно" },
        Kind::BlastFurnace => Spec { name: "Доменная печь", capacity: 0.0, rate: 0.0, time: 16.0, hint: "Immersive Engineering: железный слиток → стальной. Без энергии" },
        Kind::ArcFurnace => Spec { name: "Дуговая печь", capacity: 4_000.0, rate: 40.0, time: 2.0, hint: "Immersive Engineering: руда → 2 слитка сразу, лом → сталь" },
        Kind::Smeltery => Spec { name: "Плавильня", capacity: 0.0, rate: 0.0, time: 3.0, hint: "Tinkers' Construct: лава рядом — руда → 2 слитка, кобальт → манюллин. Без энергии" },
        Kind::Enricher => Spec { name: "Обогатитель", capacity: 320.0, rate: 16.0, time: 6.0, hint: "Руда → 3 пыли" },
    }
}

/// Makes energy (generators of every kind).
pub fn is_source(k: Kind) -> bool {
    matches!(k, Kind::Generator | Kind::Solar | Kind::Wind | Kind::Geo | Kind::Water | Kind::Reactor | Kind::Magma | Kind::Dynamo | Kind::Lunar)
}

/// Burns fuel from its slot (the coal generator and the reactor).
pub fn burns(k: Kind) -> bool {
    matches!(k, Kind::Generator | Kind::Reactor | Kind::Magma)
}

/// Slots in the reactor's core (6 × 3), and its hull heat limit before plating.
pub const REACTOR_SLOTS: usize = 18;
pub const REACTOR_COLS: usize = 6;
pub const REACTOR_MAX_HEAT: f32 = 5_000.0;

/// A part that goes into the reactor core.
pub fn is_reactor_part(id: ItemId) -> bool {
    matches!(id, ids::URANIUM_ROD | ids::HEAT_VENT | ids::COOLANT_CELL | ids::REACTOR_PLATING)
}

/// What one second of the reactor does (IC2-like).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReactorTick {
    /// ЭЕ/s made.
    pub power: f32,
    /// Heat added to the hull this second (after vents and capsules).
    pub heat: f32,
    /// Hull heat limit (more with plating).
    pub max_heat: f32,
    pub rods: u32,
}

/// One second of the reactor: every uranium rod pulses once, plus once more for each rod next
/// to it (left, right, above, below in the 6×3 core) — packed rods give more energy, but heat
/// grows faster than energy. Heat vents carry 6 heat/s off the hull each; coolant capsules
/// soak up to 12 heat/s each into themselves (they fill up: their wear); plating raises the
/// hull's limit by 1 000. Rods and full capsules wear (`slots` is changed).
pub fn reactor_second(slots: &mut [Option<crate::items::ItemStack>], hull: f32) -> ReactorTick {
    let n = slots.len().min(REACTOR_SLOTS);
    let id_at = |s: &[Option<crate::items::ItemStack>], i: usize| s.get(i).copied().flatten().map(|x| x.id);
    let rod = |s: &[Option<crate::items::ItemStack>], i: usize| id_at(s, i) == Some(ids::URANIUM_ROD);
    let mut t = ReactorTick { max_heat: REACTOR_MAX_HEAT, ..Default::default() };
    let mut made_heat = 0.0;
    let mut vents = 0.0;
    let mut capsules = Vec::new();
    for i in 0..n {
        match id_at(slots, i) {
            Some(ids::URANIUM_ROD) => {
                let (x, y) = (i % REACTOR_COLS, i / REACTOR_COLS);
                let mut p = 1;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx >= 0 && ny >= 0 && (nx as usize) < REACTOR_COLS && rod(slots, ny as usize * REACTOR_COLS + nx as usize) {
                        p += 1;
                    }
                }
                t.power += 10.0 * p as f32;
                made_heat += 2.0 * (p * (p + 1)) as f32;
                t.rods += 1;
                if let Some(s) = slots[i].as_mut() {
                    s.damage += 1;
                    if s.damage >= 600 {
                        slots[i] = None;
                    }
                }
            }
            Some(ids::HEAT_VENT) => vents += 6.0,
            Some(ids::COOLANT_CELL) => capsules.push(i),
            Some(ids::REACTOR_PLATING) => t.max_heat += 1_000.0,
            _ => {}
        }
    }
    // Heat on the hull: what the rods made, minus the vents (they also cool a hot hull down).
    let mut excess = made_heat - vents;
    // Capsules take what the vents could not.
    for i in capsules {
        if excess <= 0.0 {
            break;
        }
        if let Some(s) = slots[i].as_mut() {
            let room = 2_000u16.saturating_sub(s.damage) as f32;
            let take = excess.min(12.0).min(room);
            s.damage += take.ceil() as u16;
            excess -= take;
        }
    }
    t.heat = if excess < 0.0 { excess.max(-hull) } else { excess };
    t
}

/// Seconds of work one item of fuel gives this generator.
pub fn fuel_for(k: Kind, id: ItemId) -> f32 {
    match k {
        Kind::Generator => fuel(id),
        Kind::Magma => match id {
            x if x == Block::Magma as ItemId => 40.0,
            x if x == Block::Netherrack as ItemId => 8.0,
            _ => 0.0,
        },
        Kind::Reactor => 0.0,
        _ => 0.0,
    }
}

pub fn is_machine(k: Kind) -> bool {
    matches!(k, Kind::Crusher | Kind::Induction | Kind::Enricher | Kind::Compressor | Kind::Sawmill | Kind::Extractor | Kind::Extruder | Kind::Insolator | Kind::Glacier | Kind::Recycler | Kind::MatterFab | Kind::Centrifuge | Kind::Fusion | Kind::CokeOven | Kind::BlastFurnace | Kind::ArcFurnace | Kind::Smeltery)
}

/// Machines whose slot item is only a pattern: it stays, the output is made from nothing
/// (the extruder, the greenhouse, the ice maker).
pub fn keeps(k: Kind) -> bool {
    matches!(k, Kind::Extruder | Kind::Insolator | Kind::Glacier)
}

/// What a machine makes with nothing in its slot (the extruder: cobblestone).
pub fn idle_recipe(k: Kind) -> Option<(ItemId, u8)> {
    match k {
        Kind::Extruder => Some((Block::Cobblestone as ItemId, 1)),
        Kind::Glacier => Some((Block::Snow as ItemId, 1)),
        _ => None,
    }
}

/// Whether the surroundings let it work (`env` is filled once a second by the game:
/// 1 when lava and water touch the extruder, the number of water sides for the ice maker).
pub fn env_ok(k: Kind, env: f32) -> bool {
    match k {
        Kind::Extruder | Kind::Glacier | Kind::Smeltery => env > 0.0,
        _ => true,
    }
}

/// How many times longer one operation takes for this input (obsidian is slow).
pub fn slow(k: Kind, input: ItemId) -> f32 {
    match k {
        Kind::Extruder if input == Block::Obsidian as ItemId => 12.0,
        Kind::Extruder if input == Block::Stone as ItemId => 2.0,
        _ => 1.0,
    }
}

/// How many of `input` one operation takes (most take one).
pub fn need(k: Kind, input: ItemId) -> u8 {
    match (k, input) {
        (Kind::Compressor, ids::COAL) => 32,
        (Kind::Recycler, _) => 4,
        (Kind::MatterFab, _) => 8,
        (Kind::Fusion, ids::DRACONIUM_INGOT) => 4,
        (Kind::Smeltery, ids::COBALT_INGOT) => 2,
        _ => 1,
    }
}

/// Blocks that carry energy between machines: cables and the machines themselves.
pub fn conducts(b: Block) -> bool {
    b == Block::Cable || kind(b).is_some()
}

fn raw_dust(id: ItemId) -> Option<ItemId> {
    Some(match id {
        ids::RAW_IRON => ids::IRON_DUST,
        ids::RAW_GOLD => ids::GOLD_DUST,
        ids::RAW_COPPER => ids::COPPER_DUST,
        ids::RAW_TIN => ids::TIN_DUST,
        ids::RAW_SILVER => ids::SILVER_DUST,
        x if x == Block::IronOre as ItemId => ids::IRON_DUST,
        x if x == Block::GoldOre as ItemId => ids::GOLD_DUST,
        x if x == Block::CopperOre as ItemId => ids::COPPER_DUST,
        x if x == Block::TinOre as ItemId => ids::TIN_DUST,
        x if x == Block::SilverOre as ItemId => ids::SILVER_DUST,
        x if x == Block::DraconiumOre as ItemId => ids::DRACONIUM_DUST,
        _ => return None,
    })
}

/// What a machine makes from one `input` item: (item, count).
pub fn recipe(k: Kind, input: ItemId) -> Option<(ItemId, u8)> {
    match k {
        Kind::Crusher => {
            if let Some(d) = raw_dust(input) {
                return Some((d, 2));
            }
            let b = |x: Block| x as ItemId;
            Some(match input {
                x if x == b(Block::Stone) => (b(Block::Cobblestone), 1),
                x if x == b(Block::Cobblestone) => (b(Block::Gravel), 1),
                x if x == b(Block::Gravel) => (b(Block::Sand), 1),
                x if x == b(Block::CoalOre) => (ids::COAL, 3),
                _ => return None,
            })
        }
        Kind::Enricher => raw_dust(input).map(|d| (d, 3)),
        Kind::Compressor => {
            let b = |x: Block| x as ItemId;
            Some(match input {
                x if x == b(Block::Sand) => (b(Block::Sandstone), 1),
                x if x == b(Block::Snow) => (b(Block::Ice), 1),
                x if x == b(Block::Ice) => (b(Block::PackedIce), 1),
                ids::COAL => (ids::DIAMOND, 1),
                _ => return None,
            })
        }
        Kind::Sawmill => {
            let b = |x: Block| x as ItemId;
            Some(match input {
                x if x == b(Block::OakLog) => (b(Block::OakPlanks), 6),
                x if x == b(Block::BirchLog) => (b(Block::BirchPlanks), 6),
                x if x == b(Block::SpruceLog) => (b(Block::SprucePlanks), 6),
                x if x == b(Block::AcaciaLog) => (b(Block::AcaciaPlanks), 6),
                x if [Block::OakPlanks, Block::BirchPlanks, Block::SprucePlanks, Block::AcaciaPlanks].iter().any(|p| b(*p) == x) => (ids::STICK, 4),
                _ => return None,
            })
        }
        Kind::Extractor => {
            let b = |x: Block| x as ItemId;
            Some(match input {
                ids::BONE => (ids::BONE_MEAL, 4),
                x if x == b(Block::Gravel) => (ids::FLINT, 1),
                x if [Block::OakLeaves, Block::BirchLeaves, Block::SpruceLeaves, Block::AcaciaLeaves].iter().any(|l| b(*l) == x) => (ids::BONE_MEAL, 1),
                _ => return None,
            })
        }
        Kind::Induction => crate::interaction::smelt_result(input).map(|o| (o, 1)),
        Kind::Extruder => [Block::Cobblestone, Block::Stone, Block::Obsidian].iter().find(|b| **b as ItemId == input).map(|b| (*b as ItemId, 1)),
        Kind::Glacier => [Block::Snow, Block::Ice].iter().find(|b| **b as ItemId == input).map(|b| (*b as ItemId, 1)),
        Kind::Centrifuge => match input {
            x if x == Block::Gravel as ItemId => Some((ids::FLINT, 2)),
            x if x == Block::Netherrack as ItemId => Some((ids::GUNPOWDER, 1)),
            x if x == Block::Sand as ItemId => Some((ids::GOLD_DUST, 1)),
            ids::BONE => Some((ids::BONE_MEAL, 4)),
            ids::SCRAP => Some((ids::IRON_DUST, 1)),
            _ => None,
        },
        Kind::Fusion => match input {
            ids::DRACONIUM_INGOT => Some((ids::AWAKENED_DRACONIUM, 1)),
            ids::ENERGY_CRYSTAL => Some((ids::DRACONIC_CORE, 1)),
            ids::DIAMOND => Some((ids::DRACONIUM_DUST, 2)),
            _ => None,
        },
        Kind::CokeOven => match input {
            ids::COAL => Some((ids::COKE, 1)),
            x if [Block::OakLog, Block::BirchLog, Block::SpruceLog, Block::AcaciaLog].iter().any(|l| *l as ItemId == x) => Some((ids::COAL, 1)),
            _ => None,
        },
        Kind::BlastFurnace => (input == ids::IRON_INGOT).then_some((ids::STEEL_INGOT, 1)),
        Kind::ArcFurnace | Kind::Smeltery => {
            if k == Kind::Smeltery && input == ids::COBALT_INGOT {
                return Some((ids::MANYULLYN_INGOT, 1));
            }
            if k == Kind::ArcFurnace && input == ids::SCRAP {
                return Some((ids::STEEL_INGOT, 1));
            }
            if input == Block::DraconiumOre as ItemId {
                return Some((ids::DRACONIUM_INGOT, 2));
            }
            if input == Block::CobaltOre as ItemId {
                return Some((ids::COBALT_INGOT, 2));
            }
            raw_dust(input).and_then(crate::interaction::smelt_result).map(|i| (i, 2))
        }
        Kind::Recycler => (input != ids::SCRAP && input != ids::UU_MATTER).then_some((ids::SCRAP, 1)),
        Kind::MatterFab => (input == ids::SCRAP).then_some((ids::UU_MATTER, 1)),
        Kind::Insolator => match input {
            ids::WHEAT_SEEDS => Some((ids::WHEAT, 1)),
            ids::CARROT => Some((ids::CARROT, 2)),
            ids::POTATO => Some((ids::POTATO, 2)),
            x if [Block::FlowerRed, Block::FlowerYellow, Block::FlowerBlue].iter().any(|f| *f as ItemId == x) => Some((x, 2)),
            _ => None,
        },
        _ => None,
    }
}

/// One block of the machine network for `share` (index-aligned).
#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub kind: Kind,
    /// Energy store of this node (the spec's, plus «Накопитель» upgrades).
    pub cap: f32,
    pub energy: f32,
    /// Generators: ЭЕ/s it makes now. Machines: ЭЕ/s it wants now (0 when idle).
    pub flow: f32,
}

/// Moves energy inside one connected network for `dt` seconds: generators fill their own
/// buffer, then everything flows from those with energy to machines that work and on into
/// energy cells. Returns the new energy of each node.
pub fn share(nodes: &[Node], dt: f32) -> Vec<f32> {
    let mut e: Vec<f32> = nodes.iter().map(|n| n.energy).collect();
    // 1. Generators produce into themselves (up to capacity).
    for (i, n) in nodes.iter().enumerate() {
        if is_source(n.kind) {
            e[i] = (e[i] + n.flow * dt).min(n.cap);
        }
    }
    // 2. Pool everything that can be given: generators and cells.
    let givers: Vec<usize> = (0..nodes.len()).filter(|&i| is_source(nodes[i].kind) || is_store(nodes[i].kind)).collect();
    let mut pool: f32 = givers.iter().map(|&i| e[i]).sum();
    for &i in &givers {
        e[i] = 0.0;
    }
    // 3. Machines take what they need for this frame (filling their buffer up to a second's worth).
    let is_machine = |k: Kind| is_machine(k) || matches!(k, Kind::Motor | Kind::Quarry | Kind::Magnet | Kind::Tesla | Kind::ChargePad);
    for (i, n) in nodes.iter().enumerate() {
        if is_machine(n.kind) {
            // A machine keeps only a few seconds of work in its buffer: the rest of the
            // energy stays in the cells for the others.
            let want = (n.cap.min(n.flow.max(spec(n.kind).rate) * 3.0) - e[i]).max(0.0);
            let got = want.min(pool);
            e[i] += got;
            pool -= got;
        }
    }
    // 4. Machines fill their own small stores, the rest goes to the cells (storage) and
    //    then back into the generators' buffers.
    for (i, n) in nodes.iter().enumerate() {
        if is_machine(n.kind) && pool > 0.0 {
            let put = (n.cap - e[i]).max(0.0).min(pool);
            e[i] += put;
            pool -= put;
        }
    }
    for &i in givers.iter().filter(|&&i| is_store(nodes[i].kind)).chain(givers.iter().filter(|&&i| !is_store(nodes[i].kind))) {
        let room = nodes[i].cap - e[i];
        let put = room.min(pool);
        e[i] += put;
        pool -= put;
    }
    e
}

/// IC2-style upgrades in a machine's right-hand slots: (time ×, energy use ×, extra store).
/// Each «Ускоритель» works 30 % faster and uses 60 % more power; each «Накопитель» adds
/// 2 000 ЭЕ of store.
pub fn upgrades(slots: &[Option<crate::items::ItemStack>]) -> (f32, f32, f32) {
    let mut oc = 0u32;
    let mut st = 0u32;
    for s in slots.iter().flatten() {
        match s.id {
            ids::OVERCLOCKER => oc += s.count as u32,
            ids::STORAGE_UPGRADE => st += s.count as u32,
            _ => {}
        }
    }
    let oc = oc.min(16) as i32;
    (0.7f32.powi(oc), 1.6f32.powi(oc), st.min(64) as f32 * 2_000.0)
}

/// An upgrade item for the machine slots.
pub fn is_upgrade(id: ItemId) -> bool {
    matches!(id, ids::OVERCLOCKER | ids::STORAGE_UPGRADE)
}

/// Fuel value in seconds of burning (same as the furnace).
pub fn fuel(id: ItemId) -> f32 {
    crate::interaction::fuel_time(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_flows_from_a_generator_through_to_a_machine_and_a_cell() {
        let mut nodes = vec![
            Node { kind: Kind::Generator, cap: spec(Kind::Generator).capacity, energy: 0.0, flow: 20.0 },
            Node { kind: Kind::Crusher, cap: spec(Kind::Crusher).capacity, energy: 0.0, flow: 10.0 },
            Node { kind: Kind::Cell, cap: spec(Kind::Cell).capacity, energy: 0.0, flow: 0.0 },
        ];
        let mut total_made = 0.0;
        for _ in 0..600 {
            let e = share(&nodes, 0.05);
            for (n, v) in nodes.iter_mut().zip(e) {
                n.energy = v;
            }
            total_made += 20.0 * 0.05;
            // The crusher works: spends its rate.
            nodes[1].energy = (nodes[1].energy - 10.0 * 0.05).max(0.0);
        }
        assert!(nodes[1].energy > 5.0, "the machine is fed: {}", nodes[1].energy);
        assert!(nodes[2].energy > 50.0, "the surplus is stored: {}", nodes[2].energy);
        let held: f32 = nodes.iter().map(|n| n.energy).sum();
        assert!(held <= total_made + 1.0, "no energy from nowhere");
    }

    #[test]
    fn overclockers_speed_up_and_cost_more() {
        use crate::items::ItemStack;
        assert_eq!(upgrades(&[]), (1.0, 1.0, 0.0));
        let (t, e, s) = upgrades(&[Some(ItemStack::new(ids::OVERCLOCKER, 2)), Some(ItemStack::new(ids::STORAGE_UPGRADE, 1))]);
        assert!((t - 0.49).abs() < 1e-4 && (e - 2.56).abs() < 1e-4 && s == 2_000.0);
    }

    #[test]
    fn packed_rods_give_more_and_overheat_without_vents() {
        use crate::items::ItemStack;
        let rod = Some(ItemStack::new(ids::URANIUM_ROD, 1));
        let vent = Some(ItemStack::new(ids::HEAT_VENT, 1));
        let mut one = vec![None; REACTOR_SLOTS];
        one[0] = rod;
        one[1] = vent;
        let t = reactor_second(&mut one, 0.0);
        assert_eq!((t.power, t.rods), (10.0, 1));
        assert!(t.heat <= 0.0, "one rod, one vent: stays cool");
        let mut two = vec![None; REACTOR_SLOTS];
        two[0] = rod;
        two[1] = rod;
        let t = reactor_second(&mut two, 0.0);
        assert_eq!(t.power, 40.0, "neighbours pulse each other");
        assert!(t.heat > 20.0, "and heat up fast without vents");
        assert_eq!(two[0].unwrap().damage, 1, "rods wear");
    }

    #[test]
    fn machines_have_recipes() {
        assert_eq!(recipe(Kind::Crusher, ids::RAW_IRON), Some((ids::IRON_DUST, 2)));
        assert_eq!(recipe(Kind::Enricher, ids::RAW_GOLD), Some((ids::GOLD_DUST, 3)));
        assert_eq!(recipe(Kind::Sawmill, Block::OakLog as ItemId), Some((Block::OakPlanks as ItemId, 6)));
        assert_eq!((recipe(Kind::Compressor, ids::COAL), need(Kind::Compressor, ids::COAL)), (Some((ids::DIAMOND, 1)), 32));
        assert_eq!(recipe(Kind::Induction, ids::IRON_DUST), Some((ids::IRON_INGOT, 1)));
        assert_eq!(recipe(Kind::Crusher, Block::Cobblestone as ItemId), Some((Block::Gravel as ItemId, 1)));
        assert_eq!(idle_recipe(Kind::Extruder), Some((Block::Cobblestone as ItemId, 1)));
        assert_eq!(recipe(Kind::Insolator, ids::WHEAT_SEEDS), Some((ids::WHEAT, 1)));
        assert!(keeps(Kind::Insolator) && !keeps(Kind::Crusher) && !env_ok(Kind::Extruder, 0.0));
        assert!(fuel_for(Kind::Magma, Block::Magma as ItemId) > 0.0 && is_source(Kind::Magma));
        assert_eq!((recipe(Kind::Recycler, ids::COAL), need(Kind::Recycler, ids::COAL)), (Some((ids::SCRAP, 1)), 4));
        assert_eq!(recipe(Kind::MatterFab, ids::SCRAP), Some((ids::UU_MATTER, 1)));
        assert!(recipe(Kind::Recycler, ids::SCRAP).is_none());
        assert_eq!(recipe(Kind::Centrifuge, Block::Gravel as ItemId), Some((ids::FLINT, 2)));
        // Mod packs: Tinkers' smeltery doubles ore, IE's arc furnace too, coke and steel, fusion.
        assert_eq!(recipe(Kind::Smeltery, Block::CobaltOre as ItemId), Some((ids::COBALT_INGOT, 2)));
        assert_eq!(recipe(Kind::ArcFurnace, ids::RAW_IRON), Some((ids::IRON_INGOT, 2)));
        assert_eq!(recipe(Kind::CokeOven, ids::COAL), Some((ids::COKE, 1)));
        assert_eq!(recipe(Kind::BlastFurnace, ids::IRON_INGOT), Some((ids::STEEL_INGOT, 1)));
        assert_eq!(recipe(Kind::Fusion, ids::DRACONIUM_INGOT), Some((ids::AWAKENED_DRACONIUM, 1)));
        assert_eq!(recipe(Kind::Crusher, Block::DraconiumOre as ItemId), Some((ids::DRACONIUM_DUST, 2)));
        assert!(is_store(Kind::DracoCore) && is_machine(Kind::Smeltery) && !env_ok(Kind::Smeltery, 0.0));
        assert!(is_source(Kind::Lunar) && is_store(Kind::BigCell) && !is_store(Kind::Tesla));
    }
}
