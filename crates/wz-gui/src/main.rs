// wz-gui — Phase 8 데스크톱 GUI.
//
// `docs/gui-architecture.md` §3.1 구조:
//   main.rs  : 진입점 + eframe::App
//   state.rs : AppState (단일 진실 소스)
//   panels/  : tree_panel, inspector, viewport, avatar_panel
//   async_io.rs     : 파일 열기/로딩/맵 로딩
//   render_bridge.rs: wz-render ↔ egui 브릿지

mod app;
mod async_io;
mod background;
mod camera_input;
mod illuminant;
mod life;
mod panels;
mod particles;
mod portal;
mod reactor;
mod render_bridge;
mod skill;
mod sound_player;
mod state;
mod water;
mod worldmap;

use app::PocApp;

/// 스모크 모드: WZ 를 열어 첫 맵을 로드하고 뷰포트를 PNG 로 저장한 뒤 종료한다.
/// CI 에서 `--smoke <wz_path>` 로 호출한다.
#[derive(Clone)]
pub struct SmokeConfig {
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
