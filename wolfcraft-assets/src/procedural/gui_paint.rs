//! Original GUI icons (gui atlas). Drawn as white anti-aliased glyphs so the UI can tint them
//! with any theme colour; the logo and the status icons carry their own colours.

use super::{Painter, Rgb, SpriteList, circle, polygon, ring, rrect, segment};
use crate::image::Image;

pub const ICON_SIZE: u32 = 32;

pub mod gui {
    pub const LOGO: u16 = 0;
    pub const PLAY: u16 = 1;
    pub const PLUS: u16 = 2;
    pub const GEAR: u16 = 3;
    pub const EXIT: u16 = 4;
    pub const SEARCH: u16 = 5;
    pub const CLOSE: u16 = 6;
    pub const MINIMIZE: u16 = 7;
    pub const CHEVRON_DOWN: u16 = 8;
    pub const CHECK: u16 = 9;
    pub const TRASH: u16 = 10;
    pub const EDIT: u16 = 11;
    pub const FOLDER: u16 = 12;
    pub const HEART: u16 = 13;
    pub const HUNGER: u16 = 14;
    pub const STAR: u16 = 15;
    pub const SUN: u16 = 16;
    pub const MOON: u16 = 17;
    pub const DOWNLOAD: u16 = 18;
    pub const REFRESH: u16 = 19;
    pub const GLOBE: u16 = 20;
    pub const BACKPACK: u16 = 21;
    pub const HAMMER: u16 = 22;
    pub const ARROW_LEFT: u16 = 23;
    pub const SORT: u16 = 24;
    pub const CROSSHAIR: u16 = 25;
    pub const CUBE: u16 = 26;
    pub const WARNING: u16 = 27;
    pub const BRUSH: u16 = 28;
    pub const TREE: u16 = 29;
    pub const PROPS: u16 = 30;
    pub const OUTPUT: u16 = 31;
    pub const CODE: u16 = 32;
    pub const UNDO: u16 = 33;
    pub const REDO: u16 = 34;
    pub const COPY: u16 = 35;
    pub const PASTE: u16 = 36;
    pub const SELECT: u16 = 37;
    pub const STOP: u16 = 38;
    pub const FILL: u16 = 39;
    pub const PANEL_WOOD:u16=40;
    pub const PANEL_METAL:u16=41;
    pub const PANEL_RESONANCE:u16=42;
    pub const PANEL_MAGIC:u16=43;
    pub const PANEL_KITCHEN:u16=44;
    pub const PANEL_PORTABLE:u16=45;
    pub const PANEL_STONE:u16=46;
    pub const PANEL_DARK:u16=47;
    pub const PANEL_WORKBENCH:u16=48;
    pub const PANEL_FURNACE:u16=49;
    pub const PANEL_CHEST:u16=50;
    pub const PANEL_FORGE:u16=51;
    pub const ART_WORKBENCH:u16=52;
    pub const ART_FURNACE:u16=53;
    pub const ART_CHEST:u16=54;
    pub const ART_FORGE:u16=55;
    pub const COUNT: u16 = 48;
}

const WHITE: Rgb = Rgb(1.0, 1.0, 1.0);

fn icon(f: impl FnOnce(&mut Painter)) -> Image {
    let mut q = Painter::new(ICON_SIZE, ICON_SIZE);
    f(&mut q);
    q.finish()
}

fn heart_shape(cx: f32, cy: f32, s: f32) -> impl Fn(f32, f32) -> bool {
    move |u, v| {
        let x = (u - cx) / s;
        let y = -(v - cy) / s;
        (x * x + y * y - 1.0).powi(3) - x * x * y * y * y <= 0.0
    }
}

