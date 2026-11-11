//! Themes: Pixel-art style: dark and light palettes, square corners, 2 px hard borders, hard offset\n//! shadows, nearest-neighbour icons, compact spacing and fast (≈150 ms) animations.

use egui::{Color32, FontData, FontDefinitions, FontFamily, Id, Margin, Rounding, Shadow, Stroke, TextStyle, Vec2};
use wolfcraft_engine::config::ThemeMode;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub surface_hi: Color32,
    pub surface_hover: Color32,
    pub outline: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub on_accent: Color32,
    pub secondary: Color32,
    pub danger: Color32,
    pub success: Color32,
    pub warn: Color32,
    pub scrim: Color32,
}

pub const RADIUS_SM: f32 = 0.0;
pub const RADIUS_MD: f32 = 0.0;
pub const RADIUS_LG: f32 = 0.0;
/// Animation duration (seconds); within the requested 120–200 ms.
pub const ANIM_TIME: f32 = 0.15;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            dark: true,
            bg: rgb(0x0E1114),
            surface: rgb(0x161A1F),
            surface_hi: rgb(0x1E242B),
            surface_hover: rgb(0x262E37),
            outline: rgb(0x2C353F),
            text: rgb(0xE8ECF0),
            text_muted: rgb(0x8E99A5),
            accent: rgb(0x22C1CC),
            accent_hover: rgb(0x4BD3DC),
            on_accent: rgb(0x04181A),
            secondary: rgb(0xFF9A3C),
            danger: rgb(0xF0545B),
            success: rgb(0x3DD68C),
            warn: rgb(0xFFC53D),
            scrim: Color32::from_rgba_premultiplied(0, 0, 0, 170),
        }
    }

    pub fn light() -> Self {
        Self {
            dark: false,
            bg: rgb(0xEEF1F5),
            surface: rgb(0xFFFFFF),
            surface_hi: rgb(0xF3F5F8),
            surface_hover: rgb(0xE6EAF0),
            outline: rgb(0xD3D9E1),
            text: rgb(0x11161B),
            text_muted: rgb(0x5B6672),
            accent: rgb(0x07727B),
            accent_hover: rgb(0x055E66),
            on_accent: rgb(0xFFFFFF),
            secondary: rgb(0xE5760B),
            danger: rgb(0xD63A42),
            success: rgb(0x1E9E63),
            warn: rgb(0xB88400),
            scrim: Color32::from_rgba_premultiplied(10, 14, 20, 150),
        }
    }

    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        }
    }
}

fn palette_id() -> Id {
    Id::new("wolfcraft-palette")
}

/// Palette currently applied to the context.
pub fn palette(ctx: &egui::Context) -> Palette {
    ctx.data(|d| d.get_temp::<Palette>(palette_id())).unwrap_or_else(Palette::dark)
}

/// Loads Segoe UI (Windows system font, has Cyrillic) in front of egui's default fonts.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    // Pixel font (OFL, Latin + Cyrillic) embedded in the binary; Segoe UI stays as a fallback.
    fonts.font_data.insert("Pixel".to_string(), FontData::from_static(include_bytes!("../assets/PixelifySans.ttf")));
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let candidates = [("SegoeUI", "segoeui.ttf"), ("SegoeUIBold", "segoeuib.ttf")];
    let mut loaded = Vec::new();
    for (name, file) in candidates {
        let path = std::path::Path::new(&windir).join("Fonts").join(file);
        if let Ok(bytes) = std::fs::read(&path) {
            fonts.font_data.insert(name.to_string(), FontData::from_owned(bytes));
            loaded.push(name.to_string());
        }
    }
    if let Some(regular) = loaded.first() {
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, regular.clone());
    }
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "Pixel".to_string());
    let mut bold = vec!["Pixel".to_string()];
    bold.extend(loaded.get(1).cloned());
    bold.push("Ubuntu-Light".into());
    fonts.families.insert(FontFamily::Name("bold".into()), bold);
    ctx.set_fonts(fonts);
}

pub fn bold_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, FontFamily::Name("bold".into()))
}

/// Installs fonts once and applies the visuals of `mode`.
pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
}

pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    let p = Palette::for_mode(mode);
    ctx.data_mut(|d| d.insert_temp(palette_id(), p));

    let mut style = (*ctx.style()).clone();
    style.animation_time = ANIM_TIME;
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.spacing.window_margin = Margin::same(14.0);
    style.spacing.interact_size = Vec2::new(36.0, 30.0);
    style.spacing.slider_width = 180.0;
    style.spacing.scroll = egui::style::ScrollStyle { bar_width: 6.0, floating: true, ..egui::style::ScrollStyle::floating() };

    style.text_styles.insert(TextStyle::Heading, egui::FontId::new(26.0, FontFamily::Name("bold".into())));
    style.text_styles.insert(TextStyle::Body, egui::FontId::new(15.0, FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Button, egui::FontId::new(15.0, FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Small, egui::FontId::new(12.5, FontFamily::Proportional));
    style.text_styles.insert(TextStyle::Monospace, egui::FontId::new(13.5, FontFamily::Monospace));

    let v = &mut style.visuals;
    v.dark_mode = p.dark;
    v.override_text_color = Some(p.text);
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.surface_hi;
    v.faint_bg_color = p.surface_hi;
    v.window_stroke = Stroke::new(2.0_f32, p.outline);
    v.window_rounding = Rounding::same(RADIUS_LG);
    v.menu_rounding = Rounding::same(RADIUS_MD);
    v.window_shadow = Shadow { offset: Vec2::new(4.0, 4.0), blur: 0.0, spread: 0.0, color: Color32::from_black_alpha(if p.dark { 160 } else { 70 }) };
    v.popup_shadow = v.window_shadow;
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = p.accent.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(2.0_f32, p.accent);

    let r = Rounding::same(RADIUS_SM);
    for (w, fill) in [
        (&mut v.widgets.noninteractive, p.surface),
        (&mut v.widgets.inactive, p.surface_hi),
        (&mut v.widgets.hovered, p.surface_hover),
        (&mut v.widgets.active, p.surface_hover),
        (&mut v.widgets.open, p.surface_hover),
    ] {
        w.rounding = r;
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(2.0_f32, p.outline);
        w.fg_stroke = Stroke::new(2.0_f32, p.text);
    }
    v.widgets.hovered.bg_stroke = Stroke::new(2.0_f32, p.accent.gamma_multiply(0.7));
    v.widgets.active.bg_stroke = Stroke::new(2.0_f32, p.accent);
    ctx.set_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f32 {
        let f = |v: u8| {
            let s = v as f32 / 255.0;
            if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn both_themes_have_readable_text() {
        for p in [Palette::dark(), Palette::light()] {
            assert!(contrast(p.text, p.bg) > 7.0, "body text on background");
            assert!(contrast(p.text, p.surface) > 7.0, "body text on cards");
            assert!(contrast(p.on_accent, p.accent) > 4.5, "button label on accent (dark={})", p.dark);
            assert!(contrast(p.text_muted, p.surface) > 3.0, "muted text (dark={})", p.dark);
        }
    }

    #[test]
    fn animation_time_in_requested_range() {
        assert!((0.12..=0.2).contains(&ANIM_TIME));
        assert!(RADIUS_SM == 0.0 && RADIUS_LG == 0.0, "pixel style: square corners");
    }

    #[test]
    fn apply_stores_palette() {
        let ctx = egui::Context::default();
        apply(&ctx, ThemeMode::Light);
        assert!(!palette(&ctx).dark);
        apply(&ctx, ThemeMode::Dark);
        assert!(palette(&ctx).dark);
    }
}
