// === OpenAnime — DPI Proxy Ana Modülü ===
// Tüm DPI atlatma sistemini yönetir: proxy, ayarlar, bağlantı kontrolü

use crate::dbg_log;

pub mod bypass_detect;
mod http_mod;
pub mod methods;
pub mod remote_proxy;
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
    pub connection_stage: String,
}

/// check_connection()'un detaylı sonucu
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ConnectionResult {
    Ok,
    /// Sunucu yanıt verdi ama isteği REDDETTİ (401/403/429): Cloudflare
    /// "Just a moment" sayfası ya da OpenAnime Vanguard "Unauthorized".
    /// Bu bir AĞ sorunu DEĞİLDİR — TCP+TLS+HTTP baştan sona çalıştı.
    Challenged,
    Timeout,
    Forbidden,
    DnsFailure,
    ServerError,
    TlsError,
    NetworkUnreachable,
}

impl ConnectionResult {
    /// Sunucuya FİİLEN ulaşıldı mı?
    ///
    /// NEDEN AYRI BİR KAVRAM: DPI engellemesi paket seviyesinde olur —
    /// bağlantı düşer, TLS handshake bozulur, DNS boş döner. Karşı taraftan
    /// bir HTTP yanıtı GELDİYSE yol açıktır; 401/403/5xx yalnızca sunucunun
    /// o isteğe verdiği cevaptır. Bu ikisi karıştırıldığında (eski davranış:
    /// 403 → `Forbidden` → "engellenmişiz") uygulama Cloudflare'in bot
    /// sayfasını "internet yok" sanıp sırayla tüm DPI yöntemlerini deniyor,
    /// hepsi aynı 403'ü alıp başarısız sayılıyor ve sonunda çevrimdışı moda
    /// düşüyordu. Üstelik bu tarama WebView'in canlı trafiğinin aktığı
    /// proxy'nin yöntemini saniyeler içinde defalarca değiştiriyor.
    pub fn is_reachable(self) -> bool {
        matches!(
            self,
            ConnectionResult::Ok
                | ConnectionResult::Challenged
                | ConnectionResult::Forbidden
                | ConnectionResult::ServerError
        )
    }
}

/// DPI Proxy Yöneticisi — app başlatılırken oluşturulur
pub struct DpiProxyManager {
    pub settings: Mutex<GoodbyeSettings>,
    pub proxy_running: Arc<Mutex<bool>>,
    pub current_method: Arc<Mutex<Option<DpiMethod>>>,
    pub connection_stage: Mutex<String>,
    /// `request_bypass()` için son istek zamanı (bkz. BYPASS_COOLDOWN).
    last_bypass_request: Mutex<Option<std::time::Instant>>,
}

/// JS'ten gelen bypass isteklerinin en fazla hangi sıklıkta işleneceği.
/// Sayfa her yenilendiğinde init.js sayacı sıfırdan başlar; art arda gelen
/// yenilemelerde aynı istek dakikada birkaç kez geliyordu.
const BYPASS_COOLDOWN: Duration = Duration::from_secs(120);

impl DpiProxyManager {
    pub fn new(app: &tauri::AppHandle) -> Self {
        let settings = GoodbyeSettings::load(app);

        // Harici DPI bypass araçlarını tara ve fragmentasyon davranışını buna
        // göre ayarla (bkz. bypass_detect). Açılış özeti her zaman loglanır.
        let behavior = bypass_detect::refresh_and_log(true);

        // Geriye dönük alan: UI "sistemde harici araç var mı" bilgisini bundan
        // okuyor. Artık yalnızca GoodbyeDPI değil, tespit edilen HERHANGİ bir
        // aracı (Zapret/ByeDPI/WARP/WinDivert) kapsıyor.
        let system_running = behavior != bypass_detect::ProxyBehavior::Fragment;

        let mut settings = settings;
        settings.system_goodbye_running = system_running;
        settings.save(app);

        // Periyodik yeniden tarama: kullanıcı uygulama açıkken GoodbyeDPI'ı
        // başlatabilir/kapatabilir ya da WARP'ı açıp kapatabilir. Yalnızca
        // başlangıçta bakmak bu durumları kaçırırdı. Log yalnızca durum
        // DEĞİŞTİĞİNDE basılır (force_log=false), böylece log kirlenmez.
        tauri::async_runtime::spawn(async {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                // Tespit senkron komutlar çalıştırır (tasklist/sc/netsh);
                // async çalıştırıcıyı bloklamamak için ayrı thread'e alınır.
                let _ = tauri::async_runtime::spawn_blocking(|| {
                    bypass_detect::refresh_and_log(false)
                })
                .await;
            }
        });

