use std::collections::HashMap;
use wgpu::{BindGroup, ComputePipeline, RenderPipeline, Device, TextureFormat, TextureUsages};
use super::texture::GpuTexture;

pub struct ResourceCache {
    pipelines_compute: HashMap<String, ComputePipeline>,
    pipelines_render: HashMap<String, RenderPipeline>,
    bind_groups: HashMap<String, BindGroup>,
    textures: HashMap<String, GpuTexture>,
}

impl ResourceCache {
    pub fn new() -> Self {
        Self {
            pipelines_compute: HashMap::new(),
            pipelines_render: HashMap::new(),
            bind_groups: HashMap::new(),
            textures: HashMap::new(),
        }
    }

    /// Gets or creates a cached compute pipeline.
    pub fn get_compute_pipeline<F>(&mut self, name: &str, create_fn: F) -> &ComputePipeline
    where
        F: FnOnce() -> ComputePipeline,
    {
        self.pipelines_compute
            .entry(name.to_string())
            .or_insert_with(create_fn)
    }

    /// Gets or creates a cached render pipeline.
    pub fn get_render_pipeline<F>(&mut self, name: &str, create_fn: F) -> &RenderPipeline
    where
        F: FnOnce() -> RenderPipeline,
    {
        self.pipelines_render
            .entry(name.to_string())
            .or_insert_with(create_fn)
    }

    /// Gets or creates a cached bind group.
    pub fn get_bind_group<F>(&mut self, key: &str, create_fn: F) -> &BindGroup
    where
        F: FnOnce() -> BindGroup,
    {
        self.bind_groups
            .entry(key.to_string())
            .or_insert_with(create_fn)
    }

    /// Clears cached bind groups (useful when textures are recreated).
    pub fn clear_bind_groups(&mut self) {
        self.bind_groups.clear();
    }

    /// Gets or allocates a cached intermediate texture. Reallocates only if dimensions change.
    pub fn get_texture(
        &mut self,
        device: &Device,
        name: &str,
        width: u32,
        height: u32,
        format: TextureFormat,
        usage: TextureUsages,
    ) -> &GpuTexture {
        let needs_recreation = if let Some(tex) = self.textures.get(name) {
            if tex.width != width || tex.height != height || tex.format != format {
                println!(
                    "[Resource Cache] Recreating texture '{}': Old size {}x{} (format {:?}) -> New size {}x{} (format {:?})",
                    name, tex.width, tex.height, tex.format, width, height, format
                );
                true
            } else {
                false
            }
        } else {
            false
        };

        if needs_recreation {
            let new_tex = GpuTexture::new(device, width, height, format, usage, Some(name));
            self.textures.insert(name.to_string(), new_tex);
            // Clear bind groups associated with the old texture
            self.bind_groups.clear();
        }

        self.textures.entry(name.to_string()).or_insert_with(|| {
            GpuTexture::new(device, width, height, format, usage, Some(name))
        })
    }

    /// Clears all cached resources.
    pub fn clear(&mut self) {
        self.pipelines_compute.clear();
        self.pipelines_render.clear();
        self.bind_groups.clear();
        self.textures.clear();
    }
}
