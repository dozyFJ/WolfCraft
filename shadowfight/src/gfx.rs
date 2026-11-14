//! Smooth 3D renderer: lit meshes (spheres, cylinders, cones) drawn as instances, a gradient
//! sky with a sun (or an eclipse), fog, rim light, 4× MSAA. No blocks anywhere.

use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    model: [[f32; 4]; 4],
    color: [f32; 4],
    /// rgb rim colour, a = rim strength.
    rim: [f32; 4],
    /// x emissive, y floor pattern, z wobble (shadow smoke), w unused.
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    cam: [f32; 4],
    sun_dir: [f32; 4],
    sun_color: [f32; 4],
    sky_top: [f32; 4],
    sky_horizon: [f32; 4],
    fog: [f32; 4],
    misc: [f32; 4],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Sphere = 0,
    Cylinder = 1,
    Cone = 2,
}

const SHAPES: usize = 3;

/// What the frame shows, filled by the game every frame.
pub struct Scene {
    pub eye: Vec3,
    pub target: Vec3,
    pub fov_y: f32,
    pub sky_top: Vec3,
    pub sky_horizon: Vec3,
    pub sun_dir: Vec3,
    pub sun_color: Vec3,
    pub sun: f32,
    pub fog: f32,
    pub eclipse: bool,
    pub time: f32,
    /// Red flash when the player is hit (0..1).
    pub hurt: f32,
    items: [Vec<Instance>; SHAPES],
}

impl Scene {
    pub fn new() -> Self {
        Self {
            eye: Vec3::new(0.0, 1.6, -3.0),
            target: Vec3::ZERO,
            fov_y: 70f32.to_radians(),
            sky_top: Vec3::new(0.1, 0.1, 0.3),
            sky_horizon: Vec3::new(0.9, 0.5, 0.3),
            sun_dir: Vec3::new(0.5, 0.3, 0.8),
            sun_color: Vec3::ONE,
            sun: 1.0,
            fog: 0.02,
            eclipse: false,
            time: 0.0,
            hurt: 0.0,
            items: [Vec::new(), Vec::new(), Vec::new()],
        }
    }

    pub fn clear(&mut self) {
        for v in &mut self.items {
            v.clear();
        }
    }

    pub fn add(&mut self, shape: Shape, model: Mat4, color: Vec3, rim: Vec3, rim_k: f32, emissive: f32) {
        self.items[shape as usize].push(Instance { model: model.to_cols_array_2d(), color: [color.x, color.y, color.z, 1.0], rim: [rim.x, rim.y, rim.z, rim_k], params: [emissive, 0.0, 0.0, 0.0] });
    }

    pub fn add_floor(&mut self, model: Mat4, color: Vec3) {
        self.items[Shape::Cylinder as usize].push(Instance { model: model.to_cols_array_2d(), color: [color.x, color.y, color.z, 1.0], rim: [0.0; 4], params: [0.0, 1.0, 0.0, 0.0] });
    }

    pub fn sphere(&mut self, at: Vec3, r: Vec3, color: Vec3, rim: Vec3, rim_k: f32, emissive: f32) {
        self.add(Shape::Sphere, Mat4::from_scale_rotation_translation(r, Quat::IDENTITY, at), color, rim, rim_k, emissive);
    }

    /// A rounded limb from `a` to `b`: a cylinder with a ball at each end.
    pub fn limb(&mut self, a: Vec3, b: Vec3, r: f32, color: Vec3, rim: Vec3, rim_k: f32) {
        let d = b - a;
        let len = d.length();
        if len > 1e-4 {
            let q = Quat::from_rotation_arc(Vec3::Y, d / len);
            self.add(Shape::Cylinder, Mat4::from_scale_rotation_translation(Vec3::new(r, len, r), q, a), color, rim, rim_k, 0.0);
        }
        self.sphere(a, Vec3::splat(r), color, rim, rim_k, 0.0);
        self.sphere(b, Vec3::splat(r), color, rim, rim_k, 0.0);
    }

