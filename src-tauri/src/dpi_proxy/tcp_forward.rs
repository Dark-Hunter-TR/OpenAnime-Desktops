// === OpenAnime — HTTP/HTTPS Proxy (CONNECT destekli) ===
// GoodbyeDPI fragmentasyon mantığının Rust portu
// WebView2 --proxy-server ile kullanılmak üzere tasarlanmıştır

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::time::Duration;

use super::bypass_detect;
use super::methods::DpiMethod;
use super::tls_detect;
use crate::{dbg_log, log};

const PROXY_ADDR: &str = "127.0.0.1:1453";
const FRAGMENT_DELAY: Duration = Duration::from_millis(2);

/// Proxy sunucusunu başlat — arkaplanda çalışır
pub async fn start_proxy_internal(
    current_method: Arc<Mutex<Option<DpiMethod>>>,
    running: Arc<Mutex<bool>>,
) {
    dbg_log!("[DPI Proxy] === TCP Proxy Başlatılıyor ===");
    dbg_log!("[DPI Proxy] Adres: {}", PROXY_ADDR);

    let listener = match TcpListener::bind(PROXY_ADDR).await {
        Ok(l) => {
            dbg_log!("[DPI Proxy] HTTP proxy başlatıldı: {}", PROXY_ADDR);
            l
        }
        Err(e) => {
            log!("[Bağlantı] Hızlandırıcı başlatılamadı (port meşgul olabilir). Site açılmazsa uygulamayı yeniden başlatın.");
            dbg_log!("[DPI Proxy] bind hatası ({}): {}", PROXY_ADDR, e);
            return;
        }
    };

    loop {
        if !*running.lock().await {
            dbg_log!("[DPI Proxy] Proxy durduruluyor...");
            break;
        }

        let accept = tokio::time::timeout(Duration::from_secs(1), listener.accept()).await;
        match accept {
            Ok(Ok((client, addr))) => {
                let current_method = current_method.clone();
                tokio::spawn(async move {
                    let method = {
                        let guard = current_method.lock().await;
                        guard.clone().unwrap_or_else(|| {
                            super::methods::get_method_by_id(0).unwrap().clone()
                        })
                    };

                    if let Err(e) = handle_http_proxy(client, method).await {
                        // Avoid spamming canvas connection reset errors which are expected
                        let is_canvas = e.contains("canvas");
                        if !is_canvas {
                            dbg_log!("[DPI Proxy] Bağlantı hatası ({}): {}", addr, e);
                        }
                    }
                });
            }
            Ok(Err(e)) => {
                dbg_log!("[DPI Proxy] Accept hatası: {}", e);
            }
            Err(_) => continue,
        }
    }

    dbg_log!("[DPI Proxy] Proxy sonlandı.");
}

/// Hedef bizim kendi altyapımız mı (openani.me ve alt alan adları)?
///
/// NEDEN ÖNEMLİ: HTTP header oyunları (Host: → hoSt:, mixed case, boşluk
/// kaydırma) ARADAKİ DPI kutusunu şaşırtmak içindir; hedef sunucunun kendisi
/// bunları görür ve normalden sapmış bir istek olarak değerlendirir.
/// openani.me önünde Cloudflare bot yönetimi + OpenAnime Vanguard var; kendi
/// isteklerimizi bu katmanlara "acayip" göstermenin hiçbir faydası, gözden
/// düşme riski ise var. Paket seviyesindeki fragmentasyon ise sunucuya
/// TAMAMEN görünmezdir (TCP tekrar birleştirilir) — o yüzden fragmentasyon
/// kendi alan adlarımızda da uygulanmaya devam eder, yalnızca header
/// manipülasyonu kapatılır.
fn is_own_domain(host: &str) -> bool {
    let h = host.split(':').next().unwrap_or(host);
    h.eq_ignore_ascii_case("openani.me") || h.to_ascii_lowercase().ends_with(".openani.me")
}

