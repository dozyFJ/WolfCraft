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
const NAV_H: f32 = 64.0;

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
        &[(Tab::Home, "Главная"), (Tab::Studio, "Редактор"), (Tab::Server, "Сервер"), (Tab::Skins, "Профиль")]
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
        bg.rect_filled(full, 0.0, p.bg);
        bg.rect_filled(Rect::from_min_max(full.min, Pos2::new(full.right(), screen.top() + TOOLBAR_H)), 0.0, p.surface);
        bg.rect_filled(Rect::from_min_max(Pos2::new(full.left(), screen.bottom() - NAV_H), full.max), 0.0, p.surface);
        bg.line_segment([Pos2::new(full.left(), screen.top() + TOOLBAR_H), Pos2::new(full.right(), screen.top() + TOOLBAR_H)], Stroke::new(1.0_f32, p.outline));
        bg.line_segment([Pos2::new(full.left(), screen.bottom() - NAV_H), Pos2::new(full.right(), screen.bottom() - NAV_H)], Stroke::new(1.0_f32, p.outline));
    }

    // ---- Toolbar: each page has its own (title and its own buttons) ----
    let page = tabs[tab].0;
    egui::TopBottomPanel::top("launcher-toolbar")
        .exact_height(TOOLBAR_H)
        .frame(Frame::none().fill(p.surface).inner_margin(Margin::symmetric(16.0, 8.0)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                if page == Tab::Home {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                    icons.paint(ui.painter(), r, icon::LOGO, Color32::WHITE);
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(if is_studio_app() { "WolfStudio" } else { "WolfCraft" }).font(bold_font(20.0)).color(p.text));
                } else if page == Tab::Server {
                    for (place, title) in [(false, "Сервер"), (true, "Плейс")] {
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
                            if pill_button(ui, icons, Some(icon::PLUS), "Новый", ButtonKind::Primary, 110.0, true).clicked() {
                                let n = worlds.iter().filter(|w| w.cfg.studio_project || is_studio_app()).count() + 1;
                                ui.ctx().data_mut(|d| d.insert_temp(Id::new("studio-new-form"), Some((format!("Сервер {n}"), 0u8))));
                            }
                        }
                        Tab::Server => {
                            if round_icon(ui, icons, &p, icon::SEARCH).clicked() {
                                action = MenuAction::OpenNetwork;
                            }
                            if round_icon(ui, icons, &p, icon::PLUS).clicked() {
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

    // «Сервер»: its own tabs right under the toolbar.
    let mut server_tab = 0;
    if page == Tab::Server {
        egui::TopBottomPanel::top("launcher-server-tabs")
            .exact_height(46.0)
            .frame(Frame::none().fill(p.surface).inner_margin(Margin::symmetric(10.0, 4.0)).stroke(Stroke::new(1.0_f32, p.outline)))
            .show(ctx, |ui| {
                server_tab = crate::server_page::tab_bar(ui, &p);
            });
    }

    // ---- Bottom navigation ----
    egui::TopBottomPanel::bottom("launcher-nav")
        .exact_height(NAV_H)
        .frame(Frame::none().fill(p.surface).stroke(Stroke::new(1.0_f32, p.outline)))
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let w = rect.width() / tabs.len() as f32;
            for (i, (t, label)) in tabs.iter().enumerate() {
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
                    tab = i;
                }
            }
        });
    ctx.data_mut(|d| d.insert_temp(tab_id, tab));

    // Account and launch controls stay visible while Home scrolls.
    if tabs[tab].0 == Tab::Home {
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
                            ui.label("Обновить игру можно кнопкой «Обновить» в шапке");
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
            match tabs[tab].0 {
                Tab::Home => home(ui, icons, &p, planet, &mut action),
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

/// Studio home, like Roblox Studio: servers are places. Create one here (name and template),
/// it appears in the list; tap it to open the editor scene; «Запустить» publishes it as a
/// running server. Worlds of WolfServer in the network can be edited live too.
/// Сервер: host one of your worlds (or a place) for friends, or join / edit someone's server.
fn server(ui: &mut Ui, icons: &GuiIcons, p: &Palette, worlds: &[crate::main_menu::WorldEntry], place_mode: bool, action: &mut MenuAction) {
    let addr_id = Id::new(("launcher-server-addr", place_mode));
    let mut addr: String = ui.ctx().data(|d| d.get_temp(addr_id)).unwrap_or_default();
    let card = |ui: &mut Ui, add: &mut dyn FnMut(&mut Ui)| {
        Frame::none().fill(p.surface).rounding(Rounding::same(18.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    };
    // Join or edit by code / address (the page title is in the toolbar).
    card(ui, &mut |ui| {
        ui.label(egui::RichText::new(if place_mode { "ВОЙТИ В ПЛЕЙС" } else { "ВОЙТИ НА СЕРВЕР" }).size(11.5).color(p.text_muted));
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
        ui.label(egui::RichText::new(if place_mode { "ЗАПУСТИТЬ СВОЙ ПЛЕЙС" } else { "ЗАПУСТИТЬ СВОЙ СЕРВЕР" }).size(11.5).color(p.text_muted));
        ui.label(egui::RichText::new(if place_mode { "Выберите плейс — в чате появится код входа для друзей." } else { "Выберите мир — в чате появится код входа для друзей." }).size(13.0).color(p.text_muted));
        ui.add_space(4.0);
        for w in worlds.iter().filter(|w| w.cfg.studio_project == place_mode).take(40) {
            ui.horizontal(|ui| {
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
                    ui.label(egui::RichText::new(if w.cfg.studio_project { "Плейс" } else { "Мир" }).size(12.0).color(p.text_muted));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if pill_button(ui, icons, Some(icon::GLOBE), "Запустить", ButtonKind::Primary, 120.0, true).clicked() {
                        *action = MenuAction::HostServer(Some(w.cfg.id.clone()));
                    }
                });
            });
            ui.add_space(4.0);
        }
        if !worlds.iter().any(|w| w.cfg.studio_project == place_mode) {
            ui.label(egui::RichText::new(if place_mode { "Плейсов пока нет — создайте плейс в «Редакторе»." } else { "Миров пока нет — создайте мир в «Главной»." }).size(13.0).color(p.text_muted));
        }
        if pill_button(ui, icons, Some(icon::PLUS), if place_mode { "Новый плейс" } else { "Новый сетевой мир" }, ButtonKind::Secondary, ui.available_width(), true).clicked() {
            *action = if place_mode { MenuAction::NewStudioProject } else { MenuAction::HostServer(None) };
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(addr_id, addr));
}

fn studio(ui: &mut Ui, icons: &GuiIcons, p: &Palette, version: &str, worlds: &[crate::main_menu::WorldEntry], action: &mut MenuAction) {
    let _ = version;
    let form_id = Id::new("studio-new-form");
    let mut form: Option<(String, u8)> = ui.ctx().data(|d| d.get_temp(form_id)).unwrap_or(None);
    // In the WolfStudio app every world is a server (its own folder, apart from game worlds).
    let projects: Vec<&crate::main_menu::WorldEntry> = worlds.iter().filter(|w| w.cfg.studio_project || is_studio_app()).collect();

    // «+ Новый» is in the page's toolbar.

    // Creating a server: name and template, right here in the menu (not in the editor).
    if let Some((name, template)) = &mut form {
        let mut close = false;
        Frame::none().fill(p.surface).rounding(Rounding::same(18.0)).stroke(Stroke::new(1.5_f32, p.accent)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("НОВЫЙ СЕРВЕР").size(11.5).color(p.text_muted));
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

    if projects.is_empty() && form.is_none() {
        Frame::none().fill(p.surface).rounding(Rounding::same(18.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(18.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Пока нет серверов").font(bold_font(16.0)).color(p.text));
            ui.label(egui::RichText::new("Сервер — это ваша игра, как плейс в Roblox Studio. Создайте его, откройте в редакторе, стройте и пишите скрипт на Luau, потом «Запустить» — и друзья заходят.").size(13.0).color(p.text_muted));
        });
    }
    // Server cards: tap the card to open the editor scene.
    for w in projects {
        let name = w.cfg.name.strip_prefix("Проект").map(|rest| format!("Сервер{rest}")).unwrap_or_else(|| w.cfg.name.clone());
        let out = Frame::none().fill(p.surface).rounding(Rounding::same(16.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
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

    // Worlds of WolfServer (phone or PC) in this network: they appear here by themselves as
    // soon as a server starts, and open with the studio tools — edits go straight to the server.
    section(ui, p, "Миры на серверах", "");
    let nearby = NEARBY.lock().map(|n| n.clone()).unwrap_or_default();
    ui.horizontal(|ui| {
        let t = ui.ctx().input(|i| i.time) as f32;
        let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
        ui.painter().circle_stroke(r.center(), 3.0 + (t * 1.5).fract() * 6.0, Stroke::new(1.5_f32, p.accent.gamma_multiply(1.0 - (t * 1.5).fract())));
        ui.painter().circle_filled(r.center(), 2.5, p.accent);
        ui.label(egui::RichText::new("Запущенные серверы в вашей сети появляются здесь сами").size(13.0).color(p.text_muted));
    });
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    if nearby.is_empty() {
        ui.label(egui::RichText::new("Пока нет. Запустите сервер на вкладке «Сервер» в WolfStudio или WolfServer.exe на ПК.").size(13.0).color(p.text_muted));
    }
    for (name, addr, players, max) in &nearby {
        Frame::none().fill(p.surface).rounding(Rounding::same(16.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(12.0)).show(ui, |ui| {
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
    Frame::none().fill(p.surface).rounding(Rounding::same(18.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
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
            Frame::none().fill(p.surface).rounding(Rounding::same(14.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::symmetric(8.0, 10.0)).show(ui, |ui| {
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
        Frame::none().fill(p.surface).rounding(Rounding::same(18.0)).stroke(Stroke::new(1.0_f32, p.outline)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
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


