#![allow(dead_code)]
// === OpenAnime — GPU Algılama ve Donanım Bilgisi ===
//
// Windows: `wmic path win32_videocontroller get` komutu ile GPU listesi alınır.
// macOS: `system_profiler SPDisplaysDataType` komutu ile GPU listesi alınır.
// Bu yöntem Windows API tip uyumsuzluklarından etkilenmez.

use std::sync::Mutex;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use std::process::Command;

use serde::Serialize;

/// GPU donanım bilgisi — kullanıcıya gösterilmek üzere serileştirilir.
#[derive(Serialize, Clone, Debug)]
pub struct GpuInfo {
    pub vendor: String,
    pub adapter_name: String,
    pub vram_mb: u64,
    pub is_intel: bool,
    pub is_nvidia: bool,
    pub is_amd: bool,
    pub is_apple_silicon: bool,
    pub is_integrated: bool,
}

/// GPU algılama sonucu (Rust tarafında tutulan önbellek).
pub struct GpuState {
    pub detected: Mutex<Option<Vec<GpuInfo>>>,
    pub webgpu_vendor: Mutex<Option<String>>,
}

impl Default for GpuState {
    fn default() -> Self {
        Self {
            detected: Mutex::new(None),
            webgpu_vendor: Mutex::new(None),
        }
    }
}

/// Verilen vendor adından marka bilgilerini çıkar.
fn classify_vendor(name: &str) -> (bool, bool, bool, bool, bool) {
    let lower = name.to_lowercase();
    let is_intel = lower.contains("intel") || lower.contains("arc");
    let is_nvidia = lower.contains("nvidia") || lower.contains("geforce") || lower.contains("quadro");
    let is_amd = lower.contains("amd") || lower.contains("radeon") || lower.contains("firepro");
    let is_apple = lower.contains("apple");
    let is_integrated = is_intel || lower.contains("integrated");
    (is_intel, is_nvidia, is_amd, is_apple, is_integrated)
}

/// Vendor adından kısa etiket döndürür.
fn vendor_tag(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    if lower.contains("intel") || lower.contains("arc") {
        "Intel"
    } else if lower.contains("nvidia") || lower.contains("geforce") || lower.contains("quadro") {
        "NVIDIA"
    } else if lower.contains("amd") || lower.contains("radeon") || lower.contains("firepro") {
        "AMD"
    } else if lower.contains("apple") {
        "Apple"
    } else {
        "Diğer"
    }
}

/// Windows: `wmic path win32_videocontroller get name,adapterram /format:csv`
#[cfg(target_os = "windows")]
fn detect_windows_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    // wmic ile GPU listesini al
    let output = Command::new("wmic")
        .args(&["path", "win32_videocontroller", "get", "name,adapterram", "/format:csv"])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return gpus,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut first = true;

    for line in stdout.lines() {
        // İlk satır başlık satırıdır (Node,Name,AdapterRAM)
        if first {
            first = false;
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // CSV format: Node,Name,AdapterRAM
        let parts: Vec<&str> = trimmed.split(',').collect();
        if parts.len() < 2 {
            continue;
        }

        let name = if parts.len() >= 2 { parts[1].trim() } else { "" };
        let vram_str = if parts.len() >= 3 { parts[2].trim() } else { "" };

        if name.is_empty() {
            continue;
        }

        // VRAM byte -> MB
        let vram_mb: u64 = vram_str.parse::<u64>().unwrap_or(0) / (1024 * 1024);
        let (is_intel, is_nvidia, is_amd, is_apple, is_integrated) = classify_vendor(name);
        let vendor = vendor_tag(name);

        gpus.push(GpuInfo {
            vendor: vendor.to_string(),
            adapter_name: name.to_string(),
            vram_mb,
            is_intel,
            is_nvidia,
            is_amd,
            is_apple_silicon: is_apple,
            is_integrated,
        });
    }

    // wmic çalışmazsa (WoW64 vb.) reg.exe ile dene (yedek)
    if gpus.is_empty() {
        gpus = detect_windows_gpus_fallback();
    }

    gpus
}

/// Windows yedek yöntem: `reg query` ile GPU listesi
#[cfg(target_os = "windows")]
fn detect_windows_gpus_fallback() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    let output = Command::new("reg")
        .args(&[
            "query",
            "HKLM\\SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}",
            "/s",
            "/v",
            "DriverDesc",
        ])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return gpus,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        let trimmed = line.trim();
        // "    DriverDesc    REG_SZ    Intel(R) ..." formatını bekle
        if !trimmed.contains("REG_SZ") {
            continue;
        }
        let parts: Vec<&str> = trimmed.splitn(3, "REG_SZ").collect();
        if parts.len() < 2 {
            continue;
        }
        let name = parts[1].trim();
        if name.is_empty() {
            continue;
        }

        let (is_intel, is_nvidia, is_amd, is_apple, is_integrated) = classify_vendor(name);
        let vendor = vendor_tag(name);

        gpus.push(GpuInfo {
            vendor: vendor.to_string(),
            adapter_name: name.to_string(),
            vram_mb: 0,
            is_intel,
            is_nvidia,
            is_amd,
            is_apple_silicon: is_apple,
            is_integrated,
        });
    }

    gpus
}

