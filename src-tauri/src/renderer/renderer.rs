use std::sync::Arc;
use wgpu::{Adapter, Device, Queue, Sampler, SamplerDescriptor, AddressMode, FilterMode, TextureFormat, TextureUsages, CommandEncoderDescriptor};
use tauri::Window;

use super::adapter::select_adapter;
use super::device::create_device_and_queue;
use super::surface::SurfaceManager;
use super::texture::GpuTexture;
use super::upload::TextureUploader;
use super::compute::VideoComputePipeline;
use super::present::Presenter;
use super::cache::ResourceCache;
use super::shader::ShaderSystem;
use super::pipeline::PipelineBuilder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpscaleMode {
    Bicubic,
    Lanczos,
    Anime,
}

pub struct WebGpuRenderer {
    adapter: Arc<Adapter>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    surface_manager: SurfaceManager,
    
    // Core compute and rendering pipelines
    compute_pipeline: VideoComputePipeline,
    presenter: Presenter,
    
    // Resource cache (pipelines, bind groups, textures)
    cache: ResourceCache,
    
    // Sampling state
    sampler: Sampler,
    
    // Video textures
    input_texture: Option<Arc<GpuTexture>>,
    has_previous_frame: bool,
    
    // Configurable pipeline switches
    pub upscale_mode: UpscaleMode,
    pub enable_denoise: bool,
    pub enable_edge_enhancement: bool,
    pub enable_sharpen: bool,
    pub enable_frame_gen: bool,
}

