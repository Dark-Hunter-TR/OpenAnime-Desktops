use wgpu::{Device, ShaderModule, ShaderModuleDescriptor, ShaderSource};

/// Shader system that compiles WGSL shaders stored in separate files.
pub struct ShaderSystem;

impl ShaderSystem {
    pub fn compile_bicubic(device: &Device) -> ShaderModule {
        let source = include_str!("shaders/bicubic.wgsl");
        device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Bicubic Upscale Shader"),
            source: ShaderSource::Wgsl(source.into()),
        })
    }

    pub fn compile_lanczos(device: &Device) -> ShaderModule {
        let source = include_str!("shaders/lanczos.wgsl");
        device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Lanczos Upscale Shader"),
            source: ShaderSource::Wgsl(source.into()),
        })
    }

    pub fn compile_edge_enhancement(device: &Device) -> ShaderModule {
        let source = include_str!("shaders/edge_enhancement.wgsl");
        device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Edge Enhancement Shader"),
            source: ShaderSource::Wgsl(source.into()),
        })
    }

    pub fn compile_sharpen(device: &Device) -> ShaderModule {
        let source = include_str!("shaders/sharpen.wgsl");
        device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Sharpen Shader"),
            source: ShaderSource::Wgsl(source.into()),
        })
    }

    pub fn compile_denoise(device: &Device) -> ShaderModule {
        let source = include_str!("shaders/denoise.wgsl");
        device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Denoise Shader"),
            source: ShaderSource::Wgsl(source.into()),
        })
    }

    pub fn compile_anime_upscale(device: &Device) -> ShaderModule {
        let source = include_str!("shaders/anime_upscale.wgsl");
        device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Anime-style Upscale Shader"),
            source: ShaderSource::Wgsl(source.into()),
        })
    }

    pub fn compile_motion_vector(device: &Device) -> ShaderModule {
        let source = include_str!("shaders/motion_vector.wgsl");
        device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Motion Vector Estimator Shader"),
            source: ShaderSource::Wgsl(source.into()),
        })
    }

    pub fn compile_frame_gen(device: &Device) -> ShaderModule {
        let source = include_str!("shaders/frame_gen.wgsl");
        device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Frame Generator Shader"),
            source: ShaderSource::Wgsl(source.into()),
        })
    }

    pub fn compile_present(device: &Device) -> ShaderModule {
        let source = include_str!("shaders/present.wgsl");
        device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Present Render Shader"),
            source: ShaderSource::Wgsl(source.into()),
        })
    }
}
