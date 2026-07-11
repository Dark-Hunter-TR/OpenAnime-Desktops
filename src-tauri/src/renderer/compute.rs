use std::sync::Arc;
use wgpu::{Device, Queue, CommandEncoder, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, BindingType, TextureSampleType, TextureViewDimension, SamplerBindingType, StorageTextureAccess, TextureFormat, BindGroupDescriptor, BindGroupEntry, BindingResource, ComputePassDescriptor};
use super::texture::GpuTexture;
use super::cache::ResourceCache;
use super::shader::ShaderSystem;
use super::pipeline::PipelineBuilder;

pub struct VideoComputePipeline {
    device: Arc<Device>,
    
    // Bind group layouts
    pub layout_single_io: BindGroupLayout, // 1 input texture, 1 sampler, 1 storage output
    pub layout_motion: BindGroupLayout,    // current frame, previous frame, storage motion vector output
    pub layout_frame_gen: BindGroupLayout, // current, previous, motion vector texture, sampler, storage output
}

impl VideoComputePipeline {
    pub fn new(device: Arc<Device>) -> Self {
        // Create BindGroupLayout for Denoise, Upscale, Edge, Sharpen
        let layout_single_io = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Compute Layout Single IO"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        // Create BindGroupLayout for Motion Vectors
        let layout_motion = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Compute Layout Motion Vector"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        // Use rg16float format for storing vectors
                        format: TextureFormat::Rg16Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        // Create BindGroupLayout for Frame Generation
        let layout_frame_gen = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Compute Layout Frame Generation"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        Self {
            device,
            layout_single_io,
            layout_motion,
            layout_frame_gen,
        }
    }

    /// Runs a single compute pass on the command encoder.
    pub fn dispatch_stage(
        &self,
        encoder: &mut CommandEncoder,
        cache: &mut ResourceCache,
        stage_name: &str,
        pipeline_builder: impl FnOnce(&Device) -> wgpu::ComputePipeline,
        input_texture: &GpuTexture,
        sampler: &wgpu::Sampler,
        output_texture: &GpuTexture,
    ) {
        let pipeline = cache.get_compute_pipeline(stage_name, || pipeline_builder(&self.device));

        let bind_group_key = format!(
            "{}_bg_{:p}_{:p}_{:p}",
            stage_name,
            input_texture.view(),
            sampler,
            output_texture.view()
        );

        let bind_group = cache.get_bind_group(&bind_group_key, || {
            self.device.create_bind_group(&BindGroupDescriptor {
                label: Some(&format!("{} Bind Group", stage_name)),
                layout: &self.layout_single_io,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(input_texture.view()),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(sampler),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::TextureView(output_texture.view()),
                    },
                ],
            })
        });

        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some(stage_name),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(pipeline);
        compute_pass.set_bind_group(0, bind_group, &[]);
        
        let workgroups_x = (output_texture.width() + 15) / 16;
        let workgroups_y = (output_texture.height() + 15) / 16;
        compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
    }

    /// Dispatches the Motion Vector block matching compute shader.
    pub fn dispatch_motion_vector(
        &self,
        encoder: &mut CommandEncoder,
        cache: &mut ResourceCache,
        current_tex: &GpuTexture,
        previous_tex: &GpuTexture,
        output_mv: &GpuTexture,
    ) {
        let pipeline = cache.get_compute_pipeline("motion_vector", || {
            let shader = ShaderSystem::compile_motion_vector(&self.device);
            PipelineBuilder::build_compute_pipeline(&self.device, &shader, &[&self.layout_motion], Some("Motion Vector Pipeline"))
        });

        let bind_group_key = format!(
            "motion_vector_bg_{:p}_{:p}_{:p}",
            current_tex.view(),
            previous_tex.view(),
            output_mv.view()
        );

        let bind_group = cache.get_bind_group(&bind_group_key, || {
            self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Motion Vector Bind Group"),
                layout: &self.layout_motion,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(current_tex.view()),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(previous_tex.view()),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::TextureView(output_mv.view()),
                    },
                ],
            })
        });

        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Motion Vector Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(pipeline);
        compute_pass.set_bind_group(0, bind_group, &[]);
        
        let workgroups_x = (output_mv.width() + 7) / 8;
        let workgroups_y = (output_mv.height() + 7) / 8;
        compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
    }

    /// Dispatches the Frame Interpolation / Generation compute shader.
    pub fn dispatch_frame_gen(
        &self,
        encoder: &mut CommandEncoder,
        cache: &mut ResourceCache,
        current_tex: &GpuTexture,
        previous_tex: &GpuTexture,
        mv_tex: &GpuTexture,
        sampler: &wgpu::Sampler,
        output_frame: &GpuTexture,
    ) {
        let pipeline = cache.get_compute_pipeline("frame_gen", || {
            let shader = ShaderSystem::compile_frame_gen(&self.device);
            PipelineBuilder::build_compute_pipeline(&self.device, &shader, &[&self.layout_frame_gen], Some("Frame Gen Pipeline"))
        });

        let bind_group_key = format!(
            "frame_gen_bg_{:p}_{:p}_{:p}_{:p}_{:p}",
            current_tex.view(),
            previous_tex.view(),
            mv_tex.view(),
            sampler,
            output_frame.view()
        );

        let bind_group = cache.get_bind_group(&bind_group_key, || {
            self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Frame Gen Bind Group"),
                layout: &self.layout_frame_gen,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(current_tex.view()),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(previous_tex.view()),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::TextureView(mv_tex.view()),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::Sampler(sampler),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: BindingResource::TextureView(output_frame.view()),
                    },
                ],
            })
        });

        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Frame Gen Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(pipeline);
        compute_pass.set_bind_group(0, bind_group, &[]);
        
        let workgroups_x = (output_frame.width() + 15) / 16;
        let workgroups_y = (output_frame.height() + 15) / 16;
        compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
    }
}
