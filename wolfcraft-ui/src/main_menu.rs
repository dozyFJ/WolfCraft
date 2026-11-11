//! Menu screens: main menu, world selection, world creation, settings, loading, pause.

use crate::gui_renderer::GuiIcons;
use crate::theme_manager::{ANIM_TIME, Palette, RADIUS_LG, RADIUS_MD, bold_font, palette};
use crate::ui_components::*;
use egui::{Align2, Color32, FontId, Frame, Margin, Pos2, Rect, Rounding, Sense, Stroke, TextureHandle, Ui, Vec2};
use wolfcraft_assets::gui as icon;
use wolfcraft_common::platform::format_timestamp;
use wolfcraft_engine::config::*;

#[derive(Clone, Debug, PartialEq)]
pub enum MenuAction {
    None,
    OpenWorldSelect,
    OpenCreate,
    OpenSettings,
    Quit,
    Back,
    PlayWorld(String),
    /// Connect to a WolfServer at this address.
    JoinServer(String),
    /// Start a server on this device and join it.
    HostServer,
    CreateWorld(Box<WorldConfig>),
    DeleteWorld(String),
    RenameWorld(String, String),
    ToggleTheme,
    Resume,
    SaveAndQuit,
    SaveGame,
}

pub struct WorldEntry {
    pub cfg: WorldConfig,
    pub preview: Option<TextureHandle>,
}

// ---------------------------------------------------------------------------------------------
// Shared chrome

/// Top bar: logo + name left, version + settings right. Returns true if settings was clicked.
pub fn top_bar(ctx: &egui::Context, icons: &GuiIcons, title: &str, version: &str, show_settings: bool) -> bool {
    let p = palette(ctx);
    let mut settings = false;
    egui::TopBottomPanel::top("menu-topbar")
        .exact_height(58.0)
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(2.0_f32, p.outline)).inner_margin(Margin::symmetric(18.0, 8.0)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                icons.paint(ui.painter(), r, icon::LOGO, Color32::WHITE);
                ui.add_space(4.0);
                ui.label(egui::RichText::new("WolfCraft").font(bold_font(21.0)).color(p.text));
                if !title.is_empty() {
                    ui.label(egui::RichText::new("›").size(18.0).color(p.text_muted));
                    ui.label(egui::RichText::new(title).size(16.0).color(p.text_muted));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if show_settings && icon_button(ui, icons, icon::GEAR, 36.0, ButtonKind::Ghost).on_hover_text("Настройки").clicked() {
                        settings = true;
                    }
                    badge(ui, &format!("v{version}"), p.accent);
                });
            });
        });
    settings
}

fn body_frame(p: &Palette) -> Frame {
    Frame::none().fill(p.bg).inner_margin(Margin::symmetric(24.0, 18.0))
}

fn bottom_bar<R>(ctx: &egui::Context, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = palette(ctx);
    egui::TopBottomPanel::bottom("menu-bottombar")
        .exact_height(66.0)
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(2.0_f32, p.outline)).inner_margin(Margin::symmetric(24.0, 12.0)))
        .show(ctx, |ui| ui.horizontal_centered(add).inner)
        .inner
}

fn decor(ui: &Ui, icons: &GuiIcons, p: &Palette) {
    let rect = ui.max_rect();
    let c = Pos2::new(rect.right() - rect.width() * 0.22, rect.center().y + 20.0);
    for (i, r) in [140.0, 210.0, 290.0, 380.0, 480.0].iter().enumerate() {
        ui.painter().circle_stroke(c, *r, Stroke::new(2.0_f32, p.outline.gamma_multiply(0.55 - i as f32 * 0.08)));
    }
    let size = 300.0;
    icons.paint(ui.painter(), Rect::from_center_size(c, Vec2::splat(size)), icon::LOGO, Color32::WHITE.gamma_multiply(if p.dark { 0.12 } else { 0.22 }));
}

// ---------------------------------------------------------------------------------------------
// Main menu

/// Space backdrop with a big planet rising from the bottom edge (the planet of the last played world).
fn planet_backdrop(ui: &Ui, planet: Option<&TextureHandle>, p: &Palette) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(2, 7, 13));
    // Twinkling stars at fixed pseudo-random spots.
    let t = ui.input(|i| i.time) as f32;
    for i in 0..140u32 {
        let h = i.wrapping_mul(2654435761).rotate_left(7) ^ 0x9e37_79b9;
        let x = (h % 10_000) as f32 / 10_000.0;
        let y = ((h / 10_000) % 10_000) as f32 / 10_000.0;
        let tw = 0.55 + 0.45 * (t * (0.6 + (h % 7) as f32 * 0.2) + i as f32).sin();
        let r = if h % 11 == 0 { 1.6 } else { 1.0 };
        painter.circle_filled(Pos2::new(rect.left() + x * rect.width(), rect.top() + y * rect.height()), r, Color32::from_white_alpha((150.0 * tw) as u8));
    }
    if let Some(tex) = planet {
        // Slow drift up and down so the screen feels alive.
        let d = rect.width().max(rect.height() * 1.6);
        let c = Pos2::new(rect.center().x, rect.bottom() + d * 0.18 + (t * 0.25).sin() * 6.0);
        painter.image(tex.id(), Rect::from_center_size(c, Vec2::splat(d)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }
    // Soft shade so the menu stays readable.
    painter.rect_filled(rect, Rounding::ZERO, p.bg.gamma_multiply(if p.dark { 0.25 } else { 0.15 }));
    ui.ctx().request_repaint();
}

pub fn main_menu(ctx: &egui::Context, icons: &GuiIcons, version: &str, data_dir: &str, dark: bool, planet: Option<&TextureHandle>) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
    // The header strip is for the PC window only; phones use the whole screen.
    if !cfg!(target_os = "android") && top_bar(ctx, icons, "", version, true) {
        action = MenuAction::OpenSettings;
    }
    egui::CentralPanel::default().frame(Frame::none().fill(p.bg)).show(ctx, |ui| {
        let rect = ui.max_rect();
        planet_backdrop(ui, planet, &p);
        let col = Rect::from_center_size(rect.center(), Vec2::new(360.0, rect.height().min(400.0)));
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(col), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                let (r, _) = ui.allocate_exact_size(Vec2::splat(88.0), Sense::hover());
                icons.paint(ui.painter(), r, icon::LOGO, Color32::WHITE);
                ui.add_space(6.0);
                ui.label(egui::RichText::new("WolfCraft").font(bold_font(40.0)).color(Color32::WHITE));
                ui.label(egui::RichText::new("Исследуйте целую планету из блоков").size(15.0).color(Color32::from_gray(200)));
                ui.add_space(26.0);
                ui.spacing_mut().item_spacing.y = 12.0;
                if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, 320.0, true).clicked() {
                    action = MenuAction::OpenWorldSelect;
                }
                if pill_button(ui, icons, Some(icon::GEAR), "Настройки", ButtonKind::Secondary, 320.0, true).clicked() {
                    action = MenuAction::OpenSettings;
                }
                if pill_button(ui, icons, Some(icon::EXIT), "Выход", ButtonKind::Danger, 320.0, true).clicked() {
                    action = MenuAction::Quit;
                }
            }));
        });
        // Footer.
        if !cfg!(target_os = "android") {
        ui.painter().text(Pos2::new(rect.left() + 8.0, rect.bottom() - 12.0), Align2::LEFT_BOTTOM, format!("Данные: {data_dir}"), FontId::proportional(12.0), p.text_muted);
        }
        let tb = Rect::from_center_size(Pos2::new(rect.right() - 28.0, rect.bottom() - 28.0), Vec2::splat(38.0));
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tb), |ui| {
            if icon_button(ui, icons, if dark { icon::SUN } else { icon::MOON }, 38.0, ButtonKind::Secondary).on_hover_text(if dark { "Светлая тема" } else { "Тёмная тема" }).clicked() {
                action = MenuAction::ToggleTheme;
            }
        });
    });
    action
}

