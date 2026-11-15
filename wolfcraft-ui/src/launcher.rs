//! Mobile launcher: the game's home screen. A toolbar on top, the page in the middle and a
//! bottom navigation bar with tabs: Главная, Скины and Настройки. Launch controls live on Home.
//! The studio is a separate app (WolfStudio): the same code started with `set_studio_app`,
//! whose launcher shows Проекты and Настройки only.

use crate::gui_renderer::GuiIcons;
use crate::main_menu::MenuAction;
use crate::theme_manager::{Palette, bold_font, palette};
use crate::ui_components::{ButtonKind, pill_button};
use egui::{Align2, Color32, FontId, Frame, Id, Margin, Pos2, Rect, Rounding, Sense, Stroke, TextureHandle, Ui, Vec2};
use wolfcraft_assets::gui as icon;

const TOOLBAR_H: f32 = 56.0;
const NAV_H: f32 = 50.0;
const HEAD_H: f32 = 52.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Settings,
    Studio,
    Server,
    Skins,
}

/// The app's data folder (the «Сервер» page keeps its files under `server/` there).
static DATA_DIR: std::sync::RwLock<Option<std::path::PathBuf>> = std::sync::RwLock::new(None);

pub fn set_data_dir(dir: std::path::PathBuf) {
    if let Ok(mut d) = DATA_DIR.write() {
        *d = Some(dir);
    }
}

fn data_dir() -> Option<std::path::PathBuf> {
    DATA_DIR.read().ok().and_then(|d| d.clone())
}

