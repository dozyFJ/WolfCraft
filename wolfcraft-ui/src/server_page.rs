//! The launcher's «Сервер» page: tabs under its toolbar — Дашборд, Плагины, Моды, Ядро,
//! Настройки, Консоль, Логи. Files live in `<data>/server/` in the same layout
//! as WolfServer (server.toml, plugins/<id>/, modules/<id>.lua, core/<id>/), so a set-up
//! made here also works on the PC server.

use crate::gui_renderer::GuiIcons;
use crate::theme_manager::{Palette, bold_font};
use crate::ui_components::{ButtonKind, pill_button};
use egui::{Color32, FontId, Frame, Id, Margin, Rounding, Stroke, Ui, Vec2};
use std::path::{Path, PathBuf};
use wolfcraft_assets::gui as icon;

pub const TABS: [&str; 7] = ["Дашборд", "Плагины", "Моды", "Ядро", "Настройки", "Консоль", "Логи"];

/// Keep the place profile apart from the ordinary server, preserving existing server files.
pub fn profile_data(data: &Path, place: bool) -> PathBuf {
    if place { data.join("place") } else { data.to_path_buf() }
}

/// The server's folder (created on first use).
pub fn root(data: &Path) -> PathBuf {
    let r = data.join("server");
    for f in ["plugins", "modules", "core", "data"] {
        let _ = std::fs::create_dir_all(r.join(f));
    }
    r
}

fn config_path(data: &Path) -> PathBuf {
    root(data).join("server.toml")
}

/// A value from server.toml (`key = value`, quotes stripped).
pub fn config_value(data: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(config_path(data)).ok()?;
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
    })
}

/// Sets `key = value` in server.toml (`value` is written as is: quote strings yourself).
pub fn set_config(data: &Path, key: &str, value: &str) {
    let path = config_path(data);
    let mut lines: Vec<String> = std::fs::read_to_string(&path).unwrap_or_default().lines().map(str::to_string).collect();
    let line = format!("{key} = {value}");
    match lines.iter().position(|l| l.split_once('=').is_some_and(|(k, _)| k.trim() == key)) {
        Some(i) => lines[i] = line,
        None => lines.insert(0, line),
    }
    let _ = std::fs::write(path, lines.join("\n") + "\n");
}

fn enabled_plugins(data: &Path) -> Vec<String> {
    config_value(data, "plugins").map(|v| v.trim_matches(|c| c == '[' || c == ']').split(',').map(|s| s.trim().trim_matches('"').to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default()
}

fn set_plugins(data: &Path, list: &[String]) {
    let v = format!("[{}]", list.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", "));
    set_config(data, "plugins", &v);
}

fn safe_id(s: &str) -> String {
    s.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect::<String>().trim_matches('_').chars().take(32).collect()
}

fn card(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Ui)) {
    Frame::none().fill(p.surface).rounding(Rounding::same(18.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

fn caption(ui: &mut Ui, p: &Palette, t: &str) {
    ui.label(egui::RichText::new(t).size(11.5).color(p.text_muted));
}

/// The tab strip right under the toolbar. Returns the chosen tab.
pub fn tab_bar(ui: &mut Ui, p: &Palette) -> usize {
    let id = Id::new("server-page-tab");
    let mut cur: usize = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(0);
    cur = cur.min(TABS.len() - 1);
    egui::ScrollArea::horizontal().id_salt("server-tabs").drag_to_scroll(true).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for (i, t) in TABS.iter().enumerate() {
                let on = i == cur;
                let g = ui.painter().layout_no_wrap(t.to_string(), FontId::proportional(14.0), if on { Color32::WHITE } else { p.text });
                let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 30.0, 40.0), egui::Sense::click());
                let round = Rounding::same(r.height() / 2.0);
                if on {
                    // Glossy Aero pill.
                    ui.painter().rect_filled(r.translate(Vec2::new(0.0, 2.0)), round, Color32::from_black_alpha(45));
                    ui.painter().rect_filled(r, round, p.accent);
                    let shine = egui::Rect::from_min_max(r.min + Vec2::new(r.height() * 0.4, 2.5), egui::pos2(r.right() - r.height() * 0.4, r.center().y));
                    ui.painter().rect_filled(shine, Rounding::same(shine.height() / 2.0), Color32::from_white_alpha(45));
                    ui.painter().rect_stroke(r, round, egui::Stroke::new(1.0_f32, Color32::from_white_alpha(110)));
                } else {
                    ui.painter().rect_filled(r, round, if resp.hovered() { p.surface_hover } else { Color32::from_black_alpha(if p.dark { 70 } else { 20 }) });
                    ui.painter().rect_stroke(r, round, egui::Stroke::new(1.0_f32, Color32::from_white_alpha(if p.dark { 40 } else { 120 })));
                }
                ui.painter().galley(egui::pos2(r.left() + 15.0, r.center().y - g.size().y / 2.0), g, p.text);
                if resp.clicked() {
                    cur = i;
                }
            }
        });
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, cur));
    cur
}

