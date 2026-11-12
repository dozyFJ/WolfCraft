struct Globals {
    view_proj: mat4x4<f32>,
    sun: vec4<f32>,      // xyz: direction to the sun (world), w: unused
    cam: vec4<f32>,      // xyz: camera position in world space (f32, planet-sized)
    fog: vec4<f32>,      // rgb: fog colour, a: density (1 / fog distance)
    misc: vec4<f32>,     // x: time, y: underwater (0/1), z: crack, w: shader preset (0 off, 1 light, 2 ultra)
    env: vec4<f32>,      // x: season 0..4 (spring, summer, autumn, winter), y: snow cover, z: humidity
    fx: vec4<f32>,       // x: relief mode (0 off, 1 lit, 2 parallax), y: relief strength, z: flags (1 wind, 2 water, 4 grading, 8 cartoon)
    tiles: array<vec4<f32>, 256>, // xy: uv min, zw: uv size of every block sprite
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var entities_atlas: texture_2d<f32>;
@group(0) @binding(4) var items_atlas: texture_2d<f32>;
@group(0) @binding(5) var relief: texture_2d<f32>; // height maps of the block tiles (red)

fn fx_on(bit: u32) -> bool {
    return (u32(g.fx.z + 0.5) & bit) != 0u;
}

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) flags: u32,
    @location(3) light: vec2<f32>,
    @location(4) inst: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) flags: u32,
    @location(2) light: vec2<f32>,
    @location(3) world: vec3<f32>,
    @location(4) rel: vec3<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    var rel = in.pos + in.inst.xyz;
    // Shaders: leaves and plants sway in the wind. The phase comes from the world position,
    // so corners shared by neighbouring blocks move together (no cracks).
    let sway_leaves = (in.flags >> 24u) & 1u;
    let sway_plant = (in.flags >> 25u) & 1u;
    if (fx_on(1u) && (sway_leaves + sway_plant) > 0u) {
        let w = g.cam.xyz + rel;
        let t = g.misc.x;
        let phase = dot(w, vec3<f32>(0.37, 0.23, 0.31));
        var amp = 0.03;
        if (sway_plant == 1u) {
            amp = 0.09 * clamp(1.0 - in.uv.y, 0.0, 1.0); // the top sways, the root stays
        }
        let gust = 0.6 + 0.4 * sin(t * 0.35 + phase * 0.05);
        rel = rel + vec3<f32>(sin(t * 1.7 + phase), sin(t * 1.3 + phase * 1.3) * 0.4, cos(t * 1.5 + phase * 0.8)) * amp * gust;
    }
    out.clip = g.view_proj * vec4<f32>(rel, 1.0);
    out.uv = in.uv;
    out.flags = in.flags;
    out.light = in.light;
    out.rel = rel;
    out.world = g.cam.xyz + rel;
    return out;
}

fn face_normal(face: u32) -> vec3<f32> {
    switch face {
        case 0u: { return vec3<f32>(1.0, 0.0, 0.0); }
        case 1u: { return vec3<f32>(-1.0, 0.0, 0.0); }
        case 2u: { return vec3<f32>(0.0, 1.0, 0.0); }
        case 3u: { return vec3<f32>(0.0, -1.0, 0.0); }
        case 4u: { return vec3<f32>(0.0, 0.0, 1.0); }
        default: { return vec3<f32>(0.0, 0.0, -1.0); }
    }
}

// Real face normal from the geometry (cube-sphere faces are not world-axis aligned), facing the camera.
// Must be called in uniform control flow (before any discard).
fn geom_normal(rel: vec3<f32>) -> vec3<f32> {
    var n = normalize(cross(dpdx(rel), dpdy(rel)));
    if (dot(n, rel) > 0.0) {
        n = -n;
    }
    return n;
}

