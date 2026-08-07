// === OpenAnime Süper Bildirimler ===
//
// Arka planda OpenAnime bildirimlerini dinleyip masaüstü toast bildirimi gösterir.
//
// MİMARİ NOTU (önemli):
// Burada HİÇBİR RENDER YOK. Site arka planda "kısmen render" edilmez — WebView2
// bunu yapamaz (ya tüm belgeyi render eder ya hiç). Bunun yerine sitenin kendi
// kullandığı bildirim akışına doğrudan Rust'tan bağlanılır:
//
//   Gizli/kapalı ana pencere → Rust arka plan görevi
//   → GET api.openani.me/user/notifications/sse (Server-Sent Events)
//   → sunucu yeni bildirimi ANINDA push eder → şeffaf toast penceresine event
//
// Poll YOK: açık bir HTTP bağlantısı boşta beklerken CPU harcamaz, bildirim
// gecikmesi sıfırdır. (İlk sürüm 60 sn'de bir /user/notifications'ı poll
// ediyordu — öyle bir GET rotası yok, 404 dönüyordu. Doğru yol SSE.)
//
// PROTOKOL (sitenin kendi istemcisinden birebir):
//   İstek : GET {API}/user/notifications/sse,  header: Authorization: <token>
//   Olaylar (SSE `data:` satırında JSON):
//     {"type":"initial","data":[ ...mevcut bildirimler... ]}  → toast GÖSTERME
//     {"type":"new","data":{ ...tek bildirim... }}            → toast göster
//   Kopunca: 1 sn ile başlayıp 30 sn'ye kadar katlanan bekleyişle yeniden bağlan.
//
// Bildirim alanları: title, message, href, readAt (null = okunmamış), type.
//
// KİMLİK DOĞRULAMA:
// `Authorization: <token>` — "Bearer" ÖN EKİ YOK. Token, `token` adlı çerezde.
// Çerez WebView2 deposundan okunur, böylece sayfa açık olmasa da çalışır.
//
// AKIŞ ÖLÇÜMÜ — 20 SANİYE KURALI (canlı testle KESİNLEŞTİ):
// Sunucu "initial" bloğunu gönderdikten sonra hiçbir şey göndermiyor ve akışı
// yanıt başlıklarından TAM ~20,0 sn sonra (±100 ms, 224+ örnek, birden çok gün,
// istisnasız) temiz olmayan biçimde kapatıyor. Akış düzgün bitseydi `Ok(())`
// dönerdi; her seferinde gövde çözme hatası geliyor.
//
// BUNLAR DENENDİ VE ELENDİ — tekrar denemeyin:
//   • Ayrıştırma/tamponlama hatası DEĞİL: chunk'lar JSON'un ortasından
//     bölünüyor, birleştirilip sorunsuz parse ediliyor.
//   • reqwest kaynaklı DEĞİL: 0.12 varsayılanları doğrulandı
//     (http2_keep_alive_interval=None, pool_idle_timeout=90 sn).
//   • Yerel DPI proxy'si kaynaklı DEĞİL: akış oradan geçmiyor (günlüklerde
//     eşleşen CONNECT yok).
//   • BOŞTA-ZAMAN AŞIMI DEĞİL: `use_rustls_tls()` ile ALPN açılıp bağlantı
//     fiilen HTTP/2.0'a çıkarıldı ve HTTP/2 PING keep-alive (10 sn) devreye
//     alındı — akış YİNE 19,93 sn'de kesildi. Protokolden ve PING'den bağımsız.
//
// SONUÇ: 20 sn sunucunun bu uç noktaya koyduğu SABİT AKIŞ ÖMRÜ. İstemci bunu
// engelleyemez; sitenin kendi EventSource'u da sürekli yeniden bağlanıyor
// (tarayıcı bunu sessizce yaptığı için kullanıcı fark etmiyor). Doğru yaklaşım
// kopmayı önlemeye çalışmak değil, yeniden bağlanmayı UCUZ, SESSİZ ve VERİ
// KAYIPSIZ yapmaktır — kaçan bildirim bir sonraki "initial" listesinde
// yakalanır (tazelik + `seen` süzgeçleri tekrarı eler).
//
// TAZELEME ÖLÇÜMÜ — GATEWAY-TOKEN ARKA PLANDA TAZELENEMEZ (canlı test):
// Vanguard `Gateway-Token`'ı ~60 sn yaşıyor ve yalnızca RENDER EDEN bir sayfada
// yenilenebiliyor. Tepside 8 dakika boyunca yapılan test: motor `Media`ya
// uyandırıldı, sayfa 4 kez tam olarak yeniden yüklendi (DPI günlüğünde tam
// CONNECT fırtınası görünüyor) — token BİR KEZ BİLE tazelenmedi. Tazelenme
// ancak pencere ön plana döndüğü an (03:00:49) gerçekleşti.
// Sebep büyük olasılıkla challenge'ın görsel doğrulamaya bağlı olması
// (günlüklerde sürekli `challenges.cloudflare.com` + `canvas.openani.me`).
//
// SONUÇ: arka planda bağlanmayı denemek GARANTİLİ 400'dür. Bu yüzden token
// bayatken ve hiçbir pencere ön planda değilken akış DURAKLATILIR; pencere geri
// gelince sayfa token'ı kendiliğinden tazeler ve akış sürer. Kaçan bildirim
// kaybolmaz, "initial" listesinde yakalanır.
//
// DEADLOCK UYARISI:
// cookies_for_url() Windows'ta SENKRON komut/event handler içinde çağrılırsa
// KİLİTLENİR (wry#583). Bu yüzden yalnızca spawn_blocking içinde, ana thread
// dışında çağrılır. Bunu run_on_main_thread'e SARMAYIN.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};

const API_ORIGIN: &str = "https://api.openani.me";
const SITE_ORIGIN: &str = "https://openani.me";

const RECONNECT_MIN_MS: u64 = 1_000;
const RECONNECT_MAX_MS: u64 = 30_000;

/// Bir akışın "sağlıklı" sayılması için yaşaması gereken en kısa süre.
///
/// 15 sn SEÇİLDİ ÇÜNKÜ sunucu her akışı ~20 sn'de kapatıyor (canlı testle
/// doğrulandı; protokolden bağımsız, bkz. AKIŞ ÖLÇÜMÜ notu). Yani 20 sn'lik
/// akış BAŞARISIZLIK DEĞİL, sunucunun normal çevrimidir — sitenin kendi
/// EventSource'u da aynı şekilde sürekli yeniden bağlanır. Bu yüzden eşik
/// 20'nin altında: normal çevrim bekleyişi sıfırlar ve hemen yeniden bağlanırız
/// (bildirim gecikmesi düşük kalır). Yalnızca 15 sn'yi bile doldurmadan ölen
/// akışlar (gerçek ağ/kimlik sorunu) bekleyişi kademeli olarak 30 sn'ye açar.
const HEALTHY_STREAM_MS: u64 = 15_000;

/// Gateway-Token bu yaştan sonra BAYAT sayılır ve kullanılmadan önce
/// tazelenmeye çalışılır.
///
/// ÖLÇÜM (oturum günlüklerinden, 6 ayrı oturum): pencere tepsiye gizlendikten
/// (→ sayfa donduruldu → token yansıtma durdu) sonra ilk HTTP 400
/// "…çerezleri temizleyin." hatası, token'ın son yansıtılmasından 52–74 sn
/// sonra geliyor. 45 sn bu aralığın güvenli tarafında kalır.
const GATEWAY_TOKEN_MAX_AGE_MS: u64 = 45_000;

/// İki tazeleme denemesi arasındaki en kısa süre (sayfayı boş yere uyandırma).
const TOKEN_REFRESH_COOLDOWN_MS: u64 = 20_000;

/// Uyandırılan sayfanın taze token üretmesi için tanınan süre.
const TOKEN_REFRESH_WAIT_MS: u64 = 4_000;

/// Bu süreden uzun arka planda kalan sayfanın oturumu BAYAT sayılır ve pencere
/// geri getirilirken bir kez yeniden yüklenir (bkz. restore_and_focus_window).
/// Gateway-Token ~60 sn yaşadığı için eşik onun biraz altında tutuldu.
const STALE_SESSION_MS: u64 = 60_000;

/// "Görüldü" listesi sınırsız büyümesin.
const MAX_SEEN: usize = 400;
/// Toast için "taze" sayılma penceresi. createdAt bundan daha eski bir bildirim,
/// listede yeni belirse bile masaüstüne ATILMAZ (birikmiş eski bildirim koruması).
const MAX_TOAST_AGE_MS: u64 = 180 * 60 * 1000; // 30 dakika

/// Şu anki zaman (unix milisaniye).
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ──────────────────────────────────────────────
// Durum
// ──────────────────────────────────────────────

/// Sitenin JS köprüsünden yansıttığı oturum/hesap bilgisi. Özel tepsi menüsü
/// (native_tray_menu) hangi öğeleri göstereceğine buna bakarak karar verir.
#[derive(Default, Clone)]
struct Account {
    logged_in: bool,
    profile_url: Option<String>,
    username: Option<String>,
    avatar_url: Option<String>,
    /// SSE bildirimlerindeki userId'den türetilen profil URL'i — JS DOM'dan
    /// profil bulamazsa yedek (menüde "Profil Görüntüle").
    sse_profile_url: Option<String>,
}

