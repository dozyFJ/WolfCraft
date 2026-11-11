//! World creation settings (stored in `worlds/<id>/world.toml`) and global game settings.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlanetSize {
    Small,
    #[default]
    Medium,
    Large,
    Huge,
}

impl PlanetSize {
    pub const ALL: [PlanetSize; 4] = [PlanetSize::Small, PlanetSize::Medium, PlanetSize::Large, PlanetSize::Huge];
    /// Sea-level radius in blocks.
    pub fn radius(self) -> u32 {
        match self {
            PlanetSize::Small => 512,
            PlanetSize::Medium => 1024,
            PlanetSize::Large => 2048,
            PlanetSize::Huge => 4096,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            PlanetSize::Small => "Маленькая",
            PlanetSize::Medium => "Средняя",
            PlanetSize::Large => "Большая",
            PlanetSize::Huge => "Огромная",
        }
    }
    /// Equator length in blocks.
    pub fn circumference(self) -> u32 {
        (self.radius() as f32 * std::f32::consts::TAU) as u32
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorldType {
    #[default]
    Standard,
    Flat,
    Supercontinent,
    Islands,
}

impl WorldType {
    pub const ALL: [WorldType; 4] = [WorldType::Standard, WorldType::Flat, WorldType::Supercontinent, WorldType::Islands];
    pub fn label(self) -> &'static str {
        match self {
            WorldType::Standard => "Стандартный",
            WorldType::Flat => "Плоский",
            WorldType::Supercontinent => "Суперконтинент",
            WorldType::Islands => "Острова",
        }
    }
}

/// Shape of the planet. Worlds saved before cube planets existed load as spheres.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlanetShape {
    #[default]
    Sphere,
    Cube,
}

impl PlanetShape {
    pub const ALL: [PlanetShape; 2] = [PlanetShape::Cube, PlanetShape::Sphere];
    pub fn label(self) -> &'static str {
        match self {
            PlanetShape::Sphere => "Шар",
            PlanetShape::Cube => "Куб",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContinentCount {
    #[default]
    Auto,
    C3to5,
    C5to8,
    C8to12,
}

impl ContinentCount {
    pub const ALL: [ContinentCount; 4] = [ContinentCount::Auto, ContinentCount::C3to5, ContinentCount::C5to8, ContinentCount::C8to12];
    pub fn label(self) -> &'static str {
        match self {
            ContinentCount::Auto => "Автоматически",
            ContinentCount::C3to5 => "3–5",
            ContinentCount::C5to8 => "5–8",
            ContinentCount::C8to12 => "8–12",
        }
    }
    /// Inclusive range; `Auto` means "pick from 3..=8 using the seed".
    pub fn range(self) -> (u32, u32) {
        match self {
            ContinentCount::Auto => (3, 8),
            ContinentCount::C3to5 => (3, 5),
            ContinentCount::C5to8 => (5, 8),
            ContinentCount::C8to12 => (8, 12),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Peaceful,
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 4] = [Difficulty::Peaceful, Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];
    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Peaceful => "Мирная",
            Difficulty::Easy => "Лёгкая",
            Difficulty::Normal => "Нормальная",
            Difficulty::Hard => "Сложная",
        }
    }
    /// Multiplier applied to incoming damage.
    pub fn damage_scale(self) -> f32 {
        match self {
            Difficulty::Peaceful => 0.0,
            Difficulty::Easy => 0.6,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.5,
        }
    }
    /// Multiplier applied to hunger drain.
    pub fn hunger_scale(self) -> f32 {
        match self {
            Difficulty::Peaceful => 0.0,
            Difficulty::Easy => 0.7,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.4,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WorldConfig {
    pub id: String,
    pub name: String,
    pub seed: u64,
    pub mode: GameMode,
    pub size: PlanetSize,
    pub world_type: WorldType,
    pub continents: ContinentCount,
    pub structures: bool,
    pub difficulty: Difficulty,
    pub created: u64,
    pub last_played: u64,
    /// Version of the generator; bump when terrain output changes.
    pub generator_version: u32,
    /// Save format of this world folder (missing in worlds made before 0.2.0 = 1).
    #[serde(default = "first_save_format")]
    pub format_version: u32,
    /// Planet shape (missing = sphere, the only shape of older worlds).
    #[serde(default)]
    pub shape: PlanetShape,
}

fn first_save_format() -> u32 {
    1
}

pub const GENERATOR_VERSION: u32 = 1;

impl WorldConfig {
    pub fn new(name: &str, seed: u64) -> Self {
        let now = wolfcraft_common::platform::unix_now();
        Self {
            id: String::new(),
            name: name.to_string(),
            seed,
            mode: GameMode::Survival,
            size: PlanetSize::Medium,
            world_type: WorldType::Standard,
            continents: ContinentCount::Auto,
            structures: true,
            difficulty: Difficulty::Normal,
            created: now,
            last_played: now,
            generator_version: GENERATOR_VERSION,
            format_version: wolfcraft_common::version::SAVE_FORMAT,
            shape: PlanetShape::Sphere,
        }
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        // Write atomically: temp file + rename so a crash never leaves a half-written world.toml.
        let tmp = dir.join("world.toml.tmp");
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, dir.join("world.toml")).map_err(|e| e.to_string())
    }

    /// Text of `world.toml`, as a server sends it to its clients.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// World settings received from a server (the text of its `world.toml`).
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("world.toml: {e}"))
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(dir.join("world.toml")).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut cfg: WorldConfig = toml::from_str(&text).map_err(|e| format!("world.toml: {e}"))?;
        if cfg.id.is_empty() {
            cfg.id = dir.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        }
        let current = wolfcraft_common::version::SAVE_FORMAT;
        if cfg.format_version > current {
            return Err(format!("мир создан в более новой версии игры (формат {}, поддерживается {current})", cfg.format_version));
        }
        if cfg.format_version < current {
            // Migration 1 -> 2 adds only optional side files, so the data stays as it is.
            // Keep the old world.toml next to the new one before rewriting it.
            let _ = std::fs::copy(dir.join("world.toml"), dir.join(format!("world.v{}.toml.bak", cfg.format_version)));
            log::info!("world {}: save format {} -> {current}", cfg.id, cfg.format_version);
            cfg.format_version = current;
            cfg.save(dir)?;
        }
        Ok(cfg)
    }
}

