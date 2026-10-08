//! wgpu 파이프라인 도달성 스파이크 (headless).
//!
//! # 왜 이 파일이 존재하는가
//!
//! 리포트 §9.3 은 "egui + wgpu 로 MapRender" 를 Phase 6 GUI 방향으로 *제안*
//!했을 뿐, 실제 측정은 없었다. Phase 6 을 6~8주 착수하기 전에 **반나절짜리 스파이크**로
//! 다음을 실측한다:
//!
//! 1. 이 머신에서 wgpu 어댑터가 잡히는가 (Vulkan / DX12 / Metal 중 무엇이 되는가)
//! 2. `sprite.wgsl` + `maplestory.wgsl` 로 **실제 렌더 파이프라인이 생성되는가**
//! 3. 그 파이프라인으로 GPU에 그림을 그리고 **픽셀을 되읽을 수 있는가**
//!
//! 3번이 핵심이다. "파이프라인이 만들어졌다" 는 것은 "그려진다" 를 의미하지 않는다.
//! 텍스처 없음 렌더 타깃 + 되읽기까지 성공해야, Phase 6 착수가 정당화된다.
//!
//! # 창 없이 도는 이유
//!
//! CI(GitHub Actions 러너)와 개발자 노트북 양쪽에서 돌려야 한다. 창을 여는
//! 스파이크는 헤드리스 러너에서 실패하므로, `wgpu::Backends::empty()` 로 오프스크린
//! 렌더 타깃을 만들고 `copy_texture_to_buffer` 로 되읽는다.
//!
//! # 스킵 조건
//!
//! GPU 가 하나도 없는 환경(컨테이너, WSL1, CI 일부 러너)에서는 스파이크가
//! 실패가 아니라 **스킵**이어야 한다. GPU 부재를 렌더러 결함으로 오인하면
//! CI 가 거짓 음성을 낸다. 그래서 실패와 스킵을 구분한다.

use wz_render::shader::{self, MaplePixelShader, SpritePipelineDesc, WgslModule};

fn main() {
    println!("=== wgpu 파이프라인 도달성 스파이크 ===");

    let Some(gpu) = Gpu::probe() else {
        println!("SKIP: 이 머신에서 wgpu 어댑터를 잡지 못했다 (GPU 없음 / 드라이버 미설치)");
        println!("      → 결함이 아니라 환경 제약이므로 성공으로 종료");
        std::process::exit(0);
    };
    println!("adapter : {}", gpu.adapter_info);
    println!("backend : {:?}", gpu.backend);

    // ── 1. 3개 픽셀 셰이더 전부에 대해 실제 파이프라인 생성 ──────────────
    let mut created = 0usize;
    for desc in shader::all_pipelines() {
        let label = desc.pixel_shader.wz_id();
        match gpu.create_pipeline(&desc) {
            Ok(()) => {
                println!("  [OK]   {label}");
                created += 1;
            }
            Err(e) => {
                println!("  [FAIL] {label}: {e}");
                std::process::exit(1);
            }
        }
    }
    assert_eq!(created, 3, "3개 파이프라인 전부 생성되어야 한다");
    println!("\n파이프라인 생성 3/3 성공");

    // ── 2. 실제 그림 그리고 되읽기 ─────────────────────────────────────
    // 픽셀 셰이더는 uniform 을 읽으므로 최소한의 유효한 CB 를 채워야 한다.
    // `light` 셰이더가 제일 단순하다 (uniform + texture + sampler).
    match gpu.render_and_readback() {
        Ok(px) => {
            println!("되읽기 {} 바이트", px.len());
            let nonzero = px.iter().filter(|b| **b != 0).count();
            println!("0 이 아닌 바이트: {}/{}", nonzero, px.len());
            if nonzero == 0 {
                println!("FAIL: 전부 0 — 셰이더가 아무것도 그리지 않았다 (silent no-op)");
                std::process::exit(1);
            }
            println!("\n결론: 파이프라인 생성 → 실제 드로우 → GPU 되읽기 통과");
            println!("      Phase 6 (egui + wgpu) 착수 근거 확보");
        }
        Err(e) => {
            println!("FAIL: 렌더/되읽기 실패: {e}");
            std::process::exit(1);
        }
    }

    // ── 3. inter-stage ABI 요약 (사람이 읽는 형태) ──────────────────
    let sprite = shader::assemble(WgslModule::Sprite);
    let maple = shader::assemble(WgslModule::MaplePixel);
    let vs = shader::inter_stage_layout(&sprite, "vs_main", naga::ShaderStage::Vertex).unwrap();
    println!("\ninter-stage 레이아웃:");
    println!("  정점 출력 : {:?}", vs);
    for p in [
        MaplePixelShader::Light,
        MaplePixelShader::WaterBack,
        MaplePixelShader::WaterFront,
    ] {
        let fs = shader::inter_stage_layout(&maple, p.entry_point(), naga::ShaderStage::Fragment)
            .unwrap();
        println!("  {:<12}: {:?}", p.wz_id(), fs);
    }
}

