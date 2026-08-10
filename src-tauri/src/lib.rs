#![allow(linker_messages)]

use tauri::{WebviewWindowBuilder, WebviewUrl, Manager};
use std::sync::Mutex;
use std::sync::Arc;
use std::collections::HashMap;

/// Uygulama gerçekten kapanıyor mu (tepsi menüsü "Kapat" ile). true iken
/// pencere/oturum kapanışları arkaplan tepsi oturumunu yeniden AÇMAZ —
/// aksi halde gerçek çıkışta bile arkada yeni bir pencere doğardı.
pub(crate) static APP_QUITTING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Zoom seviyesini tüm pencereler arasında paylaşmak için state
pub struct ZoomState {
    pub level: Mutex<f64>,
}

impl Default for ZoomState {
    fn default() -> Self {
        Self { level: Mutex::new(1.0) }
    }
}

pub mod logger;
mod dpi_proxy;
#[cfg(target_os = "windows")]
mod perf_mode;
#[cfg(target_os = "windows")]
mod perf_report;
mod gpu_info;

/// Performans modu kararı için paylaşılan durum.
///
/// Kural: TAM PERFORMANS yalnızca (video oynuyor VE pencere odakta) iken.
/// Diğer her durumda (ana sayfa, duraklatılmış video, alt-tab) → VERİMLİLİK.
///
/// NOT: `player_playing` ve `suspended` pencere etiketine göre ayrı ayrı
/// tutulur — tek bir global bool kullanılırsa bir pencerenin durumu
/// diğerini eziyor (örn. arkaplandaki hafif tepsi oturumu "oynamıyorum"
/// derse, asıl izleme penceresindeki video yanlışlıkla askıya alınabilir
/// ya da tam tersi, kapanan bir pencerenin "oynuyor" durumu takılı kalıp
/// hiçbir pencere askıya alınamaz).
#[derive(Default)]
pub struct PerfState {
    /// Pencere etiketi -> o pencerede video fiilen oynuyor mu (JS bildirir)
    pub player_playing: Mutex<HashMap<String, bool>>,
    /// Herhangi bir pencere odakta mı
    pub focused: Mutex<bool>,
    /// Pencere etiketi -> WebView2'ye en son uygulanan arka plan durumu.
    /// (Eskiden `bool` idi; video oynarken kullanılan ara durum için 3 durumlu
    /// `BgMode`'a çevrildi — bkz. set_background_suspend.)
    #[cfg(target_os = "windows")]
    pub suspended: Mutex<HashMap<String, BgMode>>,
    /// Pencere etiketi -> arka plana (Media/DeepSleep) geçtiği an.
    /// Ön plana dönünce silinir. Sayfanın ne kadar dondurulmuş kaldığını
    /// bilmek oturum tazeliği için gerekli (bkz. background_duration).
    #[cfg(target_os = "windows")]
    pub bg_since: Mutex<HashMap<String, std::time::Instant>>,
}

#[allow(non_snake_case)]
mod discordRPC;
// Süper Bildirim toast'ı + özel tepsi menüsü YALNIZCA Windows'ta native
// (WPF/PowerShell) olarak gösterilir. Diğer platformlarda (ör. macOS CI derlemesi)
// super_notifications'ın çapraz-platform derlenebilmesi için no-op stub sağlanır.
// Uygulama zaten baştan aşağı Windows-native; stub sadece derlemeyi geçirir.
#[cfg(windows)]
mod native_toast;
#[cfg(not(windows))]
mod native_toast {
    #![allow(dead_code)]
    pub const CLICK_SIGNAL_FILE: &str = "OpenAnime_toast_click.txt";
    pub struct ToastContent<'a> {
        pub title: &'a str,
        pub body: &'a str,
        pub notif_type: &'a str,
        pub poster_path: Option<&'a str>,
        pub url: Option<&'a str>,
    }
    pub fn show_rich(_content: &ToastContent) {}
}

#[cfg(windows)]
mod native_tray_menu;
#[cfg(not(windows))]
mod native_tray_menu {
    #![allow(dead_code)]
    pub const TRAY_ACTION_FILE: &str = "OpenAnime_tray_action.txt";
    pub struct MenuHeader {
        pub name: String,
        pub subtitle: String,
    }
    pub struct MenuEntry {
        pub label: String,
        pub glyph: u32,
        pub action: String,
        pub danger: bool,
    }
    pub fn show(_header: Option<MenuHeader>, _entries: Vec<MenuEntry>, _icon_rect: (f64, f64, f64, f64)) {}
}

mod super_notifications;

mod updater;
mod local_video_server;

#[cfg(target_os = "windows")]
#[link(name = "shell32")]
extern "system" {
    fn SetCurrentProcessExplicitAppUserModelID(app_id: *const u16) -> i32;
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
mod gpu_switch_macos {
    use std::sync::Mutex;

    #[repr(C)]
    struct CGLPixelFormatObject {
        _opaque: [u8; 0],
    }
    type CGLPixelFormatObj = *mut CGLPixelFormatObject;

    #[repr(C)]
    struct CGLContextObject {
        _opaque: [u8; 0],
    }
    type CGLContextObj = *mut CGLContextObject;

    type CGLError = i32;
    type CGLPixelFormatAttribute = i32;
    type GLint = i32;

    const K_CGL_PFA_NO_RECOVERY: CGLPixelFormatAttribute = 72;
    const K_CGL_PFA_ACCELERATED: CGLPixelFormatAttribute = 73;

    #[link(name = "OpenGL", kind = "framework")]
    extern "C" {
        fn CGLChoosePixelFormat(
            attribs: *const CGLPixelFormatAttribute,
            pix: *mut CGLPixelFormatObj,
            npix: *mut GLint,
        ) -> CGLError;
        fn CGLDestroyPixelFormat(pix: CGLPixelFormatObj) -> CGLError;
        fn CGLCreateContext(
            pix: CGLPixelFormatObj,
            share: CGLContextObj,
            ctx: *mut CGLContextObj,
        ) -> CGLError;
        fn CGLDestroyContext(ctx: CGLContextObj) -> CGLError;
    }

    struct DiscreteGpuHandle {
        pixel_format: CGLPixelFormatObj,
        context: CGLContextObj,
    }

    unsafe impl Send for DiscreteGpuHandle {}

    static ACTIVE_CONTEXT: Mutex<Option<DiscreteGpuHandle>> = Mutex::new(None);

    pub fn activate() -> Result<(), String> {
        let mut guard = ACTIVE_CONTEXT.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Ok(());
        }
        unsafe {
            let attribs: [CGLPixelFormatAttribute; 3] =
                [K_CGL_PFA_ACCELERATED, K_CGL_PFA_NO_RECOVERY, 0];
            let mut pix: CGLPixelFormatObj = std::ptr::null_mut();
            let mut npix: GLint = 0;
            let err = CGLChoosePixelFormat(attribs.as_ptr(), &mut pix, &mut npix);
            if err != 0 || pix.is_null() {
                return Err(format!("CGLChoosePixelFormat failed: {}", err));
            }
            let mut ctx: CGLContextObj = std::ptr::null_mut();
            let err2 = CGLCreateContext(pix, std::ptr::null_mut(), &mut ctx);
            if err2 != 0 || ctx.is_null() {
                CGLDestroyPixelFormat(pix);
                return Err(format!("CGLCreateContext failed: {}", err2));
            }
            *guard = Some(DiscreteGpuHandle {
                pixel_format: pix,
                context: ctx,
            });
        }
        Ok(())
    }

    pub fn deactivate() -> Result<(), String> {
        let mut guard = ACTIVE_CONTEXT.lock().map_err(|e| e.to_string())?;
        if let Some(handle) = guard.take() {
            unsafe {
                CGLDestroyContext(handle.context);
                CGLDestroyPixelFormat(handle.pixel_format);
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMMON_INIT_SCRIPT — Tüm webview'lara enjekte edilen JavaScript
// Sıralama: polyfill → network → webgpu → ui → discord → updater → video → tema
// Her blok yorumla ayrılmıştır.
// ═══════════════════════════════════════════════════════════════════════════════
/// Webview'lara enjekte edilen ortak init script'i döndürür.
/// (Linux'a özgü overlay/WebGPU köprüsü kaldırıldı; Windows/macOS webview'ı
/// WebGPU'yu native sağladığından ek bayrağa gerek yok.)
fn build_init_script() -> String {
    COMMON_INIT_SCRIPT.to_string()
}

/// Performans modunu mevcut duruma göre yeniden uygula.
///
/// İki mekanizmayı BİRLİKTE ayarlar (farklı şeyleri etkilerler):
///   SetMemoryUsageTargetLevel → BELLEK   (Chromium cache'lerini atar)
///   EcoQoS                    → CPU/GÜÇ  (belleği AZALTMAZ)
#[cfg(target_os = "windows")]
fn refresh_perf_mode(app: &tauri::AppHandle) {
    let focused = *app.state::<PerfState>().focused.lock().unwrap();
    // Kopyasını al: aşağıdaki döngü boyunca kilidi tutmayalım.
    let playing_map = app.state::<PerfState>().player_playing.lock().unwrap().clone();

    for (label, window) in app.webview_windows() {
        // TAM PERFORMANS kararı pencereye özel: yalnızca BU pencerede video
        // oynuyorsa VE uygulama odaktaysa tam bellek/CPU verilir. Başka bir
        // pencerede (örn. arkaplan tepsi oturumu) oynayan video BU pencereyi
        // tam performansa geçirmez.
        let full_perf = playing_map.get(&label).copied().unwrap_or(false) && focused;
        // BELLEK hedefi CPU kararından AYRI tutulur (ikisi farklı şeyler yapar,
        // bkz. fonksiyon başı notu).
        //
        // NEDEN: `SetMemoryUsageTargetLevel(LOW)` Chromium'a "cache'leri at"
        // demektir ve GPU kaynaklarını da bırakır. Eski kural `playing &&
        // focused` olduğu için VİDEO DURAKLATILDIĞI anda — kullanıcı ekrana
        // bakarken, örneğin oynatıcının ayar menüsünü açmak için duraklattığında
        // — webview LOW'a düşüyordu. WebGPU tabanlı oynatıcıda bu, cihazın
        // kaybına ("WebGPU: Destroyed" → "Player destroyed.") yol açabiliyor;
        // ardından sitenin ayar menüsü alt bileşeni artık null olan oynatıcıdan
        // `OFGPresets` okumaya çalışıp mount sırasında patlıyor ve alt menü
        // DOM'a hiç eklenmiyor (genişleyen bölümlerin "çalışmaması" bu).
        //
        // Artık ölçüt ODAKTA OLMAK: kullanıcı pencereye bakıyorsa duraklatmış
        // olsa bile belleği kısmayız. Tasarruf yalnızca pencere arka plandayken
        // yapılır — asıl kazanç zaten orada. EcoQoS (CPU) eski kuralda kalır;
        // o GPU durumuna dokunmaz.
        let memory_normal = focused;
        let _ = window.with_webview(move |webview| unsafe {
            use webview2_com::Microsoft::Web::WebView2::Win32::{
                ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
            };
            use windows_core::Interface;

            let controller = webview.controller();
            if Interface::as_raw(&controller).is_null() {
                return;
            }
            let core_webview = match controller.CoreWebView2() {
                Ok(c) => c,
                Err(_) => return,
            };

            // 1) Bellek hedefi
            if let Ok(wv19) = core_webview.cast::<ICoreWebView2_19>() {
                let level = if memory_normal {
                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
                } else {
                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
                };
                let _ = wv19.SetMemoryUsageTargetLevel(level);
            }

            // 2) EcoQoS — WebView2 alt süreçlerini bulmak için browser pid'i al.
            //    (Süreç ağacından gidilemez; bkz. perf_mode.rs notu.)
            let mut pid: u32 = 0;
            if core_webview.BrowserProcessId(&mut pid).is_ok() && pid != 0 {
                perf_mode::apply_eco_mode(pid, !full_perf);
            }
        });
    }
}

/// Bir pencerenin arka plan durumu — ÜÇ durumlu.
///
/// Eskiden tek bir `suspend: bool` vardı ve "video oynuyorsa hiçbir şey yapma"
/// deniyordu. Sonuç: tepsiye küçültülüp video izlenirken sayfa ÖN PLANDAKİ gibi
/// tam hızda render etmeye devam ediyordu (kimse görmediği hâlde). Ara durum
/// eklendi.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BgMode {
    /// Pencere görünür — motor tam çalışır.
    Foreground,
    /// Tepside/minimize AMA video oynuyor. Yalnızca `SetIsVisible(false)`:
    /// render/compositing/rAF durur, GPU yüzeyleri bırakılır, sayfa "hidden"
    /// sayılır (Chromium arka plan kısıtlaması devreye girer) — ama MEDYA
    /// OYNATIMI DEVAM EDER. Chromium gizli sayfalarda medyayı çalmayı sürdürür;
    /// WebView2'nin `ICoreWebView2_8`'de `IsDocumentPlayingAudio`/`IsMuted`
    /// sunmasının sebebi de budur.
    ///
    /// TrySuspend BİLEREK çağrılmaz: motoru dondurur, yani videoyu da durdurur.
    /// (Zaten aktif medya varken TrySuspend reddedilir.) `trim_working_sets` de
    /// çağrılmaz: aktif decode sürerken sayfaları RAM'den atmak sadece anında
    /// geri-sayfalama (page-in) maliyeti üretir.
    Media,
    /// Tepside/minimize ve hiçbir şey oynamıyor → tam donma.
    /// `SetIsVisible(false)` + `TrySuspend` + `EmptyWorkingSet`.
    DeepSleep,
}

/// WebView2'yi verilen arka plan durumuna geçirir.
///
/// TrySuspend YALNIZCA WebView görünmezken çalışır; bu yüzden önce SetIsVisible(false).
///
/// Son uygulanan durum PENCERE BAŞINA tutulur (PerfState.suspended).
/// YALNIZCA gerçek geçişte iş yapılır: aksi halde her focus/resized olayında
/// Resume+SetIsVisible(true) yeniden çağrılır, SetIsVisible odak olayını
/// yeniden tetikler ve `focused` sonsuza dek flap eder (EcoQoS AÇIK/KAPALI
/// spam'i + ön planda istenmeyen TrySuspend).
/// NOT: Eskiden tek global AtomicBool'du — iki pencere varken birinin
/// suspend/resume çağrısı diğerininkini "durum zaten böyle" sanıp sessizce
/// atlatıyordu. Pencere etiketine göre ayrı tutulması bu çakışmayı giderir.
#[cfg(target_os = "windows")]
fn set_background_suspend(window: &tauri::WebviewWindow, mode: BgMode) {
    let label = window.label().to_string();
    let previous = {
        let st = window.app_handle().state::<PerfState>();
        let mut map = st.suspended.lock().unwrap();
        let prev = map.get(&label).copied().unwrap_or(BgMode::Foreground);
        // Durum değişmediyse HİÇBİR ŞEY yapma — geri besleme döngüsünü kırar.
        if prev == mode {
            return;
        }
        map.insert(label.clone(), mode);
        prev
    };

    dbg_log!("[PerfMode] Arka plan modu ({}): {:?} → {:?}", label, previous, mode);

    // NOT: arka plan SÜRESİ burada tutulmaz — bu fonksiyon mod değişmediğinde
    // erken döndüğü için sayaç kaçırılırdı. Süre, pencerenin gerçek
    // görünürlüğüne göre `update_background_mode` içinde tutulur.

    let _ = window.with_webview(move |webview| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
        use webview2_com::TrySuspendCompletedHandler;
        use windows_core::Interface;

        let controller = webview.controller();
        if Interface::as_raw(&controller).is_null() {
            return;
        }
        let core = match controller.CoreWebView2() {
            Ok(c) => c,
            Err(_) => return,
        };
        let wv3 = match core.cast::<ICoreWebView2_3>() {
            Ok(w) => w,
            Err(_) => return, // Runtime çok eski — sessiz geç
        };

        match mode {
            BgMode::Foreground => {
                let _ = wv3.Resume();
                let _ = controller.SetIsVisible(true);
            }

            BgMode::Media => {
                // DeepSleep'ten geliyorsak motor DONMUŞ durumda: önce Resume,
                // yoksa video hiç oynamaz. (Ör. tepside dururken bildirimden
                // gelen bir gezinme oynatmayı başlatırsa.)
                if previous == BgMode::DeepSleep {
                    let _ = wv3.Resume();
                }
                // Görünmez yap — render durur, oynatma sürer. TrySuspend YOK.
                let _ = controller.SetIsVisible(false);
            }

            BgMode::DeepSleep => {
                let _ = controller.SetIsVisible(false);

                // TrySuspend tamamlanınca (motor gerçekten dondurulunca) working set'i
                // boşalt — böylece RAM'den atılan sayfalar hemen geri yüklenmez.
                let core_for_handler = core.clone();
                let handler = TrySuspendCompletedHandler::create(Box::new(
                    move |_errorcode, is_successful| {
                        if is_successful {
                            let mut pid: u32 = 0;
                            if core_for_handler.BrowserProcessId(&mut pid).is_ok() && pid != 0 {
                                perf_mode::trim_working_sets(pid);
                            }
                        } else {
                            // TrySuspend aktif medya/indirme varken ya da çok eski
                            // runtime'da reddedilir. SetIsVisible(false) yine de
                            // geçerlidir (render durur), ama bellek beklendiği kadar
                            // düşmez — tepsi RAM'i ölçülürken ilk bakılacak yer burası.
                            dbg_log!("[PerfMode] TrySuspend REDDEDİLDİ — motor donmadı, bellek yüksek kalabilir");
                        }
                        Ok(())
                    },
                ));
                let _ = wv3.TrySuspend(&handler);
            }
        }
    });
}

/// Webview'a arka plan (tepsi/minimize) durumunu bildirir.
///
/// JS tarafı (js/modules/background-mode.js) bunu alınca Page Visibility
/// API'sini geçersiz kılar ve kendi periyodik timer'larını duraklatır. Bu,
/// `SetIsVisible(false)`'a EK bir kattır: askıya alma başarısız olursa
/// (eski WebView2 runtime'ı, TrySuspend'i reddeden medya durumu vb.) sayfa
/// yine de arka planda olduğunu bilir ve iş üretmeyi keser.
/// NEDEN `emit_to` DEĞİL de `eval`: Tauri olay hedefleme (target) eşleşmesi
/// burada sessizce başarısız oluyordu. JS tarafındaki `event.listen(...)`
/// seçenek verilmediğinde kendini `{kind:"Any"}` hedefiyle kaydeder; Rust'taki
/// `emit_to(label, …)` ise `AnyLabel{label}` hedefiyle yayar ve Tauri'nin
/// `filter_target` eşlemesi `AnyLabel` → yalnızca `Window|Webview|
/// WebviewWindow|AnyLabel` adaylarını kabul eder — `Any` bu listede YOKTUR.
/// Yani olay hiçbir zaman teslim edilmezdi. `app.emit()` (global) teslim
/// ederdi ama bu kez sinyal İLGİSİZ pencerelere de giderdi: ön plandaki bir
/// pencere kendini gizli sanıp animasyonlarını keserdi.
///
/// `eval` tam olarak istenen webview'da çalışır, hedef eşleşmesi gerektirmez.
/// (Aynı yöntem super_notifications.rs > navigate_to içinde de kullanılıyor.)
/// `mode`: "foreground" | "media" | "hidden" — JS tarafındaki karşılığı için
/// bkz. js/modules/background-mode.js.
/// (Windows'a özel: tüm çağıranlar arka plan modu mantığının içinde ve o mantık
/// WebView2'ye özgü. cfg olmadan macOS derlemesinde dead_code uyarısı verirdi.)
#[cfg(target_os = "windows")]
fn emit_background_state(app: &tauri::AppHandle, label: &str, mode: &str) {
    let Some(window) = app.get_webview_window(label) else {
        return;
    };
    let _ = window.eval(&format!(
        "try{{window.__oaBackground&&window.__oaBackground.apply(\"{}\");}}catch(e){{}}",
        mode
    ));
}

#[cfg(target_os = "windows")]
fn js_mode_name(mode: BgMode) -> &'static str {
    match mode {
        BgMode::Foreground => "foreground",
        BgMode::Media => "media",
        BgMode::DeepSleep => "hidden",
    }
}

/// Pencerenin görünürlük + oynatma durumuna göre arka plan modunu belirler.
///
///   görünür                        → Foreground
///   (minimize|gizli) + video var   → Media      (render durur, oynatma sürer)
///   (minimize|gizli) + video yok   → DeepSleep  (tam donma)
///
/// `playing` bilgisi JS'ten gelir (player-perf.js → oa_set_player_playing) ve
/// pencere başına tutulur. Video tepsideyken BİTERSE JS `ended` olayında
/// `playing=false` bildirir; bu fonksiyon yeniden çalışır ve pencere kendiliğinden
/// Media'dan DeepSleep'e düşer — bölüm bitince RAM'in geri verilmesi bu sayede olur.
#[cfg(target_os = "windows")]
fn update_background_mode(app: &tauri::AppHandle, label: &str) {
    let Some(window) = app.get_webview_window(label) else {
        return;
    };
    let playing = app
        .state::<PerfState>()
        .player_playing
        .lock()
        .unwrap()
        .get(label)
        .copied()
        .unwrap_or(false);
    let minimized = window.is_minimized().unwrap_or(false);
    let visible = window.is_visible().unwrap_or(true);
    let hidden = minimized || !visible;

    // ARKA PLAN SÜRESİ — motor moduna DEĞİL, pencerenin gerçek görünürlüğüne
    // bağlanır (bkz. background_duration / restore_and_focus_window).
    //
    // NEDEN BURADA: sayaç önce `set_background_suspend` içindeydi ve mod
    // geçişine bakıyordu. Süper Bildirimler açıkken pencere tepside bilerek
    // `Foreground` modunda tutulduğu için (render sürsün diye) o fonksiyon
    // "mod değişmedi" deyip erken dönüyor, sayaç HİÇ BAŞLAMIYORDU: pencere
    // 66 sn tepside kalmasına rağmen geri dönüşte oturum tazeleme yenilemesi
    // tetiklenmedi. Görünürlük ölçütü mod seçiminden bağımsızdır, bu yüzden
    // doğru sinyal budur.
    {
        let st = app.state::<PerfState>();
        let mut since = st.bg_since.lock().unwrap();
        if hidden {
            // Zaten arka plandaysa sayacı SIFIRLAMA — kesintisiz süre ölçülür.
            since.entry(label.to_string()).or_insert_with(std::time::Instant::now);
        } else {
            since.remove(label);
        }
    }

    let mut mode = if !hidden {
        BgMode::Foreground
    } else if playing {
        BgMode::Media
    } else {
        BgMode::DeepSleep
    };

    // SÜPER BİLDİRİM İSTİSNASI: açıkken arka plandaki pencere RENDER ETMEYE
    // devam etmeli. Vanguard `Gateway-Token`'ı yalnızca render eden sayfada
    // tazelenebiliyor (canlı testle kanıtlandı — `Media`da 4 tam sayfa
    // yenilemesine rağmen token bir kez bile tazelenmedi, çünkü Media
    // `SetIsVisible(false)` demek ve rAF/canvas durur; Cloudflare challenge'ı
    // da tam olarak ona dayanıyor). `Foreground` modu `Resume()` +
    // `SetIsVisible(true)` uygular; pencerenin kendisi (HWND) `hide()` ile
    // gizli kaldığı için kullanıcı hiçbir şey görmez, ama motor çalışır.
    // TAKAS: TrySuspend/working-set trim uygulanamaz → tepside RAM düşmez.
    if mode != BgMode::Foreground && super_notifications::needs_live_page(app) {
        dbg_log!(
            "[PerfMode] Süper Bildirim açık → {} penceresi arka planda canlı tutuluyor (render sürüyor)",
            label
        );
        mode = BgMode::Foreground;
    }

    // LİNK AYIKLAYICI SOLVER İSTİSNASI: bu pencereler `resolve_turkanime_embed`
    // tarafından `.hide()` ile gizlenir gizlenmez bu fonksiyon (Resized/Focused
    // olayı üzerinden) otomatik çağrılıyor ve `playing=false` olduğu için
    // DeepSleep'e düşürüyordu — motor donuyor, bölüm sayfası hiç yüklenemiyordu
    // (canlı testte doğrulandı: #videodetay 8 saniyede hiç oluşmadı). Süper
    // Bildirim istisnasıyla aynı mekanizma: HWND gizli kalır, motor çalışmaya
    // devam eder.
    if mode != BgMode::Foreground && label.starts_with(LINK_SOLVER_LABEL_PREFIX) {
        dbg_log!(
            "[PerfMode] Link Ayıklayıcı solver → {} penceresi canlı tutuluyor (render sürüyor)",
            label
        );
        mode = BgMode::Foreground;
    }

    // JS'e ÖNCE haber ver: DeepSleep'te motor donduğu için sinyal sonradan
    // gönderilseydi sayfaya hiç ulaşmazdı.
    emit_background_state(app, label, js_mode_name(mode));
    set_background_suspend(&window, mode);
}

/// Pencere tepsiye gizlendiğinde çağrılır (bkz. WindowEvent::CloseRequested).
///
/// NEDEN AYRI BİR YOL: `window.hide()` Windows'ta yalnızca `ShowWindow(SW_HIDE)`
/// yapar — WM_SIZE üretmez, dolayısıyla tao `Resized` olayı YAYMAZ ve
/// `update_background_mode` hiç çağrılmazdı. Sonuç: TrySuspend + working-set
/// trim makinesi yazılı olduğu hâlde tepsi yolunda ölü koddu; WebView2 tam
/// bellekle ayakta kalıyordu. (Doğrulama: tao 0.35 `Resized`ı SADECE WM_SIZE'dan
/// üretir; tauri-runtime-wry `WindowMessage::Hide`ı tao'ya yönlendirir ve
/// wry'nin `controller.SetIsVisible` çağrısına dokunmaz.)
///
/// Askıya alma geciktirilir: JS'in `background-mode` olayını işleyip
/// animasyon/timer'larını durdurması için bir pencere bırakır. Gecikme ayrı bir
/// thread'de beklenir — ana thread'i (dolayısıyla gizleme animasyonunu) bloke etmez.
///
/// GECİKME NEDEN 1200 ms: X'e basınca sayfa artık /settings'e yönlendiriliyor
/// (bkz. CloseRequested). `TrySuspend` sayfa YÜKLENİRKEN reddedilir — 250 ms ile
/// askıya alma tam gezinmenin ortasına denk gelip sessizce başarısız oluyor,
/// motor uyanık ve bellek yüksek kalıyordu. Bu süre hafif /settings sayfasının
/// yüklenmesini bekler.
#[cfg(target_os = "windows")]
const TRAY_SUSPEND_DELAY_MS: u64 = 1_200;

#[cfg(target_os = "windows")]
fn enter_tray_background(app: &tauri::AppHandle, label: &str) {
    let playing = app
        .state::<PerfState>()
        .player_playing
        .lock()
        .unwrap()
        .get(label)
        .copied()
        .unwrap_or(false);

    // Gizlenme ANINDAKİ oynatma durumu kararı belirler: video oynuyorsa sayfa
    // "media" moduna (timer'ların çoğu durur, Discord RPC + oynatıcı bildirimi
    // sürer), oynamıyorsa "hidden" moduna (her şey durur) geçer.
    //
    // Süper Bildirimler açıkken İKİSİ DE OLMAZ: sayfaya "foreground" denir ki
    // TÜM timer'lar (özellikle 30 sn'lik Gateway-Token yansıtması) çalışmaya
    // devam etsin ve Page Visibility geçersiz kılınmasın — Vanguard challenge'ı
    // sayfanın kendini görünür sanmasına bağlı (bkz. update_background_mode).
    let js_mode = if super_notifications::needs_live_page(app) {
        "foreground"
    } else if playing {
        "media"
    } else {
        "hidden"
    };
    emit_background_state(app, label, js_mode);

    let app_c = app.clone();
    let label_c = label.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(TRAY_SUSPEND_DELAY_MS));
        // Ayrı thread'den çağrılır: is_visible()/with_webview kendi içlerinde
        // ana thread'e dispatch eder. (Ana thread'in İÇİNDEN yeniden dispatch
        // etmek kilitlenme yaratıyordu — bkz. refresh_perf_mode'daki not.)
        update_background_mode(&app_c, &label_c);
    });
}

