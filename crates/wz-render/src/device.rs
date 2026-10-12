//! wgpu 디바이스 초기화 및 2D 렌더러 파이프라인.
//!
//! # ADR-4 계층 규칙
//!
//! 외부 계층에 wgpu 네이티브 타입(`Device`, `Queue`, `TextureView`)을 노출하지 않고,
//! `RenderContext`, `GpuKey`, `GpuSlot`으로 캡슐화한다.

use std::collections::HashMap;
use std::sync::Arc;

use crate::batch::{SpriteBatch, SpriteVertex};
use crate::camera::SpriteUniformsGpu;
use crate::shader::{assemble, WgslModule};
use crate::GpuKey;

/// WGPU GPU 컨텍스트 래퍼.
pub struct RenderContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    textures: HashMap<GpuKey, (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup)>,
    sampler: wgpu::Sampler,
}

impl RenderContext {
    /// Headless(오프스크린) 또는 범용 렌더 컨텍스트 생성.
    pub fn new_headless() -> Result<Self, String> {
        let instance = wgpu::Instance::default();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .map_err(|e| format!("wgpu 어댑터 요청 실패: {e:?}"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .map_err(|e| format!("wgpu 디바이스 생성 실패: {e:?}"))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // 1. 유니폼 버퍼 (208 바이트)
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite_uniform_buffer"),
            size: std::mem::size_of::<SpriteUniformsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 2. 바인드 그룹 레이아웃 0 (정점 유니폼)
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uniform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // 3. 바인드 그룹 레이아웃 1 (픽셀 텍스처 + 샘플러)
        // maplestory.wgsl fs_main(light ps):
        // @group(1) @binding(0) var<uniform> u: LightUniforms;
        // @group(1) @binding(1) var src_tex: texture_2d<f32>;
        // @group(1) @binding(2) var src_sampler: sampler;
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // 4. 파이프라인 레이아웃
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite_pipeline_layout"),
            bind_group_layouts: &[
                Some(&uniform_bind_group_layout),
                Some(&texture_bind_group_layout),
            ],
            immediate_size: 0,
        });

        // 5. WGSL 셰이더 모듈
        let vs_src = assemble(WgslModule::Sprite);
        let fs_src = assemble(WgslModule::MaplePixel);

        let vs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite_vs_module"),
            source: wgpu::ShaderSource::Wgsl(vs_src.into()),
        });
        let fs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("maple_fs_module"),
            source: wgpu::ShaderSource::Wgsl(fs_src.into()),
        });

        // 6. 렌더 파이프라인 (RGBA8UnormSrgb 타깃, 알파 블렌딩)
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite_render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vs_module,
                entry_point: Some("vs_main"),
                buffers: &[Some(SpriteVertex::LAYOUT)],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &fs_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // 7. 기본 동적 정점/인덱스 버퍼 (초기 64KB)
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite_vertex_buffer"),
            size: 65536,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite_index_buffer"),
            size: 65536,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            uniform_bind_group,
            texture_bind_group_layout,
            textures: HashMap::new(),
            sampler,
        })
    }

    /// RGBA8 텍스처 업로드 및 GpuKey 등록.
    pub fn create_texture(
        &mut self,
        key: GpuKey,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<(), String> {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Light ps 용 기본 더미 유니폼 버퍼 (112 바이트)
        let light_uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light_dummy_uniform"),
            size: 112,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite_texture_bind_group"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: light_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        self.textures.insert(key, (texture, view, bind_group));
        Ok(())
    }

    /// 배치 렌더링을 렌더 패스에 기록.
    pub fn render_batch<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        batch: &'a SpriteBatch,
    ) {
        if batch.vertices.is_empty() || batch.indices.is_empty() {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        for cmd in &batch.commands {
            if let Some((_, _, bind_group)) = self.textures.get(&cmd.texture_key) {
                render_pass.set_bind_group(1, bind_group, &[]);
                let end = cmd.index_start + cmd.index_count;
                render_pass.draw_indexed(cmd.index_start..end, 0, 0..1);
            }
        }
    }

    /// 유니폼 및 정점/인덱스 버퍼 업데이트.
    pub fn update_buffers(&mut self, uniforms: &SpriteUniformsGpu, batch: &SpriteBatch) {
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));

        if !batch.vertices.is_empty() {
            let vert_bytes = bytemuck::cast_slice(&batch.vertices);
            if vert_bytes.len() as u64 > self.vertex_buffer.size() {
                self.vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("sprite_vertex_buffer_expanded"),
                    size: (vert_bytes.len() as u64).next_power_of_two(),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            self.queue.write_buffer(&self.vertex_buffer, 0, vert_bytes);
        }

        if !batch.indices.is_empty() {
            let idx_bytes = bytemuck::cast_slice(&batch.indices);
            if idx_bytes.len() as u64 > self.index_buffer.size() {
                self.index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("sprite_index_buffer_expanded"),
                    size: (idx_bytes.len() as u64).next_power_of_two(),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            self.queue.write_buffer(&self.index_buffer, 0, idx_bytes);
        }
    }

    /// 오프스크린 텍스처로 배치를 렌더링하고 RGBA8 픽셀 버퍼로 리드백한다.
    pub fn render_to_rgba(
        &mut self,
        width: u32,
        height: u32,
        batch: &SpriteBatch,
        uniforms: &SpriteUniformsGpu,
        clear_color: [f64; 4],
    ) -> Result<Vec<u8>, String> {
        self.update_buffers(uniforms, batch);

        let texture_desc = wgpu::TextureDescriptor {
            label: Some("offscreen_target_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let target_texture = self.device.create_texture(&texture_desc);
        let target_view = target_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // wgpu 리드백 버퍼 정렬 (256 바이트 정렬)
        let unaligned_bytes_per_row = width * 4;
        let align = 256;
        let bytes_per_row = (unaligned_bytes_per_row + align - 1) & !(align - 1);
        let buffer_size = (bytes_per_row * height) as u64;

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging_readback_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("offscreen_render_encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("offscreen_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear_color[0],
                            g: clear_color[1],
                            b: clear_color[2],
                            a: clear_color[3],
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            self.render_batch(&mut render_pass, batch);
        }

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        // GPU 버퍼 맵핑 및 CPU 복사
        let slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("{e:?}"))?;
        rx.recv()
            .map_err(|e| format!("Map buffer recv error: {e:?}"))?
            .map_err(|e| format!("Map buffer error: {e:?}"))?;

        let mapped = slice
            .get_mapped_range()
            .map_err(|e| format!("{e:?}"))?;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            pixels.extend_from_slice(&mapped[start..end]);
        }

        drop(mapped);
        staging_buffer.unmap();

        Ok(pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_context_creation() {
        // Headless 디바이스 인스턴스화 테스트
        match RenderContext::new_headless() {
            Ok(mut ctx) => {
                // 단색 4x4 RGBA 텍스처 생성
                let pixels = vec![255u8; 4 * 4 * 4];
                assert!(ctx.create_texture(GpuKey(0), 4, 4, &pixels).is_ok());
            }
            Err(e) => {
                // CI나 GPU 드라이버가 없는 환경에서의 안전한 스킵 처리
                eprintln!("GPU Headless 초기화 스킵 (어댑터 없음): {e}");
            }
        }
    }

    #[test]
    fn test_render_to_rgba_readback() {
        match RenderContext::new_headless() {
            Ok(mut ctx) => {
                let batch = SpriteBatch::new();
                let clear_color = [0.2, 0.4, 0.6, 1.0];
                let width = 64;
                let height = 64;
                let camera = crate::Camera2D::new(width as f32, height as f32);
                let uniforms = camera.build_uniforms(0.0, 0.0);
                let res = ctx.render_to_rgba(width, height, &batch, &uniforms, clear_color);
                assert!(res.is_ok());
                let pixels = res.unwrap();
                assert_eq!(pixels.len(), (width * height * 4) as usize);
                // 첫 픽셀의 RGBA 값 확인 (0.2, 0.4, 0.6, 1.0 -> ~51, ~102, ~153, 255)
                assert!(pixels[3] >= 250); // Alpha ~ 255
            }
            Err(e) => {
                eprintln!("GPU Headless 초기화 스킵 (어댑터 없음): {e}");
            }
        }
    }
}
