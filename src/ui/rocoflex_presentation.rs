use super::rocoflex_gpu::CanvasTexture;
use crate::synthesis::rocoflex::wire;
use eframe::{
    egui::{self, *},
    egui_wgpu::RenderState,
};
use egui::epaint::{ClippedPrimitive, ImageDelta, Primitive, Vertex, textures::{TextureFilter, TextureWrapMode}};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Picture {
    size: Vec2,
    textures: BTreeMap<u64, [usize; 2]>,
    meshes: Vec<ClippedPrimitive>,
    canvas: Option<CanvasTexture>,
    free: Vec<u64>,
    pub cursor: usize,
    pub ime: Option<([f32; 4], [f32; 4], bool)>,
}

impl Picture {
    pub fn ready(&self) -> bool { self.canvas.as_ref().and_then(CanvasTexture::id).is_some() }

    pub fn update(&mut self, gpu: &RenderState, frame: &wire::Frame, mut bytes: &[u8], scale: f32) -> Result<(), String> {
        let invalid = || "Rocoflex returned an invalid canvas frame".to_owned();
        if !frame.size.into_iter().zip(wire::MIN_SIZE).all(|(v, lo)| (lo..=4096.0).contains(&v)) {
            return Err(invalid());
        }
        if frame.reuse_picture && (!frame.meshes.is_empty() || !frame.free.is_empty()
            || self.size != Vec2::from(frame.size))
        {
            return Err(invalid());
        }

        let canvas = self.canvas.get_or_insert_with(|| CanvasTexture::new(gpu));
        for id in self.free.drain(..) {
            self.textures.remove(&id);
            canvas.free(TextureId::Managed(id));
        }
        let limit = gpu.device.limits().max_texture_dimension_2d.min(8192) as usize;
        for texture in &frame.textures {
            let [width, height] = texture.size;
            if width == 0 || width > limit || height == 0 || height > limit { return Err(invalid()); }
            let (rgba, rest) = bytes.split_at_checked(width * height * 4).ok_or_else(invalid)?;
            bytes = rest;
            let options = sampling(texture.sampling).ok_or_else(invalid)?;
            let image = ColorImage::from_rgba_premultiplied(texture.size, rgba);
            let delta = if let Some(at) = texture.offset {
                let size = self.textures.get(&texture.id).ok_or_else(invalid)?;
                if !(0..2).all(|i| at[i].checked_add(texture.size[i]).is_some_and(|end| end <= size[i])) {
                    return Err(invalid());
                }
                ImageDelta::partial(at, image, options)
            } else {
                self.textures.insert(texture.id, texture.size);
                ImageDelta::full(image, options)
            };
            canvas.upload(TextureId::Managed(texture.id), &delta);
        }
        if !frame.reuse_picture {
            let mut meshes = Vec::with_capacity(frame.meshes.len());
            for part in &frame.meshes {
                let (mesh, remaining) = mesh(part, bytes, &self.textures).ok_or_else(invalid)?;
                meshes.push(mesh);
                bytes = remaining;
            }
            self.meshes = meshes;
        }
        if !bytes.is_empty() { return Err(invalid()); }
        if !frame.reuse_picture || !frame.textures.is_empty() {
            canvas.paint(frame.size, scale, &self.meshes);
        }
        self.size = frame.size.into();
        self.free.clone_from(&frame.free);
        self.cursor = frame.cursor;
        self.ime = frame.ime;
        Ok(())
    }

    pub fn draw(&self, ui: &Ui, rect: Rect) -> bool {
        let Some(texture) = self.canvas.as_ref().and_then(CanvasTexture::id) else { return false; };
        ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()))
            .image(texture, rect, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), Color32::WHITE);
        true
    }
}

fn mesh<'a>(part: &wire::Mesh, bytes: &'a [u8], textures: &BTreeMap<u64, [usize; 2]>)
    -> Option<(ClippedPrimitive, &'a [u8])>
{
    if !textures.contains_key(&part.id) || part.indices % 3 != 0 || !part.clip.into_iter().all(f32::is_finite) {
        return None;
    }
    let (vertices, rest) = bytes.split_at_checked(part.vertices.checked_mul(20)?)?;
    let (indices, rest) = rest.split_at_checked(part.indices.checked_mul(4)?)?;
    let mut mesh = Mesh::with_texture(TextureId::Managed(part.id));
    mesh.vertices.reserve(part.vertices);
    mesh.indices.reserve(part.indices);
    for v in vertices.chunks_exact(20) {
        let at = |index: usize| f32::from_le_bytes(v[index..index + 4].try_into().unwrap());
        let (x, y, u, w) = (at(0), at(4), at(8), at(12));
        if ![x, y, u, w].into_iter().all(f32::is_finite) { return None; }
        let color = Color32::from_rgba_premultiplied(v[16], v[17], v[18], v[19]);
        mesh.vertices.push(Vertex { pos: pos2(x, y), uv: pos2(u, w), color });
    }
    for word in indices.chunks_exact(4) {
        let index = u32::from_le_bytes(word.try_into().unwrap());
        if index as usize >= part.vertices { return None; }
        mesh.indices.push(index);
    }
    Some((ClippedPrimitive { clip_rect: rectangle(part.clip), primitive: Primitive::Mesh(mesh) }, rest))
}

pub(super) fn rectangle([l, t, r, b]: [f32; 4]) -> Rect { Rect::from_min_max(pos2(l, t), pos2(r, b)) }

fn sampling([mag, min, wrap, mip]: [u8; 4]) -> Option<TextureOptions> {
    let filter = |v| [TextureFilter::Nearest, TextureFilter::Linear].get(v as usize).copied();
    Some(TextureOptions { magnification: filter(mag)?, minification: filter(min)?,
        wrap_mode: *[TextureWrapMode::ClampToEdge, TextureWrapMode::Repeat, TextureWrapMode::MirroredRepeat].get(wrap as usize)?,
        mipmap_mode: if mip == 0 { None } else { Some(filter(mip - 1)?) } })
}
