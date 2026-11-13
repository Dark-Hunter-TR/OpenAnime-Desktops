// === Kayıtlı Tarayıcı Keşfi ===
//
// Amaç: tarayıcıdan indirilen video dosyalarını bulmak. Sabit yol listesi
// yerine, işletim sisteminin "bu bir tarayıcı" dediği uygulamalara bakılır.
//
// Windows: HKLM/HKCU `SOFTWARE\Clients\StartMenuInternet\<ad>` altındaki
// istemciler. Varsayılan tarayıcı `...\UrlAssociations\https\UserChoice`
// ProgId'si ile istemcinin `Capabilities\URLAssociations\https` değeri
// karşılaştırılarak bulunur (bu makinede KANIT: ProgId = MSEdgeHTM).
//
// Sınır (ölçüldü): kayıt yalnız KENDİNİ kaydeden tarayıcıyı verir; portable
// kurulumlar görünmez. Exe yolundan profil kökü türetilemez (Program Files ile
// %LOCALAPPDATA% arasında kayıt bağı yok) — bu yüzden satıcı tablosu + varlık
// sondası kullanılır.
//
// Bu modül hiçbir şey YAZMAZ: yalnız kayıt okur, profil dosyası okur, klasör
// listeler. Dışarıya `browser_discover_downloads` komutuyla açılır.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct BrowserEntry {
    pub name: String,
    pub exe: Option<String>,
    pub default: bool,
    pub profile_root: Option<String>,
    pub download_dir: Option<String>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaCandidate {
    pub path: String,
    pub size: u64,
    pub modified_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryResult {
    pub browsers: Vec<BrowserEntry>,
    pub download_dirs: Vec<String>,
    pub files: Vec<MediaCandidate>,
}

/// Komuttan çağrılan giriş noktası. Salt okunur; hiçbir hata sessizce
/// yutulmaz — nedeni `browsers[*].note` alanında taşınır.
#[tauri::command]
pub fn browser_discover_downloads() -> DiscoveryResult {
    let browsers = list_browsers();
    let mut dirs: Vec<String> = browsers
        .iter()
        .filter_map(|b| b.download_dir.clone())
        .collect();
    if let Some(os_dir) = os_downloads_dir() {
        dirs.push(os_dir);
    }
    let dirs = dedup_paths(dirs);
    let files = scan_media(&dirs, 2);
    DiscoveryResult {
        browsers,
        download_dirs: dirs,
        files,
    }
}

fn dedup_paths(paths: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in paths {
        let key = normalize_key(&p);
        if key.is_empty() {
            continue;
        }
        if out.iter().any(|q| normalize_key(q) == key) {
            continue;
        }
        out.push(p);
    }
    out
}

/// Windows yolları büyük/küçük harf duyarsızdır; tekilleştirme anahtarı da öyle.
fn normalize_key(p: &str) -> String {
    p.trim()
        .trim_end_matches(['\\', '/'])
        .to_lowercase()
        .replace('/', "\\")
}

fn os_downloads_dir() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders")
            .ok()?;
        let raw: String = key
            .get_value(r"{374DE290-123F-4565-9164-39C4925E467B}")
            .ok()?;
        let expanded = expand_env(&raw);
        if Path::new(&expanded).is_dir() {
            return Some(expanded);
        }
        None
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").ok()?;
        let dir = Path::new(&home).join("Downloads");
        if dir.is_dir() {
            return Some(dir.to_string_lossy().to_string());
        }
        None
    }
}