/// DNS engellemelerini aşmak için hedef adresi Cloudflare DoH ile çözer
async fn resolve_target_doh(target: &str) -> String {
    // Cloudflare WARP aktifken DNS'i EZMİYORUZ: WARP kendi çözümleyicisini ve
    // yönlendirmesini kuruyor; üstüne DoH ile bulduğumuz IP'yi dayatmak WARP'ın
    // tüneliyle çakışıp bağlantıyı bozabiliyor (bkz. bypass_detect).
    if !bypass_detect::current_behavior().allows_doh_override() {
        dbg_log!("[DPI Proxy] WARP aktif — DoH DNS ezmesi atlandı: {}", target);
        return target.to_string();
    }

    let host = target.split(':').next().unwrap_or(target);
    if host == "openani.me" || host.ends_with(".openani.me") {
        if let Some(ip) = super::remote_proxy::resolve_dns_doh(host).await {
            let port = target.split(':').nth(1).unwrap_or("443");
            let new_target = format!("{}:{}", ip, port);
            dbg_log!("[DPI Proxy] DNS Bypass (DoH): {} -> {}", target, new_target);
            return new_target;
        }
    }
    target.to_string()
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

    dbg_log!(
        "[DPI Proxy] Gelen istek: {} ({} bayt)",
        request_line,
        n
    );

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

    dbg_log!("[DPI Proxy] CONNECT {} (yöntem: #{}, {})", target, method.id, method.name);

    let connect_target = resolve_target_doh(target).await;

    // Hedefe bağlan
    let mut server = match TcpStream::connect(&connect_target).await {
        Ok(s) => {
            dbg_log!("[DPI Proxy]   Hedefe bağlanıldı: {}", connect_target);
            s
        }
        Err(e) => {
            // canvas domainleri için bağlantı hatalarını sessizce geç
            if is_canvas {
                dbg_log!("[DPI Proxy]   Canvas domain bağlantı hatası (beklenen): {} - {}", target, e);
                return Ok(());
            }
            dbg_log!("[DPI Proxy]   Hedefe bağlanılamadı ({}): {}", connect_target, e);
            return Err(format!("Hedefe bağlanılamadı ({}): {}", connect_target, e));
        }
    };

    let _ = server.set_nodelay(true);
    let _ = client.set_nodelay(true);

    // Proxy'den 200 Connection Established cevabı gönder
    let response = "HTTP/1.1 200 Connection Established\r\n\r\n";
    dbg_log!("[DPI Proxy]   200 Connection Established gönderiliyor...");
    client.write_all(response.as_bytes())
        .await
        .map_err(|e| format!("200 CEVABI GÖNDERİLEMEDİ: {}", e))?;

    // flush
    client.flush().await.map_err(|e| e.to_string())?;
    dbg_log!("[DPI Proxy]   200 CEVABI gönderildi, TLS tünellemesi başlıyor...");

    // TLS tünellemesi — ClientHello fragmentasyonu
    handle_tls_tunnel(&mut client, &mut server, &method).await?;

    // Kalan veriyi çift yönlü kopyala
    dbg_log!("[DPI Proxy]   Çift yönlü kopyalama başlatılıyor: {}", target);
    bidirectional_copy(client, server).await;
    dbg_log!("[DPI Proxy]   Bağlantı kapandı: {}", target);
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

    dbg_log!("[DPI Proxy] HTTP {} -> {} (hedef: {})", parts[0], url_str, target);

    let connect_target = resolve_target_doh(&target).await;

    // Hedefe bağlan
    let mut server = match TcpStream::connect(&connect_target).await {
        Ok(s) => {
            dbg_log!("[DPI Proxy]   HTTP hedefe bağlanıldı: {}", connect_target);
            s
        }
        Err(e) => {
            dbg_log!("[DPI Proxy]   HTTP hedefe bağlanılamadı ({}): {}", connect_target, e);
            return Err(format!("HTTP hedefe bağlanılamadı ({}): {}", connect_target, e));
        }
    };

    let _ = server.set_nodelay(true);
    let _ = client.set_nodelay(true);

    // HTTP verisine manipülasyon + fragmentasyon uygula
    let mut data = first_data.to_vec();
    dbg_log!("[DPI Proxy]   HTTP veri boyutu: {} bayt, fragment: {}", data.len(), method.http_fragment_size);

    // Harici araç aktifse hiçbir manipülasyon yapma — düz ilet (bkz. bypass_detect).
    // Header case/space oyunları da DPI manipülasyonudur; harici araç zaten
    // paket seviyesinde müdahale ediyorsa üst üste binmemeleri gerekir.
    let behavior = bypass_detect::current_behavior();
    if !behavior.allows_fragmentation() {
        dbg_log!(
            "[DPI Proxy]   Harici bypass aracı aktif ({:?}) — HTTP manipülasyonu/fragmentasyonu atlandı",
            behavior
        );
        server.write_all(&data).await.map_err(|e| e.to_string())?;
        bidirectional_copy(client, server).await;
        return Ok(());
    }

    // Header manipülasyonu — kendi alan adlarımızda ASLA (bkz. is_own_domain).
    if is_own_domain(host) {
        if method.http_host_removespace || method.http_host_mixedcase || method.http_host_case {
            dbg_log!(
                "[DPI Proxy]   {} bizim alan adımız — HTTP header manipülasyonu atlandı (fragmentasyon sürüyor)",
                host
            );
        }
    } else if method.http_host_removespace || method.http_host_mixedcase || method.http_host_case {
        let mut manipulations: Vec<&str> = Vec::new();
        if method.http_host_removespace {
            let _ = super::http_mod::remove_host_space(&mut data);
            manipulations.push("remove_space");
        }
        if method.http_host_mixedcase {
            let _ = super::http_mod::mix_host_case(&mut data);
            manipulations.push("mixed_case");
        }
        if method.http_host_case {
            let _ = super::http_mod::replace_host_with_host(&mut data);
            manipulations.push("host_case");
        }
        dbg_log!("[DPI Proxy]   Header manipülasyonları uygulandı: {:?}", manipulations);
    }

    // Fragmentasyon
    let frag_size = method.http_fragment_size as usize;
    if frag_size > 0 && frag_size < data.len() {
        dbg_log!(
            "[DPI Proxy]   Fragmentasyon uygulanıyor: {} bayt (reverse: {})",
            frag_size,
            method.reverse_fragment
        );
        if method.reverse_fragment {
            server.write_all(&data[frag_size..]).await.map_err(|e| e.to_string())?;
            tokio::time::sleep(FRAGMENT_DELAY).await;
            server.write_all(&data[..frag_size]).await.map_err(|e| e.to_string())?;
        } else {
            server.write_all(&data[..frag_size]).await.map_err(|e| e.to_string())?;
            tokio::time::sleep(FRAGMENT_DELAY).await;
            server.write_all(&data[frag_size..]).await.map_err(|e| e.to_string())?;
        }
        dbg_log!("[DPI Proxy]   Fragmentasyon tamamlandı");
    } else {
        dbg_log!("[DPI Proxy]   Fragmentasyon yok (frag_size={}, data.len={})", frag_size, data.len());
        server.write_all(&data).await.map_err(|e| e.to_string())?;
    }

    // Kalan veriyi çift yönlü kopyala
    dbg_log!("[DPI Proxy]   HTTP çift yönlü kopyalama başlatılıyor...");
    bidirectional_copy(client, server).await;
    dbg_log!("[DPI Proxy]   HTTP bağlantı kapandı: {}", target);
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
        Ok(n) => {
            dbg_log!("[DPI Proxy]   TLS ClientHello okundu: {} bayt", n);
            n
        }
        Err(e) => {
            // Client bağlantıyı kapattıysa (Cloudflare Turnstile vb.) sessizce dön
            dbg_log!("[DPI Proxy]   TLS ClientHello okuma hatası: {} (muhtemelen canvas/cloudflare)", e);
            return Ok(());
        }
    };

    if n == 0 {
        dbg_log!("[DPI Proxy]   TLS ClientHello boş (bağlantı kapandı)");
        return Ok(());
    }

    // Harici bir DPI bypass aracı (GoodbyeDPI/Zapret/ByeDPI) veya WARP aktifse
    // ClientHello'ya DOKUNMA. İki katman üst üste parçalarsa handshake bozulup
    // bağlantı düşebiliyor — bu durumda tek yaptığımız düz tünelleme olmalı.
    let behavior = bypass_detect::current_behavior();
    if !behavior.allows_fragmentation() {
        dbg_log!(
            "[DPI Proxy]   Harici bypass aracı aktif ({:?}) — TLS fragmentasyonu atlandı, {} bayt düz iletiliyor",
            behavior,
            n
        );
        let _ = server.write_all(&buf[..n]).await;
        return Ok(());
    }

    let frag_size = method.https_fragment_size as usize;

    if frag_size > 0 && frag_size < n {
        if method.fragment_by_sni {
            if let Some(sni) = tls_detect::extract_sni(&buf[..n]) {
                let sni_offset = unsafe { sni.as_ptr().offset_from(buf.as_ptr()) } as usize;
                dbg_log!(
                    "[DPI Proxy]   SNI fragmentasyon: offset={}, sni={:?}",
                    sni_offset,
                    std::str::from_utf8(sni).unwrap_or("(invalid utf8)")
                );
                if sni_offset > 0 && sni_offset < n {
                    dbg_log!("[DPI Proxy]   SNI fragmentasyon uygulanıyor: {} bayt -> bekle -> {} bayt", sni_offset, n - sni_offset);
                    let _ = server.write_all(&buf[..sni_offset]).await;
                    tokio::time::sleep(FRAGMENT_DELAY).await;
                    let _ = server.write_all(&buf[sni_offset..n]).await;
                    dbg_log!("[DPI Proxy]   SNI fragmentasyon tamamlandı");
                    return Ok(());
                }
            } else {
                dbg_log!("[DPI Proxy]   SNI çıkarılamadı, normal fragmentasyon deneniyor");
            }
        }

        dbg_log!(
            "[DPI Proxy]   HTTPS fragmentasyon: {} bayt (reverse: {}, toplam: {} bayt)",
            frag_size,
            method.reverse_fragment,
            n
        );
        if method.reverse_fragment {
            let _ = server.write_all(&buf[frag_size..n]).await;
            tokio::time::sleep(FRAGMENT_DELAY).await;
            let _ = server.write_all(&buf[..frag_size]).await;
        } else {
            let _ = server.write_all(&buf[..frag_size]).await;
            tokio::time::sleep(FRAGMENT_DELAY).await;
            let _ = server.write_all(&buf[frag_size..n]).await;
        }
        dbg_log!("[DPI Proxy]   HTTPS fragmentasyon tamamlandı");
    } else {
        dbg_log!(
            "[DPI Proxy]   HTTPS fragmentasyon yok (frag_size={}, n={}), direkt iletiliyor",
            frag_size, n
        );
        let _ = server.write_all(&buf[..n]).await;
    }

    Ok(())
}

