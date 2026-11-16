//! Themes: modern Material-style UI with an industrial touch — Inter type, blue accent,
//! rounded cards and pill buttons, 1 px hairline borders, soft shadows, letter-spaced
//! uppercase section labels, dark and light palettes, fast (≈150 ms) animations.

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

pub const RADIUS_SM: f32 = 8.0;
pub const RADIUS_MD: f32 = 12.0;
pub const RADIUS_LG: f32 = 20.0;
/// Animation duration (seconds); within the requested 120–200 ms.
pub const ANIM_TIME: f32 = 0.12;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            dark: true,
            bg: rgb(0x0E1014),
            surface: rgb(0x171A20),
            surface_hi: rgb(0x1F232B),
            surface_hover: rgb(0x282D37),
            outline: rgb(0x2D333E),
            text: rgb(0xE6E9EF),
            text_muted: rgb(0x9AA3B2),
            accent: rgb(0x2563EB),
            accent_hover: rgb(0x3B76F0),
            on_accent: rgb(0xFFFFFF),
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
            bg: rgb(0xF3F5F9),
            surface: rgb(0xFFFFFF),
            surface_hi: rgb(0xF0F2F6),
            surface_hover: rgb(0xE4E8EF),
            outline: rgb(0xD6DCE5),
            text: rgb(0x111418),
            text_muted: rgb(0x5A6473),
            accent: rgb(0x1D5FD6),
            accent_hover: rgb(0x174FB4),
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

/// Inter (OFL, Latin + Cyrillic) in front of egui's default fonts: regular for text, SemiBold
/// for headings and buttons.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("Inter".to_string(), FontData::from_static(include_bytes!("../assets/Inter-Regular.otf")));
    fonts.font_data.insert("InterSemi".to_string(), FontData::from_static(include_bytes!("../assets/Inter-SemiBold.otf")));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "Inter".to_string());
    fonts.families.insert(FontFamily::Name("bold".into()), vec!["InterSemi".to_string(), "Inter".to_string(), "Ubuntu-Light".into()]);
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
    v.window_stroke = Stroke::new(1.0_f32, p.outline);
    v.window_rounding = Rounding::same(RADIUS_LG);
    v.menu_rounding = Rounding::same(RADIUS_MD);
    v.window_shadow = Shadow { offset: Vec2::new(0.0, 8.0), blur: 28.0, spread: 0.0, color: Color32::from_black_alpha(if p.dark { 120 } else { 40 }) };
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
        w.bg_stroke = Stroke::new(1.0_f32, p.outline);
        w.fg_stroke = Stroke::new(1.5_f32, p.text);
    }
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent.gamma_multiply(0.7));
    v.widgets.active.bg_stroke = Stroke::new(1.5_f32, p.accent);
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
        assert!(RADIUS_SM > 0.0 && RADIUS_LG > RADIUS_MD, "modern style: rounded corners");
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
