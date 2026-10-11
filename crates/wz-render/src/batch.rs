//! 2D 스프라이트 정점 버퍼 구조 및 배치 수집기.
//!
//! # 정점 포맷 (WGSL `VsIn` 과 100% 일치)
//!
//! - `position`: `[f32; 4]` (16 바이트, offset 0)
//! - `color`:    `[f32; 4]` (16 바이트, offset 16)
//! - `texcoord`: `[f32; 2]` ( 8 바이트, offset 32)
//! - 총 `array_stride = 40` 바이트.

use bytemuck::{Pod, Zeroable};
use crate::{AtlasRect, GpuKey};

/// WGSL `VsIn` 과 바이너리 호환되는 스프라이트 정점.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SpriteVertex {
    pub position: [f32; 4],
    pub color: [f32; 4],
    pub texcoord: [f32; 2],
}

impl SpriteVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: 40,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 32,
                shader_location: 2,
            },
        ],
    };

    pub fn new(pos_xy: [f32; 2], z: f32, uv: [f32; 2], color: [f32; 4]) -> Self {
        Self {
            position: [pos_xy[0], pos_xy[1], z, 1.0],
            color,
            texcoord: uv,
        }
    }
}

/// 텍스처/재질 변경 시 끊어지는 단위 배치 명령.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawCommand {
    pub texture_key: GpuKey,
    pub index_start: u32,
    pub index_count: u32,
}

/// 2D 스프라이트 정점/인덱스 수집 및 배치 분할기.
#[derive(Debug, Default)]
pub struct SpriteBatch {
    pub vertices: Vec<SpriteVertex>,
    pub indices: Vec<u32>,
    pub commands: Vec<DrawCommand>,
    current_key: Option<GpuKey>,
    current_start: u32,
}

impl SpriteBatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// 배치 초기화.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();
        self.current_key = None;
        self.current_start = 0;
    }

    /// 단일 쿼드(Quad, 4 정점 + 6 인덱스) 스프라이트 추가.
    ///
    /// - `pos`: [x, y, width, height]
    /// - `z`: 깊이값
    /// - `uv`: 정규화된 텍스처 UV 사각형 [u0, v0, u1, v1]
    /// - `color`: RGBA 틴트 [r, g, b, a]
    pub fn push_quad(
        &mut self,
        texture_key: GpuKey,
        pos: [f32; 4],
        z: f32,
        uv: [f32; 4],
        color: [f32; 4],
    ) {
        // 텍스처가 바뀌면 Batch Break
        if self.current_key != Some(texture_key) {
            self.flush_current_command();
            self.current_key = Some(texture_key);
            self.current_start = self.indices.len() as u32;
        }

        let base_idx = self.vertices.len() as u32;
        let x0 = pos[0];
        let y0 = pos[1];
        let x1 = pos[0] + pos[2];
        let y1 = pos[1] + pos[3];

        let u0 = uv[0];
        let v0 = uv[1];
        let u1 = uv[2];
        let v1 = uv[3];

        // 4개 정점 (TL, TR, BR, BL)
        self.vertices.push(SpriteVertex::new([x0, y0], z, [u0, v0], color));
        self.vertices.push(SpriteVertex::new([x1, y0], z, [u1, v0], color));
        self.vertices.push(SpriteVertex::new([x1, y1], z, [u1, v1], color));
        self.vertices.push(SpriteVertex::new([x0, y1], z, [u0, v1], color));

        // 6개 인덱스 (0, 1, 2) + (0, 2, 3)
        self.indices.extend_from_slice(&[
            base_idx,
            base_idx + 1,
            base_idx + 2,
            base_idx,
            base_idx + 2,
            base_idx + 3,
        ]);
    }

    /// 아틀라스 사각형(`AtlasRect`)을 이용한 쿼드 추가 헬퍼.
    pub fn push_atlas_quad(
        &mut self,
        texture_key: GpuKey,
        pos: [f32; 4],
        z: f32,
        rect: &AtlasRect,
        atlas_size: (u32, u32),
        color: [f32; 4],
    ) {
        self.push_quad(texture_key, pos, z, rect.uv(atlas_size.0, atlas_size.1), color);
    }

    /// 배치 수집 완료 및 최종 커맨드 마감.
    pub fn finish(&mut self) {
        self.flush_current_command();
    }

    fn flush_current_command(&mut self) {
        if let Some(key) = self.current_key {
            let current_count = self.indices.len() as u32 - self.current_start;
            if current_count > 0 {
                self.commands.push(DrawCommand {
                    texture_key: key,
                    index_start: self.current_start,
                    index_count: current_count,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprite_vertex_memory_layout() {
        assert_eq!(std::mem::size_of::<SpriteVertex>(), 40);
        assert_eq!(std::mem::align_of::<SpriteVertex>(), 4);
    }

    #[test]
    fn test_batch_breaking_on_texture_change() {
        let mut batch = SpriteBatch::new();

        // 텍스처 0 에 쿼드 2개
        batch.push_quad(GpuKey(0), [0.0, 0.0, 10.0, 10.0], 0.0, [0.0, 0.0, 1.0, 1.0], [1.0; 4]);
        batch.push_quad(GpuKey(0), [10.0, 0.0, 10.0, 10.0], 0.0, [0.0, 0.0, 1.0, 1.0], [1.0; 4]);

        // 텍스처 1 로 변경 -> 쿼드 1개
        batch.push_quad(GpuKey(1), [20.0, 0.0, 10.0, 10.0], 0.0, [0.0, 0.0, 1.0, 1.0], [1.0; 4]);

        batch.finish();

        assert_eq!(batch.vertices.len(), 12);
        assert_eq!(batch.indices.len(), 18);
        assert_eq!(batch.commands.len(), 2);

        assert_eq!(batch.commands[0].texture_key, GpuKey(0));
        assert_eq!(batch.commands[0].index_start, 0);
        assert_eq!(batch.commands[0].index_count, 12);

        assert_eq!(batch.commands[1].texture_key, GpuKey(1));
        assert_eq!(batch.commands[1].index_start, 12);
        assert_eq!(batch.commands[1].index_count, 6);
    }
}
