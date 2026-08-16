// ═══════════════════════════════════════════════════════════
// Temel Render Pipeline (B-basic) — LUT + FRC + Scale
// ═══════════════════════════════════════════════════════════
//
// openani.me'nin temel görüntü zincirini native wgpu ile kopyalar:
//
//   video karesi (RGBA8) ──► [scale pass] ──► [display pass: LUT + FRC] ──► yüzey
//                            (yalnızca boyutlar farklıysa)
//
// Site 5 shader kullanır; biz yalnızca texture_2d varyantlarını alırız
// (DISPLAY_SHADER = #0, SCALE_SHADER = #3). texture_external varyantları
// (#1/#2/#4) native wgpu'da yoktur — kareyi biz texture_2d'ye yazarız.
//
// NOT (WGSL → Rust düzen eşleşmesi): uniform struct'lar WGSL'deki ile BİREBİR
// aynı bellek düzeninde olmalı. `#[repr(C)]` + bytemuck::Pod bunu garanti eder.
//   FrcUniforms  { frameCount: u32, frcEnabled: u32 }        → 8 bayt
//   ScaleUniforms{ sourceSize: vec2<f32>, outputSize: vec2<f32> } → 16 bayt

use std::mem;

use wgpu::util::DeviceExt;

/// Fullscreen quad köşesi: konum (vec4) + uv (vec2).
/// Hem DISPLAY hem SCALE shader'ın vertex girişiyle aynı düzen.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 4],
    uv: [f32; 2],
}

/// WGSL `FrcUniforms` ile birebir (frame count + FRC açık mı).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FrcUniforms {
    pub frame_count: u32,
    pub frc_enabled: u32,
}

/// WGSL `ScaleUniforms` ile birebir (kaynak/çıktı çözünürlüğü).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ScaleUniforms {
    pub source_size: [f32; 2],
    pub output_size: [f32; 2],
}

// NDC'de ekranı kaplayan iki üçgen (6 köşe). uv köşesi (0,0)→(1,1);
// shader'lar Y eksenini kendileri çevirir (texelCoords.y = 1.0 - y).
// DİKKAT: karenin üst/alt yönelimi, çözücünün verdiği bayt sırasına bağlıdır;
// görüntü ters çıkarsa buradaki uv değerlerini (ya da kareyi) dikey çevir.
const VERTICES: [Vertex; 6] = [
    Vertex { position: [-1.0, -1.0, 0.0, 1.0], uv: [0.0, 1.0] },
    Vertex { position: [1.0, -1.0, 0.0, 1.0], uv: [1.0, 1.0] },
    Vertex { position: [-1.0, 1.0, 0.0, 1.0], uv: [0.0, 0.0] },
    Vertex { position: [-1.0, 1.0, 0.0, 1.0], uv: [0.0, 0.0] },
    Vertex { position: [1.0, -1.0, 0.0, 1.0], uv: [1.0, 1.0] },
    Vertex { position: [1.0, 1.0, 0.0, 1.0], uv: [1.0, 0.0] },
];

/// 512×512 LUT = 64³ 3D renk tablosu (8×8 karo, her biri 64×64).
pub const LUT_SIZE: u32 = 512;
/// rgba16float olduğu için texel başına 4×2 bayt.
pub const LUT_BYTES: usize = (LUT_SIZE as usize) * (LUT_SIZE as usize) * 4 * 2;

fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x2,
            },
        ],
    }
}

