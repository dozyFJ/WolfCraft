//! The story: five chapters, each a place, a shadow to beat and a few lines of text.

use crate::fight::Ai;

#[derive(Clone, Copy)]
pub struct Theme {
    pub sky_top: [f32; 3],
    pub sky_horizon: [f32; 3],
    pub sun_dir: [f32; 3],
    pub sun: f32,
    pub sun_color: [f32; 3],
    pub fog: f32,
    pub floor: [f32; 3],
    pub stone: [f32; 3],
    pub torch: [f32; 3],
    /// The shadow's outline and eyes.
    pub aura: [f32; 3],
    pub eclipse: bool,
}

pub struct Chapter {
    pub title: &'static str,
    pub place: &'static str,
    pub foe: &'static str,
    pub intro: &'static str,
    pub outro: &'static str,
    pub ai: Ai,
    pub theme: Theme,
}

pub const PROLOGUE: &str = "Монах-изгнанник Арен нашёл в горах заброшенный монастырь. В его подвале — Врата Теней, запечатанные древним кругом. Арен коснулся печати… и его собственная тень оторвалась от ног и ушла во тьму за Вратами.\n\nБез тени человек угасает. Чтобы вернуть её, Арену придётся пройти сквозь пять теней-стражей.";

pub const EPILOGUE: &str = "Владыка Теней рассыпается пеплом. Из его сердца выходит тень Арена — тихая, покорная — и снова ложится у его ног.\n\nВрата закрываются. Но на камне печати проступает новая трещина…\n\nКонец первой книги.";

const fn ai(aggression: f32, reaction: f32, block: f32, dodge: f32, kick: f32, hook: f32, speed: f32, damage: f32, hp: f32, shadow: bool) -> Ai {
    Ai { aggression, reaction, block_skill: block, dodge_skill: dodge, kick_bias: kick, hook_bias: hook, speed, damage, hp, uses_shadow: shadow }
}

