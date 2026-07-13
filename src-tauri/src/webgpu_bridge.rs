// ═══════════════════════════════════════════════════════════════════════════════
// webgpu_bridge.rs — Linux-only WebGPU-over-IPC bridge
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(target_os = "linux")]
pub mod inner {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use tauri::{Manager, WebviewWindow, Window, window::WindowBuilder, Emitter};

    // ─────────────────────────────────────────────────────────────────
    // ID allocation + generic registries
    // ─────────────────────────────────────────────────────────────────

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    fn next_id() -> u64 {
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    // ─────────────────────────────────────────────────────────────────
    // Adapter detection cache
    // ─────────────────────────────────────────────────────────────────
    // Adapter enumeration + software-fallback probing + GPU/distro/pkg-manager
    // detection is expensive (spawns processes, scans the filesystem, and can
    // create additional wgpu Instances). The site calls navigator.gpu.requestAdapter()
    // on every hover-preview canvas, so without caching this entire chain re-runs
    // on every single hover, causing severe UI stutter. We cache the outcome:
    // successes are cached for the lifetime of the app session; failures are
    // cached with a 60s TTL so a background retry can silently pick up a driver
    // fix (e.g. the user manually installed a package) without any user action.
    struct AdapterCacheEntry {
        result: Result<AdapterInfo, String>,
        computed_at: std::time::Instant,
    }

    const ADAPTER_FAILURE_RETRY_TTL: std::time::Duration = std::time::Duration::from_secs(60);

    static ADAPTER_CACHE: OnceLock<Mutex<Option<AdapterCacheEntry>>> = OnceLock::new();
    fn adapter_cache() -> &'static Mutex<Option<AdapterCacheEntry>> {
        ADAPTER_CACHE.get_or_init(|| Mutex::new(None))
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
        overlay: Window,
        surface: Option<wgpu::Surface<'static>>,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        pending_surface_texture: Option<wgpu::SurfaceTexture>,
        /// Kare başına registry sızıntısını önlemek için: bir önceki
        /// getCurrentTexture()'ın view id'si — yenisi kaydedilmeden ve
        /// present()'te temizlenir.
        current_view_id: Option<u64>,
        /// Son bilinen sayfa-içi (viewport) bounds — ana pencere taşındığında
        /// ekran konumunu yeniden hesaplamak için.
        viewport: (i32, i32, u32, u32),
        /// Overlay ilk gerçek kare present edilene dek gizli tutulur; hiç
        /// boyanmamış siyah kutu ekranda asla belirmesin.
        shown: bool,
        /// Tanılama: bu context'ten kaç kare present edildi.
        presents: u64,
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
        SetVertexBuffer { slot: u32, buffer: u64, offset: u64 },
        SetIndexBuffer { buffer: u64, format_u32: bool, offset: u64 },
        Draw { vertex_count: u32, instance_count: u32 },
        DrawIndexed { index_count: u32, instance_count: u32 },
        EndRenderPass,
        CopyBufferToTexture { src: u64, dst_texture: u64, bytes_per_row: u32, width: u32, height: u32 },
        CopyTextureToTexture { src: u64, dst: u64, width: u32, height: u32 },
    }

    #[derive(Default)]
    struct RecordedEncoder {
        ops: Vec<RecordedOp>,
    }

    // ─────────────────────────────────────────────────────────────────
    // Bridge state
    // ─────────────────────────────────────────────────────────────────

    pub struct BridgeState {
        instance: &'static wgpu::Instance,
        adapter: Option<Arc<wgpu::Adapter>>,
        device: Option<Arc<wgpu::Device>>,
        queue: Option<Arc<wgpu::Queue>>,
        registries: Registries,
    }

    static BRIDGE: OnceLock<Mutex<BridgeState>> = OnceLock::new();