/// Pencereyi askıdan KOŞULSUZ çıkarır (tepsi tıklaması, toast tıklaması,
/// ikinci kopya başlatma vb. tüm geri-yükleme yollarında çağrılır).
///
/// NEDEN: askıdan çıkma normalde `WindowEvent::Focused(true)` ile tetikleniyor.
/// Ama `show()` tek başına odak olayı üretmez ve `set_focus()` de pencere zaten
/// odak sahibi görünüyorsa olayı yeniden yaymayabilir. O durumda webview askıda
/// (SetIsVisible(false) + TrySuspend) kalır ve kullanıcı SİYAH/DONUK bir pencere
/// görürdü. Bu yüzden geri-yükleme yolları olaya güvenmez, doğrudan bunu çağırır.
pub(crate) fn resume_webview(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        // Arka plan sayacını AÇIKÇA sıfırla. Çağıran (restore_and_focus_window)
        // süreyi bu satırdan ÖNCE okur; burada temizlemek aynı arka plan
        // dönemine ait ikinci bir geri-yüklemenin oturum tazeleme yenilemesini
        // tekrar tetiklemesini önler.
        clear_background_since(&window.app_handle(), window.label());
        set_background_suspend(window, BgMode::Foreground);
        emit_background_state(&window.app_handle(), window.label(), "foreground");
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
    }
}

/// Pencerenin arka plan sayacını sıfırlar (pencere geri geldi).
#[cfg(target_os = "windows")]
fn clear_background_since(app: &tauri::AppHandle, label: &str) {
    // Kilit doğrudan bağlanır (dosyadaki diğer PerfState kullanımlarıyla aynı
    // biçim): `if let` kalıbı burada State ödünçlemesini kilit korumasından
    // uzun yaşatmadığı için derlenmiyor.
    let st = app.state::<PerfState>();
    let mut since = st.bg_since.lock().unwrap();
    since.remove(label);
}

/// Herhangi bir pencere ÖN PLANDA (görünür + motor tam çalışır) mı?
///
/// Vanguard `Gateway-Token`'ı yalnızca RENDER EDEN bir sayfada tazelenebiliyor
/// (Turnstile/canvas doğrulaması gerektiriyor). Bu yüzden "token tazelenebilir
/// mi" sorusunun cevabı fiilen budur — bkz. super_notifications.
pub(crate) fn any_window_foreground(app: &tauri::AppHandle) -> bool {
    #[cfg(target_os = "windows")]
    {
        let state = app.state::<PerfState>();
        let Ok(map) = state.suspended.lock() else {
            return false;
        };
        // Kayıtta hiç yoksa pencere henüz arka plana alınmamıştır → ön plan.
        app.webview_windows().keys().any(|label| {
            map.get(label).copied().unwrap_or(BgMode::Foreground) == BgMode::Foreground
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        true
    }
}

/// Pencerenin kesintisiz olarak ne kadardır arka planda (Media/DeepSleep)
/// olduğunu döndürür. Ön plandaysa None.
pub(crate) fn background_duration(
    app: &tauri::AppHandle,
    label: &str,
) -> Option<std::time::Duration> {
    #[cfg(target_os = "windows")]
    {
        app.state::<PerfState>()
            .bg_since
            .lock()
            .ok()?
            .get(label)
            .map(|t| t.elapsed())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, label);
        None
    }
}

/// Motoru SCRIPT ÇALIŞTIRABİLECEK duruma getirir — ama pencereyi GÖSTERMEZ.
///
/// NEDEN GEREKLİ: tepsideyken pencere `BgMode::DeepSleep`'tedir, yani WebView2
/// `TrySuspend` ile DONDURULMUŞTUR; bu hâldeyken `eval` çalışmaz. Oysa Vanguard
/// `Gateway-Token`'ı ~60 sn'de bir bayatlıyor ve onu tazeleyebilecek TEK yer
/// sayfanın kendisi (bkz. super_notifications::refresh_gateway_token).
///
/// `Media` modu tam olarak bu iş için biçilmiş kaftan: `Resume()` çağrılır ama
/// `SetIsVisible(false)` korunur — motor çalışır, render/compositing çalışmaz,
/// pencere görünmez. İşi biten çağıran `restore_background_mode` ile eski
/// duruma (genelde DeepSleep) geri döndürmelidir, yoksa motor uyanık kalıp
/// tepsi RAM kazancını yer.
///
/// Pencere ZATEN ön plandaysa hiçbir şey yapılmaz — onu Media'ya düşürmek
/// görünür bir pencerenin render'ını durdururdu.
pub(crate) fn wake_webview_for_script(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        let label = window.label().to_string();
        let current = window
            .app_handle()
            .state::<PerfState>()
            .suspended
            .lock()
            .unwrap()
            .get(&label)
            .copied()
            .unwrap_or(BgMode::Foreground);
        if current == BgMode::Foreground {
            return;
        }
        set_background_suspend(window, BgMode::Media);
        // JS'e de "media" de: `keepInMedia` işaretli timer'lar (token yansıtma)
        // yeniden kurulsun, yoksa motor uyanık olsa da timer'lar durmuş kalır.
        emit_background_state(&window.app_handle(), &label, "media");
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
    }
}

/// `wake_webview_for_script` ile uyandırılan pencereyi gerçek durumuna
/// (görünürlük + oynatma) göre yeniden değerlendirip uygun arka plan moduna
/// döndürür — tepsideki bir pencere için bu genelde DeepSleep'e geri dönmektir.
pub(crate) fn restore_background_mode(app: &tauri::AppHandle, label: &str) {
    #[cfg(target_os = "windows")]
    update_background_mode(app, label);
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, label);
    }
}

