#[cfg(target_os = "linux")]
pub mod inner {
    use std::sync::{Arc, Mutex};
    use std::thread;
    use tauri::{Emitter, Manager, WebviewWindow, Window, window::WindowBuilder};
    use crate::video_decode::inner::GstPlayer;
    use crate::renderer::WebGpuRenderer;

    pub struct RenderState {
        pub renderer: WebGpuRenderer,
    }

    impl RenderState {
        pub async fn new(window: Window) -> Result<Self, String> {
            // Initialize the WebGpuRenderer with VSync enabled
            let renderer = WebGpuRenderer::new(window, true).await?;
            Ok(Self { renderer })
        }

        pub fn resize(&mut self, new_width: u32, new_height: u32) {
            self.renderer.resize(new_width, new_height);
        }

        pub fn update_video_texture(&mut self, frame_width: u32, frame_height: u32, rgba_data: &[u8]) {
            self.renderer.update_video_texture(frame_width, frame_height, rgba_data);
        }

        pub fn prepare_and_submit(&mut self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
            self.renderer.prepare_and_submit()
        }
    }

    // Shared global state for managing the player instance and rendering loop
    pub struct NativePlayerManager {
        pub player: Option<GstPlayer>,
        pub render_state: Option<Arc<Mutex<RenderState>>>,
        pub overlay_window: Option<Window>,
        pub teardown_thread: Option<std::thread::JoinHandle<()>>,
        /// Son bilinen viewport bounds (x, y, w, h) — ana pencere taşınınca
        /// ekran konumunu yeniden hesaplamak için.
        pub last_bounds: Option<(i32, i32, u32, u32)>,
    }

    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU32, Ordering};
    static MANAGER: OnceLock<Mutex<NativePlayerManager>> = OnceLock::new();
    static CONSECUTIVE_LOCK_FAILURES: AtomicU32 = AtomicU32::new(0);

    pub fn get_manager() -> &'static Mutex<NativePlayerManager> {
        MANAGER.get_or_init(|| Mutex::new(NativePlayerManager {
            player: None,
            render_state: None,
            overlay_window: None,
            teardown_thread: None,
            last_bounds: None,
        }))
    }

    /// Starts the native GStreamer + WGPU overlay player.
    pub async fn start_player(url: &str, main_window: WebviewWindow, start_paused: bool) -> Result<(), String> {
        let app: tauri::AppHandle = main_window.app_handle().clone();

        // 1. Wait for any previous teardown thread to complete to ensure hardware
        // decoder resources and GStreamer state transitions are clean.
        {
            let mut manager = get_manager().lock().unwrap_or_else(|p| p.into_inner());
            let previous_teardown = manager.teardown_thread.take();
            drop(manager);

            if let Some(handle) = previous_teardown {
                println!("[Native Render] Waiting for previous GStreamer teardown thread to join...");
                let _ = handle.join();
            }
        }

        // 2. Destroy any existing overlay/player before creating new ones,
        // so we never have two GStreamer pipelines or overlay windows alive
        // at once. Destroying windows and dropping players must happen outside
        // the MANAGER lock to avoid blocking other calls.
        {
            let mut manager = get_manager().lock().unwrap_or_else(|p| p.into_inner());
            let old_win = manager.overlay_window.take();
            let old_player = manager.player.take();
            manager.render_state = None;
            
            drop(manager);

            if let Some(win) = old_win {
                let win_clone = win.clone();
                let app_clone = app.clone();
                let _ = app_clone.run_on_main_thread(move || {
                    let _ = win_clone.close();
                });
            }
            drop(old_player);
        }

        // 2. Build the transparent, borderless overlay window ON THE MAIN
        // THREAD. Window building touches GTK/X11 internals and must never
        // run from a Tokio worker thread.
        //
        // Webview'sız düz pencere kullanılır: wgpu'nun tek ihtiyacı bir raw
        // window handle. Önceden her overlay tam bir WebKit webview başlatıp
        // bundled frontend'i yüklüyordu (~100 MB israf + webkit 2.44+
        // renderer bug'larına ekstra maruziyet).
        let (window_tx, window_rx) = tokio::sync::oneshot::channel::<Result<Window, String>>();
        let (realize_tx, realize_rx) = tokio::sync::oneshot::channel::<()>();
        let realize_tx = Arc::new(Mutex::new(Some(realize_tx)));

        let app_for_build = app.clone();
        app.run_on_main_thread(move || {
            let mut builder = WindowBuilder::new(&app_for_build, "gst_overlay")
                .title("Video Overlay")
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .focused(false)
                .skip_taskbar(true);
            // NOT: pencere görünür yaratılır (transparent olduğundan ilk kareye
            // dek zaten şeffaftır). visible(false) KULLANILAMAZ: tao/GTK'da
            // gizli pencere realize olmaz → GDK penceresi (XID) oluşmaz →
            // wgpu surface kurulamaz ve realize beklemesi zaman aşımına düşer.
            // Ana pencereye transient bağla (X11 z-order/minimize uyumu).
            // parent() self'i tüketir; hata pratikte yalnızca main penceresi
            // yokken oluşur — o durumda overlay zaten anlamsızdır, hata döndür.
            if let Some(parent) = app_for_build.get_window("main") {
                builder = match builder.parent(&parent) {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = window_tx.send(Err(format!("Overlay parent bağlanamadı: {}", e)));
                        return;
                    }
                };
            }
            let build_result = builder.build();

            match build_result {
                Ok(overlay) => {
                    // Overlay yalnızca görüntü çizer: fare olaylarını YUTMASIN —
                    // altındaki player kontrollerine tıklanabilsin. ("player'da
                    // bazı yerlere tıklanmıyor" sorununun kökü buydu.)
                    let _ = overlay.set_ignore_cursor_events(true);
                    let realize_tx_for_event = realize_tx.clone();
                    overlay.on_window_event(move |event| {
                        if matches!(event, tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_)) {
                            if let Some(tx) = realize_tx_for_event
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .take()
                            {
                                let _ = tx.send(());
                            }
                        }
                    });
                    let _ = window_tx.send(Ok(overlay));
                }
                Err(e) => {
                    let _ = window_tx.send(Err(format!("Failed to create overlay window: {}", e)));
                }
            }
        })
        .map_err(|e| format!("Failed to dispatch overlay creation to main thread: {}", e))?;

        let overlay = window_rx
            .await
            .map_err(|_| "Main thread dropped the overlay window channel".to_string())??;

        {
            let mut manager = get_manager().lock().unwrap_or_else(|p| p.into_inner());
            manager.overlay_window = Some(overlay.clone());
        }

        // Helper to cleanly close the overlay and reset manager references on errors
        let cleanup_on_error = |err_msg: String| -> String {
            eprintln!("[Native Render] Error starting native player: {}", err_msg);
            let app_for_close = app.clone();
            let overlay_for_close = overlay.clone();
            let _ = app_for_close.run_on_main_thread(move || {
                let _ = overlay_for_close.close();
            });
            if let Ok(mut manager) = get_manager().lock() {
                manager.overlay_window = None;
                manager.player = None;
                manager.render_state = None;
            }
            err_msg
        };

        // 3. Wait for the window to actually realize before touching Vulkan
        // at all. If it doesn't realize within 500ms, don't risk the
        // deadlock -- cancel the native path and fall back to the HTML5
        // player via the same event the GStreamer error handler uses.
        match tokio::time::timeout(std::time::Duration::from_millis(500), realize_rx).await {
            Ok(Ok(())) => {
                // Realize sonrası yeniden uygula: Wayland/GTK giriş bölgesini
                // haritalama sırasında sıfırlayabiliyor.
                let _ = overlay.set_ignore_cursor_events(true);
            }
            _ => {
                cleanup_on_error("Overlay window did not realize in time".to_string());
                let _ = main_window.emit(
                    "openanime://gst-fallback",
                    "Overlay window did not realize in time".to_string(),
                );
                return Ok(());
            }
        }

        // 4. Initialize the GStreamer player. It comes up in NULL/READY
        // state now -- playback is started explicitly via play() below,
        // outside of any manager lock.
        let player = match GstPlayer::new(url, main_window.clone()) {
            Ok(p) => p,
            Err(e) => return Err(cleanup_on_error(format!("GStreamer init failed: {}", e))),
        };
        let frame_signal = player.get_frame_signal();

        // 5. Initialize WGPU state on the now-realized overlay window.
        let render_state = match RenderState::new(overlay.clone()).await {
            Ok(rs) => rs,
            Err(e) => return Err(cleanup_on_error(format!("WGPU init failed: {}", e))),
        };
        let render_state_shared = Arc::new(Mutex::new(render_state));

        // 6. Start playback if not start_paused. Otherwise, pause to preroll
        // the first frame without starting playback. GStreamer's state change
        // can be asynchronous; preroll completion is tracked via AsyncDone on the bus.
        if !start_paused {
            if let Err(e) = player.play() {
                return Err(cleanup_on_error(format!("Failed to start GStreamer playback: {}", e)));
            }
        } else {
            if let Err(e) = player.pause() {
                return Err(cleanup_on_error(format!("Failed to pause GStreamer: {}", e)));
            }
        }

        {
            let mut manager = get_manager().lock().unwrap_or_else(|p| p.into_inner());
            manager.render_state = Some(render_state_shared.clone());
            manager.player = Some(player);
        }

        // 7. Spawn background render thread with Condvar synchronization.
        // We use a zero-allocation double-buffer frame swapping mechanism:
        // instead of allocating and deallocating Vec<u8> every single frame,
        // we swap the vector inside the DecodedFrame with our local buffer
        // using std::mem::swap (O(1)). GStreamer then writes to the swapped
        // buffer in the next frame. This completely removes allocation churn
        // and fixes UI stuttering/lag on Linux.
        let rs_ref = render_state_shared.clone();
        let main_window_for_thread = main_window.clone();
        let overlay_for_thread = overlay.clone();
        let mut overlay_shown = false;
        let mut local_frame_data = Vec::new();

        thread::spawn(move || {
            loop {
                let mut frame_width = 0;
                let mut frame_height = 0;

                // Wait for the next decoded frame using condition variable
                {
                    let mut guard = frame_signal.frame.lock().unwrap_or_else(|p| p.into_inner());
                    loop {
                        {
                            let running = frame_signal.is_running.lock().unwrap_or_else(|p| p.into_inner());
                            if !*running {
                                return;
                            }
                        }
                        if let Some(ref frame) = *guard {
                            if frame.new_frame_available {
                                break;
                            }
                        }
                        guard = frame_signal
                            .condvar
                            .wait(guard)
                            .unwrap_or_else(|p| p.into_inner());
                    }

                    if let Some(ref mut frame) = *guard {
                        frame_width = frame.width;
                        frame_height = frame.height;
                        std::mem::swap(&mut frame.data, &mut local_frame_data);
                        frame.new_frame_available = false;
                    }
                }

                // Render the frame immediately using our local zero-allocation buffer
                let presentation_result = {
                    let mut rs = rs_ref.lock().unwrap_or_else(|p| p.into_inner());
                    rs.update_video_texture(frame_width, frame_height, &local_frame_data);
                    rs.prepare_and_submit()
                };

                match presentation_result {
                    Ok(output) => {
                        output.present();
                        if !overlay_shown {
                            overlay_shown = true;
                            let _ = overlay_for_thread.show();
                            let _ = overlay_for_thread.set_ignore_cursor_events(true);
                        }
                    }
                    // Lost/Outdated: prepare_and_submit içindeki tek seferlik
                    // reconfigure+retry de başarısız olmuş demektir — bir
                    // sonraki karede yeniden dene, kareyi atla.
                    Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                        eprintln!("[Native Render] Surface lost/outdated, frame skipped; retrying next frame.");
                    }
                    Err(wgpu::SurfaceError::Timeout) => {
                        eprintln!("[Native Render] Surface timeout, frame skipped.");
                    }
                    // OutOfMemory kurtarılamaz: HTML5 player'a temiz geçiş yap
                    // ve render döngüsünden çık.
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        eprintln!("[Native Render] Surface out of memory — falling back to HTML5 player.");
                        let _ = main_window_for_thread.emit(
                            "openanime://gst-fallback",
                            "GPU out of memory".to_string(),
                        );
                        return;
                    }
                }
            }
        });

        Ok(())
    }

    pub fn stop_player() {
        // Pull the overlay window and player out of the manager under a
        // short-lived lock, then do the actual teardown work OUTSIDE that
        // lock. Both `overlay.close()` (needs the main thread) and dropping
        // `GstPlayer` (its Drop impl calls the blocking GStreamer
        // State::Null transition) can take a moment, and holding MANAGER
        // during either would stall any other thread waiting on it (e.g.
        // `sync_bounds` during a resize).
        let (overlay_opt, player_opt) = {
            let mut manager = get_manager().lock().unwrap_or_else(|p| p.into_inner());
            let overlay = manager.overlay_window.take();
            let player = manager.player.take();
            manager.render_state = None;
            (overlay, player)
        };

        if let Some(overlay) = overlay_opt {
            let app = overlay.app_handle().clone();
            let overlay_for_close = overlay.clone();
            // Window destruction is a GTK/main-thread operation, same as
            // creation -- dispatch it rather than calling close() from
            // whatever thread stop_player() happens to run on.
            let _ = app.run_on_main_thread(move || {
                let _ = overlay_for_close.close();
            });
        }

        // Drop the GstPlayer asynchronously in a separate thread to prevent blocking
        // the main UI thread during GStreamer's State::Null state transition.
        // Store the join handle in the manager so that it can be joined prior to starting a new player.
        if let Some(player) = player_opt {
            let handle = thread::spawn(move || {
                drop(player);
                println!("[Native Render] GStreamer player dropped asynchronously in background thread.");
            });
            let mut manager = get_manager().lock().unwrap_or_else(|p| p.into_inner());
            manager.teardown_thread = Some(handle);
        }

        println!("[Native Render] Native player stopped and overlay closed.");
    }

    pub fn sync_bounds(x: i32, y: i32, width: u32, height: u32, main_window: WebviewWindow) {
        let mut manager_guard = match get_manager().try_lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        manager_guard.last_bounds = Some((x, y, width, height));

        let overlay_opt = manager_guard.overlay_window.clone();
        let rs_shared_opt = manager_guard.render_state.clone();
        drop(manager_guard);

        if let Some(overlay) = overlay_opt {
            let scale_factor = main_window.scale_factor().unwrap_or(1.0);
            // KÖK NEDEN DÜZELTMESİ: viewport koordinatına ana pencerenin
            // EKRANDAKİ konumu eklenir (önceden yalnız ölçek çarpılıyordu —
            // X11'de overlay yanlış yere gidiyordu).
            let origin = main_window
                .inner_position()
                .unwrap_or(tauri::PhysicalPosition::new(0, 0));
            let physical_pos = tauri::PhysicalPosition::new(
                origin.x + (x as f64 * scale_factor).round() as i32,
                origin.y + (y as f64 * scale_factor).round() as i32,
            );
            let physical_size = tauri::PhysicalSize::new(
                (width as f64 * scale_factor) as u32,
                (height as f64 * scale_factor) as u32,
            );

            let _ = overlay.set_position(tauri::Position::Physical(physical_pos));
            let _ = overlay.set_size(tauri::Size::Physical(physical_size));
            // Tıklama geçirgenliğini her bounds senkronunda yeniden garanti et.
            let _ = overlay.set_ignore_cursor_events(true);

            // Resize wgpu surface configuration
            if let Some(rs_shared) = rs_shared_opt {
                if let Ok(mut rs) = rs_shared.try_lock() {
                    CONSECUTIVE_LOCK_FAILURES.store(0, Ordering::Relaxed);
                    rs.resize(physical_size.width, physical_size.height);
                } else {
                    let failures = CONSECUTIVE_LOCK_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
                    if failures % 10 == 0 {
                        eprintln!("[Native Render] Lock contention detected ({} failures), resize skipped.", failures);
                    }
                }
            }
        }
    }

    /// Ana pencere taşındığında overlay'i kayıtlı son bounds ile yeniden
    /// konumlandırır (lib.rs Moved event'inden çağrılır).
    pub fn reposition(app: &tauri::AppHandle) {
        let Some(main) = app.get_webview_window("main") else { return };
        let bounds = match get_manager().try_lock() {
            Ok(g) => g.last_bounds,
            Err(_) => return,
        };
        if let Some((x, y, w, h)) = bounds {
            sync_bounds(x, y, w, h, main);
        }
    }

    pub fn control_play() -> Result<(), String> {
        let manager = get_manager().lock().unwrap_or_else(|p| p.into_inner());
        if let Some(player) = &manager.player {
            player.play()?;
        }
        Ok(())
    }

    pub fn control_pause() -> Result<(), String> {
        let manager = get_manager().lock().unwrap_or_else(|p| p.into_inner());
        if let Some(player) = &manager.player {
            player.pause()?;
        }
        Ok(())
    }

    pub fn control_seek(time: f64) -> Result<(), String> {
        let manager = get_manager().lock().unwrap_or_else(|p| p.into_inner());
        if let Some(player) = &manager.player {
            player.seek(time)?;
        }
        Ok(())
    }
}