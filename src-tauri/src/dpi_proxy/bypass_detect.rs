// ═══════════════════════════════════════════════════════════════════════
// OpenAnime — Harici DPI Bypass Aracı Tespiti
// ═══════════════════════════════════════════════════════════════════════
// Amaç:
//   Sistemde ZATEN çalışan bir DPI atlatma aracı (Zapret, GoodbyeDPI, ByeDPI)
//   veya sistem geneli tünel (Cloudflare WARP) varsa tespit etmek ve DPI
//   Proxy'nin kendi TLS/HTTP manipülasyonunu buna göre kısmak.
//
// NEDEN GEREKLİ:
//   Bu proxy TLS ClientHello'yu parçalıyor (tcp_forward.rs). Sistemde zaten
//   paket seviyesinde aynı işi yapan bir araç varsa iki manipülasyon üst üste
//   biniyor: ClientHello iki kez bölünüyor, kimi sunucu/ara katman bunu
//   bozuk handshake sayıp bağlantıyı düşürüyor. Yani "bypass + bypass"
//   toplamda DAHA KÖTÜ sonuç veriyor. Cloudflare WARP'ta ise trafik zaten
//   şifreli bir tünelin içinden geçtiğinden fragmentasyonun hiçbir faydası
//   yok; üstüne DoH ile DNS'i de ezmek WARP'ın kendi çözümleyicisiyle
//   çakışıyor.
//
// TASARIM (test edilebilirlik):
//   Sistemle konuşan kısım (komut çalıştırma) ile KARAR VEREN kısım
//   bilinçli olarak ayrıldı. Aşağıdaki fonksiyonların tamamı saf (pure) ve
//   birim testi yazılabilir:
//     • normalize_process_name / classify_process_name
//     • parse_tasklist_csv / parse_ps_output
//     • detect_tools_from_names
//     • parse_sc_query_running
//     • output_has_warp_interface
//     • decide_behavior
//   Yalnızca `detect()` ve `refresh_and_log()` gerçek sisteme dokunur.
//
// Bağlantılı dosyalar:
//   • mod.rs         — başlangıçta + periyodik olarak refresh_and_log() çağırır
//   • tcp_forward.rs — current_behavior() ile fragmentasyon/DoH kararını verir
// ═══════════════════════════════════════════════════════════════════════

use std::sync::atomic::{AtomicU8, Ordering};

use crate::{dbg_log, log};

// ═══════════════════════════════════════════════════════════
// Tipler
// ═══════════════════════════════════════════════════════════

/// Tespit edilebilen harici araçlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum BypassTool {
    Zapret,
    GoodbyeDpi,
    ByeDpi,
    CloudflareWarp,
}

impl BypassTool {
    /// Log/UI'da gösterilecek okunabilir ad.
    pub fn label(self) -> &'static str {
        match self {
            BypassTool::Zapret => "Zapret",
            BypassTool::GoodbyeDpi => "GoodbyeDPI",
            BypassTool::ByeDpi => "ByeDPI",
            BypassTool::CloudflareWarp => "Cloudflare WARP",
        }
    }
}

/// DPI Proxy'nin bu tespite göre alacağı davranış.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ProxyBehavior {
    /// Harici araç yok — mevcut fragmentasyon mantığı normal çalışır.
    Fragment,
    /// Harici bir DPI bypass aracı aktif — çakışmamak için DÜZ tünelleme
    /// (fragmentasyon ve HTTP header manipülasyonu devre dışı).
    PassThrough,
    /// Cloudflare WARP aktif — ek olarak DoH DNS ezmesi de devre dışı;
    /// trafiğe hiç dokunmayıp WARP'ın kendi tüneline/çözümleyicisine bırakılır.
    WarpBypass,
}

impl ProxyBehavior {
    /// Fragmentasyon ve HTTP header manipülasyonu uygulanmalı mı?
    pub fn allows_fragmentation(self) -> bool {
        matches!(self, ProxyBehavior::Fragment)
    }

