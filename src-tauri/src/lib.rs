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
mod gpu;
#[cfg(target_os = "linux")]
mod gpu_detector;
mod webgpu_bridge;
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
mod theme_app;
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
///
/// Linux'ta başına çalışma zamanı bayrakları eklenir:
///   __OA_OVERLAY_OK__ — overlay pencereleri (WebGPU canvas sunumu)
///   konumlandırılabiliyor mu (bkz. `configure_display_backend`). `false` ise
///   `webgpu-native-shim.js` `navigator.gpu`'yu HİÇ kurmaz, site kendi
///   HTML5/HLS yoluna döner — player yine çalışır, yalnızca upscale/kare
///   oluşturma devre dışı kalır.
/// Windows/macOS webview'ı WebGPU'yu zaten native sağladığından bu bayraklara
/// ihtiyaç duymaz (shim `isLinux` kontrolüyle kendini zaten devre dışı bırakır).
fn build_init_script() -> String {
    #[cfg(target_os = "linux")]
    {
        // OPENANIME_DISABLE_WEBGPU=1: tanılama bayrağı — shim navigator.gpu'yu
        // hiç kurmaz, dolayısıyla wgpu instance'ı proseste hiç yüklenmez.
        // "Render/donma sorunu WebGPU köprüsünden mi geliyor?" sorusunu izole
        // etmek için.
        let webgpu_disabled = std::env::var("OPENANIME_DISABLE_WEBGPU")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if webgpu_disabled {
            println!("[Display] OPENANIME_DISABLE_WEBGPU=1 — WebGPU shim devre dışı (tanılama modu)");
        }
        let overlay_flag = overlays_supported() && !webgpu_disabled;
        return format!(
            "window.__OA_OVERLAY_OK__={};\n{}",
            overlay_flag, COMMON_INIT_SCRIPT
        );
    }
    #[cfg(not(target_os = "linux"))]
    {
        COMMON_INIT_SCRIPT.to_string()
    }
}

