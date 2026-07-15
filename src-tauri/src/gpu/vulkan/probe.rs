// ═══════════════════════════════════════════════════════════════════════════════
// gpu/vulkan/probe.rs — Vulkan probe (Linux desteği kaldırıldı → no-op stub)
//
// Gerçek wgpu tabanlı Vulkan doğrulama yalnızca Linux yolundaydı. Linux desteği
// tamamen kaldırıldığından bu modül artık her hedefte atlanan bir stub döndürür;
// Windows (WebView2) ve macOS (WKWebView) WebGPU'yu native olarak sağlar.
// ═══════════════════════════════════════════════════════════════════════════════

pub mod inner {
    use crate::gpu::diagnostics::types::*;

    pub async fn run_vulkan_probe() -> VulkanProbeResult {
        VulkanProbeResult {
            status: VulkanProbeStatus::Ok,
            steps: vec![VulkanProbeStep::success("Platform", "Vulkan probe atlandı")],
            instance_version: None,
            device_name: None,
            icd_files: Vec::new(),
            backend: GpuBackend::Unknown,
        }
    }
}
