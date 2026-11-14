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
    /// Enter the main menu of the installed game version.
    LaunchVersion(String),
    OpenCreate,
    OpenSettings,
    CheckUpdates,
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
    /// New nickname from the launcher profile.
    SetNickname(String),
    /// Studio: create a server (place) in the menu without opening the editor.
    /// `template`: 0 baseplate, 1 flat, 2 planet.
    CreateStudioProject { name: String, template: u8 },
    /// Open this world in the studio (editor).
    OpenStudio(String),
    /// Join a server's world (address) with the studio tools: edits go straight to the server.
    EditServerInStudio(String),
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
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::symmetric(24.0, 12.0)))
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
        ui.painter().circle_stroke(c, *r, Stroke::new(1.0_f32, p.outline.gamma_multiply(0.55 - i as f32 * 0.08)));
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
/// Loading and splash: stars over a dark sky, the name large, one short line of what is
/// happening, a slim bar that glides to its value, and a tip. No counters or technical text.
fn loading_panel(ctx: &egui::Context, icons: &GuiIcons, title: &str, subtitle: &str, fraction: f32, tip: &str, version: &str, interactive: bool) -> bool {
    let _ = icons;
    let p = palette(ctx);
    let mut clicked = false;
    // The bar moves smoothly instead of jumping between chunk batches.
    let shown = ctx.animate_value_with_time(egui::Id::new("loading-bar"), fraction.clamp(0.0, 1.0), 0.35);
    egui::CentralPanel::default().frame(Frame::none()).show(ctx, |ui| {
        planet_backdrop(ui, None, &p);
        let rect = ui.max_rect();
        if interactive {
            clicked = ui.interact(rect, egui::Id::new("splash-skip"), Sense::click()).clicked();
        }
        let scale = (rect.height() / 540.0).clamp(0.6, 1.0);
        let painter = ui.painter();
        let cx = rect.center().x;
        let mid = rect.top() + rect.height() * 0.44;
        painter.text(Pos2::new(cx, mid - 34.0 * scale), Align2::CENTER_BOTTOM, title, bold_font(40.0 * scale), Color32::WHITE);
        // A soft pulse on the status line while work goes on.
        let t = ui.input(|i| i.time) as f32;
        let pulse = 0.7 + 0.3 * (t * 2.2).sin().abs();
        painter.text(Pos2::new(cx, mid - 4.0 * scale), Align2::CENTER_CENTER, subtitle, FontId::proportional(15.0), Color32::from_white_alpha((200.0 * pulse) as u8));
        let w = (rect.width() * 0.42).clamp(180.0, 420.0);
        let bar = Rect::from_center_size(Pos2::new(cx, mid + 26.0 * scale), Vec2::new(w, 6.0));
        painter.rect_filled(bar, Rounding::same(3.0), Color32::from_white_alpha(28));
        let fill = Rect::from_min_size(bar.min, Vec2::new(bar.width() * shown, bar.height()));
        painter.rect_filled(fill, Rounding::same(3.0), p.accent);
        // A light running along the filled part.
        if shown > 0.02 && shown < 0.999 {
            let x = fill.left() + fill.width() * ((t * 0.6).fract());
            painter.rect_filled(Rect::from_center_size(Pos2::new(x, bar.center().y), Vec2::new(30.0, 6.0)), Rounding::same(3.0), Color32::from_white_alpha(70));
        }
        painter.text(Pos2::new(bar.right(), bar.bottom() + 8.0), Align2::RIGHT_TOP, format!("{:.0}%", shown * 100.0), FontId::proportional(12.0), Color32::from_white_alpha(150));
        // The tip near the bottom, wrapped.
        let g = painter.layout(tip.to_string(), FontId::proportional(13.5), Color32::from_white_alpha(160), (rect.width() - 64.0).min(560.0));
        let tip_pos = Pos2::new(cx - g.size().x / 2.0, rect.bottom() - 46.0 * scale - g.size().y);
        painter.galley(tip_pos, g, Color32::from_white_alpha(160));
        painter.text(Pos2::new(rect.right() - 12.0, rect.bottom() - 8.0), Align2::RIGHT_BOTTOM, format!("v{version}"), FontId::proportional(11.0), Color32::from_white_alpha(110));
    });
    ctx.request_repaint_after(std::time::Duration::from_millis(33));
    clicked
}

pub fn splash_screen(ctx:&egui::Context,icons:&GuiIcons,progress:f32,version:&str)->bool {
    let tip=TIPS[((ctx.input(|i|i.time)/5.0) as usize)%TIPS.len()];
    loading_panel(ctx,icons,"WolfCraft","Подготовка игры",progress,tip,version,true)
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

/// Classic game main menu, entered after launching from the launcher.
pub fn game_menu(ctx: &egui::Context, icons: &GuiIcons, version: &str, planet: Option<&TextureHandle>) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
    egui::CentralPanel::default().frame(Frame::none()).show(ctx, |ui| {
        planet_backdrop(ui, planet, &p);
        let rect = ui.max_rect();
        // Dock of icon buttons on a half-circle at the bottom; the menu column is centred above it.
        // Upright: the arc under the menu. Lying phone: the menu on the left, the arc on the right.
        let wide = rect.width() > rect.height() * 1.2;
        let (arc_r, arc_cx) = if wide {
            ((rect.width() * 0.5 * 0.42).min(rect.height() - 110.0).clamp(90.0, 170.0), rect.left() + rect.width() * 0.75)
        } else {
            ((rect.width() * 0.40).clamp(118.0, 170.0), rect.center().x)
        };
        let dock_h = arc_r + 70.0;
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("game-dock-h"), dock_h));
        let above = if wide { Rect::from_min_max(rect.min, Pos2::new(rect.center().x, rect.bottom())) } else { Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.bottom() - dock_h - 8.0)) };
        let width = (above.width() - 32.0).clamp(180.0, 360.0);
        let height = (above.height() - 16.0).clamp(150.0, 320.0);
        let col = Rect::from_center_size(above.center(), Vec2::new(width, height));
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(col), |ui| {
            egui::ScrollArea::vertical().id_salt("game-main-menu").show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("WolfCraft").font(bold_font(40.0)).color(Color32::WHITE));
                    ui.add_space(24.0);
                    ui.spacing_mut().item_spacing.y = 12.0;
                    let w = width.min(320.0);
                    if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, w, true).clicked() {
                        action = MenuAction::OpenWorldSelect;
                    }
                    if pill_button(ui, icons, Some(icon::GEAR), "Настройки", ButtonKind::Secondary, w, true).clicked() {
                        action = MenuAction::OpenSettings;
                    }
                    if pill_button(ui, icons, Some(icon::EXIT), "Выход", ButtonKind::Secondary, w, true)
                        .on_hover_text("Вернуться в лаунчер").clicked() {
                        action = MenuAction::Back;
                    }
                });
            });
        });
        if let Some(a) = game_dock(ui, icons, &p, rect, arc_cx, arc_r, version) {
            action = a;
        }
    });
    if let Some(a) = game_menu_sheet(ctx, icons, &p) {
        action = a;
    }
    action
}

