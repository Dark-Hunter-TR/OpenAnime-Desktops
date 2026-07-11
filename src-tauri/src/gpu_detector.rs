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

/// Cheap vendor-only lookup (sysfs read, with an lspci fallback only if needed).
/// Unlike `detect_gpu()`, this does NOT create a wgpu Vulkan instance or block on
/// an adapter request, so it's safe to call from a hot path (e.g. after an
/// adapter-detection failure) without introducing extra stutter.
pub fn detect_vendor_only() -> String {
    determine_vendor()
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

#[allow(dead_code)]
pub fn detect_pkg_manager() -> String {
    if fs::metadata("/usr/bin/pacman").is_ok() {
        "pacman".to_string()
    } else if fs::metadata("/usr/bin/apt").is_ok() || fs::metadata("/usr/bin/apt-get").is_ok() {
        "apt".to_string()
    } else if fs::metadata("/usr/bin/dnf").is_ok() {
        "dnf".to_string()
    } else if fs::metadata("/usr/bin/zypper").is_ok() {
        "zypper".to_string()
    } else {
        "unknown".to_string()
    }
}

#[allow(dead_code)]
pub fn has_pkexec() -> bool {
    fs::metadata("/usr/bin/pkexec").is_ok()
}

#[allow(dead_code)]
pub fn check_missing_icds(vendor: &str) -> Vec<String> {
    let mut missing = Vec::new();
    let icd_dir = "/usr/share/vulkan/icd.d";
    
    let mut icd_files = Vec::new();
    if let Ok(entries) = fs::read_dir(icd_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".json") {
                    icd_files.push(name.to_lowercase());
                }
            }
        }
    }

    if icd_files.is_empty() {
        missing.push("all".to_string());
    } else {
        match vendor {
            "NVIDIA" => {
                if !icd_files.iter().any(|f| f.contains("nvidia")) {
                    missing.push("nvidia".to_string());
                }
            }
            "AMD" => {
                if !icd_files.iter().any(|f| f.contains("radeon") || f.contains("amd")) {
                    missing.push("amd".to_string());
                }
            }
            "Intel" => {
                if !icd_files.iter().any(|f| f.contains("intel")) {
                    missing.push("intel".to_string());
                }
            }
            _ => {}
        }
    }
    missing
}

#[allow(dead_code)]
pub fn get_whitelisted_install_command(pkg_manager: &str, package_set: &str) -> Option<(Vec<String>, String)> {
    match (pkg_manager, package_set) {
        ("pacman", "nvidia") => Some((
            vec!["-S", "--noconfirm", "nvidia-utils", "lib32-nvidia-utils", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo pacman -S nvidia-utils lib32-nvidia-utils vulkan-tools".to_string()
        )),
        ("pacman", "amd") => Some((
            vec!["-S", "--noconfirm", "vulkan-radeon", "lib32-vulkan-radeon", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo pacman -S vulkan-radeon lib32-vulkan-radeon vulkan-tools".to_string()
        )),
        ("pacman", "intel") => Some((
            vec!["-S", "--noconfirm", "vulkan-intel", "lib32-vulkan-intel", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo pacman -S vulkan-intel lib32-vulkan-intel vulkan-tools".to_string()
        )),
        ("pacman", "all") => Some((
            vec!["-S", "--noconfirm", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo pacman -S vulkan-tools".to_string()
        )),

        ("apt", "nvidia") => Some((
            vec!["install", "-y", "nvidia-driver-535", "nvidia-utils-535", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo apt install nvidia-driver-535 nvidia-utils-535 vulkan-tools".to_string()
        )),
        ("apt", "amd") | ("apt", "intel") => Some((
            vec!["install", "-y", "mesa-vulkan-drivers", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo apt install mesa-vulkan-drivers vulkan-tools".to_string()
        )),
        ("apt", "all") => Some((
            vec!["install", "-y", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo apt install vulkan-tools".to_string()
        )),

        ("dnf", "nvidia") => Some((
            vec!["install", "-y", "akmod-nvidia", "xorg-x11-drv-nvidia-cuda", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo dnf install akmod-nvidia xorg-x11-drv-nvidia-cuda vulkan-tools".to_string()
        )),
        ("dnf", "amd") | ("dnf", "intel") => Some((
            vec!["install", "-y", "mesa-vulkan-drivers", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo dnf install mesa-vulkan-drivers vulkan-tools".to_string()
        )),
        ("dnf", "all") => Some((
            vec!["install", "-y", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo dnf install vulkan-tools".to_string()
        )),

        ("zypper", "nvidia") => Some((
            vec!["install", "-y", "x11-video-nvidiaG06", "nvidia-glG06", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo zypper install x11-video-nvidiaG06 nvidia-glG06 vulkan-tools".to_string()
        )),
        ("zypper", "amd") | ("zypper", "intel") => Some((
            vec!["install", "-y", "mesa-vulkan-device-select", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo zypper install mesa-vulkan-device-select vulkan-tools".to_string()
        )),
        ("zypper", "all") => Some((
            vec!["install", "-y", "vulkan-tools"].into_iter().map(String::from).collect(),
            "sudo zypper install vulkan-tools".to_string()
        )),

        _ => None,
    }
}

#[tauri::command]
pub async fn install_gpu_packages(
    app: tauri::AppHandle,
    package_set: String,
) -> Result<(), String> {
    use tauri::Emitter;

    let pkg_manager = detect_pkg_manager();
    if pkg_manager == "unknown" {
        return Err("Bilinmeyen paket yöneticisi. Lütfen paketleri manuel olarak kurun.".to_string());
    }

    if !has_pkexec() {
        return Err("Sistemde 'pkexec' bulunamadı. Lütfen paketleri terminal üzerinden manuel olarak kurun.".to_string());
    }

    let (args, command_display) = get_whitelisted_install_command(&pkg_manager, &package_set)
        .ok_or_else(|| "Geçersiz veya yetkisiz paket kümesi isteği.".to_string())?;

    println!("[GPU Installer] Running pkexec with command: {}", command_display);
    
    let app_clone = app.clone();
    let pkg_manager_binary = match pkg_manager.as_str() {
        "pacman" => "pacman",
        "apt" => "apt",
        "dnf" => "dnf",
        "zypper" => "zypper",
        _ => return Err("Bilinmeyen paket yöneticisi.".to_string()),
    };

    std::thread::spawn(move || {
        let _ = app_clone.emit("openanime://install-progress", format!("Kurulum başlatılıyor: {}\nŞifre onayı bekleniyor...", command_display));
        
        let mut cmd = std::process::Command::new("pkexec");
        cmd.arg(pkg_manager_binary);
        cmd.args(&args);

        match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                
                if output.status.success() {
                    let _ = app_clone.emit("openanime://install-progress", format!("{}\n\n✅ Kurulum başarıyla tamamlandı! Sürücülerin aktif olması için lütfen OpenAnime uygulamasını kapatıp yeniden başlatın.", stdout));
                } else {
                    let _ = app_clone.emit(
                        "openanime://install-progress",
                        format!("❌ Kurulum başarısız oldu veya iptal edildi (Hata Kodu: {}).\n\nHata Çıktısı:\n{}\n{}", output.status.code().unwrap_or(-1), stdout, stderr)
                    );
                }
            }
            Err(e) => {
                let _ = app_clone.emit("openanime://install-progress", format!("❌ Sistem komutu çalıştırılamadı: {}", e));
            }
        }
    });

    Ok(())
}