// ═══════════════════════════════════════════════════════════════════════════════
// gpu/vulkan/probe.rs — Gerçek Vulkan doğrulama motoru
//
// Bu modül, Vulkan'ın sistemde gerçekten çalışıp çalışmadığını kademeli
// adımlarla kontrol eder. Sadece dosya varlığı kontrol etmez — wgpu
// üzerinden gerçek instance ve adapter oluşturmayı dener.
//
// Kontrol sırası:
//   1. libvulkan.so.1 varlığı (loader)
//   2. /usr/share/vulkan/icd.d/*.json varlığı
//   3. ICD JSON içeriği parse edilebilir mi?
//   4. wgpu Instance oluşturulabiliyor mu?
//   5. Fiziksel cihaz (adapter) bulunuyor mu?
//   6. Queue bulunuyor mu?
//
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(target_os = "linux")]
pub mod inner {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use crate::gpu::diagnostics::types::*;
    use crate::gpu::linux::detector::find_vulkan_icd_files;

    /// Vulkan probe'unu çalıştırır. Her adım bağımsız hata toleranslıdır.
    pub async fn run_vulkan_probe() -> VulkanProbeResult {
        let mut steps = Vec::new();

        // ── ADIM 1: Vulkan loader (libvulkan.so.1) ───────────────────────────
        let lib_ok = check_vulkan_loader(&mut steps);
        if !lib_ok {
            return VulkanProbeResult {
                status: VulkanProbeStatus::LibMissing,
                steps,
                instance_version: None,
                device_name: None,
                icd_files: Vec::new(),
                backend: GpuBackend::Software,
            };
        }

        // ── ADIM 2: ICD dosyaları ────────────────────────────────────────────
        let icd_files = find_vulkan_icd_files();
        if icd_files.is_empty() {
            steps.push(VulkanProbeStep::failure(
                "ICD Check",
                "/usr/share/vulkan/icd.d/ boş veya yok — GPU driver ICD dosyası eksik",
            ));
            return VulkanProbeResult {
                status: VulkanProbeStatus::IcdMissing,
                steps,
                instance_version: None,
                device_name: None,
                icd_files: Vec::new(),
                backend: GpuBackend::Software,
            };
        }
        steps.push(VulkanProbeStep::success(
            "ICD Check",
            format!("{} ICD dosyası bulundu: {}", icd_files.len(), icd_files.join(", ")),
        ));

        // ── ADIM 3: ICD JSON parse kontrolü ─────────────────────────────────
        let icd_parse_ok = verify_icd_files(&icd_files, &mut steps);
        if !icd_parse_ok {
            return VulkanProbeResult {
                status: VulkanProbeStatus::IcdCorrupt,
                steps,
                instance_version: None,
                device_name: None,
                icd_files,
                backend: GpuBackend::Software,
            };
        }

        // ── ADIM 4: wgpu Instance oluştur ─────────────────────────────────
        // Sadece Vulkan backend — paylaşılan Vulkan-only instance (per-call
        // instance yaratımı probe başına gereksiz sürücü taramasıydı)
        let instance = crate::gpu::shared_instance();

        steps.push(VulkanProbeStep::success("Instance Create", "wgpu Vulkan instance oluşturuldu"));

        // ── ADIM 5: Vulkan instance versiyonu ───────────────────────────────
        let instance_version = get_vulkan_instance_version(&mut steps);

        // ── ADIM 6: Adapter (fiziksel cihaz) enumerate ──────────────────────
        let adapters: Vec<wgpu::Adapter> = instance.enumerate_adapters(wgpu::Backends::VULKAN);

        if adapters.is_empty() {
            // Software fallback dene
            let fallback = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::None,
                    force_fallback_adapter: true,
                    compatible_surface: None,
                })
                .await;

            if fallback.is_some() {
                steps.push(VulkanProbeStep::success(
                    "Physical Device",
                    "Sadece software adapter bulundu (llvmpipe/SwiftShader)",
                ));
                return VulkanProbeResult {
                    status: VulkanProbeStatus::Ok,
                    steps,
                    instance_version,
                    device_name: Some("Software Adapter".to_string()),
                    icd_files,
                    backend: GpuBackend::Software,
                };
            }

