//! Mobile launcher: the game's home screen. A toolbar on top, the page in the middle and a
//! bottom navigation bar with tabs: Главная, Студия, Скины and Настройки. Launch controls live on Home.

use crate::gui_renderer::GuiIcons;
use crate::main_menu::MenuAction;
use crate::theme_manager::{Palette, bold_font, palette};
use crate::ui_components::{ButtonKind, pill_button};
use egui::{Align2, Color32, FontId, Frame, Id, Margin, Pos2, Rect, Rounding, Sense, Stroke, TextureHandle, Ui, Vec2};
use wolfcraft_assets::gui as icon;

const TOOLBAR_H: f32 = 56.0;
const NAV_H: f32 = 64.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Settings,
    Studio,
    Skins,
}

const TABS: [(Tab, &str); 4] = [(Tab::Home, "Главная"), (Tab::Studio, "Студия"), (Tab::Skins, "Скины"), (Tab::Settings, "Настройки")];

struct Banner {
    title: &'static str,
    text: &'static str,
    button: &'static str,
    top: Color32,
    bottom: Color32,
    planet: bool,
}

const BANNERS: [Banner; 4] = [
    Banner { title: "Целая планета из блоков", text: "Дойдите до ребра куба и шагните на соседнюю грань", button: "Играть", top: Color32::from_rgb(10, 22, 48), bottom: Color32::from_rgb(18, 92, 120), planet: true },
    Banner { title: "Лук и стрелы", text: "Зажмите, чтобы натянуть тетиву, отпустите — выстрел", button: "Играть", top: Color32::from_rgb(70, 36, 16), bottom: Color32::from_rgb(170, 96, 34), planet: false },
    Banner { title: "Игра по сети", text: "Создайте сервер на телефоне и позовите друзей", button: "Открыть", top: Color32::from_rgb(28, 22, 70), bottom: Color32::from_rgb(96, 60, 170), planet: false },
    Banner { title: "Свои текстуры", text: "Встроенные паки 16×–128× и загрузка своих", button: "Настройки", top: Color32::from_rgb(16, 52, 30), bottom: Color32::from_rgb(52, 140, 72), planet: false },
];

struct Release {
    version: &'static str,
    name: &'static str,
    date: &'static str,
    points: &'static [&'static str],
}

const RELEASES: [Release; 6] = [
    Release { version: "0.3.0", name: "Стройка и мебель", date: "ноябрь 2026", points: &["Дуговое стекло и купола", "Формы и материалы", "Двери и мебель", "Конструктор на верстаке"] },
    Release { version: "0.2.1", name: "Лаунчер и графика", date: "ноябрь 2026", points: &["Запуск с главной", "Ник под выбором версии", "Прозрачная вода", "Обновления в настройках"] },
    Release { version: "0.2.0", name: "Кубы и луки", date: "ноябрь 2026", points: &["Кубические планеты", "Лук, стрелы, доспехи", "Новые блоки и двери", "Лаунчер и новый чат"] },
    Release { version: "0.1.9", name: "Новый HUD", date: "октябрь 2026", points: &["Островок паузы и чата", "Миникарта и карта мира", "Компас вокруг карты"] },
    Release { version: "0.1.8", name: "Текстурпаки", date: "октябрь 2026", points: &["Паки 16×–128×", "Загрузка своих паков", "Быстрый полёт в космосе"] },
    Release { version: "0.1.7", name: "Мультиплеер", date: "сентябрь 2026", points: &["Сервер на телефоне", "Приватные миры", "Игра по сети"] },
];

struct News {
    title: &'static str,
    date: &'static str,
    text: &'static str,
    color: Color32,
    icon: u16,
}

const NEWS: [News; 4] = [
    News { title: "Инструменты как в Майне", date: "11 ноября", text: "Кирка, топор, лопата и меч держатся в руке по-новому, у каждого свой удар.", color: Color32::from_rgb(230, 150, 50), icon: icon::HAMMER },
    News { title: "Ровный мир на кубе", date: "11 ноября", text: "Блоки на гранях кубической планеты теперь стоят строго вертикально.", color: Color32::from_rgb(40, 180, 200), icon: icon::CUBE },
    News { title: "Новый чат", date: "11 ноября", text: "Полупрозрачная панель, ники цветом и кнопка отправки.", color: Color32::from_rgb(140, 100, 230), icon: icon::EDIT },
    News { title: "Доспехи", date: "10 ноября", text: "Четыре слота доспехов в инвентаре: шлем, нагрудник, поножи и ботинки.", color: Color32::from_rgb(90, 190, 100), icon: icon::STAR },
];