        Self {
            settings: Mutex::new(settings),
            proxy_running: Arc::new(Mutex::new(false)),
            current_method: Arc::new(Mutex::new(None)),
            connection_stage: Mutex::new("idle".to_string()),
            last_bypass_request: Mutex::new(None),
        }
    }

    /// Proxy'yi başlat (arkaplan task'i)
    pub async fn start_proxy(&self, app: &tauri::AppHandle, method_id: u32) -> Result<(), String> {
        // Harici bir bypass aracı (Zapret/GoodbyeDPI/ByeDPI) veya WARP aktifken
        // tcp_forward zaten hiçbir manipülasyon uygulamıyor — yani 1..8 arası
        // yöntemlerin HEPSİ fiilen Direct gibi davranıyor. Buna rağmen yöntem
        // kimliğini değiştirip kaydetmek yalnızca gürültü üretiyordu: log'da
        // "Host Case Change aktif" görünüyor, ayar dosyasına yazılıyor ve
        // kullanıcı bir header manipülasyonunun uygulandığını sanıyor.
        // Kararı TEK yerde veriyoruz: aktif yöntem Direct'e sabitlenir.
        let behavior = bypass_detect::current_behavior();
        let method_id = if !behavior.allows_fragmentation() && method_id != 0 {
            dbg_log!(
                "[DPI Proxy] Harici bypass aracı aktif ({:?}) — yöntem #{} yerine Direct (#0) uygulanıyor",
                behavior,
                method_id
            );
            0
        } else {
            method_id
        };

        let method = methods::get_method_by_id(method_id)
            .ok_or_else(|| format!("Yöntem bulunamadı: {}", method_id))?;

        // Aynı yöntem zaten aktifse ve dinleyici ayaktaysa hiçbir şey yapma.
        // Eski davranışta her çağrı (sayfa yenilemesi başına en az bir tane)
        // "Proxy yöntemi güncelleniyor" satırı basıp ayar dosyasını yeniden
        // yazıyordu. `proxy_running` kontrolü şart: dinleyici düşmüşse
        // (port meşgul, bind hatası) yeniden ayağa kaldırmamız gerekir.
        let same_method =
            self.current_method.lock().await.as_ref().map(|m| m.id) == Some(method_id);
        if same_method && *self.proxy_running.lock().await {
            dbg_log!("[DPI Proxy] Yöntem #{} zaten aktif, değişiklik yok.", method_id);
            return Ok(());
        }

        dbg_log!(
            "[DPI Proxy] Proxy yöntemi güncelleniyor: #{} ({})",
            method_id, method.name
        );

        // Update the active method in the shared Arc
        *self.current_method.lock().await = Some(method.clone());

        // Ensure the background listener loop is running
        let mut running = self.proxy_running.lock().await;
        if !*running {
            *running = true;
            let running_clone = self.proxy_running.clone();
            let current_method_clone = self.current_method.clone();
            tokio::spawn(async move {
                tcp_forward::start_proxy_internal(current_method_clone, running_clone).await;
            });
        }

        // Ayarları güncelle
        let mut settings = self.settings.lock().await;
        settings.is_active = true;
        settings.active_method_id = Some(method_id);
        settings.save(app);

        dbg_log!(
            "[DPI Proxy] Proxy yöntemi başarıyla uygulandı (#{}).",
            method_id
        );
        Ok(())
    }

    /// Proxy'yi durdur (Direct moduna geçer)
    pub async fn stop_proxy(&self, app: &tauri::AppHandle) {
        dbg_log!("[DPI Proxy] Proxy bypass kapatılıyor (Direct moda geçiliyor)...");
        *self.current_method.lock().await = None;

        let mut settings = self.settings.lock().await;
        settings.is_active = false;
        settings.active_method_id = Some(0); // 0 means Direct
        settings.save(app);

        dbg_log!("[DPI Proxy] Proxy bypass durduruldu (Direct mode aktif).");
    }

    /// Detaylı bağlantı kontrolü
    pub async fn check_connection_detailed(&self, use_proxy: bool) -> ConnectionResult {
        check_openanime_connection(use_proxy).await
    }

    /// Tüm yöntemleri dene ve çalışanı bul
    pub async fn test_all_methods(
        &self,
        app: &tauri::AppHandle,
    ) -> Option<u32> {
        // Harici araç/WARP aktifken tarama ANLAMSIZ: proxy hiçbir yönteme
        // göre farklı davranmıyor (bkz. start_proxy). Sekiz yöntemi sırayla
        // denemek yalnızca 8 kez ağ isteği atıp aynı sonucu alıyor.
        let behavior = bypass_detect::current_behavior();
        if !behavior.allows_fragmentation() {
            dbg_log!(
                "[DPI Proxy] Harici bypass aracı aktif ({:?}) — yöntem taraması atlandı, Direct kullanılıyor",
                behavior
            );
            let _ = self.start_proxy(app, 0).await;
            let result = self.check_connection_detailed(true).await;
            return if result.is_reachable() { Some(0) } else { None };
        }

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
            dbg_log!("[DPI Proxy] Yöntem #{} deneniyor... ({})", method_id, method_name);

            // Proxy'yi bu yöntemle başlat
            if let Err(e) = self.start_proxy(app, method_id).await {
                dbg_log!("[DPI Proxy] Proxy başlatma hatası: {}", e);
                continue;
            }

            // start_proxy inside already sleeps 100ms, no need to wait 3 seconds.
            // We check the connection immediately through the local proxy.
            let result = self.check_connection_detailed(true).await;
            let mut settings = self.settings.lock().await;

            // Ölçüt "HTTP 200 aldık mı" DEĞİL, "sunucuya ulaştık mı".
            // Cloudflare/Vanguard'ın 401/403'ü yöntemin başarısız olduğunu
            // göstermez — o yanıt zaten hedeften geliyor, yani paket yolu açık.
            if result.is_reachable() {
                dbg_log!(
                    "[DPI Proxy] Yöntem #{} çalışıyor! (yanıt: {:?})",
                    method_id,
                    result
                );
                settings.mark_method_success(method_id);
                settings.save(app);
                return Some(method_id);
            }

            dbg_log!("[DPI Proxy] Yöntem #{} başarısız: {:?}", method_id, result);
            settings.mark_method_fail(method_id);
            settings.save(app);

            self.stop_proxy(app).await;
        }

        dbg_log!("[DPI Proxy] Hiçbir yöntem çalışmadı.");
        None
    }

    /// JS'ten gelen "bağlantı kopuyor, bypass dene" isteğini karşılar
    /// (Tauri komutu: `reopen_with_proxy`).
    ///
    /// ESKİ DAVRANIŞ VE NEDEN YANLIŞTI:
    ///   Komut koşulsuz `start_proxy(app, 1)` çağırıyordu. Yani:
    ///     • Açılışta bulunmuş ÇALIŞAN yöntemi eziyor ve #1'i ayar dosyasına
    ///       kalıcı yazıyordu (sonraki açılış da #1 ile başlıyordu),
    ///     • Bağlantının gerçekten kopup kopmadığına HİÇ bakmıyordu,
    ///     • Sayfa her yenilendiğinde init.js sayacı sıfırdan başladığı için
    ///       art arda tetikleniyordu.
    ///   Log'daki tekrar eden "reopen_with_proxy çağrıldı → yöntem #1" dizisi
    ///   buydu. (Komut WebView'e dokunmaz; sayfayı yenileyen watchdog'dur,
    ///   bkz. js/modules/page-recovery.js.)
    pub async fn request_bypass(&self, app: &tauri::AppHandle) -> Result<(), String> {
        {
            let mut last = self.last_bypass_request.lock().await;
            if let Some(t) = *last {
                if t.elapsed() < BYPASS_COOLDOWN {
                    dbg_log!(
                        "[DPI Proxy] Bypass isteği yok sayıldı (önceki istek {} sn önce, bekleme {} sn)",
                        t.elapsed().as_secs(),
                        BYPASS_COOLDOWN.as_secs()
                    );
                    return Ok(());
                }
            }
            *last = Some(std::time::Instant::now());
        }

        let behavior = bypass_detect::current_behavior();
        if !behavior.allows_fragmentation() {
            dbg_log!(
                "[DPI Proxy] Bypass isteği: harici araç aktif ({:?}) — yöntem değiştirilmiyor",
                behavior
            );
            return Ok(());
        }

        // Önce GERÇEKTEN ulaşılamıyor mu diye bak. Anti-bot/yetki yanıtları
        // (401/403) yöntem değiştirmeyi gerektirmez; yöntem değiştirmek o
        // yanıtı zaten düzeltmez, sadece canlı bağlantıları tazeler.
        let result = self.check_connection_detailed(true).await;
        if result.is_reachable() {
            dbg_log!(
                "[DPI Proxy] Bypass isteği: sunucuya ulaşılıyor ({:?}) — yöntem DEĞİŞTİRİLMEDİ",
                result
            );
            return Ok(());
        }

        dbg_log!("[DPI Proxy] Bypass isteği: bağlantı yok ({:?}), yöntem taraması başlıyor", result);
        match self.test_all_methods(app).await {
            Some(id) => dbg_log!("[DPI Proxy] Bypass isteği: çalışan yöntem #{}", id),
            None => dbg_log!("[DPI Proxy] Bypass isteği: çalışan yöntem bulunamadı"),
        }
        Ok(())
    }

    /// Uzak proxy fallback adımını dener
    // Windows DPI arka plan akışında kullanılır (cfg(windows)); Linux'ta ölü görünür.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub async fn try_remote_proxy_fallback(&self, _app: &tauri::AppHandle) -> Result<(), String> {
        dbg_log!("[DPI Proxy] Uzak proxy fallback deneniyor...");
        *self.connection_stage.lock().await = "trying_proxy".to_string();
        
        match remote_proxy::try_remote_proxy_connection().await {
            Ok(_) => {
                dbg_log!("[DPI Proxy] Uzak proxy fallback başarılı!");
                *self.connection_stage.lock().await = "success".to_string();
                Ok(())
            }
            Err(e) => {
                dbg_log!("[DPI Proxy] Uzak proxy fallback başarısız: {}", e);
                *self.connection_stage.lock().await = "failed".to_string();
                Err(e)
            }
        }
    }

    /// Mevcut durumu döndür (frontend için)
    pub async fn get_status(&self) -> DpiStatus {
        let method_name = {
            let current = self.current_method.lock().await;
            current
                .as_ref()
                .map(|m| m.name.clone())
                .unwrap_or_else(|| "Direct (Bypass Yok)".to_string())
        };

        let settings = self.settings.lock().await;
        DpiStatus {
            proxy_running: settings.is_active,
            active_method_id: settings.active_method_id,
            active_method_name: method_name,
            is_blocking_detected: settings.is_blocking_detected,
            blocked_reason: settings.blocked_reason.clone(),
            system_goodbye_running: settings.system_goodbye_running,
            connection_stage: self.connection_stage.lock().await.clone(),
        }
    }

}