// ---------------------------------------------------------------------------------------------
// World selection

#[derive(Default)]
pub struct WorldSelectState {
    /// 0 = single player (saved worlds), 1 = play over the network.
    pub mode: usize,
    /// Address typed for a network game.
    pub address: String,
    /// Why the last connection attempt failed.
    pub net_error: Option<String>,
    pub search: String,
    /// Search field shown in the header (opened with the magnifier button).
    pub search_open: bool,
    /// Focus the search field on the next frame (just opened).
    pub search_focus: bool,
    /// Sort menu under the header sort button.
    pub sort_open: bool,
    pub sort: usize,
    pub selected: Option<String>,
    pub rename: Option<(String, String)>,
    pub confirm_delete: Option<String>,
}

/// The "play over the network" tab: LAN worlds and a server address.
fn online_tab(ui: &mut egui::Ui, icons: &GuiIcons, p: &crate::theme_manager::Palette, st: &mut WorldSelectState) -> Option<MenuAction> {
    let mut join = None;
    ui.label(egui::RichText::new("Подключиться к серверу").font(bold_font(18.0)).color(p.text));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        text_input(ui, icons, &mut st.address, "Адрес, например 192.168.0.10:25570", 420.0, Some(icon::GLOBE));
        ui.add_space(8.0);
        let ready = !st.address.trim().is_empty();
        if pill_button(ui, icons, Some(icon::PLAY), "Подключиться", ButtonKind::Primary, 0.0, ready).clicked() {
            st.net_error = None;
            join = Some(MenuAction::JoinServer(st.address.trim().to_string()));
        }
    });
    if let Some(e) = &st.net_error {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(e).size(14.0).color(p.danger));
    }
    ui.add_space(16.0);
    row_card(ui, false, |ui| {
        ui.label(egui::RichText::new("Игра по Wi‑Fi и в интернете").font(bold_font(16.0)).color(p.text));
        ui.label(egui::RichText::new("Запустите WolfServer на компьютере — он покажет адрес. Введите этот адрес здесь на ПК и на телефоне (в одной сети Wi‑Fi), и вы окажетесь в одном мире: видите друг друга, общие постройки и чат.").size(13.5).color(p.text_muted));
    });
    ui.add_space(12.0);
    row_card(ui, false, |ui| {
        ui.label(egui::RichText::new("Сервер на этом устройстве").font(bold_font(16.0)).color(p.text));
        ui.label(egui::RichText::new("Игра сама запустит сервер и войдёт в него. Адрес для друзей появится в чате. Работает и на телефоне: раздайте Wi‑Fi или будьте в одной сети.").size(13.5).color(p.text_muted));
        ui.add_space(6.0);
        if pill_button(ui, icons, Some(icon::GLOBE), "Создать сервер", ButtonKind::Secondary, 0.0, true).clicked() {
            st.net_error = None;
            join = Some(MenuAction::HostServer);
        }
    });
    join
}

pub fn sorted_filtered<'a>(worlds: &'a [WorldEntry], search: &str, sort: usize) -> Vec<&'a WorldEntry> {
    let q = search.trim().to_lowercase();
    let mut v: Vec<&WorldEntry> = worlds.iter().filter(|w| q.is_empty() || w.cfg.name.to_lowercase().contains(&q)).collect();
    match sort {
        1 => v.sort_by(|a, b| a.cfg.name.to_lowercase().cmp(&b.cfg.name.to_lowercase())),
        2 => v.sort_by(|a, b| b.cfg.created.cmp(&a.cfg.created)),
        _ => v.sort_by(|a, b| b.cfg.last_played.cmp(&a.cfg.last_played)),
    }
    v
}

const SORT_NAMES: [&str; 3] = ["Недавние", "По названию", "По созданию"];

/// Square outlined icon button for the header; `active` paints it in the accent colour.
fn square_button(ui: &mut Ui, icons: &GuiIcons, ic: u16, active: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::click());
    let (fill, stroke, fg) = if active {
        (p.accent, p.accent, p.on_accent)
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        (p.surface_hover, p.accent, p.text)
    } else {
        (p.surface_hi, p.outline, p.text)
    };
    ui.painter().rect_filled(rect, Rounding::ZERO, fill);
    ui.painter().rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0_f32, stroke));
    icons.paint(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(22.0)), ic, fg);
    resp
}