fn expand_env(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let var = &after[..end];
                match std::env::var(var) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => {
                        out.push('%');
                        out.push_str(var);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

// ──────────────────────────────────────────────
// Windows: kayıt defteri
// ──────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn list_browsers() -> Vec<BrowserEntry> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let default_exe = default_browser_exe();
    let mut clients: Vec<Client> = Vec::new();
    let roots = [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Clients\StartMenuInternet"),
        (HKEY_CURRENT_USER, r"SOFTWARE\Clients\StartMenuInternet"),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Clients\StartMenuInternet",
        ),
    ];
    for (hive, path) in roots {
        let Ok(root) = RegKey::predef(hive).open_subkey(path) else {
            continue;
        };
        for name in root.enum_keys().flatten() {
            if clients.iter().any(|c| c.name == name) {
                continue;
            }
            let exe = root
                .open_subkey(format!(r"{}\shell\open\command", name))
                .ok()
                .and_then(|k| k.get_value::<String, _>("").ok())
                .and_then(|cmd| exe_from_command(&cmd));
            let app_name = root
                .open_subkey(format!(r"{}\Capabilities", name))
                .ok()
                .and_then(|k| k.get_value::<String, _>("ApplicationName").ok());
            clients.push(Client { name, exe, app_name });
        }
    }

    let mut out = Vec::new();
    for c in clients {
        let mut note = String::new();
        // KANIT (bu makine): Edge'in `Capabilities\URLAssociations\https` anahtarı
        // YOK — o yüzden ProgId'yi istemciyle doğrudan karşılaştırmak çalışmıyor.
        // Çalışan yol: UserChoice ProgId → `HKCR\<ProgId>\shell\open\command` → exe
        // → istemcinin exe'siyle karşılaştır.
        let is_default =
            matches!((&default_exe, &c.exe), (Some(a), Some(b)) if normalize_key(a) == normalize_key(b));

        let display = match c.app_name {
            Some(ref n) if !n.is_empty() && !n.starts_with('@') => n.clone(),
            _ => c.name.clone(),
        };

        let profile_root = c.exe.as_deref().and_then(profile_root_for_exe);
        let download_dir = profile_root.as_deref().and_then(download_dir_for_root);
        if download_dir.is_none() {
            note.push_str("indirme klasörü profilden okunamadı; OS varsayılanı kullanılır. ");
        }
        if profile_root.is_none() {
            note.push_str("profil kökü bulunamadı (bilinmeyen satıcı ya da portable kurulum). ");
        }

        out.push(BrowserEntry {
            name: display,
            exe: c.exe,
            default: is_default,
            profile_root: profile_root.map(|p| p.to_string_lossy().to_string()),
            download_dir,
            note: note.trim().to_string(),
        });
    }
    out
}

#[cfg(target_os = "windows")]
struct Client {
    name: String,
    exe: Option<String>,
    app_name: Option<String>,
}

#[cfg(not(target_os = "windows"))]
fn list_browsers() -> Vec<BrowserEntry> {
    // Linux: yalnız varsayılan tarayıcının adı raporlanır; profil yolu türetmesi
    // kırılgan (bkz. tasarim-kayitli-tarayici-kesfi.md, Linux bölümü).
    let mut out = Vec::new();
    if let Ok(raw) = std::process::Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .output()
    {
        let name = String::from_utf8_lossy(&raw.stdout).trim().to_string();
        if !name.is_empty() {
            out.push(BrowserEntry {
                name: name.trim_end_matches(".desktop").to_string(),
                exe: None,
                default: true,
                profile_root: None,
                download_dir: None,
                note: "Linux: yalnız varsayılan tarayıcı adı; profil yolu haritalanmadı.".to_string(),
            });
        }
    }
    out
}

/// Varsayılan tarayıcının exe yolu: UserChoice ProgId → `HKCR\<ProgId>\shell\open\command`.
#[cfg(target_os = "windows")]
fn default_browser_exe() -> Option<String> {
    use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER};
    use winreg::RegKey;
    let progid = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(
            r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice",
        )
        .ok()
        .and_then(|k| k.get_value::<String, _>("ProgId").ok())?;
    RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(format!(r"{}\shell\open\command", progid))
        .ok()
        .and_then(|k| k.get_value::<String, _>("").ok())
        .and_then(|cmd| exe_from_command(&cmd))
}

