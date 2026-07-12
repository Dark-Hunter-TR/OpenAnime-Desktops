use std::sync::Arc;
use std::sync::OnceLock;
use wgpu::{Adapter, Device, Queue, DeviceDescriptor, Features, Limits};

// Device-lost kurtarması için sıfırlanabilir paylaşımlı device/queue.
// (Önceden OnceCell idi — reset imkânı olmadığından device kaybında
// uygulama ömrü boyunca ölü device'a saplanıp kalıyordu.)
static SHARED_DEVICE_QUEUE: std::sync::Mutex<Option<(Arc<Device>, Arc<Queue>)>> =
    std::sync::Mutex::new(None);

// Uygulama handle'ı: device hata/lost callback'lerinin frontend'e event
// yayınlayabilmesi için setup sırasında kaydedilir.
static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

pub fn set_app_handle(handle: tauri::AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

fn emit_gpu_event(event: &str, payload: String) {
    if let Some(handle) = APP_HANDLE.get() {
        use tauri::Emitter;
        let _ = handle.emit(event, payload);
    }
}

/// Creates the WGPU device and queue from the selected adapter.
/// Requests advanced features like SHADER_F16 and BGRA8UNORM_STORAGE if supported.
/// Caches the result in a thread-safe static OnceCell so both the renderer and the WebGPU bridge
/// share the exact same device and queue instance.
pub async fn create_device_and_queue(adapter: &Adapter) -> Result<(Arc<Device>, Arc<Queue>), String> {
    // Hızlı yol: zaten oluşturulmuşsa onu döndür.
    if let Some(shared) = SHARED_DEVICE_QUEUE.lock().unwrap_or_else(|p| p.into_inner()).clone() {
        return Ok(shared);
    }

    let supported_features = adapter.features();
    let mut required_features = Features::empty();

    // Enable f16 support in shaders if available for faster compute upscaling
    if supported_features.contains(Features::SHADER_F16) {
        required_features |= Features::SHADER_F16;
    }

    // Enable BGRA8unorm storage texture support if available (highly common on Linux/Vulkan)
    if supported_features.contains(Features::BGRA8UNORM_STORAGE) {
        required_features |= Features::BGRA8UNORM_STORAGE;
    }

    let (device, queue) = adapter
        .request_device(
            &DeviceDescriptor {
                label: Some("OpenAnime Video Renderer Device"),
                required_features,
                required_limits: Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        )
        .await
        .map_err(|e| format!("Failed to create WGPU Device: {}", e))?;

    // wgpu'nun varsayılan davranışı doğrulama hatalarında panic'tir;
    // bunun yerine logla + frontend'e event yayınla ki uygulama yaşasın.
    device.on_uncaptured_error(Box::new(|error| {
        let msg = format!("[WebGPU] Uncaptured error: {}", error);
        crate::log!("{}", msg);
        emit_gpu_event("openanime://webgpu-uncaptured-error", msg);
    }));

    device.set_device_lost_callback(|reason, message| {
        let msg = format!("[WebGPU] Device lost ({:?}): {}", reason, message);
        crate::log!("{}", msg);
        emit_gpu_event("openanime://webgpu-device-lost", msg);
    });

    println!("[WebGPU Renderer] Created WGPU Device and Queue successfully.");
    let created = (Arc::new(device), Arc::new(queue));

    // Yarış durumunda ilk yazan kazanır; bizimki boşa gitse de sorun değil.
    let mut slot = SHARED_DEVICE_QUEUE.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(existing) = slot.clone() {
        return Ok(existing);
    }
    *slot = Some(created.clone());
    Ok(created)
}

/// Device-lost kurtarması: paylaşılan device/queue'yu düşürür; sonraki
/// create_device_and_queue() çağrısı sıfırdan kurar.
pub fn reset_shared_device() {
    let mut slot = SHARED_DEVICE_QUEUE.lock().unwrap_or_else(|p| p.into_inner());
    *slot = None;
}