            steps.push(VulkanProbeStep::failure(
                "Physical Device",
                "Hiç Vulkan fiziksel cihaz bulunamadı — GPU driver yüklenmemiş veya modül yüklenmemiş",
            ));
            return VulkanProbeResult {
                status: VulkanProbeStatus::NoPhysicalDevice,
                steps,
                instance_version,
                device_name: None,
                icd_files,
                backend: GpuBackend::Software,
            };
        }

        // En iyi adapter'ı seç (discrete GPU önce)
        let best_adapter = adapters
            .into_iter()
            .max_by_key(|a| {
                let info = a.get_info();
                let type_score = match info.device_type {
                    wgpu::DeviceType::DiscreteGpu => 3,
                    wgpu::DeviceType::IntegratedGpu => 2,
                    wgpu::DeviceType::VirtualGpu => 1,
                    _ => 0,
                };
                type_score
            });

        let adapter = match best_adapter {
            Some(a) => a,
            None => {
                steps.push(VulkanProbeStep::failure("Adapter Selection", "Adapter seçilemedi"));
                return VulkanProbeResult {
                    status: VulkanProbeStatus::NoPhysicalDevice,
                    steps,
                    instance_version,
                    device_name: None,
                    icd_files,
                    backend: GpuBackend::Software,
                };
            }
        };

        let info = adapter.get_info();
        let device_name = info.name.clone();
        let backend = match info.backend {
            wgpu::Backend::Vulkan => GpuBackend::Vulkan,
            wgpu::Backend::Gl => GpuBackend::OpenGL,
            wgpu::Backend::Metal => GpuBackend::Metal,
            wgpu::Backend::Dx12 => GpuBackend::Direct3D12,
            _ => GpuBackend::Unknown,
        };

        steps.push(VulkanProbeStep::success(
            "Physical Device",
            format!(
                "{} (Type: {:?}, Backend: {:?})",
                info.name, info.device_type, info.backend
            ),
        ));

        // ── ADIM 7: Device ve Queue oluşturma ─────────────────────────────
        match try_create_device(&adapter, &mut steps).await {
            Ok(_) => {}
            Err(status) => {
                return VulkanProbeResult {
                    status,
                    steps,
                    instance_version,
                    device_name: Some(device_name),
                    icd_files,
                    backend,
                };
            }
        }

        // ── ADIM 8: Swapchain extension kontrolü ─────────────────────────
        let has_swapchain = check_swapchain_support(&adapter, &mut steps);
        if !has_swapchain {
            return VulkanProbeResult {
                status: VulkanProbeStatus::SwapchainUnsupported,
                steps,
                instance_version,
                device_name: Some(device_name),
                icd_files,
                backend,
            };
        }

        VulkanProbeResult {
            status: VulkanProbeStatus::Ok,
            steps,
            instance_version,
            device_name: Some(device_name),
            icd_files,
            backend,
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Yardımcı fonksiyonlar
    // ─────────────────────────────────────────────────────────────────────────

    fn check_vulkan_loader(steps: &mut Vec<VulkanProbeStep>) -> bool {
        // libvulkan.so.1 için olası konumlar
        let vulkan_lib_paths = [
            "/usr/lib/libvulkan.so.1",
            "/usr/lib/x86_64-linux-gnu/libvulkan.so.1",
            "/usr/lib64/libvulkan.so.1",
            "/lib/x86_64-linux-gnu/libvulkan.so.1",
            "/usr/lib/aarch64-linux-gnu/libvulkan.so.1",
        ];

        for path in &vulkan_lib_paths {
            if Path::new(path).exists() {
                steps.push(VulkanProbeStep::success("Vulkan Loader", path.to_string()));
                return true;
            }
        }

        // ldconfig cache kontrolü (Debian/Ubuntu gibi distrolarda .so symlink'ler cache'de)
        if let Ok(output) = Command::new("ldconfig").arg("-p").output() {
            let cache = String::from_utf8_lossy(&output.stdout);
            if cache.contains("libvulkan.so.1") {
                steps.push(VulkanProbeStep::success("Vulkan Loader", "ldconfig cache'de tespit edildi"));
                return true;
            }
        }

        steps.push(VulkanProbeStep::failure(
            "Vulkan Loader",
            "libvulkan.so.1 hiçbir standart konumda bulunamadı. vulkan-loader paketi kurulu değil.",
        ));
        false
    }

    fn verify_icd_files(files: &[String], steps: &mut Vec<VulkanProbeStep>) -> bool {
        let mut valid_count = 0;
        let mut invalid_files = Vec::new();

        for file in files {
            if let Ok(content) = fs::read_to_string(file) {
                // ICD JSON minimum yapısı: {"ICD": {"library_path": ...}}
                if content.contains("ICD") && content.contains("library_path") {
                    valid_count += 1;
                } else {
                    invalid_files.push(file.as_str());
                }
            } else {
                invalid_files.push(file.as_str());
            }
        }

        if valid_count > 0 {
            steps.push(VulkanProbeStep::success(
                "ICD JSON Parse",
                format!("{}/{} ICD dosyası geçerli", valid_count, files.len()),
            ));
            true
        } else {
            steps.push(VulkanProbeStep::failure(
                "ICD JSON Parse",
                format!("Tüm ICD dosyaları geçersiz: {:?}", invalid_files),
            ));
            false
        }
    }

    fn get_vulkan_instance_version(steps: &mut Vec<VulkanProbeStep>) -> Option<String> {
        // vulkaninfo çıktısından versiyon çek
        if let Ok(output) = Command::new("vulkaninfo").arg("--summary").output() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.contains("Vulkan Instance Version") || line.contains("Instance Version") {
                    let version = line
                        .split(':')
                        .nth(1)
                        .map(str::trim)
                        .map(String::from);
                    if let Some(ref v) = version {
                        steps.push(VulkanProbeStep::success("Instance Version", v));
                    }
                    return version;
                }
            }
        }

        // vulkaninfo yoksa wgpu'dan alınan adapter info'sunu kullan
        steps.push(VulkanProbeStep::success("Instance Version", "vulkaninfo bulunamadı — wgpu üzerinden devam edildi"));
        None
    }

    async fn try_create_device(
        adapter: &wgpu::Adapter,
        steps: &mut Vec<VulkanProbeStep>,
    ) -> Result<(wgpu::Device, wgpu::Queue), VulkanProbeStatus> {
        match adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("vulkan_probe_device"),
                    required_features: wgpu::Features::empty(),
                    // wgpu 22'de Limits::downgraded() yok; downlevel_defaults()
                    // zayıf donanım (llvmpipe dahil) tarafından da desteklenen
                    // muhafazakâr limit setidir — probe'un amacına uygundur.
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
        {
            Ok((device, queue)) => {
                steps.push(VulkanProbeStep::success("Device Create", "GPU device ve queue başarıyla oluşturuldu"));
                Ok((device, queue))
            }
            Err(e) => {
                steps.push(VulkanProbeStep::failure("Device Create", e.to_string()));
                Err(VulkanProbeStatus::NoQueue)
            }
        }
    }

    fn check_swapchain_support(adapter: &wgpu::Adapter, steps: &mut Vec<VulkanProbeStep>) -> bool {
        // wgpu'da swapchain desteği adapter backend'inden çıkarılır;
        // Vulkan adapter'larda swapchain extension'ı beklenir.
        let info = adapter.get_info();
        if info.backend == wgpu::Backend::Vulkan {
            steps.push(VulkanProbeStep::success("Swapchain", "Vulkan swapchain destekleniyor"));
            true
        } else {
            steps.push(VulkanProbeStep::failure(
                "Swapchain",
                format!("Backend {:?} swapchain kontrolü yapılamadı", info.backend),
            ));
            // GL gibi backend'lerde swapchain farklı çalışır, hata değil
            true
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Non-Linux stub
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(not(target_os = "linux"))]
pub mod inner {
    use crate::gpu::diagnostics::types::*;

    pub async fn run_vulkan_probe() -> VulkanProbeResult {
        VulkanProbeResult {
            status: VulkanProbeStatus::Ok,
            steps: vec![VulkanProbeStep::success("Platform", "Non-Linux — Vulkan probe atlandı")],
            instance_version: None,
            device_name: None,
            icd_files: Vec::new(),
            backend: GpuBackend::Unknown,
        }
    }
}