/// World-select header: logo and title left; settings, version, sort and search (square icons) right.
/// The magnifier opens a search field right in the header. Returns true if settings was clicked.
fn world_select_header(ctx: &egui::Context, icons: &GuiIcons, st: &mut WorldSelectState, version: &str) -> bool {
    let p = palette(ctx);
    let mut settings = false;
    let mut sort_anchor = None;
    egui::TopBottomPanel::top("menu-topbar")
        .exact_height(58.0)
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(2.0_f32, p.outline)).inner_margin(Margin::symmetric(18.0, 8.0)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                icons.paint(ui.painter(), r, icon::LOGO, Color32::WHITE);
                ui.add_space(4.0);
                ui.label(egui::RichText::new("WolfCraft").font(bold_font(21.0)).color(p.text));
                if !st.search_open {
                    ui.label(egui::RichText::new("›").size(18.0).color(p.text_muted));
                    ui.label(egui::RichText::new("Выбор мира").size(16.0).color(p.text_muted));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if square_button(ui, icons, icon::GEAR, false).clicked() {
                        settings = true;
                    }
                    if st.mode == 0 {
                        let sort = square_button(ui, icons, icon::SORT, st.sort_open);
                        if sort.clicked() {
                            st.sort_open = !st.sort_open;
                        }
                        sort_anchor = Some(sort.rect);
                        if square_button(ui, icons, if st.search_open { icon::CLOSE } else { icon::SEARCH }, st.search_open).clicked() {
                            st.search_open = !st.search_open;
                            st.search_focus = st.search_open;
                            if !st.search_open {
                                st.search.clear();
                            }
                        }
                        if st.search_open {
                            let w = (ui.available_width() - 8.0).clamp(140.0, 360.0);
                            let r = text_input(ui, icons, &mut st.search, "Поиск миров…", w, Some(icon::SEARCH));
                            if st.search_focus {
                                st.search_focus = false;
                                r.request_focus();
                            }
                        }
                    }
                    if !st.search_open {
                        badge(ui, &format!("v{version}"), p.accent);
                    }
                });
            });
        });
    // Sort menu drops down under its button.
    if st.mode == 0 && st.sort_open {
        if let Some(a) = sort_anchor {
            let area = egui::Area::new(egui::Id::new("world-sort-menu"))
                .order(egui::Order::Foreground)
                .fixed_pos(Pos2::new(a.right() - 200.0, a.bottom() + 8.0))
                .show(ctx, |ui| {
                    Frame::none().fill(p.surface).stroke(Stroke::new(2.0_f32, p.outline)).inner_margin(Margin::same(8.0)).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        for (i, name) in SORT_NAMES.iter().enumerate() {
                            let kind = if st.sort == i { ButtonKind::Primary } else { ButtonKind::Ghost };
                            if pill_button(ui, icons, if st.sort == i { Some(icon::CHECK) } else { None }, name, kind, 184.0, true).clicked() {
                                st.sort = i;
                                st.sort_open = false;
                            }
                        }
                    });
                });
            // Tap outside closes the menu.
            let clicked_outside = ctx.input(|i| i.pointer.any_click()) && !area.response.rect.contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default()) && !a.contains(ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default());
            if clicked_outside {
                st.sort_open = false;
            }
        }
    }
    settings
}

pub fn world_select(ctx: &egui::Context, icons: &GuiIcons, st: &mut WorldSelectState, worlds: &[WorldEntry], version: &str) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
    if world_select_header(ctx, icons, st, version) {
        action = MenuAction::OpenSettings;
    }
    bottom_bar(ctx, |ui| {
        if pill_button(ui, icons, Some(icon::ARROW_LEFT), "Назад", ButtonKind::Secondary, 140.0, true).clicked() {
            action = MenuAction::Back;
        }
        if st.mode == 0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if pill_button(ui, icons, Some(icon::PLUS), "Создать мир", ButtonKind::Primary, 180.0, true).clicked() {
                    action = MenuAction::OpenCreate;
                }
                let can = st.selected.as_ref().map(|s| worlds.iter().any(|w| &w.cfg.id == s)).unwrap_or(false);
                if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Secondary, 150.0, can).clicked() {
                    if let Some(s) = &st.selected {
                        action = MenuAction::PlayWorld(s.clone());
                    }
                }
            });
        }
    });
    // Left sidebar: play modes stacked vertically; their content fills the right side.
    egui::SidePanel::left("play-mode-side")
        .resizable(false)
        .exact_width(230.0)
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(2.0_f32, p.outline)).inner_margin(Margin::symmetric(14.0, 18.0)))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            for (i, ic, label) in [(0usize, icon::CUBE, "Одиночная игра"), (1, icon::GLOBE, "Игра по сети")] {
                let kind = if st.mode == i { ButtonKind::Primary } else { ButtonKind::Secondary };
                if pill_button(ui, icons, Some(ic), label, kind, ui.available_width(), true).clicked() {
                    st.mode = i;
                }
            }
        });
    egui::CentralPanel::default().frame(body_frame(&p)).show(ctx, |ui| {
        if st.mode == 1 {
            if let Some(a) = online_tab(ui, icons, &p, st) {
                action = a;
            }
            return;
        }
        let list = sorted_filtered(worlds, &st.search, st.sort);
        if list.is_empty() {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
                icons.paint(ui.painter(), r, icon::GLOBE, p.text_muted.gamma_multiply(0.5));
                ui.label(egui::RichText::new(if worlds.is_empty() { "Миров пока нет" } else { "Ничего не найдено" }).font(bold_font(20.0)).color(p.text));
                ui.label(egui::RichText::new(if worlds.is_empty() { "Создайте первую планету — нажмите «Создать мир»" } else { "Измените запрос поиска" }).color(p.text_muted));
            });
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for w in list {
                let selected = st.selected.as_deref() == Some(w.cfg.id.as_str());
                let resp = row_card(ui, selected, |ui| {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(76.0), Sense::hover());
                        ui.painter().rect_filled(r, Rounding::ZERO, Color32::from_rgb(8, 12, 22));
                        if let Some(tex) = &w.preview {
                            ui.painter().image(tex.id(), r.shrink(2.0), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                        }
                        ui.painter().rect_stroke(r, Rounding::ZERO, Stroke::new(2.0_f32, p.outline));
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&w.cfg.name).font(bold_font(18.0)).color(p.text));
                            ui.horizontal(|ui| {
                                badge(ui, if w.cfg.mode == GameMode::Creative { "Творческий" } else { "Выживание" }, if w.cfg.mode == GameMode::Creative { p.secondary } else { p.accent });
                                badge(ui, w.cfg.size.label(), p.text_muted);
                                badge(ui, w.cfg.world_type.label(), p.text_muted);
                            });
                            ui.label(egui::RichText::new(format!("Сохранён: {}   ·   Seed: {}", format_timestamp(w.cfg.last_played), w.cfg.seed as i64)).size(12.5).color(p.text_muted));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, icons, icon::TRASH, 34.0, ButtonKind::Danger).on_hover_text("Удалить").clicked() {
                                st.confirm_delete = Some(w.cfg.id.clone());
                            }
                            if icon_button(ui, icons, icon::EDIT, 34.0, ButtonKind::Ghost).on_hover_text("Переименовать").clicked() {
                                st.rename = Some((w.cfg.id.clone(), w.cfg.name.clone()));
                            }
                            if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, 0.0, true).clicked() {
                                action = MenuAction::PlayWorld(w.cfg.id.clone());
                            }
                        });
                    });
                });
                if resp.clicked() {
                    st.selected = Some(w.cfg.id.clone());
                }
                if resp.double_clicked() {
                    action = MenuAction::PlayWorld(w.cfg.id.clone());
                }
                ui.add_space(8.0);
            }
        });
    });

    if let Some((id, name)) = &mut st.rename {
        let mut close = false;
        let mut apply = false;
        let dismissed = modal(ctx, "rename-world", "Переименовать мир", 380.0, |ui| {
            text_input(ui, icons, name, "Название мира", 360.0, Some(icon::EDIT));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if pill_button(ui, icons, Some(icon::CHECK), "Сохранить", ButtonKind::Primary, 0.0, !name.trim().is_empty()).clicked() {
                    apply = true;
                }
                if pill_button(ui, icons, Some(icon::CLOSE), "Отмена", ButtonKind::Secondary, 0.0, true).clicked() {
                    close = true;
                }
            });
        });
        if apply {
            action = MenuAction::RenameWorld(id.clone(), name.trim().to_string());
        }
        if apply || close || dismissed {
            st.rename = None;
        }
    }
    if let Some(id) = st.confirm_delete.clone() {
        let name = worlds.iter().find(|w| w.cfg.id == id).map(|w| w.cfg.name.clone()).unwrap_or_default();
        let mut close = false;
        let dismissed = modal(ctx, "delete-world", "Удалить мир?", 400.0, |ui| {
            ui.label(egui::RichText::new(format!("«{name}» будет перемещён в корзину Windows. Его можно восстановить оттуда.")).color(p.text));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if pill_button(ui, icons, Some(icon::TRASH), "Удалить", ButtonKind::Danger, 0.0, true).clicked() {
                    action = MenuAction::DeleteWorld(id.clone());
                    close = true;
                }
                if pill_button(ui, icons, Some(icon::CLOSE), "Отмена", ButtonKind::Secondary, 0.0, true).clicked() {
                    close = true;
                }
            });
        });
        if close || dismissed {
            st.confirm_delete = None;
            if st.selected.as_deref() == Some(id.as_str()) {
                st.selected = None;
            }
        }
    }
    action
}

