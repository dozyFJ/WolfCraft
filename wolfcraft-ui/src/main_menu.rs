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
    /// Start a server on this device and join it: with one of the player's worlds
    /// (private world for friends) or a fresh network world (`None`).
    HostServer(Option<String>),
    CreateWorld(Box<WorldConfig>),
    DeleteWorld(String),
    RenameWorld(String, String),
    ToggleTheme,
    Resume,
    SaveAndQuit,
    SaveGame,
    /// Pick a skin preset (launcher "Скины" tab).
    SetSkin(u8),
    /// Open this world in the studio (editor).
    OpenStudio(String),
    /// Studio: a new empty project (a baseplate world) opened right away.
    NewStudioProject,
    /// World list opened straight on "Игра по сети → Рядом".
    OpenNetwork,
}

pub struct WorldEntry {
    pub cfg: WorldConfig,
    pub preview: Option<TextureHandle>,
}

// ---------------------------------------------------------------------------------------------
// Shared chrome

/// Former top bar: the header is gone, its title and version are shown in the centre of the
/// bottom bar instead. Always returns false (settings open from the main menu).
pub fn top_bar(ctx: &egui::Context, _icons: &GuiIcons, title: &str, version: &str, _show_settings: bool) -> bool {
    let text = if title.is_empty() { format!("WolfCraft · v{version}") } else { format!("{title} · v{version}") };
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("menu-bar-title"), text));
    false
}

fn body_frame(p: &Palette) -> Frame {
    Frame::none().fill(p.bg).inner_margin(Margin::symmetric(24.0, 18.0))
}

fn bottom_bar<R>(ctx: &egui::Context, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = palette(ctx);
    egui::TopBottomPanel::bottom("menu-bottombar")
        .exact_height(66.0)
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(2.0_f32, p.outline)).inner_margin(Margin::symmetric(24.0, 12.0)))
        .show(ctx, |ui| {
            let title: Option<String> = ctx.data_mut(|d| d.remove_temp(egui::Id::new("menu-bar-title")));
            let rect = ui.max_rect();
            let r = ui.horizontal_centered(add).inner;
            if let Some(t) = title {
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, t, FontId::proportional(14.0), p.text_muted);
            }
            r
        })
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

/// Hints shown under the splash loading bar.
const TIPS: [&str; 9] = [
    "Планета — это куб: дойдите до ребра и шагните на соседнюю грань",
    "В творческом режиме дважды нажмите прыжок, чтобы полететь",
    "Чем выше летите, тем быстрее: в космосе скорость растёт",
    "Верстак открывает сетку крафта 3×3",
    "Свои текстуры: Настройки → Текстуры → загрузить пак по ссылке",
    "Пауза и чат — в островке вверху экрана",
    "Корзина в творческом инвентаре удаляет предмет из выбранного слота",
    "Сервер на телефоне: Играть → Игра по сети → Создать сервер",
    "Ночью появляются монстры — постройте укрытие",
];

