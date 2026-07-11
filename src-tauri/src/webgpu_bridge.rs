// ═══════════════════════════════════════════════════════════════════════════════
// webgpu_bridge.rs — Linux-only WebGPU-over-IPC bridge
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(target_os = "linux")]
pub mod inner {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use tauri::{Manager, WebviewWindow, WebviewWindowBuilder, WebviewUrl};

    // ─────────────────────────────────────────────────────────────────
    // ID allocation + generic registries
    // ─────────────────────────────────────────────────────────────────

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    fn next_id() -> u64 {
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    #[derive(Default)]
    struct Registries {
        buffers: HashMap<u64, wgpu::Buffer>,
        textures: HashMap<u64, wgpu::Texture>,
        texture_views: HashMap<u64, wgpu::TextureView>,
        samplers: HashMap<u64, wgpu::Sampler>,
        shader_modules: HashMap<u64, wgpu::ShaderModule>,
        bind_group_layouts: HashMap<u64, wgpu::BindGroupLayout>,
        pipeline_layouts: HashMap<u64, wgpu::PipelineLayout>,
        bind_groups: HashMap<u64, wgpu::BindGroup>,
        compute_pipelines: HashMap<u64, wgpu::ComputePipeline>,
        render_pipelines: HashMap<u64, wgpu::RenderPipeline>,
        encoders: HashMap<u64, RecordedEncoder>,
        command_buffers: HashMap<u64, wgpu::CommandBuffer>,
        canvas_contexts: HashMap<u64, CanvasContext>,
    }

    struct CanvasContext {
        overlay: WebviewWindow,
        surface: Option<wgpu::Surface<'static>>,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        pending_surface_texture: Option<wgpu::SurfaceTexture>,
    }

    // ─────────────────────────────────────────────────────────────────
    // Recorded command encoder
    // ─────────────────────────────────────────────────────────────────

    enum RecordedOp {
        BeginComputePass,
        SetComputePipeline(u64),
        SetBindGroup { index: u32, bind_group: u64 },
        DispatchWorkgroups { x: u32, y: u32, z: u32 },
        EndComputePass,
        BeginRenderPass { view: u64, clear: Option<[f64; 4]> },
        SetRenderPipeline(u64),
        SetRenderBindGroup { index: u32, bind_group: u64 },
        Draw { vertex_count: u32, instance_count: u32 },
        EndRenderPass,
        CopyBufferToTexture { src: u64, dst_texture: u64, bytes_per_row: u32, width: u32, height: u32 },
        CopyTextureToTexture { src: u64, dst: u64, width: u32, height: u32 },
        WriteTimestamp,
    }

    #[derive(Default)]
    struct RecordedEncoder {
        ops: Vec<RecordedOp>,
    }

    // ─────────────────────────────────────────────────────────────────
    // Bridge state
    // ─────────────────────────────────────────────────────────────────

    pub struct BridgeState {
        instance: wgpu::Instance,
        adapter: Option<Arc<wgpu::Adapter>>,
        device: Option<Arc<wgpu::Device>>,
        queue: Option<Arc<wgpu::Queue>>,
        registries: Registries,
    }

    static BRIDGE: OnceLock<Mutex<BridgeState>> = OnceLock::new();

    fn bridge() -> &'static Mutex<BridgeState> {
        BRIDGE.get_or_init(|| {
            Mutex::new(BridgeState {
                instance: wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::VULKAN,
                    ..Default::default()
                }),
                adapter: None,
                device: None,
                queue: None,
                registries: Registries::default(),
            })
        })
    }

    fn lock() -> std::sync::MutexGuard<'static, BridgeState> {
        bridge().lock().unwrap_or_else(|p| p.into_inner())
    }

    // ─────────────────────────────────────────────────────────────────
    // Adapter / Device
    // ─────────────────────────────────────────────────────────────────

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct AdapterInfo {
        pub id: u64,
        pub name: String,
        pub is_fallback_adapter: bool,
    }

    #[tauri::command]
    pub async fn gpu_request_adapter() -> Result<AdapterInfo, String> {
        let (chosen, id) = {
            let state = lock();
            let adapters = state.instance.enumerate_adapters(wgpu::Backends::VULKAN);
            let chosen = adapters
                .into_iter()
                .max_by_key(|a| match a.get_info().device_type {
                    wgpu::DeviceType::DiscreteGpu => 3,
                    wgpu::DeviceType::IntegratedGpu => 1,
                    _ => 0,
                })
                .ok_or("No Vulkan adapter available".to_string())?;
            let id = next_id();
            (chosen, id)
        };

        let info = chosen.get_info();
        {
            let mut state = lock();
            state.adapter = Some(Arc::new(chosen));
        }

        Ok(AdapterInfo {
            id,
            name: info.name,
            is_fallback_adapter: false,
        })
    }

    #[tauri::command]
    pub async fn gpu_request_device() -> Result<u64, String> {
        let adapter = {
            let state = lock();
            state
                .adapter
                .as_ref()
                .cloned()
                .ok_or("requestDevice() called before requestAdapter()".to_string())?
        };

        // Share the single device/queue instance generated by the renderer module
        let (device, queue) = crate::renderer::device::create_device_and_queue(&adapter).await?;

        let mut state = lock();
        state.device = Some(device);
        state.queue = Some(queue);
        Ok(0)
    }

    fn device() -> Result<Arc<wgpu::Device>, String> {
        lock().device.clone().ok_or("No GPUDevice created yet".to_string())
    }
    fn queue() -> Result<Arc<wgpu::Queue>, String> {
        lock().queue.clone().ok_or("No GPUDevice created yet".to_string())
    }

    // ─────────────────────────────────────────────────────────────────
    // Buffers
    // ─────────────────────────────────────────────────────────────────

    #[tauri::command]
    pub async fn gpu_create_buffer(id: u64, size: u64, usage: u32, mapped_at_creation: bool) -> Result<(), String> {
        let dev = device()?;
        let buf = dev.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::from_bits_truncate(usage),
            mapped_at_creation,
        });
        lock().registries.buffers.insert(id, buf);
        Ok(())
    }

    #[tauri::command]
    pub async fn gpu_write_buffer(buffer_id: u64, offset: u64, data_base64: String) -> Result<(), String> {
        // TODO: Optimize buffer uploads using raw binary channels in a future release if performance requires it.
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data_base64)
            .map_err(|e| e.to_string())?;
        let q = queue()?;
        let state = lock();
        let buf = state
            .registries
            .buffers
            .get(&buffer_id)
            .ok_or("Unknown buffer id")?;
        q.write_buffer(buf, offset, &bytes);
        Ok(())
    }

    #[tauri::command]
    pub async fn gpu_buffer_map_async(buffer_id: u64, mode: u32, offset: u64, size: u64) -> Result<String, String> {
        let (rx, dev) = {
            let state = lock();
            let buf = state
                .registries
                .buffers
                .get(&buffer_id)
                .ok_or("Unknown buffer id")?;

            let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), wgpu::BufferAsyncError>>();
            let map_mode = if mode == 1 { wgpu::MapMode::Read } else { wgpu::MapMode::Write };
            buf.slice(offset..(offset + size)).map_async(map_mode, move |result| {
                let _ = tx.send(result);
            });

            let dev = state.device.clone().ok_or("No device")?;
            (rx, dev)
        }; // Lock state dropped here

        dev.poll(wgpu::Maintain::Wait);

        rx.await
            .map_err(|_| "Channel closed".to_string())?
            .map_err(|e| e.to_string())?;

        let view_b64 = {
            let state = lock();
            let buf = state
                .registries
                .buffers
                .get(&buffer_id)
                .ok_or("Unknown buffer id")?;
            let view = buf.slice(offset..(offset + size)).get_mapped_range();
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&view);
            drop(view);
            b64
        };

        Ok(view_b64)
    }

    #[tauri::command]
    pub async fn gpu_buffer_unmap(buffer_id: u64, data_base64: Option<String>) -> Result<(), String> {
        let state = lock();
        let buf = state
            .registries
            .buffers
            .get(&buffer_id)
            .ok_or("Unknown buffer id")?;

        if let Some(b64) = data_base64 {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| e.to_string())?;
            let mut view = buf.slice(..).get_mapped_range_mut();
            let len = bytes.len().min(view.len());
            view[..len].copy_from_slice(&bytes[..len]);
            drop(view);
        }

        buf.unmap();
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────
    // Textures / Samplers
    // ─────────────────────────────────────────────────────────────────

    #[tauri::command]
    pub async fn gpu_create_texture(
        id: u64,
        width: u32,
        height: u32,
        format: String,
        usage: u32,
    ) -> Result<(), String> {
        let dev = device()?;
        let tex_format = parse_texture_format(&format)?;
        let tex = dev.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: tex_format,
            usage: wgpu::TextureUsages::from_bits_truncate(usage),
            view_formats: &[],
        });
        lock().registries.textures.insert(id, tex);
        Ok(())
    }

    #[tauri::command]
    pub async fn gpu_texture_create_view(id: u64, texture_id: u64) -> Result<(), String> {
        let state = lock();
        let tex = state.registries.textures.get(&texture_id).ok_or("Unknown texture id")?;
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        drop(state);
        lock().registries.texture_views.insert(id, view);
        Ok(())
    }

    #[tauri::command]
    pub async fn gpu_write_texture(
        texture_id: u64,
        width: u32,
        height: u32,
        bytes_per_row: u32,
        data_base64: String,
    ) -> Result<(), String> {
        // Check if native player is active. If so, discard any IPC write texture requests to save CPU/IPC bandwidth
        #[cfg(target_os = "linux")]
        {
            if let Ok(manager) = crate::native_render::inner::get_manager().try_lock() {
                if manager.player.is_some() {
                    return Ok(());
                }
            }
        }

        // TODO: Optimize texture uploads using raw binary channels in a future release if performance requires it.
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data_base64)
            .map_err(|e| e.to_string())?;
        let q = queue()?;
        let state = lock();
        let tex = state.registries.textures.get(&texture_id).ok_or("Unknown texture id")?;
        q.write_texture(
            wgpu::ImageCopyTexture {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        Ok(())
    }

    #[tauri::command]
    pub async fn gpu_create_sampler(id: u64) -> Result<(), String> {
        let dev = device()?;
        let sampler = dev.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        lock().registries.samplers.insert(id, sampler);
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────
    // Shader modules
    // ─────────────────────────────────────────────────────────────────

    #[tauri::command]
    pub async fn gpu_create_shader_module(id: u64, code: String) -> Result<(), String> {
        let dev = device()?;
        let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(code.into()),
        });
        lock().registries.shader_modules.insert(id, module);
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────
    // Bind group layouts / pipeline layouts / bind groups
    // ─────────────────────────────────────────────────────────────────

    #[derive(serde::Deserialize)]
    pub struct BindGroupLayoutEntryDesc {
        pub binding: u32,
        pub visibility: u32,
        pub kind: String,
        pub buffer_type: Option<String>,
        pub sample_type: Option<String>,
        pub storage_format: Option<String>,
    }

    #[tauri::command]
    pub async fn gpu_create_bind_group_layout(id: u64, entries: Vec<BindGroupLayoutEntryDesc>) -> Result<(), String> {
        let dev = device()?;
        let wgpu_entries: Vec<wgpu::BindGroupLayoutEntry> = entries
            .iter()
            .map(|e| {
                let visibility = wgpu::ShaderStages::from_bits_truncate(e.visibility);
                let ty = match e.kind.as_str() {
                    "buffer" => wgpu::BindingType::Buffer {
                        ty: match e.buffer_type.as_deref() {
                            Some("storage") => wgpu::BufferBindingType::Storage { read_only: false },
                            Some("read-only-storage") => wgpu::BufferBindingType::Storage { read_only: true },
                            _ => wgpu::BufferBindingType::Uniform,
                        },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    "sampler" => wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    "texture" => wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: match e.sample_type.as_deref() {
                            Some("unfilterable-float") => wgpu::TextureSampleType::Float { filterable: false },
                            _ => wgpu::TextureSampleType::Float { filterable: true },
                        },
                    },
                    "storage_texture" => wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: parse_texture_format(e.storage_format.as_deref().unwrap_or("rgba8unorm"))
                            .unwrap_or(wgpu::TextureFormat::Rgba8Unorm),
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    other => return Err(format!("Unsupported bind group entry kind: {}", other)),
                };
                Ok(wgpu::BindGroupLayoutEntry {
                    binding: e.binding,
                    visibility,
                    ty,
                    count: None,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let layout = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &wgpu_entries,
        });
        lock().registries.bind_group_layouts.insert(id, layout);
        Ok(())
    }

    #[tauri::command]
    pub async fn gpu_create_pipeline_layout(id: u64, bind_group_layout_ids: Vec<u64>) -> Result<(), String> {
        let dev = device()?;
        let state = lock();
        let layouts: Vec<&wgpu::BindGroupLayout> = bind_group_layout_ids
            .iter()
            .map(|l_id| state.registries.bind_group_layouts.get(l_id).ok_or("Unknown bind group layout id"))
            .collect::<Result<_, _>>()?;
        let pipeline_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &layouts,
            push_constant_ranges: &[],
        });
        drop(state);
        lock().registries.pipeline_layouts.insert(id, pipeline_layout);
        Ok(())
    }

    #[derive(serde::Deserialize)]
    pub struct BindGroupEntryDesc {
        pub binding: u32,
        pub kind: String,
        pub resource_id: u64,
    }

    #[tauri::command]
    pub async fn gpu_create_bind_group(
        id: u64,
        layout_id: u64,
        entries: Vec<BindGroupEntryDesc>,
    ) -> Result<(), String> {
        let dev = device()?;
        let state = lock();
        let layout = state
            .registries
            .bind_group_layouts
            .get(&layout_id)
            .ok_or("Unknown bind group layout id")?;

        let wgpu_entries: Vec<wgpu::BindGroupEntry> = entries
            .iter()
            .map(|e| {
                let resource = match e.kind.as_str() {
                    "buffer" => {
                        let buf = state.registries.buffers.get(&e.resource_id).ok_or("Unknown buffer id")?;
                        wgpu::BindingResource::Buffer(buf.as_entire_buffer_binding())
                    }
                    "sampler" => {
                        let s = state.registries.samplers.get(&e.resource_id).ok_or("Unknown sampler id")?;
                        wgpu::BindingResource::Sampler(s)
                    }
                    "texture_view" => {
                        let v = state
                            .registries
                            .texture_views
                            .get(&e.resource_id)
                            .ok_or("Unknown texture view id")?;
                        wgpu::BindingResource::TextureView(v)
                    }
                    other => return Err(format!("Unsupported bind group resource kind: {}", other)),
                };
                Ok(wgpu::BindGroupEntry { binding: e.binding, resource })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let bind_group = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &wgpu_entries,
        });
        drop(state);
        lock().registries.bind_groups.insert(id, bind_group);
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────
    // Pipelines
    // ─────────────────────────────────────────────────────────────────

    #[tauri::command]
    pub async fn gpu_create_compute_pipeline(
        id: u64,
        pipeline_layout_id: u64,
        shader_module_id: u64,
        entry_point: String,
    ) -> Result<(), String> {
        let dev = device()?;
        let state = lock();
        let layout = state
            .registries
            .pipeline_layouts
            .get(&pipeline_layout_id)
            .ok_or("Unknown pipeline layout id")?;
        let module = state
            .registries
            .shader_modules
            .get(&shader_module_id)
            .ok_or("Unknown shader module id")?;
        let pipeline = dev.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(layout),
            module,
            entry_point: &entry_point,
            compilation_options: Default::default(),
            cache: None,
        });
        drop(state);
        lock().registries.compute_pipelines.insert(id, pipeline);
        Ok(())
    }

    #[tauri::command]
    pub async fn gpu_create_render_pipeline(
        id: u64,
        pipeline_layout_id: u64,
        shader_module_id: u64,
        vs_entry: String,
        fs_entry: String,
        target_format: String,
    ) -> Result<(), String> {
        let dev = device()?;
        let state = lock();
        let layout = state
            .registries
            .pipeline_layouts
            .get(&pipeline_layout_id)
            .ok_or("Unknown pipeline layout id")?;
        let module = state
            .registries
            .shader_modules
            .get(&shader_module_id)
            .ok_or("Unknown shader module id")?;
        let format = parse_texture_format(&target_format)?;

        let pipeline = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: &vs_entry,
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: &fs_entry,
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
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
        drop(state);
        lock().registries.render_pipelines.insert(id, pipeline);
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────
    // Command encoding
    // ─────────────────────────────────────────────────────────────────

    #[tauri::command]
    pub async fn gpu_create_command_encoder(id: u64) -> Result<(), String> {
        lock().registries.encoders.insert(id, RecordedEncoder::default());
        Ok(())
    }

    #[tauri::command]
    pub async fn gpu_encoder_begin_compute_pass(encoder_id: u64) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::BeginComputePass)
    }
    #[tauri::command]
    pub async fn gpu_encoder_set_compute_pipeline(encoder_id: u64, pipeline_id: u64) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::SetComputePipeline(pipeline_id))
    }
    #[tauri::command]
    pub async fn gpu_encoder_set_bind_group(encoder_id: u64, index: u32, bind_group_id: u64) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::SetBindGroup { index, bind_group: bind_group_id })
    }
    #[tauri::command]
    pub async fn gpu_encoder_dispatch_workgroups(encoder_id: u64, x: u32, y: u32, z: u32) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::DispatchWorkgroups { x, y, z })
    }
    #[tauri::command]
    pub async fn gpu_encoder_end_compute_pass(encoder_id: u64) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::EndComputePass)
    }

    #[tauri::command]
    pub async fn gpu_encoder_begin_render_pass(
        encoder_id: u64,
        view_id: u64,
        clear: Option<[f64; 4]>,
    ) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::BeginRenderPass { view: view_id, clear })
    }
    #[tauri::command]
    pub async fn gpu_encoder_set_render_pipeline(encoder_id: u64, pipeline_id: u64) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::SetRenderPipeline(pipeline_id))
    }
    #[tauri::command]
    pub async fn gpu_encoder_set_render_bind_group(encoder_id: u64, index: u32, bind_group_id: u64) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::SetRenderBindGroup { index, bind_group: bind_group_id })
    }
    #[tauri::command]
    pub async fn gpu_encoder_draw(encoder_id: u64, vertex_count: u32, instance_count: u32) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::Draw { vertex_count, instance_count })
    }
    #[tauri::command]
    pub async fn gpu_encoder_end_render_pass(encoder_id: u64) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::EndRenderPass)
    }

    #[tauri::command]
    pub async fn gpu_encoder_copy_buffer_to_texture(
        encoder_id: u64,
        src: u64,
        dst_texture: u64,
        bytes_per_row: u32,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        push_op(
            encoder_id,
            RecordedOp::CopyBufferToTexture {
                src,
                dst_texture,
                bytes_per_row,
                width,
                height,
            },
        )
    }

    #[tauri::command]
    pub async fn gpu_encoder_copy_texture_to_texture(
        encoder_id: u64,
        src: u64,
        dst: u64,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        push_op(
            encoder_id,
            RecordedOp::CopyTextureToTexture {
                src,
                dst,
                width,
                height,
            },
        )
    }

    fn push_op(encoder_id: u64, op: RecordedOp) -> Result<(), String> {
        let mut state = lock();
        let enc = state
            .registries
            .encoders
            .get_mut(&encoder_id)
            .ok_or("Unknown command encoder id")?;
        enc.ops.push(op);
        Ok(())
    }

    #[tauri::command]
    pub async fn gpu_encoder_finish(id: u64, encoder_id: u64) -> Result<(), String> {
        let dev = device()?;
        let mut state = lock();
        let recorded = state
            .registries
            .encoders
            .remove(&encoder_id)
            .ok_or("Unknown command encoder id")?;

        let mut encoder = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let mut i = 0usize;
        while i < recorded.ops.len() {
            match &recorded.ops[i] {
                RecordedOp::BeginComputePass => {
                    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: None,
                        timestamp_writes: None,
                    });
                    i += 1;
                    while i < recorded.ops.len() {
                        match &recorded.ops[i] {
                            RecordedOp::SetComputePipeline(pid) => {
                                let p = state.registries.compute_pipelines.get(pid).ok_or("Unknown compute pipeline id")?;
                                pass.set_pipeline(p);
                            }
                            RecordedOp::SetBindGroup { index, bind_group } => {
                                let bg = state.registries.bind_groups.get(bind_group).ok_or("Unknown bind group id")?;
                                pass.set_bind_group(*index, bg, &[]);
                            }
                            RecordedOp::DispatchWorkgroups { x, y, z } => {
                                pass.dispatch_workgroups(*x, *y, *z);
                            }
                            RecordedOp::EndComputePass => {
                                i += 1;
                                break;
                            }
                            _ => return Err("Unexpected op inside compute pass".into()),
                        }
                        i += 1;
                    }
                    continue;
                }
                RecordedOp::BeginRenderPass { view, clear } => {
                    let view_ref = state.registries.texture_views.get(view).ok_or("Unknown texture view id")?;
                    let load = match clear {
                        Some([r, g, b, a]) => wgpu::LoadOp::Clear(wgpu::Color { r: *r, g: *g, b: *b, a: *a }),
                        None => wgpu::LoadOp::Load,
                    };
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: view_ref,
                            resolve_target: None,
                            ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });
                    i += 1;
                    while i < recorded.ops.len() {
                        match &recorded.ops[i] {
                            RecordedOp::SetRenderPipeline(pid) => {
                                let p = state.registries.render_pipelines.get(pid).ok_or("Unknown render pipeline id")?;
                                pass.set_pipeline(p);
                            }
                            RecordedOp::SetRenderBindGroup { index, bind_group } => {
                                let bg = state.registries.bind_groups.get(bind_group).ok_or("Unknown bind group id")?;
                                pass.set_bind_group(*index, bg, &[]);
                            }
                            RecordedOp::Draw { vertex_count, instance_count } => {
                                pass.draw(0..*vertex_count, 0..*instance_count);
                            }
                            RecordedOp::EndRenderPass => {
                                i += 1;
                                break;
                            }
                            _ => return Err("Unexpected op inside render pass".into()),
                        }
                        i += 1;
                    }
                    continue;
                }
                RecordedOp::CopyBufferToTexture { src, dst_texture, bytes_per_row, width, height } => {
                    let buf = state.registries.buffers.get(src).ok_or("Unknown buffer id")?;
                    let tex = state.registries.textures.get(dst_texture).ok_or("Unknown texture id")?;
                    encoder.copy_buffer_to_texture(
                        wgpu::ImageCopyBuffer {
                            buffer: buf,
                            layout: wgpu::ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(*bytes_per_row),
                                rows_per_image: Some(*height),
                            },
                        },
                        wgpu::ImageCopyTexture {
                            texture: tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::Extent3d { width: *width, height: *height, depth_or_array_layers: 1 },
                    );
                }
                RecordedOp::CopyTextureToTexture { src, dst, width, height } => {
                    let src_tex = state.registries.textures.get(src).ok_or("Unknown source texture id")?;
                    let dst_tex = state.registries.textures.get(dst).ok_or("Unknown destination texture id")?;
                    encoder.copy_texture_to_texture(
                        wgpu::ImageCopyTexture {
                            texture: src_tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::ImageCopyTexture {
                            texture: dst_tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::Extent3d { width: *width, height: *height, depth_or_array_layers: 1 },
                    );
                }
                RecordedOp::WriteTimestamp => {}
                _ => return Err("Op encountered outside of any pass".into()),
            }
            i += 1;
        }

        let cmd_buf = encoder.finish();
        drop(state);
        lock().registries.command_buffers.insert(id, cmd_buf);
        Ok(())
    }

    #[tauri::command]
    pub async fn gpu_queue_submit(command_buffer_ids: Vec<u64>) -> Result<(), String> {
        let q = queue()?;
        let mut state = lock();
        let buffers: Vec<wgpu::CommandBuffer> = command_buffer_ids
            .iter()
            .map(|id| state.registries.command_buffers.remove(id).ok_or("Unknown command buffer id"))
            .collect::<Result<_, _>>()?;
        drop(state);
        q.submit(buffers);
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────
    // Canvas presentation (overlay)
    // ─────────────────────────────────────────────────────────────────

    #[tauri::command]
    pub async fn gpu_canvas_get_context(
        main_window: WebviewWindow,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<u64, String> {
        let app: tauri::AppHandle = main_window.app_handle().clone();
        let label = format!("gpu_canvas_{}", next_id());

        let (window_tx, window_rx) = tokio::sync::oneshot::channel::<Result<WebviewWindow, String>>();
        let (realize_tx, realize_rx) = tokio::sync::oneshot::channel::<()>();
        let realize_tx = Arc::new(Mutex::new(Some(realize_tx)));

        let app_for_build = app.clone();
        let label_for_build = label.clone();
        app.run_on_main_thread(move || {
            let build_result = WebviewWindowBuilder::new(&app_for_build, label_for_build, WebviewUrl::default())
                .title("GPU Canvas Overlay")
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .position(x as f64, y as f64)
                .inner_size(width.max(1) as f64, height.max(1) as f64)
                .build();

            match build_result {
                Ok(overlay) => {
                    let _ = overlay.set_ignore_cursor_events(true);

                    let realize_tx_for_event = realize_tx.clone();
                    overlay.on_window_event(move |event| {
                        if matches!(event, tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_)) {
                            if let Some(tx) = realize_tx_for_event.lock().unwrap_or_else(|p| p.into_inner()).take() {
                                let _ = tx.send(());
                            }
                        }
                    });
                    let _ = window_tx.send(Ok(overlay));
                }
                Err(e) => {
                    let _ = window_tx.send(Err(format!("Failed to create GPU canvas overlay: {}", e)));
                }
            }
        })
        .map_err(|e| format!("Failed to dispatch overlay creation to main thread: {}", e))?;

        let overlay = window_rx
            .await
            .map_err(|_| "Main thread dropped the overlay window channel".to_string())??;

        match tokio::time::timeout(std::time::Duration::from_millis(500), realize_rx).await {
            Ok(Ok(())) => {}
            _ => {
                let _ = overlay.close();
                return Err("GPU canvas overlay did not realize in time".to_string());
            }
        }

        let ctx_id = next_id();
        lock().registries.canvas_contexts.insert(
            ctx_id,
            CanvasContext {
                overlay,
                surface: None,
                format: wgpu::TextureFormat::Bgra8Unorm,
                width,
                height,
                pending_surface_texture: None,
            },
        );
        Ok(ctx_id)
    }

    #[tauri::command]
    pub async fn gpu_canvas_configure(context_id: u64, format: String) -> Result<(), String> {
        let dev = device()?;
        let tex_format = parse_texture_format(&format)?;

        let (overlay, width, height) = {
            let state = lock();
            let ctx = state.registries.canvas_contexts.get(&context_id).ok_or("Unknown canvas context id")?;
            (ctx.overlay.clone(), ctx.width, ctx.height)
        };

        let adapter = {
            let state = lock();
            state.adapter.clone().ok_or("No adapter selected")?
        };

        let surface = {
            let state = lock();
            state.instance.create_surface(overlay).map_err(|e| e.to_string())?
        };

        let caps = surface.get_capabilities(&adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: tex_format,
            width: width.max(1),
            height: height.max(1),
            present_mode: if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
                wgpu::PresentMode::Mailbox
            } else {
                wgpu::PresentMode::Fifo
            },
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&dev, &config);

        let mut state = lock();
        let ctx = state.registries.canvas_contexts.get_mut(&context_id).ok_or("Unknown canvas context id")?;
        ctx.surface = Some(surface);
        ctx.format = tex_format;
        Ok(())
    }

    #[tauri::command]
    pub async fn gpu_canvas_get_current_texture(context_id: u64) -> Result<u64, String> {
        let mut state = lock();
        let ctx = state.registries.canvas_contexts.get_mut(&context_id).ok_or("Unknown canvas context id")?;
        let surface = ctx.surface.as_ref().ok_or("configure() not called yet")?;
        let output = surface.get_current_texture().map_err(|e| format!("get_current_texture failed: {}", e))?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        ctx.pending_surface_texture = Some(output);
        drop(state);
        let id = next_id();
        lock().registries.texture_views.insert(id, view);
        Ok(id)
    }

    #[tauri::command]
    pub async fn gpu_canvas_present(context_id: u64) -> Result<(), String> {
        let mut state = lock();
        let ctx = state.registries.canvas_contexts.get_mut(&context_id).ok_or("Unknown canvas context id")?;
        if let Some(output) = ctx.pending_surface_texture.take() {
            output.present();
        }
        Ok(())
    }

    #[tauri::command]
    pub async fn gpu_canvas_sync_bounds(context_id: u64, x: i32, y: i32, width: u32, height: u32) -> Result<(), String> {
        let dev = {
            let state = lock();
            state.device.clone()
        };

        let mut state = lock();
        let ctx = state.registries.canvas_contexts.get_mut(&context_id).ok_or("Unknown canvas context id")?;
        let _ = ctx.overlay.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x as f64, y as f64)));
        let _ = ctx.overlay.set_size(tauri::Size::Logical(tauri::LogicalSize::new(width as f64, height as f64)));
        
        ctx.width = width;
        ctx.height = height;

        if let Some(surface) = &ctx.surface {
            if let Some(d) = &dev {
                let config = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: ctx.format,
                    width: width.max(1),
                    height: height.max(1),
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode: wgpu::CompositeAlphaMode::PostMultiplied,
                    view_formats: vec![],
                    desired_maximum_frame_latency: 2,
                };
                surface.configure(d, &config);
            }
        }
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────
    // Helpers
    // ─────────────────────────────────────────────────────────────────

    fn parse_texture_format(s: &str) -> Result<wgpu::TextureFormat, String> {
        Ok(match s {
            "rgba8unorm" => wgpu::TextureFormat::Rgba8Unorm,
            "rgba8unorm-srgb" => wgpu::TextureFormat::Rgba8UnormSrgb,
            "bgra8unorm" => wgpu::TextureFormat::Bgra8Unorm,
            "r32float" => wgpu::TextureFormat::R32Float,
            "rgba32float" => wgpu::TextureFormat::Rgba32Float,
            "rgba16float" => wgpu::TextureFormat::Rgba16Float,
            other => return Err(format!("Unsupported/unrecognized texture format: {}", other)),
        })
    }
}

