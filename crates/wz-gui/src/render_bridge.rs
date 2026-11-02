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
        if state.map_back_image.is_some() || !state.map_tile_images.is_empty() {
            if let Some((bw, bh, bpx)) = &state.map_back_image {
                let back_key = GpuKey(2);
                let _ = rc.create_texture(back_key, *bw, *bh, bpx);
                batch.push_quad(back_key, [0.0, 0.0, *bw as f32, *bh as f32], 0.0,
                    [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
            }
            for (i, (tx, ty, tw, th, tpx)) in state.map_tile_images.iter().enumerate() {
                let key = GpuKey(100 + i as u32);
                let _ = rc.create_texture(key, *tw, *th, tpx);
                batch.push_quad(key, [*tx as f32, *ty as f32, *tw as f32, *th as f32], 0.0,
                    [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
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
        ui.label(format!("발판 {}개, 타일 {}개 표시", m.footholds.len(), state.map_tile_images.len()));
    }

    ui.label(format!("카메라: pos=({:.1},{:.1}) zoom={:.2}  (드래그=팬, 휠=줌)",
        state.camera.center.x, state.camera.center.y, state.camera.zoom));
}
