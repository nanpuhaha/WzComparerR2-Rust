// wz-gui — Phase 8 데스크톱 GUI.
//
// `docs/gui-architecture.md` §3.1 구조:
//   main.rs  : 진입점 + eframe::App
//   state.rs : AppState (단일 진실 소스)
//   panels/  : tree_panel, inspector, viewport, avatar_panel
//   async_io.rs     : 파일 열기/로딩/맵 로딩
//   render_bridge.rs: wz-render ↔ egui 브릿지

mod async_io;
mod panels;
mod render_bridge;
mod state;

use panels::inspector;
use panels::tree_panel;
use panels::viewport;
use state::AppState;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 700.0])
            .with_title("WzComparerR2-Rust GUI"),
        ..Default::default()
    };
    eframe::run_native(
        "wz-gui",
        options,
        Box::new(|_cc| Ok(Box::new(PocApp::default()))),
    )
}

#[derive(Default)]
struct PocApp {
    state: AppState,
}

impl eframe::App for PocApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        async_io::poll_load_result(&mut self.state, ui.ctx());

        egui::Panel::top("menubar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("열기 (Ctrl+O)").clicked() {
                    async_io::open_file_dialog(&mut self.state);
                }
                if self.state.loading {
                    ui.spinner();
                    ui.label(&self.state.status);
                } else {
                    ui.label(&self.state.status);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("wz-gui");
                });
            });
        });

        egui::Panel::left("tree_panel")
            .default_size(320.0)
            .show(ui, |ui| {
                tree_panel::show(ui, &mut self.state);
            });

        egui::CentralPanel::default().show(ui, |ui| {
            inspector::show(ui, &mut self.state);
            ui.separator();
            ui.heading("맵 뷰포트");
            viewport::show(ui, &mut self.state);
        });
    }
}
