use crate::async_io;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let now = ui.input(|i| i.time);
    let mut repaint = false;
    if async_io::tick_effect(state, now) {
        async_io::render_loaded_avatar(state);
    }
    if async_io::tick_chair(state, now) {
        async_io::render_loaded_avatar(state);
    }
    if state.effect_auto_play && !state.effect_frames.is_empty() {
        repaint = true;
    }
    if state.chair_auto_play && !state.chair_frames.is_empty() {
        repaint = true;
    }
    if repaint {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(50));
    }
    ui.label("29개 슬롯 (드롭다운 선택)");
    egui::ScrollArea::vertical()
        .max_height(240.0)
        .show(ui, |ui| {
            for (i, slot) in AVATAR_SLOTS.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{:>2}. {:<15}", i, slot));
                    if i < 29 {
                        let mut sel = state.avatar_slot_selection[i];
                        let preview = if sel == usize::MAX {
                            "없음".to_string()
                        } else if let Some(part) = state.avatar_loaded_parts.get(sel) {
                            format!("{} ({})", part.slot, sel)
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
                                    let label = if let Some(part) = state.avatar_loaded_parts.get(j)
                                    {
                                        format!("{} ({})", part.slot, j)
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

    ui.separator();
    ui.label("프리즘 (Prism)");
    let mut prism_changed = ui
        .checkbox(&mut state.prism_enabled, "프리즘 적용")
        .changed();
    ui.add_enabled_ui(state.prism_enabled, |ui| {
        egui::ComboBox::from_id_salt("prism_color_type")
            .selected_text(PRISM_COLORS[state.prism_color_type.clamp(0, 6) as usize])
            .show_ui(ui, |ui| {
                for (i, name) in PRISM_COLORS.iter().enumerate() {
                    prism_changed |= ui
                        .selectable_value(&mut state.prism_color_type, i as i32, *name)
                        .changed();
                }
            });
        prism_changed |= ui
            .add(egui::Slider::new(&mut state.prism_hue, 0..=359).text("Hue"))
            .changed();
        prism_changed |= ui
            .add(egui::Slider::new(&mut state.prism_saturation, 1..=199).text("Saturation"))
            .changed();
        prism_changed |= ui
            .add(egui::Slider::new(&mut state.prism_brightness, 1..=199).text("Brightness"))
            .changed();
        prism_changed |= ui
            .checkbox(&mut state.prism_convert_pure_black, "순수 검정 변환")
            .changed();
    });
    if state.prism_enabled {
        let prism = wz_avatar::PrismData {
            color_type: state.prism_color_type,
            hue: state.prism_hue,
            saturation: state.prism_saturation,
            brightness: state.prism_brightness,
            convert_pure_black: state.prism_convert_pure_black,
        };
        if !prism.is_valid() {
            ui.label(
                egui::RichText::new(
                    "Hue/Saturation/Brightness 중 하나를 기본값에서 바꿔야 적용됩니다",
                )
                .weak(),
            );
        }
    }
    if prism_changed && !state.avatar_loaded_parts.is_empty() {
        async_io::render_loaded_avatar(state);
    }

    ui.separator();
    ui.label("Hair/Face 믹스 컬러");
    let mut mix_changed = ui
        .checkbox(&mut state.hair_mix_enabled, "Hair 믹스")
        .changed();
    ui.add_enabled_ui(state.hair_mix_enabled, |ui| {
        egui::ComboBox::from_id_salt("hair_mix_color")
            .selected_text(MIX_COLORS[state.hair_mix_color.clamp(0, 8) as usize])
            .show_ui(ui, |ui| {
                for (i, name) in MIX_COLORS.iter().enumerate() {
                    mix_changed |= ui
                        .selectable_value(&mut state.hair_mix_color, i as i32, *name)
                        .changed();
                }
            });
        mix_changed |= ui
            .add(egui::Slider::new(&mut state.hair_mix_opacity, 0..=100).text("Hair Opacity"))
            .changed();
    });
    mix_changed |= ui
        .checkbox(&mut state.face_mix_enabled, "Face 믹스")
        .changed();
    ui.add_enabled_ui(state.face_mix_enabled, |ui| {
        egui::ComboBox::from_id_salt("face_mix_color")
            .selected_text(MIX_COLORS[state.face_mix_color.clamp(0, 8) as usize])
            .show_ui(ui, |ui| {
                for (i, name) in MIX_COLORS.iter().enumerate() {
                    mix_changed |= ui
                        .selectable_value(&mut state.face_mix_color, i as i32, *name)
                        .changed();
                }
            });
        mix_changed |= ui
            .add(egui::Slider::new(&mut state.face_mix_opacity, 0..=100).text("Face Opacity"))
            .changed();
    });
    ui.separator();
    ui.label("Effect / Custom Origin");
    if state.effect_frames.is_empty() {
        ui.add(egui::Slider::new(&mut state.effect_frame, 0..=30).text("Effect frame"));
    } else {
        let max_frame = state.effect_frames.len() - 1;
        if state.effect_frame > max_frame {
            state.effect_frame = max_frame;
        }
        if ui
            .add(egui::Slider::new(&mut state.effect_frame, 0..=max_frame).text("Effect frame"))
            .changed()
        {
            async_io::apply_effect_frame(state);
            async_io::render_loaded_avatar(state);
        }
    }
    ui.checkbox(&mut state.effect_auto_play, "Effect 자동 재생");
    if !state.chair_frames.is_empty() {
        let max_frame = state.chair_frames.len() - 1;
        if state.chair_frame > max_frame {
            state.chair_frame = max_frame;
        }
        if ui
            .add(egui::Slider::new(&mut state.chair_frame, 0..=max_frame).text("Chair frame"))
            .changed()
        {
            state.chair_last_time = 0.0;
            async_io::apply_chair_frame(state);
            async_io::render_loaded_avatar(state);
        }
        let auto = ui.checkbox(&mut state.chair_auto_play, "Rolling house 자동 재생");
        if auto.changed() {
            state.chair_last_time = 0.0;
        }
    }
    ui.horizontal(|ui| {
        ui.label("Custom origin:");
        ui.text_edit_singleline(&mut state.custom_origin_input);
    });
    ui.label("형식: cap:10,20;ear:5,5 (childName 기준, 장비 로드 시 적용)");
    if ui
        .button("믹스/특수 레이어 적용 (장비 다시 로드)")
        .clicked()
    {
        async_io::load_avatar_with_equipment_input(state);
    }
    if mix_changed {
        ui.label(
            egui::RichText::new("Hair/Face 믹스와 특수 레이어는 장비 로드 시 적용됩니다").weak(),
        );
    }
    if !state.avatar_loaded_parts.is_empty() {
        ui.separator();
        ui.label(format!(
            "로드된 파트 ({}):",
            state.avatar_loaded_parts.len()
        ));
        for (i, part) in state.avatar_loaded_parts.iter().enumerate() {
            ui.label(format!(
                "  {}: {} ({}x{}) @ {},{} z={}",
                i,
                part.slot,
                part.width,
                part.height,
                part.position.0,
                part.position.1,
                part.z_index
            ));
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
        async_io::render_loaded_avatar(state);
    }
    ui.label(&state.avatar_status);
    if let Some((w, h, rgba)) = &state.avatar_image {
        let ci = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
        let tex = ui
            .ctx()
            .load_texture("avatar_body", ci, egui::TextureOptions::LINEAR);
        ui.add(egui::Image::new(&tex).max_width(120.0));
        ui.label(format!("Body: {}x{}", w, h));
    }
    if let Some((w, h, rgba)) = &state.avatar_rendered {
        let ci = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
        let tex = ui
            .ctx()
            .load_texture("avatar_rendered", ci, egui::TextureOptions::LINEAR);
        ui.add(egui::Image::new(&tex).max_width(200.0));
        ui.label(format!("Rendered: {}x{}", w, h));
    }
}

pub const MIX_COLORS: [&str; 9] = ["0", "1", "2", "3", "4", "5", "6", "7", "8"];

pub const PRISM_COLORS: [&str; 7] = [
    "전체 색상",
    "빨간색",
    "노란색",
    "초록색",
    "청록색",
    "파란색",
    "자주색",
];

pub const AVATAR_SLOTS: [&str; 29] = [
    "Body",
    "Head",
    "Face",
    "Hair",
    "Cap",
    "Coat",
    "Longcoat",
    "Pants",
    "Shoes",
    "Glove",
    "SubWeapon",
    "Cape",
    "Weapon",
    "Earrings",
    "FaceAccessory",
    "EyeAccessory",
    "Taming",
    "Saddle",
    "Chair",
    "Effect",
    "Pendant",
    "Belt",
    "ShoulderPad",
    "Pocket",
    "Emblem",
    "Ring1",
    "Ring2",
    "Ring3",
    "Ring4",
];
