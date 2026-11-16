//! Item icons (16x16, items atlas).

use super::{Painter, Rgb, SpriteList, rnd};
use crate::image::Image;

pub mod items {
    pub const STICK: u16 = 0;
    pub const APPLE: u16 = 1;
    pub const COAL: u16 = 2;
    pub const RAW_IRON: u16 = 3;
    pub const IRON_INGOT: u16 = 4;
    pub const DIAMOND: u16 = 5;
    pub const BREAD: u16 = 6;
    pub const GOLD_INGOT: u16 = 7;
    pub const RAW_GOLD: u16 = 8;
    pub const MUSHROOM_STEW: u16 = 9;
    /// Tools: TOOLS_BASE + material * 4 + kind. material: 0 wood, 1 stone, 2 iron, 3 diamond;
    /// kind: 0 pickaxe, 1 axe, 2 shovel, 3 sword.
    pub const TOOLS_BASE: u16 = 10;
    pub const SHIELD: u16 = 26;
    pub const RAW_COPPER: u16 = 27;
    pub const RAW_TIN: u16 = 28;
    pub const RAW_SILVER: u16 = 29;
    pub const COPPER_INGOT: u16 = 30;
    pub const TIN_INGOT: u16 = 31;
    pub const BRONZE_INGOT: u16 = 32;
    pub const SILVER_INGOT: u16 = 33;
    pub const RAW_MEAT: u16 = 34;
    pub const COOKED_MEAT: u16 = 35;
    pub const ROTTEN_FLESH: u16 = 36;
    /// Armor: ARMOR_BASE + material * 4 + slot (helmet, chestplate, leggings, boots).
    pub const ARMOR_BASE: u16 = 37;
    pub const BOW: u16 = 53;
    pub const ARROW: u16 = 54;
    pub const STRING: u16 = 55;
    pub const FLINT: u16 = 56;
    pub const LEATHER: u16 = 57;
    pub const RAW_URANIUM: u16 = 58;
    pub const URANIUM_INGOT: u16 = 59;
    /// Hoes: HOE_BASE + material (wood, stone, iron, diamond).
    pub const HOE_BASE: u16 = 60;
    pub const WHEAT_SEEDS: u16 = 64;
    pub const WHEAT: u16 = 65;
    pub const CARROT: u16 = 66;
    pub const POTATO: u16 = 67;
    pub const BAKED_POTATO: u16 = 68;
    pub const SLIME: u16 = 69;
    /// Spawn souls, one per creature (order of `SOUL_COLORS`).
    pub const SOUL_BASE: u16 = 70;
    pub const BONE: u16 = 78;
    pub const BONE_MEAL: u16 = 79;
    pub const BOOK: u16 = 80;
    pub const ENCH_BOOK: u16 = 81;
    pub const BOTTLE: u16 = 82;
    /// Potions, one per effect (order of `POTION_COLORS`).
    pub const POTION_BASE: u16 = 83;
    /// Tipped arrows: poison, slowness, harm.
    pub const TIPPED_BASE: u16 = 92;
    pub const REVIVAL_HEART:u16=95;
    pub const WINGS:u16=96;
    pub const RITUAL_SEAL:u16=97;
    pub const RESONANCE_RUNE:u16=98;
    pub const VOLCANIC_CORE:u16=99;
    pub const GUNPOWDER: u16 = 100;
    pub const LIGHTER: u16 = 101;
    pub const GRENADE: u16 = 102;
    pub const GRENADE_LAUNCHER: u16 = 103;
    pub const ROCKET: u16 = 104;
    pub const BAZOOKA: u16 = 105;
    /// «Питомник»: capsule eggs (common, rare, legendary) and the ten pets.
    pub const EGG_BASE: u16 = 106;
    pub const PET_BASE: u16 = 109;
    /// Firearms: pistol, rifle, shotgun, sniper rifle; then bullets and shells.
    pub const PISTOL: u16 = 119;
    pub const RIFLE: u16 = 120;
    pub const SHOTGUN: u16 = 121;
    pub const SNIPER: u16 = 122;
    pub const BULLETS: u16 = 123;
    pub const SHELLS: u16 = 124;
    /// Riding: a saddle and the dragon's soul (spawns a dragon).
    pub const SADDLE: u16 = 125;
    pub const DRAGON_SOUL: u16 = 126;
    pub const RAW_FISH: u16 = 127;
    pub const COOKED_FISH: u16 = 128;
    pub const STAR_REMOTE: u16 = 129;
    pub const STAR_BLASTER: u16 = 130;
    pub const STAR_HAMMER: u16 = 131;
    pub const IRON_DUST: u16 = 132;
    pub const GOLD_DUST: u16 = 133;
    pub const COPPER_DUST: u16 = 134;
    pub const TIN_DUST: u16 = 135;
    pub const SILVER_DUST: u16 = 136;
    pub const CIRCUIT: u16 = 137;
    pub const PLASMA_RIFLE: u16 = 138;
    pub const RAILGUN: u16 = 139;
    pub const ENERGY_SWORD: u16 = 140;
    pub const JETPACK: u16 = 141;
    pub const NANO_BASE: u16 = 142;
    pub const MECH: u16 = 146;
    pub const OVERCLOCKER: u16 = 147;
    pub const STORAGE_UPGRADE: u16 = 148;
    pub const URANIUM_ROD: u16 = 149;
    pub const HEAT_VENT: u16 = 150;
    pub const COOLANT_CELL: u16 = 151;
    pub const REACTOR_PLATING: u16 = 152;
    pub const DRILL: u16 = 153;
    pub const CHAINSAW: u16 = 154;
    pub const IRON_PLATE: u16 = 155;
    pub const COPPER_PLATE: u16 = 156;
    pub const GOLD_PLATE: u16 = 157;
    pub const SCRAP: u16 = 158;
    pub const UU_MATTER: u16 = 159;
    pub const OMNI_TOOL: u16 = 160;
    pub const MINING_LASER: u16 = 161;
    pub const BATTERY: u16 = 162;
    pub const ENERGY_CRYSTAL: u16 = 163;
    // Mod packs: Draconic Evolution, Immersive Engineering, Tinkers' Construct.
    pub const DRACONIUM_DUST: u16 = 164;
    pub const DRACONIUM_INGOT: u16 = 165;
    pub const AWAKENED_DRACONIUM: u16 = 166;
    pub const DRACONIC_CORE: u16 = 167;
    pub const DRACONIC_PICKAXE: u16 = 168;
    pub const DRACONIC_SWORD: u16 = 169;
    pub const COKE: u16 = 170;
    pub const STEEL_INGOT: u16 = 171;
    pub const COBALT_INGOT: u16 = 172;
    pub const MANYULLYN_INGOT: u16 = 173;
    pub const TINKER_HAMMER: u16 = 174;
    pub const TINKER_EXCAVATOR: u16 = 175;
    pub const LUMBER_AXE: u16 = 176;
    pub const CLEAVER: u16 = 177;
    pub const COUNT: u16 = 178;
    /// Liquid colour of each potion: healing, regeneration, speed, strength, resistance,
    /// fire resistance, poison, slowness, harm (same as the engine's effect colours).
    pub const POTION_COLORS: [[f32; 3]; 9] = [
        [0.97, 0.25, 0.35], [0.95, 0.45, 0.75], [0.45, 0.85, 1.0], [0.85, 0.30, 0.15], [0.55, 0.55, 0.70],
        [1.0, 0.62, 0.15], [0.35, 0.75, 0.20], [0.40, 0.45, 0.60], [0.40, 0.08, 0.10],
    ];
    /// Glow of each spawn soul: ghoul, crawler, boar, sheep, cow, chicken, wolf, slime.
    pub const SOUL_COLORS: [[f32; 3]; 8] = [
        [0.55, 0.80, 0.45], [0.62, 0.35, 0.85], [0.80, 0.55, 0.45], [0.95, 0.95, 0.92],
        [0.95, 0.80, 0.85], [1.0, 0.85, 0.40], [0.70, 0.75, 0.85], [0.45, 1.0, 0.45],
    ];
}