/// JS bildirir: oynatıcıda video oynuyor mu? `window` parametresi Tauri
/// tarafından otomatik enjekte edilir (çağıran webview) — JS tarafında
/// ayrıca göndermeye gerek yok. Bu sayede durum pencereye özel tutulur.
#[tauri::command]
fn oa_set_player_playing(playing: bool, window: tauri::WebviewWindow, app: tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    {
        let label = window.label().to_string();
        {
            let st = app.state::<PerfState>();
            let mut map = st.player_playing.lock().unwrap();
            if map.get(&label).copied().unwrap_or(false) == playing {
                return; // durum değişmedi — API'yi boşuna çağırma
            }
            map.insert(label.clone(), playing);
        }
        dbg_log!("[PerfMode] Video oynuyor ({}) = {}", label, playing);
        refresh_perf_mode(&app);

        // Oynatma durumu askıya alma kararını da etkiler: arka planda video
        // biterse (minimize + playing=false) artık askıya alınabilir; minimize
        // iken oynatma başlarsa geri döndürülmeli. Artık HER ZAMAN çağıran
        // pencerenin kendi etiketiyle — sabit "main" değil.
        update_background_mode(&app, &label);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (playing, window, app);
    }
}


const COMMON_INIT_SCRIPT: &str = concat!(
    "(function () {\nif (window.self !== window.top) {\n  let isBuilder = false;\n  try {\n    isBuilder = window.location.search.includes(\"theme_builder=true\") || sessionStorage.getItem(\"theme_builder_active\") === \"true\";\n  } catch (e) {}\n  if (!isBuilder) return;\n}\n",

    // ──────────────────────────────────────────────
    // BLOK 1: TAURI BRIDGE (UPDATED MOCKS)
    // ──────────────────────────────────────────────
    include_str!("js/modules/tauri-bridge.js"),
    "\n",

    // ──────────────────────────────────────────────
    // BLOK 1B: ARKA PLAN (TEPSİ) MODU
    // Page Visibility API'sini geçersiz kılar ve `oaBgInterval` ile kurulan
    // timer'ları duraklatır. Rust bunu `window.__oaBackground.apply(bool)`
    // ile DOĞRUDAN tetikler (bkz. emit_background_state).
    // DİĞER MODÜLLERDEN ÖNCE gelmeli: sonraki bloklar (init.js, discord-rpc,
    // super-notifications-ui, player-perf) `window.oaBgInterval`i kullanıyor.
    // ──────────────────────────────────────────────
    include_str!("js/modules/background-mode.js"),
    "\n",

    // ──────────────────────────────────────────────
    // BLOK 2: AĞ DURUMU & ÖNBELLEK & GÖRSEL BOYUTLANDIRMA
    // ──────────────────────────────────────────────
    include_str!("js/modules/networkStore.js"),
    "\n",
    "{\nconst NETWORK_CACHE_CSS = String.raw`",
    include_str!("js/modules/network-cache.css"),
    "`;\n",
    include_str!("js/modules/network-cache.js"),
    "}\n",
    "{\n",
    include_str!("js/modules/image-rightsizer.js"),
    "\n}\n",

    // ──────────────────────────────────────────────
    // BLOK 4: PENCERE & ARAYÜZ KONTROLLERİ
    // ──────────────────────────────────────────────
    "{\nconst ZOOM_MANAGER_CSS = String.raw`",
    include_str!("js/modules/zoom-manager.css"),
    "`;\n",
    include_str!("js/modules/zoom-manager.js"),
    "}\n",

    "{\nconst WINDOW_CONTROLS_CSS = String.raw`",
    include_str!("js/modules/window-controls.css"),
    "`;\n",
    include_str!("js/modules/window-controls.js"),
    "}\n",

    include_str!("js/modules/keyboard-shortcuts.js"),
    "\n",

    // ──────────────────────────────────────────────
    // BLOK 4B: HIZLI ARAMA (Ctrl+T)
    // Spotlight tarzı arama katmanı. Kısayolun KENDİSİ yukarıdaki
    // keyboard-shortcuts.js'te kayıtlı (tüm kısayollar tek yerde); bu blok
    // yalnızca katmanı ve arama mantığını sağlayıp `window.__oaQuickSearch`
    // API'sini yayımlar. Sıra önemsiz: kısayol, API'yi kayıt anında değil
    // tuşa basıldığında çağırır.
    // ──────────────────────────────────────────────
    "{\nconst QUICK_SEARCH_CSS = String.raw`",
    include_str!("js/modules/quick-search.css"),
    "`;\n",
    include_str!("js/modules/quick-search.js"),
    "}\n",

    include_str!("js/modules/link-interceptor.js"),
    "\n",
    include_str!("js/modules/fullscreen-manager.js"),
    "\n",

    // ──────────────────────────────────────────────
    // BLOK 5: DISCORD RICH PRESENCE
    // Kendi IIFE bloğu içinde, updater yok.
    // ──────────────────────────────────────────────
    "{\n",
    include_str!("js/modules/discord/state.js"),
    "\n",
    include_str!("js/modules/discord/anime-extractor.js"),
    "\n",
    include_str!("js/modules/discord/poster-fetcher.js"),
    "\n",
    include_str!("js/modules/discord/settings-ui.js"),
    "\n",
    include_str!("js/modules/discord/discord-rpc.js"),
    "\n}\n",

    // ──────────────────────────────────────────────
    // BLOK 5B: SÜPER BİLDİRİMLER
    // Ayar kartı (/settings) + Gateway-Token köprüsü (her sayfada).
    // Bildirimleri arka planda okuyup toast gösteren kısım Rust tarafında:
    // src-tauri/src/super_notifications.rs
    // ──────────────────────────────────────────────
    "{\n",
    include_str!("js/modules/super-notifications-ui.js"),
    "\n}\n",

    // ──────────────────────────────────────────────
    // BLOK 6: GÜNCELLEME ARAYÜZÜ
    // Kendi IIFE bloğu — localStorage + DOM yönetimi
    // ──────────────────────────────────────────────
    "{\n",
    include_str!("js/modules/updater-ui.js"),
    "\n}\n",

    // ──────────────────────────────────────────────
    // BLOK 6B: SÜPER AÇILIŞ (SPLASH SCREEN VARYANTLARI)
    // "Muptezel Anime" varyantının Canvas render motoru, super-opening.js'in
    // kendi IIFE'sinden de erişilebilmesi için AYNI blokta ondan önce
    // enjekte edilir (sloppy-mode function hoisting sayesinde görünür).
    // ──────────────────────────────────────────────
    "{\n",
    include_str!("js/modules/logo-animator/textures.js"),
    "\n",
    include_str!("js/modules/logo-animator/logo-animator.js"),
    "\n",
    include_str!("js/modules/super-opening.js"),
    "\n}\n",

    // ──────────────────────────────────────────────
    // BLOK 6C: WebGPU ADAPTER ALGILAMA
    // navigator.gpu.requestAdapter() ile WebGPU'nun hangi GPU'yu kullandığını
    // tespit eder ve Rust tarafına bildirir. (Super Opening'den sonra, video
    // bloklarından önce gelir — GPU seçiminin ekrandaki ilk içerikten önce
    // yapılması içindir.)
    // ──────────────────────────────────────────────
    include_str!("js/modules/webgpu-detect.js"),
    "\n",

    // ──────────────────────────────────────────────
    // BLOK 7: SAYFA KURTARMA & VİDEO İYİLEŞTİRİCİ
    // ──────────────────────────────────────────────
    include_str!("js/modules/page-recovery.js"),
    "\n",
    include_str!("js/modules/video-optimizer.js"),
    "\n",

    // Oynatıcı durumunu Rust'a bildirir (performans/verimlilik modu kararı için)
    include_str!("js/modules/player-perf.js"),
    "\n",

    // ──────────────────────────────────────────────
    // BLOK 7B: YEREL VİDEO OYNATICI (KOPYASIZ STREAM)
    // localStorage.local_video_path + port ile çalışır.
    // ──────────────────────────────────────────────
    include_str!("js/modules/local-player.js"),
    "\n",

    // ──────────────────────────────────────────────
    // BLOK 7C: YEREL KÜTÜPHANE YÖNETİMİ
    // Sidebar butonu + bölüm ekle butonu + library yönetimi
    // ──────────────────────────────────────────────
    include_str!("js/modules/local-library.js"),
    "\n",

    // ──────────────────────────────────────────────
    // BLOK 7D: DASHBOARD İYİLEŞTİRMELERİ
    // Sidebar gruplama (katlanabilir), form state hafızası (bellek-içi,
    // sahne değişiminde kaybolmaz) ve çözünürlük checkbox genişlik düzeltmesi.
    // Yalnızca /dashboard rotasında aktif olur (bkz. dosya içi route guard'ı).
    // ──────────────────────────────────────────────
    include_str!("js/modules/dashboard-enhancer.js"),
    "\n",

    // ──────────────────────────────────────────────
    // BLOK 7E: LİNK AYIKLAYICI
    // Dashboard'a kaynak site (turkanime.tv) linklerini toplayan bir sekme
    // ekler. Ağ istekleri fetch_external_html/resolve_turkanime_embed Rust
    // komutları üzerinden yapılır (turkanime CORS başlığı göndermiyor).
    // ──────────────────────────────────────────────
    "{\nconst LINK_EXTRACTOR_CSS = String.raw`",
    include_str!("js/modules/link-extractor/link-extractor.css"),
    "`;\n",
    include_str!("js/modules/link-extractor/core.js"),
    "\n",
    include_str!("js/modules/link-extractor/sources/turkanime.js"),
    "\n",
    include_str!("js/modules/link-extractor/sources/animecix.js"),
    "\n}\n",

    // ──────────────────────────────────────────────
    // BLOK 8: TEMA SİSTEMİ
    // ──────────────────────────────────────────────
    "{\n",
    "const THEME_UI_CSS = String.raw`",
    include_str!("js/modules/theme/theme-styles.css"),
    "`;\n",
    "const THEME_HIDE_CSS = String.raw`",
    include_str!("js/modules/theme/theme-hide.css"),
    "`;\n",
    include_str!("js/modules/theme/theme-core.js"),
    "\n",
    include_str!("js/modules/theme/theme-page-core.js"),
    "\n",
    include_str!("js/modules/theme/theme-styles.js"),
    "\n",
    include_str!("js/modules/theme/theme-page-render.js"),
    "\n",
    include_str!("js/modules/theme/theme-observer.js"),
    "\n}\n",

    // ──────────────────────────────────────────────
    // BLOK 9: TITLE BAR DÜZELTMESİ (sheet/modal)
    // Sabit CSS kullanılmaz — zoom-aware dinamik düzeltme
    // window-controls.js içindeki fixSheetContent() ile yapılır.
    // SADECE sheet-overlay fix'i kalıcı CSS olarak enjekte edilir.
    // ──────────────────────────────────────────────
    "(function(){\n",
    "try{\n",
    "var s=document.createElement('style');\n",
    "s.id='oa-titlebar-fix';\n",
    "s.textContent='",
    ".sheet-overlay{top:0!important;height:100vh!important;}",
    "';\n",
    "if(document.head)document.head.appendChild(s);\n",
    "else document.addEventListener('DOMContentLoaded',function(){if(document.head)document.head.appendChild(s);},{once:true});\n",
    "}catch(e){}\n",
    "})();\n",

    // ──────────────────────────────────────────────
    // BLOK 10: BAŞLATMA (EN SON ÇALIŞIR)
    // ──────────────────────────────────────────────
    include_str!("js/init.js"),
    "\n})();"
);

#[cfg(target_os = "windows")]
pub const WINDOWS_BASE_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,msTrackingPrevention --enable-features=ParallelDownloading,HardwareMediaKeyHandling,CanvasOopRasterization --enable-quic --enable-fast-unload --enable-gpu-rasterization --enable-zero-copy --enable-gpu-memory-buffer-video-frames --renderer-process-limit=1 --disk-cache-size=1073741824 --media-cache-size=536870912 --js-flags=\"--max-old-space-size=1024\" --force-gpu-selection=high-performance --force_high_performance_gpu";

/// Proxy aktifken kullanılacak browser args
#[cfg(target_os = "windows")]
pub const WINDOWS_PROXY_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,msTrackingPrevention --enable-features=ParallelDownloading,HardwareMediaKeyHandling,CanvasOopRasterization --enable-quic --enable-fast-unload --enable-gpu-rasterization --enable-zero-copy --enable-gpu-memory-buffer-video-frames --renderer-process-limit=1 --disk-cache-size=1073741824 --media-cache-size=536870912 --js-flags=\"--max-old-space-size=1024\" --force-gpu-selection=high-performance --force_high_performance_gpu --proxy-server=http://127.0.0.1:1453";

pub(crate) fn platform_user_agent() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1"
    }
    #[cfg(target_os = "macos")]
    {
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1"
    }
}