    fn bridge() -> &'static Mutex<BridgeState> {
        BRIDGE.get_or_init(|| {
            Mutex::new(BridgeState {
                // Uygulama geneli paylaşılan instance (panik-korumalı, tek sefer).
                instance: crate::gpu::shared_instance(),
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

    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    pub struct AdapterInfo {
        pub id: u64,
        pub name: String,
        pub is_fallback_adapter: bool,
        pub is_software_adapter: bool,
        /// WebGPU spec özellik adları (örn. "shader-f16", "timestamp-query").
        /// Site tarafı `adapter.features.has(...)` ile kontrol edebilsin diye
        /// gerçek `wgpu::Features`'tan dönüştürülür (webgpu-native-shim.js
        /// önceden bunu her zaman boş `Set()` bırakıyordu).
        pub features: Vec<String>,
        /// WebGPU spec limit adları (camelCase, örn. "maxTextureDimension2D")
        /// → değer. Gerçek `wgpu::Limits`'ten dönüştürülür.
        pub limits: HashMap<String, u64>,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    pub struct DeviceInfo {
        pub id: u64,
        pub features: Vec<String>,
        pub limits: HashMap<String, u64>,
    }

    /// `wgpu::Features` bitmask'ını WebGPU spesifikasyonundaki resmi
    /// (kebab-case) özellik adlarına dönüştürür. Yalnızca 1:1 spec karşılığı
    /// olan bayraklar dahil edilir.
    fn wgpu_features_to_names(features: wgpu::Features) -> Vec<String> {
        let table: &[(wgpu::Features, &str)] = &[
            (wgpu::Features::DEPTH_CLIP_CONTROL, "depth-clip-control"),
            (wgpu::Features::TIMESTAMP_QUERY, "timestamp-query"),
            (wgpu::Features::TEXTURE_COMPRESSION_BC, "texture-compression-bc"),
            (wgpu::Features::TEXTURE_COMPRESSION_ETC2, "texture-compression-etc2"),
            (wgpu::Features::TEXTURE_COMPRESSION_ASTC, "texture-compression-astc"),
            (wgpu::Features::INDIRECT_FIRST_INSTANCE, "indirect-first-instance"),
            (wgpu::Features::SHADER_F16, "shader-f16"),
            (wgpu::Features::RG11B10UFLOAT_RENDERABLE, "rg11b10ufloat-renderable"),
            (wgpu::Features::BGRA8UNORM_STORAGE, "bgra8unorm-storage"),
            (wgpu::Features::FLOAT32_FILTERABLE, "float32-filterable"),
        ];
        table
            .iter()
            .filter(|(flag, _)| features.contains(*flag))
            .map(|(_, name)| name.to_string())
            .collect()
    }

    /// `wgpu::Limits`'i WebGPU spesifikasyonundaki camelCase limit adlarına
    /// dönüştürür. Compute/render pipeline kurulumu için en çok kontrol edilen
    /// alanlar kapsanır (tam liste değil — site tarafının pratikte baktığı
    /// çekirdek limitler).
    fn wgpu_limits_to_map(limits: &wgpu::Limits) -> HashMap<String, u64> {
        let mut map = HashMap::new();
        map.insert("maxTextureDimension1D".to_string(), limits.max_texture_dimension_1d as u64);
        map.insert("maxTextureDimension2D".to_string(), limits.max_texture_dimension_2d as u64);
        map.insert("maxTextureDimension3D".to_string(), limits.max_texture_dimension_3d as u64);
        map.insert("maxTextureArrayLayers".to_string(), limits.max_texture_array_layers as u64);
        map.insert("maxBindGroups".to_string(), limits.max_bind_groups as u64);
        map.insert("maxSampledTexturesPerShaderStage".to_string(), limits.max_sampled_textures_per_shader_stage as u64);
        map.insert("maxSamplersPerShaderStage".to_string(), limits.max_samplers_per_shader_stage as u64);
        map.insert("maxStorageBuffersPerShaderStage".to_string(), limits.max_storage_buffers_per_shader_stage as u64);
        map.insert("maxStorageTexturesPerShaderStage".to_string(), limits.max_storage_textures_per_shader_stage as u64);
        map.insert("maxUniformBuffersPerShaderStage".to_string(), limits.max_uniform_buffers_per_shader_stage as u64);
        map.insert("maxUniformBufferBindingSize".to_string(), limits.max_uniform_buffer_binding_size as u64);
        map.insert("maxStorageBufferBindingSize".to_string(), limits.max_storage_buffer_binding_size as u64);
        map.insert("maxVertexBuffers".to_string(), limits.max_vertex_buffers as u64);
        map.insert("maxBufferSize".to_string(), limits.max_buffer_size);
        map.insert("maxVertexAttributes".to_string(), limits.max_vertex_attributes as u64);
        map.insert("maxVertexBufferArrayStride".to_string(), limits.max_vertex_buffer_array_stride as u64);
        map.insert("maxComputeWorkgroupStorageSize".to_string(), limits.max_compute_workgroup_storage_size as u64);
        map.insert("maxComputeInvocationsPerWorkgroup".to_string(), limits.max_compute_invocations_per_workgroup as u64);
        map.insert("maxComputeWorkgroupSizeX".to_string(), limits.max_compute_workgroup_size_x as u64);
        map.insert("maxComputeWorkgroupSizeY".to_string(), limits.max_compute_workgroup_size_y as u64);
        map.insert("maxComputeWorkgroupSizeZ".to_string(), limits.max_compute_workgroup_size_z as u64);
        map.insert("maxComputeWorkgroupsPerDimension".to_string(), limits.max_compute_workgroups_per_dimension as u64);
        map
    }

    #[derive(serde::Serialize)]
    pub struct AdapterDiagnostics {
        pub vulkan_adapters_found: usize,
        pub gl_adapters_found: usize,
        pub adapter_names: Vec<String>,
        pub hint: String,
        pub pkg_manager: String,
        pub missing_vulkan_packages: Vec<String>,
        pub has_pkexec: bool,
        pub recommended_command: String,
        pub recommended_packages_id: String,
    }

    #[tauri::command]
    pub async fn gpu_request_adapter(app: tauri::AppHandle, window: WebviewWindow) -> Result<AdapterInfo, String> {
        // ─── Cache check ───────────────────────────────────────────
        // Successes are cached for the whole session (the adapter itself is
        // already stored in BridgeState, so re-detecting would be redundant).
        // Failures are cached with a short TTL so we don't hammer the system
        // on every hover, but still recover automatically if drivers get
        // installed later in the same session.
        {
            let cache = adapter_cache().lock().unwrap_or_else(|p| p.into_inner());
            if let Some(entry) = cache.as_ref() {
                match &entry.result {
                    Ok(info) => {
                        return Ok(info.clone());
                    }
                    Err(err_json) => {
                        if entry.computed_at.elapsed() < ADAPTER_FAILURE_RETRY_TTL {
                            return Err(err_json.clone());
                        }
                        println!("[WebGPU Bridge] Adapter failure cache TTL expired, retrying detection in background...");
                        // TTL expired: fall through and recompute below.
                    }
                }
            }
        }

        let result = gpu_request_adapter_uncached(app, window).await;

        {
            let mut cache = adapter_cache().lock().unwrap_or_else(|p| p.into_inner());
            *cache = Some(AdapterCacheEntry {
                result: result.clone(),
                computed_at: std::time::Instant::now(),
            });
        }

        result
    }

    // The original (expensive) detection logic, now only invoked on a cache miss.
    async fn gpu_request_adapter_uncached(app: tauri::AppHandle, main_window: WebviewWindow) -> Result<AdapterInfo, String> {
        let mut all_adapters = {
            let state = lock();
            state.instance.enumerate_adapters(wgpu::Backends::VULKAN | wgpu::Backends::GL)
        };

        // KÖK NEDEN DÜZELTMESİ (hibrit PRIME): adapter'ın bu pencereye
        // GERÇEKTEN sunum yapıp yapamadığını ölç. Ana pencereden geçici bir
        // probe surface açılır (yalnızca sorgulanır, configure edilmez,
        // seçimden sonra düşürülür). Önceden skor körlemesine Discrete'i
        // (NVIDIA) seçiyordu; XWayland penceresi iGPU'da yaşadığında NVIDIA
        // sunum yapamaz → pipeline çalışır ama tek kare basılamaz
        // (sahadaki "ses var, görüntü yok").
        let probe_surface = {
            let state = lock();
            match state.instance.create_surface(main_window.clone()) {
                Ok(sf) => Some(sf),
                Err(e) => {
                    println!("[WebGPU Bridge] Probe surface açılamadı ({}): sunum uyumluluğu skorlanamayacak", e);
                    None
                }
            }
        };

        println!("[WebGPU Bridge] Enumerate Adapters found {} devices:", all_adapters.len());
        let mut adapter_names = Vec::new();
        let mut vulkan_adapters_found = 0;
        let mut gl_adapters_found = 0;
        for (index, adapter) in all_adapters.iter().enumerate() {
            let info = adapter.get_info();
            let backend_str = format!("{:?}", info.backend);
            let type_str = format!("{:?}", info.device_type);
            let present_ok = probe_surface
                .as_ref()
                .map(|sf| adapter.is_surface_supported(sf))
                .unwrap_or(false);
            println!("  [#{}] Name: '{}', Backend: {}, Type: {}, present={}", index, info.name, backend_str, type_str, present_ok);
            adapter_names.push(format!("{} ({})", info.name, backend_str));
            match info.backend {
                wgpu::Backend::Vulkan => vulkan_adapters_found += 1,
                wgpu::Backend::Gl => gl_adapters_found += 1,
                _ => {}
            }
        }

        // Score and choose the best hardware adapter.
        // Sunum yeteneği HER ŞEYDEN önce gelir (+100): sunum yapamayan
        // "güçlü" GPU yerine sunum yapabilen iGPU tercih edilir.
        let chosen_idx = all_adapters
            .iter()
            .enumerate()
            .max_by_key(|(_, a)| {
                let info = a.get_info();
                let present_score = probe_surface
                    .as_ref()
                    .map(|sf| if a.is_surface_supported(sf) { 100 } else { 0 })
                    .unwrap_or(0);
                let backend_score = match info.backend {
                    wgpu::Backend::Vulkan => 3,
                    wgpu::Backend::Gl => 0,
                    _ => 0,
                };
                let type_score = match info.device_type {
                    wgpu::DeviceType::DiscreteGpu => 2,
                    wgpu::DeviceType::IntegratedGpu => 1,
                    _ => 0,
                };
                present_score + backend_score + type_score
            })
            .map(|(idx, _)| idx);
        drop(probe_surface);

        let mut chosen = chosen_idx.map(|idx| all_adapters.remove(idx));
        let mut is_software_fallback = false;

        // Try software fallback as a last resort if no hardware adapter is found
        if chosen.is_none() {
            println!("[WebGPU Bridge] No hardware GPU adapters found via enumeration. Trying force_fallback_adapter (CPU)...");
            // IMPORTANT: Clone the instance *before* .await so the MutexGuard is
            // dropped prior to the async suspension point. Holding a MutexGuard
            // across .await makes the future non-Send, which Tauri's command
            // runtime requires.
            let instance_clone = {
                let state = lock();
                state.instance
            };
            let fallback_opt = instance_clone.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                force_fallback_adapter: true,
                compatible_surface: None,
            }).await;
            if let Some(fallback_adapter) = fallback_opt {
                println!("[WebGPU Bridge] Successfully acquired software fallback adapter: {}", fallback_adapter.get_info().name);
                chosen = Some(fallback_adapter);
                is_software_fallback = true;
            }
        }

        let adapter = match chosen {
            Some(a) => a,
            None => {
                // NOTE: intentionally using the cheap vendor-only lookup here, not
                // detect_gpu() — that function creates a *third* wgpu Vulkan
                // instance and blocks synchronously on an adapter request, which
                // is redundant (we already know adapter acquisition failed above)
                // and was a major contributor to hover-triggered UI stutter.
                let vendor = crate::gpu_detector::detect_vendor_only();
                let missing_vulkan_packages = crate::gpu_detector::check_missing_icds(&vendor);
                let pkg_manager = crate::gpu_detector::detect_pkg_manager();
                let has_pkexec = crate::gpu_detector::has_pkexec();
                
                let recommended_packages_id = missing_vulkan_packages.first().cloned().unwrap_or_else(|| "all".to_string());
                
                let recommended_command = crate::gpu_detector::get_whitelisted_install_command(&pkg_manager, &recommended_packages_id)
                    .map(|(_, cmd)| cmd)
                    .unwrap_or_else(|| {
                        if pkg_manager != "unknown" {
                            format!("Lütfen '{}' paketini manuel olarak kurun.", recommended_packages_id)
                        } else {
                            "Lütfen dağıtımınızın paket yöneticisinden ekran kartınıza uygun Vulkan sürücülerini kurun.".to_string()
                        }
                    });

                let hint = if pkg_manager != "unknown" {
                    format!("Vulkan sürücüsü kurulu olmayabilir. Şu komutla kurabilirsiniz:\n{}", recommended_command)
                } else {
                    "Vulkan sürücüsü kurulu olmayabilir veya GPU uyumsuz olabilir. Lütfen dağıtımınızın paket yöneticisinden ekran kartınıza uygun Vulkan sürücülerini kurun.".to_string()
                };

                let diagnostics = AdapterDiagnostics {
                    vulkan_adapters_found,
                    gl_adapters_found,
                    adapter_names,
                    hint,
                    pkg_manager,
                    missing_vulkan_packages,
                    has_pkexec,
                    recommended_command,
                    recommended_packages_id,
                };
                let err_json = serde_json::to_string(&diagnostics).unwrap_or_else(|_| "No adapter available".to_string());
                return Err(err_json);
            }
        };

        let info = adapter.get_info();
        let is_software_adapter = is_software_fallback || info.device_type == wgpu::DeviceType::Cpu;
        let is_gl_fallback = info.backend == wgpu::Backend::Gl;
        // Adapter'ı state'e taşımadan önce gerçek features/limits'i oku —
        // önceden bu veri hiç toplanmıyor, JS tarafı her zaman boş görüyordu.
        let features = wgpu_features_to_names(adapter.features());
        let limits = wgpu_limits_to_map(&adapter.limits());

        // Emit warnings to frontend if software rendering or OpenGL fallback is used
        if is_software_adapter {
            let _ = app.emit("openanime://gpu-warning", "Yazılımsal (CPU) render kullanılıyor, performans 4K upscale için yetersizdir.");
        } else if is_gl_fallback {
            let _ = app.emit("openanime://gpu-warning", "OpenGL fallback renderer kullanılıyor, performans veya uyumluluk sorunları yaşanabilir.");
        }

        let id = next_id();
        {
            let mut state = lock();
            state.adapter = Some(Arc::new(adapter));
        }

        Ok(AdapterInfo {
            id,
            name: info.name,
            is_fallback_adapter: is_software_fallback,
            is_software_adapter,
            features,
            limits,
        })
    }

    #[tauri::command]
    pub async fn gpu_request_device() -> Result<DeviceInfo, String> {
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
        // Gerçek device features/limits'ini oku — önceden JS tarafı hiç
        // görmüyordu, GPUDevice her zaman boş Set()/{} ile kuruluyordu.
        let features = wgpu_features_to_names(device.features());
        let limits = wgpu_limits_to_map(&device.limits());

        let mut state = lock();
        state.device = Some(device);
        state.queue = Some(queue);
        Ok(DeviceInfo { id: 0, features, limits })
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
        mip_level_count: Option<u32>,
    ) -> Result<(), String> {
        let dev = device()?;
        let tex_format = parse_texture_format(&format)?;
        let tex = dev.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: mip_level_count.unwrap_or(1).max(1),
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

    #[derive(serde::Deserialize, Default)]
    #[serde(rename_all = "camelCase", default)]
    pub struct SamplerDesc {
        pub mag_filter: Option<String>,
        pub min_filter: Option<String>,
        pub mipmap_filter: Option<String>,
        pub address_mode_u: Option<String>,
        pub address_mode_v: Option<String>,
        pub address_mode_w: Option<String>,
    }

    #[tauri::command]
    pub async fn gpu_create_sampler(id: u64, descriptor: Option<SamplerDesc>) -> Result<(), String> {
        let dev = device()?;
        let d = descriptor.unwrap_or_default();
        let mipmap_filter = match d.mipmap_filter.as_deref() {
            Some("linear") => wgpu::FilterMode::Linear,
            _ => wgpu::FilterMode::Nearest,
        };
        let sampler = dev.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: parse_address_mode(d.address_mode_u.as_deref()),
            address_mode_v: parse_address_mode(d.address_mode_v.as_deref()),
            address_mode_w: parse_address_mode(d.address_mode_w.as_deref()),
            mag_filter: parse_filter_mode(d.mag_filter.as_deref()),
            min_filter: parse_filter_mode(d.min_filter.as_deref()),
            mipmap_filter,
            ..Default::default()
        });
        lock().registries.samplers.insert(id, sampler);
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────
    // Shader modules
    // ─────────────────────────────────────────────────────────────────

    /// Shader modülü oluşturur. Derleme/doğrulama hatası varsa panic yerine
    /// error scope ile yakalanıp mesaj olarak döndürülür — shim bunu
    /// getCompilationInfo() için saklar.
    #[tauri::command]
    pub async fn gpu_create_shader_module(id: u64, code: String) -> Result<Option<String>, String> {
        let dev = device()?;
        dev.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(code.into()),
        });
        lock().registries.shader_modules.insert(id, module);

        let fut = dev.pop_error_scope();
        dev.poll(wgpu::Maintain::Poll);
        let compile_error = fut.await.map(|e| e.to_string());
        if let Some(ref err) = compile_error {
            crate::log!("[WebGPU Bridge] Shader derleme hatası (id {}): {}", id, err);
        }
        Ok(compile_error)
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
        /// Buffer binding'leri için isteğe bağlı offset/size (WebGPU
        /// GPUBufferBinding.offset/size karşılığı).
        pub offset: Option<u64>,
        pub size: Option<u64>,
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
                        if e.offset.is_some() || e.size.is_some() {
                            wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: buf,
                                offset: e.offset.unwrap_or(0),
                                size: e.size.and_then(std::num::NonZeroU64::new),
                            })
                        } else {
                            wgpu::BindingResource::Buffer(buf.as_entire_buffer_binding())
                        }
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

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct VertexAttributeDesc {
        pub format: String,
        pub offset: u64,
        pub shader_location: u32,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct VertexBufferLayoutDesc {
        pub array_stride: u64,
        #[serde(default)]
        pub step_mode: Option<String>,
        pub attributes: Vec<VertexAttributeDesc>,
    }

    fn parse_vertex_format(s: &str) -> Result<wgpu::VertexFormat, String> {
        Ok(match s {
            "float32" => wgpu::VertexFormat::Float32,
            "float32x2" => wgpu::VertexFormat::Float32x2,
            "float32x3" => wgpu::VertexFormat::Float32x3,
            "float32x4" => wgpu::VertexFormat::Float32x4,
            "uint32" => wgpu::VertexFormat::Uint32,
            "uint32x2" => wgpu::VertexFormat::Uint32x2,
            "uint32x4" => wgpu::VertexFormat::Uint32x4,
            "sint32" => wgpu::VertexFormat::Sint32,
            "sint32x2" => wgpu::VertexFormat::Sint32x2,
            "sint32x4" => wgpu::VertexFormat::Sint32x4,
            "unorm8x4" => wgpu::VertexFormat::Unorm8x4,
            "snorm8x4" => wgpu::VertexFormat::Snorm8x4,
            "uint8x4" => wgpu::VertexFormat::Uint8x4,
            "float16x2" => wgpu::VertexFormat::Float16x2,
            "float16x4" => wgpu::VertexFormat::Float16x4,
            other => return Err(format!("Unsupported vertex format: {}", other)),
        })
    }

    #[tauri::command]
    pub async fn gpu_create_render_pipeline(
        id: u64,
        pipeline_layout_id: u64,
        shader_module_id: u64,
        vs_entry: String,
        fs_entry: String,
        target_format: String,
        vertex_buffers: Option<Vec<VertexBufferLayoutDesc>>,
        topology: Option<String>,
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

        // JS'ten gelen vertex buffer layout'larını wgpu tiplerine çevir.
        // Attribute dizileri, layout struct'ları onlara referans verdiği için
        // ayrı bir Vec'te sabit tutulmalı.
        let vb_descs = vertex_buffers.unwrap_or_default();
        let attr_lists: Vec<Vec<wgpu::VertexAttribute>> = vb_descs
            .iter()
            .map(|vb| {
                vb.attributes
                    .iter()
                    .map(|a| {
                        Ok(wgpu::VertexAttribute {
                            format: parse_vertex_format(&a.format)?,
                            offset: a.offset,
                            shader_location: a.shader_location,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .collect::<Result<Vec<_>, String>>()?;
        let vb_layouts: Vec<wgpu::VertexBufferLayout> = vb_descs
            .iter()
            .zip(attr_lists.iter())
            .map(|(vb, attrs)| wgpu::VertexBufferLayout {
                array_stride: vb.array_stride,
                step_mode: match vb.step_mode.as_deref() {
                    Some("instance") => wgpu::VertexStepMode::Instance,
                    _ => wgpu::VertexStepMode::Vertex,
                },
                attributes: attrs,
            })
            .collect();

        let topology = match topology.as_deref() {
            Some("point-list") => wgpu::PrimitiveTopology::PointList,
            Some("line-list") => wgpu::PrimitiveTopology::LineList,
            Some("line-strip") => wgpu::PrimitiveTopology::LineStrip,
            Some("triangle-strip") => wgpu::PrimitiveTopology::TriangleStrip,
            _ => wgpu::PrimitiveTopology::TriangleList,
        };

        let pipeline = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: &vs_entry,
                buffers: &vb_layouts,
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
                topology,
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
    pub async fn gpu_encoder_set_vertex_buffer(encoder_id: u64, slot: u32, buffer_id: u64, offset: Option<u64>) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::SetVertexBuffer { slot, buffer: buffer_id, offset: offset.unwrap_or(0) })
    }
    #[tauri::command]
    pub async fn gpu_encoder_set_index_buffer(encoder_id: u64, buffer_id: u64, format: String, offset: Option<u64>) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::SetIndexBuffer {
            buffer: buffer_id,
            format_u32: format == "uint32",
            offset: offset.unwrap_or(0),
        })
    }
    #[tauri::command]
    pub async fn gpu_encoder_draw(encoder_id: u64, vertex_count: u32, instance_count: u32) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::Draw { vertex_count, instance_count })
    }
    #[tauri::command]
    pub async fn gpu_encoder_draw_indexed(encoder_id: u64, index_count: u32, instance_count: u32) -> Result<(), String> {
        push_op(encoder_id, RecordedOp::DrawIndexed { index_count, instance_count })
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
                            RecordedOp::SetVertexBuffer { slot, buffer, offset } => {
                                let buf = state.registries.buffers.get(buffer).ok_or("Unknown vertex buffer id")?;
                                pass.set_vertex_buffer(*slot, buf.slice(*offset..));
                            }
                            RecordedOp::SetIndexBuffer { buffer, format_u32, offset } => {
                                let buf = state.registries.buffers.get(buffer).ok_or("Unknown index buffer id")?;
                                let fmt = if *format_u32 { wgpu::IndexFormat::Uint32 } else { wgpu::IndexFormat::Uint16 };
                                pass.set_index_buffer(buf.slice(*offset..), fmt);
                            }
                            RecordedOp::Draw { vertex_count, instance_count } => {
                                pass.draw(0..*vertex_count, 0..*instance_count);
                            }
                            RecordedOp::DrawIndexed { index_count, instance_count } => {
                                pass.draw_indexed(0..*index_count, 0, 0..*instance_count);
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

    /// queue.onSubmittedWorkDone() karşılığı: gönderilmiş işin GPU'da
    /// tamamlanmasını bekler.
    #[tauri::command]
    pub async fn gpu_queue_on_submitted_work_done() -> Result<(), String> {
        let q = queue()?;
        let dev = device()?;
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        q.on_submitted_work_done(move || {
            let _ = tx.send(());
        });
        dev.poll(wgpu::Maintain::Wait);
        rx.await.map_err(|_| "on_submitted_work_done callback dropped".to_string())
    }

    // ─────────────────────────────────────────────────────────────────
    // Binary IPC transport — base64 yerine ham byte gövdesi
    // ─────────────────────────────────────────────────────────────────
    // Tauri v2 raw invoke: JS `invoke(cmd, bytes, { headers })` çağrısında
    // gövde InvokeBody::Raw olarak gelir; parametreler header'da taşınır.
    // Base64 encode/decode ve %33 boyut şişmesi tamamen ortadan kalkar.

    fn header_u64(request: &tauri::ipc::Request<'_>, name: &str) -> Result<u64, String> {
        request
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| format!("Missing/invalid header: {}", name))
    }

    fn raw_body<'a>(request: &'a tauri::ipc::Request<'_>) -> Result<&'a [u8], String> {
        match request.body() {
            tauri::ipc::InvokeBody::Raw(bytes) => Ok(bytes.as_slice()),
            _ => Err("Expected raw binary body".to_string()),
        }
    }

    #[tauri::command]
    pub fn gpu_write_buffer_bin(request: tauri::ipc::Request<'_>) -> Result<(), String> {
        let buffer_id = header_u64(&request, "x-buffer-id")?;
        let offset = header_u64(&request, "x-offset").unwrap_or(0);
        let bytes = raw_body(&request)?;

        let q = queue()?;
        let state = lock();
        let buf = state
            .registries
            .buffers
            .get(&buffer_id)
            .ok_or("Unknown buffer id")?;
        q.write_buffer(buf, offset, bytes);
        Ok(())
    }

    #[tauri::command]
    pub fn gpu_write_texture_bin(request: tauri::ipc::Request<'_>) -> Result<(), String> {
        // Native player aktifken IPC üzerinden gelen kare yazımlarını at
        // (bant genişliği koruması) — base64 yolundaki davranışla aynı.
        if let Ok(manager) = crate::native_render::inner::get_manager().try_lock() {
            if manager.player.is_some() {
                return Ok(());
            }
        }

        let texture_id = header_u64(&request, "x-texture-id")?;
        let width = header_u64(&request, "x-width")? as u32;
        let height = header_u64(&request, "x-height")? as u32;
        let bytes_per_row = header_u64(&request, "x-bytes-per-row")? as u32;
        let bytes = raw_body(&request)?;

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
            bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        Ok(())
    }

    /// mapAsync + okuma: ham binary yanıt döndürür (base64 yok).
    #[tauri::command]
    pub async fn gpu_buffer_read_bin(
        buffer_id: u64,
        offset: u64,
        size: u64,
    ) -> Result<tauri::ipc::Response, String> {
        let (rx, dev) = {
            let state = lock();
            let buf = state
                .registries
                .buffers
                .get(&buffer_id)
                .ok_or("Unknown buffer id")?;

            let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), wgpu::BufferAsyncError>>();
            buf.slice(offset..(offset + size)).map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });

            let dev = state.device.clone().ok_or("No device")?;
            (rx, dev)
        };

        dev.poll(wgpu::Maintain::Wait);

        rx.await
            .map_err(|_| "Channel closed".to_string())?
            .map_err(|e| e.to_string())?;

        let bytes = {
            let state = lock();
            let buf = state
                .registries
                .buffers
                .get(&buffer_id)
                .ok_or("Unknown buffer id")?;
            let view = buf.slice(offset..(offset + size)).get_mapped_range();
            let data = view.to_vec();
            drop(view);
            buf.unmap();
            data
        };

        Ok(tauri::ipc::Response::new(bytes))
    }