/// One of the tabs (0, «Дашборд», is drawn by the launcher itself).
pub fn page(ui: &mut Ui, icons: &GuiIcons, p: &Palette, data: &Path, tab: usize) {
    match tab {
        1 => plugins(ui, icons, p, data),
        2 => modules(ui, icons, p, data),
        3 => core(ui, icons, p, data),
        4 => settings(ui, icons, p, data),
        5 => console(ui, icons, p, data),
        6 => logs(ui, icons, p, data),
        _ => {},
    }
}

fn plugins(ui: &mut Ui, icons: &GuiIcons, p: &Palette, data: &Path) {
    let dir = root(data).join("plugins");
    let mut on = enabled_plugins(data);
    let mut list: Vec<String> = std::fs::read_dir(&dir).map(|rd| rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
    list.sort();
    let new_id = Id::new(("server-new-plugin", data));
    let mut name: String = ui.ctx().data(|d| d.get_temp(new_id)).unwrap_or_default();
    card(ui, p, |ui| {
        caption(ui, p, "НОВЫЙ ПЛАГИН");
        let w = ui.available_width();
        crate::ui_components::text_input(ui, icons, &mut name, "ID, например pvp-arena", w, Some(icon::PLUS));
        ui.add_space(6.0);
        if pill_button(ui, icons, Some(icon::CHECK), "Создать", ButtonKind::Primary, w, !safe_id(&name).is_empty()).clicked() {
            let id = safe_id(&name);
            let d = dir.join(&id);
            let _ = std::fs::create_dir_all(&d);
            let _ = std::fs::write(d.join("plugin.toml"), format!("name = \"{id}\"\nversion = \"0.1.0\"\n"));
            let _ = std::fs::write(d.join("main.lua"), "-- Плагин сервера: события игроков и мира\nfunction on_join(player)\n  say(player .. \" зашёл на сервер\")\nend\n");
            name.clear();
        }
    });
    if list.is_empty() {
        ui.label(egui::RichText::new("Плагинов пока нет. Плагин — папка с plugin.toml и main.lua.").color(p.text_muted));
    }
    for id in list {
        card(ui, p, |ui| {
            ui.horizontal(|ui| {
                let mut enabled = on.contains(&id);
                if ui.checkbox(&mut enabled, "").changed() {
                    if enabled {
                        on.push(id.clone());
                    } else {
                        on.retain(|x| x != &id);
                    }
                    set_plugins(data, &on);
                }
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&id).font(bold_font(15.0)).color(p.text));
                    ui.label(egui::RichText::new(if enabled { "Включён" } else { "Выключен" }).size(12.0).color(if enabled { p.success } else { p.text_muted }));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if pill_button(ui, icons, Some(icon::TRASH), "", ButtonKind::Danger, 44.0, true).clicked() {
                        let _ = std::fs::remove_dir_all(dir.join(&id));
                        on.retain(|x| x != &id);
                        set_plugins(data, &on);
                    }
                });
            });
        });
    }
    ui.ctx().data_mut(|d| d.insert_temp(new_id, name));
}

