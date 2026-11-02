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
    }

    ui.separator();
    ui.heading("아바타 (PoC 3.5)");
    show_avatar(ui, state);
}

fn show_avatar(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label("29개 슬롯 (PoC 3.5 — Body 이미지 로딩)");
    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
        for (i, slot) in AVATAR_SLOTS.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{:>2}. {}", i, slot));
                ui.label(egui::RichText::new("(empty)").weak());
            });
        }
    });
    if ui.button("Body 이미지 로드 (Character.wz)").clicked() {
        async_io::load_avatar_body(state);
    }
    if ui.button("아바타 렌더 (오프스크린)").clicked() {
        async_io::render_avatar_body(state);
    }
    ui.label(&state.avatar_status);
    if let Some((w, h, rgba)) = &state.avatar_image {
        let ci = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
        let tex = ui.ctx().load_texture("avatar_body", ci, egui::TextureOptions::LINEAR);
        ui.add(egui::Image::new(&tex).max_width(200.0));
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
