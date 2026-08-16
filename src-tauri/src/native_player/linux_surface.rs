// ═══════════════════════════════════════════════════════════
// Linux Yüzey İskeleti — GTK penceresi + wgpu surface + olay döngüsü
// ═══════════════════════════════════════════════════════════
//
// B-basic'in ÇALIŞTIRILABİLİR en küçük kabuğu. Şimdilik gerçek video decode
// YOKTUR — kare kaynağı bir TEST DESENİDİR (make_test_frame). Amaç pipeline'ın
// (scale + LUT + FRC) doğru kurulduğunu ve çizdiğini görsel olarak doğrulamak.
//
// GERÇEK KARE KAYNAĞI (sonraki aşama): video demux/decode katmanı (ffmpeg/
// GStreamer) RGBA8 kare üretip `player.render(...)`'e verir. Bu katman
// B-basic'in DIŞINDADIR; buradaki render çağrısı onunla birebir aynıdır.
//
// NOT: Bu dosya Linux'ta derlenir ve DOĞRULANMAMIŞTIR (Windows'ta cargo check
// cfg(target_os="linux") olduğu için atlar). wgpu/gtk4 sürüm API'si değişirse
// derleme hatası veren ilk yerler şunlardır: instance.create_surface(...),
// request_device(...) ikinci argümanı, SurfaceConfiguration alanları.

use gtk4::glib;
use gtk4::prelude::*;

use crate::native_player::pipeline::{self, WebGpuPlayer};

/// Test karesi: zamanla değişen bir degraden RGBA8'i (640×360).
/// Pipeline'ı görsel doğrulamak için yeterli; gerçek decode ile değiştirilecek.
fn make_test_frame(frame_count: u32) -> Vec<u8> {
    const W: u32 = 640;
    const H: u32 = 360;
    let mut out = Vec::with_capacity((W * H * 4) as usize);
    let hue = (frame_count as f32 * 0.01).sin() * 0.5 + 0.5;
    for y in 0..H {
        for x in 0..W {
            let r = (x as f32 / W as f32 * 255.0) as u8;
            let g = (y as f32 / H as f32 * 255.0) as u8;
            let b = (hue * 255.0) as u8;
            out.extend_from_slice(&[r, g, b, 255]);
        }
    }
    out
}

/// GTK + wgpu'yu kurup render döngüsünü başlatır. Bloklayan giriş noktası.
pub fn run() {
    let app = gtk4::Application::builder()
        .application_id("com.openanime.nativeplayer")
        .build();

    app.connect_activate(|app| {
        let window = gtk4::ApplicationWindow::builder()
            .application(app)
            .title("OpenAnime Native Player (test)")
            .default_width(1280)
            .default_height(720)
            .build();

        // ── wgpu kurulumu ──
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = match instance.create_surface(window.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[NativePlayer] yüzey oluşturulamadı: {e}");
                return;
            }
        };

        let adapter = match pollster::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        )) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("[NativePlayer] adapter alınamadı: {e}");
                return;
            }
        };

        let (device, queue) = match pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("oa_native_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        )) {
            Ok(dq) => dq,
            Err(e) => {
                eprintln!("[NativePlayer] device alınamadı: {e}");
                return;
            }
        };

        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let (width, height) = (1280u32, 720u32);
        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            },
        );

        // Test karesi 640×360, pencere 1280×720 → scale pass çalışır.
        let mut player = WebGpuPlayer::new(
            &device,
            &queue,
            surface_format,
            None,          // LUT yok → kimlik LUT
            [640, 360],    // kare boyutu
            [width, height],
        );

        // ── Render döngüsü (~60fps) ──
        let mut frame_count: u32 = 0;
        glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            frame_count += 1;
            let frame = make_test_frame(frame_count);
            if let Ok(st) = surface.get_current_texture() {
                let view = st.texture.create_view(&wgpu::TextureViewDescriptor::default());
                if let Err(e) = player.render(&device, &queue, &view, &frame, frame_count, false) {
                    eprintln!("[NativePlayer] render hatası: {e}");
                }
                st.present();
            }
            glib::ControlFlow::Continue
        });

        window.present();
    });

    app.run();
}