#[derive(Default)]
pub struct SuperNotifState {
    /// Kullanıcı ayarı (Süper Bildirimler açık mı).
    pub enabled: AtomicBool,
    /// Dinleyici döngüsü yalnızca bir kez başlatılır.
    listener_started: AtomicBool,
    /// Tıklama sinyal dosyası izleyicisi yalnızca bir kez başlatılır.
    click_watcher_started: AtomicBool,
    /// SSE akışı en az bir kez 200 ile bağlandıysa true → kullanıcı kesin giriş
    /// yapmıştır (tepsi menüsü öğelerini bu belirler; JS DOM sezgisinden bağımsız).
    sse_authed: AtomicBool,
    /// Sayfadan gelen Gateway-Token (varsa isteğe eklenir).
    gateway_token: Mutex<Option<String>>,
    /// Gateway-Token'ın sayfadan en son yansıtıldığı an. Token KISA ÖMÜRLÜ
    /// (ölçüm: ~60 sn) olduğundan yaşı bilinmeden kullanılamaz — bkz.
    /// GATEWAY_TOKEN_MAX_AGE_MS ve refresh_gateway_token.
    gateway_token_at: Mutex<Option<std::time::Instant>>,
    /// Sayfaya en son ne zaman "token'ı tazele" dendiği — uyandırma
    /// isteklerinin birbirini kovalamasını önler.
    last_token_refresh: Mutex<Option<std::time::Instant>>,
    /// Sitenin KENDİ api.openani.me isteklerinden yansıtılan canlı
    /// `Authorization` token'ı. SPA erişim token'ını bellekte tutup her istek
    /// öncesi yeniler; WebView2 çerez deposundaki `token` kopyası bayatlayıp
    /// 401 döndürebilir. Doluysa çerez yerine bu kullanılır.
    auth_token: Mutex<Option<String>>,
    /// Toast gösterilmiş bildirim kimlikleri — tekrarları eler.
    seen: Mutex<HashSet<String>>,
    /// Sitenin yansıttığı oturum/hesap bilgisi (özel tepsi menüsü için).
    account: Mutex<Account>,
}

/// `enabled` durumunun diske yansıması. NEDEN GEREKLİ: `SuperNotifState`
/// her process başlangıcında `enabled=false` ile başlar; gerçek tercih
/// yalnızca sayfa yüklenip JS `sn_set_enabled` çağırınca öğrenilir. Kullanıcı
/// pencereyi bu gerçekleşmeden ÖNCE kapatırsa (ExitRequested), Rust tarafı
/// "kapalı" sanıp tepsi oturumunu hiç açmadan doğrudan çıkıyordu — bu dosya
/// son bilinen değeri process'ler arası hatırlatarak bu yarışı ortadan
/// kaldırır (bkz. lib.rs RunEvent::ExitRequested).
fn super_notif_flag_path() -> std::path::PathBuf {
    std::env::temp_dir().join("OpenAnime_super_notif_enabled.flag")
}

impl SuperNotifState {
    pub fn new() -> Self {
        let state = Self::default();
        if let Ok(v) = std::fs::read_to_string(super_notif_flag_path()) {
            state.enabled.store(v.trim() != "0", Ordering::SeqCst);
        }
        // Bayrak dosyası yoksa (ilk kurulum / silinmiş) GÜVENLİ varsayılan:
        // KAPALI. Eskiden burada `true` yazılıyordu — kullanıcı ayarı daha
        // önce elle kapatmış olsa bile (localStorage'daki gerçek tercih
        // sayfa yüklenip JS `sn_set_enabled` çağırana kadar birkaç yüz ms
        // gecikmeli öğrenilir), process her başlangıçta kısa süreliğine
        // "açık" görünüyordu. `Default` zaten `AtomicBool::default() == false`
        // olduğundan burada ayrıca yazmaya gerek yok.
        state
    }
}

#[derive(Clone, Serialize, Debug)]
pub struct ToastPayload {
    pub id: String,
    pub title: String,
    pub body: String,
    /// Sunucunun bildirim `type`'ı (toast rozet ikonu/rengini belirler).
    pub notif_type: String,
    pub image: Option<String>,
    pub url: Option<String>,
}

/// Sitenin bildirim nesnesi. Alan adları openani.me istemcisinden doğrulandı.
#[derive(Deserialize, Debug)]
struct Notification {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    href: Option<String>,
    /// null → okunmamış. Okunmuş bildirim için toast gösterilmez.
    #[serde(rename = "readAt", default)]
    read_at: Option<Value>,
    // NOT: `alias = "id"` KULLANMA. Sunucu JSON'unda hem `_id` hem `id` birlikte
    // geliyor; alias olursa serde "duplicate field" hatası verip TÜM bildirimi
    // düşürüyor (Vec<Notification> deserialize başarısız → 0 öğe → toast yok).
    // `_id` kanonik kimlik; ayrı `id` alanı yok sayılır.
    #[serde(rename = "_id", default)]
    id: Option<String>,
    #[serde(rename = "createdAt", default)]
    created_at: Option<Value>,
    /// Bildirimi alan kullanıcının kimliği → profil URL'i (tepsi menüsü yedeği).
    #[serde(rename = "userId", default)]
    user_id: Option<String>,
    /// Bildirim türü: "comment-like", "comment-reply", "new-episode" vb.
    /// Toast rozet ikonu ve aksan rengini seçmekte kullanılır.
    #[serde(rename = "type", default)]
    kind: Option<String>,
}

#[derive(Deserialize, Debug)]
struct SseEvent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    data: Value,
}

// ──────────────────────────────────────────────
// Yardımcılar
// ──────────────────────────────────────────────

/// Göreli yolları mutlak URL'ye çevirir (/anime/x → https://openani.me/anime/x).
fn absolutize(raw: &str) -> String {
    let s = raw.trim();
    if s.starts_with("http://") || s.starts_with("https://") {
        s.to_string()
    } else if let Some(rest) = s.strip_prefix('/') {
        format!("{}/{}", SITE_ORIGIN, rest)
    } else {
        format!("{}/{}", SITE_ORIGIN, s)
    }
}

impl Notification {
    fn is_unread(&self) -> bool {
        matches!(self.read_at, None | Some(Value::Null))
    }

    /// createdAt (unix milisaniye) — sayı ya da float olarak gelebilir.
    fn created_at_ms(&self) -> Option<u64> {
        self.created_at
            .as_ref()
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
    }

    /// Bildirim yeterince taze mi (createdAt son `max_age_ms` içinde).
    /// createdAt yoksa: güvenli taraf → TAZE DEĞİL (eski backlog toast'lanmasın).
    fn is_recent(&self, now_ms: u64, max_age_ms: u64) -> bool {
        match self.created_at_ms() {
            Some(ts) => now_ms.saturating_sub(ts) <= max_age_ms,
            None => false,
        }
    }

    /// Kimlik alanı yoksa içerikten deterministik bir imza üret.
    fn stable_id(&self) -> String {
        if let Some(id) = self.id.as_deref().filter(|s| !s.is_empty()) {
            return id.to_string();
        }
        let sig = format!(
            "{}|{}|{}",
            self.title.as_deref().unwrap_or(""),
            self.message.as_deref().unwrap_or(""),
            self.created_at
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_default(),
        );
        let mut hash: u64 = 1469598103934665603;
        for b in sig.as_bytes() {
            hash ^= *b as u64;
            hash = hash.wrapping_mul(1099511628211);
        }
        format!("sig_{:x}", hash)
    }

    fn into_payload(self) -> ToastPayload {
        let id = self.stable_id();
        ToastPayload {
            id,
            title: self
                .title
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "OpenAnime".to_string()),
            body: self.message.unwrap_or_default(),
            notif_type: self.kind.unwrap_or_default(),
            image: None,
            url: self.href.map(|h| absolutize(&h)),
        }
    }
}

/// URL'den anime slug'ını çıkarır: `/anime/chainsmoker-cat/1/1#...` → `chainsmoker-cat`.
fn slug_from_url(url: &str) -> Option<String> {
    let idx = url.find("/anime/")? + "/anime/".len();
    let rest = &url[idx..];
    let end = rest
        .find(|c| c == '/' || c == '#' || c == '?')
        .unwrap_or(rest.len());
    let slug = &rest[..end];
    if slug.is_empty() {
        None
    } else {
        Some(slug.to_string())
    }
}

/// Poster URL'sini toast için normalize eder: TMDB → OpenAnime CDN, küçük boyut.
/// (Sitenin poster-fetcher.js'iyle aynı kural; toast küçük olduğu için w200.)
fn normalize_poster(url: &str) -> String {
    if !url.starts_with("http") {
        return url.to_string();
    }
    let mut out = url.replace("image.tmdb.org", "image.openanime.net");
    // /t/p/<boyut>/ segmentini w200 ile değiştir.
    if let Some(start) = out.find("/t/p/") {
        let after = start + "/t/p/".len();
        if let Some(rel) = out[after..].find('/') {
            out.replace_range(start..after + rel + 1, "/t/p/w200/");
        }
    }
    out
}

/// WebView2 çerez deposundan `token` çerezini okur.
///
/// Yalnızca ana thread DIŞINDAN çağrılmalı (spawn_blocking). Bkz. dosya başı.
fn auth_token(app: &AppHandle) -> Option<String> {
    let win = app.get_webview_window("main")?;
    let url: tauri::Url = SITE_ORIGIN.parse().ok()?;

    let mut cookies = win.cookies_for_url(url).unwrap_or_default();
    if cookies.is_empty() {
        cookies = win.cookies().unwrap_or_default();
    }

    // Teşhis: çerez ADLARI loglanır, DEĞERLERİ asla — token bir kimlik bilgisi
    // ve oturum logu diske yazılıyor.
    let names: Vec<&str> = cookies.iter().map(|c| c.name()).collect();
    crate::dbg_log!("[SüperBildirim] Çerezler: {:?}", names);

    let tok = cookies
        .iter()
        .find(|c| c.name() == "token")
        .map(|c| c.value().to_string())
        .filter(|v| !v.is_empty());

    match &tok {
        Some(t) => crate::dbg_log!("[SüperBildirim] token çerezi bulundu ({} karakter)", t.len()),
        None => crate::dbg_log!("[SüperBildirim] token çerezi YOK"),
    }

    tok
}

// ──────────────────────────────────────────────
// SSE dinleyici
// ──────────────────────────────────────────────

