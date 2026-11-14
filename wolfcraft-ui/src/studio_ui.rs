//! WolfCraft Studio: edit a server (place) like in Roblox Studio.
//!
//! * A menu row of tabs (Главная, Модель, Кисть, Тест, Вид) over a ribbon of icon tools.
//! * Docked windows in resizable columns (split screen): Тулбокс on the left (blocks, items,
//!   creatures, tools — everything of the game), Проводник (the place as a tree) and Свойства
//!   (what is selected in it) on the right, Вывод (output of scripts and commands) and the
//!   code window (the place's Luau script) in the middle.
//! * Кисть: Blender-style sculpting on blocks (add, erase, paint, raise, smooth, flatten) with
//!   a size and a ball or cube shape; Модель: WorldEdit tools on a selection.
//! * ▶ Играть hides the studio and plays the place; ■ Стоп comes back.
//!
//! The middle stays the live world, built with the usual touch controls.

use crate::chat_ui::ChatKind;
use crate::gui_renderer::GuiIcons;
use crate::theme_manager::{Palette, bold_font, palette};
use egui::{Align2, Color32, FontId, Frame, Id, Margin, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use wolfcraft_assets::gui as icon;
use wolfcraft_engine::inventory::Inventory;
use wolfcraft_engine::items::{self, ItemStack};

pub const MENU_H: f32 = 26.0;
pub const RIBBON_H: f32 = 54.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    Home,
    Model,
    Brush,
    Test,
    View,
}

/// What fills the middle: the world, or the code window over it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Center {
    #[default]
    World,
    Script,
}

/// A node of the Проводник.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Node {
    #[default]
    Workspace,
    Players,
    Player(usize),
    Mobs,
    Mob(usize),
    Drops,
    Lighting,
    Scripts,
    Script,
    /// The block last tapped in the world.
    Target,
}

/// Names of the brush operations, in the order of the game's `BrushOp::ALL`.
pub const BRUSH_OPS: [&str; 6] = ["Добавить", "Стереть", "Покрасить", "Поднять", "Сгладить", "Выровнять"];

pub struct StudioState {
    pub tab: Tab,
    pub center: Center,
    pub show_toolbox: bool,
    pub show_explorer: bool,
    pub show_props: bool,
    pub show_output: bool,
    pub search: String,
    pub toolbox_cat: usize,
    /// The place's script (Luau).
    pub script: String,
    /// Short status line in the ribbon ("Сохранено", ...).
    pub status: String,
    pub status_t: f64,
    /// Column widths, output height and the Проводник/Свойства split (fraction).
    pub left_w: f32,
    pub right_w: f32,
    pub out_h: f32,
    pub split: f32,
    pub selected: Node,
    /// Brush mode: taps and holds on the world sculpt instead of placing and breaking.
    pub brush_on: bool,
    pub brush_op: usize,
    pub brush_r: i32,
    pub brush_cube: bool,
    /// Play-testing: the studio is hidden, only a stop button stays.
    pub playtest: bool,
    /// Kept for the old two-panel layout's settings (assets/script toggles).
    pub assets_open: bool,
    pub script_open: bool,
    /// Portrait layout: the docked window shown under the world, and how much of the
    /// screen below the ribbon the world takes (0..1).
    pub dock_tab: usize,
    pub dock_open: bool,
    pub dock_frac: f32,
    /// Bottom of the world view (points), for the touch controls (portrait).
    pub view_bottom: Option<f32>,
    /// Right edge of the world view (points) when the dock is at the side (landscape).
    pub view_right: Option<f32>,
}

impl Default for StudioState {
    fn default() -> Self {
        Self {
            tab: Tab::Home,
            center: Center::World,
            show_toolbox: true,
            show_explorer: true,
            show_props: true,
            show_output: false,
            search: String::new(),
            toolbox_cat: 0,
            script: String::new(),
            status: String::new(),
            status_t: 0.0,
            left_w: 250.0,
            right_w: 240.0,
            out_h: 120.0,
            split: 0.5,
            selected: Node::Workspace,
            brush_on: false,
            brush_op: 0,
            brush_r: 2,
            brush_cube: false,
            playtest: false,
            assets_open: true,
            script_open: false,
            dock_tab: 0,
            // Every window starts closed: the whole screen is the world, pills open them.
            dock_open: false,
            dock_frac: 0.5,
            view_bottom: None,
        }
    }
}

/// What the studio shows about the running place (filled by the game each frame).
#[derive(Default, Clone)]
pub struct StudioView {
    pub project: String,
    pub seed: i64,
    pub creative: bool,
    pub clock: String,
    pub date: String,
    /// Hour of the day 0..24.
    pub hour: f32,
    pub spawning: bool,
    /// (name, position, health)
    pub players: Vec<(String, [f64; 3], f32)>,
    /// (name, hp, max hp, position)
    pub mobs: Vec<(String, f32, f32, [f64; 3])>,
    pub drops: usize,
    pub output: Vec<(ChatKind, String)>,
    /// Block id in the hand (for brush and model tools) and its name.
    pub held_block: Option<(u16, String)>,
    pub selection: (Option<[i32; 3]>, Option<[i32; 3]>),
    /// The block last tapped or aimed at: (block id, name, cell).
    pub target: Option<(u16, String, [i32; 3])>,
    pub script_running: bool,
    /// Phone layout: columns end above the touch controls.
    pub compact: bool,
    pub now: f64,
}

#[derive(Clone, PartialEq, Debug)]
pub enum StudioAction {
    /// Save the world and the script, leave to the launcher.
    Exit,
    /// Run the place's script now (fresh).
    RunScript,
    StopScript,
    /// Save the world and the script.
    Save,
    /// Save, then start a server with this world so friends can join.
    Publish,
    /// Toggle day and night.
    DayNight,
    /// A chat command (WorldEdit tools, settings…).
    Command(String),
    SetHour(f32),
    SetCreative(bool),
    SetSpawning(bool),
    RemoveMob(usize),
    GoTo([f64; 3]),
    /// The selected block becomes corner 1 or 2 of the model selection.
    TargetCorner(u8),
    /// The selected block's kind into the hand.
    PickTarget,
    /// Remove the selected block.
    RemoveTarget,
    /// Start or stop play-testing.
    Playtest(bool),
}

/// Commands of a script: trimmed lines without comments, each starting with "/".
pub fn script_commands(script: &str) -> Vec<String> {
    script
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| if l.starts_with('/') { l.to_string() } else { format!("/{l}") })
        .collect()
}

/// The ribbon-and-dock layout in landscape too (the older side-windows layout stays below).
const DOCK_LAYOUT: bool = true;

const CATS: [&str; 12] = ["Шаблоны", "Все", "Природа", "Дерево", "Строительство", "Руды", "Растения", "Предметы", "Инструменты", "Вооружение", "Доспехи", "Мобы"];

