// wz-gui — wz-render ↔ egui 브릿지.
//
// `docs/gui-architecture.md` §7: wgpu 타입이 panels/ 로 새지 않게 이 파일에 격리한다.

use crate::state::AppState;
use wz_render::atlas::AtlasShelfPacker;
use wz_render::batch::SpriteBatch;
use wz_render::{GpuKey, SpotLight};

fn argb_components(packed: u32) -> [f32; 4] {
    let b = (packed & 0xFF) as f32 / 255.0;
    let g = ((packed >> 8) & 0xFF) as f32 / 255.0;
    let r = ((packed >> 16) & 0xFF) as f32 / 255.0;
    let a = ((packed >> 24) & 0xFF) as f32 / 255.0;
    [r, g, b, a]
}

fn draw_background_layers(batch: &mut SpriteBatch, state: &AppState, front: bool) {
    let center = [state.camera.center.x, state.camera.center.y];
    let viewport = [state.viewport_size.0 as f32, state.viewport_size.1 as f32];
    let time = state.map_background_time;
    for (i, layer) in state.map_background_images.iter().enumerate() {
        if layer.front != front || layer.frames.is_empty() {
            continue;
        }
        let frame = layer.frame_at(time);
        let Some((fw, fh, _)) = layer.frames.get(frame) else {
            continue;
        };
        let position = crate::background::background_position(layer, center, time);
        let (cx, cy) = crate::background::effective_tile_size(layer);
        let (l, t, r, b) =
            crate::background::tile_range(layer, position, center, viewport, state.camera.zoom);
        let uv = if layer.flip {
            [1.0, 0.0, 0.0, 1.0]
        } else {
            [0.0, 0.0, 1.0, 1.0]
        };
        let color = [1.0, 1.0, 1.0, layer.alpha as f32 / 255.0];
        let key = GpuKey(10_000 + i as u32 * 64 + frame as u32);
        let z = if front { 1000.0 } else { -1000.0 + i as f32 };
        for ty in t..b {
            for tx in l..r {
                let x = position[0] + tx as f32 * cx as f32;
                let y = position[1] + ty as f32 * cy as f32;
                batch.push_quad(key, [x, y, *fw as f32, *fh as f32], z, uv, color);
            }
        }
    }
}