/// Logo animatörü ("Muptezel Anime" splash) base64 dokularını lazy döndürür.
/// Eskiden COMMON_INIT_SCRIPT'e ~244KB gömülüp her webview'da parse ediliyordu;
/// artık yalnızca ilgili varyant çalıştığında istenir (bkz. logo-animator.js).
#[tauri::command]
fn oa_get_logo_textures() -> String {
    include_str!("js/modules/logo-animator/textures.js").to_string()
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
    // ÖNCELİKLE: sinyal HEMEN GÖNDERİLMEZ. CloseRequested yukarıda sayfa
    // /settings'e navigasyon başlatıyor — bu anlık eval eski sayfanın context'ine
    // gider ve navigasyon bitince kaybolur. Yeni /settings sayfasının yüklenmesi
    // ve initialization_script'in çalışması için kısa bir süre bekleyip
    // sonra sinyali gönderiyoruz.
    //
    // Gecikme takvimi:
    //   0 ms        → hide() + navigasyon başlar
    //   ~500 ms     → /settings yüklendi, init script çalıştı → "hidden" sinyali
    //   1200 ms     → update_background_mode → TrySuspend (motor donar)
    const BG_SIGNAL_DELAY_MS: u64 = 500;

    let app_c = app.clone();
    let label_c = label.to_string();
    let js_mode_c = js_mode.to_string();
    std::thread::spawn(move || {
        // 1) /settings yüklenmesini bekle, sonra JS'e "hidden" sinyali gönder
        std::thread::sleep(std::time::Duration::from_millis(BG_SIGNAL_DELAY_MS));
        emit_background_state(&app_c, &label_c, &js_mode_c);

        // 2) Kalan süre kadar bekle, sonra motoru askıya al
        let remaining = TRAY_SUSPEND_DELAY_MS.saturating_sub(BG_SIGNAL_DELAY_MS);
        if remaining > 0 {
            std::thread::sleep(std::time::Duration::from_millis(remaining));
        }
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

        // GÜVENLİK AĞI: Resume() + SetIsVisible(true) WebView2 motorunun
        // uyanmasını başlatır ama motor tam uyanmadan gönderilen eval kuyruğa
        // alınıp kaybolabilir. Kısa bir gecikmeyle ikinci bir "foreground"
        // sinyali göndermek bunu garanti altına alır. Fix 1 (background-mode.js)
        // ile "foreground" sinyali her zaman syncTimers çağırdığı için ikinci
        // çağrı zararsız — timer'ları tekrar taramak ucuz bir işlem.
        let app_for_retry = window.app_handle().clone();
        let label_for_retry = window.label().to_string();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            emit_background_state(&app_for_retry, &label_for_retry, "foreground");
        });
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
    // BLOK 2B: WEBGPU IPC KÖPRÜSÜ (SADECE LİNUX)
    // WebKitGTK navigator.gpu sunmuyor; bu shim onu JS tarafında taklit edip
    // her çağrıyı IPC ile src-tauri/src/webgpu_bridge.rs'teki gerçek wgpu'ya
    // yönlendirir. Site kendi kodunu (gerçek OFG kare-oluşturma dahil) hiç
    // değişmeden çalıştırır. Kendini Windows/macOS'ta (`isLinux` kontrolü) ve
    // overlay konumlandırılamıyorsa (__OA_OVERLAY_OK__, bkz. build_init_script)
    // devre dışı bırakır — BURADA ekstra bir platform dalına gerek yok.
    // Sonraki bloklardaki webgpu-inspector.js/webgpu-detect.js'in
    // navigator.gpu'yu bulabilmesi için ONLARDAN ÖNCE gelmeli.
    // ──────────────────────────────────────────────
    include_str!("js/modules/webgpu-native-shim.js"),
    "\n",

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
    // NOT: textures.js (~244KB base64) artık BURAYA gömülmez — yalnızca ilgili
    // varyant çalıştığında `oa_get_logo_textures` komutuyla lazy çekilir.
    // ──────────────────────────────────────────────
    "{\n",
    include_str!("js/modules/logo-animator/logo-animator.js"),
    "\n",
    include_str!("js/modules/super-opening.js"),
    "\n}\n",

    // ──────────────────────────────────────────────
    // BLOK 6C: WEBGPU PIPELINE TEŞHİS ARACI (yalnızca window.__oaInspectWebGPU=true)
    // openani.me'nin kendi WebGPU oynatıcısını tersine mühendislik için GEÇİCİ
    // bir teşhis aracı. GPUDevice/GPUCanvasContext/GPUCommandEncoder üzerindeki
    // shader/doku/pass çağrılarını yakalar. Varsayılan KAPALI — hook'lar her
    // sayfada kurulur ama yalnızca bayrak açıkken kayıt üretir (sıfır yan etki).
    //
    // NEDEN BURADA: site kendi device'ını isteyip ht.init()'i çalıştırmadan ÖNCE
    // hook'ların kurulu olması şart. Bu blok document-start'ta çalışır ve
    // webgpu-detect'ten bile önce gelir (adapter seçiminden önce). Log'lar
    // oa_js_log → dbg_log! üzerinden gider, yani yalnızca dev build'de ya da
    // OA_DEBUG=1 ile session loguna düşer (bkz. logger.rs).
    // ──────────────────────────────────────────────
    include_str!("js/modules/webgpu-inspector.js"),
    "\n",

    // ──────────────────────────────────────────────
    // BLOK 6D: WebGPU ADAPTER ALGILAMA
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
    // Yalnızca /dashboard rotasında aktif olur (bkz. dosya içi route
    // guard'ı + body.oa-dashboard-active CSS scoping'i). Sidebar gruplama,
    // form hafızası ve oynatıcı hatırlamayı içerir. Oynatıcı geri yükleme
    // TEK ATIMLIKTIR (sidebar gezinmesi başına 1 deneme, token korumalı).
    //
    // NOT: player-dialog-dismiss.js bilinçli olarak enjekte EDİLMEDİ —
    // kendi observer'ı dialogu kapattıkça yeniden tetiklenip sonsuz
    // tıklama döngüsüne giriyordu. Aynı işi enhancer'ın adım 2'si
    // (dialogu kapat) zaten güvenli yapıyor.
    // ──────────────────────────────────────────────
    include_str!("js/modules/dashboard-enhancer.js"),
    "\n",

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
    include_str!("js/modules/theme/theme-install-modal.js"),
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
    // BLOK 9C: "???"
    // Belirli bir profil sayfasında isme çift tıklayınca beliren gizli menü.
    // Resmi Electron istemcisindeki sağ tık menüsü şakasının karşılığı.
    // ──────────────────────────────────────────────
    "{\nconst EASTER_EGG_CSS = String.raw`",
    include_str!("js/modules/easter-egg.css"),
    "`;\n",
    include_str!("js/modules/easter-egg.js"),
    "}\n",

    // ──────────────────────────────────────────────
    // BLOK 10: BAŞLATMA (EN SON ÇALIŞIR)
    // ──────────────────────────────────────────────
    include_str!("js/init.js"),
    "\n})();"
);