    // ─────────────────────────────────────────────────────────────────
    // Native video frame import — IPC'den sıfır byte geçer
    // ─────────────────────────────────────────────────────────────────

    /// GStreamer player aktifken son çözülmüş kareyi köprüye kayıtlı bir
    /// wgpu::Texture'a doğrudan kopyalar. Sitenin kendi WebGPU pipeline'ı
    /// böylece gerçek video kareleriyle beslenir (importExternalTexture).
    /// Dönüş: kare kopyalandıysa [width, height], kare yoksa None.
    #[tauri::command]
    pub async fn gpu_import_video_frame(texture_id: u64) -> Result<Option<[u32; 2]>, String> {
        let frame_signal = {
            let manager = crate::native_render::inner::get_manager()
                .try_lock()
                .map_err(|_| "Native player busy".to_string())?;
            match &manager.player {
                Some(player) => player.get_frame_signal(),
                None => return Ok(None),
            }
        };

        let q = queue()?;
        let state = lock();
        let tex = state.registries.textures.get(&texture_id).ok_or("Unknown texture id")?;
        let tex_w = tex.width();
        let tex_h = tex.height();

        let guard = frame_signal.frame.lock().unwrap_or_else(|p| p.into_inner());
        let Some(ref frame) = *guard else {
            return Ok(None);
        };
        if frame.width == 0 || frame.height == 0 || frame.data.is_empty() {
            return Ok(None);
        }
        if frame.width != tex_w || frame.height != tex_h {
            // Shim texture'ı gerçek video boyutuyla yeniden oluşturabilsin
            // diye kare boyutunu hata mesajında bildir.
            return Err(format!(
                "frame-size-mismatch:{}x{}", frame.width, frame.height
            ));
        }

        q.write_texture(
            wgpu::ImageCopyTexture {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame.data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(frame.width * 4),
                rows_per_image: Some(frame.height),
            },
            wgpu::Extent3d { width: frame.width, height: frame.height, depth_or_array_layers: 1 },
        );
        Ok(Some([frame.width, frame.height]))
    }

