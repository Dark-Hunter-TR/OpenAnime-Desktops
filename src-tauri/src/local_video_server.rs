/// ────────────────────────────────────────────────────────────
/// 🎥 Local Video Server (Kopyasız Stream)
/// ────────────────────────────────────────────────────────────
/// Amaç: Yerel MP4 dosyasını IndexedDB'ye kopyalamadan, direkt
/// diskten HTTP stream olarak WebView'a aktarmak.
///
/// Nasıl çalışır:
///   1. Rust tarafında 127.0.0.1:{PORT}'da küçük bir HTTP server
///      başlatılır.
///   2. JS tarafında fetch interceptor, CDN video URL'sini
///      http://127.0.0.1:{PORT}/local-video?path=...'e yönlendirir.
///   3. Rust dosyayı diskten parça parça okur, Range byte
///      isteklerini destekler (seeking için).
///   4. WebGPU player stream'i alır ve oynatır.
///
/// Avantajları:
///   - MP4 olduğu yerde kalır (Download klasörü)
///   - Hiç kopya oluşmaz, IndexedDB kullanılmaz
///   - Büyük dosyalar (GB) sorunsuz çalışır
///   - Range byte → seeking desteği
/// ────────────────────────────────────────────────────────────

use regex::Regex;
use std::collections::HashSet;
use std::hash::{BuildHasher, Hasher};
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
use tiny_http::{Header, Response, Server, StatusCode};

/// Sunucunun servis edebileceği dosya uzantıları (küçük harf). Yerel video
/// seçicisinin kabul ettiği uzantılar + HLS parçaları. Bunların dışındaki
/// hiçbir dosya (belge, anahtar, kimlik bilgisi…) bu sunucudan okunamaz.
const MEDIA_EXTS: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "avi", "mov", "wmv", "flv", "3gp", "ogv", "ogg", "mpg",
    "mpeg", "ts", "m2ts", "mts", "m3u8", "m4s", "aac", "ac3",
];

/// Yol, izin verilen bir medya uzantısına mı sahip? (`read_file_head` da kullanır.)
pub fn has_media_extension(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| MEDIA_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Origin başlığı yalnızca openani.me (ve alt alanları) ise kabul edilir.
fn origin_allowed(origin: &str) -> bool {
    origin == "https://openani.me"
        || (origin.starts_with("https://") && origin.ends_with(".openani.me"))
}

static URI_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"URI="([^"]*)""#).unwrap());

/// Her açılışta üretilen rastgele erişim anahtarı (128 bit). OS'tan tohumlanan
/// `RandomState` ile türetilir; dışarıdan tahmin edilemez.
fn generate_token() -> String {
    let mut out = String::with_capacity(32);
    for i in 0..2u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(i);
        if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            h.write_u128(d.as_nanos());
        }
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

// ════════════════════════════════════════════════════════════
// STATE
// ════════════════════════════════════════════════════════════

/// Paylaşılan durum — port + videoId → dosya yolu eşlemesi
pub struct LocalVideoState {
    pub port: Mutex<u16>,
    /// Her istekte `t=` parametresiyle gelmek zorunda olan gizli anahtar.
    pub token: String,
    /// Sunucunun yeniden yazdığı HLS playlist'lerinin atıf yaptığı ek dosyalar
    /// (anahtar/init gibi uzantısı medya olmayanlar). Yalnızca playlist'ten gelir.
    extra_allowed: Mutex<HashSet<String>>,
}

impl LocalVideoState {
    pub fn new() -> Self {
        Self {
            port: Mutex::new(0),
            token: generate_token(),
            extra_allowed: Mutex::new(HashSet::new()),
        }
    }

    fn path_allowed(&self, path: &str) -> bool {
        has_media_extension(path)
            || self
                .extra_allowed
                .lock()
                .map(|set| set.contains(path))
                .unwrap_or(false)
    }

    fn allow_extra(&self, path: &str) {
        if let Ok(mut set) = self.extra_allowed.lock() {
            if set.len() > 50_000 {
                set.clear();
            }
            set.insert(path.to_string());
        }
    }
}

// ════════════════════════════════════════════════════════════
// SERVER
// ════════════════════════════════════════════════════════════

/// Server'ı 127.0.0.1:{random port}'da başlatır.
/// Port'u state'e kaydedip döndürür.
pub fn start_server(state: &Arc<LocalVideoState>) -> Result<u16, String> {
    let server = Server::http("127.0.0.1:0")
        .map_err(|e| format!("Local video server başlatılamadı: {}", e))?;

    let port = server.server_addr().to_ip().unwrap().port();
    // (println kaldırıldı — lib.rs zaten log! ile basıyor; çift satır oluyordu.)

    // Port'u state'e kaydet
    *state.port.lock().map_err(|e| e.to_string())? = port;

    // State'i thread'lere geçir
    let shared_state = state.clone();

    thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url().to_string();
            let headers = request.headers().iter()
                .map(|h| (h.field.to_string().to_lowercase(), String::from_utf8_lossy(h.value.as_bytes()).to_string()))
                .collect::<Vec<_>>();
            let st = shared_state.clone();

            thread::spawn(move || {
                let header_of = |name: &str| {
                    headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
                };
                let origin = header_of("origin");

                // Kötü niyetli bir sayfadan gelen istek: Origin varsa openani.me olmalı.
                if let Some(ref o) = origin {
                    if !origin_allowed(o) {
                        let _ = request.respond(Response::from_string("Forbidden").with_status_code(403));
                        return;
                    }
                }

                if url == "/" || url == "/ping" {
                    let resp = Response::from_string("OK")
                        .with_header(cors_header(origin.as_deref()));
                    let _ = request.respond(resp);
                    return;
                }

                if url.starts_with("/local-video") {
                    let query = url.split('?').nth(1).unwrap_or("");

                    // Erişim anahtarı yoksa/yanlışsa dosyaya hiç dokunma.
                    if extract_query_param(query, "t").as_deref() != Some(st.token.as_str()) {
                        let _ = request.respond(Response::from_string("Forbidden").with_status_code(403));
                        return;
                    }

                    let file_path = extract_query_param(query, "path");
                    let range_header = header_of("range");

                    if let Some(ref path) = file_path {
                        if !st.path_allowed(path) {
                            let _ = request.respond(
                                Response::from_string("Forbidden: desteklenmeyen dosya türü").with_status_code(403),
                            );
                            return;
                        }
                        if is_hls_playlist(path) {
                            serve_playlist(request, path, &st, origin.as_deref());
                        } else {
                            serve_file(request, path, range_header, origin.as_deref());
                        }
                    } else {
                        let resp = Response::from_string("HATA: 'path' parametresi gerekli")
                            .with_status_code(400);
                        let _ = request.respond(resp);
                    }
                } else {
                    let resp = Response::from_string("Not Found")
                        .with_status_code(404);
                    let _ = request.respond(resp);
                }
            });
        }
    });

    Ok(port)
}

