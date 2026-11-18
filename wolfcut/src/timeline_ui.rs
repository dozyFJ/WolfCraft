use crate::{
    Editor, theme,
    timeline::{Title, caption_image},
};
use eframe::egui::{
    self, Align2, Color32, ColorImage, FontId, Pos2, Rect, RichText, Sense, Stroke, TextureOptions,
    Vec2,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    Video(usize),
    Text(usize),
    Audio(usize),
}

struct Block {
    item: Item,
    name: String,
    start: f64,
    end: f64,
}

#[derive(Clone, Copy)]
struct TimelineDrop {
    track_area: Rect,
    origin: f32,
    pixels_per_second: f32,
}

const LANE_HEIGHT: f32 = 35.0;
const RULER_HEIGHT: f32 = 22.0;
const VIDEO: Color32 = Color32::from_rgb(50, 77, 128);
const TEXT: Color32 = Color32::from_rgb(97, 53, 126);
const AUDIO: Color32 = Color32::from_rgb(38, 87, 103);

impl Editor {
    pub(super) fn drop_position(&self, context: &egui::Context) -> f64 {
        let layout =
            context.data(|data| data.get_temp::<TimelineDrop>(egui::Id::new("timeline_drop")));
        let pointer = context.input(|input| input.pointer.latest_pos());
        if let (Some(layout), Some(pointer)) = (layout, pointer)
            && layout.track_area.contains(pointer)
        {
            return f64::from((pointer.x - layout.origin) / layout.pixels_per_second)
                .clamp(0.0, (self.duration() - 0.04).max(0.0));
        }
        self.playhead().min((self.duration() - 0.04).max(0.0))
    }

    pub(super) fn duration(&self) -> f64 {
        self.clips.iter().map(|clip| clip.length()).sum()
    }

    pub(super) fn playhead(&self) -> f64 {
        self.clips.get(self.selected).map_or(0.0, |clip| {
            self.clips[..self.selected]
                .iter()
                .map(|clip| clip.length())
                .sum::<f64>()
                + self.cursor
                - clip.start
        })
    }

    pub(super) fn seek(&mut self, absolute: f64) {
        let mut remaining = absolute.clamp(0.0, self.duration());
        let last = self.clips.len().saturating_sub(1);
        for (index, clip) in self.clips.iter().enumerate() {
            if remaining < clip.length() || index == last {
                let cursor = clip.start + remaining.min(clip.length());
                if index != self.selected {
                    self.frame = None;
                }
                if index != self.selected || (self.cursor - cursor).abs() > 0.0001 {
                    self.selected = index;
                    self.cursor = cursor;
                    self.invalidate_frame();
                }
                return;
            }
            remaining -= clip.length();
        }
    }