/// Teşhis için: uzun metni kısaltır (tek satıra indirip ilk `max` karakter).
fn preview(s: &str, max: usize) -> String {
    let one_line = s.replace('\n', "\\n");
    let short: String = one_line.chars().take(max).collect();
    if one_line.chars().count() > max {
        format!("{}…", short)
    } else {
        short
    }
}

/// Tek bir SSE olayını (ham `data:` bloğu) işler.
fn handle_sse_block(app: &AppHandle, block: &str) {
    // SSE alanları: `data:`, `event:`, `id:`, `:` (yorum/keep-alive).
    // Bir olay birden çok `data:` satırına bölünmüş olabilir.
    let mut data = String::new();
    let mut event_name: Option<String> = None;
    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.trim_start());
        } else if let Some(rest) = line.strip_prefix("event:") {
            event_name = Some(rest.trim().to_string());
        }
    }

    // TEŞHİS: gelen HER blok loglanır (dbg). Sunucunun tel üzerinde ne
    // gönderdiğini birebir görürüz — sorun burada saklı olabilir.
    crate::dbg_log!(
        "[SüperBildirim] SSE blok · event:{:?} · data:{}",
        event_name,
        preview(&data, 400)
    );

    if data.is_empty() {
        // Keep-alive / yorum satırı — bağlantı canlı demektir.
        return;
    }

    let ev = match serde_json::from_str::<SseEvent>(&data) {
        Ok(ev) => ev,
        Err(e) => {
            crate::dbg_log!(
                "[SüperBildirim] SSE JSON parse HATASI: {} · veri: {}",
                e,
                preview(&data, 400)
            );
            return;
        }
    };

    crate::dbg_log!("[SüperBildirim] SSE olay tipi: '{}'", ev.kind);

    match ev.kind.as_str() {
        // Hem "initial" (bağlanınca gelen liste) hem "new" (canlı) aynı süzgeçten
        // geçer: OKUNMAMIŞ + TAZE + görülmemiş olanlar toast'lanır. Zaman damgası
        // koruması eski backlog'u eler; ilk-bağlanış özel-durumuna gerek kalmadı.
        "initial" => {
            let items: Vec<Notification> = serde_json::from_value(ev.data).unwrap_or_default();
            crate::dbg_log!("[SüperBildirim] initial · {} öğe", items.len());
            process_notifications(app, items);
        }
        "new" => match serde_json::from_value::<Notification>(ev.data) {
            Ok(n) => {
                crate::dbg_log!("[SüperBildirim] new · 1 öğe");
                process_notifications(app, vec![n]);
            }
            Err(e) => {
                crate::dbg_log!("[SüperBildirim] 'new' bildirimi parse edilemedi: {}", e);
            }
        },
        other => {
            // Sunucu "initial"/"new" DIŞINDA bir tip gönderiyorsa burada görürüz.
            crate::dbg_log!(
                "[SüperBildirim] BİLİNMEYEN olay tipi '{}' · veri: {}",
                other,
                preview(&ev.data.to_string(), 400)
            );
        }
    }
}

/// Bildirimleri değerlendirip toast'lanacakları gösterir.
///
/// Gösterme koşulu: OKUNMAMIŞ (readAt null) VE TAZE (createdAt son
/// `MAX_TOAST_AGE_MS` içinde) VE daha önce gösterilmemiş. Tümü `seen`'e eklenir.
///
/// Zaman damgası koruması kritiktir: reconnect'te veya `seen` budandığında eski
/// bir bildirim (1 gün / 20 gün önce) tekrar listede belirirse "yeni" sanılıp
/// masaüstüne atılmasın. Sadece gerçekten yeni gelenler gösterilir.
fn process_notifications(app: &AppHandle, items: Vec<Notification>) {
    if items.is_empty() {
        return;
    }
    let now = now_millis();
    let state = app.state::<SuperNotifState>();

    // userId → profil URL yedeği (JS DOM'dan profil bulunamazsa tepsi menüsü kullanır).
    if let Some(uid) = items.iter().find_map(|n| n.user_id.as_deref()) {
        if let Ok(mut acc) = state.account.lock() {
            if acc.sse_profile_url.is_none() {
                acc.sse_profile_url = Some(format!("{}/profile/{}", SITE_ORIGIN, uid));
            }
        }
    }

    let mut fresh: Vec<ToastPayload> = Vec::new();
    {
        let Ok(mut seen) = state.seen.lock() else {
            return;
        };
        for n in items {
            let id = n.stable_id();
            let unread = n.is_unread();
            let recent = n.is_recent(now, MAX_TOAST_AGE_MS);
            let seen_before = seen.contains(&id);
            let show = unread && recent && !seen_before;
            crate::dbg_log!(
                "[SüperBildirim]   öğe id={} · unread={} · taze={} · görüldü={} · başlık={:?} → GÖSTER={}",
                id, unread, recent, seen_before, n.title.as_deref().unwrap_or(""), show
            );
            seen.insert(id);
            if show {
                fresh.push(n.into_payload());
            }
        }
        if seen.len() > MAX_SEEN {
            let excess: Vec<String> = seen.iter().take(seen.len() - MAX_SEEN).cloned().collect();
            for e in excess {
                seen.remove(&e);
            }
        }
    }
    if fresh.is_empty() {
        crate::dbg_log!("[SüperBildirim] gösterilecek yeni/taze bildirim yok");
    } else {
        dispatch(app, fresh);
    }
}

/// Akışın başarısızlık nedeni.
struct StreamFailure {
    msg: String,
    /// Vanguard kimlik reddi (HTTP 400/401). Bu hata KENDİ KENDİNE geçmez:
    /// aynı bayat Gateway-Token'la tekrar denemek hep aynı sonucu verir, o
    /// yüzden çağıran token'ı tazelemek zorundadır (bkz. start_listener).
    gateway_rejected: bool,
}

/// `?` ile mevcut `Result<_, String>` dönüşlerini olduğu gibi kullanabilmek için.
impl From<String> for StreamFailure {
    fn from(msg: String) -> Self {
        Self {
            msg,
            gateway_rejected: false,
        }
    }
}

