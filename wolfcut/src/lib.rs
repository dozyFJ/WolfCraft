#[cfg(target_os = "android")]
mod android;
mod media;
mod theme;

use eframe::egui::{self, Color32, ColorImage, RichText, TextureHandle, TextureOptions};
use media::{Clip, MediaResult};
use std::{path::PathBuf, sync::mpsc, thread};

use theme::ACCENT;

enum Event {
    Imported(Vec<Clip>, Vec<String>),
    Frame(u64, MediaResult<image::RgbaImage>),
    Progress(usize, usize),
    Exported(PathBuf, MediaResult<()>),
}

struct Editor {
    clips: Vec<Clip>,
    selected: usize,
    cursor: f64,
    frame: Option<TextureHandle>,
    revision: u64,
    pending_frame: bool,
    frame_dirty: bool,
    importing: bool,
    exporting: bool,
    status: String,
    #[cfg(target_os = "android")]
    updating: bool,
    #[cfg(target_os = "android")]
    update_available: bool,
    #[cfg(target_os = "android")]
    update_manual: bool,
    #[cfg(target_os = "android")]
    confirm_update: bool,
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
}

impl Editor {
    fn new(context: &egui::Context) -> Self {
        let (sender, receiver) = mpsc::channel();
        theme::install(context);
        #[cfg(target_os = "android")]
        let updating = android::update(false).is_ok();
        Self {
            clips: Vec::new(),
            selected: 0,
            cursor: 0.0,
            frame: None,
            revision: 0,
            pending_frame: false,
            frame_dirty: false,
            importing: false,
            exporting: false,
            status: "Добавьте видео, чтобы начать монтаж.".into(),
            #[cfg(target_os = "android")]
            updating,
            #[cfg(target_os = "android")]
            update_available: false,
            #[cfg(target_os = "android")]
            update_manual: false,
            #[cfg(target_os = "android")]
            confirm_update: false,
            sender,
            receiver,
        }
    }

    fn import(&mut self, paths: Vec<PathBuf>, context: &egui::Context) {
        if paths.is_empty() || self.importing {
            return;
        }
        self.importing = true;
        self.status = "Чтение видео…".into();
        let sender = self.sender.clone();
        let context = context.clone();
        thread::spawn(move || {
            let mut clips = Vec::new();
            let mut errors = Vec::new();
            for path in paths {
                match media::probe(path.clone()) {
                    Ok(clip) => clips.push(clip),
                    Err(error) => errors.push(format!("{} — {error}", path.display())),
                }
            }
            let _ = sender.send(Event::Imported(clips, errors));
            context.request_repaint();
        });
    }

    fn select(&mut self, index: usize) {
        self.selected = index;
        self.cursor = self.clips.get(index).map_or(0.0, |clip| clip.start);
        self.frame = None;
        self.invalidate_frame();
    }

    fn invalidate_frame(&mut self) {
        self.revision += 1;
        self.frame_dirty = true;
    }

    fn request_frame(&mut self, context: &egui::Context) {
        if self.pending_frame || !self.frame_dirty {
            return;
        }
        self.frame_dirty = false;
        let Some(clip) = self.clips.get(self.selected) else {
            return;
        };
        let path = clip.path.clone();
        let time = self.cursor.min((clip.duration - 0.04).max(0.0));
        let revision = self.revision;
        let sender = self.sender.clone();
        let context = context.clone();
        self.pending_frame = true;
        thread::spawn(move || {
            let _ = sender.send(Event::Frame(revision, media::frame(&path, time)));
            context.request_repaint();
        });
    }

    #[cfg(not(target_os = "android"))]
    fn export(&mut self, context: &egui::Context) {
        let Some(destination) = rfd::FileDialog::new()
            .set_file_name("wolfcut.mp4")
            .add_filter("MP4", &["mp4"])
            .save_file()
        else {
            return;
        };
        if !destination
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
        {
            self.status = "Укажите имя файла с расширением .mp4.".into();
            return;
        }
        if let Ok(target) = destination.canonicalize() {
            if self
                .clips
                .iter()
                .any(|clip| clip.path.canonicalize().is_ok_and(|path| path == target))
            {
                self.status =
                    "Выберите другой файл для экспорта, чтобы сохранить исходное видео.".into();
                return;
            }
        }
        self.render(context, destination);
    }

