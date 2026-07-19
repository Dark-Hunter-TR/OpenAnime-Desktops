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

use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};
use std::thread;
use tiny_http::{Header, Response, Server, StatusCode};

// ════════════════════════════════════════════════════════════
// STATE
// ════════════════════════════════════════════════════════════

/// Paylaşılan durum — port + videoId → dosya yolu eşlemesi
pub struct LocalVideoState {
    pub port: Mutex<u16>,
}

impl LocalVideoState {
    pub fn new() -> Self {
        Self {
            port: Mutex::new(0),
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

    // Thread pool — 4 thread ile eşzamanlı istekleri işle
    thread::spawn(move || {
        let _state = shared_state;
        for request in server.incoming_requests() {
            let url = request.url().to_string();
            let headers = request.headers().iter()
                .map(|h| (h.field.to_string().to_lowercase(), String::from_utf8_lossy(h.value.as_bytes()).to_string()))
                .collect::<Vec<_>>();

            thread::spawn(move || {
                if url == "/" || url == "/ping" {
                    let resp = Response::from_string("OK")
                        .with_header(cors_header());
                    let _ = request.respond(resp);
                    return;
                }

                if url.starts_with("/local-video") {
                    let query = url.split('?').nth(1).unwrap_or("");
                    let file_path = extract_query_param(query, "path");
                    let range_header = headers.iter()
                        .find(|(k, _)| k == "range")
                        .map(|(_, v)| v.clone());

                    if let Some(ref path) = file_path {
                        serve_file(request, path, range_header);
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

fn cors_header() -> Header {
    Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap()
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

fn get_mime_type(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".mp4") { "video/mp4" }
    else if lower.ends_with(".mkv") { "video/x-matroska" }
    else if lower.ends_with(".webm") { "video/webm" }
    else { "application/octet-stream" }
}

// ════════════════════════════════════════════════════════════
// DOSYA SERVİSİ
// ════════════════════════════════════════════════════════════

fn serve_file(
    request: tiny_http::Request,
    file_path: &str,
    range_header: Option<String>,
) {
    let mime = get_mime_type(file_path);
    let cors = cors_header();

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
            let parts: Vec<&str> = range_val.split('-').collect();
            let start: u64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            let end: u64 = if parts.len() > 1 && !parts[1].is_empty() {
                parts[1].parse().unwrap_or(file_size - 1)
            } else {
                file_size - 1
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
