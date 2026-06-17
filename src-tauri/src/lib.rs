use tauri::{WebviewWindowBuilder, WebviewUrl, Manager};

#[cfg(target_os = "windows")]
#[link(name = "shell32")]
extern "system" {
    fn SetCurrentProcessExplicitAppUserModelID(app_id: *const u16) -> i32;
}

#[cfg(target_os = "macos")]
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


#[tauri::command]
async fn open_new_window(app: tauri::AppHandle, url: String) -> Result<(), String> {
    println!("[Tauri] open_new_window invoked with URL: {}", url);
    let label = format!("win_{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0));

    #[cfg(target_os = "windows")]
    let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1";
    #[cfg(target_os = "linux")]
    let user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1";
    #[cfg(target_os = "macos")]
    let user_agent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1";
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    let user_agent = "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1";

    let win_builder = WebviewWindowBuilder::new(
        &app,
        &label,
        WebviewUrl::External(tauri::Url::parse(&url).map_err(|e| {
            let err_msg = format!("[Tauri] Url parse error: {}", e);
            eprintln!("{}", err_msg);
            err_msg
        })?)
    )
    .title("OpenAnime")
    .inner_size(1280.0, 848.0)
    .min_inner_size(800.0, 500.0)
    .center()
    .decorations(false)
    .zoom_hotkeys_enabled(true)
    .user_agent(user_agent);

    // OpenAnime modülleri - tek IIFE içinde birleştiriliyor (shared scope = .bak mantığı)
    // concat!() kullanıyoruz çünkü format!() JS'deki {} karakterlerini bozuyor
    let win_builder = win_builder
        .initialization_script(concat!(
            "(function () {\nif (window.self !== window.top) return;\n",
            include_str!("js/modules/tauri-bridge.js"),
            "\n",
            include_str!("js/modules/webgpu-patcher.js"),
            "\n",
            include_str!("js/modules/performance-css.js"),
            "\n",
            include_str!("js/modules/zoom-manager.js"),
            "\n",
            include_str!("js/modules/window-controls.js"),
            "\n",
            include_str!("js/modules/keyboard-shortcuts.js"),
            "\n",
            include_str!("js/modules/link-interceptor.js"),
            "\n",
            include_str!("js/modules/fullscreen-manager.js"),
            "\n",
            include_str!("js/init.js"),
            "\n})();"
        ));

    #[cfg(target_os = "windows")]
    let win_builder = win_builder
        .additional_browser_args("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --enable-features=ParallelDownloading --enable-quic --enable-fast-unload --js-flags=\"--max-old-space-size=512 --optimize-for-size\" --force-gpu-selection=high-performance --force_high_performance_gpu");

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

#[cfg(target_os = "windows")]
fn setup_windows_gpu_preference() {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_str) = exe_path.to_str() {
            println!("[Tauri] Setting DirectX GpuPreference to High Performance for: {}", exe_str);
            let _ = std::process::Command::new("reg")
                .args(&[
                    "add",
                    "HKCU\\Software\\Microsoft\\DirectX\\UserGpuPreferences",
                    "/v",
                    exe_str,
                    "/t",
                    "REG_SZ",
                    "/d",
                    "GpuPreference=2;",
                    "/f",
                ])
                .output();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    {
        setup_windows_gpu_preference();
        let app_id = "com.darkhunter.openanime-desktops";
        let wide_id: Vec<u16> = app_id.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(wide_id.as_ptr());
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(target_os = "windows")]
            let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1";
            #[cfg(target_os = "linux")]
            let user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1";
            #[cfg(target_os = "macos")]
            let user_agent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1";
            #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
            let user_agent = "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 OpenAnime/0.1.0 (Desktop) Tauri/1.0.1";

            let win_builder = WebviewWindowBuilder::new(
                app, 
                "main", 
                WebviewUrl::External("https://openani.me/".parse().unwrap())
            )
            .title("OpenAnime")
            .inner_size(1280.0, 848.0)
            .min_inner_size(800.0, 500.0)
            .center()
            .decorations(false)
            .zoom_hotkeys_enabled(true)
            .user_agent(user_agent);

            // OpenAnime modülleri - tek IIFE içinde birleştiriliyor (shared scope = .bak mantığı)
            // concat!() kullanıyoruz çünkü format!() JS'deki {} karakterlerini bozuyor
            let win_builder = win_builder
                .initialization_script(concat!(
                    "(function () {\nif (window.self !== window.top) return;\n",
                    include_str!("js/modules/tauri-bridge.js"),
                    "\n",
                    include_str!("js/modules/webgpu-patcher.js"),
                    "\n",
                    include_str!("js/modules/performance-css.js"),
                    "\n",
                    include_str!("js/modules/zoom-manager.js"),
                    "\n",
                    include_str!("js/modules/window-controls.js"),
                    "\n",
                    include_str!("js/modules/keyboard-shortcuts.js"),
                    "\n",
                    include_str!("js/modules/link-interceptor.js"),
                    "\n",
                    include_str!("js/modules/fullscreen-manager.js"),
                    "\n",
                    include_str!("js/init.js"),
                    "\n})();"
                ));

            #[cfg(target_os = "windows")]
            let win_builder = win_builder
                .additional_browser_args("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --enable-features=ParallelDownloading --enable-quic --enable-fast-unload --js-flags=\"--max-old-space-size=512 --optimize-for-size\" --force-gpu-selection=high-performance --force_high_performance_gpu");

            win_builder.build()?;
            Ok(())
        })
        .on_window_event(|window, event| {
            #[cfg(target_os = "windows")]
            {
                if window.label() == "main" {
                    if let tauri::WindowEvent::Focused(focused) = event {
                        let is_focused = *focused;
                        if let Some(webview_window) = window.app_handle().get_webview_window(window.label()) {
                            let _ = webview_window.with_webview(move |webview| {
                                unsafe {
                                    use webview2_com::Microsoft::Web::WebView2::Win32::{
                                        ICoreWebView2_19,
                                        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
                                        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
                                    };
                                    use windows_core::Interface;
                                    
                                    let controller = webview.controller();
                                    if !Interface::as_raw(&controller).is_null() {
                                        if let Ok(core_webview) = controller.CoreWebView2() {
                                            if let Ok(webview_19) = core_webview.cast::<ICoreWebView2_19>() {
                                                let level = if is_focused {
                                                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
                                                } else {
                                                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
                                                };
                                                let _ = webview_19.SetMemoryUsageTargetLevel(level);
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            open_new_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}