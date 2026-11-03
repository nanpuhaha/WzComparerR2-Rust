use crate::async_io;
use crate::panels::{catalog, inspector, tree_panel, viewport};
use crate::state::AppState;

#[derive(Default)]
pub struct PocApp {
    state: AppState,
    pub smoke: Option<crate::SmokeConfig>,
    smoke_phase: u8, // 0=open, 1=wait load, 2=load map, 3=render+save, 4=done
    smoke_wait_frames: u32,
}

impl eframe::App for PocApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 스모크 모드
        if let Some(cfg) = self.smoke.clone() {
            match self.smoke_phase {
                0 => {
                    // 파일 열기
                    async_io::open_file_dialog_with_path(&mut self.state, cfg.wz_path.clone());
                    self.smoke_phase = 1;
                }
                1 => {
                    async_io::poll_load_result(&mut self.state, ui.ctx());
        async_io::poll_map_images(&mut self.state, ui.ctx());
                    self.smoke_wait_frames += 1;
                    if self.state.tree.is_some() {
                        self.smoke_phase = 2;
                    } else if self.smoke_wait_frames > 300 {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                2 => {
                    async_io::load_map_by_path(&mut self.state);
                    // 동기적으로 이미지 로딩 완료를 기다린다 (스모크 모드 전용)
                    let mut waited = 0u32;
                    while self.state.map_images_rx.is_some() && waited < 3000 {
                        async_io::poll_map_images(&mut self.state, ui.ctx());
                        waited += 1;
                    }
                    self.smoke_phase = 3;
                }
                3 => {
                    // 뷰포트 렌더 (texture 생성)
                    viewport::show(ui, &mut self.state);
                    // 아바타 렌더도 스모크에 포함한다
                    if self.state.avatar_image.is_none() {
                        async_io::load_avatar_body(&mut self.state);
                    }
                    if self.state.avatar_image.is_some() && self.state.avatar_rendered.is_none() {
                        async_io::render_avatar_body(&mut self.state);
                    }
                    // texture 가 생겼으면 성공으로 간주하고 종료한다.
                    if self.state.viewport_texture.is_some() {
                        println!("SMOKE OK: viewport texture produced");
                        if self.state.avatar_rendered.is_some() {
                            println!("SMOKE OK: avatar rendered");
                        }
                        self.smoke_phase = 4;
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                _ => {}
            }
            // 스모크 모드에서는 패널을 그리지 않는다
            return;
        }

        // 일반 모드
        async_io::poll_load_result(&mut self.state, ui.ctx());
        async_io::poll_map_images(&mut self.state, ui.ctx());

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
            catalog::show(ui, &mut self.state);
            ui.separator();
            ui.heading("맵 뷰포트");
            viewport::show(ui, &mut self.state);
        });
    }
}