    #[cfg(target_os = "android")]
    fn export(&mut self, context: &egui::Context) {
        self.render(context, android::session().join("wolfcut.mp4"));
    }

    fn render(&mut self, context: &egui::Context, destination: PathBuf) {
        self.exporting = true;
        self.status = "Подготовка экспорта…".into();
        let clips = self.clips.clone();
        let sender = self.sender.clone();
        let context = context.clone();
        thread::spawn(move || {
            let result = media::export(&clips, &destination, |part| {
                let _ = sender.send(Event::Progress(part, clips.len()));
                context.request_repaint();
            });
            let _ = sender.send(Event::Exported(destination, result));
            context.request_repaint();
        });
    }

    fn receive(&mut self, context: &egui::Context) {
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                Event::Imported(clips, errors) => {
                    self.importing = false;
                    let count = clips.len();
                    let first = self.clips.is_empty();
                    self.clips.extend(clips);
                    if first && count > 0 {
                        self.select(0);
                    }
                    self.status = if errors.is_empty() {
                        format!("Добавлено клипов: {count}")
                    } else {
                        format!("Добавлено клипов: {count}. {}", errors.join(" · "))
                    };
                }
                Event::Frame(revision, result) => {
                    self.pending_frame = false;
                    if revision == self.revision {
                        match result {
                            Ok(image) => {
                                let size = [image.width() as usize, image.height() as usize];
                                let pixels =
                                    ColorImage::from_rgba_unmultiplied(size, image.as_raw());
                                self.frame = Some(context.load_texture(
                                    "preview",
                                    pixels,
                                    TextureOptions::LINEAR,
                                ));
                            }
                            Err(error) => self.status = error,
                        }
                    }
                }
                Event::Progress(part, total) => {
                    self.status = format!("Экспорт MP4 · клип {part} из {total}")
                }
                Event::Exported(path, result) => {
                    #[cfg(target_os = "android")]
                    if result.is_ok() {
                        match android::pick_export() {
                            Ok(()) => {
                                self.status = "Выберите, куда сохранить видео.".into();
                                continue;
                            }
                            Err(error) => self.status = error,
                        }
                        self.exporting = false;
                        continue;
                    }
                    self.exporting = false;
                    self.status = match result {
                        Ok(()) => format!("Видео сохранено · {}", path.display()),
                        Err(error) => format!("Ошибка экспорта · {error}"),
                    };
                }
            }
        }
    }
}

impl Editor {
    #[cfg(not(target_os = "android"))]
    fn pick_import(&mut self, context: &egui::Context) {
        let paths = rfd::FileDialog::new()
            .add_filter("Видео", &["mp4", "mov", "mkv", "avi", "webm", "m4v"])
            .pick_files()
            .unwrap_or_default();
        self.import(paths, context);
    }

    #[cfg(target_os = "android")]
    fn pick_import(&mut self, context: &egui::Context) {
        match android::pick_import() {
            Ok(()) => {
                self.importing = true;
                self.status = "Выберите видео.".into();
                context.request_repaint();
            }
            Err(error) => self.status = error,
        }
    }

    #[cfg(target_os = "android")]
    fn begin_update(&mut self) {
        self.update_manual = true;
        match android::update(self.update_available) {
            Ok(()) => {
                self.updating = true;
                self.status = if self.update_available {
                    "Подготовка обновления…".into()
                } else {
                    "Проверка обновлений…".into()
                };
            }
            Err(error) => self.status = error,
        }
    }

    #[cfg(target_os = "android")]
    fn poll_android(&mut self, context: &egui::Context) {
        if !self.importing && !self.exporting && !self.updating {
            return;
        }
        match android::poll() {
            Ok(Some(android::ResultEvent::Import(paths, status))) => {
                self.importing = false;
                self.status = status;
                self.import(paths, context);
            }
            Ok(Some(android::ResultEvent::Export(status))) => {
                self.exporting = false;
                self.status = status;
            }
            Ok(Some(android::ResultEvent::Update(status, available))) => {
                self.updating = false;
                self.update_available = available;
                if self.update_manual || available {
                    self.status = status;
                }
            }
            Ok(None) => {}
            Err(error) => {
                self.importing = false;
                self.exporting = false;
                self.updating = false;
                self.status = error;
            }
        }
        if self.importing || self.exporting || self.updating {
            context.request_repaint_after(std::time::Duration::from_millis(150));
        }
    }
}

