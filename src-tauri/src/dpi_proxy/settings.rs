// === OpenAnime — DPI Proxy Ayarları / Veritabanı Yönetimi ===
// goodbye_settings.json dosyasını okur, yazar, günceller

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;
use crate::dbg_log;

use super::methods::{DpiMethodRecord, MethodStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoodbyeSettings {
    pub version: u32,
    pub last_updated: String,

    /// En son başarılı yöntemin ID'si
    pub active_method_id: Option<u32>,

    /// Tüm denenmiş yöntemlerin kayıtları
    pub methods: Vec<DpiMethodRecord>,

    /// Proxy şu an çalışıyor mu?
    pub is_active: bool,

    /// ISP engellemesi tespit edildi mi?
    pub is_blocking_detected: bool,

    /// Son engel nedeni
    pub blocked_reason: String,

    /// Sistemde harici GoodbyeDPI çalışıyor mu?
    pub system_goodbye_running: bool,
}

impl Default for GoodbyeSettings {
    fn default() -> Self {
        Self {
            version: 1,
            last_updated: String::new(),
            active_method_id: None,
            methods: super::methods::ALL_METHODS
                .iter()
                .map(|m| DpiMethodRecord {
                    id: m.id,
                    status: MethodStatus::Untested,
                    success_count: 0,
                    fail_count: 0,
                    first_success: None,
                    last_tested: None,
                })
                .collect(),
            is_active: false,
            is_blocking_detected: false,
            blocked_reason: String::new(),
            system_goodbye_running: false,
        }
    }
}

impl GoodbyeSettings {
    fn default_path(app: &tauri::AppHandle) -> PathBuf {
        let local_data = app
            .path()
            .app_local_data_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        local_data.join("goodbye_settings.json")
    }

    /// Veritabanını yükle (yoksa varsayılan oluştur)
    pub fn load(app: &tauri::AppHandle) -> Self {
        let path = Self::default_path(app);
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str::<GoodbyeSettings>(&content) {
                        Ok(settings) => {
                            dbg_log!("[DPI Proxy] Ayarlar yüklendi: {}", path.display());
                            return settings;
                        }
                        Err(e) => {
                            dbg_log!(
                                "[DPI Proxy] Ayarlar bozuk, sıfırlanıyor: {}",
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    dbg_log!("[DPI Proxy] Ayarlar okunamadı: {}", e);
                }
            }
        }
        let default = Self::default();
        default.save(app);
        default
    }

    /// Veritabanını kaydet
    pub fn save(&self, app: &tauri::AppHandle) {
        let path = Self::default_path(app);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    dbg_log!("[DPI Proxy] Ayarlar yazılamadı: {}", e);
                } else {
                    dbg_log!("[DPI Proxy] Ayarlar kaydedildi: {}", path.display());
                }
            }
            Err(e) => {
                dbg_log!("[DPI Proxy] Ayarlar serialize edilemedi: {}", e);
            }
        }
    }

    /// Bir yöntemi başarılı olarak işaretle
    pub fn mark_method_success(&mut self, method_id: u32) {
        let now = chrono_now();
        if let Some(record) = self.methods.iter_mut().find(|m| m.id == method_id) {
            record.status = MethodStatus::Working;
            record.success_count += 1;
            record.last_tested = Some(now.clone());
            if record.first_success.is_none() {
                record.first_success = Some(now);
            }
        }
        self.active_method_id = Some(method_id);
        self.is_blocking_detected = true;
        self.blocked_reason = "bypassed".to_string();
    }

    /// Bir yöntemi başarısız olarak işaretle
    pub fn mark_method_fail(&mut self, method_id: u32) {
        if let Some(record) = self.methods.iter_mut().find(|m| m.id == method_id) {
            record.status = MethodStatus::Failed;
            record.fail_count += 1;
            record.last_tested = Some(chrono_now());
        }
    }

}

fn chrono_now() -> String {
    // Basit ISO 8601 zaman damgası (chrono kütüphanesi olmadan)
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Saniyeleri ISO formatına çevir (basit)
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // 2026-01-01'den beri gün sayısı (yaklaşık)
    let year = 1970 + (days as f64 / 365.25) as u64;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+03:00",
        year, 1, 1, hours, minutes, seconds
    )
}