/// Draws the studio over the world. Returns the actions and the screen areas (points) that
/// its windows cover, so touches there do not steer the game.
pub fn draw(ctx: &egui::Context, icons: &GuiIcons, st: &mut StudioState, view: &StudioView, inv: &mut Inventory) -> (Vec<StudioAction>, Vec<Rect>) {
    let p = palette(ctx);
    let screen = ctx.screen_rect();
    let mut actions = Vec::new();
    let mut covered = Vec::new();

    if st.playtest {
        // Play-testing: only a small bar with the stop button.
        let r = Rect::from_center_size(Pos2::new(screen.center().x, screen.top() + 24.0), Vec2::new(230.0, 38.0));
        covered.push(r);
        egui::Area::new(Id::new("studio-playtest")).order(egui::Order::Foreground).fixed_pos(r.min).show(ctx, |ui| {
            Frame::none().fill(p.surface).rounding(Rounding::same(19.0)).stroke(Stroke::new(1.5_f32, p.accent)).inner_margin(Margin::symmetric(10.0, 4.0)).show(ui, |ui| {
                ui.set_width(r.width() - 20.0);
                ui.horizontal_centered(|ui| {
                    ui.label(egui::RichText::new("▶ Игра идёт").font(bold_font(14.0)).color(p.success));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::ui_components::pill_button(ui, icons, Some(icon::STOP), "Стоп", crate::ui_components::ButtonKind::Danger, 96.0, true).clicked() {
                            actions.push(StudioAction::Playtest(false));
                        }
                    });
                });
            });
        });
        return (actions, covered);
    }

    // One layout for phones in both orientations: ribbon, window pills, and a dock (under
    // the world when upright, at the right side in landscape).
    if DOCK_LAYOUT || screen.height() > screen.width() {
        return draw_portrait(ctx, icons, st, view, inv, &p, actions, covered);
    }
    st.view_bottom = None;
    // ---- Menu row (tabs) and ribbon ----
    let top_bar = Rect::from_min_size(screen.min, Vec2::new(screen.width(), MENU_H + RIBBON_H));
    covered.push(top_bar);
    egui::Area::new(Id::new("studio-top")).order(egui::Order::Foreground).fixed_pos(top_bar.min).show(ctx, |ui| {
        let painter = ui.painter().clone();
        painter.rect_filled(Rect::from_min_size(top_bar.min, Vec2::new(top_bar.width(), MENU_H)), 0.0, p.bg);
        painter.rect_filled(Rect::from_min_max(Pos2::new(top_bar.left(), top_bar.top() + MENU_H), top_bar.max), 0.0, p.surface);
        painter.line_segment([Pos2::new(top_bar.left(), top_bar.bottom()), top_bar.right_bottom()], Stroke::new(1.0_f32, p.outline));
        ui.set_width(top_bar.width());
        ui.set_height(top_bar.height());
        // Menu row: logo, the tabs, the place name and the play button.
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(Rect::from_min_size(top_bar.min + Vec2::new(8.0, 0.0), Vec2::new(top_bar.width() - 16.0, MENU_H))), |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                icons.paint(ui.painter(), r, icon::HAMMER, p.accent);
                ui.label(egui::RichText::new("Студия").font(bold_font(13.0)).color(p.text));
                ui.add_space(10.0);
                for (t, name) in [(Tab::Home, "Главная"), (Tab::Model, "Модель"), (Tab::Brush, "Кисть"), (Tab::Test, "Тест"), (Tab::View, "Вид")] {
                    let on = st.tab == t;
                    let g = ui.painter().layout_no_wrap(name.to_string(), FontId::proportional(13.0), if on { p.accent } else { p.text_muted });
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 18.0, MENU_H), Sense::click());
                    if on {
                        ui.painter().rect_filled(Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 3.0), r.max), 0.0, p.accent);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, 0.0, p.surface_hover);
                    }
                    ui.painter().galley(Pos2::new(r.left() + 9.0, r.center().y - g.size().y / 2.0), g, p.text);
                    if resp.clicked() {
                        st.tab = t;
                        if t == Tab::Brush {
                            st.brush_on = true;
                        }
                    }
                }
                ui.add_space(10.0);
                ui.label(egui::RichText::new(&view.project).size(12.0).color(p.text_muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !st.status.is_empty() && view.now - st.status_t < 4.0 {
                        ui.label(egui::RichText::new(&st.status).size(12.0).color(p.success));
                    }
                });
            });
        });
        // Ribbon.
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(Rect::from_min_size(top_bar.min + Vec2::new(6.0, MENU_H + 3.0), Vec2::new(top_bar.width() - 12.0, RIBBON_H - 6.0))), |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                ribbon(ui, icons, &p, st, view, &mut actions);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if tool(ui, icons, &p, icon::EXIT, "Выйти", false).clicked() {
                        actions.push(StudioAction::Exit);
                    }
                    if tool(ui, icons, &p, icon::GLOBE, "Сервер", false).on_hover_text("Сохранить и запустить этот сервер: друзья смогут зайти").clicked() {
                        actions.push(StudioAction::Publish);
                    }
                    if tool(ui, icons, &p, icon::DOWNLOAD, "Сохранить", false).clicked() {
                        actions.push(StudioAction::Save);
                    }
                    if play_button(ui, &p).clicked() {
                        actions.push(StudioAction::Playtest(true));
                    }
                });
            });
        });
    });

    // ---- Layout: columns (split screen) ----
    let body_top = top_bar.bottom() + 6.0;
    // On phones the columns stop above the touch controls (joystick, jump).
    let body_bottom = if view.compact { screen.top() + (screen.height() * 0.66).max(body_top - screen.top() + 140.0) } else { screen.bottom() - 6.0 };
    let max_side = (screen.width() * 0.34).max(160.0);
    st.left_w = st.left_w.clamp(160.0, max_side);
    st.right_w = st.right_w.clamp(160.0, max_side);
    let left = st.show_toolbox.then(|| Rect::from_min_max(Pos2::new(screen.left() + 6.0, body_top), Pos2::new(screen.left() + 6.0 + st.left_w, body_bottom)));
    let right_on = st.show_explorer || st.show_props;
    let right = right_on.then(|| Rect::from_min_max(Pos2::new(screen.right() - 6.0 - st.right_w, body_top), Pos2::new(screen.right() - 6.0, body_bottom)));
    let mid_l = left.map(|r| r.right() + 6.0).unwrap_or(screen.left() + 6.0);
    let mid_r = right.map(|r| r.left() - 6.0).unwrap_or(screen.right() - 6.0);

    // ---- Тулбокс ----
    if let Some(rect) = left {
        covered.push(rect);
        window(ctx, "studio-toolbox", rect, &p, icons, icon::CUBE, "Тулбокс", |ui| toolbox(ui, icons, &p, st, inv, &mut actions));
        if let Some(dx) = splitter(ctx, "studio-split-left", Rect::from_min_max(Pos2::new(rect.right() - 2.0, rect.top() + 30.0), Pos2::new(rect.right() + 8.0, rect.bottom())), true, &p) {
            st.left_w += dx;
        }
    }

    // ---- Проводник + Свойства (right column, split) ----
    if let Some(rect) = right {
        covered.push(rect);
        let both = st.show_explorer && st.show_props;
        st.split = st.split.clamp(0.2, 0.8);
        let mid = if both { rect.top() + rect.height() * st.split } else if st.show_explorer { rect.bottom() } else { rect.top() };
        if st.show_explorer {
            let r = Rect::from_min_max(rect.min, Pos2::new(rect.right(), if both { mid - 3.0 } else { rect.bottom() }));
            window(ctx, "studio-explorer", r, &p, icons, icon::TREE, "Проводник", |ui| explorer(ui, &p, st, view));
        }
        if st.show_props {
            let r = Rect::from_min_max(Pos2::new(rect.left(), if both { mid + 3.0 } else { rect.top() }), rect.max);
            window(ctx, "studio-props", r, &p, icons, icon::PROPS, "Свойства", |ui| properties(ui, icons, &p, st, view, &mut actions));
        }
        if both {
            if let Some(dy) = splitter(ctx, "studio-split-right", Rect::from_min_max(Pos2::new(rect.left(), mid - 5.0), Pos2::new(rect.right(), mid + 5.0)), false, &p) {
                st.split += dy / rect.height().max(1.0);
            }
        }
        if let Some(dx) = splitter(ctx, "studio-split-rightw", Rect::from_min_max(Pos2::new(rect.left() - 8.0, rect.top() + 30.0), Pos2::new(rect.left() + 2.0, rect.bottom())), true, &p) {
            st.right_w -= dx;
        }
    }

    // ---- Code window (middle) ----
    let mut mid_top = body_top;
    if st.center == Center::Script && mid_r - mid_l > 160.0 {
        let r = Rect::from_min_max(Pos2::new(mid_l, body_top), Pos2::new(mid_r, body_bottom.max(body_top + 160.0)));
        covered.push(r);
        window(ctx, "studio-code", r, &p, icons, icon::CODE, "Скрипт · script.lua (Luau)", |ui| code_window(ui, icons, &p, st, view, &mut actions));
        mid_top = r.bottom() + 6.0;
    }

    // ---- Вывод ----
    if st.show_output && mid_r - mid_l > 160.0 {
        st.out_h = st.out_h.clamp(70.0, (screen.height() * 0.5).max(80.0));
        let y0 = if st.center == Center::Script { mid_top } else { body_top };
        let r = Rect::from_min_size(Pos2::new(mid_l, y0), Vec2::new(mid_r - mid_l, st.out_h));
        covered.push(r);
        window(ctx, "studio-output", r, &p, icons, icon::OUTPUT, "Вывод", |ui| output(ui, &p, view));
        if let Some(dy) = splitter(ctx, "studio-split-out", Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 2.0), Pos2::new(r.right(), r.bottom() + 8.0)), false, &p) {
            st.out_h += dy;
        }
    }

    // Brush mode hint over the world.
    if st.brush_on && st.center == Center::World {
        let txt = format!("Кисть: {} · размер {} · {} — тапните или держите на мире", BRUSH_OPS[st.brush_op.min(5)], st.brush_r, if st.brush_cube { "куб" } else { "шар" });
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("studio-brush-hint")));
        let g = painter.layout_no_wrap(txt, FontId::proportional(13.0), Color32::WHITE);
        let r = Rect::from_center_size(Pos2::new((mid_l + mid_r) / 2.0, body_top + 18.0 + if st.show_output { st.out_h } else { 0.0 }), g.size() + Vec2::new(20.0, 10.0));
        painter.rect_filled(r, Rounding::same(12.0), Color32::from_rgba_unmultiplied(20, 22, 30, 200));
        painter.galley(r.min + Vec2::new(10.0, 5.0), g, Color32::WHITE);
    }
    (actions, covered)
}