static STUDIO_APP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Runs this build as the WolfStudio app (set once at start-up, before the first frame).
pub fn set_studio_app(on: bool) {
    STUDIO_APP.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn is_studio_app() -> bool {
    STUDIO_APP.load(std::sync::atomic::Ordering::Relaxed)
}

/// WolfServer worlds heard on the local network (name, address, players, max), kept fresh by
/// the app while the studio's launcher is open.
static NEARBY: std::sync::Mutex<Vec<(String, String, usize, usize)>> = std::sync::Mutex::new(Vec::new());

pub fn set_nearby(list: Vec<(String, String, usize, usize)>) {
    if let Ok(mut n) = NEARBY.lock() {
        *n = list;
    }
}

/// Tabs of the bottom bar: the game has no studio tab any more, the studio app only has its
/// projects and settings.
fn tabs() -> &'static [(Tab, &'static str)] {
    if is_studio_app() {
        &[(Tab::Studio, "Проекты"), (Tab::Settings, "Настройки")]
    } else {
        // One app: play, build and host — settings are under the gear on top and in Профиль.
        // No separate «Редактор»: tap a mini-game card on «Сервер» to edit it.
        &[(Tab::Home, "Главная"), (Tab::Server, "Сервер"), (Tab::Skins, "Профиль")]
    }
}

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
    let mode_id = Id::new("launcher-server-place-mode");
    let mut place_mode: bool = ctx.data(|d| d.get_temp(mode_id)).unwrap_or(false);
    let tab_id = Id::new("launcher-tab-v2");
    let mut tab: usize = ctx.data(|d| d.get_temp(tab_id)).unwrap_or(0);
    let tabs = tabs();
    tab = tab.min(tabs.len() - 1);

    // ---- Full-bleed background ----
    // The phone's safe margins (and the strip under the status bar) are outside egui's
    // screen; paint the page and bar colours out to the real edges so nothing looks cut.
    {
        let screen = ctx.screen_rect();
        let full = screen.expand(160.0);
        let bg = ctx.layer_painter(egui::LayerId::background()).with_clip_rect(full);
        // The living world from the loading splash, dimmed so cards stay readable.
        crate::main_menu::world_backdrop(&bg, full, ctx.input(|i| i.time) as f32);
        bg.rect_filled(full, 0.0, p.bg.gamma_multiply(if p.dark { 0.38 } else { 0.30 }));
        // Aero glass header with the page menu across the top (no bottom dock).
        let top = Rect::from_min_max(full.min, Pos2::new(full.right(), screen.top() + HEAD_H + NAV_H));
        bg.rect_filled(top, 0.0, glass_tint(&p));
        bg.add(grad_shape(Rect::from_min_max(Pos2::new(top.left(), screen.top()), Pos2::new(top.right(), screen.top() + HEAD_H * 0.7)), 0.0, 0.0, glass_gloss(&p), Color32::TRANSPARENT));
        bg.line_segment([Pos2::new(full.left(), screen.top() + HEAD_H), Pos2::new(full.right(), screen.top() + HEAD_H)], Stroke::new(1.0_f32, glass_rim(&p).gamma_multiply(0.4)));
        bg.line_segment([Pos2::new(full.left(), top.bottom()), Pos2::new(full.right(), top.bottom())], Stroke::new(1.0_f32, glass_rim(&p)));
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }

    // ---- Header: logo and title; theme and updates on the right ----
    egui::TopBottomPanel::top("launcher-head")
        .exact_height(HEAD_H)
        .frame(Frame::none().inner_margin(Margin::symmetric(14.0, 8.0)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                icons.paint(ui.painter(), r, icon::LOGO, Color32::WHITE);
                ui.add_space(4.0);
                ui.label(egui::RichText::new(if is_studio_app() { "WolfStudio" } else { "WolfCraft" }).font(bold_font(20.0)).color(p.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if round_icon(ui, icons, &p, if dark { icon::SUN } else { icon::MOON }).on_hover_text(if dark { "Светлая тема" } else { "Тёмная тема" }).clicked() {
                        action = MenuAction::ToggleTheme;
                    }
                    if round_icon(ui, icons, &p, icon::REFRESH).on_hover_text("Проверить обновления").clicked() {
                        action = MenuAction::CheckUpdates;
                    }
                });
            });
        });

    // ---- Page menu under the header: a sliding row of pills (icon + name) ----
    egui::TopBottomPanel::top("launcher-nav")
        .exact_height(NAV_H)
        .frame(Frame::none().inner_margin(Margin::symmetric(10.0, 6.0)))
        .show(ctx, |ui| {
            egui::ScrollArea::horizontal().id_salt("launcher-nav-strip").auto_shrink([false, true]).drag_to_scroll(true).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    for (i, (t, label)) in tabs.iter().enumerate() {
                        let on = tab == i;
                        let col = if on { Color32::WHITE } else { p.text };
                        let galley = ui.painter().layout_no_wrap(label.to_string(), bold_font(14.0), col);
                        let (r, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 50.0, 38.0), Sense::click());
                        if on {
                            gloss_capsule(ui, r, p.accent);
                        } else {
                            let hot = resp.hovered() || resp.is_pointer_button_down_on();
                            ui.painter().rect_filled(r, Rounding::same(19.0), if hot { glass_gloss(&p).gamma_multiply(2.0) } else { Color32::from_black_alpha(if p.dark { 50 } else { 16 }) });
                            ui.painter().rect_stroke(r, Rounding::same(19.0), Stroke::new(1.0, glass_rim(&p).gamma_multiply(0.6)));
                        }
                        nav_icon(ui, icons, *t, Rect::from_center_size(Pos2::new(r.left() + 22.0, r.center().y), Vec2::splat(18.0)), col);
                        let pos = Pos2::new(r.left() + 38.0, r.center().y);
                        if on {
                            shadow_text(ui.painter(), pos, Align2::LEFT_CENTER, label, bold_font(14.0), col);
                        } else {
                            ui.painter().text(pos, Align2::LEFT_CENTER, *label, bold_font(14.0), col);
                        }
                        if resp.clicked() {
                            tab = i;
                        }
                    }
                });
            });
        });

    // ---- Toolbar: each page has its own (title and its own buttons) ----
    let page = tabs[tab].0;
    if page != Tab::Home {
    egui::TopBottomPanel::top("launcher-toolbar")
        .exact_height(TOOLBAR_H)
        .frame(Frame::none().fill(glass_tint(&p).gamma_multiply(0.7)).inner_margin(Margin::symmetric(16.0, 8.0)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                if page == Tab::Home {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                    icons.paint(ui.painter(), r, icon::LOGO, Color32::WHITE);
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(if is_studio_app() { "WolfStudio" } else { "WolfCraft" }).font(bold_font(20.0)).color(p.text));
                } else if page == Tab::Server {
                    for (place, title) in [(false, "Сервер"), (true, "Мини-игры")] {
                        if pill_button(ui, icons, None, title, if place_mode == place { ButtonKind::Primary } else { ButtonKind::Secondary }, 86.0, true).clicked() {
                            place_mode = place;
                            ctx.data_mut(|d| d.insert_temp(Id::new("server-page-tab"), 0usize));
                        }
                    }
                } else {
                    let title = match page {
                        Tab::Studio => "Редактор",
                        Tab::Server => "Сервер",
                        Tab::Skins => "Профиль",
                        Tab::Settings => "Настройки",
                        Tab::Home => "",
                    };
                    ui.label(egui::RichText::new(title).font(bold_font(21.0)).color(p.text));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if matches!(page, Tab::Home | Tab::Skins | Tab::Settings) {
                        let update = if ui.ctx().screen_rect().width() > 480.0 {
                            pill_button(ui, icons, Some(icon::REFRESH), "Обновить", ButtonKind::Secondary, 0.0, true)
                        } else {
                            round_icon(ui, icons, &p, icon::REFRESH).on_hover_text("Обновить приложение")
                        };
                        if update.clicked() { action = MenuAction::CheckUpdates; }
                    }
                    match page {
                        Tab::Home => {
                            if round_icon(ui, icons, &p, icon::GEAR).clicked() {
                                action = MenuAction::OpenSettings;
                            }
                            if round_icon(ui, icons, &p, if dark { icon::SUN } else { icon::MOON }).clicked() {
                                action = MenuAction::ToggleTheme;
                            }
                            let (r, _) = ui.allocate_exact_size(Vec2::new(64.0, 26.0), Sense::hover());
                            ui.painter().rect_stroke(r, Rounding::same(13.0), Stroke::new(1.5_f32, p.accent));
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, format!("v{version}"), FontId::proportional(12.0), p.accent);
                        }
                        Tab::Studio => {
                            // «+ Новый» opens the new-place form on the page.
                            if pill_button(ui, icons, Some(icon::PLUS), "", ButtonKind::Primary, 44.0, true).on_hover_text("Новая мини-игра").clicked() {
                                let n = worlds.iter().filter(|w| w.cfg.studio_project || is_studio_app()).count() + 1;
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(Id::new("studio-new-form"), Some((format!("Мини-игра {n}"), 0u8)));
                                    d.insert_temp(Id::new("studio-page-tab"), 1usize);
                                });
                            }
                        }
                        Tab::Server => {
                            if round_icon(ui, icons, &p, icon::SEARCH).clicked() {
                                action = MenuAction::OpenNetwork;
                            }
                            if round_icon(ui, icons, &p, icon::PLUS).on_hover_text(if place_mode { "Новая мини-игра" } else { "Новый сетевой мир" }).clicked() {
                                action = if place_mode { MenuAction::NewStudioProject } else { MenuAction::HostServer(None) };
                            }
                        }
                        Tab::Skins | Tab::Settings => {
                            if round_icon(ui, icons, &p, icon::GEAR).clicked() {
                                action = MenuAction::OpenSettings;
                            }
                            if round_icon(ui, icons, &p, if dark { icon::SUN } else { icon::MOON }).clicked() {
                                action = MenuAction::ToggleTheme;
                            }
                        }
                    }
                });
            });
        });

    }

    // «Сервер»: its own tabs right under the toolbar.
    let mut server_tab = 0;
    if page == Tab::Server {
        // Дашборд, Плагины, Моды…: a pill menu along the bottom.
        egui::TopBottomPanel::bottom("launcher-server-tabs")
            .exact_height(60.0)
            .frame(Frame::none().fill(glass_tint(&p)).inner_margin(Margin::symmetric(10.0, 8.0)).stroke(Stroke::new(1.0_f32, glass_rim(&p))))
            .show(ctx, |ui| {
                server_tab = crate::server_page::tab_bar(ui, &p);
            });
    }

    ctx.data_mut(|d| d.insert_temp(tab_id, tab));

    // ---- Page ----
    egui::CentralPanel::default().frame(Frame::none().inner_margin(Margin::symmetric(16.0, 14.0))).show(ctx, |ui| {
        egui::ScrollArea::vertical().id_salt(("launcher-page", tab)).auto_shrink([false, false]).drag_to_scroll(true).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            match tabs[tab].0 {
                Tab::Home => home(ui, icons, &p, planet, version, nickname, dark, &mut action),
                Tab::Settings => launcher_settings(ui, icons, &p, version, dark, &mut action),
                Tab::Studio => studio(ui, icons, &p, version, worlds, &mut action),
                Tab::Server => match (server_tab, data_dir()) {
                    (0, _) | (_, None) => server(ui, icons, &p, worlds, place_mode, &mut action),
                    (t, Some(d)) => crate::server_page::page(ui, icons, &p, &crate::server_page::profile_data(&d, place_mode), t),
                },
                Tab::Skins => {
                    profile(ui, icons, &p, skin, nickname, worlds, &mut action);
                    if !is_studio_app() {
                        ui.add_space(6.0);
                        launcher_settings(ui, icons, &p, version, dark, &mut action);
                    }
                }
            }
            ui.add_space(8.0);
        });
    });
    ctx.data_mut(|d| d.insert_temp(mode_id, place_mode));
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
            pt.rect_filled(Rect::from_min_max(Pos2::new(c.x - 2.5, c.y + s * 0.3), Pos2::new(c.x + 2.5, c.y + s)), Rounding::same(10.0), col);
        }
        Tab::Settings => icons.paint(pt, r, icon::GEAR, col),
        Tab::Studio => icons.paint(pt, r, icon::HAMMER, col),
        Tab::Server => icons.paint(pt, r, icon::GLOBE, col),
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
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).font(bold_font(18.0)).color(p.text));
        if !more.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(more).size(13.0).color(p.accent));
            });
        }
    });
    ui.add_space(4.0);
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