/// What the dock's «Новости» and «Ещё» open over the menu.
#[derive(Clone, Copy, PartialEq)]
enum Sheet {
    News,
    More,
}

fn open_settings_at(ctx: &egui::Context, tab: usize) -> MenuAction {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("settings-tab"), tab));
    MenuAction::OpenSettings
}

/// The dock under the game menu: Скины, Магазин, Сеть, Новости, Ещё on a half-circle, each a
/// see-through round button with a soft icon and a caption; in the middle of the arc a small
/// version badge.
fn game_dock(ui: &mut Ui, icons: &GuiIcons, p: &Palette, rect: Rect, cx: f32, arc_r: f32, version: &str) -> Option<MenuAction> {
    const ITEMS: [(u16, &str); 5] = [(icon::BRUSH, "Скины"), (icon::BACKPACK, "Магазин"), (icon::GLOBE, "Сеть"), (icon::STAR, "Новости"), (icon::PROPS, "Ещё")];
    const BTN: f32 = 27.0;
    let c = Pos2::new(cx, rect.bottom() - 30.0);
    let painter = ui.painter().clone();
    // The arc itself: a faint line through the buttons.
    let n = 48;
    let pts: Vec<Pos2> = (0..=n).map(|k| {
        let a = std::f32::consts::PI * (1.0 + k as f32 / n as f32);
        c + Vec2::new(a.cos(), a.sin()) * arc_r
    }).collect();
    painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, Color32::from_white_alpha(28))));
    // Version badge in the middle.
    let short: String = version.split('.').take(2).collect::<Vec<_>>().join(".");
    painter.circle_filled(c, 26.0, Color32::from_white_alpha(14));
    painter.circle_stroke(c, 26.0, Stroke::new(1.0_f32, Color32::from_white_alpha(60)));
    painter.text(c - Vec2::new(0.0, 4.0), Align2::CENTER_CENTER, format!("v{short}"), bold_font(14.0), Color32::from_white_alpha(220));
    painter.text(c + Vec2::new(0.0, 10.0), Align2::CENTER_CENTER, "версия", FontId::proportional(9.0), Color32::from_white_alpha(130));
    let sheet_id = egui::Id::new("game-menu-sheet");
    let open: Option<Sheet> = ui.ctx().data(|d| d.get_temp(sheet_id)).flatten();
    let mut out = None;
    for (i, (ic, label)) in ITEMS.iter().enumerate() {
        let a = std::f32::consts::PI * (1.0 + (i as f32 + 0.5) / ITEMS.len() as f32);
        let at = c + Vec2::new(a.cos(), a.sin()) * arc_r;
        // Button and caption together are the tap area.
        let r = Rect::from_min_max(at - Vec2::new(BTN + 6.0, BTN), at + Vec2::new(BTN + 6.0, BTN + 22.0));
        let resp = ui.interact(r, egui::Id::new(("game-dock", i)), Sense::click());
        let on = (i == 3 && open == Some(Sheet::News)) || (i == 4 && open == Some(Sheet::More));
        let pressed = resp.is_pointer_button_down_on();
        let fill = if on { p.accent.gamma_multiply(0.35) } else if pressed || resp.hovered() { Color32::from_white_alpha(40) } else { Color32::from_white_alpha(16) };
        ui.painter().circle_filled(at, BTN, fill);
        ui.painter().circle_stroke(at, BTN, Stroke::new(1.2_f32, if on { p.accent } else { Color32::from_white_alpha(70) }));
        let col = if on { p.accent } else { Color32::from_white_alpha(200) };
        icons.paint(ui.painter(), Rect::from_center_size(at, Vec2::splat(24.0)), *ic, col);
        ui.painter().text(Pos2::new(at.x, at.y + BTN + 11.0), Align2::CENTER_CENTER, *label, FontId::proportional(11.5), if on { p.accent } else { Color32::from_white_alpha(170) });
        if resp.clicked() {
            match i {
                0 => out = Some(open_settings_at(ui.ctx(), 0)),
                1 => out = Some(open_settings_at(ui.ctx(), 2)),
                2 => out = Some(MenuAction::OpenNetwork),
                _ => {
                    let want = if i == 3 { Sheet::News } else { Sheet::More };
                    let next = if open == Some(want) { None } else { Some(want) };
                    ui.ctx().data_mut(|d| d.insert_temp(sheet_id, next));
                }
            }
        }
    }
    out
}

/// «Новости» and «Ещё»: a card above the dock.
fn game_menu_sheet(ctx: &egui::Context, icons: &GuiIcons, p: &Palette) -> Option<MenuAction> {
    let sheet_id = egui::Id::new("game-menu-sheet");
    let open: Sheet = ctx.data(|d| d.get_temp::<Option<Sheet>>(sheet_id)).flatten()?;
    let screen = ctx.screen_rect();
    let w = (screen.width() - 32.0).min(420.0);
    let mut out = None;
    let mut close = false;
    egui::Area::new(egui::Id::new("game-menu-sheet-area")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -ctx.data(|d| d.get_temp::<f32>(egui::Id::new("game-dock-h"))).unwrap_or(96.0) - 8.0)).show(ctx, |ui| {
        Frame::none().fill(p.surface).rounding(Rounding::same(20.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
            ui.set_width(w - 32.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(if open == Sheet::News { "Что нового" } else { "Ещё" }).font(bold_font(18.0)).color(p.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if square_button(ui, icons, icon::CLOSE, false).clicked() {
                        close = true;
                    }
                });
            });
            ui.add_space(6.0);
            match open {
                Sheet::News => {
                    egui::ScrollArea::vertical().max_height((screen.height() * 0.45).max(120.0)).show(ui, |ui| {
                        for (title, text) in NEWS {
                            ui.label(egui::RichText::new(title).font(bold_font(14.5)).color(p.text));
                            ui.label(egui::RichText::new(text).size(13.0).color(p.text_muted));
                            ui.add_space(6.0);
                        }
                    });
                }
                Sheet::More => {
                    let bw = ui.available_width();
                    for (ic, label, tab) in [(icon::REFRESH, "Обновления", 6usize), (icon::CROSSHAIR, "Управление", 5), (icon::EDIT, "Интерфейс", 4), (icon::SUN, "Графика", 1), (icon::HAMMER, "Рука и вид", 3)] {
                        if pill_button(ui, icons, Some(ic), label, ButtonKind::Secondary, bw, true).clicked() {
                            out = Some(open_settings_at(ctx, tab));
                            close = true;
                        }
                    }
                    if pill_button(ui, icons, Some(icon::ARROW_LEFT), "В лаунчер", ButtonKind::Ghost, bw, true).clicked() {
                        out = Some(MenuAction::Back);
                        close = true;
                    }
                }
            }
        });
    });
    if close {
        ctx.data_mut(|d| d.insert_temp::<Option<Sheet>>(sheet_id, None));
    }
    out
}