/// The launcher. `skin` is the selected skin preset.
pub fn show(ctx: &egui::Context, icons: &GuiIcons, version: &str, dark: bool, planet: Option<&TextureHandle>, skin: u8, nickname: &str, worlds: &[crate::main_menu::WorldEntry]) -> MenuAction {
    let p = palette(ctx);
    let mut action = MenuAction::None;
    let tab_id = Id::new("launcher-tab-v2");
    let mut tab: usize = ctx.data(|d| d.get_temp(tab_id)).unwrap_or(0);
    tab = tab.min(2);

    // ---- Toolbar ----
    egui::TopBottomPanel::top("launcher-toolbar")
        .exact_height(TOOLBAR_H)
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::symmetric(16.0, 8.0)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                icons.paint(ui.painter(), r, icon::LOGO, Color32::WHITE);
                ui.add_space(4.0);
                ui.label(egui::RichText::new("WolfCraft").font(bold_font(20.0)).color(p.text));
                // Upright phone: room only for the name and the two buttons.
                let wide = ui.ctx().screen_rect().width() > 480.0;
                if wide {
                    ui.label(egui::RichText::new(TABS[tab].1).size(14.0).color(p.text_muted));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if round_icon(ui, icons, &p, icon::GEAR).clicked() {
                        action = MenuAction::OpenSettings;
                    }
                    if round_icon(ui, icons, &p, if dark { icon::SUN } else { icon::MOON }).clicked() {
                        action = MenuAction::ToggleTheme;
                    }
                    if wide {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(64.0, 26.0), Sense::hover());
                        ui.painter().rect_stroke(r, Rounding::same(13.0), Stroke::new(1.5_f32, p.accent));
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, format!("v{version}"), FontId::proportional(12.0), p.accent);
                    }
                });
            });
        });

    // ---- Bottom navigation ----
    egui::TopBottomPanel::bottom("launcher-nav")
        .exact_height(NAV_H)
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(1.0_f32, p.outline)))
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let w = rect.width() / TABS.len() as f32;
            for (i, (t, label)) in TABS.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(rect.left() + w * i as f32, rect.top()), Vec2::new(w, rect.height()));
                let resp = ui.interact(r, Id::new(("launcher-nav", i)), Sense::click());
                let on = tab == i;
                let col = if on { p.accent } else if resp.hovered() { p.text } else { p.text_muted };
                if on {
                    let pill = Rect::from_center_size(Pos2::new(r.center().x, r.top() + 22.0), Vec2::new(56.0, 30.0));
                    ui.painter().rect_filled(pill, Rounding::same(15.0), p.accent.gamma_multiply(0.18));
                }
                let ic = Rect::from_center_size(Pos2::new(r.center().x, r.top() + 22.0), Vec2::splat(22.0));
                nav_icon(ui, icons, *t, ic, col);
                ui.painter().text(Pos2::new(r.center().x, r.bottom() - 13.0), Align2::CENTER_CENTER, *label, FontId::proportional(12.5), col);
                if resp.clicked() {
                    if *t == Tab::Settings {
                        action = MenuAction::OpenSettings;
                    } else {
                        tab = i;
                    }
                }
            }
        });
    ctx.data_mut(|d| d.insert_temp(tab_id, tab));

    // Account and launch controls stay visible while Home scrolls.
    if TABS[tab].0 == Tab::Home {
        egui::TopBottomPanel::bottom("launcher-launch")
            .exact_height(100.0)
            .frame(Frame::none().fill(p.surface).inner_margin(Margin::symmetric(16.0, 10.0)))
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(format!("Ник: {nickname}")).color(p.text));
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 118.0).max(100.0);
                    egui::ComboBox::from_id_salt("launcher-version")
                        .width(width).selected_text(format!("WolfCraft {version}"))
                        .show_ui(ui, |ui| {
                            ui.label(format!("{version} — установлена"));
                            ui.separator();
                            ui.label("Другие версии доступны через обновление APK");
                        });
                    if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, 108.0, true).clicked() {
                        action = MenuAction::LaunchVersion(version.to_string());
                    }
                });
            });
    }

    // ---- Page ----
    egui::CentralPanel::default().frame(Frame::none().fill(p.bg).inner_margin(Margin::symmetric(16.0, 14.0))).show(ctx, |ui| {
        egui::ScrollArea::vertical().id_salt(("launcher-page", tab)).auto_shrink([false, false]).drag_to_scroll(true).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            match TABS[tab].0 {
                Tab::Home => home(ui, icons, &p, planet, &mut action),
                Tab::Settings => {}
                Tab::Studio => studio(ui, icons, &p, version, worlds, &mut action),
                Tab::Skins => skins(ui, &p, skin, &mut action),
            }
            ui.add_space(8.0);
        });
    });
    action
}

