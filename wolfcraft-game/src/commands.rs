//! Chat commands: /help /gamemode /time /tp /spawn /give /clear /kill /heal /feed /fly /seed
//! /pos /biome /difficulty /save /say

use crate::session::GameSession;
use glam::DVec3;
use wolfcraft_engine::config::{Difficulty, GameMode};
use wolfcraft_engine::items::{self, ItemStack};
use wolfcraft_engine::player::DAY_LENGTH;
use wolfcraft_ui::chat_ui::ChatKind;

pub type Reply = Vec<(ChatKind, String)>;

fn sys(t: impl Into<String>) -> (ChatKind, String) {
    (ChatKind::System, t.into())
}
fn err(t: impl Into<String>) -> (ChatKind, String) {
    (ChatKind::Error, t.into())
}

pub const HELP: &[&str] = &[
    "Команды (T или / — открыть чат):",
    "/gamemode <выживание|творческий>   — режим игры (или /gm s|c)",
    "/time set <день|ночь|полдень|полночь|рассвет|закат|ЧЧ[:ММ]>, /time add <часы>",
    "/tp <x y z>  (допустимы ~ и ~N)   ·   /tp <широта> <долгота>   ·   /tp spawn",
    "/spawn — вернуться в точку появления",
    "/give <предмет> [кол-во]   ·   /clear — очистить инвентарь",
    "/heal  /feed  /kill  /fly   ·   /difficulty <мирная|лёгкая|нормальная|сложная>",
    "/pos  /biome  /seed  /save  /say <текст>",
    "/season [весна|лето|осень|зима] — узнать или сменить время года",
    "/month [январь…декабрь|1–12] — узнать дату или перейти к месяцу",
    "/arena <камень|лес|вулкан> — построить арену босса с алтарём прямо перед собой",
    "/enchant <чары> [уровень] — зачаровать предмет в руке (острота, сноровка, крепость, защита…)",
    "/effect <эффект> [секунды] — наложить эффект (скорость, сила, регенерация…); /effect clear",
    "/demo — построить перед собой выставку искрита: схемы, автодвери, ферма, потайной проход",
    "//help — команды WorldEdit: //pos1 //pos2 //set //replace //walls //copy //paste //sphere //cyl //undo …",
    "/ад — перейти в Ад,  /мир — вернуться (или через адский портал: 4 обсидиана + светокамень)",
];

/// Parses one coordinate token: `12.5`, `~` (current) or `~-3` (offset from current).
pub fn parse_coord(tok: &str, current: f64) -> Option<f64> {
    if let Some(rest) = tok.strip_prefix('~') {
        if rest.is_empty() {
            return Some(current);
        }
        return rest.parse::<f64>().ok().map(|d| current + d);
    }
    tok.parse::<f64>().ok()
}

/// Time of day in seconds of world time from words or `HH[:MM]`.
pub fn parse_time(s: &str) -> Option<f64> {
    let frac = match s.to_lowercase().as_str() {
        "day" | "день" => 0.35,
        "noon" | "полдень" => 0.5,
        "night" | "ночь" => 0.9,
        "midnight" | "полночь" => 0.0,
        "sunrise" | "рассвет" => 0.25,
        "sunset" | "закат" => 0.75,
        other => {
            let (h, m) = match other.split_once(':') {
                Some((h, m)) => (h.parse::<f64>().ok()?, m.parse::<f64>().ok()?),
                None => (other.parse::<f64>().ok()?, 0.0),
            };
            if !(0.0..24.0).contains(&h) || !(0.0..60.0).contains(&m) {
                return None;
            }
            (h + m / 60.0) / 24.0
        }
    };
    Some(frac * DAY_LENGTH)
}

fn parse_mode(s: &str) -> Option<GameMode> {
    match s.to_lowercase().as_str() {
        "survival" | "s" | "0" | "выживание" | "в" => Some(GameMode::Survival),
        "creative" | "c" | "1" | "творческий" | "т" => Some(GameMode::Creative),
        _ => None,
    }
}

