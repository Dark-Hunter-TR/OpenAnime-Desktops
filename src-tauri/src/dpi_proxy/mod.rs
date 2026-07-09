// === OpenAnime — DPI Proxy Ana Modülü ===
// Tüm DPI atlatma sistemini yönetir: proxy, ayarlar, bağlantı kontrolü

mod http_mod;
pub mod methods;
pub mod settings;
mod tcp_forward;
mod tls_detect;

use std::sync::Arc;
use std::time::Duration;
use tauri::Manager;
use tokio::sync::Mutex;

pub use methods::DpiMethod;
pub use settings::GoodbyeSettings;

/// DPI atlatma durumunu frontend'e bildirir
#[derive(Debug, Clone, serde::Serialize)]
pub struct DpiStatus {
    pub proxy_running: bool,
    pub active_method_id: Option<u32>,
    pub active_method_name: String,
    pub is_blocking_detected: bool,
    pub blocked_reason: String,
    pub system_goodbye_running: bool,
}

/// check_connection()'un detaylı sonucu
#[derive(Debug, Clone, serde::Serialize)]
pub enum ConnectionResult {
    Ok,
    Timeout,
    Forbidden,
    DnsFailure,
    ServerError,
    TlsError,
    NetworkUnreachable,
}

/// DPI Proxy Yöneticisi — app başlatılırken oluşturulur
pub struct DpiProxyManager {
    pub settings: Mutex<GoodbyeSettings>,
    pub proxy_running: Arc<Mutex<bool>>,
    pub current_method: Mutex<Option<DpiMethod>>,
}

impl DpiProxyManager {
    pub fn new(app: &tauri::AppHandle) -> Self {
        let settings = GoodbyeSettings::load(app);
        let system_running = is_system_goodbye_running();

        println!(
            "[DPI Proxy] Sistemde harici GoodbyeDPI: {}",
            if system_running { "EVET" } else { "HAYIR" }
        );

        // system_goodbye_running alanını güncelle
        let mut settings = settings;
        settings.system_goodbye_running = system_running;
        settings.save(app);

        Self {
            settings: Mutex::new(settings),
            proxy_running: Arc::new(Mutex::new(false)),
            current_method: Mutex::new(None),
        }
    }