    /// Kendi DoH (DNS-over-HTTPS) çözümlememizi uygulamalı mıyız?
    /// WARP kendi DNS'ini kurduğu için onun üstüne binmiyoruz.
    pub fn allows_doh_override(self) -> bool {
        !matches!(self, ProxyBehavior::WarpBypass)
    }

    fn as_u8(self) -> u8 {
        match self {
            ProxyBehavior::Fragment => 0,
            ProxyBehavior::PassThrough => 1,
            ProxyBehavior::WarpBypass => 2,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            1 => ProxyBehavior::PassThrough,
            2 => ProxyBehavior::WarpBypass,
            _ => ProxyBehavior::Fragment,
        }
    }
}

/// Tespit sonucunun tamamı.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct DetectionReport {
    /// Process/servis adından tespit edilen araçlar (tekrarsız).
    pub tools: Vec<BypassTool>,
    /// WinDivert çekirdek sürücüsü yüklü ve ÇALIŞIYOR mu (Windows).
    /// GoodbyeDPI ve Zapret/winws bunu kullanır — process adı değişse bile
    /// (yeniden adlandırılmış exe, fork) bu sinyal ayakta kalır.
    pub windivert_active: bool,
    /// Cloudflare WARP'a ait bir ağ arayüzü var mı.
    pub warp_interface: bool,
}

