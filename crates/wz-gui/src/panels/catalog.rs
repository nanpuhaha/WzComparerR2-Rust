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
    ui.label(&state.db2_status);
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
