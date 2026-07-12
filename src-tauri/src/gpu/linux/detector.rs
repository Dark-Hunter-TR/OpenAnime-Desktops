// ═══════════════════════════════════════════════════════════════════════════════
// gpu/linux/detector.rs — Linux-spesifik GPU algılama motoru
//
// Bu modül Linux'a özgü tüm GPU bilgilerini toplar:
//   • /sys/class/drm — PCI vendor/device ID'si
//   • /proc/driver/nvidia — NVIDIA driver versiyonu
//   • /etc/os-release — dağıtım tespiti
//   • glxinfo / EGL — OpenGL renderer string
//   • Mesa version
//   • vainfo — VAAPI
//   • DMA-BUF desteği
//   • Wayland/X11 tespiti
//   • NVIDIA Nouveau kontrolü
//   • AMD RADV / AMDVLK ayrımı
//   • Intel ANV / Mesa ayrımı
//
// Hiçbir unwrap() / panic! kullanılmaz. Her adım hata toleranslıdır.
// ═══════════════════════════════════════════════════════════════════════════════

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::gpu::diagnostics::types::*;
use crate::gpu::diagnostics::report::*;

/// Linux GPU algılama sonucu (ham veri, henüz rapor haline getirilmemiş).
pub struct LinuxGpuDetection {
    pub vendor: GpuVendor,
    pub renderer: String,
    pub driver_version: String,
    pub mesa_version: Option<String>,
    pub opengl_renderer: Option<String>,
    pub opengl_version: Option<String>,
    pub display_server: DisplayServer,
    pub pci_vendor_id: Option<String>,
    pub pci_device_id: Option<String>,
    pub nvidia_driver_version: Option<String>,
    pub nvidia_is_proprietary: bool,
    pub nvidia_is_nouveau: bool,
    pub amd_driver: Option<String>,
    pub intel_driver: Option<String>,
    pub vaapi_supported: bool,
    pub dmabuf_supported: bool,
    pub distro: LinuxDistro,
    pub pkg_manager: String,
    pub has_pkexec: bool,
    pub log: Vec<LogEntry>,
}

