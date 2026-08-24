//! Kanal farkındalıklı güncelleyici.
//!
//! ## Neden Rust tarafında
//!
//! `@tauri-apps/plugin-updater`'ın JS API'si endpoint'i çalışma anında
//! değiştiremiyor: `check()` seçenekleri yalnızca başlık/zaman aşımı/proxy
//! alıyor, adres `tauri.conf.json`'dan geliyor. Kanal seçimi ise tam olarak
//! "hangi manifesti okuyacağız" sorusu. Endpoint'i ezebilen tek yol Rust
//! tarafındaki `updater_builder().endpoints(...)`, bu yüzden kontrol ve
//! indirme buraya taşındı.
//!
//! ## Kanal başına ayrı manifest
//!
//! Kanallar `main` dalındaki `updater/latest-<kanal>.json` dosyaları. Yayın
//! iş akışı her sürümden sonra yalnızca KENDİ kanalının dosyasını güncelliyor
//! (bkz. `.github/workflows/release.yml`). Ayrımın kritik sonucu şu: Stable
//! kanaldaki bir kullanıcıya alpha/beta sürümü asla görünmez, çünkü o sürüm
//! Stable manifeste hiç yazılmaz. Tek bir manifeste "en yeni sürüm" yazıp
//! istemcide filtrelemek aynı garantiyi vermezdi.
//!
//! ## "Kanalda sürüm yok" durumu
//!
//! Bir kanaldan henüz hiç yayın yapılmamışsa o dosya depoda yoktur ve
//! `raw.githubusercontent.com` 404 döner. Eklentinin `check()`'i bunu ağ
//! hatasından ayırt etmiyor — ikisi de "kontrol başarısız" olurdu. Kullanıcıya
//! "Stable sürüm mevcut değil" ile "internet yok" arasındaki farkı
//! gösterebilmek için manifesti önce kendimiz çekip durumuna bakıyoruz.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Kanal manifestlerinin bulunduğu dizin (ham dosya erişimi).
///
/// Release varlıkları yerine `raw.githubusercontent.com`: manifest, yayın
/// tamamlandıktan sonra `main`'e commit'leniyor, dolayısıyla taslak ya da
/// silinmiş release'lerden etkilenmiyor ve kanal başına ayrı dosya tutmak
/// mümkün oluyor.
const MANIFEST_BASE: &str =
    "https://raw.githubusercontent.com/Dark-Hunter-TR/OpenAnime-Desktops/main/updater";

/// Aynı kanal için ardışık kontrollerde ağa çıkmadan önce beklenen süre.
///
/// Açılış kontrolü ile Ayarlar'daki "şimdi kontrol et" aynı fonksiyonu
/// çağırıyor; kullanıcı sekmeler arasında gezinirken arka arkaya istek
/// yapılmasın diye. Elle tetiklenen kontrol `force` ile bunu atlıyor.
const CACHE_TTL: Duration = Duration::from_secs(300);

/// Tel formatı (`Deserialize`/`Serialize` string'i) frontend'in
/// `localStorage`'ta hâlâ tuttuğu tarihi `"release"` değeriyle birebir
/// eşleşiyor — JS/localStorage tarafında hiçbir şey değişmedi, yalnızca Rust
/// artık bunu serbest bir `String` yerine tipli bir enum olarak işliyor.
/// Dosya adları (bkz. `file()`) ise `latest-stable.json` olarak kalıyor.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[serde(rename = "release")]
    Stable,
    Beta,
    Alpha,
}

impl Channel {
    fn file(self) -> &'static str {
        match self {
            Channel::Stable => "latest-stable.json",
            Channel::Beta => "latest-beta.json",
            Channel::Alpha => "latest-alpha.json",
        }
    }

    /// Arayüzde gösterilen ad. Durum metinleri burada üretildiği için çeviri
    /// de burada duruyor.
    fn label(self) -> &'static str {
        match self {
            Channel::Stable => "Stable",
            Channel::Beta => "Beta",
            Channel::Alpha => "Alpha",
        }
    }
}

/// Bir kontrolün sonucu.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    /// Kanalda henüz hiç yayın yok (manifest dosyası depoda bulunmuyor).
    ///
    /// `available: false` ile aynı şey değil: biri "güncelsin", diğeri "bu
    /// kanaldan hiç sürüm çıkmamış".
    pub channel_empty: bool,
    pub available: bool,
    pub channel: Channel,
    pub channel_label: String,
    /// Güncelleme varsa yeni sürüm; yoksa `None`.
    pub version: Option<String>,
    pub date: Option<String>,
    pub body: Option<String>,
    /// Kanaldaki en son sürüm — güncelleme olmasa bile dolu.
    ///
    /// Kullanıcı alpha'dan Stable kanala geçtiğinde oradaki sürüm daha ESKİ
    /// olabiliyor; o durumda güncelleme sunulmuyor ama arayüzün "bu kanalın
    /// en sonu şu" diyebilmesi gerekiyor.
    pub latest_version: Option<String>,
}