// ---------------------------------------------------------------------------------------------
// Create world

pub struct CreateWorldState {
    pub name: String,
    pub seed_text: String,
    pub mode: GameMode,
    pub size: PlanetSize,
    pub world_type: WorldType,
    pub continents: ContinentCount,
    pub structures: bool,
    pub difficulty: Difficulty,
    pub error: Option<String>,
    /// Set whenever a setting that changes the planet changed; the owner re-renders the preview.
    pub preview_dirty: bool,
    pub preview_seed: u64,
    /// Selected section tab (0 = Основное, 1 = Планета).
    pub tab: usize,
}

impl Default for CreateWorldState {
    fn default() -> Self {
        Self {
            name: "Новый мир".into(),
            seed_text: String::new(),
            mode: GameMode::Survival,
            size: PlanetSize::Medium,
            world_type: WorldType::Standard,
            continents: ContinentCount::Auto,
            structures: true,
            difficulty: Difficulty::Normal,
            error: None,
            preview_dirty: true,
            preview_seed: 0,
            tab: 0,
        }
    }
}

impl CreateWorldState {
    /// Builds the config; `random_seed` is used when the seed field is empty.
    pub fn to_config(&self, random_seed: u64) -> Result<WorldConfig, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Введите название мира".into());
        }
        if name.chars().count() > 40 {
            return Err("Название слишком длинное (максимум 40 символов)".into());
        }
        let seed = parse_seed(&self.seed_text, random_seed);
        let mut cfg = WorldConfig::new(name, seed);
        cfg.mode = self.mode;
        cfg.size = self.size;
        cfg.world_type = self.world_type;
        cfg.continents = self.continents;
        cfg.structures = self.structures;
        cfg.difficulty = self.difficulty;
        Ok(cfg)
    }

    /// The seed currently shown in the preview (explicit seed or the preview's random one).
    pub fn effective_seed(&self) -> u64 {
        parse_seed(&self.seed_text, self.preview_seed)
    }
}