/// Studio splash: logo, name, a loading bar and rotating tips. Returns true when tapped.
pub fn splash_screen(ctx: &egui::Context, icons: &GuiIcons, progress: f32, version: &str) -> bool {
    let mut tapped = false;
    egui::CentralPanel::default().frame(Frame::none().fill(Color32::from_rgb(10, 12, 18))).show(ctx, |ui| {
        let rect = ui.max_rect();
        let resp = ui.allocate_rect(rect, Sense::click());
        tapped = resp.clicked();
        let painter = ui.painter();
        let t = ui.input(|i| i.time) as f32;
        ui.ctx().request_repaint();
        let fade = (t * 2.0).min(1.0);
        let c = rect.center() - Vec2::new(0.0, 40.0);
        // Logo: the cube, with a soft glow pulsing behind it.
        let glow = 0.5 + 0.5 * (t * 2.2).sin();
        for (i, r) in [120.0, 95.0, 72.0].iter().enumerate() {
            painter.circle_filled(c - Vec2::new(0.0, 56.0), *r, Color32::from_rgba_unmultiplied(38, 198, 218, (6.0 + glow * 6.0 + i as f32 * 4.0) as u8));
        }
        icons.paint(painter, Rect::from_center_size(c - Vec2::new(0.0, 56.0), Vec2::splat(110.0)), icon::LOGO, Color32::WHITE.gamma_multiply(fade));
        painter.text(c + Vec2::new(0.0, 36.0), Align2::CENTER_CENTER, "WOLFCRAFT", bold_font(46.0), Color32::from_rgb(240, 240, 232).gamma_multiply(fade));
        painter.text(c + Vec2::new(0.0, 76.0), Align2::CENTER_CENTER, "S T U D I O S", FontId::proportional(16.0), Color32::from_rgb(38, 198, 218).gamma_multiply(fade));
        // Loading bar.
        let w = (rect.width() * 0.45).clamp(240.0, 520.0);
        let bar = Rect::from_center_size(Pos2::new(rect.center().x, c.y + 130.0), Vec2::new(w, 14.0));
        painter.rect_filled(bar, Rounding::ZERO, Color32::from_rgb(28, 32, 42));
        painter.rect_stroke(bar.expand(3.0), Rounding::ZERO, Stroke::new(2.0_f32, Color32::from_rgb(70, 78, 96)));
        let fill = Rect::from_min_size(bar.min, Vec2::new(bar.width() * progress.clamp(0.0, 1.0), bar.height()));
        painter.rect_filled(fill, Rounding::ZERO, Color32::from_rgb(38, 198, 218));
        // Shine running along the filled part.
        let sx = fill.left() + (t * 180.0) % fill.width().max(1.0);
        painter.rect_filled(Rect::from_min_max(Pos2::new(sx, bar.top()), Pos2::new((sx + 24.0).min(fill.right()), bar.bottom())), Rounding::ZERO, Color32::from_white_alpha(50));
        // Tip under the bar, changing every few seconds.
        let tip = TIPS[((t / 3.5) as usize) % TIPS.len()];
        painter.text(Pos2::new(rect.center().x, bar.bottom() + 26.0), Align2::CENTER_TOP, format!("Совет: {tip}"), FontId::proportional(14.0), Color32::from_gray(170));
        painter.text(Pos2::new(rect.right() - 12.0, rect.bottom() - 10.0), Align2::RIGHT_BOTTOM, format!("v{version}"), FontId::proportional(12.0), Color32::from_gray(90));
    });
    tapped
}

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
        // Static stars: no twinkling, nothing in the menu moves (no shimmer on phones).
        let tw = 0.55 + 0.45 * ((h % 7) as f32 / 6.0);
        let r = if h % 11 == 0 { 1.6 } else { 1.0 };
        painter.circle_filled(Pos2::new(rect.left() + x * rect.width(), rect.top() + y * rect.height()), r, Color32::from_white_alpha((150.0 * tw) as u8));
    }
    if let Some(tex) = planet {
        // Slow drift up and down so the screen feels alive.
        let d = rect.width().max(rect.height() * 1.6);
        // Low on the screen: only the top of the planet shows, under the buttons.
        let c = Pos2::new(rect.center().x.round(), (rect.bottom() + d * 0.30).round());
        painter.image(tex.id(), Rect::from_center_size(c, Vec2::splat(d)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }
    // Soft shade so the menu stays readable, darker from the top down to the middle where the
    // logo and title sit.
    painter.rect_filled(rect, Rounding::ZERO, p.bg.gamma_multiply(if p.dark { 0.25 } else { 0.15 }));
    let mut m = egui::Mesh::default();
    let dark = Color32::from_black_alpha(200);
    let clear = Color32::from_black_alpha(0);
    let mid = rect.top() + rect.height() * 0.62;
    m.colored_vertex(rect.left_top(), dark);
    m.colored_vertex(rect.right_top(), dark);
    m.colored_vertex(Pos2::new(rect.right(), mid), clear);
    m.colored_vertex(Pos2::new(rect.left(), mid), clear);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(m));
    let _ = t;
}

/// Home screen: the mobile launcher (toolbar, pages, bottom navigation).
pub fn main_menu(ctx: &egui::Context, icons: &GuiIcons, version: &str, data_dir: &str, dark: bool, planet: Option<&TextureHandle>, skin: u8, worlds: &[WorldEntry]) -> MenuAction {
    let _ = data_dir;
    crate::launcher::show(ctx, icons, version, dark, planet, skin, worlds)
}

// ---------------------------------------------------------------------------------------------
// World selection

