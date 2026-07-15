// ═══════════════════════════════════════════════════════════════════════════════
// gpu/backend/selector.rs — Platform-aware GPU backend seçici
//
// Her platformda mevcut backend'leri sırayla dener ve başarılı olanı seçer.
// Sonuç session boyunca cache'lenir (OnceLock).
//
// Sıralama:
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
    #[cfg(target_os = "windows")]
    {
        return select_windows_backend().await;
    }

    #[cfg(target_os = "macos")]
    {
        return select_macos_backend().await;
    }

    // Linux desteği kaldırıldı; Windows/macOS dışındaki hedeflerde Software.
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        GpuBackend::Software
    }
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