// ════════════════════════════════════════════════════════════
// YARDIMCILAR
// ════════════════════════════════════════════════════════════

/// CORS yalnızca izin verilen Origin'e yansıtılır (`*` YOK). Origin yoksa
/// (medya öğesi doğrudan istiyorsa) tarayıcı CORS aramaz; yalnız Vary döner.
fn cors_header(origin: Option<&str>) -> Header {
    match origin {
        Some(o) if origin_allowed(o) => {
            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], o.as_bytes())
                .unwrap_or_else(|_| Header::from_bytes(&b"Vary"[..], &b"Origin"[..]).unwrap())
        }
        _ => Header::from_bytes(&b"Vary"[..], &b"Origin"[..]).unwrap(),
    }
}

fn extract_query_param(query: &str, name: &str) -> Option<String> {
    query.split('&')
        .find(|p| p.starts_with(&format!("{}=", name)))
        .and_then(|p| {
            let val = p.strip_prefix(&format!("{}=", name))?;
            percent_encoding::percent_decode_str(val)
                .decode_utf8()
                .ok()
                .map(|s| s.to_string())
        })
}

fn is_hls_playlist(path: &str) -> bool {
    path.to_lowercase().ends_with(".m3u8")
}

/// Konteyner/codec ne olursa olsun (H.264, HEVC/H265, AV1, VP9...) demux/decode
/// tarayıcının kendi işi — burada yalnızca doğru Content-Type ile servis
/// ediyoruz. HLS (.m3u8/.ts/.m4s) ayrı ele alınıyor, bkz. serve_playlist().
fn get_mime_type(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".mp4") || lower.ends_with(".m4v") { "video/mp4" }
    else if lower.ends_with(".mkv") { "video/x-matroska" }
    else if lower.ends_with(".webm") { "video/webm" }
    else if lower.ends_with(".avi") { "video/x-msvideo" }
    else if lower.ends_with(".mov") { "video/quicktime" }
    else if lower.ends_with(".wmv") { "video/x-ms-wmv" }
    else if lower.ends_with(".flv") { "video/x-flv" }
    else if lower.ends_with(".3gp") { "video/3gpp" }
    else if lower.ends_with(".ogv") || lower.ends_with(".ogg") { "video/ogg" }
    else if lower.ends_with(".mpg") || lower.ends_with(".mpeg") { "video/mpeg" }
    else if lower.ends_with(".m2ts") || lower.ends_with(".mts") { "video/mp2t" }
    else if lower.ends_with(".ts") { "video/mp2t" }
    else if lower.ends_with(".m3u8") { "application/vnd.apple.mpegurl" }
    else if lower.ends_with(".m4s") { "video/iso.segment" }
    else if lower.ends_with(".aac") { "audio/aac" }
    else if lower.ends_with(".ac3") { "audio/ac3" }
    else { "application/octet-stream" }
}

