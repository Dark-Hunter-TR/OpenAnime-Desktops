// ═══════════════════════════════════════════════════════════════════════════════
// gpu/diagnostics/types.rs — Tüm GPU tanılama tipleri
//
// Bu modül, GPU altyapısının her katmanında kullanılan enum ve struct
// tanımlarını barındırır. Hiçbir logic burada yoktur — sadece veri modeli.
// ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// GPU Vendor
// ─────────────────────────────────────────────────────────────────────────────

/// Sistemde tespit edilen GPU üreticisi veya sürücü türü.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuVendor {
    /// NVIDIA proprietary driver (nvidia.ko) — vendor ID: 0x10DE
    Nvidia,
    /// AMD discrete/integrated GPU — vendor ID: 0x1002
    Amd,
    /// Intel integrated/Arc GPU — vendor ID: 0x8086
    Intel,
    /// Mesa open-source GPU stack (radv, anv, iris, ...)
    Mesa,
    /// NVIDIA Nouveau open-source driver
    Nouveau,
    /// Mesa llvmpipe — tamamen yazılımsal CPU renderer
    LlvmPipe,
    /// Google SwiftShader — yazılımsal Vulkan/WebGPU renderer
    SwiftShader,
    /// VirtIO GPU — sanal makine/QEMU GPU
    VirtIo,
    /// VMware SVGA GPU — VMware sanal makinesi
    Vmware,
    /// Bilinmeyen/tespit edilemeyen GPU
    Unknown,
}

impl GpuVendor {
    pub fn as_str(&self) -> &'static str {
        match self {
            GpuVendor::Nvidia => "NVIDIA",
            GpuVendor::Amd => "AMD",
            GpuVendor::Intel => "Intel",
            GpuVendor::Mesa => "Mesa",
            GpuVendor::Nouveau => "Nouveau",
            GpuVendor::LlvmPipe => "llvmpipe",
            GpuVendor::SwiftShader => "SwiftShader",
            GpuVendor::VirtIo => "VirtIO",
            GpuVendor::Vmware => "VMware",
            GpuVendor::Unknown => "Unknown",
        }
    }

    /// PCI vendor ID hex string'inden vendor çıkarır.
    pub fn from_pci_vendor_hex(hex: &str) -> Option<Self> {
        let cleaned = hex.trim().to_lowercase();
        let cleaned = cleaned.trim_start_matches("0x");
        match cleaned {
            "10de" => Some(GpuVendor::Nvidia),
            "1002" => Some(GpuVendor::Amd),
            "8086" => Some(GpuVendor::Intel),
            "1af4" | "1b36" => Some(GpuVendor::VirtIo),
            "15ad" => Some(GpuVendor::Vmware),
            _ => None,
        }
    }
}

impl std::fmt::Display for GpuVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Display Server
// ─────────────────────────────────────────────────────────────────────────────

/// Aktif display server protokolü.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayServer {
    Wayland,
    X11,
    Unknown,
}

impl DisplayServer {
    pub fn as_str(&self) -> &'static str {
        match self {
            DisplayServer::Wayland => "Wayland",
            DisplayServer::X11 => "X11",
            DisplayServer::Unknown => "Unknown",
        }
    }
}

impl std::fmt::Display for DisplayServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GPU Backend
// ─────────────────────────────────────────────────────────────────────────────

/// Seçilen veya denenen render backend'i.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuBackend {
    /// Vulkan — Linux birincil tercih
    Vulkan,
    /// OpenGL — Vulkan yoksa Linux fallback
    OpenGL,
    /// ANGLE — OpenGL üstü Vulkan/Metal çeviri katmanı
    Angle,
    /// Direct3D 12 — Windows birincil tercih
    Direct3D12,
    /// Direct3D 11 — Windows fallback
    Direct3D11,
    /// Metal — macOS birincil tercih
    Metal,
    /// Tamamen yazılımsal renderer (llvmpipe / SwiftShader)
    Software,
    /// Henüz belirlenmemiş
    Unknown,
}

