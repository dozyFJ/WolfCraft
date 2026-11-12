//! wgpu world renderer: chunk meshes, frustum culling, sky/fog, block outline.

use crate::cubesphere::{ChunkKey, Grid};
use crate::mesher::{MeshData, Vertex};
use self::model::ModelVertex;
use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Mat4, Vec3, Vec4};
use std::collections::{HashMap, HashSet, VecDeque};
use wolfcraft_assets::{AtlasKind, AtlasManager, TextureAtlas};

pub mod model;
pub mod sky;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const MAX_TILES: usize = 256;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    sun: [f32; 4],
    cam: [f32; 4],
    fog: [f32; 4],
    misc: [f32; 4],
    /// x: season 0..4 (spring, summer, autumn, winter), y: snow cover 0..1, z: biome
    /// humidity 0..1, w: unused.
    env: [f32; 4],
    /// x: relief mode (0 off, 1 lit relief, 2 parallax depth), y: relief strength, z: effect
    /// flags (1 wind, 2 water, 4 colour grading), w: unused.
    fx: [f32; 4],
    tiles: [[f32; 4]; MAX_TILES],
}

/// An atlas on the GPU (nearest filtering, no mip bleeding thanks to extruded padding).
pub struct GpuAtlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    kind: AtlasKind,
    generation: u64,
}