fn parse_difficulty(s: &str) -> Option<Difficulty> {
    match s.to_lowercase().as_str() {
        "peaceful" | "p" | "0" | "мирная" => Some(Difficulty::Peaceful),
        "easy" | "e" | "1" | "лёгкая" | "легкая" => Some(Difficulty::Easy),
        "normal" | "n" | "2" | "нормальная" => Some(Difficulty::Normal),
        "hard" | "h" | "3" | "сложная" => Some(Difficulty::Hard),
        _ => None,
    }
}

/// Pushes a position that lies inside generated terrain outwards until it is free.
fn lift_out_of_terrain(s: &GameSession, pos: DVec3) -> DVec3 {
    let d = wolfcraft_engine::cubesphere::up(pos);
    if pos == DVec3::ZERO {
        return pos;
    }
    let mut p = pos;
    for _ in 0..600 {
        if !s.planet.block_at_world(p).is_solid() && !s.planet.block_at_world(p + d).is_solid() {
            return p;
        }
        p += d;
    }
    p
}

pub fn run(s: &mut GameSession, line: &str) -> Reply {
    let line = line.trim();
    if line.is_empty() {
        return vec![];
    }
    let Some(body) = line.strip_prefix('/') else {
        return vec![(ChatKind::Player, format!("<{}> {line}", s.nickname))];
    };
    let mut it = body.split_whitespace();
    let cmd = it.next().unwrap_or("").to_lowercase();
    let args: Vec<&str> = it.collect();
    // WorldEdit: a double slash (//set …).
    if let Some(we) = cmd.strip_prefix('/') {
        return crate::worldedit::run(s, we, &args).unwrap_or_else(|| vec![err(format!("Неизвестная команда WorldEdit: //{we}. Список: //help"))]);
    }
    match cmd.as_str() {
        "help" | "?" | "помощь" => HELP.iter().map(|l| sys(*l)).collect(),
        "hell" | "nether" | "overworld" | "ад" | "мир" => {
            let want_hell = matches!(cmd.as_str(), "hell" | "nether" | "ад");
            if want_hell == s.hell {
                return vec![err(if s.hell { "Вы уже в Аду" } else { "Вы уже в обычном мире" })];
            }
            s.switch_dimension();
            vec![sys(if s.hell { "Вы в Аду. Портал для возврата рядом." } else { "Вы вернулись в обычный мир." })]
        }
        "say" => vec![(ChatKind::Player, format!("<{}> {}", s.nickname, args.join(" ")))],
        "gamemode" | "gm" => {
            let Some(m) = args.first().and_then(|a| parse_mode(a)) else { return vec![err("Использование: /gamemode <выживание|творческий>")] };
            s.cfg.mode = m;
            s.player.mode = m;
            if m == GameMode::Survival {
                s.player.body.flying = false;
            }
            vec![sys(format!("Режим игры: {}", if m == GameMode::Creative { "творческий" } else { "выживание" }))]
        }
        "difficulty" => {
            let Some(d) = args.first().and_then(|a| parse_difficulty(a)) else { return vec![err("Использование: /difficulty <мирная|лёгкая|нормальная|сложная>")] };
            s.cfg.difficulty = d;
            vec![sys(format!("Сложность: {}", d.label()))]
        }
        "time" => match args.as_slice() {
            [] | ["query"] => vec![sys(format!("Время мира: {}", s.clock()))],
            ["set", v] => match parse_time(v) {
                Some(t) => {
                    s.player.world_time = t;
                    vec![sys(format!("Время установлено: {}", s.clock()))]
                }
                None => vec![err("Неизвестное время. Примеры: день, ночь, полдень, 18:30")],
            },
            ["add", h] => match h.parse::<f64>() {
                Ok(h) => {
                    s.player.world_time += h / 24.0 * DAY_LENGTH;
                    vec![sys(format!("Время: {}", s.clock()))]
                }
                Err(_) => vec![err("Использование: /time add <часы>")],
            },
            _ => vec![err("Использование: /time set <значение> | /time add <часы>")],
        },
        "tp" | "teleport" => {
            let cur = s.player.body.pos;
            match args.as_slice() {
                ["spawn"] => teleport_spawn(s),
                [lat, lon] => match (lat.parse::<f64>(), lon.parse::<f64>()) {
                    (Ok(la), Ok(lo)) if la.abs() <= 90.0 => {
                        let (la, lo) = (la.to_radians(), lo.to_radians());
                        let dir = DVec3::new(la.cos() * lo.cos(), la.sin(), la.cos() * lo.sin());
                        let pos = s.planet.stand_position(dir).unwrap_or_else(|| dir * (s.planet.surface_radius(dir) + 3.0));
                        s.teleport(pos);
                        vec![sys(format!("Телепортация: широта {:.2}°, долгота {:.2}°", la.to_degrees(), lo.to_degrees()))]
                    }
                    _ => vec![err("Широта от -90 до 90, долгота в градусах")],
                },
                [x, y, z] => match (parse_coord(x, cur.x), parse_coord(y, cur.y), parse_coord(z, cur.z)) {
                    (Some(x), Some(y), Some(z)) => {
                        let pos = lift_out_of_terrain(s, DVec3::new(x, y, z));
                        s.teleport(pos);
                        vec![sys(format!("Телепортация: {:.1} {:.1} {:.1}", pos.x, pos.y, pos.z))]
                    }
                    _ => vec![err("Не удалось разобрать координаты. Пример: /tp 100 1030 -50 или /tp ~ ~10 ~")],
                },
                _ => vec![err("Использование: /tp <x y z> | /tp <широта долгота> | /tp spawn")],
            }
        }
        "spawn" => teleport_spawn(s),
        "give" => {
            if args.is_empty() {
                return vec![err("Использование: /give <предмет> [количество]")];
            }
            let (name_parts, count) = match args.last().and_then(|l| l.parse::<u32>().ok()) {
                Some(n) if args.len() > 1 => (&args[..args.len() - 1], n),
                _ => (&args[..], 1),
            };
            let name = name_parts.join(" ");
            let Some(id) = items::find_item(&name) else { return vec![err(format!("Предмет не найден: {name}"))] };
            let count = count.clamp(1, 2304);
            let max = items::info(id).max_stack as u32;
            let mut left = count;
            while left > 0 {
                let n = left.min(max);
                if let Some(rest) = s.player.inventory.add(ItemStack::new(id, n as u8)) {
                    left = left - n + rest.count as u32;
                    return vec![err(format!("Инвентарь полон: выдано {} из {}", count - left, count))];
                }
                left -= n;
            }
            vec![sys(format!("Выдано: {} ×{}", items::info(id).name, count))]
        }
        "clear" => {
            s.player.inventory = Default::default();
            vec![sys("Инвентарь очищен")]
        }
        "kill" => {
            s.player.survival.damage(1000.0, Difficulty::Hard);
            vec![sys("Вы погибли")]
        }
        "heal" => {
            s.player.survival.health = wolfcraft_engine::player::MAX_HEALTH;
            s.player.survival.dead = false;
            vec![sys("Здоровье восстановлено")]
        }
        "feed" => {
            s.player.survival.hunger = wolfcraft_engine::player::MAX_HUNGER;
            s.player.survival.saturation = 20.0;
            vec![sys("Голод утолён")]
        }
        "fly" => {
            if !s.is_creative() {
                return vec![err("Полёт доступен только в творческом режиме")];
            }
            s.player.toggle_flight();
            vec![sys(if s.player.body.flying { "Полёт включён" } else { "Полёт выключен" })]
        }
        "seed" => vec![sys(format!("Seed: {}", s.cfg.seed as i64))],
        "pos" => {
            let p = s.player.body.pos;
            let up = s.player.up();
            vec![sys(format!("XYZ {:.1} {:.1} {:.1} · широта {:.2}° долгота {:.2}° · высота {:.1}", p.x, p.y, p.z, up.y.asin().to_degrees(), up.z.atan2(up.x).to_degrees(), s.planet.grid.radial(p) - s.planet.radius))]
        }
        "demo" | "демо" => match s.build_iskrit_demo() {
            Ok(m) => vec![sys(m)],
            Err(e) => vec![err(e)],
        },
        "arena" | "арена" => {
            use wolfcraft_engine::bosses::BossKind;
            let kind = match args.first().map(|a| a.to_lowercase()) {
                Some(a) if a.starts_with("кам") || a.starts_with("stone") || a == "1" => BossKind::Stone,
                Some(a) if a.starts_with("лес") || a.starts_with("чащ") || a.starts_with("root") || a == "2" => BossKind::Root,
                Some(a) if a.starts_with("вулк") || a.starts_with("пеп") || a.starts_with("ember") || a == "3" => BossKind::Ember,
                _ => return vec![sys("Арены боссов: /arena камень (Каменный страж) · /arena лес (Хранитель чащи) · /arena вулкан (Сердце вулкана)")],
            };
            match s.build_arena(kind) {
                Ok(_) => vec![sys(format!("Арена готова впереди: «{}». Подойдите к алтарю в центре и тапните по нему (Печать древних выдана). Убрать арену: //undo", kind.name()))],
                Err(e) => vec![err(e)],
            }
        }
        "enchant" | "чары" => {
            use wolfcraft_engine::magic::{self, ALL_ENCH};
            let Some(q) = args.first().map(|a| a.to_lowercase()) else {
                let names: Vec<&str> = ALL_ENCH.iter().map(|e| e.name()).collect();
                return vec![sys(format!("Чары: {}", names.join(", ")))];
            };
            let Some(e) = ALL_ENCH.iter().copied().find(|e| e.name().to_lowercase().starts_with(&q)) else { return vec![err("Нет таких чар. /enchant — список")] };
            let lvl = args.get(1).and_then(|l| l.parse::<u8>().ok()).unwrap_or(e.max_level()).clamp(1, e.max_level());
            let Some(st) = s.player.inventory.slots[s.player.inventory.selected].as_mut() else { return vec![err("Возьмите предмет в руку")] };
            if !e.fits(magic::kind_of(st.id)) && st.id != wolfcraft_engine::items::ids::ENCH_BOOK {
                return vec![err(format!("{} не подходит для «{}»", e.name(), st.info().name))];
            }
            st.ench = magic::with(st.ench, e, lvl);
            vec![sys(format!("Зачаровано: {}", st.display_name()))]
        }
        "effect" | "эффект" => {
            use wolfcraft_engine::magic::ALL_EFFECTS;
            let Some(q) = args.first().map(|a| a.to_lowercase()) else {
                let names: Vec<&str> = ALL_EFFECTS.iter().map(|e| e.name()).collect();
                return vec![sys(format!("Эффекты: {}", names.join(", ")))];
            };
            if q == "clear" || q == "снять" {
                s.player.effects.clear();
                return vec![sys("Эффекты сняты")];
            }
            let Some(e) = ALL_EFFECTS.iter().copied().find(|e| e.name().to_lowercase().starts_with(&q)) else { return vec![err("Нет такого эффекта. /effect — список")] };
            let secs = args.get(1).and_then(|l| l.parse::<f32>().ok()).unwrap_or(e.duration().max(30.0));
            s.player.apply_effect(e, 1, secs);
            vec![sys(format!("Эффект: {}", e.name()))]
        }
        "month" | "месяц" => {
            use wolfcraft_engine::player::{DAY_LENGTH, MONTH_NAMES, month_start};
            let Some(a) = args.first().map(|a| a.to_lowercase()) else {
                return vec![sys(format!("Сейчас: {}", s.season_label()))];
            };
            let idx = a.parse::<usize>().ok().filter(|n| (1..=12).contains(n)).map(|n| n - 1).or_else(|| {
                let en = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
                MONTH_NAMES.iter().position(|m| a.chars().count() >= 3 && m.to_lowercase().starts_with(&a.chars().take(3).collect::<String>())).or_else(|| en.iter().position(|e| a.starts_with(e)))
            });
            let Some(idx) = idx else { return vec![err("Месяцы: январь … декабрь или 1–12")] };
            // Keep the time of day, move to the 1st of that month.
            let t = s.player.world_time;
            s.player.world_time = month_start(t, idx) + t.rem_euclid(DAY_LENGTH);
            vec![sys(format!("Дата: {}", s.season_label()))]
        }
        "season" | "сезон" => {
            use wolfcraft_engine::player::{DAY_LENGTH, SEASON_DAYS};
            let year = DAY_LENGTH * SEASON_DAYS * 4.0;
            let idx = match args.first().map(|a| a.to_lowercase()) {
                None => return vec![sys(format!("Сейчас: {}", s.season_label()))],
                Some(a) if a.starts_with("вес") || a == "spring" => 0.0,
                Some(a) if a.starts_with("лет") || a == "summer" => 1.0,
                Some(a) if a.starts_with("осе") || a == "autumn" => 2.0,
                Some(a) if a.starts_with("зим") || a == "winter" => 3.0,
                Some(_) => return vec![err("Сезоны: весна, лето, осень, зима")],
            };
            // Keep the time of day, move to the middle of the chosen season of this year.
            let t = s.player.world_time;
            let year_start = (t / year).floor() * year;
            let day = t.rem_euclid(DAY_LENGTH);
            s.player.world_time = year_start + (idx + 0.5) * DAY_LENGTH * SEASON_DAYS - (idx + 0.5) * DAY_LENGTH * SEASON_DAYS % DAY_LENGTH + day;
            vec![sys(format!("Время года: {}", s.season_label()))]
        }
        "biome" => {
            let col = s.planet.column(s.player.up());
            vec![sys(format!("Биом: {} · температура {:.0}% · влажность {:.0}%", col.biome.label(), col.temp * 100.0, col.humid * 100.0))]
        }
        "save" => {
            s.save();
            vec![sys("Мир сохранён")]
        }
        other => vec![err(format!("Неизвестная команда: /{other}. Введите /help"))],
    }
}