/// Tünel boyunca kullanılan kopyalama tamponu.
///
/// 8 KiB, kapak görselleri/video segmentleri gibi büyük gövdelerde bayt başına
/// gereksiz syscall üretiyordu. 64 KiB tipik TCP pencere boyutuyla uyumlu.
const COPY_BUF_SIZE: usize = 64 * 1024;

/// Tünelin İKİ YÖNÜ birden bu süre boyunca sessiz kalırsa bağlantı kapatılır.
///
/// ÖNCEDEN 30 sn'ydi ve YÖN BAŞINA uygulanıyordu — bu, sağlıklı bir keep-alive
/// bağlantısını öldürüyordu: tarayıcı isteğini gönderip yanıtı aldıktan sonra
/// her iki yön de doğal olarak susar, 30 sn sonra tünel koparılırdı. Kullanıcı
/// biraz bekleyip kaydırdığında Chromium'un yeniden bağlanması gerekiyordu —
/// yani TCP + CONNECT + TLS el sıkışması + fragmentasyon gecikmesi baştan.
/// Kapak görsellerinin geç gelmesinin başlıca sebebi buydu.
const TUNNEL_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Watchdog'un boşta kalma kontrolünü ne sıklıkla yaptığı.
const IDLE_CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// Tek yönü kopyalar. Zaman aşımı YOK — boşta kalma denetimi çağıran taraftaki
/// ortak watchdog'a aittir (bkz. `bidirectional_copy`).
///
/// EOF'ta karşı tarafın YAZMA yarısını kapatır (half-close). Bağlantının
/// tamamını düşürmek yerine yarı kapatmak, TCP tünelinin doğru davranışıdır:
/// bir yön bittiğinde diğer yön akmaya devam edebilir.
async fn copy_half(
    mut reader: impl tokio::io::AsyncRead + Unpin,
    mut writer: impl tokio::io::AsyncWrite + Unpin,
    last_activity_ms: &std::sync::atomic::AtomicU64,
    started: std::time::Instant,
) -> Result<(), std::io::Error> {
    use std::sync::atomic::Ordering;

    let mut buf = vec![0u8; COPY_BUF_SIZE];
    loop {
        let n = reader.read(&mut buf).await?;
        // Her iki yön de aynı sayacı günceller; watchdog yalnızca İKİSİ birden
        // sustuğunda devreye girer.
        last_activity_ms.store(started.elapsed().as_millis() as u64, Ordering::Relaxed);
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n]).await?;
        last_activity_ms.store(started.elapsed().as_millis() as u64, Ordering::Relaxed);
    }
    // Yarı kapatma — karşı yön akmaya devam etsin.
    let _ = writer.shutdown().await;
    Ok(())
}