/// Akışa bağlanır ve kopana kadar olayları işler.
/// Ok(()) → akış düzgün sonlandı (yeniden bağlanılmalı).
///
/// `connected`: bağlantı 200 ile kurulduğu AN. None kalırsa hiç bağlanılamamış
/// demektir. Çağıran, akışın ne kadar yaşadığını buradan hesaplar ve yeniden
/// bağlanma bekleyişini buna göre ayarlar (bkz. start_listener).
async fn run_stream(
    app: &AppHandle,
    connected: &mut Option<std::time::Instant>,
) -> Result<(), StreamFailure> {
    // Öncelik sırası:
    //   1) Sitenin canlı isteklerinden yansıtılan Authorization token'ı
    //      (JS köprüsü — settings-ui). Çerezdeki kopya bayat olabildiğinden
    //      site ne gönderiyorsa BİREBİR onu kullanmak 401'i önler.
    //   2) Yoksa WebView2 çerez deposundaki `token`.
    let relayed = app
        .state::<SuperNotifState>()
        .auth_token
        .lock()
        .ok()
        .and_then(|t| t.clone());

    let token = match relayed {
        Some(t) => t,
        None => {
            let app_c = app.clone();
            tauri::async_runtime::spawn_blocking(move || auth_token(&app_c))
                .await
                .map_err(|e| format!("çerez görevi düştü: {}", e))?
                .ok_or_else(|| "oturum token'ı yok (giriş yapılmamış)".to_string())?
        }
    };

    // Gateway-Token BAYAT mı? Vanguard token'ı ~60 sn'de bir geçersizleşiyor;
    // bayat token'la bağlanmak garanti HTTP 400 demek. Önce tazelemeyi dene
    // (bkz. refresh_gateway_token). Tazelenemezse yine de deneriz — belki
    // ölçtüğümüzden uzun yaşar; hata gelirse aşağıdaki 400 kolu devreye girer.
    match gateway_token_age(app) {
        Some(age) if age >= Duration::from_millis(GATEWAY_TOKEN_MAX_AGE_MS) => {
            refresh_gateway_token(app, &format!("bayat: {:?}", age)).await;
        }
        None => {
            refresh_gateway_token(app, "token yok").await;
        }
        _ => {}
    }

    let gateway = app
        .state::<SuperNotifState>()
        .gateway_token
        .lock()
        .ok()
        .and_then(|g| g.clone());

    // Akış süresiz açık kalır — genel timeout YOK, yalnızca bağlanma timeout'u.
    //
    // KEEP-ALIVE DENENDİ, İŞE YARAMADI — tekrar denemeyin (canlı test edildi):
    // 20 sn'lik kesilme bir BOŞTA-ZAMAN AŞIMI DEĞİL, sunucunun bu uç noktaya
    // koyduğu sabit akış ömrü. Kanıt: `use_rustls_tls()` + HTTP/2 PING
    // (interval 10 sn) ile bağlantı fiilen HTTP/2.0'a çıkarıldı ve akış YİNE
    // 19,93 sn'de kesildi — protokolden ve PING'den tamamen bağımsız.
    // Bu yüzden ne h2 keep-alive ne de rustls zorlaması burada duruyor:
    // fayda sağlamıyorlar, rustls ayrıca Windows sertifika deposu yerine
    // paketlenmiş kök setini kullandığı için kurumsal/MITM ağlarda risk.
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .user_agent(crate::platform_user_agent())
        .build()
        .map_err(|e| e.to_string())?;

    let token_len = token.len();
    let mut req = client
        .get(format!("{}/user/notifications/sse", API_ORIGIN))
        // Token BİREBİR gönderilir: yansıtılan değer sitenin başlığının aynısı
        // (ön ek dahil), çerez yedeği ise ham token — ikisi de olduğu gibi geçer.
        .header("Authorization", token)
        .header("Accept", "text/event-stream")
        .header("Origin", SITE_ORIGIN)
        .header("Referer", format!("{}/", SITE_ORIGIN));

    let has_gateway = gateway.is_some();
    if let Some(g) = gateway {
        req = req.header("Gateway-Token", g);
    }

    crate::dbg_log!(
        "[SüperBildirim] SSE isteği · {}/user/notifications/sse · token={} karakter · gateway={}",
        API_ORIGIN,
        token_len,
        if has_gateway { "var" } else { "YOK" }
    );

    let resp = req.send().await.map_err(|e| format!("bağlantı: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        // Gövdeyi de al: sunucu neden reddettiğini burada söylüyor
        // (ör. Vanguard gateway mi, token mı).
        let body = resp.text().await.unwrap_or_default();
        // 400/401 = Vanguard kimlik reddi. 400 gövdesi "Hata NNNNNN çerezleri
        // temizleyin." (kod her seferinde değişir), 401 ise "…denied by
        // OpenAnime Vanguard". İkisi de bayat/eksik Gateway-Token demek —
        // AYNI token'la tekrar denemenin faydası yok, tazelenmeli.
        let gateway_rejected = matches!(status.as_u16(), 400 | 401);
        return Err(StreamFailure {
            msg: format!(
                "HTTP {} — {}",
                status.as_u16(),
                body.chars().take(200).collect::<String>()
            ),
            gateway_rejected,
        });
    }

    let started = std::time::Instant::now();
    *connected = Some(started);
    // 200 ile bağlanıldı → kullanıcı kesin giriş yapmış (tepsi menüsü için).
    app.state::<SuperNotifState>()
        .sse_authed
        .store(true, Ordering::SeqCst);
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("(yok)")
        .to_string();
    // HTTP sürümü TEŞHİS İÇİN kritik: HTTP/2 ise yukarıdaki PING keep-alive'ı
    // fiilen çalışır; HTTP/1.1'e düşülmüşse PING diye bir şey YOKTUR ve akışı
    // canlı tutmanın protokol düzeyinde bir yolu kalmaz (yalnızca TCP keepalive).
    // Akış erken kopmaya devam ederse bakılacak İLK satır budur.
    let http_version = format!("{:?}", resp.version());
    crate::dbg_log!(
        "[SüperBildirim] Bildirim akışına bağlanıldı · HTTP {} · {} · content-type: {}",
        status.as_u16(),
        http_version,
        content_type
    );

    let mut stream = resp.bytes_stream();
    // BAYT tamponu (String DEĞİL). NEDEN: bir chunk sınırı çok baytlı bir UTF-8
    // karakterinin ORTASINA düşebilir — loglar chunk'ların JSON'un tam ortasından
    // bölündüğünü gösteriyor. Eski kod her chunk'ı tek tek `from_utf8_lossy`'den
    // geçirdiği için, sınıra denk gelen bir Türkçe karakter (ş/ğ/İ…) sessizce
    // U+FFFD'ye dönüşüp bildirim başlığını/metnini bozuyordu. Artık dönüşüm
    // yalnızca TAM olay bloklarında yapılır; blok sınırı daima bir '\n' (ASCII)
    // sonrasıdır, dolayısıyla hiçbir karakter ikiye bölünemez.
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk_no: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                // Akış koptu. Süreyi de bildir: "sessiz kalıp tam 20 sn'de
                // koparıldı" ile "gerçek ağ hatası" ancak böyle ayırt edilir.
                return Err(format!("akış ({:?} sonra): {}", started.elapsed(), e).into());
            }
        };
        chunk_no += 1;
        crate::dbg_log!(
            "[SüperBildirim] chunk #{} · {} bayt · {}",
            chunk_no,
            chunk.len(),
            preview(&String::from_utf8_lossy(&chunk), 400)
        );
        buf.extend_from_slice(&chunk);

        // Olaylar boş satırla ayrılır ("\n\n", "\r\n\r\n" ya da "\r\r").
        while let Some((pos, sep_len)) = find_event_end(&buf) {
            let block_bytes: Vec<u8> = buf.drain(..pos + sep_len).collect();
            let block = String::from_utf8_lossy(&block_bytes).replace("\r\n", "\n");
            handle_sse_block(app, &block);
        }

        // Bozuk/aşırı uzun veri bellek şişirmesin.
        if buf.len() > 1_000_000 {
            crate::dbg_log!("[SüperBildirim] tampon 1MB aştı, temizleniyor (ayraç bulunamadı?)");
            buf.clear();
        }
    }

    crate::dbg_log!(
        "[SüperBildirim] akış chunk döngüsü bitti ({} chunk, {:?} sürdü)",
        chunk_no,
        started.elapsed()
    );
    Ok(())
}

/// Tampondaki ilk SSE olay ayracını bulur → (ayracın başladığı indeks, uzunluğu).
/// SSE olayları boş bir satırla biter; satır sonu "\n", "\r\n" veya "\r" olabilir.
fn find_event_end(buf: &[u8]) -> Option<(usize, usize)> {
    for i in 0..buf.len() {
        if buf[i..].starts_with(b"\r\n\r\n") {
            return Some((i, 4));
        }
        if buf[i..].starts_with(b"\n\n") || buf[i..].starts_with(b"\r\r") {
            return Some((i, 2));
        }
    }
    None
}

pub fn start_listener(app: &AppHandle) {
    let state = app.state::<SuperNotifState>();
    if state.listener_started.swap(true, Ordering::SeqCst) {
        return; // zaten çalışıyor
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::dbg_log!("[SüperBildirim] Dinleyici başladı (SSE)");
        let mut backoff = RECONNECT_MIN_MS;
        /// Art arda kaç Vanguard reddinden sonra sayfa yeniden yüklenir.
        const GATEWAY_RELOAD_AFTER: u32 = 3;
        let mut gateway_rejections: u32 = 0;
        // "Ön plan bekleniyor" mesajı bir kez yazılsın (günlüğü doldurmasın).
        let mut waiting_for_foreground = false;

        loop {
            if !app
                .state::<SuperNotifState>()
                .enabled
                .load(Ordering::SeqCst)
            {
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }

            // BAYAT TOKEN + ARKA PLAN = DENEME. Bağlanmak GARANTİLİ 400 demek
            // (Vanguard render eden sayfa istiyor, tepside token tazelenemiyor).
            // Eskiden burada durmadan deneniyordu: ~35 sn'de bir kesin-400 alan
            // istek + her ~3,5 dk'da bir tam sayfa yeniden yüklemesi, saatlerce.
            // Bunun yerine bekliyoruz; pencere ön plana dönünce sayfa token'ı
            // kendiliğinden tazeliyor ve akış hemen kaldığı yerden sürüyor.
            // Bu sırada gelen bildirimler KAYBOLMUYOR — bağlanınca "initial"
            // listesiyle yakalanıyor (tazelik + `seen` süzgeçleri tekrarı eler).
            let token_stale = gateway_token_age(&app)
                .is_none_or(|age| age >= Duration::from_millis(GATEWAY_TOKEN_MAX_AGE_MS));
            if token_stale && !crate::any_window_foreground(&app) {
                if !waiting_for_foreground {
                    waiting_for_foreground = true;
                    crate::dbg_log!(
                        "[SüperBildirim] Gateway-Token bayat ve pencere arka planda → akış duraklatıldı (pencere geri gelince sürecek)"
                    );
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            if waiting_for_foreground {
                waiting_for_foreground = false;
                crate::dbg_log!("[SüperBildirim] Pencere geri geldi → akış sürdürülüyor");
                backoff = RECONNECT_MIN_MS;
            }

            let mut connected: Option<std::time::Instant> = None;
            let outcome = run_stream(&app, &mut connected).await;
            let lived = connected.map(|t| t.elapsed());

            match (&outcome, lived) {
                (Ok(()), _) => crate::dbg_log!("[SüperBildirim] Akış kapandı, yeniden bağlanılacak"),
                // Bağlanıp veri aldıktan sonra sessizce koparılmak bu sunucuda
                // OLAĞAN (bkz. AKIŞ ÖLÇÜMÜ notu): keep-alive gönderilmiyor ve
                // sessiz bağlantı yol üzerinde kesiliyor. Bunu "hata" diye
                // bağırmak günlüğü doldurup gerçek hataları gizliyordu.
                (Err(e), Some(d)) => crate::dbg_log!(
                    "[SüperBildirim] Akış {:?} sonra kesildi, yeniden bağlanılıyor · {}",
                    d,
                    e.msg
                ),
                (Err(e), None) => crate::dbg_log!("[SüperBildirim] Akış hatası: {}", e.msg),
            }

            // Vanguard reddi (400/401): token bayat. Tekrar denemeden ÖNCE
            // tazele — yoksa aynı ölü token'la sonsuza dek aynı 400 alınır
            // (sahadaki davranış tam olarak buydu). Tazeleme de sonuç vermezse
            // art arda birkaç retten sonra sayfayı yeniden yükleterek Vanguard'ı
            // sıfırdan kurdur.
            if outcome.as_ref().err().is_some_and(|e| e.gateway_rejected) {
                gateway_rejections += 1;
                let refreshed = refresh_gateway_token(&app, "Vanguard reddi").await;
                // Sayfayı yeniden yükletmek YALNIZCA ön planda anlamlı: arka
                // planda Vanguard challenge'ı tamamlanamadığı için reload hiçbir
                // şey değiştirmiyor, sadece bedava bir sayfa yüklemesi maliyeti
                // çıkarıyordu (ölçüm: 4 reload, 0 tazelenme).
                if !refreshed
                    && gateway_rejections >= GATEWAY_RELOAD_AFTER
                    && crate::any_window_foreground(&app)
                {
                    reload_page_for_gateway(&app);
                    gateway_rejections = 0;
                    // Sayfanın yeniden yüklenip Vanguard'ı kurması zaman alır.
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            } else {
                gateway_rejections = 0;
            }

            // Bekleyişi YALNIZCA akış "sağlıklı" yaşadıysa sıfırla.
            //
            // Eskiden bağlanabilmek tek başına yeterliydi (`connected == true`).
            // Ama bu sunucuda akış her seferinde ~20 sn'de kesildiğinden bekleyiş
            // her turda 1 sn'ye geri dönüyor, uygulama ömrü boyunca ~21 sn'de bir
            // yeniden bağlanıp aynı "initial" listesini baştan indiriyordu.
            // Artık kısa ömürlü akışlar bekleyişi sıfırlamaz: kopma ısrar ederse
            // aralık kademeli olarak 30 sn'ye açılır. Bu sırada gelen bildirim
            // KAYBOLMAZ — bağlantı kurulunca "initial" listesiyle yine yakalanır
            // (tazelik + `seen` süzgeçleri tekrarı zaten eliyor).
            let healthy = lived.is_some_and(|d| d >= Duration::from_millis(HEALTHY_STREAM_MS));
            if healthy {
                backoff = RECONNECT_MIN_MS;
            }

            tokio::time::sleep(Duration::from_millis(backoff)).await;
            backoff = (backoff * 2).min(RECONNECT_MAX_MS);
        }
    });
}

// ──────────────────────────────────────────────
// Toast gösterimi
// ──────────────────────────────────────────────

/// Yeni bildirimleri native WPF toast olarak gösterir.
///
/// Görünüm/render ayrıntıları: src-tauri/src/native_toast.rs. WebView penceresi
/// kullanılmaz (uzak siteyi saran ana WebView'a / asset pipeline'ına bağımlı
/// olmamak için). Toast'lar sağ altta, aynı anda tekli olarak gösterilir; yeni
/// bildirim önceki toast'ın yerini alır.
fn dispatch(app: &AppHandle, items: Vec<ToastPayload>) {
    for it in items {
        crate::log!("[Bildirim] {}", it.title);
        // Poster çekimi ağ isteği → toast'ı bloklamamak için ayrı görevde.
        // Poster gelmezse (ör. bildirim bir animeye bağlı değilse) toast yine
        // tip rozetiyle çıkar.
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // Poster için toplam ~6 sn bütçe; aşılırsa toast rozet ikonuyla çıkar.
            let poster = match it.url.as_deref() {
                Some(u) => tokio::time::timeout(Duration::from_secs(6), resolve_poster(&app, u))
                    .await
                    .ok()
                    .flatten(),
                None => None,
            };
            crate::native_toast::show_rich(&crate::native_toast::ToastContent {
                title: &it.title,
                body: &it.body,
                notif_type: &it.notif_type,
                poster_path: poster.as_deref(),
                url: it.url.as_deref(),
            });
        });
    }
}