    pub fn count(&self) -> usize {
        self.items.iter().map(|v| v.len()).sum()
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

struct Mesh {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    count: u32,
}

pub struct Gfx {
    mesh: Vec<Mesh>,
    pipe: wgpu::RenderPipeline,
    sky: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    bind: wgpu::BindGroup,
    inst: wgpu::Buffer,
    inst_cap: usize,
    color: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    samples: u32,
}

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

impl Gfx {
    pub fn new(adapter: &wgpu::Adapter, device: &wgpu::Device, format: wgpu::TextureFormat, size: (u32, u32)) -> Self {
        let flags = adapter.get_texture_format_features(format).flags;
        let samples = if flags.sample_count_supported(4) && flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE) { 4 } else { 1 };
        log::info!("shadow gfx: {format:?}, msaa {samples}");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("shadow"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let globals = device.create_buffer(&wgpu::BufferDescriptor { label: Some("globals"), size: std::mem::size_of::<Globals>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None }],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("globals"), layout: &bgl, entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }] });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("shadow"), bind_group_layouts: &[&bgl], push_constant_ranges: &[] });
        let vattrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        let iattrs = wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4];
        let buffers = [
            wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &vattrs },
            wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Instance>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &iattrs },
        ];
        let ms = wgpu::MultisampleState { count: samples, mask: !0, alpha_to_coverage_enabled: false };
        let target = [Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })];
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("meshes"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: "vs_main", compilation_options: Default::default(), buffers: &buffers },
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: Default::default(), bias: Default::default() }),
            multisample: ms,
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: "fs_main", compilation_options: Default::default(), targets: &target }),
            multiview: None,
            cache: None,
        });
        let sky = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: "vs_sky", compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: false, depth_compare: wgpu::CompareFunction::LessEqual, stencil: Default::default(), bias: Default::default() }),
            multisample: ms,
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: "fs_sky", compilation_options: Default::default(), targets: &target }),
            multiview: None,
            cache: None,
        });
        let mesh = [sphere(28, 18), cylinder(28), cone(28)]
            .into_iter()
            .map(|(v, i)| Mesh {
                vb: device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("vb"), contents: bytemuck::cast_slice(&v), usage: wgpu::BufferUsages::VERTEX }),
                ib: device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("ib"), contents: bytemuck::cast_slice(&i), usage: wgpu::BufferUsages::INDEX }),
                count: i.len() as u32,
            })
            .collect();
        let inst_cap = 1024;
        let inst = device.create_buffer(&wgpu::BufferDescriptor { label: Some("inst"), size: (inst_cap * std::mem::size_of::<Instance>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let (color, depth) = targets(device, format, samples, size);
        Self { mesh, pipe, sky, globals, bind, inst, inst_cap, color, depth, size, format, samples }
    }

    pub fn render(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, size: (u32, u32), scene: &Scene) {
        if size != self.size {
            let (c, d) = targets(device, self.format, self.samples, size);
            self.color = c;
            self.depth = d;
            self.size = size;
        }
        let aspect = size.0.max(1) as f32 / size.1.max(1) as f32;
        let proj = Mat4::perspective_rh(scene.fov_y, aspect, 0.03, 200.0);
        let v = Mat4::look_at_rh(scene.eye, scene.target, Vec3::Y);
        let vp = proj * v;
        let sd = scene.sun_dir.normalize();
        let g = Globals {
            view_proj: vp.to_cols_array_2d(),
            inv_view_proj: vp.inverse().to_cols_array_2d(),
            cam: [scene.eye.x, scene.eye.y, scene.eye.z, 1.0],
            sun_dir: [sd.x, sd.y, sd.z, scene.sun],
            sun_color: [scene.sun_color.x, scene.sun_color.y, scene.sun_color.z, if scene.eclipse { 1.0 } else { 0.0 }],
            sky_top: [scene.sky_top.x, scene.sky_top.y, scene.sky_top.z, 1.0],
            sky_horizon: [scene.sky_horizon.x, scene.sky_horizon.y, scene.sky_horizon.z, 1.0],
            fog: [scene.sky_horizon.x, scene.sky_horizon.y, scene.sky_horizon.z, scene.fog],
            misc: [scene.time, scene.hurt, 0.0, 0.0],
        };
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&g));
        let total = scene.count();
        if total > self.inst_cap {
            self.inst_cap = total.next_power_of_two();
            self.inst = device.create_buffer(&wgpu::BufferDescriptor { label: Some("inst"), size: (self.inst_cap * std::mem::size_of::<Instance>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }
        let mut ranges = [(0u32, 0u32); SHAPES];
        let mut at = 0usize;
        for (k, list) in scene.items.iter().enumerate() {
            if !list.is_empty() {
                queue.write_buffer(&self.inst, (at * std::mem::size_of::<Instance>()) as u64, bytemuck::cast_slice(list));
            }
            ranges[k] = (at as u32, (at + list.len()) as u32);
            at += list.len();
        }
        let (target, resolve) = match &self.color {
            Some(c) => (c, Some(view)),
            None => (view, None),
        };
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target, resolve_target: resolve, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_pipeline(&self.pipe);
        pass.set_vertex_buffer(1, self.inst.slice(..));
        for (k, m) in self.mesh.iter().enumerate() {
            let (a, b) = ranges[k];
            if b > a {
                pass.set_vertex_buffer(0, m.vb.slice(..));
                pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.count, 0, a..b);
            }
        }
        pass.set_pipeline(&self.sky);
        pass.draw(0..3, 0..1);
    }
}

fn targets(device: &wgpu::Device, format: wgpu::TextureFormat, samples: u32, size: (u32, u32)) -> (Option<wgpu::TextureView>, wgpu::TextureView) {
    let extent = wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 };
    let mk = |fmt, label| {
        device
            .create_texture(&wgpu::TextureDescriptor { label: Some(label), size: extent, mip_level_count: 1, sample_count: samples, dimension: wgpu::TextureDimension::D2, format: fmt, usage: wgpu::TextureUsages::RENDER_ATTACHMENT, view_formats: &[] })
            .create_view(&Default::default())
    };
    let color = if samples > 1 { Some(mk(format, "msaa")) } else { None };
    (color, mk(DEPTH, "depth"))
}