fn round_icon(ui: &mut Ui, icons: &GuiIcons, p: &Palette, ic: u16) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
    let fill = if resp.hovered() || resp.is_pointer_button_down_on() { p.surface_hover } else { p.bg };
    ui.painter().circle_filled(r.center(), 18.0, fill);
    icons.paint(ui.painter(), r.shrink(9.0), ic, p.text);
    resp
}

fn nav_icon(ui: &Ui, icons: &GuiIcons, t: Tab, r: Rect, col: Color32) {
    let pt = ui.painter();
    match t {
        Tab::Home => {
            // A little house.
            let (c, s) = (r.center(), r.width() / 2.0);
            let roof = vec![Pos2::new(c.x - s, c.y - 1.0), Pos2::new(c.x, c.y - s), Pos2::new(c.x + s, c.y - 1.0)];
            pt.add(egui::Shape::line(roof, Stroke::new(2.2_f32, col)));
            let body = Rect::from_min_max(Pos2::new(c.x - s * 0.7, c.y - 1.0), Pos2::new(c.x + s * 0.7, c.y + s));
            pt.rect_stroke(body, Rounding::same(1.0), Stroke::new(2.2_f32, col));
            pt.rect_filled(Rect::from_min_max(Pos2::new(c.x - 2.5, c.y + s * 0.3), Pos2::new(c.x + 2.5, c.y + s)), Rounding::ZERO, col);
        }
        Tab::Settings => icons.paint(pt, r, icon::GEAR, col),
        Tab::Studio => icons.paint(pt, r, icon::HAMMER, col),
        Tab::Skins => {
            // A small figure: head and shoulders.
            let c = r.center();
            pt.circle_stroke(Pos2::new(c.x, c.y - 4.0), 5.0, Stroke::new(2.2_f32, col));
            let sh = vec![Pos2::new(c.x - 9.0, c.y + 10.0), Pos2::new(c.x - 7.0, c.y + 4.0), Pos2::new(c.x + 7.0, c.y + 4.0), Pos2::new(c.x + 9.0, c.y + 10.0)];
            pt.add(egui::Shape::line(sh, Stroke::new(2.2_f32, col)));
        }
    }
}

fn section(ui: &mut Ui, p: &Palette, title: &str, more: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).font(bold_font(18.0)).color(p.text));
        if !more.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(more).size(13.0).color(p.accent));
            });
        }
    });
}

fn gradient(ui: &Ui, r: Rect, top: Color32, bottom: Color32) {
    let mut m = egui::Mesh::default();
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.right_bottom(), bottom);
    m.colored_vertex(r.left_bottom(), bottom);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(m));
}

// ---------------------------------------------------------------------------------------------
// Главная

fn home(ui: &mut Ui, icons: &GuiIcons, p: &Palette, planet: Option<&TextureHandle>, action: &mut MenuAction) {
    banner_slider(ui, p, planet, action);
    section(ui, p, "Релизы", "все версии");
    egui::ScrollArea::horizontal().id_salt("launcher-releases").auto_shrink([false, true]).drag_to_scroll(true).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            for (i, r) in RELEASES.iter().enumerate() {
                release_card(ui, p, r, i == 0);
            }
        });
    });
    section(ui, p, "Новости", "");
    for n in &NEWS {
        news_card(ui, icons, p, n);
    }
}