fn teleport_spawn(s: &mut GameSession) -> Reply {
    let spawn = s.player.spawn;
    let dir = wolfcraft_engine::cubesphere::col_dir(spawn);
    let pos = s.planet.stand_position(dir).unwrap_or(spawn);
    s.teleport(pos);
    vec![sys("Вы в точке появления")]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{LoadRequest, start};
    use wolfcraft_engine::block::Block;
    use wolfcraft_engine::config::{PlanetSize, WorldConfig};

    fn load(name: &str) -> GameSession {
        let dir = std::env::temp_dir().join(format!("wc_cmd_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cfg = WorldConfig::new("Чат", 77);
        cfg.size = PlanetSize::Small;
        cfg.structures = false;
        let sh = start(LoadRequest { cfg, worlds_dir: dir, is_new: true });
        while !sh.is_done() {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        GameSession::new(sh.take_result().unwrap().unwrap())
    }

    #[test]
    fn coordinate_and_time_parsing() {
        assert_eq!(parse_coord("12.5", 0.0), Some(12.5));
        assert_eq!(parse_coord("~", 7.0), Some(7.0));
        assert_eq!(parse_coord("~-3", 7.0), Some(4.0));
        assert_eq!(parse_coord("abc", 0.0), None);
        assert_eq!(parse_time("полдень"), Some(DAY_LENGTH * 0.5));
        assert_eq!(parse_time("6"), Some(DAY_LENGTH * 0.25));
        assert_eq!(parse_time("18:30"), Some((18.5 / 24.0) * DAY_LENGTH));
        assert_eq!(parse_time("25"), None);
        assert_eq!(parse_time("ничего"), None);
        assert_eq!(parse_mode("с"), None);
        assert_eq!(parse_mode("творческий"), Some(GameMode::Creative));
        assert_eq!(parse_difficulty("Сложная"), Some(Difficulty::Hard));
    }

    #[test]
    fn worldedit_sets_copies_pastes_and_undoes() {
        let mut s = load("we");
        assert_eq!(run(&mut s, "//set камень")[0].0, ChatKind::Error, "survival: no WorldEdit");
        run(&mut s, "/gm творческий");
        let t = std::time::Instant::now();
        while s.chunks.get_block(s.chunks.grid().world_to_cell(s.player.body.pos)).is_none() {
            s.update(&crate::session::InputFrame::default(), 0.016, 3, true);
            assert!(t.elapsed().as_secs() < 60);
        }
        let g = s.chunks.grid();
        let me = g.world_to_cell(s.player.body.pos);
        // A 3×3×2 box of stone two blocks above the head.
        s.we.pos1 = g.offset(me, 1, 1, 3);
        s.we.pos2 = g.offset(me, 3, 3, 4);
        assert!(run(&mut s, "//size")[0].1.contains("18"));
        let r = run(&mut s, "//set stone");
        assert_eq!(r[0].0, ChatKind::System, "{r:?}");
        let at = |s: &GameSession, di, dj, dk| s.chunks.get_block(g.offset(me, di, dj, dk).unwrap());
        assert_eq!(at(&s, 2, 2, 3), Some(Block::Stone));
        // Replace, copy and paste further along.
        run(&mut s, "//replace камень кирпичи");
        assert_eq!(at(&s, 1, 1, 4), Some(Block::Bricks));
        run(&mut s, "//copy");
        s.player.body.pos = g.cell_center(g.offset(me, 0, 6, 0).unwrap());
        run(&mut s, "//paste -a");
        assert_eq!(at(&s, 2, 8, 3), Some(Block::Bricks), "pasted six blocks over");
        // Undo the paste, then the replace.
        run(&mut s, "//undo");
        assert_ne!(at(&s, 2, 8, 3), Some(Block::Bricks));
        run(&mut s, "//undo");
        assert_eq!(at(&s, 2, 2, 3), Some(Block::Stone));
        run(&mut s, "//redo");
        assert_eq!(at(&s, 2, 2, 3), Some(Block::Bricks));
        assert_eq!(run(&mut s, "//ой")[0].0, ChatKind::Error);
    }

    #[test]
    fn chat_commands_change_the_game() {
        let mut s = load("a");
        // Plain text is a chat message.
        let r = run(&mut s, "привет всем");
        assert_eq!(r, vec![(ChatKind::Player, "<Игрок> привет всем".to_string())]);
        // Unknown command.
        assert_eq!(run(&mut s, "/ой")[0].0, ChatKind::Error);
        // Give.
        let r = run(&mut s, "/give лампа 20");
        assert_eq!(r[0].0, ChatKind::System, "{r:?}");
        assert_eq!(s.player.inventory.count(Block::Lamp as u16), 20);
        assert_eq!(run(&mut s, "/give железная кирка")[0].0, ChatKind::System);
        assert_eq!(s.player.inventory.count(items::ids::tool(2, 0)), 1);
        assert_eq!(run(&mut s, "/give нетакого 5")[0].0, ChatKind::Error);
        // Game mode & flight.
        assert_eq!(run(&mut s, "/fly")[0].0, ChatKind::Error, "survival cannot fly");
        run(&mut s, "/gm творческий");
        assert_eq!(s.cfg.mode, GameMode::Creative);
        assert_eq!(s.player.mode, GameMode::Creative);
        run(&mut s, "/fly");
        assert!(s.player.body.flying);
        run(&mut s, "/gamemode survival");
        assert!(!s.player.body.flying);
        // Time.
        run(&mut s, "/time set полдень");
        assert_eq!(s.clock(), "12:00");
        run(&mut s, "/time add 6");
        assert_eq!(s.clock(), "18:00");
        // Clear.
        run(&mut s, "/clear");
        assert!(s.player.inventory.is_empty());
        // Difficulty.
        run(&mut s, "/difficulty сложная");
        assert_eq!(s.cfg.difficulty, Difficulty::Hard);
        assert!(!run(&mut s, "/help").is_empty());
    }

    #[test]
    fn teleport_commands_move_the_player_to_valid_ground() {
        let mut s = load("tp");
        let before = s.player.body.pos;
        let r = run(&mut s, "/tp 10 -5");
        assert_eq!(r[0].0, ChatKind::System, "{r:?}");
        let p = s.player.body.pos;
        assert!((p - before).length() > 50.0, "teleported somewhere else");
        assert!(p.length() > s.planet.radius - 5.0, "{}", p.length());
        let dir = p.normalize();
        assert!((dir.y.asin().to_degrees() - 10.0).abs() < 1.0, "latitude");
        assert!((dir.z.atan2(dir.x).to_degrees() + 5.0).abs() < 1.0, "longitude");
        // Free-space check at the destination (not inside solid terrain).
        assert!(!s.planet.block_at_world(p).is_solid());
        // Relative coordinates.
        let cur = s.player.body.pos;
        run(&mut s, "/tp ~ ~20 ~");
        assert!((s.player.body.pos.y - (cur.y + 20.0)).abs() < 60.0);
        run(&mut s, "/spawn");
        assert!((s.player.body.pos - s.player.spawn).length() < 6.0);
        assert_eq!(run(&mut s, "/tp 1 2 3 4")[0].0, ChatKind::Error);
        assert_eq!(run(&mut s, "/tp 95 0")[0].0, ChatKind::Error);
    }
}
