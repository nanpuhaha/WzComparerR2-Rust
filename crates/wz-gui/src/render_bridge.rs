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

fn draw_skill(batch: &mut SpriteBatch, state: &AppState) {
    let Some(skill) = &state.skill_view else {
        return;
    };
    let Some(frame) = skill.frame_at(state.skill_time) else {
        return;
    };
    let index = skill
        .frames
        .iter()
        .position(|f| std::ptr::eq(f, frame))
        .unwrap_or(0);
    let x = (skill.x - frame.origin.0) as f32;
    let y = (skill.y - frame.origin.1) as f32;
    let uv = if skill.flip {
        [1.0, 0.0, 0.0, 1.0]
    } else {
        [0.0, 0.0, 1.0, 1.0]
    };
    batch.push_quad(
        GpuKey(50_000 + index as u32),
        [x, y, frame.width as f32, frame.height as f32],
        200.0 + skill.id as f32 * 0.000001,
        uv,
        [1.0, 1.0, 1.0, 1.0],
    );
}

fn portal_key(view: usize, frame: usize) -> GpuKey {
    GpuKey(40_000 + view as u32 * 64 + frame as u32)
}

fn draw_portals(batch: &mut SpriteBatch, state: &AppState) {
    for (i, portal) in state.map_portals.iter().enumerate() {
        if portal.portal_type < 0 {
            continue;
        }
        let Some(frame) = portal.frame_at(state.map_portal_time) else {
            continue;
        };
        let frame_index = portal.animation.frame_at(state.map_portal_time);
        let x = (portal.x - frame.origin.0) as f32;
        let y = (portal.y - frame.origin.1) as f32;
        batch.push_quad(
            portal_key(i, frame_index),
            [x, y, frame.width as f32, frame.height as f32],
            5.0 + portal.id as f32 * 0.000001,
            [0.0, 0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        );
    }
}

fn life_key(view: usize, frame: usize) -> GpuKey {
    GpuKey(30_000 + view as u32 * 64 + frame as u32)
}

fn draw_lifes(batch: &mut SpriteBatch, state: &AppState) {
    for (i, life) in state.map_lifes.iter().enumerate() {
        if life.hidden {
            continue;
        }
        let Some(frame) = life.draw_frame(state.map_life_time) else {
            continue;
        };
        let frame_index = if life.moving && !life.walk.frames.is_empty() {
            life.walk.frame_at(state.map_life_time)
        } else {
            life.animation.frame_at(state.map_life_time)
        };
        let x = life.position_x - frame.origin.0 as f32;
        let y = life.position_y - frame.origin.1 as f32;
        let uv = if life.flip {
            [1.0, 0.0, 0.0, 1.0]
        } else {
            [0.0, 0.0, 1.0, 1.0]
        };
        batch.push_quad(
            life_key(i, frame_index),
            [x, y, frame.width as f32, frame.height as f32],
            10.0 + life.index as f32 * 0.001,
            uv,
            [1.0, 1.0, 1.0, 1.0],
        );
    }
}

fn reactor_key(view: usize, stage: usize, hit: bool, frame: usize) -> GpuKey {
    GpuKey(
        20_000 + view as u32 * 1024 + (if hit { 32 } else { 0 }) + stage as u32 * 64 + frame as u32,
    )
}

fn draw_reactors(batch: &mut SpriteBatch, state: &AppState) {
    for (i, view) in state.map_reactor_images.iter().enumerate() {
        let Some(draw) = view.draw_at(state.map_reactor_time) else {
            continue;
        };
        let animation = draw.animation;
        let frame_index = animation.frame_at(if draw.hit {
            (state.map_reactor_time - view.hit_started).max(0.0) * 1000.0
        } else {
            state.map_reactor_time * 1000.0
        });
        let stage_index = if draw.hit {
            view.hits
                .keys()
                .position(|k| Some(*k) == view.hit_stage)
                .unwrap_or(0)
        } else {
            view.stages
                .keys()
                .position(|k| *k == view.stage)
                .unwrap_or(0)
        };
        let frame = draw.frame;
        let x = (view.x - frame.origin.0) as f32;
        let y = (view.y - frame.origin.1) as f32;
        let uv = if view.flip {
            [1.0, 0.0, 0.0, 1.0]
        } else {
            [0.0, 0.0, 1.0, 1.0]
        };
        batch.push_quad(
            reactor_key(i, stage_index, draw.hit, frame_index),
            [x, y, frame.width as f32, frame.height as f32],
            view.index as f32 * 0.001,
            uv,
            [1.0, 1.0, 1.0, 1.0],
        );
    }
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
    if let Some(skill) = &state.skill_view {
        if skill.frames.len() > 1 {
            let now = ui.input(|i| i.time);
            if state.skill_last > 0.0 {
                state.skill_time += (now - state.skill_last).clamp(0.0, 0.1);
            }
            state.skill_last = now;
            state.viewport_dirty = true;
            ui.ctx().request_repaint();
        }
    }
    let life_animating = state.map_lifes.iter().any(|l| {
        l.state != crate::life::LifeState::Alive
            || l.animation.frames.len() > 1
            || l.walk.frames.len() > 1
            || l.hit.frames.len() > 1
            || l.die.frames.len() > 1
            || (!l.npc && !l.hidden && l.rx1 > l.rx0)
    });
    if life_animating {
        let now = ui.input(|i| i.time);
        let dt = if state.map_life_last > 0.0 {
            (now - state.map_life_last).clamp(0.0, 0.1)
        } else {
            0.0
        };
        state.map_life_time += dt;
        state.map_life_last = now;
        let footholds = state
            .current_map
            .as_ref()
            .map(|m| m.footholds_by_id.clone())
            .unwrap_or_default();
        for life in &mut state.map_lifes {
            life.tick(dt, state.map_life_time, &footholds);
        }
        state.viewport_dirty = true;
        ui.ctx().request_repaint();
    }
    let portal_animating = state
        .map_portals
        .iter()
        .any(|p| p.animation.frames.len() > 1);
    if portal_animating {
        let now = ui.input(|i| i.time);
        if state.map_portal_last > 0.0 {
            state.map_portal_time += (now - state.map_portal_last).clamp(0.0, 0.1);
        }
        state.map_portal_last = now;
        state.viewport_dirty = true;
        ui.ctx().request_repaint();
    }
    let reactor_animating = state.map_reactor_images.iter().any(|v| {
        v.hit_stage.is_some()
            || v.stages.values().any(|a| a.frames.len() > 1)
            || v.hits.values().any(|a| a.frames.len() > 1)
    });
    if reactor_animating {
        let now = ui.input(|i| i.time);
        if state.map_reactor_last > 0.0 {
            state.map_reactor_time += (now - state.map_reactor_last).clamp(0.0, 0.1);
        }
        state.map_reactor_last = now;
        for view in &mut state.map_reactor_images {
            view.tick(state.map_reactor_time);
        }
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

    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let local = glam::Vec2::new(pos.x - resp.rect.min.x, pos.y - resp.rect.min.y);
            let world = state.camera.screen_to_world(local);
            let mut hit = false;
            let mut sound = None;
            // 몹 클릭 피격/사망이 리액터보다 우선한다.
            for life in state.map_lifes.iter_mut().rev() {
                if life.npc || life.hidden || life.state != crate::life::LifeState::Alive {
                    continue;
                }
                let Some(frame) = life.draw_frame(state.map_life_time) else {
                    continue;
                };
                let x = life.position_x - frame.origin.0 as f32;
                let y = life.position_y - frame.origin.1 as f32;
                if world.x >= x
                    && world.x <= x + frame.width as f32
                    && world.y >= y
                    && world.y <= y + frame.height as f32
                {
                    life.hit(state.map_life_time);
                    hit = true;
                    break;
                }
            }
            if !hit {
                for view in state.map_reactor_images.iter_mut().rev() {
                    let Some(draw) = view.draw_at(state.map_reactor_time) else {
                        continue;
                    };
                    let frame = draw.frame;
                    let x = (view.x - frame.origin.0) as f32;
                    let y = (view.y - frame.origin.1) as f32;
                    if world.x >= x
                        && world.x <= x + frame.width as f32
                        && world.y >= y
                        && world.y <= y + frame.height as f32
                    {
                        sound = view.click(state.map_reactor_time);
                        hit = true;
                        break;
                    }
                }
            }
            if hit {
                if let Some((id, stage)) = sound {
                    crate::sound_player::play_reactor_sound(state, id, stage);
                }
                state.viewport_dirty = true;
            }
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
            }
        }

        let mut batch = SpriteBatch::new();
        if !state.viewport_cached_layers.is_empty()
            || state.viewport_cached_has_back
            || !state.map_background_images.is_empty()
            || !state.map_reactor_images.is_empty()
            || !state.map_lifes.is_empty()
            || !state.map_portals.is_empty()
            || state.skill_view.is_some()
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
            for (i, view) in state.map_reactor_images.iter().enumerate() {
                for (si, (_, animation)) in view.stages.iter().enumerate() {
                    for (f, frame) in animation.frames.iter().enumerate() {
                        let _ = rc.create_texture(
                            reactor_key(i, si, false, f),
                            frame.width,
                            frame.height,
                            &frame.rgba,
                        );
                    }
                }
                for (si, (_, animation)) in view.hits.iter().enumerate() {
                    for (f, frame) in animation.frames.iter().enumerate() {
                        let _ = rc.create_texture(
                            reactor_key(i, si, true, f),
                            frame.width,
                            frame.height,
                            &frame.rgba,
                        );
                    }
                }
            }
            for (i, layer) in state.map_background_images.iter().enumerate() {
                for (f, (fw, fh, fpx)) in layer.frames.iter().enumerate() {
                    let _ =
                        rc.create_texture(GpuKey(10_000 + i as u32 * 64 + f as u32), *fw, *fh, fpx);
                }
            }
            for (i, life) in state.map_lifes.iter().enumerate() {
                for (f, frame) in life.animation.frames.iter().enumerate() {
                    let _ =
                        rc.create_texture(life_key(i, f), frame.width, frame.height, &frame.rgba);
                }
            }
            for (i, portal) in state.map_portals.iter().enumerate() {
                for (f, frame) in portal.animation.frames.iter().enumerate() {
                    let _ =
                        rc.create_texture(portal_key(i, f), frame.width, frame.height, &frame.rgba);
                }
            }
            if let Some(skill) = &state.skill_view {
                for (f, frame) in skill.frames.iter().enumerate() {
                    let _ = rc.create_texture(
                        GpuKey(50_000 + f as u32),
                        frame.width,
                        frame.height,
                        &frame.rgba,
                    );
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
        draw_lifes(&mut batch, state);
        draw_portals(&mut batch, state);
        draw_skill(&mut batch, state);
        draw_reactors(&mut batch, state);
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

    // 라이프 이름 (원본과 같이 NPC/몹 머리 위에 표시)
    if let Some(tex) = &state.viewport_texture {
        let painter = ui.painter();
        let rect = resp.rect;
        for life in &state.map_lifes {
            if life.hidden || life.hide_name {
                continue;
            }
            let Some(name) = &life.name else {
                continue;
            };
            if let Some(frame) = life.draw_frame(state.map_life_time) {
                let top = glam::Vec2::new(life.x as f32, (life.y - frame.height as i32) as f32);
                let screen = state.camera.world_to_screen(top);
                let pos = egui::pos2(rect.min.x + screen.x, rect.min.y + screen.y);
                painter.text(
                    pos,
                    egui::Align2::CENTER_BOTTOM,
                    name,
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgb(255, 235, 120),
                );
            }
        }
        let _ = tex;
    }

    // 맵 오브젝트 툴팁 (ToolTip/<n> + String/ToolTipHelp.img)
    if resp.hovered() {
        if let Some(pointer) = resp.hover_pos() {
            let local = glam::Vec2::new(pointer.x - resp.rect.min.x, pointer.y - resp.rect.min.y);
            let world = state.camera.screen_to_world(local);
            for tip in &state.map_tooltips {
                if world.x >= tip.x1 as f32
                    && world.x <= tip.x2 as f32
                    && world.y >= tip.y1 as f32
                    && world.y <= tip.y2 as f32
                {
                    if let Some(title) = &tip.title {
                        let text = match &tip.desc {
                            Some(desc) if !desc.is_empty() => format!("{title}\n{desc}"),
                            _ => title.clone(),
                        };
                        let painter = ui.painter();
                        painter.rect_filled(
                            egui::Rect::from_min_size(
                                pointer + egui::vec2(12.0, 12.0),
                                egui::vec2(240.0, 34.0),
                            ),
                            4.0,
                            egui::Color32::from_black_alpha(220),
                        );
                        painter.text(
                            pointer + egui::vec2(18.0, 16.0),
                            egui::Align2::LEFT_TOP,
                            text,
                            egui::FontId::proportional(12.0),
                            egui::Color32::WHITE,
                        );
                    }
                    break;
                }
            }
        }
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

    // 스카이웨일/조명 클러스터 경로 (원본 편집 오버레이)
    if let Some(m) = &state.current_map {
        let painter = ui.painter();
        let rect = resp.rect;
        let to_screen = |x: f32, y: f32| {
            let s = state.camera.world_to_screen(glam::Vec2::new(x, y));
            egui::pos2(rect.min.x + s.x, rect.min.y + s.y)
        };
        for whale in &m.sky_whales {
            for dx in [0, -whale.width / 2, whale.width / 2] {
                let a = to_screen((whale.start.0 + dx) as f32, whale.start.1 as f32);
                let b = to_screen((whale.end.0 + dx) as f32, whale.end.1 as f32);
                painter.line_segment([a, b], egui::Stroke::new(1.0, egui::Color32::CYAN));
                painter.text(
                    b,
                    egui::Align2::LEFT_BOTTOM,
                    &whale.name,
                    egui::FontId::proportional(9.0),
                    egui::Color32::CYAN,
                );
            }
        }
        for cluster in &m.illuminant_clusters {
            for dx in [0, -cluster.size / 2, cluster.size / 2] {
                let a = to_screen((cluster.start.0 + dx) as f32, cluster.start.1 as f32);
                let b = to_screen((cluster.end.0 + dx) as f32, cluster.end.1 as f32);
                painter.line_segment(
                    [a, b],
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 220, 120)),
                );
            }
            let a = to_screen(cluster.start.0 as f32, cluster.start.1 as f32);
            let b = to_screen(cluster.end.0 as f32, cluster.end.1 as f32);
            painter.circle_filled(a, 3.0, egui::Color32::YELLOW);
            painter.circle_filled(b, 3.0, egui::Color32::YELLOW);
        }
    }

    ui.label(format!(
        "카메라: pos=({:.1},{:.1}) zoom={:.2}  (드래그=팬, 휠=줌)",
        state.camera.center.x, state.camera.center.y, state.camera.zoom
    ));
}