/// Turns any text into a safe folder name.
pub fn slugify(name: &str) -> String {
    let mut s = String::new();
    for c in name.trim().chars() {
        if c.is_alphanumeric() {
            s.extend(c.to_lowercase());
        } else if (c == ' ' || c == '-' || c == '_') && !s.ends_with('_') {
            s.push('_');
        }
    }
    let s = s.trim_matches('_').to_string();
    if s.is_empty() { "world".to_string() } else { s.chars().take(40).collect() }
}

/// Folder name for a new world that does not collide with existing ones.
pub fn unique_world_id(worlds_dir: &Path, name: &str) -> String {
    let base = slugify(name);
    let mut id = base.clone();
    let mut n = 2;
    while worlds_dir.join(&id).exists() {
        id = format!("{base}_{n}");
        n += 1;
    }
    id
}

/// Parses a user-entered seed: empty → random, number → that number, other text → hash.
pub fn parse_seed(text: &str, random: u64) -> u64 {
    let t = text.trim();
    if t.is_empty() {
        return random;
    }
    if let Ok(n) = t.parse::<i64>() {
        return n as u64;
    }
    // FNV-1a over the text (stable across runs/platforms).
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in t.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Lists worlds in `worlds_dir`, newest played first.
pub fn list_worlds(worlds_dir: &Path) -> Vec<WorldConfig> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(worlds_dir) {
        for e in rd.flatten() {
            if e.path().join("world.toml").exists() {
                match WorldConfig::load(&e.path()) {
                    Ok(c) => out.push(c),
                    Err(err) => log::warn!("skipping world: {err}"),
                }
            }
        }
    }
    out.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    out
}

// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

