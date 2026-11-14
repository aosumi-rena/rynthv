use eframe::{egui::{TextureId, epaint::{ClippedPrimitive, ImageDelta}}, egui_wgpu::{self, RenderState}};
use wgpu::{self as gpu, TextureFormat};

pub(super) struct CanvasTexture {
    host: RenderState,
    painter: egui_wgpu::Renderer,
    output: Option<Target>,
}

struct Target {
    id: TextureId,
    view: gpu::TextureView,
    extent: [u32; 2],
}

impl CanvasTexture {
    pub fn new(host: &RenderState) -> Self {
        Self {
            host: host.clone(),
            painter: egui_wgpu::Renderer::new(&host.device, TextureFormat::Rgba8Unorm,
                egui_wgpu::RendererOptions { dithering: false, ..Default::default() }),
            output: None,
        }
    }

    pub fn upload(&mut self, id: TextureId, delta: &ImageDelta) {
        self.painter.update_texture(&self.host.device, &self.host.queue, id, delta);
    }

    pub fn free(&mut self, id: TextureId) { self.painter.free_texture(&id); }

    pub fn id(&self) -> Option<TextureId> { self.output.as_ref().map(|target| target.id) }

    pub fn paint(&mut self, size: [f32; 2], scale: f32, primitives: &[ClippedPrimitive]) {
        let max = self.host.device.limits().max_texture_dimension_2d.min(4096) as f32;
        let scale = scale.min(max / size[0]).min(max / size[1]);
        let extent = size.map(|value| (value * scale).ceil().clamp(1.0, max) as u32);
        if self.output.as_ref().is_none_or(|old| old.extent != extent) {
            let texture = self.host.device.create_texture(&gpu::TextureDescriptor {
                label: Some("Rocoflex canvas"),
                size: gpu::Extent3d { width: extent[0], height: extent[1], depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: gpu::TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: gpu::TextureUsages::RENDER_ATTACHMENT | gpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&gpu::TextureViewDescriptor::default());
            let mut renderer = self.host.renderer.write();
            let id = if let Some(old) = &self.output {
                renderer.update_egui_texture_from_wgpu_texture(&self.host.device, &view, gpu::FilterMode::Linear, old.id);
                old.id
            } else {
                renderer.register_native_texture(&self.host.device, &view, gpu::FilterMode::Linear)
            };
            self.output = Some(Target { id, view, extent });
        }
        let target = self.output.as_ref().unwrap();
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: extent, pixels_per_point: scale };
        let mut encoder = self.host.device.create_command_encoder(&gpu::CommandEncoderDescriptor {
            label: Some("Rocoflex canvas"),
        });
        let callbacks = self.painter.update_buffers(&self.host.device, &self.host.queue, &mut encoder, primitives, &screen);
        {
            let pass = encoder.begin_render_pass(&gpu::RenderPassDescriptor {
                label: Some("Rocoflex canvas"),
                color_attachments: &[Some(gpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: gpu::Operations { load: gpu::LoadOp::Clear(gpu::Color::TRANSPARENT), store: gpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            self.painter.render(&mut pass.forget_lifetime(), primitives, &screen);
        }
        self.host.queue.submit(callbacks.into_iter().chain([encoder.finish()]));
    }
}

impl Drop for CanvasTexture {
    fn drop(&mut self) {
        if let Some(target) = self.output.take() { self.host.renderer.write().free_texture(&target.id); }
    }
}