// Every tool runs from the handle at the bottom-right (the hand model grips it at about pixel
// 12.5, 12.5) to the head at the top-left.
const PICKAXE: [&str; 16] = [
    "................", "...xxxxxxxxx....", "..xXXXXXXXXXx...", "..xx...hh..xx...", "..x....hh...x...", ".......hh.......",
    "........hh......", "........hh......", ".........hh.....", ".........hh.....", "..........hh....", "..........hh....",
    "...........hh...", "...........hh...", "............h...", "................",
];
const AXE: [&str; 16] = [
    "................", "................", ".hh.............", "xXhh............", "xXXhh...........", "xXXXhh..........",
    "xXXXXhh.........", "xXXXxxhh........", "xxxx...hh.......", "........hh......", ".........hh.....", "..........hh....",
    "...........hh...", "............hh..", ".............hh.", "................",
];
const SHOVEL: [&str; 16] = [
    ".xx.............", "xXXx............", "xXXXx...........", ".xXXx...........", "..xxx...........", "....hh..........",
    ".....hh.........", "......hh........", ".......hh.......", "........hh......", ".........hh.....", "..........hh....",
    "...........hh...", "............hh..", ".............hh.", "................",
];
const SWORD: [&str; 16] = [
    ".x..............", "xXx.............", ".xXx............", "..xXx...........", "...xXx..........", "....xXx.........",
    ".....xXx........", "......xXx....w..", ".......xXx..w...", "........xx.w....", "..........w.....", ".........w.hh...",
    "........w..hh...", ".......w.....hw.", ".............ww.", "................",
];
// Tinkers' hammer: a heavy two-sided block head (digs 3×3).
const HAMMER_TPL: [&str; 16] = [
    "................", ".xxxxxxx........", ".xXXXXXXx.......", ".xXXXXXXx.......", ".xXXXXXXx.......", ".xxxxhhxx.......",
    "......hh........", ".......hh.......", "........hh......", ".........hh.....", "..........hh....", "...........hh...",
    "............hh..", ".............hh.", "..............h.", "................",
];
// Tinkers' excavator: a broad spade (digs 3×3).
const EXCAVATOR_TPL: [&str; 16] = [
    "xxxx............", "xXXXx...........", "xXXXXx..........", "xXXXXx..........", ".xXXXx..........", "..xxxhh.........",
    "......hh........", ".......hh.......", "........hh......", ".........hh.....", "..........hh....", "...........hh...",
    "............hh..", ".............hh.", "..............h.", "................",
];
// Lumber axe: a broad blade on a long handle (fells a whole tree).
const LUMBER_TPL: [&str; 16] = [
    "................", "..hh............", "xXhh............", "xXXhh...........", "xXXXhh..........", "xXXXXhh.........",
    "xXXXXXhh........", "xXXXXxxhh.......", "xXXXx...hh......", "xxxx.....hh.....", "..........hh....", "...........hh...",
    "............hh..", ".............hh.", "..............h.", "................",
];
// Cleaver: a wide heavy blade.
const CLEAVER_TPL: [&str; 16] = [
    "xx..............", "xXxx............", "xXXXx...........", ".xXXXx..........", "..xXXXx.........", "...xXXXx........",
    "....xXXXx.......", ".....xXXXx..w...", "......xXXXx.w...", ".......xXXxw....", "........xxw.....", ".........w.hh...",
    "........w..hh...", ".......w.....hw.", ".............ww.", "................",
];
// Hoe: a short blade bent off the top of the handle (same grip as the other tools).
const HOE: [&str; 16] = [
    "................", "..xxxxx.........", ".xXXXXhh........", ".xx...hh........", ".......hh.......", "........hh......",
    "........hh......", ".........hh.....", ".........hh.....", "..........hh....", "..........hh....", "...........hh...",
    "...........hh...", "............hh..", ".............h..", "................",
];
const LIGHTER_TPL: [&str; 16] = [
    "................",
    ".......y........",
    "......yfy.......",
    "......fyf.......",
    "......fyyf......",
    ".......ff.......",
    ".....mmmmmm.....",
    ".....mmXXmm.....",
    ".....rrrrrr.....",
    ".....rRRRRr.....",
    ".....rRRRRr.....",
    ".....rRRRRr.....",
    ".....rRRRRr.....",
    ".....rRRRRr.....",
    ".....rrrrrr.....",
    "................",
];
const GRENADE_TPL: [&str; 16] = [
    "................",
    "........mm......",
    ".......m..m.....",
    "......mmmm.m....",
    ".....gGGGGg.....",
    "....gGGGGGGg....",
    "...gGGgGGgGGg...",
    "...gGGGGGGGGg...",
    "...gGgGGgGGGg...",
    "...gGGGGGGgGg...",
    "...gGGgGGGGGg...",
    "....gGGGGGGg....",
    ".....gGGGGg.....",
    "......gggg......",
    "................",
    "................",
];
const LAUNCHER_TPL: [&str; 16] = [
    "................",
    "................",
    "...........XX...",
    "..........XmmX..",
    ".........XmmmmX.",
    "........XmmmmX..",
    ".......XmmmmX...",
    "......XmmmmX....",
    ".....XhhmmX.....",
    "....hhhXmX......",
    "...hhhh.X.......",
    "..hhhh..........",
    ".hhhh...........",
    ".hhh............",
    "................",
    "................",
];
const ROCKET_TPL: [&str; 16] = [
    "................",
    "............rr..",
    "...........rRRr.",
    "..........mRRr..",
    ".........mmmr...",
    "........mmmm....",
    ".......mmmm.....",
    "......mmmm......",
    ".....mmmm.......",
    "....mmmm........",
    "...fmmm.........",
    "..fffm..........",
    ".fyff...........",
    ".yf.............",
    "................",
    "................",
];
const BAZOOKA_TPL: [&str; 16] = [
    "................",
    "..............X.",
    ".............XoX",
    "............XooX",
    "...........XooX.",
    "..........XooX..",
    ".........XooX...",
    "........XooX....",
    ".......XooXh....",
    "......XooXhh....",
    ".....XooX.h.....",
    "....XooX........",
    "...XooX.........",
    "..XooX..........",
    "..XXX...........",
    "................",
];
const PISTOL_TPL: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "..XXXXXXXXXXX...",
    "..XmmmmmmmmmXX..",
    "..XmmmmmmmmmmX..",
    "..XXXXXXhhhXXX..",
    "......XXhhhX....",
    ".......XhhhX....",
    "........hhhX....",
    "........hhhh....",
    "........hhhh....",
    "........XXXX....",
    "................",
    "................",
];
const RIFLE_TPL: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "........XX......",
    "XXXXXXXXmmXXX...",
    "XmmmmmmmmmmmmXX.",
    ".XXXXXmmmmmmhhhh",
    ".....XXmXXXhhhhh",
    ".....XmmX.hhh.hh",
    ".....XmmX.hh....",
    "......XmmX......",
    "......XmmX......",
    ".......XX.......",
    "................",
    "................",
];
const SHOTGUN_TPL: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "XXXXXXXXXXX.....",
    "mmmmmmmmmmmX....",
    "XXXXXXXXXXmXhh..",
    "..XhhhhhhXXhhhh.",
    "...XXXXXX.hhhhhh",
    "..........hh.hhh",
    "..........h...hh",
    "................",
    "................",
    "................",
    "................",
];
const SNIPER_TPL: [&str; 16] = [
    "................",
    "................",
    "................",
    "........XXXXX...",
    "........XLmmX...",
    ".........XXX....",
    "XXXXXXXXXmmXXX..",
    "mmmmmmmmmmmmmhhh",
    "XXXXXXXXXmXhhhhh",
    "........XmXhh.hh",
    ".........XXh....",
    "................",
    "................",
    "................",
    "................",
    "................",
];
const BULLETS_TPL: [&str; 16] = [
    "................",
    "................",
    "...y.....y......",
    "..yyy...yyy.....",
    "..yyy...yyy.....",
    "..ccc...ccc..y..",
    "..ccc...ccc.yyy.",
    "..ccc...ccc.yyy.",
    "..ccc...ccc.ccc.",
    "..ccc...ccc.ccc.",
    "..ccc...ccc.ccc.",
    "..CCC...CCC.ccc.",
    ".............ccc",
    "............CCC.",
    "................",
    "................",
];
const STAR_HAMMER_TPL: [&str; 16] = [
    "................",
    ".....KKKKKKK....",
    "....KgggggggK...",
    "....KgGCCCGgK...",
    "....KgCCCCCgK...",
    "....KgGCCCGgK...",
    "....KGGGGGGGK...",
    ".....KKKBKKK....",
    "........B.......",
    "........B.......",
    ".......B........",
    ".......B........",
    "......B.........",
    "......B.........",
    ".....KK.........",
    "................",
];
const STAR_BLASTER_TPL: [&str; 16] = [
    "................",
    "................",
    "................",
    "...........C....",
    "..KKKKKKKKKKCC..",
    ".KgggggGGGGGKCC.",
    ".KGGGGGGCCCGK...",
    ".KKKKKGGGGGGK...",
    ".....KGYKKKKK...",
    ".....KGYK.......",
    "....KGGK........",
    "....KGGK........",
    "...KGGK.........",
    "...KKKK.........",
    "................",
    "................",
];
const STAR_REMOTE_TPL: [&str; 16] = [
    "..........Y.....",
    ".........YYY....",
    "..........Y.....",
    ".....KKKKKK.....",
    "....KGGGGGGK....",
    "....KCCCCCCK....",
    "....KGGGGGGK....",
    "....KGRrRRGK....",
    "....KGRRRRGK....",
    "....KGRRRRGK....",
    "....KGGGGGGK....",
    "....KGYGGYGK....",
    "....KGGGGGGK....",
    "....KGYGGYGK....",
    ".....KKKKKK.....",
    "................",
];
const SADDLE_TPL: [&str; 16] = [
    "................",
    "................",
    "................",
    "...DD......DD...",
    "..DLLD....DLLD..",
    "..DLlLDDDDLlLD..",
    "..DLllllllllLD..",
    "...DLllllllLD...",
    "....DLLLLLLD....",
    ".....DDDDDD.....",
    "......D..D......",
    "......D..D......",
    ".....MM..MM.....",
    ".....MM..MM.....",
    "................",
    "................",
];
const SHELLS_TPL: [&str; 16] = [
    "................",
    "................",
    "................",
    "..RRR....RRR....",
    "..RRR....RRR....",
    "..RRR....RRR....",
    "..rRR....rRR....",
    "..RRR....RRR....",
    "..RRR....RRR....",
    "..rRR....rRR....",
    "..ccc....ccc....",
    "..CCC....CCC....",
    "................",
    "................",
    "................",
    "................",
];
const MATERIALS: [Rgb; 4] = [Rgb(0.62, 0.45, 0.25), Rgb(0.55, 0.55, 0.55), Rgb(0.85, 0.85, 0.88), Rgb(0.35, 0.90, 0.85)];

fn p(f: impl FnOnce(&mut Painter)) -> Image {
    let mut q = Painter::new(16, 16);
    f(&mut q);
    q.finish()
}