fn home(ui: &mut Ui, icons: &GuiIcons, p: &Palette, planet: Option<&TextureHandle>, version: &str, nickname: &str, dark: bool, action: &mut MenuAction) {
    let _ = (planet, dark);
    // Under the page menu: the releases first,
    section(ui, p, "Релизы", "все версии");
    egui::ScrollArea::horizontal().id_salt("launcher-releases").auto_shrink([false, true]).drag_to_scroll(true).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            for (i, r) in RELEASES.iter().enumerate() {
                release_card(ui, p, r, i == 0);
            }
        });
    });
    // then the news,
    section(ui, p, "Новости", "");
    for n in &NEWS {
        news_card(ui, icons, p, n);
        ui.add_space(2.0);
    }
    // and the play stack at the end: the version and «Играть».
    ui.add_space(10.0);
    play_banner(ui, icons, p, version, nickname, action);
    ui.add_space(8.0);
}

/// Aero segmented tabs: a glass groove with the chosen tab as a glossy accent capsule.
#[allow(dead_code)]
fn aero_tabs(ui: &mut Ui, p: &Palette, names: &[&str], cur: usize) -> Option<usize> {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 38.0), Sense::hover());
    let pt = ui.painter();
    pt.rect_filled(rect, Rounding::same(19.0), Color32::from_black_alpha(if p.dark { 70 } else { 28 }));
    pt.rect_stroke(rect, Rounding::same(19.0), Stroke::new(1.0, glass_rim(p).gamma_multiply(0.7)));
    let tw = w / names.len() as f32;
    let mut out = None;
    for (i, n) in names.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(rect.left() + tw * i as f32, rect.top()), Vec2::new(tw, rect.height())).shrink(3.0);
        let resp = ui.interact(r, Id::new(("aero-tab", n)), Sense::click());
        if i == cur {
            gloss_capsule(ui, r, p.accent);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, Rounding::same(r.height() / 2.0), glass_gloss(p));
        }
        if i == cur {
            shadow_text(ui.painter(), r.center(), Align2::CENTER_CENTER, n, bold_font(14.0), Color32::WHITE);
        } else {
            ui.painter().text(r.center(), Align2::CENTER_CENTER, *n, bold_font(14.0), p.text);
        }
        if resp.clicked() {
            out = Some(i);
        }
    }
    out
}

/// A glossy Aero capsule in colour `c`: lighter at the top, a white shine over the upper half,
/// a bright rim.
fn gloss_capsule(ui: &Ui, r: Rect, c: Color32) {
    let rad = r.height() / 2.0;
    let pt = ui.painter();
    pt.rect_filled(r.translate(Vec2::new(0.0, 2.0)), Rounding::same(rad), Color32::from_black_alpha(45));
    // Body: lighter at the top, deeper at the bottom.
    grad_rect(pt, r, rad, rad, mix(c, Color32::WHITE, 0.18), mix(c, Color32::BLACK, 0.22));
    // Shine: a soft reflection over the upper half that fades out before the middle.
    let shine = Rect::from_min_max(r.min + Vec2::new(rad * 0.6, 2.5), Pos2::new(r.right() - rad * 0.6, r.center().y));
    grad_rect(pt, shine, shine.height() * 0.7, shine.height() * 0.3, Color32::from_white_alpha(70), Color32::TRANSPARENT);
    pt.rect_stroke(r, Rounding::same(rad), Stroke::new(1.0, mix(c, Color32::BLACK, 0.4)));
    pt.rect_stroke(r.shrink(1.0), Rounding::same(rad - 1.0), Stroke::new(1.0, Color32::from_white_alpha(70)));
}

/// A glossy Aero button: accent when `on`, clear glass otherwise.
#[allow(dead_code)]
fn aero_button(ui: &mut Ui, p: &Palette, icons: &GuiIcons, ic: Option<u16>, label: &str, on: bool) -> egui::Response {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 42.0), Sense::click());
    let press = resp.is_pointer_button_down_on();
    if on {
        gloss_capsule(ui, r, if press { mix(p.accent, Color32::BLACK, 0.15) } else { p.accent });
    } else {
        let pt = ui.painter();
        pt.rect_filled(r, Rounding::same(21.0), Color32::from_black_alpha(if p.dark { 60 } else { 18 }));
        let shine = Rect::from_min_max(r.min + Vec2::new(14.0, 2.5), Pos2::new(r.right() - 14.0, r.center().y));
        grad_rect(pt, shine, shine.height() * 0.7, shine.height() * 0.3, glass_gloss(p).gamma_multiply(if resp.hovered() { 1.8 } else { 1.2 }), Color32::TRANSPARENT);
        pt.rect_stroke(r, Rounding::same(21.0), Stroke::new(1.0, glass_rim(p).gamma_multiply(0.8)));
    }
    let col = if on { Color32::WHITE } else { p.text };
    let galley = ui.painter().layout_no_wrap(label.to_string(), bold_font(15.0), col);
    let total = galley.size().x + if ic.is_some() { 26.0 } else { 0.0 };
    let x = r.center().x - total / 2.0;
    if let Some(ic) = ic {
        icons.paint(ui.painter(), Rect::from_min_size(Pos2::new(x, r.center().y - 9.0), Vec2::splat(18.0)), ic, col);
    }
    let pos = Pos2::new(x + total - galley.size().x, r.center().y);
    if on {
        shadow_text(ui.painter(), pos, Align2::LEFT_CENTER, label, bold_font(15.0), col);
    } else {
        ui.painter().text(pos, Align2::LEFT_CENTER, label, bold_font(15.0), col);
    }
    resp
}