impl GpuBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            GpuBackend::Vulkan => "Vulkan",
            GpuBackend::OpenGL => "OpenGL",
            GpuBackend::Angle => "ANGLE",
            GpuBackend::Direct3D12 => "Direct3D 12",
            GpuBackend::Direct3D11 => "Direct3D 11",
            GpuBackend::Metal => "Metal",
            GpuBackend::Software => "Software",
            GpuBackend::Unknown => "Unknown",
        }
    }

    pub fn is_hardware(&self) -> bool {
        !matches!(self, GpuBackend::Software | GpuBackend::Unknown)
    }
}

impl std::fmt::Display for GpuBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Vulkan Probe Status
// ─────────────────────────────────────────────────────────────────────────────

/// Vulkan doğrulama sürecinin her adımının sonucu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VulkanProbeStatus {
    /// Vulkan tamamen kullanılabilir
    Ok,
    /// libvulkan.so.1 sistemde bulunamadı
    LibMissing,
    /// /usr/share/vulkan/icd.d/ dizini boş veya yok
    IcdMissing,
    /// ICD JSON dosyaları parse edilemedi
    IcdCorrupt,
    /// wgpu Instance oluşturulamadı
    InstanceFailed,
    /// Hiç fiziksel GPU cihazı bulunamadı
    NoPhysicalDevice,
    /// Uygun queue family bulunamadı
    NoQueue,
    /// Surface oluşturulamadı (headless ortam veya Wayland eksik)
    SurfaceFailed,
    /// Swapchain extension desteklenmiyor
    SwapchainUnsupported,
}

impl VulkanProbeStatus {
    pub fn is_ok(&self) -> bool {
        *self == VulkanProbeStatus::Ok
    }

    /// Kullanıcıya gösterilen hata mesajı
    pub fn error_message(&self) -> &'static str {
        match self {
            VulkanProbeStatus::Ok => "Vulkan hazır",
            VulkanProbeStatus::LibMissing => "libvulkan.so.1 bulunamadı — Vulkan loader kurulu değil",
            VulkanProbeStatus::IcdMissing => "Vulkan ICD driver dosyaları eksik (/usr/share/vulkan/icd.d/ boş)",
            VulkanProbeStatus::IcdCorrupt => "Vulkan ICD JSON dosyaları okunamıyor — driver kurulumu bozuk olabilir",
            VulkanProbeStatus::InstanceFailed => "Vulkan Instance oluşturulamadı — driver başlatılamıyor",
            VulkanProbeStatus::NoPhysicalDevice => "Hiç Vulkan fiziksel cihaz bulunamadı — GPU driver yüklenmemiş",
            VulkanProbeStatus::NoQueue => "Uygun GPU queue bulunamadı — driver sorunlu olabilir",
            VulkanProbeStatus::SurfaceFailed => "Vulkan Surface oluşturulamadı — display server bağlantısı yok",
            VulkanProbeStatus::SwapchainUnsupported => "Swapchain extension desteklenmiyor — driver çok eski",
        }
    }

    /// Kullanıcıya önerilen çözüm adımı
    pub fn fix_hint(&self) -> &'static str {
        match self {
            VulkanProbeStatus::Ok => "",
            VulkanProbeStatus::LibMissing => "Vulkan loader paketini kurun (örn: libvulkan1, vulkan-loader)",
            VulkanProbeStatus::IcdMissing => "GPU'nuz için Vulkan ICD driver paketini kurun",
            VulkanProbeStatus::IcdCorrupt => "GPU driver paketini kaldırıp yeniden kurun",
            VulkanProbeStatus::InstanceFailed => "GPU driver'ı yeniden yükleyin ve sistemi yeniden başlatın",
            VulkanProbeStatus::NoPhysicalDevice => "GPU driver paketinin doğru kurulduğunu doğrulayın",
            VulkanProbeStatus::NoQueue => "GPU driver'ı güncelleyin veya yeniden kurun",
            VulkanProbeStatus::SurfaceFailed => "Display server ortam değişkenlerini kontrol edin",
            VulkanProbeStatus::SwapchainUnsupported => "GPU driver'ı en güncel sürüme güncelleyin",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WebGPU Status
// ─────────────────────────────────────────────────────────────────────────────

/// Browser/wgpu WebGPU durumu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebGpuStatus {
    /// WebGPU tamamen kullanılabilir (browser native)
    NativeOk,
    /// wgpu IPC bridge üzerinden çalışıyor (Linux shim)
    BridgeOk,
    /// WebGPU yazılımsal adapter ile çalışıyor
    SoftwareFallback,
    /// requestAdapter() başarısız oldu
    AdapterFailed,
    /// requestDevice() başarısız oldu
    DeviceFailed,
    /// navigator.gpu mevcut değil
    NoNavigatorGpu,
    /// Bu platform/runtime'da WebGPU desteklenmiyor
    Unsupported,
}