pub(crate) fn build_new_window(app: &tauri::AppHandle, url: String) -> Result<(), String> {
    dbg_log!("[Tauri] build_new_window invoked with URL: {}", url);

    let label = format!(
        "win_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );

    let user_agent = platform_user_agent();
    
    let parsed_url = url.parse::<tauri::Url>()
        .map_err(|e| format!("Invalid URL: {}", e))?;

    let app_handle = app.clone();
    let win_builder = WebviewWindowBuilder::new(
        app,
        &label,
        WebviewUrl::External(parsed_url),
    )
    .title("OpenAnime")
    .inner_size(1280.0, 848.0)
    .min_inner_size(800.0, 500.0)
    .center()
    .decorations(false)
    .zoom_hotkeys_enabled(true)
    .user_agent(user_agent)
    .on_new_window(move |new_url, _features| {
        dbg_log!(
            "[Tauri] Intercepted new window request from secondary window for URL: {}",
            new_url
        );
        let app_c = app_handle.clone();
        let url_str = new_url.to_string();
        std::thread::spawn(move || {
            if let Err(e) = build_new_window(&app_c, url_str) {
                dbg_log!("[Tauri] on_new_window -> build_new_window error: {}", e);
            }
        });
        tauri::webview::NewWindowResponse::Deny
    })
    .initialization_script(build_init_script());

    #[cfg(target_os = "windows")]
    let win_builder = win_builder.additional_browser_args(WINDOWS_PROXY_ARGS);

    match win_builder.build() {
        Ok(_) => {
            dbg_log!("[Tauri] Successfully created new window with label: {}", label);
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("[Tauri] Window build failed: {}", e);
            dbg_log!("{}", err_msg);
            Err(err_msg)
        }
    }
}

/// Son gerçek pencere kapandığında (bkz. RunEvent::ExitRequested) çağrılır.
/// Süper Bildirimler açıkken uygulamanın Discord RPC/bildirim/gateway-token
/// köprüsünü canlı tutması için sitenin `/settings` sayfasında duran, GÖRÜNMEZ
/// ve hafif (video/ağır DOM içermeyen) bir pencere açar. Tepsi ikonuna
/// tıklanınca bu pencere gösterilir; o andan itibaren sıradan bir pencere
/// gibi davranır — kapatılırsa bu fonksiyon (koşullar hâlâ sağlanıyorsa)
/// yenisini açar.
/// Tepsi oturumu pencerelerinin etiket ön eki. Etiket her açılışta BENZERSİZ
/// (zaman damgalı) üretilir — sabit bir isim (örn. hep "tray_session")
/// kullanılsaydı, az önce kapanmış aynı isimli pencere Tauri'nin iç kaydından
/// silinmeden yenisi açılmaya çalışılırsa `build()` "etiket zaten kullanımda"
/// hatasıyla SESSİZCE başarısız oluyordu — bu da arka arkaya kapat/aç
/// denemelerinde bazen tepsi oturumunun hiç açılmamasına yol açıyordu.
pub(crate) const TRAY_SESSION_LABEL_PREFIX: &str = "tray_session_";

fn maybe_spawn_tray_session(app: &tauri::AppHandle) {
    // Yarış durumu koruması: başka bir yol zaten pencere açmış olabilir.
    if !app.webview_windows().is_empty() {
        return;
    }
    dbg_log!("[Tepsi] Son pencere kapandı, hafif arkaplan oturumu açılıyor (/settings)");
    if let Err(e) = spawn_tray_session_window(app) {
        dbg_log!("[Tepsi] Arkaplan oturumu açılamadı: {}", e);
    }
}

fn spawn_tray_session_window(app: &tauri::AppHandle) -> Result<(), String> {
    let user_agent = platform_user_agent();
    let parsed_url = "https://openani.me/settings"
        .parse::<tauri::Url>()
        .map_err(|e| e.to_string())?;

    let label = format!(
        "{}{}",
        TRAY_SESSION_LABEL_PREFIX,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );

    let app_handle = app.clone();
    let win_builder = WebviewWindowBuilder::new(
        app,
        &label,
        WebviewUrl::External(parsed_url),
    )
    .title("OpenAnime")
    .inner_size(1280.0, 848.0)
    .min_inner_size(800.0, 500.0)
    .center()
    .decorations(false)
    .visible(false)
    .zoom_hotkeys_enabled(true)
    .user_agent(user_agent)
    .on_new_window(move |new_url, _features| {
        let app_c = app_handle.clone();
        let url_str = new_url.to_string();
        std::thread::spawn(move || {
            if let Err(e) = build_new_window(&app_c, url_str) {
                dbg_log!("[Tauri] tray_session on_new_window error: {}", e);
            }
        });
        tauri::webview::NewWindowResponse::Deny
    })
    .initialization_script(build_init_script());

    #[cfg(target_os = "windows")]
    let win_builder = win_builder.additional_browser_args(WINDOWS_PROXY_ARGS);

    let window = win_builder.build().map_err(|e| e.to_string())?;
    dbg_log!("[Tepsi] Hafif arkaplan oturumu oluşturuldu (label: {})", label);

    // Görünmez + oynatmıyor → hemen askıya alınabilir, açılıştan itibaren
    // minimum RAM/CPU kullanır.
    #[cfg(target_os = "windows")]
    update_background_mode(app, window.label());
    #[cfg(not(target_os = "windows"))]
    let _ = &window;

    Ok(())
}

#[tauri::command]
async fn open_new_window(app: tauri::AppHandle, url: String) -> Result<(), String> {
    build_new_window(&app, url)
}

#[tauri::command]
fn set_zoom_level(state: tauri::State<'_, ZoomState>, level: f64) -> Result<(), String> {
    let mut zoom = state.level.lock().map_err(|e| e.to_string())?;
    *zoom = level;
    dbg_log!("[Tauri] Zoom seviyesi kaydedildi: {:.0}%", level * 100.0);
    Ok(())
}

#[tauri::command]
fn get_zoom_level(state: tauri::State<'_, ZoomState>) -> Result<f64, String> {
    let zoom = state.level.lock().map_err(|e| e.to_string())?;
    Ok(*zoom)
}

#[tauri::command]
async fn reopen_with_proxy(app: tauri::AppHandle) -> Result<(), String> {
    // İSİM YANILTICI (geriye dönük uyumluluk için korundu — js/init.js,
    // js/modules/page-recovery.js ve permissions/dpi_proxy.toml bu adı
    // kullanıyor): pencereyi YENİDEN AÇMAZ, WebView'e hiç dokunmaz.
    // Yalnızca yerel proxy'nin bypass yöntemini gözden geçirir.
    //
    // Kullanıcı "sürekli F5 atılıyor" derken gördüğü yenilenmenin kaynağı bu
    // komut DEĞİL, boş ekran watchdog'unun `location.reload()` çağrısıdır
    // (js/modules/page-recovery.js). Bu komut o yenilemenin ARDINDAN
    // tetikleniyordu; sıralama yüzünden sebep gibi görünüyordu.
    dbg_log!("[Tauri] reopen_with_proxy çağrıldı (yalnızca proxy yöntemi gözden geçirilir).");
    let dpi = app.state::<dpi_proxy::DpiProxyManager>();
    dpi.request_bypass(&app).await
}

#[tauri::command]
async fn update_discord_presence(
    state: tauri::State<'_, discordRPC::DiscordState>,
    page: discordRPC::AppPage,
    metadata: Option<discordRPC::PresenceMetadata>,
    window_label: Option<String>,
) -> Result<(), String> {
    state.update(page, metadata, window_label);
    Ok(())
}

#[tauri::command]
async fn clear_discord_presence(
    state: tauri::State<'_, discordRPC::DiscordState>,
) -> Result<(), String> {
    state.clear();
    Ok(())
}

#[tauri::command]
async fn set_discord_rpc_enabled(
    state: tauri::State<'_, discordRPC::DiscordState>,
    enabled: bool,
) -> Result<(), String> {
    state.set_enabled(enabled);
    Ok(())
}

#[tauri::command]
async fn set_focused_window(
    state: tauri::State<'_, discordRPC::DiscordState>,
    label: Option<String>,
) -> Result<(), String> {
    state.set_focused_window(label);
    Ok(())
}

#[tauri::command]
async fn close_window_label(app: tauri::AppHandle, label: Option<String>) -> Result<(), String> {
    let target = label.as_deref().unwrap_or("main");
    if let Some(win) = app.get_webview_window(target) {
        win.close()
            .map_err(|e| format!("[Tauri] Pencere kapatma hatası: {}", e))?;
        dbg_log!("[Tauri] Pencere kapatıldı: {}", target);
    } else {
        dbg_log!("[Tauri] Kapatılacak pencere bulunamadı: {}", target);
    }
    Ok(())
}

// (proxy_request kaldırıldı — hiçbir JS/frontend tarafından çağrılmıyordu.)

/// `host` bir loopback/özel-ağ/link-local adresi mi? IP literal ise oktetlere
/// bakar; "localhost" gibi hostname'leri de reddeder (DNS çözümü burada
/// yapılmıyor — asıl SSRF koruması reqwest'in kendi bağlantısında değil,
/// çağıranın bariz özel adres yazmasını engellemekte).
fn is_private_or_loopback_host(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") {
        return true;
    }
    if let Ok(ip) = lower.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00) == 0xfc00
            }
        };
    }
    false
}

/// Dışarıya giden her fetch komutu için ortak kapı: yalnızca `https` şeması ve
/// yalnızca genel (özel/loopback olmayan) hostlar. Host allowlist BURADA
/// uygulanmaz — çağıranlar (ör. `fetch_external_html`) kendi allowlist'ini
/// bunun üstüne ekler.
fn is_https_and_not_private(url: &str) -> Result<url::Url, String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("Geçersiz URL: {e}"))?;
    if parsed.scheme() != "https" {
        return Err("Yalnızca https izinli".to_string());
    }
    let host = parsed.host_str().ok_or_else(|| "Host yok".to_string())?;
    if is_private_or_loopback_host(host) {
        return Err("Özel/yerel adreslere istek yasak".to_string());
    }
    Ok(parsed)
}

#[tauri::command]
async fn fetch_css(url: String) -> Result<String, String> {
    is_https_and_not_private(&url)?;

    let client = reqwest::Client::builder()
        .user_agent(platform_user_agent())
        .build()
        .map_err(|e| format!("Client build error: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Fetch error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }

    response.text().await.map_err(|e| format!("Read error: {}", e))
}

#[tauri::command]
async fn check_connection() -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build();
    if let Ok(client) = client {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let url = format!("https://openani.me/?nocache={}", now);
        
        let req = client.get(&url)
            .header("Cache-Control", "no-cache")
            .header("Pragma", "no-cache");

        if let Ok(resp) = req.send().await {
            // HERHANGİ bir HTTP yanıtı geldiyse bağlantı vardır. 401/403
            // (Cloudflare bot sayfası, Vanguard reddi) veya 5xx sunucunun
            // cevabıdır — "internet yok" demek değildir.
            // (Aynı ayrım: dpi_proxy::ConnectionResult::is_reachable.)
            dbg_log!("[Tauri] check_connection: HTTP {}", resp.status().as_u16());
            true
        } else {
            false
        }
    } else {
        false
    }
}

/// Link Ayıklayıcı — kaynak site sayfalarını Rust tarafında çeker. Tarayıcıdan
/// doğrudan fetch imkânsız: turkanime.tv hiç Access-Control-Allow-* göndermiyor.
/// Yalnızca turkanime.tv (ve alt alan adları) için host allowlist uygulanır —
/// `is_https_and_not_private` şema/private-IP kontrolünün üstüne eklenir.
#[derive(serde::Serialize)]
struct ExternalPage {
    status: u16,
    body: String,
}

fn is_allowed_link_extractor_host(host: &str) -> bool {
    host == "turkanime.tv" || host.ends_with(".turkanime.tv")
}

#[tauri::command]
async fn fetch_external_html(
    url: String,
    referer: Option<String>,
    ajax: Option<bool>,
) -> Result<ExternalPage, String> {
    let parsed = is_https_and_not_private(&url)?;
    let host = parsed.host_str().unwrap_or("");
    if !is_allowed_link_extractor_host(host) {
        return Err(format!("İzin verilmeyen host: {}", host));
    }

    let client = reqwest::Client::builder()
        .user_agent(platform_user_agent())
        .build()
        .map_err(|e| format!("Client build error: {}", e))?;

    let mut req = client.get(&url);
    if let Some(r) = referer {
        req = req.header("Referer", r);
    }
    if ajax.unwrap_or(false) {
        req = req.header("X-Requested-With", "XMLHttpRequest");
    }

    let response = req.send().await.map_err(|e| format!("Fetch error: {}", e))?;
    let status = response.status().as_u16();
    let body = response.text().await.map_err(|e| format!("Read error: {}", e))?;
    Ok(ExternalPage { status, body })
}

/// Link Ayıklayıcı — turkanime'in AES ile şifrelediği embed'lerini çözer.
/// Anahtarı reverse-engineer ETMEZ (turkanime'in embed JS paketi içerik-hash'li
/// dosya adına sahip, her derlemede değişir). Bunun yerine gerçek bir kullanıcı
/// gibi davranır:
///   1. Gizli pencereyi BÖLÜM SAYFASININ KENDİSİNE açar (düz URL, hash yok —
///      önceki tasarım embed URL'sine DOĞRUDAN top-level navigasyon yapıyordu
///      ve `#/url/<base64>` parçası bazen `#/`'ye düşüyordu; canlı testte
///      doğrulandı, kök sebep hâlâ net değil ama bölüm sayfasının hash'i yok,
///      bu riski tamamen ortadan kaldırıyor).
///   2. Sitenin KENDİ `IndexIcerik(ajaxPath, 'videodetay')` fonksiyonunu
///      çağırarak gerçek buton tıklamasını birebir simüle eder (fansub sonra
///      oynatıcı sırasıyla, `click_paths` listesi kadar).
///   3. Sonuçtaki `#videodetay .video-icerik iframe` şifreliyse, ONUN
///      `contentDocument`'ine iner (aynı origin — kullanıcının kendi canlı
///      konsol testiyle doğrulandı: erişim engellenmiyor) ve içindeki gerçek
///      `/player/<token>` linkini okur.
/// `build_init_script()` bu pencereye BİLEREK enjekte EDİLMEZ — openani.me'ye
/// özel (ör. network-cache.js window.fetch/XHR override ediyor).
pub(crate) const LINK_SOLVER_LABEL_PREFIX: &str = "oa_link_solver_";
/// Aynı milisaniyeye denk gelen paralel çözümler aynı etikete sahip olabiliyordu
/// (canlı testte doğrulandı) — zaman damgasının yanına atomik sayaç eklenir.
static LINK_SOLVER_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Aynı anda kaç solver penceresi açılabilir. Canlı testte doğrulandı: bir
/// bölümdeki tüm şifreli oynatıcılar (4+) paralel açılınca yerel DPI-atlatma
/// proxy'si (127.0.0.1:1453) + WebView2 başlatma tıkanıyor — bazı pencereler
/// #videodetay'ı bile 8 saniyede oluşturamıyordu. Kullanıcı isteğiyle TAM
/// SIRALI yapıldı (limit 1) — hem tıkanmayı önler hem de aynı anda birden
/// fazla pencerenin (kısa süreliğine de olsa) art arda görünmesi ihtimalini
/// azaltır. Sınırlama JS tarafındaki Promise.all paralelliğini bozmaz,
/// yalnızca gerçek eşzamanlı pencere sayısını kısar (fazlası kuyrukta bekler).
static LINK_SOLVER_SEMAPHORE: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
fn link_solver_semaphore() -> &'static tokio::sync::Semaphore {
    LINK_SOLVER_SEMAPHORE.get_or_init(|| tokio::sync::Semaphore::new(1))
}

/// SIRALAMANIN KANITI: log satır sırasına güvenmek yerine gerçek eşzamanlı
/// aktif pencere sayısını sayar. permit alındığı anda +1, fonksiyon
/// (herhangi bir çıkış yolundan — Drop garanti eder) bittiğinde -1. Eğer
/// sıralama gerçekten bozuksa, `active_now` bir noktada 1'i geçtiğini
/// dbg_log!'da gösterir; hiç geçmiyorsa sıralama kanıtlanmış olur (varsayım
/// değil, ölçüm).
static LINK_SOLVER_ACTIVE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
struct ActiveGuard;
impl Drop for ActiveGuard {
    fn drop(&mut self) {
        LINK_SOLVER_ACTIVE.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Solver penceresini Drop'ta KOŞULSUZ kapatır. ÖNEMLİ: dıştaki
/// `tokio::time::timeout` süresi dolunca içteki future'ı BEKLEMEDEN düşürür
/// (cancel) — o anda `win.close()`'a sıra gelmemiş olabilir, pencere sızar.
/// Bu guard, future nasıl biterse bitsin (başarı/hata/zaman aşımı/iptal)
/// pencerenin gerçekten kapanmasını garanti eder.
struct WindowCloseGuard(tauri::WebviewWindow);
impl Drop for WindowCloseGuard {
    fn drop(&mut self) {
        let _ = self.0.close();
    }
}

/// Bir tek eval_with_callback çağrısını oneshot kanalıyla Result'a çevirir —
/// tüm poll döngülerinde tekrarlanan boilerplate'i tekilleştirir.
async fn eval_once(win: &tauri::WebviewWindow, js: &str, timeout_ms: u64) -> Option<String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let tx_cb = tx.clone();
    let _ = win.eval_with_callback(js, move |r: String| {
        if let Ok(mut g) = tx_cb.lock() {
            if let Some(t) = g.take() {
                let _ = t.send(r);
            }
        }
    });
    tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), rx)
        .await
        .ok()
        .and_then(|r| r.ok())
}

const SOLVER_DIAG_JS: &str = r#"(function () {
    try {
        var cf = !!document.querySelector('#challenge-running, .cf-browser-verification, #cf-wrapper, iframe[src*="challenges.cloudflare.com"], #cf-turnstile');
        var b = document.body;
        return JSON.stringify({
            href: location.href,
            ready: document.readyState,
            title: (document.title || '').slice(0, 80),
            cf: cf,
            videodetay: !!document.querySelector('#videodetay'),
            indexIcerikFn: typeof IndexIcerik === 'function',
            bodyLen: b ? b.innerHTML.length : -1
        });
    } catch (e) { return 'DIAG_ERR:' + String(e); }
})()"#;