/// The block under the slider: who plays, which version, and a big glossy «Играть».
fn play_banner(ui: &mut Ui, icons: &GuiIcons, p: &Palette, version: &str, nickname: &str, action: &mut MenuAction) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 170.0), Sense::hover());
    let pt = ui.painter();
    pt.rect_filled(rect.translate(Vec2::new(0.0, 4.0)).expand(1.5), Rounding::same(20.0), Color32::from_black_alpha(55));
    // Deep blue Aero glass with a soft sheen fading out at the top.
    grad_rect(pt, rect, 20.0, 20.0, Color32::from_rgba_unmultiplied(48, 110, 190, 235), Color32::from_rgba_unmultiplied(16, 40, 86, 240));
    let shine = Rect::from_min_max(rect.min + Vec2::new(2.0, 2.0), Pos2::new(rect.right() - 2.0, rect.top() + 58.0));
    grad_rect(pt, shine, 18.0, 0.0, Color32::from_white_alpha(46), Color32::TRANSPARENT);
    pt.rect_stroke(rect.shrink(1.0), Rounding::same(19.0), Stroke::new(1.0, Color32::from_white_alpha(80)));
    pt.rect_stroke(rect, Rounding::same(20.0), Stroke::new(1.0, Color32::from_black_alpha(70)));
    let x = rect.left() + 18.0;
    pt.text(Pos2::new(x, rect.top() + 16.0), Align2::LEFT_TOP, "ГОТОВЫ?", FontId::proportional(11.0), Color32::from_rgb(170, 215, 255));
    shadow_text(pt, Pos2::new(x, rect.top() + 32.0), Align2::LEFT_TOP, &format!("Играть как {nickname}"), bold_font(20.0), Color32::WHITE);
    // Version picker and the button, inside the banner.
    let row = Rect::from_min_max(Pos2::new(x, rect.bottom() - 92.0), Pos2::new(rect.right() - 18.0, rect.bottom() - 14.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::top_down(egui::Align::Min)));
    child.spacing_mut().item_spacing.y = 8.0;
    // The version box as dark glass inside the banner.
    {
        let v = &mut child.visuals_mut().widgets;
        for w in [&mut v.inactive, &mut v.hovered, &mut v.active, &mut v.open] {
            w.weak_bg_fill = Color32::from_black_alpha(80);
            w.bg_fill = Color32::from_black_alpha(80);
            w.bg_stroke = Stroke::new(1.0, Color32::from_white_alpha(60));
            w.rounding = Rounding::same(10.0);
        }
    }
    egui::ComboBox::from_id_salt("launcher-version")
        .width(row.width())
        .selected_text(egui::RichText::new(format!("WolfCraft {version}")).color(Color32::WHITE))
        .show_ui(&mut child, |ui| {
            ui.label(format!("{version} — установлена"));
            ui.separator();
            ui.label("Новые версии — кнопкой «Проверить» в блоке «Обновления»");
        });
    let (br, resp) = child.allocate_exact_size(Vec2::new(row.width(), 44.0), Sense::click());
    gloss_capsule(&child, br, if resp.is_pointer_button_down_on() { Color32::from_rgb(52, 150, 48) } else { Color32::from_rgb(62, 172, 56) });
    let galley = child.painter().layout_no_wrap("Играть".to_string(), bold_font(18.0), Color32::WHITE);
    let tw = galley.size().x + 28.0;
    let gx = br.center().x - tw / 2.0;
    icons.paint(child.painter(), Rect::from_min_size(Pos2::new(gx, br.center().y - 9.0), Vec2::splat(18.0)).translate(Vec2::new(0.0, 1.2)), icon::PLAY, Color32::from_black_alpha(110));
    icons.paint(child.painter(), Rect::from_min_size(Pos2::new(gx, br.center().y - 9.0), Vec2::splat(18.0)), icon::PLAY, Color32::WHITE);
    shadow_text(child.painter(), Pos2::new(gx + 28.0, br.center().y), Align2::LEFT_CENTER, "Играть", bold_font(18.0), Color32::WHITE);
    if resp.clicked() {
        *action = MenuAction::LaunchVersion(version.to_string());
    }
}

#[allow(dead_code)]
fn banner_slider(ui: &mut Ui, p: &Palette, planet: Option<&TextureHandle>, action: &mut MenuAction) {
    let ctx = ui.ctx().clone();
    let w = ui.available_width();
    // Phones held upright get a taller banner: the text and the planet each have room.
    let narrow = w < 480.0;
    let h = if narrow { (w * 0.62).clamp(220.0, 270.0) } else { (w * 0.36).clamp(180.0, 220.0) };
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
                // Upright: a smaller planet tucked into the lower right corner, under the text.
                let (d, c) = if narrow {
                    let d = r.width() * 0.58;
                    (d, Pos2::new(r.right() - d * 0.22, r.bottom() - d * 0.12))
                } else {
                    let d = r.height() * 1.25;
                    (d, Pos2::new(r.right() - d * 0.32, r.center().y + d * 0.18))
                };
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
        let text_w = if r.width() > 480.0 { r.width() * 0.62 } else if b.planet { r.width() * 0.6 } else { r.width() - 48.0 };
        // A soft shade behind the text so it reads over the planet.
        let shade = Rect::from_min_max(r.left_top(), Pos2::new(r.left() + text_w + 36.0, r.bottom()));
        let mut sm = egui::Mesh::default();
        let dark = Color32::from_black_alpha(90);
        sm.colored_vertex(shade.left_top(), dark);
        sm.colored_vertex(shade.right_top(), Color32::TRANSPARENT);
        sm.colored_vertex(shade.right_bottom(), Color32::TRANSPARENT);
        sm.colored_vertex(shade.left_bottom(), dark);
        sm.add_triangle(0, 1, 2);
        sm.add_triangle(0, 2, 3);
        clip.add(egui::Shape::mesh(sm));
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
    // Aero: a sheen over the upper half and a bright rim; the square corners are rounded off
    // with the page backdrop's tint.
    let shine = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + rect.height() * 0.42));
    grad_rect(ui.painter(), shine, 14.0, 0.0, Color32::from_white_alpha(34), Color32::TRANSPARENT);
    ui.painter().rect_stroke(rect, Rounding::same(14.0), Stroke::new(1.0_f32, glass_rim(p)));
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
    // Upright phones: wider cards, the next one peeks in from the right (swipe for more).
    let cw = if ui.ctx().screen_rect().width() < 480.0 { (ui.ctx().screen_rect().width() - 32.0) * 0.8 } else { 210.0 };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(cw, 176.0), Sense::hover());
    let pt = ui.painter();
    pt.rect_filled(rect, Rounding::same(12.0), glass_tint(p));
    pt.rect_stroke(rect, Rounding::same(12.0), Stroke::new(1.0_f32, if latest { p.accent } else { glass_rim(p) }));
    let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 54.0));
    let (a, b) = if latest { (p.accent, p.accent.gamma_multiply(0.45)) } else { (Color32::from_rgb(60, 66, 80), Color32::from_rgb(36, 40, 50)) };
    let shape = egui::epaint::RectShape::filled(head, Rounding { nw: 12.0, ne: 12.0, sw: 0.0, se: 0.0 }, a);
    pt.add(shape);
    grad_rect(pt, Rect::from_min_max(head.min, Pos2::new(head.right(), head.bottom() - 6.0)), 12.0, 0.0, Color32::from_white_alpha(50), Color32::TRANSPARENT);
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
    glass_card(ui, p, 12.0, Margin::same(12.0), Stroke::new(1.0_f32, p.outline), |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(52.0), Sense::hover());
            gradient(ui, r, n.color, n.color.gamma_multiply(0.55));
            icons.paint(ui.painter(), r.shrink(13.0), n.icon, Color32::WHITE);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                // Title, then the date on its own line: nothing squeezes on a narrow screen.
                ui.label(egui::RichText::new(n.title).font(bold_font(15.0)).color(p.text));
                ui.label(egui::RichText::new(n.date).size(11.5).color(if p.dark { Color32::from_rgb(140, 195, 255) } else { mix(p.accent, Color32::BLACK, 0.2) }));
                ui.label(egui::RichText::new(n.text).size(13.0).color(p.text_muted));
            });
        });
    });
}

// ---------------------------------------------------------------------------------------------
// Игра

// ---------------------------------------------------------------------------------------------
// Студия

