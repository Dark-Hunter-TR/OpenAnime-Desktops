use std::sync::Arc;
use wgpu::{Adapter, Instance, RequestAdapterOptions, PowerPreference};

/// Selects the best Vulkan adapter.
/// Prioritizes Discrete GPUs, then Integrated GPUs, and finally fallback CPU adapters.
pub async fn select_adapter(instance: &Instance) -> Result<Arc<Adapter>, String> {
    // We target Vulkan specifically for Linux rendering
    let adapters = instance.enumerate_adapters(wgpu::Backends::VULKAN);
    
    // Sort adapters so DiscreteGpu is preferred, IntegratedGpu next, then other types.
    let best_adapter = adapters
        .into_iter()
        .max_by_key(|a| {
            let info = a.get_info();
            match info.device_type {
                wgpu::DeviceType::DiscreteGpu => 3,
                wgpu::DeviceType::IntegratedGpu => 2,
                wgpu::DeviceType::Cpu => 0,
                _ => 1,
            }
        });

    if let Some(adapter) = best_adapter {
        let info = adapter.get_info();
        println!(
            "[WebGPU Renderer] Selected adapter: {} (Type: {:?}, Backend: {:?})",
            info.name, info.device_type, info.backend
        );
        return Ok(Arc::new(adapter));
    }

    // Fallback: request default adapter
    println!("[WebGPU Renderer] No Vulkan adapter enumerated. Requesting fallback adapter...");
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or_else(|| "Failed to find any compatible GPU adapter with Vulkan".to_string())?;

    let info = adapter.get_info();
    println!(
        "[WebGPU Renderer] Selected fallback adapter: {} (Backend: {:?})",
        info.name, info.backend
    );
    Ok(Arc::new(adapter))
}
