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
}

/// High-level Avatar Scene managing assembly, anchor resolving, and GPU batching.
pub struct AvatarScene {
    pub canvas: AvatarCanvas,
}

impl AvatarScene {
    pub fn new(zmap: Arc<ZMap>) -> Self {
        Self {
            canvas: AvatarCanvas::new(zmap),
        }
    }

    pub fn set_slot(&mut self, slot: AvatarSlot, item_desc: Option<String>) {
        self.canvas.set_slot(slot, item_desc);
    }

    /// Assembles bones into sorted layers, then builds textured GPU quad batches.
    pub fn build_batch(&self, root_bone: &Bone, atlas_size: (u32, u32)) -> AvatarBatch {
        let layers = self.canvas.generate_layers(root_bone);
        let mut batch = AvatarBatch {
            layer_count: layers.len(),
            ..Default::default()
        };

        let (atlas_w, atlas_h) = atlas_size;

        for (idx, layer) in layers.iter().enumerate() {
            let z = idx as f32 * 0.001;
            let x0 = layer.position.x as f32;
            let y0 = layer.position.y as f32;
            let x1 = x0 + layer.width as f32;
            let y1 = y0 + layer.height as f32;

            // Compute UV coordinates (normalized or 0..1 fallback)
            let (u0, v0, u1, v1) = if atlas_w > 0 && atlas_h > 0 {
                (
                    layer.origin.x.max(0) as f32 / atlas_w as f32,
                    layer.origin.y.max(0) as f32 / atlas_h as f32,
                    (layer.origin.x.max(0) as u32 + layer.width) as f32 / atlas_w as f32,
                    (layer.origin.y.max(0) as u32 + layer.height) as f32 / atlas_h as f32,
                )
            } else {
                (0.0, 0.0, 1.0, 1.0)
            };

            let white = [1.0, 1.0, 1.0, 1.0];
            let base_idx = batch.vertices.len() as u32;

            // 4 Quad Vertices (TL, TR, BR, BL)
            batch.vertices.push(AvatarVertex {
                position: [x0, y0, z],
                uv: [u0, v0],
                color: white,
            });
            batch.vertices.push(AvatarVertex {
                position: [x1, y0, z],
                uv: [u1, v0],
                color: white,
            });
            batch.vertices.push(AvatarVertex {
                position: [x1, y1, z],
                uv: [u1, v1],
                color: white,
            });
            batch.vertices.push(AvatarVertex {
                position: [x0, y1, z],
                uv: [u0, v1],
                color: white,
            });

            // 2 Triangles (0, 1, 2) and (0, 2, 3)
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
    use crate::{Point, SkinLayer};

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

        let batch = scene.build_batch(&root, (1024, 1024));
        assert_eq!(batch.layer_count, 1);
        assert_eq!(batch.vertex_count(), 4);
        assert_eq!(batch.index_count(), 6);
    }
}