/// Belirli bir JS boolean ifadesi doğru olana kadar (ya da zaman aşımına kadar)
/// `eval_with_callback` ile yoklar. Sayfanın #videodetay gibi bir öğe
/// oluşturmasını beklemek için kullanılır. Her yoklamada ayrıca gerçek sayfa
/// durumunu (href/ready/başlık/Cloudflare işareti) `label` etiketiyle loglar —
/// tek bir "loaded=false" yerine bekleme boyunca NE OLDUĞU görünür kılınır.
async fn wait_for_js_condition(win: &tauri::WebviewWindow, label: &str, js_bool_expr: &str, timeout_ms: u64) -> bool {
    let probe = format!(
        "(function(){{ try {{ return !!({}); }} catch (e) {{ return false; }} }})()",
        js_bool_expr
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    let mut tick: u32 = 0;
    while std::time::Instant::now() < deadline {
        tick += 1;
        if let Some(r) = eval_once(win, &probe, 600).await {
            if r.trim() == "true" {
                return true;
            }
        }
        if tick % 4 == 0 {
            if let Some(diag) = eval_once(win, SOLVER_DIAG_JS, 600).await {
                dbg_log!("[LinkSolver] DIAG label={} tick={} => {}", label, tick, diag);
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    false
}

#[tauri::command]
async fn resolve_turkanime_embed(
    app: tauri::AppHandle,
    episode_url: String,
    click_paths: Vec<String>,
) -> Result<String, String> {
    let parsed = is_https_and_not_private(&episode_url)?;
    let host = parsed.host_str().unwrap_or("").to_string();
    if !is_allowed_link_extractor_host(&host) {
        return Err(format!("İzin verilmeyen host: {}", host));
    }

    let counter = LINK_SOLVER_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let label = format!(
        "{}{}_{}",
        LINK_SOLVER_LABEL_PREFIX,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        counter
    );

    dbg_log!("[LinkSolver] kuyrukta bekliyor: label={}", label);
    // TEK SEFERDE BİR PENCERE: canlı testte doğrulandı — birden fazla solver
    // penceresi aynı anda açılınca yerel DPI-atlatma proxy'si + WebView2
    // başlatma tıkanıyor, bazı pencereler #videodetay'ı bile oluşturamıyordu.
    // Semafor limiti 1'e indirildi (bkz. link_solver_semaphore).
    let _permit = link_solver_semaphore().acquire().await;
    let active_now = LINK_SOLVER_ACTIVE.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let _active_guard = ActiveGuard;
    if active_now > 1 {
        dbg_log!("[LinkSolver] !!! SIRALAMA BOZUK !!! label={} active_now={}", label, active_now);
    }

    // TÜM ZİNCİRE (navigasyon + #videodetay/IndexIcerik bekleme + tüm
    // tıklamalar + sonuç okuma) TEK BİR ÜST TIMEOUT uygulanır. Alt adımların
    // (15sn + N×6sn + 8sn) toplamı teorik olarak birikip belirsiz uzayabiliyordu
    // — artık bir link çözümü en fazla bu kadar sürebilir, aşılırsa hemen
    // bırakılıp kuyruktaki bir sonraki linke geçilir.
    const OVERALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
    match tokio::time::timeout(
        OVERALL_TIMEOUT,
        resolve_turkanime_embed_core(&app, &label, active_now, parsed, &episode_url, &click_paths),
    )
    .await
    {
        Ok(inner_result) => inner_result,
        Err(_) => {
            dbg_log!("[LinkSolver] ÜST ZAMAN AŞIMI ({:?}): label={} — pencere WindowCloseGuard ile kapatılacak", OVERALL_TIMEOUT, label);
            Err("Zaman aşımı: embed çözülemedi (üst limit)".to_string())
        }
    }
}

/// `resolve_turkanime_embed`'in asıl işi: pencere açma → sayfa+IndexIcerik
/// bekleme → tıklama zinciri → sonucu okuma. ÖNEMLİ: dıştaki
/// `tokio::time::timeout` süresi dolunca bu future'ı BEKLEMEDEN düşürür
/// (cancel) — bu yüzden pencere kapatma kodun içinde EXPLICIT `win.close()`
/// çağrılarıyla DEĞİL, `WindowCloseGuard`'ın Drop'uyla garanti edilir (future
/// nasıl biterse bitsin — başarı/hata/üst zaman aşımı — çalışır).
async fn resolve_turkanime_embed_core(
    app: &tauri::AppHandle,
    label: &str,
    active_now: u32,
    parsed: url::Url,
    episode_url: &str,
    click_paths: &[String],
) -> Result<String, String> {
    dbg_log!("[LinkSolver] build() öncesi: label={} active_now={} episode={} clicks={:?}", label, active_now, episode_url, click_paths);
    // GÖRÜNMEZLİK: pencere GÖRÜNÜR oluşturulur (WebView2'nin gerçekten
    // başlaması için — `.visible(false)` ile Windows'ta motor hiç
    // başlamıyordu, canlı CDP testiyle doğrulandı), hemen ardından `hide()`
    // ile gizlenir. `hide()` normalde EcoQoS'u tetikleyip motoru donduruyordu
    // (DeepSleep) — bunu `update_background_mode`'daki solver istisnası
    // (LINK_SOLVER_LABEL_PREFIX ile başlayan pencereler her zaman Foreground'da
    // tutulur) engeller; motor "hide" sonrası da çalışmaya devam eder, kullanıcı
    // hiçbir şey görmez, ekranda parlama olmaz (önceki off-screen-konum
    // denemesi WM'nin pencereyi kısa süreliğine varsayılan konumda göstermesine
    // yol açıyordu — canlı testte 2 saniyelik görünme olarak doğrulandı).
    // NOT: Reklam/analitik-budama init script'i DENENDİ, TÜM pencereler
    // #videodetay'ı hiç oluşturamadı hâle geldi — `.initialization_script()`
    // eklemek de `additional_browser_args` gibi WebView2'nin farklı bir
    // ortam/profil kullanmasına yol açıyor olabilir. GERİ ALINDI.
    let win_builder = WebviewWindowBuilder::new(app, label, WebviewUrl::External(parsed))
        .inner_size(800.0, 600.0)
        .visible(true)
        .focused(false)
        .skip_taskbar(true)
        .decorations(false)
        .user_agent(platform_user_agent());

    // ÖNEMLİ: Ana pencere ve open_new_window/spawn_tray_session_window İLE
    // AYNI proxy args'ı uygular (DPI-atlatma proxy'si olmadan turkanime.tv'ye
    // doğrudan bağlanmaya çalışıyordu — canlı testte doğrulandı).
    #[cfg(target_os = "windows")]
    let win_builder = win_builder.additional_browser_args(WINDOWS_PROXY_ARGS);

    let win = win_builder
        .build()
        .map_err(|e| format!("Solver penceresi açılamadı: {}", e))?;
    dbg_log!("[LinkSolver] build() TAMAM: label={}", label);
    // Bundan sonraki HER çıkış yolunda (return, ?, ya da dıştaki timeout'un
    // bu future'ı iptal etmesi) pencere garanti olarak kapanır.
    let _close_guard = WindowCloseGuard(win.clone());
    match win.hide() {
        Ok(()) => dbg_log!("[LinkSolver] hide() TAMAM: label={}", label),
        Err(e) => dbg_log!("[LinkSolver] hide() HATA (devam ediliyor): label={} err={}", label, e),
    }

    // `#videodetay` VE `IndexIcerik` fonksiyonunun İKİSİNİ birden bekle.
    // Canlı testte doğrulandı: #videodetay DOM'da hazır olduğunda site
    // scriptleri (jQuery + kendi JS'i) henüz yüklenmemiş olabiliyor ve ilk
    // tıklama denemesi "no-fn" dönüp sessizce kayboluyordu — sayfa varsayılan
    // oynatıcıda kalıyor, sonuç yanlış oluyordu.
    let loaded = wait_for_js_condition(
        &win,
        label,
        "document.querySelector('#videodetay') && typeof IndexIcerik === 'function'",
        15000,
    )
    .await;
    dbg_log!("[LinkSolver] sayfa+IndexIcerik bekleme sonucu: label={} loaded={}", label, loaded);
    if !loaded {
        if let Some(diag) = eval_once(&win, SOLVER_DIAG_JS, 1500).await {
            dbg_log!("[LinkSolver] SON DIAG (yükleme başarısız): label={} => {}", label, diag);
        }
        return Err("Bölüm sayfası yüklenemedi (#videodetay / IndexIcerik yok)".to_string());
    }

    for (i, path) in click_paths.iter().enumerate() {
        let path_json = serde_json::to_string(path).map_err(|e| e.to_string())?;
        let click_js = format!(
            "(function(){{ if (typeof IndexIcerik === 'function') {{ IndexIcerik({}, 'videodetay'); return 'clicked'; }} return 'no-fn'; }})()",
            path_json
        );
        let click_result = eval_once(&win, &click_js, 1000).await;
        dbg_log!("[LinkSolver] tıklama {}/{}: label={} sonuç={:?}", i + 1, click_paths.len(), label, click_result);

        // Ajax'ın #videodetay'ı yeniden kurmasını bekle. Sabit uyku yerine
        // gerçek koşul yoklanır: ara adımlarda (fansub seçimi) bir sonraki
        // IndexIcerik butonunun, son adımda ise oynatıcı iframe'inin gelmesi
        // beklenir. Böylece yavaş yanıtlarda erken devam edilip yanlış
        // sonuç okunmaz.
        let is_last = i + 1 == click_paths.len();
        let cond = if is_last {
            "document.querySelector('#videodetay .video-icerik iframe')"
        } else {
            "document.querySelector('#videodetay button[onclick*=\"IndexIcerik\"]')"
        };
        let settled = wait_for_js_condition(&win, label, cond, 6000).await;
        dbg_log!("[LinkSolver] tıklama {} sonrası bekleme: label={} settled={}", i + 1, label, settled);
    }

    // JSON.stringify ile döner — `eval_with_callback` bunu bir JS string'i
    // olarak zaten JSON'a saracağından, gövde ÇİFT JSON-kodlu gelir. Önceki
    // `trim_matches('"')` yaklaşımı iç kaçış karakterlerini (\") doğru
    // çözmüyordu — burada gerçek serde_json ile iki kez çözülür.
    const READ_JS: &str = r#"(function () {
        function j(o) { return JSON.stringify(o); }
        var ifr = document.querySelector('#videodetay .video-icerik iframe');
        if (!ifr) return j({found: false, reason: 'no-outer-iframe'});
        var src = ifr.src || '';
        if (src.indexOf('/embed/') === -1) return j({found: true, link: src, reason: 'outer-not-encrypted'});
        var doc;
        try { doc = ifr.contentDocument; } catch (e) { return j({found: false, reason: 'contentDocument-throw: ' + String(e)}); }
        if (!doc) return j({found: false, reason: 'contentDocument-null'});
        var inner = doc.querySelector('#app iframe[src]');
        if (!inner) {
            return j({
                found: false,
                reason: 'no-inner-iframe',
                innerReady: doc.readyState,
                innerTitle: doc.title,
                innerBodyLen: doc.body ? doc.body.innerHTML.length : -1
            });
        }
        return j({found: true, link: inner.src, reason: 'ok'});
    })()"#;

    #[derive(serde::Deserialize, Debug)]
    struct ReadDiag {
        found: bool,
        #[serde(default)]
        link: Option<String>,
        // Yalnızca dbg_log!'daki {:?} çıktısı için tutulur (tanı amaçlı) —
        // koddan doğrudan okunmuyor, derleyici bunu "hiç okunmuyor" sanıp
        // uyarı veriyor.
        #[allow(dead_code)]
        #[serde(default)]
        reason: Option<String>,
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    let mut result: Option<String> = None;
    let mut tick: u32 = 0;
    while std::time::Instant::now() < deadline {
        tick += 1;
        if let Some(r) = eval_once(&win, READ_JS, 1200).await {
            let inner_json: Option<String> = serde_json::from_str(&r).ok();
            let diag: Option<ReadDiag> = inner_json.as_deref().and_then(|s| serde_json::from_str(s).ok());
            dbg_log!("[LinkSolver] READ label={} tick={} raw={} diag={:?}", label, tick, r, diag);
            if let Some(d) = diag {
                if d.found {
                    if let Some(link) = d.link {
                        result = Some(link);
                        break;
                    }
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }

    result.ok_or_else(|| "Zaman aşımı: embed çözülemedi".to_string())
}

// ──────────────────────────────────────────────
// Link Ayıklayıcı — AnimeCix (animecix.tv) kaynağı
//
// turkanime'den TAMAMEN FARKLI bir mimari gerektiriyor: animecix.tv'nin
// TÜMÜ (ilk HTML dahil) Cloudflare bot-koruması arkasında — canlı testte
// `curl` "Attention Required! | Cloudflare" sayfası döndü, gerçek site hiç
// gelmedi. Bu yüzden `fetch_external_html`'in düz `reqwest` çağrısı
// animecix.tv için asla çalışmaz; turkanime'deki gibi yalnızca embed
// çözümü değil, SAYFANIN KENDİSİ de gizli bir WebviewWindow'dan geçmeli.
//
// Keşif (headful + stealth Puppeteer ile canlı doğrulandı, tahmin değil):
//   1. animecix.tv Angular SPA'sı, bölüm sayfasına girince kendi JS'i ile
//      `GET /secure/titles/{titleId}?titleId=...&seasonNumber=...&episodeNumber=...`
//      çağırıyor — dönen JSON'un `title.videos` dizisinde İLGİLİ SEZON/BÖLÜM
//      için HER fansub/çeviri grubunun `tau-video.xyz/embed/<hash>` embed
//      URL'si ve sayısal `id`'si ZATEN var. Kullanıcının her `.translator-item`
//      butonuna tek tek tıklamasına gerek YOK — bu tek çağrı hepsini veriyor.
//   2. `title.translatorPoints` (`{template_id: puan}`) ve ayrı bir
//      `GET /secure/translators` çağrısı (id → gerçek isim, ör. "Kitsune Fansub")
//      görüntü adlarını çözüyor.
//   3. Bu İKİ uç nokta da (`/secure/titles/...` ve `/secure/translators`)
//      Cloudflare korumalı — `curl` ile doğrudan denendi, ikisi de 403 döndü.
//      Ama animecix.tv'nin KENDİ sayfası içinden `fetch(..., {credentials:
//      'include'})` ile çağrıldığında (sayfa zaten CF meydan okumasını geçmiş
//      oturum çerezleriyle) ikisi de düz JSON döndürüyor. Bu yüzden bu iki
//      çağrı gizli pencerenin İÇİNDEN tetiklenir, dıştan değil.
//   4. Kullanıcının "odağın tau player olsun" dediği servis budur:
//      `tau-video.xyz`. Kritik fark: bu servisin KENDİSİ Cloudflare
//      korumasız — `tau-video.xyz/api/video/<hash>?vid=<id>` düz `curl` ile
//      bile 200 dönüyor ve gerçek 480p/720p/1080p MP4 linklerini İÇİNDE
//      ŞİFRESİZ veriyor. turkanime'deki AES çözme adımının animecix'te
//      KARŞILIĞI YOK — bu adım gizli pencereye hiç ihtiyaç duymadan, doğrudan
//      Rust'tan `reqwest` ile yapılabilir (çok daha hızlı, paralelleştirilebilir).
//
// Sonuç mimari: gizli pencere yalnızca BİR KEZ, yalnızca CF oturumunu almak
// ve iki `/secure/...` çağrısını tetiklemek için açılır; gerçek video linki
// çözümü (tau-video.xyz) pencere kapandıktan SONRA düz `reqwest` ile yapılır.
// ──────────────────────────────────────────────

fn is_allowed_animecix_host(host: &str) -> bool {
    host == "animecix.tv" || host.ends_with(".animecix.tv")
}

fn is_allowed_tau_video_host(host: &str) -> bool {
    host == "tau-video.xyz" || host.ends_with(".tau-video.xyz")
}

#[derive(serde::Deserialize, Debug)]
struct AnimecixVideoEntry {
    #[serde(default)]
    episode_num: i64,
    #[serde(default)]
    season_num: i64,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    id: i64,
    #[serde(default)]
    template: i64,
    #[serde(default)]
    quality: String,
}

#[derive(serde::Deserialize, Debug, Default)]
struct AnimecixTitleData {
    #[serde(default)]
    videos: Vec<AnimecixVideoEntry>,
    #[serde(default, rename = "translatorPoints")]
    translator_points: std::collections::HashMap<String, f64>,
}

#[derive(serde::Deserialize, Debug, Default)]
struct AnimecixTitlesResponse {
    #[serde(default)]
    title: AnimecixTitleData,
}

#[derive(serde::Deserialize, Debug)]
struct AnimecixTranslatorEntry {
    id: i64,
    #[serde(default)]
    translator: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
struct TauVideoQuality {
    label: String,
    url: String,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(serde::Deserialize, Debug)]
struct TauVideoResponse {
    #[serde(default)]
    urls: Vec<TauVideoQuality>,
}

#[derive(serde::Serialize, Clone)]
struct AnimecixQualityUrl {
    label: String,
    url: String,
    size: Option<u64>,
}

#[derive(serde::Serialize, Clone)]
struct AnimecixVideoLink {
    translator_id: i64,
    translator_name: String,
    rating: Option<f64>,
    quality: String,
    urls: Vec<AnimecixQualityUrl>,
}

/// Gizli pencere içinden tetiklenen iki `fetch()` çağrısının sonucunu
/// `window.__oaAnimecix*` globallerine yazan kickoff script'i. `eval_once`
/// senkron bir sonuç bekler — `fetch()` asenkron olduğundan sonucu doğrudan
/// döndüremez; bu yüzden turkanime'deki tıklama-sonrası-bekleme deseniyle
/// aynı yaklaşım kullanılır: kickoff başlatır, `wait_for_js_condition` sonucu
/// bekler, ayrı bir READ adımı asıl veriyi okur.
fn animecix_kickoff_js(title_id: u64, season_number: u32, episode_number: u32) -> String {
    format!(
        r#"(function(){{
            window.__oaAnimecixTitles = null;
            window.__oaAnimecixTranslators = null;
            window.__oaAnimecixError = null;
            Promise.all([
                fetch('/secure/titles/{title_id}?titleId={title_id}&seasonNumber={season}&episodeNumber={episode}&page=1&perPage=100', {{credentials: 'include'}}).then(function(r){{ return r.text(); }}),
                fetch('/secure/translators', {{credentials: 'include'}}).then(function(r){{ return r.text(); }})
            ]).then(function(results){{
                window.__oaAnimecixTitles = results[0];
                window.__oaAnimecixTranslators = results[1];
            }}).catch(function(e){{
                window.__oaAnimecixError = String(e);
            }});
            return 'started';
        }})()"#,
        title_id = title_id,
        season = season_number,
        episode = episode_number,
    )
}

const ANIMECIX_READ_JS: &str = r#"(function () {
    return JSON.stringify({
        titles: window.__oaAnimecixTitles,
        translators: window.__oaAnimecixTranslators,
        error: window.__oaAnimecixError
    });
})()"#;

#[derive(serde::Deserialize, Debug, Default)]
struct AnimecixReadResult {
    #[serde(default)]
    titles: Option<String>,
    #[serde(default)]
    translators: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[tauri::command]
async fn resolve_animecix_episode(
    app: tauri::AppHandle,
    title_id: u64,
    season_number: u32,
    episode_number: u32,
) -> Result<Vec<AnimecixVideoLink>, String> {
    let counter = LINK_SOLVER_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let label = format!(
        "{}animecix_{}_{}",
        LINK_SOLVER_LABEL_PREFIX,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        counter
    );

    dbg_log!("[LinkSolver][AnimeCix] kuyrukta bekliyor: label={}", label);
    // AYNI kuyruk (link_solver_semaphore) turkanime ile PAYLAŞILIYOR — ikisi
    // aynı anda paralel açılırsa aynı DPI-atlatma proxy'si + WebView2
    // başlatma tıkanması riski turkanime'de zaten canlı testte görülmüştü.
    let _permit = link_solver_semaphore().acquire().await;
    let active_now = LINK_SOLVER_ACTIVE.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let _active_guard = ActiveGuard;
    if active_now > 1 {
        dbg_log!("[LinkSolver][AnimeCix] !!! SIRALAMA BOZUK !!! label={} active_now={}", label, active_now);
    }

    const OVERALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
    match tokio::time::timeout(
        OVERALL_TIMEOUT,
        resolve_animecix_episode_core(&app, &label, title_id, season_number, episode_number),
    )
    .await
    {
        Ok(inner_result) => inner_result,
        Err(_) => {
            dbg_log!("[LinkSolver][AnimeCix] ÜST ZAMAN AŞIMI ({:?}): label={}", OVERALL_TIMEOUT, label);
            Err("Zaman aşımı: AnimeCix verisi alınamadı (üst limit)".to_string())
        }
    }
}

async fn resolve_animecix_episode_core(
    app: &tauri::AppHandle,
    label: &str,
    title_id: u64,
    season_number: u32,
    episode_number: u32,
) -> Result<Vec<AnimecixVideoLink>, String> {
    let episode_url = format!(
        "https://animecix.tv/titles/{}/season/{}/episode/{}",
        title_id, season_number, episode_number
    );
    let parsed = is_https_and_not_private(&episode_url)?;
    let host = parsed.host_str().unwrap_or("").to_string();
    if !is_allowed_animecix_host(&host) {
        return Err(format!("İzin verilmeyen host: {}", host));
    }

    dbg_log!("[LinkSolver][AnimeCix] build() öncesi: label={} url={}", label, episode_url);
    let win_builder = WebviewWindowBuilder::new(app, label, WebviewUrl::External(parsed))
        .inner_size(800.0, 600.0)
        .visible(true)
        .focused(false)
        .skip_taskbar(true)
        .decorations(false)
        .user_agent(platform_user_agent());
    #[cfg(target_os = "windows")]
    let win_builder = win_builder.additional_browser_args(WINDOWS_PROXY_ARGS);

    let win = win_builder
        .build()
        .map_err(|e| format!("Solver penceresi açılamadı: {}", e))?;
    dbg_log!("[LinkSolver][AnimeCix] build() TAMAM: label={}", label);
    let close_guard = WindowCloseGuard(win.clone());
    match win.hide() {
        Ok(()) => dbg_log!("[LinkSolver][AnimeCix] hide() TAMAM: label={}", label),
        Err(e) => dbg_log!("[LinkSolver][AnimeCix] hide() HATA (devam ediliyor): label={} err={}", label, e),
    }

    // Cloudflare meydan okumasının geçmesini bekle (turkanime'de #videodetay
    // beklerken burada "sayfa başlığı artık CF challenge metni değil" beklenir
    // — canlı testte "Just a moment...", "Attention Required!", "Checking your
    // browser" başlıkları görüldü).
    let loaded = wait_for_js_condition(
        &win,
        label,
        "document.readyState === 'complete' && !/just a moment|attention required|checking your browser/i.test(document.title)",
        20000,
    )
    .await;
    dbg_log!("[LinkSolver][AnimeCix] CF geçiş bekleme sonucu: label={} loaded={}", label, loaded);
    if !loaded {
        return Err("AnimeCix sayfası yüklenemedi (Cloudflare meydan okuması geçilemedi)".to_string());
    }

    let kickoff_js = animecix_kickoff_js(title_id, season_number, episode_number);
    let kickoff_result = eval_once(&win, &kickoff_js, 1500).await;
    dbg_log!("[LinkSolver][AnimeCix] kickoff sonucu: label={} => {:?}", label, kickoff_result);

    let fetched = wait_for_js_condition(
        &win,
        label,
        "window.__oaAnimecixTitles !== null || window.__oaAnimecixError !== null",
        10000,
    )
    .await;
    dbg_log!("[LinkSolver][AnimeCix] fetch bekleme sonucu: label={} fetched={}", label, fetched);
    if !fetched {
        return Err("AnimeCix veri isteği zaman aşımına uğradı".to_string());
    }

    let raw = eval_once(&win, ANIMECIX_READ_JS, 2000)
        .await
        .ok_or_else(|| "AnimeCix sonucu okunamadı".to_string())?;
    // `eval_with_callback` sonucu bir JS string'i olarak JSON'a sarar —
    // turkanime'deki READ_JS ile aynı ÇİFT JSON-kodlu gövde.
    let inner_json: String = serde_json::from_str(&raw)
        .map_err(|e| format!("AnimeCix sonucu çözülemedi (dış JSON): {}", e))?;
    let read: AnimecixReadResult = serde_json::from_str(&inner_json)
        .map_err(|e| format!("AnimeCix sonucu çözülemedi (iç JSON): {}", e))?;

    if let Some(err) = read.error {
        return Err(format!("AnimeCix fetch hatası: {}", err));
    }
    let titles_raw = read.titles.ok_or_else(|| "AnimeCix başlık verisi boş döndü".to_string())?;
    let translators_raw = read.translators.unwrap_or_default();

    let titles_resp: AnimecixTitlesResponse = serde_json::from_str(&titles_raw)
        .map_err(|e| format!("AnimeCix başlık JSON'u ayrıştırılamadı: {}", e))?;
    let translators: Vec<AnimecixTranslatorEntry> =
        serde_json::from_str(&translators_raw).unwrap_or_default();
    let translator_names: std::collections::HashMap<i64, String> = translators
        .into_iter()
        .map(|t| {
            let name = if !t.translator.is_empty() {
                t.translator
            } else {
                t.name.unwrap_or_else(|| format!("Çevirmen #{}", t.id))
            };
            (t.id, name)
        })
        .collect();

    let season_i = season_number as i64;
    let episode_i = episode_number as i64;
    let matching: Vec<&AnimecixVideoEntry> = titles_resp
        .title
        .videos
        .iter()
        .filter(|v| {
            v.season_num == season_i
                && v.episode_num == episode_i
                && v.kind == "embed"
                && v.url.starts_with("https://tau-video.xyz/embed/")
        })
        .collect();
    dbg_log!(
        "[LinkSolver][AnimeCix] eşleşen video girdisi: label={} count={}",
        label,
        matching.len()
    );

    // Pencere artık gerekli değil — tau-video.xyz çözümü düz reqwest ile,
    // Cloudflare'e hiç takılmadan yapılır. `close_guard` fonksiyon dönüşünde
    // zaten kapatacaktı; burada erken bırakmak solver kuyruğunu daha hızlı
    // boşaltır (bir sonraki bölüm/çağrı beklemeden başlayabilir).
    drop(close_guard);

    let client = reqwest::Client::builder()
        .user_agent(platform_user_agent())
        .build()
        .map_err(|e| format!("Client build error: {}", e))?;

    let mut out: Vec<AnimecixVideoLink> = Vec::new();
    for entry in matching {
        let hash = entry.url.trim_start_matches("https://tau-video.xyz/embed/");
        let tau_url = format!("https://tau-video.xyz/api/video/{}?vid={}", hash, entry.id);
        let tau_parsed = match is_https_and_not_private(&tau_url) {
            Ok(u) => u,
            Err(e) => {
                dbg_log!("[LinkSolver][AnimeCix] tau URL reddedildi: {} err={}", tau_url, e);
                continue;
            }
        };
        if !is_allowed_tau_video_host(tau_parsed.host_str().unwrap_or("")) {
            continue;
        }
        let resp = match client.get(tau_url.clone()).send().await {
            Ok(r) => r,
            Err(e) => {
                dbg_log!("[LinkSolver][AnimeCix] tau fetch hatası: {} err={}", tau_url, e);
                continue;
            }
        };
        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                dbg_log!("[LinkSolver][AnimeCix] tau body okunamadı: {} err={}", tau_url, e);
                continue;
            }
        };
        let tau_data: TauVideoResponse = match serde_json::from_str(&text) {
            Ok(d) => d,
            Err(e) => {
                dbg_log!("[LinkSolver][AnimeCix] tau JSON ayrıştırılamadı: {} err={}", tau_url, e);
                continue;
            }
        };
        if tau_data.urls.is_empty() {
            continue;
        }
        let translator_name = translator_names
            .get(&entry.template)
            .cloned()
            .unwrap_or_else(|| format!("Çevirmen #{}", entry.template));
        let rating = titles_resp
            .title
            .translator_points
            .get(&entry.template.to_string())
            .copied();
        out.push(AnimecixVideoLink {
            translator_id: entry.template,
            translator_name,
            rating,
            quality: entry.quality.clone(),
            urls: tau_data
                .urls
                .into_iter()
                .map(|u| AnimecixQualityUrl { label: u.label, url: u.url, size: u.size })
                .collect(),
        });
    }

    if out.is_empty() {
        return Err("Bu bölüm için çözülebilir video bulunamadı".to_string());
    }
    Ok(out)
}

/// AnimeCix sezon sayfası (`/titles/{id}/season/{n}`) DOĞRUDAN o sezonun tüm
/// bölüm kartlarını (`.episode-card-container`) DOM'a basıyor — canlı testte
/// doğrulandı, `seasonNumber` API parametresiyle `episodes[]` denendi ama
/// SÜREKLİ boş/alakasız veri döndü (başka bir title'ın sezon verisiyle
/// kirlenmiş görünüyordu). API'ye güvenmek yerine turkanime'deki gibi gerçek
/// DOM'dan okunur: her karttaki `<img class="episode-poster" alt="...">`
/// temiz bölüm başlığını, en yakın `<a href="/titles/{id}/season/{n}/episode/{e}">`
/// de bölüm URL'sini verir.
#[derive(serde::Serialize, Clone)]
struct AnimecixEpisodeRef {
    episode_number: u32,
    title: String,
    url: String,
}

#[derive(serde::Deserialize, Debug)]
struct AnimecixEpisodeCard {
    episode_number: u32,
    #[serde(default)]
    title: String,
    #[serde(default)]
    href: String,
}

const ANIMECIX_EPISODE_LIST_JS: &str = r#"(function () {
    var cards = Array.prototype.slice.call(document.querySelectorAll('.episode-card-container'));
    var out = [];
    cards.forEach(function (c) {
        var a = c.closest('a') || c.querySelector('a');
        var href = a ? a.getAttribute('href') : null;
        if (!href) return;
        var m = /\/episode\/(\d+)/.exec(href);
        if (!m) return;
        var img = c.querySelector('img.episode-poster');
        var title = img ? (img.getAttribute('alt') || '') : '';
        out.push({ episode_number: parseInt(m[1], 10), title: title, href: href });
    });
    return JSON.stringify(out);
})()"#;

