use std::sync::Arc;
use wgpu::{Adapter, Device, Queue, DeviceDescriptor, Features, Limits};
use tokio::sync::OnceCell;

static SHARED_DEVICE_QUEUE: OnceCell<(Arc<Device>, Arc<Queue>)> = OnceCell::const_new();

/// Creates the WGPU device and queue from the selected adapter.
/// Requests advanced features like SHADER_F16 and BGRA8UNORM_STORAGE if supported.
/// Caches the result in a thread-safe static OnceCell so both the renderer and the WebGPU bridge
/// share the exact same device and queue instance.
pub async fn create_device_and_queue(adapter: &Adapter) -> Result<(Arc<Device>, Arc<Queue>), String> {
    let shared = SHARED_DEVICE_QUEUE.get_or_try_init(|| async {
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

        println!("[WebGPU Renderer] Created WGPU Device and Queue successfully.");
        Ok::<_, String>((Arc::new(device), Arc::new(queue)))
    }).await?;

    Ok(shared.clone())
}