    /// Proxy'yi başlat (arkaplan task'i)
    pub async fn start_proxy(&self, app: &tauri::AppHandle, method_id: u32) -> Result<(), String> {
        // Zaten çalışıyorsa durdur
        {
            let mut running = self.proxy_running.lock().await;
            if *running {
                *running = false;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        let method = methods::get_method_by_id(method_id)
            .ok_or_else(|| format!("Yöntem bulunamadı: {}", method_id))?;

        println!(
            "[DPI Proxy] Proxy başlatılıyor... yöntem #{}: {}",
            method_id, method.name
        );

        // method'ı owned hale getir (static'ten clone)
        let method_owned = method.clone();

        *self.current_method.lock().await = Some(method_owned.clone());
        *self.proxy_running.lock().await = true;

        // Arkaplanda proxy task'ini başlat
        let running = self.proxy_running.clone();
        tokio::spawn(async move {
            tcp_forward::start_proxy_internal(method_owned, running).await;
        });

        // Proxy'nin başlaması için kısa bekle
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Ayarları güncelle
        let mut settings = self.settings.lock().await;
        settings.is_active = true;
        settings.save(app);

        println!("[DPI Proxy] Proxy başlatıldı.");
        Ok(())
    }

    /// Proxy'yi durdur
    /// Proxy'nin şu anda çalışıp çalışmadığını döndürür (async olmayan, hızlı kontrol)
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.proxy_running.try_lock().map(|g| *g).unwrap_or(false)
    }

    pub async fn stop_proxy(&self, app: &tauri::AppHandle) {
        println!("[DPI Proxy] Proxy durduruluyor...");
        *self.proxy_running.lock().await = false;
        *self.current_method.lock().await = None;

        let mut settings = self.settings.lock().await;
        settings.is_active = false;
        settings.save(app);

        println!("[DPI Proxy] Proxy durduruldu.");
    }

    /// Detaylı bağlantı kontrolü
    pub async fn check_connection_detailed(&self) -> ConnectionResult {
        check_openanime_connection().await
    }

    /// Tüm yöntemleri dene ve çalışanı bul
    pub async fn test_all_methods(
        &self,
        app: &tauri::AppHandle,
    ) -> Option<u32> {
        let method_order: Vec<u32> = {
            let settings = self.settings.lock().await;

            // Önce çalışan yöntemi dene
            if let Some(active_id) = settings.active_method_id {
                if methods::get_method_by_id(active_id).is_some() {
                    let mut order = vec![active_id];
                    for m in &settings.methods {
                        if m.id != active_id && !matches!(m.status, methods::MethodStatus::Failed) {
                            order.push(m.id);
                        }
                    }
                    for m in &settings.methods {
                        if !order.contains(&m.id) {
                            order.push(m.id);
                        }
                    }
                    order
                } else {
                    methods::default_method_order()
                }
            } else {
                methods::default_method_order()
            }
        };

        for &method_id in &method_order {
            let method_name = methods::get_method_by_id(method_id)
                .map(|m| m.name.as_str())
                .unwrap_or("?");
            println!("[DPI Proxy] Yöntem #{} deneniyor... ({})", method_id, method_name);

            // Proxy'yi bu yöntemle başlat
            if let Err(e) = self.start_proxy(app, method_id).await {
                eprintln!("[DPI Proxy] Proxy başlatma hatası: {}", e);
                continue;
            }

            tokio::time::sleep(Duration::from_secs(3)).await;

            let result = self.check_connection_detailed().await;
            let mut settings = self.settings.lock().await;

            match result {
                ConnectionResult::Ok => {
                    println!("[DPI Proxy] ✅ Yöntem #{} çalışıyor!", method_id);
                    settings.mark_method_success(method_id);
                    settings.save(app);
                    return Some(method_id);
                }
                _ => {
                    println!("[DPI Proxy] ❌ Yöntem #{} başarısız: {:?}", method_id, result);
                    settings.mark_method_fail(method_id);
                    settings.save(app);
                }
            }

            self.stop_proxy(app).await;
        }

        println!("[DPI Proxy] Hiçbir yöntem çalışmadı.");
        None
    }

    /// Mevcut durumu döndür (frontend için)
    pub async fn get_status(&self) -> DpiStatus {
        let method_name = {
            let current = self.current_method.lock().await;
            current
                .as_ref()
                .map(|m| m.name.clone())
                .unwrap_or_else(|| "—".to_string())
        };

        let settings = self.settings.lock().await;
        DpiStatus {
            proxy_running: *self.proxy_running.lock().await,
            active_method_id: settings.active_method_id,
            active_method_name: method_name,
            is_blocking_detected: settings.is_blocking_detected,
            blocked_reason: settings.blocked_reason.clone(),
            system_goodbye_running: settings.system_goodbye_running,
        }
    }

    /// Proxy ayarlarını sıfırla (frontend için)
    pub async fn reset_settings(&self, app: &tauri::AppHandle) {
        let mut settings = self.settings.lock().await;
        *settings = GoodbyeSettings::default();
        settings.save(app);
    }
}

// ===== Tauri Komutları =====

#[tauri::command]
pub async fn dpi_start_proxy(
    app: tauri::AppHandle,
    method_id: u32,
) -> Result<(), String> {
    let state = app.state::<DpiProxyManager>();
    state.start_proxy(&app, method_id).await
}

#[tauri::command]
pub async fn dpi_stop_proxy(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<DpiProxyManager>();
    state.stop_proxy(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn dpi_test_methods(app: tauri::AppHandle) -> Result<Option<u32>, String> {
    let state = app.state::<DpiProxyManager>();
    Ok(state.test_all_methods(&app).await)
}

#[tauri::command]
pub async fn dpi_get_status(app: tauri::AppHandle) -> Result<DpiStatus, String> {
    let state = app.state::<DpiProxyManager>();
    Ok(state.get_status().await)
}

#[tauri::command]
pub async fn dpi_check_connection(app: tauri::AppHandle) -> Result<ConnectionResult, String> {
    let state = app.state::<DpiProxyManager>();
    Ok(state.check_connection_detailed().await)
}

#[tauri::command]
pub async fn dpi_reset_settings(app: tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<DpiProxyManager>();
    state.reset_settings(&app).await;
    Ok(())
}

// Static listeyi döndür — serileştirilebilir versiyon
#[tauri::command]
pub async fn dpi_get_methods() -> Result<Vec<DpiMethod>, String> {
    Ok(methods::ALL_METHODS.to_vec())
}

// ===== İç Yardımcılar =====

/// Detaylı bağlantı kontrolü — hata tipini analiz eder
async fn check_openanime_connection() -> ConnectionResult {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .danger_accept_invalid_certs(false)
        .build();

    let client = match client {
        Ok(c) => c,
        Err(_) => return ConnectionResult::NetworkUnreachable,
    };

    // 1. Aşama: DNS çözümleme
    let dns_result = tokio::time::timeout(Duration::from_secs(3), async {
        tokio::net::lookup_host("openani.me:443").await
    })
    .await;

    match dns_result {
        Ok(Ok(mut addrs)) => {
            if addrs.next().is_none() {
                return ConnectionResult::DnsFailure;
            }
        }
        _ => return ConnectionResult::DnsFailure,
    }

    // 2. Aşama: TLS + HTTP isteği
    let url = format!(
        "https://openani.me/?nocache={}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );

    let req = client
        .get(&url)
        .header("Cache-Control", "no-cache")
        .header("Pragma", "no-cache");

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                ConnectionResult::Ok
            } else if status == reqwest::StatusCode::FORBIDDEN {
                ConnectionResult::Forbidden
            } else if status.is_server_error() {
                ConnectionResult::ServerError
            } else if status.is_redirection() {
                ConnectionResult::Ok
            } else {
                ConnectionResult::Forbidden
            }
        }
        Err(e) => {
            if e.is_timeout() || e.is_connect() {
                ConnectionResult::Timeout
            } else if e.is_request() {
                ConnectionResult::TlsError
            } else {
                ConnectionResult::NetworkUnreachable
            }
        }
    }
}

/// Sistem genelinde GoodbyeDPI çalışıyor mu kontrol et
fn is_system_goodbye_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("tasklist")
            .args(&["/FI", "IMAGENAME eq goodbyedpi.exe", "/NH"])
            .output();
        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.contains("goodbyedpi.exe")
            }
            Err(_) => false,
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}