#[tauri::command]
async fn list_animecix_season_episodes(
    app: tauri::AppHandle,
    title_id: u64,
    season_number: u32,
) -> Result<Vec<AnimecixEpisodeRef>, String> {
    let counter = LINK_SOLVER_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let label = format!(
        "{}animecixlist_{}_{}",
        LINK_SOLVER_LABEL_PREFIX,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        counter
    );

    dbg_log!("[LinkSolver][AnimeCix] bölüm listesi kuyrukta bekliyor: label={}", label);
    let _permit = link_solver_semaphore().acquire().await;
    let active_now = LINK_SOLVER_ACTIVE.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let _active_guard = ActiveGuard;
    if active_now > 1 {
        dbg_log!("[LinkSolver][AnimeCix] !!! SIRALAMA BOZUK !!! label={} active_now={}", label, active_now);
    }

    const OVERALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
    match tokio::time::timeout(
        OVERALL_TIMEOUT,
        list_animecix_season_episodes_core(&app, &label, title_id, season_number),
    )
    .await
    {
        Ok(inner_result) => inner_result,
        Err(_) => {
            dbg_log!("[LinkSolver][AnimeCix] bölüm listesi ÜST ZAMAN AŞIMI: label={}", label);
            Err("Zaman aşımı: AnimeCix bölüm listesi alınamadı".to_string())
        }
    }
}

async fn list_animecix_season_episodes_core(
    app: &tauri::AppHandle,
    label: &str,
    title_id: u64,
    season_number: u32,
) -> Result<Vec<AnimecixEpisodeRef>, String> {
    let season_url = format!("https://animecix.tv/titles/{}/season/{}", title_id, season_number);
    let parsed = is_https_and_not_private(&season_url)?;
    let host = parsed.host_str().unwrap_or("").to_string();
    if !is_allowed_animecix_host(&host) {
        return Err(format!("İzin verilmeyen host: {}", host));
    }

    dbg_log!("[LinkSolver][AnimeCix] bölüm listesi build() öncesi: label={} url={}", label, season_url);
    let win_builder = WebviewWindowBuilder::new(app, label, WebviewUrl::External(parsed))
        .inner_size(800.0, 600.0)
        .visible(true)
        .focused(false)
        .skip_taskbar(true)
        .decorations(false)
        .user_agent(platform_user_agent());
    #[cfg(target_os = "windows")]
    let win_builder = win_builder.additional_browser_args(WINDOWS_PROXY_ARGS);

    let win = win_builder
        .build()
        .map_err(|e| format!("Solver penceresi açılamadı: {}", e))?;
    let _close_guard = WindowCloseGuard(win.clone());
    let _ = win.hide();

    let loaded = wait_for_js_condition(
        &win,
        label,
        "document.readyState === 'complete' && !/just a moment|attention required|checking your browser/i.test(document.title)",
        20000,
    )
    .await;
    if !loaded {
        return Err("AnimeCix sezon sayfası yüklenemedi (Cloudflare meydan okuması geçilemedi)".to_string());
    }

    let has_cards = wait_for_js_condition(
        &win,
        label,
        "document.querySelectorAll('.episode-card-container').length > 0",
        15000,
    )
    .await;
    if !has_cards {
        return Err("Bu sezon için bölüm kartı bulunamadı".to_string());
    }

    let raw = eval_once(&win, ANIMECIX_EPISODE_LIST_JS, 2000)
        .await
        .ok_or_else(|| "Bölüm listesi okunamadı".to_string())?;
    let cards: Vec<AnimecixEpisodeCard> = serde_json::from_str(&raw)
        .map_err(|e| format!("Bölüm listesi JSON'u ayrıştırılamadı: {}", e))?;

    let mut out: Vec<AnimecixEpisodeRef> = cards
        .into_iter()
        .map(|c| AnimecixEpisodeRef {
            episode_number: c.episode_number,
            title: if c.title.is_empty() {
                format!("{}. Bölüm", c.episode_number)
            } else {
                c.title
            },
            url: format!(
                "https://animecix.tv/titles/{}/season/{}/episode/{}",
                title_id, season_number, c.episode_number
            ),
        })
        .collect();
    out.sort_by_key(|e| e.episode_number);
    out.dedup_by_key(|e| e.episode_number);

    if out.is_empty() {
        return Err("Bu sezon için bölüm bulunamadı".to_string());
    }
    Ok(out)
}

/// Link Ayıklayıcı — bir oynatıcı linkinin gerçekten erişilebilir olup
/// olmadığını sunucu tarafında (reqwest ile) kontrol eder. Önceki denemedeki
/// `fetch(url, { mode: "no-cors" })` yöntemi promise'i HER ZAMAN başarıyla
/// çözdüğü için yanıltıcıydı (kırık linkler bile "çalışıyor" gösteriyordu) —
/// burada gerçek bir HTTP durum kodu okunur. Host allowlist YOK: çözülen
/// linkler rastgele CDN'lerden gelir (dood.watch, voe.sx, media.cm, vb.),
/// bu yüzden `fetch_css` ile aynı gerekçeyle yalnızca şema/private-IP
/// kısıtı uygulanır.
#[derive(serde::Serialize)]
struct LinkStatus {
    ok: bool,
    status: Option<u16>,
    error: Option<String>,
}

#[tauri::command]
async fn check_link_status(url: String, referer: Option<String>) -> Result<LinkStatus, String> {
    is_https_and_not_private(&url)?;

    let client = reqwest::Client::builder()
        .user_agent(platform_user_agent())
        .timeout(std::time::Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| format!("Client build error: {}", e))?;

    let mut head_req = client.head(&url);
    if let Some(r) = &referer {
        head_req = head_req.header("Referer", r.clone());
    }
    if let Ok(resp) = head_req.send().await {
        let status = resp.status();
        // Bazı CDN'ler HEAD desteklemiyor (405/501) — bu durumda küçük bir
        // GET (Range ile, video gövdesini indirmeden) dener.
        if status.as_u16() != 405 && status.as_u16() != 501 {
            return Ok(LinkStatus {
                ok: status.is_success() || status.is_redirection(),
                status: Some(status.as_u16()),
                error: None,
            });
        }
    }

    let mut get_req = client.get(&url).header("Range", "bytes=0-1023");
    if let Some(r) = &referer {
        get_req = get_req.header("Referer", r.clone());
    }
    match get_req.send().await {
        Ok(resp) => {
            let status = resp.status();
            Ok(LinkStatus {
                ok: status.is_success() || status.is_redirection(),
                status: Some(status.as_u16()),
                error: None,
            })
        }
        Err(e) => Ok(LinkStatus { ok: false, status: None, error: Some(e.to_string()) }),
    }
}

