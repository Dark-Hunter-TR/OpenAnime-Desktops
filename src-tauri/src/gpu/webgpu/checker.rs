// ═══════════════════════════════════════════════════════════════════════════════
// gpu/webgpu/checker.rs — WebGPU durum kontrolü ve özellik listesi
//
// Bu modül, wgpu adapter'ından WebGPU durum bilgilerini toplar.
// Frontend'den navigator.gpu kontrolü JS tarafında yapılır (gpu-diagnostics.js).
// Rust tarafında adapter features/limits raporlanır.
// ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use crate::gpu::diagnostics::types::*;

/// wgpu adapter'ından elde edilen WebGPU durum özeti.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebGpuAdapterInfo {
    pub name: String,
    pub vendor: u32,
    pub device: u32,
    pub device_type: String,
    pub backend: String,
    pub is_software: bool,
    // Özellikler
    pub msaa_x4: bool,
    pub msaa_x8: bool,
    pub timestamp_query: bool,
    pub pipeline_statistics_query: bool,
    pub texture_compression_bc: bool,
    pub texture_compression_etc2: bool,
    pub indirect_first_instance: bool,
    pub shader_f64: bool,
    pub multi_draw_indirect: bool,
    // Limitler
    pub max_texture_dimension_2d: u32,
    pub max_storage_buffer_binding_size: u64,
    pub max_compute_workgroup_size_x: u32,
    pub max_compute_workgroup_size_y: u32,
    pub max_compute_workgroup_size_z: u32,
    pub max_bind_groups: u32,
    pub max_color_attachments: u32,
}

/// wgpu adapter'ından WebGPU bilgilerini çıkarır. Adapter None ise empty döner.
pub fn extract_webgpu_info(adapter: &wgpu::Adapter) -> WebGpuAdapterInfo {
    let info = adapter.get_info();
    let features = adapter.features();
    let limits = adapter.limits();

    let is_software = info.device_type == wgpu::DeviceType::Cpu
        || info.name.to_lowercase().contains("llvmpipe")
        || info.name.to_lowercase().contains("swiftshader")
        || info.name.to_lowercase().contains("software");

    WebGpuAdapterInfo {
        name: info.name.clone(),
        vendor: info.vendor,
        device: info.device,
        device_type: format!("{:?}", info.device_type),
        backend: format!("{:?}", info.backend),
        is_software,
        msaa_x4: features.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES),
        msaa_x8: false, // wgpu'da MSAA x8 ayrı feature yok — platform-spesifik
        timestamp_query: features.contains(wgpu::Features::TIMESTAMP_QUERY),
        pipeline_statistics_query: features.contains(wgpu::Features::PIPELINE_STATISTICS_QUERY),
        texture_compression_bc: features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
        texture_compression_etc2: features.contains(wgpu::Features::TEXTURE_COMPRESSION_ETC2),
        indirect_first_instance: features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE),
        shader_f64: features.contains(wgpu::Features::SHADER_F64),
        multi_draw_indirect: features.contains(wgpu::Features::MULTI_DRAW_INDIRECT),
        max_texture_dimension_2d: limits.max_texture_dimension_2d,
        max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size as u64,
        max_compute_workgroup_size_x: limits.max_compute_workgroup_size_x,
        max_compute_workgroup_size_y: limits.max_compute_workgroup_size_y,
        max_compute_workgroup_size_z: limits.max_compute_workgroup_size_z,
        max_bind_groups: limits.max_bind_groups,
        max_color_attachments: limits.max_color_attachments,
    }
}

/// wgpu adapter'ından WebGpuStatus belirler.
pub fn determine_webgpu_status(adapter_info: &WebGpuAdapterInfo) -> WebGpuStatus {
    if adapter_info.is_software {
        WebGpuStatus::SoftwareFallback
    } else {
        WebGpuStatus::BridgeOk
    }
}
