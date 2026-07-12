// ═══════════════════════════════════════════════════════════════════════════════
// gpu/wgpu_fb/fallback.rs — wgpu Fallback Aktivasyonu
//
// Browser WebGPU (navigator.gpu) kullanılamıyorsa, Rust tarafındaki wgpu
// renderer otomatik olarak devreye girer. Bu modül, bu geçişi yönetir.
//
// Frontend tarafından "openanime://webgpu-fallback-needed" eventi alındığında
// bu modül çağrılır ve "openanime://webgpu-fallback-active" eventi emit eder.
// ═══════════════════════════════════════════════════════════════════════════════

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Fallback'in aktif olup olmadığını atomik olarak takip eder.
/// Birden fazla çağrıda gereksiz tekrar engellenir.
static FALLBACK_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Fallback'in defalarca başlatılmasını önleyen guard.
static FALLBACK_INIT: OnceLock<()> = OnceLock::new();

/// wgpu fallback'in şu anda aktif olup olmadığını döndürür.
pub fn is_fallback_active() -> bool {
    FALLBACK_ACTIVE.load(Ordering::Relaxed)
}

/// wgpu fallback'i aktive eder. İdempotent — birden fazla çağrı güvenlidir.
/// Tauri AppHandle üzerinden frontend'e "openanime://webgpu-fallback-active" eventi emit eder.
pub fn activate_fallback(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Emitter;

    // Zaten aktifse tekrar başlatma
    if FALLBACK_ACTIVE.load(Ordering::Relaxed) {
        return Ok(());
    }

    FALLBACK_INIT.get_or_init(|| {
        println!("[wgpu Fallback] Browser WebGPU kullanılamıyor — Rust wgpu bridge fallback aktive ediliyor");
        FALLBACK_ACTIVE.store(true, Ordering::Relaxed);
    });

    // Frontend'e bildir — JS tarafı buna göre davranır
    app.emit("openanime://webgpu-fallback-active", serde_json::json!({
        "active": true,
        "reason": "Browser WebGPU desteklenmiyor, wgpu bridge kullanılıyor"
    })).map_err(|e| format!("Fallback event emit hatası: {}", e))?;

    println!("[wgpu Fallback] ✓ Fallback aktive edildi ve frontend'e bildirildi");
    Ok(())
}

/// Fallback'i deaktive eder (örn. driver güncellendikten sonra).
pub fn deactivate_fallback(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Emitter;

    if !FALLBACK_ACTIVE.load(Ordering::Relaxed) {
        return Ok(());
    }

    FALLBACK_ACTIVE.store(false, Ordering::Relaxed);

    app.emit("openanime://webgpu-fallback-active", serde_json::json!({
        "active": false,
        "reason": "wgpu bridge deaktive edildi"
    })).map_err(|e| format!("Fallback deactivate event hatası: {}", e))?;

    Ok(())
}

/// Tauri komutu: frontend'den fallback durumunu sorgular.
#[tauri::command]
pub async fn gpu_fallback_status() -> bool {
    is_fallback_active()
}

/// Tauri komutu: frontend'den fallback'i zorla aktive eder.
#[tauri::command]
pub async fn gpu_activate_fallback(app: tauri::AppHandle) -> Result<(), String> {
    activate_fallback(&app)
}