// ===== Tauri Komutları =====
// (dpi_start_proxy/dpi_stop_proxy/dpi_check_connection/dpi_reset_settings/
//  dpi_get_methods kaldırıldı — hiçbir JS/frontend'ten çağrılmıyordu; proxy
//  yaşam döngüsü lib.rs setup arka plan akışından yönetiliyor.)

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

// ===== İç Yardımcılar =====

/// Detaylı bağlantı kontrolü — hata tipini analiz eder
async fn check_openanime_connection(use_proxy: bool) -> ConnectionResult {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        // WebView'in gönderdiği User-Agent'ın AYNISI. Kısaltılmış/yapay bir UA
        // Cloudflare'in bot yönetimini gereksiz yere tetikliyor ve kontrol
        // isteği, tarayıcı sorunsuz gezinirken bile 403 alabiliyordu.
        .user_agent(crate::platform_user_agent())
        .danger_accept_invalid_certs(false);

    // DoH ile DNS ezmesi — WARP aktifken ATLANIR. (tcp_forward.rs bu kuralı
    // zaten uyguluyordu; burada uygulanmıyordu, yani WARP'lı kullanıcıda
    // kontrol isteği WARP'ın çözümleyicisini baypas edip tünel dışına
    // çıkabiliyor ve tarayıcıdan FARKLI bir sonuç üretebiliyordu.)
    if bypass_detect::current_behavior().allows_doh_override() {
        if let Some(ip) = remote_proxy::resolve_dns_doh("openani.me").await {
            dbg_log!("[DPI Proxy] DNS Bypass (DoH): openani.me resolved to {}", ip);
            let socket_addr = std::net::SocketAddr::new(ip, 443);
            builder = builder.resolve("openani.me", socket_addr);
        } else {
            dbg_log!("[DPI Proxy] Warning: Cloudflare DoH failed, falling back to system DNS");
        }
    } else {
        dbg_log!("[DPI Proxy] WARP aktif — bağlantı kontrolünde DoH DNS ezmesi atlandı");
    }

    if use_proxy {
        if let Ok(proxy) = reqwest::Proxy::all("http://127.0.0.1:1453") {
            builder = builder.proxy(proxy);
        }
    } else {
        builder = builder.no_proxy();
    }

    let client = match builder.build() {
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
            // Buraya gelindiyse DNS + TCP + TLS + HTTP baştan sona çalıştı.
            // Statü kodu artık AĞIN değil, SUNUCUNUN kararıdır.
            if status.is_success() || status.is_redirection() {
                ConnectionResult::Ok
            } else if matches!(status.as_u16(), 401 | 403 | 429) {
                // Cloudflare "Just a moment" / OpenAnime Vanguard reddi.
                // Yanıt başlığından hangisi olduğunu ayırt edip loglayalım —
                // bu bilgi olmadan "403" görüp DPI engeli sanılıyordu.
                let mitigated = resp.headers().contains_key("cf-mitigated")
                    || resp
                        .headers()
                        .get("server")
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.eq_ignore_ascii_case("cloudflare"))
                        .unwrap_or(false);
                dbg_log!(
                    "[DPI Proxy] Sunucu isteği reddetti (HTTP {}, cloudflare={}) — AĞ SORUNU DEĞİL, \
                     bot koruması/oturum katmanı. DPI yöntemi değiştirilmeyecek.",
                    status.as_u16(),
                    mitigated
                );
                ConnectionResult::Challenged
            } else if status.is_server_error() {
                ConnectionResult::ServerError
            } else {
                dbg_log!("[DPI Proxy] Beklenmeyen statü: HTTP {}", status.as_u16());
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

#[cfg(test)]
mod tests {
    use super::ConnectionResult;

    #[test]
    fn http_yaniti_gelen_her_durum_ulasilabilir_sayilir() {
        // Sunucudan cevap geldiyse ağ yolu açıktır — bunlar DPI engeli değil.
        assert!(ConnectionResult::Ok.is_reachable());
        assert!(ConnectionResult::Challenged.is_reachable());
        assert!(ConnectionResult::Forbidden.is_reachable());
        assert!(ConnectionResult::ServerError.is_reachable());
    }

    #[test]
    fn tasima_katmani_hatalari_ulasilamaz_sayilir() {
        // Gerçek engellemenin göründüğü yer: bağlantı hiç kurulamıyor.
        assert!(!ConnectionResult::Timeout.is_reachable());
        assert!(!ConnectionResult::DnsFailure.is_reachable());
        assert!(!ConnectionResult::TlsError.is_reachable());
        assert!(!ConnectionResult::NetworkUnreachable.is_reachable());
    }
}

// NOT: Eski `is_system_goodbye_running()` kaldırıldı. Yalnızca
// `goodbyedpi.exe` process adına bakıyordu ve sonucu HİÇBİR davranışa
// bağlanmıyordu (sadece settings'e yazılıp UI'a raporlanıyordu) — yani
// tespit edilse bile fragmentasyon yine de uygulanıyor, araçlar çakışıyordu.
// Yerini bypass_detect modülü aldı: Zapret/GoodbyeDPI/ByeDPI/WARP process
// ve servis taraması + WinDivert sürücüsü + WARP ağ arayüzü kontrolü, ve
// sonucun tcp_forward'daki fragmentasyon/DoH kararına fiilen bağlanması.