/// Kimlik (identity) LUT'u üretir: renk derecesi olmadan video birebir geçer.
/// Gerçek LUT (sitenin renk tonu) yakalanana kadar bu kullanılır.
/// Dönüş: rgba16float, 512×512, f16 (little-endian), satır-major baytlar.
///
/// LUT düzeni (shader'ın decode mantığıyla birebir): 512×512 = 8×8 karo,
/// her karo 64×64. Karo (tx,ty) mavi seviyesini `b = ty*8 + tx` kodlar;
/// karo içi (x%64, y%64) = (r, g). Kimlik → değer = (r/63, g/63, b/63, 1).
pub fn generate_identity_lut() -> Vec<u8> {
    use half::f16;
    let mut out = Vec::with_capacity(LUT_BYTES);
    for y in 0..LUT_SIZE {
        let ty = y / 64; // karo satırı
        let g = y % 64;
        for x in 0..LUT_SIZE {
            let tx = x / 64; // karo sütunu
            let r = x % 64;
            let b = ty * 8 + tx;
            let px = [
                f16::from_f32(r as f32 / 63.0),
                f16::from_f32(g as f32 / 63.0),
                f16::from_f32(b as f32 / 63.0),
                f16::from_f32(1.0),
            ];
            for c in px {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
    }
    out
}

pub struct WebGpuPlayer {
    vertex_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    lut_texture: wgpu::Texture,
    frame_texture: wgpu::Texture,
    frame_view: wgpu::TextureView,
    frc_buffer: wgpu::Buffer,
    scale_buffer: wgpu::Buffer,
    display_pipeline: wgpu::RenderPipeline,
    display_bind_group: wgpu::BindGroup,
    scale_pipeline: wgpu::RenderPipeline,
    scale_bind_group: wgpu::BindGroup,
    scale_target: Option<wgpu::Texture>,
    scale_target_view: Option<wgpu::TextureView>,
    frame_size: [u32; 2],
    output_size: [u32; 2],
    needs_scale: bool,
}

impl WebGpuPlayer {
    /// Pipeline'ı kurar. `lut_rgba16f` boşsa kimlik LUT üretilir.
    /// `frame_size` video çözünürlüğü, `output_size` yüzey çözünürlüğü.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        lut_rgba16f: Option<&[u8]>,
        frame_size: [u32; 2],
        output_size: [u32; 2],
    ) -> Self {
        let needs_scale = frame_size != output_size;

        // ── Vertex buffer ──
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("oa_quad_vertex_buffer"),
            contents: bytemuck::cast_slice(&VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // ── Sampler (linear, clamp) ──
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("oa_linear_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // ── LUT dokusu (rgba16float) ──
        let lut_extent = wgpu::Extent3d {
            width: LUT_SIZE,
            height: LUT_SIZE,
            depth_or_array_layers: 1,
        };
        let lut_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("oa_lut"),
            size: lut_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let lut_view = lut_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let lut_bytes = match lut_rgba16f {
            Some(b) if b.len() == LUT_BYTES => b.to_vec(),
            _ => generate_identity_lut(),
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &lut_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &lut_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(LUT_SIZE * 4 * 2), // 4 bileşen × 2 bayt (f16)
                rows_per_image: None,
            },
            lut_extent,
        );

        // ── Kare dokusu (RGBA8) ──
        let frame_extent = wgpu::Extent3d {
            width: frame_size[0],
            height: frame_size[1],
            depth_or_array_layers: 1,
        };
        let frame_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("oa_frame"),
            size: frame_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let frame_view = frame_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // ── Uniform buffer'lar ──
        let frc_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("oa_frc_uniform"),
            size: mem::size_of::<FrcUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scale_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("oa_scale_uniform"),
            size: mem::size_of::<ScaleUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Shader modülleri ──
        let display_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("oa_display_shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                crate::native_player::shaders::DISPLAY_SHADER,
            )),
        });
        let scale_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("oa_scale_shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                crate::native_player::shaders::SCALE_SHADER,
            )),
        });

        // ── Bind group layout'ları (WGSL binding'leriyle birebir) ──
        // display: 0=sampler, 1=imageTexture, 2=frc(uniform), 3=filterImageTexture(LUT)
        let display_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("oa_display_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        // scale: 0=sampler, 1=sourceTexture, 2=scaleInfo(uniform)
        let scale_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("oa_scale_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // ── Ölçek hedefi (yalnızca gerekiyorsa) ──
        let (scale_target, scale_target_view) = if needs_scale {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("oa_scale_target"),
                size: wgpu::Extent3d {
                    width: output_size[0],
                    height: output_size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            (Some(tex), Some(view))
        } else {
            (None, None)
        };

        // display pass'in örneklediği kaynak: ölçek yoksa kare, varsa ölçek hedefi.
        let display_source = if needs_scale {
            scale_target_view.as_ref().unwrap()
        } else {
            &frame_view
        };

        // ── Pipeline layout'ları ──
        let display_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("oa_display_layout"),
            bind_group_layouts: &[&display_bgl],
            push_constant_ranges: &[],
        });
        let scale_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("oa_scale_layout"),
            bind_group_layouts: &[&scale_bgl],
            push_constant_ranges: &[],
        });

        // ── Render pipeline'lar ──
        let display_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("oa_display_pipeline"),
            layout: Some(&display_layout),
            vertex: wgpu::VertexState {
                module: &display_module,
                entry_point: Some("vertex_main"),
                compilation_options: Default::default(),
                buffers: &[vertex_buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &display_module,
                entry_point: Some("fragment_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // scale pass hedefi = ölçek hedefi (RGBA8) ya da yüzey (ölçek yokken kullanılmaz).
        let scale_target_format = if needs_scale {
            wgpu::TextureFormat::Rgba8Unorm
        } else {
            surface_format
        };
        let scale_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("oa_scale_pipeline"),
            layout: Some(&scale_layout),
            vertex: wgpu::VertexState {
                module: &scale_module,
                entry_point: Some("vertex_main"),
                compilation_options: Default::default(),
                buffers: &[vertex_buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &scale_module,
                entry_point: Some("fragment_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: scale_target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // ── Bind group'lar ──
        let display_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("oa_display_bind_group"),
            layout: &display_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(display_source),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: frc_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&lut_view),
                },
            ],
        });

        let scale_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("oa_scale_bind_group"),
            layout: &scale_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&frame_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: scale_buffer.as_entire_binding(),
                },
            ],
        });

        // scale uniform'ı bir kez doldur (boyutlar sabit).
        queue.write_buffer(
            &scale_buffer,
            0,
            bytemuck::cast_slice(&[ScaleUniforms {
                source_size: [frame_size[0] as f32, frame_size[1] as f32],
                output_size: [output_size[0] as f32, output_size[1] as f32],
            }]),
        );

        Self {
            vertex_buffer,
            sampler,
            lut_texture,
            frame_texture,
            frame_view,
            frc_buffer,
            scale_buffer,
            display_pipeline,
            display_bind_group,
            scale_pipeline,
            scale_bind_group,
            scale_target,
            scale_target_view,
            frame_size,
            output_size,
            needs_scale,
        }
    }

    /// Bir kareyi yükleyip çizer. `frame_bytes` RGBA8, `frame_size[0]*frame_size[1]*4`
    /// bayt olmalı. `frame_count` FRC dithering'in kare-alternatif fazı için kullanılır.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_view: &wgpu::TextureView,
        frame_bytes: &[u8],
        frame_count: u32,
        frc_enabled: bool,
    ) -> Result<(), String> {
        let expected = (self.frame_size[0] * self.frame_size[1] * 4) as usize;
        if frame_bytes.len() != expected {
            return Err(format!(
                "kare boyutu uyuşmuyor: {} bayt bekleniyor, {} geldi",
                expected,
                frame_bytes.len()
            ));
        }

        // Kareyi texture'a yükle.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.frame_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            frame_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.frame_size[0] * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: self.frame_size[0],
                height: self.frame_size[1],
                depth_or_array_layers: 1,
            },
        );

        // FRC uniform'ı güncelle.
        queue.write_buffer(
            &self.frc_buffer,
            0,
            bytemuck::cast_slice(&[FrcUniforms {
                frame_count,
                frc_enabled: if frc_enabled { 1 } else { 0 },
            }]),
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("oa_render_encoder"),
        });

        // 1) Ölçek pass'i (gerekiyorsa): kare → ölçek hedefi.
        if self.needs_scale {
            let target = self.scale_target_view.as_ref().unwrap();
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("oa_scale_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&self.scale_pipeline);
                pass.set_bind_group(0, &self.scale_bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                pass.draw(0..6, 0..1);
            }
        }

        // 2) Display pass'i: kaynak → yüzey (LUT + FRC).
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("oa_display_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.display_pipeline);
            pass.set_bind_group(0, &self.display_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..6, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }
}