// ════════════════════════════════════════════════════════════
// DOSYA SERVİSİ
// ════════════════════════════════════════════════════════════

fn serve_file(
    request: tiny_http::Request,
    file_path: &str,
    range_header: Option<String>,
    origin: Option<&str>,
) {
    let mime = get_mime_type(file_path);
    let cors = cors_header(origin);

    // Dosya metadata
    let metadata = match std::fs::metadata(file_path) {
        Ok(m) => m,
        Err(e) => {
            let resp = Response::from_string(format!("Dosya bulunamadı: {} ({})", file_path, e))
                .with_status_code(404);
            let _ = request.respond(resp);
            return;
        }
    };

    let file_size = metadata.len();
    let file_size_str = file_size.to_string();

    // ── Range isteği var mı? (seeking) ──
    if let Some(ref range) = range_header {
        if let Some(range_val) = range.strip_prefix("bytes=") {
            // "a-b", "a-" ve "-n" (son n bayt) biçimleri; geçersiz/çakışan aralık → 416.
            let parsed: Option<(u64, u64)> = (|| {
                if file_size == 0 {
                    return None;
                }
                let (a, b) = range_val.split_once('-')?;
                let last = file_size - 1;
                let (start, end) = if a.is_empty() {
                    let n: u64 = b.parse().ok()?;
                    if n == 0 {
                        return None;
                    }
                    (file_size.saturating_sub(n), last)
                } else {
                    let start: u64 = a.parse().ok()?;
                    let end: u64 = if b.is_empty() { last } else { b.parse().ok()? };
                    (start, end.min(last))
                };
                if start > end || start >= file_size {
                    return None;
                }
                Some((start, end))
            })();

            let Some((start, end)) = parsed else {
                let resp = Response::from_string("Range Not Satisfiable")
                    .with_status_code(416)
                    .with_header(
                        Header::from_bytes(&b"Content-Range"[..], format!("bytes */{}", file_size).as_bytes()).unwrap(),
                    );
                let _ = request.respond(resp);
                return;
            };

            let content_length = end - start + 1;

            // Yeni file handle + seek
            let mut file = match std::fs::File::open(file_path) {
                Ok(f) => f,
                Err(e) => {
                    let resp = Response::from_string(format!("Dosya açılamadı: {}", e))
                        .with_status_code(500);
                    let _ = request.respond(resp);
                    return;
                }
            };

            if let Err(e) = file.seek(SeekFrom::Start(start)) {
                let resp = Response::from_string(format!("Seek hatası: {}", e))
                    .with_status_code(500);
                let _ = request.respond(resp);
                return;
            }

            let resp = Response::new(
                StatusCode(206),
                vec![
                    Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap(),
                    Header::from_bytes(&b"Content-Length"[..], content_length.to_string().as_bytes()).unwrap(),
                    Header::from_bytes(&b"Content-Range"[..], format!("bytes {}-{}/{}", start, end, file_size).as_bytes()).unwrap(),
                    Header::from_bytes(&b"Accept-Ranges"[..], &b"bytes"[..]).unwrap(),
                    Header::from_bytes(&b"Connection"[..], &b"keep-alive"[..]).unwrap(),
                    cors,
                ],
                Box::new(file) as Box<dyn Read + Send>,
                Some(content_length as usize),
                None, // trailing headers
            );
            let _ = request.respond(resp);
            return;
        }
    }

    // ── Tam dosya (range yok) ──
    let file = match std::fs::File::open(file_path) {
        Ok(f) => f,
        Err(e) => {
            let resp = Response::from_string(format!("Dosya açılamadı: {}", e))
                .with_status_code(500);
            let _ = request.respond(resp);
            return;
        }
    };

    let resp = Response::new(
        StatusCode(200),
        vec![
            Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap(),
            Header::from_bytes(&b"Content-Length"[..], file_size_str.as_bytes()).unwrap(),
            Header::from_bytes(&b"Accept-Ranges"[..], &b"bytes"[..]).unwrap(),
            Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap(),
            cors,
        ],
        Box::new(file) as Box<dyn Read + Send>,
        Some(file_size as usize),
        None, // trailing headers
    );
    let _ = request.respond(resp);
}