impl GpuAtlas {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, atlas: &TextureAtlas) -> Self {
        let (texture, view) = Self::create(device, queue, atlas);
        Self { texture, view, kind: atlas.kind(), generation: atlas.generation() }
    }

    fn create(device: &wgpu::Device, queue: &wgpu::Queue, atlas: &TextureAtlas) -> (wgpu::Texture, wgpu::TextureView) {
        let img = atlas.image();
        let size = wgpu::Extent3d { width: img.width, height: img.height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{:?}-atlas", atlas.kind())),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &img.pixels,
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(img.width * 4), rows_per_image: Some(img.height) },
            size,
        );
        let view = texture.create_view(&Default::default());
        (texture, view)
    }

    /// Uploads only what changed since the last call (partial writes for animated sprites,
    /// full recreate when the atlas was repacked). Returns true if the texture object changed.
    pub fn sync(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, atlas: &mut TextureAtlas) -> bool {
        if atlas.generation() != self.generation {
            let (t, v) = Self::create(device, queue, atlas);
            self.texture = t;
            self.view = v;
            self.generation = atlas.generation();
            atlas.take_dirty();
            return true;
        }
        let dirty = atlas.take_dirty();
        let img = atlas.image();
        for r in dirty {
            let mut data = Vec::with_capacity((r.w * r.h * 4) as usize);
            for y in r.y..r.y + r.h {
                let s = ((y * img.width + r.x) * 4) as usize;
                data.extend_from_slice(&img.pixels[s..s + (r.w * 4) as usize]);
            }
            queue.write_texture(
                wgpu::ImageCopyTexture { texture: &self.texture, mip_level: 0, origin: wgpu::Origin3d { x: r.x, y: r.y, z: 0 }, aspect: wgpu::TextureAspect::All },
                &data,
                wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(r.w * 4), rows_per_image: Some(r.h) },
                wgpu::Extent3d { width: r.w, height: r.h, depth_or_array_layers: 1 },
            );
        }
        false
    }

    pub fn kind(&self) -> AtlasKind {
        self.kind
    }

    /// One-channel linear texture from the red channel of `atlas` (relief heights).
    pub fn new_height(device: &wgpu::Device, queue: &wgpu::Queue, atlas: &TextureAtlas) -> Self {
        let img = atlas.image();
        let size = wgpu::Extent3d { width: img.width, height: img.height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("relief-atlas"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let data: Vec<u8> = img.pixels.chunks_exact(4).map(|p| p[0]).collect();
        queue.write_texture(
            wgpu::ImageCopyTexture { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &data,
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(img.width), rows_per_image: Some(img.height) },
            size,
        );
        let view = texture.create_view(&Default::default());
        Self { texture, view, kind: atlas.kind(), generation: atlas.generation() }
    }
}

struct MeshBuf {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    vcap: u64,
    icap: u64,
    index_count: u32,
}

struct ChunkGpu {
    opaque: Option<MeshBuf>,
    water: Option<MeshBuf>,
    /// World position the mesh vertices are relative to.
    origin: DVec3,
    /// Bounding sphere for culling.
    center: DVec3,
    radius: f64,
    /// Opaque indices of blocks and shapes; the rest (grass, flowers) is skipped far away.
    detail_start: u32,
}

/// Beyond this distance (blocks) chunks are drawn without small details (grass, flowers).
pub const DETAIL_DIST: f32 = 56.0;

/// Chunks that can be seen from `start` (cave culling, as in Minecraft): a breadth-first walk
/// over the chunk grid that only passes from one side of a chunk to another when open blocks
/// connect them (`vis`), and never turns back towards the camera. Chunks behind solid rock or
/// walls are not reached. Unknown chunks count as open, so nothing visible is ever lost;
/// crossing a cube edge also resets the walk to "open" (the axes turn there).
pub fn reachable_chunks(grid: &Grid, start: ChunkKey, radius_chunks: f64, vis: impl Fn(ChunkKey) -> u64) -> HashSet<ChunkKey> {
    use crate::mesher::{SIDE_DIRS, sides_connected};
    let origin = grid.chunk_center(start);
    let mut seen: HashSet<ChunkKey> = HashSet::new();
    // (chunk, side it was entered through, directions taken so far)
    let mut queue: VecDeque<(ChunkKey, Option<usize>, u8)> = VecDeque::new();
    seen.insert(start);
    queue.push_back((start, None, 0));
    while let Some((key, from, dirs)) = queue.pop_front() {
        let v = vis(key);
        for (d, &(di, dj, dk)) in SIDE_DIRS.iter().enumerate() {
            if dirs & (1 << (d ^ 1)) != 0 {
                continue; // would walk back towards the camera
            }
            if let Some(e) = from {
                if !sides_connected(v, e, d) {
                    continue;
                }
            }
            let Some(nb) = grid.chunk_neighbor(key, di, dj, dk) else { continue };
            if seen.contains(&nb) || (grid.chunk_center(nb) - origin).length() / 16.0 > radius_chunks {
                continue;
            }
            seen.insert(nb);
            if nb.f == key.f {
                queue.push_back((nb, Some(d ^ 1), dirs | (1 << d)));
            } else {
                queue.push_back((nb, None, 0));
            }
        }
    }
    seen
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStats {
    pub draw_calls: u32,
    pub triangles: u32,
    pub chunks_total: usize,
    pub chunks_visible: usize,
    /// In view but hidden behind walls or rock (cave culling).
    pub chunks_hidden: usize,
    pub buffer_bytes: u64,
    pub uploads_this_frame: u32,
}

pub struct FrameParams {
    /// Season 0..4 (spring, summer, autumn, winter), snow cover 0..1 and the biome's humidity:
    /// leaves and grass take the season's colours, winter puts snow on them.
    pub season: f32,
    pub snow: f32,
    pub humidity: f32,
    /// Fog thickness of the biome (1 = normal, more = mistier).
    pub fog_density: f32,
    /// Texture relief: 0 off, 1 lit relief (normal from the height maps), 2 parallax depth.
    pub relief_mode: u8,
    /// Relief strength (1 = normal).
    pub relief_strength: f32,
    /// Shader effects: 1 wind in leaves and grass, 2 water waves and glints, 4 colour grading.
    pub fx_flags: u8,
    /// Shader preset: 0 off, 1 light (swaying leaves and grass, water waves, colour
    /// grading), 2 ultra (also texture relief lit by the sun).
    pub shaders: u8,
    pub eye: DVec3,
    pub forward: Vec3,
    pub up: Vec3,
    pub fov_y: f32,
    pub aspect: f32,
    pub sun_dir: Vec3,
    /// Daylight at the player's location, 0..1.
    pub day: f32,
    pub underwater: bool,
    pub view_distance_blocks: f32,
    pub time: f32,
    /// Corners of the targeted cell (index bits: i, j, k), world space.
    pub outline: Option<[DVec3; 8]>,
    /// Mining progress 0..1 on the outlined block (cracks); 0 = none.
    pub crack: f32,
    /// Vertical FOV of the first-person view model (hands/held item), independent of `fov_y`.
    pub viewmodel_fov_y: f32,
}

pub struct WorldRenderer {
    opaque_pipeline: wgpu::RenderPipeline,
    water_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    crack_pipeline: wgpu::RenderPipeline,
    crack_buf: wgpu::Buffer,
    model_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    sky_buf: wgpu::Buffer,
    sky_cap: usize,
    sky_count: u32,
    model_buf: wgpu::Buffer,
    model_cap: usize,
    model_count: u32,
    hands_buf: wgpu::Buffer,
    hands_cap: usize,
    hands_count: u32,
    /// Textured block held in the right hand (world-pipeline vertices, 72 max).
    held_buf: wgpu::Buffer,
    held_count: u32,
    zero_inst: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    globals: wgpu::Buffer,
    bind_group_hands: wgpu::BindGroup,
    globals_hands: wgpu::Buffer,
    sampler: wgpu::Sampler,
    pub blocks_atlas: GpuAtlas,
    /// Height maps of the block textures (relief shaders).
    relief_atlas: GpuAtlas,
    entities_atlas: GpuAtlas,
    items_atlas: GpuAtlas,
    tile_rects: [[f32; 4]; MAX_TILES],
    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
    /// World resolution scale (< 1 renders into the offscreen texture and stretches it to the screen).
    render_scale: f32,
    offscreen: Option<(wgpu::Texture, wgpu::TextureView, wgpu::BindGroup)>,
    blit_pipeline: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,
    color_format: wgpu::TextureFormat,
    chunks: HashMap<ChunkKey, ChunkGpu>,
    grid: Grid,
    /// Visibility sets of every meshed chunk (also empty ones), for cave culling.
    chunk_vis: HashMap<ChunkKey, u64>,
    /// Chunks reachable from the camera chunk, recomputed when the camera changes chunk or
    /// chunks were (re)meshed.
    reachable: Option<(ChunkKey, HashSet<ChunkKey>)>,
    vis_dirty: bool,
    frames_since_cull: u32,
    /// Skip chunks hidden behind walls and rock (on by default).
    pub occlusion_culling: bool,
    instance_buf: wgpu::Buffer,
    instance_cap: usize,
    line_buf: wgpu::Buffer,
    pub stats: RenderStats,
    uploads: u32,
}

fn vertex_layouts() -> ([wgpu::VertexAttribute; 4], [wgpu::VertexAttribute; 1]) {
    (
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Uint32, 3 => Float32x2],
        wgpu::vertex_attr_array![4 => Float32x4],
    )
}

impl WorldRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, color_format: wgpu::TextureFormat, atlases: &AtlasManager, width: u32, height: u32) -> Self {
        let blocks = atlases.get(AtlasKind::Blocks);
        let blocks_atlas = GpuAtlas::new(device, queue, blocks);
        let relief_atlas = GpuAtlas::new_height(device, queue, atlases.relief());
        let entities_atlas = GpuAtlas::new(device, queue, atlases.get(AtlasKind::Entities));
        let items_atlas = GpuAtlas::new(device, queue, atlases.get(AtlasKind::Items));
        set_model_textures(atlases);
        let mut tile_rects = [[0.0f32; 4]; MAX_TILES];
        for (i, rect) in tile_rects.iter_mut().enumerate().take(blocks.len().min(MAX_TILES)) {
            let uv = blocks.uv(i);
            *rect = [uv.min[0], uv.min[1], uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]];
        }

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("world-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
            ],
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pixel-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let bind_group = Self::make_bind_group(device, &layout, &globals, [&blocks_atlas.view, &entities_atlas.view, &items_atlas.view, &relief_atlas.view], &sampler);
        let globals_hands = device.create_buffer(&wgpu::BufferDescriptor { label: Some("globals-hands"), size: std::mem::size_of::<Globals>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let bind_group_hands = Self::make_bind_group(device, &layout, &globals_hands, [&blocks_atlas.view, &entities_atlas.view, &items_atlas.view, &relief_atlas.view], &sampler);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("world-shader"), source: wgpu::ShaderSource::Wgsl(include_str!("world.wgsl").into()) });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("world-pl"), bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let (va, ia) = vertex_layouts();
        let vbuffers = [
            wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &va },
            wgpu::VertexBufferLayout { array_stride: 16, step_mode: wgpu::VertexStepMode::Instance, attributes: &ia },
        ];
        let make = |label: &str, fs: &str, cull: Option<wgpu::Face>, blend: Option<wgpu::BlendState>, depth_write: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                vertex: wgpu::VertexState { module: &shader, entry_point: "vs_main", compilation_options: Default::default(), buffers: &vbuffers },
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, front_face: wgpu::FrontFace::Ccw, cull_mode: cull, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: depth_write,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: fs,
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: color_format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview: None,
                cache: None,
            })
        };
        let opaque_pipeline = make("opaque", "fs_opaque", Some(wgpu::Face::Back), None, true);
        let water_pipeline = make("water", "fs_water", None, Some(KEEP_ALPHA_BLEND), false);

        let line_attrs = wgpu::vertex_attr_array![0 => Float32x3];
        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("outline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_line",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout { array_stride: 12, step_mode: wgpu::VertexStepMode::Vertex, attributes: &line_attrs }],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::LineList, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: false, depth_compare: wgpu::CompareFunction::LessEqual, stencil: Default::default(), bias: Default::default() }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_line",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: color_format, blend: Some(KEEP_ALPHA_BLEND), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview: None,
            cache: None,
        });

        let crack_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2];
        let crack_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("crack"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_crack",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout { array_stride: 20, step_mode: wgpu::VertexStepMode::Vertex, attributes: &crack_attrs }],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: false, depth_compare: wgpu::CompareFunction::LessEqual, stencil: Default::default(), bias: Default::default() }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_crack",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: color_format, blend: Some(KEEP_ALPHA_BLEND), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview: None,
            cache: None,
        });
        let crack_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("crack-vb"), size: 20 * 36, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });

        let model_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x2, 4 => Float32];
        let model_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("model"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_model",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<ModelVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &model_attrs }],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, front_face: wgpu::FrontFace::Ccw, cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: true, depth_compare: wgpu::CompareFunction::Less, stencil: Default::default(), bias: Default::default() }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_model",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview: None,
            cache: None,
        });
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_model",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<ModelVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &model_attrs }],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: false, depth_compare: wgpu::CompareFunction::Always, stencil: Default::default(), bias: Default::default() }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_sky",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview: None,
            cache: None,
        });
        let sky_cap = 16384;
        let sky_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("sky-vb"), size: (sky_cap * std::mem::size_of::<ModelVertex>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let model_cap = 2048;
        let model_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("model-vb"), size: (model_cap * std::mem::size_of::<ModelVertex>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });

        let hands_cap = 256;
        let held_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("held-vb"), size: (72 * std::mem::size_of::<Vertex>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let zero_inst = device.create_buffer(&wgpu::BufferDescriptor { label: Some("zero-inst"), size: 16, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        queue.write_buffer(&zero_inst, 0, bytemuck::cast_slice(&[0.0f32; 4]));
        let hands_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("hands-vb"), size: (hands_cap * std::mem::size_of::<ModelVertex>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });

        let instance_cap = 1024;
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("instances"), size: (instance_cap * 16) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let line_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("outline-lines"), size: 24 * 12, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let (_, depth_view) = Self::make_depth(device, width, height);
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("blit"), source: wgpu::ShaderSource::Wgsl(include_str!("blit.wgsl").into()) });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let blit_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("blit-pl"), bind_group_layouts: &[&blit_layout], push_constant_ranges: &[] });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit"),
            layout: Some(&blit_pl),
            vertex: wgpu::VertexState { module: &blit_shader, entry_point: "vs", compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState { module: &blit_shader, entry_point: "fs", compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: color_format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            multiview: None,
            cache: None,
        });
        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, address_mode_u: wgpu::AddressMode::ClampToEdge, address_mode_v: wgpu::AddressMode::ClampToEdge, ..Default::default() });

        Self {
            opaque_pipeline,
            water_pipeline,
            line_pipeline,
            crack_pipeline,
            crack_buf,
            model_pipeline,
            sky_pipeline,
            sky_buf,
            sky_cap,
            sky_count: 0,
            model_buf,
            model_cap,
            model_count: 0,
            hands_buf,
            hands_cap,
            hands_count: 0,
            held_buf,
            held_count: 0,
            zero_inst,
            layout,
            bind_group,
            globals,
            bind_group_hands,
            globals_hands,
            sampler,
            blocks_atlas,
            relief_atlas,
            entities_atlas,
            items_atlas,
            tile_rects,
            depth_view,
            depth_size: (width.max(1), height.max(1)),
            render_scale: 1.0,
            offscreen: None,
            blit_pipeline,
            blit_layout,
            blit_sampler,
            color_format,
            chunks: HashMap::new(),
            grid: Grid::with_cells(16),
            chunk_vis: HashMap::new(),
            reachable: None,
            vis_dirty: true,
            frames_since_cull: 0,
            occlusion_culling: true,
            instance_buf,
            instance_cap,
            line_buf,
            stats: RenderStats::default(),
            uploads: 0,
        }
    }

    fn make_bind_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, globals: &wgpu::Buffer, views: [&wgpu::TextureView; 4], sampler: &wgpu::Sampler) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("world-bg"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(views[0]) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(views[1]) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(views[2]) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(views[3]) },
            ],
        })
    }

    fn make_depth(device: &wgpu::Device, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let t = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let v = t.create_view(&Default::default());
        (t, v)
    }

    /// Renders the world at a fraction of the screen resolution (phones) and stretches it.
    pub fn set_render_scale(&mut self, scale: f32) {
        self.render_scale = scale.clamp(0.4, 1.0);
        self.depth_size = (0, 0);
    }

    /// The screen size is w x h pixels; the world targets are render_scale times that.
    pub fn resize(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        let tw = ((w.max(1) as f32 * self.render_scale) as u32).max(1);
        let th = ((h.max(1) as f32 * self.render_scale) as u32).max(1);
        if (tw, th) != self.depth_size {
            self.depth_view = Self::make_depth(device, tw, th).1;
            self.depth_size = (tw, th);
            self.offscreen = None;
        }
        if self.render_scale < 0.99 && self.offscreen.is_none() {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("world-offscreen"),
                size: wgpu::Extent3d { width: tw, height: th, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.color_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("blit-bg"),
                layout: &self.blit_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.blit_sampler) }],
            });
            self.offscreen = Some((tex, view, bg));
        } else if self.render_scale >= 0.99 {
            self.offscreen = None;
        }
    }

    /// Pushes animated/changed atlas data to the GPU. Cheap when nothing changed.
    /// Swaps in new atlases (another resource pack). Chunk meshes keep working: they refer to
    /// tiles by index and the shader looks the UV rectangles up.
    pub fn reload_atlases(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, atlases: &AtlasManager) {
        set_model_textures(atlases);
        let blocks = atlases.get(AtlasKind::Blocks);
        self.blocks_atlas = GpuAtlas::new(device, queue, blocks);
        self.relief_atlas = GpuAtlas::new_height(device, queue, atlases.relief());
        self.entities_atlas = GpuAtlas::new(device, queue, atlases.get(AtlasKind::Entities));
        self.items_atlas = GpuAtlas::new(device, queue, atlases.get(AtlasKind::Items));
        for (i, rect) in self.tile_rects.iter_mut().enumerate().take(blocks.len().min(MAX_TILES)) {
            let uv = blocks.uv(i);
            *rect = [uv.min[0], uv.min[1], uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]];
        }
        self.bind_group = Self::make_bind_group(device, &self.layout, &self.globals, [&self.blocks_atlas.view, &self.entities_atlas.view, &self.items_atlas.view, &self.relief_atlas.view], &self.sampler);
        self.bind_group_hands = Self::make_bind_group(device, &self.layout, &self.globals_hands, [&self.blocks_atlas.view, &self.entities_atlas.view, &self.items_atlas.view, &self.relief_atlas.view], &self.sampler);
    }

    pub fn sync_atlas(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, atlases: &mut AtlasManager) {
        let atlas = atlases.get_mut(AtlasKind::Blocks);
        if self.blocks_atlas.sync(device, queue, atlas) {
            for (i, rect) in self.tile_rects.iter_mut().enumerate().take(atlas.len().min(MAX_TILES)) {
                let uv = atlas.uv(i);
                *rect = [uv.min[0], uv.min[1], uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]];
            }
            self.bind_group = Self::make_bind_group(device, &self.layout, &self.globals, [&self.blocks_atlas.view, &self.entities_atlas.view, &self.items_atlas.view, &self.relief_atlas.view], &self.sampler);
            self.bind_group_hands = Self::make_bind_group(device, &self.layout, &self.globals_hands, [&self.blocks_atlas.view, &self.entities_atlas.view, &self.items_atlas.view, &self.relief_atlas.view], &self.sampler);
        }
    }

    fn upload_buf(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, old: Option<MeshBuf>, verts: &[Vertex], idx: &[u32]) -> Option<MeshBuf> {
        if idx.is_empty() {
            return None;
        }
        let vbytes: &[u8] = bytemuck::cast_slice(verts);
        let ibytes: &[u8] = bytemuck::cast_slice(idx);
        self.uploads += 1;
        // Reuse existing buffers when the new mesh fits and is not wastefully smaller.
        if let Some(mut m) = old {
            let fits = vbytes.len() as u64 <= m.vcap && ibytes.len() as u64 <= m.icap && (vbytes.len() as u64) * 4 >= m.vcap;
            if fits {
                queue.write_buffer(&m.vb, 0, vbytes);
                queue.write_buffer(&m.ib, 0, ibytes);
                m.index_count = idx.len() as u32;
                return Some(m);
            }
        }
        let vcap = (vbytes.len() as u64).next_power_of_two().max(256);
        let icap = (ibytes.len() as u64).next_power_of_two().max(256);
        let vb = device.create_buffer(&wgpu::BufferDescriptor { label: Some("chunk-vb"), size: vcap, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let ib = device.create_buffer(&wgpu::BufferDescriptor { label: Some("chunk-ib"), size: icap, usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        queue.write_buffer(&vb, 0, vbytes);
        queue.write_buffer(&ib, 0, ibytes);
        Some(MeshBuf { vb, ib, vcap, icap, index_count: idx.len() as u32 })
    }

    /// Planet grid used to place chunk meshes. Changing it drops every uploaded chunk.
    pub fn set_grid(&mut self, grid: Grid) {
        if grid != self.grid {
            self.grid = grid;
            self.clear();
        }
    }

    pub fn upload_mesh(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, key: ChunkKey, mesh: MeshData) {
        let (old_o, old_w) = match self.chunks.remove(&key) {
            Some(c) => (c.opaque, c.water),
            None => (None, None),
        };
        let opaque = self.upload_buf(device, queue, old_o, &mesh.opaque, &mesh.opaque_idx);
        let water = self.upload_buf(device, queue, old_w, &mesh.water, &mesh.water_idx);
        if self.chunk_vis.insert(key, mesh.vis) != Some(mesh.vis) {
            self.vis_dirty = true;
        }
        if opaque.is_some() || water.is_some() {
            let (center, radius) = self.grid.chunk_bounds(key);
            self.chunks.insert(key, ChunkGpu { opaque, water, origin: self.grid.chunk_ref(key), center, radius, detail_start: mesh.detail_start });
        }
    }

    /// Sets the sky triangles (from sky::build_sky), drawn first and unlit.
    pub fn set_sky(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, verts: &[ModelVertex]) {
        if verts.len() > self.sky_cap {
            self.sky_cap = verts.len().next_power_of_two();
            self.sky_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("sky-vb"), size: (self.sky_cap * std::mem::size_of::<ModelVertex>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }
        if !verts.is_empty() {
            queue.write_buffer(&self.sky_buf, 0, bytemuck::cast_slice(verts));
        }
        self.sky_count = verts.len() as u32;
    }

    /// Sets the entity-model triangles drawn this frame (empty = nothing).
    pub fn set_model(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, verts: &[ModelVertex]) {
        if verts.len() > self.model_cap {
            self.model_cap = verts.len().next_power_of_two();
            self.model_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("model-vb"), size: (self.model_cap * std::mem::size_of::<ModelVertex>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }
        if !verts.is_empty() {
            queue.write_buffer(&self.model_buf, 0, bytemuck::cast_slice(verts));
        }
        self.model_count = verts.len() as u32;
    }

    /// Sets the first-person arm triangles (empty = no arms). They are drawn on top of the
    /// world with their own depth, so they never sink into nearby blocks.
    /// Sets the held block (from `model::build_held_block`); empty = nothing in hand.
    pub fn set_held(&mut self, queue: &wgpu::Queue, verts: &[Vertex]) {
        let n = verts.len().min(72);
        if n > 0 {
            queue.write_buffer(&self.held_buf, 0, bytemuck::cast_slice(&verts[..n]));
        }
        self.held_count = n as u32;
    }

    pub fn set_hands(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, verts: &[ModelVertex]) {
        if verts.len() > self.hands_cap {
            self.hands_cap = verts.len().next_power_of_two();
            self.hands_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("hands-vb"), size: (self.hands_cap * std::mem::size_of::<ModelVertex>()) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }
        if !verts.is_empty() {
            queue.write_buffer(&self.hands_buf, 0, bytemuck::cast_slice(verts));
        }
        self.hands_count = verts.len() as u32;
    }

    pub fn remove_chunk(&mut self, key: ChunkKey) {
        self.chunks.remove(&key);
        if self.chunk_vis.remove(&key).is_some() {
            self.vis_dirty = true;
        }
    }

    pub fn clear(&mut self) {
        self.chunks.clear();
        self.chunk_vis.clear();
        self.reachable = None;
        self.vis_dirty = true;
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Records the world pass. `view` is the swapchain image; `clear` the sky colour.
    pub fn render(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, p: &FrameParams, clear: [f32; 3], fog_color: [f32; 3]) {
        let target: &wgpu::TextureView = self.offscreen.as_ref().map(|o| &o.1).unwrap_or(view);
        let far = p.view_distance_blocks * 1.6 + 48.0;
        let proj = Mat4::perspective_rh(p.fov_y, p.aspect.max(0.01), 0.08, far);
        let viewm = Mat4::look_to_rh(Vec3::ZERO, p.forward, p.up);
        let vp = proj * viewm;

        let mut g = Globals {
            view_proj: vp.to_cols_array_2d(),
            sun: [p.sun_dir.x, p.sun_dir.y, p.sun_dir.z, 0.0],
            cam: [p.eye.x as f32, p.eye.y as f32, p.eye.z as f32, 0.0],
            // Fog is mixed in linear space inside the shader; the caller passes display colours.
            fog: [fog_color[0].powf(2.2), fog_color[1].powf(2.2), fog_color[2].powf(2.2), p.fog_density.max(0.5) / (p.view_distance_blocks * 1.15).max(16.0)],
            misc: [p.time, p.underwater as u32 as f32, p.crack, p.shaders as f32],
            env: [p.season, p.snow, p.humidity, 0.0],
            fx: [p.relief_mode as f32, p.relief_strength, p.fx_flags as f32, 0.0],
            tiles: self.tile_rects,
        };
        if p.underwater {
            g.fog[3] = 1.0 / 60.0;
        }
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&g));
        // View model: same camera orientation, its own projection (FOV) and a short far plane.
        let vm_fov = if p.viewmodel_fov_y > 0.1 { p.viewmodel_fov_y } else { p.fov_y };
        let mut gh = g;
        gh.view_proj = (Mat4::perspective_rh(vm_fov, p.aspect.max(0.01), 0.02, 8.0) * viewm).to_cols_array_2d();
        queue.write_buffer(&self.globals_hands, 0, bytemuck::bytes_of(&gh));

        // Cave culling: which chunks can be seen from the camera's chunk at all. Recomputed
        // when the camera enters another chunk, or a few frames after chunks were (re)meshed.
        self.frames_since_cull += 1;
        let cam_chunk = ChunkKey::of(self.grid.world_to_cell(p.eye));
        let radius_chunks = (p.view_distance_blocks / 16.0) as f64 + 1.5;
        if self.occlusion_culling {
            let moved = self.reachable.as_ref().is_none_or(|(k, _)| *k != cam_chunk);
            if moved || (self.vis_dirty && self.frames_since_cull >= 10) {
                let vis = &self.chunk_vis;
                let set = reachable_chunks(&self.grid, cam_chunk, radius_chunks, |k| vis.get(&k).copied().unwrap_or(crate::mesher::VIS_ALL));
                self.reachable = Some((cam_chunk, set));
                self.vis_dirty = false;
                self.frames_since_cull = 0;
            }
        } else {
            self.reachable = None;
        }
        // Frustum and distance culling in camera-relative space.
        let planes = frustum_planes(&vp);
        let max_dist = p.view_distance_blocks * 1.15 + 8.0;
        let mut visible: Vec<(ChunkKey, f32)> = Vec::with_capacity(self.chunks.len());
        let mut offsets: Vec<[f32; 4]> = Vec::with_capacity(self.chunks.len());
        let mut hidden = 0usize;
        for (key, c) in self.chunks.iter() {
            let centre = (c.center - p.eye).as_vec3();
            if centre.length() - c.radius as f32 > max_dist || !sphere_in_frustum(&planes, centre, c.radius as f32) {
                continue;
            }
            if self.reachable.as_ref().is_some_and(|(_, set)| !set.contains(key)) {
                hidden += 1;
                continue;
            }
            visible.push((*key, centre.length_squared()));
        }
        visible.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        for (key, _) in &visible {
            let o = (self.chunks[key].origin - p.eye).as_vec3();
            offsets.push([o.x, o.y, o.z, 0.0]);
        }
        if offsets.len() > self.instance_cap {
            self.instance_cap = offsets.len().next_power_of_two();
            self.instance_buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("instances"), size: (self.instance_cap * 16) as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }
        if !offsets.is_empty() {
            queue.write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(&offsets));
        }

        // Block outline (12 edges, slightly inflated to avoid z-fighting).
        let mut has_outline = false;
        let mut has_crack = false;
        if let Some(corners) = p.outline {
            // Push the corners slightly away from the cell centre to avoid z-fighting.
            let centre = corners.iter().fold(DVec3::ZERO, |s, c| s + *c) / 8.0;
            let pt = |n: usize| ((corners[n] - centre) * 1.006 + centre - p.eye).as_vec3().to_array();
            let mut v: Vec<[f32; 3]> = Vec::with_capacity(24);
            for a in 0..8usize {
                for bit in [1usize, 2, 4] {
                    if a & bit == 0 {
                        v.push(pt(a));
                        v.push(pt(a | bit));
                    }
                }
            }
            queue.write_buffer(&self.line_buf, 0, bytemuck::cast_slice(&v));
            has_outline = true;
            if p.crack > 0.01 {
                // Six faces with their own 0..1 uv, pushed out a little more than the outline.
                let q = |n: usize| ((corners[n] - centre) * 1.004 + centre - p.eye).as_vec3();
                let mut tri: Vec<[f32; 5]> = Vec::with_capacity(36);
                for axis in [1usize, 2, 4] {
                    let (ua, va) = match axis {
                        1 => (2usize, 4usize),
                        2 => (1, 4),
                        _ => (1, 2),
                    };
                    for side in [0usize, axis] {
                        let quad = [(0usize, 0usize), (1, 0), (1, 1), (0, 1)].map(|(u, w)| {
                            let n = side | if u == 1 { ua } else { 0 } | if w == 1 { va } else { 0 };
                            let v = q(n);
                            [v.x, v.y, v.z, u as f32, w as f32]
                        });
                        for i in [0usize, 1, 2, 0, 2, 3] {
                            tri.push(quad[i]);
                        }
                    }
                }
                queue.write_buffer(&self.crack_buf, 0, bytemuck::cast_slice(&tri));
                has_crack = true;
            }
        }

        let mut stats = RenderStats { chunks_total: self.chunks.len(), chunks_visible: visible.len(), chunks_hidden: hidden, ..Default::default() };
        let detail_d2 = DETAIL_DIST * DETAIL_DIST;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: clear[0] as f64, g: clear[1] as f64, b: clear[2] as f64, a: 1.0 }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.depth_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));

            if self.sky_count > 0 {
                pass.set_pipeline(&self.sky_pipeline);
                pass.set_vertex_buffer(0, self.sky_buf.slice(..));
                pass.draw(0..self.sky_count, 0..1);
            }

            pass.set_pipeline(&self.opaque_pipeline);
            for (i, (cc, d2)) in visible.iter().enumerate() {
                if let Some(c) = self.chunks.get(cc) {
                    if let Some(m) = c.opaque.as_ref() {
                        // Far chunks: blocks only, no grass and flowers (level of detail).
                        let count = if *d2 > detail_d2 { c.detail_start.min(m.index_count) } else { m.index_count };
                        if count == 0 {
                            continue;
                        }
                        pass.set_vertex_buffer(0, m.vb.slice(..));
                        pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..count, 0, i as u32..i as u32 + 1);
                        stats.draw_calls += 1;
                        stats.triangles += count / 3;
                    }
                }
            }

            if self.model_count > 0 {
                pass.set_pipeline(&self.model_pipeline);
                pass.set_vertex_buffer(0, self.model_buf.slice(..));
                pass.draw(0..self.model_count, 0..1);
                stats.draw_calls += 1;
                stats.triangles += self.model_count / 3;
            }

            if has_crack {
                pass.set_pipeline(&self.crack_pipeline);
                pass.set_vertex_buffer(0, self.crack_buf.slice(..));
                pass.draw(0..36, 0..1);
                stats.draw_calls += 1;
            }
            if has_outline {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_vertex_buffer(0, self.line_buf.slice(..));
                pass.draw(0..24, 0..1);
                stats.draw_calls += 1;
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            }

            // Water: far to near so blending composes correctly.
            pass.set_pipeline(&self.water_pipeline);
            for (i, (cc, _)) in visible.iter().enumerate().rev() {
                if let Some(m) = self.chunks.get(cc).and_then(|c| c.water.as_ref()) {
                    pass.set_vertex_buffer(0, m.vb.slice(..));
                    pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.index_count, 0, i as u32..i as u32 + 1);
                    stats.draw_calls += 1;
                    stats.triangles += m.index_count / 3;
                }
            }
        }
        // First-person arms: keep the world image, start from an empty depth buffer.
        if self.hands_count > 0 || self.held_count > 0 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hands-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.depth_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.bind_group_hands, &[]);
            pass.set_pipeline(&self.model_pipeline);
            if self.hands_count > 0 {
                pass.set_vertex_buffer(0, self.hands_buf.slice(..));
                pass.draw(0..self.hands_count, 0..1);
            }
            if self.held_count > 0 {
                pass.set_pipeline(&self.opaque_pipeline);
                pass.set_vertex_buffer(0, self.held_buf.slice(..));
                pass.set_vertex_buffer(1, self.zero_inst.slice(..));
                pass.draw(0..self.held_count, 0..1);
                stats.draw_calls += 1;
            }
            stats.draw_calls += 1;
            stats.triangles += self.hands_count / 3;
        }
        if let Some((_, _, bg)) = &self.offscreen {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world-blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
        }
        stats.buffer_bytes = self.chunks.values().map(|c| c.opaque.iter().chain(c.water.iter()).map(|m| m.vcap + m.icap).sum::<u64>()).sum();
        stats.uploads_this_frame = std::mem::take(&mut self.uploads);
        self.stats = stats;
    }
}