const DOCK_TABS: [(&str, u16); 5] = [("Тулбокс", icon::CUBE), ("Проводник", icon::TREE), ("Свойства", icon::PROPS), ("Вывод", icon::OUTPUT), ("Код", icon::CODE)];

/// Portrait studio (phone held upright): tabs and a sideways-scrolling ribbon on top, the
/// live world in the middle (with the touch controls), and a dock of windows below with
/// tabs; the border between world and dock can be dragged.
#[allow(clippy::too_many_arguments)]
fn draw_portrait(ctx: &egui::Context, icons: &GuiIcons, st: &mut StudioState, view: &StudioView, inv: &mut Inventory, p: &Palette, mut actions: Vec<StudioAction>, mut covered: Vec<Rect>) -> (Vec<StudioAction>, Vec<Rect>) {
    let screen = ctx.screen_rect();
    const PILLS_H: f32 = 38.0;
    let top_bar = Rect::from_min_size(screen.min, Vec2::new(screen.width(), MENU_H + RIBBON_H + PILLS_H));
    covered.push(top_bar);
    egui::Area::new(Id::new("studio-top-p")).order(egui::Order::Foreground).fixed_pos(top_bar.min).show(ctx, |ui| {
        let painter = ui.painter().clone();
        painter.rect_filled(Rect::from_min_size(top_bar.min, Vec2::new(top_bar.width(), MENU_H)), 0.0, p.bg);
        painter.rect_filled(Rect::from_min_max(Pos2::new(top_bar.left(), top_bar.top() + MENU_H), top_bar.max), 0.0, p.surface);
        ui.set_width(top_bar.width());
        ui.set_height(top_bar.height());
        // Tabs row with the play button on the right.
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(Rect::from_min_size(top_bar.min + Vec2::new(4.0, 0.0), Vec2::new(top_bar.width() - 8.0, MENU_H))), |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (t, name) in [(Tab::Home, "Главная"), (Tab::Model, "Модель"), (Tab::Brush, "Кисть"), (Tab::Test, "Тест"), (Tab::View, "Вид")] {
                    let on = st.tab == t;
                    let g = ui.painter().layout_no_wrap(name.to_string(), FontId::proportional(12.5), if on { p.accent } else { p.text_muted });
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 12.0, MENU_H), Sense::click());
                    if on {
                        ui.painter().rect_filled(Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 3.0), r.max), 0.0, p.accent);
                    }
                    ui.painter().galley(Pos2::new(r.left() + 6.0, r.center().y - g.size().y / 2.0), g, p.text);
                    if resp.clicked() {
                        st.tab = t;
                        if t == Tab::Brush {
                            st.brush_on = true;
                        }
                    }
                }
                // Always at hand on the right: exit, publish (run as a server), save.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (ic, hint, act) in [(icon::EXIT, "Выйти (сохранит)", StudioAction::Exit), (icon::GLOBE, "Запустить сервер", StudioAction::Publish), (icon::DOWNLOAD, "Сохранить", StudioAction::Save)] {
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(30.0, MENU_H), Sense::click());
                        if resp.hovered() || resp.is_pointer_button_down_on() {
                            ui.painter().rect_filled(r.shrink(2.0), Rounding::same(6.0), p.surface_hover);
                        }
                        icons.paint(ui.painter(), Rect::from_center_size(r.center(), Vec2::splat(16.0)), ic, p.text);
                        if resp.on_hover_text(hint).clicked() {
                            actions.push(act);
                        }
                    }
                });
            });
        });
        // Window pills under the ribbon: a tap opens that window in the dock below, a tap
        // on the open one hides the dock (the whole screen is the world then).
        let pills = Rect::from_min_size(top_bar.min + Vec2::new(0.0, MENU_H + RIBBON_H), Vec2::new(top_bar.width(), PILLS_H));
        painter.rect_filled(pills, 0.0, p.bg);
        painter.line_segment([pills.left_bottom(), pills.right_bottom()], Stroke::new(1.0_f32, p.outline));
        let gap = 5.0;
        let pw = (pills.width() - gap * (DOCK_TABS.len() as f32 + 1.0)) / DOCK_TABS.len() as f32;
        for (i, (name, ic)) in DOCK_TABS.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(pills.left() + gap + (pw + gap) * i as f32, pills.top() + 5.0), Vec2::new(pw, PILLS_H - 10.0));
            let resp = ui.interact(r, Id::new(("dock-pill", i)), Sense::click());
            let on = st.dock_open && st.dock_tab == i;
            let round = Rounding::same(r.height() / 2.0);
            ui.painter().rect_filled(r, round, if on { p.accent } else if resp.hovered() { p.surface_hover } else { p.surface });
            if !on {
                ui.painter().rect_stroke(r, round, Stroke::new(1.0_f32, p.outline));
            }
            let col = if on { p.on_accent } else { p.text };
            let g = ui.painter().layout_no_wrap(name.to_string(), FontId::proportional(10.5), col);
            let total = 13.0 + 3.0 + g.size().x;
            let x0 = r.center().x - total / 2.0;
            icons.paint(ui.painter(), Rect::from_center_size(Pos2::new(x0 + 6.5, r.center().y), Vec2::splat(13.0)), *ic, col);
            ui.painter().galley(Pos2::new(x0 + 16.0, r.center().y - g.size().y / 2.0), g, col);
            if resp.clicked() {
                if on {
                    st.dock_open = false;
                } else {
                    st.dock_tab = i;
                    st.dock_open = true;
                }
            }
        }
        // Ribbon: scrolls sideways; save/server/exit at its end, ▶ first.
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(Rect::from_min_size(top_bar.min + Vec2::new(4.0, MENU_H + 3.0), Vec2::new(top_bar.width() - 8.0, RIBBON_H - 6.0))), |ui| {
            egui::ScrollArea::horizontal().id_salt("studio-ribbon-p").drag_to_scroll(true).show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    if play_button(ui, p).clicked() {
                        actions.push(StudioAction::Playtest(true));
                    }
                    sep(ui, p);
                    ribbon(ui, icons, p, st, view, &mut actions);
                    sep(ui, p);
                    if tool(ui, icons, p, icon::DOWNLOAD, "Сохранить", false).clicked() {
                        actions.push(StudioAction::Save);
                    }
                    if tool(ui, icons, p, icon::GLOBE, "Сервер", false).clicked() {
                        actions.push(StudioAction::Publish);
                    }
                    if tool(ui, icons, p, icon::EXIT, "Выйти", false).clicked() {
                        actions.push(StudioAction::Exit);
                    }
                });
            });
        });
    });

    // The ribbon's "Скрипт" opens the code tab of the dock here.
    if st.center == Center::Script {
        st.center = Center::World;
        st.dock_tab = 4;
        st.dock_open = true;
    }
    if !st.dock_open {
        st.view_bottom = None;
        st.view_right = None;
        return (actions, covered);
    }
    // World and dock: under the world when upright, beside it on the right in landscape.
    let landscape = screen.width() > screen.height();
    let dock = if landscape {
        let avail = screen.width();
        st.dock_frac = st.dock_frac.clamp(0.35, 0.75);
        let split_x = screen.left() + avail * st.dock_frac;
        st.view_bottom = None;
        st.view_right = Some(split_x);
        if let Some(dx) = splitter(ctx, "studio-split-dock-x", Rect::from_min_max(Pos2::new(split_x - 7.0, top_bar.bottom()), Pos2::new(split_x + 7.0, screen.bottom())), true, p) {
            st.dock_frac += dx / avail.max(1.0);
        }
        Rect::from_min_max(Pos2::new(split_x, top_bar.bottom()), screen.max)
    } else {
        let avail = screen.bottom() - top_bar.bottom();
        st.dock_frac = st.dock_frac.clamp(0.25, 0.8);
        let split_y = top_bar.bottom() + avail * st.dock_frac;
        st.view_bottom = Some(split_y);
        st.view_right = None;
        if let Some(dy) = splitter(ctx, "studio-split-dock", Rect::from_min_max(Pos2::new(screen.left(), split_y - 7.0), Pos2::new(screen.right(), split_y + 7.0)), false, p) {
            st.dock_frac += dy / avail.max(1.0);
        }
        Rect::from_min_max(Pos2::new(screen.left(), split_y), screen.max)
    };
    covered.push(dock);
    egui::Area::new(Id::new("studio-dock")).order(egui::Order::Foreground).fixed_pos(dock.min).show(ctx, |ui| {
        Frame::none().fill(p.surface).stroke(Stroke::new(1.0_f32, p.outline)).show(ui, |ui| {
            ui.set_width(dock.width());
            ui.set_height(dock.height());
            ui.set_max_height(dock.height());
            // A grip line on top: drag it to resize.
            let (grip, _) = ui.allocate_exact_size(Vec2::new(dock.width(), 10.0), Sense::hover());
            ui.painter().rect_filled(Rect::from_center_size(grip.center(), Vec2::new(44.0, 4.0)), Rounding::same(2.0), p.outline);
            Frame::none().inner_margin(Margin::same(6.0)).show(ui, |ui| {
                let h = dock.height() - 10.0 - 12.0;
                ui.set_width(dock.width() - 12.0);
                ui.set_height(h);
                ui.set_max_height(h);
                ui.vertical(|ui| match st.dock_tab {
                    0 => toolbox(ui, icons, p, st, inv, &mut actions),
                    1 => explorer(ui, p, st, view),
                    2 => properties(ui, icons, p, st, view, &mut actions),
                    3 => output(ui, p, view),
                    _ => code_window(ui, icons, p, st, view, &mut actions),
                });
            });
        });
    });
    // Selecting in the Проводник shows its Свойства next time; brush hint over the world.
    if st.brush_on {
        let txt = format!("Кисть: {} · {} · {}", BRUSH_OPS[st.brush_op.min(5)], st.brush_r, if st.brush_cube { "куб" } else { "шар" });
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("studio-brush-hint")));
        let g = painter.layout_no_wrap(txt, FontId::proportional(12.5), Color32::WHITE);
        let r = Rect::from_center_size(Pos2::new(screen.center().x, top_bar.bottom() + 16.0), g.size() + Vec2::new(18.0, 8.0));
        painter.rect_filled(r, Rounding::same(11.0), Color32::from_rgba_unmultiplied(20, 22, 30, 200));
        painter.galley(r.min + Vec2::new(9.0, 4.0), g, Color32::WHITE);
    }
    (actions, covered)
}