/// Studio home, like Roblox Studio: servers are places. Create one here (name and template),
/// it appears in the list; tap it to open the editor scene; «Запустить» publishes it as a
/// running server. Worlds of WolfServer in the network can be edited live too.
/// Сервер: host one of your worlds (or a place) for friends, or join / edit someone's server.
fn server(ui: &mut Ui, icons: &GuiIcons, p: &Palette, worlds: &[crate::main_menu::WorldEntry], place_mode: bool, action: &mut MenuAction) {
    let addr_id = Id::new(("launcher-server-addr", place_mode));
    let mut addr: String = ui.ctx().data(|d| d.get_temp(addr_id)).unwrap_or_default();
    let card = |ui: &mut Ui, add: &mut dyn FnMut(&mut Ui)| {
        glass_card(ui, p, 18.0, Margin::same(16.0), Stroke::new(1.0_f32, p.outline), |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    };
    // Statistics of this profile (server or mini-games each have their own).
    if let Some(d) = data_dir() {
        let prof = crate::server_page::profile_data(&d, place_mode);
        let stats = crate::server_page::read_stats(&prof);
        let count = |f: &str| std::fs::read_dir(crate::server_page::root(&prof).join(f)).map(|r| r.flatten().count()).unwrap_or(0);
        stats_slider(ui, p, &stats, place_mode, count("plugins"), count("modules"));
    }
    // Join or edit by code / address (the page title is in the toolbar).
    card(ui, &mut |ui| {
        ui.label(egui::RichText::new(if place_mode { "ВОЙТИ В МИНИ-ИГРУ" } else { "ВОЙТИ НА СЕРВЕР" }).size(11.5).color(p.text_muted));
        let w = ui.available_width();
        crate::ui_components::text_input(ui, icons, &mut addr, "Код K7QX2 или адрес 192.168.0.10", w, Some(icon::GLOBE));
        ui.add_space(6.0);
        let ready = !addr.trim().is_empty();
        ui.horizontal(|ui| {
            let half = (w - 8.0) / 2.0;
            if pill_button(ui, icons, Some(icon::PLAY), "Играть", ButtonKind::Primary, half, ready).clicked() {
                *action = MenuAction::JoinServer(addr.trim().to_string());
            }
            if pill_button(ui, icons, Some(icon::HAMMER), "Редактировать", ButtonKind::Secondary, half, ready).clicked() {
                *action = MenuAction::EditServerInStudio(addr.trim().to_string());
            }
        });
        ui.add_space(4.0);
        if pill_button(ui, icons, Some(icon::SEARCH), "Сервера рядом (Wi‑Fi)", ButtonKind::Secondary, w, true).clicked() {
            *action = MenuAction::OpenNetwork;
        }
    });
    // Host: a world or a place, opened for friends (a code appears in the chat).
    card(ui, &mut |ui| {
        ui.label(egui::RichText::new(if place_mode { "ЗАПУСТИТЬ МИНИ-ИГРУ" } else { "ЗАПУСТИТЬ СВОЙ СЕРВЕР" }).size(11.5).color(p.text_muted));
        ui.label(egui::RichText::new(if place_mode { "Выберите мини-игру — в чате появится код входа для друзей." } else { "Выберите мир — в чате появится код входа для друзей." }).size(13.0).color(p.text_muted));
        ui.add_space(4.0);
        for w in worlds.iter().filter(|w| w.cfg.studio_project == place_mode).take(40) {
            let row = ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
                match &w.preview {
                    Some(t) => {
                        ui.painter().image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                    }
                    None => {
                        ui.painter().rect_filled(r, Rounding::same(8.0), p.bg);
                    }
                }
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&w.cfg.name).font(bold_font(15.0)).color(p.text));
                    ui.label(egui::RichText::new(if w.cfg.studio_project { "Мини-игра · нажмите, чтобы редактировать" } else { "Мир" }).size(12.0).color(p.text_muted));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if pill_button(ui, icons, Some(icon::GLOBE), "Запустить", ButtonKind::Primary, 120.0, true).clicked() {
                        *action = MenuAction::HostServer(Some(w.cfg.id.clone()));
                    }
                });
            });
            // Tap the mini-game itself (not «Запустить») to open it in the editor.
            if place_mode {
                let tap = ui.interact(row.response.rect, Id::new(("place-card", &w.cfg.id)), Sense::click());
                if tap.clicked() && matches!(action, MenuAction::None) {
                    *action = MenuAction::OpenStudio(w.cfg.id.clone());
                }
            }
            ui.add_space(4.0);
        }
        if !worlds.iter().any(|w| w.cfg.studio_project == place_mode) {
            ui.label(egui::RichText::new(if place_mode { "Мини-игр пока нет — нажмите «Новая мини-игра»." } else { "Миров пока нет — создайте мир в «Главной»." }).size(13.0).color(p.text_muted));
        }
        if pill_button(ui, icons, Some(icon::PLUS), if place_mode { "Новая мини-игра" } else { "Новый сетевой мир" }, ButtonKind::Secondary, ui.available_width(), true).clicked() {
            *action = if place_mode { MenuAction::NewStudioProject } else { MenuAction::HostServer(None) };
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(addr_id, addr));
}

