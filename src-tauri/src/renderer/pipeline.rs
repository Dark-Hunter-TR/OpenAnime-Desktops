use wgpu::{Device, ShaderModule, ComputePipeline, RenderPipeline, PipelineLayoutDescriptor, ComputePipelineDescriptor, RenderPipelineDescriptor, VertexState, FragmentState, ColorTargetState, BlendState, ColorWrites, PrimitiveState, PrimitiveTopology, FrontFace, Face, PolygonMode, MultisampleState, TextureFormat};

pub struct PipelineBuilder;

impl PipelineBuilder {
    /// Creates a compute pipeline from a shader module.
    pub fn build_compute_pipeline(
        device: &Device,
        module: &ShaderModule,
        bind_group_layouts: &[&wgpu::BindGroupLayout],
        label: Option<&str>,
    ) -> ComputePipeline {
        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label,
            bind_group_layouts,
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&ComputePipelineDescriptor {
            label,
            layout: Some(&layout),
            module,
            entry_point: "main",
            compilation_options: Default::default(),
            cache: None,
        })
    }

    /// Creates a render pipeline for fullscreen presentation.
    pub fn build_present_pipeline(
        device: &Device,
        module: &ShaderModule,
        bind_group_layouts: &[&wgpu::BindGroupLayout],
        target_format: TextureFormat,
        label: Option<&str>,
    ) -> RenderPipeline {
        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label,
            bind_group_layouts,
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&RenderPipelineDescriptor {
            label,
            layout: Some(&layout),
            vertex: VertexState {
                module,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: target_format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }
}