fn modules(ui: &mut Ui, icons: &GuiIcons, p: &Palette, data: &Path) {
    let dir = root(data).join("modules");
    let mut list: Vec<String> = std::fs::read_dir(&dir).map(|rd| rd.flatten().filter_map(|e| e.file_name().to_string_lossy().strip_suffix(".lua").map(str::to_string)).collect()).unwrap_or_default();
    list.sort();
    let new_id = Id::new(("server-new-module", data));
    let mut name: String = ui.ctx().data(|d| d.get_temp(new_id)).unwrap_or_default();
    card(ui, p, |ui| {
        caption(ui, p, "НОВЫЙ МОД");
        ui.label(egui::RichText::new("Мод — Luau-модуль: общие функции для плагинов и мини-игр.").size(12.5).color(p.text_muted));
        let w = ui.available_width();
        crate::ui_components::text_input(ui, icons, &mut name, "ID, например economy", w, Some(icon::CODE));
        ui.add_space(6.0);
        if pill_button(ui, icons, Some(icon::CHECK), "Создать", ButtonKind::Primary, w, !safe_id(&name).is_empty()).clicked() {
            let id = safe_id(&name);
            let _ = std::fs::write(dir.join(format!("{id}.lua")), format!("-- Мод «{id}»\nlocal M = {{}}\n\nfunction M.hello()\n  return \"привет из {id}\"\nend\n\nreturn M\n"));
            name.clear();
        }
    });
    for id in list {
        card(ui, p, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("{id}.lua")).font(bold_font(15.0)).color(p.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if pill_button(ui, icons, Some(icon::TRASH), "", ButtonKind::Danger, 44.0, true).clicked() {
                        let _ = std::fs::remove_file(dir.join(format!("{id}.lua")));
                    }
                });
            });
        });
    }
    ui.ctx().data_mut(|d| d.insert_temp(new_id, name));
}

fn core(ui: &mut Ui, icons: &GuiIcons, p: &Palette, data: &Path) {
    let cur = config_value(data, "core").unwrap_or_else(|| "builtin".into());
    let mut cores: Vec<(String, String)> = vec![("builtin".into(), "WolfCraft — встроенное ядро".into())];
    if let Ok(rd) = std::fs::read_dir(root(data).join("core")) {
        for e in rd.flatten().filter(|e| e.path().is_dir()) {
            let id = e.file_name().to_string_lossy().to_string();
            cores.push((id.clone(), format!("Своё ядро «{id}»")));
        }
    }
    card(ui, p, |ui| {
        caption(ui, p, "ЯДРО СЕРВЕРА");
        ui.label(egui::RichText::new("Ядро задаёт правила мира: встроенное — обычный WolfCraft. Свои ядра лежат в папке server/core.").size(12.5).color(p.text_muted));
        ui.add_space(6.0);
        for (id, name) in &cores {
            let on = &cur == id;
            let w = ui.available_width();
            if pill_button(ui, icons, Some(if on { icon::CHECK } else { icon::CUBE }), name, if on { ButtonKind::Primary } else { ButtonKind::Secondary }, w, true).clicked() {
                set_config(data, "core", &format!("\"{id}\""));
            }
            ui.add_space(4.0);
        }
        ui.label(egui::RichText::new(format!("Версия: WolfCraft {}", wolfcraft_common::VERSION)).size(12.0).color(p.text_muted));
    });
}