fn banner_slider(ui: &mut Ui, p: &Palette, planet: Option<&TextureHandle>, action: &mut MenuAction) {
    let ctx = ui.ctx().clone();
    let w = ui.available_width();
    let h = (w * 0.36).clamp(180.0, 220.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click_and_drag());
    let id = Id::new("launcher-banner");
    let now = ctx.input(|i| i.time);
    // (current slide, time of the last change)
    let (mut cur, mut since): (usize, f64) = ctx.data(|d| d.get_temp(id)).unwrap_or((0, now));
    let n = BANNERS.len();
    // Swipe left/right to change the slide; otherwise it moves on by itself every 6 s.
    let drag = if resp.dragged() { resp.drag_delta().x } else { 0.0 };
    let off_id = id.with("off");
    let mut off: f32 = ctx.data(|d| d.get_temp(off_id)).unwrap_or(0.0);
    off += drag;
    if resp.drag_stopped() {
        if off < -w * 0.15 {
            cur = (cur + 1) % n;
            since = now;
        } else if off > w * 0.15 {
            cur = (cur + n - 1) % n;
            since = now;
        }
        off = 0.0;
    }
    if !resp.dragged() && now - since > 6.0 {
        cur = (cur + 1) % n;
        since = now;
    }
    ctx.data_mut(|d| {
        d.insert_temp(id, (cur, since));
        d.insert_temp(off_id, off);
    });
    ctx.request_repaint_after(std::time::Duration::from_millis(500));

    let clip = ui.painter().with_clip_rect(rect);
    for (k, idx) in [(-1i32, (cur + n - 1) % n), (0, cur), (1, (cur + 1) % n)] {
        let r = rect.translate(Vec2::new(k as f32 * (w + 12.0) + off, 0.0));
        if !r.intersects(rect) {
            continue;
        }
        let b = &BANNERS[idx];
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(r.left_top(), b.top);
        mesh.colored_vertex(r.right_top(), b.top);
        mesh.colored_vertex(r.right_bottom(), b.bottom);
        mesh.colored_vertex(r.left_bottom(), b.bottom);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        clip.add(egui::Shape::mesh(mesh));
        // Decoration on the right: the planet, or big pixel cubes.
        if b.planet {
            if let Some(tex) = planet {
                let d = r.height() * 1.25;
                let c = Pos2::new(r.right() - d * 0.32, r.center().y + d * 0.18);
                clip.image(tex.id(), Rect::from_center_size(c, Vec2::splat(d)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
        } else if r.width() > 480.0 {
            for (j, (dx, dy, s)) in [(0.14f32, 0.30f32, 0.34f32), (0.30, 0.62, 0.22), (0.06, 0.70, 0.16)].iter().enumerate() {
                let side = r.height() * s;
                let c = Pos2::new(r.right() - r.height() * (dx + 0.12) - side * 0.5, r.top() + r.height() * dy);
                let q = Rect::from_center_size(c, Vec2::splat(side));
                clip.rect_filled(q, Rounding::same(4.0), Color32::from_white_alpha(28 + j as u8 * 10));
                clip.rect_stroke(q, Rounding::same(4.0), Stroke::new(2.0_f32, Color32::from_white_alpha(60)));
            }
        }
        // Text on the left, over a soft shade.
        let text_w = if r.width() > 480.0 { r.width() * 0.62 } else { r.width() - 40.0 };
        let x = r.left() + 20.0;
        clip.text(Pos2::new(x, r.top() + 22.0), Align2::LEFT_TOP, "WOLFCRAFT", FontId::proportional(11.0), Color32::from_white_alpha(170));
        let title = clip.layout(b.title.to_string(), bold_font(22.0), Color32::WHITE, text_w);
        let th = title.size().y;
        clip.galley(Pos2::new(x, r.top() + 40.0), title, Color32::WHITE);
        let text = clip.layout(b.text.to_string(), FontId::proportional(13.5), Color32::from_white_alpha(215), text_w);
        clip.galley(Pos2::new(x, r.top() + 46.0 + th), text, Color32::WHITE);
        let btn = Rect::from_min_size(Pos2::new(x, r.bottom() - 46.0), Vec2::new(112.0, 32.0));
        clip.rect_filled(btn, Rounding::same(16.0), Color32::WHITE);
        clip.text(btn.center(), Align2::CENTER_CENTER, b.button, bold_font(14.0), b.top);
    }
    ui.painter().rect_stroke(rect, Rounding::same(14.0), Stroke::new(1.0_f32, p.outline));
    // Dots.
    let dots_w = n as f32 * 14.0;
    for i in 0..n {
        let c = Pos2::new(rect.center().x - dots_w / 2.0 + 7.0 + i as f32 * 14.0, rect.bottom() - 12.0);
        if i == cur {
            ui.painter().rect_filled(Rect::from_center_size(c, Vec2::new(16.0, 6.0)), Rounding::same(3.0), Color32::WHITE);
        } else {
            ui.painter().circle_filled(c, 3.0, Color32::from_white_alpha(110));
        }
    }
    // Tap: the slide's button.
    if resp.clicked() {
        *action = match cur {
            2 => MenuAction::OpenNetwork,
            3 => MenuAction::OpenSettings,
            _ => MenuAction::LaunchVersion(wolfcraft_common::VERSION.to_string()),
        };
    }
}

fn release_card(ui: &mut Ui, p: &Palette, r: &Release, latest: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(210.0, 168.0), Sense::hover());
    let pt = ui.painter();
    pt.rect_filled(rect, Rounding::same(12.0), p.surface);
    pt.rect_stroke(rect, Rounding::same(12.0), Stroke::new(1.0_f32, if latest { p.accent } else { p.outline }));
    let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 54.0));
    let (a, b) = if latest { (p.accent, p.accent.gamma_multiply(0.45)) } else { (Color32::from_rgb(60, 66, 80), Color32::from_rgb(36, 40, 50)) };
    let shape = egui::epaint::RectShape::filled(head, Rounding { nw: 12.0, ne: 12.0, sw: 0.0, se: 0.0 }, a);
    pt.add(shape);
    let _ = b;
    pt.text(Pos2::new(rect.left() + 14.0, head.top() + 10.0), Align2::LEFT_TOP, format!("v{}", r.version), bold_font(18.0), Color32::WHITE);
    pt.text(Pos2::new(rect.left() + 14.0, head.top() + 33.0), Align2::LEFT_TOP, r.date, FontId::proportional(11.5), Color32::from_white_alpha(200));
    if latest {
        let tag = Rect::from_min_size(Pos2::new(rect.right() - 70.0, head.top() + 12.0), Vec2::new(58.0, 20.0));
        pt.rect_filled(tag, Rounding::same(10.0), Color32::from_white_alpha(230));
        pt.text(tag.center(), Align2::CENTER_CENTER, "новая", FontId::proportional(11.0), p.accent);
    }
    pt.text(Pos2::new(rect.left() + 14.0, head.bottom() + 10.0), Align2::LEFT_TOP, r.name, bold_font(14.0), p.text);
    for (i, line) in r.points.iter().enumerate() {
        let y = head.bottom() + 34.0 + i as f32 * 19.0;
        pt.circle_filled(Pos2::new(rect.left() + 18.0, y + 7.0), 2.5, p.accent);
        pt.text(Pos2::new(rect.left() + 27.0, y), Align2::LEFT_TOP, *line, FontId::proportional(12.5), p.text_muted);
    }
}

