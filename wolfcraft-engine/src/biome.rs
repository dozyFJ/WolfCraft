//! Biome list and surface palettes.

use crate::block::Block;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    Ocean,
    DeepOcean,
    FrozenOcean,
    /// Tropical shallow sea with coral reefs.
    WarmOcean,
    /// Cool sea with kelp forests and a gravel floor.
    ColdOcean,
    Beach,
    Plains,
    Forest,
    BirchForest,
    Jungle,
    Taiga,
    Tundra,
    SnowyPlains,
    Desert,
    Badlands,
    Savanna,
    Swamp,
    Mountains,
    SnowyPeaks,
    Volcano,
    /// Pink cherry trees on gentle hills.
    CherryGrove,
    /// Dense dark oaks with wide crowns, mushrooms under them.
    DarkForest,
    /// Open grass thick with flowers.
    FlowerMeadow,
}

impl Biome {
    pub fn label(self) -> &'static str {
        match self {
            Biome::Ocean => "Океан",
            Biome::DeepOcean => "Глубокий океан",
            Biome::FrozenOcean => "Ледяной океан",
            Biome::WarmOcean => "Тёплый океан",
            Biome::ColdOcean => "Холодный океан",
            Biome::Beach => "Пляж",
            Biome::Plains => "Равнины",
            Biome::Forest => "Лес",
            Biome::BirchForest => "Берёзовый лес",
            Biome::Jungle => "Джунгли",
            Biome::Taiga => "Тайга",
            Biome::Tundra => "Тундра",
            Biome::SnowyPlains => "Ледники",
            Biome::Desert => "Пустыня",
            Biome::Badlands => "Бесплодные земли",
            Biome::Savanna => "Саванна",
            Biome::Swamp => "Болото",
            Biome::Mountains => "Горы",
            Biome::SnowyPeaks => "Снежные пики",
            Biome::Volcano => "Вулкан",
            Biome::CherryGrove => "Вишнёвая роща",
            Biome::DarkForest => "Тёмный лес",
            Biome::FlowerMeadow => "Цветочный луг",
        }
    }

    pub fn is_ocean(self) -> bool {
        matches!(self, Biome::Ocean | Biome::DeepOcean | Biome::FrozenOcean | Biome::WarmOcean | Biome::ColdOcean)
    }

    /// (top block, filler block, deeper layer block) for land; the filler is 3 blocks thick.
    pub fn palette(self) -> (Block, Block, Block) {
        match self {
            Biome::Plains | Biome::Forest | Biome::BirchForest | Biome::Jungle | Biome::Taiga | Biome::CherryGrove | Biome::DarkForest | Biome::FlowerMeadow => (Block::Grass, Block::Dirt, Block::Stone),
            Biome::Tundra => (Block::DryGrass, Block::Dirt, Block::Stone),
            Biome::SnowyPlains => (Block::Snow, Block::PackedIce, Block::Stone),
            Biome::Desert | Biome::Beach => (Block::Sand, Block::Sand, Block::Sandstone),
            Biome::Badlands => (Block::RedSand, Block::Terracotta, Block::Terracotta),
            Biome::Savanna => (Block::DryGrass, Block::Dirt, Block::Stone),
            Biome::Swamp => (Block::SwampGrass, Block::Mud, Block::Stone),
            Biome::Mountains => (Block::Stone, Block::Stone, Block::Stone),
            Biome::SnowyPeaks => (Block::Snow, Block::Snow, Block::Stone),
            Biome::Volcano => (Block::Ash, Block::Basalt, Block::Basalt),
            Biome::Ocean | Biome::DeepOcean | Biome::FrozenOcean => (Block::Sand, Block::Sand, Block::Stone),
            Biome::WarmOcean => (Block::Sand, Block::Sand, Block::Sandstone),
            Biome::ColdOcean => (Block::Gravel, Block::Gravel, Block::Stone),
        }
    }
}
