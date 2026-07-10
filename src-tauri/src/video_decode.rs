#[cfg(target_os = "linux")]
pub mod inner {
    use gstreamer::prelude::*;
    use std::sync::{Arc, Mutex, Condvar};
    use std::thread;
    use std::time::Duration;
    use tauri::Emitter;

    pub struct DecodedFrame {
        pub width: u32,
        pub height: u32,
        pub data: Vec<u8>,
        pub new_frame_available: bool,
    }

    pub struct FrameSignal {
        pub frame: Mutex<Option<DecodedFrame>>,
        pub condvar: Condvar,
        pub is_running: Mutex<bool>,
    }

    // `#[derive(Clone)]` is safe here only because every field is a cheap,
    // reference-counted handle rather than owned/unique state:
    //   - `playbin: gstreamer::Element` is a GObject wrapper (via glib) --
    //     cloning it bumps a refcount and yields another handle to the SAME
    //     underlying pipeline, it does not construct a second pipeline.
    //   - `frame_signal: Arc<FrameSignal>` and `is_playing: Arc<Mutex<bool>>`
    //     are both `Arc`, so cloning shares the same signal/state.
    // If a field is ever added here that owns unique resources (e.g. a raw
    // handle, a non-refcounted pipeline object, a file descriptor), this
    // derive must be removed or replaced with a manual `Clone` impl that
    // does NOT duplicate the pipeline.
    #[derive(Clone)]
    pub struct GstPlayer {
        playbin: gstreamer::Element,
        frame_signal: Arc<FrameSignal>,
        is_playing: Arc<Mutex<bool>>,
    }

    impl GstPlayer {
        pub fn new(url: &str, window: tauri::WebviewWindow) -> Result<Self, String> {
            gstreamer::init().map_err(|e| format!("GStreamer init failed: {}", e))?;

            // Create playbin3 element
            let playbin = gstreamer::ElementFactory::make("playbin3")
                .name("player")
                .build()
                .map_err(|e| format!("Failed to create playbin3: {}", e))?;

            // Optimization: Parallelize YUV->RGBA conversion with videoconvert using 4 threads
            let bin = gstreamer::parse::bin_from_description(
                "videoconvert n-threads=4 ! video/x-raw,format=RGBA ! appsink name=sink sync=true",
                true,
            )
            .map_err(|e| format!("Failed to parse video sink description: {}", e))?;

            // Set video-sink and uri properties on playbin
            playbin.set_property("video-sink", &bin);
            playbin.set_property("uri", url);

            // Retrieve the appsink element from the custom bin
            let appsink_el = bin.by_name("sink").ok_or("Appsink 'sink' not found in bin")?;
            let appsink = appsink_el
                .dynamic_cast::<gstreamer_app::AppSink>()
                .map_err(|_| "Failed to cast element to AppSink")?;

            let frame_signal = Arc::new(FrameSignal {
                frame: Mutex::new(None),
                condvar: Condvar::new(),
                is_running: Mutex::new(true),
            });
            let frame_signal_clone = frame_signal.clone();

            // Configure appsink callbacks to intercept decoded RGBA frames
            appsink.set_callbacks(
                gstreamer_app::AppSinkCallbacks::builder()
                    .new_sample(move |sink| {
                        let sample = sink.pull_sample().map_err(|_| gstreamer::FlowError::Error)?;
                        let buffer = sample.buffer().ok_or(gstreamer::FlowError::Error)?;
                        
                        // Extract frame dimensions from caps
                        let caps = sample.caps().ok_or(gstreamer::FlowError::Error)?;
                        let structure = caps.structure(0).ok_or(gstreamer::FlowError::Error)?;
                        let width = structure.get::<i32>("width").map_err(|_| gstreamer::FlowError::Error)? as u32;
                        let height = structure.get::<i32>("height").map_err(|_| gstreamer::FlowError::Error)? as u32;

                        if let Ok(map) = buffer.map_readable() {
                            let mut guard = frame_signal_clone
                                .frame
                                .lock()
                                .unwrap_or_else(|p| p.into_inner());
                            
                            if let Some(ref mut frame) = *guard {
                                frame.width = width;
                                frame.height = height;
                                if frame.data.len() != map.len() {
                                    frame.data.resize(map.len(), 0);
                                }
                                frame.data.copy_from_slice(&map);
                                frame.new_frame_available = true;
                            } else {
                                *guard = Some(DecodedFrame {
                                    width,
                                    height,
                                    data: map.to_vec(),
                                    new_frame_available: true,
                                });
                            }
                            frame_signal_clone.condvar.notify_one();
                        }

                        Ok(gstreamer::FlowSuccess::Ok)
                     })
                     .build(),
            );

            // Position synchronization thread
            let playbin_clone = playbin.clone();
            let is_playing = Arc::new(Mutex::new(true));
            let is_playing_clone = is_playing.clone();
            let window_clone = window.clone();

            thread::spawn(move || {
                loop {
                    {
                        let playing = is_playing_clone.lock().unwrap_or_else(|p| p.into_inner());
                        if !*playing {
                            break;
                        }
                    }

                    if let Some(time) = playbin_clone.query_position::<gstreamer::ClockTime>() {
                        let seconds = time.seconds() as f64 + (time.mseconds() as f64 / 1000.0) % 1.0;
                        let js_code = format!("if (window.__TAURI_GST_TIME_SYNC__) window.__TAURI_GST_TIME_SYNC__({});", seconds);
                        let _ = window_clone.eval(&js_code);
                    }

                    thread::sleep(Duration::from_millis(200));
                }
            });

            // GStreamer Pipeline Bus Message Monitoring
            let bus = playbin.bus().ok_or("Failed to get pipeline bus")?;
            let is_playing_bus_clone = is_playing.clone();
            let frame_signal_bus_clone = frame_signal.clone();
            let window_bus_clone = window.clone();

            thread::spawn(move || {
                for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
                    use gstreamer::MessageView;
                    match msg.view() {
                        MessageView::Error(err) => {
                            eprintln!("[GStreamer Error] {}", err.error());
                            let _ = window_bus_clone.emit("openanime://gst-fallback", err.error().to_string());
                            break;
                        }
                        MessageView::Eos(_) => {
                            println!("[GStreamer] End of Stream reached.");
                            break;
                        }
                        MessageView::AsyncDone(_) => {
                            // Preroll for the async Playing transition
                            // triggered in play() has now completed.
                            println!("[GStreamer] AsyncDone received, preroll complete.");
                        }
                        _ => {}
                    }
                }
                
                // Signal termination on error or end of stream
                {
                    let mut playing = is_playing_bus_clone.lock().unwrap_or_else(|p| p.into_inner());
                    *playing = false;
                }
                let mut running = frame_signal_bus_clone
                    .is_running
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                *running = false;
                frame_signal_bus_clone.condvar.notify_all();
            });