pub fn create_world(ctx: &egui::Context, icons: &GuiIcons, st: &mut CreateWorldState, preview: Option<&TextureHandle>, version: &str, random_seed: u64) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
    if top_bar(ctx, icons, "Создать мир", version, true) {
        action = MenuAction::OpenSettings;
    }
    bottom_bar(ctx, |ui| {
        if pill_button(ui, icons, Some(icon::ARROW_LEFT), "Назад", ButtonKind::Secondary, 150.0, true).clicked() {
            action = MenuAction::Back;
        }
        if let Some(e) = &st.error {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
            icons.paint(ui.painter(), r, icon::WARNING, p.warn);
            ui.label(egui::RichText::new(e).color(p.warn));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if pill_button(ui, icons, Some(icon::CUBE), "Создать мир", ButtonKind::Primary, 200.0, true).clicked() {
                match st.to_config(random_seed) {
                    Ok(cfg) => {
                        st.error = None;
                        action = MenuAction::CreateWorld(Box::new(cfg));
                    }
                    Err(e) => st.error = Some(e),
                }
            }
        });
    });
    // Left sidebar: sections stacked vertically (like the world list).
    egui::SidePanel::left("cw-side")
        .resizable(false)
        .exact_width(210.0)
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(2.0_f32, p.outline)).inner_margin(Margin::symmetric(14.0, 18.0)))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            for (i, ic, label) in [(0usize, icon::EDIT, "Основное"), (1, icon::GLOBE, "Планета")] {
                let kind = if st.tab == i { ButtonKind::Primary } else { ButtonKind::Secondary };
                if pill_button(ui, icons, Some(ic), label, kind, ui.available_width(), true).clicked() {
                    st.tab = i;
                }
            }
        });
    // Planet preview right next to the sidebar; sized to fit the screen height.
    let avail_h = ctx.available_rect().height();
    let globe = (avail_h - 120.0).clamp(160.0, 300.0);
    egui::SidePanel::left("cw-preview")
        .resizable(false)
        .exact_width(globe + 28.0)
        .frame(Frame::none().fill(p.bg).stroke(Stroke::new(2.0_f32, p.outline)).inner_margin(Margin::same(14.0)))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("cw-preview-scroll").auto_shrink([false, false]).show(ui, |ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(globe), Sense::hover());
                ui.painter().rect_filled(r, Rounding::ZERO, Color32::from_rgb(6, 9, 18));
                if let Some(tex) = preview {
                    ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                } else {
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, "Генерация…", FontId::proportional(15.0), p.text_muted);
                }
                ui.painter().rect_stroke(r, Rounding::ZERO, Stroke::new(2.0_f32, p.outline));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(format!("Seed: {}", st.effective_seed() as i64)).size(13.0).color(p.text_muted));
                let mins = st.size.circumference() as f32 / 4.3 / 60.0;
                ui.label(egui::RichText::new(format!("Радиус {} блоков · обход ≈ {:.0} мин", st.size.radius(), mins)).size(13.0).color(p.text_muted));
            });
        });
    // Settings of the chosen section on the right.
    egui::CentralPanel::default().frame(body_frame(&p)).show(ctx, |ui| {
        egui::ScrollArea::both().id_salt("cw-form").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            if st.tab == 0 {
                stacked_row(ui, "Название мира", None, |ui| {
                    text_input(ui, icons, &mut st.name, "Новый мир", 300.0, Some(icon::EDIT));
                });
                ui.add_space(10.0);
                stacked_row(ui, "Seed", Some("Пусто — случайный. Можно ввести число или текст"), |ui| {
                    if text_input(ui, icons, &mut st.seed_text, "Случайный", 250.0, Some(icon::GLOBE)).changed() {
                        st.preview_dirty = true;
                    }
                    if square_button(ui, icons, icon::REFRESH, false).clicked() {
                        let t = (ui.ctx().input(|i| i.time) * 1.0e6) as u64;
                        st.seed_text = (wolfcraft_engine::planet::splitmix(random_seed ^ t) % 1_000_000_000).to_string();
                        st.preview_dirty = true;
                    }
                });
                ui.add_space(10.0);
                stacked_row(ui, "Режим", None, |ui| {
                    segmented(ui, "cw-mode", &[(GameMode::Survival, "Выживание"), (GameMode::Creative, "Творческий")], &mut st.mode);
                });
                ui.add_space(10.0);
                stacked_row(ui, "Сложность", None, |ui| {
                    let opts: Vec<(Difficulty, &str)> = Difficulty::ALL.iter().map(|d| (*d, d.label())).collect();
                    segmented(ui, "cw-diff", &opts, &mut st.difficulty);
                });
            } else {
                stacked_row(ui, "Размер планеты", Some(&format!("Окружность ≈ {} блоков", st.size.circumference())), |ui| {
                    let opts: Vec<(PlanetSize, &str)> = PlanetSize::ALL.iter().map(|d| (*d, d.label())).collect();
                    if segmented(ui, "cw-size", &opts, &mut st.size) {
                        st.preview_dirty = true;
                    }
                });
                ui.add_space(10.0);
                stacked_row(ui, "Тип мира", None, |ui| {
                    let opts: Vec<(WorldType, &str)> = WorldType::ALL.iter().map(|d| (*d, d.label())).collect();
                    if segmented(ui, "cw-type", &opts, &mut st.world_type) {
                        st.preview_dirty = true;
                    }
                });
                ui.add_space(10.0);
                let uses = st.world_type == WorldType::Standard;
                stacked_row(ui, "Количество материков", Some(if uses { "Центры материков выбираются по seed" } else { "Не используется для этого типа мира" }), |ui| {
                    ui.add_enabled_ui(uses, |ui| {
                        let opts: Vec<(ContinentCount, &str)> = ContinentCount::ALL.iter().map(|d| (*d, d.label())).collect();
                        if segmented(ui, "cw-cont", &opts, &mut st.continents) {
                            st.preview_dirty = true;
                        }
                    });
                });
                ui.add_space(10.0);
                stacked_row(ui, "Генерация структур", Some("Деревни, руины, храмы, крепости, шахты"), |ui| {
                    toggle_switch(ui, &mut st.structures);
                });
            }
        });
    });
    action
}

// ---------------------------------------------------------------------------------------------
// Settings

#[derive(Default, Debug, PartialEq, Eq)]
pub struct SettingsResult {
    pub changed: bool,
    pub back: bool,
    /// Re-scan the resource pack folders.
    pub refresh_packs: bool,
    /// Download a resource pack (.zip) from this link.
    pub download: Option<String>,
}

/// What the "Текстуры" tab shows.
pub struct PacksView<'a> {
    pub list: &'a [wolfcraft_assets::packs::PackInfo],
    /// Download / scan status line.
    pub status: &'a str,
    /// Folder where the player can drop packs.
    pub folder: &'a str,
}

const FPS_LIMITS: [u32; 7] = [0, 30, 60, 90, 120, 144, 240];

const SETTINGS_TABS: [(u16, &str); 6] = [(icon::HEART, "Профиль"), (icon::SUN, "Графика"), (icon::CUBE, "Текстуры"), (icon::HAMMER, "Рука"), (icon::EDIT, "Интерфейс"), (icon::CROSSHAIR, "Управление")];