#[tauri::command]
async fn go_online(window: tauri::WebviewWindow) -> Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let url_str = format!("https://openani.me/?nocache={}", now);
    dbg_log!("[Tauri] Navigating online to: {}", url_str);
    let parsed_url = url_str.parse::<tauri::Url>()
        .map_err(|e| format!("Failed to parse online URL: {}", e))?;
    window.navigate(parsed_url)
        .map_err(|e| format!("Navigation failed: {}", e))
}

#[tauri::command]
async fn go_offline(window: tauri::WebviewWindow) -> Result<(), String> {
    let url = if cfg!(debug_assertions) {
        "http://localhost:1420/".to_string()
    } else {
        "tauri://localhost/".to_string()
    };
    dbg_log!("[Tauri] Navigating offline to: {}", url);
    window.navigate(url.parse().map_err(|e| format!("{}", e))?)
        .map_err(|e| format!("Failed to navigate offline: {}", e))
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
struct ThemeMeta {
    name: String,
    author: String,
    version: String,
    description: String,
    #[serde(rename = "preview_color")]
    preview_color: String,
    #[serde(rename = "created_at")]
    created_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
struct ThemeJson {
    #[serde(rename = "$schema")]
    schema: String,
    meta: ThemeMeta,
    colors: std::collections::HashMap<String, String>,
    typography: std::collections::HashMap<String, String>,
    background: serde_json::Value,
    effects: serde_json::Value,
    #[serde(default)]
    custom_css: String,
}

#[tauri::command]
async fn list_themes(app: tauri::AppHandle) -> Result<Vec<ThemeJson>, String> {
    let local_data = app.path().app_local_data_dir()
        .map_err(|e| format!("Failed to get app local data dir: {}", e))?;
    let themes_dir = local_data.join("themes");
    if !themes_dir.exists() {
        return Ok(Vec::new());
    }
    
    let mut themes = Vec::new();
    let entries = std::fs::read_dir(themes_dir)
        .map_err(|e| format!("Failed to read themes dir: {}", e))?;
    
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Ok(file_content) = std::fs::read_to_string(&path) {
                    if let Ok(theme) = serde_json::from_str::<ThemeJson>(&file_content) {
                        themes.push(theme);
                    }
                }
            }
        }
    }
    
    themes.sort_by(|a, b| b.meta.created_at.cmp(&a.meta.created_at));
    Ok(themes)
}

// (save_theme / delete_theme kaldırıldı — tema kaydetme/silme frontend'te
// henüz yok; list_themes/load_theme/apply_theme_css kullanımda ve korunuyor.)

#[tauri::command]
async fn load_theme(app: tauri::AppHandle, name: String) -> Result<ThemeJson, String> {
    let local_data = app.path().app_local_data_dir()
        .map_err(|e| format!("Failed to get app local data dir: {}", e))?;
    let themes_dir = local_data.join("themes");
    
    let safe_name = name.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>();
    let file_path = themes_dir.join(format!("{}.json", safe_name));
    
    if !file_path.exists() {
        return Err(format!("Theme {} does not exist", name));
    }
    
    let file_content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read theme file: {}", e))?;
    
    let theme = serde_json::from_str::<ThemeJson>(&file_content)
        .map_err(|e| format!("Failed to parse theme: {}", e))?;
    
    Ok(theme)
}

#[tauri::command]
async fn apply_theme_css(app: tauri::AppHandle, theme_id: String, css: String) -> Result<(), String> {
    use tauri::Emitter;
    dbg_log!("[Tauri] Emitting theme-apply for theme: {}", theme_id);
    app.emit("openanime://theme-apply", serde_json::json!({
        "themeId": theme_id,
        "css": css
    })).map_err(|e| format!("Failed to emit event: {}", e))?;
    Ok(())
}

/// JS hata köprüsü: webview içindeki console.error/warn, window.onerror ve
/// unhandledrejection mesajlarını terminal/session loguna akıtır — sahadaki
/// "sessiz" web tarafı çökmelerinin faili böyle yakalanır. Oturum başına
/// 300 mesajla sınırlıdır (flood koruması).
#[tauri::command]
fn oa_js_log(level: String, msg: String) {
    static COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if n < 300 {
        let mut m = msg;
        m.truncate(1024);
        dbg_log!("[JS {}] {}", level, m);
        if n == 299 {
            dbg_log!("[JS] mesaj limiti (300) doldu — sonraki JS logları bastırılıyor");
        }
    }
}

#[cfg(target_os = "windows")]
fn setup_windows_gpu_preference() {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_str) = exe_path.to_str() {
            dbg_log!(
                "[Tauri] Setting DirectX GpuPreference to High Performance for: {}",
                exe_str
            );
            let mut cmd = std::process::Command::new("reg");
            cmd.args(&[
                "add",
                "HKCU\\Software\\Microsoft\\DirectX\\UserGpuPreferences",
                "/v",
                exe_str,
                "/t",
                "REG_SZ",
                "/d",
                "GpuPreference=2;",
                "/f",
            ]);
            // Konsol penceresi açılmasını engelle
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            let _ = cmd.output();
        }
    }
}

/// ────────────────────────────────────────────────────────────
/// 🎥 Local Video Server Komutları
/// ────────────────────────────────────────────────────────────
/// `get_local_video_port` — Server'ın dinlediği port'u döndürür
/// (register_local_video kaldırıldı — çağıran yoktu; eşleme server
///  tarafında yönetiliyor.)
/// ────────────────────────────────────────────────────────────

#[tauri::command]
fn get_local_video_port(state: tauri::State<'_, Arc<local_video_server::LocalVideoState>>) -> Result<u16, String> {
    let port = state.port.lock().map_err(|e| e.to_string())?;
    Ok(*port)
}

/// "super_logo" varyantının olası dosya adları — SADECE MP4.
const SUPER_LOGO_MEDIA_NAMES: &[(&str, &str)] = &[
    ("superlogo.mp4", "video/mp4"),
    ("super_logo.mp4", "video/mp4"),
];

/// "muptezel_anime" varyantı artık bir dosyaya dayanmıyor — açılışta
/// js/modules/logo-animator/logo-animator.js ile Canvas üzerinde gerçek
/// zamanlı render ediliyor (bkz. super-opening.js). Bu yüzden medya adı
/// eşlemesi tek varyanta (super_logo) indirgendi.
fn media_names_for_variant(_variant: &str) -> &'static [(&'static str, &'static str)] {
    SUPER_LOGO_MEDIA_NAMES
}

/// Verilen dosya adı/MIME listesindeki ilk mevcut dosyayı diskte arar.
/// Sıra: (1) Tauri resource dizini (paketlenmiş NSIS kurulumunda asıl yer —
/// bkz. tauri.conf.json > bundle.resources), (2) cwd/exe göreli tahminler
/// (dev ortamı / portable kullanım için yedek).
fn resolve_super_opening_video_path(
    app: &tauri::AppHandle,
    media_names: &'static [(&'static str, &'static str)],
) -> Option<(std::path::PathBuf, &'static str)> {
    let mut candidates: Vec<(std::path::PathBuf, &'static str)> = Vec::new();

    for (name, mime) in media_names {
        if let Ok(resource_path) = app.path().resolve(*name, tauri::path::BaseDirectory::Resource) {
            candidates.push((resource_path, mime));
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        for (name, mime) in media_names {
            candidates.push((cwd.join("static").join(name), mime));
            candidates.push((cwd.join("static").join("openings").join(name), mime));
        }

        if let Some(parent) = cwd.parent() {
            for (name, mime) in media_names {
                candidates.push((parent.join("static").join(name), mime));
                candidates.push((parent.join("static").join("openings").join(name), mime));
            }
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        let mut cur = exe_path.parent();
        for _ in 0..4 {
            if let Some(dir) = cur {
                for (name, mime) in media_names {
                    candidates.push((dir.join("static").join(name), mime));
                    candidates.push((dir.join(name), mime));
                }
                cur = dir.parent();
            } else {
                break;
            }
        }
    }

    candidates.into_iter().find(|(p, _)| p.exists())
}

/// ARTIK KULLANILMIYOR (bkz. get_super_opening_video_data): `openani.me`
/// gibi PUBLIC bir HTTPS sayfasından `127.0.0.1`'e `<video src>` isteği,
/// Chromium/WebView2'nin Private Network Access korumasına takılıp
/// SESSİZCE engelleniyordu — video hiç görünmüyordu. Komut geriye dönük
/// uyumluluk için duruyor, super-opening.js artık bunu çağırmıyor.
#[tauri::command]
fn get_super_opening_video_url(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<local_video_server::LocalVideoState>>,
) -> Result<String, String> {
    let port = *state.port.lock().map_err(|e| e.to_string())?;
    if port == 0 {
        return Err("Local video server port not ready".to_string());
    }

    let (path, _mime) = resolve_super_opening_video_path(&app, SUPER_LOGO_MEDIA_NAMES)
        .ok_or_else(|| "No super opening video file found on disk".to_string())?;
    let path_str = path.to_string_lossy().to_string();
    let encoded = percent_encoding::utf8_percent_encode(&path_str, percent_encoding::NON_ALPHANUMERIC).to_string();
    Ok(format!("http://127.0.0.1:{}/local-video?path={}", port, encoded))
}

#[derive(serde::Serialize)]
struct SuperOpeningMedia {
    data: String,
    mime: String,
}

/// Süper Açılış MP4 videosunu (super_logo varyantı) doğrudan Tauri IPC
/// üzerinden (ağ isteği YOK) JS'e base64 olarak taşır. `muptezel_anime`
/// varyantı bu komutu hiç çağırmaz — o Canvas ile render edilir.
/// `get_super_opening_video_url`'in aksine bu bir HTTP isteği değil — bu
/// yüzden Private Network Access / mixed-content gibi tarayıcı
/// korumalarına hiç takılmaz. JS tarafı base64'ü dönen `mime` tipine göre
/// Blob'a çevirip `<video>` öğesine verir.
#[tauri::command]
fn get_super_opening_video_data(app: tauri::AppHandle, variant: String) -> Result<SuperOpeningMedia, String> {
    use base64::Engine;

    let (path, mime) = resolve_super_opening_video_path(&app, media_names_for_variant(&variant))
        .ok_or_else(|| "No super opening video file found on disk".to_string())?;

    let bytes = std::fs::read(&path).map_err(|e| {
        format!("Video dosyası okunamadı ({}): {}", path.to_string_lossy(), e)
    })?;

    dbg_log!(
        "[Süper Açılış] Medya IPC ile taşınıyor: {} ({} bayt, {})",
        path.to_string_lossy(),
        bytes.len(),
        mime
    );

    Ok(SuperOpeningMedia {
        data: base64::engine::general_purpose::STANDARD.encode(&bytes),
        mime: mime.to_string(),
    })
}

/// ────────────────────────────────────────────────────────────
/// Dosya Seçme Dialogu
/// ────────────────────────────────────────────────────────────
/// Kullanıcının işletim sistemi dosya seçme dialogu ile MP4
/// dosyası seçmesini sağlar. Seçilen dosyanın tam yolunu döndürür.
/// ────────────────────────────────────────────────────────────
#[tauri::command]
async fn pick_mp4_file() -> Result<String, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Yerel Video Dosyası Seç")
        .add_filter(
            "Video Dosyaları",
            &[
                "mp4", "mkv", "webm", "avi", "mov", "wmv", "flv", "m4v", "3gp", "ogv", "mpg",
                "mpeg", "ts", "m2ts", "mts", "m3u8",
            ],
        )
        .pick_file()
        .await
        .ok_or_else(|| "Kullanıcı dosya seçmedi".to_string())?;
    
    let path = file.path().to_string_lossy().to_string();
    dbg_log!("[LocalLibrary] Seçilen dosya: {}", path);
    Ok(path)
}

/// ────────────────────────────────────────────────────────────
/// 📄 Dosyanın İlk N Baytını Oku
/// ────────────────────────────────────────────────────────────
/// IndexedDB'ye yazılacak dummy blob için dosyanın sadece ilk
/// 100KB'ını okur. Bu sayede Svelte player geçerli bir MP4
/// başlığı görür ve sağlam initialize olur. Asıl video stream
/// local-player.js ile Rust HTTP server'dan gelir.
/// ────────────────────────────────────────────────────────────
#[tauri::command]
async fn read_file_head(path: String, max_bytes: u32) -> Result<Vec<u8>, String> {
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(&path)
        .await
        .map_err(|e| format!("Dosya açılamadı: {}", e))?;

    let max = max_bytes.min(5_242_880) as usize; // max 5MB güvenlik limiti
    let mut buffer = vec![0u8; max];
    let n = file
        .read(&mut buffer)
        .await
        .map_err(|e| format!("Dosya okunamadı: {}", e))?;

    buffer.truncate(n);
    Ok(buffer)
}

/// Panic mesajı + backtrace'i hem session log'a hem de kalıcı bir crash
/// dosyasına yazar; "uygulama sessizce çöküyor" raporları böylece kanıtlı gelir.
fn install_crash_logger() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let report = format!(
            "===== OPENANIME PANIC =====\n{}\n\nBacktrace:\n{}\n",
            info, backtrace
        );
        dbg_log!("{}", report);

        let crash_path = dirs_cache_path().join("crash.log");
        if let Some(parent) = crash_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&crash_path, &report);

        default_hook(info);
    }));
}

/// ~/.cache/openanime (veya platform eşdeğeri; bulunamazsa temp dizini).
fn dirs_cache_path() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    let base = std::env::var("LOCALAPPDATA").map(std::path::PathBuf::from).ok();
    #[cfg(not(target_os = "windows"))]
    let base = std::env::var("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .ok()
        .or_else(|| std::env::var("HOME").ok().map(|h| std::path::PathBuf::from(h).join(".cache")));

    base.unwrap_or_else(std::env::temp_dir).join("openanime")
}