/// Dashboard banner: swipeable glass cards with charts — players online, launches per day,
/// hours hosted, content. Each profile (server / mini-games) has its own numbers.
fn stats_slider(ui: &mut Ui, p: &Palette, st: &crate::server_page::Stats, place: bool, plugins: usize, mods: usize) {
    let w = ui.available_width();
    let h = 190.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click_and_drag());
    let id = Id::new(("dash-slider", place));
    let n = 4usize;
    let mut cur: usize = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(0);
    let off_id = id.with("off");
    let mut off: f32 = ui.ctx().data(|d| d.get_temp(off_id)).unwrap_or(0.0);
    if resp.dragged() {
        off += resp.drag_delta().x;
    }
    if resp.drag_stopped() {
        if off < -w * 0.15 && cur + 1 < n {
            cur += 1;
        } else if off > w * 0.15 && cur > 0 {
            cur -= 1;
        }
        off = 0.0;
    }
    ui.ctx().data_mut(|d| {
        d.insert_temp(id, cur);
        d.insert_temp(off_id, off);
    });
    let now = st.samples.last().map(|s| s.0).unwrap_or(0).max(st.runs.last().copied().unwrap_or(0));
    let day = |t: u64| t / 86_400;
    let today = day(now.max(1));
    // Last 7 days: launches and hours hosted.
    let mut runs = [0f32; 7];
    let mut hours = [0f32; 7];
    for t in &st.runs {
        let k = today.saturating_sub(day(*t));
        if k < 7 {
            runs[6 - k as usize] += 1.0;
        }
    }
    for (t, _) in &st.samples {
        let k = today.saturating_sub(day(*t));
        if k < 7 {
            hours[6 - k as usize] += 10.0 / 3600.0;
        }
    }
    let players: Vec<f32> = st.samples.iter().rev().take(60).rev().map(|s| s.1 as f32).collect();
    let peak = st.samples.iter().map(|s| s.1).max().unwrap_or(0);
    let total_h: f32 = st.samples.len() as f32 * 10.0 / 3600.0;
    let accent = p.accent;
    let clip = ui.painter().with_clip_rect(rect);
    for k in 0..n {
        let r = rect.translate(Vec2::new((k as f32 - cur as f32) * (w + 12.0) + off, 0.0));
        if !r.intersects(rect) {
            continue;
        }
        let (top, bottom) = match k {
            0 => (Color32::from_rgb(40, 110, 200), Color32::from_rgb(18, 44, 96)),
            1 => (Color32::from_rgb(120, 70, 200), Color32::from_rgb(48, 24, 96)),
            2 => (Color32::from_rgb(30, 150, 120), Color32::from_rgb(12, 66, 58)),
            _ => (Color32::from_rgb(200, 120, 40), Color32::from_rgb(96, 50, 16)),
        };
        clip.add(grad_shape(r, 18.0, 18.0, top, bottom));
        clip.add(grad_shape(Rect::from_min_max(r.min + Vec2::splat(2.0), Pos2::new(r.right() - 2.0, r.top() + 60.0)), 16.0, 0.0, Color32::from_white_alpha(40), Color32::TRANSPARENT));
        clip.rect_stroke(r, Rounding::same(18.0), Stroke::new(1.0, Color32::from_white_alpha(110)));
        let x = r.left() + 16.0;
        let (label, big, note) = match k {
            0 => ("ИГРОКИ ОНЛАЙН", players.last().map(|v| format!("{v:.0}")).unwrap_or_else(|| "0".into()), format!("пик {peak}")),
            1 => ("ЗАПУСКИ ЗА НЕДЕЛЮ", format!("{:.0}", runs.iter().sum::<f32>()), format!("всего {}", st.runs.len())),
            2 => ("ЧАСЫ РАБОТЫ", format!("{:.1} ч", hours.iter().sum::<f32>()), format!("всего {total_h:.1} ч")),
            _ => (if place { "МИНИ-ИГРЫ: КОНТЕНТ" } else { "СЕРВЕР: КОНТЕНТ" }, format!("{}", plugins + mods), format!("плагинов {plugins} · модов {mods}")),
        };
        clip.text(Pos2::new(x, r.top() + 14.0), Align2::LEFT_TOP, label, FontId::proportional(11.0), Color32::from_white_alpha(200));
        shadow_text(&clip, Pos2::new(x, r.top() + 30.0), Align2::LEFT_TOP, &big, bold_font(30.0), Color32::WHITE);
        clip.text(Pos2::new(r.right() - 16.0, r.top() + 14.0), Align2::RIGHT_TOP, note, FontId::proportional(12.0), Color32::from_white_alpha(210));
        // The chart along the bottom of the card.
        let chart = Rect::from_min_max(Pos2::new(x, r.top() + 82.0), Pos2::new(r.right() - 16.0, r.bottom() - 26.0));
        clip.line_segment([chart.left_bottom(), chart.right_bottom()], Stroke::new(1.0, Color32::from_white_alpha(70)));
        match k {
            0 => {
                // Line chart of players (last 10 minutes of hosting).
                if players.len() >= 2 {
                    let max = players.iter().cloned().fold(1.0f32, f32::max);
                    let pts: Vec<Pos2> = players.iter().enumerate().map(|(i, v)| Pos2::new(chart.left() + chart.width() * i as f32 / (players.len() - 1) as f32, chart.bottom() - chart.height() * v / max)).collect();
                    let mut area = pts.clone();
                    area.push(chart.right_bottom());
                    area.push(chart.left_bottom());
                    let mut m = egui::Mesh::default();
                    for (i, p) in pts.iter().enumerate() {
                        let b = Pos2::new(p.x, chart.bottom());
                        m.colored_vertex(*p, Color32::from_white_alpha(70));
                        m.colored_vertex(b, Color32::from_white_alpha(0));
                        if i > 0 {
                            let j = (i * 2) as u32;
                            m.add_triangle(j - 2, j - 1, j);
                            m.add_triangle(j - 1, j + 1, j);
                        }
                    }
                    clip.add(egui::Shape::mesh(m));
                    clip.add(egui::Shape::line(pts, Stroke::new(2.5, Color32::WHITE)));
                } else {
                    clip.text(chart.center(), Align2::CENTER_CENTER, "Запустите — здесь появится график", FontId::proportional(12.5), Color32::from_white_alpha(190));
                }
            }
            1 | 2 => {
                let vals = if k == 1 { runs } else { hours };
                let max = vals.iter().cloned().fold(0.001f32, f32::max);
                let bw = chart.width() / 7.0;
                for (i, v) in vals.iter().enumerate() {
                    let hgt = (chart.height() * v / max).max(2.0);
                    let b = Rect::from_min_max(Pos2::new(chart.left() + bw * i as f32 + bw * 0.18, chart.bottom() - hgt), Pos2::new(chart.left() + bw * (i as f32 + 1.0) - bw * 0.18, chart.bottom()));
                    clip.add(grad_shape(b, 4.0, 0.0, Color32::from_white_alpha(if i == 6 { 240 } else { 170 }), Color32::from_white_alpha(70)));
                    let lab = ["-6", "-5", "-4", "-3", "-2", "вч", "сг"][i];
                    clip.text(Pos2::new(b.center().x, chart.bottom() + 4.0), Align2::CENTER_TOP, lab, FontId::proportional(10.0), Color32::from_white_alpha(170));
                }
            }
            _ => {
                // Two bars: plugins and mods.
                let max = (plugins.max(mods).max(1)) as f32;
                for (i, (name, v)) in [("Плагины", plugins), ("Моды", mods)].iter().enumerate() {
                    let y = chart.top() + 6.0 + i as f32 * 26.0;
                    clip.text(Pos2::new(chart.left(), y + 9.0), Align2::LEFT_CENTER, *name, FontId::proportional(12.5), Color32::WHITE);
                    let bar = Rect::from_min_size(Pos2::new(chart.left() + 70.0, y + 2.0), Vec2::new((chart.width() - 90.0) * *v as f32 / max, 14.0));
                    clip.rect_filled(Rect::from_min_size(bar.min, Vec2::new(chart.width() - 90.0, 14.0)), Rounding::same(7.0), Color32::from_white_alpha(30));
                    clip.add(grad_shape(bar, 7.0, 7.0, Color32::from_white_alpha(230), Color32::from_white_alpha(150)));
                    clip.text(Pos2::new(chart.right(), y + 9.0), Align2::RIGHT_CENTER, v.to_string(), bold_font(13.0), Color32::WHITE);
                }
            }
        }
    }
    // Dots.
    for i in 0..n {
        let c = Pos2::new(rect.center().x - (n as f32 - 1.0) * 7.0 + i as f32 * 14.0, rect.bottom() - 10.0);
        if i == cur {
            ui.painter().rect_filled(Rect::from_center_size(c, Vec2::new(16.0, 6.0)), Rounding::same(3.0), Color32::WHITE);
        } else {
            ui.painter().circle_filled(c, 3.0, Color32::from_white_alpha(110));
        }
    }
    let _ = accent;
    ui.add_space(4.0);
}