/// Resource packs: built-in ones and the player's own, plus import from a link.
fn packs_tab(ui: &mut Ui, icons: &GuiIcons, p: &Palette, s: &mut Settings, packs: &PacksView, res: &mut SettingsResult) {
    for pack in packs.list {
        let sel = s.texture_pack == pack.id;
        let resp = row_card(ui, sel, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                icons.paint(ui.painter(), r, if sel { icon::CHECK } else if pack.path.is_some() { icon::FOLDER } else { icon::CUBE }, if sel { p.accent } else { p.text_muted });
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&pack.name).font(bold_font(15.0)).color(if sel { p.accent } else { p.text }));
                    if !pack.description.is_empty() {
                        ui.label(egui::RichText::new(&pack.description).size(12.5).color(p.text_muted));
                    }
                });
            });
        });
        if resp.clicked() && !sel {
            s.texture_pack = pack.id.clone();
            res.changed = true;
        }
        ui.add_space(6.0);
    }
    ui.add_space(8.0);
    section_label(ui, "Свой пак");
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Пак — папка или .zip с картинками blocks/, items/, entities/, gui/, particles/ (имя_спрайта.png, любой размер) и pack.json {\"name\": \"...\"}. Положите его в папку:").size(12.5).color(p.text_muted));
    ui.label(egui::RichText::new(packs.folder).size(12.5).color(p.text));
    ui.add_space(6.0);
    let url_id = egui::Id::new("pack-url");
    let mut url: String = ui.ctx().data(|d| d.get_temp(url_id)).unwrap_or_default();
    ui.horizontal(|ui| {
        text_input(ui, icons, &mut url, "Ссылка на .zip", 300.0, Some(icon::GLOBE));
        if pill_button(ui, icons, Some(icon::DOWNLOAD), "Загрузить", ButtonKind::Primary, 0.0, !url.trim().is_empty()).clicked() {
            res.download = Some(url.trim().to_string());
        }
        if square_button(ui, icons, icon::REFRESH, false).on_hover_text("Обновить список").clicked() {
            res.refresh_packs = true;
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(url_id, url));
    if !packs.status.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(packs.status).size(13.0).color(p.text_muted));
    }
}

pub fn settings_screen(ctx: &egui::Context, icons: &GuiIcons, s: &mut Settings, version: &str, packs: &PacksView) -> SettingsResult {
    let p = palette(ctx);
    let mut res = SettingsResult::default();
    top_bar(ctx, icons, "Настройки", version, false);
    bottom_bar(ctx, |ui| {
        if pill_button(ui, icons, Some(icon::ARROW_LEFT), "Назад", ButtonKind::Primary, 160.0, true).clicked() {
            res.back = true;
        }
        ui.label(egui::RichText::new("Изменения применяются сразу и сохраняются автоматически").size(13.0).color(p.text_muted));
    });
    // Selected section is kept in egui memory, so the screen needs no extra state.
    let tab_id = egui::Id::new("settings-tab");
    let mut tab: usize = ctx.data(|d| d.get_temp(tab_id)).unwrap_or(0);
    egui::SidePanel::left("settings-side")
        .resizable(false)
        .exact_width(230.0)
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(2.0_f32, p.outline)).inner_margin(Margin::symmetric(14.0, 18.0)))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("settings-side-scroll").auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                for (i, (ic, label)) in SETTINGS_TABS.iter().enumerate() {
                    let kind = if tab == i { ButtonKind::Primary } else { ButtonKind::Secondary };
                    if pill_button(ui, icons, Some(*ic), label, kind, ui.available_width(), true).clicked() {
                        tab = i;
                    }
                }
            });
        });
    ctx.data_mut(|d| d.insert_temp(tab_id, tab));
    egui::CentralPanel::default().frame(body_frame(&p)).show(ctx, |ui| {
        egui::ScrollArea::vertical().id_salt("settings-body").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, SETTINGS_TABS[tab].1);
            ui.add_space(8.0);
            match tab {
                0 => {
                    setting_row(ui, "Никнейм", Some("Показывается в чате"), |ui| {
                        res.changed |= ui.add(egui::TextEdit::singleline(&mut s.nickname).desired_width(200.0).char_limit(16)).changed();
                    });
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Скин").font(FontId::proportional(15.0)).color(p.text));
                    ui.label(egui::RichText::new("Видно в виде от третьего лица").font(FontId::proportional(12.5)).color(p.text_muted));
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        for (i, name) in wolfcraft_engine::renderer::model::SKIN_NAMES.iter().enumerate() {
                            let kind = if s.skin as usize == i { ButtonKind::Primary } else { ButtonKind::Secondary };
                            if pill_button(ui, icons, None, name, kind, 0.0, true).clicked() {
                                s.skin = i as u8;
                                res.changed = true;
                            }
                        }
                    });
                }
                1 => {
                    setting_row(ui, "Дальность прорисовки", Some("В чанках по 16 блоков. Больше — красивее, но тяжелее"), |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.render_distance, 3..=16).suffix(" чанк.")).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Поле зрения", None, |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.fov, 50.0..=110.0).suffix("°").integer()).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Частицы блоков", Some("0 — выключены"), |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.particles, 0.0..=2.0)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Вертикальная синхронизация", Some("Убирает разрывы изображения"), |ui| {
                        res.changed |= toggle_switch(ui, &mut s.vsync).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Ограничение FPS", Some("Работает без VSync"), |ui| {
                        let labels: Vec<String> = FPS_LIMITS.iter().map(|f| if *f == 0 { "Без ограничения".to_string() } else { format!("{f} FPS") }).collect();
                        let mut idx = FPS_LIMITS.iter().position(|f| *f == s.max_fps).unwrap_or(0);
                        if dropdown(ui, icons, "fps-limit", &labels, &mut idx, 200.0) {
                            s.max_fps = FPS_LIMITS[idx];
                            res.changed = true;
                        }
                    });
                    ui.add_space(8.0);
                    if !cfg!(target_os = "android") {
                        setting_row(ui, "Полноэкранный режим", None, |ui| {
                            res.changed |= toggle_switch(ui, &mut s.fullscreen).changed();
                        });
                        ui.add_space(8.0);
                    }
                    setting_row(ui, "Показывать FPS", None, |ui| {
                        res.changed |= toggle_switch(ui, &mut s.show_fps).changed();
                    });
                }
                2 => {
                    packs_tab(ui, icons, &p, s, packs, &mut res);
                }
                3 => {
                    setting_row(ui, "Поле зрения руки", Some("Отдельно от основной камеры"), |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.vm_fov, 40.0..=100.0).suffix("°").integer()).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Масштаб", None, |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.vm_scale, 0.6..=1.6)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Сдвиг по горизонтали", None, |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.vm_x, -0.3..=0.3)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Сдвиг по вертикали", None, |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.vm_y, -0.3..=0.3)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Размах анимации", None, |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.hand_anim, 0.4..=1.0)).changed();
                    });
                }
                4 => {
                    setting_row(ui, "Тема", None, |ui| {
                        res.changed |= segmented(ui, "theme", &[(ThemeMode::Dark, "Тёмная"), (ThemeMode::Light, "Светлая")], &mut s.theme);
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Масштаб интерфейса", None, |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.ui_scale, 0.75..=1.75).fixed_decimals(2)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Громкость", Some("Звуки блоков, шагов и урона"), |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.volume, 0.0..=1.0)).changed();
                    });
                }
                _ => {
                    setting_row(ui, "Чувствительность", Some("Скорость поворота камеры"), |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.mouse_sensitivity, 0.2..=3.0).fixed_decimals(2)).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Инверсия по вертикали", None, |ui| {
                        res.changed |= toggle_switch(ui, &mut s.invert_y).changed();
                    });
                }
            }
        });
    });
    res
}