struct Gpu {
    // Instance/Adapter 핸들은 Device 가 살아 있는 동안 유지돼야 한다.
    // (wgpu 백엔드에 따라 Adapter 를 먼저 드롭하면 Device 가 무효화된다.)
    // 지금은 값만 보관하고 직접 참조하지는 않으므로 dead_code 를 허용한다.
    #[allow(dead_code)]
    instance: wgpu::Instance,
    #[allow(dead_code)]
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_info: String,
    backend: wgpu::Backend,
}

impl Gpu {
    fn probe() -> Option<Self> {
        // 창 없는 렌더를 위해 백엔드를 제한하지 않는다 (Vulkan/DX12/Metal 전부 시도).
        // wgpu 30 의 `InstanceDescriptor` 에는 `Default` 가 없다.
        // 헤드리스(창 없음)용으로 제공되는 생성자를 쓴다 — 우리가 하려는
        // 것이 정확히 그 경우다. 백엔드는 전부 열어 Vulkan/DX12/Metal 중
        // 머신이 지원하는 것을 고르게 한다.
        let mut instance = wgpu::InstanceDescriptor::new_without_display_handle();
        instance.backends = wgpu::Backends::all();
        let instance = wgpu::Instance::new(instance);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .ok()?;

        let info = adapter.get_info();
        let adapter_info = format!("{} ({:?} {:?})", info.name, info.device_type, info.backend);

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("spike"),
            required_features: wgpu::Features::empty(),
            required_limits:
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .ok()?;

        Some(Self {
            instance,
            adapter,
            device,
            queue,
            adapter_info,
            backend: info.backend,
        })
    }

