// === OpenAnime — Remote Proxy Fallback Modülü ===
// Uzak proxy olarak Cloudflare (DoH ve Worker) entegrasyonu.

use std::time::Duration;
use std::net::IpAddr;

/// Cloudflare/Google DNS-over-HTTPS (DoH) JSON API kullanarak DNS çözer.
/// Bu sayede servis sağlayıcının DNS engellemeleri (DNS poisoning) aşılır.
pub async fn resolve_dns_doh(domain: &str) -> Option<IpAddr> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .no_proxy()
        .build()
        .ok()?;

    let urls = vec![
        format!("https://1.1.1.1/dns-query?name={}&type=A", domain),
        format!("https://dns.google/resolve?name={}&type=A", domain),
        format!("https://1.0.0.1/dns-query?name={}&type=A", domain),
        format!("https://8.8.8.8/resolve?name={}&type=A", domain),
    ];

    for url in urls {
        if let Ok(resp) = client.get(&url).header("accept", "application/dns-json").send().await {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(answers) = json.get("Answer").and_then(|a| a.as_array()) {
                            for answer in answers {
                                if let Some(data) = answer.get("data").and_then(|d| d.as_str()) {
                                    if let Ok(ip) = data.parse::<IpAddr>() {
                                        return Some(ip);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Uzak proxy olarak Cloudflare Worker / Reverse Proxy bağlantısını dener.
/// DPI engellerinin yanı sıra coğrafi/yurt dışı IP engellerini aşmak için tasarlanmıştır.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub async fn try_remote_proxy_connection() -> Result<(), String> {
    println!("[Remote Proxy] Cloudflare Worker reverse proxy fallback deneniyor...");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;

    // TODO: Kullanıcının veya projenin kendine ait Cloudflare Worker reverse proxy URL'si buraya yazılacaktır.
    // Örnek Worker kodu:
    // addEventListener('fetch', event => {
    //   let url = new URL(event.request.url);
    //   url.hostname = 'openani.me';
    //   event.respondWith(fetch(url, event.request));
    // });
    let worker_proxy_url = "https://proxy.openani.me/health"; 

    // Bağlantıyı test et
    match client.get(worker_proxy_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("[Remote Proxy] ✅ Cloudflare Worker proxy bağlantısı başarılı!");
                Ok(())
            } else {
                Err(format!("Cloudflare Worker proxy HTTP hata kodu: {}", resp.status()))
            }
        }
        Err(e) => {
            Err(format!("Cloudflare Worker proxy bağlantı hatası: {}", e))
        }
    }
}
