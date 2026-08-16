// ═══════════════════════════════════════════════════════════
// Native WebGPU Oynatıcı (yalnızca Linux)
// ═══════════════════════════════════════════════════════════
//
// Linux'ta WebKitGTK WebGPU sunmadığı için openani.me'nin kendi oynatıcısı
// render edemiyor. Bu modül, sitenin TEMEL render pipeline'ını (LUT renk
// derecelendirme + FRC dithering + scale) native `wgpu` ile kopyalar.
//
// Bu BİRİNCİ aşamadır ("B-basic"): yalnızca 5 temel shader (#0–#4). Sitenin
// OpenFrameGeneration (OFG) frame-interpolasyon motoru (conv2d CNN) buraya
// DAHİL DEĞİLDİR — ayrı ve çok daha büyük bir iştir (bkz. webgpu-inspector
// yakalama notları).
//
// Dosyalar:
//   shaders.rs      — yakalanan WGSL kaynakları (birebir)
//   pipeline.rs     — doku/bind group/render pass kurulumu (B'nin çekirdeği)
//   linux_surface.rs— GTK penceresi + wgpu surface + olay döngüsü (ince iskelet)

pub mod pipeline;
pub mod shaders;

#[cfg(target_os = "linux")]
pub mod linux_surface;

/// `linux_surface::run` içinden çağrılır. Asıl giriş noktası.
#[cfg(target_os = "linux")]
pub fn run() {
    linux_surface::run();
}
