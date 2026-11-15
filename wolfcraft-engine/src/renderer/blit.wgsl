@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;

// Post effects (like the PC shader packs): x, y = where the sun is on the screen (0..1),
// z = how much of it shows (0 behind the camera or at night), w = flags (1 bloom, 2 rays).
struct Post {
    sun: vec4<f32>,
};
@group(0) @binding(2) var<uniform> post: Post;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    var o: Out;
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    o.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, y);
    return o;
}

fn lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Only what is really bright glows: lava, torches, lamps, the sun, sunlit snow.
fn bright(uv: vec2<f32>) -> vec3<f32> {
    let c = textureSampleLevel(src, smp, uv, 0.0).rgb;
    return c * smoothstep(0.78, 0.98, lum(c));
}

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    var rgb = textureSampleLevel(src, smp, in.uv, 0.0).rgb;
    let flags = u32(post.sun.w + 0.5);
    let px = 1.0 / vec2<f32>(textureDimensions(src, 0));
    if ((flags & 1u) != 0u) {
        // Bloom: two rings of taps around the pixel collect the light of bright neighbours.
        var glow = vec3<f32>(0.0);
        for (var i = 0; i < 8; i = i + 1) {
            let a = f32(i) * 0.785398;
            let d = vec2<f32>(cos(a), sin(a));
            glow = glow + bright(in.uv + d * px * 5.0) * 0.6 + bright(in.uv + d.yx * vec2<f32>(1.0, -1.0) * px * 12.0) * 0.4;
        }
        rgb = rgb + glow / 8.0 * 0.55;
    }
    if ((flags & 2u) != 0u && post.sun.z > 0.01) {
        // Light shafts: march from the pixel towards the sun, gathering bright sky; leaves and
        // walls in between block it, so rays fan out through the gaps.
        let to_sun = post.sun.xy - in.uv;
        var acc = 0.0;
        var decay = 1.0;
        let steps = 20;
        for (var i = 0; i < steps; i = i + 1) {
            let t = (f32(i) + 0.5) / f32(steps);
            let c = textureSampleLevel(src, smp, in.uv + to_sun * t * 0.85, 0.0).rgb;
            acc = acc + smoothstep(0.62, 0.92, lum(c)) * decay;
            decay = decay * 0.94;
        }
        let near = 1.0 - smoothstep(0.0, 0.75, length(to_sun * vec2<f32>(1.6, 1.0)));
        rgb = rgb + vec3<f32>(1.0, 0.86, 0.62) * (acc / f32(steps)) * near * post.sun.z * 0.55;
    }
    return vec4<f32>(rgb, 1.0);
}
