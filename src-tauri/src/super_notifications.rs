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

impl SuperNotifState {
    pub fn new() -> Self {
        Self::default()
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

/// Akışa bağlanır ve kopana kadar olayları işler.
/// Ok(()) → akış düzgün sonlandı (yeniden bağlanılmalı).
async fn run_stream(app: &AppHandle, connected: &mut bool) -> Result<(), String> {
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

    let gateway = app
        .state::<SuperNotifState>()
        .gateway_token
        .lock()
        .ok()
        .and_then(|g| g.clone());

    // Akış süresiz açık kalır — genel timeout YOK, yalnızca bağlanma timeout'u.
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
        return Err(format!(
            "HTTP {} — {}",
            status.as_u16(),
            body.chars().take(200).collect::<String>()
        ));
    }

    *connected = true;
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
    crate::dbg_log!(
        "[SüperBildirim] Bildirim akışına bağlanıldı · HTTP {} · content-type: {}",
        status.as_u16(),
        content_type
    );

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut chunk_no: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("akış: {}", e))?;
        chunk_no += 1;
        let text = String::from_utf8_lossy(&chunk).replace("\r\n", "\n");
        // TEŞHİS: her ham chunk'ın boyutu + önizlemesi (dbg). Sunucu olayları
        // "\n\n" ile ayırmıyorsa veya farklı format gönderiyorsa burada belli olur.
        crate::dbg_log!(
            "[SüperBildirim] chunk #{} · {} bayt · {}",
            chunk_no,
            chunk.len(),
            preview(&text, 400)
        );
        buf.push_str(&text);

        // Olaylar boş satırla ayrılır.
        while let Some(pos) = buf.find("\n\n") {
            let block: String = buf.drain(..pos + 2).collect();
            handle_sse_block(app, &block);
        }

        // Bozuk/aşırı uzun veri bellek şişirmesin.
        if buf.len() > 1_000_000 {
            crate::dbg_log!("[SüperBildirim] tampon 1MB aştı, temizleniyor (ayraç bulunamadı?)");
            buf.clear();
        }
    }

    crate::dbg_log!("[SüperBildirim] akış chunk döngüsü bitti ({} chunk işlendi)", chunk_no);
    Ok(())
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

        loop {
            if !app
                .state::<SuperNotifState>()
                .enabled
                .load(Ordering::SeqCst)
            {
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }

            let mut connected = false;
            match run_stream(&app, &mut connected).await {
                Ok(()) => crate::dbg_log!("[SüperBildirim] Akış kapandı, yeniden bağlanılacak"),
                Err(e) => crate::dbg_log!("[SüperBildirim] Akış hatası: {}", e),
            }

            // Bağlantı kurulabildiyse bekleyişi sıfırla (site de böyle yapıyor);
            // kurulamıyorsa (giriş yok / ağ yok) kademeli olarak seyrelt.
            if connected {
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

/// Sinyal dosyasını okuyup siler; BOM/boşluk temizler. Boşsa None.
/// (PowerShell `Set-Content -Encoding UTF8` başa BOM `\u{feff}` ekler.)
fn consume_signal(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let _ = std::fs::remove_file(path);
    let s = content.trim_start_matches('\u{feff}').trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
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
    let _ = main.show();
    let _ = main.unminimize();
    let _ = main.set_focus();

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

/// Tepsi ikonuna sol tıklanınca (veya menüden "OpenAnime'ı Aç" seçilince)
/// çağrılır. Artık "main" penceresi hiçbir zaman gizli tutulmuyor (X
/// tuşuna basılınca gerçekten kapanıyor) — bu yüzden burada üç durumu ele
/// alırız: normal içerik penceresi varsa onu göster, yoksa arkaplandaki
/// hafif tepsi oturumunu (/settings) göster, o da yoksa (örn. Süper
/// Bildirimler kapalıyken her şey kapatılmıştı) sıfırdan yeni bir pencere aç.
pub fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        return;
    }
    if let Some((_, win)) = app.webview_windows().into_iter().next() {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        return;
    }
    if let Err(e) = crate::build_new_window(app, "https://openani.me/".to_string()) {
        crate::dbg_log!("[Tepsi] Sıfırdan pencere açılamadı: {}", e);
    }
}

pub fn ensure_tray(app: &AppHandle) -> Result<(), String> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    if app.tray_by_id(TRAY_ID).is_some() {
        return Ok(());
    }

    // Native menü YOK: sağ tık özel WPF menüsünü açar (native_tray_menu).
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("OpenAnime")
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let app = tray.app_handle();
                match button {
                    MouseButton::Left => show_main(app),   // sol tık → göster
                    MouseButton::Right => {
                        // Menüyü FARENİN değil, tepsi İKONUNUN kendi ekran
                        // dikdörtgenine göre konumlandıracağız — Tauri bunu
                        // event ile birlikte native olarak veriyor. Windows'ta
                        // ikon rect'i normalde fiziksel piksel gelir; Logical
                        // durumunu da (scale_factor=1 varsayarak) ele alıyoruz.
                        let (icon_x, icon_y) = match rect.position {
                            tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
                            tauri::Position::Logical(p) => (p.x, p.y),
                        };
                        let (icon_w, icon_h) = match rect.size {
                            tauri::Size::Physical(s) => (s.width as f64, s.height as f64),
                            tauri::Size::Logical(s) => (s.width, s.height),
                        };
                        open_tray_menu(app, (icon_x, icon_y, icon_w, icon_h));
                    }
                    _ => {}
                }
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
fn remove_tray(app: &AppHandle) {
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
    crate::log!(
        "[Süper Bildirim] {}",
        if enabled { "açıldı" } else { "kapatıldı" }
    );

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
    }
    Ok(())
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
            // URL yok: yalnızca pencereyi öne getir.
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.show();
                let _ = main.unminimize();
                let _ = main.set_focus();
            }
        }
    }
    Ok(())
}