fn news_card(ui: &mut Ui, icons: &GuiIcons, p: &Palette, n: &News) {
    Frame::none().fill(p.surface).rounding(Rounding::same(12.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(52.0), Sense::hover());
            gradient(ui, r, n.color, n.color.gamma_multiply(0.55));
            icons.paint(ui.painter(), r.shrink(13.0), n.icon, Color32::WHITE);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(n.title).font(bold_font(15.0)).color(p.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(n.date).size(12.0).color(p.text_muted));
                    });
                });
                ui.label(egui::RichText::new(n.text).size(13.0).color(p.text_muted));
            });
        });
    });
}

// ---------------------------------------------------------------------------------------------
// Игра

// ---------------------------------------------------------------------------------------------
// Студия

fn studio(ui: &mut Ui, icons: &GuiIcons, p: &Palette, version: &str, worlds: &[crate::main_menu::WorldEntry], action: &mut MenuAction) {
    Frame::none().fill(p.surface).rounding(Rounding::same(14.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(18.0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(72.0), Sense::hover());
            icons.paint(ui.painter(), r, icon::LOGO, Color32::WHITE);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("WolfCraft Студия").font(bold_font(22.0)).color(p.text));
                ui.label(egui::RichText::new("Создавайте свои игровые серверы: карта, скрипты на Luau, свои правила").size(13.5).color(p.text_muted));
                ui.label(egui::RichText::new(format!("Версия игры {version}")).size(13.0).color(p.accent));
            });
        });
    });
    section(ui, p, "Мои серверы", "");
    ui.label(egui::RichText::new("Каждый сервер — это ваша игра (как плейс в Роблоксе). Новый сервер начинается с серой плиты-основы: стройте из блоков игры, пишите скрипт на Luau и запускайте — друзья заходят и играют").size(13.0).color(p.text_muted));
    if pill_button(ui, icons, Some(icon::PLUS), "Новый сервер (основа)", ButtonKind::Primary, ui.available_width(), true).clicked() {
        *action = MenuAction::NewStudioProject;
    }
    let projects: Vec<&crate::main_menu::WorldEntry> = worlds.iter().filter(|w| w.cfg.studio_project).collect();
    if projects.is_empty() {
        ui.label(egui::RichText::new("Пока нет серверов").size(13.0).color(p.text_muted));
    }
    for w in projects {
        Frame::none().fill(p.surface).rounding(Rounding::same(12.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(10.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
                if let Some(t) = &w.preview {
                    ui.painter().image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                } else {
                    ui.painter().rect_filled(r, Rounding::same(8.0), p.bg);
                }
                ui.vertical(|ui| {
                    // Older studio worlds were named "Проект N": they are servers now.
                    let name = w.cfg.name.strip_prefix("Проект").map(|rest| format!("Сервер{rest}")).unwrap_or_else(|| w.cfg.name.clone());
                    ui.label(egui::RichText::new(name).font(bold_font(15.0)).color(p.text));
                    ui.label(egui::RichText::new(format!("Сид {}", w.cfg.seed as i64)).size(12.0).color(p.text_muted));
                    ui.horizontal(|ui| {
                        if pill_button(ui, icons, Some(icon::HAMMER), "Изменить", ButtonKind::Primary, 120.0, true).clicked() {
                            *action = MenuAction::OpenStudio(w.cfg.id.clone());
                        }
                        if pill_button(ui, icons, Some(icon::GLOBE), "Запустить", ButtonKind::Secondary, 120.0, true).clicked() {
                            *action = MenuAction::HostServer(Some(w.cfg.id.clone()));
                        }
                    });
                });
            });
        });
    }
    section(ui, p, "О игре", "");
    for (t, d) in [
        ("Планета вместо плоского мира", "Шар или куб: обойдите его целиком, улетите в космос и вернитесь"),
        ("Свой сервер в кармане", "Телефон становится сервером, друзья подключаются по сети"),
        ("Только мобильная версия", "Игра сделана под сенсорное управление и экраны телефонов"),
    ] {
        Frame::none().fill(p.surface).rounding(Rounding::same(12.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(t).font(bold_font(15.0)).color(p.text));
            ui.label(egui::RichText::new(d).size(13.0).color(p.text_muted));
        });
    }
    ui.add_space(6.0);
    if pill_button(ui, icons, Some(icon::EXIT), "Выйти из игры", ButtonKind::Danger, ui.available_width(), true).clicked() {
        *action = MenuAction::Quit;
    }
}

