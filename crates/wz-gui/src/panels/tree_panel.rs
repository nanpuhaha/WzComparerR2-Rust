use crate::async_io;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label("WZ 트리");
    ui.text_edit_singleline(&mut state.search);
    ui.separator();

    if state.tree.is_none() {
        ui.label("WZ 파일을 열어 주세요.");
        return;
    }
    let tree = state.tree.clone().unwrap();
    let archive = state.shared_archive.lock().unwrap().clone();

    let mut rows = Vec::new();
    async_io::build_rows(&mut rows, &tree, archive.as_deref(), state, 0, "");
    if !state.search.is_empty() {
        let q = state.search.to_lowercase();
        rows.retain(|r| r.name.to_lowercase().contains(&q));
    }

    let row_height = 18.0;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, row_height, rows.len(), |ui, range| {
            for row in &rows[range] {
                async_io::render_row(ui, state, row);
            }
        });
}
