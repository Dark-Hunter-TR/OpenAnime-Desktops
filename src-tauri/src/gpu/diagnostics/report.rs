// ═══════════════════════════════════════════════════════════════════════════════
// gpu/diagnostics/report.rs — FullGpuReport oluşturucu ve log formatıcı
//
// Bu modül, platform-spesifik detector'lardan gelen verileri birleştirerek
// tek bir FullGpuReport üretir. Tüm log adımları burada formatlanır.
// ═══════════════════════════════════════════════════════════════════════════════

use super::types::*;

/// Log adımlarını terminal için okunabilir formata çevirir.
/// Hem uygulamanın iç log sistemine hem de frontend'e gönderilmek üzere
/// kullanılabilir.
pub fn format_log_as_text(entries: &[LogEntry]) -> String {
    entries
        .iter()
        .map(|e| {
            let symbol = if e.ok { "✓" } else { "✗" };
            if let Some(detail) = &e.detail {
                format!("  {} {}: {}", symbol, e.label, detail)
            } else {
                format!("  {} {}", symbol, e.label)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// FullGpuReport için default (boş) rapor oluşturucu.
/// Bir hata oluştuğunda ya da Linux dışı platformlarda kullanılır.
pub fn empty_report() -> FullGpuReport {
    FullGpuReport {
        vendor: "Unknown".to_string(),
        vendor_enum: GpuVendor::Unknown,
        renderer: "Unknown".to_string(),
        driver_version: "Unknown".to_string(),
        mesa_version: None,
        vulkan_version: None,
        vulkan_status: VulkanProbeStatus::IcdMissing,
        vulkan_steps: Vec::new(),
        vulkan_icd_files: Vec::new(),
        webgpu_status: WebGpuStatus::Unsupported,
        opengl_renderer: None,
        opengl_version: None,
        display_server: DisplayServer::Unknown,
        backend: GpuBackend::Unknown,
        adapter_name: None,
        device_id: None,
        pci_vendor_id: None,
        pci_device_id: None,
        vaapi_supported: false,
        dmabuf_supported: false,
        hw_accel: false,
        video_decode: false,
        video_encode: false,
        msaa_supported: false,
        compute_shader: false,
        timestamp_query: false,
        max_texture_dimension: None,
        max_storage_buffer_binding_size: None,
        max_compute_workgroup_size: None,
        nvidia_driver_version: None,
        nvidia_is_proprietary: false,
        nvidia_is_nouveau: false,
        amd_driver: None,
        intel_driver: None,
        init_log: Vec::new(),
        pkg_manager: "unknown".to_string(),
        recommended_packages: String::new(),
        recommended_command: String::new(),
        has_pkexec: false,
        critical_error: None,
    }
}

/// VulkanProbeResult'tan FullGpuReport'a Vulkan verilerini doldurur.
pub fn apply_vulkan_probe(report: &mut FullGpuReport, probe: &VulkanProbeResult) {
    report.vulkan_status = probe.status.clone();
    report.vulkan_steps = probe.steps.clone();
    report.vulkan_icd_files = probe.icd_files.clone();
    report.vulkan_version = probe.instance_version.clone();
    report.backend = probe.backend.clone();

    if probe.status.is_ok() {
        report.init_log.push(LogEntry::ok_detail(
            "Vulkan",
            probe.instance_version.as_deref().unwrap_or("version bilinmiyor"),
        ));
        if let Some(device) = &probe.device_name {
            report.init_log.push(LogEntry::ok_detail("Vulkan Device", device));
        }
    } else {
        report.init_log.push(LogEntry::fail(
            "Vulkan",
            probe.status.error_message(),
        ));
        // İlk başarısız adımı da log'a ekle
        for step in &probe.steps {
            if !step.ok {
                if let Some(err) = &step.error {
                    report.init_log.push(LogEntry::fail(
                        format!("  → {}", step.step),
                        err,
                    ));
                }
                break; // Sadece ilk hata
            }
        }
    }
}

/// Paket yöneticisi bilgisine göre önerilen kurulum komutunu oluşturur.
pub fn build_install_command(pkg_manager: &str, vendor: &GpuVendor) -> (String, String) {
    let vendor_key = match vendor {
        GpuVendor::Nvidia => "nvidia",
        GpuVendor::Amd => "amd",
        GpuVendor::Intel => "intel",
        _ => "all",
    };

    match (pkg_manager, vendor_key) {
        // ── Arch / Manjaro / EndeavourOS ──────────────────────────
        ("pacman", "nvidia") => (
            "nvidia-utils lib32-nvidia-utils vulkan-tools".to_string(),
            "sudo pacman -S nvidia-utils lib32-nvidia-utils vulkan-tools".to_string(),
        ),
        ("pacman", "amd") => (
            "vulkan-radeon lib32-vulkan-radeon vulkan-tools".to_string(),
            "sudo pacman -S vulkan-radeon lib32-vulkan-radeon vulkan-tools".to_string(),
        ),
        ("pacman", "intel") => (
            "vulkan-intel lib32-vulkan-intel vulkan-tools".to_string(),
            "sudo pacman -S vulkan-intel lib32-vulkan-intel vulkan-tools".to_string(),
        ),
        ("pacman", _) => (
            "vulkan-tools".to_string(),
            "sudo pacman -S vulkan-tools".to_string(),
        ),

        // ── Ubuntu / Debian / Mint / Pop!_OS ──────────────────────
        ("apt", "nvidia") => (
            "nvidia-driver-535 nvidia-utils-535 vulkan-tools".to_string(),
            "sudo apt update && sudo apt install -y nvidia-driver-535 nvidia-utils-535 vulkan-tools".to_string(),
        ),
        ("apt", _) => (
            "mesa-vulkan-drivers vulkan-tools".to_string(),
            "sudo apt update && sudo apt install -y mesa-vulkan-drivers vulkan-tools".to_string(),
        ),

        // ── Fedora ────────────────────────────────────────────────
        ("dnf", "nvidia") => (
            "akmod-nvidia xorg-x11-drv-nvidia-cuda vulkan-tools".to_string(),
            "sudo dnf install -y akmod-nvidia xorg-x11-drv-nvidia-cuda vulkan-tools".to_string(),
        ),
        ("dnf", _) => (
            "mesa-vulkan-drivers vulkan-tools".to_string(),
            "sudo dnf install -y mesa-vulkan-drivers vulkan-tools".to_string(),
        ),

        // ── OpenSUSE ──────────────────────────────────────────────
        ("zypper", "nvidia") => (
            "x11-video-nvidiaG06 nvidia-glG06 vulkan-tools".to_string(),
            "sudo zypper install -y x11-video-nvidiaG06 nvidia-glG06 vulkan-tools".to_string(),
        ),
        ("zypper", _) => (
            "mesa-vulkan-device-select vulkan-tools".to_string(),
            "sudo zypper install -y mesa-vulkan-device-select vulkan-tools".to_string(),
        ),

        // ── NixOS ─────────────────────────────────────────────────
        ("nix", "nvidia") => (
            "nixpkgs.nvidia_x11 nixpkgs.vulkan-tools".to_string(),
            "# NixOS: hardware.opengl.enable = true; hardware.nvidia.package = config.boot.kernelPackages.nvidiaPackages.stable;".to_string(),
        ),
        ("nix", _) => (
            "nixpkgs.mesa nixpkgs.vulkan-tools".to_string(),
            "# NixOS: hardware.opengl.enable = true; hardware.opengl.extraPackages = with pkgs; [ mesa ];".to_string(),
        ),

        _ => (
            String::new(),
            "Dağıtımınızın paket yöneticisinden GPU'nuz için Vulkan sürücülerini kurun.".to_string(),
        ),
    }
}