/// Linux GPU algılamasını çalıştırır. Her adım hata toleranslıdır.
pub fn detect() -> LinuxGpuDetection {
    let mut log = Vec::new();

    // ── 1. Display server tespiti ────────────────────────────────────────
    let display_server = detect_display_server(&mut log);

    // ── 2. PCI bilgileri ve vendor tespiti ──────────────────────────────
    let (vendor, pci_vendor_id, pci_device_id, renderer) = detect_pci_info(&mut log);

    // ── 3. OpenGL renderer (glxinfo veya EGL üzerinden) ─────────────────
    let (opengl_renderer, opengl_version, mesa_version) = detect_opengl_info(&mut log);

    // ── 4. NVIDIA driver tespiti ─────────────────────────────────────────
    let (nvidia_driver_version, nvidia_is_proprietary, nvidia_is_nouveau) =
        detect_nvidia_driver(&vendor, &mut log);

    // ── 5. AMD driver tespiti (RADV vs AMDVLK) ──────────────────────────
    let amd_driver = detect_amd_driver(&vendor, &opengl_renderer, &mut log);

    // ── 6. Intel driver tespiti (ANV vs Mesa) ────────────────────────────
    let intel_driver = detect_intel_driver(&vendor, &opengl_renderer, &mut log);

    // ── 7. Driver version string'i ───────────────────────────────────────
    let driver_version = build_driver_version_string(
        &vendor,
        &nvidia_driver_version,
        &mesa_version,
        &opengl_version,
    );

    // ── 8. VAAPI desteği ─────────────────────────────────────────────────
    let vaapi_supported = detect_vaapi(&mut log);

    // ── 9. DMA-BUF desteği ───────────────────────────────────────────────
    let dmabuf_supported = detect_dmabuf(&vendor, &mut log);

    // ── 10. Dağıtım ve paket yöneticisi tespiti ──────────────────────────
    let distro = detect_distro();
    let pkg_manager = distro.pkg_manager().to_string();
    let has_pkexec = Path::new("/usr/bin/pkexec").exists();

    if has_pkexec {
        log.push(LogEntry::ok("pkexec mevcut"));
    }

    LinuxGpuDetection {
        vendor,
        renderer,
        driver_version,
        mesa_version,
        opengl_renderer,
        opengl_version,
        display_server,
        pci_vendor_id,
        pci_device_id,
        nvidia_driver_version,
        nvidia_is_proprietary,
        nvidia_is_nouveau,
        amd_driver,
        intel_driver,
        vaapi_supported,
        dmabuf_supported,
        distro,
        pkg_manager,
        has_pkexec,
        log,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Display Server Tespiti
// ─────────────────────────────────────────────────────────────────────────────

fn detect_display_server(log: &mut Vec<LogEntry>) -> DisplayServer {
    // WAYLAND_DISPLAY env var varlığı Wayland'ı gösterir
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        log.push(LogEntry::ok("Display Server: Wayland"));
        return DisplayServer::Wayland;
    }

    // XDG_SESSION_TYPE kontrolü
    if let Ok(session_type) = std::env::var("XDG_SESSION_TYPE") {
        match session_type.to_lowercase().as_str() {
            "wayland" => {
                log.push(LogEntry::ok("Display Server: Wayland (XDG_SESSION_TYPE)"));
                return DisplayServer::Wayland;
            }
            "x11" | "mir" => {
                log.push(LogEntry::ok("Display Server: X11 (XDG_SESSION_TYPE)"));
                return DisplayServer::X11;
            }
            _ => {}
        }
    }

    // DISPLAY env var varlığı X11'i gösterir
    if std::env::var("DISPLAY").is_ok() {
        log.push(LogEntry::ok("Display Server: X11"));
        return DisplayServer::X11;
    }

    log.push(LogEntry::fail("Display Server", "Tespit edilemedi"));
    DisplayServer::Unknown
}

// ─────────────────────────────────────────────────────────────────────────────
// PCI Bilgileri ve Vendor Tespiti
// ─────────────────────────────────────────────────────────────────────────────

fn detect_pci_info(log: &mut Vec<LogEntry>) -> (GpuVendor, Option<String>, Option<String>, String) {
    // Önce /sys/class/drm üzerinden deneriz (kernel doğrudan sağlar)
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // card0, card1 vb. — tire içerenleri atla (card0-HDMI gibi connector'lar)
            if !name.starts_with("card") || name.contains('-') {
                continue;
            }

            let base_path = entry.path();

            // vendor
            let vendor_path = base_path.join("device/vendor");
            let vendor_hex = match fs::read_to_string(&vendor_path) {
                Ok(v) => v.trim().to_lowercase(),
                Err(_) => continue,
            };

            // device
            let device_hex = fs::read_to_string(base_path.join("device/device"))
                .map(|s| s.trim().to_lowercase())
                .ok();

            if let Some(vendor_enum) = GpuVendor::from_pci_vendor_hex(&vendor_hex) {
                let renderer = build_renderer_string_from_vendor(&vendor_enum, &device_hex);
                log.push(LogEntry::ok_detail("PCI Vendor", format!("{} ({})", vendor_enum.as_str(), &vendor_hex)));
                if let Some(ref dev) = device_hex {
                    log.push(LogEntry::ok_detail("PCI Device ID", dev));
                }
                return (vendor_enum, Some(vendor_hex), device_hex, renderer);
            }
        }
    }

    // Fallback: lspci çıktısını parse et
    if let Ok(output) = Command::new("lspci").output() {
        let lspci = String::from_utf8_lossy(&output.stdout).to_lowercase();
        let vendor = parse_lspci_vendor(&lspci, log);
        let renderer = vendor.as_str().to_string();
        return (vendor, None, None, renderer);
    }

    // Son fallback: /proc/driver/nvidia
    if Path::new("/proc/driver/nvidia").exists() {
        log.push(LogEntry::ok("NVIDIA driver tespit edildi (/proc/driver/nvidia)"));
        return (GpuVendor::Nvidia, None, None, "NVIDIA".to_string());
    }

    log.push(LogEntry::fail("PCI Vendor", "Tespit edilemedi"));
    (GpuVendor::Unknown, None, None, "Unknown GPU".to_string())
}

