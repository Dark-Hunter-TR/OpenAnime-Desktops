use std::sync::Arc;
use wgpu::{Adapter, Instance, RequestAdapterOptions, PowerPreference};

/// Selects the best Vulkan adapter.
/// Prioritizes: SUNUM YAPABİLEN adapter (>varsa probe surface) > Vulkan > Discrete.
/// Hibrit PRIME sistemlerde pencere iGPU'da yaşar; sunum yapamayan dGPU'yu
/// seçmek "pipeline çalışıyor ama görüntü yok" üretir.
pub async fn select_adapter(
    instance: &Instance,
    probe_surface: Option<&wgpu::Surface<'_>>,
) -> Result<Arc<Adapter>, String> {
    // We target Vulkan and GL for Linux rendering
    let adapters = instance.enumerate_adapters(wgpu::Backends::VULKAN | wgpu::Backends::GL);

    let best_adapter = adapters
        .into_iter()
        .max_by_key(|a| {
            let info = a.get_info();
            let present_score = probe_surface
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
        });

    if let Some(adapter) = best_adapter {
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu {
            return Err("Software adapter (CPU/llvmpipe) is not supported for high-performance rendering".to_string());
        }
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