/// Global user settings (`settings.toml` in the data directory).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub render_distance: u32,
    pub fov: f32,
    pub vsync: bool,
    /// 0 = unlimited.
    pub max_fps: u32,
    pub mouse_sensitivity: f32,
    /// Mouse up looks down.
    pub invert_y: bool,
    pub ui_scale: f32,
    pub theme: ThemeMode,
    pub fullscreen: bool,
    pub show_fps: bool,
    /// Player name shown in chat.
    pub nickname: String,
    /// Skin preset index.
    pub skin: u8,
    /// First-person hand/item (view model): own FOV, scale and offset.
    pub vm_fov: f32,
    pub vm_scale: f32,
    pub vm_x: f32,
    pub vm_y: f32,
    /// Master volume 0..1.
    pub volume: f32,
    /// Amount of block particles (0 = off, 1 = normal, 2 = a lot).
    pub particles: f32,
    /// Strength of the hand swing animation (0.4 = subtle, 1 = full).
    pub hand_anim: f32,
    /// Active resource pack: `builtin:16|32|64|128` or `user:<folder or zip name>`.
    pub texture_pack: String,
    /// Screen refresh rate asked from the phone (60, 90 or 120 Hz).
    pub refresh_rate: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            render_distance: 8,
            fov: 75.0,
            vsync: true,
            max_fps: 0,
            mouse_sensitivity: 1.0,
            invert_y: false,
            ui_scale: 1.0,
            theme: ThemeMode::Dark,
            fullscreen: false,
            show_fps: true,
            nickname: "Игрок".to_string(),
            skin: 0,
            vm_fov: 62.0,
            vm_scale: 1.0,
            vm_x: 0.0,
            vm_y: 0.0,
            volume: 0.6,
            particles: 1.0,
            hand_anim: 1.0,
            texture_pack: "builtin:16".into(),
            refresh_rate: 120,
        }
    }
}

impl Settings {
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join("settings.toml")
    }
    pub fn load(data_dir: &Path) -> Self {
        std::fs::read_to_string(Self::path(data_dir)).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }
    pub fn save(&self, data_dir: &Path) {
        let _ = std::fs::create_dir_all(data_dir);
        if let Ok(t) = toml::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(data_dir), t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip() {
        let dir = std::env::temp_dir().join("wc_cfg_test").join("my_world");
        let _ = std::fs::remove_dir_all(&dir);
        let mut c = WorldConfig::new("Мой мир", 42);
        c.size = PlanetSize::Huge;
        c.world_type = WorldType::Islands;
        c.continents = ContinentCount::C8to12;
        c.structures = false;
        c.save(&dir).unwrap();
        let back = WorldConfig::load(&dir).unwrap();
        assert_eq!(back.size, PlanetSize::Huge);
        assert_eq!(back.world_type, WorldType::Islands);
        assert_eq!(back.seed, 42);
        assert!(!back.structures);
        assert_eq!(back.name, "Мой мир");
    }

    #[test]
    fn seed_parsing() {
        assert_eq!(parse_seed("", 7), 7);
        assert_eq!(parse_seed("123", 7), 123);
        assert_eq!(parse_seed("wolf", 7), parse_seed("wolf", 9));
        assert_ne!(parse_seed("wolf", 7), parse_seed("wolf2", 7));
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("  My World! "), "my_world");
        assert_eq!(slugify("???"), "world");
        assert_eq!(slugify("Мир 1"), "мир_1");
    }

    #[test]
    fn old_world_without_format_version_is_migrated_and_backed_up() {
        let dir = std::env::temp_dir().join("wc_cfg_migrate");
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = WorldConfig::new("Старый", 7);
        cfg.id = "old".into();
        cfg.save(&dir).unwrap();
        // Strip the field as a 0.1.0 world would look.
        let text = std::fs::read_to_string(dir.join("world.toml")).unwrap();
        let old: String = text.lines().filter(|l| !l.starts_with("format_version")).collect::<Vec<_>>().join("\n");
        std::fs::write(dir.join("world.toml"), old).unwrap();
        let loaded = WorldConfig::load(&dir).unwrap();
        assert_eq!(loaded.format_version, wolfcraft_common::version::SAVE_FORMAT);
        assert_eq!(loaded.seed, 7);
        assert!(dir.join("world.v1.toml.bak").exists());
        // A world from a newer game is refused instead of being damaged.
        let newer = std::fs::read_to_string(dir.join("world.toml")).unwrap().replace(&format!("format_version = {}", loaded.format_version), "format_version = 999");
        std::fs::write(dir.join("world.toml"), newer).unwrap();
        assert!(WorldConfig::load(&dir).is_err());
    }
}
