use crate::async_io;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("Catalog (Item/Skill/Character)");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Character body skin:");
        let mut skin = state.avatar_body_skin;
        egui::ComboBox::from_id_salt("char_skin")
            .selected_text(format!("{}", skin))
            .show_ui(ui, |ui| {
                for s in [2000i32, 2001, 2002, 2003, 2004, 2005] {
                    if ui
                        .selectable_value(&mut skin, s, format!("{}", s))
                        .changed()
                    {
                        state.avatar_body_skin = skin;
                    }
                }
            });
        state.avatar_body_skin = skin;
        if ui.button("Apply").clicked() {
            async_io::load_avatar_from_loader(state);
        }
    });

    ui.separator();
    if let Some(linker) = &state.string_linker {
        ui.label(format!("Items: {}", linker.string_item.len()));
        egui::ScrollArea::vertical()
            .max_height(120.0)
            .show(ui, |ui| {
                for (id, sr) in &linker.string_item {
                    ui.label(format!("{} - {}", id, sr.name));
                }
            });
        ui.separator();
        ui.label(format!("Skills: {}", linker.string_skill.len()));
        egui::ScrollArea::vertical()
            .max_height(120.0)
            .show(ui, |ui| {
                for (id, sr) in &linker.string_skill {
                    ui.label(format!("{} - {}", id, sr.name));
                }
            });
    } else {
        ui.label("StringLinker not loaded.");
    }

    ui.separator();
    ui.heading("DB2 데이터 뷰어");
    ui.horizontal(|ui| {
        ui.label("filter:");
        ui.text_edit_singleline(&mut state.db2_filter);
        if ui.button("선택 이미지 로드").clicked() {
            let Some(sel) = state.selected.clone() else {
                state.db2_status = "선택된 노드 없음".into();
                return;
            };
            let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
                state.db2_status = "아카이브 없음".into();
                return;
            };
            let name = sel.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
            let image_name = if name.ends_with(".img") {
                name.to_string()
            } else {
                format!("{name}.img")
            };
            let Some(tree) = &state.tree else {
                state.db2_status = "트리 없음".into();
                return;
            };
            let Some(entry) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some(image_name.as_str()))
            else {
                state.db2_status = format!("엔트리 없음: {image_name}");
                return;
            };
            let Some(offset) = entry.offset else {
                state.db2_status = "오프셋 없음".into();
                return;
            };
            match archive.parse_image_at(offset) {
                Ok(mut node) => {
                    node.name = std::sync::Arc::from(image_name.as_str());
                    state.db2_rows = wz_db::db2_image_rows(&node);
                    state.db2_status = format!("{} rows", state.db2_rows.len());
                }
                Err(e) => state.db2_status = format!("파싱 실패: {e}"),
            }
        }
    });
    ui.horizontal(|ui| {
        if ui.button("선택 이미지 아이콘 보기").clicked() {
            let Some(sel) = state.selected.clone() else {
                state.db2_status = "선택된 노드 없음".into();
                return;
            };
            let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
                state.db2_status = "아카이브 없음".into();
                return;
            };
            let name = sel.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
            let image_name = if name.ends_with(".img") {
                name.to_string()
            } else {
                format!("{name}.img")
            };
            let Some(tree) = &state.tree else {
                state.db2_status = "트리 없음".into();
                return;
            };
            let Some(entry) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some(image_name.as_str()))
            else {
                state.db2_status = format!("엔트리 없음: {image_name}");
                return;
            };
            let Some(offset) = entry.offset else {
                state.db2_status = "오프셋 없음".into();
                return;
            };
            if let Ok(mut node) = archive.parse_image_at(offset) {
                node.name = std::sync::Arc::from(image_name.as_str());
                let mut icons = Vec::new();
                fn walk(
                    node: &wz_core::image::Node,
                    parent: &str,
                    source: &std::sync::Arc<dyn wz_pool::ByteSource>,
                    icons: &mut Vec<(String, u32, u32, Vec<u8>)>,
                ) {
                    use wz_core::image::ImageValue;
                    let path = if parent.is_empty() {
                        node.name.to_string()
                    } else {
                        format!("{parent}/{}", node.name)
                    };
                    if let Some(ImageValue::Png(png)) = &node.value {
                        if let Ok((w, h, rgba)) =
                            wz_image::decode_png_ref_to_rgba(png, source)
                        {
                            icons.push((path.clone(), w, h, rgba));
                        }
                    }
                    for child in node.object_children() {
                        walk(child, &path, source, icons);
                    }
                }
                walk(&node, "", archive.source(), &mut icons);
                state.db2_icons = icons
                    .into_iter()
                    .map(|(path, w, h, rgba)| {
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [w as usize, h as usize],
                            &rgba,
                        );
                        (
                            path,
                            ui.ctx().load_texture(
                                "db2_icon",
                                image,
                                egui::TextureOptions::NEAREST,
                            ),
                        )
                    })
                    .collect();
                state.db2_status = format!("{} icons", state.db2_icons.len());
            }
        }
    });
    ui.label(&state.db2_status);
    ui.horizontal_wrapped(|ui| {
        for (path, texture) in &state.db2_icons {
            let response = ui.add(
                egui::Image::new(texture)
                    .max_size(egui::vec2(64.0, 64.0))
                    .sense(egui::Sense::hover()),
            );
            response.on_hover_text(path);
        }
    });
    ui.horizontal(|ui| {
        if ui.button("선택 이미지 DB2 Item 그리드").clicked() {
            let Some(linker) = state.string_linker.as_ref() else {
                state.db2_status = "StringLinker를 먼저 로드하세요".into();
                return;
            };
            let Some(sel) = state.selected.clone() else {
                state.db2_status = "선택된 노드 없음".into();
                return;
            };
            let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
                state.db2_status = "아카이브 없음".into();
                return;
            };
            let name = sel.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
            let image_name = if name.ends_with(".img") {
                name.to_string()
            } else {
                format!("{name}.img")
            };
            let Some(tree) = &state.tree else {
                state.db2_status = "트리 없음".into();
                return;
            };
            let Some(entry) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some(image_name.as_str()))
            else {
                state.db2_status = format!("엔트리 없음: {image_name}");
                return;
            };
            let Some(offset) = entry.offset else {
                state.db2_status = "오프셋 없음".into();
                return;
            };
            if let Ok(mut node) = archive.parse_image_at(offset) {
                node.name = std::sync::Arc::from(image_name.as_str());
                state.db2_item_rows = wz_db::db2_item_grid_rows(&node, linker);
                state.db2_status = format!("item rows {}", state.db2_item_rows.len());
            }
        }
    });
    if !state.db2_item_rows.is_empty() {
        egui::ScrollArea::vertical()
            .id_salt("db2_item_grid_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                egui::Grid::new("db2_item_grid")
                    .striped(true)
                    .num_columns(3)
                    .show(ui, |ui| {
                        ui.strong("id");
                        ui.strong("name");
                        ui.strong("desc");
                        ui.end_row();
                        for row in state.db2_item_rows.iter().take(200) {
                            ui.label(&row.id);
                            ui.label(&row.name);
                            ui.label(row.desc.replace('\n', " "));
                            ui.end_row();
                        }
                    });
            });
    }
    ui.horizontal(|ui| {
        if ui.button("현재 WZ DB2 Map 그리드").clicked() {
            let Some(linker) = state.string_linker.as_ref() else {
                state.db2_status = "StringLinker를 먼저 로드하세요".into();
                return;
            };
            let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
                state.db2_status = "아카이브 없음".into();
                return;
            };
            let Some(tree) = &state.tree else {
                state.db2_status = "트리 없음".into();
                return;
            };
            state.db2_map_rows = wz_db::db2_map_grid_rows(&archive, tree, linker);
            state.db2_status = format!("map rows {}", state.db2_map_rows.len());
        }
    });
    if !state.db2_map_rows.is_empty() {
        egui::ScrollArea::vertical()
            .id_salt("db2_map_grid_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                egui::Grid::new("db2_map_grid")
                    .striped(true)
                    .num_columns(5)
                    .show(ui, |ui| {
                        ui.strong("id");
                        ui.strong("street");
                        ui.strong("map");
                        ui.strong("mini");
                        ui.strong("link");
                        ui.end_row();
                        for row in state.db2_map_rows.iter().take(200) {
                            ui.label(&row.id);
                            ui.label(&row.street_name);
                            ui.label(&row.map_name);
                            ui.label(if row.has_minimap { "Y" } else { "" });
                            ui.label(
                                row.link
                                    .map(|v| v.to_string())
                                    .unwrap_or_default(),
                            );
                            ui.end_row();
                        }
                    });
            });
    }
    ui.horizontal(|ui| {
        if ui.button("현재 WZ DB2 Mob 그리드").clicked() {
            let Some(linker) = state.string_linker.as_ref() else {
                state.db2_status = "StringLinker를 먼저 로드하세요".into();
                return;
            };
            let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
                state.db2_status = "아카이브 없음".into();
                return;
            };
            let Some(tree) = &state.tree else {
                state.db2_status = "트리 없음".into();
                return;
            };
            state.db2_mob_rows = wz_db::db2_mob_grid_rows(&archive, tree, linker);
            state.db2_status = format!("mob rows {}", state.db2_mob_rows.len());
        }
    });
    if !state.db2_mob_rows.is_empty() {
        egui::ScrollArea::vertical()
            .id_salt("db2_mob_grid_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                egui::Grid::new("db2_mob_grid")
                    .striped(true)
                    .num_columns(8)
                    .show(ui, |ui| {
                        for label in ["id", "name", "stand", "fly", "move", "hit", "die", "link"] {
                            ui.strong(label);
                        }
                        ui.end_row();
                        for row in state.db2_mob_rows.iter().take(200) {
                            let flag = |v: bool| if v { "Y" } else { "" };
                            ui.label(&row.id);
                            ui.label(&row.name);
                            ui.label(flag(row.has_stand));
                            ui.label(flag(row.has_fly));
                            ui.label(flag(row.has_move));
                            ui.label(flag(row.has_hit));
                            ui.label(flag(row.has_die));
                            ui.label(
                                row.link.map(|v| v.to_string()).unwrap_or_default(),
                            );
                            ui.end_row();
                        }
                    });
            });
    }
    ui.horizontal(|ui| {
        if ui.button("현재 WZ DB2 Skill 그리드").clicked() {
            let Some(linker) = state.string_linker.as_ref() else {
                state.db2_status = "StringLinker를 먼저 로드하세요".into();
                return;
            };
            let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
                state.db2_status = "아카이브 없음".into();
                return;
            };
            let Some(tree) = &state.tree else {
                state.db2_status = "트리 없음".into();
                return;
            };
            state.db2_skill_rows = wz_db::db2_skill_grid_rows(&archive, tree, linker);
            state.db2_status = format!("skill rows {}", state.db2_skill_rows.len());
        }
    });
    if !state.db2_skill_rows.is_empty() {
        egui::ScrollArea::vertical()
            .id_salt("db2_skill_grid_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                egui::Grid::new("db2_skill_grid")
                    .striped(true)
                    .num_columns(6)
                    .show(ui, |ui| {
                        for label in ["id", "type", "book", "name", "icon", "h"] {
                            ui.strong(label);
                        }
                        ui.end_row();
                        for row in state.db2_skill_rows.iter().take(200) {
                            ui.label(&row.id);
                            ui.label(&row.row_type);
                            ui.label(&row.book);
                            ui.label(&row.name);
                            ui.label(if row.has_icon { "Y" } else { "" });
                            ui.label(row.h_count.to_string());
                            ui.end_row();
                        }
                    });
            });
    }
    ui.horizontal(|ui| {
        if ui.button("현재 WZ DB2 NPC 그리드").clicked() {
            let Some(linker) = state.string_linker.as_ref() else {
                state.db2_status = "StringLinker를 먼저 로드하세요".into();
                return;
            };
            let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
                state.db2_status = "아카이브 없음".into();
                return;
            };
            let Some(tree) = &state.tree else {
                state.db2_status = "트리 없음".into();
                return;
            };
            state.db2_npc_rows = wz_db::db2_npc_grid_rows(&archive, tree, linker);
            state.db2_status = format!("npc rows {}", state.db2_npc_rows.len());
        }
    });
    if !state.db2_npc_rows.is_empty() {
        egui::ScrollArea::vertical()
            .id_salt("db2_npc_grid_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                egui::Grid::new("db2_npc_grid")
                    .striped(true)
                    .num_columns(5)
                    .show(ui, |ui| {
                        for label in ["id", "name", "stand", "default", "link"] {
                            ui.strong(label);
                        }
                        ui.end_row();
                        for row in state.db2_npc_rows.iter().take(200) {
                            let flag = |v: bool| if v { "Y" } else { "" };
                            ui.label(&row.id);
                            ui.label(&row.name);
                            ui.label(flag(row.has_stand_0));
                            ui.label(flag(row.has_default_0));
                            ui.label(
                                row.link.map(|v| v.to_string()).unwrap_or_default(),
                            );
                            ui.end_row();
                        }
                    });
            });
    }
    ui.horizontal(|ui| {
        if ui.button("현재 WZ DB2 Reactor 그리드").clicked() {
            let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
                state.db2_status = "아카이브 없음".into();
                return;
            };
            let Some(tree) = &state.tree else {
                state.db2_status = "트리 없음".into();
                return;
            };
            state.db2_reactor_rows = wz_db::db2_reactor_grid_rows(&archive, tree);
            state.db2_status = format!("reactor rows {}", state.db2_reactor_rows.len());
        }
    });
    if !state.db2_reactor_rows.is_empty() {
        egui::ScrollArea::vertical()
            .id_salt("db2_reactor_grid_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                egui::Grid::new("db2_reactor_grid")
                    .striped(true)
                    .num_columns(4)
                    .show(ui, |ui| {
                        for label in ["id", "icon", "info", "link"] {
                            ui.strong(label);
                        }
                        ui.end_row();
                        for row in state.db2_reactor_rows.iter().take(200) {
                            let flag = |v: bool| if v { "Y" } else { "" };
                            ui.label(&row.id);
                            ui.label(flag(row.has_icon));
                            ui.label(flag(row.has_info));
                            ui.label(
                                row.link.map(|v| v.to_string()).unwrap_or_default(),
                            );
                            ui.end_row();
                        }
                    });
            });
    }
    ui.horizontal(|ui| {
        if ui.button("현재 WZ DB2 Music 그리드").clicked() {
            let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
                state.db2_status = "아카이브 없음".into();
                return;
            };
            let Some(tree) = &state.tree else {
                state.db2_status = "트리 없음".into();
                return;
            };
            let root_name = state
                .archive_path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("Sound.wz")
                .to_string();
            state.db2_music_rows = wz_db::db2_music_grid_rows(&archive, tree, &root_name);
            state.db2_status = format!("music rows {}", state.db2_music_rows.len());
        }
    });
    if !state.db2_music_rows.is_empty() {
        egui::ScrollArea::vertical()
            .id_salt("db2_music_grid_scroll")
            .max_height(200.0)
            .show(ui, |ui| {
                egui::Grid::new("db2_music_grid")
                    .striped(true)
                    .num_columns(3)
                    .show(ui, |ui| {
                        ui.strong("path");
                        ui.strong("bytes");
                        ui.strong("ms");
                        ui.end_row();
                        for row in state.db2_music_rows.iter().take(200) {
                            ui.label(&row.path);
                            ui.label(row.data_len.to_string());
                            ui.label(row.duration_ms.to_string());
                            ui.end_row();
                        }
                    });
            });
    }
    let filter = state.db2_filter.clone();
    let rows: Vec<&wz_db::Db2Row> = state
        .db2_rows
        .iter()
        .filter(|r| {
            filter.is_empty() || r.path.contains(&filter) || r.value.contains(&filter)
        })
        .take(200)
        .collect();
    egui::ScrollArea::vertical()
        .max_height(240.0)
        .show(ui, |ui| {
            egui::Grid::new("db2_grid")
                .striped(true)
                .num_columns(3)
                .show(ui, |ui| {
                    ui.strong("path");
                    ui.strong("type");
                    ui.strong("value");
                    ui.end_row();
                    for row in rows {
                        ui.label(&row.path);
                        ui.label(&row.node_type);
                        ui.label(&row.value);
                        ui.end_row();
                    }
                });
        });
}
