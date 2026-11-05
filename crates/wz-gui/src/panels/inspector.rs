use crate::async_io;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("인스펙터");
    if let Some(sel) = state.selected.clone() {
        ui.label(format!("선택: {sel}"));
        if let Some(tree) = &state.tree {
            let name = sel.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
            if let Some(e) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some(name))
            {
                ui.label(format!("타입: {}", e.node_type));
                ui.label(format!("data_len: {}", e.data_len));
                ui.label(format!("cs32: {}", e.cs32));
                ui.label(format!("file_pos: {}", e.file_pos));
                ui.label(format!("offset: {:?}", e.offset));
            }
        }
    } else {
        ui.label("노드를 선택하세요.");
    }

    ui.separator();
    ui.heading("이미지 뷰어");
    if let Some(path) = state.current_image_path.clone() {
        ui.label(format!("이미지: {path}"));
        if let Some(info) = &state.current_image_info {
            ui.label(info);
        }
    } else {
        ui.label("이미지 노드를 선택하세요.");
    }

    // 선택된 노드가 gear item 이면 스탯 표시
    if let Some(sel) = state.selected.clone() {
        if let Some(tree) = &state.tree {
            let name = sel.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
            if let Some(e) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some(name))
            {
                if e.node_type == 4 {
                    // 이미지 엔트리 — parse + gear props
                    if let Some(arch) = state.shared_archive.lock().unwrap().clone() {
                        if let Some(off) = e.offset {
                            if let Ok(node) = arch.parse_image_at(off) {
                                let props = wz_db::extract_gear_props(&node);
                                if !props.is_empty() {
                                    ui.separator();
                                    ui.heading("스탯");
                                    for (k, v) in &props {
                                        ui.label(format!("{}: {}", k, v));
                                    }
                                    let sel_key = sel.clone();
                                    let tooltip_id =
                                        name.trim_end_matches(".img").parse::<i32>().ok();
                                    if state.tooltip_for.as_deref() != Some(sel_key.as_str()) {
                                        state.tooltip_for = Some(sel_key);
                                        let title = state
                                            .string_linker
                                            .as_ref()
                                            .and_then(|linker| {
                                                tooltip_id.and_then(|id| linker.get_by_id(id))
                                            })
                                            .map(|sr| sr.name.clone())
                                            .unwrap_or_else(|| name.to_string());
                                        let lines: Vec<String> = props
                                            .iter()
                                            .map(|(k, v)| format!("{k}: {v}"))
                                            .collect();
                                        let icon = node.first_canvas().and_then(|png| {
                                            wz_image::decode_png_ref_to_rgba(&png, arch.source())
                                                .ok()
                                        });
                                        let data_root = state
                                            .data_root
                                            .clone()
                                            .or_else(|| async_io::canvas::find_data_root(None));
                                        state.tooltip_bitmap =
                                            data_root.as_deref().and_then(|root| {
                                                wz_avatar::render_tooltip_bitmap(
                                                    root, &title, &lines, icon,
                                                )
                                            });
                                        state.tooltip_texture = None;
                                    }
                                    if let Some((w, h, rgba)) = &state.tooltip_bitmap {
                                        if state.tooltip_texture.is_none() {
                                            let image = egui::ColorImage::from_rgba_unmultiplied(
                                                [*w as usize, *h as usize],
                                                rgba,
                                            );
                                            state.tooltip_texture = Some(ui.ctx().load_texture(
                                                "item_tooltip",
                                                image,
                                                egui::TextureOptions::NEAREST,
                                            ));
                                        }
                                    }
                                    if let Some(texture) = &state.tooltip_texture {
                                        ui.separator();
                                        ui.label("비트맵 툴팁");
                                        ui.image((texture.id(), texture.size_vec2()));
                                    }

                                    // StringLinker 툴팁 자동 표시
                                    let id_str = name.trim_end_matches(".img");
                                    if let Ok(id) = id_str.parse::<i32>() {
                                        if let Some(linker) = &state.string_linker {
                                            if let Some(tooltip) = linker.format_tooltip(id) {
                                                ui.separator();
                                                ui.label("아이템 정보:");
                                                for line in tooltip.lines() {
                                                    ui.label(line);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // StringLinker 정보 표시 (gear/item/skill/map/mob/npc)
    if let Some(sel) = state.selected.clone() {
        if let Some(tree) = &state.tree {
            let name = sel.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
            if let Some(e) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some(name))
            {
                if e.node_type == 4 {
                    let id_str = name.trim_end_matches(".img");
                    // 숫자 ID 시도
                    if let Ok(id) = id_str.parse::<i32>() {
                        if let Some(linker) = &state.string_linker {
                            if let Some(sr) = linker.get_by_id(id) {
                                ui.separator();
                                ui.heading("StringLinker");
                                if !sr.name.is_empty() {
                                    ui.label(format!("이름: {}", sr.name));
                                }
                                if let Some(desc) = &sr.desc {
                                    if !desc.is_empty() {
                                        ui.label(format!("설명: {}", desc));
                                    }
                                }
                                if let Some(pdesc) = &sr.pdesc {
                                    if !pdesc.is_empty() {
                                        ui.label(format!("pdesc: {}", pdesc));
                                    }
                                }
                                if !sr.skill_h.is_empty() {
                                    ui.label("스킬 H:");
                                    for h in &sr.skill_h {
                                        ui.label(format!("  {}", h));
                                    }
                                }
                            }
                        }
                    }
                    // 문자열 키 시도 (skill ID like "0000012")
                    if let Some(linker) = &state.string_linker {
                        if let Some(sr) = linker.string_skill_str.get(id_str) {
                            ui.separator();
                            ui.heading("StringLinker (str key)");
                            if !sr.name.is_empty() {
                                ui.label(format!("이름: {}", sr.name));
                            }
                            if let Some(desc) = &sr.desc {
                                if !desc.is_empty() {
                                    ui.label(format!("설명: {}", desc));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    ui.separator();
    ui.heading("사운드");
    ui.horizontal(|ui| {
        if ui.button("맵 BGM 재생").clicked() {
            match crate::sound_player::play_map_bgm(state) {
                Ok(()) => {}
                Err(e) => state.sound_status = format!("BGM 오류: {e}"),
            }
        }
        if ui.button("정지").clicked() {
            crate::sound_player::stop(state);
        }
    });
    ui.horizontal(|ui| {
        ui.label("Sound 경로:");
        ui.text_edit_singleline(&mut state.sound_path);
        if ui.button("재생").clicked() {
            let Some(wz) = state.archive_path.clone() else {
                state.sound_status = "WZ 파일을 먼저 여세요".into();
                return;
            };
            let path = state.sound_path.clone();
            match crate::sound_player::play_from_archive(state, &wz, &path) {
                Ok(()) => {}
                Err(e) => state.sound_status = format!("재생 오류: {e}"),
            }
        }
    });
    ui.label(&state.sound_status);
    ui.separator();
    ui.heading("맵 (PoC 2.5)");
    ui.horizontal(|ui| {
        ui.label("맵 경로:");
        ui.text_edit_singleline(&mut state.map_path);
        if ui.button("Load Map").clicked() {
            async_io::load_map_by_path(state);
        }
    });
    if ui.button("선택 노드를 맵으로 로드").clicked() {
        async_io::load_map_from_selection(state);
    }
    ui.label(&state.map_status);
    if let Some(m) = &state.current_map {
        ui.label(format!("bgm: {:?}", m.info.bgm));
        ui.label(format!(
            "town/swim/fly: {} / {} / {}",
            m.info.town, m.info.swim, m.info.fly
        ));
        if let Some(vr) = m.info.vrect {
            ui.label(format!("vrect: {:?}", vr));
        }
        ui.label(format!(
            "footholds: {}, portals: {}, life: {}, bg: {}, obj: {}, tile: {}",
            m.footholds.len(),
            m.portals.len(),
            m.life.len(),
            m.backgrounds.len(),
            m.objects.len(),
            m.tiles.len()
        ));
        ui.label(format!("ladders/ropes: {}", m.ladders_ropes.len()));
        if let Some(info) = &m.mini_map {
            ui.label(format!(
                "minimap: {}x{} center=({},{}) mag={}",
                info.width, info.height, info.center_x, info.center_y, info.mag
            ));
        }
    }

    // 미니맵 지역명/맵 이름 (StringLinker)
    if let (Some(id), Some(linker)) = (
        state.current_map.as_ref().and_then(|m| m.info.map_id),
        state.string_linker.as_ref(),
    ) {
        if let Some((street, name)) = linker.map_display_names(id) {
            if !street.is_empty() {
                ui.label(
                    egui::RichText::new(street)
                        .small()
                        .color(egui::Color32::GRAY),
                );
            }
            if !name.is_empty() {
                ui.heading(name);
            }
        }
    }

    // 미니맵 캔버스 + 현재 카메라 위치 표시
    if state.map_minimap_texture.is_none() {
        if let Some((w, h, rgba, _)) = &state.map_minimap {
            let image = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
            state.map_minimap_texture = Some(ui.ctx().load_texture(
                "map_minimap",
                image,
                egui::TextureOptions::NEAREST,
            ));
        }
    }
    if let (Some(texture), Some((_, _, _, info))) = (&state.map_minimap_texture, &state.map_minimap)
    {
        let max_w = 240.0f32;
        let tw = texture.size_vec2().x.max(1.0);
        let th = texture.size_vec2().y.max(1.0);
        let scale = (max_w / tw).min(1.0);
        let response =
            ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(tw * scale, th * scale)));
        if info.width > 0 && info.height > 0 {
            let rect = response.rect;
            let cx = (state.camera.center.x + info.center_x as f32) / info.width as f32;
            let cy = (state.camera.center.y + info.center_y as f32) / info.height as f32;
            if (0.0..=1.0).contains(&cx) && (0.0..=1.0).contains(&cy) {
                let pos = egui::pos2(
                    rect.min.x + cx * rect.width(),
                    rect.min.y + cy * rect.height(),
                );
                ui.painter()
                    .circle_stroke(pos, 4.0, egui::Stroke::new(1.5, egui::Color32::YELLOW));
            }
        }

        if state.map_minimap_icon_textures.is_none() {
            if let Some(icons) = &state.map_minimap_icons {
                let mut textures = crate::state::MinimapIconTextures::default();
                let make = |raw: &Option<(u32, u32, Vec<u8>)>,
                            slot: &mut Option<egui::TextureHandle>| {
                    if let Some((w, h, rgba)) = raw {
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [*w as usize, *h as usize],
                            rgba,
                        );
                        *slot = Some(ui.ctx().load_texture(
                            "minimap_icon",
                            image,
                            egui::TextureOptions::NEAREST,
                        ));
                    }
                };
                make(&icons.portal, &mut textures.portal);
                make(&icons.npc, &mut textures.npc);
                make(&icons.mob, &mut textures.mob);
                make(&icons.hidden_portal, &mut textures.hidden_portal);
                state.map_minimap_icon_textures = Some(textures);
            }
        }
        if let (Some(textures), Some(m)) = (&state.map_minimap_icon_textures, &state.current_map) {
            let rect = response.rect;
            for portal in &m.portals {
                let icon = match portal.p_type {
                    2 | 7 | 10 => textures.portal.as_ref(),
                    8 | 11 if portal.shown_at_minimap => textures.hidden_portal.as_ref(),
                    _ => None,
                };
                if let Some(icon) = icon {
                    draw_minimap_icon(ui, rect, icon, portal.x, portal.y, info);
                }
            }
            for life in &state.map_lifes {
                if life.hidden {
                    continue;
                }
                let icon = if life.npc {
                    textures.npc.as_ref()
                } else {
                    textures.mob.as_ref()
                };
                if let Some(icon) = icon {
                    draw_minimap_icon(
                        ui,
                        rect,
                        icon,
                        life.position_x as i32,
                        life.position_y as i32,
                        info,
                    );
                }
            }
        }
    }

    ui.separator();
    ui.heading("월드맵");
    ui.horizontal(|ui| {
        if ui.button("목록 새로고침").clicked() {
            if let Some(root) = state.data_root.clone() {
                state.worldmap_list = crate::worldmap::list_worldmaps(&root);
                if state.worldmap_selected.is_empty() {
                    state.worldmap_selected =
                        state.worldmap_list.first().cloned().unwrap_or_default();
                }
            }
        }
        egui::ComboBox::from_id_salt("worldmap_select")
            .selected_text(&state.worldmap_selected)
            .show_ui(ui, |ui| {
                let list = state.worldmap_list.clone();
                for name in list {
                    ui.selectable_value(&mut state.worldmap_selected, name.clone(), name);
                }
            });
        if ui.button("불러오기").clicked() {
            if let Some(root) = state.data_root.clone() {
                state.worldmap_view =
                    async_io::map_loader::load_worldmap_view(&root, &state.worldmap_selected);
                state.worldmap_texture = None;
            }
        }
    });
    if state.worldmap_texture.is_none() {
        if let Some(view) = &state.worldmap_view {
            if let Some((w, h, rgba)) = &view.base {
                let image =
                    egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
                state.worldmap_texture = Some(ui.ctx().load_texture(
                    "worldmap",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
    }
    if let (Some(texture), Some(view)) = (&state.worldmap_texture, &state.worldmap_view) {
        ui.label(format!("{} ({})", view.display_name, view.name));
        let max_w = 360.0f32;
        let tw = texture.size_vec2().x.max(1.0);
        let th = texture.size_vec2().y.max(1.0);
        let scale = (max_w / tw).min(1.0);
        let response =
            ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(tw * scale, th * scale)));
        let rect = response.rect;
        let painter = ui.painter();
        for spot in &view.spots {
            let sx = rect.width() * 0.5 + (spot.x - view.base_spot.0 as f32) * scale;
            let sy = rect.height() * 0.5 + (spot.y - view.base_spot.1 as f32) * scale;
            let pos = egui::pos2(rect.min.x + sx, rect.min.y + sy);
            let color = if spot.spot_type == 0 {
                egui::Color32::from_rgb(255, 80, 80)
            } else {
                egui::Color32::from_rgb(255, 190, 80)
            };
            painter.circle_filled(pos, 3.0, color);
            let title = spot
                .title
                .clone()
                .or_else(|| spot.desc.clone())
                .or_else(|| spot.map_nos.first().map(|id| id.to_string()));
            if let Some(title) = &title {
                painter.text(
                    pos + egui::vec2(5.0, -5.0),
                    egui::Align2::LEFT_BOTTOM,
                    title,
                    egui::FontId::proportional(10.0),
                    egui::Color32::WHITE,
                );
            }
        }
    }

    ui.separator();
    ui.heading("스킬 미리보기");
    ui.horizontal(|ui| {
        ui.label("Skill ID:");
        ui.text_edit_singleline(&mut state.skill_id_input);
        ui.label("Ani:");
        ui.text_edit_singleline(&mut state.skill_ani_input);
    });
    ui.horizontal(|ui| {
        ui.label("x:");
        ui.add(egui::DragValue::new(&mut state.skill_x));
        ui.label("y:");
        ui.add(egui::DragValue::new(&mut state.skill_y));
        ui.checkbox(&mut state.skill_flip, "flip");
        if ui.button("소환").clicked() {
            let Some(root) = state.data_root.clone() else {
                state.sound_status = "Data 루트 없음".into();
                return;
            };
            let id = state.skill_id_input.trim().parse::<i32>().unwrap_or(0);
            let ani = state.skill_ani_input.trim().to_string();
            if let Some(mut skill) = async_io::map_loader::load_skill_view(&root, id, &ani) {
                skill.x = state.skill_x;
                skill.y = state.skill_y;
                skill.flip = state.skill_flip;
                state.skill_view = Some(skill);
                state.skill_time = 0.0;
                state.skill_last = 0.0;
                state.viewport_dirty = true;
            } else {
                state.sound_status = format!("스킬을 찾지 못했습니다: {id}/{ani}");
            }
        }
    });
    if state.skill_view.is_some() {
        ui.label("뷰포트에 스킬 애니메이션을 표시합니다");
    }

    ui.separator();
    ui.heading("StringLinker (툴팁)");
    ui.label(&state.string_linker_status);
    if ui.button("String.wz 로드").clicked() {
        async_io::load_string_linker(state);
    }
    // 선택된 gear item 의 툴팁 표시
    if let Some(sel) = state.selected.clone() {
        if let Some(tree) = &state.tree {
            let name = sel.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
            if let Some(e) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some(name))
            {
                if e.node_type == 4 {
                    let id_str = name.trim_end_matches(".img");
                    if let Ok(id) = id_str.parse::<i32>() {
                        if let Some(linker) = &state.string_linker {
                            if let Some(tooltip) = linker.format_tooltip(id) {
                                ui.separator();
                                ui.label("툴팁:");
                                for line in tooltip.lines() {
                                    ui.label(line);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    ui.separator();
    ui.heading("아바타 (PoC 3.5)");
    crate::panels::avatar_panel::show(ui, state);
}

fn draw_minimap_icon(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    icon: &egui::TextureHandle,
    world_x: i32,
    world_y: i32,
    info: &wz_db::MiniMapInfo,
) {
    if info.width <= 0 || info.height <= 0 {
        return;
    }
    let x = (world_x as f32 + info.center_x as f32) / info.width as f32;
    let y = (world_y as f32 + info.center_y as f32) / info.height as f32;
    if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
        return;
    }
    let center = egui::pos2(
        rect.min.x + x * rect.width(),
        rect.min.y + y * rect.height(),
    );
    let size = icon.size_vec2();
    let pos = egui::pos2(center.x - size.x * 0.5, center.y - size.y * 0.5);
    ui.painter().image(
        icon.id(),
        egui::Rect::from_min_size(pos, size),
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
}
