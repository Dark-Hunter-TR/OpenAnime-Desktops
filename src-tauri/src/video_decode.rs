#[cfg(target_os = "linux")]
pub mod inner {
    use gstreamer::prelude::*;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    pub struct DecodedFrame {
        pub width: u32,
        pub height: u32,
        pub data: Vec<u8>,
    }

    pub struct GstPlayer {
        playbin: gstreamer::Element,
        latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
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

            // Create our video sink bin: videoconvert -> appsink
            let bin = gstreamer::parse_bin_from_description(
                "videoconvert ! video/x-raw,format=RGBA ! appsink name=sink sync=true",
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

            let latest_frame = Arc::new(Mutex::new(None));
            let latest_frame_clone = latest_frame.clone();

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
                            let mut frame_guard = latest_frame_clone.lock().unwrap();
                            *frame_guard = Some(DecodedFrame {
                                width,
                                height,
                                data: map.to_vec(),
                            });
                        }

                        Ok(gstreamer::FlowSuccess::Ok)
                    })
                    .build(),
            );

            // Spawn a background thread to periodically query playbin position and sync it back to Javascript
            let playbin_clone = playbin.clone();
            let is_playing = Arc::new(Mutex::new(true));
            let is_playing_clone = is_playing.clone();
            let window_clone = window.clone();

            thread::spawn(move || {
                loop {
                    {
                        let playing = is_playing_clone.lock().unwrap();
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

            // Start playing the stream
            playbin
                .set_state(gstreamer::State::Playing)
                .map_err(|e| format!("Failed to set playbin state to Playing: {}", e))?;

            Ok(Self {
                playbin,
                latest_frame,
                is_playing,
            })
        }

        pub fn play(&self) -> Result<(), String> {
            self.playbin
                .set_state(gstreamer::State::Playing)
                .map_err(|e| format!("Failed to resume GStreamer: {}", e))?;
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

        pub fn get_latest_frame(&self) -> Option<DecodedFrame> {
            let mut guard = self.latest_frame.lock().unwrap();
            guard.take()
        }
    }

    impl Drop for GstPlayer {
        fn drop(&mut self) {
            // Signal position sync thread to terminate
            if let Ok(mut playing) = self.is_playing.lock() {
                *playing = false;
            }
            // Stop and release GStreamer playbin pipeline
            let _ = self.playbin.set_state(gstreamer::State::Null);
        }
    }
}