// Seasons: leaves (bit 24), plants (bit 25) and grass tops (bit 26) change colour through the
// year: fresh green in spring, autumn reds, oranges and yellows (per tree block), bare grey-green
// and snow on top in winter. Smooth between seasons.
fn season_tint(c: vec3<f32>, in: VsOut, n: vec3<f32>) -> vec3<f32> {
    let leaves = (in.flags >> 24u) & 1u;
    let plant = (in.flags >> 25u) & 1u;
    let turf = (in.flags >> 26u) & 1u;
    if (leaves + plant + turf == 0u) {
        return c;
    }
    let s = g.env.x;
    let spring = smoothstep(0.0, 0.3, s) * (1.0 - smoothstep(0.8, 1.2, s));
    let autumn = smoothstep(1.75, 2.25, s) * (1.0 - smoothstep(2.8, 3.15, s));
    let winter = smoothstep(2.8, 3.15, s) * (1.0 - smoothstep(3.8, 4.0, s)) + (1.0 - smoothstep(0.0, 0.3, s));
    let lum = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    var o = c;
    // Spring: fresher, lighter green.
    o = mix(o, o * vec3<f32>(0.95, 1.10, 0.90), spring);
    // Autumn: every leaf block picks orange, red or yellow; grass turns straw-coloured.
    let cell = floor(in.world + vec3<f32>(0.001));
    let h = fract(sin(dot(cell, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
    var fall = vec3<f32>(0.95, 0.80, 0.25);
    if (h < 0.4) {
        fall = vec3<f32>(0.95, 0.52, 0.14);
    } else if (h < 0.6) {
        fall = vec3<f32>(0.78, 0.22, 0.12);
    }
    let strength = select(0.55, 0.9, leaves == 1u);
    let grass_fall = vec3<f32>(0.78, 0.66, 0.32);
    let target_fall = select(grass_fall, fall, leaves == 1u) * lum * 2.1;
    o = mix(o, target_fall, autumn * strength);
    // Winter: dull grey-green, then snow settles on what faces the sky.
    o = mix(o, vec3<f32>(lum) * vec3<f32>(0.85, 0.95, 0.85), winter * 0.45);
    let up = normalize(in.world);
    var settle = 1.0;
    if (leaves == 1u) {
        settle = smoothstep(0.2, 0.7, dot(n, up));
    }
    let snow = winter * g.env.y * settle * select(0.9, 0.5, plant == 1u);
    o = mix(o, vec3<f32>(0.90, 0.94, 1.0) * (0.85 + 0.25 * lum), snow);
    return o;
}

fn fog_amount(d: f32) -> f32 {
    let x = d * g.fog.a * 1.1;
    return clamp(1.0 - exp(-x * x * x * x), 0.0, 1.0);
}

fn shade(in: VsOut, color0: vec4<f32>, n: vec3<f32>) -> vec3<f32> {
    let color = vec4<f32>(season_tint(color0.rgb, in, n), color0.a);
    let emit = (in.flags >> 16u) & 1u;
    let face = (in.flags >> 17u) & 7u;
    let up = normalize(in.world);
    let sun = normalize(g.sun.xyz);
    let day = smoothstep(-0.10, 0.14, dot(up, sun));
    let _axis = face_normal(face);
    // Classic voxel face shading: top brightest, sides darker, bottom darkest -> cubes read as 3D.
    var face_k = 1.0;
    switch face {
        case 2u: { face_k = 1.0; }
        case 3u: { face_k = 0.55; }
        case 0u, 1u: { face_k = 0.82; }
        default: { face_k = 0.68; }
    }
    // Directional light comes from the sun, ambient from the sky; at night only dim moonlight.
    var diffuse = max(dot(n, sun), 0.0);
    let toon = fx_on(8u);
    var ao = in.light.x;
    var sky_amb = 0.58;
    var night_min = 0.10;
    if (toon) {
        // Cartoon light: a soft two-tone sun (lit / shade with a smooth step between them),
        // bright ambient so shadows stay light and colourful, gentle side shading and AO.
        diffuse = smoothstep(0.08, 0.32, diffuse) * 0.75 + diffuse * 0.25;
        switch face {
            case 2u: { face_k = 1.0; }
            case 3u: { face_k = 0.74; }
            case 0u, 1u: { face_k = 0.90; }
            default: { face_k = 0.82; }
        }
        ao = mix(1.0, ao, 0.55);
        sky_amb = 0.70;
        night_min = 0.18;
    }
    // Full sun on a top face stays just under 1, so bright surfaces keep their colour
    // instead of washing out to white.
    let sky_light = mix(night_min, sky_amb + (0.94 - sky_amb) * diffuse, day) * mix(in.light.y, sqrt(in.light.y), select(0.0, 0.6, toon));
    // Cool tint at night, warm at sunrise/sunset.
    let night_tint = vec3<f32>(0.78, 0.88, 1.18);
    let warm = vec3<f32>(1.08, 0.95, 0.85);
    let twilight = 1.0 - abs(day * 2.0 - 1.0);
    let sky_rgb = vec3<f32>(max(sky_light, 0.04)) * mix(night_tint, vec3<f32>(1.0), day) * mix(vec3<f32>(1.0), warm, twilight * 0.5);
    // Block light (torches, lava, lamps): level 0..15 from the mesher, warm and with a
    // Minecraft-like falloff (bright near the source, a soft glow at the edge).
    let blk = f32((in.flags >> 20u) & 15u) / 15.0;
    let torch = blk * blk * (0.35 + 0.75 * blk);
    let torch_rgb = vec3<f32>(1.0, 0.80, 0.55) * torch;
    // Light adds up, but never brighter than full daylight.
    var lit = min(sky_rgb + torch_rgb * (1.0 - 0.6 * day * in.light.y), vec3<f32>(1.15));
    if (toon) {
        // Lighting like the big simulator places: a warm sun with cool blue-tinted shade by
        // day, golden-pink light at sunrise and sunset, and a bright moonlit blue night where
        // everything stays readable (the moon lights from the side opposite the sun).
        let sky_k = mix(in.light.y, sqrt(in.light.y), 0.6);
        let shade_t = 1.0 - diffuse;
        let day_rgb = (0.80 + 0.18 * diffuse) * mix(vec3<f32>(1.04, 1.0, 0.92), vec3<f32>(0.86, 0.93, 1.10), shade_t * 0.8);
        let moon = smoothstep(0.0, 0.4, max(dot(n, -sun), 0.0)) * 0.6 + 0.4;
        let night_rgb = vec3<f32>(0.30, 0.38, 0.62) * (0.75 + 0.35 * moon);
        let gold = vec3<f32>(1.16, 0.90, 0.78);
        var amb = mix(night_rgb, day_rgb, day) * mix(vec3<f32>(1.0), gold, twilight * 0.6);
        amb = amb * sky_k;
        // Torches glow warmer and further at night.
        lit = min(max(amb, vec3<f32>(0.05)) + torch_rgb * (1.15 - 0.75 * day * in.light.y), vec3<f32>(1.2));
    }
    var rgb = color.rgb * lit * ao * face_k;
    if (toon && emit == 0u) {
        // Light from the sky dome: cool from above, a warm bounce from below.
        let ny = dot(n, up);
        let hemi = mix(vec3<f32>(1.05, 0.98, 0.92), vec3<f32>(0.96, 1.0, 1.06), ny * 0.5 + 0.5);
        rgb = rgb * hemi;
        // (No plastic sheen: blocks stay matte, like painted voxels.)
    }
    if (emit == 1u) {
        rgb = color.rgb * (0.85 + 0.15 * in.light.x);
    }
    // Distance fog (the cartoon look adds a light aerial haze that starts sooner).
    let d = length(in.rel);
    // Clear air nearby, the fog closes in only towards the edge of the view distance (it
    // just hides where the world ends); the cartoon look adds a faint haze far away.
    var f = fog_amount(d);
    if (fx_on(8u)) {
        f = max(f, clamp((d * g.fog.a - 0.45) * 0.25, 0.0, 0.10));
    }
    rgb = mix(rgb, g.fog.rgb, f);
    if (g.misc.y > 0.5) {
        let w = clamp(d / 36.0, 0.0, 0.92);
        rgb = mix(rgb * vec3<f32>(0.55, 0.75, 1.0), vec3<f32>(0.05, 0.22, 0.42), w);
    }
    return rgb;
}

// Colour grading: the game is bright and colourful (more saturation always), and with
// shaders on a soft contrast curve and warm highlights on top.
fn grade(c: vec3<f32>) -> vec3<f32> {
    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let vivid = max(mix(vec3<f32>(l), c, 1.10), vec3<f32>(0.0));
    if (fx_on(8u)) {
        // Cartoon: very juicy colours, lifted shadows, cool shade and sunny highlights.
        var o = max(mix(vec3<f32>(l), c, 1.14), vec3<f32>(0.0));
        o = o * 0.94 + vec3<f32>(0.012, 0.016, 0.03);
        o = o * o * (3.0 - 2.0 * o) * 0.25 + o * 0.75;
        return o * mix(vec3<f32>(0.97, 0.99, 1.04), vec3<f32>(1.04, 1.01, 0.96), clamp(l * 1.6, 0.0, 1.0));
    }
    if (!fx_on(4u)) {
        return vivid;
    }
    var o = mix(vec3<f32>(l), c, 1.22);
    o = max(o, vec3<f32>(0.0));
    o = o * o * (3.0 - 2.0 * o) * 0.35 + o * 0.65; // soft S-curve
    return o * vec3<f32>(1.02, 1.0, 0.97);
}

// The swapchain is not sRGB (egui needs that), so the world shader encodes its own output.
fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    return pow(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));
}

fn sample_tile(in: VsOut) -> vec4<f32> {
    let tile = in.flags & 0xFFFFu;
    let r = g.tiles[tile];
    let f = clamp(fract(in.uv), vec2<f32>(0.001), vec2<f32>(0.999));
    // Gradients of the unwrapped uv: the mip level stays smooth across tile repeats (fract
    // would otherwise pick the smallest level along every block edge).
    let suv = in.uv * r.zw;
    return textureSampleGrad(atlas, samp, r.xy + f * r.zw, dpdx(suv), dpdy(suv));
}

// Height (0 deep .. 1 high) of the tile under `uv` (any value: the tile repeats).
fn tile_height(r: vec4<f32>, uv: vec2<f32>) -> f32 {
    let f = clamp(fract(uv), vec2<f32>(0.001), vec2<f32>(0.999));
    return textureSampleLevel(relief, samp, r.xy + f * r.zw, 0.0).r;
}

fn sample_at(r: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let f = clamp(fract(uv), vec2<f32>(0.001), vec2<f32>(0.999));
    return textureSampleLevel(atlas, samp, r.xy + f * r.zw, 0.0);
}

@fragment
fn fs_opaque(in: VsOut) -> @location(0) vec4<f32> {
    let c0 = sample_tile(in);
    var n = geom_normal(in.rel);
    // Derivatives must be taken in uniform control flow (before discard and branches).
    let dpx = dpdx(in.rel);
    let dpy = dpdy(in.rel);
    let duvx = dpdx(in.uv);
    let duvy = dpdy(in.uv);
    if (c0.a < 0.5) {
        discard;
    }
    var c = c0;
    let n_geom = n;
    var relief_light = 1.0;
    let mode = g.fx.x;
    let dist = length(in.rel);
    let emit = (in.flags >> 16u) & 1u;
    let det = duvx.x * duvy.y - duvx.y * duvy.x;
    if (mode > 0.5 && dist < 40.0 && emit == 0u && abs(det) > 1e-12) {
        let r = g.tiles[in.flags & 0xFFFFu];
        // Tangent frame from screen-space derivatives (no tangents stored in the mesh).
        let t = normalize((dpx * duvy.y - dpy * duvx.y) / det);
        let b = normalize((dpy * duvx.x - dpx * duvy.x) / det);
        var uv = in.uv;
        let strength = g.fx.y;
        // Parallax: walk into the height map along the view ray, so gaps sink in and the
        // surface shifts with the camera (close up only).
        if (mode > 1.5 && dist < 20.0) {
            let v = normalize(-in.rel);
            let vt = vec3<f32>(dot(v, t), dot(v, b), dot(v, n));
            // Fade out with distance and at grazing angles (where the steps would smear).
            let fade = (1.0 - smoothstep(12.0, 20.0, dist)) * smoothstep(0.12, 0.45, vt.z);
            let shift = vt.xy / max(vt.z, 0.35) * (0.05 * strength * fade);
            let steps = 16.0;
            let step_uv = shift / steps;
            var depth = 0.0;
            var map_depth = 1.0 - tile_height(r, uv);
            var prev_uv = uv;
            var prev_gap = map_depth;
            for (var i = 0; i < 16; i = i + 1) {
                if (depth >= map_depth) {
                    break;
                }
                prev_uv = uv;
                prev_gap = map_depth - depth;
                uv = uv - step_uv;
                depth = depth + 1.0 / steps;
                map_depth = 1.0 - tile_height(r, uv);
            }
            // Relief occlusion: interpolate between the last two steps for a clean edge.
            let after = depth - map_depth;
            let wgt = clamp(after / max(after + prev_gap, 1e-4), 0.0, 1.0);
            uv = mix(uv, prev_uv, wgt);
            c = sample_at(r, uv);
            if (c.a < 0.5) {
                c = c0;
                uv = in.uv;
            }
        }
        // Lit relief: the slope of the height map tilts the normal, so the sun lights one
        // side of every bump and the other falls into shade.
        let size = vec2<f32>(textureDimensions(relief, 0));
        let texel = 1.0 / max(r.z * size.x, 1.0);
        let h0 = tile_height(r, uv);
        let hx = tile_height(r, uv + vec2<f32>(texel, 0.0));
        let hy = tile_height(r, uv + vec2<f32>(0.0, texel));
        // Slope per tile = height step per texel × texels per tile; the relief depth is about
        // a tenth of a block, the same for 16× and 4K packs.
        let tile_px = r.z * size.x;
        let k = clamp(0.1 * tile_px, 1.6, 14.0) * strength * (1.0 - smoothstep(24.0, 40.0, dist));
        n = normalize(n - t * (hx - h0) * k - b * (hy - h0) * k);
        // Make the relief read clearly: the side of a bump facing the sun gets lighter, the
        // far side darker (on top of the normal lighting, which only partly follows the sun).
        let sun = normalize(g.sun.xyz);
        let day = smoothstep(-0.10, 0.14, dot(normalize(in.world), sun));
        relief_light = clamp(1.0 + 1.1 * (dot(n, sun) - dot(n_geom, sun)) * day, 0.55, 1.45);
        // Cavities get a little darker (ambient occlusion from the height map).
        c = vec4<f32>(c.rgb * mix(1.0, 0.72 + 0.28 * h0, min(strength, 1.5) * 0.6), c.a);
    }
    return vec4<f32>(to_srgb(grade(shade(in, c, n) * relief_light)), 1.0);
}

@fragment
fn fs_water(in: VsOut) -> @location(0) vec4<f32> {
    let c = sample_tile(in);
    var n = geom_normal(in.rel);
    var rgb = shade(in, c, n);
    var a = c.a;
    if (fx_on(2u) && g.misc.y < 0.5) {
        // Shaders: small moving waves, a sun glint and more sky reflected at grazing angles.
        let w = in.world;
        let t = g.misc.x;
        let wave = vec3<f32>(
            sin(dot(w, vec3<f32>(1.3, 0.2, 0.7)) * 1.7 + t * 1.9) + 0.5 * sin(dot(w, vec3<f32>(-0.4, 0.9, 1.6)) * 2.9 + t * 2.7),
            0.0,
            cos(dot(w, vec3<f32>(0.6, -0.8, 1.1)) * 1.9 + t * 1.6) + 0.5 * cos(dot(w, vec3<f32>(1.7, 0.3, -0.5)) * 3.3 + t * 2.2),
        );
        let nw = normalize(n + wave * 0.06);
        let v = normalize(-in.rel);
        let sun = normalize(g.sun.xyz);
        let up = normalize(in.world);
        let day = smoothstep(-0.10, 0.14, dot(up, sun));
        let spec = pow(max(dot(reflect(-sun, nw), v), 0.0), 120.0) * day * in.light.y;
        let fres = pow(1.0 - max(dot(nw, v), 0.0), 3.0);
        rgb = mix(rgb, g.fog.rgb * mix(0.35, 1.0, day), fres * 0.55) + vec3<f32>(1.0, 0.95, 0.85) * spec * 1.4;
        a = mix(a, 1.0, fres * 0.5);
    }
    return vec4<f32>(to_srgb(grade(rgb)), a);
}

// ---- block outline ----
struct LineOut {
    @builtin(position) clip: vec4<f32>,
};

@vertex
fn vs_line(@location(0) pos: vec3<f32>) -> LineOut {
    var out: LineOut;
    out.clip = g.view_proj * vec4<f32>(pos, 1.0);
    return out;
}

@fragment
fn fs_line() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 0.85);
}