// ---------------------------------------------------------------------------------------------
// Loading

pub struct LoadingView<'a> {
    pub title: &'a str,
    pub stages: &'a [String],
    /// Index of the stage currently running (== stages.len() when everything is done).
    pub current: usize,
    /// Progress of the whole operation in 0..1.
    pub fraction: f32,
    pub detail: &'a str,
    /// Chunk map around the player, row by row (0 none, 1 generated, 2 ready); empty = no map yet.
    pub grid: &'a [u8],
}

pub fn loading_screen(ctx: &egui::Context, icons: &GuiIcons, v: &LoadingView) {
    let p = palette(ctx);
    egui::CentralPanel::default().frame(Frame::none().fill(p.bg)).show(ctx, |ui| {
        decor(ui, icons, &p);
        let rect = ui.max_rect();
        let col = Rect::from_center_size(rect.center(), Vec2::new(480.0, if v.grid.is_empty() { 520.0 } else { 700.0 }));
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(col), |ui| {
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
                    icons.paint(ui.painter(), r, icon::LOGO, Color32::WHITE);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(v.title).font(bold_font(20.0)).color(p.text));
                        ui.label(egui::RichText::new(v.detail).size(13.0).color(p.text_muted));
                    });
                });
                ui.add_space(14.0);
                for (i, s) in v.stages.iter().enumerate() {
                    let done = i < v.current;
                    let active = i == v.current;
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
                        if done {
                            icons.paint(ui.painter(), r, icon::CHECK, p.success);
                        } else if active {
                            let t = ui.input(|i| i.time) as f32;
                            ui.ctx().request_repaint();
                            let c = r.center();
                            ui.painter().circle_stroke(c, 7.0, Stroke::new(2.0_f32, p.outline));
                            let a = t * 6.0;
                            ui.painter().circle_filled(c + Vec2::new(a.cos(), a.sin()) * 7.0, 2.8, p.accent);
                        } else {
                            ui.painter().circle_filled(r.center(), 3.0, p.outline);
                        }
                        ui.label(egui::RichText::new(s).size(14.5).color(if done { p.text_muted } else if active { p.text } else { p.text_muted.gamma_multiply(0.7) }));
                    });
                }
                ui.add_space(14.0);
                progress_bar(ui, v.fraction, &format!("{:.0}%", v.fraction * 100.0), ui.available_width());
                // Chunk map: squares fill in as the terrain around the player appears.
                let side = (v.grid.len() as f32).sqrt().round() as usize;
                if side > 0 && side * side == v.grid.len() {
                    ui.add_space(14.0);
                    let cell = (150.0 / side as f32).floor().max(4.0);
                    let size = cell * side as f32;
                    ui.vertical_centered(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
                        let painter = ui.painter();
                        painter.rect_filled(r.expand(4.0), Rounding::ZERO, p.bg);
                        painter.rect_stroke(r.expand(4.0), Rounding::ZERO, Stroke::new(2.0_f32, p.outline));
                        for (i, s) in v.grid.iter().enumerate() {
                            let (cx, cy) = ((i % side) as f32, (i / side) as f32);
                            let q = Rect::from_min_size(r.min + Vec2::new(cx * cell, cy * cell), Vec2::splat(cell - 1.0));
                            let color = match s {
                                2 => p.success,
                                1 => p.accent.gamma_multiply(0.55),
                                _ => p.outline.gamma_multiply(0.5),
                            };
                            painter.rect_filled(q, Rounding::ZERO, color);
                        }
                    });
                }
            });
        });
    });
}

// ---------------------------------------------------------------------------------------------
// Pause

/// What the pause card shows about the running game.
pub struct PauseInfo {
    pub world_name: String,
    pub mode: String,
    pub clock: String,
    pub biome: String,
    pub seed: i64,
    pub coords: [f64; 3],
    pub camera: String,
}

