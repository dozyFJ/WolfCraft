use ab_glyph::{Font, FontRef, ScaleFont, point};
use image::{Rgba, RgbaImage};
use std::path::PathBuf;

#[derive(Clone)]
pub struct Title {
    pub text: String,
    pub start: f64,
    pub duration: f64,
}

#[derive(Clone)]
pub struct Audio {
    pub path: PathBuf,
    pub start: f64,
    pub duration: f64,
}

impl Audio {
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }
}

pub fn caption_image(text: &str) -> RgbaImage {
    let font = FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).expect("embedded font");
    let scaled = font.as_scaled(40.0);
    let mut lines = vec![String::new()];
    let mut width = 0.0;
    for character in text.trim().chars().take(120) {
        let advance = scaled.h_advance(font.glyph_id(character));
        if character == '\n' || width + advance > 1060.0 {
            if lines.len() == 4 {
                break;
            }
            lines.push(String::new());
            width = 0.0;
        }
        if character != '\n' {
            lines.last_mut().expect("caption line").push(character);
            width += advance;
        }
    }

    let line_width = |line: &str| -> f32 {
        line.chars()
            .map(|character| scaled.h_advance(font.glyph_id(character)))
            .sum()
    };
    let width = lines
        .iter()
        .map(|line| line_width(line))
        .fold(0.0, f32::max)
        .ceil() as u32
        + 36;
    let height = lines.len() as u32 * 48 + 18;
    let mut image = RgbaImage::from_pixel(width, height, Rgba([13, 17, 31, 218]));
    for (index, line) in lines.iter().enumerate() {
        let mut x = (width as f32 - line_width(line)) * 0.5;
        let baseline = 9.0 + scaled.ascent() + index as f32 * 48.0;
        for character in line.chars() {
            let id = font.glyph_id(character);
            let mut glyph = id.with_scale(40.0);
            glyph.position = point(x, baseline);
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds().min;
                outlined.draw(|column, row, coverage| {
                    let column = (bounds.x as i32 + column as i32) as u32;
                    let row = (bounds.y as i32 + row as i32) as u32;
                    if column < width && row < height {
                        let value = (13.0 + 242.0 * coverage) as u8;
                        image.put_pixel(
                            column,
                            row,
                            Rgba([value, value, value, (218.0 + 37.0 * coverage) as u8]),
                        );
                    }
                });
            }
            x += scaled.h_advance(id);
        }
    }
    image
}
