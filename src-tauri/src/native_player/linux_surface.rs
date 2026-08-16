// ═══════════════════════════════════════════════════════════
// Linux Yüzey İskeleti — winit penceresi + wgpu surface + olay döngüsü
// ═══════════════════════════════════════════════════════════
//
// B-basic'in ÇALIŞTIRILABİLİR en küçük kabuğu. Şimdilik gerçek video decode
// YOKTUR — kare kaynağı bir TEST DESENİDİR (make_test_frame). Amaç pipeline'ın
// (scale + LUT + FRC) doğru kurulduğunu ve çizdiğini görsel olarak doğrulamak.
//
// NEDEN winit (gtk4 değil): gtk4-rs 0.9 raw-window-handle entegrasyonu SUNMUYOR
// (Cargo.lock'ta doğrulandı — gtk4'ün deps listesinde raw-window-handle yok).
// gtk4 + wgpu yüzeyi, gdk4-x11/gdk4-wayland ile platforma özgü el kodlaması
// gerektiriyor. winit ise wgpu'nun standart pencere kütüphanesi ve
// raw-window-handle'ı ilk elden destekliyor.
//
// GTK OVERLAY (Tauri penceresinin üstüne çizim) SONRAKİ aşama: orada gtk4
// değil, tauri::Window'ın raw handle'ı kullanılacak (tauri::Window
// HasWindowHandle uygular). Yani GTK'ya erişim overlay aşamasında
// tauri::Window üzerinden olur — bu test kabuğu için gtk4 gerekmiyor.

use std::sync::Arc;

use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

use crate::native_player::pipeline::WebGpuPlayer;

/// Test karesi: zamanla değişen bir degraden RGBA8'i (640×360).
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

/// winit + wgpu'yu kurup render döngüsünü başlatır. Bloklayan giriş noktası.
pub fn run() {
    let event_loop = EventLoop::new().expect("olay döngüsü oluşturulamadı");
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("OpenAnime Native Player (test)")
            .with_inner_size(LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)
            .expect("pencere oluşturulamadı"),
    );

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let surface = instance
        .create_surface(window.clone())
        .expect("wgpu yüzeyi oluşturulamadı");

    // wgpu 24: request_adapter `Result` değil `Option` döndürür.
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .expect("WebGPU adapter alınamadı");

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("oa_native_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
        },
        None,
    ))
    .expect("WebGPU device alınamadı");

    let size = window.inner_size();
    let caps = surface.get_capabilities(&adapter);
    let surface_format = caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(caps.formats[0]);
    surface.configure(
        &device,
        &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
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
        None,       // LUT yok → kimlik LUT
        [640, 360], // kare boyutu
        [size.width, size.height],
    );
    let mut frame_count: u32 = 0;

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => elwt.exit(),
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    frame_count += 1;
                    let frame = make_test_frame(frame_count);
                    if let Ok(st) = surface.get_current_texture() {
                        let view = st
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        if let Err(e) =
                            player.render(&device, &queue, &view, &frame, frame_count, false)
                        {
                            eprintln!("[NativePlayer] render hatası: {e}");
                        }
                        st.present();
                    }
                    window.request_redraw();
                }
                _ => {}
            }
        })
        .unwrap();
}