/// Uygulama hiç açılamadan ölürse kullanıcıya ne yapacağını söyleyen native diyalog.
fn show_fatal_startup_error(err: &dyn std::fmt::Display) {
    let message = format!(
        "OpenAnime başlatılamadı / OpenAnime failed to start:\n\n{}\n",
        err
    );

    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("OpenAnime")
        .set_description(&message)
        .show();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_crash_logger();

    #[cfg(target_os = "windows")]
    {
        setup_windows_gpu_preference();
        let app_id = "com.openanime.desktop";
        let wide_id: Vec<u16> = app_id
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(wide_id.as_ptr());
        }
    }

    let local_video_state = Arc::new(local_video_server::LocalVideoState::new());

    // Local video server'ı hemen başlat (arka plan thread)
    let lv_state = local_video_state.clone();
    if let Ok(port) = local_video_server::start_server(&lv_state) {
        dbg_log!("[LocalVideo] Server başlatıldı: 127.0.0.1:{}", port);
    } else {
        dbg_log!("[LocalVideo] Server başlatılamadı!");
    }
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // İkinci bir kopya başlatılmaya çalışıldığında (örn. hızlı ardışık
            // çift tık) yeni bir process başlatmak yerine mevcut pencereyi
            // (tepsideyse bile) öne getiririz. Tek koruma bu olmadığında iki
            // ayrı process yarışıyor ve sonunda iki pencere birden açılıyordu.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                // Tepsideyken webview askıya alınmış olabilir — geri döndür,
                // yoksa öne gelen pencere boş görünür.
                resume_webview(&window);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(discordRPC::DiscordState::new())
        .manage(updater::UpdaterState::new())
        .manage(ZoomState::default())
        .manage(PerfState::default())
        .manage(super_notifications::SuperNotifState::new())
        .manage(local_video_state)
        .manage(gpu_info::GpuState::default());

    // DPI Proxy manager'ı oluştur (setup'tan önce olmalı)
    // .manage()'i setup'tan sonra kullanacağız

    let builder = builder.setup(|app| {
        // Logger'ı en başta başlat
        logger::init(app.handle());

        log!("[OpenAnime] Başlatılıyor…");
        dbg_log!("[Setup] Build modu: {}", if cfg!(debug_assertions) { "DEBUG" } else { "RELEASE" });
        dbg_log!("[Setup] Platform: {}", std::env::consts::OS);

        // DPI Proxy manager'ı başlat
        let app_handle = app.handle().clone();
        let dpi_manager = dpi_proxy::DpiProxyManager::new(&app_handle);
        app.manage(dpi_manager);
        let user_agent = platform_user_agent();

        // Dakikada bir RAM/CPU/uyku-durumu raporu (bkz. perf_report.rs).
        // Sadece gözlem — hiçbir davranışı etkilemez.
        #[cfg(target_os = "windows")]
        {
            let app_handle_for_perf = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    perf_report::report(&app_handle_for_perf);
                }
            });
        }

        // DPI proxy'yi en baştan başlat (arkaplan) - Windows için 3 adımlı bağlantı doğrulama akışı
        #[cfg(target_os = "windows")]
        {
            let dpi = app.state::<dpi_proxy::DpiProxyManager>();
            let method_id = {
                let settings = tauri::async_runtime::block_on(async { dpi.settings.lock().await });
                settings.active_method_id.unwrap_or(0) // Default to 0 (Direct) or 1 if none
            };
            dbg_log!("[Setup] Yerel proxy sunucusu başlatılıyor (yöntem #{})...", method_id);
            let _ = tauri::async_runtime::block_on(async {
                dpi.start_proxy(&app_handle, method_id).await
            });
        }

        #[cfg(target_os = "windows")]
        let (browser_args, proxy_status_msg) = (WINDOWS_PROXY_ARGS, "Proxy AKTİF (127.0.0.1:1453)");

        #[cfg(not(target_os = "windows"))]
        let proxy_status_msg = "Proxy DEVADIŞI";

        dbg_log!("[Setup] WebView modu: {}", proxy_status_msg);

        #[cfg(target_os = "windows")]
        let app_handle_for_check = app_handle.clone();
        #[cfg(target_os = "windows")]
        tauri::async_runtime::spawn(async move {
            let dpi = app_handle_for_check.state::<dpi_proxy::DpiProxyManager>();
            dbg_log!("[Setup Background] Arkaplan bağlantı kontrolü başladı...");

            // ADIM 1: Doğrudan bağlantı kontrolü (Direct/Method 0)
            {
                let mut stage = dpi.connection_stage.lock().await;
                *stage = "checking_direct".to_string();
            }
            let original_method = {
                let settings = dpi.settings.lock().await;
                settings.active_method_id.unwrap_or(0)
            };
            let _ = dpi.start_proxy(&app_handle_for_check, 0).await;

            let direct_check = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                dpi.check_connection_detailed(true)
            ).await;

            // ÖLÇÜT: "sunucuya ulaşabildik mi", "HTTP 200 aldık mı" DEĞİL.
            // Cloudflare'in bot sayfası (403) ya da Vanguard'ın 401'i buraya
            // düşünce eski kod bunu DPI engeli sanıp sırayla 8 yöntemi
            // deniyordu. Tarama, WebView'in canlı trafiğinin aktığı proxy'nin
            // yöntemini saniyeler içinde defalarca değiştirdiğinden sayfa
            // yüklemeleri yarıda kalıyor, boş ekran watchdog'u devreye girip
            // sayfayı yeniliyordu ("sürekli F5"). Sonunda da hiçbir yöntem
            // "başarılı" sayılamadığı için çevrimdışı moda düşülüyordu.
            let direct_reachable = matches!(&direct_check, Ok(r) if r.is_reachable());

            if direct_reachable {
                if let Ok(result) = &direct_check {
                    if *result == dpi_proxy::ConnectionResult::Challenged {
                        log!("[Bağlantı] Sunucuya ulaşıldı (site bot koruması yanıt verdi) — bypass gerekmiyor");
                    } else {
                        log!("[Bağlantı] İnternet bağlantısı kuruldu");
                    }
                }
                let mut stage = dpi.connection_stage.lock().await;
                *stage = "success".to_string();
            } else {
                    dbg_log!("[Setup Background] Doğrudan bağlantı sonucu: {:?}", direct_check);
                    log!("[Bağlantı] Erişim kısıtlı — engel aşma deneniyor…");
                    {
                        let mut stage = dpi.connection_stage.lock().await;
                        *stage = "trying_dpi".to_string();
                    }

                    let test_id = if original_method == 0 { 1 } else { original_method };
                    let _ = dpi.start_proxy(&app_handle_for_check, test_id).await;

                    let proxy_check = tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        dpi.check_connection_detailed(true)
                    ).await;

                    let mut is_working = false;
                    if matches!(&proxy_check, Ok(r) if r.is_reachable()) {
                        dbg_log!("[Setup Background] Kayıtlı DPI yöntemi (#{}) çalışıyor!", test_id);
                        is_working = true;
                    }

                    if !is_working {
                        dbg_log!("[Setup Background] Kayıtlı DPI yöntemi çalışmadı. Tüm yöntemler taranıyor...");
                        if let Some(working_id) = dpi.test_all_methods(&app_handle_for_check).await {
                            dbg_log!("[Setup Background] Çalışan yeni DPI yöntemi bulundu: #{}", working_id);
                            is_working = true;
                        }
                    }

                    if is_working {
                        let mut stage = dpi.connection_stage.lock().await;
                        *stage = "success".to_string();
                    } else {
                        // ADIM 3: Proxy Fallback
                        dbg_log!("[Setup Background] Tüm DPI yöntemleri başarısız. Adım 3: Uzak proxy fallback deneniyor...");
                        match dpi.try_remote_proxy_fallback(&app_handle_for_check).await {
                            Ok(_) => {
                                dbg_log!("[Setup Background] Uzak proxy fallback başarılı!");
                            }
                            Err(_) => {
                                log!("[Bağlantı] İnternete bağlanılamadı — çevrimdışı moda geçiliyor");
                                let mut stage = dpi.connection_stage.lock().await;
                                *stage = "failed".to_string();
                            }
                        }
                    }
            }
        });

        let main_url = WebviewUrl::External("https://openani.me/".parse().unwrap());
        dbg_log!("[Setup] Ana URL: https://openani.me/");
        dbg_log!("[Setup] Pencere oluşturuluyor (1280x848, decorations: false)...");

        let app_handle = app.handle().clone();
        let win_builder = WebviewWindowBuilder::new(
            app,
            "main",
            main_url,
        )
        .title("OpenAnime")
        .inner_size(1280.0, 848.0)
        .min_inner_size(800.0, 500.0)
        .center()
        .decorations(false)
        .zoom_hotkeys_enabled(true)
        .user_agent(user_agent)
        .on_new_window(move |url, _features| {
            dbg_log!(
                "[Tauri] Yeni pencere isteği (main penceresinden): {}",
                url
            );
            let app_c = app_handle.clone();
            let url_str = url.to_string();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = open_new_window(app_c, url_str).await {
                    dbg_log!("[Tauri] Yeni pencere açma hatası: {}", e);
                }
            });
            tauri::webview::NewWindowResponse::Deny
        })
        .initialization_script(build_init_script());

        #[cfg(target_os = "windows")]
        let win_builder = win_builder.additional_browser_args(browser_args);

        dbg_log!("[Setup] Pencere build ediliyor...");
        match win_builder.build() {
            Ok(_window) => {
                dbg_log!("[Setup] Ana pencere başarıyla oluşturuldu (label: main)");
                dbg_log!("[Setup] WebView URL: https://openani.me/");

                // Periyodik performans modu yenilemesi.
                // Gerekçe (ölçümle bulundu): WebView2 çalışırken YENİ süreç doğuruyor
                // — Cloudflare Turnstile iframe'i kendi renderer'ını açıyor ve o süreç
                // biz modu uyguladıktan SONRA doğduğu için EcoQoS'suz kalıyordu.
                // Tek seferlik uygulama yetmiyor; 10 sn'de bir yeniden uygula.
                #[cfg(target_os = "windows")]
                {
                    let app_for_perf = app.handle().clone();
                    std::thread::spawn(move || loop {
                        std::thread::sleep(std::time::Duration::from_secs(10));
                        // DOĞRUDAN çağrılır — run_on_main_thread'e SARILMAZ.
                        // with_webview zaten kendisi ana thread'e dispatch ediyor;
                        // ana thread'in içinden tekrar dispatch etmek kendi kendine
                        // kilitlenme yaratıyor (denendi: uygulama donup kapandı).
                        refresh_perf_mode(&app_for_perf);
                    });
                }

                // Tepsi ikonu ve tıklama/eylem izleyicisini her zaman başlat!
                match super_notifications::ensure_tray(app.handle()) {
                    Ok(()) => log!("[OpenAnime] Tepsi ikonu oluşturuldu"),
                    Err(e) => log!("[OpenAnime][HATA] Tepsi ikonu oluşturulamadı: {}", e),
                }
                super_notifications::start_click_watcher(app.handle());

                log!("[OpenAnime] Hazır");
                Ok(())
            }
            Err(e) => {
                log!("[Hata] Uygulama penceresi açılamadı: {}", e);
                dbg_log!("===== OPENANIME SETUP HATA =====");
                Err(Box::new(e))
            }
        }
    })
        .on_window_event(|window, event| {
            let app_handle = window.app_handle().clone();
            let label = window.label().to_string();

            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let quitting = APP_QUITTING.load(std::sync::atomic::Ordering::SeqCst);

                    if !quitting {
                        api.prevent_close();

                        // ── X = "uygulamayı tepsiye park et" ──────────────
                        // 1) TÜM medyayı duraklat. Bu OLMADAN video arka planda
                        //    çalmaya devam ediyordu: X pencereyi kapatmıyor,
                        //    gizliyor; oynatma sürerken seçilen `BgMode::Media`
                        //    ise motoru BİLEREK dondurmuyor (bkz. BgMode::Media
                        //    doküman notu) — yani "kapattım ama ses devam
                        //    ediyor" tam olarak bu tasarımın sonucuydu.
                        // 2) Hafif bir sayfaya (/settings) git: oynatıcı DOM'u
                        //    tamamen yıkılır, arka plan belleği en aza iner.
                        //    (Tepsi oturumu penceresi de aynı sebeple /settings
                        //    kullanıyor — bkz. spawn_tray_session_window.)
                        // NOT: buradaki `window` bir `tauri::Window` (webview
                        // değil) — script çalıştırmak için etiketten
                        // WebviewWindow'u almak gerekiyor.
                        if let Some(wv) = app_handle.get_webview_window(&label) {
                            let _ = wv.eval(
                                "try{document.querySelectorAll('video,audio').forEach(function(m){try{m.pause();}catch(e){}});}catch(e){}\
                                 try{window.location.href='https://openani.me/settings';}catch(e){}",
                            );
                        }

                        // Oynatma durumunu Rust tarafında da HEMEN düşür.
                        // JS'in `oa_set_player_playing(false)` bildirimini
                        // beklemek yarış yaratırdı: bildirim geç kalırsa
                        // update_background_mode hâlâ "oynuyor" görüp Media'yı
                        // seçer ve motoru dondurmazdı.
                        #[cfg(target_os = "windows")]
                        {
                            let st = app_handle.state::<PerfState>();
                            st.player_playing.lock().unwrap().insert(label.clone(), false);
                        }

                        let _ = window.hide();
                        log!("[Tauri] Pencere tepsiye gizlendi: {}", label);

                        // hide() SADECE HWND'yi gizler; WebView2 controller
                        // "görünür" kalır ve motor tam bellekle çalışmaya devam
                        // eder. Ayrıca hide() WM_SIZE üretmediği için `Resized`
                        // yolundaki askıya alma da tetiklenmez. Bu yüzden arka
                        // plan modunu BURADAN açıkça başlatıyoruz.
                        #[cfg(target_os = "windows")]
                        enter_tray_background(&app_handle, &label);
                    }
                }
                tauri::WindowEvent::Focused(true) => {
                    let label_c = label.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Some(state) =
                            app_handle.try_state::<discordRPC::DiscordState>()
                        {
                            state.set_focused_window(Some(label_c));
                        }
                    });
                }
                // X butonu pencereyi KAPATMAZ, tepsiye gizler (bkz. yukarıdaki
                // CloseRequested kolu). Gizlenen pencere WebView2'siyle birlikte
                // ayakta kalır — bu yüzden gizleme anında `enter_tray_background`
                // ile açıkça askıya alınır, yoksa arka planda tam bellek tüketir.
                //
                // (Vaktiyle X'in gerçekten kapatması denenmiş ve bu yorum ondan
                // kalmıştı; kod hide()'a geri dönmüş ama yorum güncellenmemişti.
                // maybe_spawn_tray_session yolu hâlâ geçerli, sadece X ile değil
                // gerçek pencere kapanışlarıyla tetikleniyor.)
                tauri::WindowEvent::Destroyed => {
                    #[cfg(target_os = "windows")]
                    {
                        let st = app_handle.state::<PerfState>();
                        st.player_playing.lock().unwrap().remove(&label);
                        st.suspended.lock().unwrap().remove(&label);
                        st.bg_since.lock().unwrap().remove(&label);
                    }
                }
                _ => {}
            }

            #[cfg(target_os = "windows")]
            {
                // Odak değişimi tek başına karar vermez — oynatıcı durumuyla
                // birleştirilip refresh_perf_mode'da değerlendirilir.
                // (Eskiden burada doğrudan SetMemoryUsageTargetLevel çağrılıyordu;
                //  artık tek karar noktası var, iki yerde çelişen mantık kalmasın.)
                if let tauri::WindowEvent::Focused(focused) = event {
                    let app = window.app_handle().clone();
                    {
                        let st = app.state::<PerfState>();
                        let mut f = st.focused.lock().unwrap();
                        *f = *focused;
                    }
                    refresh_perf_mode(&app);

                    // Odak GELİNCE (pencere geri geldi/gösterildi) askıdan çıkar.
                    // Odak KAYBINDA askıya ALMA — sadece alt-tab olabilir; askıya
                    // alma yalnızca minimize/gizleme (Resized/hide) ile tetiklenir.
                    if *focused {
                        update_background_mode(&app, window.label());
                    }
                }

                // Minimize/geri-yükleme burada yakalanır (Tauri'de ayrı Minimized
                // eventi yok — Resized + is_minimized ile anlaşılır).
                if let tauri::WindowEvent::Resized(_) = event {
                    update_background_mode(&window.app_handle(), window.label());
                }
            }
        });

        let run_result = builder.invoke_handler(tauri::generate_handler![
            open_new_window,
            update_discord_presence,
            clear_discord_presence,
            set_discord_rpc_enabled,
            set_focused_window,
            close_window_label,
            // 🎥 Local video server — port sorgula & süper açılış videosu
            get_local_video_port,
            get_super_opening_video_url,
            get_super_opening_video_data,
            // 🎥 Local video server — videoId ↔ dosya yolu eşlemesi kaydet
            fetch_css,
            check_connection,
            // 🔗 Link Ayıklayıcı — kaynak site sayfası çekme + şifreli embed çözme + link durum testi
            fetch_external_html,
            resolve_turkanime_embed,
            resolve_animecix_episode,
            list_animecix_season_episodes,
            check_link_status,
            go_online,
            go_offline,
            list_themes,
            load_theme,
            apply_theme_css,
            logger::get_session_log,
            updater::get_app_version,
            updater::check_for_updates,
            updater::start_update_download,
            // DPI Proxy komutları
            reopen_with_proxy,
            set_zoom_level,
            get_zoom_level,
            dpi_proxy::dpi_test_methods,
            dpi_proxy::dpi_get_status,
            // Yerel dosya seçme dialogu
            pick_mp4_file,
            // 📄 Dosyanın ilk N baytını oku (IndexedDB dummy blob için)
            read_file_head,
            // JS hata köprüsü (webview console/onerror → terminal log)
            oa_js_log,
            // Performans/verimlilik modu — JS oynatıcı durumunu bildirir
            oa_set_player_playing,
            // GPU Bilgisi
            gpu_info::oa_get_gpu_info,
            gpu_info::oa_get_gpu_hint,
            gpu_info::oa_set_webgpu_vendor,
            // Süper Bildirimler
            super_notifications::sn_set_enabled,
            super_notifications::sn_set_gateway_token,
            super_notifications::sn_set_auth_token,
            super_notifications::sn_set_account,
            super_notifications::sn_open_notification,
            super_notifications::sn_test_toast,
            super_notifications::sn_test_notifications
        ])
        .build(tauri::generate_context!());

    let app = match run_result {
        Ok(app) => app,
        Err(err) => {
            log!("[Hata] Uygulama başlatılamadı: {}", err);
            show_fatal_startup_error(&err);
            std::process::exit(1);
        }
    };

    // Son pencere kapandığında (RunEvent::ExitRequested) varsayılan davranış
    // tüm process'i sonlandırmaktır. Süper Bildirimler açıksa bunu engelleyip
    // hafif, görünmez bir arkaplan oturumu (/settings) açarız — Discord RPC ve
    // bildirim akışı böylece canlı kalır. Tepsi menüsünden gerçek "Kapat"
    // (APP_QUITTING=true) bu engellemeyi hiçbir zaman tetiklemez.
    app.run(move |app_handle, event| {
        if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
            // AppHandle::restart() (güncelleme kurulumu sonrası tetiklenir) bu
            // event'i code=RESTART_EXIT_CODE ile fırlatır. Bu durumda hiçbir şey
            // yapmadan çıkıyoruz: Tauri'nin kendi prevent_exit() implementasyonu
            // zaten bu code'da no-op (bkz. tauri app.rs ExitRequestApi), ama biz
            // yine de gereksiz bir arkaplan tepsi oturumu açmayalım — restart
            // sürüyorken yeni bir pencere doğurmak anlamsız/yarış durumu yaratır.
            if code == Some(tauri::RESTART_EXIT_CODE) {
                return;
            }
            if APP_QUITTING.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            // Tray her zaman aktif: çıkışı engelle ve arkaplan oturumu aç.
            api.prevent_exit();
            let app_c = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                maybe_spawn_tray_session(&app_c);
            });
        }
    });
}