/// The ribbon's tools for the current tab.
fn ribbon(ui: &mut Ui, icons: &GuiIcons, p: &Palette, st: &mut StudioState, view: &StudioView, actions: &mut Vec<StudioAction>) {
    let block = view.held_block.as_ref().map(|b| b.0);
    let need_block = |actions: &mut Vec<StudioAction>, cmd: &str| match block {
        Some(id) => actions.push(StudioAction::Command(format!("//{cmd} {id}"))),
        None => actions.push(StudioAction::Command("/say Возьмите блок из Тулбокса в руку".into())),
    };
    match st.tab {
        Tab::Home => {
            if tool(ui, icons, p, icon::SELECT, "Выбор", !st.brush_on).clicked() {
                st.brush_on = false;
            }
            if tool(ui, icons, p, icon::BRUSH, "Кисть", st.brush_on).clicked() {
                st.brush_on = !st.brush_on;
                if st.brush_on {
                    st.tab = Tab::Brush;
                }
            }
            sep(ui, p);
            if tool(ui, icons, p, icon::CUBE, "Тулбокс", st.dock_open && st.dock_tab == 0).clicked() {
                toggle_dock(st, 0);
            }
            if tool(ui, icons, p, icon::CODE, "Скрипт", st.dock_open && st.dock_tab == 4).clicked() {
                toggle_dock(st, 4);
            }
            if tool(ui, icons, p, icon::UNDO, "Отменить", false).clicked() {
                actions.push(StudioAction::Command("//undo".into()));
            }
            if tool(ui, icons, p, icon::REDO, "Повторить", false).clicked() {
                actions.push(StudioAction::Command("//redo".into()));
            }
        }
        Tab::Model => {
            if tool(ui, icons, p, icon::CROSSHAIR, "Точка 1", view.selection.0.is_some()).clicked() {
                actions.push(StudioAction::Command("//hpos1".into()));
            }
            if tool(ui, icons, p, icon::CROSSHAIR, "Точка 2", view.selection.1.is_some()).clicked() {
                actions.push(StudioAction::Command("//hpos2".into()));
            }
            sep(ui, p);
            if tool(ui, icons, p, icon::FILL, "Заполнить", false).clicked() {
                need_block(actions, "set");
            }
            if tool(ui, icons, p, icon::CUBE, "Стены", false).clicked() {
                need_block(actions, "walls");
            }
            if tool(ui, icons, p, icon::CUBE, "Контур", false).clicked() {
                need_block(actions, "outline");
            }
            if tool(ui, icons, p, icon::TRASH, "Очистить", false).clicked() {
                actions.push(StudioAction::Command("//set air".into()));
            }
            sep(ui, p);
            if tool(ui, icons, p, icon::COPY, "Копия", false).clicked() {
                actions.push(StudioAction::Command("//copy".into()));
            }
            if tool(ui, icons, p, icon::CLOSE, "Вырезать", false).clicked() {
                actions.push(StudioAction::Command("//cut".into()));
            }
            if tool(ui, icons, p, icon::PASTE, "Вставить", false).clicked() {
                actions.push(StudioAction::Command("//paste".into()));
            }
            sep(ui, p);
            if tool(ui, icons, p, icon::UNDO, "Отменить", false).clicked() {
                actions.push(StudioAction::Command("//undo".into()));
            }
            if tool(ui, icons, p, icon::REDO, "Повторить", false).clicked() {
                actions.push(StudioAction::Command("//redo".into()));
            }
        }
        Tab::Brush => {
            if tool(ui, icons, p, icon::BRUSH, if st.brush_on { "Вкл" } else { "Выкл" }, st.brush_on).clicked() {
                st.brush_on = !st.brush_on;
            }
            sep(ui, p);
            for (i, name) in BRUSH_OPS.iter().enumerate() {
                let ic = [icon::PLUS, icon::TRASH, icon::BRUSH, icon::CHEVRON_DOWN, icon::REFRESH, icon::MINIMIZE][i];
                if tool(ui, icons, p, ic, name, st.brush_on && st.brush_op == i).clicked() {
                    st.brush_op = i;
                    st.brush_on = true;
                }
            }
            sep(ui, p);
            if tool(ui, icons, p, icon::MINIMIZE, "Меньше", false).clicked() {
                st.brush_r = (st.brush_r - 1).max(1);
            }
            ui.label(egui::RichText::new(format!("{}", st.brush_r)).font(bold_font(16.0)).color(p.text));
            if tool(ui, icons, p, icon::PLUS, "Больше", false).clicked() {
                st.brush_r = (st.brush_r + 1).min(12);
            }
            if tool(ui, icons, p, icon::CUBE, if st.brush_cube { "Куб" } else { "Шар" }, false).clicked() {
                st.brush_cube = !st.brush_cube;
            }
            if tool(ui, icons, p, icon::UNDO, "Отменить", false).clicked() {
                actions.push(StudioAction::Command("//undo".into()));
            }
        }
        Tab::Test => {
            if tool(ui, icons, p, icon::PLAY, "Играть", false).clicked() {
                actions.push(StudioAction::Playtest(true));
            }
            sep(ui, p);
            if tool(ui, icons, p, icon::CODE, "Запуск", view.script_running).clicked() {
                actions.push(StudioAction::RunScript);
            }
            if tool(ui, icons, p, icon::STOP, "Стоп", false).clicked() {
                actions.push(StudioAction::StopScript);
            }
            if tool(ui, icons, p, icon::SUN, "День/ночь", false).clicked() {
                actions.push(StudioAction::DayNight);
            }
        }
        Tab::View => {
            // The windows of the dock (same as the pills under the ribbon).
            for (i, (name, ic)) in DOCK_TABS.iter().enumerate() {
                if tool(ui, icons, p, *ic, name, st.dock_open && st.dock_tab == i).clicked() {
                    toggle_dock(st, i);
                }
            }
            if tool(ui, icons, p, icon::CLOSE, "Скрыть всё", false).clicked() {
                st.dock_open = false;
            }
        }
    }
}

