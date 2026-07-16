use std::sync::Mutex;
use std::time::{Instant, Duration};
use serde::Serialize;
use tauri_plugin_updater::Update;
use tauri::{Emitter, Manager};

pub struct UpdaterState {
    pub current_update: Mutex<Option<Update>>,
    pub cache: Mutex<Option<(Instant, String, serde_json::Value)>>,
    pub is_downloading: Mutex<bool>,
}

impl UpdaterState {
    pub fn new() -> Self {
        Self {
            current_update: Mutex::new(None),
            cache: Mutex::new(None),
            is_downloading: Mutex::new(false),
        }
    }
}

#[allow(dead_code)]
#[derive(Serialize, Clone, Debug)]
pub struct UpdateCheckResult {
    pub available: bool,
    pub version: String,
    pub date: Option<String>,
    pub body: Option<String>,
}

#[tauri::command]
pub fn get_app_version(app: tauri::AppHandle) -> String {
    app.config().version.clone().unwrap_or_else(|| "1.0.2-beta-02".to_string())
}

#[tauri::command]
pub async fn check_for_updates(
    app: tauri::AppHandle,
    state: tauri::State<'_, UpdaterState>,
    channel: String,
    target_version: Option<String>,
    force: Option<bool>,
) -> Result<serde_json::Value, String> {
    let is_rollback = target_version.is_some();
    let is_force = force.unwrap_or(false);
    
    // 1. Cache Kontrolü (Rollback veya Force değilse ve 5 dakika geçerliyse)
    if !is_rollback && !is_force {
        let cache_lock = state.cache.lock().unwrap();
        if let Some((instant, cached_channel, data)) = &*cache_lock {
            if instant.elapsed() < Duration::from_secs(300) && cached_channel == &channel {
                crate::log!("[Updater] Returning cached update manifest for channel: {}", channel);
                return Ok(data.clone());
            }
        }
    }

    // 2. Kanal / Hedef Sürüm URL'sini belirleme
    let mut url = if let Some(version) = &target_version {
        format!(
            "https://github.com/Dark-Hunter-TR/OpenAnime-Desktops/releases/download/{}/latest.json",
            version
        )
    } else {
        match channel.to_lowercase().as_str() {
            "beta" => "https://raw.githubusercontent.com/Dark-Hunter-TR/OpenAnime-Desktops/main/updater/latest-beta.json".to_string(),
            "alpha" => "https://raw.githubusercontent.com/Dark-Hunter-TR/OpenAnime-Desktops/main/updater/latest-alpha.json".to_string(),
            _ => "https://github.com/Dark-Hunter-TR/OpenAnime-Desktops/releases/latest/download/latest.json".to_string(),
        }
    };

    if is_force && url.contains("raw.githubusercontent.com") {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        url = format!("{}?t={}", url, timestamp);
    }

    crate::log!(
        "[Updater] Checking updates on URL: {} (Rollback: {}, Force: {})",
        url, is_rollback, is_force
    );

    // 3. Tauri Updater Builder yapılandırması
    use tauri_plugin_updater::UpdaterExt;
    let mut builder = app.updater_builder();
    builder = builder.endpoints(vec![url.parse().map_err(|e| format!("URL Parse Hatası: {}", e))?])
        .map_err(|e| format!("Updater endpoints hatası: {}", e))?;
    
    // Eğer rollback yapılıyorsa, sürüm karşılaştırmasını bypass edip her zaman güncellemeye izin ver
    if is_rollback {
        builder = builder.version_comparator(|_current, _remote| {
            crate::log!("[Updater] Rollback active, version comparison bypassed.");
            true
        });
    }

    let updater = builder.build().map_err(|e| format!("Updater Build Hatası: {}", e))?;
    let update_result = updater.check().await.map_err(|e| {
        crate::log!("[Updater] Güncelleme sorgusu başarısız: {}", e);
        format!("Güncelleme kontrolü başarısız: {}", e)
    })?;

    let response = if let Some(update) = update_result {
        // Rust state'ine bu update'i kaydet (indirme/kurulum için kullanılacak)
        let mut current_update = state.current_update.lock().unwrap();
        *current_update = Some(update.clone());

        let date_str = update.date.map(|d| d.to_string());

        serde_json::json!({
            "available": true,
            "version": update.version,
            "date": date_str,
            "body": update.body,
        })
    } else {
        serde_json::json!({
            "available": false,
        })
    };

    // Cache'le (sadece normal ve force olmayan durumlar için)
    if !is_rollback && !is_force {
        let mut cache_lock = state.cache.lock().unwrap();
        *cache_lock = Some((Instant::now(), channel, response.clone()));
    }

    Ok(response)
}

#[tauri::command]
pub async fn start_update_download(
    app: tauri::AppHandle,
    state: tauri::State<'_, UpdaterState>,
) -> Result<(), String> {
    // Birden fazla indirmeyi önle
    {
        let mut downloading = state.is_downloading.lock().unwrap();
        if *downloading {
            return Err("İndirme işlemi zaten devam ediyor.".to_string());
        }
        *downloading = true;
    }

    let update = {
        let current_update = state.current_update.lock().unwrap();
        current_update.clone()
    };

    let update = match update {
        Some(u) => u,
        None => {
            let mut downloading = state.is_downloading.lock().unwrap();
            *downloading = false;
            return Err("İndirilecek aktif güncelleme bulunamadı. Önce kontrol edin.".to_string());
        }
    };

    let app_c = app.clone();

    // İndirmeyi asenkron arka planda çalıştır
    tauri::async_runtime::spawn(async move {
        let state_c = app_c.state::<UpdaterState>();
        let mut downloaded = 0;
        let mut content_length = None;
        
        let app_progress = app_c.clone();
        let result = update.download_and_install(
            move |chunk_length, total_length| {
                downloaded += chunk_length;
                if content_length.is_none() {
                    content_length = total_length;
                }
                
                let percent = if let Some(total) = content_length {
                    (downloaded as f64 / total as f64 * 100.0).round() as u32
                } else {
                    0
                };

                let _ = app_progress.emit("openanime://update-progress", serde_json::json!({
                    "status": "downloading",
                    "downloaded": downloaded,
                    "total": content_length,
                    "percent": percent
                }));
            },
            move || {
                crate::log!("[Updater] Download completed, invoking silent install...");
            }
        ).await;

        // İndirme bitti veya hata aldı, bayrağı sıfırla
        {
            let mut downloading = state_c.is_downloading.lock().unwrap();
            *downloading = false;
        }

        match result {
            Ok(_) => {
                let _ = app_c.emit("openanime://update-progress", serde_json::json!({
                    "status": "success",
                    "percent": 100
                }));
                crate::log!("[Updater] Update finished successfully. App should exit now.");
            }
            Err(e) => {
                crate::log!("[Updater] Güncelleme hatası: {}", e);
                let _ = app_c.emit("openanime://update-progress", serde_json::json!({
                    "status": "error",
                    "message": format!("Güncelleme başarısız: {}", e)
                }));
            }
        }
    });
    Ok(())
}

// (debug_log kaldırıldı — hiçbir JS tarafından çağrılmıyordu.)
