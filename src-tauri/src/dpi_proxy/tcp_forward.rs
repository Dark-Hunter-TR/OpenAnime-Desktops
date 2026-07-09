// === OpenAnime — HTTP/HTTPS Proxy (CONNECT destekli) ===
// GoodbyeDPI fragmentasyon mantığının Rust portu
// WebView2 --proxy-server ile kullanılmak üzere tasarlanmıştır

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::time::Duration;

use super::methods::DpiMethod;
use super::tls_detect;

const PROXY_ADDR: &str = "127.0.0.1:1453";
const FRAGMENT_DELAY: Duration = Duration::from_millis(2);

/// Proxy sunucusunu başlat — arkaplanda çalışır
pub async fn start_proxy_internal(method: DpiMethod, running: Arc<Mutex<bool>>) {
    let listener = match TcpListener::bind(PROXY_ADDR).await {
        Ok(l) => {
            println!("[DPI Proxy] HTTP proxy başlatıldı: {} (yöntem: {})", PROXY_ADDR, method.name);
            l
        }
        Err(e) => {
            eprintln!("[DPI Proxy] Proxy başlatılamadı: {}", e);
            return;
        }
    };

    loop {
        if !*running.lock().await {
            println!("[DPI Proxy] Proxy durduruluyor...");
            break;
        }

        let accept = tokio::time::timeout(Duration::from_secs(1), listener.accept()).await;
        match accept {
            Ok(Ok((client, addr))) => {
                println!("[DPI Proxy] Yeni bağlantı: {}", addr);
                let method = method.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_http_proxy(client, method).await {
                        eprintln!("[DPI Proxy] Bağlantı hatası ({}): {}", addr, e);
                    }
                });
            }
            Ok(Err(e)) => {
                eprintln!("[DPI Proxy] Accept hatası: {}", e);
            }
            Err(_) => continue,
        }
    }

    println!("[DPI Proxy] Proxy sonlandı.");
}

/// HTTP Proxy girişi — CONNECT veya direkt HTTP isteklerini yönetir
async fn handle_http_proxy(mut client: TcpStream, method: DpiMethod) -> Result<(), String> {
    let mut buf = vec![0u8; 4096];
    let n = client.read(&mut buf).await
        .map_err(|e| format!("İstek okuma hatası: {}", e))?;
    if n == 0 {
        return Err("Bağlantı kapandı".to_string());
    }

    let line_end = buf[..n].iter().position(|&b| b == b'\n')
        .ok_or_else(|| "Geçersiz HTTP isteği: satır sonu yok".to_string())?;
    let request_line = std::str::from_utf8(&buf[..line_end])
        .map_err(|e| format!("Geçersiz UTF-8: {}", e))?;
    let request_line = request_line.trim_end_matches('\r').trim_end_matches('\n');

    if request_line.starts_with("CONNECT ") {
        handle_connect(client, &buf[..n], method).await
    } else if request_line.starts_with("GET ") || request_line.starts_with("POST ") || 
              request_line.starts_with("PUT ") || request_line.starts_with("DELETE ") ||
              request_line.starts_with("HEAD ") || request_line.starts_with("OPTIONS ") ||
              request_line.starts_with("PATCH ") {
        handle_http_request(client, &buf[..n], method).await
    } else {
        Err(format!("Bilinmeyen proxy isteği: {}", request_line))
    }
}

/// CONNECT handler — HTTPS tünellemesi
async fn handle_connect(
    mut client: TcpStream,
    first_data: &[u8],
    method: DpiMethod,
) -> Result<(), String> {
    // İlk satır: CONNECT openani.me:443 HTTP/1.1
    let line_end = first_data.iter().position(|&b| b == b'\n')
        .ok_or_else(|| "Geçersiz CONNECT".to_string())?;
    let request_line = std::str::from_utf8(&first_data[..line_end])
        .map_err(|e| format!("Geçersiz UTF-8: {}", e))?;
    let parts: Vec<&str> = request_line.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return Err("Geçersiz CONNECT isteği".to_string());
    }
    let target = parts[1];

    // canvas.openani.me → Cloudflare Turnstile (bot koruması)
    // Bu domain'e bağlantılar WebView tarafından sık sık iptal edilir (10053).
    // Cloudflare canvas fingerprinting script'i bağlantıyı hızlıca açıp kapatır,
    // bu proxy kaynaklı bir hata DEĞİLDİR, normal davranıştır.
    let is_canvas = target.contains("canvas.openani.me");

    println!("[DPI Proxy] CONNECT {} (yöntem: {})", target, method.name);

    // Hedefe bağlan
    let mut server = match TcpStream::connect(target).await {
        Ok(s) => s,
        Err(e) => {
            // canvas domainleri için bağlantı hatalarını sessizce geç
            if is_canvas {
                return Ok(());
            }
            return Err(format!("Hedefe bağlanılamadı ({}): {}", target, e));
        }
    };

    let _ = server.set_nodelay(true);
    let _ = client.set_nodelay(true);

    // Proxy'den 200 Connection Established cevabı gönder
    let response = "HTTP/1.1 200 Connection Established\r\n\r\n";
    client.write_all(response.as_bytes())
        .await
        .map_err(|e| format!("200 CEVABI GÖNDERİLEMEDİ: {}", e))?;

    // flush
    client.flush().await.map_err(|e| e.to_string())?;

    // TLS tünellemesi — ClientHello fragmentasyonu
    handle_tls_tunnel(&mut client, &mut server, &method).await?;

    // Kalan veriyi çift yönlü kopyala
    bidirectional_copy(client, server).await;
    Ok(())
}

