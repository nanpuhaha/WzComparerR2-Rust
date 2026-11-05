// wz-gui — 비동기 I/O 및 백그라운드 파싱.
//
// `docs/gui-architecture.md` §5: UI 스레드는 블로킹 I/O 를 하지 않는다.
// 워커 스레드가 결과를 채널로 보내고, UI 스레드는 `poll_load_result` 로
// 매 프레임 폴링한다.

use crate::state::AppState;
use wz_core::WzArchive;

// ── 파일 열기 ─────────────────────────────────────────────────────────

pub fn load_string_linker(state: &mut AppState) {
    let Some(data_root) = state.data_root.clone() else {
        state.string_linker_status = "data_root 가 설정되지 않았습니다".into();
        return;
    };
    // String.wz 는 컨테이너일 수 있다. String_000.wz 를 우선 시도한다.
    let string_dir = data_root.join("String");
    let candidates = [
        string_dir.join("String_000.wz"),
        string_dir.join("String.wz"),
    ];
    let string_wz = match candidates.iter().find(|p| p.exists()) {
        Some(p) => p.clone(),
        None => {
            state.string_linker_status = format!("String WZ 없음: {}", string_dir.display());
            return;
        }
    };
    match (|| -> Result<wz_db::StringLinker, String> {
        let archive = WzArchive::open(&string_wz).map_err(|e| format!("open: {e}"))?;
        let (arena, _) = wz_db::Arena::from_archive(&archive);
        let mut linker = wz_db::StringLinker::new();
        linker.load_string_wz(&arena, arena.root());
        Ok(linker)
    })() {
        Ok(linker) => {
            state.string_linker_status =
                format!("StringLinker 로드 완료 (eqp: {})", linker.string_eqp.len());
            state.string_linker = Some(linker);
        }
        Err(e) => {
            state.string_linker_status = format!("StringLinker 로드 실패: {e}");
        }
    }
}

// ── 아바타 장비 로딩 ─────────────────────────────────────────────────