    // ─────────────────────────────────────────────────────────────────
    // Kaynak yaşam döngüsü + hata kapsamları + kurtarma
    // ─────────────────────────────────────────────────────────────────

    /// JS tarafındaki destroy() çağrılarının Rust registry'sini gerçekten
    /// temizlemesi için. Önceden buffer/texture'lar bölümler arası sonsuza
    /// dek sızıyordu.
    #[tauri::command]
    pub async fn gpu_destroy_resource(kind: String, id: u64) -> Result<(), String> {
        let overlay_to_close = {
            let mut state = lock();
            let r = &mut state.registries;
            match kind.as_str() {
                "buffer" => { r.buffers.remove(&id); None }
                "texture" => { r.textures.remove(&id); None }
                "texture_view" => { r.texture_views.remove(&id); None }
                "sampler" => { r.samplers.remove(&id); None }
                "shader_module" => { r.shader_modules.remove(&id); None }
                "bind_group" => { r.bind_groups.remove(&id); None }
                "bind_group_layout" => { r.bind_group_layouts.remove(&id); None }
                "pipeline_layout" => { r.pipeline_layouts.remove(&id); None }
                "compute_pipeline" => { r.compute_pipelines.remove(&id); None }
                "render_pipeline" => { r.render_pipelines.remove(&id); None }
                "command_buffer" => { r.command_buffers.remove(&id); None }
                "encoder" => { r.encoders.remove(&id); None }
                "canvas_context" => r.canvas_contexts.remove(&id).map(|ctx| ctx.overlay),
                other => return Err(format!("Unknown resource kind: {}", other)),
            }
        };

        if let Some(overlay) = overlay_to_close {
            let app = overlay.app_handle().clone();
            let _ = app.run_on_main_thread(move || {
                let _ = overlay.close();
            });
        }
        Ok(())
    }

