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

/// 스모크 모드: WZ 를 열어 첫 맵을 로드하고 뷰포트를 PNG 로 저장한 뒤 종료한다.
/// CI 에서 `--smoke <wz_path>` 로 호출한다.
#[derive(Clone)]
struct SmokeConfig {
    wz_path: std::path::PathBuf,
}

fn parse_smoke() -> Option<SmokeConfig> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--smoke" && i + 1 < args.len() {
            let wz = std::path::PathBuf::from(&args[i + 1]);
            return Some(SmokeConfig { wz_path: wz });
        }
        i += 1;
    }
    None
}

fn main() -> eframe::Result<()> {
    let smoke = parse_smoke();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 700.0])
            .with_title("WzComparerR2-Rust GUI"),
        ..Default::default()
    };
    eframe::run_native(
        "wz-gui",
        options,
        Box::new(move |_cc| {
            let mut app = PocApp::default();
            if let Some(cfg) = smoke {
                app.smoke = Some(cfg);
            }
            Ok(Box::new(app))
        }),
    )
}

#[derive(Default)]
struct PocApp {
    state: AppState,
    smoke: Option<SmokeConfig>,
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
            ui.heading("맵 뷰포트");
            viewport::show(ui, &mut self.state);
        });
    }
}