pub static CHAPTERS: [Chapter; 5] = [
    Chapter {
        title: "Глава I. Отражение",
        place: "Двор монастыря, закат",
        foe: "Отражение",
        intro: "Во дворе ждёт первая тень — его собственное отражение, неуклюжее и медленное. Оно повторяет его движения с опозданием. «Бей первым, — шепчет ветер, — пока оно учится».",
        outro: "Отражение тает, как дым. На камнях остаётся след — дорога вниз, к бамбуковой роще.",
        ai: ai(0.35, 0.55, 0.15, 0.05, 0.1, 0.15, 0.85, 0.7, 70.0, false),
        theme: Theme { sky_top: [0.10, 0.12, 0.30], sky_horizon: [0.95, 0.55, 0.32], sun_dir: [0.6, 0.25, 0.75], sun: 1.1, sun_color: [1.0, 0.75, 0.5], fog: 0.020, floor: [0.42, 0.36, 0.30], stone: [0.50, 0.44, 0.38], torch: [1.0, 0.6, 0.25], aura: [1.0, 0.55, 0.25], eclipse: false },
    },
    Chapter {
        title: "Глава II. Шёпот",
        place: "Бамбуковая роща, туман",
        foe: "Шёпот",
        intro: "В тумане рощи кто-то смеётся. Шёпот быстрый, он не стоит на месте и жалит лёгкими ударами. Блок спасёт от града ударов — а после него бей в ответ.",
        outro: "Смех обрывается. Туман редеет, и за деревьями виден каменный мост над пропастью.",
        ai: ai(0.75, 0.40, 0.25, 0.25, 0.15, 0.1, 1.15, 0.8, 80.0, false),
        theme: Theme { sky_top: [0.30, 0.40, 0.38], sky_horizon: [0.70, 0.78, 0.72], sun_dir: [-0.4, 0.6, 0.5], sun: 0.6, sun_color: [0.9, 1.0, 0.9], fog: 0.055, floor: [0.30, 0.36, 0.26], stone: [0.36, 0.42, 0.34], torch: [0.6, 1.0, 0.6], aura: [0.4, 1.0, 0.55], eclipse: false },
    },
    Chapter {
        title: "Глава III. Страж Врат",
        place: "Каменный мост, гроза",
        foe: "Страж",
        intro: "На мосту стоит Страж — огромная тень в каменной броне. Его блок почти не пробить лёгкими ударами. Тяжёлый удар ломает защиту. Но и его кулак крушит без пощады.",
        outro: "Страж падает на колено и склоняет голову: «Проходи». Мост ведёт к храму на скале, над которым гаснет солнце.",
        ai: ai(0.45, 0.30, 0.65, 0.05, 0.2, 0.55, 0.8, 1.25, 130.0, false),
        theme: Theme { sky_top: [0.06, 0.07, 0.10], sky_horizon: [0.25, 0.28, 0.34], sun_dir: [0.2, 0.7, -0.6], sun: 0.5, sun_color: [0.7, 0.8, 1.0], fog: 0.04, floor: [0.30, 0.30, 0.32], stone: [0.38, 0.38, 0.40], torch: [0.5, 0.7, 1.0], aura: [0.45, 0.65, 1.0], eclipse: false },
    },
    Chapter {
        title: "Глава IV. Вдова Затмения",
        place: "Храм на скале, затмение",
        foe: "Вдова",
        intro: "Солнце закрыла чёрная луна. Вдова Затмения танцует вокруг, уходит от ударов и бьёт ногами издалека. Держи дистанцию короткой — и уклоняйся сам.",
        outro: "Луна сходит с солнца. Вдова исчезает, оставив венец из тени. В нём — дорога к Сердцу Врат.",
        ai: ai(0.6, 0.25, 0.3, 0.6, 0.7, 0.2, 1.2, 1.05, 110.0, true),
        theme: Theme { sky_top: [0.03, 0.02, 0.06], sky_horizon: [0.45, 0.18, 0.30], sun_dir: [0.0, 0.35, 1.0], sun: 0.7, sun_color: [1.0, 0.5, 0.7], fog: 0.03, floor: [0.24, 0.20, 0.24], stone: [0.32, 0.26, 0.30], torch: [1.0, 0.35, 0.65], aura: [1.0, 0.3, 0.75], eclipse: true },
    },
    Chapter {
        title: "Глава V. Владыка Теней",
        place: "Сердце Врат",
        foe: "Владыка",
        intro: "Здесь, в Сердце Врат, ждёт Владыка Теней. Внутри него бьётся тень Арена. Он знает все приёмы прежних стражей — и копит силу Тени для сокрушительного удара.",
        outro: "",
        ai: ai(0.8, 0.2, 0.5, 0.45, 0.35, 0.4, 1.15, 1.25, 160.0, true),
        theme: Theme { sky_top: [0.02, 0.0, 0.03], sky_horizon: [0.30, 0.04, 0.10], sun_dir: [0.0, 0.15, 1.0], sun: 0.8, sun_color: [1.0, 0.25, 0.2], fog: 0.035, floor: [0.16, 0.10, 0.12], stone: [0.22, 0.14, 0.16], torch: [1.0, 0.2, 0.15], aura: [1.0, 0.18, 0.12], eclipse: false },
    },
];

/// Progress kept in the app's data folder: how many chapters are open.
pub fn load_progress(dir: &std::path::Path) -> usize {
    std::fs::read_to_string(dir.join("progress.txt")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(1).clamp(1, CHAPTERS.len() + 1)
}

pub fn save_progress(dir: &std::path::Path, open: usize) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join("progress.txt"), open.to_string());
}

#[cfg(test)]
mod tests {
    #[test]
    fn progress_round_trips() {
        let dir = std::env::temp_dir().join(format!("shadow-test-{}", std::process::id()));
        assert_eq!(super::load_progress(&dir), 1);
        super::save_progress(&dir, 3);
        assert_eq!(super::load_progress(&dir), 3);
        let _ = std::fs::remove_dir_all(dir);
    }
}
