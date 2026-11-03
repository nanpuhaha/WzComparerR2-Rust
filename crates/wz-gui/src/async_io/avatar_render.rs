// wz-gui — 비동기 I/O 및 백그라운드 파싱.
//
// `docs/gui-architecture.md` §5: UI 스레드는 블로킹 I/O 를 하지 않는다.
// 워커 스레드가 결과를 채널로 보내고, UI 스레드는 `poll_load_result` 로
// 매 프레임 폴링한다.



use crate::state::AppState;

// ── 파일 열기 ─────────────────────────────────────────────────────────

pub fn render_avatar_body(state: &mut AppState) {
    let Some((w, h, rgba)) = state.avatar_image.clone() else {
        state.avatar_status = "Body 이미지를 먼저 로드하세요".into();
        return;
    };
    use wz_render::{atlas::AtlasShelfPacker, batch::SpriteBatch, camera::Camera2D, device::RenderContext};

    let mut rc = match RenderContext::new_headless() {
        Ok(r) => r,
        Err(e) => { state.avatar_status = format!("RenderContext 실패: {e}"); return; }
    };
    let mut packer = AtlasShelfPacker::new(w.max(1), h.max(1));
    let slot = match packer.allocate_and_write(w, h, &rgba) {
        Some(s) => s,
        None => { state.avatar_status = "아틀라스 할당 실패".into(); return; }
    };
    for page in packer.pages() {
        let _ = rc.create_texture(page.key, page.width, page.height, &page.pixels);
    }
    let mut batch = SpriteBatch::new();
    batch.push_atlas_quad(slot.key, [0.0, 0.0, w as f32, h as f32], 0.0, &slot.rect, (w, h), [1.0, 1.0, 1.0, 1.0]);
    batch.finish();

    let cam = Camera2D::new(w as f32, h as f32);
    let uniforms = cam.build_uniforms(0.0, 0.0);
    match rc.render_to_rgba(w, h, &batch, &uniforms, [0.08, 0.08, 0.12, 1.0]) {
        Ok(out) => {
            state.avatar_status = format!("아바타 렌더 완료: {}x{}", w, h);
            state.avatar_rendered = Some((w, h, out));
        }
        Err(e) => { state.avatar_status = format!("렌더 실패: {e}"); }
    }
}

