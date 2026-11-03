use crate::async_io;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("인스펙터");
    if let Some(sel) = state.selected.clone() {
        ui.label(format!("선택: {sel}"));
        if let Some(tree) = &state.tree {
            let name = sel.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
            if let Some(e) = tree.root.entries.iter().find(|e| e.name.as_deref() == Some(name)) {
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
            if let Some(e) = tree.root.entries.iter().find(|e| e.name.as_deref() == Some(name)) {
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
            if let Some(e) = tree.root.entries.iter().find(|e| e.name.as_deref() == Some(name)) {
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
        ui.label(format!("town/swim/fly: {} / {} / {}", m.info.town, m.info.swim, m.info.fly));
        if let Some(vr) = m.info.vrect {
            ui.label(format!("vrect: {:?}", vr));
        }
        ui.label(format!("footholds: {}, portals: {}, life: {}, bg: {}, obj: {}, tile: {}",
            m.footholds.len(), m.portals.len(), m.life.len(),
            m.backgrounds.len(), m.objects.len(), m.tiles.len()));
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
            if let Some(e) = tree.root.entries.iter().find(|e| e.name.as_deref() == Some(name)) {
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
    show_avatar(ui, state);
}

fn show_avatar(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label("29개 슬롯 (드롭다운 선택)");
    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
        for (i, slot) in AVATAR_SLOTS.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{:>2}. {:<15}", i, slot));
                if i < 29 {
                    let mut sel = state.avatar_slot_selection[i];
                    let preview = if sel == usize::MAX {
                        "없음".to_string()
                    } else if let Some((name, _, _)) = state.avatar_loaded_parts.get(sel) {
                        format!("{} ({})", name, sel)
                    } else {
                        format!("Part {}", sel)
                    };
                    egui::ComboBox::from_id_salt(format!("slot_{}", i))
                        .selected_text(preview)
                        .show_ui(ui, |ui| {
                            if ui.selectable_value(&mut sel, usize::MAX, "없음").changed() {
                                state.avatar_slot_selection[i] = sel;
                            }
                            for j in 0..state.avatar_loaded_parts.len() {
                                let label = if let Some((name, _, _)) = state.avatar_loaded_parts.get(j) {
                                    format!("{} ({})", name, j)
                                } else {
                                    format!("Part {}", j)
                                };
                                if ui.selectable_value(&mut sel, j, label).changed() {
                                    state.avatar_slot_selection[i] = sel;
                                }
                            }
                        });
                    state.avatar_slot_selection[i] = sel;
                } else {
                    ui.label(egui::RichText::new("(empty)").weak());
                }
            });
        }
    });
    if ui.button("Body 이미지 로드 (Character.wz)").clicked() {
        async_io::load_avatar_body(state);
    }
    if ui.button("아바타 로드 (loader, 14 파트)").clicked() {
        async_io::load_avatar_from_loader(state);
    }
    ui.horizontal(|ui| {
        ui.label("장비:");
        ui.text_edit_singleline(&mut state.avatar_equipment_input);
        if ui.button("장비 로드").clicked() {
            async_io::load_avatar_with_equipment_input(state);
        }
    });
    ui.label("형식: Cap:1000000,Coat:1040000,Pants:1060000");
    // 로드된 파트 목록
    if !state.avatar_loaded_parts.is_empty() {
        ui.separator();
        ui.label(format!("로드된 파트 ({}):", state.avatar_loaded_parts.len()));
        for (i, (slot, w, h)) in state.avatar_loaded_parts.iter().enumerate() {
            ui.label(format!("  {}: {} ({}x{})", i, slot, w, h));
        }
    }
    // 로드된 파트 목록 표시
    if state.avatar_rendered.is_some() || state.avatar_image.is_some() {
        ui.label(format!("렌더링된 파트: {}", state.avatar_status));
    }
    if ui.button("아바타 렌더 (오프스크린, 선택 파트)").clicked() {
        // 드롭다운 선택 → avatar_selected_slots 동기화
        state.avatar_selected_slots.clear();
        for i in 0..29usize {
            let sel = state.avatar_slot_selection[i];
            if sel != usize::MAX {
                state.avatar_selected_slots.insert(sel);
            }
        }
        async_io::load_avatar_from_loader(state);
    }
    ui.label(&state.avatar_status);
    if let Some((w, h, rgba)) = &state.avatar_image {
        let ci = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
        let tex = ui.ctx().load_texture("avatar_body", ci, egui::TextureOptions::LINEAR);
        ui.add(egui::Image::new(&tex).max_width(120.0));
        ui.label(format!("Body: {}x{}", w, h));
    }
    if let Some((w, h, rgba)) = &state.avatar_rendered {
        let ci = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
        let tex = ui.ctx().load_texture("avatar_rendered", ci, egui::TextureOptions::LINEAR);
        ui.add(egui::Image::new(&tex).max_width(200.0));
        ui.label(format!("Rendered: {}x{}", w, h));
    }
}


const AVATAR_SLOTS: [&str; 29] = [
    "Body", "Head", "Face", "Hair", "Cap", "Coat", "Longcoat", "Pants", "Shoes",
    "Glove", "SubWeapon", "Cape", "Weapon", "Earrings", "FaceAccessory",
    "EyeAccessory", "Taming", "Saddle", "Chair", "Effect", "Pendant", "Belt",
    "ShoulderPad", "Pocket", "Emblem", "Ring1", "Ring2", "Ring3", "Ring4",
];