fn sphere(seg: u32, rings: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    for r in 0..=rings {
        let phi = std::f32::consts::PI * r as f32 / rings as f32;
        for s in 0..=seg {
            let th = std::f32::consts::TAU * s as f32 / seg as f32;
            let n = [phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin()];
            v.push(Vertex { pos: n, normal: n });
        }
    }
    let w = seg + 1;
    for r in 0..rings {
        for s in 0..seg {
            let a = r * w + s;
            let b = a + w;
            i.extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    (v, i)
}

/// Unit cylinder along +Y from 0 to 1, radius 1, with caps.
fn cylinder(seg: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    for s in 0..=seg {
        let th = std::f32::consts::TAU * s as f32 / seg as f32;
        let (x, z) = (th.cos(), th.sin());
        v.push(Vertex { pos: [x, 0.0, z], normal: [x, 0.0, z] });
        v.push(Vertex { pos: [x, 1.0, z], normal: [x, 0.0, z] });
    }
    for s in 0..seg {
        let a = s * 2;
        i.extend_from_slice(&[a, a + 1, a + 2, a + 1, a + 3, a + 2]);
    }
    for (y, ny) in [(1.0f32, 1.0f32), (0.0, -1.0)] {
        let c = v.len() as u32;
        v.push(Vertex { pos: [0.0, y, 0.0], normal: [0.0, ny, 0.0] });
        for s in 0..=seg {
            let th = std::f32::consts::TAU * s as f32 / seg as f32;
            v.push(Vertex { pos: [th.cos(), y, th.sin()], normal: [0.0, ny, 0.0] });
        }
        for s in 0..seg {
            if ny > 0.0 {
                i.extend_from_slice(&[c, c + 2 + s, c + 1 + s]);
            } else {
                i.extend_from_slice(&[c, c + 1 + s, c + 2 + s]);
            }
        }
    }
    (v, i)
}

/// Unit cone: base radius 1 at y = 0, tip at y = 1.
fn cone(seg: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    let k = std::f32::consts::FRAC_1_SQRT_2;
    for s in 0..=seg {
        let th = std::f32::consts::TAU * s as f32 / seg as f32;
        let (x, z) = (th.cos(), th.sin());
        v.push(Vertex { pos: [x, 0.0, z], normal: [x * k, k, z * k] });
        v.push(Vertex { pos: [0.0, 1.0, 0.0], normal: [x * k, k, z * k] });
    }
    for s in 0..seg {
        let a = s * 2;
        i.extend_from_slice(&[a, a + 1, a + 2]);
    }
    (v, i)
}

pub(crate) const SHADER: &str = r#"
struct Globals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    sky_top: vec4<f32>,
    sky_horizon: vec4<f32>,
    fog: vec4<f32>,
    misc: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) m0: vec4<f32>,
    @location(3) m1: vec4<f32>,
    @location(4) m2: vec4<f32>,
    @location(5) m3: vec4<f32>,
    @location(6) color: vec4<f32>,
    @location(7) rim: vec4<f32>,
    @location(8) params: vec4<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) n: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) rim: vec4<f32>,
    @location(4) params: vec4<f32>,
};

@vertex
fn vs_main(v: VIn) -> VOut {
    let m = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    let world = m * vec4<f32>(v.pos, 1.0);
    let s = vec3<f32>(dot(v.m0.xyz, v.m0.xyz), dot(v.m1.xyz, v.m1.xyz), dot(v.m2.xyz, v.m2.xyz));
    let n = mat3x3<f32>(v.m0.xyz, v.m1.xyz, v.m2.xyz) * (v.normal / max(s, vec3<f32>(1e-6)));
    var o: VOut;
    o.clip = g.view_proj * world;
    o.world = world.xyz;
    o.n = n;
    o.color = v.color;
    o.rim = v.rim;
    o.params = v.params;
    return o;
}

fn tonemap(c: vec3<f32>) -> vec3<f32> {
    let x = c * 0.9;
    let t = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
    return pow(clamp(t, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));
}

fn finish(c: vec3<f32>, frag: vec2<f32>) -> vec4<f32> {
    var col = tonemap(c);
    // Red edges when the player takes a hit.
    let hurt = g.misc.y;
    if (hurt > 0.0) {
        col = mix(col, vec3<f32>(0.6, 0.0, 0.0), hurt * 0.35);
    }
    return vec4<f32>(col, 1.0);
}