fn build_renderer_string_from_vendor(vendor: &GpuVendor, device_id: &Option<String>) -> String {
    match device_id {
        Some(id) => format!("{} (PCI:{})", vendor.as_str(), id.trim_start_matches("0x")),
        None => vendor.as_str().to_string(),
    }
}

fn parse_lspci_vendor(lspci: &str, log: &mut Vec<LogEntry>) -> GpuVendor {
    // VGA compatible controller satırlarını filtrele
    for line in lspci.lines() {
        if line.contains("vga") || line.contains("3d") || line.contains("display") {
            if line.contains("nvidia") {
                log.push(LogEntry::ok_detail("lspci Vendor", "NVIDIA"));
                return GpuVendor::Nvidia;
            } else if line.contains("amd") || line.contains("ati") || line.contains("radeon") {
                log.push(LogEntry::ok_detail("lspci Vendor", "AMD"));
                return GpuVendor::Amd;
            } else if line.contains("intel") {
                log.push(LogEntry::ok_detail("lspci Vendor", "Intel"));
                return GpuVendor::Intel;
            } else if line.contains("virtio") {
                log.push(LogEntry::ok_detail("lspci Vendor", "VirtIO"));
                return GpuVendor::VirtIo;
            } else if line.contains("vmware") {
                log.push(LogEntry::ok_detail("lspci Vendor", "VMware"));
                return GpuVendor::Vmware;
            }
        }
    }
    GpuVendor::Unknown
}

// ─────────────────────────────────────────────────────────────────────────────
// OpenGL Renderer ve Mesa Version
// ─────────────────────────────────────────────────────────────────────────────

fn detect_opengl_info(log: &mut Vec<LogEntry>) -> (Option<String>, Option<String>, Option<String>) {
    // glxinfo -B (özet çıktı) deneriz
    if let Ok(output) = Command::new("glxinfo").arg("-B").output() {
        let text = String::from_utf8_lossy(&output.stdout);
        let renderer = extract_line_value(&text, "OpenGL renderer string:");
        let version = extract_line_value(&text, "OpenGL version string:");
        let mesa_ver = extract_mesa_version(&text);

        if renderer.is_some() {
            log.push(LogEntry::ok_detail(
                "OpenGL Renderer",
                renderer.as_deref().unwrap_or(""),
            ));
        }
        if let Some(ref v) = mesa_ver {
            log.push(LogEntry::ok_detail("Mesa Version", v));
        }

        return (renderer, version, mesa_ver);
    }

    // glxinfo yoksa eglinfo deneriz
    if let Ok(output) = Command::new("eglinfo").output() {
        let text = String::from_utf8_lossy(&output.stdout);
        let renderer = extract_line_value(&text, "EGL_RENDERER:");
        let mesa_ver = extract_mesa_version(&text);
        return (renderer, None, mesa_ver);
    }

    // Son çare: /proc/driver/nvidia/version (NVIDIA özel)
    if let Ok(content) = fs::read_to_string("/proc/driver/nvidia/version") {
        let version_line = content.lines().next().unwrap_or("").to_string();
        log.push(LogEntry::ok_detail("NVIDIA Version (proc)", &version_line));
        return (Some("NVIDIA".to_string()), Some(version_line), None);
    }

    (None, None, None)
}