    fn create_pipeline(&self, desc: &SpritePipelineDesc) -> Result<(), String> {
        let vs_src = shader::assemble(WgslModule::Sprite);
        let fs_src = shader::assemble(WgslModule::MaplePixel);

        let vs_module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("sprite"),
                source: wgpu::ShaderSource::Wgsl(vs_src.into()),
            });
        let fs_module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(desc.pixel_shader.wz_id()),
                source: wgpu::ShaderSource::Wgsl(fs_src.into()),
            });

        // ★ 여기서 inter-stage 불일치가 터진다. naga 검증은 각 모듈을 개별적으로
        // 통과시키지만 파이프라인 생성은 정점/픽셀을 함께 본다.
        // (이 스파이크가 시작 전에 이미 그 차이를 고쳤지만, 이 지점이 검출 지점이다.)
        let module = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("spike"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &vs_module,
                    entry_point: Some(desc.vertex_shader.entry_point()),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        // pos(16) + color(16) + uv(8) = 40 바이트.
                        // vec2 의 8바이트 정렬 때문에 36 이 아니라 40 이다.
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
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &fs_module,
                    entry_point: Some(desc.pixel_shader.entry_point()),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        // 생성 요청의 검증 결과는 에러 핸들러로 오므로, 살아있으면 성공이다.
        let _ = module;
        Ok(())
    }

    fn render_and_readback(&self) -> Result<Vec<u8>, String> {
        const W: u32 = 64;
        const H: u32 = 64;

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("spike-target"),
            size: wgpu::Extent3d {
                width: W,
                height: H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());

        // rows_per_image 는 256 배수 정렬이 필요 없다(write_* 는 예외).
        // 되읽기는 copy_texture_to_buffer 라서 256 정렬이 필요하다.
        let bytes_per_row = W * 4; // 256 → 이미 정렬됨
        debug_assert_eq!(bytes_per_row % 256, 0);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("spike-readback"),
            size: u64::from(bytes_per_row) * u64::from(H),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // `light` 유니폼: z + player_pos/radii/color/top/bottom/min_y/max_y
        // (WGSL LightUniforms 레이아웃 그대로, 96 바이트)
        let mut u = [0f32; 24];
        u[0] = 0.0; // z (0 이면 깊이 감쇠 비활성)
        u[2] = 0.5; // player_pos.x
        u[3] = 0.5; // player_pos.y
        u[4] = 0.0; // inner
        u[5] = 2.0; // outer (큰 값 → 전역 조명)
        u[6..10].copy_from_slice(&[1.0, 1.0, 1.0, 1.0]); // player_light_color
        u[10..14].copy_from_slice(&[0.2, 0.4, 0.9, 1.0]); // top_color
        u[14..18].copy_from_slice(&[0.9, 0.4, 0.2, 1.0]); // bottom_color
        u[18] = -1.0; // min_y
        u[19] = 1.0; // max_y

        let ubo = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("spike-material-cb"),
            size: (u.len() * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&ubo, 0, bytemuck::cast_slice(&u));

        // ── 정점 프레임 CB (group 0) ──
        // SpriteUniforms 레이아웃 (float 인덱스, `common.wgsl` 참조):
        //   [ 0..16) vp            (mat4, column-major)
        //   [16..32) vp_inv        (mat4)
        //   [32..36) resolution_time (vec4)
        //   [36..52) world         (mat4)
        let mut f = [0f32; 52];
        // vp = identity (clip space 로 그대로 통과)
        f[0] = 1.0; // col0.x
        f[5] = 1.0; // col1.y
        f[10] = 1.0; // col2.z
        f[15] = 1.0; // col3.w
                     // world = identity (오프셋 36). column-major 이므로 대각은 0,5,10,15.
        for col in [0usize, 5, 10, 15] {
            f[36 + col] = 1.0;
        }
        f[32] = 1.0; // resolution_time.x
        f[33] = 1.0; // resolution_time.y
        f[34] = 0.0; // resolution_time.z
        f[35] = 0.0; // resolution_time.w (시간)
        let frame_ubo = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("spike-frame-cb"),
            size: (f.len() * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue
            .write_buffer(&frame_ubo, 0, bytemuck::cast_slice(&f));

        // 1×1 흰색 텍스처 (src_tex)
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("spike-src"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let tex_view = tex.create_view(&Default::default());
        let sampler = self
            .device
            .create_sampler(&wgpu::SamplerDescriptor::default());

        // group(0) = 정점 프레임 CB. `common.wgsl` 의 그룹 규약.
        let frame_bgl = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("spike-frame-bgl"),
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
        let frame_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("spike-frame-bg"),
            layout: &frame_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_ubo.as_entire_binding(),
            }],
        });

        // group(1) = 픽셀 재질 리소스. 정점과 그룹이 달라야 충돌하지 않는다.
        let bgl = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("spike-material-bgl"),
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
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("spike-bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ubo.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&tex_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // 삼각형 하나. clip space Y 가 위쪽(+) 이므로 정점 순서를 맞춘다.
        //   a=(-1,-1) b=(3,-1) c=(-1,3)
        // 정점당 10 float = pos(4) + color(4) + uv(2) = 40 바이트.
        // 이 순서가 array_stride/offset 과 정확히 일치해야 한다.
        let verts: [f32; 30] = [
            // x, y, z, w | r, g, b, a | u, v
            -1.0, -1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, //
            3.0, -1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, //
            -1.0, 3.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 1.0, //
        ];
        let vbo = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("spike-verts"),
            size: std::mem::size_of_val(&verts) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue
            .write_buffer(&vbo, 0, bytemuck::cast_slice(&verts));

        let vs_module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("sprite"),
                source: wgpu::ShaderSource::Wgsl(shader::assemble(WgslModule::Sprite).into()),
            });
        let fs_module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("light"),
                source: wgpu::ShaderSource::Wgsl(shader::assemble(WgslModule::MaplePixel).into()),
            });
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("spike-pl"),
                bind_group_layouts: &[Some(&frame_bgl), Some(&bgl)],
                immediate_size: 0,
            });
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("spike"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &vs_module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
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
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &fs_module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("spike"),
            });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("spike"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
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
            pass.set_bind_group(0, &frame_bg, &[]);
            pass.set_bind_group(1, &bg, &[]);
            pass.set_vertex_buffer(0, vbo.slice(..));
            pass.draw(0..3, 0..1);
        }
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(H),
                },
            },
            wgpu::Extent3d {
                width: W,
                height: H,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(enc.finish()));

        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        rx.recv()
            .map_err(|e| format!("map 콜백 실패: {e}"))?
            .map_err(|e| format!("맵 실패: {e}"))?;

        // `get_mapped_range` 가 돌려주는 뷰는 borrow 를 잡고 있으므로, 데이터를
        // 복사한 뒤 **그 뷰를 먼저 죽여야** `unmap` 이 성공한다.
        // (wgpu: "You cannot unmap a buffer that still has accessible mapped views")
        let out = {
            let data = slice
                .get_mapped_range()
                .map_err(|e| format!("get_mapped_range 실패: {e:?}"))?;
            data.to_vec()
        };
        buffer.unmap();
        Ok(out)
    }
}