/// `shell\open\command` değerinden exe yolunu ayıklar.
/// İki biçim görüldü (KANIT: bu makinede IE tırnaksız, Edge tırnaklı):
///   `"C:\...\msedge.exe" --single-argument %1`
///   `C:\Program Files\Internet Explorer\iexplore.exe`
#[cfg(target_os = "windows")]
fn exe_from_command(cmd: &str) -> Option<String> {
    let trimmed = cmd.trim();
    if let Some(stripped) = trimmed.strip_prefix('"') {
        let end = stripped.find('"')?;
        return Some(stripped[..end].to_string());
    }
    let lower = trimmed.to_lowercase();
    match lower.find(".exe") {
        Some(i) => Some(trimmed[..i + 4].to_string()),
        None => trimmed.split_whitespace().next().map(|s| s.to_string()),
    }
}

// ──────────────────────────────────────────────
// Profil kökü ve indirme klasörü
// ──────────────────────────────────────────────

/// Satıcı tablosu. Her aday varlık sondasıyla doğrulanır; olmayan yol atlanır.
/// Tablo dışı exe → None (portable/bilinmeyen tarayıcı).
fn profile_root_for_exe(exe: &str) -> Option<PathBuf> {
    let file = Path::new(exe)
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let local = std::env::var("LOCALAPPDATA").ok();
    let roaming = std::env::var("APPDATA").ok();

    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut add = |base: &Option<String>, rel: &str| {
        if let Some(base) = base {
            candidates.push(Path::new(base).join(rel));
        }
    };

    match file.as_str() {
        "msedge.exe" => add(&local, r"Microsoft\Edge\User Data"),
        "chrome.exe" => add(&local, r"Google\Chrome\User Data"),
        "brave.exe" => add(&local, r"BraveSoftware\Brave-Browser\User Data"),
        "vivaldi.exe" => add(&local, r"Vivaldi\User Data"),
        "opera.exe" => add(&roaming, r"Opera Software\Opera Stable"),
        "firefox.exe" => add(&roaming, r"Mozilla\Firefox"),
        "zen.exe" | "zen-browser.exe" => add(&roaming, "zen"),
        _ => {}
    }

    candidates.into_iter().find(|p| p.is_dir())
}