    /// GPUDevice.pushErrorScope() karşılığı.
    #[tauri::command]
    pub async fn gpu_push_error_scope(filter: String) -> Result<(), String> {
        let dev = device()?;
        let f = match filter.as_str() {
            "out-of-memory" => wgpu::ErrorFilter::OutOfMemory,
            "internal" => wgpu::ErrorFilter::Internal,
            _ => wgpu::ErrorFilter::Validation,
        };
        dev.push_error_scope(f);
        Ok(())
    }

    /// GPUDevice.popErrorScope() karşılığı: hata olduysa mesajını döndürür.
    #[tauri::command]
    pub async fn gpu_pop_error_scope() -> Result<Option<String>, String> {
        let dev = device()?;
        let fut = dev.pop_error_scope();
        dev.poll(wgpu::Maintain::Wait);
        Ok(fut.await.map(|e| e.to_string()))
    }

    /// Device-lost kurtarması: köprü durumunu ve paylaşılan device'ı sıfırlar.
    /// Sonraki requestAdapter()/requestDevice() temiz bir kurulum yapar.
    #[tauri::command]
    pub async fn gpu_reset_bridge() -> Result<(), String> {
        let overlays: Vec<Window> = {
            let mut state = lock();
            state.device = None;
            state.queue = None;
            let contexts = std::mem::take(&mut state.registries.canvas_contexts);
            state.registries = Registries::default();
            contexts.into_values().map(|ctx| ctx.overlay).collect()
        };

        for overlay in overlays {
            let app = overlay.app_handle().clone();
            let _ = app.run_on_main_thread(move || {
                let _ = overlay.close();
            });
        }

        crate::renderer::device::reset_shared_device();
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────
    // Canvas presentation (overlay)
    // ─────────────────────────────────────────────────────────────────

    /// Sayfa-içi (viewport, CSS px) koordinatı EKRAN fiziksel koordinatına
    /// çevirir: ekran = main.inner_position() + viewport × scale_factor.
    /// KÖK NEDEN DÜZELTMESİ: overlay'ler önceden viewport koordinatıyla
    /// doğrudan ekrana konumlandırılıyordu (ana pencere konumu eklenmeden) —
    /// Wayland'da set_position no-op olduğundan gizli kalan bu bug, X11'e
    /// geçilince overlay'leri yanlış yere (ekran sol-üstüne) yerleştiriyordu:
    /// siyah video alanı + "donmuş UI" görüntüsünün kaynağı.
    fn viewport_to_screen(main: &WebviewWindow, x: i32, y: i32) -> tauri::PhysicalPosition<i32> {
        let scale = main.scale_factor().unwrap_or(1.0);
        let origin = main
            .inner_position()
            .unwrap_or(tauri::PhysicalPosition::new(0, 0));
        tauri::PhysicalPosition::new(
            origin.x + (x as f64 * scale).round() as i32,
            origin.y + (y as f64 * scale).round() as i32,
        )
    }

    fn viewport_size_physical(main: &WebviewWindow, w: u32, h: u32) -> tauri::PhysicalSize<u32> {
        let scale = main.scale_factor().unwrap_or(1.0);
        tauri::PhysicalSize::new(
            (w.max(1) as f64 * scale).round() as u32,
            (h.max(1) as f64 * scale).round() as u32,
        )
    }

    /// Ana pencere taşındığında tüm canvas overlay'lerini kayıtlı viewport
    /// bounds'larıyla yeniden konumlandırır (lib.rs Moved event'inden çağrılır).
    pub fn reposition_overlays(app: &tauri::AppHandle) {
        let Some(main) = app.get_webview_window("main") else { return };
        let items: Vec<(Window, (i32, i32, u32, u32))> = {
            let state = lock();
            state
                .registries
                .canvas_contexts
                .values()
                .map(|ctx| (ctx.overlay.clone(), ctx.viewport))
                .collect()
        };
        for (overlay, (x, y, w, h)) in items {
            let _ = overlay.set_position(tauri::Position::Physical(viewport_to_screen(&main, x, y)));
            let _ = overlay.set_size(tauri::Size::Physical(viewport_size_physical(&main, w, h)));
        }
    }

    #[tauri::command]
    pub async fn gpu_canvas_get_context(
        main_window: WebviewWindow,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<u64, String> {
        let app: tauri::AppHandle = main_window.app_handle().clone();

        // Canvas overlay fırtınası önlemi: menü önizlemeleri gibi ikincil
        // canvas'lar sınırsız X11 penceresi yaratmasın. Video canvas'ı ilk
        // context olduğundan etkilenmez.
        {
            let state = lock();
            let count = state.registries.canvas_contexts.len();
            if count >= 2 {
                println!("[WebGPU Bridge] Canvas overlay limiti aşıldı (mevcut={}) — yeni context reddedildi", count);
                return Err(format!("canvas overlay limit reached ({})", count));
            }
        }

        let label = format!("gpu_canvas_{}", next_id());

        // Webview'sız düz pencere: wgpu surface için yalnızca raw window
        // handle gerekir; WebKit webview başlatmak israf ve risk.
        let (window_tx, window_rx) = tokio::sync::oneshot::channel::<Result<Window, String>>();
        let (realize_tx, realize_rx) = tokio::sync::oneshot::channel::<()>();
        let realize_tx = Arc::new(Mutex::new(Some(realize_tx)));

        let app_for_build = app.clone();
        let label_for_build = label.clone();
        app.run_on_main_thread(move || {
            let mut builder = WindowBuilder::new(&app_for_build, label_for_build)
                .title("GPU Canvas Overlay")
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .focused(false)
                .skip_taskbar(true)
                .inner_size(width.max(1) as f64, height.max(1) as f64);
            // Ana pencereye transient bağla (X11'de z-order/minimize uyumu).
            // parent() self'i tüketir; hata pratikte yalnızca main penceresi
            // yokken oluşur — o durumda overlay zaten anlamsızdır, hata döndür.
            if let Some(parent) = app_for_build.get_window("main") {
                builder = match builder.parent(&parent) {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = window_tx.send(Err(format!("Overlay parent bağlanamadı: {}", e)));
                        return;
                    }
                };
            }
            let build_result = builder.build();

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
            Ok(Ok(())) => {
                // Wayland/GTK'da giriş bölgesi pencere haritalandığında
                // sıfırlanabiliyor — realize SONRASI yeniden uygula ki
                // overlay fare olaylarını yutmasın (player tıklanabilsin).
                let _ = overlay.set_ignore_cursor_events(true);
            }
            _ => {
                let _ = overlay.close();
                return Err("GPU canvas overlay did not realize in time".to_string());
            }
        }

        // Ekran koordinatına dönüştürerek konumlandır (viewport + ana pencere).
        let _ = overlay.set_position(tauri::Position::Physical(viewport_to_screen(&main_window, x, y)));
        let _ = overlay.set_size(tauri::Size::Physical(viewport_size_physical(&main_window, width, height)));

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
                current_view_id: None,
                viewport: (x, y, width, height),
                shown: false,
                presents: 0,
            },
        );
        println!("[WebGPU Bridge] Canvas overlay yaratıldı: ctx={} viewport=({},{} {}x{})", ctx_id, x, y, width, height);
        Ok(ctx_id)
    }