/// Centered pause screen shown on Escape: dims the world and presents one custom card.
pub fn pause_menu(ctx: &egui::Context, icons: &GuiIcons, info: &PauseInfo) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
    let screen = ctx.screen_rect();
    // Scrim over the frozen world.
    ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("pause-scrim"))).rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(4, 8, 14, 168));
    let t = ctx.animate_bool_with_time(egui::Id::new("pause-in"), true, ANIM_TIME);
    egui::Area::new(egui::Id::new("pause-card")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::new(0.0, (1.0 - t) * 18.0)).show(ctx, |ui| {
        Frame::none()
            .fill(p.surface)
            .rounding(Rounding::ZERO)
            .stroke(Stroke::new(2.0_f32, p.outline))
            .shadow(egui::Shadow { offset: Vec2::new(6.0, 6.0), blur: 0.0, spread: 0.0, color: Color32::from_black_alpha(150) })
            .inner_margin(Margin::same(22.0))
            .show(ui, |ui| {
                ui.set_width(380.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(52.0), Sense::hover());
                    icons.paint(ui.painter(), r, icon::LOGO, Color32::WHITE);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Пауза").font(bold_font(28.0)).color(p.text));
                        ui.label(egui::RichText::new(&info.world_name).size(14.5).color(p.text_muted));
                    });
                });
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    badge(ui, &info.mode, p.accent);
                    badge(ui, &info.clock, p.secondary);
                    badge(ui, &info.biome, p.text_muted);
                });
                ui.add_space(12.0);
                divider(ui);
                ui.add_space(12.0);
                ui.spacing_mut().item_spacing.y = 9.0;
                ui.vertical_centered(|ui| {
                    if pill_button(ui, icons, Some(icon::PLAY), "Продолжить игру", ButtonKind::Primary, 380.0, true).clicked() {
                        action = MenuAction::Resume;
                    }
                    if pill_button(ui, icons, Some(icon::GEAR), "Настройки", ButtonKind::Secondary, 380.0, true).clicked() {
                        action = MenuAction::OpenSettings;
                    }
                    if pill_button(ui, icons, Some(icon::DOWNLOAD), "Сохранить игру", ButtonKind::Secondary, 380.0, true).clicked() {
                        action = MenuAction::SaveGame;
                    }
                    if pill_button(ui, icons, Some(icon::ARROW_LEFT), "Сохранить и выйти в меню", ButtonKind::Secondary, 380.0, true).clicked() {
                        action = MenuAction::SaveAndQuit;
                    }
                });
            });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && action == MenuAction::None {
        // Handled by the app too; this keeps the card responsive if egui sees the key first.
    }
    action
}

/// Death screen.
pub fn death_screen(ctx: &egui::Context, icons: &GuiIcons) -> MenuAction {
    let mut action = MenuAction::None;
    modal(ctx, "death", "Вы погибли", 340.0, |ui| {
        ui.spacing_mut().item_spacing.y = 10.0;
        ui.vertical_centered(|ui| {
            if pill_button(ui, icons, Some(icon::REFRESH), "Возродиться", ButtonKind::Primary, 300.0, true).clicked() {
                action = MenuAction::Resume;
            }
            if pill_button(ui, icons, Some(icon::FOLDER), "Выйти в меню", ButtonKind::Secondary, 300.0, true).clicked() {
                action = MenuAction::SaveAndQuit;
            }
        });
    });
    action
}

#[allow(dead_code)]
const _ANIM: f32 = ANIM_TIME;

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (egui::Context, GuiIcons) {
        let ctx = egui::Context::default();
        crate::theme_manager::install(&ctx);
        crate::theme_manager::apply(&ctx, ThemeMode::Dark);
        let atlases = wolfcraft_assets::AtlasManager::new(None).unwrap();
        let icons = GuiIcons::new(&ctx, &atlases);
        (ctx, icons)
    }

    fn frame(ctx: &egui::Context, mut f: impl FnMut(&egui::Context)) {
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 760.0)));
        let _ = ctx.run(raw, |c| f(c));
    }

    #[test]
    fn all_screens_render_in_both_themes() {
        let (ctx, icons) = setup();
        let worlds = vec![WorldEntry { cfg: WorldConfig::new("Тест", 5), preview: None }];
        for mode in [ThemeMode::Dark, ThemeMode::Light] {
            crate::theme_manager::apply(&ctx, mode);
            frame(&ctx, |c| {
                main_menu(c, &icons, "0.1.0", "C:\\data", mode == ThemeMode::Dark, None);
            });
            let mut ws = WorldSelectState::default();
            frame(&ctx, |c| {
                world_select(c, &icons, &mut ws, &worlds, "0.1.0");
            });
            let mut cw = CreateWorldState::default();
            frame(&ctx, |c| {
                create_world(c, &icons, &mut cw, None, "0.1.0", 1);
            });
            let mut s = Settings::default();
            frame(&ctx, |c| {
                settings_screen(c, &icons, &mut s, "0.1.0", &PacksView { list: &[], status: "", folder: "" });
            });
            let stages = vec!["A".to_string(), "B".to_string()];
            frame(&ctx, |c| loading_screen(c, &icons, &LoadingView { title: "t", stages: &stages, current: 1, fraction: 0.4, detail: "d", grid: &[] }));
            frame(&ctx, |c| {
                pause_menu(c, &icons, &PauseInfo { world_name: "Мир".into(), mode: "Выживание".into(), clock: "12:00".into(), biome: "Лес".into(), seed: 5, coords: [1.0, 2.0, 3.0], camera: "Вид от первого лица".into() });
            });
        }
    }

    #[test]
    fn create_world_validation_and_config() {
        let mut st = CreateWorldState::default();
        st.name = "  ".into();
        assert!(st.to_config(1).is_err());
        st.name = "x".repeat(41);
        assert!(st.to_config(1).is_err());
        st.name = "Мир".into();
        st.size = PlanetSize::Huge;
        st.world_type = WorldType::Islands;
        st.structures = false;
        st.mode = GameMode::Creative;
        st.seed_text = "12345".into();
        let c = st.to_config(99).unwrap();
        assert_eq!((c.seed, c.size, c.world_type, c.structures, c.mode), (12345, PlanetSize::Huge, WorldType::Islands, false, GameMode::Creative));
        st.seed_text.clear();
        assert_eq!(st.to_config(99).unwrap().seed, 99);
    }

    #[test]
    fn world_list_sorting_and_search() {
        let mk = |name: &str, created: u64, played: u64| {
            let mut c = WorldConfig::new(name, 1);
            c.id = name.to_lowercase();
            c.created = created;
            c.last_played = played;
            WorldEntry { cfg: c, preview: None }
        };
        let w = vec![mk("Бета", 1, 30), mk("Альфа", 3, 10), mk("Гамма", 2, 20)];
        let names = |v: Vec<&WorldEntry>| v.iter().map(|w| w.cfg.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(sorted_filtered(&w, "", 0)), vec!["Бета", "Гамма", "Альфа"]);
        assert_eq!(names(sorted_filtered(&w, "", 1)), vec!["Альфа", "Бета", "Гамма"]);
        assert_eq!(names(sorted_filtered(&w, "", 2)), vec!["Альфа", "Гамма", "Бета"]);
        assert_eq!(names(sorted_filtered(&w, "ГАМ", 0)), vec!["Гамма"]);
    }
}