/// Models sample skins and item icons from these atlases.
fn set_model_textures(atlases: &AtlasManager) {
            let ent = atlases.get(AtlasKind::Entities);
            let it = atlases.get(AtlasKind::Items);
            let rect = |u: wolfcraft_assets::UvRect| [u.min[0], u.min[1], u.max[0], u.max[1]];
            model::set_textures(model::ModelTextures {
                player: (ent.len() > wolfcraft_assets::entities::PLAYER as usize).then(|| rect(ent.uv(wolfcraft_assets::entities::PLAYER as usize))),
                items: it.uvs().iter().map(|u| rect(*u)).collect(),
                item_pixels: (0..it.len())
                    .map(|i| {
                        // RGBA pixels of each 16x16 icon, for 3D (voxel) held items.
                        let r = it.rect(i);
                        let img = it.image();
                        let mut px = Vec::with_capacity(256);
                        for y in 0..16u32 {
                            for x in 0..16u32 {
                                let (sx, sy) = (r.x + x * r.w / 16, r.y + y * r.h / 16);
                                let o = ((sy * img.width + sx) * 4) as usize;
                                px.push([img.pixels[o], img.pixels[o + 1], img.pixels[o + 2], img.pixels[o + 3]]);
                            }
                        }
                        px
                    })
                    .collect(),
            });
        }

