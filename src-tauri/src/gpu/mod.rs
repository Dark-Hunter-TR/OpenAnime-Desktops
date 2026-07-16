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
pub mod wgpu_fb;
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
    #[cfg(target_os = "windows")]
    {
        compute_windows_report().await
    }

    #[cfg(target_os = "macos")]
    {
        compute_macos_report().await
    }

    // Linux desteği kaldırıldı; Windows/macOS dışındaki her hedefte boş rapor.
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        empty_report()
    }
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
