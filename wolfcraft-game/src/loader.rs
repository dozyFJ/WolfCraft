//! World creation / loading pipeline. Runs on a worker thread and reports real progress.
//!
//! Stages (the last one — generating the starting chunks — is driven by the chunk manager on
//! the main thread):
//!  0 save settings · 1 seed · 2 continent & ocean map · 3 climate & biomes ·
//!  4 safe spawn point · 5 structures near spawn · 6 starting chunks

use glam::{DVec3, IVec3};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use wolfcraft_engine::biome::Biome;
use wolfcraft_engine::config::{GameMode, WorldConfig};
use wolfcraft_engine::planet::{Planet, Rng};
use wolfcraft_engine::player::Player;
use wolfcraft_engine::storage::Storage;
use wolfcraft_engine::structures;

pub const STAGES: [&str; 7] = [
    "Сохранение настроек мира",
    "Подготовка seed",
    "Карта материков и океанов",
    "Расчёт климата и биомов",
    "Поиск безопасной точки появления",
    "Размещение структур рядом",
    "Генерация стартовых чанков",
];

pub struct Loaded {
    pub cfg: WorldConfig,
    pub dir: PathBuf,
    pub planet: Arc<Planet>,
    pub player: Player,
    pub storage: Arc<Storage>,
    pub restored: bool,
}

#[derive(Default)]
pub struct LoadShared {
    stage: AtomicUsize,
    /// Fraction of the worker stages, f32 bits.
    fraction: AtomicU32,
    done: AtomicBool,
    detail: Mutex<String>,
    result: Mutex<Option<Result<Loaded, String>>>,
}

impl LoadShared {
    fn set(&self, stage: usize, fraction: f32, detail: String) {
        self.stage.store(stage, Ordering::SeqCst);
        self.fraction.store(fraction.to_bits(), Ordering::SeqCst);
        if let Ok(mut d) = self.detail.lock() {
            *d = detail;
        }
    }
    pub fn stage(&self) -> usize {
        self.stage.load(Ordering::SeqCst)
    }
    pub fn fraction(&self) -> f32 {
        f32::from_bits(self.fraction.load(Ordering::SeqCst))
    }
    pub fn detail(&self) -> String {
        self.detail.lock().map(|d| d.clone()).unwrap_or_default()
    }
    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }
    pub fn take_result(&self) -> Option<Result<Loaded, String>> {
        self.result.lock().ok().and_then(|mut r| r.take())
    }
}

pub struct LoadRequest {
    pub cfg: WorldConfig,
    pub worlds_dir: PathBuf,
    pub is_new: bool,
}

pub fn start(req: LoadRequest) -> Arc<LoadShared> {
    let shared = Arc::new(LoadShared::default());
    let s = shared.clone();
    std::thread::Builder::new()
        .name("world-loader".into())
        .spawn(move || {
            let r = run(&req, &s);
            if let Err(e) = &r {
                log::error!("world load failed: {e}");
            }
            if let Ok(mut slot) = s.result.lock() {
                *slot = Some(r);
            }
            s.done.store(true, Ordering::SeqCst);
        })
        .expect("spawn loader thread");
    shared
}