impl WebGpuStatus {
}

// ─────────────────────────────────────────────────────────────────────────────
// Vulkan Probe Detail — tek adım sonucu
// ─────────────────────────────────────────────────────────────────────────────

/// Vulkan probe sürecindeki tek bir adımın ayrıntılı sonucu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulkanProbeStep {
    /// Adımın kısa adı (örn: "ICD Check")
    pub step: String,
    /// Bu adım başarılı mıydı?
    pub ok: bool,
    /// Başarısız olursa hata mesajı
    pub error: Option<String>,
    /// Başarılı olursa bulunan değer (örn: Vulkan version string)
    pub value: Option<String>,
}

impl VulkanProbeStep {
    pub fn success(step: impl Into<String>, value: impl Into<String>) -> Self {
        VulkanProbeStep {
            step: step.into(),
            ok: true,
            error: None,
            value: Some(value.into()),
        }
    }

    pub fn failure(step: impl Into<String>, error: impl Into<String>) -> Self {
        VulkanProbeStep {
            step: step.into(),
            ok: false,
            error: Some(error.into()),
            value: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Vulkan Probe Result — tüm adımların özeti
// ─────────────────────────────────────────────────────────────────────────────

/// Vulkan doğrulama sürecinin tüm adımlarını ve sonucunu içerir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulkanProbeResult {
    pub status: VulkanProbeStatus,
    pub steps: Vec<VulkanProbeStep>,
    /// Bulunan Vulkan instance version (varsa)
    pub instance_version: Option<String>,
    /// Bulunan fiziksel cihaz adı (varsa)
    pub device_name: Option<String>,
    /// Bulunan ICD dosyaları
    pub icd_files: Vec<String>,
    /// Adapter backend bilgisi
    pub backend: GpuBackend,
}

// ─────────────────────────────────────────────────────────────────────────────
// Full GPU Report — Tanılama sayfasına gönderilecek tam rapor
// ─────────────────────────────────────────────────────────────────────────────

/// GPU tanılama sisteminin ürettiği tam rapor.
/// Bu struct Frontend'deki gpu-diagnostics.js'e JSON olarak gönderilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullGpuReport {
    // ── Temel Bilgiler ──────────────────────────────────────────────
    pub vendor: String,
    pub vendor_enum: GpuVendor,
    pub renderer: String,
    pub driver_version: String,
    pub mesa_version: Option<String>,

    // ── API Durumları ───────────────────────────────────────────────
    pub vulkan_version: Option<String>,
    pub vulkan_status: VulkanProbeStatus,
    pub vulkan_steps: Vec<VulkanProbeStep>,
    pub vulkan_icd_files: Vec<String>,
    pub webgpu_status: WebGpuStatus,
    pub opengl_renderer: Option<String>,
    pub opengl_version: Option<String>,

    // ── Platform / Display ──────────────────────────────────────────
    pub display_server: DisplayServer,
    pub backend: GpuBackend,

    // ── Adapter / Device Bilgileri ──────────────────────────────────
    pub adapter_name: Option<String>,
    pub device_id: Option<String>,
    pub pci_vendor_id: Option<String>,
    pub pci_device_id: Option<String>,

    // ── Feature Desteği ─────────────────────────────────────────────
    pub vaapi_supported: bool,
    pub dmabuf_supported: bool,
    pub hw_accel: bool,
    pub video_decode: bool,
    pub video_encode: bool,
    pub msaa_supported: bool,
    pub compute_shader: bool,
    pub timestamp_query: bool,

    // ── Limitler ────────────────────────────────────────────────────
    pub max_texture_dimension: Option<u32>,
    pub max_storage_buffer_binding_size: Option<u64>,
    pub max_compute_workgroup_size: Option<u32>,

    // ── NVIDIA Spesifik ─────────────────────────────────────────────
    pub nvidia_driver_version: Option<String>,
    pub nvidia_is_proprietary: bool,
    pub nvidia_is_nouveau: bool,

    // ── AMD Spesifik ─────────────────────────────────────────────────
    pub amd_driver: Option<String>, // "RADV" veya "AMDVLK"

    // ── Intel Spesifik ───────────────────────────────────────────────
    pub intel_driver: Option<String>, // "ANV" veya "Mesa"

    // ── Tanılama Logu ────────────────────────────────────────────────
    pub init_log: Vec<LogEntry>,

    // ── Paket Yönetimi ───────────────────────────────────────────────
    pub pkg_manager: String,
    pub recommended_packages: String,
    pub recommended_command: String,
    pub has_pkexec: bool,

    // ── Hata Varsa ───────────────────────────────────────────────────
    pub critical_error: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Log Entry — tanılama adımlarını kayıt altına almak için
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ok: bool,
    pub label: String,
    pub detail: Option<String>,
}

impl LogEntry {
    pub fn ok(label: impl Into<String>) -> Self {
        LogEntry { ok: true, label: label.into(), detail: None }
    }