/// Bildirim URL'sindeki animenin posterini API'den çekip %TEMP%'e indirir,
/// yerel dosya yolunu döner. Slug yoksa / poster bulunamazsa None.
///
/// Kimlik: SSE ile aynı `Authorization` + `Gateway-Token` kullanılır
/// (`/anime/{slug}` endpoint'i Vanguard korumalı). Poster CDN'i halka açık.
/// Slug bazında %TEMP%'te önbelleklenir — aynı anime için tekrar indirmez.
async fn resolve_poster(app: &AppHandle, url: &str) -> Option<String> {
    let slug = slug_from_url(url)?;

    // Önbellek: openanime-toast-poster-<slug>.jpg
    let safe: String = slug
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    let mut cache = std::env::temp_dir();
    cache.push(format!("openanime-toast-poster-{}.jpg", safe));
    if cache.exists() {
        return Some(cache.to_string_lossy().into_owned());
    }

    let state = app.state::<SuperNotifState>();
    let auth = state.auth_token.lock().ok().and_then(|t| t.clone());
    let gateway = state.gateway_token.lock().ok().and_then(|g| g.clone());

    // Kısa timeout: poster toast'ı geciktirmemeli. Süre aşılırsa çağıran
    // (dispatch) zaten posteri atlayıp toast'ı rozet ikonuyla gösterir.
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(4))
        .user_agent(crate::platform_user_agent())
        .build()
        .ok()?;

    let mut req = client
        .get(format!("{}/anime/{}", API_ORIGIN, slug))
        .header("Accept", "application/json")
        .header("Origin", SITE_ORIGIN)
        .header("Referer", format!("{}/", SITE_ORIGIN));
    if let Some(a) = auth {
        req = req.header("Authorization", a);
    }
    if let Some(g) = gateway {
        req = req.header("Gateway-Token", g);
    }

    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        crate::dbg_log!(
            "[SüperBildirim] poster meta HTTP {} · slug={}",
            resp.status().as_u16(),
            slug
        );
        return None;
    }
    // reqwest'in "json" feature'ı açık değil → metni alıp serde_json ile parse et.
    let text = resp.text().await.ok()?;
    let json: Value = serde_json::from_str(&text).ok()?;

    // poster-fetcher.js ile aynı öncelik: pictures.avatar → banner → seasons[0].poster
    let avatar = json
        .pointer("/pictures/avatar")
        .and_then(|v| v.as_str())
        .or_else(|| json.pointer("/pictures/banner").and_then(|v| v.as_str()))
        .or_else(|| json.pointer("/seasons/0/poster").and_then(|v| v.as_str()))?;
    if avatar.contains("canvas.openani.me") {
        return None; // yer tutucu kapak; toast'ta gösterme
    }

    let poster_url = normalize_poster(avatar);
    let bytes = client.get(&poster_url).send().await.ok()?.bytes().await.ok()?;
    if bytes.is_empty() {
        return None;
    }
    std::fs::write(&cache, &bytes).ok()?;
    crate::dbg_log!("[SüperBildirim] poster indirildi · slug={} · {} bayt", slug, bytes.len());
    Some(cache.to_string_lossy().into_owned())
}

// ──────────────────────────────────────────────
// Tıklama köprüsü (WPF toast → Rust → sayfa)
// ──────────────────────────────────────────────
//
// WPF toast ayrı bir PowerShell süreci; Tauri'ye doğrudan geri kanalı yok.
// Toast'a tıklanınca clickUrl bir sinyal dosyasına yazılır (bkz. native_toast).
// Burada o dosyayı kısa aralıkla izleyip, belirince uygulamayı açıp URL'ye
// gideriz. Poll aralığı düşük CPU (dosya var mı kontrolü); yalnızca dinleyici
// açıkken çalışır.

fn click_signal_path() -> std::path::PathBuf {
    std::env::temp_dir().join(crate::native_toast::CLICK_SIGNAL_FILE)
}

fn tray_action_path() -> std::path::PathBuf {
    std::env::temp_dir().join(crate::native_tray_menu::TRAY_ACTION_FILE)
}

/// Son işlenen sinyal içeriği, dosya başına (silme başarısız olsa bile aynı
/// eylemi tekrar tekrar işlemeyi önler — bkz. `consume_signal` notu).
static LAST_CONSUMED_SIGNAL: Mutex<Option<(std::path::PathBuf, String)>> = Mutex::new(None);

/// Sinyal dosyasını okuyup siler; BOM/boşluk temizler. Boşsa None.
/// (PowerShell `Set-Content -Encoding UTF8` başa BOM `\u{feff}` ekler.)
///
/// ÖNEMLİ: `remove_file` başarısız olabilir (ör. AV taraması dosyayı anlık
/// kilitliyorsa) — bu durumda dosya diskte kalır ve 350ms'lik izleyici döngüsü
/// AYNI içeriği bir SONRAKİ turda tekrar okuyup Some döndürürdü, bu da
/// `navigate_to`'nun (tam sayfa `window.location.href` ataması, yani fiilen
/// F5) sürekli tekrar tetiklenmesine — kullanıcının "sayfa kendiliğinden
/// sürekli yenileniyor" olarak gördüğü davranışa — yol açardı. Silme
/// başarısız olsa da AYNI içeriği iki kez işlememek için son işlenen değeri
/// dosya başına saklayıp karşılaştırıyoruz.
fn consume_signal(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let s = content.trim_start_matches('\u{feff}').trim().to_string();
    if s.is_empty() {
        return None;
    }

    let removed = std::fs::remove_file(path).is_ok();
    if !removed {
        // Silme başarısız oldu: bu tam olarak daha önce işlediğimiz içerikse
        // (dosya hâlâ eski haliyle diskte duruyorsa) tekrar işleme, atla.
        let mut guard = LAST_CONSUMED_SIGNAL.lock().ok()?;
        if let Some((last_path, last_content)) = guard.as_ref() {
            if last_path == path && last_content == &s {
                return None;
            }
        }
        *guard = Some((path.to_path_buf(), s.clone()));
    } else {
        // Silme başarılıysa iz sürmeye gerek yok — bir sonraki yazım zaten
        // yeni bir dosya oluşturacak.
        if let Ok(mut guard) = LAST_CONSUMED_SIGNAL.lock() {
            if guard.as_ref().map(|(p, _)| p.as_path()) == Some(path) {
                *guard = None;
            }
        }
    }

    Some(s)
}

/// Özel tepsi menüsünden gelen eylemi uygular: "show" | "quit" | "nav:<url>".
fn handle_tray_action(app: &AppHandle, action: &str) {
    if action == "show" {
        show_main(app);
    } else if action == "quit" {
        crate::dbg_log!("[TepsiMenu] menüden çıkış");
        // RunEvent::ExitRequested'te arkaplan oturumunun yeniden açılmasını
        // engellemek için gerçek çıkış bayrağını ÖNCE set ediyoruz.
        crate::APP_QUITTING.store(true, std::sync::atomic::Ordering::SeqCst);
        app.exit(0);
    } else if let Some(url) = action.strip_prefix("nav:") {
        crate::dbg_log!("[TepsiMenu] menü → {}", url);
        navigate_to(app, url);
    }
}