/// Çift yönlü TCP kopyalama.
///
/// `select!` YERİNE `join!`: eskiden hangi yön önce biterse tünelin TAMAMI
/// düşürülüyordu (her iki TcpStream de fonksiyondan çıkarken drop ediliyordu).
/// HTTP/2'de tek bağlantı üzerinde çoklanan tüm görsel istekleri bu yüzden
/// birlikte iptal oluyordu. Artık iki yön de kendi doğal sonuna kadar çalışır;
/// tünel yalnızca ikisi de bittiğinde veya ortak watchdog tetiklendiğinde kapanır.
async fn bidirectional_copy(mut client: TcpStream, mut server: TcpStream) {
    use std::sync::atomic::{AtomicU64, Ordering};

    let started = std::time::Instant::now();
    let last_activity_ms = AtomicU64::new(0);

    let (mut cr, mut cw) = client.split();
    let (mut sr, mut sw) = server.split();

    let pump = async {
        let _ = tokio::join!(
            copy_half(&mut cr, &mut sw, &last_activity_ms, started),
            copy_half(&mut sr, &mut cw, &last_activity_ms, started),
        );
    };

    let watchdog = async {
        loop {
            tokio::time::sleep(IDLE_CHECK_INTERVAL).await;
            let idle_ms = started
                .elapsed()
                .as_millis()
                .saturating_sub(last_activity_ms.load(Ordering::Relaxed) as u128);
            if idle_ms >= TUNNEL_IDLE_TIMEOUT.as_millis() {
                dbg_log!(
                    "[DPI Proxy]   Tünel {} sn boyunca çift yönlü sessiz kaldı — kapatılıyor",
                    TUNNEL_IDLE_TIMEOUT.as_secs()
                );
                break;
            }
        }
    };

    tokio::select! {
        _ = pump => {},
        _ = watchdog => {},
    }
}

#[cfg(test)]
mod tests {
    use super::is_own_domain;

    #[test]
    fn own_domain_matches_site_and_subdomains() {
        assert!(is_own_domain("openani.me"));
        assert!(is_own_domain("openani.me:443"));
        assert!(is_own_domain("api.openani.me"));
        assert!(is_own_domain("API.OpenAni.me:443"));
        assert!(is_own_domain("canvas.openani.me"));
    }

    #[test]
    fn own_domain_rejects_lookalikes() {
        // Sonek kontrolü nokta İLE yapılmalı: "notopenani.me" bizim değil.
        assert!(!is_own_domain("notopenani.me"));
        assert!(!is_own_domain("openani.me.evil.com"));
        assert!(!is_own_domain("example.com:80"));
    }
}