// ════════════════════════════════════════════════════════════
// HLS (.m3u8) SERVİSİ — "sahte dosya" yaklaşımı
// ════════════════════════════════════════════════════════════
// <video src> hiçbir tarayıcıda (WebView2 dahil) .m3u8'i doğrudan
// çözemiyor — site zaten kendi bundle'ında hls.js taşıyor
// (bkz. local-player.js: window.Hls ile attachMedia). Bizim işimiz
// yalnızca playlist içindeki GÖRECELİ yolları (segment .ts/.m4s,
// alt varyant .m3u8, #EXT-X-KEY / #EXT-X-MAP URI'leri) diskteki
// gerçek konuma göre çözüp yine /local-video?path=... sarmalayıcısına
// çevirmek. Böylece hls.js her parçayı da bizim server'dan ister ve
// Range/CORS zaten var olan serve_file() yolundan geçer.
fn serve_playlist(
    request: tiny_http::Request,
    file_path: &str,
    state: &LocalVideoState,
    origin: Option<&str>,
) {
    let cors = cors_header(origin);

    let raw = match std::fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            let resp = Response::from_string(format!("Dosya bulunamadı: {} ({})", file_path, e))
                .with_status_code(404);
            let _ = request.respond(resp);
            return;
        }
    };

    let base_dir = Path::new(file_path).parent().unwrap_or_else(|| Path::new(""));
    let rewritten = rewrite_playlist(&raw, base_dir, state);
    let bytes = rewritten.into_bytes();
    let len = bytes.len();

    let resp = Response::new(
        StatusCode(200),
        vec![
            Header::from_bytes(&b"Content-Type"[..], get_mime_type(file_path).as_bytes()).unwrap(),
            Header::from_bytes(&b"Content-Length"[..], len.to_string().as_bytes()).unwrap(),
            Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap(),
            cors,
        ],
        Cursor::new(bytes),
        Some(len),
        None,
    );
    let _ = request.respond(resp);
}

/// Playlist'i satır satır gezer. `#` ile başlamayan her satır bir URI'dir
/// (segment ya da alt-varyant .m3u8 — master/media playlist farkı gözetmeye
/// gerek yok, ikisi de aynı kuralla çözülür). `#EXT-X-KEY` / `#EXT-X-MAP`
/// gibi etiket satırlarındaki `URI="..."` alanı da ayrıca çözülür.
fn rewrite_playlist(content: &str, base_dir: &Path, state: &LocalVideoState) -> String {
    let mut out = String::with_capacity(content.len() + 256);

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }
        if trimmed.starts_with('#') {
            if trimmed.contains("URI=\"") {
                let replaced = URI_RE.replace(line, |caps: &regex::Captures| {
                    format!("URI=\"{}\"", resolve_uri(base_dir, &caps[1], state))
                });
                out.push_str(&replaced);
            } else {
                out.push_str(line);
            }
        } else {
            out.push_str(&resolve_uri(base_dir, trimmed, state));
        }
        out.push('\n');
    }
    out
}

/// Playlist içindeki bir URI değerini (segment/alt-playlist/anahtar/init) diskteki
/// gerçek dosyaya göre çözüp `/local-video?path=...` sarmalayıcısına çevirir.
/// Zaten mutlak bir HTTP(S) URL ise ya da zaten bizim sarmalayıcımızsa dokunmaz.
fn resolve_uri(base_dir: &Path, value: &str, state: &LocalVideoState) -> String {
    if value.starts_with("http://") || value.starts_with("https://") || value.starts_with("/local-video") {
        return value.to_string();
    }
    let joined = base_dir.join(value);
    let abs = joined.to_string_lossy().to_string();
    // Playlist'in atıf yaptığı dosya (anahtar/init dahil) bu oturumda servis
    // edilebilir — yalnızca playlist klasörünün İÇİNDEKİ göreli yollar için
    // (mutlak yol ya da `..` ile dışarı çıkan atıf istisna açmaz).
    let rel = Path::new(value);
    let stays_inside = !rel.is_absolute()
        && !rel.components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::Prefix(_)));
    if stays_inside {
        state.allow_extra(&abs);
    }
    let encoded = percent_encoding::utf8_percent_encode(&abs, percent_encoding::NON_ALPHANUMERIC).to_string();
    format!("/local-video?path={}&t={}", encoded, state.token)
}