    pub fn ok_detail(label: impl Into<String>, detail: impl Into<String>) -> Self {
        LogEntry { ok: true, label: label.into(), detail: Some(detail.into()) }
    }

    pub fn fail(label: impl Into<String>, detail: impl Into<String>) -> Self {
        LogEntry { ok: false, label: label.into(), detail: Some(detail.into()) }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NvidiaDriverType
// ─────────────────────────────────────────────────────────────────────────────


// ─────────────────────────────────────────────────────────────────────────────
// AmdDriverType
// ─────────────────────────────────────────────────────────────────────────────


// ─────────────────────────────────────────────────────────────────────────────
// IntelDriverType
// ─────────────────────────────────────────────────────────────────────────────


// ─────────────────────────────────────────────────────────────────────────────
// Linux Distribution
// ─────────────────────────────────────────────────────────────────────────────

/// Tespit edilen Linux dağıtımı — paket önerileri için kullanılır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxDistro {
    Arch,
    Manjaro,
    EndeavourOs,
    Fedora,
    Ubuntu,
    Debian,
    Mint,
    PopOs,
    OpenSuse,
    NixOs,
    Unknown(String),
}

impl LinuxDistro {
    pub fn from_id(id: &str) -> Self {
        match id.trim().to_lowercase().as_str() {
            "arch" => LinuxDistro::Arch,
            "manjaro" => LinuxDistro::Manjaro,
            "endeavouros" => LinuxDistro::EndeavourOs,
            "fedora" => LinuxDistro::Fedora,
            "ubuntu" => LinuxDistro::Ubuntu,
            "debian" => LinuxDistro::Debian,
            "linuxmint" | "mint" => LinuxDistro::Mint,
            "pop" | "pop-os" => LinuxDistro::PopOs,
            "opensuse" | "opensuse-leap" | "opensuse-tumbleweed" => LinuxDistro::OpenSuse,
            "nixos" => LinuxDistro::NixOs,
            other => LinuxDistro::Unknown(other.to_string()),
        }
    }

    /// Arch tabanlı dağıtımlar için paket yöneticisi pacman
    pub fn pkg_manager(&self) -> &'static str {
        match self {
            LinuxDistro::Arch | LinuxDistro::Manjaro | LinuxDistro::EndeavourOs => "pacman",
            LinuxDistro::Fedora => "dnf",
            LinuxDistro::Ubuntu | LinuxDistro::Debian | LinuxDistro::Mint | LinuxDistro::PopOs => "apt",
            LinuxDistro::OpenSuse => "zypper",
            LinuxDistro::NixOs => "nix",
            LinuxDistro::Unknown(_) => "unknown",
        }
    }
}