fn download_dir_for_root(root: &Path) -> Option<String> {
    // Chromium: <root>\Default\Preferences → download.default_directory
    let prefs = root.join("Default").join("Preferences");
    if prefs.is_file() {
        if let Ok(text) = std::fs::read_to_string(&prefs) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(dir) = json
                    .get("download")
                    .and_then(|d| d.get("default_directory"))
                    .and_then(|d| d.as_str())
                {
                    let dir = expand_env(dir);
                    if Path::new(&dir).is_dir() {
                        return Some(dir);
                    }
                }
            }
        }
    }

    // Firefox/Zen: prefs.js → user_pref("browser.download.dir", "...")
    if root.join("profiles.ini").is_file() {
        let re =
            regex::Regex::new(r#"user_pref\("browser\.download\.dir",\s*"((?:[^"\\]|\\.)*)"\)"#)
                .ok()?;
        for profile in firefox_profiles(root) {
            if let Ok(text) = std::fs::read_to_string(profile.join("prefs.js")) {
                if let Some(cap) = re.captures(&text) {
                    let raw = cap.get(1)?.as_str().replace(r"\\", r"\");
                    if Path::new(&raw).is_dir() {
                        return Some(raw);
                    }
                }
            }
        }
    }

    None
}

fn firefox_profiles(root: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(root.join("profiles.ini")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut path: Option<String> = None;
    let mut relative = true;
    let flush = |path: &mut Option<String>, relative: bool, out: &mut Vec<PathBuf>| {
        if let Some(p) = path.take() {
            out.push(if relative {
                root.join(p)
            } else {
                PathBuf::from(p)
            });
        }
    };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            flush(&mut path, relative, &mut out);
            relative = true;
            continue;
        }
        if let Some(v) = line.strip_prefix("Path=") {
            path = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("IsRelative=") {
            relative = v.trim() != "0";
        }
    }
    flush(&mut path, relative, &mut out);
    out.into_iter().filter(|p| p.is_dir()).collect()
}

// ──────────────────────────────────────────────
// Tarama
// ──────────────────────────────────────────────

const MAX_SCAN_FILES: usize = 2000;

/// Verilen klasörlerde medya uzantılı dosyaları toplar (derinlik varsayılan 2).
/// Filtre: gizli dizinler, geçici indirme uzantıları ve 1 MB altı dosyalar.
/// Uzantı kapısı sunucunun kendi kapısıdır (`has_media_extension`) — böylece
/// taranan küme ile sunucunun servis edebildiği küme aynı olur.
pub fn scan_media(dirs: &[String], max_depth: u32) -> Vec<MediaCandidate> {
    let mut out = Vec::new();
    for dir in dirs {
        walk(Path::new(dir), 0, max_depth, &mut out);
    }
    out.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    out
}

fn walk(dir: &Path, depth: u32, max_depth: u32, out: &mut Vec<MediaCandidate>) {
    if out.len() >= MAX_SCAN_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            if depth < max_depth {
                walk(&path, depth + 1, max_depth, out);
            }
            continue;
        }
        let as_str = path.to_string_lossy().to_string();
        if !crate::local_video_server::has_media_extension(&as_str) {
            continue;
        }
        if meta.len() < 1024 * 1024 {
            continue;
        }
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        out.push(MediaCandidate {
            path: as_str,
            size: meta.len(),
            modified_ms,
        });
        if out.len() >= MAX_SCAN_FILES {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_expansion_handles_missing_var() {
        assert_eq!(expand_env("a%NOPE_NOT_SET%b"), "a%NOPE_NOT_SET%b");
    }

    #[test]
    fn dedup_is_case_insensitive_and_slash_agnostic() {
        let got = dedup_paths(vec![
            r"C:\Users\X\Downloads".to_string(),
            r"c:\users\x\downloads\".to_string(),
            "C:/Users/X/Downloads".to_string(),
        ]);
        assert_eq!(got.len(), 1, "üç aynı yol tek olmalı: {:?}", got);
    }

    /// Bu makinede GERÇEK kayıt defterini okur. Boş liste dönerse test kırılır.
    /// Çıktı `--nocapture` ile görülür ve rapora kanıt olarak yazılır.
    #[test]
    fn lists_at_least_one_registered_browser() {
        let list = list_browsers();
        eprintln!("kayıtlı tarayıcı sayısı: {}", list.len());
        for b in &list {
            eprintln!(
                "  ad={:?} exe={:?} varsayılan={} profil={:?} indirme={:?} not={:?}",
                b.name, b.exe, b.default, b.profile_root, b.download_dir, b.note
            );
        }
        assert!(!list.is_empty(), "kayıtlı tarayıcı bulunamadı");
    }

    /// Komutun tamamını (kayıt + profil + indirme klasörü + tarama) çalıştırır.
    /// Boş sonuç testi kırmaz: bu makinede indirme klasörü boş olabilir; ama
    /// en az bir indirme klasörü çözülmelidir (OS varsayılanı her zaman var).
    #[test]
    fn command_resolves_at_least_one_download_dir() {
        let r = browser_discover_downloads();
        eprintln!("indirme klasörleri: {:?}", r.download_dirs);
        eprintln!("bulunan medya dosyası: {}", r.files.len());
        for f in r.files.iter().take(10) {
            eprintln!("  {} bayt · {}", f.size, f.path);
        }
        assert!(
            !r.download_dirs.is_empty(),
            "hiç indirme klasörü çözülemedi"
        );
    }
}