impl DetectionReport {
    /// Log için insan okunabilir özet ("GoodbyeDPI, WinDivert sürücüsü" gibi).
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = self.tools.iter().map(|t| t.label().to_string()).collect();
        if self.windivert_active {
            parts.push("WinDivert sürücüsü".to_string());
        }
        if self.warp_interface && !self.tools.contains(&BypassTool::CloudflareWarp) {
            parts.push("WARP ağ arayüzü".to_string());
        }
        if parts.is_empty() {
            "yok".to_string()
        } else {
            parts.join(", ")
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Saf (pure) yardımcılar — birim testi kapsamında
// ═══════════════════════════════════════════════════════════

/// Bilinen process adları → araç eşlemesi.
///
/// TAM EŞLEŞME kullanılıyor (`contains` DEĞİL): "contains" ile "warp" aramak
/// "warpaint.exe" gibi alakasız süreçleri de yakalayıp fragmentasyonu boş yere
/// kapatırdı. Adlar `normalize_process_name` ile küçük harfe indirilip ".exe"
/// uzantısı atıldıktan sonra karşılaştırılır.
const PROCESS_TABLE: &[(&str, BypassTool)] = &[
    // Zapret ailesi: Linux'ta nfqws/tpws, Windows portunda winws.
    ("zapret", BypassTool::Zapret),
    ("nfqws", BypassTool::Zapret),
    ("tpws", BypassTool::Zapret),
    ("winws", BypassTool::Zapret),
    // GoodbyeDPI ("goodbydpi" yaygın bir yanlış yazım/fork adı — ikisi de).
    ("goodbyedpi", BypassTool::GoodbyeDpi),
    ("goodbydpi", BypassTool::GoodbyeDpi),
    // ByeDPI ve CLI ikizi ciadpi (yerel SOCKS5 proxy olarak çalışır).
    ("byedpi", BypassTool::ByeDpi),
    ("ciadpi", BypassTool::ByeDpi),
    // Cloudflare WARP: servis (warp-svc), CLI ve GUI istemcisi.
    ("warp-svc", BypassTool::CloudflareWarp),
    ("warp-cli", BypassTool::CloudflareWarp),
    ("cloudflarewarp", BypassTool::CloudflareWarp),
    ("cloudflare warp", BypassTool::CloudflareWarp),
];

/// Windows'ta kontrol edilecek servis adları (sc query ile).
/// Araç process olarak görünmese bile (servis durdurulmuş ama kurulu, ya da
/// SYSTEM hesabında çalışıp tasklist'te kısıtlı görünüyor) servis kaydı sinyal verir.
#[cfg(target_os = "windows")]
const SERVICE_TABLE: &[(&str, BypassTool)] = &[
    ("zapret", BypassTool::Zapret),
    ("winws", BypassTool::Zapret),
    ("goodbyedpi", BypassTool::GoodbyeDpi),
    ("CloudflareWARP", BypassTool::CloudflareWarp),
];

/// WinDivert sürücüsünün olası servis adları (sürüme göre değişir).
#[cfg(target_os = "windows")]
const WINDIVERT_SERVICES: &[&str] = &["WinDivert", "WinDivert1.4", "WinDivert1.3"];

/// Process adını karşılaştırmaya hazırlar: küçük harf, kırpılmış, ".exe" yok.
/// Yol verilirse yalnızca dosya adı kısmı alınır (`ps -o comm=` bazen tam yol verir).
pub fn normalize_process_name(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"');
    // Yol ayırıcılarından sonrasını al (hem / hem \ — platformdan bağımsız).
    let file = trimmed
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(trimmed);
    let lower = file.to_ascii_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_string()
}

/// Tek bir process adını bilinen araçlara göre sınıflandırır.
pub fn classify_process_name(raw: &str) -> Option<BypassTool> {
    let name = normalize_process_name(raw);
    if name.is_empty() {
        return None;
    }
    PROCESS_TABLE
        .iter()
        .find(|(pattern, _)| *pattern == name)
        .map(|(_, tool)| *tool)
}

/// Process adı listesinden tespit edilen araçları (tekrarsız) çıkarır.
pub fn detect_tools_from_names<S: AsRef<str>>(names: &[S]) -> Vec<BypassTool> {
    let mut found: Vec<BypassTool> = Vec::new();
    for n in names {
        if let Some(tool) = classify_process_name(n.as_ref()) {
            if !found.contains(&tool) {
                found.push(tool);
            }
        }
    }
    found
}

/// `tasklist /NH /FO CSV` çıktısından process adlarını ayıklar.
/// Satır formatı: "iexplore.exe","1234","Console","1","10.000 K"
pub fn parse_tasklist_csv(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            // İlk CSV alanı (tırnak içinde) process adıdır.
            let first = line.split("\",\"").next()?;
            let name = first.trim_start_matches('"').trim_end_matches('"').trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

/// `ps -A -o comm=` çıktısından process adlarını ayıklar (Linux/macOS).
/// Windows derlemesinde çağrılmaz (list_process_names cfg ile ayrılmış) ama
/// testleri her platformda çalışır — bu yüzden dead_code uyarısı bastırılıyor.
#[cfg_attr(target_os = "windows", allow(dead_code))]
pub fn parse_ps_output(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

/// `sc query <servis>` çıktısında servisin ÇALIŞIR durumda olup olmadığı.
/// Servis kurulu değilse sc hata döner ve bu fonksiyon false verir.
/// Not: Windows dil paketine bağlı olmamak için sayısal durum kodu (4 = RUNNING)
/// aranır; "RUNNING"/"ÇALIŞIYOR" gibi yerelleştirilmiş metne güvenilmez.
pub fn parse_sc_query_running(output: &str) -> bool {
    output.lines().any(|line| {
        let l = line.trim();
        if !l.to_ascii_uppercase().starts_with("STATE") {
            return false;
        }
        // "STATE              : 4  RUNNING"
        match l.split(':').nth(1) {
            Some(rest) => rest.split_whitespace().next() == Some("4"),
            None => false,
        }
    })
}

/// Ağ arayüzü listesi çıktısında Cloudflare WARP arayüzü var mı.
/// `netsh interface show interface` (Windows) veya `ip -o link show` /
/// `ifconfig -l` (Linux/macOS) çıktısıyla çalışır.
///
/// Yalnız başına "warp" aramıyoruz — başka bir arayüz adında geçebilir;
/// WARP'ın kurduğu arayüz adları bilinçli olarak tam ifadelerle eşleştirilir.
pub fn output_has_warp_interface(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.contains("cloudflare warp") || lower.contains("cloudflarewarp")
}

/// Tespit sonucundan davranış kararı.
///
/// Öncelik sırası bilinçli:
///   1. WARP (sistem geneli tünel) — en kapsayıcı durum, trafiğe hiç dokunma.
///   2. Diğer DPI araçları veya WinDivert sürücüsü — düz tünelleme.
///   3. Hiçbiri — normal fragmentasyon.
pub fn decide_behavior(report: &DetectionReport) -> ProxyBehavior {
    if report.warp_interface || report.tools.contains(&BypassTool::CloudflareWarp) {
        return ProxyBehavior::WarpBypass;
    }
    if !report.tools.is_empty() || report.windivert_active {
        return ProxyBehavior::PassThrough;
    }
    ProxyBehavior::Fragment
}

// ═══════════════════════════════════════════════════════════
// Global durum — tcp_forward her bağlantıda ucuzca okur
// ═══════════════════════════════════════════════════════════

/// Güncel davranış. AtomicU8: bağlantı başına kilitsiz okuma (sıcak yol).
static BEHAVIOR: AtomicU8 = AtomicU8::new(0); // 0 = Fragment (güvenli varsayılan)

/// tcp_forward bunu her bağlantıda çağırır — kilit yok, maliyeti ihmal edilebilir.
pub fn current_behavior() -> ProxyBehavior {
    ProxyBehavior::from_u8(BEHAVIOR.load(Ordering::Relaxed))
}

fn store_behavior(b: ProxyBehavior) {
    BEHAVIOR.store(b.as_u8(), Ordering::Relaxed);
}

// ═══════════════════════════════════════════════════════════
// Sistemle konuşan kısım
// ═══════════════════════════════════════════════════════════

/// Komutu çalıştırıp stdout'u döndürür. Windows'ta konsol penceresi açmaz.
/// Hata durumunda None (tespit "yok" sayılır — asla panic/blok yok).
fn run_capture(program: &str, args: &[&str]) -> Option<String> {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    // sc query gibi komutlar hata durumunda stdout'a da yazabiliyor; ikisini birleştir.
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.stderr.is_empty() {
        s.push('\n');
        s.push_str(&String::from_utf8_lossy(&out.stderr));
    }
    Some(s)
}

/// Çalışan process adlarını listeler (platforma göre).
fn list_process_names() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        match run_capture("tasklist", &["/NH", "/FO", "CSV"]) {
            Some(out) => parse_tasklist_csv(&out),
            None => Vec::new(),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        match run_capture("ps", &["-A", "-o", "comm="]) {
            Some(out) => parse_ps_output(&out),
            None => Vec::new(),
        }
    }
}

/// Windows servis kayıtlarından araç tespiti.
#[cfg(target_os = "windows")]
fn detect_from_services() -> Vec<BypassTool> {
    let mut found = Vec::new();
    for (svc, tool) in SERVICE_TABLE {
        if let Some(out) = run_capture("sc", &["query", svc]) {
            if parse_sc_query_running(&out) && !found.contains(tool) {
                dbg_log!("[DPI Proxy] Servis tespit edildi: {} ({})", svc, tool.label());
                found.push(*tool);
            }
        }
    }
    found
}

/// WinDivert çekirdek sürücüsü çalışıyor mu (Windows).
#[cfg(target_os = "windows")]
fn detect_windivert() -> bool {
    for svc in WINDIVERT_SERVICES {
        if let Some(out) = run_capture("sc", &["query", svc]) {
            if parse_sc_query_running(&out) {
                dbg_log!("[DPI Proxy] WinDivert sürücüsü aktif: {}", svc);
                return true;
            }
        }
    }
    false
}

/// Cloudflare WARP ağ arayüzü var mı.
fn detect_warp_interface() -> bool {
    #[cfg(target_os = "windows")]
    {
        run_capture("netsh", &["interface", "show", "interface"])
            .map(|o| output_has_warp_interface(&o))
            .unwrap_or(false)
    }
    #[cfg(target_os = "linux")]
    {
        run_capture("ip", &["-o", "link", "show"])
            .map(|o| output_has_warp_interface(&o))
            .unwrap_or(false)
    }
    #[cfg(target_os = "macos")]
    {
        run_capture("ifconfig", &["-a"])
            .map(|o| output_has_warp_interface(&o))
            .unwrap_or(false)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        false
    }
}

/// Sistemi tarayıp tam tespit raporunu üretir.
pub fn detect() -> DetectionReport {
    let names = list_process_names();
    let mut tools = detect_tools_from_names(&names);

    #[cfg(target_os = "windows")]
    {
        for t in detect_from_services() {
            if !tools.contains(&t) {
                tools.push(t);
            }
        }
    }

    let windivert_active = {
        #[cfg(target_os = "windows")]
        {
            detect_windivert()
        }
        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    };

    let warp_interface = detect_warp_interface();

    DetectionReport {
        tools,
        windivert_active,
        warp_interface,
    }
}

/// Tespiti çalıştırır, global davranışı günceller ve SONUÇ DEĞİŞTİYSE loglar.
///
/// Periyodik çağrıldığı için her seferinde log basmıyoruz — yalnızca durum
/// değişiminde. İlk çağrıda `force_log` ile açılış özeti her hâlükârda basılır.
pub fn refresh_and_log(force_log: bool) -> ProxyBehavior {
    let report = detect();
    let behavior = decide_behavior(&report);
    let previous = current_behavior();
    store_behavior(behavior);

    if force_log || behavior != previous {
        match behavior {
            ProxyBehavior::WarpBypass => log!(
                "[DPI Proxy] Tespit: {} aktif, trafik manipülasyonu ve DoH devre dışı (WARP tüneline dokunulmuyor)",
                report.summary()
            ),
            ProxyBehavior::PassThrough => log!(
                "[DPI Proxy] Tespit: {} aktif, fragmentasyon devre dışı bırakıldı (düz tünelleme)",
                report.summary()
            ),
            ProxyBehavior::Fragment => log!(
                "[DPI Proxy] Tespit: Aktif DPI bypass aracı yok, fragmentasyon normal çalışıyor"
            ),
        }
    }

    dbg_log!(
        "[DPI Proxy] Tespit ayrıntısı: araçlar={:?}, windivert={}, warp_arayüzü={}",
        report.tools,
        report.windivert_active,
        report.warp_interface
    );

    behavior
}

// ═══════════════════════════════════════════════════════════
// Testler
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_exe_path_and_case() {
        assert_eq!(normalize_process_name("GoodbyeDPI.exe"), "goodbyedpi");
        assert_eq!(normalize_process_name("  winws.EXE  "), "winws");
        assert_eq!(normalize_process_name(r"C:\Tools\zapret\winws.exe"), "winws");
        assert_eq!(normalize_process_name("/usr/sbin/nfqws"), "nfqws");
        assert_eq!(normalize_process_name("\"warp-svc.exe\""), "warp-svc");
    }

    #[test]
    fn classifies_known_tools() {
        assert_eq!(classify_process_name("goodbyedpi.exe"), Some(BypassTool::GoodbyeDpi));
        assert_eq!(classify_process_name("goodbydpi.exe"), Some(BypassTool::GoodbyeDpi));
        assert_eq!(classify_process_name("winws.exe"), Some(BypassTool::Zapret));
        assert_eq!(classify_process_name("nfqws"), Some(BypassTool::Zapret));
        assert_eq!(classify_process_name("tpws"), Some(BypassTool::Zapret));
        assert_eq!(classify_process_name("ciadpi.exe"), Some(BypassTool::ByeDpi));
        assert_eq!(classify_process_name("byedpi"), Some(BypassTool::ByeDpi));
        assert_eq!(classify_process_name("warp-svc.exe"), Some(BypassTool::CloudflareWarp));
    }

    #[test]
    fn ignores_unrelated_processes() {
        // Kısmi eşleşmeyle yanlış pozitif ÜRETMEMELİ (contains yerine tam eşleşme).
        assert_eq!(classify_process_name("warpaint.exe"), None);
        assert_eq!(classify_process_name("zapretinator.exe"), None);
        assert_eq!(classify_process_name("mygoodbyedpi-helper.exe"), None);
        assert_eq!(classify_process_name("chrome.exe"), None);
        assert_eq!(classify_process_name(""), None);
    }

    #[test]
    fn dedupes_tools_across_process_names() {
        // Zapret hem nfqws hem winws olarak görünebilir → tek kayıt.
        let names = vec!["nfqws", "winws.exe", "chrome.exe", "explorer.exe"];
        let tools = detect_tools_from_names(&names);
        assert_eq!(tools, vec![BypassTool::Zapret]);
    }

    #[test]
    fn parses_tasklist_csv() {
        let out = "\"System Idle Process\",\"0\",\"Services\",\"0\",\"8 K\"\r\n\
                   \"goodbyedpi.exe\",\"4242\",\"Console\",\"1\",\"5.120 K\"\r\n\
                   \"chrome.exe\",\"1337\",\"Console\",\"1\",\"120.000 K\"\r\n";
        let names = parse_tasklist_csv(out);
        assert!(names.contains(&"goodbyedpi.exe".to_string()));
        assert!(names.contains(&"chrome.exe".to_string()));
        let tools = detect_tools_from_names(&names);
        assert_eq!(tools, vec![BypassTool::GoodbyeDpi]);
    }

    #[test]
    fn parses_ps_output() {
        let out = "systemd\n/usr/sbin/nfqws\nfirefox\n\n";
        let names = parse_ps_output(out);
        assert_eq!(names.len(), 3);
        assert_eq!(detect_tools_from_names(&names), vec![BypassTool::Zapret]);
    }

    #[test]
    fn sc_query_detects_running_state_numerically() {
        let running = "SERVICE_NAME: WinDivert\n        TYPE               : 1  KERNEL_DRIVER\n        STATE              : 4  RUNNING\n";
        let stopped = "SERVICE_NAME: WinDivert\n        TYPE               : 1  KERNEL_DRIVER\n        STATE              : 1  STOPPED\n";
        let missing = "[SC] EnumQueryServicesStatus:OpenService FAILED 1060:\n";
        assert!(parse_sc_query_running(running));
        assert!(!parse_sc_query_running(stopped));
        assert!(!parse_sc_query_running(missing));
    }

    /// Gerçek `sc.exe query Dnscache` çıktısı — Türkçe Windows 11'de
    /// (26200) birebir yakalandı. sc.exe, sistem dili Türkçe olsa bile
    /// alan adlarını ("STATE") ve durum metnini ("RUNNING") İNGİLİZCE
    /// bırakıyor; ayrıca satır sonlarında ekstra boşluk bırakıyor.
    /// Bu test o gözlemi sabitler: parser yerelleştirmeden etkilenmemeli
    /// ve sondaki boşluklar sonucu bozmamalı.
    #[test]
    fn sc_query_parses_real_turkish_windows_output() {
        let real = "\r\nSERVICE_NAME: Dnscache \r\n        TYPE               : 10  WIN32_OWN_PROCESS  \r\n        STATE              : 4  RUNNING \r\n                                (NOT_STOPPABLE, NOT_PAUSABLE, IGNORES_SHUTDOWN)\r\n        WIN32_EXIT_CODE    : 0  (0x0)\r\n";
        assert!(parse_sc_query_running(real));
    }

    /// Gerçek `tasklist /NH /FO CSV` çıktısı (Türkçe Windows 11). Bellek
    /// sütununda binlik ayıracı olarak nokta kullanılıyor ("2.672 K") —
    /// parser yalnızca ilk alanı aldığı için bundan etkilenmemeli.
    #[test]
    fn tasklist_parses_real_windows_output() {
        let real = "\"System Idle Process\",\"0\",\"Services\",\"0\",\"8 K\"\r\n\
                    \"System\",\"4\",\"Services\",\"0\",\"2.672 K\"\r\n\
                    \"Secure System\",\"188\",\"Services\",\"0\",\"83.828 K\"\r\n";
        let names = parse_tasklist_csv(real);
        assert_eq!(names, vec!["System Idle Process", "System", "Secure System"]);
        assert!(detect_tools_from_names(&names).is_empty());
    }

    /// Gerçek `netsh interface show interface` çıktısı. "Radmin VPN" gibi
    /// ALAKASIZ bir VPN arayüzü WARP sanılmamalı — aksi halde fragmentasyon
    /// boş yere tamamen kapanırdı.
    #[test]
    fn netsh_does_not_false_positive_on_other_vpns() {
        let real = "\r\nAdmin State    State          Type             Interface Name\r\n\
                    -------------------------------------------------------------------------\r\n\
                    Enabled        Connected      Dedicated        Wi-Fi\r\n\
                    Enabled        Connected      Dedicated        Radmin VPN\r\n\
                    Enabled        Disconnected   Dedicated        Ethernet\r\n";
        assert!(!output_has_warp_interface(real));
    }

    #[test]
    fn detects_warp_interface_from_netsh() {
        let netsh = "Admin State    State          Type             Interface Name\n\
                     -------------------------------------------------------------\n\
                     Enabled        Connected      Dedicated        Ethernet\n\
                     Enabled        Connected      Dedicated        Cloudflare WARP\n";
        assert!(output_has_warp_interface(netsh));

        let plain = "Enabled        Connected      Dedicated        Ethernet\n";
        assert!(!output_has_warp_interface(plain));
    }

    #[test]
    fn behavior_no_tools_keeps_fragmentation() {
        let r = DetectionReport::default();
        assert_eq!(decide_behavior(&r), ProxyBehavior::Fragment);
        assert!(ProxyBehavior::Fragment.allows_fragmentation());
        assert!(ProxyBehavior::Fragment.allows_doh_override());
    }

    #[test]
    fn behavior_dpi_tool_disables_fragmentation() {
        let r = DetectionReport {
            tools: vec![BypassTool::GoodbyeDpi],
            ..Default::default()
        };
        let b = decide_behavior(&r);
        assert_eq!(b, ProxyBehavior::PassThrough);
        assert!(!b.allows_fragmentation());
        // Düz tünellemede DoH hâlâ faydalı (DNS engeli ayrı bir sorun).
        assert!(b.allows_doh_override());
    }

    #[test]
    fn behavior_windivert_alone_disables_fragmentation() {
        // Araç yeniden adlandırılmış olabilir; sürücü sinyali tek başına yeter.
        let r = DetectionReport {
            windivert_active: true,
            ..Default::default()
        };
        assert_eq!(decide_behavior(&r), ProxyBehavior::PassThrough);
    }

    #[test]
    fn behavior_warp_takes_precedence_over_other_tools() {
        let r = DetectionReport {
            tools: vec![BypassTool::GoodbyeDpi, BypassTool::CloudflareWarp],
            windivert_active: true,
            warp_interface: true,
        };
        let b = decide_behavior(&r);
        assert_eq!(b, ProxyBehavior::WarpBypass);
        assert!(!b.allows_fragmentation());
        assert!(!b.allows_doh_override());
    }

    #[test]
    fn behavior_warp_interface_alone_is_enough() {
        // warp-svc process'i görünmese bile (yetki kısıtı) arayüz sinyali yeter.
        let r = DetectionReport {
            warp_interface: true,
            ..Default::default()
        };
        assert_eq!(decide_behavior(&r), ProxyBehavior::WarpBypass);
    }

    #[test]
    fn summary_is_human_readable() {
        assert_eq!(DetectionReport::default().summary(), "yok");

        let r = DetectionReport {
            tools: vec![BypassTool::GoodbyeDpi],
            windivert_active: true,
            warp_interface: false,
        };
        assert_eq!(r.summary(), "GoodbyeDPI, WinDivert sürücüsü");

        // WARP zaten araç listesindeyse arayüzü ayrıca yazma (tekrar olmasın).
        let r2 = DetectionReport {
            tools: vec![BypassTool::CloudflareWarp],
            windivert_active: false,
            warp_interface: true,
        };
        assert_eq!(r2.summary(), "Cloudflare WARP");
    }

    #[test]
    fn behavior_roundtrips_through_atomic_repr() {
        for b in [
            ProxyBehavior::Fragment,
            ProxyBehavior::PassThrough,
            ProxyBehavior::WarpBypass,
        ] {
            assert_eq!(ProxyBehavior::from_u8(b.as_u8()), b);
        }
        // Bilinmeyen değer güvenli varsayılana düşer.
        assert_eq!(ProxyBehavior::from_u8(99), ProxyBehavior::Fragment);
    }
}
