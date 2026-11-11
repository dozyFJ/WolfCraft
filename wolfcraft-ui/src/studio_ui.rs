//! Studio: edit a world like a project. A ribbon on top (exit, panels, run, save, publish as a
//! server), the "Ассеты" panel with every block and item of the game on the left and the world
//! script (chat commands, one per line, run when the world starts) on the right. The middle of
//! the screen stays the live world, built with the usual touch controls.

use crate::gui_renderer::GuiIcons;
use crate::theme_manager::{Palette, bold_font, palette};
use egui::{Align2, Color32, FontId, Frame, Id, Margin, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use wolfcraft_assets::gui as icon;
use wolfcraft_engine::inventory::Inventory;
use wolfcraft_engine::items::{self, ItemStack};

pub const RIBBON_H: f32 = 48.0;

#[derive(Default)]
pub struct StudioState {
    pub assets_open: bool,
    pub script_open: bool,
    pub search: String,
    /// World script: chat commands, one per line (`#` starts a comment).
    pub script: String,
    /// Short status line in the ribbon ("Сохранено", "Скрипт выполнен", ...).
    pub status: String,
    pub status_t: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StudioAction {
    /// Save the world and the script, leave to the launcher.
    Exit,
    /// Run every command of the script now.
    RunScript,
    /// Save the world and the script.
    Save,
    /// Save, then start a server with this world so friends can join.
    Publish,
    /// Toggle day and night.
    DayNight,
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

/// Draws the studio over the world. Returns the actions and the screen areas (points) that
/// the panels cover, so touches there do not steer the game.
pub fn draw(ctx: &egui::Context, icons: &GuiIcons, st: &mut StudioState, project: &str, inv: &mut Inventory, now: f64) -> (Vec<StudioAction>, Vec<Rect>) {
    let p = palette(ctx);
    let screen = ctx.screen_rect();
    let mut actions = Vec::new();
    let mut covered = Vec::new();

    // ---- Ribbon ----
    let ribbon = Rect::from_min_size(screen.min, Vec2::new(screen.width(), RIBBON_H));
    covered.push(ribbon);
    egui::Area::new(Id::new("studio-ribbon")).order(egui::Order::Foreground).fixed_pos(ribbon.min).show(ctx, |ui| {
        Frame::none().fill(p.surface).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::symmetric(10.0, 6.0)).show(ui, |ui| {
            ui.set_width(screen.width() - 20.0);
            ui.set_height(RIBBON_H - 12.0);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if tool(ui, icons, &p, icon::ARROW_LEFT, "Выйти", false).clicked() {
                    actions.push(StudioAction::Exit);
                }
                ui.add_space(4.0);
                let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                icons.paint(ui.painter(), r, icon::HAMMER, p.accent);
                ui.label(egui::RichText::new("Студия").font(bold_font(16.0)).color(p.text));
                ui.label(egui::RichText::new(project).size(13.0).color(p.text_muted));
                ui.add_space(10.0);
                if tool(ui, icons, &p, icon::CUBE, "Ассеты", st.assets_open).clicked() {
                    st.assets_open = !st.assets_open;
                }
                if tool(ui, icons, &p, icon::EDIT, "Скрипт", st.script_open).clicked() {
                    st.script_open = !st.script_open;
                }
                if tool(ui, icons, &p, icon::PLAY, "Запуск", false).clicked() {
                    actions.push(StudioAction::RunScript);
                }
                if tool(ui, icons, &p, icon::SUN, "День/ночь", false).clicked() {
                    actions.push(StudioAction::DayNight);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if tool(ui, icons, &p, icon::GLOBE, "Опубликовать", false).on_hover_text("Сохранить и запустить сервер с этим миром").clicked() {
                        actions.push(StudioAction::Publish);
                    }
                    if tool(ui, icons, &p, icon::DOWNLOAD, "Сохранить", false).clicked() {
                        actions.push(StudioAction::Save);
                    }
                    if !st.status.is_empty() && now - st.status_t < 4.0 {
                        ui.label(egui::RichText::new(&st.status).size(13.0).color(p.success));
                    }
                });
            });
        });
    });

    let top = screen.top() + RIBBON_H + 8.0;
    // ---- Assets (left) ----
    if st.assets_open {
        let w = (screen.width() * 0.30).clamp(250.0, 340.0);
        let h = (screen.height() * 0.58).min(screen.height() - RIBBON_H - 20.0);
        let rect = Rect::from_min_size(Pos2::new(screen.left() + 8.0, top), Vec2::new(w, h));
        covered.push(rect);
        panel(ctx, "studio-assets", rect, &p, |ui| {
            header(ui, &p, "Ассеты", "блоки и предметы игры");
            ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("Поиск…").desired_width(f32::INFINITY));
            ui.add_space(4.0);
            let q = st.search.trim().to_lowercase();
            let mut list: Vec<u16> = items::creative_palette().into_iter().filter(|id| q.is_empty() || items::info(*id).name.to_lowercase().contains(&q)).collect();
            list.sort_by_key(|id| !items::is_new(*id));
            let cell = 46.0;
            let cols = ((ui.available_width() + 4.0) / (cell + 4.0)).floor().max(1.0) as usize;
            egui::ScrollArea::vertical().id_salt("studio-assets-scroll").auto_shrink([false, false]).drag_to_scroll(true).show(ui, |ui| {
                for row in list.chunks(cols) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for id in row {
                            let (r, resp) = ui.allocate_exact_size(Vec2::splat(cell), Sense::click());
                            let held = inv.held().map(|s| s.id) == Some(*id);
                            ui.painter().rect_filled(r, Rounding::same(6.0), if resp.hovered() { p.surface_hover } else { p.bg });
                            ui.painter().rect_stroke(r, Rounding::same(6.0), Stroke::new(if held { 2.0_f32 } else { 1.0_f32 }, if held { p.accent } else { p.outline }));
                            icons.paint_item(ui.painter(), r.shrink(7.0), *id);
                            if resp.clicked() {
                                // Into the hand: the selected hotbar slot gets a full stack.
                                inv.slots[inv.selected] = Some(ItemStack::new(*id, items::info(*id).max_stack));
                            }
                            resp.on_hover_text(items::info(*id).name);
                        }
                    });
                    ui.add_space(0.0);
                }
                if list.is_empty() {
                    ui.label(egui::RichText::new("Ничего не найдено").color(p.text_muted));
                }
            });
        });
    }

    // ---- Script (right) ----
    if st.script_open {
        let w = (screen.width() * 0.34).clamp(280.0, 420.0);
        let h = (screen.height() * 0.62).min(screen.height() - RIBBON_H - 20.0);
        let rect = Rect::from_min_size(Pos2::new(screen.right() - 8.0 - w, top), Vec2::new(w, h));
        covered.push(rect);
        panel(ctx, "studio-script", rect, &p, |ui| {
            header(ui, &p, "Скрипт мира", "команды по строке, при входе в мир");
            let editor_h = ui.available_height() - 46.0;
            egui::ScrollArea::vertical().id_salt("studio-script-scroll").max_height(editor_h).auto_shrink([false, false]).show(ui, |ui| {
                ui.add_sized(
                    Vec2::new(ui.available_width(), editor_h),
                    egui::TextEdit::multiline(&mut st.script)
                        .font(FontId::monospace(14.0))
                        .code_editor()
                        .hint_text("# пример\n/time set день\n/give факел 16\n/say Добро пожаловать!")
                        .desired_rows(8),
                );
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if crate::ui_components::pill_button(ui, icons, Some(icon::PLAY), "Запустить", crate::ui_components::ButtonKind::Primary, 140.0, true).clicked() {
                    actions.push(StudioAction::RunScript);
                }
                let n = script_commands(&st.script).len();
                ui.label(egui::RichText::new(format!("команд: {n}")).size(13.0).color(p.text_muted));
            });
        });
    }
    (actions, covered)
}