/// HTTP istekleri için direkt proxy
async fn handle_http_request(
    client: TcpStream,
    first_data: &[u8],
    method: DpiMethod,
) -> Result<(), String> {
    // URL'den host'u çıkar
    let data_str = std::str::from_utf8(first_data).map_err(|e| e.to_string())?;
    let line_end = data_str.find('\n').unwrap_or(data_str.len());
    let request_line = data_str[..line_end].trim_end_matches('\r').trim_end_matches('\n');
    let parts: Vec<&str> = request_line.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return Err("Geçersiz HTTP isteği".to_string());
    }
    let url_str = parts[1];

    let host = if url_str.starts_with("http://") || url_str.starts_with("https://") {
        let without_scheme = url_str.trim_start_matches("http://").trim_start_matches("https://");
        let path_idx = without_scheme.find('/').unwrap_or(without_scheme.len());
        &without_scheme[..path_idx]
    } else {
        url_str
    };

    let target = if host.contains(':') {
        host.to_string()
    } else {
        format!("{}:80", host)
    };

    println!("[DPI Proxy] HTTP -> {} (hedef: {})", url_str, target);

    // Hedefe bağlan
    let mut server = TcpStream::connect(&target)
        .await
        .map_err(|e| format!("HTTP hedefe bağlanılamadı ({}): {}", target, e))?;

    let _ = server.set_nodelay(true);
    let _ = client.set_nodelay(true);

    // HTTP verisine manipülasyon + fragmentasyon uygula
    let mut data = first_data.to_vec();

    // Header manipülasyonu
    if method.http_host_removespace || method.http_host_mixedcase || method.http_host_case {
        if method.http_host_removespace {
            let _ = super::http_mod::remove_host_space(&mut data);
        }
        if method.http_host_mixedcase {
            let _ = super::http_mod::mix_host_case(&mut data);
        }
        if method.http_host_case {
            let _ = super::http_mod::replace_host_with_host(&mut data);
        }
    }

    // Fragmentasyon
    let frag_size = method.http_fragment_size as usize;
    if frag_size > 0 && frag_size < data.len() {
        if method.reverse_fragment {
            server.write_all(&data[frag_size..]).await.map_err(|e| e.to_string())?;
            tokio::time::sleep(FRAGMENT_DELAY).await;
            server.write_all(&data[..frag_size]).await.map_err(|e| e.to_string())?;
        } else {
            server.write_all(&data[..frag_size]).await.map_err(|e| e.to_string())?;
            tokio::time::sleep(FRAGMENT_DELAY).await;
            server.write_all(&data[frag_size..]).await.map_err(|e| e.to_string())?;
        }
    } else {
        server.write_all(&data).await.map_err(|e| e.to_string())?;
    }

    // Kalan veriyi çift yönlü kopyala
    bidirectional_copy(client, server).await;
    Ok(())
}

/// TLS tünellemesi — ClientHello fragmentasyonu
async fn handle_tls_tunnel(
    client: &mut TcpStream,
    server: &mut TcpStream,
    method: &DpiMethod,
) -> Result<(), String> {
    let mut buf = vec![0u8; 4096];
    let n = match client.read(&mut buf).await {
        Ok(n) => n,
        Err(_) => {
            // Client bağlantıyı kapattıysa (Cloudflare Turnstile vb.) sessizce dön
            return Ok(());
        }
    };

    if n == 0 {
        return Ok(());
    }

    let frag_size = method.https_fragment_size as usize;

    if frag_size > 0 && frag_size < n {
        if method.fragment_by_sni {
            if let Some(sni) = tls_detect::extract_sni(&buf[..n]) {
                let sni_offset = unsafe { sni.as_ptr().offset_from(buf.as_ptr()) } as usize;
                if sni_offset > 0 && sni_offset < n {
                    let _ = server.write_all(&buf[..sni_offset]).await;
                    tokio::time::sleep(FRAGMENT_DELAY).await;
                    let _ = server.write_all(&buf[sni_offset..n]).await;
                    return Ok(());
                }
            }
        }

        if method.reverse_fragment {
            let _ = server.write_all(&buf[frag_size..n]).await;
            tokio::time::sleep(FRAGMENT_DELAY).await;
            let _ = server.write_all(&buf[..frag_size]).await;
        } else {
            let _ = server.write_all(&buf[..frag_size]).await;
            tokio::time::sleep(FRAGMENT_DELAY).await;
            let _ = server.write_all(&buf[frag_size..n]).await;
        }
    } else {
        let _ = server.write_all(&buf[..n]).await;
    }

    Ok(())
}

/// Çift yönlü TCP kopyalama
async fn bidirectional_copy(mut client: TcpStream, mut server: TcpStream) {
    let (mut cr, mut cw) = client.split();
    let (mut sr, mut sw) = server.split();

    tokio::select! {
        _ = tokio::io::copy(&mut cr, &mut sw) => {},
        _ = tokio::io::copy(&mut sr, &mut cw) => {},
    }
}