/// Opens window `tab` in the dock, or hides the dock if that window is already shown.
fn toggle_dock(st: &mut StudioState, tab: usize) {
    if st.dock_open && st.dock_tab == tab {
        st.dock_open = false;
    } else {
        st.dock_tab = tab;
        st.dock_open = true;
    }
}

/// Ready-made server mini-games: (name, description, command that builds it in front of you).
const TEMPLATES: [(&str, &str, &str); 2] = [
    ("Питомник", "Монетные блоки, инкубаторы, капсулы-яйца и питомцы, которые ходят за игроком", "/питомник"),
    ("Симулятор кнопок", "Деньги, множитель, перерождения и кнопки на полу", "/кнопки"),
];

fn toolbox(ui: &mut Ui, icons: &GuiIcons, p: &Palette, st: &mut StudioState, inv: &mut Inventory, actions: &mut Vec<StudioAction>) {
    ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("Поиск…").desired_width(f32::INFINITY));
    ui.add_space(3.0);
    // Category chips (blocks, items, creatures, weapons…) in one row that scrolls sideways.
    egui::ScrollArea::horizontal().id_salt("studio-toolbox-cats").max_height(24.0).show(ui, |ui| ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(3.0, 3.0);
        for (i, c) in CATS.iter().enumerate() {
            let on = st.toolbox_cat == i;
            let g = ui.painter().layout_no_wrap(c.to_string(), FontId::proportional(11.5), if on { p.on_accent } else { p.text });
            let (r, resp) = ui.allocate_exact_size(g.size() + Vec2::new(12.0, 6.0), Sense::click());
            ui.painter().rect_filled(r, Rounding::same(9.0), if on { p.accent } else if resp.hovered() { p.surface_hover } else { p.bg });
            ui.painter().galley(r.min + Vec2::new(6.0, 3.0), g, p.text);
            if resp.clicked() {
                st.toolbox_cat = i;
            }
        }
    }));
    ui.add_space(3.0);
    let q = st.search.trim().to_lowercase();
    let cat = CATS[st.toolbox_cat.min(CATS.len() - 1)];
    if cat == "Шаблоны" && q.is_empty() {
        // Templates: a mini-game is built in front of you; their blocks are below.
        egui::ScrollArea::vertical().id_salt("studio-templates-scroll").max_height(ui.available_height()).auto_shrink([false, false]).show(ui, |ui| {
            for (name, desc, cmd) in TEMPLATES {
                let out = Frame::none().fill(p.bg).rounding(Rounding::same(8.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(8.0)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(egui::RichText::new(name).font(bold_font(13.5)).color(p.text));
                    ui.label(egui::RichText::new(desc).size(11.5).color(p.text_muted));
                    ui.label(egui::RichText::new("Нажмите — построится перед вами").size(11.0).color(p.accent));
                });
                if ui.interact(out.response.rect, Id::new(("studio-template", cmd)), Sense::click()).clicked() {
                    actions.push(StudioAction::Command(cmd.to_string()));
                }
                ui.add_space(4.0);
            }
            ui.label(egui::RichText::new("Блоки мини-игр").size(11.5).color(p.text_muted));
            let cell = 42.0;
            let cols = ((ui.available_width() + 3.0) / (cell + 3.0)).floor().max(1.0) as usize;
            for row in items::minigame_items().chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    for id in row {
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(cell), Sense::click());
                        ui.painter().rect_filled(r, Rounding::same(5.0), if resp.hovered() { p.surface_hover } else { p.bg });
                        ui.painter().rect_stroke(r, Rounding::same(5.0), Stroke::new(1.0_f32, p.outline));
                        icons.paint_item(ui.painter(), r.shrink(6.0), *id);
                        if resp.clicked() {
                            inv.slots[inv.selected] = Some(ItemStack::new(*id, items::info(*id).max_stack));
                        }
                        resp.on_hover_text(items::info(*id).name);
                    }
                });
            }
        });
        return;
    }
    let mut list: Vec<u16> = items::creative_palette()
        .into_iter()
        .filter(|id| q.is_empty() || items::info(*id).name.to_lowercase().contains(&q))
        .filter(|id| cat == "Все" || cat == "Шаблоны" || !q.is_empty() || items::category(*id) == cat)
        .collect();
    list.sort_by_key(|id| !items::is_new(*id));
    let cell = 42.0;
    let cols = ((ui.available_width() + 3.0) / (cell + 3.0)).floor().max(1.0) as usize;
    egui::ScrollArea::vertical().id_salt("studio-toolbox-scroll").max_height(ui.available_height()).auto_shrink([false, false]).drag_to_scroll(true).show(ui, |ui| {
        for row in list.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                for id in row {
                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(cell), Sense::click());
                    let held = inv.held().map(|s| s.id) == Some(*id);
                    ui.painter().rect_filled(r, Rounding::same(5.0), if resp.hovered() { p.surface_hover } else { p.bg });
                    ui.painter().rect_stroke(r, Rounding::same(5.0), Stroke::new(if held { 2.0_f32 } else { 1.0_f32 }, if held { p.accent } else { p.outline }));
                    icons.paint_item(ui.painter(), r.shrink(6.0), *id);
                    if resp.clicked() {
                        // Into the hand: the selected hotbar slot gets a full stack.
                        inv.slots[inv.selected] = Some(ItemStack::new(*id, items::info(*id).max_stack));
                    }
                    resp.on_hover_text(items::info(*id).name);
                }
            });
        }
        if list.is_empty() {
            ui.label(egui::RichText::new("Ничего не найдено").color(p.text_muted));
        }
    });
}