fn blob(c: Rgb, r: f32, seed: u32) -> Image {
    p(|q| {
        for y in 0..16 {
            for x in 0..16 {
                let dx = x as f32 - 7.5;
                let dy = y as f32 - 8.0;
                let d = (dx * dx + dy * dy).sqrt() + (rnd(x, y, seed) - 0.5) * 1.5;
                if d < r {
                    q.put(x, y, c.scale(1.0 - (dx + dy) * 0.025), 1.0);
                }
            }
        }
    })
}

const HELMET: [&str; 16] = [
    "................", "................", "....xxxxxxxx....", "...xXXXXXXXXx...", "..xXXhhhhhhXXx..", "..xXhXXXXXXhXx..",
    "..xXXXXXXXXXXx..", "..xXXXXXXXXXXx..", "..xXXx....xXXx..", "..xXx......xXx..", "..xx........xx..", "................",
    "................", "................", "................", "................",
];
const CHEST: [&str; 16] = [
    "................", "..xxx......xxx..", ".xXXXx....xXXXx.", ".xXXXXxxxxXXXXx.", ".xXXXXXXXXXXXXx.", ".xXhXXXXXXXXhXx.",
    ".xXXXXXXXXXXXXx.", "..xXXXXXXXXXXx..", "..xXXXhhhhXXXx..", "..xXXXXXXXXXXx..", "..xXXXXXXXXXXx..", "..xXXXXXXXXXXx..",
    "..xXXXXXXXXXXx..", "..xxxxxxxxxxxx..", "................", "................",
];
const LEGS: [&str; 16] = [
    "................", "...xxxxxxxxxx...", "..xXXXXXXXXXXx..", "..xXhhhhhhhhXx..", "..xXXXXXXXXXXx..", "..xXXXXXXXXXXx..",
    "..xXXXxxxxXXXx..", "..xXXXx..xXXXx..", "..xXXXx..xXXXx..", "..xXXXx..xXXXx..", "..xXXXx..xXXXx..", "..xXXXx..xXXXx..",
    "..xXXXx..xXXXx..", "..xxxxx..xxxxx..", "................", "................",
];
const BOOTS: [&str; 16] = [
    "................", "................", "................", "................", "..xxxx....xxxx..", "..xXXx....xXXx..",
    "..xXXx....xXXx..", "..xXXx....xXXx..", "..xXXx....xXXx..", ".xXXXXx..xXXXXx.", ".xXhhXXx.xXhhXXx", ".xXXXXXx.xXXXXXx",
    ".xxxxxxx.xxxxxxx", "................", "................", "................",
];

/// A fish lying sideways: oval body, forked tail on the left, an eye on the right.
fn fish(body: Rgb, fin: Rgb) -> Image {
    p(move |q| {
        for y in 0..16u32 {
            for x in 0..16u32 {
                let dx = (x as f32 - 8.5) / 5.5;
                let dy = (y as f32 - 8.0) / 3.2;
                if dx * dx + dy * dy <= 1.0 {
                    let shade = if y < 7 { 0.85 } else if y > 9 { 1.12 } else { 1.0 };
                    q.put(x, y, body.scale(shade), 1.0);
                }
                // Tail.
                let tx = 3i32 - x as i32;
                if x <= 3 && (y as i32 - 8).abs() <= tx + 2 && (y as i32 - 8).abs() >= tx - 1 {
                    q.put(x, y, fin, 1.0);
                }
            }
        }
        for x in 7..10u32 {
            q.put(x, 5, fin, 1.0);
        }
        q.put(12, 7, Rgb(0.05, 0.05, 0.08), 1.0);
    })
}

fn meat(c: Rgb, bone: Rgb) -> Image {
    p(|q| {
        for y in 4..13u32 {
            for x in 3..13u32 {
                let dx = x as f32 - 7.5;
                let dy = y as f32 - 8.5;
                if (dx / 4.8).powi(2) + (dy / 4.0).powi(2) < 1.0 {
                    q.put(x, y, c.add((rnd(x, y, 170) - 0.5) * 0.1 - dy * 0.02), 1.0);
                }
            }
        }
        for i in 0..4u32 {
            q.put(11 + i / 2, 3 - i / 2 + 1, bone, 1.0);
            q.put(12 + i / 2, 2 + i / 2, bone, 1.0);
        }
    })
}

fn ingot(c: Rgb) -> Image {
    // A bar seen from the front and a little from above: a light top face over a front face.
    p(|q| {
        for y in 5..12u32 {
            for x in 1..15u32 {
                let top = y < 8;
                // Top face is a trapezoid (narrower at the back), the front is a full rectangle.
                let inset = if top { 8 - y } else { 0 };
                if x < 1 + inset || x >= 15 - inset {
                    continue;
                }
                let col = if top { c.add(0.12) } else if y == 11 { c.scale(0.7) } else { c.scale(0.92) };
                q.put(x, y, col, 1.0);
            }
        }
        // A highlight along the front edge of the top.
        for x in 3..12u32 {
            q.put(x, 7, c.add(0.22), 1.0);
        }
    })
}