fn settings(ui: &mut Ui, icons: &GuiIcons, p: &Palette, data: &Path) {
    let id = Id::new(("server-settings-form", data));
    let (mut name, mut motd, mut max, mut pvp): (String, String, f32, bool) = ui.ctx().data(|d| d.get_temp(id)).unwrap_or_else(|| {
        (
            config_value(data, "name").unwrap_or_else(|| "Мой сервер".into()),
            config_value(data, "motd").unwrap_or_else(|| "Добро пожаловать!".into()),
            config_value(data, "max_players").and_then(|v| v.parse().ok()).unwrap_or(16.0),
            config_value(data, "pvp").map(|v| v == "true").unwrap_or(true),
        )
    });
    card(ui, p, |ui| {
        caption(ui, p, "НАСТРОЙКИ СЕРВЕРА");
        let w = ui.available_width();
        ui.label(egui::RichText::new("Название").color(p.text));
        crate::ui_components::text_input(ui, icons, &mut name, "Мой сервер", w, None);
        ui.label(egui::RichText::new("Приветствие").color(p.text));
        crate::ui_components::text_input(ui, icons, &mut motd, "Добро пожаловать!", w, None);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Игроков максимум").color(p.text));
            ui.add(egui::Slider::new(&mut max, 1.0..=64.0).step_by(1.0));
        });
        ui.checkbox(&mut pvp, egui::RichText::new("PvP: игроки могут бить друг друга").color(p.text));
        ui.add_space(6.0);
        if pill_button(ui, icons, Some(icon::CHECK), "Сохранить", ButtonKind::Primary, w, true).clicked() {
            let q = |s: &str| format!("\"{}\"", s.replace(['"', '\\'], ""));
            set_config(data, "name", &q(&name));
            set_config(data, "motd", &q(&motd));
            set_config(data, "max_players", &(max as u32).to_string());
            set_config(data, "pvp", if pvp { "true" } else { "false" });
        }
        ui.label(egui::RichText::new(format!("Порт: {}", wolfcraft_net_port())).size(12.0).color(p.text_muted));
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, (name, motd, max, pvp)));
}

/// The game's server port (the same as WolfServer's).
fn wolfcraft_net_port() -> u16 {
    25570
}

fn read_log(data: &Path, filter: Option<&[&str]>, n: usize) -> String {
    let text = std::fs::read_to_string(data.join("logs").join("latest.log")).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| filter.is_none_or(|f| f.iter().any(|w| l.to_lowercase().contains(w)))).collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

fn logs(ui: &mut Ui, icons: &GuiIcons, p: &Palette, data: &Path) {
    let id = Id::new(("server-log-cache", data));
    let now = ui.ctx().input(|i| i.time);
    let mut cache: (f64, String) = ui.ctx().data(|d| d.get_temp(id)).unwrap_or((-10.0, String::new()));
    let refresh = pill_button(ui, icons, Some(icon::REFRESH), "Обновить", ButtonKind::Secondary, ui.available_width(), true).clicked();
    if refresh || now - cache.0 > 3.0 {
        cache = (now, read_log(data, None, 200));
        ui.ctx().data_mut(|d| d.insert_temp(id, cache.clone()));
    }
    card(ui, p, |ui| {
        caption(ui, p, "LOGS/LATEST.LOG");
        ui.label(egui::RichText::new(if cache.1.is_empty() { "Лог пуст".to_string() } else { cache.1.clone() }).font(FontId::monospace(11.0)).color(Color32::from_rgb(190, 198, 208)));
    });
}

fn console(ui: &mut Ui, icons: &GuiIcons, p: &Palette, data: &Path) {
    let auto = root(data).join("autoexec.txt");
    let id = Id::new(("server-console-line", data));
    let mut line: String = ui.ctx().data(|d| d.get_temp(id)).unwrap_or_default();
    card(ui, p, |ui| {
        caption(ui, p, "КОНСОЛЬ");
        let text = read_log(data, Some(&["server", "сервер", "net", "join", "игрок"]), 40);
        ui.label(egui::RichText::new(if text.is_empty() { "Пока тихо. Запустите сервер или мини-игру на вкладке «Дашборд».".to_string() } else { text }).font(FontId::monospace(11.0)).color(Color32::from_rgb(190, 198, 208)));
    });
    card(ui, p, |ui| {
        caption(ui, p, "КОМАНДЫ ПРИ ЗАПУСКЕ");
        ui.label(egui::RichText::new("Выполняются каждый раз, когда вы запускаете сервер: /time set день, /gamemode creative, приветствие в чат…").size(12.5).color(p.text_muted));
        let w = ui.available_width();
        crate::ui_components::text_input(ui, icons, &mut line, "/time set день", w, Some(icon::CODE));
        ui.add_space(6.0);
        if pill_button(ui, icons, Some(icon::PLUS), "Добавить", ButtonKind::Primary, w, !line.trim().is_empty()).clicked() {
            let mut text = std::fs::read_to_string(&auto).unwrap_or_default();
            text.push_str(line.trim());
            text.push('\n');
            let _ = std::fs::write(&auto, text);
            line.clear();
        }
        let cmds: Vec<String> = std::fs::read_to_string(&auto).unwrap_or_default().lines().map(str::to_string).filter(|l| !l.trim().is_empty()).collect();
        let mut remove = None;
        for (i, c) in cmds.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(c).font(FontId::monospace(12.5)).color(p.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").clicked() {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = remove {
            let rest: Vec<&String> = cmds.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, c)| c).collect();
            let _ = std::fs::write(&auto, rest.iter().map(|c| format!("{c}\n")).collect::<String>());
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, line));
}

