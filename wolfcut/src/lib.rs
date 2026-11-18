#[cfg(target_os = "android")]
mod android;
mod media;

use eframe::egui::{self, Color32, ColorImage, RichText, TextureHandle, TextureOptions};
use media::{Clip, MediaResult};
use std::{path::PathBuf, sync::mpsc, thread};

const ACCENT: Color32 = Color32::from_rgb(175, 239, 112);

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
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
}

impl Editor {
    fn new(context: &egui::Context) -> Self {
        let (sender, receiver) = mpsc::channel();
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(23, 25, 28);
        visuals.window_fill = visuals.panel_fill;
        visuals.extreme_bg_color = Color32::from_rgb(13, 15, 17);
        visuals.selection.bg_fill = Color32::from_rgb(54, 79, 42);
        context.set_visuals(visuals);
        #[cfg(target_os = "android")]
        context.style_mut(|style| {
            style.spacing.interact_size.y = 32.0;
        });
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
    fn poll_android(&mut self, context: &egui::Context) {
        if !self.importing && !self.exporting {
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
            Ok(None) => {}
            Err(error) => {
                self.importing = false;
                self.exporting = false;
                self.status = error;
            }
        }
        if self.importing || self.exporting {
            context.request_repaint_after(std::time::Duration::from_millis(150));
        }
    }
}

impl eframe::App for Editor {
    fn update(&mut self, context: &egui::Context, _: &mut eframe::Frame) {
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

        egui::TopBottomPanel::top("toolbar").show(context, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading(RichText::new("WolfCut").color(ACCENT).strong());
                ui.separator();
                if ui
                    .add_enabled(!self.importing, egui::Button::new("+ Импорт"))
                    .clicked()
                {
                    self.pick_import(context);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button = egui::Button::new(
                        RichText::new("Экспорт MP4").color(Color32::BLACK).strong(),
                    )
                    .fill(ACCENT);
                    if ui
                        .add_enabled(!self.clips.is_empty() && !self.exporting, button)
                        .clicked()
                    {
                        self.export(context);
                    }
                });
            });
            ui.add_space(8.0);
        });

        egui::TopBottomPanel::bottom("status").show(context, |ui| {
            ui.add_space(4.0);
            ui.label(&self.status);
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("timeline").show(context, |ui| {
            ui.add_space(9.0);
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
                                let title = format!("{}\n{:.2} с", clip.name(), clip.length());
                                let button =
                                    egui::Button::new(title).selected(index == self.selected);
                                if ui.add_sized([124.0, 52.0], button).clicked() {
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
                        .add_enabled(index > 0, egui::Button::new("← Раньше"))
                        .clicked()
                    {
                        self.clips.swap(index, index - 1);
                        self.selected -= 1;
                    }
                    if ui
                        .add_enabled(
                            self.selected + 1 < self.clips.len(),
                            egui::Button::new("Позже →"),
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
            ui.add_space(9.0);
        });

        if let Some(clip) = self.clips.get_mut(self.selected) {
            let mut trim_changed = false;
            egui::SidePanel::right("trim")
                .default_width(205.0)
                .show(context, |ui| {
                    ui.heading("Обрезка");
                    ui.weak(clip.name());
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

        egui::CentralPanel::default().show(context, |ui| {
            if let Some(clip) = self.clips.get(self.selected) {
                ui.heading("Предпросмотр");
                let size = egui::vec2(
                    ui.available_width(),
                    (ui.available_height() - 38.0).max(32.0),
                );
                egui::Frame::none()
                    .fill(Color32::from_rgb(12, 14, 16))
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
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Перетащите видео сюда").size(26.0));
                });
            }
        });

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