#[derive(Default)]
pub struct WorldSelectState {
    /// 0 = single player (saved worlds), 1 = play over the network.
    pub mode: usize,
    /// Address typed for a network game.
    pub address: String,
    /// Right sidebar in "Игра по сети": 0 nearby (found on the local network), 1 host a
    /// server, 2 by code or address, 3 saved servers.
    pub net_tab: usize,
    /// Servers heard on the local network (Wi-Fi, hotspot): (name, address, players, max).
    pub nearby: Vec<(String, String, usize, usize)>,
    /// Saved servers (name, address); `servers_changed` tells the app to save them.
    pub servers: Vec<(String, String)>,
    pub servers_changed: bool,
    pub new_server_name: String,
    pub new_server_addr: String,
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

/// "Игра по сети": content of the tab chosen in the right sidebar.
fn online_tab(ui: &mut egui::Ui, icons: &GuiIcons, p: &crate::theme_manager::Palette, st: &mut WorldSelectState, worlds: &[WorldEntry]) -> Option<MenuAction> {
    let mut act = None;
    if let Some(e) = &st.net_error {
        ui.label(egui::RichText::new(e).size(14.0).color(p.danger));
        ui.add_space(8.0);
    }
    match st.net_tab {
        0 => {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Рядом").font(bold_font(18.0)).color(p.text));
                // Little radar: the list refreshes by itself.
                let t = ui.ctx().input(|i| i.time) as f32;
                let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                ui.painter().circle_stroke(r.center(), 4.0 + (t * 1.5).fract() * 7.0, Stroke::new(1.5_f32, p.accent.gamma_multiply(1.0 - (t * 1.5).fract())));
                ui.painter().circle_filled(r.center(), 3.0, p.accent);
                ui.label(egui::RichText::new("ищем игры в вашей сети Wi-Fi и точке доступа").size(13.0).color(p.text_muted));
            });
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
            ui.add_space(6.0);
            egui::ScrollArea::vertical().id_salt("nearby").max_height(ui.available_height() - 10.0).show(ui, |ui| {
                for (name, addr, players, max) in &st.nearby {
                    row_card(ui, false, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                            icons.paint(ui.painter(), r, icon::GLOBE, p.success);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(name).font(bold_font(16.0)).color(p.text));
                                ui.label(egui::RichText::new(format!("игроков {players}/{max} · {addr}")).size(12.5).color(p.text_muted));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, 0.0, players < max).clicked() {
                                    st.net_error = None;
                                    act = Some(MenuAction::JoinServer(addr.clone()));
                                }
                            });
                        });
                    });
                    ui.add_space(6.0);
                }
                if st.nearby.is_empty() {
                    ui.label(egui::RichText::new("Пока никого. Сервер появится здесь сам, как только друг его откроет.").size(13.5).color(p.text_muted));
                }
                ui.add_space(10.0);
                for (title, text) in [
                    ("Одна сеть Wi-Fi", "Один игрок открывает мир во вкладке «Создать сервер» — у остальных он сразу появляется в этом списке."),
                    ("Только мобильный интернет", "Операторы не пропускают входящие подключения, поэтому напрямую через мобильную сеть сыграть нельзя. Включите точку доступа на телефоне с сервером и подключите к ней друзей: сервер появится здесь, а интернет на игру не тратится."),
                    ("Код приглашения", "Сервер показывает в чате код вида ABCD-EFGH. Его можно ввести во вкладке «По коду»."),
                ] {
                    row_card(ui, false, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new(title).font(bold_font(15.0)).color(p.text));
                        ui.label(egui::RichText::new(text).size(13.0).color(p.text_muted));
                    });
                    ui.add_space(6.0);
                }
            });
        }
        3 => {
            ui.label(egui::RichText::new("Мои серверы").font(bold_font(18.0)).color(p.text));
            ui.add_space(6.0);
            let mut remove = None;
            egui::ScrollArea::vertical().id_salt("servers").max_height(ui.available_height() - 70.0).show(ui, |ui| {
                if st.servers.is_empty() {
                    ui.label(egui::RichText::new("Список пуст. Добавьте сервер ниже: имя и адрес (например 192.168.0.10:25570).").size(13.5).color(p.text_muted));
                }
                for (i, (name, addr)) in st.servers.iter().enumerate() {
                    row_card(ui, false, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                            icons.paint(ui.painter(), r, icon::GLOBE, p.accent);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(name).font(bold_font(16.0)).color(p.text));
                                ui.label(egui::RichText::new(addr).size(12.5).color(p.text_muted));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if icon_button(ui, icons, icon::TRASH, 34.0, ButtonKind::Danger).clicked() {
                                    remove = Some(i);
                                }
                                if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, 0.0, true).clicked() {
                                    act = Some(MenuAction::JoinServer(addr.clone()));
                                }
                            });
                        });
                    });
                    ui.add_space(6.0);
                }
            });
            if let Some(i) = remove {
                st.servers.remove(i);
                st.servers_changed = true;
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                text_input(ui, icons, &mut st.new_server_name, "Имя", 150.0, Some(icon::EDIT));
                text_input(ui, icons, &mut st.new_server_addr, "Адрес:порт", 220.0, Some(icon::GLOBE));
                let ok = !st.new_server_addr.trim().is_empty();
                if pill_button(ui, icons, Some(icon::PLUS), "Добавить", ButtonKind::Secondary, 0.0, ok).clicked() {
                    let name = if st.new_server_name.trim().is_empty() { st.new_server_addr.trim().to_string() } else { st.new_server_name.trim().to_string() };
                    st.servers.push((name, st.new_server_addr.trim().to_string()));
                    st.new_server_name.clear();
                    st.new_server_addr.clear();
                    st.servers_changed = true;
                }
            });
        }
        1 => {
            ui.label(egui::RichText::new("Создать сервер").font(bold_font(18.0)).color(p.text));
            ui.label(egui::RichText::new("Выберите мир — телефон станет сервером. Друзья в той же сети Wi-Fi или подключённые к вашей точке доступа увидят его во вкладке «Рядом»; код приглашения появится в чате.").size(13.0).color(p.text_muted));
            ui.add_space(8.0);
            egui::ScrollArea::vertical().id_salt("private-worlds").max_height(ui.available_height() - 60.0).show(ui, |ui| {
                for w in worlds {
                    row_card(ui, false, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
                            ui.painter().rect_filled(r, Rounding::ZERO, Color32::from_rgb(8, 12, 22));
                            if let Some(tex) = &w.preview {
                                ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                            }
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(&w.cfg.name).font(bold_font(16.0)).color(p.text));
                                ui.label(egui::RichText::new(format!("{} · {}", if w.cfg.mode == GameMode::Creative { "Творческий" } else { "Выживание" }, w.cfg.size.label())).size(12.5).color(p.text_muted));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if pill_button(ui, icons, Some(icon::GLOBE), "Открыть для друзей", ButtonKind::Primary, 0.0, true).clicked() {
                                    st.net_error = None;
                                    act = Some(MenuAction::HostServer(Some(w.cfg.id.clone())));
                                }
                            });
                        });
                    });
                    ui.add_space(6.0);
                }
                if worlds.is_empty() {
                    ui.label(egui::RichText::new("У вас пока нет миров.").color(p.text_muted));
                }
            });
            ui.add_space(6.0);
            if pill_button(ui, icons, Some(icon::PLUS), "Новый сетевой мир", ButtonKind::Secondary, 0.0, true).clicked() {
                st.net_error = None;
                act = Some(MenuAction::HostServer(None));
            }
        }
        _ => {
            ui.label(egui::RichText::new("По коду или адресу").font(bold_font(18.0)).color(p.text));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                text_input(ui, icons, &mut st.address, "Код ABCD-EFGH или адрес 192.168.0.10", 360.0, Some(icon::GLOBE));
                ui.add_space(8.0);
                let ready = !st.address.trim().is_empty();
                if pill_button(ui, icons, Some(icon::PLAY), "Подключиться", ButtonKind::Primary, 0.0, ready).clicked() {
                    st.net_error = None;
                    act = Some(MenuAction::JoinServer(st.address.trim().to_string()));
                }
            });
            ui.add_space(14.0);
            row_card(ui, false, |ui| {
                ui.label(egui::RichText::new("Как играть вместе").font(bold_font(16.0)).color(p.text));
                ui.label(egui::RichText::new("Один игрок открывает мир («Создать сервер») — в чате появятся код приглашения и адрес. Остальные вводят код здесь или просто выбирают сервер во вкладке «Рядом». Вы окажетесь в одном мире: видите друг друга, общие постройки и чат.").size(13.5).color(p.text_muted));
            });
        }
    }
    act
}

