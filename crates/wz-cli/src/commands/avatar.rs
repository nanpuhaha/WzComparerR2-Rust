use std::path::PathBuf;



pub fn cmd_avatar(
    demo: bool,
    render: Option<PathBuf>,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;
    use wz_avatar::{AvatarSlot, Bone, Point, SkinLayer, ZMap};

    println!("=== 29개 MapleStory 아바타 슬롯 ===");
    for i in 0..AvatarSlot::TOTAL_SLOTS {
        if let Some(slot) = AvatarSlot::from_index(i) {
            println!("  [{:02}] {:?}", i, slot);
        }
    }

    let zmap_order = vec![
        "backHair".into(),
        "body".into(),
        "arm".into(),
        "head".into(),
        "hair".into(),
    ];
    let zmap = Arc::new(ZMap::new(zmap_order));
    let mut root = Bone::new("@root");

    let mut body_skin = SkinLayer::new("body");
    body_skin.z = "body".into();
    body_skin.width = 32;
    body_skin.height = 32;
    root.append_bone(body_skin, "navel", Point::new(10, 20));

    let mut head_skin = SkinLayer::new("head");
    head_skin.z = "head".into();
    head_skin.width = 44;
    head_skin.height = 44;
    root.append_bone(head_skin, "navel", Point::new(0, 10));

    let scene = wz_avatar::AvatarScene::new(zmap);

    if demo {
        println!();
        println!("=== 아바타 Z-Map 레이어 조합 & 앵커 정렬 데모 ===");
        let layers = scene.canvas.generate_layers(&root);
        println!("생성된 레이어 수: {}", layers.len());
        for (i, layer) in layers.iter().enumerate() {
            println!(
                "  레이어 #{i}: {} (pos: {:?}, size: {}x{}, prio: {})",
                layer.name, layer.position, layer.width, layer.height, layer.z_index
            );
        }
        let batch = scene.build_batch(&root);
        println!(
            "생성된 GPU 렌더 배치: 정점 {} 개, 인덱스 {} 개 (삼각형 {} 개)",
            batch.vertex_count(),
            batch.index_count(),
            batch.index_count() / 3
        );
    }

    let resolved_output = output.or_else(|| {
        render
            .as_ref()
            .filter(|p| !p.as_os_str().is_empty())
            .cloned()
    });
    let should_render = render.is_some() || resolved_output.is_some();

    if should_render {
        println!();
        println!("=== wz-render 아바타 스프라이트 배치 및 오프스크린 렌더 ===");
        let layers = scene.canvas.generate_layers(&root);

        let atlas_w = 512u32;
        let atlas_h = 512u32;
        let mut packer = wz_render::AtlasShelfPacker::new(atlas_w, atlas_h);

        let mut layer_slots = Vec::with_capacity(layers.len());

        for layer in &layers {
            let lw = layer.width.max(1);
            let lh = layer.height.max(1);

            // 레이어별 시각적 구분을 위한 색상 RGBA 픽셀 생성 (body: 살색, head: 갈색 등)
            let color = match layer.name.as_str() {
                "body" => [240u8, 200, 160, 255],
                "head" => [180u8, 120, 80, 255],
                _ => [220u8, 220, 220, 255],
            };
            let mut pixels = Vec::with_capacity((lw * lh * 4) as usize);
            for _ in 0..(lw * lh) {
                pixels.extend_from_slice(&color);
            }

            let slot = packer
                .allocate_and_write(lw, lh, &pixels)
                .ok_or("아틀라스 텍스처 슬롯 할당 실패")?;
            println!(
                "레이어 '{}' [{}x{}] -> 아틀라스 할당: GpuKey({}), Rect [x:{}, y:{}, w:{}, h:{}]",
                layer.name, lw, lh, slot.key.0, slot.rect.x, slot.rect.y, slot.rect.w, slot.rect.h
            );

            layer_slots.push(slot);
        }

        let render_w = 256u32;
        let render_h = 256u32;
        let camera = wz_render::Camera2D::new(render_w as f32, render_h as f32);
        let uniforms = camera.build_uniforms(0.0, 0.0);
        println!(
            "2D 직교 카메라 유니폼 생성 완료: 뷰포트 [{}, {}], 줌 {}",
            camera.viewport_size.x, camera.viewport_size.y, camera.zoom
        );

        let mut sprite_batch = wz_render::SpriteBatch::new();
        for (layer, slot) in layers.iter().zip(layer_slots.iter()) {
            let uv = slot.rect.uv(atlas_w, atlas_h);
            // 카메라가 (0, 0)을 바라보므로 아바타 레이어 좌표를 원점 주변에 배치
            let world_x = layer.position.x as f32;
            let world_y = layer.position.y as f32;

            sprite_batch.push_quad(
                slot.key,
                [world_x, world_y, slot.rect.w as f32, slot.rect.h as f32],
                0.0,
                uv,
                [1.0, 1.0, 1.0, 1.0],
            );
        }
        sprite_batch.finish();

        println!(
            "아바타 스프라이트 배치 빌드 완료: 정점 {}개, 인덱스 {}개, 드로우 커맨드 {}개",
            sprite_batch.vertices.len(),
            sprite_batch.indices.len(),
            sprite_batch.commands.len()
        );

        match wz_render::RenderContext::new_headless() {
            Ok(mut ctx) => {
                for page in packer.pages() {
                    ctx.create_texture(page.key, page.width, page.height, &page.pixels)?;
                }
                ctx.update_buffers(&uniforms, &sprite_batch);
                println!("wgpu 렌더 컨텍스트 초기화 및 GPU 아틀라스 페이지 업로드 성공 (Headless)");

                let clear_color = [0.08, 0.08, 0.12, 1.0];
                let pixels =
                    ctx.render_to_rgba(render_w, render_h, &sprite_batch, &uniforms, clear_color)?;
                println!(
                    "오프스크린 렌더링 및 픽셀 리드백 성공: {}x{} (총 {} 바이트)",
                    render_w,
                    render_h,
                    pixels.len()
                );

                if let Some(out_path) = resolved_output {
                    let is_png = out_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
                    if is_png {
                        let png_bytes = wz_image::encode_png_rgba8(render_w, render_h, &pixels);
                        std::fs::write(&out_path, &png_bytes)?;
                        println!(
                            "렌더링된 아바타 PNG 이미지 저장 완료 ({} 바이트): {}",
                            png_bytes.len(),
                            out_path.display()
                        );
                    } else {
                        std::fs::write(&out_path, &pixels)?;
                        println!("렌더링된 아바타 RGBA 원시 버퍼 저장 완료: {}", out_path.display());
                    }
                }
            }
            Err(e) => {
                println!(
                    "(안내) 호환되는 GPU 어댑터가 없는 환경이므로 실제 디바이스 생성을 건너뜁니다: {e}"
                );
            }
        }
    }
    Ok(())
}