/// «Новости» in the game menu: what came in recent updates.
const NEWS: [(&str, &str); 6] = [
    ("Оружие", "Пистолет, автомат, дробовик и снайперская винтовка с 3D-моделями, прицелами и отдачей. Патроны и дробь — в «Вооружении»."),
    ("Разрушаемость", "Постройки без опоры рушатся, блоки падают и разбиваются, взрывы раскидывают обломки."),
    ("Игра по сети", "Короткие коды входа: введите код сервера во вкладке «По коду»."),
    ("WolfStudio", "Свои серверы как плейсы: редактор, свободная камера, «Играть», скрипты блоков на Luau."),
    ("Мини-игры", "«Питомник» и «Симулятор кнопок» — шаблоны для серверов в Тулбоксе студии."),
    ("Взрывчатка", "Динамит с зажигалкой, гранаты, гранатомёт и базука."),
];

/// Home screen: the mobile launcher (toolbar, pages, bottom navigation).
pub fn main_menu(ctx: &egui::Context, icons: &GuiIcons, version: &str, data_dir: &str, dark: bool, planet: Option<&TextureHandle>, skin: u8, nickname: &str, worlds: &[WorldEntry]) -> MenuAction {
    let _ = data_dir;
    crate::launcher::show(ctx, icons, version, dark, planet, skin, nickname, worlds)
}

// ---------------------------------------------------------------------------------------------
// World selection

#[derive(Default)]
pub struct WorldSelectState {
    /// 0 = single player (saved worlds), 1 = play over the network.
    pub mode: usize,
    /// Address typed for a network game.
    pub address: String,
    /// Header tabs in "Игра по сети": 0 nearby (found on the local network), 1 host a
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
    /// «Вставить» pressed: the app puts the clipboard into `address` (phones cannot paste
    /// into the game's own text fields).
    pub paste_request: bool,
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
                    // A tap anywhere on the row joins too (not only on «Играть»).
                    let row = row_card(ui, false, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                            icons.paint(ui.painter(), r, icon::GLOBE, p.success);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(name).font(bold_font(16.0)).color(p.text));
                                ui.label(egui::RichText::new(format!("игроков {players}/{max} · {addr}")).size(12.5).color(p.text_muted));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let b = pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, 0.0, players < max);
                                // Where the button is and what it saw (read by the tap test).
                                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("dbg_play"), (b.rect, b.hovered(), b.contains_pointer(), b.is_pointer_button_down_on(), b.enabled())));
                                if b.clicked() {
                                    st.net_error = None;
                                    act = Some(MenuAction::JoinServer(addr.clone()));
                                }
                            });
                        });
                    });
                    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("dbg_row"), (row.rect, row.hovered(), row.contains_pointer(), row.is_pointer_button_down_on(), row.sense.click)));
                    if row.clicked() && players < max && act.is_none() {
                        st.net_error = None;
                        act = Some(MenuAction::JoinServer(addr.clone()));
                    }
                    ui.add_space(6.0);
                }
                if st.nearby.is_empty() {
                    ui.label(egui::RichText::new("Пока никого. Сервер появится здесь сам, как только друг его откроет.").size(13.5).color(p.text_muted));
                }
                ui.add_space(10.0);
                for (title, text) in [
                    ("Одна сеть Wi-Fi", "Один игрок открывает мир во вкладке «Создать сервер» — у остальных он сразу появляется в этом списке."),
                    ("Только мобильный интернет", "Операторы не пропускают входящие подключения, поэтому напрямую через мобильную сеть сыграть нельзя. Включите точку доступа на телефоне с сервером и подключите к ней друзей: сервер появится здесь, а интернет на игру не тратится."),
                    ("Код приглашения", "Сервер показывает короткий код вида K7QX2 (в чате, в WolfStudio и в WolfServer). Введите его во вкладке «По коду»."),
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
                    let mut trash = Rect::NOTHING;
                    let row = row_card(ui, false, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                            icons.paint(ui.painter(), r, icon::GLOBE, p.accent);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(name).font(bold_font(16.0)).color(p.text));
                                ui.label(egui::RichText::new(addr).size(12.5).color(p.text_muted));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let t = icon_button(ui, icons, icon::TRASH, 34.0, ButtonKind::Danger);
                                trash = t.rect;
                                if t.clicked() {
                                    remove = Some(i);
                                }
                                if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, 0.0, true).clicked() {
                                    act = Some(MenuAction::JoinServer(addr.clone()));
                                }
                            });
                        });
                    });
                    if row.clicked() && act.is_none() && remove.is_none() {
                        let on_trash = row.interact_pointer_pos().is_some_and(|p| trash.expand(4.0).contains(p));
                        if on_trash {
                            remove = Some(i);
                        } else {
                            act = Some(MenuAction::JoinServer(addr.clone()));
                        }
                    }
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
                            ui.painter().rect_filled(r, Rounding::same(10.0), Color32::from_rgb(8, 12, 22));
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
            let full = ui.available_width().min(520.0);
            text_input(ui, icons, &mut st.address, "Код K7QX2 или адрес 192.168.0.10", full, Some(icon::GLOBE));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let half = (full - 8.0) / 2.0;
                if pill_button(ui, icons, Some(icon::PASTE), "Вставить", ButtonKind::Secondary, half, true).clicked() {
                    st.paste_request = true;
                }
                let ready = !st.address.trim().is_empty();
                if pill_button(ui, icons, Some(icon::PLAY), "Подключиться", ButtonKind::Primary, half, ready).clicked() {
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
    ui.painter().rect_filled(rect, Rounding::same(10.0), fill);
    ui.painter().rect_stroke(rect, Rounding::same(10.0), Stroke::new(1.0_f32, stroke));
    icons.paint(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(22.0)), ic, fg);
    resp
}