pub fn sorted_filtered<'a>(worlds: &'a [WorldEntry], search: &str, sort: usize) -> Vec<&'a WorldEntry> {
    let q = search.trim().to_lowercase();
    // Studio projects live in the Studio, not among the game worlds.
    let mut v: Vec<&WorldEntry> = worlds.iter().filter(|w| !w.cfg.studio_project && (q.is_empty() || w.cfg.name.to_lowercase().contains(&q))).collect();
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
/// World list tools (settings, sort, search) next to "Назад" in the bottom bar.
fn world_select_tools(ui: &mut Ui, icons: &GuiIcons, st: &mut WorldSelectState) -> (bool, Option<Rect>) {
    let mut settings = false;
    let mut sort_anchor = None;
    ui.spacing_mut().item_spacing.x = 8.0;
    if square_button(ui, icons, icon::GEAR, false).clicked() {
        settings = true;
    }
    if st.mode == 0 {
        if square_button(ui, icons, if st.search_open { icon::CLOSE } else { icon::SEARCH }, st.search_open).clicked() {
            st.search_open = !st.search_open;
            st.search_focus = st.search_open;
            if !st.search_open {
                st.search.clear();
            }
        }
        let sort = square_button(ui, icons, icon::SORT, st.sort_open);
        if sort.clicked() {
            st.sort_open = !st.sort_open;
        }
        sort_anchor = Some(sort.rect);
        if st.search_open {
            let w = (ui.available_width() - 360.0).clamp(120.0, 300.0);
            let r = text_input(ui, icons, &mut st.search, "Поиск миров…", w, Some(icon::SEARCH));
            if st.search_focus {
                st.search_focus = false;
                r.request_focus();
            }
        }
    }
    (settings, sort_anchor)
}