pub(crate) fn build() -> Vec<(String, Image)> {
    use gui::*;
    let mut l = SpriteList::new();

    // Logo: isometric cube with a wolf-ear notch, teal/orange.
    l.add(
        LOGO,
        "logo",
        icon(|q| {
            q.shape(Rgb(0.10, 0.62, 0.68), &polygon(vec![(0.5, 0.06), (0.92, 0.28), (0.5, 0.5), (0.08, 0.28)]));
            q.shape(Rgb(0.07, 0.42, 0.50), &polygon(vec![(0.08, 0.28), (0.5, 0.5), (0.5, 0.94), (0.08, 0.72)]));
            q.shape(Rgb(0.98, 0.55, 0.15), &polygon(vec![(0.92, 0.28), (0.5, 0.5), (0.5, 0.94), (0.92, 0.72)]));
            // wolf ears carved from the top face
            q.erase(&polygon(vec![(0.30, 0.20), (0.38, 0.10), (0.46, 0.22)]));
            q.erase(&polygon(vec![(0.54, 0.22), (0.62, 0.10), (0.70, 0.20)]));
        }),
    );
    l.add(PLAY, "play", icon(|q| q.shape(WHITE, &polygon(vec![(0.30, 0.18), (0.82, 0.5), (0.30, 0.82)]))));
    l.add(
        PLUS,
        "plus",
        icon(|q| {
            q.shape(WHITE, &segment(0.5, 0.2, 0.5, 0.8, 0.14));
            q.shape(WHITE, &segment(0.2, 0.5, 0.8, 0.5, 0.14));
        }),
    );
    l.add(
        GEAR,
        "gear",
        icon(|q| {
            q.shape(WHITE, &|u, v| {
                let (dx, dy) = (u - 0.5, v - 0.5);
                let r = (dx * dx + dy * dy).sqrt();
                let a = dy.atan2(dx);
                let teeth = ((a * 4.0).cos() * 0.5 + 0.5).powf(0.35);
                r < 0.30 + 0.12 * teeth && r > 0.0
            });
            q.erase(&circle(0.5, 0.5, 0.12));
        }),
    );
    l.add(
        EXIT,
        "exit",
        icon(|q| {
            q.shape(WHITE, &ring(0.5, 0.55, 0.28, 0.12));
            q.erase(&|u, v| v < 0.4 && (u - 0.5).abs() < 0.12);
            q.shape(WHITE, &segment(0.5, 0.12, 0.5, 0.52, 0.12));
        }),
    );
    l.add(
        SEARCH,
        "search",
        icon(|q| {
            q.shape(WHITE, &ring(0.43, 0.43, 0.24, 0.1));
            q.shape(WHITE, &segment(0.62, 0.62, 0.84, 0.84, 0.12));
        }),
    );
    l.add(
        CLOSE,
        "close",
        icon(|q| {
            q.shape(WHITE, &segment(0.25, 0.25, 0.75, 0.75, 0.1));
            q.shape(WHITE, &segment(0.75, 0.25, 0.25, 0.75, 0.1));
        }),
    );
    l.add(MINIMIZE, "minimize", icon(|q| q.shape(WHITE, &segment(0.25, 0.65, 0.75, 0.65, 0.1))));
    l.add(
        CHEVRON_DOWN,
        "chevron_down",
        icon(|q| {
            q.shape(WHITE, &segment(0.25, 0.38, 0.5, 0.63, 0.11));
            q.shape(WHITE, &segment(0.75, 0.38, 0.5, 0.63, 0.11));
        }),
    );
    l.add(
        CHECK,
        "check",
        icon(|q| {
            q.shape(WHITE, &segment(0.22, 0.52, 0.42, 0.72, 0.12));
            q.shape(WHITE, &segment(0.42, 0.72, 0.8, 0.28, 0.12));
        }),
    );
    l.add(
        TRASH,
        "trash",
        icon(|q| {
            q.shape(WHITE, &rrect(0.25, 0.32, 0.75, 0.88, 0.05));
            q.shape(WHITE, &rrect(0.18, 0.2, 0.82, 0.28, 0.03));
            q.shape(WHITE, &rrect(0.4, 0.1, 0.6, 0.2, 0.03));
            q.erase(&rrect(0.37, 0.42, 0.41, 0.78, 0.01));
            q.erase(&rrect(0.59, 0.42, 0.63, 0.78, 0.01));
        }),
    );
    l.add(
        EDIT,
        "edit",
        icon(|q| {
            q.shape(WHITE, &segment(0.26, 0.74, 0.72, 0.28, 0.2));
            q.shape(WHITE, &polygon(vec![(0.14, 0.86), (0.2, 0.64), (0.36, 0.8)]));
        }),
    );
    l.add(
        FOLDER,
        "folder",
        icon(|q| {
            q.shape(WHITE, &rrect(0.1, 0.3, 0.9, 0.82, 0.06));
            q.shape(WHITE, &rrect(0.1, 0.2, 0.45, 0.4, 0.05));
        }),
    );
    l.add(HEART, "heart", icon(|q| q.shape(Rgb(0.92, 0.22, 0.28), &heart_shape(0.5, 0.52, 0.30))));
    l.add(
        HUNGER,
        "hunger",
        icon(|q| {
            q.shape(Rgb(0.85, 0.52, 0.22), &circle(0.55, 0.42, 0.26));
            q.shape(Rgb(0.95, 0.88, 0.75), &segment(0.28, 0.7, 0.14, 0.86, 0.12));
            q.shape(Rgb(0.95, 0.88, 0.75), &circle(0.14, 0.86, 0.07));
        }),
    );
    l.add(
        STAR,
        "star",
        icon(|q| {
            let mut pts = Vec::new();
            for i in 0..10 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let r = if i % 2 == 0 { 0.4 } else { 0.17 };
                pts.push((0.5 + a.cos() * r, 0.54 + a.sin() * r));
            }
            q.shape(Rgb(0.60, 0.90, 0.25), &polygon(pts));
        }),
    );
    l.add(
        SUN,
        "sun",
        icon(|q| {
            q.shape(WHITE, &circle(0.5, 0.5, 0.2));
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                q.shape(WHITE, &segment(0.5 + a.cos() * 0.31, 0.5 + a.sin() * 0.31, 0.5 + a.cos() * 0.42, 0.5 + a.sin() * 0.42, 0.07));
            }
        }),
    );
    l.add(
        MOON,
        "moon",
        icon(|q| {
            q.shape(WHITE, &circle(0.5, 0.5, 0.36));
            q.erase(&circle(0.65, 0.4, 0.3));
        }),
    );
    l.add(
        DOWNLOAD,
        "download",
        icon(|q| {
            q.shape(WHITE, &segment(0.5, 0.15, 0.5, 0.6, 0.12));
            q.shape(WHITE, &polygon(vec![(0.28, 0.5), (0.72, 0.5), (0.5, 0.74)]));
            q.shape(WHITE, &rrect(0.2, 0.8, 0.8, 0.88, 0.03));
        }),
    );
    l.add(
        REFRESH,
        "refresh",
        icon(|q| {
            q.shape(WHITE, &ring(0.5, 0.5, 0.28, 0.1));
            q.erase(&|u, v| u > 0.5 && v < 0.5 && (u - 0.5).abs() < 0.3 && (v - 0.5).abs() < 0.3 && u - 0.5 > 0.5 - v - 0.02 + 0.0);
            q.shape(WHITE, &polygon(vec![(0.62, 0.14), (0.88, 0.3), (0.58, 0.38)]));
        }),
    );
    l.add(
        GLOBE,
        "globe",
        icon(|q| {
            q.shape(WHITE, &ring(0.5, 0.5, 0.36, 0.08));
            q.shape(WHITE, &|u, v| (((u - 0.5) / 0.16).powi(2) + ((v - 0.5) / 0.36).powi(2) - 1.0).abs() < 0.3);
            q.shape(WHITE, &segment(0.14, 0.5, 0.86, 0.5, 0.06));
        }),
    );
    l.add(
        BACKPACK,
        "backpack",
        icon(|q| {
            q.shape(WHITE, &rrect(0.22, 0.22, 0.78, 0.88, 0.14));
            q.erase(&rrect(0.32, 0.55, 0.68, 0.72, 0.04));
            q.shape(WHITE, &ring(0.5, 0.2, 0.12, 0.07));
        }),
    );
    l.add(
        HAMMER,
        "hammer",
        icon(|q| {
            q.shape(WHITE, &segment(0.3, 0.82, 0.62, 0.42, 0.11));
            q.shape(WHITE, &rrect(0.32, 0.14, 0.82, 0.38, 0.05));
        }),
    );
    l.add(
        ARROW_LEFT,
        "arrow_left",
        icon(|q| {
            q.shape(WHITE, &segment(0.2, 0.5, 0.8, 0.5, 0.1));
            q.shape(WHITE, &segment(0.2, 0.5, 0.42, 0.28, 0.1));
            q.shape(WHITE, &segment(0.2, 0.5, 0.42, 0.72, 0.1));
        }),
    );
    l.add(
        SORT,
        "sort",
        icon(|q| {
            q.shape(WHITE, &segment(0.2, 0.28, 0.8, 0.28, 0.09));
            q.shape(WHITE, &segment(0.2, 0.5, 0.62, 0.5, 0.09));
            q.shape(WHITE, &segment(0.2, 0.72, 0.44, 0.72, 0.09));
        }),
    );
    l.add(
        CROSSHAIR,
        "crosshair",
        icon(|q| {
            q.shape(WHITE, &segment(0.5, 0.1, 0.5, 0.4, 0.07));
            q.shape(WHITE, &segment(0.5, 0.6, 0.5, 0.9, 0.07));
            q.shape(WHITE, &segment(0.1, 0.5, 0.4, 0.5, 0.07));
            q.shape(WHITE, &segment(0.6, 0.5, 0.9, 0.5, 0.07));
        }),
    );
    l.add(
        CUBE,
        "cube",
        icon(|q| {
            q.shape(WHITE, &polygon(vec![(0.5, 0.08), (0.9, 0.3), (0.5, 0.52), (0.1, 0.3)]));
            q.shape(WHITE.scale(0.7), &polygon(vec![(0.1, 0.34), (0.48, 0.56), (0.48, 0.92), (0.1, 0.7)]));
            q.shape(WHITE.scale(0.85), &polygon(vec![(0.9, 0.34), (0.52, 0.56), (0.52, 0.92), (0.9, 0.7)]));
        }),
    );
    l.add(
        WARNING,
        "warning",
        icon(|q| {
            q.shape(Rgb(1.0, 0.76, 0.2), &polygon(vec![(0.5, 0.1), (0.92, 0.84), (0.08, 0.84)]));
            q.erase(&rrect(0.46, 0.38, 0.54, 0.62, 0.03));
            q.erase(&circle(0.5, 0.72, 0.05));
        }),
    );
    // Studio icons.
    l.add(BRUSH, "brush", icon(|q| {
        q.shape(WHITE, &segment(0.82, 0.12, 0.42, 0.56, 0.09));
        q.shape(WHITE, &polygon(vec![(0.34, 0.50), (0.50, 0.66), (0.30, 0.90), (0.10, 0.92), (0.16, 0.72)]));
    }));
    l.add(TREE, "tree", icon(|q| {
        q.shape(WHITE, &rrect(0.10, 0.10, 0.40, 0.28, 0.03));
        q.shape(WHITE, &rrect(0.40, 0.42, 0.90, 0.58, 0.03));
        q.shape(WHITE, &rrect(0.40, 0.72, 0.90, 0.88, 0.03));
        q.shape(WHITE, &segment(0.22, 0.28, 0.22, 0.80, 0.05));
        q.shape(WHITE, &segment(0.22, 0.50, 0.40, 0.50, 0.05));
        q.shape(WHITE, &segment(0.22, 0.80, 0.40, 0.80, 0.05));
    }));
    l.add(PROPS, "props", icon(|q| {
        for (i, y) in [0.22f32, 0.5, 0.78].iter().enumerate() {
            q.shape(WHITE, &segment(0.10, *y, 0.90, *y, 0.05));
            let x = [0.30f32, 0.65, 0.45][i];
            q.shape(WHITE, &circle(x, *y, 0.09));
        }
    }));
    l.add(OUTPUT, "output", icon(|q| {
        q.shape(WHITE, &rrect(0.08, 0.16, 0.92, 0.84, 0.08));
        q.erase(&rrect(0.14, 0.24, 0.86, 0.78, 0.04));
        q.shape(WHITE, &segment(0.24, 0.36, 0.40, 0.50, 0.05));
        q.shape(WHITE, &segment(0.40, 0.50, 0.24, 0.64, 0.05));
        q.shape(WHITE, &segment(0.48, 0.66, 0.72, 0.66, 0.05));
    }));
    l.add(CODE, "code", icon(|q| {
        q.shape(WHITE, &segment(0.32, 0.26, 0.12, 0.50, 0.07));
        q.shape(WHITE, &segment(0.12, 0.50, 0.32, 0.74, 0.07));
        q.shape(WHITE, &segment(0.68, 0.26, 0.88, 0.50, 0.07));
        q.shape(WHITE, &segment(0.88, 0.50, 0.68, 0.74, 0.07));
        q.shape(WHITE, &segment(0.56, 0.18, 0.44, 0.82, 0.06));
    }));
    l.add(UNDO, "undo", icon(|q| {
        q.shape(WHITE, &ring(0.55, 0.56, 0.30, 0.09));
        q.erase(&polygon(vec![(0.0, 0.0), (0.55, 0.0), (0.55, 0.56), (0.0, 0.56)]));
        q.shape(WHITE, &polygon(vec![(0.10, 0.40), (0.42, 0.20), (0.42, 0.62)]));
        q.shape(WHITE, &segment(0.40, 0.41, 0.56, 0.26, 0.09));
    }));
    l.add(REDO, "redo", icon(|q| {
        q.shape(WHITE, &ring(0.45, 0.56, 0.30, 0.09));
        q.erase(&polygon(vec![(0.45, 0.0), (1.0, 0.0), (1.0, 0.56), (0.45, 0.56)]));
        q.shape(WHITE, &polygon(vec![(0.90, 0.40), (0.58, 0.20), (0.58, 0.62)]));
        q.shape(WHITE, &segment(0.60, 0.41, 0.44, 0.26, 0.09));
    }));
    l.add(COPY, "copy", icon(|q| {
        q.shape(WHITE, &rrect(0.30, 0.30, 0.88, 0.88, 0.06));
        q.erase(&rrect(0.37, 0.37, 0.81, 0.81, 0.03));
        q.shape(WHITE, &rrect(0.12, 0.12, 0.62, 0.62, 0.06));
    }));
    l.add(PASTE, "paste", icon(|q| {
        q.shape(WHITE, &rrect(0.16, 0.18, 0.84, 0.92, 0.06));
        q.erase(&rrect(0.23, 0.30, 0.77, 0.85, 0.03));
        q.shape(WHITE, &rrect(0.34, 0.08, 0.66, 0.26, 0.04));
        q.shape(WHITE, &segment(0.32, 0.50, 0.68, 0.50, 0.05));
        q.shape(WHITE, &segment(0.32, 0.66, 0.60, 0.66, 0.05));
    }));
    l.add(SELECT, "select", icon(|q| {
        q.shape(WHITE, &polygon(vec![(0.18, 0.10), (0.80, 0.52), (0.52, 0.58), (0.68, 0.88), (0.56, 0.93), (0.40, 0.63), (0.20, 0.82)]));
    }));
    l.add(STOP, "stop", icon(|q| {
        q.shape(WHITE, &rrect(0.20, 0.20, 0.80, 0.80, 0.06));
    }));
    l.add(FILL, "fill", icon(|q| {
        q.shape(WHITE, &polygon(vec![(0.5, 0.10), (0.86, 0.46), (0.50, 0.82), (0.14, 0.46)]));
        q.erase(&polygon(vec![(0.5, 0.22), (0.74, 0.46), (0.50, 0.70), (0.26, 0.46)]));
        q.shape(WHITE, &polygon(vec![(0.26, 0.46), (0.74, 0.46), (0.50, 0.70)]));
        q.shape(WHITE, &circle(0.86, 0.80, 0.08));
    }));
    for (id,name,base,edge) in [
        (PANEL_WOOD,"panel_wood",[72,46,30],[174,119,65]),
        (PANEL_METAL,"panel_metal",[42,52,62],[132,158,172]),
        (PANEL_RESONANCE,"panel_resonance",[28,22,49],[89,214,206]),
        (PANEL_MAGIC,"panel_magic",[47,27,62],[185,112,223]),
        (PANEL_KITCHEN,"panel_kitchen",[50,68,69],[183,216,207]),
        (PANEL_PORTABLE,"panel_portable",[49,39,72],[154,124,206]),
        (PANEL_STONE,"panel_stone",[52,56,61],[146,151,159]),
        (PANEL_DARK,"panel_dark",[23,28,35],[67,83,99]),
        (PANEL_WORKBENCH,"panel_workbench",[103,66,38],[218,167,93]),
        (PANEL_FURNACE,"panel_furnace",[61,56,53],[174,126,83]),
        (PANEL_CHEST,"panel_chest",[83,52,31],[199,151,71]),
        (PANEL_FORGE,"panel_forge",[43,53,68],[161,182,198]),
    ] {let mut img=Image::new(32,32);for y in 0..32 {for x in 0..32 {
        let border=x<3||y<3||x>=29||y>=29;
        let seam=if matches!(id,PANEL_WOOD|PANEL_WORKBENCH|PANEL_CHEST) {(y%8)==0}else if matches!(id,PANEL_STONE|PANEL_FURNACE) {(y%12)==0||((x+if y<12{0}else{8})%16==0)}else{false};
        let stud=(x==4||x==27)&&(y==4||y==27);
        let c=if border||stud {edge}else if seam{base.map(|v|v/2)}else{base};
        img.set(x,y,[c[0],c[1],c[2],255]);
    }}l.add(id,name,img);}
    // Original tool illustrations live in the GUI atlas, independent of the UI layout.
    for (id,name) in [(ART_WORKBENCH,"art_workbench"),(ART_FURNACE,"art_furnace"),(ART_CHEST,"art_chest"),(ART_FORGE,"art_forge")] {
        let mut q=Painter::new(128,72);
        let dark=Rgb(37.0/255.0,27.0/255.0,23.0/255.0);let brass=Rgb(232.0/255.0,184.0/255.0,91.0/255.0);let steel=Rgb(182.0/255.0,204.0/255.0,217.0/255.0);
        match id {
            ART_WORKBENCH=>{
                q.shape(Rgb(178.0/255.0,121.0/255.0,64.0/255.0),&rrect(0.05,0.68,0.95,0.85,0.04));
                q.shape(dark,&rrect(0.12,0.82,0.20,0.98,0.02));q.shape(dark,&rrect(0.80,0.82,0.88,0.98,0.02));
                q.shape(Rgb(93.0/255.0,55.0/255.0,32.0/255.0),&segment(0.23,0.59,0.70,0.16,0.065));
                q.shape(steel,&polygon(vec![(0.51,0.12),(0.69,0.05),(0.89,0.36),(0.79,0.48)]));
                q.shape(brass,&rrect(0.15,0.12,0.31,0.44,0.03));
                q.shape(Rgb(222.0/255.0,204.0/255.0,152.0/255.0),&polygon(vec![(0.42,0.48),(0.69,0.43),(0.77,0.61),(0.47,0.65)]));
            },
            ART_FURNACE=>{
                q.shape(Rgb(122.0/255.0,127.0/255.0,132.0/255.0),&rrect(0.17,0.05,0.84,0.94,0.06));
                q.shape(dark,&rrect(0.25,0.22,0.76,0.76,0.08));
                q.shape(Rgb(224.0/255.0,83.0/255.0,35.0/255.0),&polygon(vec![(0.28,0.71),(0.35,0.40),(0.47,0.56),(0.58,0.27),(0.73,0.71)]));
                q.shape(Rgb(255.0/255.0,199.0/255.0,75.0/255.0),&polygon(vec![(0.40,0.71),(0.51,0.44),(0.62,0.71)]));
                for x in [0.30,0.43,0.56,0.69] {q.shape(steel,&segment(x,0.71,x,0.79,0.025));}
                q.shape(brass,&rrect(0.26,0.83,0.74,0.88,0.01));
            },
            ART_CHEST=>{
                q.shape(dark,&rrect(0.11,0.18,0.91,0.92,0.06));
                q.shape(Rgb(177.0/255.0,111.0/255.0,49.0/255.0),&rrect(0.15,0.22,0.87,0.86,0.04));
                q.shape(Rgb(111.0/255.0,65.0/255.0,34.0/255.0),&segment(0.16,0.48,0.86,0.48,0.035));
                for x in [0.23,0.74] {q.shape(brass,&rrect(x,0.23,x+0.07,0.85,0.01));}
                q.shape(brass,&rrect(0.43,0.40,0.59,0.66,0.03));
                q.shape(dark,&circle(0.51,0.50,0.035));q.shape(dark,&segment(0.51,0.50,0.51,0.59,0.018));
            },
            _=>{
                q.shape(Rgb(83.0/255.0,105.0/255.0,125.0/255.0),&polygon(vec![(0.10,0.36),(0.88,0.36),(0.79,0.51),(0.57,0.55),(0.57,0.74),(0.72,0.84),(0.27,0.84),(0.41,0.71),(0.41,0.54),(0.21,0.49)]));
                q.shape(steel,&segment(0.14,0.37,0.86,0.37,0.045));
                q.shape(Rgb(163.0/255.0,105.0/255.0,55.0/255.0),&segment(0.54,0.30,0.82,0.04,0.04));
                q.shape(steel,&polygon(vec![(0.43,0.11),(0.50,0.02),(0.68,0.19),(0.61,0.28)]));
                for (x,y) in [(0.26,0.17),(0.34,0.25),(0.17,0.29)] {q.shape(brass,&segment(x,y,x+0.04,y-0.07,0.014));}
            }
        }
        l.add(id,name,q.finish());
    }
    l.0
}

