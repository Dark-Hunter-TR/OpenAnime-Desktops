#![allow(linker_messages)]

use tauri::{WebviewWindowBuilder, WebviewUrl, Manager};
use std::sync::Mutex;
use std::sync::Arc;

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

/// Performans modu kararı için paylaşılan durum.
///
/// Kural: TAM PERFORMANS yalnızca (video oynuyor VE pencere odakta) iken.
/// Diğer her durumda (ana sayfa, duraklatılmış video, alt-tab) → VERİMLİLİK.
#[derive(Default)]
pub struct PerfState {
    /// Oynatıcıda video fiilen oynuyor mu (JS bildirir)
    pub player_playing: Mutex<bool>,
    /// Herhangi bir pencere odakta mı
    pub focused: Mutex<bool>,
}

#[allow(non_snake_case)]
mod discordRPC;

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
    let full_perf = {
        let st = app.state::<PerfState>();
        let playing = *st.player_playing.lock().unwrap();
        let focused = *st.focused.lock().unwrap();
        playing && focused
    };

    for (_label, window) in app.webview_windows() {
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
                let level = if full_perf {
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

/// JS bildirir: oynatıcıda video oynuyor mu?
#[tauri::command]
fn oa_set_player_playing(playing: bool, app: tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    {
        {
            let st = app.state::<PerfState>();
            let mut p = st.player_playing.lock().unwrap();
            if *p == playing {
                return; // durum değişmedi — API'yi boşuna çağırma
            }
            *p = playing;
        }
        log!("[PerfMode] Video oynuyor = {}", playing);
        refresh_perf_mode(&app);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (playing, app);
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
    // BLOK 2: AĞ ÖNBELLEK & GÖRSEL BOYUTLANDIRMA
    // ──────────────────────────────────────────────
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
    // BLOK 5B: SÜPER BİLDİRİMLER (Ayar placeholder — "Yakında")
    // Discord RPC kartının altına devre dışı bir ayar kartı ekler.
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
pub const WINDOWS_BASE_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,msTrackingPrevention --enable-features=ParallelDownloading,HardwareMediaKeyHandling,CanvasOopRasterization --enable-quic --enable-fast-unload --enable-gpu-rasterization --enable-zero-copy --enable-gpu-memory-buffer-video-frames --disk-cache-size=1073741824 --media-cache-size=536870912 --js-flags=\"--max-old-space-size=2048\" --force-gpu-selection=high-performance --force_high_performance_gpu";

/// Proxy aktifken kullanılacak browser args
#[cfg(target_os = "windows")]
pub const WINDOWS_PROXY_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,msTrackingPrevention --enable-features=ParallelDownloading,HardwareMediaKeyHandling,CanvasOopRasterization --enable-quic --enable-fast-unload --enable-gpu-rasterization --enable-zero-copy --enable-gpu-memory-buffer-video-frames --disk-cache-size=1073741824 --media-cache-size=536870912 --js-flags=\"--max-old-space-size=2048\" --force-gpu-selection=high-performance --force_high_performance_gpu --proxy-server=http://127.0.0.1:1453";

fn platform_user_agent() -> &'static str {
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

fn build_new_window(app: &tauri::AppHandle, url: String) -> Result<(), String> {
    println!("[Tauri] build_new_window invoked with URL: {}", url);

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
        println!(
            "[Tauri] Intercepted new window request from secondary window for URL: {}",
            new_url
        );
        let app_c = app_handle.clone();
        let url_str = new_url.to_string();
        std::thread::spawn(move || {
            if let Err(e) = build_new_window(&app_c, url_str) {
                eprintln!("[Tauri] on_new_window -> build_new_window error: {}", e);
            }
        });
        tauri::webview::NewWindowResponse::Deny
    })
    .initialization_script(build_init_script());

    #[cfg(target_os = "windows")]
    let win_builder = win_builder.additional_browser_args(WINDOWS_PROXY_ARGS);

    match win_builder.build() {
        Ok(_) => {
            println!("[Tauri] Successfully created new window with label: {}", label);
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("[Tauri] Window build failed: {}", e);
            eprintln!("{}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
async fn open_new_window(app: tauri::AppHandle, url: String) -> Result<(), String> {
    build_new_window(&app, url)
}

#[tauri::command]
fn set_zoom_level(state: tauri::State<'_, ZoomState>, level: f64) -> Result<(), String> {
    let mut zoom = state.level.lock().map_err(|e| e.to_string())?;
    *zoom = level;
    println!("[Tauri] Zoom seviyesi kaydedildi: {:.0}%", level * 100.0);
    Ok(())
}

#[tauri::command]
fn get_zoom_level(state: tauri::State<'_, ZoomState>) -> Result<f64, String> {
    let zoom = state.level.lock().map_err(|e| e.to_string())?;
    Ok(*zoom)
}

#[tauri::command]
async fn reopen_with_proxy(app: tauri::AppHandle) -> Result<(), String> {
    println!("[Tauri] reopen_with_proxy çağrıldı.");
    // Sadece proxy'yi başlat. Pencere açma/kapatma yapmıyoruz çünkü
    // bu Tauri'yi çökertiyor. Proxy en baştan başlatılıp pencere
    // direkt --proxy-server ile açılmalı.
    let dpi = app.state::<dpi_proxy::DpiProxyManager>();
    if let Err(e) = dpi.start_proxy(&app, 1).await {
        eprintln!("[Tauri] Proxy #1 başlatılamadı: {}", e);
    }
    println!("[Tauri] Proxy başlatıldı. (not: WebView proxy kullanmıyor olabilir)");
    Ok(())
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
        println!("[Tauri] Pencere kapatıldı: {}", target);
    } else {
        println!("[Tauri] Kapatılacak pencere bulunamadı: {}", target);
    }
    Ok(())
}

// (proxy_request kaldırıldı — hiçbir JS/frontend tarafından çağrılmıyordu.)


#[tauri::command]
async fn fetch_css(url: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .user_agent("OpenAnime-Desktop/1.0")
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
            let status = resp.status();
            status.is_success() || status.is_redirection() || status == reqwest::StatusCode::FORBIDDEN
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
    println!("[Tauri] Navigating online to: {}", url_str);
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
    println!("[Tauri] Navigating offline to: {}", url);
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
    println!("[Tauri] Emitting theme-apply for theme: {}", theme_id);
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
        log!("[JS {}] {}", level, m);
        if n == 299 {
            log!("[JS] mesaj limiti (300) doldu — sonraki JS logları bastırılıyor");
        }
    }
}

#[cfg(target_os = "windows")]
fn setup_windows_gpu_preference() {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_str) = exe_path.to_str() {
            println!(
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

/// ────────────────────────────────────────────────────────────
/// 📁 Dosya Seçme Dialogu
/// ────────────────────────────────────────────────────────────
/// Kullanıcının işletim sistemi dosya seçme dialogu ile MP4
/// dosyası seçmesini sağlar. Seçilen dosyanın tam yolunu döndürür.
/// ────────────────────────────────────────────────────────────
#[tauri::command]
async fn pick_mp4_file() -> Result<String, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Yerel Video Dosyası Seç")
        .add_filter("Video Dosyaları", &["mp4", "mkv", "webm", "avi", "mov"])
        .pick_file()
        .await
        .ok_or_else(|| "Kullanıcı dosya seçmedi".to_string())?;
    
    let path = file.path().to_string_lossy().to_string();
    println!("[LocalLibrary] Seçilen dosya: {}", path);
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
        log!("{}", report);

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
        let app_id = "com.darkhunter.openanime-desktops";
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
        log!("[LocalVideo] ✅ Server başlatıldı: 127.0.0.1:{}", port);
    } else {
        log!("[LocalVideo] ❌ Server başlatılamadı!");
    }
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(discordRPC::DiscordState::new())
        .manage(updater::UpdaterState::new())
        .manage(ZoomState::default())
        .manage(PerfState::default())
        .manage(local_video_state);

    // DPI Proxy manager'ı oluştur (setup'tan önce olmalı)
    // .manage()'i setup'tan sonra kullanacağız

    let builder = builder.setup(|app| {
        // Logger'ı en başta başlat
        logger::init(app.handle());

        log!("===== OPENANIME SETUP BAŞLADI =====");
        log!("[Setup] Build modu: {}", if cfg!(debug_assertions) { "DEBUG" } else { "RELEASE" });
        log!("[Setup] Platform: {}", std::env::consts::OS);

        // DPI Proxy manager'ı başlat
        let app_handle = app.handle().clone();
        let dpi_manager = dpi_proxy::DpiProxyManager::new(&app_handle);
        app.manage(dpi_manager);
        let user_agent = platform_user_agent();

        // DPI proxy'yi en baştan başlat (arkaplan) - Windows için 3 adımlı bağlantı doğrulama akışı
        #[cfg(target_os = "windows")]
        {
            let dpi = app.state::<dpi_proxy::DpiProxyManager>();
            let method_id = {
                let settings = tauri::async_runtime::block_on(async { dpi.settings.lock().await });
                settings.active_method_id.unwrap_or(0) // Default to 0 (Direct) or 1 if none
            };
            log!("[Setup] Yerel proxy sunucusu başlatılıyor (yöntem #{})...", method_id);
            let _ = tauri::async_runtime::block_on(async {
                dpi.start_proxy(&app_handle, method_id).await
            });
        }

        #[cfg(target_os = "windows")]
        let (browser_args, proxy_status_msg) = (WINDOWS_PROXY_ARGS, "Proxy AKTİF (127.0.0.1:1453)");

        #[cfg(not(target_os = "windows"))]
        let proxy_status_msg = "Proxy DEVADIŞI";

        log!("[Setup] WebView modu: {}", proxy_status_msg);

        #[cfg(target_os = "windows")]
        let app_handle_for_check = app_handle.clone();
        #[cfg(target_os = "windows")]
        tauri::async_runtime::spawn(async move {
            let dpi = app_handle_for_check.state::<dpi_proxy::DpiProxyManager>();
            log!("[Setup Background] Arkaplan bağlantı kontrolü başladı...");

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

            match direct_check {
                Ok(dpi_proxy::ConnectionResult::Ok) => {
                    log!("[Setup Background] ✅ Doğrudan bağlantı başarılı, bypass gereksiz.");
                    let mut stage = dpi.connection_stage.lock().await;
                    *stage = "success".to_string();
                    let _ = dpi.start_proxy(&app_handle_for_check, 0).await;
                }
                _ => {
                    log!("[Setup Background] ❌ Doğrudan bağlantı başarısız. Adım 2: DPI bypass deneniyor...");
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
                    if let Ok(dpi_proxy::ConnectionResult::Ok) = proxy_check {
                        log!("[Setup Background] ✅ Kayıtlı DPI yöntemi (#{}) çalışıyor!", test_id);
                        is_working = true;
                    }

                    if !is_working {
                        log!("[Setup Background] Kayıtlı DPI yöntemi çalışmadı. Tüm yöntemler taranıyor...");
                        if let Some(working_id) = dpi.test_all_methods(&app_handle_for_check).await {
                            log!("[Setup Background] ✅ Çalışan yeni DPI yöntemi bulundu: #{}", working_id);
                            is_working = true;
                        }
                    }

                    if is_working {
                        let mut stage = dpi.connection_stage.lock().await;
                        *stage = "success".to_string();
                    } else {
                        // ADIM 3: Proxy Fallback
                        log!("[Setup Background] ❌ Tüm DPI yöntemleri başarısız. Adım 3: Uzak proxy fallback deneniyor...");
                        match dpi.try_remote_proxy_fallback(&app_handle_for_check).await {
                            Ok(_) => {
                                log!("[Setup Background] ✅ Uzak proxy fallback başarılı!");
                            }
                            Err(_) => {
                                log!("[Setup Background] ❌ Tüm bağlantı adımları başarısız. Çevrimdışı moda düşülüyor.");
                                let mut stage = dpi.connection_stage.lock().await;
                                *stage = "failed".to_string();
                            }
                        }
                    }
                }
            }
        });

        let main_url = WebviewUrl::External("https://openani.me/".parse().unwrap());
        log!("[Setup] Ana URL: https://openani.me/");
        log!("[Setup] Pencere oluşturuluyor (1280x848, decorations: false)...");

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
            println!(
                "[Tauri] Yeni pencere isteği (main penceresinden): {}",
                url
            );
            let app_c = app_handle.clone();
            let url_str = url.to_string();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = open_new_window(app_c, url_str).await {
                    eprintln!("[Tauri] Yeni pencere açma hatası: {}", e);
                }
            });
            tauri::webview::NewWindowResponse::Deny
        })
        .initialization_script(build_init_script());

        #[cfg(target_os = "windows")]
        let win_builder = win_builder.additional_browser_args(browser_args);

        log!("[Setup] Pencere build ediliyor...");
        match win_builder.build() {
            Ok(_window) => {
                log!("[Setup] ✅ Ana pencere başarıyla oluşturuldu (label: main)");
                log!("[Setup] WebView URL: https://openani.me/");

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

                log!("===== OPENANIME SETUP TAMAM =====");
                Ok(())
            }
            Err(e) => {
                log!("[Setup] ❌ ANA PENCERE OLUŞTURULAMADI: {}", e);
                log!("===== OPENANIME SETUP HATA =====");
                Err(Box::new(e))
            }
        }
    })
        .on_window_event(|window, event| {
            let app_handle = window.app_handle().clone();
            let label = window.label().to_string();

            match event {
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
            // 🎥 Local video server — port sorgula
            get_local_video_port,
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
            updater::check_for_updates,
            updater::start_update_download,
            // DPI Proxy komutları
            reopen_with_proxy,
            set_zoom_level,
            get_zoom_level,
            dpi_proxy::dpi_test_methods,
            dpi_proxy::dpi_get_status,
            // 📁 Yerel dosya seçme dialogu
            pick_mp4_file,
            // 📄 Dosyanın ilk N baytını oku (IndexedDB dummy blob için)
            read_file_head,
            // JS hata köprüsü (webview console/onerror → terminal log)
            oa_js_log,
            // Performans/verimlilik modu — JS oynatıcı durumunu bildirir
            oa_set_player_playing
        ])
        .run(tauri::generate_context!());

    if let Err(err) = run_result {
        log!("[Fatal] Tauri uygulaması başlatılamadı: {}", err);
        show_fatal_startup_error(&err);
        std::process::exit(1);
    }
}