/// Ana pencereyi öne getirip verilen URL'ye gider (SSE toast tıklaması + komut
/// ortak kullanır). Tam sayfa yüklemesi: SPA router'ının iç API'sine bağımlı
/// olmamak için `location.href` set edilir (kırılgan değil, kesin çalışır).
pub fn restore_and_focus_window(win: &tauri::WebviewWindow) {
    // Süreyi resume'dan ÖNCE oku: `resume_webview` pencereyi Foreground'a
    // aldığı anda arka plan sayacı silinir.
    let slept = crate::background_duration(&win.app_handle(), win.label());

    if win.is_minimized().unwrap_or(false) {
        let _ = win.unminimize();
    }
    let _ = win.show();
    let _ = win.set_focus();

    // Askıdan çıkarmayı `Focused(true)` olayına BIRAKMA: show() odak olayı
    // üretmez, set_focus() de pencere zaten odak sahibiyse olayı yaymayabilir.
    // O durumda webview askıda (SetIsVisible(false) + TrySuspend) kalır ve
    // kullanıcı boş/donuk pencere görür. Bu yüzden koşulsuz geri döndürüyoruz.
    crate::resume_webview(win);

    // OTURUM TAZELEME (Bug 3): sayfa uzun süre dondurulmuş kaldıysa SPA'nın
    // bellekte tuttuğu erişim token'ı ve Vanguard `Gateway-Token`'ı çoktan
    // geçersizleşmiştir (ölçüm: gateway token ~60 sn yaşıyor). Donmuş sayfa
    // uyandırıldığında bu ölü kimlik bilgileriyle istek atıp 400/401 alıyor ve
    // arayüzü "çıkış yapılmış" gibi çiziyordu — kullanıcının gördüğü "tepsiden
    // açınca hesaptan çıkmış görünüyor" durumu tam olarak budur.
    // Tek seferlik tam sayfa yüklemesi SPA'yı sıfırdan kurar; çerezdeki kalıcı
    // oturum hâlâ geçerli olduğu için kullanıcı giriş yapmış olarak döner.
    if slept.is_some_and(|d| d >= Duration::from_millis(STALE_SESSION_MS)) {
        crate::dbg_log!(
            "[SüperBildirim] Pencere {:?} arka planda kaldı → oturum tazelemek için sayfa yenileniyor",
            slept.unwrap_or_default()
        );
        let _ = win.eval("try{window.location.reload();}catch(e){}");
    }
}

/// Ana pencereyi öne getirip verilen URL'ye gider (SSE toast tıklaması + komut
/// ortak kullanır). Tam sayfa yüklemesi: SPA router'ının iç API'sine bağımlı
/// olmamak için `location.href` set edilir (kırılgan değil, kesin çalışır).
fn navigate_to(app: &AppHandle, url: &str) {
    let target = absolutize(url);

    // "main" yoksa (artık X ile gerçekten kapanabiliyor) açık başka bir
    // pencere (örn. arkaplan tepsi oturumu) var mı diye bak; o da yoksa
    // doğrudan hedef URL'de yeni bir pencere aç.
    let main = app
        .get_webview_window("main")
        .or_else(|| app.webview_windows().into_iter().next().map(|(_, w)| w));

    let Some(main) = main else {
        if let Err(e) = crate::build_new_window(app, target) {
            crate::dbg_log!("[Tepsi] navigate_to: pencere açılamadı: {}", e);
        }
        return;
    };
    restore_and_focus_window(&main);

    if target.ends_with("/logout") {
        if let Ok(cookies) = main.cookies() {
            for cookie in cookies {
                let _ = main.delete_cookie(cookie);
            }
        }
        
        let home_url = absolutize("/");
        let script = format!(
            r#"try{{
                localStorage.clear();
                sessionStorage.clear();
                var cookies = document.cookie.split(";");
                for (var i = 0; i < cookies.length; i++) {{
                    var cookie = cookies[i].trim();
                    var eqPos = cookie.indexOf("=");
                    var name = eqPos > -1 ? cookie.substr(0, eqPos) : cookie;
                    document.cookie = name + "=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/";
                    document.cookie = name + "=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/;domain=.openani.me";
                }}
                window.location.href = {};
            }}catch(e){{}}"#,
            serde_json::to_string(&home_url).unwrap_or_else(|_| "\"/\"".into())
        );
        let _ = main.eval(&script);
    } else {
        let script = format!(
            "try{{window.location.href={};}}catch(e){{}}",
            serde_json::to_string(&target).unwrap_or_else(|_| "\"/\"".into())
        );
        let _ = main.eval(&script);
    }
}

pub fn start_click_watcher(app: &AppHandle) {
    let state = app.state::<SuperNotifState>();
    if state.click_watcher_started.swap(true, Ordering::SeqCst) {
        return;
    }
    // Açılışta bayat sinyal dosyalarını temizle (önceki oturumdan kalmış olabilir).
    let _ = std::fs::remove_file(click_signal_path());
    let _ = std::fs::remove_file(tray_action_path());

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::dbg_log!("[SüperBildirim] Tıklama/menü izleyicisi başladı");
        loop {
            tokio::time::sleep(Duration::from_millis(350)).await;
            // Toast tıklaması → ilgili sayfaya git.
            if let Some(url) = consume_signal(&click_signal_path()) {
                crate::log!("[Bildirim] Toast tıklandı → {}", url);
                navigate_to(&app, &url);
            }
            // Özel tepsi menüsü eylemi.
            if let Some(action) = consume_signal(&tray_action_path()) {
                handle_tray_action(&app, &action);
            }
        }
    });
}

// ──────────────────────────────────────────────
// Tepsi (tray) ikonu
// ──────────────────────────────────────────────
//
// Tepsi ikonu YALNIZCA Süper Bildirimler açıkken var olur. Kapalıyken
// uygulamanın arka planda yaşamasına gerek yok, dolayısıyla tepside
// durmasının da anlamı yok (X normal çıkış yapar — bkz. lib.rs).

const TRAY_ID: &str = "oa-tray";

pub fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        restore_and_focus_window(&win);
        return;
    }
    if let Some((_, win)) = app.webview_windows().into_iter().next() {
        restore_and_focus_window(&win);
        return;
    }
    if let Err(e) = crate::build_new_window(app, "https://openani.me/".to_string()) {
        crate::dbg_log!("[Tepsi] Sıfırdan pencere açılamadı: {}", e);
    }
}

pub fn ensure_tray(app: &AppHandle) -> Result<(), String> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_visible(true);
        return Ok(());
    }

    // Native menü YOK: sağ tık özel WPF menüsünü açar (native_tray_menu).
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("OpenAnime")
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle().clone();
            match event {
                TrayIconEvent::Click { button, button_state, rect, .. } => {
                    crate::log!("[Tepsi] Tıklama olayı: button={:?} state={:?}", button, button_state);
                    let (icon_x, icon_y) = match rect.position {
                        tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
                        tauri::Position::Logical(p) => (p.x, p.y),
                    };
                    let (icon_w, icon_h) = match rect.size {
                        tauri::Size::Physical(s) => (s.width as f64, s.height as f64),
                        tauri::Size::Logical(s) => (s.width, s.height),
                    };

                    tauri::async_runtime::spawn(async move {
                        match button {
                            MouseButton::Left => {
                                if button_state == MouseButtonState::Up {
                                    crate::log!("[Tepsi] Sol tık → show_main");
                                    show_main(&app);
                                }
                            }
                            MouseButton::Right => {
                                // Left koluyla aynı sebep: bu event Down VE Up için
                                // ayrı ayrı geliyor. Up filtresi olmadan tek fiziksel
                                // sağ tık `open_tray_menu`'yu İKİ KEZ tetikliyordu —
                                // native_tray_menu::show() bir öncekini `kill()`
                                // ettiğinden, Down'da başlatılan PowerShell/WPF süreci
                                // (Add-Type derlemesi 500ms-1sn sürebiliyor, bkz.
                                // native_tray_menu.rs) genelde henüz pencereyi
                                // göstermeden Up'ta öldürülüyor; art arda tıklamalarda
                                // veya yavaş sistemlerde menü hiç görünmeden kalabiliyordu.
                                if button_state == MouseButtonState::Up {
                                    crate::log!("[Tepsi] Sağ tık → open_tray_menu rect=({},{},{},{})", icon_x, icon_y, icon_w, icon_h);
                                    open_tray_menu(&app, (icon_x, icon_y, icon_w, icon_h));
                                }
                            }
                            _ => {}
                        }
                    });
                }
                TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => {
                    crate::log!("[Tepsi] Çift tık → show_main");
                    tauri::async_runtime::spawn(async move {
                        show_main(&app);
                    });
                }
                _ => {}
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app).map_err(|e| e.to_string())?;
    crate::dbg_log!("[SüperBildirim] Tepsi ikonu oluşturuldu");
    Ok(())
}