fn world_sort_menu(ctx: &egui::Context, icons: &GuiIcons, st: &mut WorldSelectState, sort_anchor: Option<Rect>) {
    let p = palette(ctx);
    // Sort menu opens above its button.
    if st.mode == 0 && st.sort_open {
        if let Some(a) = sort_anchor {
            let area = egui::Area::new(egui::Id::new("world-sort-menu"))
                .order(egui::Order::Foreground)
                .pivot(Align2::LEFT_BOTTOM)
                .fixed_pos(Pos2::new(a.left(), a.top() - 8.0))
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
}

pub fn world_select(ctx: &egui::Context, icons: &GuiIcons, st: &mut WorldSelectState, worlds: &[WorldEntry], version: &str) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
    let _ = version;
    let mut sort_anchor = None;
    bottom_bar(ctx, |ui| {
        if pill_button(ui, icons, Some(icon::ARROW_LEFT), "Назад", ButtonKind::Secondary, 140.0, true).clicked() {
            action = MenuAction::Back;
        }
        let (settings, anchor) = world_select_tools(ui, icons, st);
        sort_anchor = anchor;
        if settings {
            action = MenuAction::OpenSettings;
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
    world_sort_menu(ctx, icons, st, sort_anchor);
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
    if st.mode == 1 {
        // Right sidebar: network sections, mirroring the left one.
        egui::SidePanel::right("net-side")
            .resizable(false)
            .exact_width(220.0)
            .frame(Frame::none().fill(p.surface).stroke(Stroke::new(2.0_f32, p.outline)).inner_margin(Margin::symmetric(14.0, 18.0)))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                for (i, ic, label) in [(0usize, icon::SEARCH, "Рядом"), (1, icon::STAR, "Создать сервер"), (2, icon::EDIT, "По коду"), (3, icon::GLOBE, "Мои серверы")] {
                    let kind = if st.net_tab == i { ButtonKind::Primary } else { ButtonKind::Secondary };
                    if pill_button(ui, icons, Some(ic), label, kind, ui.available_width(), true).clicked() {
                        st.net_tab = i;
                    }
                }
            });
    }
    egui::CentralPanel::default().frame(body_frame(&p)).show(ctx, |ui| {
        if st.mode == 1 {
            if let Some(a) = online_tab(ui, icons, &p, st, worlds) {
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
        // Grid of world cards: big planet on top, name and tags, play / rename / delete.
        let avail = ui.available_width();
        let card_w = 200.0f32;
        let gap = 12.0;
        let cols = ((avail + gap) / (card_w + gap)).floor().max(1.0) as usize;
        let card_w = (avail - gap * (cols as f32 - 1.0)) / cols as f32 - 1.0;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for row in list.chunks(cols) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for w in row {
                        let selected = st.selected.as_deref() == Some(w.cfg.id.as_str());
                        ui.allocate_ui(Vec2::new(card_w, 10.0), |ui| {
                            ui.set_width(card_w);
                            let resp = row_card(ui, selected, |ui| {
                                ui.set_width(card_w - 26.0);
                                ui.vertical_centered(|ui| {
                                    let sz = (card_w - 60.0).min(120.0);
                                    let (r, _) = ui.allocate_exact_size(Vec2::splat(sz), Sense::hover());
                                    if let Some(tex) = &w.preview {
                                        ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                                    } else {
                                        ui.painter().rect_filled(r, Rounding::ZERO, Color32::from_rgb(8, 12, 22));
                                    }
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new(&w.cfg.name).font(bold_font(17.0)).color(p.text));
                                    ui.label(egui::RichText::new(format!("{} · {}", if w.cfg.mode == GameMode::Creative { "Творческий" } else { "Выживание" }, w.cfg.size.label())).size(12.5).color(if w.cfg.mode == GameMode::Creative { p.secondary } else { p.accent }));
                                    ui.label(egui::RichText::new(format_timestamp(w.cfg.last_played)).size(11.5).color(p.text_muted));
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        let bw = card_w - 26.0 - 2.0 * 34.0 - 2.0 * ui.spacing().item_spacing.x;
                                        if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, bw, true).clicked() {
                                            action = MenuAction::PlayWorld(w.cfg.id.clone());
                                        }
                                        if icon_button(ui, icons, icon::EDIT, 34.0, ButtonKind::Ghost).clicked() {
                                            st.rename = Some((w.cfg.id.clone(), w.cfg.name.clone()));
                                        }
                                        if icon_button(ui, icons, icon::TRASH, 34.0, ButtonKind::Danger).clicked() {
                                            st.confirm_delete = Some(w.cfg.id.clone());
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
                        });
                    }
                });
                ui.add_space(gap);
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
    pub shape: PlanetShape,
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
            shape: PlanetShape::Cube,
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
        cfg.shape = self.shape;
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
    // Planet preview on the right side; sized to fit the screen height.
    let avail_h = ctx.available_rect().height();
    let globe = (avail_h - 120.0).clamp(160.0, 300.0);
    egui::SidePanel::right("cw-preview")
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
                stacked_row(ui, "Форма планеты", Some("Куб — плоские грани со скруглёнными рёбрами"), |ui| {
                    let opts: Vec<(PlanetShape, &str)> = PlanetShape::ALL.iter().map(|d| (*d, d.label())).collect();
                    if segmented(ui, "cw-shape", &opts, &mut st.shape) {
                        st.preview_dirty = true;
                    }
                });
                ui.add_space(10.0);
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
    // Shaders: a preset, then every effect on its own (changing one switches to "Свои").
    setting_row(ui, "Шейдеры", Some("Лёгкие: рельеф, ветер, вода. Ультра: ещё глубина швов (параллакс). Мульт: мягкий яркий свет и сочные цвета, как в топовых симуляторах"), |ui| {
        let mut preset = s.shaders;
        if segmented(ui, "shaders", &[(0u8, "Выкл"), (1, "Лёгкие"), (2, "Ультра"), (4, "Мульт"), (3, "Свои")], &mut preset) {
            if preset != 3 {
                s.apply_shader_preset(preset);
            } else {
                s.shaders = 3;
            }
            res.changed = true;
        }
    });
    ui.add_space(8.0);
    let before = (s.relief_mode, s.relief_strength, s.fx_wind, s.fx_water, s.fx_grading, s.fx_toon);
    setting_row(ui, "Рельеф текстур", Some("Объём: свет и тень на каждом камне. Глубина: швы и щели уходят внутрь блока. В HD-паках подробнее"), |ui| {
        res.changed |= segmented(ui, "relief", &[(0u8, "Нет"), (1, "Объём"), (2, "Глубина")], &mut s.relief_mode);
    });
    if s.relief_mode > 0 {
        ui.add_space(6.0);
        setting_row(ui, "Сила рельефа", None, |ui| {
            res.changed |= ui.add(egui::Slider::new(&mut s.relief_strength, 0.3..=2.0).fixed_decimals(1)).changed();
        });
    }
    ui.add_space(6.0);
    setting_row(ui, "Ветер", Some("Качаются листва и трава"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fx_wind).changed();
    });
    ui.add_space(6.0);
    setting_row(ui, "Вода", Some("Волны, блики солнца и отражение неба"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fx_water).changed();
    });
    ui.add_space(6.0);
    setting_row(ui, "Цветокоррекция", Some("Контраст и сочные цвета"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fx_grading).changed();
    });
    ui.add_space(6.0);
    setting_row(ui, "Мультяшный свет", Some("Мягкие светлые тени, яркие краски, контур-подсветка мобов"), |ui| {
        res.changed |= toggle_switch(ui, &mut s.fx_toon).changed();
    });
    if (s.relief_mode, s.relief_strength, s.fx_wind, s.fx_water, s.fx_grading, s.fx_toon) != before {
        s.shaders = 3;
    }
    ui.add_space(10.0);
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
            // The cartoon pack looks its best with the cartoon light.
            if pack.id == wolfcraft_assets::packs::TOON_PACK && s.shaders != 4 {
                s.apply_shader_preset(4);
            }
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
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new("Сохраняется автоматически").size(13.0).color(p.text_muted));
        });
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
                    setting_row(ui, "Разрешение", Some("Чем меньше, тем быстрее. 75% на телефоне почти не отличить, а FPS выше"), |ui| {
                        let mut pct = (s.render_scale * 100.0).round() as u8;
                        if segmented(ui, "render-scale", &[(50u8, "50%"), (67, "67%"), (75, "75%"), (100, "100%")], &mut pct) {
                            s.render_scale = pct as f32 / 100.0;
                            res.changed = true;
                        }
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
                    setting_row(ui, "Частота экрана", Some("Плавнее на экранах 90/120 Гц, но тратит больше батареи"), |ui| {
                        res.changed |= segmented(ui, "refresh", &[(60u32, "60 Гц"), (90, "90 Гц"), (120, "120 Гц")], &mut s.refresh_rate);
                    });
                    ui.add_space(8.0);
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
                    setting_row(ui, "Выбор касанием", Some("Ломать и ставить там, где палец, без прицела"), |ui| {
                        res.changed |= toggle_switch(ui, &mut s.tap_aim).changed();
                    });
                    ui.add_space(8.0);
                    setting_row(ui, "Плавность камеры", Some("0 — поворот точно за пальцем, 1 — мягко"), |ui| {
                        res.changed |= ui.add(egui::Slider::new(&mut s.camera_smooth, 0.0..=1.0).fixed_decimals(2)).changed();
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

/// World loading, styled after the classic "loading terrain" screen: a dark tiled earth
/// backdrop, the percentage on top, a big chunk map in the middle and the current step below.
pub fn loading_screen(ctx: &egui::Context, icons: &GuiIcons, v: &LoadingView) {
    let p = palette(ctx);
    egui::CentralPanel::default().frame(Frame::none().fill(Color32::from_rgb(14, 12, 10))).show(ctx, |ui| {
        let rect = ui.max_rect();
        let painter = ui.painter().clone();
        // Backdrop: dirt blocks, darkened.
        let tile = 64.0;
        let (nx, ny) = ((rect.width() / tile).ceil() as i32, (rect.height() / tile).ceil() as i32);
        for y in 0..ny {
            for x in 0..nx {
                let r = Rect::from_min_size(rect.min + Vec2::new(x as f32 * tile, y as f32 * tile), Vec2::splat(tile));
                icons.paint_block_tile(&painter, r, wolfcraft_assets::tiles::DIRT);
            }
        }
        painter.rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(200));
        ui.ctx().request_repaint();

        // A square vessel that fills with liquid: the level is the progress, the colour
        // follows the current stage, the surface waves and bubbles rise.
        let size = (rect.height() * 0.5).clamp(140.0, 300.0);
        let r = Rect::from_center_size(Pos2::new(rect.center().x, rect.center().y + 6.0), Vec2::splat(size));
        let t = ui.input(|i| i.time) as f32;
        let pct = format!("{:.0}%", v.fraction * 100.0);
        painter.text(Pos2::new(r.center().x + 2.0, r.top() - 20.0 + 2.0), Align2::CENTER_BOTTOM, &pct, bold_font(30.0), Color32::from_black_alpha(180));
        painter.text(Pos2::new(r.center().x, r.top() - 20.0), Align2::CENTER_BOTTOM, &pct, bold_font(30.0), Color32::from_rgb(236, 232, 210));
        painter.rect_filled(r.expand(6.0).translate(Vec2::splat(6.0)), Rounding::ZERO, Color32::from_black_alpha(160));
        painter.rect_filled(r.expand(6.0), Rounding::ZERO, Color32::from_rgb(20, 22, 28));
        let inner = r;
        painter.rect_filled(inner, Rounding::ZERO, Color32::from_rgb(12, 14, 19));
        // Stage colours: seed, continents, climate, spawn, structures, chunks.
        const COLS: [[u8; 3]; 7] = [[90, 110, 220], [40, 140, 230], [38, 198, 218], [60, 200, 140], [150, 210, 70], [240, 190, 60], [92, 200, 110]];
        let stage_f = if v.stages.is_empty() { 0.0 } else { v.current as f32 + 0.5 };
        let i0 = (stage_f.floor() as usize).min(COLS.len() - 1);
        let i1 = (i0 + 1).min(COLS.len() - 1);
        let k = (stage_f - stage_f.floor()).clamp(0.0, 1.0) * 0.6;
        let mixc = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k) as u8;
        let col = Color32::from_rgb(mixc(COLS[i0][0], COLS[i1][0]), mixc(COLS[i0][1], COLS[i1][1]), mixc(COLS[i0][2], COLS[i1][2]));
        let level = inner.bottom() - inner.height() * v.fraction.clamp(0.0, 1.0);
        if v.fraction > 0.001 {
            let painter_c = painter.with_clip_rect(inner);
            let wave = |x: f32, ph: f32, amp: f32| level + (x * 0.045 + t * 2.4 + ph).sin() * amp + (x * 0.11 - t * 1.7 + ph).sin() * amp * 0.4;
            let n = 48;
            // Back wave (darker), then the front liquid body.
            for (ph, amp, shade) in [(1.7f32, 5.0f32, 0.65f32), (0.0, 4.0, 1.0)] {
                let mut pts = Vec::with_capacity(n + 3);
                for i in 0..=n {
                    let x = inner.left() + inner.width() * i as f32 / n as f32;
                    pts.push(Pos2::new(x, wave(x - inner.left(), ph, amp)));
                }
                let c = Color32::from_rgb((col.r() as f32 * shade) as u8, (col.g() as f32 * shade) as u8, (col.b() as f32 * shade) as u8);
                // One triangle mesh under the wave (no anti-aliased seams between strips).
                let mut mesh = egui::Mesh::default();
                for (i, pt) in pts.iter().enumerate() {
                    mesh.colored_vertex(*pt, c);
                    mesh.colored_vertex(Pos2::new(pt.x, inner.bottom()), c);
                    if i > 0 {
                        let b = (i as u32 - 1) * 2;
                        mesh.add_triangle(b, b + 1, b + 2);
                        mesh.add_triangle(b + 1, b + 3, b + 2);
                    }
                }
                painter_c.add(egui::Shape::mesh(mesh));
                if shade == 1.0 {
                    painter_c.add(egui::Shape::line(pts, Stroke::new(2.0_f32, Color32::from_white_alpha(110))));
                }
            }
            // Bubbles rising through the liquid.
            for b in 0..10u32 {
                let speed = 18.0 + (b * 7 % 11) as f32 * 3.0;
                let bx = inner.left() + inner.width() * (((b * 37) % 97) as f32 / 97.0);
                let y = inner.bottom() - ((t * speed + b as f32 * 31.0) % (inner.bottom() - level + 10.0).max(1.0));
                if y > level + 4.0 {
                    painter_c.circle_stroke(Pos2::new(bx + (t * 3.0 + b as f32).sin() * 3.0, y), 2.0 + (b % 3) as f32, Stroke::new(1.5_f32, Color32::from_white_alpha(90)));
                }
            }
        }
        // Frame of the vessel.
        painter.rect_stroke(r.expand(6.0), Rounding::ZERO, Stroke::new(2.0_f32, p.outline));
        painter.rect_stroke(inner, Rounding::ZERO, Stroke::new(1.0_f32, Color32::from_white_alpha(40)));
        let _ = v.grid;
        // Below: world name and the current step with a spinner.
        let y = r.bottom() + 22.0;
        let step = v.stages.get(v.current).map(|s| s.as_str()).unwrap_or("Готово");
        painter.text(Pos2::new(r.center().x, y), Align2::CENTER_TOP, v.title, bold_font(16.0), Color32::from_rgb(236, 232, 210));
        let line = if v.detail.is_empty() { step.to_string() } else { format!("{step} · {}", v.detail) };
        let g = painter.layout_no_wrap(line, FontId::proportional(13.5), Color32::from_gray(170));
        let w = g.size().x + 22.0;
        let x0 = r.center().x - w / 2.0;
        let c = Pos2::new(x0 + 7.0, y + 30.0);
        painter.circle_stroke(c, 6.0, Stroke::new(2.0_f32, Color32::from_gray(80)));
        painter.circle_filled(c + Vec2::new((t * 6.0).cos(), (t * 6.0).sin()) * 6.0, 2.5, p.accent);
        painter.galley(Pos2::new(x0 + 22.0, y + 30.0 - g.size().y / 2.0), g, Color32::from_gray(170));
        let _ = icon::LOGO;
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
/// Death screen in a cinematic style: the world fades to dark, a big title slides in,
/// then respawn / menu buttons; respawns by itself after a few seconds.
pub fn death_screen(ctx: &egui::Context, icons: &GuiIcons) -> MenuAction {
    let mut action = MenuAction::None;
    let now = ctx.input(|i| i.time);
    // Start time: reset when the screen was not shown for a moment (a new death).
    let (t0_id, last_id) = (egui::Id::new("death-t0"), egui::Id::new("death-last"));
    let last: f64 = ctx.data(|d| d.get_temp(last_id)).unwrap_or(-10.0);
    let mut t0: f64 = ctx.data(|d| d.get_temp(t0_id)).unwrap_or(now);
    if now - last > 0.5 {
        t0 = now;
    }
    ctx.data_mut(|d| {
        d.insert_temp(t0_id, t0);
        d.insert_temp(last_id, now);
    });
    let t = (now - t0) as f32;
    ctx.request_repaint();
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("death-fx")));
    let fade = (t / 1.2).clamp(0.0, 1.0);
    painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(20, 8, 8, (170.0 * fade) as u8));
    // Letterbox bars slide in.
    let bar = screen.height() * 0.12 * fade;
    painter.rect_filled(Rect::from_min_size(screen.min, Vec2::new(screen.width(), bar)), 0.0, Color32::BLACK);
    painter.rect_filled(Rect::from_min_max(Pos2::new(screen.left(), screen.bottom() - bar), screen.max), 0.0, Color32::BLACK);
    // Title: grows from large to normal and fades in.
    let ta = ((t - 0.4) / 0.8).clamp(0.0, 1.0);
    let size = 64.0 + (1.0 - ta) * 30.0;
    let c = screen.center() - Vec2::new(0.0, 40.0);
    painter.text(c + Vec2::splat(4.0), Align2::CENTER_CENTER, "ВЫ ПОГИБЛИ", bold_font(size), Color32::from_black_alpha((220.0 * ta) as u8));
    painter.text(c, Align2::CENTER_CENTER, "ВЫ ПОГИБЛИ", bold_font(size), Color32::from_rgba_unmultiplied(200, 30, 35, (255.0 * ta) as u8));
    const AUTO: f32 = 6.0;
    let left = (AUTO - t).ceil().max(0.0);
    if t > 1.4 {
        painter.text(c + Vec2::new(0.0, 52.0), Align2::CENTER_CENTER, format!("Возрождение через {left:.0}…"), FontId::proportional(16.0), Color32::from_gray(210));
        egui::Area::new(egui::Id::new("death-buttons")).order(egui::Order::Foreground).anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 110.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if pill_button(ui, icons, Some(icon::REFRESH), "Возродиться", ButtonKind::Primary, 200.0, true).clicked() {
                    action = MenuAction::Resume;
                }
                ui.add_space(12.0);
                if pill_button(ui, icons, Some(icon::FOLDER), "В меню", ButtonKind::Secondary, 160.0, true).clicked() {
                    action = MenuAction::SaveAndQuit;
                }
            });
        });
    }
    if t >= AUTO && action == MenuAction::None {
        action = MenuAction::Resume;
    }
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
                main_menu(c, &icons, "0.1.0", "C:\\data", mode == ThemeMode::Dark, None, 0, &[]);
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
