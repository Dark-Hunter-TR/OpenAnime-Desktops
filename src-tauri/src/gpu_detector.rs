use std::fs;
use std::process::Command;

#[derive(Debug, serde::Serialize, Clone)]
pub struct GpuReport {
    pub vendor: String,
    pub vulkan_supported: bool,
    pub driver_version: String,
    pub recommended_command: String,
    pub recommended_packages: String,
}

pub fn detect_gpu() -> GpuReport {
    let vendor = determine_vendor();
    let distro = determine_distro();
    
    // Check Vulkan support via wgpu (which is our rendering backend)
    let (vulkan_supported, driver_version) = check_vulkan_support();

    let (recommended_packages, recommended_command) = if !vulkan_supported {
        get_install_instructions(&vendor, &distro)
    } else {
        (String::new(), String::new())
    };

    GpuReport {
        vendor,
        vulkan_supported,
        driver_version,
        recommended_command,
        recommended_packages,
    }
}

fn determine_vendor() -> String {
    // 1. Try reading /sys/class/drm/card*/device/vendor
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("card") && !name.contains('-') {
                let vendor_path = entry.path().join("device/vendor");
                if let Ok(vendor_hex) = fs::read_to_string(vendor_path) {
                    let cleaned = vendor_hex.trim().to_lowercase();
                    if cleaned.contains("10de") {
                        return "NVIDIA".to_string();
                    } else if cleaned.contains("1002") {
                        return "AMD".to_string();
                    } else if cleaned.contains("8086") {
                        return "Intel".to_string();
                    }
                }
            }
        }
    }

    // 2. Fallback to lspci
    if let Ok(output) = Command::new("lspci").output() {
        let lspci_str = String::from_utf8_lossy(&output.stdout).to_lowercase();
        if lspci_str.contains("nvidia") {
            return "NVIDIA".to_string();
        } else if lspci_str.contains("amd") || lspci_str.contains("ati") || lspci_str.contains("radeon") {
            return "AMD".to_string();
        } else if lspci_str.contains("intel") {
            return "Intel".to_string();
        }
    }

    "Unknown".to_string()
}

fn determine_distro() -> String {
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("ID=") {
                return line.trim_start_matches("ID=").trim_matches('"').to_string();
            }
        }
    }
    "unknown".to_string()
}

fn check_vulkan_support() -> (bool, String) {
    // Since we compile on Windows/macOS, we must isolate wgpu Vulkan checks if we're on Linux.
    #[cfg(target_os = "linux")]
    {
        // Try to create a wgpu instance with Vulkan backend
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        // Request an adapter
        let request = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        });

        // Block on the future synchronously for diagnostic check
        if let Some(adapter) = tauri::async_runtime::block_on(request) {
            let info = adapter.get_info();
            let driver_version = format!(
                "{} (Driver: {}, API: {:?})",
                info.name,
                info.driver_info,
                info.backend
            );
            return (true, driver_version);
        }
    }

    (false, "Vulkan backend unavailable or failed to initialize".to_string())
}

fn get_install_instructions(vendor: &str, distro: &str) -> (String, String) {
    match distro {
        "arch" | "manjaro" => {
            match vendor {
                "NVIDIA" => (
                    "nvidia-utils, lib32-nvidia-utils, vulkan-tools".to_string(),
                    "sudo pacman -S nvidia-utils lib32-nvidia-utils vulkan-tools".to_string(),
                ),
                "AMD" => (
                    "vulkan-radeon, lib32-vulkan-radeon, vulkan-tools".to_string(),
                    "sudo pacman -S vulkan-radeon lib32-vulkan-radeon vulkan-tools".to_string(),
                ),
                _ => (
                    "vulkan-intel, lib32-vulkan-intel, vulkan-tools".to_string(),
                    "sudo pacman -S vulkan-intel lib32-vulkan-intel vulkan-tools".to_string(),
                ),
            }
        }
        "ubuntu" | "debian" | "pop" | "mint" => {
            match vendor {
                "NVIDIA" => (
                    "nvidia-driver-535, nvidia-utils-535, vulkan-tools".to_string(),
                    "sudo apt update && sudo apt install nvidia-driver-535 nvidia-utils-535 vulkan-tools".to_string(),
                ),
                "AMD" => (
                    "mesa-vulkan-drivers, vulkan-tools".to_string(),
                    "sudo apt update && sudo apt install mesa-vulkan-drivers vulkan-tools".to_string(),
                ),
                _ => (
                    "mesa-vulkan-drivers, vulkan-tools".to_string(),
                    "sudo apt update && sudo apt install mesa-vulkan-drivers vulkan-tools".to_string(),
                ),
            }
        }
        "fedora" => {
            match vendor {
                "NVIDIA" => (
                    "akmod-nvidia, xorg-x11-drv-nvidia-cuda, vulkan-tools".to_string(),
                    "sudo dnf install akmod-nvidia xorg-x11-drv-nvidia-cuda vulkan-tools".to_string(),
                ),
                _ => (
                    "mesa-vulkan-drivers, vulkan-tools".to_string(),
                    "sudo dnf install mesa-vulkan-drivers vulkan-tools".to_string(),
                ),
            }
        }
        "opensuse" | "opensuse-tumbleweed" => {
            match vendor {
                "NVIDIA" => (
                    "x11-video-nvidiaG06, nvidia-glG06".to_string(),
                    "sudo zypper install x11-video-nvidiaG06 nvidia-glG06 vulkan-tools".to_string(),
                ),
                _ => (
                    "mesa-vulkan-device-select, vulkan-tools".to_string(),
                    "sudo zypper install mesa-vulkan-device-select vulkan-tools".to_string(),
                ),
            }
        }
        _ => (
            "Vulkan drivers (Mesa / NVIDIA proprietary)".to_string(),
            "Lütfen dağıtımınızın paket yöneticisinden ekran kartınıza uygun Vulkan sürücülerini kurun.".to_string(),
        ),
    }
}