fn explorer(ui: &mut Ui, p: &Palette, st: &mut StudioState, view: &StudioView) {
    egui::ScrollArea::vertical().id_salt("studio-explorer-scroll").max_height(ui.available_height()).auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        let mut row = |ui: &mut Ui, depth: usize, node: Node, label: String, color: Color32| {
            let on = st.selected == node;
            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::click());
            if on {
                ui.painter().rect_filled(r, Rounding::same(4.0), p.accent.gamma_multiply(0.25));
            } else if resp.hovered() {
                ui.painter().rect_filled(r, Rounding::same(4.0), p.surface_hover);
            }
            let x = r.left() + 6.0 + depth as f32 * 14.0;
            ui.painter().circle_filled(Pos2::new(x + 4.0, r.center().y), 4.0, color);
            ui.painter().text(Pos2::new(x + 14.0, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(13.0), if on { p.accent } else { p.text });
            if resp.clicked() {
                // On a phone the dock shows one window: the tap goes on to its Свойства
                // (the script opens straight in the code window).
                st.selected = node;
                if node == Node::Script {
                    st.dock_tab = 4;
                } else if !matches!(node, Node::Players | Node::Mobs) {
                    st.dock_tab = 2;
                }
            }
        };
        row(ui, 0, Node::Workspace, format!("Мир «{}»", view.project), Color32::from_rgb(90, 170, 255));
        row(ui, 1, Node::Players, format!("Игроки ({})", view.players.len()), Color32::from_rgb(120, 220, 140));
        for (i, (n, ..)) in view.players.iter().enumerate() {
            row(ui, 2, Node::Player(i), n.clone(), Color32::from_rgb(120, 220, 140));
        }
        row(ui, 1, Node::Mobs, format!("Мобы ({})", view.mobs.len()), Color32::from_rgb(240, 150, 90));
        for (i, (n, hp, max, _)) in view.mobs.iter().enumerate().take(60) {
            row(ui, 2, Node::Mob(i), format!("{n} {:.0}/{:.0}", hp, max), Color32::from_rgb(240, 150, 90));
        }
        row(ui, 1, Node::Drops, format!("Предметы на земле ({})", view.drops), Color32::from_rgb(230, 210, 110));
        let target = view.target.as_ref().map(|t| format!("Выбранный блок: {}", t.1)).unwrap_or_else(|| "Выбранный блок: —".into());
        row(ui, 1, Node::Target, target, Color32::from_rgb(150, 200, 255));
        row(ui, 0, Node::Lighting, "Освещение".into(), Color32::from_rgb(255, 220, 90));
        row(ui, 0, Node::Scripts, "Скрипты".into(), Color32::from_rgb(190, 140, 255));
        row(ui, 1, Node::Script, "script.lua".into(), Color32::from_rgb(190, 140, 255));
    });
}