/// Six frustum planes (normal.xyz, d) from a view-projection matrix (depth range 0..1).
pub fn frustum_planes(vp: &Mat4) -> [Vec4; 6] {
    let r0 = vp.row(0);
    let r1 = vp.row(1);
    let r2 = vp.row(2);
    let r3 = vp.row(3);
    let mut planes = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2];
    for p in planes.iter_mut() {
        let l = p.truncate().length();
        if l > 0.0 {
            *p /= l;
        }
    }
    planes
}

pub fn sphere_in_frustum(planes: &[Vec4; 6], centre: Vec3, radius: f32) -> bool {
    planes.iter().all(|p| p.truncate().dot(centre) + p.w >= -radius)
}

pub fn aabb_in_frustum(planes: &[Vec4; 6], min: Vec3, max: Vec3) -> bool {
    for p in planes {
        let n = p.truncate();
        let v = Vec3::new(if n.x >= 0.0 { max.x } else { min.x }, if n.y >= 0.0 { max.y } else { min.y }, if n.z >= 0.0 { max.z } else { min.z });
        if n.dot(v) + p.w < 0.0 {
            return false;
        }
    }
    true
}

/// Sky and fog colour for the given daylight (0 night … 1 day) and altitude (0 surface … 1 space).
pub fn sky_color(day: f32, twilight: f32, altitude: f32) -> [f32; 3] {
    let night = Vec3::new(0.012, 0.018, 0.05);
    // Bright, saturated blue at noon and a vivid orange-pink dusk.
    let noon = Vec3::new(0.36, 0.64, 1.0);
    let dusk = Vec3::new(1.0, 0.50, 0.32);
    let mut c = night.lerp(noon, day.clamp(0.0, 1.0));
    c = c.lerp(dusk, (twilight * 0.55).clamp(0.0, 0.55));
    c *= 1.0 - altitude.clamp(0.0, 1.0) * 0.85;
    [c.x, c.y, c.z]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_behind_a_solid_floor_are_not_reached() {
        let grid = Grid::new(1024.0);
        let start = ChunkKey::new(4, 40, 40, 64);
        // Every chunk one layer below the camera is solid rock (no sides connected).
        let vis = |k: ChunkKey| if k.ck == 63 { 0 } else { crate::mesher::VIS_ALL };
        let set = reachable_chunks(&grid, start, 4.0, vis);
        assert!(set.contains(&ChunkKey::new(4, 40, 40, 63)), "the rock right below is seen");
        assert!(set.contains(&ChunkKey::new(4, 41, 40, 63)), "and the rock beside it");
        assert!(!set.contains(&ChunkKey::new(4, 40, 40, 62)), "caves under the rock are hidden");
        assert!(set.contains(&ChunkKey::new(4, 40, 40, 66)), "the air above is open");
        let all = reachable_chunks(&grid, start, 4.0, |_| crate::mesher::VIS_ALL);
        assert!(all.contains(&ChunkKey::new(4, 40, 40, 62)), "without rock everything is reachable");
    }

    #[test]
    fn frustum_culls_behind_and_far_sideways() {
        let proj = Mat4::perspective_rh(1.2, 1.6, 0.1, 500.0);
        let view = Mat4::look_to_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);
        let planes = frustum_planes(&(proj * view));
        assert!(aabb_in_frustum(&planes, Vec3::new(-8.0, -8.0, -40.0), Vec3::new(8.0, 8.0, -24.0)));
        assert!(!aabb_in_frustum(&planes, Vec3::new(-8.0, -8.0, 24.0), Vec3::new(8.0, 8.0, 40.0)), "behind the camera");
        assert!(!aabb_in_frustum(&planes, Vec3::new(300.0, 0.0, -20.0), Vec3::new(316.0, 16.0, -4.0)), "far to the side");
        assert!(!aabb_in_frustum(&planes, Vec3::new(-8.0, -8.0, -900.0), Vec3::new(8.0, 8.0, -884.0)), "beyond far plane");
        assert!(aabb_in_frustum(&planes, Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)), "camera inside the box");
    }

    #[test]
    fn globals_layout_matches_wgsl() {
        assert_eq!(std::mem::size_of::<Globals>(), 64 + 16 * 6 + 16 * MAX_TILES);
        assert_eq!(std::mem::size_of::<Vertex>(), 32);
    }

    #[test]
    fn world_shader_is_valid_wgsl() {
        let src = include_str!("world.wgsl");
        let module = naga::front::wgsl::parse_str(src).expect("WGSL parse error");
        let mut v = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all());
        v.validate(&module).expect("WGSL validation error");
        let names: Vec<_> = module.entry_points.iter().map(|e| e.name.as_str()).collect();
        for want in ["vs_main", "fs_opaque", "fs_water", "vs_line", "fs_line", "vs_crack", "fs_crack", "vs_model", "fs_model"] {
            assert!(names.contains(&want), "missing entry point {want}");
        }
    }

    #[test]
    fn sky_gets_darker_at_night() {
        let d = sky_color(1.0, 0.0, 0.0);
        let n = sky_color(0.0, 0.0, 0.0);
        assert!(d[2] > n[2] * 5.0);
    }
}

/// Normal alpha blending for colour, but the target keeps alpha = 1: the phone compositor
/// treats a pixel with alpha < 1 as see-through.
const KEEP_ALPHA_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
    alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
};
