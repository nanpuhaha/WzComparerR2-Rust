use crate::async_io;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
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
    if !state.avatar_loaded_parts.is_empty() {
        ui.separator();
        ui.label(format!("로드된 파트 ({}):", state.avatar_loaded_parts.len()));
        for (i, (slot, w, h)) in state.avatar_loaded_parts.iter().enumerate() {
            ui.label(format!("  {}: {} ({}x{})", i, slot, w, h));
        }
    }
    if state.avatar_rendered.is_some() || state.avatar_image.is_some() {
        ui.label(format!("렌더링된 파트: {}", state.avatar_status));
    }
    if ui.button("아바타 렌더 (오프스크린, 선택 파트)").clicked() {
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

pub const AVATAR_SLOTS: [&str; 29] = [
    "Body", "Head", "Face", "Hair", "Cap", "Coat", "Longcoat", "Pants", "Shoes",
    "Glove", "SubWeapon", "Cape", "Weapon", "Earrings", "FaceAccessory",
    "EyeAccessory", "Taming", "Saddle", "Chair", "Effect", "Pendant", "Belt",
    "ShoulderPad", "Pocket", "Emblem", "Ring1", "Ring2", "Ring3", "Ring4",
];