fn properties(ui: &mut Ui, icons: &GuiIcons, p: &Palette, st: &mut StudioState, view: &StudioView, actions: &mut Vec<StudioAction>) {
    use crate::ui_components::{ButtonKind, pill_button};
    // One property row: name on the left, value on the right (fixed height).
    let prop = |ui: &mut Ui, k: &str, v: String| {
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
        ui.painter().text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, k, FontId::proportional(12.5), p.text_muted);
        ui.painter().text(Pos2::new(r.right() - 2.0, r.center().y), Align2::RIGHT_CENTER, v, FontId::proportional(12.5), p.text);
    };
    let pos = |a: &[f64; 3]| format!("{:.0} {:.0} {:.0}", a[0], a[1], a[2]);
    let w = ui.available_width();
    egui::ScrollArea::vertical().id_salt("studio-props-scroll").max_height(ui.available_height()).auto_shrink([false, false]).show(ui, |ui| match st.selected {
        Node::Workspace => {
            prop(ui, "Имя", view.project.clone());
            prop(ui, "Seed", view.seed.to_string());
            let mut creative = view.creative;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Творческий").size(12.5).color(p.text_muted));
                if ui.checkbox(&mut creative, "").changed() {
                    actions.push(StudioAction::SetCreative(creative));
                }
            });
            let mut spawning = view.spawning;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Мобы появляются").size(12.5).color(p.text_muted));
                if ui.checkbox(&mut spawning, "").changed() {
                    actions.push(StudioAction::SetSpawning(spawning));
                }
            });
        }
        Node::Players | Node::Mobs | Node::Drops | Node::Scripts => {
            ui.label(egui::RichText::new("Выберите элемент в Проводнике").size(12.5).color(p.text_muted));
            if pill_button(ui, icons, Some(icon::TREE), "Проводник", ButtonKind::Secondary, w, true).clicked() {
                st.dock_tab = 1;
            }
        }
        Node::Player(i) => {
            if let Some((n, at, hp)) = view.players.get(i) {
                prop(ui, "Ник", n.clone());
                prop(ui, "Позиция", pos(at));
                prop(ui, "Здоровье", format!("{hp:.0}/20"));
                if pill_button(ui, icons, None, "Вылечить", ButtonKind::Secondary, w, true).clicked() {
                    actions.push(StudioAction::Command("/heal".into()));
                }
                if pill_button(ui, icons, None, "На спавн", ButtonKind::Secondary, w, true).clicked() {
                    actions.push(StudioAction::Command("/spawn".into()));
                }
            }
        }
        Node::Mob(i) => {
            if let Some((n, hp, max, at)) = view.mobs.get(i) {
                prop(ui, "Вид", n.clone());
                prop(ui, "Здоровье", format!("{hp:.0}/{max:.0}"));
                prop(ui, "Позиция", pos(at));
                if pill_button(ui, icons, None, "Телепорт к нему", ButtonKind::Secondary, w, true).clicked() {
                    actions.push(StudioAction::GoTo(*at));
                }
                if pill_button(ui, icons, Some(icon::TRASH), "Удалить", ButtonKind::Danger, w, true).clicked() {
                    actions.push(StudioAction::RemoveMob(i));
                    st.selected = Node::Mobs;
                }
            } else {
                st.selected = Node::Mobs;
            }
        }
        Node::Lighting => {
            prop(ui, "Время", view.clock.clone());
            prop(ui, "Дата", view.date.clone());
            let mut h = view.hour;
            ui.label(egui::RichText::new("Время суток").size(12.5).color(p.text_muted));
            if ui.add(egui::Slider::new(&mut h, 0.0..=23.9).show_value(false)).changed() {
                actions.push(StudioAction::SetHour(h));
            }
            ui.horizontal(|ui| {
                if pill_button(ui, icons, Some(icon::SUN), "День", ButtonKind::Secondary, 0.0, true).clicked() {
                    actions.push(StudioAction::SetHour(12.0));
                }
                if pill_button(ui, icons, Some(icon::MOON), "Ночь", ButtonKind::Secondary, 0.0, true).clicked() {
                    actions.push(StudioAction::SetHour(0.0));
                }
            });
            ui.label(egui::RichText::new("Месяц").size(12.5).color(p.text_muted));
            ui.horizontal_wrapped(|ui| {
                for (m, name) in ["Янв", "Фев", "Мар", "Апр", "Май", "Июн", "Июл", "Авг", "Сен", "Окт", "Ноя", "Дек"].iter().enumerate() {
                    if ui.small_button(*name).clicked() {
                        actions.push(StudioAction::Command(format!("/month {}", m + 1)));
                    }
                }
            });
        }
        Node::Target => match &view.target {
            Some((id, name, at)) => {
                prop(ui, "Блок", name.clone());
                prop(ui, "ID", id.to_string());
                prop(ui, "Ячейка", format!("{} {} {}", at[0], at[1], at[2]));
                ui.horizontal(|ui| {
                    let half = (w - 8.0) / 2.0;
                    if pill_button(ui, icons, Some(icon::CROSSHAIR), "Точка 1", ButtonKind::Secondary, half, true).clicked() {
                        actions.push(StudioAction::TargetCorner(1));
                    }
                    if pill_button(ui, icons, Some(icon::CROSSHAIR), "Точка 2", ButtonKind::Secondary, half, true).clicked() {
                        actions.push(StudioAction::TargetCorner(2));
                    }
                });
                if pill_button(ui, icons, Some(icon::CUBE), "Взять в руку", ButtonKind::Secondary, w, true).clicked() {
                    actions.push(StudioAction::PickTarget);
                }
                if pill_button(ui, icons, Some(icon::TRASH), "Удалить блок", ButtonKind::Danger, w, true).clicked() {
                    actions.push(StudioAction::RemoveTarget);
                }
            }
            None => {
                ui.label(egui::RichText::new("Наведите прицел или нажмите на блок в мире — здесь появятся его свойства").size(12.5).color(p.text_muted));
            }
        },
        Node::Script => {
            prop(ui, "Язык", "Luau".into());
            prop(ui, "Строк", st.script.lines().count().to_string());
            prop(ui, "Работает", if view.script_running { "да" } else { "нет" }.into());
            if pill_button(ui, icons, Some(icon::CODE), "Открыть код", ButtonKind::Primary, w, true).clicked() {
                st.center = Center::Script;
            }
        }
    });
}

fn code_window(ui: &mut Ui, icons: &GuiIcons, p: &Palette, st: &mut StudioState, view: &StudioView, actions: &mut Vec<StudioAction>) {
    use crate::ui_components::{ButtonKind, pill_button};
    ui.horizontal(|ui| {
        if pill_button(ui, icons, Some(icon::PLAY), "Запустить", ButtonKind::Primary, 0.0, true).clicked() {
            actions.push(StudioAction::RunScript);
        }
        if pill_button(ui, icons, Some(icon::STOP), "Стоп", ButtonKind::Secondary, 0.0, view.script_running).clicked() {
            actions.push(StudioAction::StopScript);
        }
        if pill_button(ui, icons, Some(icon::CLOSE), "Закрыть", ButtonKind::Ghost, 0.0, true).clicked() {
            st.center = Center::World;
            st.dock_open = false;
        }
    });
    ui.add_space(4.0);
    let h = ui.available_height();
    egui::ScrollArea::vertical().id_salt("studio-code-scroll").max_height(h).auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            // Line numbers.
            let n = st.script.lines().count().max(1) + usize::from(st.script.ends_with('\n'));
            let nums: String = (1..=n).map(|i| format!("{i}\n")).collect();
            ui.label(egui::RichText::new(nums).font(FontId::monospace(14.0)).color(p.text_muted));
            ui.add_sized(
                Vec2::new(ui.available_width(), h.max(120.0)),
                egui::TextEdit::multiline(&mut st.script)
                    .font(FontId::monospace(14.0))
                    .code_editor()
                    .frame(false)
                    .hint_text("-- Luau\nprint(\"Привет!\")\ngui.button{ id = \"b\", text = \"Жми\", on_click = function() chat(\"Нажали!\") end }")
                    .desired_rows(10),
            );
        });
    });
}

fn output(ui: &mut Ui, p: &Palette, view: &StudioView) {
    egui::ScrollArea::vertical().id_salt("studio-output-scroll").max_height(ui.available_height()).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        if view.output.is_empty() {
            ui.label(egui::RichText::new("Пока пусто: здесь print() скрипта и ответы команд").size(12.5).color(p.text_muted));
        }
        for (k, t) in &view.output {
            let c = match k {
                ChatKind::Error => Color32::from_rgb(255, 110, 100),
                ChatKind::System => Color32::from_rgb(220, 220, 170),
                ChatKind::Player => p.text,
            };
            ui.label(egui::RichText::new(t).font(FontId::monospace(12.5)).color(c));
        }
    });
}