/// macOS: `system_profiler SPDisplaysDataType` ile GPU listesi
#[cfg(target_os = "macos")]
fn detect_macos_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    let output = Command::new("system_profiler")
        .args(&["SPDisplaysDataType"])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return gpus,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current_name: Option<String> = None;
    let mut current_vram: u64 = 0;

    for line in stdout.lines() {
        let trimmed = line.trim();

        // "Chipset Model: ..."
        if let Some(val) = trimmed.strip_prefix("Chipset Model: ") {
            current_name = Some(val.trim().to_string());
        }

        // "VRAM (Total): ... MB" veya "VRAM: ... MB"
        if let Some(val) = trimmed.strip_prefix("VRAM (Total): ") {
            let vram_str = val.trim();
            if let Some(mb) = vram_str.strip_suffix(" MB") {
                current_vram = mb.trim().parse::<u64>().unwrap_or(0);
            }
        } else if let Some(val) = trimmed.strip_prefix("VRAM: ") {
            let vram_str = val.trim();
            if let Some(mb) = vram_str.strip_suffix(" MB") {
                current_vram = mb.trim().parse::<u64>().unwrap_or(0);
            }
        }

        // Yeni bir grafik kartı bölümü başlıyor (boş satır veya yeni bölüm)
        if trimmed.starts_with("Graphics/") || trimmed.starts_with("Displays:") {
            // Bir önceki kartı kaydet
            if let Some(name) = current_name.take() {
                let (is_intel, is_nvidia, is_amd, is_apple, is_integrated) = classify_vendor(&name);
                let vendor = vendor_tag(&name);
                let is_apple_silicon = std::env::consts::ARCH == "aarch64" && is_apple;

                gpus.push(GpuInfo {
                    vendor: vendor.to_string(),
                    adapter_name: name,
                    vram_mb: current_vram,
                    is_intel,
                    is_nvidia,
                    is_amd,
                    is_apple_silicon,
                    is_integrated,
                });
                current_vram = 0;
            }
        }
    }

    // Son kartı da ekle
    if let Some(name) = current_name {
        let (is_intel, is_nvidia, is_amd, is_apple, is_integrated) = classify_vendor(&name);
        let vendor = vendor_tag(&name);
        let is_apple_silicon = std::env::consts::ARCH == "aarch64" && is_apple;

        gpus.push(GpuInfo {
            vendor: vendor.to_string(),
            adapter_name: name,
            vram_mb: current_vram,
            is_intel,
            is_nvidia,
            is_amd,
            is_apple_silicon,
            is_integrated,
        });
    }

    // system_profiler çıktısı yoksa varsayılan
    if gpus.is_empty() {
        let is_apple_silicon = std::env::consts::ARCH == "aarch64";
        gpus.push(GpuInfo {
            vendor: if is_apple_silicon { "Apple" } else { "Intel" }.to_string(),
            adapter_name: if is_apple_silicon {
                "Apple Silicon (M Serisi)".to_string()
            } else {
                "Intel / AMD (Mac)".to_string()
            },
            vram_mb: 0,
            is_intel: !is_apple_silicon,
            is_nvidia: false,
            is_amd: false,
            is_apple_silicon,
            is_integrated: false,
        });
    }

    gpus
}

/// Diğer platformlarda boş liste.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn detect_other_gpus() -> Vec<GpuInfo> {
    Vec::new()
}

/// GPU bilgisini tespit eder (platforma özel).
pub fn detect() -> Vec<GpuInfo> {
    #[cfg(target_os = "windows")]
    {
        detect_windows_gpus()
    }
    #[cfg(target_os = "macos")]
    {
        detect_macos_gpus()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        detect_other_gpus()
    }
}

/// Ana GPU'yu seçer (ilk ayrık GPU öncelikli, yoksa ilk).
pub fn primary_gpu(gpus: &[GpuInfo]) -> Option<GpuInfo> {
    gpus.iter().find(|g| !g.is_integrated).cloned().or_else(|| gpus.first().cloned())
}

/// GPU markasına göre kullanılacak WebView2 ek bayraklarını döndürür.
pub fn vendor_hint(gpus: &[GpuInfo]) -> Option<&'static str> {
    let primary = primary_gpu(gpus)?;
    if primary.is_intel {
        Some("intel")
    } else if primary.is_nvidia {
        Some("nvidia")
    } else if primary.is_amd {
        Some("amd")
    } else if primary.is_apple_silicon {
        Some("apple-silicon")
    } else {
        None
    }
}

/// WebGPU adapter vendor bilgisini JS'ten alıp state'e yazar.
#[tauri::command]
pub fn oa_set_webgpu_vendor(state: tauri::State<'_, GpuState>, vendor: String) -> Result<(), String> {
    let mut v = state.webgpu_vendor.lock().map_err(|e| e.to_string())?;
    *v = Some(vendor);
    Ok(())
}

/// GPU bilgisini döndürür (ilk çağrıda tespit edilir ve önbelleğe alınır).
#[tauri::command]
pub fn oa_get_gpu_info(state: tauri::State<'_, GpuState>) -> Result<Vec<GpuInfo>, String> {
    let mut cache = state.detected.lock().map_err(|e| e.to_string())?;
    if cache.is_none() {
        *cache = Some(detect());
    }
    Ok(cache.clone().unwrap_or_default())
}

/// Ana GPU'nun markasını döndürür ("intel" / "nvidia" / "amd" / "apple" / "bilinmiyor").
#[tauri::command]
pub fn oa_get_gpu_hint(state: tauri::State<'_, GpuState>) -> Result<String, String> {
    let cache = state.detected.lock().map_err(|e| e.to_string())?;
    if let Some(gpus) = cache.as_ref() {
        let primary = primary_gpu(gpus);
        if let Some(g) = primary {
            if g.is_intel { return Ok("intel".to_string()); }
            if g.is_nvidia { return Ok("nvidia".to_string()); }
            if g.is_amd { return Ok("amd".to_string()); }
            if g.is_apple_silicon { return Ok("apple".to_string()); }
            return Ok("bilinmiyor".to_string());
        }
    }
    Ok("bilinmiyor".to_string())
}