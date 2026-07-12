// ═══════════════════════════════════════════════════════════════════════════════
// gpu/backend/selector.rs — Platform-aware GPU backend seçici
//
// Her platformda mevcut backend'leri sırayla dener ve başarılı olanı seçer.
// Sonuç session boyunca cache'lenir (OnceLock).
//
// Sıralama:
//   Linux:   Vulkan → OpenGL → ANGLE → Software
//   Windows: D3D12 → D3D11 → Vulkan → OpenGL → Software
//   macOS:   Metal → Software
// ═══════════════════════════════════════════════════════════════════════════════

use std::sync::OnceLock;
use crate::gpu::diagnostics::types::*;

static SELECTED_BACKEND: OnceLock<GpuBackend> = OnceLock::new();

/// Platforma göre en iyi backend'i seçer. Sonuç session boyunca cache'lenir.
pub async fn select_best_backend() -> GpuBackend {
    if let Some(cached) = SELECTED_BACKEND.get() {
        return cached.clone();
    }

    let backend = determine_backend_for_platform().await;

    // OnceLock set başarısız olursa (başka thread zaten set ettiyse) mevcut değeri kullan
    let _ = SELECTED_BACKEND.set(backend.clone());
    SELECTED_BACKEND.get().cloned().unwrap_or(backend)
}

async fn determine_backend_for_platform() -> GpuBackend {
    #[cfg(target_os = "linux")]
    {
        return select_linux_backend().await;
    }

    #[cfg(target_os = "windows")]
    {
        return select_windows_backend().await;
    }

    #[cfg(target_os = "macos")]
    {
        return select_macos_backend().await;
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        GpuBackend::Software
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Linux: Vulkan → OpenGL → ANGLE → Software
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
async fn select_linux_backend() -> GpuBackend {
    // 1. Vulkan: ICD dosyaları var ve wgpu Vulkan adapter bulunabiliyorsa
    if try_vulkan_backend().await {
        println!("[GPU Backend] ✓ Vulkan backend seçildi");
        return GpuBackend::Vulkan;
    }
    println!("[GPU Backend] ✗ Vulkan başarısız, OpenGL deneniyor...");

    // 2. OpenGL: Mesa GL adapter bulunabiliyorsa
    if try_opengl_backend().await {
        println!("[GPU Backend] ✓ OpenGL backend seçildi");
        return GpuBackend::OpenGL;
    }
    println!("[GPU Backend] ✗ OpenGL başarısız, ANGLE deneniyor...");

    // 3. ANGLE: ANGLE loader mevcut mu?
    if try_angle_backend() {
        println!("[GPU Backend] ✓ ANGLE backend seçildi");
        return GpuBackend::Angle;
    }
    println!("[GPU Backend] ✗ ANGLE mevcut değil, Software kullanılıyor");

    // 4. Software: llvmpipe / SwiftShader
    GpuBackend::Software
}

#[cfg(target_os = "linux")]
async fn try_vulkan_backend() -> bool {
    use std::path::Path;

    // Hızlı kontrol: ICD dosyaları var mı?
    let icd_exists = Path::new("/usr/share/vulkan/icd.d").exists()
        && std::fs::read_dir("/usr/share/vulkan/icd.d")
            .map(|mut d| d.next().is_some())
            .unwrap_or(false);

    let lib_exists = [
        "/usr/lib/libvulkan.so.1",
        "/usr/lib/x86_64-linux-gnu/libvulkan.so.1",
        "/usr/lib64/libvulkan.so.1",
    ]
    .iter()
    .any(|p| Path::new(p).exists());

    if !icd_exists || !lib_exists {
        return false;
    }

    // wgpu Vulkan adapter dene
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });

    let adapters = instance.enumerate_adapters(wgpu::Backends::VULKAN);
    if !adapters.is_empty() {
        return true;
    }

    // Force request ile son şans
    instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .is_some()
}

#[cfg(target_os = "linux")]
async fn try_opengl_backend() -> bool {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::GL,
        ..Default::default()
    });

    instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .is_some()
}

#[cfg(target_os = "linux")]
fn try_angle_backend() -> bool {
    // ANGLE loader path'leri
    let angle_paths = [
        "/usr/lib/libEGL_angle.so",
        "/usr/lib/x86_64-linux-gnu/libEGL_angle.so",
        "/usr/lib/libGLESv2_angle.so",
    ];
    angle_paths.iter().any(|p| std::path::Path::new(p).exists())
}

// ─────────────────────────────────────────────────────────────────────────────
// Windows: D3D12 → D3D11 → Vulkan → OpenGL → Software
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
async fn select_windows_backend() -> GpuBackend {
    // Windows'ta wgpu bağımlılığı yoktur (yalnızca Linux target'ında derlenir).
    // WebView2 (Chromium) render yolu Direct3D 12 üzerinden yürür; bu yüzden
    // stub olarak D3D12 döndürülür. Gerçek adapter probe'u bu platformda yapılmaz.
    println!("[GPU Backend] Windows: Direct3D 12 (WebView2 varsayılan yolu) seçildi");
    GpuBackend::Direct3D12
}

// ─────────────────────────────────────────────────────────────────────────────
// macOS: Metal → Software
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
async fn select_macos_backend() -> GpuBackend {
    // macOS'ta wgpu bağımlılığı yoktur (yalnızca Linux target'ında derlenir).
    // WKWebView render yolu Metal üzerinden yürür; stub olarak Metal döndürülür.
    println!("[GPU Backend] macOS: Metal (WKWebView varsayılan yolu) seçildi");
    GpuBackend::Metal
}