/// A docked window: title bar with an icon, then the content.
fn window(ctx: &egui::Context, id: &str, rect: Rect, p: &Palette, icons: &GuiIcons, ic: u16, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::Area::new(Id::new(id)).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
        Frame::none().fill(p.surface.gamma_multiply(0.97)).rounding(Rounding::same(6.0)).stroke(Stroke::new(1.0_f32, p.outline)).show(ui, |ui| {
            ui.set_width(rect.width());
            ui.set_height(rect.height());
            let (bar, _) = ui.allocate_exact_size(Vec2::new(rect.width(), 24.0), Sense::hover());
            ui.painter().rect_filled(bar, Rounding { nw: 6.0, ne: 6.0, sw: 0.0, se: 0.0 }, p.bg);
            icons.paint(ui.painter(), Rect::from_center_size(Pos2::new(bar.left() + 14.0, bar.center().y), Vec2::splat(14.0)), ic, p.accent);
            ui.painter().text(Pos2::new(bar.left() + 28.0, bar.center().y), Align2::LEFT_CENTER, title, bold_font(12.5), p.text);
            Frame::none().inner_margin(Margin::same(6.0)).show(ui, |ui| {
                ui.set_width(rect.width() - 12.0);
                ui.set_height(rect.height() - 36.0);
                ui.set_max_height(rect.height() - 44.0);
                ui.vertical(|ui| add(ui));
            });
        });
    });
}

/// A drag handle between windows; returns how far it was dragged this frame.
fn splitter(ctx: &egui::Context, id: &str, rect: Rect, vertical: bool, p: &Palette) -> Option<f32> {
    let mut out = None;
    egui::Area::new(Id::new(id)).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(rect.size(), Sense::drag());
        if resp.hovered() || resp.dragged() {
            ui.painter().rect_filled(r, 2.0, p.accent.gamma_multiply(0.6));
            ui.ctx().set_cursor_icon(if vertical { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::ResizeVertical });
        }
        if resp.dragged() {
            let d = resp.drag_delta();
            out = Some(if vertical { d.x } else { d.y });
        }
    });
    out
}

fn sep(ui: &mut Ui, p: &Palette) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(9.0, RIBBON_H - 18.0), Sense::hover());
    ui.painter().line_segment([Pos2::new(r.center().x, r.top()), Pos2::new(r.center().x, r.bottom())], Stroke::new(1.0_f32, p.outline));
}

/// The green ▶ Играть button.
fn play_button(ui: &mut Ui, p: &Palette) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(92.0, RIBBON_H - 14.0), Sense::click());
    let fill = if resp.hovered() || resp.is_pointer_button_down_on() { Color32::from_rgb(70, 200, 110) } else { Color32::from_rgb(52, 170, 90) };
    ui.painter().rect_filled(r, Rounding::same(8.0), fill);
    let c = Pos2::new(r.left() + 22.0, r.center().y);
    ui.painter().add(egui::Shape::convex_polygon(vec![c + Vec2::new(-6.0, -8.0), c + Vec2::new(8.0, 0.0), c + Vec2::new(-6.0, 8.0)], Color32::WHITE, Stroke::NONE));
    ui.painter().text(Pos2::new(r.left() + 58.0, r.center().y), Align2::CENTER_CENTER, "Играть", bold_font(14.0), Color32::WHITE);
    let _ = p;
    resp
}

/// Ribbon button: icon over a short caption.
fn tool(ui: &mut Ui, icons: &GuiIcons, p: &Palette, ic: u16, label: &str, on: bool) -> egui::Response {
    let w = 12.0 + label.chars().count() as f32 * 6.8;
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w.max(46.0), RIBBON_H - 10.0), Sense::click());
    let fill = if on {
        p.accent.gamma_multiply(0.22)
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        p.surface_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(r, Rounding::same(6.0), fill);
    let col = if on { p.accent } else { p.text };
    icons.paint(ui.painter(), Rect::from_center_size(Pos2::new(r.center().x, r.top() + 14.0), Vec2::splat(18.0)), ic, col);
    ui.painter().text(Pos2::new(r.center().x, r.bottom() - 8.0), Align2::CENTER_CENTER, label, FontId::proportional(10.5), col);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_lines_become_commands() {
        let s = "# комментарий\n/time set день\n\n give факел 4 \nsay привет";
        assert_eq!(script_commands(s), vec!["/time set день", "/give факел 4", "/say привет"]);
    }

    #[test]
    fn studio_draws_every_tab_without_panics() {
        let ctx = egui::Context::default();
        crate::theme_manager::install(&ctx);
        crate::theme_manager::apply(&ctx, wolfcraft_engine::config::ThemeMode::Dark);
        let atlases = wolfcraft_assets::atlas_manager::AtlasManager::new(None).unwrap();
        let icons = GuiIcons::new(&ctx, &atlases);
        let mut st = StudioState::default();
        let mut inv = Inventory::default();
        let view = StudioView { project: "Тест".into(), mobs: vec![("Овца".into(), 8.0, 8.0, [1.0, 2.0, 3.0])], players: vec![("Я".into(), [0.0; 3], 20.0)], compact: true, ..Default::default() };
        for tab in [Tab::Home, Tab::Model, Tab::Brush, Tab::Test, Tab::View] {
            for node in [Node::Workspace, Node::Lighting, Node::Mob(0), Node::Player(0), Node::Script] {
                st.tab = tab;
                st.selected = node;
                st.show_output = true;
                st.center = if tab == Tab::Test { Center::Script } else { Center::World };
                let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 420.0))), ..Default::default() };
                let _ = ctx.run(input, |c| {
                    let (_, covered) = draw(c, &icons, &mut st, &view, &mut inv);
                    assert!(!covered.is_empty());
                });
            }
        }
    }

    /// Writes the studio's layout (untextured triangles) for a look at it.
    #[test]
    #[ignore]
    fn dump_layout() {
        let ctx = egui::Context::default();
        crate::theme_manager::install(&ctx);
        crate::theme_manager::apply(&ctx, wolfcraft_engine::config::ThemeMode::Dark);
        let atlases = wolfcraft_assets::atlas_manager::AtlasManager::new(None).unwrap();
        let icons = GuiIcons::new(&ctx, &atlases);
        let mut st = StudioState { selected: Node::Lighting, show_output: true, ..Default::default() };
        let mut inv = Inventory::default();
        let view = StudioView { project: "Мой сервер".into(), compact: true, mobs: vec![("Овца".into(), 8.0, 8.0, [1.0, 2.0, 3.0])], players: vec![("Я".into(), [0.0; 3], 20.0)], ..Default::default() };
        let mut out = String::new();
        for _ in 0..3 {
            let (w, h) = if std::env::var("PORTRAIT").is_ok() { (360.0, 780.0) } else { (800.0, 360.0) };
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h))), ..Default::default() };
            let o = ctx.run(input, |c| {
                draw(c, &icons, &mut st, &view, &mut inv);
            });
            out.clear();
            for cp in ctx.tessellate(o.shapes, 1.0) {
                if let egui::epaint::Primitive::Mesh(m) = cp.primitive {
                    let font = m.texture_id == egui::TextureId::default();
                    for t in m.indices.chunks(3) {
                        let v: Vec<_> = t.iter().map(|i| m.vertices[*i as usize]).collect();
                        let c0 = Pos2::new((v[0].pos.x + v[1].pos.x + v[2].pos.x) / 3.0, (v[0].pos.y + v[1].pos.y + v[2].pos.y) / 3.0);
                        if !cp.clip_rect.contains(c0) {
                            continue;
                        }
                        let c = v[0].color;
                        out += &format!("{} {} {} {} {} {} {} {} {} {}\n", v[0].pos.x, v[0].pos.y, v[1].pos.x, v[1].pos.y, v[2].pos.x, v[2].pos.y, c.r(), c.g(), c.b(), if font { 1 } else { 0 });
                    }
                }
            }
        }
        std::fs::write(std::env::var("LAYOUT_OUT").unwrap_or("/tmp/studio.txt".into()), out).unwrap();
    }
}
