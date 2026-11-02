// wz-gui — wz-render ↔ egui 브릿지.
//
// `docs/gui-architecture.md` §7: wgpu 타입이 panels/ 로 새지 않게 이 파일에 격리한다.


use crate::state::AppState;
use wz_render::batch::SpriteBatch;
use wz_render::atlas::AtlasShelfPacker;
use wz_render::GpuKey;

pub fn render_viewport(ui: &mut egui::Ui, state: &mut AppState) {
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
        let slot = packer.allocate_and_write(1, 1, &[255, 255, 255, 255]).unwrap();
        let atlas_key = slot.key;
        let atlas_w = 64u32;
        let atlas_h = 64u32;
        for page in packer.pages() {
            let _ = rc.create_texture(page.key, page.width, page.height, &page.pixels);
        }

        let mut batch = SpriteBatch::new();
        if state.map_back_image.is_some() || !state.map_tile_images.is_empty() || !state.map_obj_images.is_empty() {
            // z-order 정렬: 모든 레이어를 (z, key, pos, wh) 로 수집한 뒤 z 오름차순으로 push.
            // wz-render 는 깊이 테스트 없이 painter's algorithm 이므로 push 순서가 z 순서다.
            struct Layer { z: i32, key: GpuKey, x: f32, y: f32, w: f32, h: f32 }
            let mut layers: Vec<Layer> = Vec::new();
            // back (non-front): z = -1000
            if state.map_back_image.is_some() {
                layers.push(Layer { z: -1000, key: GpuKey(2), x: 0.0, y: 0.0, w: 0.0, h: 0.0 });
            }
            // tiles: z = -500 (TileItem 에 z 없음, 기본)
            for (i, (tx, ty, tw, th, _)) in state.map_tile_images.iter().enumerate() {
                layers.push(Layer { z: -500, key: GpuKey(100 + i as u32), x: *tx as f32, y: *ty as f32, w: *tw as f32, h: *th as f32 });
            }
            // objects: z = ObjItem.z (current_map 에서 조회)
            for (i, (ox, oy, ow, oh, _)) in state.map_obj_images.iter().enumerate() {
                let z = state.current_map.as_ref()
                    .and_then(|m| m.objects.get(i))
                    .map(|o| o.z)
                    .unwrap_or(0);
                layers.push(Layer { z, key: GpuKey(200 + i as u32), x: *ox as f32, y: *oy as f32, w: *ow as f32, h: *oh as f32 });
            }
            // back (front): z = +1000 — front 플래그는 map_back_image 에 없으므로 생략.

            // 텍스처 업로드
            if let Some((bw, bh, bpx)) = &state.map_back_image {
                let _ = rc.create_texture(GpuKey(2), *bw, *bh, bpx);
            }
            for (i, (_, _, tw, th, tpx)) in state.map_tile_images.iter().enumerate() {
                let _ = rc.create_texture(GpuKey(100 + i as u32), *tw, *th, tpx);
            }
            for (i, (_, _, ow, oh, opx)) in state.map_obj_images.iter().enumerate() {
                let _ = rc.create_texture(GpuKey(200 + i as u32), *ow, *oh, opx);
            }

            // 모든 레이어를 z 순서로 정렬해 push (painter's algorithm).
            // back 은 z=-1000 으로 layers 에 이미 들어 있다.
            let mut sorted: Vec<&Layer> = layers.iter().collect();
            sorted.sort_by_key(|l| l.z);
            for l in sorted {
                if l.key.0 == 2 {
                    if let Some((bw, bh, _)) = &state.map_back_image {
                        batch.push_quad(l.key, [0.0, 0.0, *bw as f32, *bh as f32], l.z as f32,
                            [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
                    }
                } else {
                    batch.push_quad(l.key, [l.x, l.y, l.w, l.h], l.z as f32,
                        [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
                }
            }
        } else {
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
                    batch.push_quad(atlas_key, [x, y, 36.0, 36.0], 0.0,
                        slot.rect.uv(atlas_w, atlas_h), color);
                }
            }
        }
        batch.finish();

        let uniforms = state.camera.build_uniforms(0.0, 0.0);
        if let Ok(rgba) = rc.render_to_rgba(w, h, &batch, &uniforms, [0.08, 0.08, 0.12, 1.0]) {
            let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            state.viewport_texture = Some(ui.ctx().load_texture("viewport", ci, egui::TextureOptions::LINEAR));
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
            let s1 = state.camera.world_to_screen(glam::Vec2::new(fh.x1 as f32, fh.y1 as f32));
            let s2 = state.camera.world_to_screen(glam::Vec2::new(fh.x2 as f32, fh.y2 as f32));
            let p1 = egui::pos2(rect.min.x + s1.x, rect.min.y + s1.y);
            let p2 = egui::pos2(rect.min.x + s2.x, rect.min.y + s2.y);
            painter.line_segment([p1, p2], egui::Stroke::new(1.5, egui::Color32::GREEN));
        }
        ui.label(format!("발판 {}개, 타일 {}개, 오브젝트 {}개 표시", m.footholds.len(), state.map_tile_images.len(), state.map_obj_images.len()));
    }

    ui.label(format!("카메라: pos=({:.1},{:.1}) zoom={:.2}  (드래그=팬, 휠=줌)",
        state.camera.center.x, state.camera.center.y, state.camera.zoom));
}
