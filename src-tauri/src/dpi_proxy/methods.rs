// ═══════════════════════════════════════════════════════════════════════
// 🛡️ DPI Atlatma Yöntemleri (DPI Bypass Methods)
// ═══════════════════════════════════════════════════════════════════════
// Amaç:
//   Deep Packet Inspection (DPI) tarafından engellenen bağlantıları atlatmak
//   için kullanılan 9 farklı stratejinin tanımlaması ve yönetimi.
//   DPI engellemesi: HTTP/TLS trafiğinin header, Host, SNI alanlarını
//   inceleyip sansürleme. Atlatma: fragmentasyon, case obfuscation vb.
//
// Bağlantılı Dosyalar:
//   • mod.rs (dpi_proxy) — DpiProxySettings, yöntem sıraması, test mantığı
//   • tcp_forward.rs — fragment + case manipülasyonları uygulaması
//   • settings.rs — method order ve state yönetimi
//
// Yöntemler (0-8):
//   0) Direct — bypass yok (baseline test)
//   1) Host case — Host: hoSt: (VERY light)
//   2-4) Fragment — HTTP/TLS split packet (DPI trigger'ı bypass)
//   5) SNI fragment — TLS handshake'in SNI field'inden önce fragment
//   6) Reverse fragment — small→large (staggered pattern)
//   7-8) Combined — case + fragment + full aggressive
// ═══════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

// ═══════════════════════════════════════════════════════════
// Durum ve Kayıt Yapıları
// ═══════════════════════════════════════════════════════════

/// MethodStatus — Her DPI yöntemi için test durumu takibi.
/// WHY: Dış sistemlerin DPI politikası değişebilir, yöntemi test etmeden
/// kullanmak başarısızlığa neden olabilir. Status ile cache stratejisi yapılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MethodStatus {
    Untested,
    Working,
    Failed,
}

/// DpiMethodRecord — Yönteme ait test geçmişi ve başarı istatistikleri.
/// WHY: Hangi yöntem dış DPI'de başarılı, hangileri başarısız bilmek
/// lazım. Bu kayıt başarılı yönteme hızlı geçmek için (trial-and-error'dan kaçınmak).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DpiMethodRecord {
    pub id: u32,                      // Yöntem ID (0-8)
    pub status: MethodStatus,         // Test durumu (Untested/Working/Failed)
    pub success_count: u32,           // Kaç kez başarılı oldu
    pub fail_count: u32,              // Kaç kez başarısız oldu
    pub first_success: Option<String>, // İlk başarı zamanı (ISO 8601)
    pub last_tested: Option<String>,   // Son test zamanı (ISO 8601)
}

/// DpiMethod — DPI atlatma yönteminin yapılandırması.
/// WHY: Her yöntem farklı kombinasyonlar (case + fragment + SNI pattern) kullanır.
/// Bazıları lightweight (case), bazıları aggressive (full combo) = trade-off.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DpiMethod {
    pub id: u32,                      // Yöntem ID (0-8)
    pub name: String,                 // Görünen isim (UI)
    pub description: String,          // Teknik açıklaması
    pub http_host_case: bool,         // Host: header'ında case değiştir (Host: hoSt:)
    pub http_host_mixedcase: bool,    // Random case obfuscation (daha agresif)
    pub http_host_removespace: bool,  // Whitespace enjeksiyon (çok agresif, tehlikeli)
    pub http_fragment_size: u32,      // HTTP paketini kaç byte'ta kır (0=yok)
    pub https_fragment_size: u32,     // TLS paketini kaç byte'ta kır (0=yok)
    pub fragment_by_sni: bool,        // TLS SNI'dan ÖNCE fragment'ı başlat
    pub reverse_fragment: bool,       // Fragment büyüklüğünü reverse et (small→large pattern)
}

// ═══════════════════════════════════════════════════════════
// DPI Yöntemleri Listesi
// ═══════════════════════════════════════════════════════════
// WHY: 9 yöntem escalation sırasıyla tanımlanır (lightweight → aggressive).
// Sistem başta yöntem 0'dan (baseline) başlayıp başarısız olunca
// 1, 2, ..., 8'e geçiş yapar (settings.rs::default_method_order).
// Her yöntem kombinasyon: Host case obfuscation + packet fragmentasyon.

