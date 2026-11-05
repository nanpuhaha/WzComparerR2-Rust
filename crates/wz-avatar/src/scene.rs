//! End-to-end Avatar Scene and GPU Batching Pipeline.
//!
//! Bridges 29-slot MapleStory bone hierarchy with wz-render GPU vertex layouts.

use std::sync::Arc;

use crate::{AvatarCanvas, AvatarSlot, Bone, ZMap};

/// Standard 2D Textured Vertex for MapleStory sprite rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AvatarVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// An assembled GPU draw batch ready for wgpu buffer upload.
#[derive(Debug, Clone, Default)]
pub struct AvatarBatch {
    pub vertices: Vec<AvatarVertex>,
    pub indices: Vec<u32>,
    pub layer_count: usize,
}

impl From<AvatarVertex> for wz_render::SpriteVertex {
    fn from(v: AvatarVertex) -> Self {
        wz_render::SpriteVertex {
            position: [v.position[0], v.position[1], v.position[2], 1.0],
            color: v.color,
            texcoord: v.uv,
        }
    }
}

impl AvatarBatch {
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    /// `wz-render::SpriteBatch` 로 직접 변환.
    pub fn into_sprite_batch(self, texture_key: wz_render::GpuKey) -> wz_render::SpriteBatch {
        let mut sprite_batch = wz_render::SpriteBatch::new();
        let index_count = self.indices.len() as u32;

        sprite_batch.vertices = self.vertices.into_iter().map(Into::into).collect();
        sprite_batch.indices = self.indices;

        if index_count > 0 {
            sprite_batch.commands.push(wz_render::DrawCommand {
                texture_key,
                blend_mode: wz_render::BlendMode::Alpha,
                index_start: 0,
                index_count,
            });
        }

        sprite_batch
    }
}

/// High-level Avatar Scene facade managing assembly, anchor resolving, and GPU batching.
///
/// Implements [`std::ops::Deref`] and [`std::ops::DerefMut`] targeting [`AvatarCanvas`]
/// so all canvas operations are directly accessible without middle-man forwarding.
pub struct AvatarScene {
    pub canvas: AvatarCanvas,
}

impl std::ops::Deref for AvatarScene {
    type Target = AvatarCanvas;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.canvas
    }
}

impl std::ops::DerefMut for AvatarScene {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.canvas
    }
}

impl AvatarScene {
    pub fn new(zmap: Arc<ZMap>) -> Self {
        Self {
            canvas: AvatarCanvas::new(zmap),
        }
    }

    /// Access the underlying `AvatarCanvas`.
    pub fn canvas(&self) -> &AvatarCanvas {
        &self.canvas
    }

    /// Mutably access the underlying `AvatarCanvas`.
    pub fn canvas_mut(&mut self) -> &mut AvatarCanvas {
        &mut self.canvas
    }

    /// Assign an item description to a slot.
    pub fn set_slot(&mut self, slot: AvatarSlot, item_desc: Option<String>) {
        self.canvas.set_slot(slot, item_desc);
    }

    /// Read slot configurations.
    pub fn slots(&self) -> &[Option<String>; AvatarSlot::TOTAL_SLOTS] {
        &self.canvas.slots
    }

    /// Assembles bones into sorted layers using the canvas ZMap.
    pub fn layers(&self, root_bone: &Bone) -> Vec<crate::RenderLayer> {
        self.canvas.generate_layers(root_bone)
    }

    /// Assembles bones into sorted layers, then builds textured GPU quad batches.
    ///
    /// Delegates quad vertex layout calculation to [`crate::RenderLayer::to_quad_vertices`]
    /// ensuring correct encapsulation and 2D painter's order.
    pub fn build_batch(&self, root_bone: &Bone) -> AvatarBatch {
        self.build_batch_with_uvs(root_bone, None)
    }

    /// Builds textured GPU quad batches with optional per-layer UV rectangles `[u0, v0, u1, v1]`.
    pub fn build_batch_with_uvs(
        &self,
        root_bone: &Bone,
        layer_uvs: Option<&[[f32; 4]]>,
    ) -> AvatarBatch {
        let layers = self.canvas.generate_layers(root_bone);
        let mut batch = AvatarBatch {
            layer_count: layers.len(),
            ..Default::default()
        };

        for (idx, layer) in layers.iter().enumerate() {
            let z = idx as f32 * 0.001;

            // Default to full unit quad (0..1) unless custom UVs are provided
            let uv_rect = layer_uvs
                .and_then(|uvs| uvs.get(idx).copied())
                .unwrap_or([0.0, 0.0, 1.0, 1.0]);

            let white = [1.0, 1.0, 1.0, 1.0];
            let base_idx = batch.vertices.len() as u32;

            // Delegate quad geometry to RenderLayer (resolves Feature Envy smell)
            let quad_verts = layer.to_quad_vertices(z, uv_rect, white);
            batch.vertices.extend_from_slice(&quad_verts);

            // 2 Triangles per quad: (0, 1, 2) and (0, 2, 3)
            batch.indices.extend_from_slice(&[
                base_idx,
                base_idx + 1,
                base_idx + 2,
                base_idx,
                base_idx + 2,
                base_idx + 3,
            ]);
        }

        batch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AvatarSlot, Point, SkinLayer};

    #[test]
    fn test_avatar_scene_batching() {
        let zmap = Arc::new(ZMap::new(vec!["body".into(), "head".into()]));
        let mut scene = AvatarScene::new(zmap);
        scene.set_slot(AvatarSlot::Body, Some("00002000.img".into()));

        let mut root = Bone::new("@root");
        let mut body_skin = SkinLayer::new("body");
        body_skin.z = "body".into();
        body_skin.width = 30;
        body_skin.height = 30;
        root.append_bone(body_skin, "navel", Point::new(0, 0));

        let batch = scene.build_batch(&root);
        assert_eq!(batch.layer_count, 1);
        assert_eq!(batch.vertex_count(), 4);
        assert_eq!(batch.index_count(), 6);
        assert_eq!(batch.vertices[0].uv, [0.0, 0.0]);
        assert_eq!(batch.vertices[2].uv, [1.0, 1.0]);

        // Test custom UVs
        let custom_uvs = vec![[0.1, 0.2, 0.3, 0.4]];
        let batch_custom = scene.build_batch_with_uvs(&root, Some(&custom_uvs));
        assert_eq!(batch_custom.vertices[0].uv, [0.1, 0.2]);
        assert_eq!(batch_custom.vertices[2].uv, [0.3, 0.4]);

        // Test conversion to wz_render::SpriteBatch
        let sprite_batch = batch.into_sprite_batch(wz_render::GpuKey(7));
        assert_eq!(sprite_batch.vertices.len(), 4);
        assert_eq!(sprite_batch.indices.len(), 6);
        assert_eq!(sprite_batch.commands.len(), 1);
        assert_eq!(sprite_batch.commands[0].texture_key, wz_render::GpuKey(7));
        assert_eq!(sprite_batch.commands[0].index_start, 0);
        assert_eq!(sprite_batch.commands[0].index_count, 6);
        assert_eq!(sprite_batch.vertices[0].position[3], 1.0);
    }
}