fn extract_line_value(text: &str, prefix: &str) -> Option<String> {
    text.lines()
        .find(|line| line.contains(prefix))
        .and_then(|line| line.split(':').nth(1))
        .map(|v| v.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn extract_mesa_version(text: &str) -> Option<String> {
    // "OpenGL version string: 4.6 (Compatibility Profile) Mesa 23.1.9"
    // "Mesa 23.1.9" kısmını çıkar
    for line in text.lines() {
        if let Some(pos) = line.to_lowercase().find("mesa ") {
            let after = &line[pos + 5..];
            let ver: String = after
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if !ver.is_empty() {
                return Some(format!("Mesa {}", ver));
            }
        }
    }

    // /usr/share/doc/libgl1-mesa-dri dizinindeki changelog'dan çekmeyi dene (Debian/Ubuntu)
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// NVIDIA Driver Tespiti
// ─────────────────────────────────────────────────────────────────────────────

fn detect_nvidia_driver(
    vendor: &GpuVendor,
    log: &mut Vec<LogEntry>,
) -> (Option<String>, bool, bool) {
    if *vendor != GpuVendor::Nvidia {
        return (None, false, false);
    }

    // Proprietary driver: /proc/driver/nvidia/version
    if let Ok(content) = fs::read_to_string("/proc/driver/nvidia/version") {
        // "NVRM version: NVIDIA UNIX x86_64 Kernel Module  535.161.08  ..."
        let version = content
            .lines()
            .next()
            .and_then(|line| {
                // Versiyon numarasını çıkar (sayı.sayı.sayı formatı)
                line.split_whitespace()
                    .find(|token| {
                        token.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
                            && token.contains('.')
                    })
                    .map(String::from)
            })
            .unwrap_or_else(|| content.lines().next().unwrap_or("").trim().to_string());

        log.push(LogEntry::ok_detail("NVIDIA Proprietary Driver", &version));
        return (Some(version), true, false);
    }

    // Nouveau: kernel module adı kontrolü
    if let Ok(output) = Command::new("lsmod").output() {
        let modules = String::from_utf8_lossy(&output.stdout);
        if modules.contains("nouveau") {
            log.push(LogEntry::ok("Nouveau (open-source NVIDIA driver)"));
            // Nouveau version çek
            let version = fs::read_to_string("/sys/bus/platform/drivers/nouveau/version")
                .ok()
                .map(|s| format!("Nouveau {}", s.trim()));
            return (version, false, true);
        }
    }

    // modinfo ile kontrol
    if let Ok(output) = Command::new("modinfo").arg("nouveau").output() {
        if !output.stdout.is_empty() {
            let info = String::from_utf8_lossy(&output.stdout);
            let version = extract_line_value(&info, "version:");
            log.push(LogEntry::ok_detail(
                "Nouveau module",
                version.as_deref().unwrap_or("yüklü"),
            ));
            return (version.map(|v| format!("Nouveau {}", v)), false, true);
        }
    }

    log.push(LogEntry::fail("NVIDIA Driver", "Ne proprietary ne de Nouveau tespit edildi"));
    (None, false, false)
}

// ─────────────────────────────────────────────────────────────────────────────
// AMD Driver Tespiti (RADV vs AMDVLK)
// ─────────────────────────────────────────────────────────────────────────────

fn detect_amd_driver(
    vendor: &GpuVendor,
    opengl_renderer: &Option<String>,
    log: &mut Vec<LogEntry>,
) -> Option<String> {
    if *vendor != GpuVendor::Amd {
        return None;
    }

    // ICD JSON dosyalarını kontrol et
    let icd_dir = "/usr/share/vulkan/icd.d";
    if let Ok(entries) = fs::read_dir(icd_dir) {
        let files: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_lowercase))
            .collect();

        let has_radv = files.iter().any(|f| f.contains("radeon") || f.contains("radv"));
        let has_amdvlk = files.iter().any(|f| f.contains("amdvlk") || f.contains("amd_icd"));

        if has_amdvlk {
            log.push(LogEntry::ok("AMD Driver: AMDVLK (resmi AMD sürücüsü)"));
            return Some("AMDVLK".to_string());
        }
        if has_radv {
            log.push(LogEntry::ok("AMD Driver: RADV (Mesa open-source, önerilen)"));
            return Some("RADV".to_string());
        }
    }

    // OpenGL renderer string'inden çıkar
    if let Some(renderer_str) = opengl_renderer {
        let lower = renderer_str.to_lowercase();
        if lower.contains("radv") {
            return Some("RADV".to_string());
        }
        if lower.contains("amdvlk") || lower.contains("pal") {
            return Some("AMDVLK".to_string());
        }
    }

    log.push(LogEntry::fail("AMD Driver", "RADV veya AMDVLK tespit edilemedi"));
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Intel Driver Tespiti (ANV vs Mesa)
// ─────────────────────────────────────────────────────────────────────────────

fn detect_intel_driver(
    vendor: &GpuVendor,
    opengl_renderer: &Option<String>,
    log: &mut Vec<LogEntry>,
) -> Option<String> {
    if *vendor != GpuVendor::Intel {
        return None;
    }

    // ICD JSON dosyaları
    let icd_dir = "/usr/share/vulkan/icd.d";
    if let Ok(entries) = fs::read_dir(icd_dir) {
        let files: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_lowercase))
            .collect();

        if files.iter().any(|f| f.contains("intel") && f.contains("icd")) {
            log.push(LogEntry::ok("Intel Driver: ANV (Mesa Vulkan)"));
            return Some("ANV".to_string());
        }
    }

    // OpenGL renderer string'inden çıkar
    if let Some(renderer_str) = opengl_renderer {
        let lower = renderer_str.to_lowercase();
        if lower.contains("iris") || lower.contains("i965") || lower.contains("mesa") {
            log.push(LogEntry::ok("Intel Driver: Mesa (iris/i965)"));
            return Some("Mesa".to_string());
        }
        if lower.contains("anv") {
            return Some("ANV".to_string());
        }
    }

    // Intel xe / i915 modül kontrolü
    if let Ok(output) = Command::new("lsmod").output() {
        let modules = String::from_utf8_lossy(&output.stdout);
        if modules.contains("i915") || modules.contains("xe") {
            log.push(LogEntry::ok_detail("Intel Driver", "i915/xe kernel module tespit edildi"));
            return Some("Mesa".to_string());
        }
    }

    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Driver Version String Oluşturma
// ─────────────────────────────────────────────────────────────────────────────

fn build_driver_version_string(
    vendor: &GpuVendor,
    nvidia_version: &Option<String>,
    mesa_version: &Option<String>,
    opengl_version: &Option<String>,
) -> String {
    match vendor {
        GpuVendor::Nvidia => {
            if let Some(v) = nvidia_version {
                return format!("NVIDIA {}", v);
            }
        }
        GpuVendor::Amd | GpuVendor::Intel | GpuVendor::Mesa => {
            if let Some(v) = mesa_version {
                return v.clone();
            }
        }
        _ => {}
    }

    if let Some(v) = opengl_version {
        return v.clone();
    }

    "Bilinmiyor".to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// VAAPI Desteği
// ─────────────────────────────────────────────────────────────────────────────

fn detect_vaapi(log: &mut Vec<LogEntry>) -> bool {
    // vainfo komutu VAAPI desteğini listeler
    if let Ok(output) = Command::new("vainfo").output() {
        let text = String::from_utf8_lossy(&output.stdout);
        if text.contains("VAProfileH264") || text.contains("VAProfileHEVC") {
            log.push(LogEntry::ok("VAAPI: Video decode desteği mevcut"));
            return true;
        }
    }

    // /dev/dri/renderD128 varlığı DRM render node'u gösterir (VAAPI için gerekli)
    if Path::new("/dev/dri/renderD128").exists() {
        log.push(LogEntry::ok_detail("DRM Render Node", "/dev/dri/renderD128 mevcut"));
        return true;
    }

    log.push(LogEntry::fail("VAAPI", "vainfo bulunamadı veya VAAPI desteği yok"));
    false
}

// ─────────────────────────────────────────────────────────────────────────────
// DMA-BUF Desteği
// ─────────────────────────────────────────────────────────────────────────────

fn detect_dmabuf(vendor: &GpuVendor, log: &mut Vec<LogEntry>) -> bool {
    // NVIDIA proprietary driver DMA-BUF sorunlu olabiliyor (explicit sync)
    if *vendor == GpuVendor::Nvidia {
        // NVIDIA 555+ sürümlerde DMA-BUF explicit sync desteği eklendi
        if Path::new("/sys/module/nvidia/version").exists() {
            if let Ok(ver) = fs::read_to_string("/sys/module/nvidia/version") {
                let major: u32 = ver.trim().split('.').next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                if major >= 555 {
                    log.push(LogEntry::ok_detail("DMA-BUF (NVIDIA)", format!("Driver {} explicit sync destekliyor", ver.trim())));
                    return true;
                } else {
                    log.push(LogEntry::fail("DMA-BUF (NVIDIA)", format!("Driver {} < 555 — explicit sync yok, devre dışı", ver.trim())));
                    return false;
                }
            }
        }
        // Driver version bilinmiyorsa güvenli taraf: false
        log.push(LogEntry::fail("DMA-BUF (NVIDIA)", "Driver versiyonu okunamadı — devre dışı"));
        return false;
    }

    // Mesa (AMD/Intel/Mesa) için DRM prime support kontrolü
    if Path::new("/dev/dri/renderD128").exists() {
        log.push(LogEntry::ok("DMA-BUF: DRM render node mevcut (Mesa tam destek)"));
        return true;
    }

    false
}

// ─────────────────────────────────────────────────────────────────────────────
// Dağıtım Tespiti
// ─────────────────────────────────────────────────────────────────────────────

pub fn detect_distro() -> LinuxDistro {
    // /etc/os-release standart Linux dağıtım bilgisi dosyası
    let content = fs::read_to_string("/etc/os-release")
        .or_else(|_| fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default();

    // ID= satırını bul
    for line in content.lines() {
        if let Some(id) = line.strip_prefix("ID=") {
            return LinuxDistro::from_id(id.trim_matches('"'));
        }
    }

    LinuxDistro::Unknown("unknown".to_string())
}

/// Paket yöneticisini binary varlığından tespit eder (distro tespiti yetersizse fallback)
pub fn detect_pkg_manager_by_binary() -> &'static str {
    let managers = [
        ("/usr/bin/pacman", "pacman"),
        ("/usr/bin/apt", "apt"),
        ("/usr/bin/apt-get", "apt"),
        ("/usr/bin/dnf", "dnf"),
        ("/usr/bin/zypper", "zypper"),
        ("/usr/bin/nix-env", "nix"),
    ];

    for (path, name) in &managers {
        if Path::new(path).exists() {
            return name;
        }
    }

    "unknown"
}

/// Vulkan ICD dosyalarını tara ve bul.
pub fn find_vulkan_icd_files() -> Vec<String> {
    let icd_dirs = [
        "/usr/share/vulkan/icd.d",
        "/etc/vulkan/icd.d",
        "/usr/local/share/vulkan/icd.d",
    ];

    let mut files = Vec::new();
    for dir in &icd_dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".json") {
                        files.push(format!("{}/{}", dir, name));
                    }
                }
            }
        }
    }
    files
}

/// Renderer string'den yazılımsal renderer olup olmadığını tespit eder.
pub fn is_software_renderer(renderer: &str) -> bool {
    let lower = renderer.to_lowercase();
    lower.contains("llvmpipe")
        || lower.contains("softpipe")
        || lower.contains("swiftshader")
        || lower.contains("software")
        || lower.contains("cpu")
}

/// Renderer string'den GpuVendor refinement'ı yapar.
/// llvmpipe/SwiftShader gibi yazılımsal renderer'ları özel vendor'a taşır.
pub fn refine_vendor_from_renderer(base_vendor: GpuVendor, renderer: &str) -> GpuVendor {
    let lower = renderer.to_lowercase();
    if lower.contains("llvmpipe") {
        return GpuVendor::LlvmPipe;
    }
    if lower.contains("swiftshader") {
        return GpuVendor::SwiftShader;
    }
    if lower.contains("nouveau") {
        return GpuVendor::Nouveau;
    }
    base_vendor
}