// ---- breaking cracks on the targeted block ----
struct CrackIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct CrackOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_crack(in: CrackIn) -> CrackOut {
    var out: CrackOut;
    out.clip = g.view_proj * vec4<f32>(in.pos, 1.0);
    out.uv = in.uv;
    return out;
}

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fs_crack(in: CrackOut) -> @location(0) vec4<f32> {
    // 16x16 pixel cracks: jagged arms from the centre that grow with mining progress.
    // Ten visible stages (0..9), like frames of a crack animation.
    let stage = floor(clamp(g.misc.z, 0.0, 0.999) * 10.0) / 9.0;
    let px = floor(clamp(in.uv, vec2<f32>(0.0), vec2<f32>(0.999)) * 16.0);
    let d = px - vec2<f32>(7.5);
    let r = length(d) / 11.3;
    let ang = atan2(d.y, d.x);
    let arm = abs(fract(ang / 6.2831853 * 6.0 + hash2(floor(px / 3.0)) * 0.35) - 0.5);
    let on_line = arm < 0.11 + 0.05 * hash2(px);
    let speck = hash2(px + vec2<f32>(3.0, 7.0)) < stage * 0.18;
    if (!((on_line && r < stage * 1.05) || (speck && r < stage))) {
        discard;
    }
    return vec4<f32>(0.0, 0.0, 0.0, 0.62);
}

