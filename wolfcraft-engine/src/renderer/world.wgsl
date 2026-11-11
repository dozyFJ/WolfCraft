struct Globals {
    view_proj: mat4x4<f32>,
    sun: vec4<f32>,      // xyz: direction to the sun (world), w: unused
    cam: vec4<f32>,      // xyz: camera position in world space (f32, planet-sized)
    fog: vec4<f32>,      // rgb: fog colour, a: density (1 / fog distance)
    misc: vec4<f32>,     // x: time, y: underwater (0/1), z: unused, w: unused
    tiles: array<vec4<f32>, 128>, // xy: uv min, zw: uv size of every block sprite
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var entities_atlas: texture_2d<f32>;
@group(0) @binding(4) var items_atlas: texture_2d<f32>;

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
    let rel = in.pos + in.inst.xyz;
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

fn shade(in: VsOut, color: vec4<f32>, n: vec3<f32>) -> vec3<f32> {
    let emit = (in.flags >> 16u) & 1u;
    let face = (in.flags >> 17u) & 7u;
    let up = normalize(in.world);
    let sun = normalize(g.sun.xyz);
    let day = smoothstep(-0.12, 0.28, dot(up, sun));
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
    let diffuse = max(dot(n, sun), 0.0);
    let sky_light = mix(0.09, 0.52 + 0.48 * diffuse, day) * in.light.y;
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
    let lit = min(sky_rgb + torch_rgb * (1.0 - 0.6 * day * in.light.y), vec3<f32>(1.15));
    var rgb = color.rgb * lit * in.light.x * face_k;
    if (emit == 1u) {
        rgb = color.rgb * (0.85 + 0.15 * in.light.x);
    }
    // Distance fog.
    let d = length(in.rel);
    let f = clamp(1.0 - exp(-pow(d * g.fog.a, 2.0)), 0.0, 1.0);
    rgb = mix(rgb, g.fog.rgb, f);
    if (g.misc.y > 0.5) {
        let w = clamp(d / 36.0, 0.0, 0.92);
        rgb = mix(rgb * vec3<f32>(0.55, 0.75, 1.0), vec3<f32>(0.05, 0.22, 0.42), w);
    }
    return rgb;
}

// The swapchain is not sRGB (egui needs that), so the world shader encodes its own output.
fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    return pow(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));
}

fn sample_tile(in: VsOut) -> vec4<f32> {
    let tile = in.flags & 0xFFFFu;
    let r = g.tiles[tile];
    let f = clamp(fract(in.uv), vec2<f32>(0.001), vec2<f32>(0.999));
    return textureSample(atlas, samp, r.xy + f * r.zw);
}

@fragment
fn fs_opaque(in: VsOut) -> @location(0) vec4<f32> {
    let c = sample_tile(in);
    let n = geom_normal(in.rel);
    if (c.a < 0.5) {
        discard;
    }
    return vec4<f32>(to_srgb(shade(in, c, n)), 1.0);
}

@fragment
fn fs_water(in: VsOut) -> @location(0) vec4<f32> {
    let c = sample_tile(in);
    let rgb = to_srgb(shade(in, c, geom_normal(in.rel)));
    return vec4<f32>(rgb, c.a);
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
    if (in.tex < 0.5) {
        // Pixel-art grain on flat-colour creatures: every 1/16 block gets its own brightness.
        let cell = floor(in.uv);
        let h = fract(sin(dot(cell, vec2<f32>(12.9898, 78.233))) * 43758.5453);
        base = base * (0.80 + 0.36 * h);
    }
    if (in.tex > 1.5) {
        if (item.a < 0.5) {
            discard;
        }
        base = item.rgb;
    } else if (in.tex > 0.5) {
        base = skin.rgb;
    }
    let up = normalize(in.world);
    let sun = normalize(g.sun.xyz);
    let day = smoothstep(-0.12, 0.28, dot(up, sun));
    let diffuse = max(dot(normalize(in.normal), sun), 0.0);
    let light = max(mix(0.10, 0.50 + 0.50 * diffuse, day), 0.16);
    var rgb = base * light;
    let d = length(in.rel);
    let f = clamp(1.0 - exp(-pow(d * g.fog.a, 2.0)), 0.0, 1.0);
    rgb = mix(rgb, g.fog.rgb, f);
    return vec4<f32>(to_srgb(rgb), 1.0);
}

@fragment
fn fs_sky(in: ModelOut) -> @location(0) vec4<f32> {
    // Sky objects are unlit display-space colours.
    return vec4<f32>(clamp(in.color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
