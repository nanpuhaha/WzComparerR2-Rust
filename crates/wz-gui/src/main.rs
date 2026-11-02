// wz-gui — Phase 8 데스크톱 GUI PoC 0.
//
// 목표: "egui + wgpu 30 가 컴파일되고 뜨는가" 를 실측으로 확인한다.
//   - eframe 윈도우가 뜬다.
//   - egui 가 텍스트/도형을 그린다.
//   - wz-render (헤드리스 RenderContext) 가 오프스크린으로 삼각형을 렌더하고
//     그 결과를 egui 텍스처로 표시한다 (wgpu 통합 경로 검증).
//
// 이 바이너리는 PoC 0 의 게이트다. PoC 1 부터 WZ 트리 브라우저를 얹는다.

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_title("WzComparerR2-Rust GUI PoC 0"),
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
    // 오프스크린 렌더 결과를 egui 텍스처로 캐시한다.
    texture: Option<egui::TextureHandle>,
    // 한 번만 렌더하도록 한다 (매 프레임 GPU 작업 방지).
    rendered_once: bool,
}

impl eframe::App for PocApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("WzComparerR2-Rust GUI PoC 0");
            ui.label("egui + wgpu 30 통합 검증 중…");

            let rect = ui.available_rect_before_wrap();
            let painter = ui.painter();
            let center = rect.center();
            let r = 60.0;
            painter.circle_filled(center, r, egui::Color32::from_rgb(80, 160, 220));
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                "egui OK",
                egui::FontId::proportional(16.0),
                egui::Color32::WHITE,
            );

            if !self.rendered_once {
                self.rendered_once = true;
                if let Some(tex) = render_offscreen_triangle(&ctx) {
                    self.texture = Some(tex);
                }
            }
            if let Some(tex) = &self.texture {
                ui.add(egui::Image::new(tex).max_width(256.0));
                ui.label("↑ wz-render 오프스크린 렌더 결과 (wgpu 30)");
            } else {
                ui.label("⚠ wz-render 오프스크린 렌더 실패 (로그 확인)");
            }
        });
    }
}

/// wz-render 의 헤드리스 경로로 256×256 RGBA 삼각형을 렌더한다.
/// 실패하면 None — PoC 0 에서는 화면에 "render failed" 로 드러난다.
fn render_offscreen_triangle(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    use wz_render::{batch::SpriteBatch, camera::Camera2D, device::RenderContext};

    let mut rc = RenderContext::new_headless().ok()?;
    let batch = SpriteBatch::new();
    let cam = Camera2D::new(256.0, 256.0);
    let uniforms = cam.build_uniforms(0.0, 0.0);
    let rgba = rc.render_to_rgba(256, 256, &batch, &uniforms, [0.1, 0.1, 0.15, 1.0]).ok()?;

    let color_image = egui::ColorImage::from_rgba_unmultiplied([256, 256], &rgba);
    Some(ctx.load_texture("offscreen", color_image, egui::TextureOptions::LINEAR))
}