/// Oturum durumuna göre özel tepsi menüsünü kurup gösterir.
/// `icon_rect`: (left, top, width, height) — tepsi ikonunun fiziksel piksel
/// cinsinden ekran dikdörtgeni. Menü buna göre konumlanır (fareye göre değil).
fn open_tray_menu(app: &AppHandle, icon_rect: (f64, f64, f64, f64)) {
    use crate::native_tray_menu::{MenuEntry, MenuHeader};
    crate::log!("[TepsiMenu] open_tray_menu çağrıldı, rect={:?}", icon_rect);

    let acc = app
        .state::<SuperNotifState>()
        .account
        .lock()
        .ok()
        .map(|a| a.clone())
        .unwrap_or_default();

    // Giriş durumu: JS DOM sezgisi VEYA SSE'nin 200 ile bağlanmış olması.
    // Profil: JS bulduysa onu, yoksa userId'den türetilen yedeği kullan.
    let sse_authed = app
        .state::<SuperNotifState>()
        .sse_authed
        .load(Ordering::SeqCst);
    let logged_in = acc.logged_in || sse_authed;
    let profile = acc.profile_url.clone().or(acc.sse_profile_url.clone());

    crate::dbg_log!(
        "[TepsiMenu] menü açılıyor · giriş={} (js={}, sse={}) · profil={:?}",
        logged_in,
        acc.logged_in,
        sse_authed,
        profile.as_deref()
    );

    let mut entries: Vec<MenuEntry> = Vec::new();
    entries.push(MenuEntry {
        label: "OpenAnime'ı Aç".into(),
        glyph: 0xE80F, // Home
        action: "show".into(),
        danger: false,
    });

    let header = if logged_in {
        if let Some(p) = profile.clone() {
            entries.push(MenuEntry {
                label: "Profil Görüntüle".into(),
                glyph: 0xE77B, // Contact
                action: format!("nav:{}", p),
                danger: false,
            });
        }
        entries.push(MenuEntry {
            label: "Kütüphanem".into(),
            glyph: 0xE8F1, // Library
            action: format!("nav:{}/library", SITE_ORIGIN),
            danger: false,
        });
        entries.push(MenuEntry {
            label: "Son Eklenenler".into(),
            glyph: 0xE81C, // History/Recent — ana sayfa son bölümleri listeler
            action: format!("nav:{}/episodes/latest/1", SITE_ORIGIN),
            danger: false,
        });
        entries.push(MenuEntry {
            label: "Takvim".into(),
            glyph: 0xE787, // Calendar
            action: format!("nav:{}/calendar", SITE_ORIGIN),
            danger: false,
        });

        Some(MenuHeader {
            name: acc.username.clone().unwrap_or_else(|| "Hesabım".into()),
            subtitle: "Çevrimiçi".into(),
        })
    } else {
        entries.push(MenuEntry {
            label: "Son Eklenenler".into(),
            glyph: 0xE81C,
            action: format!("nav:{}/episodes/latest/1", SITE_ORIGIN),
            danger: false,
        });
        entries.push(MenuEntry {
            label: "Takvim".into(),
            glyph: 0xE787,
            action: format!("nav:{}/calendar", SITE_ORIGIN),
            danger: false,
        });
        None
    };

    entries.push(MenuEntry {
        label: "Kapat".into(),
        glyph: 0xE711, // Cancel
        action: "quit".into(),
        danger: true,
    });

    crate::native_tray_menu::show(header, entries, icon_rect);
}

#[allow(dead_code)]
pub fn remove_tray(app: &AppHandle) {
    if app.remove_tray_by_id(TRAY_ID).is_some() {
        crate::dbg_log!("[SüperBildirim] Tepsi ikonu kaldırıldı");
    }
}

// ──────────────────────────────────────────────
// Komutlar
// ──────────────────────────────────────────────

#[tauri::command]
pub async fn sn_set_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<SuperNotifState>();
    state.enabled.store(enabled, Ordering::SeqCst);
    // Bir sonraki process başlangıcında (JS henüz bildirmeden önce kapatılsa
    // bile) doğru değeri bilelim diye diske yansıt (best-effort).
    let _ = std::fs::write(super_notif_flag_path(), if enabled { "1" } else { "0" });
    crate::log!(
        "[Süper Bildirim] {}",
        if enabled { "açıldı" } else { "kapatıldı" }
    );

    // Süper Bildirimler yalnızca SSE bildirim akışını kontrol eder.
    // Tepsi ikonu her zaman görünür (bkz. lib.rs setup).
    if enabled {
        start_listener(&app);
    }

    Ok(())
}

#[tauri::command]
pub fn sn_set_gateway_token(app: AppHandle, token: String) -> Result<(), String> {
    if token.trim().is_empty() {
        return Ok(());
    }
    let state = app.state::<SuperNotifState>();
    let mut g = state.gateway_token.lock().map_err(|e| e.to_string())?;
    if g.as_deref() != Some(token.as_str()) {
        crate::dbg_log!("[SüperBildirim] Gateway-Token güncellendi");
        *g = Some(token);
        // Yaşı YALNIZCA değer gerçekten değiştiğinde sıfırla: aynı (bayat)
        // token'ın tekrar yansıtılması onu tazelemez, sadece aynı değerin hâlâ
        // orada durduğunu gösterir.
        if let Ok(mut at) = state.gateway_token_at.lock() {
            *at = Some(std::time::Instant::now());
        }
    }
    Ok(())
}

/// Arka plandaki pencerenin RENDER ETMEYE devam etmesi gerekiyor mu?
///
/// Süper Bildirimler açıkken EVET: Vanguard `Gateway-Token`'ı yalnızca render
/// eden bir sayfada tazelenebiliyor (bkz. TAZELEME ÖLÇÜMÜ notu). Sayfa
/// dondurulursa token ~60 sn içinde bayatlıyor ve bildirim akışı susuyor.
/// Bedeli: tepside TrySuspend + working-set trim uygulanamaz, yani RAM düşmez.
/// Bu bilinçli bir takas — özelliğin tepside çalışmasının bilinen tek yolu.
/// (X artık önce hafif `/settings` sayfasına gittiği için canlı tutulan sayfa
/// oynatıcı değil, ucuz bir ayar sayfası oluyor — bkz. lib.rs CloseRequested.)
pub fn needs_live_page(app: &AppHandle) -> bool {
    app.try_state::<SuperNotifState>()
        .is_some_and(|s| s.enabled.load(Ordering::SeqCst))
}

/// Gateway-Token'ın yaşı (hiç alınmadıysa None).
fn gateway_token_age(app: &AppHandle) -> Option<Duration> {
    app.state::<SuperNotifState>()
        .gateway_token_at
        .lock()
        .ok()
        .and_then(|a| *a)
        .map(|t| t.elapsed())
}

/// Sayfadan TAZE bir Gateway-Token ister ve gelmesini kısa süre bekler.
///
/// NEDEN GEREKLİ (Bug 2'nin kök nedeni): `Gateway-Token` OpenAnime Vanguard
/// tarafından üretilir, KISA ÖMÜRLÜDÜR (ölçüm: ~60 sn) ve onu tazeleyebilen tek
/// yer sayfanın kendisidir. Pencere tepsiye gizlenince WebView2 `TrySuspend`
/// ile dondurulur ve background-mode.js "hidden" modunda tüm `oaBgInterval`
/// timer'larını durdurur — token yansıtma timer'ı dahil. Böylece token bayatlar,
/// Vanguard her isteği `HTTP 400 {"error":"Hata NNNNNN çerezleri temizleyin."}`
/// ile reddeder ve tazeleyecek kimse olmadığı için bu SONSUZA DEK sürer.
/// (Token'sız denemek çözüm değil: o zaman `HTTP 401 … denied by OpenAnime
/// Vanguard` geliyor — başlık ZORUNLU.)
///
/// Çözüm: motoru pencereyi göstermeden kısa süreliğine uyandır, sayfanın kendi
/// Vanguard mantığının çalışmasına izin ver, token'ı yeniden okut, sonra eski
/// arka plan moduna (genelde DeepSleep) geri döndür.
async fn refresh_gateway_token(app: &AppHandle, reason: &str) -> bool {
    // ÖN PLAN ŞARTI (canlı testle kesinleşti — bkz. TAZELEME ÖLÇÜMÜ notu):
    // Sayfa render etmiyorsa Vanguard yeni token ÜRETMİYOR. Tepside 8 dakika
    // boyunca 4 kez tam sayfa yeniden yüklendi ve token bir kez bile
    // tazelenmedi; tazelenme ancak pencere ön plana döndüğü an gerçekleşti.
    // Bu yüzden arka plandayken denemenin FAYDASI YOK, ZARARI VAR (garantili
    // 400 alan istekler + boşuna sayfa yeniden yüklemeleri).
    if !crate::any_window_foreground(app) {
        crate::dbg_log!(
            "[SüperBildirim] token tazeleme atlandı ({}): pencere arka planda, Vanguard render eden sayfa istiyor",
            reason
        );
        return false;
    }
    // Çok sık uyandırma — sayfa boşuna diriltilmesin.
    // (Kilit bu blokta alınıp bırakılır; blok içinden `return` EDİLMEZ, aksi
    // halde `State` ödünçlemesi dönüş değerinden uzun yaşamış olurdu.)
    let cooling_down = {
        let state = app.state::<SuperNotifState>();
        let mut last = state.last_token_refresh.lock().ok();
        match last.as_deref_mut() {
            Some(slot) => match *slot {
                Some(t) if t.elapsed() < Duration::from_millis(TOKEN_REFRESH_COOLDOWN_MS) => true,
                _ => {
                    *slot = Some(std::time::Instant::now());
                    false
                }
            },
            None => true, // kilit zehirli — bu turu atla
        }
    };
    if cooling_down {
        return false;
    }

    let Some((label, win)) = app
        .get_webview_window("main")
        .map(|w| ("main".to_string(), w))
        .or_else(|| {
            app.webview_windows()
                .into_iter()
                .next()
                .map(|(l, w)| (l, w))
        })
    else {
        crate::dbg_log!("[SüperBildirim] token tazelenemedi ({}): açık pencere yok", reason);
        return false;
    };

    crate::dbg_log!(
        "[SüperBildirim] Gateway-Token tazeleniyor ({}) · pencere={}",
        reason,
        label
    );

    let before = app
        .state::<SuperNotifState>()
        .gateway_token
        .lock()
        .ok()
        .and_then(|g| g.clone());

    // Motoru uyandır (pencereyi GÖSTERMEDEN) — donmuş webview'da eval çalışmaz.
    crate::wake_webview_for_script(&win);
    let _ = win.eval("try{window.__oaSnRefreshToken&&window.__oaSnRefreshToken();}catch(e){}");

    // Taze token'ın yansıtılmasını bekle.
    let deadline = std::time::Instant::now() + Duration::from_millis(TOKEN_REFRESH_WAIT_MS);
    let mut changed = false;
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let now_tok = app
            .state::<SuperNotifState>()
            .gateway_token
            .lock()
            .ok()
            .and_then(|g| g.clone());
        if now_tok.is_some() && now_tok != before {
            changed = true;
            break;
        }
    }

    // Pencereyi gerçek durumuna göre yeniden uyut (tepsideyse DeepSleep'e döner).
    crate::restore_background_mode(app, &label);

    if changed {
        crate::dbg_log!("[SüperBildirim] Gateway-Token tazelendi");
    } else {
        // Sayfa uyandı ama Vanguard yeni token üretmedi. Tek kalan kesin yol
        // sayfayı yeniden yükletmek (sunucunun "çerezleri temizleyin" dediği
        // şeyin fiilî karşılığı) — ama bunu ancak gerçekten reddedildiğimizde
        // yaparız, düzenli bayatlama kontrolünde değil.
        crate::dbg_log!(
            "[SüperBildirim] Gateway-Token tazelenemedi ({}) — sayfa yeni token üretmedi",
            reason
        );
    }
    changed
}

