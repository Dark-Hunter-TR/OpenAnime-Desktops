#[cfg(target_os = "linux")]
pub mod inner {
    use std::sync::{Arc, Mutex};
    use std::thread;
    use tauri::{Manager, WebviewWindow, WebviewWindowBuilder, WebviewUrl};
    // use wgpu::util::DeviceExt;
    use crate::video_decode::inner::GstPlayer;

    pub struct RenderState {
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        render_pipeline: wgpu::RenderPipeline,
        sampler: wgpu::Sampler,
        
        texture: Option<wgpu::Texture>,
        bind_group: Option<wgpu::BindGroup>,
        bind_group_layout: wgpu::BindGroupLayout,
        
        width: u32,
        height: u32,
    }

    impl RenderState {
        pub async fn new(window: WebviewWindow) -> Result<Self, String> {
            let size = window.inner_size().map_err(|e| e.to_string())?;
            let width = size.width.max(1);
            let height = size.height.max(1);

            // Create wgpu instance
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::VULKAN,
                ..Default::default()
            });

            // Create surface on Tauri window
            // Since WebviewWindow implements HasWindowHandle and HasDisplayHandle, we cast it to static lifetime
            let surface = unsafe {
                let s = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
                std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(s)
            };

            // Request adapter
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .ok_or("Failed to find a compatible Vulkan adapter")?;

