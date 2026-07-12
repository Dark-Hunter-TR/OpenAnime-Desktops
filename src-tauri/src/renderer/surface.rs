use wgpu::{Adapter, CompositeAlphaMode, Device, Instance, Surface, SurfaceConfiguration, SurfaceError, SurfaceTexture, PresentMode, TextureUsages};
use tauri::WebviewWindow;

pub struct SurfaceManager {
    surface: Surface<'static>,
    config: SurfaceConfiguration,
    vsync: bool,
}

impl SurfaceManager {
    /// Creates a new surface for the given WebviewWindow.
    pub fn new(
        instance: &Instance,
        adapter: &Adapter,
        device: &Device,
        window: &WebviewWindow,
        vsync: bool,
    ) -> Result<Self, String> {
        let size = window.inner_size().map_err(|e| e.to_string())?;
        let width = size.width.max(1);
        let height = size.height.max(1);

        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;

        let caps = surface.get_capabilities(adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let present_mode = Self::select_present_mode(&caps.present_modes, vsync);

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_DST,
            format,
            width,
            height,
            present_mode,
            alpha_mode: Self::select_alpha_mode(&caps.alpha_modes),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(device, &config);

        Ok(Self {
            surface,
            config,
            vsync,
        })
    }

    /// Selects the composite alpha mode.
    ///
    /// `caps.alpha_modes[0]` is not reliable on all Vulkan/Wayland drivers:
    /// on NVIDIA, `Surface::get_capabilities()` has been observed to report
    /// `PostMultiplied` as available while the very next `Surface::configure()`
    /// call rejects it ("... not in the list of supported alpha modes: [Opaque]"),
    /// which panics and aborts the whole process (`panic = "abort"`). `Opaque`
    /// is the one mode every driver honors, so we prefer it whenever it's
    /// present — the transparent overlay window then blends opaquely instead
    /// of alpha-blending, which is a visual trade-off, not a crash.
    fn select_alpha_mode(available: &[CompositeAlphaMode]) -> CompositeAlphaMode {
        if available.contains(&CompositeAlphaMode::Opaque) {
            CompositeAlphaMode::Opaque
        } else {
            available[0]
        }
    }

    /// Selects present mode based on VSync request and hardware capabilities.
    fn select_present_mode(available: &[PresentMode], vsync: bool) -> PresentMode {
        if vsync {
            // Mailbox is preferred for low latency vsync.
            if available.contains(&PresentMode::Mailbox) {
                PresentMode::Mailbox
            } else {
                PresentMode::Fifo
            }
        } else {
            // Immediate or FifoRelaxed for disabled vsync.
            if available.contains(&PresentMode::Immediate) {
                PresentMode::Immediate
            } else if available.contains(&PresentMode::FifoRelaxed) {
                PresentMode::FifoRelaxed
            } else {
                PresentMode::Fifo
            }
        }
    }

    /// Configures/recreates surface for a new size.
    ///
    /// Re-queries surface capabilities and re-selects `alpha_mode`/`present_mode`
    /// from the fresh result before configuring. On Wayland (notably NVIDIA),
    /// the capabilities reported by an early `get_capabilities()` call (e.g. in
    /// `new()`, before the compositor has fully mapped the surface) can be wider
    /// than what a later `configure()` actually validates against — reusing a
    /// stale `alpha_mode` here caused a hard panic ("Requested alpha mode
    /// PostMultiplied is not in the list of supported alpha modes: [Opaque]")
    /// that aborted the whole process (`panic = "abort"`).
    pub fn resize(&mut self, device: &Device, adapter: &Adapter, width: u32, height: u32) {
        if width > 0 && height > 0 {
            let caps = self.surface.get_capabilities(adapter);
            self.config.present_mode = Self::select_present_mode(&caps.present_modes, self.vsync);
            self.config.alpha_mode = Self::select_alpha_mode(&caps.alpha_modes);
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(device, &self.config);
            println!("[WebGPU Renderer] Surface resized to {}x{}", width, height);
        }
    }

    /// Re-evaluates capabilities and reconfigures the surface.
    pub fn reconfigure(&mut self, device: &Device, adapter: &Adapter) {
        let caps = self.surface.get_capabilities(adapter);
        self.config.present_mode = Self::select_present_mode(&caps.present_modes, self.vsync);
        self.config.alpha_mode = Self::select_alpha_mode(&caps.alpha_modes);
        self.surface.configure(device, &self.config);
    }

    /// Toggles VSync on the fly.
    pub fn set_vsync(&mut self, device: &Device, adapter: &Adapter, vsync: bool) {
        self.vsync = vsync;
        self.reconfigure(device, adapter);
    }

    /// Gets the current frame from the swapchain.
    pub fn get_current_texture(&self) -> Result<SurfaceTexture, SurfaceError> {
        self.surface.get_current_texture()
    }

    /// Returns the surface format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Returns the surface width.
    pub fn width(&self) -> u32 {
        self.config.width
    }

    /// Returns the surface height.
    pub fn height(&self) -> u32 {
        self.config.height
    }
}