/// Commands to run when the player's own server starts.
pub fn autoexec(data: &Path) -> Vec<String> {
    std::fs::read_to_string(root(data).join("autoexec.txt")).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

/// Hosting statistics of one profile (server or mini-games): samples every 10 s while it
/// runs (`unix_time players`) in `server/stats.txt`, and launches in `server/runs.txt`.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub samples: Vec<(u64, u32)>,
    pub runs: Vec<u64>,
}

pub fn read_stats(data: &Path) -> Stats {
    let r = data.join("server");
    let nums = |f: &str| -> Vec<Vec<u64>> { std::fs::read_to_string(r.join(f)).unwrap_or_default().lines().map(|l| l.split_whitespace().filter_map(|x| x.parse().ok()).collect()).collect() };
    Stats {
        samples: nums("stats.txt").into_iter().filter(|v| v.len() >= 2).map(|v| (v[0], v[1] as u32)).collect(),
        runs: nums("runs.txt").into_iter().filter_map(|v| v.first().copied()).collect(),
    }
}

/// Adds a sample (keeps the last 2000, about 5½ hours of hosting).
pub fn add_sample(data: &Path, t: u64, players: u32) {
    let f = root(data).join("stats.txt");
    let mut lines: Vec<String> = std::fs::read_to_string(&f).unwrap_or_default().lines().map(String::from).collect();
    lines.push(format!("{t} {players}"));
    let skip = lines.len().saturating_sub(2000);
    let _ = std::fs::write(&f, lines[skip..].join("\n"));
}

/// Notes a launch (keeps the last 500).
pub fn add_run(data: &Path, t: u64) {
    let f = root(data).join("runs.txt");
    let mut lines: Vec<String> = std::fs::read_to_string(&f).unwrap_or_default().lines().map(String::from).collect();
    lines.push(t.to_string());
    let skip = lines.len().saturating_sub(500);
    let _ = std::fs::write(&f, lines[skip..].join("\n"));
}

/// Max players from server.toml (16 if not set).
pub fn max_players(data: &Path) -> usize {
    config_value(data, "max_players").and_then(|v| v.parse().ok()).unwrap_or(16).clamp(1, 64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_and_place_profiles_keep_their_settings_and_files_separate() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let data = std::env::temp_dir().join(format!("wolf-host-profiles-{}-{unique}", std::process::id()));
        let server = profile_data(&data, false);
        let place = profile_data(&data, true);
        set_config(&server, "max_players", "24");
        set_config(&place, "max_players", "8");
        set_config(&server, "core", "\"survival\"");
        set_config(&place, "core", "\"arena\"");
        set_plugins(&server, &["claims".into()]);
        set_plugins(&place, &["rounds".into()]);
        std::fs::write(root(&server).join("autoexec.txt"), "/time set день\n").unwrap();
        std::fs::write(root(&place).join("autoexec.txt"), "/gamemode creative\n").unwrap();
        assert_eq!(max_players(&server), 24);
        assert_eq!(max_players(&place), 8);
        assert_eq!(config_value(&server, "core").as_deref(), Some("survival"));
        assert_eq!(config_value(&place, "core").as_deref(), Some("arena"));
        assert_eq!(enabled_plugins(&server), vec!["claims"]);
        assert_eq!(enabled_plugins(&place), vec!["rounds"]);
        assert_eq!(autoexec(&server), vec!["/time set день"]);
        assert_eq!(autoexec(&place), vec!["/gamemode creative"]);
        assert!(data.join("server/server.toml").is_file());
        assert!(data.join("place/server/server.toml").is_file());
        std::fs::remove_dir_all(data).unwrap();
    }
}