fn make_methods() -> Vec<DpiMethod> {
    vec![
        // ID 0: Baseline (bypass yok) — control, normal bağlantı
        DpiMethod { id: 0, name: "Direct (Bypass Yok)".into(), description: "DPI bypass tekniklerini uygulamadan doğrudan bağlantı kurar".into(), http_host_case: false, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 0, https_fragment_size: 0, fragment_by_sni: false, reverse_fragment: false },

        // ID 1: Host case obfuscation (çok hafif) — sadece Host: → hoSt: değiştir
        DpiMethod { id: 1, name: "Host Case Change".into(), description: "Host: → hoSt: (en hafif)".into(), http_host_case: true, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 0, https_fragment_size: 0, fragment_by_sni: false, reverse_fragment: false },

        // ID 2: HTTP fragmentasyon (packet split) — HTTP payload'ını 2 parçaya böl
        DpiMethod { id: 2, name: "HTTP Fragment 2".into(), description: "Sadece HTTP'yi 2 parçaya böl".into(), http_host_case: false, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 2, https_fragment_size: 0, fragment_by_sni: false, reverse_fragment: false },

        // ID 3: TLS fragmentasyon — TLS ClientHello'yu 2 parçaya böl
        DpiMethod { id: 3, name: "HTTPS Fragment 2".into(), description: "Sadece TLS'yi 2 parçaya böl".into(), http_host_case: false, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 0, https_fragment_size: 2, fragment_by_sni: false, reverse_fragment: false },

        // ID 4: Çifte fragmentasyon — hem HTTP hem TLS bölün
        DpiMethod { id: 4, name: "HTTP+HTTPS Fragment 2".into(), description: "İkisini de 2 parçaya böl".into(), http_host_case: false, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 2, https_fragment_size: 2, fragment_by_sni: false, reverse_fragment: false },

        // ID 5: SNI (Server Name Indication) farkında fragmentasyon
        // TLS ClientHello'daki SNI field'i DPI'nin hedefi (SNI leak).
        // fragment_by_sni=true: SNI payload'ından ÖNCE parçayı bitir.
        DpiMethod { id: 5, name: "SNI Bazlı Fragment".into(), description: "TLS SNI'den önce parçala".into(), http_host_case: false, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 0, https_fragment_size: 1, fragment_by_sni: true, reverse_fragment: false },

        // ID 6: Reverse fragmentasyon — başla küçük parçayla, sonra büyüt
        // Normal: [small, large]. Reverse: [small, large] ama timing/pattern farklı.
        // Staggered pattern DPI inspection engine'ını kaçırabilir.
        DpiMethod { id: 6, name: "Reverse Fragment".into(), description: "Önce küçük, sonra büyük parça".into(), http_host_case: false, http_host_mixedcase: false, http_host_removespace: false, http_fragment_size: 2, https_fragment_size: 2, fragment_by_sni: false, reverse_fragment: true },

        // ID 7: Hibrit — case obfuscation + double fragment
        DpiMethod { id: 7, name: "Mixed Case + Fragment".into(), description: "Case + HTTP/HTTPS fragment".into(), http_host_case: true, http_host_mixedcase: true, http_host_removespace: false, http_fragment_size: 2, https_fragment_size: 2, fragment_by_sni: false, reverse_fragment: false },

        // ID 8: Tam agresif — TÜM teknikler bir arada (high risk policing)
        // http_host_removespace: whitespace enjeksiyonu (çok tehlikeli, bazı proxy'ler reject)
        DpiMethod { id: 8, name: "Full (en agresif)".into(), description: "Tüm teknikler bir arada".into(), http_host_case: true, http_host_mixedcase: true, http_host_removespace: true, http_fragment_size: 2, https_fragment_size: 2, fragment_by_sni: true, reverse_fragment: true },
    ]
}


// ═══════════════════════════════════════════════════════════
// Global Yöntemler Listesi ve Arama Fonksiyonları
// ═══════════════════════════════════════════════════════════

/// ALL_METHODS — Tüm DPI yöntemlerinin static listesi.
/// WHY: LazyLock kullanılır çünkü Vec<DpiMethod> oluşturması bir kez
/// yapılır (lazy initialization), sonra birden fazla thread'den
/// güvenle okunur (LazyLock ≈ OnceLock, thread-safe).
pub static ALL_METHODS: LazyLock<Vec<DpiMethod>> = LazyLock::new(make_methods);

/// get_method_by_id(id) — Verilen ID'deki yöntemi bul.
/// WHY: mod.rs::test_method() bu fonksiyonu kullanarak
/// geçerli yöntemi test sırasında runtime'da seçer.
pub fn get_method_by_id(id: u32) -> Option<&'static DpiMethod> {
    ALL_METHODS.iter().find(|m| m.id == id)
}

/// default_method_order() — Test sırası (ID 0 hariç, 1-8).
/// WHY: Sistem başta hafif yöntemlerle deneyip başarısız olunca
/// daha agresif yöntemlere geçer (escalation strategy).
/// mod.rs'deki test_next_method() bu sirayı kullanır.
pub fn default_method_order() -> Vec<u32> {
    ALL_METHODS.iter().filter(|m| m.id != 0).map(|m| m.id).collect()
}
