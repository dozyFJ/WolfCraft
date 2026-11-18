use eframe::egui::{self, Color32, Frame, Margin, RichText, Stroke, TextStyle, Vec2};

pub(crate) const ACCENT: Color32 = Color32::from_rgb(146, 188, 255);
pub(crate) const ON_ACCENT: Color32 = Color32::from_rgb(15, 33, 70);
const PURPLE: Color32 = Color32::from_rgb(225, 173, 255);
const TEXT: Color32 = Color32::from_rgb(235, 238, 255);

pub(crate) fn install(context: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = None;
    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = Color32::from_rgb(35, 41, 73);
    visuals.extreme_bg_color = Color32::from_rgb(19, 24, 49);
    visuals.selection.bg_fill = Color32::from_rgb(91, 56, 120);
    visuals.selection.stroke = Stroke::new(1.0_f32, PURPLE);
    let widgets = &mut visuals.widgets;
    for (widget, fill, foreground) in [
        (
            &mut widgets.noninteractive,
            Color32::from_rgb(31, 38, 69),
            TEXT,
        ),
        (&mut widgets.inactive, Color32::from_rgb(49, 58, 97), TEXT),
        (&mut widgets.hovered, Color32::from_rgb(60, 78, 132), TEXT),
        (&mut widgets.active, Color32::from_rgb(103, 66, 137), TEXT),
        (&mut widgets.open, Color32::from_rgb(87, 56, 116), TEXT),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke = Stroke::new(1.0_f32, foreground);
        widget.rounding = 10.0.into();
    }
    context.set_visuals(visuals);
    context.style_mut(|style| {
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(12.0, 5.0);
        style.spacing.interact_size.y = if cfg!(target_os = "android") {
            30.0
        } else {
            32.0
        };
        style.text_styles.get_mut(&TextStyle::Heading).unwrap().size = 18.0;
    });
}

pub(crate) fn glass(horizontal: f32, vertical: f32) -> Frame {
    Frame::none()
        .fill(Color32::from_rgba_unmultiplied(37, 44, 78, 210))
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(207, 221, 255, 60),
        ))
        .rounding(18.0)
        .inner_margin(Margin::symmetric(horizontal, vertical))
        .outer_margin(Margin::symmetric(7.0, 3.0))
}

pub(crate) fn primary(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(ON_ACCENT).strong())
        .fill(ACCENT)
        .rounding(14.0)
}

pub(crate) fn backdrop(context: &egui::Context) {
    let screen = context.screen_rect();
    let painter = context.layer_painter(egui::LayerId::background());
    let mut mesh = egui::Mesh::default();
    for (pos, color) in [
        (screen.left_top(), Color32::from_rgb(29, 35, 68)),
        (screen.right_top(), Color32::from_rgb(17, 38, 66)),
        (screen.right_bottom(), Color32::from_rgb(13, 19, 39)),
        (screen.left_bottom(), Color32::from_rgb(42, 28, 68)),
    ] {
        mesh.colored_vertex(pos, color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));

    for (offset, color) in [(0.23, [181, 91, 234]), (0.82, [74, 128, 255])] {
        let center = screen.left_top() + Vec2::new(screen.width() * offset, screen.height() * 0.23);
        let mut glow = egui::Mesh::default();
        glow.colored_vertex(
            center,
            Color32::from_rgba_unmultiplied(color[0], color[1], color[2], 75),
        );
        let radius = screen.size().min_elem() * 0.73;
        for step in 0..=48 {
            let angle = std::f32::consts::TAU * step as f32 / 48.0;
            glow.colored_vertex(center + Vec2::angled(angle) * radius, Color32::TRANSPARENT);
            if step > 0 {
                glow.add_triangle(0, step, step + 1);
            }
        }
        painter.add(egui::Shape::mesh(glow));
    }
}
