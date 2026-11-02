use crate::render_bridge;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    render_bridge::render_viewport(ui, state);
}
