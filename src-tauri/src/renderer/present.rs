use std::sync::Arc;
use wgpu::{Device, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, ShaderStages, BindingType, TextureSampleType, TextureViewDimension, SamplerBindingType, RenderPipeline, TextureFormat, CommandEncoder, TextureView, Sampler, BindGroupDescriptor, BindGroupEntry, BindingResource, RenderPassDescriptor, RenderPassColorAttachment, Operations, LoadOp, StoreOp, Color};
use super::cache::ResourceCache;
use super::shader::ShaderSystem;
use super::pipeline::PipelineBuilder;

pub struct Presenter {
    device: Arc<Device>,
    pub layout: BindGroupLayout,
}

impl Presenter {
    pub fn new(device: Arc<Device>) -> Self {
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Presenter Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        Self {
            device,
            layout,
        }
    }

    /// Renders the source texture onto the target swapchain surface view.
    pub fn draw(
        &self,
        encoder: &mut CommandEncoder,
        cache: &mut ResourceCache,
        src_view: &TextureView,
        sampler: &Sampler,
        target_view: &TextureView,
        target_format: TextureFormat,
    ) {
        // Retrieve or compile render pipeline for presentation
        let pipeline_name = format!("present_{:?}", target_format);
        let pipeline = cache.get_render_pipeline(&pipeline_name, || {
            let shader = ShaderSystem::compile_present(&self.device);
            PipelineBuilder::build_present_pipeline(
                &self.device,
                &shader,
                &[&self.layout],
                target_format,
                Some("Fullscreen Present Pipeline"),
            )
        });

        // Unique key for the bind group
        let bind_group_key = format!("present_bg_{:p}_{:p}", src_view, sampler);
        let bind_group = cache.get_bind_group(&bind_group_key, || {
            self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Fullscreen Present Bind Group"),
                layout: &self.layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(src_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(sampler),
                    },
                ],
            })
        });

        // Run the render pass
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Fullscreen Presentation Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..6, 0..1); // Fullscreen quad (6 vertices)
    }
}