pub fn render_viewport(ui: &mut egui::Ui, state: &mut AppState) {
    let animated_background = state
        .map_background_images
        .iter()
        .any(|l| l.frames.len() > 1 || matches!(l.tile_mode, 4..=7));
    if animated_background {
        let now = ui.input(|i| i.time);
        if state.map_background_last > 0.0 {
            state.map_background_time += (now - state.map_background_last).clamp(0.0, 0.1);
        }
        state.map_background_last = now;
        state.viewport_dirty = true;
        ui.ctx().request_repaint();
    }
    if !state.map_particle_layers.is_empty() {
        let now = ui.input(|i| i.time);
        if state.map_particle_last > 0.0 {
            state.map_particle_time += (now - state.map_particle_last).clamp(0.0, 0.1);
        }
        state.map_particle_last = now;
        state.viewport_dirty = true;
        ui.ctx().request_repaint();
    }
    let resp = ui.allocate_response(
        egui::vec2(state.viewport_size.0 as f32, state.viewport_size.1 as f32),
        egui::Sense::click_and_drag(),
    );
    if resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if let Some(last) = state.viewport_last_pos {
                let dx = pos.x - last.x;
                let dy = pos.y - last.y;
                state.camera.center.x -= dx / state.camera.zoom;
                state.camera.center.y -= dy / state.camera.zoom;
                state.viewport_dirty = true;
            }
            state.viewport_last_pos = Some(pos);
            state.viewport_dragging = true;
        }
    } else {
        state.viewport_dragging = false;
        state.viewport_last_pos = None;
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let factor = if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 };
            state.camera.zoom = (state.camera.zoom * factor).clamp(0.1, 10.0);
            state.viewport_dirty = true;
        }
    }

    if state.viewport_dirty {
        state.viewport_dirty = false;
        let (w, h) = state.viewport_size;
        let mut rc = match wz_render::RenderContext::new_headless() {
            Ok(r) => r,
            Err(_) => return,
        };
        let mut packer = AtlasShelfPacker::new(64, 64);
        let slot = packer
            .allocate_and_write(1, 1, &[255, 255, 255, 255])
            .unwrap();
        let atlas_key = slot.key;
        let atlas_w = 64u32;
        let atlas_h = 64u32;
        for page in packer.pages() {
            let _ = rc.create_texture(page.key, page.width, page.height, &page.pixels);
        }

        // 맵 데이터가 바뀌었을 때만 레이어 쿼드를 재구축한다 (캐시).
        if state.viewport_map_dirty {
            state.viewport_map_dirty = false;
            state.viewport_cached_layers.clear();
            state.viewport_cached_has_back = false;
            if state.map_back_image.is_some()
                || !state.map_tile_images.is_empty()
                || !state.map_obj_images.is_empty()
                || !state.map_background_images.is_empty()
            {
                state.viewport_cached_has_back = state.map_back_image.is_some();
                // back (non-front): z = -1000
                if state.map_back_image.is_some() && state.map_background_images.is_empty() {
                    state
                        .viewport_cached_layers
                        .push((GpuKey(2), [0.0, 0.0, 0.0, 0.0], -1000.0));
                }
                // tiles: z = -500
                for (i, (tx, ty, tw, th, _)) in state.map_tile_images.iter().enumerate() {
                    state.viewport_cached_layers.push((
                        GpuKey(100 + i as u32),
                        [*tx as f32, *ty as f32, *tw as f32, *th as f32],
                        -500.0,
                    ));
                }
                // objects: z = ObjItem.z
                for (i, (ox, oy, ow, oh, _)) in state.map_obj_images.iter().enumerate() {
                    let z = state
                        .current_map
                        .as_ref()
                        .and_then(|m| m.objects.get(i))
                        .map(|o| o.z)
                        .unwrap_or(0) as f32;
                    state.viewport_cached_layers.push((
                        GpuKey(200 + i as u32),
                        [*ox as f32, *oy as f32, *ow as f32, *oh as f32],
                        z,
                    ));
                }
                // reactors: z = 0
                for (i, (rx, ry, rw, rh, _)) in state.map_reactor_images.iter().enumerate() {
                    state.viewport_cached_layers.push((
                        GpuKey(300 + i as u32),
                        [*rx as f32, *ry as f32, *rw as f32, *rh as f32],
                        0.0,
                    ));
                }
            }
        }

        let mut batch = SpriteBatch::new();
        if !state.viewport_cached_layers.is_empty()
            || state.viewport_cached_has_back
            || !state.map_background_images.is_empty()
        {
            // 텍스처 업로드 (새 RenderContext 이므로 매 렌더 필요)
            if let Some((bw, bh, bpx)) = &state.map_back_image {
                let _ = rc.create_texture(GpuKey(2), *bw, *bh, bpx);
            }
            for (i, (_, _, tw, th, tpx)) in state.map_tile_images.iter().enumerate() {
                let _ = rc.create_texture(GpuKey(100 + i as u32), *tw, *th, tpx);
            }
            for (i, (_, _, ow, oh, opx)) in state.map_obj_images.iter().enumerate() {
                let _ = rc.create_texture(GpuKey(200 + i as u32), *ow, *oh, opx);
            }
            for (i, (_, _, rw, rh, rpx)) in state.map_reactor_images.iter().enumerate() {
                let _ = rc.create_texture(GpuKey(300 + i as u32), *rw, *rh, rpx);
            }
            for (i, layer) in state.map_background_images.iter().enumerate() {
                for (f, (fw, fh, fpx)) in layer.frames.iter().enumerate() {
                    let _ =
                        rc.create_texture(GpuKey(10_000 + i as u32 * 64 + f as u32), *fw, *fh, fpx);
                }
            }
            for (i, layer) in state.map_particle_layers.iter().enumerate() {
                let (pw, ph, ppx) = &layer.texture;
                let _ = rc.create_texture(GpuKey(600 + i as u32), *pw, *ph, ppx);
            }
            draw_background_layers(&mut batch, state, false);
            // 캐시된 레이어를 z 순서로 정렬해 push
            let mut sorted: Vec<&(GpuKey, [f32; 4], f32)> =
                state.viewport_cached_layers.iter().collect();
            sorted.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
            for (key, rect, z) in sorted {
                if key.0 == 2 {
                    if let Some((bw, bh, _)) = &state.map_back_image {
                        batch.push_quad(
                            *key,
                            [0.0, 0.0, *bw as f32, *bh as f32],
                            *z,
                            [0.0, 0.0, 1.0, 1.0],
                            [1.0, 1.0, 1.0, 1.0],
                        );
                    }
                } else {
                    batch.push_quad(*key, *rect, *z, [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
                }
            }
        } else if state.map_back_image.is_none()
            && state.map_tile_images.is_empty()
            && state.map_obj_images.is_empty()
            && state.map_reactor_images.is_empty()
            && state.map_background_images.is_empty()
        {
            // 그리드 폴백
            for gx in -2..=2i32 {
                for gy in -2..=2i32 {
                    let color = [
                        0.3 + (gx as f32 + 2.0) * 0.12,
                        0.3 + (gy as f32 + 2.0) * 0.12,
                        0.6,
                        1.0,
                    ];
                    let x = gx as f32 * 40.0 - 20.0;
                    let y = gy as f32 * 40.0 - 20.0;
                    batch.push_quad(
                        atlas_key,
                        [x, y, 36.0, 36.0],
                        0.0,
                        slot.rect.uv(atlas_w, atlas_h),
                        color,
                    );
                }
            }
            draw_background_layers(&mut batch, state, true);
        }
        // map particles: 원본 ParticleSystem.Draw 와 같은 정적 샘플링.
        if !state.map_particle_layers.is_empty() {
            let center = [state.camera.center.x, state.camera.center.y];
            let time = state.map_particle_time;
            for (i, layer) in state.map_particle_layers.iter().enumerate() {
                let blend = crate::particles::blend_mode(&layer.desc);
                let z = 100.0 + layer.z as f32;
                for sprite in crate::particles::sample_layer(layer, time, center) {
                    crate::particles::append_sprite(
                        &mut batch,
                        GpuKey(600 + i as u32),
                        &sprite,
                        z,
                        blend,
                    );
                }
            }
        }
        // map light: 원본 LightRenderer.DrawSpotLight 와 동일한 가산 메시.
        if let Some(light) = state.current_map.as_ref().and_then(|m| m.light.as_ref()) {
            for l in &light.lights {
                SpotLight {
                    pos: [l.x as f32, l.y as f32],
                    inner_radius: l.inner_radius as f32,
                    outer_radius: l.outer_radius as f32,
                    inner_angle: l.inner_angle as f32,
                    outer_angle: l.outer_angle as f32,
                    color: argb_components(l.color),
                }
                .append(&mut batch, atlas_key, 2000.0);
            }
        }
        batch.finish();

        let uniforms = state.camera.build_uniforms(0.0, 0.0);
        if let Ok(rgba) = rc.render_to_rgba(w, h, &batch, &uniforms, [0.08, 0.08, 0.12, 1.0]) {
            let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            state.viewport_texture = Some(ui.ctx().load_texture(
                "viewport",
                ci,
                egui::TextureOptions::LINEAR,
            ));
        }
    }

    if let Some(tex) = &state.viewport_texture {
        ui.add(egui::Image::new(tex).max_width(state.viewport_size.0 as f32));
    } else {
        ui.label("(뷰포트 렌더링 대기)");
    }

    // 발판 선분
    if let Some(m) = &state.current_map {
        let painter = ui.painter();
        let rect = resp.rect;
        for fh in &m.footholds {
            let s1 = state
                .camera
                .world_to_screen(glam::Vec2::new(fh.x1 as f32, fh.y1 as f32));
            let s2 = state
                .camera
                .world_to_screen(glam::Vec2::new(fh.x2 as f32, fh.y2 as f32));
            let p1 = egui::pos2(rect.min.x + s1.x, rect.min.y + s1.y);
            let p2 = egui::pos2(rect.min.x + s2.x, rect.min.y + s2.y);
            painter.line_segment([p1, p2], egui::Stroke::new(1.5, egui::Color32::GREEN));
        }
        ui.label(format!(
            "발판 {}개, 배경 {}개, 타일 {}개, 오브젝트 {}개, 리액터 {}개, 파티클 {}개, 광원 {}개 표시",
            m.footholds.len(),
            state.map_background_images.len(),
            state.map_tile_images.len(),
            state.map_obj_images.len(),
            state.map_reactor_images.len(),
            state.map_particle_layers.len(),
            m.light.as_ref().map(|l| l.lights.len()).unwrap_or(0)
        ));
    }

    ui.label(format!(
        "카메라: pos=({:.1},{:.1}) zoom={:.2}  (드래그=팬, 휠=줌)",
        state.camera.center.x, state.camera.center.y, state.camera.zoom
    ));
}