#[cfg(not(target_os = "linux"))]
pub mod inner {
    use tauri::WebviewWindow;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct AdapterInfo {
        pub id: u64,
        pub name: String,
        pub is_fallback_adapter: bool,
    }

    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    pub struct BindGroupLayoutEntryDesc {
        pub binding: u32,
        pub visibility: u32,
        pub kind: String,
        pub buffer_type: Option<String>,
        pub sample_type: Option<String>,
        pub storage_format: Option<String>,
    }

    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    pub struct BindGroupEntryDesc {
        pub binding: u32,
        pub kind: String,
        pub resource_id: u64,
    }

    #[tauri::command]
    pub async fn gpu_request_adapter() -> Result<AdapterInfo, String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_request_device() -> Result<u64, String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_create_buffer(_id: u64, _size: u64, _usage: u32, _mapped_at_creation: bool) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_write_buffer(_buffer_id: u64, _offset: u64, _data_base64: String) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_buffer_map_async(_buffer_id: u64, _mode: u32, _offset: u64, _size: u64) -> Result<String, String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_buffer_unmap(_buffer_id: u64, _data_base64: Option<String>) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_create_texture(
        _id: u64,
        _width: u32,
        _height: u32,
        _format: String,
        _usage: u32,
    ) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_texture_create_view(_id: u64, _texture_id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_write_texture(
        _texture_id: u64,
        _width: u32,
        _height: u32,
        _bytes_per_row: u32,
        _data_base64: String,
    ) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_create_sampler(_id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_create_shader_module(_id: u64, _code: String) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_create_bind_group_layout(_id: u64, _entries: Vec<BindGroupLayoutEntryDesc>) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_create_pipeline_layout(_id: u64, _bind_group_layout_ids: Vec<u64>) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_create_bind_group(
        _id: u64,
        _layout_id: u64,
        _entries: Vec<BindGroupEntryDesc>,
    ) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_create_compute_pipeline(
        _id: u64,
        _pipeline_layout_id: u64,
        _shader_module_id: u64,
        _entry_point: String,
    ) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_create_render_pipeline(
        _id: u64,
        _pipeline_layout_id: u64,
        _shader_module_id: u64,
        _vs_entry: String,
        _fs_entry: String,
        _target_format: String,
    ) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_create_command_encoder(_id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_encoder_begin_compute_pass(_encoder_id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }
    #[tauri::command]
    pub async fn gpu_encoder_set_compute_pipeline(_encoder_id: u64, _pipeline_id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }
    #[tauri::command]
    pub async fn gpu_encoder_set_bind_group(_encoder_id: u64, _index: u32, _bind_group_id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }
    #[tauri::command]
    pub async fn gpu_encoder_dispatch_workgroups(_encoder_id: u64, _x: u32, _y: u32, _z: u32) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }
    #[tauri::command]
    pub async fn gpu_encoder_end_compute_pass(_encoder_id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_encoder_begin_render_pass(
        _encoder_id: u64,
        _view_id: u64,
        _clear: Option<[f64; 4]>,
    ) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }
    #[tauri::command]
    pub async fn gpu_encoder_set_render_pipeline(_encoder_id: u64, _pipeline_id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }
    #[tauri::command]
    pub async fn gpu_encoder_set_render_bind_group(_encoder_id: u64, _index: u32, _bind_group_id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }
    #[tauri::command]
    pub async fn gpu_encoder_draw(_encoder_id: u64, _vertex_count: u32, _instance_count: u32) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }
    #[tauri::command]
    pub async fn gpu_encoder_end_render_pass(_encoder_id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_encoder_copy_buffer_to_texture(
        _encoder_id: u64,
        _src: u64,
        _dst_texture: u64,
        _bytes_per_row: u32,
        _width: u32,
        _height: u32,
    ) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_encoder_copy_texture_to_texture(
        _encoder_id: u64,
        _src: u64,
        _dst: u64,
        _width: u32,
        _height: u32,
    ) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_encoder_finish(_id: u64, _encoder_id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_queue_submit(_command_buffer_ids: Vec<u64>) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_canvas_get_context(
        _main_window: WebviewWindow,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> Result<u64, String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_canvas_configure(_context_id: u64, _format: String) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_canvas_get_current_texture(_context_id: u64) -> Result<u64, String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_canvas_present(_context_id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_canvas_sync_bounds(_context_id: u64, _x: i32, _y: i32, _width: u32, _height: u32) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }
}
