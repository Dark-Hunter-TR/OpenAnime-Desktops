use wgpu::{Adapter, CompositeAlphaMode, Device, Surface, SurfaceConfiguration, SurfaceError, SurfaceTexture, PresentMode, TextureUsages};

pub struct SurfaceManager {
    surface: Surface<'static>,
    config: SurfaceConfiguration,
    vsync: bool,
}

impl SurfaceManager {
    /// Önceden yaratılmış surface ile kurulum — adapter seçimi surface'a göre
    /// yapılabilsin diye surface artık çağıran tarafta (renderer) açılır.
    pub fn new(
        surface: Surface<'static>,
        adapter: &Adapter,
        device: &Device,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<Self, String> {
        let width = width.max(1);
        let height = height.max(1);

        // Uyumsuz surface koruması: configure fatal panic'e düşmesin.
        if !adapter.is_surface_supported(&surface) {
            return Err(format!(
                "adapter '{}' bu pencereye sunum yapamıyor (hibrit PRIME uyumsuzluğu)",
                adapter.get_info().name
            ));
        }

        let caps = surface.get_capabilities(adapter);
        if caps.formats.is_empty() {
            return Err("surface hiçbir format bildirmiyor".to_string());
        }
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
    /// Always `Auto`: pre-queried `caps.alpha_modes` is not reliable on all
    /// Vulkan/Wayland drivers — on NVIDIA, `Surface::get_capabilities()` has
    /// been observed to report `PostMultiplied` while the very next
    /// `Surface::configure()` call rejects it ("... not in the list of
    /// supported alpha modes: [Opaque]"), which panics and aborts the whole
    /// process (`panic = "abort"`). `Auto` is resolved by wgpu *inside*
    /// `configure()` against the capabilities it validates with, so it can
    /// never fail that validation.
    fn select_alpha_mode(_available: &[CompositeAlphaMode]) -> CompositeAlphaMode {
        CompositeAlphaMode::Auto
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
    /// Re-queries surface capabilities and re-selects `present_mode`/`alpha_mode`
    /// from the fresh result before configuring — on Wayland the supported set
    /// can change after the compositor maps/remaps the surface.
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