fn panel(ctx: &egui::Context, id: &str, rect: Rect, p: &Palette, add: impl FnOnce(&mut Ui)) {
    egui::Area::new(Id::new(id)).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
        Frame::none().fill(p.surface.gamma_multiply(0.96)).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(10.0)).show(ui, |ui| {
            ui.set_width(rect.width() - 20.0);
            ui.set_height(rect.height() - 20.0);
            ui.vertical(|ui| add(ui));
        });
    });
}

fn header(ui: &mut Ui, p: &Palette, title: &str, sub: &str) {
    ui.label(egui::RichText::new(title).font(bold_font(16.0)).color(p.text));
    ui.label(egui::RichText::new(sub).size(12.0).color(p.text_muted));
    ui.add_space(4.0);
}

/// Ribbon button: icon over a short caption.
fn tool(ui: &mut Ui, icons: &GuiIcons, p: &Palette, ic: u16, label: &str, on: bool) -> egui::Response {
    let w = 14.0 + label.chars().count() as f32 * 7.4;
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w.max(52.0), RIBBON_H - 12.0), Sense::click());
    let fill = if on {
        p.accent.gamma_multiply(0.22)
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        p.surface_hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(r, Rounding::same(6.0), fill);
    let col = if on { p.accent } else { p.text };
    icons.paint(ui.painter(), Rect::from_center_size(Pos2::new(r.center().x, r.top() + 11.0), Vec2::splat(16.0)), ic, col);
    ui.painter().text(Pos2::new(r.center().x, r.bottom() - 7.0), Align2::CENTER_CENTER, label, FontId::proportional(11.0), col);
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
}
