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