#[cfg(target_os = "windows")]
pub const WINDOWS_BASE_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,msTrackingPrevention --enable-features=ParallelDownloading,HardwareMediaKeyHandling --enable-quic --enable-fast-unload --enable-gpu-rasterization --enable-zero-copy --enable-gpu-memory-buffer-video-frames --renderer-process-limit=1 --disk-cache-size=134217728 --media-cache-size=67108864 --js-flags=\"--max-old-space-size=512\" --force-gpu-selection=high-performance --force_high_performance_gpu --autoplay-policy=no-user-gesture-required";

/// Proxy aktifken kullanılacak browser args
#[cfg(target_os = "windows")]
pub const WINDOWS_PROXY_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,msTrackingPrevention --enable-features=ParallelDownloading,HardwareMediaKeyHandling --enable-quic --enable-fast-unload --enable-gpu-rasterization --enable-zero-copy --enable-gpu-memory-buffer-video-frames --renderer-process-limit=1 --disk-cache-size=134217728 --media-cache-size=67108864 --js-flags=\"--max-old-space-size=512\" --force-gpu-selection=high-performance --force_high_performance_gpu --proxy-server=http://127.0.0.1:1453 --autoplay-policy=no-user-gesture-required";

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

    // macOS: `decorations(false)` bırakıyor bazı tao/macOS sürüm
    // kombinasyonlarında pencereyi NSWindowCollectionBehavior::FullScreenPrimary
    // olmadan oluşturuyor — native fullscreen (yeşil düğme / cmd+ctrl+F) ya hiç
    // çalışmıyor ya da yarım kalmış bir geçişte pencere kromunu bozuyor. Theme'in
    // kanıtlanmış deseni (tauri.macos.conf.json): decorations true + Overlay +
    // hiddenTitle — native trafik ışıkları görünür kalır, içerik onların ALTINDA
    // değil ÜSTÜNDE (overlay) render edilir, native fullscreen düzgün çalışır.
    #[cfg(target_os = "macos")]
    let win_builder = win_builder
        .decorations(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    #[cfg(not(target_os = "macos"))]
    let win_builder = win_builder.decorations(false);

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

    // bkz. build_new_window'daki macOS decorations/fullscreen açıklaması.
    #[cfg(target_os = "macos")]
    let win_builder = win_builder
        .decorations(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    #[cfg(not(target_os = "macos"))]
    let win_builder = win_builder.decorations(false);

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

/// "???" yumurta penceresi (bkz. js/modules/easter-egg.js, static/egg.html).
///
/// Resmi Electron istemcisi (OpenAnime/desktop-ts) bunu AYRI bir pencereyle
/// yapıyor: çerçevesiz + tam ekran + her zaman üstte + fare olaylarını yok
/// sayan bir BrowserWindow. Buradaki de birebir aynı ve bu bilinçli bir
/// tercih — ANA PENCEREYE HİÇ DOKUNULMAZ. Ana pencereyi tam ekrana almayı
/// denemek iki somut soruna yol açıyordu:
///
///   1. Windows'ta maximized pencereyi fullscreen'e alırken gereken
///      `hide()` + `unmaximize()` dansı pencereyi bir an görünmez yapıyor;
///      `update_background_mode` bunu DeepSleep sanıp WebView2'yi askıya
///      alıyordu (uygulama dakikalarca donuyor, F5 bile geç işliyordu).
///   2. Borderless fullscreen tek başına görev çubuğunun ALTINDA kalıyor;
///      `always_on_top` ancak pencerenin kendisine uygulanınca işe yarıyor.
const EASTER_EGG_WINDOW_LABEL: &str = "oa-egg";

#[tauri::command]
async fn oa_open_easter_egg_window(app: tauri::AppHandle) -> Result<(), String> {
    // Zaten oynuyorsa ikinci pencere açma.
    if app.get_webview_window(EASTER_EGG_WINDOW_LABEL).is_some() {
        return Ok(());
    }

    let builder = WebviewWindowBuilder::new(
        &app,
        EASTER_EGG_WINDOW_LABEL,
        WebviewUrl::App("egg.html".into()),
    )
    .title("???")
    .decorations(false)
    .fullscreen(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(true);

    // ARGÜMANLAR DİĞER PENCERELERLE AYNI OLMAK ZORUNDA. WebView2'de tüm
    // webview'ler aynı kullanıcı veri klasörü üzerinden TEK bir ortamı
    // paylaşır; ikinci bir webview farklı AdditionalBrowserArguments ile
    // istenirse ortam kurulamaz ve webview sessizce ölür — pencere doğar ama
    // içi hiç çizilmez (belirtisi: pencere getter'ları FailedToReceiveMessage
    // döner). Buraya özel bir `--autoplay-policy` bayrağı eklemek tam olarak
    // buna yol açıyordu; o bayrak artık ortak argümanların içinde
    // (bkz. WINDOWS_BASE_ARGS / WINDOWS_PROXY_ARGS).
    #[cfg(target_os = "windows")]
    let builder = builder.additional_browser_args(WINDOWS_PROXY_ARGS);

    let window = builder
        .build()
        .map_err(|e| format!("Yumurta penceresi açılamadı: {}", e))?;

    // Orijinaldeki `close` → preventDefault karşılığı: kullanıcı kapatamaz.
    // Video bitince sayfa kendini `destroy()` ile yok eder; o yol
    // CloseRequested üretmediği için buradaki koruma onu engellemez.
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
        }
    });

    // NOT: Orijinaldeki `setIgnoreMouseEvents(true)` (tıklama geçirgenliği)
    // BİLEREK uygulanmıyor. Windows'ta bu, pencereye WS_EX_LAYERED ekler ve
    // katmanlı pencere alfası ayarlanmadığı sürece pencere hiç çizilmez —
    // "pencere açıldı ama ekranda yok" tablosu tam olarak bu. Şakanın
    // gereği de değil: pencere zaten tam ekran ve kapatılamaz.

    dbg_log!("[???] Yumurta penceresi oluşturuldu");
    Ok(())
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
/// yalnızca genel (özel/loopback olmayan) hostlar.
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

/// WebGPU teşhis aracının (webgpu-inspector.js) yakaladığı binary veriyi
/// (ör. 512×512 LUT renk tablosu) base64 string olarak alıp dosyaya yazar.
/// Dönen yol app_local_data_dir/captures/ altındadır — böylece büyük binary
/// veri chat'e yapıştırılmadan doğrudan dosyadan okunabilir.
#[tauri::command]
fn oa_save_webgpu_capture(
    name: String,
    data_base64: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    use base64::Engine as _;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64.as_bytes())
        .map_err(|e| format!("base64 çözülemedi: {}", e))?;

    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("veri dizini alınamadı: {}", e))?
        .join("captures");
    std::fs::create_dir_all(&dir).map_err(|e| format!("captures dizini oluşturulamadı: {}", e))?;

    // Dosya adını güvenli tut (yalnızca alfanumerik + -_. ; yol gezintisini önler).
    let safe: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .take(64)
        .collect();
    let safe = if safe.is_empty() { "capture.bin".to_string() } else { safe };

    let path = dir.join(safe);
    std::fs::write(&path, &bytes).map_err(|e| format!("dosya yazılamadı: {}", e))?;

    dbg_log!("[WebGPU Inspect] Capture kaydedildi: {}", path.display());
    Ok(path.to_string_lossy().to_string())
}