    /// wgpu surface formatını WebGPU spec adına çevirir (canvas formatları).
    fn texture_format_to_string(f: wgpu::TextureFormat) -> &'static str {
        match f {
            wgpu::TextureFormat::Bgra8Unorm => "bgra8unorm",
            wgpu::TextureFormat::Bgra8UnormSrgb => "bgra8unorm-srgb",
            wgpu::TextureFormat::Rgba8Unorm => "rgba8unorm",
            wgpu::TextureFormat::Rgba8UnormSrgb => "rgba8unorm-srgb",
            wgpu::TextureFormat::Rgba16Float => "rgba16float",
            wgpu::TextureFormat::Rgb10a2Unorm => "rgb10a2unorm",
            _ => "bgra8unorm",
        }
    }

    /// İstenen formatı surface'ın GERÇEKTEN desteklediği listeye kelepçeler.
    /// wgpu, desteklenmeyen formatla configure() çağrısında handle_error_fatal
    /// ile panic'ler (on_uncaptured_error'a düşmez) — sahada görülen
    /// "Requested format Rgba16Float is not in list of supported formats"
    /// siyah ekranının kökü buydu.
    fn clamp_surface_format(
        requested: wgpu::TextureFormat,
        supported: &[wgpu::TextureFormat],
    ) -> wgpu::TextureFormat {
        if supported.contains(&requested) {
            return requested;
        }
        if supported.contains(&wgpu::TextureFormat::Bgra8Unorm) {
            return wgpu::TextureFormat::Bgra8Unorm;
        }
        supported.first().copied().unwrap_or(wgpu::TextureFormat::Bgra8Unorm)
    }

    /// Canvas'ı yapılandırır; gerçekten kullanılan formatı (spec adıyla)
    /// döndürür — shim bunu kendi format alanına geri yazar.
    #[tauri::command]
    pub async fn gpu_canvas_configure(context_id: u64, format: String) -> Result<String, String> {
        let dev = device()?;
        let requested_format = parse_texture_format(&format)?;

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

        // Uyumsuz surface koruması: adapter bu pencereye sunum yapamıyorsa
        // configure HİÇ çağrılmaz (wgpu configure hatası fatal panic'tir).
        if !adapter.is_surface_supported(&surface) {
            let name = adapter.get_info().name;
            crate::log!("[WebGPU Bridge] HATA: adapter '{}' bu pencereye sunum yapamıyor (hibrit PRIME uyumsuzluğu) — canvas devre dışı", name);
            return Err(format!("adapter '{}' cannot present to this window", name));
        }
        let caps = surface.get_capabilities(&adapter);
        if caps.formats.is_empty() {
            crate::log!("[WebGPU Bridge] HATA: surface caps boş — canvas devre dışı");
            return Err("surface reports no supported formats".to_string());
        }
        let tex_format = clamp_surface_format(requested_format, &caps.formats);
        crate::log!(
            "[WebGPU Bridge] Canvas configure: ctx={} istek={} kullanılan={} caps={:?} adapter='{}'",
            context_id, format, texture_format_to_string(tex_format), caps.formats, adapter.get_info().name
        );
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
            // Auto: wgpu configure() sırasında sürücünün O ANKİ desteklediği moda
            // çözümlenir. Önceden `caps.alpha_modes[0]` kullanılıyordu; NVIDIA/
            // Wayland'da get_capabilities() PostMultiplied raporlayıp configure()
            // reddedebiliyor ("not in the list of supported alpha modes: [Opaque]")
            // ve panic=abort tüm uygulamayı çökertiyordu.
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&dev, &config);

        let mut state = lock();
        let ctx = state.registries.canvas_contexts.get_mut(&context_id).ok_or("Unknown canvas context id")?;
        ctx.surface = Some(surface);
        ctx.format = tex_format;
        Ok(texture_format_to_string(tex_format).to_string())
    }

    #[tauri::command]
    pub async fn gpu_canvas_get_current_texture(context_id: u64, view_id: Option<u64>) -> Result<u64, String> {
        let mut state = lock();
        let ctx = state.registries.canvas_contexts.get_mut(&context_id).ok_or("Unknown canvas context id")?;
        let surface = ctx.surface.as_ref().ok_or("configure() not called yet")?;

        // present() çağrılmadan ikinci kez istenirse eski surface texture'ı
        // düşür — aksi halde swapchain slotu sızar.
        ctx.pending_surface_texture = None;

        let output = surface.get_current_texture().map_err(|e| {
            static ERR_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = ERR_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if n == 1 || n % 100 == 0 {
                crate::log!("[WebGPU Bridge] get_current_texture hatası (x{}): {}", n, e);
            }
            format!("get_current_texture failed: {}", e)
        })?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        ctx.pending_surface_texture = Some(output);

        // JS tarafının önceden ayırdığı id ile deterministik kayıt: shim'in
        // getCurrentTexture().createView()'i tam bu id'yi döndürür.
        let id = view_id.unwrap_or_else(next_id);
        let old_view_id = ctx.current_view_id.replace(id);
        drop(state);

        let mut state = lock();
        // Önceki karenin view'ını registry'den sil (sınırsız büyüme düzeltmesi).
        if let Some(old_id) = old_view_id {
            state.registries.texture_views.remove(&old_id);
        }
        state.registries.texture_views.insert(id, view);
        Ok(id)
    }

    #[tauri::command]
    pub async fn gpu_canvas_present(context_id: u64) -> Result<(), String> {
        let mut state = lock();
        let ctx = state.registries.canvas_contexts.get_mut(&context_id).ok_or("Unknown canvas context id")?;
        let output = ctx.pending_surface_texture.take();
        let view_id = ctx.current_view_id.take();
        if output.is_some() {
            ctx.presents += 1;
            if ctx.presents == 1 {
                println!("[WebGPU Bridge] İLK KARE basıldı (ctx={})", context_id);
            } else if ctx.presents % 300 == 0 {
                println!("[WebGPU Bridge] present sayacı: ctx={} kare={}", context_id, ctx.presents);
            }
        }
        // İlk GERÇEK kare present ediliyorsa overlay'i görünür yap —
        // o ana kadar gizli kalır (boyanmamış siyah kutu koruması).
        let show_now = if output.is_some() && !ctx.shown {
            ctx.shown = true;
            Some(ctx.overlay.clone())
        } else {
            None
        };
        if let Some(id) = view_id {
            state.registries.texture_views.remove(&id);
        }
        drop(state);
        if let Some(output) = output {
            output.present();
        }
        if let Some(overlay) = show_now {
            let _ = overlay.show();
            let _ = overlay.set_ignore_cursor_events(true);
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
        // Viewport → EKRAN dönüşümü (ana pencere konumu + ölçek dahil).
        if let Some(main) = ctx.overlay.app_handle().get_webview_window("main") {
            let _ = ctx.overlay.set_position(tauri::Position::Physical(viewport_to_screen(&main, x, y)));
            let _ = ctx.overlay.set_size(tauri::Size::Physical(viewport_size_physical(&main, width, height)));
        }
        ctx.viewport = (x, y, width, height);
        // Boyut/pozisyon değişimi giriş bölgesini sıfırlayabilir — tıklama
        // geçirgenliğini her senkronda yeniden garanti et.
        let _ = ctx.overlay.set_ignore_cursor_events(true);
        
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
                    // Önceden hardcoded PostMultiplied idi — NVIDIA/Wayland'da
                    // sürücü yalnızca Opaque destekleyince configure() panikliyor
                    // ve panic=abort tüm uygulamayı çökertiyordu ("Requested alpha
                    // mode PostMultiplied is not in the list of supported alpha
                    // modes: [Opaque]"). Auto, configure() içinde taze yetenek
                    // listesine karşı çözümlendiği için doğrulamadan geçemez hale
                    // gelmez.
                    alpha_mode: wgpu::CompositeAlphaMode::Auto,
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
            "r8unorm" => wgpu::TextureFormat::R8Unorm,
            "rg8unorm" => wgpu::TextureFormat::Rg8Unorm,
            "rgba8unorm" => wgpu::TextureFormat::Rgba8Unorm,
            "rgba8unorm-srgb" => wgpu::TextureFormat::Rgba8UnormSrgb,
            "bgra8unorm" => wgpu::TextureFormat::Bgra8Unorm,
            "bgra8unorm-srgb" => wgpu::TextureFormat::Bgra8UnormSrgb,
            "r16float" => wgpu::TextureFormat::R16Float,
            "rg16float" => wgpu::TextureFormat::Rg16Float,
            "rgba16float" => wgpu::TextureFormat::Rgba16Float,
            "r32float" => wgpu::TextureFormat::R32Float,
            "rg32float" => wgpu::TextureFormat::Rg32Float,
            "rgba32float" => wgpu::TextureFormat::Rgba32Float,
            "r32uint" => wgpu::TextureFormat::R32Uint,
            "rg32uint" => wgpu::TextureFormat::Rg32Uint,
            "rgba32uint" => wgpu::TextureFormat::Rgba32Uint,
            "rgb10a2unorm" => wgpu::TextureFormat::Rgb10a2Unorm,
            "rg11b10ufloat" => wgpu::TextureFormat::Rg11b10Float,
            "depth24plus" => wgpu::TextureFormat::Depth24Plus,
            "depth32float" => wgpu::TextureFormat::Depth32Float,
            other => return Err(format!("Unsupported/unrecognized texture format: {}", other)),
        })
    }

    fn parse_filter_mode(s: Option<&str>) -> wgpu::FilterMode {
        match s {
            Some("nearest") => wgpu::FilterMode::Nearest,
            Some("linear") => wgpu::FilterMode::Linear,
            _ => wgpu::FilterMode::Linear,
        }
    }

    fn parse_address_mode(s: Option<&str>) -> wgpu::AddressMode {
        match s {
            Some("repeat") => wgpu::AddressMode::Repeat,
            Some("mirror-repeat") => wgpu::AddressMode::MirrorRepeat,
            _ => wgpu::AddressMode::ClampToEdge,
        }
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
        pub is_software_adapter: bool,
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
    pub async fn gpu_request_adapter(_app: tauri::AppHandle, _window: WebviewWindow) -> Result<AdapterInfo, String> {
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
        _mip_level_count: Option<u32>,
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
    pub async fn gpu_create_sampler(_id: u64, _descriptor: Option<serde_json::Value>) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_create_shader_module(_id: u64, _code: String) -> Result<Option<String>, String> {
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
        _vertex_buffers: Option<serde_json::Value>,
        _topology: Option<String>,
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
    pub async fn gpu_canvas_configure(_context_id: u64, _format: String) -> Result<String, String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_canvas_get_current_texture(_context_id: u64, _view_id: Option<u64>) -> Result<u64, String> {
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

    // ── Yeni komutların Linux dışı stub'ları ────────────────────────────

    #[tauri::command]
    pub async fn gpu_queue_on_submitted_work_done() -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub fn gpu_write_buffer_bin(_request: tauri::ipc::Request<'_>) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub fn gpu_write_texture_bin(_request: tauri::ipc::Request<'_>) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_buffer_read_bin(_buffer_id: u64, _offset: u64, _size: u64) -> Result<tauri::ipc::Response, String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_import_video_frame(_texture_id: u64) -> Result<Option<[u32; 2]>, String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_destroy_resource(_kind: String, _id: u64) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_push_error_scope(_filter: String) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_pop_error_scope() -> Result<Option<String>, String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_reset_bridge() -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_encoder_set_vertex_buffer(_encoder_id: u64, _slot: u32, _buffer_id: u64, _offset: Option<u64>) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_encoder_set_index_buffer(_encoder_id: u64, _buffer_id: u64, _format: String, _offset: Option<u64>) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }

    #[tauri::command]
    pub async fn gpu_encoder_draw_indexed(_encoder_id: u64, _index_count: u32, _instance_count: u32) -> Result<(), String> {
        Err("WebGPU bridge is only supported on Linux".to_string())
    }
}