            // NOTE: playback is intentionally NOT started here anymore.
            // The pipeline is constructed in its default NULL/READY state;
            // the caller must call `play()` explicitly once construction
            // has finished, so that the (possibly async) state change
            // never happens while any manager lock is held.
            Ok(Self {
                playbin,
                frame_signal,
                is_playing,
            })
        }

        pub fn play(&self) -> Result<(), String> {
            let result = self
                .playbin
                .set_state(gstreamer::State::Playing)
                .map_err(|e| format!("Failed to resume GStreamer: {}", e))?;

            if result == gstreamer::StateChangeSuccess::Async {
                // The pipeline hasn't finished prerolling yet. Don't block
                // waiting for it here -- the bus message loop below watches
                // for `MessageView::AsyncDone` and logs when preroll
                // actually completes.
                println!("[GStreamer] Playing state change is ASYNC, waiting for AsyncDone on the bus.");
            }
            Ok(())
        }

        pub fn pause(&self) -> Result<(), String> {
            self.playbin
                .set_state(gstreamer::State::Paused)
                .map_err(|e| format!("Failed to pause GStreamer: {}", e))?;
            Ok(())
        }

        pub fn seek(&self, seconds: f64) -> Result<(), String> {
            let clock_time = gstreamer::ClockTime::from_seconds(seconds as u64);
            self.playbin
                .seek_simple(
                    gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT,
                    clock_time,
                )
                .map_err(|e| format!("Failed to seek GStreamer: {}", e))?;
            Ok(())
        }

        pub fn get_frame_signal(&self) -> Arc<FrameSignal> {
            self.frame_signal.clone()
        }
    }

    impl Drop for GstPlayer {
        fn drop(&mut self) {
            {
                let mut playing = self.is_playing.lock().unwrap_or_else(|p| p.into_inner());
                *playing = false;
            }
            {
                let mut running = self
                    .frame_signal
                    .is_running
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                *running = false;
                self.frame_signal.condvar.notify_all();
            }
            let _ = self.playbin.set_state(gstreamer::State::Null);
        }
    }
}