#[cfg(target_os = "windows")]
/// macOS'ta App Nap / güç tasarrufu tabanlı kısıtlamaları (ekran-uyku,
/// sistem boşta-uyku, arka plan QoS düşürme) uygulama boyunca devre dışı
/// bırakır. Windows tarafında EcoQoS/bellek hedefi tuning'inin (bkz.
/// refresh_perf_mode) macOS eşdeğeri hiç yoktu — WKWebView/WebGPU tarafında
/// gerçek zamanlı kare oluşturma gibi sürekli GPU/CPU yükü olan bir iş
/// varken sistemin gücü kısmasını önlemek istiyoruz.
///
/// `caffeinate` (sistemde her zaman hazır, macOS'un kendi CLI aracı) ile
/// yapılıyor — Objective-C/objc2 API'lerine (NSProcessInfo.beginActivity)
/// elle FFI yazmak yerine: bu makine yerelde Apple SDK/araç zinciri
/// olmadığından o kodu ASLA derleyip doğrulayamam (bkz. `cargo check
/// --target x86_64-apple-darwin` denemesi — bir C build script'inde
/// tıkanıyor). `caffeinate` 15+ yıldır değişmeyen, stabil bir sistem aracı;
/// burada hata riski çok daha düşük.
///
/// `-w <pid>`: caffeinate bu process çıkınca kendiliğinden sonlanır, ayrıca
/// öldürmemiz gerekmiyor.
#[cfg(target_os = "macos")]
fn prevent_macos_app_nap() {
    let pid = std::process::id().to_string();
    match std::process::Command::new("caffeinate")
        .args(["-d", "-i", "-w", &pid])
        .spawn()
    {
        Ok(_) => dbg_log!("[macOS] caffeinate başlatıldı (App Nap/uyku engellendi, pid={})", pid),
        Err(e) => dbg_log!("[macOS] caffeinate başlatılamadı: {}", e),
    }
}

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