fn studio(ui: &mut Ui, icons: &GuiIcons, p: &Palette, version: &str, worlds: &[crate::main_menu::WorldEntry], action: &mut MenuAction) {
    let _ = version;
    let form_id = Id::new("studio-new-form");
    let mut form: Option<(String, u8)> = ui.ctx().data(|d| d.get_temp(form_id)).unwrap_or(None);
    // In the WolfStudio app every world is a server (its own folder, apart from game worlds).
    let projects: Vec<&crate::main_menu::WorldEntry> = worlds.iter().filter(|w| w.cfg.studio_project || is_studio_app()).collect();

    // «+» is in the page's toolbar; the toolbar also switches Серверы / Мини-игры.

    // Creating a server: name and template, right here in the menu (not in the editor).
    if let Some((name, template)) = &mut form {
        let mut close = false;
        glass_card(ui, p, 18.0, Margin::same(16.0), Stroke::new(1.5_f32, p.accent), |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("НОВАЯ МИНИ-ИГРА").size(11.5).color(p.text_muted));
            ui.add(egui::TextEdit::singleline(name).char_limit(32).hint_text("Название").desired_width(ui.available_width()));
            ui.label(egui::RichText::new("Пустая плоская основа с сеткой, без построек — как Baseplate в Roblox Studio").size(12.5).color(p.text_muted));
            *template = 0;
            ui.horizontal(|ui| {
                if pill_button(ui, icons, None, "Отмена", ButtonKind::Secondary, 120.0, true).clicked() {
                    close = true;
                }
                if pill_button(ui, icons, Some(icon::CHECK), "Создать", ButtonKind::Primary, ui.available_width(), !name.trim().is_empty()).clicked() {
                    *action = MenuAction::CreateStudioProject { name: name.trim().to_string(), template: *template };
                    close = true;
                }
            });
        });
        if close {
            form = None;
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(form_id, form.clone()));

    {
    if projects.is_empty() && form.is_none() {
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("Нажмите «+», чтобы создать мини-игру").size(13.5).color(p.text_muted));
        });
    }
    // Server cards: tap the card to open the editor scene.
    for w in projects {
        let name = w.cfg.name.strip_prefix("Проект").map(|rest| format!("Мини-игра{rest}")).unwrap_or_else(|| w.cfg.name.clone());
        let out = glass_card(ui, p, 16.0, Margin::same(12.0), Stroke::new(1.0_f32, p.outline), |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
                if let Some(t) = &w.preview {
                    ui.painter().image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                } else {
                    ui.painter().rect_filled(r, Rounding::same(10.0), p.bg);
                }
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&name).font(bold_font(16.0)).color(p.text));
                    let kind = match w.cfg.world_type {
                        wolfcraft_engine::config::WorldType::Baseplate => "Основа",
                        wolfcraft_engine::config::WorldType::Flat => "Плоский мир",
                        _ => "Планета",
                    };
                    ui.label(egui::RichText::new(format!("{kind} · нажмите, чтобы редактировать")).size(12.5).color(p.text_muted));
                    if pill_button(ui, icons, Some(icon::GLOBE), "Запустить", ButtonKind::Secondary, 130.0, true).clicked() {
                        *action = MenuAction::HostServer(Some(w.cfg.id.clone()));
                    }
                });
            });
        });
        let card = ui.interact(out.response.rect, Id::new(("studio-card", &w.cfg.id)), Sense::click());
        if card.clicked() && matches!(action, MenuAction::None) {
            *action = MenuAction::OpenStudio(w.cfg.id.clone());
        }
    }
    }

    // Worlds of WolfServer (phone or PC) in this network: they appear here by themselves as
    // soon as a server starts, and open with the studio tools — edits go straight to the server.
    // Servers running in the network: open them with the editor tools (edits go live).
    let nearby = NEARBY.lock().map(|n| n.clone()).unwrap_or_default();
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    if !nearby.is_empty() {
        section(ui, p, "Серверы в сети", "");
    }
    for (name, addr, players, max) in &nearby {
        glass_card(ui, p, 16.0, Margin::same(12.0), Stroke::new(1.0_f32, p.outline), |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                icons.paint(ui.painter(), r, icon::GLOBE, p.success);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(name).font(bold_font(15.0)).color(p.text));
                    ui.label(egui::RichText::new(format!("{addr} · игроков {players}/{max}")).size(12.0).color(p.text_muted));
                    if pill_button(ui, icons, Some(icon::HAMMER), "Открыть в студии", ButtonKind::Primary, 170.0, players < max).clicked() {
                        *action = MenuAction::EditServerInStudio(addr.clone());
                    }
                });
            });
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Скины

/// Профиль: the player card (skin, nickname you can edit, a few numbers) and the skin choice.
fn profile(ui: &mut Ui, icons: &GuiIcons, p: &Palette, skin: u8, nickname: &str, worlds: &[crate::main_menu::WorldEntry], action: &mut MenuAction) {
    use wolfcraft_engine::renderer::model::{SKIN_NAMES, skin_colors};
    let w = ui.available_width();
    // Player card: the figure on the left, name, skin and nickname field on the right.
    glass_card(ui, p, 18.0, Margin::same(16.0), Stroke::new(1.0_f32, p.outline), |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::new(96.0, 190.0), Sense::hover());
            gradient(ui, r, p.surface_hi, p.surface);
            figure(ui, Rect::from_center_size(r.center(), Vec2::new(84.0, 172.0)), skin_colors(skin));
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                let shown = if nickname.trim().is_empty() { "Игрок" } else { nickname.trim() };
                ui.add(egui::Label::new(egui::RichText::new(shown).font(bold_font(24.0)).color(p.text)).wrap());
                let skin_name = SKIN_NAMES.get(skin as usize).copied().unwrap_or("—");
                ui.add(egui::Label::new(egui::RichText::new(format!("Скин «{skin_name}» — виден в игре, от третьего лица и другим игрокам на сервере")).size(13.0).color(p.text_muted)).wrap());
                ui.add_space(6.0);
                ui.label(egui::RichText::new("НИК").size(11.5).color(p.text_muted));
                let id = Id::new("launcher-nick-edit");
                let mut text: String = ui.ctx().data(|d| d.get_temp(id)).unwrap_or_else(|| nickname.to_string());
                let resp = ui.add(egui::TextEdit::singleline(&mut text).char_limit(16).hint_text("Игрок").desired_width(ui.available_width()));
                ui.ctx().data_mut(|d| d.insert_temp(id, text.clone()));
                let changed = text.trim() != nickname.trim();
                if (resp.lost_focus() && changed) || (changed && pill_button(ui, icons, Some(icon::CHECK), "Сохранить ник", ButtonKind::Primary, ui.available_width(), true).clicked()) {
                    *action = MenuAction::SetNickname(text.trim().to_string());
                }
            });
        });
    });
    // A few numbers about the player.
    let worlds_n = worlds.iter().filter(|w| !w.cfg.studio_project).count();
    let projects = worlds.iter().filter(|w| w.cfg.studio_project).count();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let cw = (ui.available_width() - 20.0) / 3.0;
        for (num, label) in [(worlds_n.to_string(), "миров"), (projects.to_string(), "проектов"), (SKIN_NAMES.len().to_string(), "скинов")] {
            glass_card(ui, p, 14.0, Margin::symmetric(8.0, 10.0), Stroke::new(1.0_f32, p.outline), |ui| {
                ui.set_width(cw - 16.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(num).font(bold_font(22.0)).color(p.accent));
                    ui.label(egui::RichText::new(label).size(12.0).color(p.text_muted));
                });
            });
        }
    });
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