// ---- player / entity model (flat colours, per-vertex normals) ----
struct ModelIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) tex: f32,
};

struct ModelOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) world: vec3<f32>,
    @location(3) rel: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) @interpolate(flat) tex: f32,
};

@vertex
fn vs_model(in: ModelIn) -> ModelOut {
    var out: ModelOut;
    out.clip = g.view_proj * vec4<f32>(in.pos, 1.0);
    out.normal = in.normal;
    out.color = in.color;
    out.rel = in.pos;
    out.world = g.cam.xyz + in.pos;
    out.uv = in.uv;
    out.tex = in.tex;
    return out;
}

@fragment
fn fs_model(in: ModelOut) -> @location(0) vec4<f32> {
    // Base colour: flat (sRGB constant) or sampled from the skin / item atlases (already linear).
    let skin = textureSampleLevel(entities_atlas, samp, in.uv, 0.0);
    let item = textureSampleLevel(items_atlas, samp, in.uv, 0.0);
    var base = pow(in.color, vec3<f32>(2.2));
    let toon = fx_on(8u);
    if (in.tex < 0.5 && !toon) {
        // Pixel-art grain on flat-colour creatures: every 1/16 block gets its own brightness.
        let cell = floor(in.uv);
        let h = fract(sin(dot(cell, vec2<f32>(12.9898, 78.233))) * 43758.5453);
        base = base * (0.80 + 0.36 * h);
    }
    if (in.tex > 3.5) {
        // Dropped block: a block sprite from the world atlas (sprite id in colour.r).
        let r = g.tiles[min(u32(in.color.r + 0.5), 255u)];
        let bt = textureSampleLevel(atlas, samp, r.xy + clamp(in.uv, vec2<f32>(0.01), vec2<f32>(0.99)) * r.zw, 0.0);
        if (bt.a < 0.5) {
            discard;
        }
        base = bt.rgb;
    } else if (in.tex > 1.5 && in.tex < 2.5) {
        if (item.a < 0.5) {
            discard;
        }
        base = item.rgb;
    } else if (in.tex > 0.5 && in.tex < 1.5) {
        // Only tex 1 is a skin: glowing particles (tex 3) keep their own colour.
        base = skin.rgb;
    }
    let up = normalize(in.world);
    let sun = normalize(g.sun.xyz);
    let day = smoothstep(-0.10, 0.14, dot(up, sun));
    let nn = normalize(in.normal);
    var diffuse = max(dot(nn, sun), 0.0);
    var light = max(mix(0.11, 0.58 + 0.38 * diffuse, day), 0.16);
    var rim = 0.0;
    if (toon) {
        // Cartoon creatures: two-tone soft light.
        diffuse = smoothstep(0.05, 0.30, diffuse) * 0.8 + diffuse * 0.2;
        light = max(mix(0.20, 0.70 + 0.30 * diffuse, day), 0.24);
        let bl = dot(base, vec3<f32>(0.2126, 0.7152, 0.0722));
        base = max(mix(vec3<f32>(bl), base, 1.1), vec3<f32>(0.0));
        let v = normalize(-in.rel);
        // A faint rim only (matte voxels, no plastic sheen).
        rim = pow(1.0 - max(dot(nn, v), 0.0), 3.0) * mix(0.04, 0.12, day);
    }
    if (in.tex > 2.5 && in.tex < 3.5) {
        light = 1.25; // glowing particles: embers, sparks
        rim = 0.0;
    }
    var rgb = base * light + vec3<f32>(rim);
    let d = length(in.rel);
    let f = fog_amount(d);
    rgb = mix(rgb, g.fog.rgb, f);
    return vec4<f32>(to_srgb(grade(rgb)), 1.0);
}

@fragment
fn fs_sky(in: ModelOut) -> @location(0) vec4<f32> {
    // Sky objects are unlit display-space colours.
    return vec4<f32>(clamp(in.color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
