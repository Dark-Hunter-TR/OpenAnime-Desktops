// ═══════════════════════════════════════════════════════════════════════════════
// gpu/mod.rs — GPU Tanılama Sistemi Ana Modülü
//
// Bu modül tüm alt modülleri birleştirerek:
//   1. FullGpuReport üretir (GpuSession aracılığıyla session-lifetime cache)
//   2. Linux'a özgü env var yönetimini yapar (NVIDIA/Wayland workaround'lar)
//   3. Tauri komutlarını expose eder
//
// Session cache: OnceLock<Mutex<FullGpuReport>>
//   • Başarılı rapor → session boyunca korunur
//   • Başarısız rapor → 60s TTL sonrası yeniden denenir
//
// ═══════════════════════════════════════════════════════════════════════════════

pub mod diagnostics;
pub mod backend;
pub mod vulkan;
// webgpu/checker.rs doğrudan wgpu API'sini kullanır; wgpu yalnızca Linux
// target'ında bağımlı olduğundan bu modül de Linux'a özgüdür.
#[cfg(target_os = "linux")]
pub mod webgpu;
pub mod wgpu_fb;
pub mod linux;
pub mod windows;
pub mod macos;

pub use diagnostics::types::*;
pub use diagnostics::report::*;
// Glob re-export: #[tauri::command] makrosunun ürettiği gizli __cmd__* öğeleri
// de dahil edilir; böylece lib.rs'teki generate_handler! bunları
// `gpu::gpu_fallback_status` yolundan bulabilir (isimli re-export bunu taşımaz).
pub use wgpu_fb::fallback::*;

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

// ─────────────────────────────────────────────────────────────────────────────
// Session Cache
// ─────────────────────────────────────────────────────────────────────────────

/// Başarısız raporlar için yeniden deneme süresi (60 saniye).
const FAILURE_RETRY_TTL_SECS: u64 = 60;

struct CacheEntry {
    report: FullGpuReport,
    computed_at: Instant,
    is_success: bool,
}

static GPU_CACHE: OnceLock<Mutex<Option<CacheEntry>>> = OnceLock::new();

fn gpu_cache() -> &'static Mutex<Option<CacheEntry>> {
    GPU_CACHE.get_or_init(|| Mutex::new(None))
}

// ─────────────────────────────────────────────────────────────────────────────
// Ana API
// ─────────────────────────────────────────────────────────────────────────────

/// GPU raporunu döndürür. Cache geçerliyse cache'den, değilse hesaplayarak.
/// Thread-safe, hata toleranslı.
pub async fn get_gpu_report() -> FullGpuReport {
    // Cache kontrolü
    {
        let cache = gpu_cache()
            .lock()
            .unwrap_or_else(|p| p.into_inner());

        if let Some(entry) = cache.as_ref() {
            if entry.is_success {
                // Başarılı rapor → session boyunca geçerli
                return entry.report.clone();
            }
            // Başarısız rapor → TTL kontrolü
            let elapsed = entry.computed_at.elapsed().as_secs();
            if elapsed < FAILURE_RETRY_TTL_SECS {
                return entry.report.clone();
            }
            println!("[GPU] Başarısız rapor TTL doldu ({} sn), yeniden hesaplanıyor...", elapsed);
        }
    }

    // Raporu hesapla
    let report = compute_full_report().await;
    let is_success = report.critical_error.is_none() && report.vulkan_status.is_ok();

    // Cache'e kaydet
    {
        let mut cache = gpu_cache()
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        *cache = Some(CacheEntry {
            report: report.clone(),
            computed_at: Instant::now(),
            is_success,
        });
    }

    report
}

/// Cache'i geçersiz kılar (test/debug için).
pub fn invalidate_gpu_cache() {
    let mut cache = gpu_cache()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    *cache = None;
    println!("[GPU] Cache geçersiz kılındı");
}

// ─────────────────────────────────────────────────────────────────────────────
// Platform Rapor Hesaplayıcı
// ─────────────────────────────────────────────────────────────────────────────