    pub(super) fn preview(&mut self, ui: &mut egui::Ui, size: Vec2) {
        let (viewport, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter().with_clip_rect(viewport);
        painter.rect_filled(viewport, 8.0, Color32::from_rgb(14, 18, 34));
        let Some(frame) = &self.frame else {
            painter.text(
                viewport.center(),
                Align2::CENTER_CENTER,
                if self.pending_frame || self.frame_dirty {
                    "Загрузка кадра…"
                } else {
                    "Кадр недоступен"
                },
                FontId::proportional(13.0),
                Color32::LIGHT_GRAY,
            );
            return;
        };

        let source = frame.size_vec2();
        let scale = (size.x / source.x).min(size.y / source.y);
        let video_rect = Rect::from_center_size(viewport.center(), source * scale);
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(frame.id(), video_rect, uv, Color32::WHITE);

        let time = self.playhead();
        let active = self.titles.iter().rev().find(|title| {
            !title.text.trim().is_empty()
                && time >= title.start
                && time < title.start + title.duration
        });
        if let Some(title) = active {
            let text = &title.text;
            let index = self
                .caption_textures
                .iter()
                .position(|entry| &entry.0 == text)
                .unwrap_or_else(|| {
                    if self.caption_textures.len() >= 12 {
                        drop(self.caption_textures.remove(0));
                    }
                    let bitmap = caption_image(text);
                    let image = ColorImage::from_rgba_unmultiplied(
                        [bitmap.width() as usize, bitmap.height() as usize],
                        bitmap.as_raw(),
                    );
                    let texture = ui
                        .ctx()
                        .load_texture("title", image, TextureOptions::LINEAR);
                    self.caption_textures.push((text.clone(), texture));
                    self.caption_textures.len() - 1
                });
            let texture = &self.caption_textures[index].1;
            let canvas_scale = (size.x / 1280.0).min(size.y / 720.0);
            let bounds = Rect::from_center_size(
                Pos2::new(
                    video_rect.center().x,
                    video_rect.bottom() - 12.0 - texture.size_vec2().y * canvas_scale * 0.5,
                ),
                texture.size_vec2() * canvas_scale,
            );
            painter.image(texture.id(), bounds, uv, Color32::WHITE);
        }
    }

    fn blocks(&self) -> [Vec<Block>; 3] {
        let mut time = 0.0;
        let video = self
            .clips
            .iter()
            .enumerate()
            .map(|(index, clip)| {
                let start = time;
                time += clip.length();
                Block {
                    item: Item::Video(index),
                    name: clip.name(),
                    start,
                    end: time,
                }
            })
            .collect();
        let text = self
            .titles
            .iter()
            .enumerate()
            .map(|(index, title)| Block {
                item: Item::Text(index),
                name: title.text.clone(),
                start: title.start,
                end: title.start + title.duration,
            })
            .collect();
        let audio = self
            .audio
            .iter()
            .enumerate()
            .map(|(index, track)| Block {
                item: Item::Audio(index),
                name: track.name(),
                start: track.start,
                end: track.start + track.duration,
            })
            .collect();
        [video, text, audio]
    }

    pub(super) fn timeline(&mut self, ui: &mut egui::Ui) {
        let duration = self.duration();
        ui.horizontal(|ui| {
            ui.heading("Таймлайн");
            ui.weak(format!("{:.1} / {:.1} с", self.playhead(), duration));
            if ui
                .add_enabled(self.timeline_item.is_some(), egui::Button::new("Править"))
                .clicked()
            {
                self.inspector = true;
            }
        });
        let blocks = self.blocks();
        let timeline_end = blocks
            .iter()
            .flatten()
            .fold(duration.max(5.0), |end, block| end.max(block.end));
        let pixels_per_second = (5600.0 / timeline_end).min(48.0) as f32;
        let mut selected = None;
        let mut moved = None;
        let mut seek = None;

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(66.0);
                ui.add_space(RULER_HEIGHT);
                for (index, name) in ["Видео", "Текст", "Аудио"].into_iter().enumerate()
                {
                    ui.allocate_ui_with_layout(
                        Vec2::new(66.0, LANE_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            if ui
                                .add_enabled(
                                    !self.importing || index == 1,
                                    egui::Button::new(format!("+ {name}")),
                                )
                                .clicked()
                            {
                                match index {
                                    0 => self.pick_import(false, ui.ctx()),
                                    1 => {
                                        self.titles.push(Title {
                                            text: "Текст".to_owned(),
                                            start: self.playhead().min((duration - 0.04).max(0.0)),
                                            duration: 3.0,
                                        });
                                        self.timeline_item =
                                            Some(Item::Text(self.titles.len() - 1));
                                        self.inspector = true;
                                    }
                                    _ => self.pick_import(true, ui.ctx()),
                                }
                            }
                        },
                    );
                }
            });
            let width = (timeline_end as f32 * pixels_per_second)
                .max(ui.available_width())
                .max(1.0);
            egui::ScrollArea::horizontal()
                .id_salt("timeline_tracks")
                .show(ui, |ui| {
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new(
                            width,
                            RULER_HEIGHT + 3.0 * LANE_HEIGHT + 3.0 * ui.spacing().item_spacing.y,
                        ),
                        Sense::hover(),
                    );
                    ui.data_mut(|data| {
                        data.insert_temp(
                            egui::Id::new("timeline_drop"),
                            TimelineDrop {
                                track_area: rect.intersect(ui.clip_rect()),
                                origin: rect.left(),
                                pixels_per_second,
                            },
                        )
                    });
                    let painter = ui.painter_at(rect);
                    let x_at = |time: f64| rect.left() + time as f32 * pixels_per_second;
                    let time_at = |x: f32| {
                        f64::from((x - rect.left()) / pixels_per_second).clamp(0.0, timeline_end)
                    };
                    let ruler =
                        Rect::from_min_size(rect.min, Vec2::new(rect.width(), RULER_HEIGHT));
                    let ruler_response =
                        ui.interact(ruler, ui.id().with("ruler"), Sense::click_and_drag());
                    if (ruler_response.clicked() || ruler_response.dragged())
                        && let Some(pointer) = ruler_response.interact_pointer_pos()
                    {
                        seek = Some(time_at(pointer.x));
                    }
                    let ruler_step = (52.0 / pixels_per_second).ceil() as usize;
                    for second in (0..=time_at(rect.right()).ceil() as usize).step_by(ruler_step) {
                        let x = x_at(second as f64);
                        painter.line_segment(
                            [
                                Pos2::new(x, ruler.bottom() - 5.0),
                                Pos2::new(x, ruler.bottom()),
                            ],
                            Stroke::new(1.0_f32, Color32::GRAY),
                        );
                        painter.text(
                            Pos2::new(x + 4.0, ruler.top()),
                            Align2::LEFT_TOP,
                            format!("{}:{:02}", second / 60, second % 60),
                            FontId::monospace(11.0),
                            Color32::LIGHT_GRAY,
                        );
                    }

                    for (index, (lane, fill)) in blocks.iter().zip([VIDEO, TEXT, AUDIO]).enumerate()
                    {
                        let top = rect.top()
                            + RULER_HEIGHT
                            + (index as f32 + 1.0) * ui.spacing().item_spacing.y
                            + index as f32 * LANE_HEIGHT;
                        let lane_rect = Rect::from_min_size(
                            Pos2::new(rect.left(), top),
                            Vec2::new(rect.width(), LANE_HEIGHT),
                        );
                        painter.rect_filled(lane_rect, 7.0, Color32::from_rgb(26, 31, 56));
                        for block in lane {
                            let left = x_at(block.start);
                            let right = x_at(block.end).max(left + 16.0).min(rect.right());
                            let chip = Rect::from_min_max(
                                Pos2::new(left, lane_rect.top() + 2.0),
                                Pos2::new(right, lane_rect.bottom() - 2.0),
                            );
                            let response = ui.interact(
                                chip,
                                ui.id().with(block.item),
                                Sense::click_and_drag(),
                            );
                            painter.rect_filled(chip, 6.0, fill);
                            if self.timeline_item == Some(block.item) || response.dragged() {
                                painter.rect_stroke(chip, 6.0, Stroke::new(1.0_f32, theme::ACCENT));
                            }
                            if chip.width() > 20.0 {
                                painter
                                    .with_clip_rect(chip.shrink2(Vec2::new(5.0, 0.0)))
                                    .text(
                                        Pos2::new(chip.left() + 6.0, chip.center().y),
                                        Align2::LEFT_CENTER,
                                        block.name.lines().next().unwrap_or(""),
                                        FontId::proportional(12.0),
                                        Color32::WHITE,
                                    );
                            }
                            if response.clicked() || response.double_clicked() {
                                selected =
                                    Some((block.item, block.start, response.double_clicked()));
                            }
                            if response.drag_stopped()
                                && let Some(pointer) = ui.input(|input| input.pointer.latest_pos())
                            {
                                moved = Some((block.item, time_at(pointer.x)));
                            }
                            response.on_hover_text(&block.name);
                        }
                    }
                    let x = x_at(self.playhead());
                    painter.line_segment(
                        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                        Stroke::new(1.5_f32, theme::ACCENT),
                    );
                });
        });

        if let Some(time) = seek {
            self.seek(time);
        }
        if let Some((item, start, open)) = selected {
            self.timeline_item = Some(item);
            self.seek(start);
            self.inspector |= open;
        }
        if let Some((item, time)) = moved {
            match item {
                Item::Video(index) => {
                    let target = blocks[0]
                        .iter()
                        .position(|block| time < (block.start + block.end) * 0.5)
                        .unwrap_or(self.clips.len());
                    let target = target.saturating_sub(usize::from(target > index));
                    self.move_video(index, target.min(self.clips.len() - 1));
                }
                Item::Text(index) => {
                    self.titles[index].start = time.min((duration - 0.04).max(0.0));
                    self.timeline_item = Some(item);
                    self.seek(time);
                }
                Item::Audio(index) => {
                    self.audio[index].start = time.min((duration - 0.04).max(0.0));
                    self.timeline_item = Some(item);
                    self.seek(time);
                }
            }
        }
    }

    fn move_video(&mut self, index: usize, target: usize) {
        let clip = self.clips.remove(index);
        self.clips.insert(target, clip);
        self.timeline_item = Some(Item::Video(target));
        self.select(target);
    }

    pub(super) fn show_inspector(&mut self, context: &egui::Context) {
        let Some(item) = self.timeline_item.filter(|_| self.inspector) else {
            return;
        };
        let mut show = self.inspector;
        let title = match item {
            Item::Video(_) => "Видео",
            Item::Text(_) => "Текст",
            Item::Audio(_) => "Аудио",
        };
        let duration = self.duration();
        egui::Window::new(title)
            .open(&mut show)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .default_width(245.0)
            .show(context, |ui| {
                ui.set_max_width((context.screen_rect().width() - 40.0).clamp(180.0, 275.0));
                match item {
                    Item::Video(index) => {
                        if self.selected != index {
                            self.select(index);
                        }
                        self.trim_controls(ui, true);
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(index > 0, egui::Button::new("Раньше"))
                                .clicked()
                            {
                                self.move_video(index, index - 1);
                            }
                            if ui
                                .add_enabled(
                                    index + 1 < self.clips.len(),
                                    egui::Button::new("Позже"),
                                )
                                .clicked()
                            {
                                self.move_video(index, index + 1);
                            }
                        });
                    }
                    Item::Text(index) => {
                        let title = &mut self.titles[index];
                        ui.add(
                            egui::TextEdit::singleline(&mut title.text)
                                .char_limit(120)
                                .desired_width(240.0)
                                .hint_text("Введите текст"),
                        );
                        ui.add(
                            egui::Slider::new(&mut title.start, 0.0..=(duration - 0.04).max(0.0))
                                .text("Начало, с"),
                        );
                        ui.add(
                            egui::Slider::new(&mut title.duration, 0.1..=duration.max(3.0))
                                .text("Длина, с"),
                        );
                    }
                    Item::Audio(index) => {
                        let track = &mut self.audio[index];
                        ui.add(egui::Label::new(RichText::new(track.name())).truncate());
                        ui.add(
                            egui::Slider::new(&mut track.start, 0.0..=(duration - 0.04).max(0.0))
                                .text("Начало, с"),
                        );
                        ui.weak(format!("Длина {:.2} с", track.duration));
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("Удалить").clicked() {
                        match item {
                            Item::Video(index) => {
                                self.clips.remove(index);
                                self.select(index.min(self.clips.len().saturating_sub(1)));
                            }
                            Item::Text(index) => {
                                self.titles.remove(index);
                            }
                            Item::Audio(index) => {
                                self.audio.remove(index);
                            }
                        }
                        self.timeline_item = None;
                        self.inspector = false;
                    }
                    if ui.add(theme::primary("Готово")).clicked() {
                        self.inspector = false;
                    }
                });
            });
        self.inspector &= show;
    }
}