impl WebGpuRenderer {
    pub async fn new(window: Window, vsync: bool) -> Result<Self, String> {
        // Uygulama geneli paylaşılan instance (&'static) — her player
        // başlatmada yeni instance kurmak bozuk EGL'de tekrarlanan panic
        // maliyeti demekti.
        let instance: &wgpu::Instance = crate::gpu::shared_instance();

        // Select compatible Vulkan adapter
        let adapter = select_adapter(&instance).await?;

        // Request WGPU Device and Queue
        let (device, queue) = create_device_and_queue(&adapter).await?;

        // Set up uncaptured error handler for better debuggability and stability
        device.on_uncaptured_error(Box::new(|err| {
            eprintln!("[WebGPU Renderer] Uncaptured WGPU error: {:?}", err);
        }));

        // Initialize Swapchain Surface
        let surface_manager = SurfaceManager::new(&instance, &adapter, &device, &window, vsync)?;

        // Create compute pipeline executor
        let compute_pipeline = VideoComputePipeline::new(device.clone());

        // Create fragment presentation pass executor
        let presenter = Presenter::new(device.clone());

        // Create global linear sampler for textures mapping
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Global Linear Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            adapter,
            device,
            queue,
            surface_manager,
            compute_pipeline,
            presenter,
            cache: ResourceCache::new(),
            sampler,
            input_texture: None,
            has_previous_frame: false,
            upscale_mode: UpscaleMode::Anime, // Anime mode default for OpenAnime
            enable_denoise: true,
            enable_edge_enhancement: true,
            enable_sharpen: true,
            enable_frame_gen: true, // Enable full interpolation
        })
    }

    /// Resizes the surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface_manager.resize(&self.device, &self.adapter, width, height);
    }

    /// Uploads CPU frame data to the input texture. Reallocates input texture on dimensions change.
    pub fn update_video_texture(&mut self, frame_width: u32, frame_height: u32, rgba_data: &[u8]) {
        let recreate = match &self.input_texture {
            Some(tex) => tex.width != frame_width || tex.height != frame_height,
            None => true,
        };

        if recreate {
            let input_tex = GpuTexture::new(
                &self.device,
                frame_width,
                frame_height,
                TextureFormat::Rgba8Unorm,
                TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC,
                Some("Video Input Texture"),
            );
            self.input_texture = Some(Arc::new(input_tex));
            self.cache.clear_bind_groups();
            self.has_previous_frame = false; // Reset history on size changes
        }

        if let Some(ref tex) = self.input_texture {
            TextureUploader::upload_rgba(&self.queue, tex, rgba_data);
        }
    }

    /// Performs the compute pipeline processing (Upscale, Edge, Sharpen, Denoise)
    /// and presents the output to the surface.
    pub fn prepare_and_submit(&mut self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        let output = match self.surface_manager.get_current_texture() {
            Ok(tex) => tex,
            Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                eprintln!("[WebGPU Renderer] Surface outdated or lost, reconfiguring surface...");
                let w = self.surface_manager.width();
                let h = self.surface_manager.height();
                self.surface_manager.resize(&self.device, &self.adapter, w, h);
                // Retry once after reconfiguring
                self.surface_manager.get_current_texture()?
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                eprintln!("[WebGPU Renderer] Out of memory error! Graceful fallback recommended.");
                return Err(wgpu::SurfaceError::OutOfMemory);
            }
            Err(e) => {
                return Err(e);
            }
        };
        let output_view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Video Render Command Encoder"),
        });

        if let Some(ref input_tex) = self.input_texture {
            let in_w = input_tex.width();
            let in_h = input_tex.height();
            let out_w = self.surface_manager.width();
            let out_h = self.surface_manager.height();

            // 1. Denoise stage (Input size)
            let denoise_tex = if self.enable_denoise {
                let dst = self.cache.get_texture(
                    &self.device,
                    "denoise_output",
                    in_w,
                    in_h,
                    TextureFormat::Rgba8Unorm,
                    TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
                );

                self.compute_pipeline.dispatch_stage(
                    &mut encoder,
                    &mut self.cache,
                    "denoise",
                    |dev| {
                        let shader = ShaderSystem::compile_denoise(dev);
                        PipelineBuilder::build_compute_pipeline(
                            dev,
                            &shader,
                            &[&self.compute_pipeline.layout_single_io],
                            Some("Denoise Pipeline"),
                        )
                    },
                    input_tex,
                    &self.sampler,
                    &dst,
                );
                dst
            } else {
                input_tex.clone()
            };

            // 2. Upscale stage (Input size -> Output size)
            let upscale_tex = self.cache.get_texture(
                &self.device,
                "upscale_output",
                out_w,
                out_h,
                TextureFormat::Rgba8Unorm,
                TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            );

            match self.upscale_mode {
                UpscaleMode::Bicubic => {
                    self.compute_pipeline.dispatch_stage(
                        &mut encoder,
                        &mut self.cache,
                        "bicubic",
                        |dev| {
                            let shader = ShaderSystem::compile_bicubic(dev);
                            PipelineBuilder::build_compute_pipeline(
                                dev,
                                &shader,
                                &[&self.compute_pipeline.layout_single_io],
                                Some("Bicubic Pipeline"),
                            )
                        },
                        &denoise_tex,
                        &self.sampler,
                        &upscale_tex,
                    );
                }
                UpscaleMode::Lanczos => {
                    self.compute_pipeline.dispatch_stage(
                        &mut encoder,
                        &mut self.cache,
                        "lanczos",
                        |dev| {
                            let shader = ShaderSystem::compile_lanczos(dev);
                            PipelineBuilder::build_compute_pipeline(
                                dev,
                                &shader,
                                &[&self.compute_pipeline.layout_single_io],
                                Some("Lanczos Pipeline"),
                            )
                        },
                        &denoise_tex,
                        &self.sampler,
                        &upscale_tex,
                    );
                }
                UpscaleMode::Anime => {
                    self.compute_pipeline.dispatch_stage(
                        &mut encoder,
                        &mut self.cache,
                        "anime_upscale",
                        |dev| {
                            let shader = ShaderSystem::compile_anime_upscale(dev);
                            PipelineBuilder::build_compute_pipeline(
                                dev,
                                &shader,
                                &[&self.compute_pipeline.layout_single_io],
                                Some("Anime Pipeline"),
                            )
                        },
                        &denoise_tex,
                        &self.sampler,
                        &upscale_tex,
                    );
                }
            }

            // 3. Edge Enhancement (Output size)
            let edge_tex = if self.enable_edge_enhancement {
                let dst = self.cache.get_texture(
                    &self.device,
                    "edge_output",
                    out_w,
                    out_h,
                    TextureFormat::Rgba8Unorm,
                    TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
                );

                self.compute_pipeline.dispatch_stage(
                    &mut encoder,
                    &mut self.cache,
                    "edge_enhancement",
                    |dev| {
                        let shader = ShaderSystem::compile_edge_enhancement(dev);
                        PipelineBuilder::build_compute_pipeline(
                            dev,
                            &shader,
                            &[&self.compute_pipeline.layout_single_io],
                            Some("Edge Enhancement Pipeline"),
                        )
                    },
                    &upscale_tex,
                    &self.sampler,
                    &dst,
                );
                dst
            } else {
                upscale_tex.clone()
            };

            // 4. Sharpen (Output size)
            let sharpen_tex = if self.enable_sharpen {
                let dst = self.cache.get_texture(
                    &self.device,
                    "sharpen_output",
                    out_w,
                    out_h,
                    TextureFormat::Rgba8Unorm,
                    TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
                );

                self.compute_pipeline.dispatch_stage(
                    &mut encoder,
                    &mut self.cache,
                    "sharpen",
                    |dev| {
                        let shader = ShaderSystem::compile_sharpen(dev);
                        PipelineBuilder::build_compute_pipeline(
                            dev,
                            &shader,
                            &[&self.compute_pipeline.layout_single_io],
                            Some("Sharpen Pipeline"),
                        )
                    },
                    &edge_tex,
                    &self.sampler,
                    &dst,
                );
                dst
            } else {
                edge_tex.clone()
            };

            // 5. Frame Interpolation / Generation Stage
            let final_render_tex = if self.enable_frame_gen {
                // Get or allocate previous frame container
                let prev_tex = self.cache.get_texture(
                    &self.device,
                    "previous_frame",
                    out_w,
                    out_h,
                    TextureFormat::Rgba8Unorm,
                    TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                );

                if self.has_previous_frame {
                    // Compute motion vectors
                    let mv_tex = self.cache.get_texture(
                        &self.device,
                        "motion_vectors",
                        out_w,
                        out_h,
                        TextureFormat::Rg16Float,
                        TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
                    );

                    self.compute_pipeline.dispatch_motion_vector(
                        &mut encoder,
                        &mut self.cache,
                        &sharpen_tex,
                        &prev_tex,
                        &mv_tex,
                    );

                    // Interpolate frame
                    let gen_tex = self.cache.get_texture(
                        &self.device,
                        "frame_gen_output",
                        out_w,
                        out_h,
                        TextureFormat::Rgba8Unorm,
                        TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
                    );

                    self.compute_pipeline.dispatch_frame_gen(
                        &mut encoder,
                        &mut self.cache,
                        &sharpen_tex,
                        &prev_tex,
                        &mv_tex,
                        &self.sampler,
                        &gen_tex,
                    );

                    // Copy the final processed frame to "previous_frame" to serve as history for the next frame
                    encoder.copy_texture_to_texture(
                        wgpu::ImageCopyTexture {
                            texture: &sharpen_tex.texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::ImageCopyTexture {
                            texture: &prev_tex.texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::Extent3d {
                            width: out_w,
                            height: out_h,
                            depth_or_array_layers: 1,
                        },
                    );

                    gen_tex
                } else {
                    // Seed the previous frame history buffer
                    encoder.copy_texture_to_texture(
                        wgpu::ImageCopyTexture {
                            texture: &sharpen_tex.texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::ImageCopyTexture {
                            texture: &prev_tex.texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::Extent3d {
                            width: out_w,
                            height: out_h,
                            depth_or_array_layers: 1,
                        },
                    );
                    self.has_previous_frame = true;
                    sharpen_tex.clone()
                }
            } else {
                sharpen_tex.clone()
            };

            // 6. Presentation Pass (renders processed frame view onto target surface)
            self.presenter.draw(
                &mut encoder,
                &mut self.cache,
                final_render_tex.view(),
                &self.sampler,
                &output_view,
                self.surface_manager.format(),
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(output)
    }
}