thread_local! {
    /// `with_suppressed_panic_log` içindeyken true — GPU instance kurulumu
    /// gibi "panic'leyebilir ama yakalanıp normal bir hataya çevrilecek"
    /// beklenen durumlarda tam backtrace + crash.log dosyası YAZILMASIN diye
    /// (bkz. gpu/mod.rs -> create_instance_safe). Beklenmeyen bir panic bunu
    /// hiç görmez, HER ZAMAN false'tur.
    static SUPPRESS_PANIC_LOG: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

/// Verilen kapanışı, panic hook'unun bu thread'deki panic'i sessizce (tek
/// satır log, backtrace/crash.log YOK) işlemesini sağlayarak çalıştırır.
/// `catch_unwind` ile birlikte kullanılır — panic zaten yakalanıp normal bir
/// `Result::Err`e çevrileceği için gürültülü bir crash raporuna gerek yoktur.
#[allow(dead_code)] // yalnızca Linux'ta çağrılıyor (bkz. gpu::create_instance_safe)
pub(crate) fn with_suppressed_panic_log<T>(f: impl FnOnce() -> T) -> T {
    SUPPRESS_PANIC_LOG.with(|c| c.set(true));
    let result = f();
    SUPPRESS_PANIC_LOG.with(|c| c.set(false));
    result
}

/// Panic mesajı + backtrace'i hem session log'a hem de kalıcı bir crash
/// dosyasına yazar; "uygulama sessizce çöküyor" raporları böylece kanıtlı gelir.
fn install_crash_logger() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Beklenen/yakalanan panic (bkz. with_suppressed_panic_log): tek
        // satır log, backtrace yok, crash.log yok.
        if SUPPRESS_PANIC_LOG.with(|c| c.get()) {
            log!("[Panic-Yakalandı] {}", info);
            return;
        }

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
/// Linux'ta pencere overlay'lerinin (WebGPU canvas sunumu) çalışıp
/// çalışamayacağı. Wayland'da toplevel pencereler konumlandırılamaz
/// (gtk_window_move no-op) — overlay mimarisi yalnızca X11/XWayland'da işler.
#[cfg(target_os = "linux")]
static OVERLAYS_SUPPORTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

#[cfg(target_os = "linux")]
pub fn overlays_supported() -> bool {
    *OVERLAYS_SUPPORTED.get().unwrap_or(&false)
}

/// Prosesin İLK Xlib çağrısı olarak XInitThreads() çalıştırır — GTK init'ten
/// ve her türlü XOpenDisplay'den ÖNCE çağrılmalıdır. Gerekçe: GDK (GTK3)
/// XInitThreads çağırmaz; Display kilitleri XOpenDisplay ANINDA thread
/// desteğine göre kurulduğundan GTK'nın X bağlantısı kilitsiz açılır. Oysa
/// wgpu/Vulkan WSI (canvas overlay surface'ları + swapchain present) aynı GTK
/// Display'ine tokio thread'lerinden dokunur — kilitsiz bağlantıda bu, GTK ana
/// döngüsüyle veri yarışıdır ve sahada "uygulama bir anda çizmeyi bırakıyor"
/// (UI donuk, JS/IPC yaşıyor) olarak görülüyordu.
#[cfg(target_os = "linux")]
fn init_xlib_threads() {
    if let Ok(xlib) = x11_dl::xlib::Xlib::open() {
        let status = unsafe { (xlib.XInitThreads)() };
        println!("[Display] XInitThreads çağrıldı (status={status}) — Xlib çok-thread kilitleri aktif");
    } else {
        // libX11 yok (saf Wayland sistemi olabilir) — overlay'ler zaten devre
        // dışı kalacağından sorun değil.
        println!("[Display] libX11 açılamadı — XInitThreads atlandı");
    }
}

/// Wayland oturumunda GTK'yı X11 backend'ine (XWayland) zorlar.
/// GTK init'ten ÖNCE çağrılmalıdır. Gerekçe: tao/GTK'da set_position →
/// gtk_window_move, Wayland'da belgeli no-op'tur; gpu_canvas overlay
/// pencereleri videonun/canvas'ın üzerine hizalanamaz. XWayland altında
/// konumlandırma X11 semantiğiyle çalışır.
#[cfg(target_os = "linux")]
fn configure_display_backend() {
    let native_wayland_opt_in = std::env::var("OPENANIME_NATIVE_WAYLAND")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let is_wayland_session = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let has_x11 = std::env::var_os("DISPLAY").is_some();
    let user_forced_backend = std::env::var_os("GDK_BACKEND").is_some();

    // Karar HER dalda loglanır — sahada hangi yolun seçildiği log'dan
    // birebir okunabilmeli (bazı AppImage GTK hook'ları GDK_BACKEND'i
    // kendileri export eder; o durum da görünür olmalı).
    let overlays_ok = if !is_wayland_session {
        println!("[Display] X11 oturumu — backend değişikliği gerekmedi");
        true
    } else if user_forced_backend {
        let value = std::env::var("GDK_BACKEND").unwrap_or_default();
        let ok = value.contains("x11");
        println!(
            "[Display] GDK_BACKEND önceden setli (\"{}\") — dokunulmadı{}",
            value,
            if ok { "" } else { " (x11 değil: overlay'ler devre dışı)" }
        );
        ok
    } else if native_wayland_opt_in {
        println!("[Display] OPENANIME_NATIVE_WAYLAND=1 — Wayland'da kalınıyor, overlay'ler devre dışı");
        false
    } else if has_x11 {
        std::env::set_var("GDK_BACKEND", "x11");
        println!("[Display] Wayland oturumu tespit edildi — GDK_BACKEND=x11 zorlandı (XWayland). Overlay konumlandırma aktif.");
        true
    } else {
        println!("[Display] Saf Wayland (XWayland yok) — overlay'ler devre dışı, HTML5 player modu");
        false
    };

    println!("[Display] karar: overlay={}", overlays_ok);
    let _ = OVERLAYS_SUPPORTED.set(overlays_ok);
}

pub fn run() {
    install_crash_logger();

    #[cfg(target_os = "linux")]
    {
        init_xlib_threads();
        configure_display_backend();
    }

    #[cfg(target_os = "macos")]
    {
        prevent_macos_app_nap();
    }

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

    // GPU/display server'a göre WebKit/DRM ortam değişkenlerini GPU tanılama
    // sistemi üzerinden yapılandırır (vendor tespiti, Vulkan ICD kontrolü,
    // NVIDIA DMA-BUF/explicit-sync, Wayland GBM backend workaround'ları).
    // Linux dışında no-op — `#[cfg]` burada değil, fonksiyonun kendi
    // gövdesinde (bkz. gpu/mod.rs), bu yüzden çağrı koşulsuz.
    gpu::configure_linux_gpu_env();

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

        // bkz. build_new_window'daki macOS decorations/fullscreen açıklaması.
        #[cfg(target_os = "macos")]
        let win_builder = win_builder
            .decorations(true)
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true);
        #[cfg(not(target_os = "macos"))]
        let win_builder = win_builder.decorations(false);

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
                // Ana pencere taşınınca WebGPU canvas overlay'lerini (bkz.
                // webgpu_bridge.rs -> CanvasContext) kayıtlı son viewport
                // bounds'una göre yeniden konumlandır.
                #[cfg(target_os = "linux")]
                tauri::WindowEvent::Moved(_) => {
                    webgpu_bridge::inner::reposition_overlays(&app_handle);
                }
                _ => {}
            }

            // Ana pencere odak kaybedince WebGPU overlay'lerini gizle / odak
            // dönünce geri göster — always_on_top overlay başka uygulamaların
            // üstünde boyanmamış kutu olarak kalmasın.
            #[cfg(target_os = "linux")]
            if let tauri::WindowEvent::Focused(focused) = event {
                webgpu_bridge::inner::set_overlays_visible(*focused);
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
            // "???"
            oa_open_easter_egg_window,
            // 🎥 Local video server — videoId ↔ dosya yolu eşlemesi kaydet
            fetch_css,
            check_connection,
            go_online,
            go_offline,
            list_themes,
            load_theme,
            apply_theme_css,
            logger::get_session_log,
            updater::get_app_version,
            updater::updater_check,
            updater::updater_download,
            theme_app::theme_app_status,
            theme_app::open_theme_app,
            theme_app::install_theme_app,
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
            // WebGPU teşhis — binary capture (LUT) dosyaya yaz
            oa_save_webgpu_capture,
            // Logo animatörü base64 dokuları (lazy — bkz. oa_get_logo_textures)
            oa_get_logo_textures,
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
            super_notifications::sn_test_notifications,
            // GPU tanılama (Vulkan/backend durumu, dağıtım-farkındalı hata mesajları)
            gpu::gpu_full_report,
            gpu::gpu_vulkan_status,
            gpu::gpu_backend_info,
            gpu::gpu_refresh_report,
            gpu::gpu_fallback_status,
            gpu::gpu_activate_fallback,
            // WebGPU IPC köprüsü (yalnızca Linux'ta gerçek işlev görür — bkz.
            // webgpu_bridge.rs; diğer platformlarda hata döndüren stub'lar)
            webgpu_bridge::inner::gpu_request_adapter,
            webgpu_bridge::inner::gpu_request_device,
            webgpu_bridge::inner::gpu_create_buffer,
            webgpu_bridge::inner::gpu_write_buffer,
            webgpu_bridge::inner::gpu_buffer_map_async,
            webgpu_bridge::inner::gpu_buffer_unmap,
            webgpu_bridge::inner::gpu_create_texture,
            webgpu_bridge::inner::gpu_texture_create_view,
            webgpu_bridge::inner::gpu_write_texture,
            webgpu_bridge::inner::gpu_create_sampler,
            webgpu_bridge::inner::gpu_create_shader_module,
            webgpu_bridge::inner::gpu_create_bind_group_layout,
            webgpu_bridge::inner::gpu_create_pipeline_layout,
            webgpu_bridge::inner::gpu_create_bind_group,
            webgpu_bridge::inner::gpu_create_compute_pipeline,
            webgpu_bridge::inner::gpu_create_render_pipeline,
            webgpu_bridge::inner::gpu_pipeline_get_bind_group_layout,
            webgpu_bridge::inner::gpu_upload_frame_bin,
            webgpu_bridge::inner::gpu_upload_frame,
            webgpu_bridge::inner::gpu_create_command_encoder,
            webgpu_bridge::inner::gpu_encoder_begin_compute_pass,
            webgpu_bridge::inner::gpu_encoder_set_compute_pipeline,
            webgpu_bridge::inner::gpu_encoder_set_bind_group,
            webgpu_bridge::inner::gpu_encoder_dispatch_workgroups,
            webgpu_bridge::inner::gpu_encoder_end_compute_pass,
            webgpu_bridge::inner::gpu_encoder_begin_render_pass,
            webgpu_bridge::inner::gpu_encoder_set_render_pipeline,
            webgpu_bridge::inner::gpu_encoder_set_render_bind_group,
            webgpu_bridge::inner::gpu_encoder_set_vertex_buffer,
            webgpu_bridge::inner::gpu_encoder_set_index_buffer,
            webgpu_bridge::inner::gpu_encoder_draw,
            webgpu_bridge::inner::gpu_encoder_draw_indexed,
            webgpu_bridge::inner::gpu_encoder_end_render_pass,
            webgpu_bridge::inner::gpu_encoder_copy_buffer_to_texture,
            webgpu_bridge::inner::gpu_encoder_copy_texture_to_texture,
            webgpu_bridge::inner::gpu_encoder_finish,
            webgpu_bridge::inner::gpu_queue_submit,
            webgpu_bridge::inner::gpu_queue_on_submitted_work_done,
            webgpu_bridge::inner::gpu_canvas_get_context,
            webgpu_bridge::inner::gpu_canvas_configure,
            webgpu_bridge::inner::gpu_canvas_get_current_texture,
            webgpu_bridge::inner::gpu_canvas_present,
            webgpu_bridge::inner::gpu_canvas_sync_bounds,
            webgpu_bridge::inner::gpu_write_buffer_bin,
            webgpu_bridge::inner::gpu_write_texture_bin,
            webgpu_bridge::inner::gpu_buffer_read_bin,
            webgpu_bridge::inner::gpu_import_video_frame,
            webgpu_bridge::inner::gpu_destroy_resource,
            webgpu_bridge::inner::gpu_push_error_scope,
            webgpu_bridge::inner::gpu_pop_error_scope,
            webgpu_bridge::inner::gpu_reset_bridge
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