// ---------------------------------------------------------------------------------------------
// Скины

fn skins(ui: &mut Ui, p: &Palette, skin: u8, action: &mut MenuAction) {
    use wolfcraft_engine::renderer::model::{SKIN_NAMES, skin_colors};
    let w = ui.available_width();
    // Big preview of the chosen skin.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 200.0), Sense::hover());
    gradient(ui, rect, p.surface, p.bg);
    ui.painter().rect_stroke(rect, Rounding::same(14.0), Stroke::new(1.0_f32, p.outline));
    figure(ui, Rect::from_center_size(Pos2::new(rect.left() + 110.0, rect.center().y), Vec2::new(90.0, 180.0)), skin_colors(skin));
    let name = SKIN_NAMES.get(skin as usize).copied().unwrap_or("—");
    ui.painter().text(Pos2::new(rect.left() + 200.0, rect.top() + 50.0), Align2::LEFT_TOP, name, bold_font(24.0), p.text);
    ui.painter().text(Pos2::new(rect.left() + 200.0, rect.top() + 84.0), Align2::LEFT_TOP, "Ваш скин в игре и на сервере", FontId::proportional(13.5), p.text_muted);
    ui.painter().text(Pos2::new(rect.left() + 200.0, rect.top() + 106.0), Align2::LEFT_TOP, "Виден от третьего лица и другим игрокам", FontId::proportional(13.5), p.text_muted);
    section(ui, p, "Выбор скина", "");
    let cols = ((w + 10.0) / 120.0).floor().max(2.0) as usize;
    let cw = (w - (cols as f32 - 1.0) * 10.0) / cols as f32;
    for row in (0..SKIN_NAMES.len()).collect::<Vec<_>>().chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            for &i in row {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(cw, 150.0), Sense::click());
                let on = i as u8 == skin;
                ui.painter().rect_filled(r, Rounding::same(12.0), if resp.hovered() { p.surface_hover } else { p.surface });
                ui.painter().rect_stroke(r, Rounding::same(12.0), Stroke::new(if on { 2.5_f32 } else { 1.0_f32 }, if on { p.accent } else { p.outline }));
                figure(ui, Rect::from_center_size(Pos2::new(r.center().x, r.top() + 62.0), Vec2::new(46.0, 96.0)), skin_colors(i as u8));
                ui.painter().text(Pos2::new(r.center().x, r.bottom() - 18.0), Align2::CENTER_CENTER, SKIN_NAMES[i], FontId::proportional(13.0), if on { p.accent } else { p.text });
                if resp.clicked() {
                    *action = MenuAction::SetSkin(i as u8);
                }
            }
        });
    }
}