/// World-select left sidebar, vertical: back, play modes, world actions (create, play,
/// search, sort) and settings. Search field and sort options open inside the sidebar.
fn world_select_side(ui: &mut Ui, icons: &GuiIcons, st: &mut WorldSelectState, worlds: &[WorldEntry], version: &str) -> MenuAction {
    let p = palette(ui.ctx());
    let mut action = MenuAction::None;
    let w = ui.available_width();
    ui.spacing_mut().item_spacing.y = 8.0;
    let section = |ui: &mut Ui, text: &str| {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(text).size(11.5).color(p.text_muted));
    };
    ui.horizontal(|ui| {
        if square_button(ui, icons, icon::ARROW_LEFT, false).on_hover_text("Назад").clicked() {
            action = MenuAction::Back;
        }
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(egui::RichText::new("Миры").font(bold_font(20.0)).color(p.text));
            ui.label(egui::RichText::new(format!("v{version}")).size(11.0).color(p.text_muted));
        });
    });
    section(ui, "РЕЖИМ");
    for (i, ic, label) in [(0usize, icon::CUBE, "Одиночная игра"), (1, icon::GLOBE, "Игра по сети")] {
        let kind = if st.mode == i { ButtonKind::Primary } else { ButtonKind::Secondary };
        if pill_button(ui, icons, Some(ic), label, kind, w, true).clicked() {
            st.mode = i;
        }
    }
    if st.mode == 0 {
        section(ui, "МИРЫ");
        if pill_button(ui, icons, Some(icon::PLUS), "Создать мир", ButtonKind::Primary, w, true).clicked() {
            action = MenuAction::OpenCreate;
        }
        let can = st.selected.as_ref().is_some_and(|id| worlds.iter().any(|w| &w.cfg.id == id));
        if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Secondary, w, can).clicked() {
            if let Some(id) = &st.selected {
                action = MenuAction::PlayWorld(id.clone());
            }
        }
        let kind = if st.search_open { ButtonKind::Primary } else { ButtonKind::Secondary };
        if pill_button(ui, icons, Some(if st.search_open { icon::CLOSE } else { icon::SEARCH }), "Поиск", kind, w, true).clicked() {
            st.search_open = !st.search_open;
            st.search_focus = st.search_open;
            if !st.search_open {
                st.search.clear();
            }
        }
        if st.search_open {
            let r = text_input(ui, icons, &mut st.search, "Название…", w, None);
            if st.search_focus {
                st.search_focus = false;
                r.request_focus();
            }
        }
        let kind = if st.sort_open { ButtonKind::Primary } else { ButtonKind::Secondary };
        if pill_button(ui, icons, Some(icon::SORT), "Сортировка", kind, w, true).clicked() {
            st.sort_open = !st.sort_open;
        }
        if st.sort_open {
            for (i, name) in SORT_NAMES.iter().enumerate() {
                let kind = if st.sort == i { ButtonKind::Primary } else { ButtonKind::Ghost };
                if pill_button(ui, icons, if st.sort == i { Some(icon::CHECK) } else { None }, name, kind, w, true).clicked() {
                    st.sort = i;
                    st.sort_open = false;
                }
            }
        }
    }
    section(ui, "ЕЩЁ");
    if pill_button(ui, icons, Some(icon::GEAR), "Настройки", ButtonKind::Secondary, w, true).clicked() {
        action = MenuAction::OpenSettings;
    }
    action
}