async fn compute_full_report() -> FullGpuReport {
    #[cfg(target_os = "linux")]
    {
        compute_linux_report().await
    }

    #[cfg(target_os = "windows")]
    {
        compute_windows_report().await
    }

    #[cfg(target_os = "macos")]
    {
        compute_macos_report().await
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        empty_report()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Linux Raporu
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
async fn compute_linux_report() -> FullGpuReport {
    println!("[GPU] Linux GPU raporu hesaplanıyor...");

    // 1. Platform-spesifik algılama
    let detection = linux::detector::detect();

    // 2. Vulkan probe
    let vulkan_probe = vulkan::run_vulkan_probe().await;

    // 3. Backend seçimi
    let best_backend = backend::select_best_backend().await;

    // 4. wgpu adapter bilgileri (varsa)
    let (adapter_name, webgpu_info, webgpu_status) = try_get_wgpu_adapter_info(&best_backend).await;

    // 5. Raporu birleştir
    let mut report = empty_report();

    // Temel bilgiler
    let refined_vendor = linux::detector::refine_vendor_from_renderer(
        detection.vendor.clone(),
        &detection.opengl_renderer.clone().unwrap_or_default(),
    );
    report.vendor = refined_vendor.as_str().to_string();
    report.vendor_enum = refined_vendor.clone();
    report.renderer = detection.renderer.clone();
    report.driver_version = detection.driver_version.clone();
    report.mesa_version = detection.mesa_version.clone();
    report.opengl_renderer = detection.opengl_renderer.clone();
    report.opengl_version = detection.opengl_version.clone();
    report.display_server = detection.display_server.clone();
    report.pci_vendor_id = detection.pci_vendor_id.clone();
    report.pci_device_id = detection.pci_device_id.clone();

    // NVIDIA spesifik
    report.nvidia_driver_version = detection.nvidia_driver_version.clone();
    report.nvidia_is_proprietary = detection.nvidia_is_proprietary;
    report.nvidia_is_nouveau = detection.nvidia_is_nouveau;

    // AMD spesifik
    report.amd_driver = detection.amd_driver.clone();

    // Intel spesifik
    report.intel_driver = detection.intel_driver.clone();

    // Hardware acceleration
    report.vaapi_supported = detection.vaapi_supported;
    report.dmabuf_supported = detection.dmabuf_supported;
    report.hw_accel = !matches!(best_backend, GpuBackend::Software);
    report.video_decode = detection.vaapi_supported;
    report.video_encode = detection.vaapi_supported;

    // Paket yönetimi
    report.pkg_manager = detection.pkg_manager.clone();
    report.has_pkexec = detection.has_pkexec;

    // Backend
    report.backend = best_backend.clone();

    // Vulkan
    apply_vulkan_probe(&mut report, &vulkan_probe);

    // WebGPU
    if let Some(name) = adapter_name {
        report.adapter_name = Some(name);
    }
    if let Some(info) = webgpu_info {
        report.webgpu_status = webgpu_status;
        report.msaa_supported = info.msaa_x4;
        report.compute_shader = true;
        report.timestamp_query = info.timestamp_query;
        report.max_texture_dimension = Some(info.max_texture_dimension_2d);
        report.max_storage_buffer_binding_size = Some(info.max_storage_buffer_binding_size);
        report.max_compute_workgroup_size = Some(info.max_compute_workgroup_size_x);
    } else {
        report.webgpu_status = WebGpuStatus::BridgeOk; // wgpu bridge üzerinden çalışıyor
    }

    // Paket önerileri (sadece Vulkan yoksa)
    if !vulkan_probe.status.is_ok() {
        let (pkgs, cmd) = build_install_command(&detection.pkg_manager, &refined_vendor);
        report.recommended_packages = pkgs;
        report.recommended_command = cmd;
        report.critical_error = Some(vulkan_probe.status.error_message().to_string());
    }

    // Log birleştir
    report.init_log.extend(detection.log);

    // Özet log
    println!(
        "[GPU] Rapor tamamlandı: vendor={}, backend={}, vulkan={:?}, dmabuf={}",
        report.vendor, report.backend, report.vulkan_status, report.dmabuf_supported
    );

    report
}

// ─────────────────────────────────────────────────────────────────────────────
// Windows Raporu
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
async fn compute_windows_report() -> FullGpuReport {
    // Windows'ta GPU tanısı wgpu'suz yapılır (wgpu yalnızca Linux target'ında bağımlı).
    // Backend seçimi ve adapter enumeration burada stub'dur; asıl WebGPU yolu
    // WebView2'nin (Chromium) native WebGPU desteği üzerinden yürür.
    let detection = windows::detector::detect();
    let best_backend = backend::select_best_backend().await;

    let mut report = empty_report();
    report.vendor = detection.vendor.as_str().to_string();
    report.vendor_enum = detection.vendor;
    report.renderer = detection.renderer;
    report.driver_version = detection.driver_version;
    report.display_server = detection.display_server;
    report.backend = best_backend;
    report.hw_accel = report.backend.is_hardware();
    // WebView2 (Chromium) native WebGPU sağlar; kesin durum Faz 4'te frontend'den doldurulur.
    report.webgpu_status = WebGpuStatus::NativeOk;
    report.init_log.extend(detection.log);
    report
}

// ─────────────────────────────────────────────────────────────────────────────
// macOS Raporu
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
async fn compute_macos_report() -> FullGpuReport {
    // macOS'ta GPU tanısı wgpu'suz yapılır (wgpu yalnızca Linux target'ında bağımlı).
    // Render yolu WKWebView üzerinden yürür; WKWebView'de WebGPU 2026 itibarıyla
    // hâlâ deneysel olduğundan durum "Unsupported" varsayılır, Faz 4'te güncellenebilir.
    let detection = macos::detector::detect();
    let best_backend = backend::select_best_backend().await;

    let mut report = empty_report();
    report.vendor = detection.vendor.as_str().to_string();
    report.vendor_enum = detection.vendor;
    report.renderer = detection.renderer;
    report.driver_version = detection.driver_version;
    report.display_server = detection.display_server;
    report.backend = best_backend;
    report.hw_accel = report.backend.is_hardware();
    report.webgpu_status = WebGpuStatus::Unsupported;
    report.init_log.extend(detection.log);
    report
}

// ─────────────────────────────────────────────────────────────────────────────
// wgpu Adapter Bilgisi (her platformda kullanılabilir)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
async fn try_get_wgpu_adapter_info(
    backend: &GpuBackend,
) -> (Option<String>, Option<webgpu::checker::WebGpuAdapterInfo>, WebGpuStatus) {
    let wgpu_backends = match backend {
        GpuBackend::Vulkan => wgpu::Backends::VULKAN,
        GpuBackend::OpenGL => wgpu::Backends::GL,
        GpuBackend::Metal => wgpu::Backends::METAL,
        GpuBackend::Direct3D12 => wgpu::Backends::DX12,
        GpuBackend::Direct3D11 => wgpu::Backends::DX11,
        _ => wgpu::Backends::all(),
    };

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu_backends,
        ..Default::default()
    });

    let adapter_opt = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await;

    match adapter_opt {
        Some(adapter) => {
            let info = webgpu::checker::extract_webgpu_info(&adapter);
            let status = webgpu::checker::determine_webgpu_status(&info);
            let name = info.name.clone();
            (Some(name), Some(info), status)
        }
        None => {
            // Software fallback dene
            let fallback = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::None,
                    force_fallback_adapter: true,
                    compatible_surface: None,
                })
                .await;

            match fallback {
                Some(adapter) => {
                    let info = webgpu::checker::extract_webgpu_info(&adapter);
                    let name = info.name.clone();
                    (Some(name), Some(info), WebGpuStatus::SoftwareFallback)
                }
                None => (None, None, WebGpuStatus::AdapterFailed),
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Linux Environment Variables — GPU'ya göre otomatik ayar
// ─────────────────────────────────────────────────────────────────────────────

/// Linux'ta GPU'ya ve display server'a göre WebKit/DRM ortam değişkenlerini ayarlar.
/// Bu fonksiyon `run()` başlamadan önce çağrılmalı (Tauri setup öncesi).
#[cfg(target_os = "linux")]
pub fn configure_linux_gpu_env() {
    println!("[GPU Env] Linux GPU ortam değişkenleri yapılandırılıyor...");

    let detection = linux::detector::detect();
    let is_wayland = matches!(detection.display_server, DisplayServer::Wayland);

    // ── Vulkan desteği yoksa WebKit compositing'i kapat ─────────────────────
    // Asenkron Vulkan probe'u burada çalıştırmak maliyetli olduğundan
    // sadece ICD dosyası varlığına bakarak hızlı karar veririz.
    let vulkan_available = !linux::detector::find_vulkan_icd_files().is_empty()
        && (std::path::Path::new("/usr/lib/libvulkan.so.1").exists()
            || std::path::Path::new("/usr/lib/x86_64-linux-gnu/libvulkan.so.1").exists()
            || std::path::Path::new("/usr/lib64/libvulkan.so.1").exists());

    if !vulkan_available {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        println!("[GPU Env] Vulkan yok — WEBKIT_DISABLE_COMPOSITING_MODE=1 ayarlandı");
    }

    // ── NVIDIA spesifik workaround'lar ─────────────────────────────────────
    let is_nvidia = matches!(detection.vendor, GpuVendor::Nvidia);

    if is_nvidia {
        // DMA-BUF: NVIDIA < 555 sürümde DMA-BUF sorunlu
        if !detection.dmabuf_supported {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            println!("[GPU Env] NVIDIA (eski driver) — WEBKIT_DISABLE_DMABUF_RENDERER=1 ayarlandı");
        }

        // Wayland + NVIDIA: explicit sync workaround (driver < 555)
        if is_wayland && !detection.dmabuf_supported {
            std::env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1");
            println!("[GPU Env] Wayland + NVIDIA (eski) — __NV_DISABLE_EXPLICIT_SYNC=1 ayarlandı");
        }

        // NVIDIA proprietary + Wayland: GBM backend zorla
        if is_wayland && detection.nvidia_is_proprietary {
            // nvidia sürücüsü 525+ sürümde GBM destekliyor
            if let Some(ref ver_str) = detection.nvidia_driver_version {
                let major: u32 = ver_str.split('.').next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                if major >= 525 {
                    std::env::set_var("GBM_BACKEND", "nvidia-drm");
                    std::env::set_var("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
                    println!("[GPU Env] NVIDIA {} Wayland GBM backend ayarlandı", ver_str);
                }
            }
        }
    }

    // ── VirtIO GPU (VM ortamı) ──────────────────────────────────────────────
    if matches!(detection.vendor, GpuVendor::VirtIo | GpuVendor::Vmware) {
        // VM'de hardware compositing sorunlu olabilir
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        println!("[GPU Env] VM GPU tespit edildi — WEBKIT_DISABLE_COMPOSITING_MODE=1 ayarlandı");
    }

    println!("[GPU Env] ✓ Yapılandırma tamamlandı (vendor={}, wayland={}, dmabuf={})",
        detection.vendor, is_wayland, detection.dmabuf_supported);
}

#[cfg(not(target_os = "linux"))]
pub fn configure_linux_gpu_env() {
    // Linux dışı platformlarda hiçbir şey yapma
}

// ─────────────────────────────────────────────────────────────────────────────
// Tauri Komutları
// ─────────────────────────────────────────────────────────────────────────────

/// Tam GPU raporunu döndürür. Session cache ile korunur.
#[tauri::command]
pub async fn gpu_full_report() -> FullGpuReport {
    get_gpu_report().await
}

/// GPU Vulkan durumunu döndürür (sadece Vulkan kısmı — daha hızlı).
#[tauri::command]
pub async fn gpu_vulkan_status() -> serde_json::Value {
    let report = get_gpu_report().await;
    serde_json::json!({
        "status": report.vulkan_status,
        "steps": report.vulkan_steps,
        "icd_files": report.vulkan_icd_files,
        "version": report.vulkan_version,
        "error_message": report.vulkan_status.error_message(),
        "fix_hint": report.vulkan_status.fix_hint(),
    })
}

/// Seçilen backend bilgisini döndürür.
#[tauri::command]
pub async fn gpu_backend_info() -> serde_json::Value {
    let report = get_gpu_report().await;
    serde_json::json!({
        "backend": report.backend,
        "is_hardware": report.backend.is_hardware(),
        "adapter_name": report.adapter_name,
        "vendor": report.vendor,
    })
}

/// Cache'i geçersiz kılarak raporu yeniden hesaplar.
/// Kullanıcı driver kurulumu yaptıktan sonra çağrılır.
#[tauri::command]
pub async fn gpu_refresh_report() -> FullGpuReport {
    invalidate_gpu_cache();
    get_gpu_report().await
}
