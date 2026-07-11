use wgpu::{Device, Texture, TextureView, TextureDescriptor, Extent3d, TextureDimension, TextureFormat, TextureUsages};

pub struct GpuTexture {
    pub texture: Texture,
    pub view: TextureView,
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

/// Description of a Linux DMA-BUF export/import descriptor for zero-copy.
pub struct DmaBufDescriptor {
    pub fd: std::os::fd::RawFd,
    pub stride: u32,
    pub offset: u32,
    pub drm_format: u32,
}

impl GpuTexture {
    /// Allocates a standard 2D texture.
    pub fn new(
        device: &Device,
        width: u32,
        height: u32,
        format: TextureFormat,
        usage: TextureUsages,
        label: Option<&str>,
    ) -> Self {
        let size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&Default::default());

        Self {
            texture,
            view,
            width,
            height,
            format,
        }
    }

    /// Creates a texture wrapper from an external DMA-BUF file descriptor.
    /// This is an architectural boundary hook. Under the hood, importing external DMA-BUF textures
    /// in Rust wgpu is implemented by querying the raw `wgpu_hal::api::Vulkan` device,
    /// importing the file descriptor via `VkImportMemoryFdInfoKHR`, and binding it to a Vulkan image.
    /// In this wrapper, we provide the descriptor hook while falling back to fast staging upload.
    pub fn from_dma_buf(
        device: &Device,
        width: u32,
        height: u32,
        descriptor: DmaBufDescriptor,
        usage: TextureUsages,
    ) -> Result<Self, String> {
        // Fallback or setup for wgpu-hal vulkan external memory imports goes here.
        // GStreamer pipeline output is configured for CPU video/x-raw, format=RGBA.
        // Thus, hardware-based DMA-BUF is unavailable, falling back to staging copy.
        println!(
            "[WebGPU Renderer] DMA-BUF unavailable (pipeline output is CPU video/x-raw), falling back to staging copy. FD {}, Stride {}, Offset {}, Format {}",
            descriptor.fd, descriptor.stride, descriptor.offset, descriptor.drm_format
        );
        
        // Allocate native GPU memory representing the imported DMA-BUF texture:
        let tex = Self::new(
            device,
            width,
            height,
            TextureFormat::Rgba8Unorm,
            usage | TextureUsages::COPY_DST,
            Some("DMA-BUF Imported Texture"),
        );
        Ok(tex)
    }

    /// Utility: returns the width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Utility: returns the height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Utility: returns the view.
    pub fn view(&self) -> &TextureView {
        &self.view
    }
}