pub fn world_select(ctx: &egui::Context, icons: &GuiIcons, st: &mut WorldSelectState, worlds: &[WorldEntry], version: &str) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
    let compact = ctx.screen_rect().width() < 720.0;
    if compact {
        egui::TopBottomPanel::top("worlds-phone-header")
            .frame(Frame::none().fill(p.surface).inner_margin(Margin::same(12.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if square_button(ui, icons, icon::ARROW_LEFT, false).clicked() { action = MenuAction::Back; }
                    ui.label(egui::RichText::new("Миры").font(bold_font(21.0)).color(p.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if square_button(ui, icons, icon::GEAR, false).clicked() { action = MenuAction::OpenSettings; }
                    });
                });
                ui.horizontal(|ui| {
                    let half = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
                    for (i, label) in [(0usize, "Одиночная"), (1, "По сети")] {
                        if pill_button(ui, icons, None, label, if st.mode == i { ButtonKind::Primary } else { ButtonKind::Secondary }, half, true).clicked() { st.mode = i; }
                    }
                });
                if st.mode == 0 {
                    ui.horizontal(|ui| {
                        let w = (ui.available_width() - 100.0).max(100.0);
                        if pill_button(ui, icons, Some(icon::PLUS), "Создать мир", ButtonKind::Primary, w, true).clicked() { action = MenuAction::OpenCreate; }
                        if square_button(ui, icons, icon::SEARCH, st.search_open).clicked() {
                            st.search_open = !st.search_open;
                            st.search_focus = st.search_open;
                            if !st.search_open { st.search.clear(); }
                        }
                        if square_button(ui, icons, icon::SORT, st.sort_open).clicked() { st.sort_open = !st.sort_open; }
                    });
                    if st.search_open {
                        let r = text_input(ui, icons, &mut st.search, "Название…", ui.available_width(), None);
                        if st.search_focus { st.search_focus = false; r.request_focus(); }
                    }
                    if st.sort_open {
                        egui::ScrollArea::horizontal().id_salt("world-sort-phone").show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (i, label) in SORT_NAMES.iter().enumerate() {
                                    if pill_button(ui, icons, None, label, if st.sort == i { ButtonKind::Primary } else { ButtonKind::Secondary }, 0.0, true).clicked() {
                                        st.sort = i; st.sort_open = false;
                                    }
                                }
                            });
                        });
                    }
                } else {
                    egui::ScrollArea::horizontal().id_salt("network-phone-tabs").show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (i, label) in [(0usize, "Рядом"), (1, "Создать сервер"), (2, "По коду"), (3, "Мои серверы")] {
                                if pill_button(ui, icons, None, label, if st.net_tab == i { ButtonKind::Primary } else { ButtonKind::Secondary }, 0.0, true).clicked() { st.net_tab = i; }
                            }
                        });
                    });
                }
            });
    } else {
    // Left sidebar: every control, stacked vertically.
    let side_w = (ctx.screen_rect().width() * 0.24).clamp(170.0, 230.0);
    egui::SidePanel::left("play-mode-side")
        .resizable(false)
        .exact_width(side_w)
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(12.0)))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                action = world_select_side(ui, icons, st, worlds, version);
            });
        });
    if st.mode == 1 {
        // Right sidebar in "Игра по сети": its sections, mirroring the left one.
        egui::SidePanel::right("net-side")
            .resizable(false)
            .exact_width(side_w)
            .frame(Frame::none().fill(p.surface).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(12.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("СЕТЬ").size(11.5).color(p.text_muted));
                    for (i, ic, label) in [(0usize, icon::SEARCH, "Рядом"), (1, icon::STAR, "Создать сервер"), (2, icon::EDIT, "По коду"), (3, icon::GLOBE, "Мои серверы")] {
                        let kind = if st.net_tab == i { ButtonKind::Primary } else { ButtonKind::Secondary };
                        if pill_button(ui, icons, Some(ic), label, kind, ui.available_width(), true).clicked() {
                            st.net_tab = i;
                        }
                    }
                });
            });
    }
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
                // Studio projects are not shown here, so "no worlds" counts only game worlds.
                let none = st.search.trim().is_empty() || !worlds.iter().any(|w| !w.cfg.studio_project);
                ui.label(egui::RichText::new(if none { "Миров пока нет" } else { "Ничего не найдено" }).font(bold_font(20.0)).color(p.text));
                ui.label(egui::RichText::new(if none { "Создайте первую планету — нажмите «Создать мир»" } else { "Измените запрос поиска" }).color(p.text_muted));
            });
            return;
        }
        // Vertical list: planet preview, name and tags, then play / rename / delete on the right.
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            let full = ui.available_width();
            for w in &list {
                let selected = st.selected.as_deref() == Some(w.cfg.id.as_str());
                let resp = row_card(ui, selected, |ui| {
                    ui.set_width(full - 26.0);
                    if compact {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
                            if let Some(tex) = &w.preview {
                                ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                            } else { ui.painter().rect_filled(r, Rounding::same(8.0), p.surface); }
                            ui.vertical(|ui| {
                                ui.add(egui::Label::new(egui::RichText::new(&w.cfg.name).font(bold_font(17.0)).color(p.text)).truncate());
                                ui.label(egui::RichText::new(format!("{} · {}", if w.cfg.mode == GameMode::Creative { "Творческий" } else { "Выживание" }, w.cfg.size.label())).size(12.0).color(p.text_muted));
                            });
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let play_w = (ui.available_width() - 100.0).max(100.0);
                            if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, play_w, true).clicked() { action = MenuAction::PlayWorld(w.cfg.id.clone()); }
                            if icon_button(ui, icons, icon::EDIT, 40.0, ButtonKind::Ghost).clicked() { st.rename = Some((w.cfg.id.clone(), w.cfg.name.clone())); }
                            if icon_button(ui, icons, icon::TRASH, 40.0, ButtonKind::Danger).clicked() { st.confirm_delete = Some(w.cfg.id.clone()); }
                        });
                        return;
                    }
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
                        if let Some(tex) = &w.preview {
                            ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                        } else {
                            ui.painter().rect_filled(r, Rounding::same(10.0), Color32::from_rgb(8, 12, 22));
                        }
                        let buttons = 130.0 + 2.0 * 40.0 + 2.0 * 12.0;
                        let text_w = (ui.available_width() - buttons).max(80.0);
                        ui.allocate_ui(Vec2::new(text_w, 64.0), |ui| {
                            ui.set_width(text_w);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.add(egui::Label::new(egui::RichText::new(&w.cfg.name).font(bold_font(17.0)).color(p.text)).truncate());
                                ui.label(egui::RichText::new(format!("{} · {}", if w.cfg.mode == GameMode::Creative { "Творческий" } else { "Выживание" }, w.cfg.size.label())).size(12.5).color(if w.cfg.mode == GameMode::Creative { p.secondary } else { p.accent }));
                                ui.label(egui::RichText::new(format_timestamp(w.cfg.last_played)).size(11.5).color(p.text_muted));
                            });
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, icons, icon::TRASH, 40.0, ButtonKind::Danger).clicked() {
                                st.confirm_delete = Some(w.cfg.id.clone());
                            }
                            if icon_button(ui, icons, icon::EDIT, 40.0, ButtonKind::Ghost).clicked() {
                                st.rename = Some((w.cfg.id.clone(), w.cfg.name.clone()));
                            }
                            if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, 130.0, true).clicked() {
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
    if ctx.screen_rect().width() < 720.0 { return create_world_phone(ctx, icons, st, preview, version, random_seed); }
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
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::symmetric(14.0, 18.0)))
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
        .frame(Frame::none().fill(p.bg).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(14.0)))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("cw-preview-scroll").auto_shrink([false, false]).show(ui, |ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(globe), Sense::hover());
                ui.painter().rect_filled(r, Rounding::same(10.0), Color32::from_rgb(6, 9, 18));
                if let Some(tex) = preview {
                    ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                } else {
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, "Генерация…", FontId::proportional(15.0), p.text_muted);
                }
                ui.painter().rect_stroke(r, Rounding::same(10.0), Stroke::new(1.0_f32, p.outline));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(format!("Seed: {}", st.effective_seed() as i64)).size(13.0).color(p.text_muted));
                let mins = st.size.circumference() as f32 / 4.3 / 60.0;
                ui.label(egui::RichText::new(format!("Радиус {} блоков · обход ≈ {:.0} мин", st.size.radius(), mins)).size(13.0).color(p.text_muted));
            });
        });
    // Settings of the chosen section on the right.
    egui::CentralPanel::default().frame(body_frame(&p)).show(ctx, |ui| {
        egui::ScrollArea::both().id_salt("cw-form").auto_shrink([false, false]).show(ui, |ui| {
            create_world_form(ui, icons, st, random_seed);
        });
    });
    action
}


