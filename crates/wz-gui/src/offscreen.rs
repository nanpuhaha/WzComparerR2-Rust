//! 오프스크린 wgpu 스프라이트 렌더 — 업로드/샘플링/알파 블렌드.
//!
//! GUI가 쓰는 것과 같은 `wgpu`로, CPU에서 만든 RGBA 스프라이트를 텍스처로
//! 업로드하고 화면 크기 쿼드에 그린 뒤 다시 읽어 해시한다. 창/서피스가 필요
//! 없으므로 headless 테스트에 쓸 수 있다.
//!
//! - 샘플러: `Nearest` 또는 `Linear`(ClampToEdge)
//! - 블렌드: straight-alpha source-over
//!   (`color: SrcAlpha/OneMinusSrcAlpha`, `alpha: One/OneMinusSrcAlpha`)
//! - 렌더 타깃: `Rgba8Unorm`(sRGB 변환 없음) + `COPY_SRC`

/// 업로드할 스프라이트.
pub struct GpuSprite {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleMode {
    Nearest,
    Linear,
}

const SHADER: &str = r#"
struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VOut {
    var p = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0),
        vec2(-1.0, 1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
    );
    var uv = array<vec2<f32>, 6>(
        vec2(0.0, 1.0), vec2(1.0, 1.0), vec2(0.0, 0.0),
        vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(1.0, 0.0),
    );
    var out: VOut;
    out.pos = vec4<f32>(p[i], 0.0, 1.0);
    out.uv = uv[i];
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv);
}
"#;

/// 스프라이트를 `out_w × out_h`에 렌더해 RGBA를 돌려준다.
///
/// GPU 어댑터가 없으면 `None`(테스트는 skip).
pub fn render_sprite(
    sprite: &GpuSprite,
    out_w: u32,
    out_h: u32,
    mode: SampleMode,
) -> Option<(u32, u32, Vec<u8>)> {
    use wgpu::util::DeviceExt;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("wz-offscreen"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
    }))
    .ok()?;

    let texture = device.create_texture_with_data(
        &queue,
        &wgpu::TextureDescriptor {
            label: Some("sprite"),
            size: wgpu::Extent3d {
                width: sprite.width,
                height: sprite.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &sprite.rgba,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("sprite-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: match mode {
            SampleMode::Nearest => wgpu::FilterMode::Nearest,
            SampleMode::Linear => wgpu::FilterMode::Linear,
        },
        min_filter: match mode {
            SampleMode::Nearest => wgpu::FilterMode::Nearest,
            SampleMode::Linear => wgpu::FilterMode::Linear,
        },
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d {
            width: out_w,
            height: out_h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sprite-shader"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("sprite-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sprite-bg"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("sprite-pl"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sprite-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let bytes_per_row = 256 * out_w.div_ceil(256);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(bytes_per_row) * u64::from(out_h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("sprite-encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sprite-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(out_h),
            },
        },
        wgpu::Extent3d {
            width: out_w,
            height: out_h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).ok()?;
    rx.recv().ok()?.ok()?;
    let data = slice.get_mapped_range().ok()?;

    let mut out = Vec::with_capacity((out_w * out_h * 4) as usize);
    for row in 0..out_h {
        let start = (row * bytes_per_row) as usize;
        out.extend_from_slice(&data[start..start + (out_w * 4) as usize]);
    }
    drop(data);
    buffer.unmap();

    Some((out_w, out_h, out))
}

/// 렌더 결과의 SHA-256.
pub fn render_sprite_sha256(
    sprite: &GpuSprite,
    out_w: u32,
    out_h: u32,
    mode: SampleMode,
) -> Option<String> {
    use sha2::{Digest, Sha256};
    let (_, _, rgba) = render_sprite(sprite, out_w, out_h, mode)?;
    let mut hasher = Sha256::new();
    hasher.update(&rgba);
    Some(format!("{:x}", hasher.finalize()))
}

/// 스모크/테스트 공용 2×2 스프라이트(불투명/반투명/투명).
pub fn test_sprite() -> GpuSprite {
    GpuSprite {
        width: 2,
        height: 2,
        rgba: vec![
            255, 0, 0, 255, // 빨강
            0, 255, 0, 255, // 초록
            0, 0, 255, 128, // 반투명 파랑
            0, 0, 0, 0, // 투명
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_upload_and_blend_hash_is_stable() {
        let sprite = test_sprite();
        let Some(hash) = render_sprite_sha256(&sprite, 4, 4, SampleMode::Nearest) else {
            eprintln!("skip: no wgpu adapter");
            return;
        };
        // GPU 블렌드는 straight-alpha 소스에 `SrcAlpha`를 곱하므로 색이
        // **premultiplied**로 기록된다(0,0,255,128) → (0,0,128,128).
        let (_, _, rgba) = render_sprite(&sprite, 4, 4, SampleMode::Nearest).unwrap();
        assert_eq!(rgba.len(), 4 * 4 * 4);
        assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&rgba[4..8], &[255, 0, 0, 255]);
        assert_eq!(&rgba[32..36], &[0, 0, 128, 128]);
        eprintln!("nearest 4x4 sha256={hash}");
    }

    #[test]
    fn linear_and_nearest_differ() {
        let sprite = test_sprite();
        let nearest = render_sprite_sha256(&sprite, 8, 8, SampleMode::Nearest);
        let linear = render_sprite_sha256(&sprite, 8, 8, SampleMode::Linear);
        match (nearest, linear) {
            (Some(n), Some(l)) => assert_ne!(n, l, "filters must differ"),
            _ => eprintln!("skip: no wgpu adapter"),
        }
    }
}
