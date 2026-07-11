use wgpu::{Queue, ImageCopyTexture, Origin3d, TextureAspect, ImageDataLayout, Extent3d};
use super::texture::GpuTexture;

pub struct TextureUploader;

impl TextureUploader {
    /// Uploads CPU-decoded RGBA frame data to a GPU Texture.
    /// Uses wgpu's internal optimized staging pool to copy the data directly to the texture
    /// with zero CPU allocations or copying in user space.
    pub fn upload_rgba(
        queue: &Queue,
        texture: &GpuTexture,
        rgba_data: &[u8],
    ) {
        queue.write_texture(
            ImageCopyTexture {
                texture: &texture.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            rgba_data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(texture.width() * 4),
                rows_per_image: Some(texture.height()),
            },
            Extent3d {
                width: texture.width(),
                height: texture.height(),
                depth_or_array_layers: 1,
            },
        );
    }
}