@fragment
fn fs_main(i: VOut) -> @location(0) vec4<f32> {
    let n = normalize(i.n);
    let v = normalize(g.cam.xyz - i.world);
    var base = i.color.rgb;
    if (i.params.y > 0.5) {
        // Stone floor: flagstones in rings around the centre, a bright circle at the edge.
        let p = i.world.xz;
        let r = length(p);
        let a = atan2(p.y, p.x);
        let ring = fract(r * 0.7);
        let seg = fract(a * (3.0 + floor(r * 0.7) * 2.0) / 6.2831);
        let edge = min(min(ring, 1.0 - ring), min(seg, 1.0 - seg) * 1.5);
        base = base * mix(0.62, 1.0, smoothstep(0.0, 0.06, edge));
        let h = fract(sin(dot(floor(vec2<f32>(r * 0.7, a * 4.0)), vec2<f32>(12.9898, 78.233))) * 43758.5453);
        base = base * (0.85 + h * 0.3);
        let circle = smoothstep(0.12, 0.0, abs(r - 8.8));
        base = base + vec3<f32>(0.9, 0.85, 0.7) * circle * 0.35;
    }
    let l = g.sun_dir.xyz;
    let diff = max(dot(n, l), 0.0);
    let hemi = mix(g.sky_horizon.rgb * 0.22, g.sky_top.rgb * 0.55 + vec3<f32>(0.08), n.y * 0.5 + 0.5);
    let h = normalize(l + v);
    let spec = pow(max(dot(n, h), 0.0), 40.0) * 0.35 * diff;
    let rimk = pow(1.0 - max(dot(n, v), 0.0), 2.5) * i.rim.a;
    var c = base * (hemi + diff * g.sun_dir.w * g.sun_color.rgb) + spec * g.sun_color.rgb + rimk * i.rim.rgb;
    c = c + i.color.rgb * i.params.x;
    let d = length(g.cam.xyz - i.world);
    let f = 1.0 - exp(-d * g.fog.w);
    c = mix(c, g.fog.rgb * 0.8, f);
    return finish(c, i.clip.xy);
}

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) i: u32) -> SkyOut {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    var o: SkyOut;
    o.clip = vec4<f32>(x, y, 1.0, 1.0);
    o.ndc = vec2<f32>(x, y);
    return o;
}

@fragment
fn fs_sky(i: SkyOut) -> @location(0) vec4<f32> {
    let p = g.inv_view_proj * vec4<f32>(i.ndc, 1.0, 1.0);
    let dir = normalize(p.xyz / p.w - g.cam.xyz);
    let up = clamp(dir.y, -1.0, 1.0);
    var c = mix(g.sky_horizon.rgb, g.sky_top.rgb, pow(max(up, 0.0), 0.55));
    if (up < 0.0) {
        c = g.sky_horizon.rgb * 0.5;
    }
    let sd = dot(dir, g.sun_dir.xyz);
    if (g.sun_color.w > 0.5) {
        // Eclipse: a black disc with a burning ring.
        let ang = acos(clamp(sd, -1.0, 1.0));
        let ring = smoothstep(0.012, 0.0, abs(ang - 0.075));
        let corona = exp(-max(ang - 0.075, 0.0) * 18.0) * 0.8;
        c = c + g.sun_color.rgb * (ring * 3.0 + corona);
        if (ang < 0.072) {
            c = vec3<f32>(0.0);
        }
    } else {
        c = c + g.sun_color.rgb * (pow(max(sd, 0.0), 900.0) * 6.0 + pow(max(sd, 0.0), 12.0) * 0.4) * g.sun_dir.w;
    }
    // Faint stars in dark skies.
    let s = fract(sin(dot(floor(dir.xz / max(dir.y, 0.05) * 160.0), vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let dark = 1.0 - clamp(length(g.sky_top.rgb) * 3.0, 0.0, 1.0);
    if (s > 0.996 && up > 0.15) {
        c = c + vec3<f32>(0.8) * dark * (0.5 + 0.5 * sin(g.misc.x * 2.0 + s * 900.0));
    }
    return finish(c, i.clip.xy);
}
"#;

#[cfg(test)]
mod tests {
    #[test]
    fn shader_compiles() {
        let m = naga::front::wgsl::parse_str(super::SHADER).expect("wgsl parse");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()).validate(&m).expect("wgsl validate");
    }

    #[test]
    fn meshes_have_sane_indices() {
        for (v, i) in [super::sphere(28, 18), super::cylinder(28), super::cone(28)] {
            assert!(!i.is_empty() && i.len() % 3 == 0);
            assert!(i.iter().all(|&k| (k as usize) < v.len()));
        }
    }
}