/// Vanguard bizi reddetti (400/401) ve token tazeleme de sonuç vermedi:
/// gizli sayfayı yeniden yükleyerek Vanguard'ın yeniden kurulmasını sağla.
/// Bu, sunucunun kendi talimatının ("çerezleri temizleyin") uygulamadaki
/// karşılığıdır. Nadir ve maliyetli olduğu için yalnızca reddedilme
/// döngüsünde, cooldown'a tabi çağrılır.
fn reload_page_for_gateway(app: &AppHandle) {
    let Some(win) = app
        .get_webview_window("main")
        .or_else(|| app.webview_windows().into_iter().next().map(|(_, w)| w))
    else {
        return;
    };
    crate::log!("[Bildirim] Oturum doğrulaması yenileniyor (sayfa yeniden yükleniyor)");
    crate::wake_webview_for_script(&win);
    let _ = win.eval("try{window.location.reload();}catch(e){}");
}

/// Sitenin canlı `Authorization` başlığını Rust'a yansıtır (JS köprüsü).
///
/// SPA gerçek erişim token'ını bellekte tutar ve her api.openani.me isteğine
/// bu değeri koyar. Çerezdeki `token` kopyası bayatlayıp SSE akışında 401
/// döndürebildiğinden, burada sitenin fiilen kullandığı token yakalanıp
/// çerezin yerine geçer. Değer "Bearer " ön ekiyle veya ön eksiz gelebilir —
/// site ne gönderiyorsa BİREBİR saklanır, akışa aynen eklenir.
#[tauri::command]
pub fn sn_set_auth_token(app: AppHandle, token: String) -> Result<(), String> {
    let t = token.trim();
    if t.is_empty() {
        return Ok(());
    }
    let state = app.state::<SuperNotifState>();
    let mut a = state.auth_token.lock().map_err(|e| e.to_string())?;
    if a.as_deref() != Some(t) {
        crate::dbg_log!(
            "[SüperBildirim] Authorization token güncellendi ({} karakter)",
            t.len()
        );
        *a = Some(t.to_string());
    }
    Ok(())
}

/// Sitenin oturum/hesap bilgisini Rust'a yansıtır (JS köprüsü — super-notifications-ui).
///
/// Özel tepsi menüsü (native_tray_menu) hangi öğeleri göstereceğini (giriş var mı,
/// profil URL'i, kullanıcı adı, avatar) buradan öğrenir. Avatar değişmişse arka
/// planda indirilip menü açılışında hazır tutulur.
#[tauri::command]
pub fn sn_set_account(
    app: AppHandle,
    logged_in: bool,
    profile_url: Option<String>,
    username: Option<String>,
    avatar_url: Option<String>,
) -> Result<(), String> {
    let profile_url = profile_url.filter(|s| !s.trim().is_empty());
    let username = username.filter(|s| !s.trim().is_empty());
    let avatar_url = avatar_url.filter(|s| !s.trim().is_empty());
    crate::dbg_log!(
        "[TepsiMenu] hesap relay · giriş={} · profil={:?} · isim={:?} · avatar={:?}",
        logged_in,
        profile_url.as_deref(),
        username.as_deref(),
        avatar_url.as_deref()
    );

    // STICKY birleştirme: JS relay'i flip-flop yapıyor (DOM'da avatar bazen var
    // bazen yok). None gelen alan mevcut iyi değeri EZMESİN; giriş bir kez true
    // olunca sabit kalsın. Böylece anlık boş okuma menüyü bozmaz.
    let state = app.state::<SuperNotifState>();
    let mut a = state.account.lock().map_err(|e| e.to_string())?;
    if logged_in {
        a.logged_in = true;
    }
    if profile_url.is_some() {
        a.profile_url = profile_url;
    }
    if username.is_some() {
        a.username = username;
    }
    if avatar_url.is_some() {
        a.avatar_url = avatar_url;
    }
    Ok(())
}

/// Native toast'ı elle tetikler (bildirim beklemeden).
///
/// Geliştirme/destek içindir: DevTools konsolundan
///   __TAURI__.core.invoke("sn_test_toast")
/// çağrılınca, bildirim gelmiş gibi bir masaüstü toast'ı gösterilir.
/// Ayarın açık olmasına da gerek yoktur.
#[tauri::command]
pub async fn sn_test_toast(
    app: AppHandle,
    title: Option<String>,
    body: Option<String>,
) -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    dispatch(
        &app,
        vec![ToastPayload {
            // Her çağrıda benzersiz id — aksi halde toast tarafı tekrar sayıp eler.
            id: format!("test_{}", stamp),
            title: title.unwrap_or_else(|| "OpenAnime".to_string()),
            body: body
                .unwrap_or_else(|| "Test bildirimi — masaüstü toast'ı çalışıyor.".to_string()),
            notif_type: String::new(),
            image: None,
            url: None,
        }],
    );

    Ok(())
}

/// TEST: hesaptaki MEVCUT tüm bildirimleri toast olarak gösterir (okundu/eski
/// süzgeçlerini ATLAR, `seen`'e DOKUNMAZ). DevTools konsolundan:
///   __TAURI__.core.invoke("sn_test_notifications")
/// SSE'ye tek seferlik bağlanıp ilk "initial" listesini çeker, hepsini ~2 sn
/// arayla sırayla toast'lar (native toast aynı anda tek gösterir). Kaç bildirim
/// olduğunu döndürür.
#[tauri::command]
pub async fn sn_test_notifications(app: AppHandle) -> Result<usize, String> {
    let relayed = app
        .state::<SuperNotifState>()
        .auth_token
        .lock()
        .ok()
        .and_then(|t| t.clone());
    let token = match relayed {
        Some(t) => t,
        None => {
            let app_c = app.clone();
            tauri::async_runtime::spawn_blocking(move || auth_token(&app_c))
                .await
                .map_err(|e| format!("çerez görevi: {}", e))?
                .ok_or_else(|| "oturum token'ı yok (giriş yapılmamış)".to_string())?
        }
    };
    let gateway = app
        .state::<SuperNotifState>()
        .gateway_token
        .lock()
        .ok()
        .and_then(|g| g.clone());

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .user_agent(crate::platform_user_agent())
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client
        .get(format!("{}/user/notifications/sse", API_ORIGIN))
        .header("Authorization", token)
        .header("Accept", "text/event-stream")
        .header("Origin", SITE_ORIGIN)
        .header("Referer", format!("{}/", SITE_ORIGIN));
    if let Some(g) = gateway {
        req = req.header("Gateway-Token", g);
    }
    let resp = req.send().await.map_err(|e| format!("bağlantı: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("akış: {}", e))?;
        buf.push_str(&String::from_utf8_lossy(&chunk).replace("\r\n", "\n"));
        while let Some(pos) = buf.find("\n\n") {
            let block: String = buf.drain(..pos + 2).collect();
            let mut data = String::new();
            for line in block.lines() {
                if let Some(rest) = line.strip_prefix("data:") {
                    if !data.is_empty() {
                        data.push('\n');
                    }
                    data.push_str(rest.trim_start());
                }
            }
            if data.is_empty() {
                continue;
            }
            let Ok(ev) = serde_json::from_str::<SseEvent>(&data) else {
                continue;
            };
            if ev.kind == "initial" {
                let items: Vec<Notification> =
                    serde_json::from_value(ev.data).unwrap_or_default();
                let payloads: Vec<ToastPayload> =
                    items.into_iter().map(|n| n.into_payload()).collect();
                let count = payloads.len();
                crate::log!("[Bildirim] TEST: {} bildirim toast olarak gösterilecek", count);
                let app_c = app.clone();
                for (i, p) in payloads.into_iter().enumerate() {
                    let app_c = app_c.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(i as u64 * 2000)).await;
                        let poster = match p.url.as_deref() {
                            Some(u) => resolve_poster(&app_c, u).await,
                            None => None,
                        };
                        crate::native_toast::show_rich(&crate::native_toast::ToastContent {
                            title: &p.title,
                            body: &p.body,
                            notif_type: &p.notif_type,
                            poster_path: poster.as_deref(),
                            url: p.url.as_deref(),
                        });
                    });
                }
                return Ok(count);
            }
        }
        if buf.len() > 1_000_000 {
            break;
        }
    }
    Ok(0)
}

/// Toast'a tıklandı — ana pencereyi göster ve ilgili sayfaya git.
#[tauri::command]
pub async fn sn_open_notification(app: AppHandle, url: Option<String>) -> Result<(), String> {
    match url {
        Some(u) => navigate_to(&app, &u),
        None => {
            // URL yok: yalnızca pencereyi öne getir. (restore_and_focus_window
            // kullanılır — askıya alınmış webview'ı geri döndürmeyi de o yapar.)
            if let Some(main) = app.get_webview_window("main") {
                restore_and_focus_window(&main);
            }
        }
    }
    Ok(())
}