/// Настройки лаунчера: look, game settings, version and about — right in the launcher.
fn launcher_settings(ui: &mut Ui, icons: &GuiIcons, p: &Palette, version: &str, dark: bool, action: &mut MenuAction) {
    let card = |ui: &mut Ui, title: &str, body: &mut dyn FnMut(&mut Ui)| {
        glass_card(ui, p, 18.0, Margin::same(16.0), Stroke::new(1.0_f32, p.outline), |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(title.to_uppercase()).size(11.5).color(p.text_muted));
            ui.add_space(6.0);
            body(ui);
        });
    };
    let row = |ui: &mut Ui, k: &str, v: &str| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(k).color(p.text_muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(v).color(p.text));
            });
        });
    };
    card(ui, "Оформление", &mut |ui| {
        row(ui, "Тема", if dark { "Тёмная" } else { "Светлая" });
        if pill_button(ui, icons, Some(if dark { icon::SUN } else { icon::MOON }), if dark { "Светлая тема" } else { "Тёмная тема" }, ButtonKind::Secondary, ui.available_width(), true).clicked() {
            *action = MenuAction::ToggleTheme;
        }
    });
    card(ui, "Игра", &mut |ui| {
        ui.label(egui::RichText::new("Графика, управление, звук, дальность прорисовки").size(13.0).color(p.text_muted));
        if pill_button(ui, icons, Some(icon::GEAR), "Настройки игры", ButtonKind::Primary, ui.available_width(), true).clicked() {
            *action = MenuAction::OpenSettings;
        }
    });
    card(ui, "Версия", &mut |ui| {
        row(ui, "Установлена", &format!("WolfCraft {version}"));
        row(ui, "Канал", "Тест");
        ui.label(egui::RichText::new("Новые сборки ставятся поверх, миры и профиль сохраняются").size(12.5).color(p.text_muted));
    });
    card(ui, "О лаунчере", &mut |ui| {
        row(ui, "Лаунчер", if is_studio_app() { "WolfStudio" } else { "WolfCraft" });
        row(ui, "Сервер", "WolfStudio · WolfServer.exe");
    });
}

/// Flat front view of the character in the given skin colours.
fn figure(ui: &Ui, r: Rect, (skin, hair, jacket, sleeve, trousers): ([f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3])) {
    let c = |v: [f32; 3]| Color32::from_rgb((v[0] * 255.0) as u8, (v[1] * 255.0) as u8, (v[2] * 255.0) as u8);
    let u = r.width() / 8.0; // the body is 8 units wide, 16 tall (head 4, torso 6, legs 6)
    let x0 = r.center().x - 4.0 * u;
    let y0 = r.center().y - 8.0 * u;
    let pt = ui.painter();
    let b = |x: f32, y: f32, w: f32, h: f32, col: Color32| pt.rect_filled(Rect::from_min_size(Pos2::new(x0 + x * u, y0 + y * u), Vec2::new(w * u, h * u)), Rounding::same(10.0), col);
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



/// Linear mix of two colours (unmultiplied), `t` 0..1.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let (a, b) = (a.to_srgba_unmultiplied(), b.to_srgba_unmultiplied());
    let l = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(l(0), l(1), l(2), l(3))
}

/// A rounded rectangle filled with a vertical gradient (`r_top` rounds the upper corners,
/// `r_bottom` the lower ones). Used for every Aero shine: no hard edges anywhere.
fn grad_rect(pt: &egui::Painter, r: Rect, r_top: f32, r_bottom: f32, top: Color32, bottom: Color32) {
    pt.add(grad_shape(r, r_top, r_bottom, top, bottom));
}

fn grad_shape(r: Rect, r_top: f32, r_bottom: f32, top: Color32, bottom: Color32) -> egui::Shape {
    let (rt, rb) = (r_top.min(r.width() / 2.0).min(r.height()), r_bottom.min(r.width() / 2.0).min(r.height()));
    let mut pts: Vec<Pos2> = Vec::new();
    let arc = |pts: &mut Vec<Pos2>, c: Pos2, rad: f32, from: f32| {
        if rad <= 0.5 {
            pts.push(c);
            return;
        }
        for k in 0..=6 {
            let a = (from + k as f32 * 15.0).to_radians();
            pts.push(Pos2::new(c.x + rad * a.cos(), c.y + rad * a.sin()));
        }
    };
    arc(&mut pts, Pos2::new(r.left() + rt, r.top() + rt), rt, 180.0);
    arc(&mut pts, Pos2::new(r.right() - rt, r.top() + rt), rt, 270.0);
    arc(&mut pts, Pos2::new(r.right() - rb, r.bottom() - rb), rb, 0.0);
    arc(&mut pts, Pos2::new(r.left() + rb, r.bottom() - rb), rb, 90.0);
    let col = |p: Pos2| mix(top, bottom, (p.y - r.top()) / r.height().max(1.0));
    let mut m = egui::Mesh::default();
    m.colored_vertex(r.center(), col(r.center()));
    for p in &pts {
        m.colored_vertex(*p, col(*p));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        m.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    egui::Shape::mesh(m)
}

/// Text with a soft dark shadow under it, so it reads on glossy and light surfaces.
fn shadow_text(pt: &egui::Painter, pos: Pos2, align: Align2, text: &str, font: FontId, col: Color32) {
    pt.text(pos + Vec2::new(0.0, 1.2), align, text, font.clone(), Color32::from_black_alpha(110));
    pt.text(pos, align, text, font, col);
}

/// Aero glass: a see-through tinted pane (sky-blue in light, deep blue in dark).
fn glass_tint(p: &Palette) -> Color32 {
    if p.dark { Color32::from_rgba_unmultiplied(22, 36, 60, 165) } else { Color32::from_rgba_unmultiplied(225, 238, 252, 150) }
}

/// The glossy reflection on the upper half of a glass pane.
fn glass_gloss(p: &Palette) -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, if p.dark { 26 } else { 80 })
}

/// The bright rim round a glass pane.
fn glass_rim(p: &Palette) -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, if p.dark { 55 } else { 150 })
}

/// A card in the Aero glass style: soft shadow, a tinted see-through body, a gloss over the
/// upper half, a white inner rim and the given outer edge. Content is drawn on top.
fn glass_card<R>(ui: &mut Ui, p: &Palette, r: f32, margin: Margin, edge: Stroke, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let back = ui.painter().add(egui::Shape::Noop);
    let out = Frame::none().rounding(Rounding::same(r)).inner_margin(margin).show(ui, add);
    let rect = out.response.rect;
    let hover = out.response.hovered() as u8 as f32;
    // Shine over the upper part, fading out smoothly (no hard band across the content).
    let gloss = Rect::from_min_max(rect.min + Vec2::splat(1.0), Pos2::new(rect.right() - 1.0, rect.top() + (rect.height() * 0.55).min(80.0)));
    let shine = glass_gloss(p).gamma_multiply(0.9 + 0.4 * hover);
    let shapes = vec![
        egui::Shape::rect_filled(rect.translate(Vec2::new(0.0, 4.0)).expand(1.5), Rounding::same(r + 1.5), Color32::from_black_alpha(45)),
        egui::Shape::rect_filled(rect, Rounding::same(r), glass_tint(p)),
        grad_shape(gloss, (r - 1.0).max(0.0), 0.0, shine, Color32::TRANSPARENT),
        egui::Shape::rect_stroke(rect.shrink(1.0), Rounding::same((r - 1.0).max(0.0)), Stroke::new(1.0, glass_rim(p).gamma_multiply(0.7))),
        egui::Shape::rect_stroke(rect, Rounding::same(r), Stroke::new(edge.width, if edge.color == p.outline { Color32::from_black_alpha(60) } else { edge.color })),
    ];
    ui.painter().set(back, egui::Shape::Vec(shapes));
    out
}
