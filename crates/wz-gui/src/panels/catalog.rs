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
}