fn create_world_form(ui: &mut Ui, icons: &GuiIcons, st: &mut CreateWorldState, random_seed: u64) {
            ui.spacing_mut().item_spacing.y = 6.0;
            if st.tab == 0 {
                stacked_row(ui, "Название мира", None, |ui| {
                    text_input(ui, icons, &mut st.name, "Новый мир", ui.available_width().min(300.0), Some(icon::EDIT));
                });
                ui.add_space(10.0);
                stacked_row(ui, "Seed", Some("Пусто — случайный. Можно ввести число или текст"), |ui| {
                    if text_input(ui, icons, &mut st.seed_text, "Случайный", (ui.available_width() - 48.0).clamp(80.0, 250.0), Some(icon::GLOBE)).changed() {
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

}

fn create_world_phone(ctx: &egui::Context, icons: &GuiIcons, st: &mut CreateWorldState, preview: Option<&TextureHandle>, version: &str, random_seed: u64) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
    egui::TopBottomPanel::top("create-phone-header")
        .frame(Frame::none().fill(p.surface).inner_margin(Margin::same(12.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Создать мир").font(bold_font(21.0)).color(p.text));
                ui.label(egui::RichText::new(format!("v{version}")).size(11.0).color(p.text_muted));
            });
            ui.horizontal(|ui| {
                let half = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
                for (i, label) in [(0usize, "Основное"), (1, "Планета")] {
                    if pill_button(ui, icons, None, label, if st.tab == i { ButtonKind::Primary } else { ButtonKind::Secondary }, half, true).clicked() { st.tab = i; }
                }
            });
        });
    egui::TopBottomPanel::bottom("create-phone-actions")
        .frame(Frame::none().fill(p.surface).inner_margin(Margin::same(12.0)))
        .show(ctx, |ui| {
            if let Some(error) = &st.error { ui.label(egui::RichText::new(error).color(p.warn)); }
            ui.horizontal(|ui| {
                let create_w = (ui.available_width() - 108.0).max(140.0);
                if pill_button(ui, icons, Some(icon::ARROW_LEFT), "Назад", ButtonKind::Secondary, 100.0, true).clicked() { action = MenuAction::Back; }
                if pill_button(ui, icons, Some(icon::CUBE), "Создать мир", ButtonKind::Primary, create_w, true).clicked() {
                    match st.to_config(random_seed) {
                        Ok(cfg) => { st.error = None; action = MenuAction::CreateWorld(Box::new(cfg)); }
                        Err(error) => st.error = Some(error),
                    }
                }
            });
        });
    egui::CentralPanel::default().frame(Frame::none().fill(p.bg).inner_margin(Margin::same(16.0))).show(ctx, |ui| {
        egui::ScrollArea::vertical().id_salt("create-phone-body").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let globe = ui.available_width().min(200.0);
            ui.vertical_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(globe), Sense::hover());
                ui.painter().rect_filled(r, Rounding::same(12.0), Color32::from_rgb(6, 9, 18));
                if let Some(tex) = preview { ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE); }
                else { ui.painter().text(r.center(), Align2::CENTER_CENTER, "Генерация…", FontId::proportional(15.0), p.text_muted); }
                ui.label(egui::RichText::new(format!("Seed: {}", st.effective_seed() as i64)).size(12.0).color(p.text_muted));
            });
            ui.add_space(12.0);
            create_world_form(ui, icons, st, random_seed);
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
    pub open_updates: bool,
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

const SETTINGS_TABS: [(u16, &str); 7] = [(icon::HEART, "Профиль"), (icon::SUN, "Графика"), (icon::CUBE, "Текстуры"), (icon::HAMMER, "Рука"), (icon::EDIT, "Интерфейс"), (icon::CROSSHAIR, "Управление"), (icon::REFRESH, "Обновления")];

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
            ui.set_width(ui.available_width());
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
            if wolfcraft_assets::packs::is_toon(&pack.id) && s.shaders != 4 {
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
    let tab_id = egui::Id::new("settings-tab");
    let mut tab: usize = ctx.data(|d| d.get_temp(tab_id)).unwrap_or(0);
    tab = tab.min(SETTINGS_TABS.len() - 1);
    egui::TopBottomPanel::top("settings-header")
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::symmetric(14.0, 8.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if square_button(ui, icons, icon::ARROW_LEFT, false).on_hover_text("Назад").clicked() {
                    res.back = true;
                }
                ui.label(egui::RichText::new("Настройки").font(bold_font(21.0)).color(p.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("v{version}")).size(12.0).color(p.text_muted));
                    if ui.available_width() > 270.0 {
                        ui.label(egui::RichText::new("Сохраняется автоматически").size(12.0).color(p.text_muted));
                    }
                });
            });
            ui.add_space(4.0);
            egui::ScrollArea::horizontal().id_salt("settings-tabs-scroll").auto_shrink([false, true]).show(ui, |ui| {
                let labels: Vec<&str> = SETTINGS_TABS.iter().map(|(_, label)| *label).collect();
                tab_bar(ui, "settings-section-tabs", &labels, &mut tab);
            });
        });
    ctx.data_mut(|d| d.insert_temp(tab_id, tab));
    egui::CentralPanel::default().frame(body_frame(&p)).show(ctx, |ui| {
        egui::ScrollArea::vertical().id_salt("settings-body").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            match tab {
                6 => {
                    ui.label(format!("Установлена WolfCraft {version}"));
                    ui.label(if cfg!(target_os="android") { "Обновления устанавливаются внутри приложения. Если понадобится новый загрузчик, приложение сообщит об этом." } else { "Новая версия устанавливается поверх текущей. Не удаляйте папку миров перед обновлением." });
                    ui.label(if cfg!(target_os="android") { "Для закрытого репозитория ключ чтения вводится один раз в окне обновления." } else { "Приватный GitHub потребует входа в ваш аккаунт." });
                    if pill_button(ui, icons, Some(icon::REFRESH), if cfg!(target_os="android") {"Проверить обновления"} else {"Обновления на GitHub"}, ButtonKind::Primary, 250.0, true).clicked() {
                        res.open_updates = true;
                    }
                    ui.add_space(12.0);
                    ui.label("Текстуры обновляются отдельно: раздел «Текстуры» → загрузка ZIP-пака по ссылке.");
                }
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

/// World loading, in the style of the studio splash: the same dark backdrop, an outlined
/// ring in the middle that fills with small outlined circles as the world gets ready, the
/// percentage in its centre, and the splash's loading bar with the current step below.
pub fn loading_screen(ctx:&egui::Context,icons:&GuiIcons,v:&LoadingView) {
    // A friendly line instead of the stage name and chunk counters.
    let f = v.fraction;
    let subtitle = if f < 0.2 { "Готовим планету…" } else if f < 0.4 { "Рисуем материки и океаны…" } else if f < 0.75 { "Строим землю вокруг вас…" } else { "Почти готово…" };
    let _ = (v.stages, v.current, v.detail, v.grid);
    let tip = TIPS[((ctx.input(|i| i.time) / 6.0) as usize) % TIPS.len()];
    let title = v.title.trim_start_matches("Мир ").trim_matches(|c| c == '«' || c == '»');
    loading_panel(ctx, icons, title, subtitle, f, tip, wolfcraft_common::VERSION, false);
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
            .rounding(Rounding::same(10.0))
            .stroke(Stroke::new(1.0_f32, p.outline))
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

    #[test]
    fn classic_game_menu_buttons_open_worlds_settings_and_launcher() {
        for size in [Vec2::new(1280.0, 720.0), Vec2::new(800.0, 360.0)] {
            let (ctx, icons) = setup();
            for (label, expected) in [("Играть", MenuAction::OpenWorldSelect), ("Настройки", MenuAction::OpenSettings), ("Выход", MenuAction::Back)] {
                let mut action = MenuAction::None;
                let mut frame = |events| ctx.run(egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), events, ..Default::default()
                }, |ctx| { action = game_menu(ctx, &icons, "0.2.1", None); });
                let out = frame(vec![]);
                let text = out.shapes.iter().find_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t), _ => None
                }).expect("classic menu button");
                let pos = text.pos + text.galley.size() * 0.5;
                frame(vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
                frame(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
                assert_eq!(action, expected, "{label}, {size:?}");
            }
            // The dock under the menu.
            for (label, expected) in [("Сеть", MenuAction::OpenNetwork), ("Магазин", MenuAction::OpenSettings), ("Скины", MenuAction::OpenSettings)] {
                let mut action = MenuAction::None;
                let mut frame = |events| ctx.run(egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), events, ..Default::default()
                }, |ctx| { action = game_menu(ctx, &icons, "0.2.1", None); });
                let out = frame(vec![]);
                let text = out.shapes.iter().find_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t), _ => None
                }).expect("dock item");
                let pos = text.pos + text.galley.size() * 0.5;
                frame(vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
                frame(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
                assert_eq!(action, expected, "{label}, {size:?}");
            }
            let tab: usize = ctx.data(|d| d.get_temp(egui::Id::new("settings-tab"))).unwrap_or(99);
            assert_eq!(tab, 0, "«Скины» opens the profile settings");
        }
    }


    #[test]
    fn portrait_worlds_put_controls_above_full_width_cards_and_play_works() {
        let (ctx, icons) = setup();
        let worlds = vec![WorldEntry { cfg: WorldConfig::new("Телефон", 42), preview: None }];
        let mut st = WorldSelectState::default();
        let render = |st: &mut WorldSelectState, events| {
            let mut action = MenuAction::None;
            let out = ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 844.0))),
                events, ..Default::default()
            }, |ctx| { action = world_select(ctx, &icons, st, &worlds, "0.3.0"); });
            (out, action)
        };
        render(&mut st, vec![]);
        let (out, _) = render(&mut st, vec![]);
        let text = |label: &str| out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t.pos + t.galley.size() * 0.5), _ => None
        }).expect(label);
        let single = text("Одиночная");
        let network = text("По сети");
        let create = text("Создать мир");
        let world = text("Телефон");
        let play = text("Играть");
        assert!((single.y - network.y).abs() < 2.0);
        assert!(create.y < world.y && world.y < play.y);
        assert!(play.x > 0.0 && play.x < 390.0 && play.y < 844.0);
        render(&mut st, vec![egui::Event::PointerMoved(play), egui::Event::PointerButton { pos: play, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        let (_, action) = render(&mut st, vec![egui::Event::PointerButton { pos: play, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        assert_eq!(action, MenuAction::PlayWorld(worlds[0].cfg.id.clone()));
    }

    #[test]
    fn portrait_game_menu_keeps_buttons_and_dock_inside_phone_screen() {
        for width in [320.0, 390.0] {
            let (ctx, icons) = setup();
            let mut render = || ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 740.0))),
                ..Default::default()
            }, |ctx| { game_menu(ctx, &icons, "0.3.0", None); });
            render();
            let out = render();
            for label in ["WolfCraft", "Играть", "Настройки", "Выход", "Скины", "Магазин", "Сеть", "Новости", "Ещё"] {
                let r = out.shapes.iter().find_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t.galley.rect.translate(t.pos.to_vec2())), _ => None
                }).expect(label);
                assert!(r.left() >= 0.0 && r.right() <= width + 1.0 && r.top() >= 0.0 && r.bottom() <= 740.0, "{label}: {r:?}");
            }
        }
    }

    #[test]
    fn portrait_settings_stack_nickname_and_wrap_skins() {
        let (ctx, icons) = setup();
        let mut settings = Settings::default();
        let packs = PacksView { list: &[], status: "", folder: "" };
        let mut render = || ctx.run(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 844.0))),
            ..Default::default()
        }, |ctx| { settings_screen(ctx, &icons, &mut settings, "0.3.0", &packs); });
        render();
        let out = render();
        let text = |label: &str| out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(Rect::from_min_size(t.pos, t.galley.size())), _ => None
        }).expect(label);
        assert!(text("Никнейм").bottom() < text("Игрок").top());
        for label in wolfcraft_engine::renderer::model::SKIN_NAMES {
            assert!(text(label).right() <= 390.0);
        }
    }

    #[test]
    fn world_selection_has_vertical_sidebars() {
        let (ctx, icons) = setup();
        let worlds = vec![WorldEntry { cfg: WorldConfig::new("Side test world", 42), preview: None }];
        let mut st = WorldSelectState::default();
        let render = |st: &mut WorldSelectState, events| ctx.run(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 540.0))),
            events, ..Default::default()
        }, |ctx| { let _ = world_select(ctx, &icons, st, &worlds, "0.2.1"); });
        render(&mut st, vec![]);
        let out = render(&mut st, vec![]);
        let text = |out: &egui::FullOutput, label: &str| out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t.pos + t.galley.size() * 0.5), _ => None
        }).expect(label);
        let single = text(&out, "Одиночная игра");
        let network = text(&out, "Игра по сети");
        let create = text(&out, "Создать мир");
        assert!(single.x < 240.0 && network.x < 240.0 && create.x < 240.0, "controls in the left sidebar");
        assert!(single.y < network.y && network.y < create.y, "stacked vertically");
        assert!(text(&out, "Side test world").x > 240.0, "list right of the sidebar");
        render(&mut st, vec![egui::Event::PointerMoved(network), egui::Event::PointerButton { pos: network, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        render(&mut st, vec![egui::Event::PointerButton { pos: network, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        assert_eq!(st.mode, 1);
        render(&mut st, vec![]);
        let out = render(&mut st, vec![]);
        assert!(text(&out, "По коду").x > 720.0, "network sections in the right sidebar");
    }

    #[test]
    fn settings_sections_are_tabs_below_the_header() {
        let (ctx, icons) = setup();
        let mut settings = Settings::default();
        let packs = PacksView { list: &[], status: "", folder: "" };
        let mut render = |events| ctx.run(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 540.0))),
            events, ..Default::default()
        }, |ctx| { settings_screen(ctx, &icons, &mut settings, "0.2.1", &packs); });
        render(vec![]);
        let out = render(vec![]);
        let text = |label: &str| out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t), _ => None
        }).expect("settings label");
        let title = text("Настройки");
        let graphics = text("Графика");
        assert!(graphics.pos.y > title.pos.y && graphics.pos.y < 110.0);
        assert!(text("Никнейм").pos.x < 100.0, "settings content must use the former sidebar space");
        let pos = graphics.pos + graphics.galley.size() * 0.5;
        render(vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        render(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        assert_eq!(ctx.data(|d| d.get_temp::<usize>(egui::Id::new("settings-tab"))), Some(1));
    }

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

    /// A tap = finger down in one frame, up in the next (like a phone). Returns the action of
    /// the frame where the finger lifts.
    fn tap(ctx: &egui::Context, icons: &GuiIcons, st: &mut WorldSelectState, at: Pos2) -> MenuAction {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 720.0));
        let mut act = MenuAction::None;
        for pressed in [true, false] {
            let mut raw = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            raw.events.push(egui::Event::PointerMoved(at));
            raw.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() });
            let _ = ctx.run(raw, |c| {
                let a = world_select(c, icons, st, &[], "0.3.0");
                if a != MenuAction::None {
                    act = a;
                }
            });
            if std::env::var("TAP_TRACE").is_ok() {
                let b: Option<(Rect, bool, bool, bool, bool)> = ctx.data(|d| d.get_temp(egui::Id::new("dbg_play")));
                let r: Option<(Rect, bool, bool, bool, bool)> = ctx.data(|d| d.get_temp(egui::Id::new("dbg_row")));
                println!("  {} at {at:?}: play {b:?} row {r:?} act {act:?}", if pressed { "down" } else { "up  " });
            }
        }
        // Pointer leaves the screen between taps.
        let mut raw = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        raw.events.push(egui::Event::PointerGone);
        let _ = ctx.run(raw, |c| {
            world_select(c, icons, st, &[], "0.3.0");
        });
        act
    }

    #[test]
    fn tap_on_nearby_server_joins_it() {
        let (ctx, icons) = setup();
        let mut st = WorldSelectState { mode: 1, net_tab: 0, ..Default::default() };
        st.nearby = vec![("Сервер".into(), "10.8.181.7:25570".into(), 0, 16)];
        // A few frames to lay the screen out (the rows learn their size).
        for _ in 0..3 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 720.0))), ..Default::default() };
            let _ = ctx.run(raw, |c| {
                world_select(c, &icons, &mut st, &[], "0.3.0");
            });
        }
        let want = MenuAction::JoinServer("10.8.181.7:25570".into());
        type Dbg = (Rect, bool, bool, bool, bool);
        let play: Option<Dbg> = ctx.data(|d| d.get_temp(egui::Id::new("dbg_play")));
        let row: Option<Dbg> = ctx.data(|d| d.get_temp(egui::Id::new("dbg_row")));
        println!("play button: {play:?}\nrow: {row:?}");
        if let Some((r, ..)) = play {
            // Press frame and release frame, reporting what the button saw each time.
            unsafe { std::env::set_var("TAP_TRACE", "1") };
            let a = tap(&ctx, &icons, &mut st, r.center());
            unsafe { std::env::remove_var("TAP_TRACE") };
            let after: Option<Dbg> = ctx.data(|d| d.get_temp(egui::Id::new("dbg_play")));
            println!("tap at play centre {:?} -> {a:?}; button now {after:?}", r.center());
            assert_eq!(a, want, "tap in the middle of «Играть»");
        }
        // And a tap on the row itself (on the server's name, left side).
        if let Some((r, ..)) = row {
            let a = tap(&ctx, &icons, &mut st, Pos2::new(r.left() + 120.0, r.center().y));
            assert_eq!(a, want, "tap on the server row");
        }
        assert!(play.is_some() && row.is_some(), "the nearby server row is drawn");
    }

    #[test]
    fn all_screens_render_in_both_themes() {
        let (ctx, icons) = setup();
        let worlds = vec![WorldEntry { cfg: WorldConfig::new("Тест", 5), preview: None }];
        for mode in [ThemeMode::Dark, ThemeMode::Light] {
            crate::theme_manager::apply(&ctx, mode);
            frame(&ctx, |c| {
                main_menu(c, &icons, "0.1.0", "C:\\data", mode == ThemeMode::Dark, None, 0, "Игрок", &[]);
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
    fn portrait_create_world_has_top_tabs_and_visible_working_create_button() {
        let (ctx, icons) = setup();
        let mut st = CreateWorldState::default();
        st.name = "Мобильный мир".into();
        let render = |st: &mut CreateWorldState, events| {
            let mut action = MenuAction::None;
            let out = ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 844.0))),
                events, ..Default::default()
            }, |ctx| { action = create_world(ctx, &icons, st, None, "0.3.0", 42); });
            (out, action)
        };
        render(&mut st, vec![]);
        let (out, _) = render(&mut st, vec![]);
        let text = |label: &str, below: f32| out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text == label && t.pos.y > below => Some(t.galley.rect.translate(t.pos.to_vec2())), _ => None
        }).expect(label);
        let base = text("Основное", 0.0);
        let planet = text("Планета", 0.0);
        assert!((base.center().y - planet.center().y).abs() < 2.0);
        let create = text("Создать мир", 700.0);
        assert!(create.right() <= 390.0 && create.bottom() < 844.0);
        let at = create.center();
        render(&mut st, vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        let (_, action) = render(&mut st, vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        assert!(matches!(action, MenuAction::CreateWorld(ref cfg) if cfg.name == "Мобильный мир"));
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