pub(crate) fn build() -> Vec<(String, Image)> {
    use items::*;
    let mut l = SpriteList::new();
    l.add(
        STICK,
        "stick",
        p(|q| {
            for i in 2..14u32 {
                q.put(i, 15 - i, Rgb(0.50, 0.36, 0.20), 1.0);
                q.put(i + 1, 15 - i, Rgb(0.40, 0.28, 0.15), 1.0);
            }
        }),
    );
    l.add(APPLE, "apple", {
        let mut img = blob(Rgb(0.85, 0.12, 0.12), 5.5, 50);
        img.set(8, 2, [102, 71, 38, 255]);
        img.set(8, 1, [102, 71, 38, 255]);
        img.set(9, 2, [64, 153, 51, 255]);
        img
    });
    l.add(COAL, "coal", blob(Rgb(0.15, 0.15, 0.17), 5.0, 51));
    l.add(RAW_IRON, "raw_iron", blob(Rgb(0.78, 0.60, 0.46), 5.0, 52));
    l.add(IRON_INGOT, "iron_ingot", ingot(Rgb(0.78, 0.78, 0.80)));
    l.add(DIAMOND, "diamond", blob(Rgb(0.45, 0.95, 0.90), 4.5, 53));
    l.add(
        BREAD,
        "bread",
        p(|q| {
            for y in 6..12u32 {
                for x in 2..14u32 {
                    q.put(x, y, Rgb(0.80, 0.58, 0.30).add((rnd(x, y, 54) - 0.5) * 0.08), 1.0);
                }
            }
        }),
    );
    l.add(GOLD_INGOT, "gold_ingot", ingot(Rgb(0.95, 0.80, 0.25)));
    l.add(RAW_GOLD, "raw_gold", blob(Rgb(0.92, 0.75, 0.25), 5.0, 55));
    l.add(
        MUSHROOM_STEW,
        "mushroom_stew",
        p(|q| {
            q.shape(Rgb(0.45, 0.30, 0.18), &|u, v| v > 0.5 && ((u - 0.5) / 0.42).powi(2) + ((v - 0.5) / 0.35).powi(2) < 1.0);
            q.shape(Rgb(0.70, 0.50, 0.32), &|u, v| ((u - 0.5) / 0.38).powi(2) + ((v - 0.52) / 0.08).powi(2) < 1.0);
        }),
    );
    let templates: [&[&str; 16]; 4] = [&PICKAXE, &AXE, &SHOVEL, &SWORD];
    let mat_names = ["wood", "stone", "iron", "diamond"];
    let kind_names = ["pickaxe", "axe", "shovel", "sword"];
    for (mi, m) in MATERIALS.iter().enumerate() {
        for (ki, t) in templates.iter().enumerate() {
            let m = *m;
            let img = p(|q| {
                q.template(&t[..], &|ch| match ch {
                    'x' => Some(m.scale(0.75)),
                    'X' => Some(m),
                    'h' => Some(Rgb(0.45, 0.32, 0.18)),
                    'w' => Some(Rgb(0.35, 0.25, 0.15)),
                    _ => None,
                })
            });
            l.add(TOOLS_BASE + (mi * 4 + ki) as u16, &format!("{}_{}", mat_names[mi], kind_names[ki]), img);
        }
    }
    l.add(
        SHIELD,
        "shield",
        p(|q| {
            for y in 1..15u32 {
                for x in 3..13u32 {
                    let rim = x == 3 || x == 12 || y == 1 || y == 14 || (y == 13 && (x == 4 || x == 11));
                    let c = if rim { Rgb(0.60, 0.62, 0.66) } else { Rgb(0.62, 0.45, 0.25).add((rnd(x, y, 77) - 0.5) * 0.06 + if x % 3 == 0 { -0.05 } else { 0.0 }) };
                    if !(y == 14 && (x == 3 || x == 12)) {
                        q.put(x, y, c, 1.0);
                    }
                }
            }
            for y in 6..9u32 {
                for x in 7..9u32 {
                    q.put(x, y, Rgb(0.72, 0.74, 0.78), 1.0);
                }
            }
        }),
    );
    l.add(RAW_COPPER, "raw_copper", blob(Rgb(0.80, 0.48, 0.30), 5.0, 160));
    l.add(RAW_TIN, "raw_tin", blob(Rgb(0.74, 0.78, 0.80), 5.0, 161));
    l.add(RAW_SILVER, "raw_silver", blob(Rgb(0.86, 0.88, 0.94), 5.0, 162));
    l.add(COPPER_INGOT, "copper_ingot", ingot(Rgb(0.85, 0.50, 0.32)));
    l.add(TIN_INGOT, "tin_ingot", ingot(Rgb(0.78, 0.82, 0.84)));
    l.add(BRONZE_INGOT, "bronze_ingot", ingot(Rgb(0.74, 0.54, 0.24)));
    l.add(SILVER_INGOT, "silver_ingot", ingot(Rgb(0.90, 0.92, 0.98)));
    l.add(RAW_MEAT, "raw_meat", meat(Rgb(0.82, 0.36, 0.38), Rgb(0.95, 0.75, 0.75)));
    l.add(COOKED_MEAT, "cooked_meat", meat(Rgb(0.55, 0.30, 0.16), Rgb(0.80, 0.55, 0.30)));
    l.add(ROTTEN_FLESH, "rotten_flesh", meat(Rgb(0.45, 0.50, 0.30), Rgb(0.60, 0.65, 0.40)));
    let armor_mats = [Rgb(0.55, 0.36, 0.20), Rgb(0.74, 0.54, 0.24), Rgb(0.82, 0.83, 0.86), Rgb(0.40, 0.90, 0.88)];
    let armor_tpl: [&[&str; 16]; 4] = [&HELMET, &CHEST, &LEGS, &BOOTS];
    let armor_names = ["helmet", "chestplate", "leggings", "boots"];
    for (mi, m) in armor_mats.iter().enumerate() {
        for (si, t) in armor_tpl.iter().enumerate() {
            let m = *m;
            let img = p(|q| {
                q.template(&t[..], &|ch| match ch {
                    'x' => Some(m.scale(0.72)),
                    'X' => Some(m),
                    'h' => Some(m.add(0.12)),
                    _ => None,
                })
            });
            l.add(ARMOR_BASE + (mi * 4 + si) as u16, &format!("armor_{}_{}", mi, armor_names[si]), img);
        }
    }
    l.add(BOW, "bow", p(|q| {
        // Wooden arc on the left, string on the right.
        let wood = Rgb(0.55, 0.38, 0.20);
        for y in 1..15u32 {
            let t = (y as f32 - 7.5) / 7.0;
            let x = (4.0 + (t * t) * 5.0) as u32;
            q.put(x, y, wood, 1.0);
            q.put(x + 1, y, wood.scale(0.8), 1.0);
        }
        for y in 2..14u32 {
            q.put(10, y, Rgb(0.90, 0.90, 0.86), 1.0);
        }
    }));
    l.add(ARROW, "arrow", p(|q| {
        for i in 3..13u32 {
            q.put(i, 15 - i, Rgb(0.55, 0.40, 0.22), 1.0);
        }
        for (x, y) in [(12u32, 2u32), (13, 2), (13, 3), (12, 3), (11, 3), (12, 4)] {
            q.put(x, y, Rgb(0.70, 0.72, 0.76), 1.0);
        }
        for (x, y) in [(2u32, 12u32), (2, 13), (3, 13), (3, 14), (1, 12), (2, 11)] {
            q.put(x, y, Rgb(0.92, 0.92, 0.90), 1.0);
        }
    }));
    l.add(STRING, "string", p(|q| {
        for i in 2..14u32 {
            let y = 8.0 + ((i as f32) * 0.9).sin() * 3.0;
            q.put(i, y as u32, Rgb(0.92, 0.92, 0.90), 1.0);
        }
    }));
    l.add(FLINT, "flint", blob(Rgb(0.22, 0.22, 0.24), 4.5, 90));
    l.add(LEATHER, "leather", p(|q| {
        q.shape(Rgb(0.58, 0.36, 0.20), &|u, v| ((u - 0.5) / 0.38).powi(2) + ((v - 0.5) / 0.32).powi(2) < 1.0 && !(v < 0.25 && (u - 0.5).abs() < 0.12));
    }));
    l.add(RAW_URANIUM, "raw_uranium", blob(Rgb(0.42, 0.88, 0.30), 5.0, 163));
    l.add(URANIUM_INGOT, "uranium_ingot", ingot(Rgb(0.40, 0.90, 0.28)));
    for (mi, m) in MATERIALS.iter().enumerate() {
        let m = *m;
        let img = p(|q| {
            q.template(&HOE[..], &|ch| match ch {
                'x' => Some(m.scale(0.75)),
                'X' => Some(m),
                'h' => Some(Rgb(0.45, 0.32, 0.18)),
                _ => None,
            })
        });
        l.add(HOE_BASE + mi as u16, &format!("hoe_{mi}"), img);
    }
    l.add(WHEAT_SEEDS, "wheat_seeds", p(|q| {
        // A small scatter of seeds.
        for (i, (x, y)) in [(4u32, 9u32), (7, 7), (10, 9), (6, 11), (9, 12), (12, 6), (5, 5), (11, 11)].into_iter().enumerate() {
            let c = if i % 2 == 0 { Rgb(0.42, 0.62, 0.20) } else { Rgb(0.62, 0.70, 0.28) };
            q.put(x, y, c, 1.0);
            q.put(x + 1, y, c.scale(0.8), 1.0);
            q.put(x, y + 1, c.scale(0.7), 1.0);
        }
    }));
    l.add(WHEAT, "wheat", p(|q| {
        // A sheaf: golden stalks gathered in the middle, ears on top.
        for i in 0..5i32 {
            let x0 = 4 + i * 2;
            for y in 2..15i32 {
                let x = x0 + ((y - 9) * (i - 2)) / 7;
                if !(0..16).contains(&x) {
                    continue;
                }
                let ear = y < 7;
                let c = if ear { Rgb(0.88, 0.70, 0.28).add(if y % 2 == 0 { 0.06 } else { -0.04 }) } else { Rgb(0.78, 0.66, 0.34) };
                q.put(x as u32, y as u32, c, 1.0);
                if ear {
                    q.put((x + 1).min(15) as u32, y as u32, c.scale(0.85), 1.0);
                }
            }
        }
        for x in 5..12u32 {
            q.put(x, 10, Rgb(0.55, 0.40, 0.20), 1.0);
        }
    }));
    l.add(CARROT, "carrot", p(|q| {
        // Orange root on a diagonal with green leaves at the top.
        for i in 0..10i32 {
            let (cx, cy) = (11 - i, 4 + i);
            let r = (3.0 - i as f32 * 0.28).max(0.6);
            for dy in -2..=2i32 {
                for dx in -2..=2i32 {
                    if ((dx * dx + dy * dy) as f32).sqrt() <= r {
                        let (x, y) = (cx + dx, cy + dy);
                        if (0..16).contains(&x) && (0..16).contains(&y) {
                            let c = Rgb(0.98, 0.55, 0.12).add(if (x + y) % 4 == 0 { -0.12 } else { 0.0 }).scale(1.0 - dx as f32 * 0.04);
                            q.put(x as u32, y as u32, c, 1.0);
                        }
                    }
                }
            }
        }
        for (x, y) in [(12u32, 1u32), (13, 2), (14, 1), (12, 3), (13, 4), (11, 0), (14, 3), (15, 2)] {
            q.put(x, y, Rgb(0.30, 0.72, 0.22), 1.0);
        }
    }));
    let potato = |baked: bool| {
        p(move |q| {
            for y in 3..14u32 {
                for x in 2..14u32 {
                    let dx = (x as f32 - 7.8) / 5.6;
                    let dy = (y as f32 - 8.2) / 4.6;
                    if dx * dx + dy * dy > 1.0 {
                        continue;
                    }
                    let base = if baked { Rgb(0.80, 0.55, 0.25) } else { Rgb(0.78, 0.64, 0.38) };
                    let eye = rnd(x, y, if baked { 171 } else { 172 }) < 0.08;
                    let c = if eye { base.scale(0.65) } else { base.add(-(dx + dy) * 0.08) };
                    q.put(x, y, c, 1.0);
                }
            }
            if baked {
                // A split top with steam-soft yellow flesh.
                for x in 5..11u32 {
                    q.put(x, 6, Rgb(1.0, 0.88, 0.50), 1.0);
                    q.put(x, 7, Rgb(0.96, 0.80, 0.42), 1.0);
                }
            }
        })
    };
    l.add(POTATO, "potato", potato(false));
    l.add(BAKED_POTATO, "baked_potato", potato(true));
    l.add(SLIME, "slime", p(|q| {
        // A wobbly green ball of slime with a highlight.
        for y in 3..14u32 {
            for x in 2..14u32 {
                let dx = (x as f32 - 7.8) / 5.6;
                let dy = (y as f32 - 8.6) / 5.0;
                if dx * dx + dy * dy > 1.0 {
                    continue;
                }
                let c = Rgb(0.40, 0.85, 0.36).add(-(dx + dy) * 0.08);
                q.put(x, y, c, 1.0);
            }
        }
        for (x, y) in [(5u32, 6u32), (6, 5), (6, 6)] {
            q.put(x, y, Rgb(0.85, 1.0, 0.80), 1.0);
        }
    }));
    for (i, c) in SOUL_COLORS.iter().enumerate() {
        let c = Rgb(c[0], c[1], c[2]);
        l.add(SOUL_BASE + i as u16, &format!("soul_{i}"), p(move |q| {
            // A glowing wisp: a bright orb with a flickering flame tail and two little eyes.
            for y in 0..16u32 {
                for x in 0..16u32 {
                    let dx = x as f32 - 7.5;
                    let dy = y as f32 - 9.5;
                    let d = (dx * dx + dy * dy).sqrt();
                    let flame = y < 9 && dx.abs() < (9.0 - y as f32) * 0.45 + if (x + y) % 3 == 0 { 0.6 } else { 0.0 } && y > 1;
                    if d < 5.2 {
                        let k = 1.0 - d / 5.2;
                        q.put(x, y, c.lerp(Rgb(1.0, 1.0, 1.0), k * 0.7), 1.0);
                    } else if flame {
                        q.put(x, y, c.scale(0.9), 1.0);
                    }
                }
            }
            for x in [6u32, 9] {
                q.put(x, 10, Rgb(0.10, 0.10, 0.14), 1.0);
            }
        }));
    }
    l.add(BONE, "bone", p(|q| {
        // A bone lying diagonally: a shaft with two round knobs at each end.
        let ivory = Rgb(0.95, 0.92, 0.82);
        for i in 3..13u32 {
            q.put(i, 15 - i, ivory.add(-0.04), 1.0);
            q.put(i, 14 - i, ivory, 1.0);
        }
        for (cx, cy) in [(2.5f32, 12.0f32), (4.0, 13.5), (12.0, 2.5), (13.5, 4.0)] {
            for y in 0..16u32 {
                for x in 0..16u32 {
                    let (dx, dy) = (x as f32 - cx, y as f32 - cy);
                    if dx * dx + dy * dy < 2.3 {
                        q.put(x, y, ivory.add(-(dx + dy) * 0.03), 1.0);
                    }
                }
            }
        }
    }));
    l.add(BONE_MEAL, "bone_meal", p(|q| {
        // A small heap of white powder.
        for y in 6..15u32 {
            for x in 2..14u32 {
                let dx = (x as f32 - 7.5) / 6.0;
                let top = 15.0 - 8.0 * (1.0 - dx * dx).max(0.0);
                if (y as f32) < top {
                    continue;
                }
                let c = Rgb(0.97, 0.96, 0.92).add((rnd(x, y, 181) - 0.5) * 0.10 - (y as f32 - 10.0) * 0.012);
                q.put(x, y, c, 1.0);
            }
        }
    }));
    // A book: leather cover, pages on the right edge; the enchanted one has a red rune.
    let book = |ench: bool| {
        p(move |q| {
            for y in 2..14u32 {
                for x in 3..13u32 {
                    let c = if x >= 11 {
                        Rgb(0.95, 0.92, 0.84).add(if y % 2 == 0 { -0.05 } else { 0.0 })
                    } else if x == 3 {
                        Rgb(0.40, 0.20, 0.12)
                    } else if ench {
                        Rgb(0.42, 0.14, 0.28)
                    } else {
                        Rgb(0.60, 0.32, 0.18).add((rnd(x, y, 183) - 0.5) * 0.05)
                    };
                    q.put(x, y, c, 1.0);
                }
            }
            if ench {
                // A glowing red rune on the cover.
                for (x, y) in [(7u32, 4u32), (6, 5), (8, 5), (5, 6), (9, 6), (7, 6), (7, 7), (7, 8), (6, 9), (8, 9), (7, 10)] {
                    q.put(x, y, Rgb(1.0, 0.30, 0.25), 1.0);
                }
            } else {
                for x in 5..10u32 {
                    q.put(x, 5, Rgb(0.85, 0.70, 0.30), 1.0);
                }
            }
        })
    };
    l.add(BOOK, "book", book(false));
    l.add(ENCH_BOOK, "enchanted_book", book(true));
    // Bottles: a round flask with a neck and a cork; potions are filled with colour.
    let flask = |liquid: Option<Rgb>| {
        p(move |q| {
            for y in 1..16u32 {
                for x in 2..14u32 {
                    let (fx, fy) = (x as f32 - 7.5, y as f32 - 10.0);
                    let body = fx * fx / 30.0 + fy * fy / 22.0 < 1.0;
                    let neck = (6..10).contains(&x) && (3..6).contains(&y);
                    let cork = (6..10).contains(&x) && (1..3).contains(&y);
                    if cork {
                        q.put(x, y, Rgb(0.62, 0.45, 0.28), 1.0);
                    } else if body || neck {
                        let glass = Rgb(0.78, 0.88, 0.95);
                        let c = match liquid {
                            Some(l) if body && y >= 8 => l.add(-(fy * 0.03)),
                            _ => glass.scale(if body { 0.85 } else { 0.95 }),
                        };
                        q.put(x, y, c, 1.0);
                    }
                }
            }
            // Shine on the glass.
            for (x, y) in [(5u32, 8u32), (5, 9), (4, 10)] {
                q.put(x, y, Rgb(1.0, 1.0, 1.0), 1.0);
            }
        })
    };
    l.add(BOTTLE, "bottle", flask(None));
    for (i, c) in POTION_COLORS.iter().enumerate() {
        l.add(POTION_BASE + i as u16, &format!("potion_{i}"), flask(Some(Rgb(c[0], c[1], c[2]))));
    }
    for (i, ci) in [6usize, 7, 8].into_iter().enumerate() {
        let c = POTION_COLORS[ci];
        let tip = Rgb(c[0], c[1], c[2]);
        l.add(TIPPED_BASE + i as u16, &format!("tipped_arrow_{i}"), p(move |q| {
            for i in 3..13u32 {
                q.put(i, 15 - i, Rgb(0.55, 0.40, 0.22), 1.0);
            }
            // The head dipped in the potion.
            for (x, y) in [(12u32, 2u32), (13, 2), (13, 3), (12, 3), (11, 3), (12, 4), (11, 4), (10, 5)] {
                q.put(x, y, tip, 1.0);
            }
            for (x, y) in [(2u32, 12u32), (2, 13), (3, 13), (3, 14), (1, 12), (2, 11)] {
                q.put(x, y, Rgb(0.92, 0.92, 0.90), 1.0);
            }
        }));
    }
    // Guardian rewards, drawn like the rest of our icons (bright, with a highlight).
    l.add(REVIVAL_HEART, "revival_heart", p(|q| {
        // A golden heart with a green living core.
        for y in 2..14u32 {
            for x in 1..15u32 {
                let (fx, fy) = ((x as f32 - 7.5) / 6.5, (y as f32 - 6.5) / 6.5);
                let heart = (fx * fx + fy * fy - 0.55).powi(3) + fx * fx * fy * fy * fy * -1.0 < 0.0 || ((fx.abs() - 0.45).powi(2) + (fy + 0.35).powi(2) < 0.22);
                let lower = fy > 0.0 && fx.abs() < 0.95 - fy * 0.95;
                if (heart && fy <= 0.2) || lower {
                    let core = fx.abs() + (fy - 0.05).abs() < 0.45;
                    let c = if core { Rgb(0.30, 0.95, 0.70) } else { Rgb(1.0, 0.78, 0.22).add(-(fx + fy) * 0.07) };
                    q.put(x, y, c, 1.0);
                }
            }
        }
        q.put(5, 4, Rgb(1.0, 1.0, 0.9), 1.0);
        q.put(4, 5, Rgb(1.0, 1.0, 0.9), 1.0);
    }));
    l.add(WINGS, "wind_wings", p(|q| {
        // Two feathered wings spread from a small harness.
        for side in [0u32, 1] {
            for (row, len) in [(3u32, 6u32), (4, 7), (5, 7), (6, 6), (7, 6), (8, 5), (9, 4), (10, 3), (11, 2)] {
                for k in 0..len {
                    let x = if side == 0 { 6 - k.min(6) } else { 9 + k };
                    let feather = (k + row) % 2 == 0;
                    let c = if k + 1 == len { Rgb(0.85, 0.96, 1.0) } else if feather { Rgb(0.45, 0.82, 0.98) } else { Rgb(0.32, 0.66, 0.90) };
                    q.put(x, row + k / 3, c, 1.0);
                }
            }
        }
        for y in 5..11u32 {
            q.put(7, y, Rgb(0.62, 0.45, 0.28), 1.0);
            q.put(8, y, Rgb(0.50, 0.35, 0.20), 1.0);
        }
    }));
    for (id, name, color, glow) in [
        (RITUAL_SEAL, "ritual_seal", Rgb(0.92, 0.72, 0.36), Rgb(0.55, 0.20, 0.12)),
        (RESONANCE_RUNE, "resonance_rune", Rgb(0.62, 0.38, 0.98), Rgb(0.95, 0.85, 1.0)),
        (VOLCANIC_CORE, "volcanic_core", Rgb(0.30, 0.12, 0.10), Rgb(1.0, 0.62, 0.15)),
    ] {
        l.add(id, name, p(move |q| {
            // A round token with a rim, its own sign in the middle and a highlight.
            for y in 1..15u32 {
                for x in 1..15u32 {
                    let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d > 6.6 {
                        continue;
                    }
                    let rim = d > 5.2;
                    let sign = match id {
                        RITUAL_SEAL => (dx.abs() < 0.8 && dy.abs() < 3.6) || (dy.abs() < 0.8 && dx.abs() < 3.6) || (d > 3.0 && d < 4.0),
                        RESONANCE_RUNE => (dx.abs() < 0.8 && dy.abs() < 4.0) || ((dx - dy).abs() < 0.9 && dy < 0.5 && dx.abs() < 3.0) || ((dx + dy).abs() < 0.9 && dy < 0.5 && dx.abs() < 3.0),
                        _ => (dx.abs() * 1.3 + dy.abs() < 3.6) || (rnd(x, y, 305) < 0.18 && d < 5.0),
                    };
                    let c = if sign { glow } else if rim { color.scale(0.68) } else { color.add(-(dx + dy) * 0.02) };
                    q.put(x, y, c, 1.0);
                }
            }
            q.put(4, 4, Rgb(1.0, 1.0, 0.95), 1.0);
            q.put(5, 3, Rgb(1.0, 1.0, 0.95), 1.0);
        }));
    }
    // Explosives and weapons.
    l.add(GUNPOWDER, "gunpowder", blob(Rgb(0.30, 0.30, 0.33), 5.0, 172));
    let pal = |ch: char| match ch {
        'y' => Some(Rgb(1.0, 0.90, 0.35)),
        'f' => Some(Rgb(1.0, 0.50, 0.15)),
        'm' => Some(Rgb(0.62, 0.64, 0.68)),
        'X' => Some(Rgb(0.22, 0.24, 0.22)),
        'r' => Some(Rgb(0.55, 0.10, 0.10)),
        'R' => Some(Rgb(0.86, 0.20, 0.16)),
        'g' => Some(Rgb(0.18, 0.30, 0.14)),
        'G' => Some(Rgb(0.32, 0.48, 0.24)),
        'h' => Some(Rgb(0.42, 0.30, 0.18)),
        'o' => Some(Rgb(0.40, 0.46, 0.26)),
        _ => None,
    };
    l.add(LIGHTER, "lighter", p(|q| q.template(&LIGHTER_TPL[..], &pal)));
    l.add(GRENADE, "grenade", p(|q| q.template(&GRENADE_TPL[..], &pal)));
    l.add(GRENADE_LAUNCHER, "grenade_launcher", p(|q| q.template(&LAUNCHER_TPL[..], &pal)));
    l.add(ROCKET, "rocket", p(|q| q.template(&ROCKET_TPL[..], &pal)));
    l.add(BAZOOKA, "bazooka", p(|q| q.template(&BAZOOKA_TPL[..], &pal)));
    // Capsule eggs: an egg with a coloured capsule band and a shine.
    for (i, band) in [Rgb(0.62, 0.66, 0.74), Rgb(0.30, 0.55, 0.98), Rgb(1.0, 0.75, 0.15)].into_iter().enumerate() {
        let shell = [Rgb(0.96, 0.94, 0.88), Rgb(0.82, 0.90, 1.0), Rgb(1.0, 0.93, 0.70)][i];
        l.add(EGG_BASE + i as u16, ["egg_common", "egg_rare", "egg_legendary"][i], p(move |q| {
            q.shape(shell, &|u, v| {
                let (x, y) = ((u - 0.5) / 0.36, (v - 0.55) / if v < 0.55 { 0.46 } else { 0.38 });
                x * x + y * y < 1.0
            });
            for y in 0..16u32 {
                for x in 0..16u32 {
                    let (u, v) = ((x as f32 + 0.5) / 16.0, (y as f32 + 0.5) / 16.0);
                    let (ex, ey) = ((u - 0.5) / 0.36, (v - 0.55) / if v < 0.55 { 0.46 } else { 0.38 });
                    if ex * ex + ey * ey < 1.0 && (7..=9).contains(&y) {
                        q.put(x, y, if y == 8 { band } else { band.scale(0.75) }, 1.0);
                    }
                }
            }
            q.put(5, 4, Rgb(1.0, 1.0, 1.0), 1.0);
            q.put(5, 5, Rgb(1.0, 1.0, 1.0), 1.0);
            q.put(6, 3, Rgb(1.0, 1.0, 1.0), 1.0);
        }));
    }
    // Pets: a round little creature in its colours, ears or comb, eyes; a rarity gem.
    let pets: [(&str, Rgb, Rgb, u8, Rgb); 10] = [
        ("pet_chick", Rgb(0.98, 0.96, 0.90), Rgb(0.95, 0.25, 0.20), 0, Rgb(0.85, 0.87, 0.90)),
        ("pet_sheep", Rgb(0.93, 0.91, 0.84), Rgb(0.40, 0.34, 0.30), 1, Rgb(0.85, 0.87, 0.90)),
        ("pet_boar", Rgb(0.80, 0.56, 0.48), Rgb(0.55, 0.36, 0.30), 1, Rgb(0.85, 0.87, 0.90)),
        ("pet_slime", Rgb(0.45, 0.88, 0.40), Rgb(0.30, 0.70, 0.28), 2, Rgb(0.35, 0.65, 1.0)),
        ("pet_wolf", Rgb(0.62, 0.64, 0.68), Rgb(0.40, 0.42, 0.46), 3, Rgb(0.35, 0.65, 1.0)),
        ("pet_cow", Rgb(0.96, 0.95, 0.92), Rgb(0.20, 0.18, 0.18), 1, Rgb(0.35, 0.65, 1.0)),
        ("pet_fire_chick", Rgb(1.0, 0.60, 0.20), Rgb(1.0, 0.25, 0.10), 0, Rgb(0.75, 0.40, 1.0)),
        ("pet_moon_wolf", Rgb(0.45, 0.55, 0.95), Rgb(0.85, 0.90, 1.0), 3, Rgb(0.75, 0.40, 1.0)),
        ("pet_gold_slime", Rgb(1.0, 0.82, 0.25), Rgb(0.85, 0.62, 0.10), 2, Rgb(1.0, 0.78, 0.20)),
        ("pet_ghost_wolf", Rgb(0.80, 0.95, 0.98), Rgb(0.55, 0.85, 0.95), 3, Rgb(1.0, 0.78, 0.20)),
    ];
    for (i, (name, body, accent, ears, gem)) in pets.into_iter().enumerate() {
        l.add(PET_BASE + i as u16, name, p(move |q| {
            q.shape(body, &|u, v| ((u - 0.5) / 0.38).powi(2) + ((v - 0.60) / 0.34).powi(2) < 1.0);
            match ears {
                0 => {
                    for (x, y) in [(7u32, 2u32), (8, 2), (8, 3), (7, 3), (9, 3)] {
                        q.put(x, y, accent, 1.0);
                    }
                }
                1 => {
                    for (x, y) in [(3u32, 5u32), (4, 5), (11, 5), (12, 5), (3, 6), (12, 6)] {
                        q.put(x, y, accent, 1.0);
                    }
                }
                2 => {
                    for (x, y) in [(6u32, 6u32), (10, 7), (5, 10)] {
                        q.put(x, y, body.add(0.15), 1.0);
                    }
                }
                _ => {
                    for (x, y) in [(4u32, 2u32), (4, 3), (5, 3), (11, 2), (11, 3), (10, 3)] {
                        q.put(x, y, accent, 1.0);
                    }
                }
            }
            for (x, y) in [(6u32, 8u32), (9, 8)] {
                q.put(x, y, Rgb(0.08, 0.08, 0.10), 1.0);
            }
            q.put(7, 10, accent.scale(0.8), 1.0);
            q.put(8, 10, accent.scale(0.8), 1.0);
            for (x, y) in [(13u32, 12u32), (14, 13), (13, 14), (12, 13), (13, 13)] {
                q.put(x, y, gem, 1.0);
            }
        }));
    }
    // Firearms and ammunition.
    let gpal = |ch: char| match ch {
        'X' => Some(Rgb(0.13, 0.14, 0.15)),
        'm' => Some(Rgb(0.38, 0.40, 0.43)),
        'h' => Some(Rgb(0.45, 0.30, 0.17)),
        'L' => Some(Rgb(0.45, 0.80, 1.0)),
        'y' => Some(Rgb(0.80, 0.55, 0.25)),
        'c' => Some(Rgb(0.88, 0.70, 0.30)),
        'C' => Some(Rgb(0.60, 0.45, 0.18)),
        'R' => Some(Rgb(0.80, 0.16, 0.14)),
        'r' => Some(Rgb(0.95, 0.40, 0.35)),
        _ => None,
    };
    l.add(PISTOL, "pistol", p(|q| q.template(&PISTOL_TPL[..], &gpal)));
    l.add(RIFLE, "rifle", p(|q| q.template(&RIFLE_TPL[..], &gpal)));
    l.add(SHOTGUN, "shotgun", p(|q| q.template(&SHOTGUN_TPL[..], &gpal)));
    l.add(SNIPER, "sniper", p(|q| q.template(&SNIPER_TPL[..], &gpal)));
    l.add(BULLETS, "bullets", p(|q| q.template(&BULLETS_TPL[..], &gpal)));
    l.add(SHELLS, "shells", p(|q| q.template(&SHELLS_TPL[..], &gpal)));
    l.add(SADDLE, "saddle", p(|q| q.template(&SADDLE_TPL[..], &|ch: char| match ch {
        'L' => Some(Rgb(0.55, 0.32, 0.16)),
        'l' => Some(Rgb(0.70, 0.45, 0.24)),
        'D' => Some(Rgb(0.36, 0.20, 0.10)),
        'M' => Some(Rgb(0.75, 0.76, 0.80)),
        _ => None,
    })));
    l.add(DRAGON_SOUL, "dragon_soul", p(|q| {
        // A green-gold dragon's eye in a flame wisp.
        let c = Rgb(0.20, 0.80, 0.45);
        for y in 0..16u32 {
            for x in 0..16u32 {
                let dx = x as f32 - 7.5;
                let dy = y as f32 - 9.5;
                let d = (dx * dx + dy * dy).sqrt();
                let flame = y < 9 && dx.abs() < (9.0 - y as f32) * 0.45 + if (x + y) % 3 == 0 { 0.6 } else { 0.0 } && y > 1;
                if d < 5.4 {
                    q.put(x, y, c.lerp(Rgb(1.0, 0.85, 0.30), (1.0 - d / 5.4) * 0.8), 1.0);
                } else if flame {
                    q.put(x, y, Rgb(0.95, 0.55, 0.15), 1.0);
                }
            }
        }
        for y in 7..13u32 {
            q.put(8, y, Rgb(0.08, 0.10, 0.06), 1.0);
        }
    }));
    l.add(RAW_FISH, "raw_fish", fish(Rgb(0.55, 0.70, 0.80), Rgb(0.95, 0.55, 0.30)));
    l.add(COOKED_FISH, "cooked_fish", fish(Rgb(0.78, 0.58, 0.32), Rgb(0.55, 0.35, 0.18)));
    l.add(STAR_REMOTE, "star_remote", p(|q| q.template(&STAR_REMOTE_TPL[..], &|ch: char| match ch {
        'K' => Some(Rgb(0.10, 0.11, 0.14)),
        'G' => Some(Rgb(0.30, 0.32, 0.38)),
        'R' => Some(Rgb(0.95, 0.15, 0.12)),
        'r' => Some(Rgb(1.0, 0.55, 0.50)),
        'Y' => Some(Rgb(1.0, 0.90, 0.40)),
        'C' => Some(Rgb(0.70, 0.85, 1.0)),
        _ => None,
    })));
    l.add(STAR_BLASTER, "star_blaster", p(|q| q.template(&STAR_BLASTER_TPL[..], &|ch: char| match ch {
        'K' => Some(Rgb(0.10, 0.11, 0.16)),
        'G' => Some(Rgb(0.55, 0.58, 0.68)),
        'g' => Some(Rgb(0.80, 0.82, 0.90)),
        'C' => Some(Rgb(0.40, 0.95, 1.0)),
        'Y' => Some(Rgb(1.0, 0.85, 0.30)),
        _ => None,
    })));
    l.add(STAR_HAMMER, "star_hammer", p(|q| q.template(&STAR_HAMMER_TPL[..], &|ch: char| match ch {
        'K' => Some(Rgb(0.10, 0.11, 0.16)),
        'G' => Some(Rgb(0.55, 0.58, 0.68)),
        'g' => Some(Rgb(0.82, 0.85, 0.92)),
        'C' => Some(Rgb(0.55, 0.85, 1.0)),
        'B' => Some(Rgb(0.40, 0.26, 0.14)),
        _ => None,
    })));
    // WolfTech dusts: a small heap of grains in the metal's colour.
    for (id, name, c) in [
        (IRON_DUST, "iron_dust", Rgb(0.78, 0.74, 0.70)),
        (GOLD_DUST, "gold_dust", Rgb(1.0, 0.82, 0.25)),
        (COPPER_DUST, "copper_dust", Rgb(0.90, 0.52, 0.28)),
        (TIN_DUST, "tin_dust", Rgb(0.80, 0.84, 0.88)),
        (SILVER_DUST, "silver_dust", Rgb(0.88, 0.90, 0.95)),
    ] {
        l.add(id, name, p(move |q| {
            for y in 7..14u32 {
                let half = (y - 6) as i32;
                for x in (8 - half).max(2)..(8 + half).min(14) {
                    let x = x as u32;
                    let shade = if (x * 7 + y * 3) % 5 == 0 { 0.75 } else if (x + y) % 3 == 0 { 1.12 } else { 1.0 };
                    q.put(x, y, Rgb((c.0 * shade).min(1.0), (c.1 * shade).min(1.0), (c.2 * shade).min(1.0)), 1.0);
                }
            }
        }));
    }
    l.add(CIRCUIT, "circuit", p(|q| {
        for y in 3..13u32 {
            for x in 2..14u32 {
                q.put(x, y, Rgb(0.10, 0.45, 0.20), 1.0);
            }
        }
        for x in 3..13u32 {
            q.put(x, 6, Rgb(0.95, 0.78, 0.30), 1.0);
            q.put(x, 10, Rgb(0.95, 0.78, 0.30), 1.0);
        }
        for (x, y) in [(5u32, 7u32), (9, 8), (11, 7)] {
            q.put(x, y, Rgb(0.12, 0.12, 0.14), 1.0);
            q.put(x + 1, y, Rgb(0.12, 0.12, 0.14), 1.0);
            q.put(x, y + 1, Rgb(0.12, 0.12, 0.14), 1.0);
            q.put(x + 1, y + 1, Rgb(0.12, 0.12, 0.14), 1.0);
        }
    }));
    // WolfTech arsenal, drawn as pixel art: D dark metal, G grey, c cyan glow, W white,
    // Y gold, K handle, O/R flame, N nano plating, b blue rails.
    let art = |rows: &[&str]| {
        let rows: Vec<String> = rows.iter().map(|r| r.to_string()).collect();
        p(move |q| {
            for (y, row) in rows.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    let c = match ch {
                        'D' => Rgb(0.22, 0.24, 0.28),
                        'G' => Rgb(0.55, 0.58, 0.63),
                        'c' => Rgb(0.20, 0.85, 0.95),
                        'W' => Rgb(0.92, 1.0, 1.0),
                        'Y' => Rgb(0.95, 0.75, 0.25),
                        'K' => Rgb(0.30, 0.20, 0.14),
                        'O' => Rgb(1.0, 0.60, 0.15),
                        'R' => Rgb(0.95, 0.25, 0.10),
                        'N' => Rgb(0.20, 0.23, 0.30),
                        'b' => Rgb(0.30, 0.50, 1.0),
                        _ => continue,
                    };
                    if x < 16 && y < 16 {
                        q.put(x as u32, y as u32, c, 1.0);
                    }
                }
            }
        })
    };
    l.add(PLASMA_RIFLE, "plasma_rifle", art(&["", "", "", "", "...........cc", ".DDDDDDDDDDDDDD", ".DGGGcccGGGGDDc", ".DDDDDDDDDDDD", "...DDD..DD", "...DD...DD", "..DDD", "..DD"]));
    l.add(RAILGUN, "railgun", art(&["", "", "", ".....bbbbbbbbbbb", "DDDDDDDDDDDDDDDD", ".....bbbbbbbbbbb", "...DDD.DD", "...DD", "..DDD", "..DD"]));
    l.add(ENERGY_SWORD, "energy_sword", art(&["..............cW", ".............cWc", "............cWc", "...........cWc", "..........cWc", ".........cWc", "........cWc", ".......cWc", "......cWc", "..Y..cWc", "...YcWc", "....YY", "...KYY", "..KK..Y", ".KK", "KK"]));
    l.add(JETPACK, "jetpack", art(&["", "....GGG..GGG", "...GDDDGGDDDG", "...GDDDGGDDDG", "...GDcDGGDcDG", "...GDDDGGDDDG", "...GDcDGGDcDG", "...GDDDGGDDDG", "...GDDDGGDDDG", "....GGG..GGG", "....OYO..OYO", "....YRY..YRY", ".....O....O", ".....R....R"]));
    l.add(NANO_BASE, "nano_helmet", art(&["", "", "", "....NNNNNNNN", "...NNNNNNNNNN", "...NNccccccNN", "...NNcWWWWcNN", "...NNccccccNN", "...NNN....NNN", "...NNN....NNN"]));
    l.add(NANO_BASE + 1, "nano_chestplate", art(&["", "", "..NNNN....NNNN", "..NNNNNNNNNNNN", "..NNNNNccNNNNN", "...NNNcWWcNNN", "...NNNNccNNNN", "...NNcNNNNcNN", "...NNNNNNNNNN", "...NNcNNNNcNN", "...NNNNNNNNNN", "....NNNNNNNN"]));
    l.add(NANO_BASE + 2, "nano_leggings", art(&["", "", "...NNNNNNNNNN", "...NccccccccN", "...NNNN..NNNN", "...NNcN..NcNN", "...NNNN..NNNN", "...NNNN..NNNN", "...NNcN..NcNN", "...NNNN..NNNN", "...NNNN..NNNN", "...NNNN..NNNN"]));
    l.add(NANO_BASE + 3, "nano_boots", art(&["", "", "", "", "", "", "...NNN....NNN", "...NcN....NcN", "...NNN....NNN", "..NNNN...NNNN", ".NNNcN..NNNcN", ".NNNNN..NNNNN"]));
    l.add(MECH, "mech", art(&["", ".....GGGGGG", ".....GccccG", ".....GGGGGG", "..DDGGGGGGGGDD", "..DDGGGYYGGGDD", "..DD.GGGGGG.DD", "..cc.GGGGGG.cc", ".....DD..DD", ".....DD..DD", ".....DD..DD", "....DDD..DDD"]));
    // Upgrades: a green board with a cyan arrow (speed) / a blue battery (store).
    l.add(OVERCLOCKER, "overclocker", art(&["", "", "..GGGGGGGGGGGG", "..GDDDDDDDDDDG", "..GDDDDccDDDDG", "..GDDDcccDDDDG", "..GDDccccccDDG", "..GDDDcccDDDDG", "..GDDDDccDDDDG", "..GDDDDDDDDDDG", "..GYDYDYDYDYDG", "..GGGGGGGGGGGG"]));
    l.add(STORAGE_UPGRADE, "storage_upgrade", art(&["", "", "..GGGGGGGGGGGG", "..GDDDDDDDDDDG", "..GDbbbbbbbbDG", "..GDbccccccbbG", "..GDbccccccbbG", "..GDbbbbbbbbDG", "..GDDDDDDDDDDG", "..GDDDDDDDDDDG", "..GYDYDYDYDYDG", "..GGGGGGGGGGGG"]));
    // Reactor parts: a glowing green rod in a steel sleeve, a fan grille, a blue coolant
    // capsule, a riveted plate.
    l.add(URANIUM_ROD, "uranium_rod", art(&["......GGG", "......GcG", "......GcG", "......GWG", "......GcG", "......GcG", "......GWG", "......GcG", "......GcG", "......GWG", "......GcG", "......GcG", "......GGG"]));
    l.add(HEAT_VENT, "heat_vent", art(&["", "", "..GGGGGGGGGGGG", "..GDGDGDGDGDGG", "..GGGGGGGGGGGG", "..GDGDGDGDGDGG", "..GGGGGGGGGGGG", "..GDGDGDGDGDGG", "..GGGGGGGGGGGG", "..GDGDGDGDGDGG", "..GGGGGGGGGGGG"]));
    l.add(COOLANT_CELL, "coolant_cell", art(&["", ".....GGGGG", "....GbbbbbG", "....GbcccbG", "....GbcWcbG", "....GbcccbG", "....GbcccbG", "....GbcccbG", "....GbcccbG", "....GbbbbbG", ".....GGGGG"]));
    l.add(REACTOR_PLATING, "reactor_plating", art(&["", "", "..GGGGGGGGGGGG", "..GWDDDDDDDDWG", "..GDGGGGGGGGDG", "..GDGGGGGGGGDG", "..GDGGGGGGGGDG", "..GDGGGGGGGGDG", "..GDGGGGGGGGDG", "..GWDDDDDDDDWG", "..GGGGGGGGGGGG"]));
    l.add(DRILL, "drill", art(&["", "", "", "....GGGGGGG", "...GDDDDDDGW", "...GDcDDDDGWWW", "...GDDDDDDGWWWWW", "...GDDDDDDGWWW", "...GGGGGGGGW", ".....DDD", ".....DDD", "....DDDD", "....DDD"]));
    l.add(CHAINSAW, "chainsaw", art(&["", "", "", "", ".GGGGGWDWDWDWDW", "GDDDDGG.......D", "GDcDDGGWDWDWDWDW", "GDDDDGG", ".GGGGG", "..DDD", "..DD"]));
    // Plates: a flat square sheet with a light edge, in the metal's colour.
    for (id, name, c) in [(IRON_PLATE, "iron_plate", Rgb(0.78, 0.78, 0.80)), (COPPER_PLATE, "copper_plate", Rgb(0.85, 0.52, 0.32)), (GOLD_PLATE, "gold_plate", Rgb(0.98, 0.82, 0.30))] {
        l.add(id, name, p(move |q| {
            for y in 3..13u32 {
                for x in 2..14u32 {
                    let edge = y == 3 || x == 2;
                    let dark = y == 12 || x == 13;
                    q.put(x, y, if edge { c.scale(1.15) } else if dark { c.scale(0.7) } else { c.add((rnd(x, y, id as u32) - 0.5) * 0.05) }, 1.0);
                }
            }
        }));
    }
    // WolfTech: scrap, UU-matter, the omni-tool and the mining laser.
    l.add(SCRAP, "scrap", art(&["", "", "", ".....GG.D", "....GDGGDD", "...DGGDGGG.O", "...GDOGDDGG", "....GGDGOGD", "...DDGGGDG", "....GD.GGD", ".....D..D"]));
    l.add(UU_MATTER, "uu_matter", art(&["", "", ".......b", "......bcb", ".....bcWcb", "....bcWWWcb", "...bcWWWWWcb", "....bcWWWcb", ".....bcWcb", "......bcb", ".......b"]));
    l.add(OMNI_TOOL, "omni_tool", art(&["", "", "..GGGGGGGG", ".GWWWWWWWWG", "..GGGGGGGGcY", "......GGcY", ".....DDGGY", "....DDDDDGG", "...DcDDDDGG", "...DDDDDDG", "....DDDDD", ".....DDD", ".....DD"]));
    l.add(MINING_LASER, "mining_laser", art(&["", "", "", "", "..............R", ".DDDDDDDDDDDDRRR", ".DGGGbbbbbGGGDRW", ".DDDDDDDDDDDDRRR", "...DDD..DD....R", "...DD...DD", "..DDD", "..DD"]));
    l.add(BATTERY, "battery", art(&["", "", "......GG", ".....DDDD", ".....DccD", ".....DccD", ".....DbbD", ".....DbbD", ".....DbbD", ".....DbbD", ".....DbbD", ".....DDDD"]));
    l.add(ENERGY_CRYSTAL, "energy_crystal", art(&["", "", ".......b", "......bcb", ".....bcWcb", ".....bcWcb", "....bcWWWcb", "....bcWWWcb", ".....bcWcb", "......bcb", "......bcb", ".......b"]));
    // ---- Mod packs ----
    l.add(DRACONIUM_DUST, "draconium_dust", p(|q| {
        let c = Rgb(0.62, 0.30, 0.95);
        for y in 7..14u32 {
            let half = (y - 6) as i32;
            for x in (8 - half).max(2)..(8 + half).min(14) {
                let x = x as u32;
                let shade = if (x * 7 + y * 3) % 5 == 0 { 0.7 } else if (x + y) % 3 == 0 { 1.2 } else { 1.0 };
                q.put(x, y, Rgb((c.0 * shade).min(1.0), (c.1 * shade).min(1.0), (c.2 * shade).min(1.0)), 1.0);
            }
        }
    }));
    l.add(DRACONIUM_INGOT, "draconium_ingot", ingot(Rgb(0.60, 0.28, 0.92)));
    l.add(AWAKENED_DRACONIUM, "awakened_draconium", ingot(Rgb(1.0, 0.58, 0.15)));
    l.add(DRACONIC_CORE, "draconic_core", p(|q| {
        for y in 0..16u32 {
            for x in 0..16u32 {
                let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
                let d = dx.abs() + dy.abs();
                if d < 7.0 {
                    let c = if d < 2.5 { Rgb(1.0, 0.92, 0.70) } else if d < 4.5 { Rgb(1.0, 0.55, 0.12) } else { Rgb(0.55, 0.22, 0.85) };
                    q.put(x, y, c, 1.0);
                }
            }
        }
    }));
    // Tools from the vanilla templates, in mod colours; T: head, t: its dark edge, h: handle.
    let tool = |tpl: &'static [&'static str; 16], head: Rgb, handle: Rgb| {
        p(move |q| {
            q.template(&tpl[..], &|ch| match ch {
                'x' => Some(head.scale(0.7)),
                'X' => Some(head),
                'h' | 'w' => Some(handle),
                _ => None,
            })
        })
    };
    l.add(DRACONIC_PICKAXE, "draconic_pickaxe", tool(&PICKAXE, Rgb(1.0, 0.55, 0.15), Rgb(0.45, 0.18, 0.70)));
    l.add(DRACONIC_SWORD, "draconic_sword", tool(&SWORD, Rgb(1.0, 0.60, 0.20), Rgb(0.45, 0.18, 0.70)));
    l.add(COKE, "coke", blob(Rgb(0.22, 0.22, 0.25), 5.0, 360));
    l.add(STEEL_INGOT, "steel_ingot", ingot(Rgb(0.52, 0.56, 0.62)));
    l.add(COBALT_INGOT, "cobalt_ingot", ingot(Rgb(0.25, 0.45, 0.95)));
    l.add(MANYULLYN_INGOT, "manyullyn_ingot", ingot(Rgb(0.58, 0.32, 0.78)));
    l.add(TINKER_HAMMER, "tinker_hammer", tool(&HAMMER_TPL, Rgb(0.58, 0.32, 0.78), Rgb(0.30, 0.45, 0.95)));
    l.add(TINKER_EXCAVATOR, "tinker_excavator", tool(&EXCAVATOR_TPL, Rgb(0.30, 0.45, 0.95), Rgb(0.45, 0.32, 0.18)));
    l.add(LUMBER_AXE, "lumber_axe", tool(&LUMBER_TPL, Rgb(0.58, 0.32, 0.78), Rgb(0.45, 0.32, 0.18)));
    l.add(CLEAVER, "cleaver", tool(&CLEAVER_TPL, Rgb(0.52, 0.56, 0.62), Rgb(0.35, 0.25, 0.15)));
    for (_, img) in l.0.iter_mut() {
        outline(img);
        shade_edges(img);
    }
    l.0
}