impl CheckResult {
    fn base(channel: Channel) -> Self {
        Self {
            channel_empty: false,
            available: false,
            channel,
            channel_label: channel.label().to_string(),
            version: None,
            date: None,
            body: None,
            latest_version: None,
        }
    }
}

pub struct UpdaterState {
    /// Son kontrolde bulunan güncelleme — indirme bunu kullanıyor.
    current: Mutex<Option<Update>>,
    downloading: Mutex<bool>,
    cache: Mutex<Option<(Instant, Channel, CheckResult)>>,
}

impl UpdaterState {
    pub fn new() -> Self {
        Self {
            current: Mutex::new(None),
            downloading: Mutex::new(false),
            cache: Mutex::new(None),
        }
    }
}

/// Uygulamanın kurulu sürümü. Güncelleme kontrolünden bağımsız — Ayarlar
/// kartında "Mevcut Sürüm: vX.Y.Z" göstermek için.
#[tauri::command]
pub fn get_app_version(app: tauri::AppHandle) -> String {
    app.config()
        .version
        .clone()
        .unwrap_or_else(|| "0.0.0".to_string())
}

/// Manifesti çeker.
///
/// `Ok(None)` = kanalda yayın yok (404 ya da içi boş manifest).
/// `Err(_)`   = gerçek bir hata (ağ yok, sunucu hatası, bozuk JSON).
async fn fetch_manifest(url: &str) -> Result<Option<serde_json::Value>, String> {
    let resp = reqwest::get(url)
        .await
        .map_err(|e| format!("Manifest alınamadı: {e}"))?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(format!("Manifest sunucusu {} döndü", resp.status()));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| format!("Manifest okunamadı: {e}"))?;
    if text.trim().is_empty() {
        return Ok(None);
    }

    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Manifest çözümlenemedi: {e}"))?;

    // Platform listesi boşsa indirilecek bir şey yok; dosyanın varlığı tek
    // başına "bu kanalda sürüm var" anlamına gelmiyor.
    let has_platform = value
        .get("platforms")
        .and_then(|p| p.as_object())
        .map(|o| !o.is_empty())
        .unwrap_or(false);
    if !has_platform {
        return Ok(None);
    }

    Ok(Some(value))
}

/// Seçili kanalda güncelleme olup olmadığını sorar.
#[tauri::command]
pub async fn updater_check(
    app: AppHandle,
    state: tauri::State<'_, UpdaterState>,
    channel: Channel,
    force: Option<bool>,
) -> Result<CheckResult, String> {
    let force = force.unwrap_or(false);

    // Geliştirici (debug) derlemesi her zaman Beta kanalını kontrol eder —
    // dağıtılan Stable/Alpha manifestleri henüz derlenmemiş özellikleri
    // içermeyebilir, geliştirme sırasında güncelleme kontrolünün en sık
    // değişen kanala bakması isteniyor.
    let channel = if cfg!(debug_assertions) {
        crate::log!("[Updater] Geliştirici (Dev) derlemesi algılandı. Güncelleme kanalı 'beta' olarak ayarlanıyor.");
        Channel::Beta
    } else {
        channel
    };

    if !force {
        if let Ok(cache) = state.cache.lock() {
            if let Some((at, cached_channel, result)) = &*cache {
                if *cached_channel == channel && at.elapsed() < CACHE_TTL {
                    return Ok(result.clone());
                }
            }
        }
    }

    let mut url = format!("{MANIFEST_BASE}/{}", channel.file());
    if force {
        // raw.githubusercontent yanıtları birkaç dakika önbelleğe alınıyor;
        // elle kontrol edildiğinde taze veri görmek gerekiyor.
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        url = format!("{url}?t={ts}");
    }

    crate::dbg_log!("[Updater] Checking updates on URL: {} (Force: {})", url, force);

    let manifest = fetch_manifest(&url).await?;

    let Some(manifest) = manifest else {
        if let Ok(mut current) = state.current.lock() {
            *current = None;
        }
        return Ok(CheckResult {
            channel_empty: true,
            ..CheckResult::base(channel)
        });
    };

    let latest_version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Sürüm karşılaştırması ve imza doğrulaması eklentiye bırakılıyor: kendi
    // karşılaştırmamızı yazmak, ön-sürüm sıralaması (0.2.0-alpha.2 < 0.2.0)
    // gibi ayrıntıları ikinci kez uygulamak olurdu.
    let updater = app
        .updater_builder()
        .endpoints(vec![url
            .parse()
            .map_err(|e| format!("Endpoint çözümlenemedi: {e}"))?])
        .map_err(|e| format!("Updater yapılandırılamadı: {e}"))?
        .build()
        .map_err(|e| format!("Updater kurulamadı: {e}"))?;

    let found = updater.check().await.map_err(|e| {
        crate::dbg_log!("[Updater] Güncelleme sorgusu başarısız: {}", e);
        format!("Güncelleme kontrolü başarısız: {e}")
    })?;

    let result = match found {
        Some(update) => {
            let version = update.version.clone();
            let date = update.date.map(|d| d.to_string());
            let body = update.body.clone();

            if let Ok(mut current) = state.current.lock() {
                *current = Some(update);
            }

            CheckResult {
                available: true,
                version: Some(version),
                date,
                body,
                latest_version,
                ..CheckResult::base(channel)
            }
        }
        None => {
            if let Ok(mut current) = state.current.lock() {
                *current = None;
            }
            CheckResult {
                latest_version,
                ..CheckResult::base(channel)
            }
        }
    };

    if !force {
        if let Ok(mut cache) = state.cache.lock() {
            *cache = Some((Instant::now(), channel, result.clone()));
        }
    }

    Ok(result)
}