/// Flat front view of the character in the given skin colours.
fn figure(ui: &Ui, r: Rect, (skin, hair, jacket, sleeve, trousers): ([f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3])) {
    let c = |v: [f32; 3]| Color32::from_rgb((v[0] * 255.0) as u8, (v[1] * 255.0) as u8, (v[2] * 255.0) as u8);
    let u = r.width() / 8.0; // the body is 8 units wide, 16 tall (head 4, torso 6, legs 6)
    let x0 = r.center().x - 4.0 * u;
    let y0 = r.center().y - 8.0 * u;
    let pt = ui.painter();
    let b = |x: f32, y: f32, w: f32, h: f32, col: Color32| pt.rect_filled(Rect::from_min_size(Pos2::new(x0 + x * u, y0 + y * u), Vec2::new(w * u, h * u)), Rounding::ZERO, col);
    b(2.0, 0.0, 4.0, 4.0, c(skin));
    b(2.0, 0.0, 4.0, 1.2, c(hair));
    b(2.7, 1.9, 0.8, 0.7, Color32::from_rgb(24, 28, 40));
    b(4.5, 1.9, 0.8, 0.7, Color32::from_rgb(24, 28, 40));
    b(2.0, 4.0, 4.0, 6.0, c(jacket));
    b(0.0, 4.0, 2.0, 5.0, c(sleeve));
    b(6.0, 4.0, 2.0, 5.0, c(sleeve));
    b(0.0, 9.0, 2.0, 1.0, c(skin));
    b(6.0, 9.0, 2.0, 1.0, c(skin));
    b(2.0, 9.4, 4.0, 0.6, Color32::from_rgb(240, 150, 40));
    b(2.0, 10.0, 2.0, 5.0, c(trousers));
    b(4.0, 10.0, 2.0, 5.0, c(trousers));
    b(2.0, 15.0, 2.0, 1.0, Color32::from_rgb(66, 44, 28));
    b(4.0, 15.0, 2.0, 1.0, Color32::from_rgb(66, 44, 28));
}


#[cfg(test)]
mod tests {
    use super::*;
    use wolfcraft_engine::config::ThemeMode;

    #[test]
    fn portrait_home_launches_installed_version_and_shows_nickname() {
        let ctx = egui::Context::default();
        crate::theme_manager::install(&ctx);
        crate::theme_manager::apply(&ctx, ThemeMode::Dark);
        let atlas = wolfcraft_assets::AtlasManager::new(None).unwrap();
        let icons = GuiIcons::new(&ctx, &atlas);
        let mut action = MenuAction::None;
        let mut run = |events: Vec<egui::Event>| {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(390.0, 844.0))),
                events, ..Default::default()
            };
            ctx.run(raw, |ctx| {
                action = show(ctx, &icons, "0.2.1", true, None, 0, "dozyFJ", &[]);
            })
        };
        let out = run(vec![]);
        let texts: Vec<_> = out.shapes.iter().filter_map(|s| {
            if let egui::epaint::Shape::Text(t) = &s.shape { Some(t) } else { None }
        }).collect();
        assert!(texts.iter().any(|t| t.galley.job.text.contains("dozyFJ")));
        assert!(!texts.iter().any(|t| t.galley.job.text == "Игра"));
        let play = texts.iter().find(|t| t.galley.job.text == "Играть" && t.pos.y > 650.0).expect("Home launch button");
        let pos = play.pos + play.galley.size() * 0.5;
        run(vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        run(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        assert_eq!(action, MenuAction::LaunchVersion("0.2.1".into()));
    }
}