/// Draws a 1 px dark outline around the opaque pixels (on transparent neighbours).
fn outline(img: &mut Image) {
    let (w, h) = (img.width as i32, img.height as i32);
    let mut edge = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if img.get(x as u32, y as u32)[3] != 0 {
                continue;
            }
            let touches = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                nx >= 0 && ny >= 0 && nx < w && ny < h && img.get(nx as u32, ny as u32)[3] > 200
            });
            if touches {
                edge.push((x as u32, y as u32));
            }
        }
    }
    for (x, y) in edge {
        img.set(x, y, [24, 20, 28, 255]);
    }
}

/// Light from the top-left: brighten pixels whose upper/left neighbour is empty, darken
/// those whose lower/right neighbour is empty. Gives sprites a little volume.
fn shade_edges(img: &mut Image) {
    let (w, h) = (img.width as i32, img.height as i32);
    let solid = |img: &Image, x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && img.get(x as u32, y as u32)[3] > 200 && img.get(x as u32, y as u32) != [24, 20, 28, 255];
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if !solid(img, x, y) {
                continue;
            }
            let mut px = img.get(x as u32, y as u32);
            let lit = !solid(img, x - 1, y) || !solid(img, x, y - 1);
            let dark = !solid(img, x + 1, y) || !solid(img, x, y + 1);
            let f = if lit && !dark { 1.18 } else if dark && !lit { 0.82 } else { 1.0 };
            if f != 1.0 {
                for c in px.iter_mut().take(3) {
                    *c = (*c as f32 * f).clamp(0.0, 255.0) as u8;
                }
                out.push((x as u32, y as u32, px));
            }
        }
    }
    for (x, y, px) in out {
        img.set(x, y, px);
    }
}