fn run(req: &LoadRequest, sh: &LoadShared) -> Result<Loaded, String> {
    let mut cfg = req.cfg.clone();

    // 0. Persist settings first, so a crash later still leaves a valid world folder.
    sh.set(0, 0.0, if req.is_new { "Создание папки мира".into() } else { "Чтение world.toml".into() });
    if req.is_new {
        cfg.id = wolfcraft_engine::config::unique_world_id(&req.worlds_dir, &cfg.name);
    }
    let dir = req.worlds_dir.join(&cfg.id);
    cfg.last_played = wolfcraft_common::platform::unix_now();
    cfg.save(&dir)?;
    let storage = Arc::new(Storage::open(&dir)?);
    log::info!("world '{}' ({}) at {}", cfg.name, cfg.id, dir.display());

    // 1. Seed.
    sh.set(1, 0.06, format!("Seed: {}", cfg.seed as i64));

    // 2. Continents & oceans.
    sh.set(2, 0.10, "Размещение центров материков".into());
    // Physics, camera and the sky ask for "up" without a grid at hand: tell them the shape.
    wolfcraft_engine::cubesphere::set_cube_world(cfg.shape == wolfcraft_engine::config::PlanetShape::Cube);
    let planet = Arc::new(Planet::new(&cfg));
    let mut land = 0;
    let n = 3000;
    for i in 0..n {
        let d = Rng(cfg.seed ^ (i as u64 + 1) * 7919).unit_vec();
        if planet.column(d).elev >= 0.0 {
            land += 1;
        }
        if i % 500 == 0 {
            sh.set(2, 0.10 + 0.10 * i as f32 / n as f32, format!("Выборка карты: {i}/{n}"));
        }
    }
    sh.set(2, 0.20, format!("Суша {:.0}% · материков {}", land as f32 / n as f32 * 100.0, planet.continents.len().max(1)));

    // 3. Climate & biomes.
    let mut hist = std::collections::HashMap::<&'static str, u32>::new();
    let mut checked = 0;
    for i in 0..2500 {
        let d = Rng(cfg.seed ^ 0xB10 ^ (i as u64 + 1) * 104729).unit_vec();
        let col = planet.column(d);
        if col.biome != Biome::Ocean && col.biome != Biome::DeepOcean && col.biome != Biome::FrozenOcean {
            *hist.entry(col.biome.label()).or_default() += 1;
            checked += 1;
        }
        if i % 500 == 0 {
            sh.set(3, 0.20 + 0.10 * i as f32 / 2500.0, format!("Климатические зоны: {i}/2500"));
        }
    }
    let top = hist.iter().max_by_key(|(_, c)| **c).map(|(b, _)| *b).unwrap_or("—");
    sh.set(3, 0.30, format!("Биомов суши: {} · чаще всего: {top} ({checked} проб)", hist.len()));

    // 4. Spawn point (or the saved player).
    sh.set(4, 0.32, "Поиск ровной суши".into());
    let saved = std::fs::read(storage.player_path()).ok().and_then(|b| Player::from_bytes(&b));
    let restored = saved.is_some();
    let player = match saved {
        Some(mut p) => {
            // Mode comes from world.toml (it is authoritative if edited).
            p.mode = cfg.mode;
            if cfg.mode == GameMode::Survival {
                p.body.flying = false;
            }
            p
        }
        None => {
            let spawn = planet.find_spawn();
            log::info!("spawn point: {spawn:?}");
            Player::new(spawn, cfg.mode)
        }
    };
    sh.set(4, 0.34, format!("Точка появления: {:.0}, {:.0}, {:.0}", player.body.pos.x, player.body.pos.y, player.body.pos.z));

    // 5. Structures nearby.
    let mut detail = "Структуры отключены".to_string();
    if cfg.structures {
        sh.set(5, 0.35, "Сканирование окрестностей".into());
        let c = planet.grid.world_to_cell(player.body.pos);
        let found = structures::placements_near(&planet, c.f, IVec3::new(c.i - 700, c.j - 700, 0), IVec3::new(c.i + 700, c.j + 700, 0));
        let mut kinds = std::collections::HashMap::<&str, u32>::new();
        for f in &found {
            *kinds.entry(f.kind.label()).or_default() += 1;
        }
        detail = if found.is_empty() { "В радиусе 700 блоков структур нет".into() } else { format!("В радиусе 700 блоков: {}", kinds.iter().map(|(k, v)| format!("{k} ×{v}")).collect::<Vec<_>>().join(", ")) };
    }
    sh.set(5, 0.40, detail);
    let _ = DVec3::ZERO;
    Ok(Loaded { cfg, dir, planet, player, storage, restored })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wolfcraft_engine::config::PlanetSize;

    #[test]
    fn creates_world_folder_and_reports_progress() {
        let dir = std::env::temp_dir().join("wc_loader_test");
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = WorldConfig::new("Тестовый мир", 42);
        cfg.size = PlanetSize::Small;
        let sh = start(LoadRequest { cfg, worlds_dir: dir.clone(), is_new: true });
        let t = std::time::Instant::now();
        let mut last_stage = 0;
        while !sh.is_done() {
            assert!(t.elapsed().as_secs() < 60, "loader hung");
            assert!(sh.stage() >= last_stage, "stages must not go backwards");
            last_stage = sh.stage();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let loaded = sh.take_result().unwrap().unwrap();
        assert_eq!(loaded.cfg.id, "тестовый_мир");
        assert!(dir.join("тестовый_мир").join("world.toml").exists());
        assert!(dir.join("тестовый_мир").join("regions").is_dir());
        assert!(dir.join("тестовый_мир").join("structures").is_dir());
        assert!(!loaded.restored);
        assert!(loaded.player.body.pos.length() > loaded.planet.radius);
        // Second world with the same name gets a different folder.
        let mut cfg2 = WorldConfig::new("Тестовый мир", 43);
        cfg2.size = PlanetSize::Small;
        let sh2 = start(LoadRequest { cfg: cfg2, worlds_dir: dir.clone(), is_new: true });
        while !sh2.is_done() {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(sh2.take_result().unwrap().unwrap().cfg.id, "тестовый_мир_2");
    }

    #[test]
    fn reloading_restores_the_saved_player() {
        let dir = std::env::temp_dir().join("wc_loader_test2");
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = WorldConfig::new("Мир", 5);
        cfg.size = PlanetSize::Small;
        let sh = start(LoadRequest { cfg, worlds_dir: dir.clone(), is_new: true });
        while !sh.is_done() {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let mut l = sh.take_result().unwrap().unwrap();
        l.player.body.pos += DVec3::new(0.0, 3.0, 0.0);
        l.player.survival.xp = 55;
        std::fs::write(l.storage.player_path(), l.player.to_bytes()).unwrap();
        let expected = l.player.body.pos;
        let sh = start(LoadRequest { cfg: l.cfg.clone(), worlds_dir: dir.clone(), is_new: false });
        while !sh.is_done() {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let l2 = sh.take_result().unwrap().unwrap();
        assert!(l2.restored);
        assert_eq!(l2.player.body.pos, expected);
        assert_eq!(l2.player.survival.xp, 55);
        // Same seed → same planet.
        assert_eq!(l2.planet.radius, l.planet.radius);
    }
}