impl eframe::App for Editor {
    fn update(&mut self, context: &egui::Context, _: &mut eframe::Frame) {
        theme::backdrop(context);
        self.receive(context);
        #[cfg(target_os = "android")]
        self.poll_android(context);
        let dropped = context.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect()
        });
        self.import(dropped, context);

        egui::TopBottomPanel::top("toolbar")
            .frame(theme::glass(13.0, 7.0))
            .show(context, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.heading(RichText::new("WolfCut").color(ACCENT).strong());
                    if ui
                        .add_enabled(!self.importing, egui::Button::new("+ Импорт"))
                        .clicked()
                    {
                        self.pick_import(context);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                !self.clips.is_empty() && !self.exporting,
                                theme::primary("Экспорт MP4"),
                            )
                            .clicked()
                        {
                            self.export(context);
                        }
                        #[cfg(target_os = "android")]
                        {
                            let label = if self.update_available {
                                "Обновить"
                            } else {
                                "Обновления"
                            };
                            if ui
                                .add_enabled(
                                    !self.updating && !self.importing && !self.exporting,
                                    egui::Button::new(label),
                                )
                                .clicked()
                            {
                                if self.update_available && !self.clips.is_empty() {
                                    self.confirm_update = true;
                                } else {
                                    self.begin_update();
                                }
                            }
                        }
                    });
                });
            });

        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(16.0, 3.0)))
            .show(context, |ui| {
                ui.label(RichText::new(&self.status).small().weak());
            });

        egui::TopBottomPanel::bottom("timeline")
            .frame(theme::glass(12.0, 6.0))
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Монтаж");
                    let duration = self
                        .clips
                        .iter()
                        .fold(0.0, |total, clip| total + clip.length());
                    ui.weak(format!("{duration:.2} с"));
                });
                ui.add_space(5.0);
                if self.clips.is_empty() {
                    ui.weak("Добавьте клипы на монтажную дорожку.");
                } else {
                    let mut selection = None;
                    egui::ScrollArea::horizontal()
                        .id_salt("clips")
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (index, clip) in self.clips.iter().enumerate() {
                                    let name = clip.name();
                                    let mut characters = name.chars();
                                    let mut short: String = characters.by_ref().take(14).collect();
                                    if characters.next().is_some() {
                                        short.push('…');
                                    }
                                    let title = format!("{short}\n{:.2} с", clip.length());
                                    let button = egui::Button::new(title)
                                        .selected(index == self.selected)
                                        .rounding(12.0);
                                    if ui
                                        .add_sized([136.0, 42.0], button)
                                        .on_hover_text(name)
                                        .clicked()
                                    {
                                        selection = Some(index);
                                    }
                                }
                            });
                        });
                    if let Some(index) = selection {
                        self.select(index);
                    }

                    ui.horizontal(|ui| {
                        let index = self.selected;
                        if ui
                            .add_enabled(index > 0, egui::Button::new("Раньше"))
                            .clicked()
                        {
                            self.clips.swap(index, index - 1);
                            self.selected -= 1;
                        }
                        if ui
                            .add_enabled(
                                self.selected + 1 < self.clips.len(),
                                egui::Button::new("Позже"),
                            )
                            .clicked()
                        {
                            self.clips.swap(self.selected, self.selected + 1);
                            self.selected += 1;
                        }
                        if ui.button("Удалить").clicked() {
                            self.clips.remove(self.selected);
                            self.select(self.selected.min(self.clips.len().saturating_sub(1)));
                        }
                    });
                }
            });

        if let Some(clip) = self.clips.get_mut(self.selected) {
            let mut trim_changed = false;
            egui::SidePanel::right("trim")
                .default_width(190.0)
                .resizable(false)
                .frame(theme::glass(10.0, 8.0))
                .show(context, |ui| {
                    ui.heading("Обрезка");
                    ui.add(egui::Label::new(RichText::new(clip.name()).weak()).truncate());
                    ui.add_space(10.0);
                    trim_changed |= ui
                        .add(
                            egui::Slider::new(&mut clip.start, 0.0..=(clip.end - 0.04).max(0.0))
                                .fixed_decimals(3)
                                .text("Начало, с"),
                        )
                        .changed();
                    trim_changed |= ui
                        .add(
                            egui::Slider::new(
                                &mut clip.end,
                                (clip.start + 0.04).min(clip.duration)..=clip.duration,
                            )
                            .fixed_decimals(3)
                            .text("Конец, с"),
                        )
                        .changed();
                });
            if trim_changed {
                self.cursor = self.cursor.clamp(clip.start, clip.end);
                self.invalidate_frame();
            }
        }

        egui::CentralPanel::default()
            .frame(theme::glass(10.0, 7.0))
            .show(context, |ui| {
                if let Some(clip) = self.clips.get(self.selected) {
                    ui.heading("Предпросмотр");
                    let size = egui::vec2(
                        ui.available_width(),
                        (ui.available_height() - ui.spacing().interact_size.y - 9.0).max(24.0),
                    );
                    egui::Frame::none()
                        .fill(Color32::from_rgb(14, 18, 34))
                        .show(ui, |ui| {
                            ui.allocate_ui_with_layout(
                                size,
                                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                                |ui| {
                                    if let Some(frame) = &self.frame {
                                        let source = frame.size_vec2();
                                        let scale = (size.x / source.x).min(size.y / source.y);
                                        ui.image((frame.id(), source * scale));
                                    } else {
                                        ui.weak(if self.pending_frame || self.frame_dirty {
                                            "Загрузка кадра…"
                                        } else {
                                            "Кадр недоступен"
                                        });
                                    }
                                },
                            );
                        });
                    if ui
                        .add(
                            egui::Slider::new(&mut self.cursor, clip.start..=clip.end)
                                .fixed_decimals(3)
                                .suffix(" с"),
                        )
                        .changed()
                    {
                        self.invalidate_frame();
                    }
                } else {
                    let gap = (ui.available_height() * 0.32).max(0.0);
                    ui.add_space(gap);
                    ui.vertical_centered(|ui| {
                        ui.heading("Добавьте первое видео");
                        if !cfg!(target_os = "android") {
                            ui.weak("Перетащите файл в окно или выберите его в диалоге.");
                        }
                        if ui
                            .add_enabled(!self.importing, theme::primary("Выбрать видео"))
                            .clicked()
                        {
                            self.pick_import(context);
                        }
                    });
                }
            });

        #[cfg(target_os = "android")]
        if self.confirm_update {
            egui::Window::new("Обновление WolfCut")
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(context, |ui| {
                    ui.label("Обновление закроет текущий монтаж.");
                    ui.horizontal(|ui| {
                        if ui.button("Отмена").clicked() {
                            self.confirm_update = false;
                        }
                        if ui.add(theme::primary("Продолжить")).clicked() {
                            self.confirm_update = false;
                            self.begin_update();
                        }
                    });
                });
        }

        self.request_frame(context);
    }
}

#[cfg(not(target_os = "android"))]
pub fn run_desktop() -> eframe::Result<()> {
    let native = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([720.0, 480.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "WolfCut",
        native,
        Box::new(|setup| Ok(Box::new(Editor::new(&setup.egui_ctx)))),
    )
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("WolfCut"),
    );
    if let Err(error) = android::initialize(&app) {
        log::error!("Android setup: {error}");
        return;
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        event_loop_builder: Some(Box::new(move |builder| {
            builder.with_android_app(app);
        })),
        ..Default::default()
    };
    if let Err(error) = eframe::run_native(
        "WolfCut",
        options,
        Box::new(|setup| Ok(Box::new(Editor::new(&setup.egui_ctx)))),
    ) {
        log::error!("WolfCut startup: {error}");
    }
    android::clear();
}