/// İndirme ilerlemesi. Arayüz `openanime://update-progress` olayını dinliyor.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    /// `downloading` | `installing` | `success` | `error`
    status: &'static str,
    downloaded: u64,
    /// Sunucu `Content-Length` vermezse `None` — arayüz o durumda belirsiz
    /// ilerleme çubuğuna düşüyor.
    total: Option<u64>,
    percent: u32,
    message: Option<String>,
}

impl Progress {
    fn new(status: &'static str) -> Self {
        Self {
            status,
            downloaded: 0,
            total: None,
            percent: 0,
            message: None,
        }
    }
}

/// Son kontrolde bulunan güncellemeyi indirir, kurar ve uygulamayı yeniden
/// başlatır.
///
/// Hemen dönüyor; ilerleme olay olarak akıyor. Bloke etseydi indirme boyunca
/// IPC kuyruğu beklerdi ve arayüz donardı.
#[tauri::command]
pub async fn updater_download(
    app: AppHandle,
    state: tauri::State<'_, UpdaterState>,
) -> Result<(), String> {
    {
        let mut downloading = state
            .downloading
            .lock()
            .map_err(|_| "Güncelleyici durumu okunamadı".to_string())?;
        if *downloading {
            return Err("İndirme zaten sürüyor.".to_string());
        }
        *downloading = true;
    }

    let update = state.current.lock().ok().and_then(|guard| guard.clone());

    let Some(update) = update else {
        if let Ok(mut downloading) = state.downloading.lock() {
            *downloading = false;
        }
        return Err("İndirilecek güncelleme yok; önce kontrol edin.".to_string());
    };

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut downloaded: u64 = 0;
        let mut total: Option<u64> = None;

        let on_chunk = {
            let app = app_handle.clone();
            move |chunk: usize, content_length: Option<u64>| {
                downloaded += chunk as u64;
                if total.is_none() {
                    total = content_length;
                }
                let percent = match total {
                    Some(t) if t > 0 => ((downloaded as f64 / t as f64) * 100.0).round() as u32,
                    _ => 0,
                };
                let _ = app.emit(
                    "openanime://update-progress",
                    Progress {
                        status: "downloading",
                        downloaded,
                        total,
                        percent,
                        message: None,
                    },
                );
            }
        };

        let on_finish = {
            let app = app_handle.clone();
            move || {
                let _ = app.emit(
                    "openanime://update-progress",
                    Progress {
                        percent: 100,
                        ..Progress::new("installing")
                    },
                );
            }
        };

        let result = update.download_and_install(on_chunk, on_finish).await;

        if let Some(state) = app_handle.try_state::<UpdaterState>() {
            if let Ok(mut downloading) = state.downloading.lock() {
                *downloading = false;
            }
        }

        match result {
            Ok(_) => {
                let _ = app_handle.emit(
                    "openanime://update-progress",
                    Progress {
                        percent: 100,
                        ..Progress::new("success")
                    },
                );
                crate::log!("[Güncelleme] Kuruldu, uygulama yeniden başlatılıyor…");

                // KRİTİK: restart() çağrılmazsa bu süreç açık kalmaya devam
                // eder. NSIS installer download_and_install() içinde zaten
                // başlatıldı, ama çalışan ana .exe hâlâ bu süreç tarafından
                // kilitli olduğu sürece installer onun üzerine yazamaz —
                // kurulum burada takılı kalır ya da kullanıcıdan elle
                // kapatmasını ister. Kısa bekleme, yukarıdaki "success"
                // olayının frontend'e ulaşıp UI'da görünmesi için (restart()
                // geri dönmez, hemen süreci sonlandırır).
                tokio::time::sleep(Duration::from_millis(800)).await;
                app_handle.restart();
            }
            Err(e) => {
                crate::log!("[Güncelleme] Başarısız: {}", e);
                let _ = app_handle.emit(
                    "openanime://update-progress",
                    Progress {
                        message: Some(format!("{e}")),
                        ..Progress::new("error")
                    },
                );
            }
        }
    });

    Ok(())
}