            // Request device and queue
            let (device, queue) = adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("Tauri Video Renderer Device"),
                        required_features: wgpu::Features::empty(),
                        required_limits: wgpu::Limits::default(),
                        memory_hints: Default::default(),
                    },
                    None,
                )
                .await
                .map_err(|e| format!("Failed to create wgpu device: {}", e))?;

            // Get surface capabilities
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .copied()
                .find(|f| f.is_srgb())
                .unwrap_or(surface_caps.formats[0]);

            // Optimization: Query and use low-latency mailbox (triple-buffering) present mode if supported
            let present_mode = if surface_caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
                wgpu::PresentMode::Mailbox
            } else {
                wgpu::PresentMode::Fifo
            };

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width,
                height,
                present_mode,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            // Compile shaders
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Video Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/video.wgsl").into()),
            });

            // Set up bind group layout for texture
            let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("Texture Bind Group Layout"),
            });

            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });

            // Create pipeline layout
            let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            // Create render pipeline
            let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            });

            Ok(Self {
                surface,
                device,
                queue,
                config,
                render_pipeline,
                sampler,
                texture: None,
                bind_group: None,
                bind_group_layout,
                width,
                height,
            })
        }

        pub fn resize(&mut self, new_width: u32, new_height: u32) {
            if new_width > 0 && new_height > 0 {
                self.width = new_width;
                self.height = new_height;
                self.config.width = new_width;
                self.config.height = new_height;
                self.surface.configure(&self.device, &self.config);
            }
        }

        pub fn update_video_texture(&mut self, frame_width: u32, frame_height: u32, rgba_data: &[u8]) {
            let recreate_texture = match &self.texture {
                Some(tex) => tex.width() != frame_width || tex.height() != frame_height,
                None => true,
            };

            if recreate_texture {
                // Initialize wgpu texture with video dimensions
                let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    size: wgpu::Extent3d {
                        width: frame_width,
                        height: frame_height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    label: Some("Video Frame Texture"),
                    view_formats: &[],
                });

                let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

                // Build new bind group mapping the texture view and sampler
                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                    label: Some("Video Bind Group"),
                });

                self.texture = Some(texture);
                self.bind_group = Some(bind_group);
            }

            // Upload RGBA frame to texture
            if let Some(texture) = &self.texture {
                self.queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    rgba_data,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(frame_width * 4),
                        rows_per_image: Some(frame_height),
                    },
                    wgpu::Extent3d {
                        width: frame_width,
                        height: frame_height,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }

        pub fn prepare_and_submit(&mut self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
            let output = self.surface.get_current_texture()?;
            let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.render_pipeline);
                if let Some(bind_group) = &self.bind_group {
                    render_pass.set_bind_group(0, bind_group, &[]);
                    render_pass.draw(0..6, 0..1); // 6 vertices for fullscreen quad
                }
            }

            self.queue.submit(std::iter::once(encoder.finish()));
            Ok(output)
        }
    }

    // Shared global state for managing the player instance and rendering loop
    pub struct NativePlayerManager {
        player: Option<GstPlayer>,
        render_state: Option<Arc<Mutex<RenderState>>>,
        overlay_window: Option<WebviewWindow>,
    }

    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU32, Ordering};
    static MANAGER: OnceLock<Mutex<NativePlayerManager>> = OnceLock::new();
    static CONSECUTIVE_LOCK_FAILURES: AtomicU32 = AtomicU32::new(0);

    pub fn get_manager() -> &'static Mutex<NativePlayerManager> {
        MANAGER.get_or_init(|| Mutex::new(NativePlayerManager {
            player: None,
            render_state: None,
            overlay_window: None,
        }))
    }

    pub fn start_player(url: &str, main_window: WebviewWindow) -> Result<(), String> {
        let app = main_window.app_handle();
        let mut manager = get_manager().lock().unwrap();

        // 1. Destroy existing overlay if active
        if let Some(win) = manager.overlay_window.take() {
            let _ = win.close();
        }
        manager.player = None;
        manager.render_state = None;

        // 2. Spawn transparent, borderless overlay window
        let overlay = WebviewWindowBuilder::new(
            app,
            "gst_overlay",
            WebviewUrl::default(),
        )
        .title("Video Overlay")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .build()
        .map_err(|e| format!("Failed to create overlay window: {}", e))?;

        manager.overlay_window = Some(overlay.clone());

        // 3. Initialize GStreamer player
        let player = GstPlayer::new(url, main_window.clone())?;

        // 4. Initialize WGPU state on overlay window
        let render_state = tauri::async_runtime::block_on(RenderState::new(overlay.clone()))?;
        let render_state_shared = Arc::new(Mutex::new(render_state));
        manager.render_state = Some(render_state_shared.clone());
        manager.player = Some(player);

        // 5. Spawn background render thread with Condvar synchronization
        let frame_signal = manager.player.as_ref().unwrap().get_frame_signal();
        let rs_ref = render_state_shared.clone();

        thread::spawn(move || {
            loop {
                // Wait for the next decoded frame using condition variable
                let frame = {
                    let mut guard = frame_signal.frame.lock().unwrap();
                    loop {
                        {
                            let running = frame_signal.is_running.lock().unwrap();
                            if !*running {
                                return;
                            }
                        }
                        if guard.is_some() {
                            break;
                        }
                        guard = frame_signal.condvar.wait(guard).unwrap();
                    }
                    guard.take().unwrap()
                };

                // Render the frame immediately
                let presentation_result = {
                    let mut rs = rs_ref.lock().unwrap();
                    rs.update_video_texture(frame.width, frame.height, &frame.data);
                    rs.prepare_and_submit()
                };

                if let Ok(output) = presentation_result {
                    output.present();
                }
            }
        });

        Ok(())
    }

    pub fn stop_player() {
        let mut manager = get_manager().lock().unwrap();
        if let Some(win) = manager.overlay_window.take() {
            let _ = win.close();
        }
        manager.player = None;
        manager.render_state = None;
        println!("[Native Render] Native player stopped and overlay closed.");
    }

    pub fn sync_bounds(x: i32, y: i32, width: u32, height: u32, main_window: WebviewWindow) {
        let manager_guard = match get_manager().try_lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        let overlay_opt = manager_guard.overlay_window.clone();
        let rs_shared_opt = manager_guard.render_state.clone();
        drop(manager_guard);

        if let Some(overlay) = overlay_opt {
            let scale_factor = main_window.scale_factor().unwrap_or(1.0);
            
            // Convert bounds relative to client area to physical screen coordinates
            let physical_pos = tauri::PhysicalPosition::new(
                (x as f64 * scale_factor) as i32,
                (y as f64 * scale_factor) as i32,
            );
            let physical_size = tauri::PhysicalSize::new(
                (width as f64 * scale_factor) as u32,
                (height as f64 * scale_factor) as u32,
            );

            let _ = overlay.set_position(tauri::Position::Physical(physical_pos));
            let _ = overlay.set_size(tauri::Size::Physical(physical_size));

            // Resize wgpu surface configuration
            if let Some(rs_shared) = rs_shared_opt {
                if let Ok(mut rs) = rs_shared.try_lock() {
                    CONSECUTIVE_LOCK_FAILURES.store(0, Ordering::Relaxed);
                    rs.resize(physical_size.width, physical_size.height);
                } else {
                    let failures = CONSECUTIVE_LOCK_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
                    if failures % 10 == 0 {
                        eprintln!("[Native Render] Lock contention detected ({} failures), resize skipped.", failures);
                    }
                }
            }
        }
    }

    pub fn control_play() -> Result<(), String> {
        let manager = get_manager().lock().unwrap();
        if let Some(player) = &manager.player {
            player.play()?;
        }
        Ok(())
    }

    pub fn control_pause() -> Result<(), String> {
        let manager = get_manager().lock().unwrap();
        if let Some(player) = &manager.player {
            player.pause()?;
        }
        Ok(())
    }

    pub fn control_seek(time: f64) -> Result<(), String> {
        let manager = get_manager().lock().unwrap();
        if let Some(player) = &manager.player {
            player.seek(time)?;
        }
        Ok(())
    }
}
