//! "OpenAnime Theme" — Desktops'tan tamamen ayrı, bağımsız kurulan bir Tauri
//! uygulaması (tema oluşturma editörü, bkz. `D:\OpenAnime-Theme`). Bu modül
//! yalnızca "kurulu mu, kuruluysa aç, değilse indirip kur" akışını sağlar —
//! Theme'in oluşturduğu temaların Desktops'a aktarılması ayrı ve bağlanmamış
//! bir konu (bkz. `list_themes`/`load_theme`/`apply_theme_css`), bu modülün
//! kapsamı dışında.
//!
//! ## Tespit
//!
//! Theme'in NSIS kurulumu Desktops'unkiyle AYNI Tauri şablonunu kullanıyor:
//! `HKCU\...\Uninstall\OpenAnime Theme` (varsayılan kurulum modu `currentUser`
//! olduğu için önce HKCU, sonra emin olmak için HKLM da denenir) altında
//! `InstallLocation` yazıyor. Aynı anahtar `installer.nsi` -> `Section
//! Install` içinde `WriteRegStr SHCTX "${UNINSTKEY}" ...` ile üretiliyor.
//!
//! ## Kurulum
//!
//! `updater.rs`'teki kanal manifestiyle AYNI biçim ve AYNI depoya bağlı bir
//! dosya kullanılıyor (Theme kendi CI'ında bunu üretiyor): sürüm + platform
//! başına imzalı indirme adresi. Yalnızca `url` alanı okunuyor, imza kontrolü
//! yapılmıyor — installer zaten NSIS imzalı paket + Theme'in kendi updater
//! pubkey doğrulamasından bağımsız, Windows Authenticode imzasına güveniyoruz
//! (Desktops'un NSIS'i de üçüncü taraf installer'lar için başka bir doğrulama
//! yapmıyor, bkz. WebView2 bootstrapper indirmesi).

use serde::Serialize;

const THEME_MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/Dark-Hunter-TR/OpenAnime-Thema/main/updater/latest-stable.json";
#[cfg(windows)]
const THEME_UNINSTALL_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenAnime Theme";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ThemeAppStatus {
    pub installed: bool,
    pub install_path: Option<String>,
    /// Frontend'in platforma göre farklı bir mesaj/akış göstermesi için
    /// (yalnızca Windows'ta gerçek tespit + otomatik kurulum var).
    pub platform_supported: bool,
}

#[cfg(windows)]
fn find_install() -> Option<String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(hive);
        if let Ok(key) = root.open_subkey(THEME_UNINSTALL_KEY) {
            if let Ok(loc) = key.get_value::<String, _>("InstallLocation") {
                let trimmed = loc.trim().trim_matches('"');
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn find_install() -> Option<String> {
    None
}

#[tauri::command]
pub fn theme_app_status() -> ThemeAppStatus {
    let platform_supported = cfg!(windows);
    match find_install() {
        Some(path) => ThemeAppStatus {
            installed: true,
            install_path: Some(path),
            platform_supported,
        },
        None => ThemeAppStatus {
            installed: false,
            install_path: None,
            platform_supported,
        },
    }
}

#[tauri::command]
pub fn open_theme_app() -> Result<(), String> {
    #[cfg(windows)]
    {
        let dir = find_install().ok_or_else(|| "OpenAnime Theme kurulu değil.".to_string())?;
        let exe = std::path::Path::new(&dir).join("openanime-theme.exe");
        std::process::Command::new(&exe)
            .spawn()
            .map_err(|e| format!("OpenAnime Theme başlatılamadı: {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("OpenAnime Theme yalnızca Windows'ta otomatik açılabiliyor.".to_string())
    }
}

/// Manifesti çekip Windows kurulum paketinin indirme adresini döner.
async fn fetch_theme_setup_url() -> Result<String, String> {
    let text = reqwest::get(THEME_MANIFEST_URL)
        .await
        .map_err(|e| format!("Manifest alınamadı: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Manifest okunamadı: {e}"))?;
    let manifest: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Manifest çözümlenemedi: {e}"))?;

    manifest
        .pointer("/platforms/windows-x86_64/url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Manifestte Windows paketi bulunamadı.".to_string())
}

/// `openanime://theme-install-progress` olarak yayınlanan ilerleme.
///
/// Güncelleyicinin (`updater.rs`) `openanime://update-progress` olayıyla
/// AYNI şekle sahip ama BİLEREK ayrı bir olay adında — ikisi teorik olarak
/// aynı anda tetiklenebilir (kullanıcı bu modalı açıkken uygulama güncellemesi
/// de bulunabilir), aynı event adı iki modalın ilerleme çubuğunu karıştırırdı.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ThemeInstallProgress {
    status: &'static str,
    percent: u32,
    message: Option<String>,
}

#[tauri::command]
pub async fn install_theme_app(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = app;
        return Err("Bu platformda otomatik kurulum desteklenmiyor. Theme'in GitHub Releases sayfasından indirebilirsiniz.".to_string());
    }

    #[cfg(windows)]
    {
        use tauri::Emitter;

        let emit = |status: &'static str, percent: u32, message: Option<String>| {
            let _ = app.emit(
                "openanime://theme-install-progress",
                ThemeInstallProgress {
                    status,
                    percent,
                    message,
                },
            );
        };

        emit("downloading", 0, None);

        let url = fetch_theme_setup_url().await.map_err(|e| {
            emit("error", 0, Some(e.clone()));
            e
        })?;

        let client = reqwest::Client::new();
        let mut resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                let msg = format!("İndirme başlatılamadı: {e}");
                emit("error", 0, Some(msg.clone()));
                msg
            })?;

        let total = resp.content_length();
        let tmp_path = std::env::temp_dir().join("oa_theme_setup.exe");

        {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::File::create(&tmp_path).await.map_err(|e| {
                let msg = format!("Geçici dosya oluşturulamadı: {e}");
                emit("error", 0, Some(msg.clone()));
                msg
            })?;

            let mut downloaded: u64 = 0;
            while let Some(chunk) = resp.chunk().await.map_err(|e| {
                let msg = format!("İndirme hatası: {e}");
                emit("error", 0, Some(msg.clone()));
                msg
            })? {
                file.write_all(&chunk).await.map_err(|e| {
                    let msg = format!("Yazma hatası: {e}");
                    emit("error", 0, Some(msg.clone()));
                    msg
                })?;
                downloaded += chunk.len() as u64;
                let percent = match total {
                    Some(t) if t > 0 => ((downloaded as f64 / t as f64) * 100.0).round() as u32,
                    _ => 0,
                };
                emit("downloading", percent, None);
            }
        }

        emit("installing", 100, None);

        // Pasif mod (/P): Theme'in kendi kurulum sayfasını gösterir ama
        // Welcome/License/Directory adımlarını atlar — kullanıcının burada
        // ikinci bir sihirbazı elle yönetmesi gerekmez. Aynı bayrak Desktops'un
        // kendi installer.nsi'sinde de destekleniyor (bkz. installer.nsi).
        let status = tokio::process::Command::new(&tmp_path)
            .arg("/P")
            .status()
            .await
            .map_err(|e| {
                let msg = format!("Kurulum başlatılamadı: {e}");
                emit("error", 0, Some(msg.clone()));
                msg
            })?;

        let _ = std::fs::remove_file(&tmp_path);

        if !status.success() {
            let msg = "Kurulum başarısız oldu.".to_string();
            emit("error", 0, Some(msg.clone()));
            return Err(msg);
        }

        emit("success", 100, None);
        Ok(())
    }
}
