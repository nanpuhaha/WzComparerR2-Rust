// wz-gui — 비동기 I/O 및 백그라운드 파싱.
//
// `docs/gui-architecture.md` §5: UI 스레드는 블로킹 I/O 를 하지 않는다.
// 워커 스레드가 결과를 채널로 보내고, UI 스레드는 `poll_load_result` 로
// 매 프레임 폴링한다.

use std::sync::Arc;
use std::thread;

use rfd::FileDialog;

use crate::state::{AppState, LoadMsg};
use wz_core::WzArchive;
use wz_core::dirtree::DirTree;

// ── 파일 열기 ─────────────────────────────────────────────────────────

pub fn open_file_dialog(state: &mut AppState) {
    if state.loading {
        return;
    }
    let picked = FileDialog::new()
        .add_filter("WZ files", &["wz"])
        .add_filter("All files", &["*"])
        .pick_file();
    let Some(path) = picked else { return; };

    state.loading = true;
    state.status = format!("로딩 중: {}", path.display());
    state.tree = None;
    state.archive = None;
    state.archive_path = Some(path.clone());
    *state.shared_archive.lock().unwrap() = None;
    state.image_children.clear();
    state.expanded.clear();
    state.selected = None;
    state.image_textures.clear();
    state.map_back_image = None;
    state.map_tile_images.clear();
    state.current_map = None;
    state.avatar_image = None;

    let shared = state.shared_archive.clone();
    let (tx, rx) = crossbeam_channel::unbounded();
    state.load_rx = Some(rx);

    thread::spawn(move || {
        let result = (|| -> Result<DirTree, String> {
            let archive = WzArchive::open(&path).map_err(|e| format!("open: {e}"))?;
            let tree = archive.read_tree().map_err(|e| format!("read_tree: {e}"))?;
            *shared.lock().unwrap() = Some(Arc::new(archive));
            Ok(tree)
        })();
        let msg = match result {
            Ok(t) => LoadMsg::Loaded { tree: t },
            Err(e) => LoadMsg::Failed(e),
        };
        let _ = tx.send(msg);
    });
}

pub fn open_file_dialog_with_path(state: &mut AppState, path: std::path::PathBuf) {
    if state.loading { return; }
    state.loading = true;
    state.status = format!("로딩 중: {}", path.display());
    state.tree = None;
    state.archive = None;
    state.archive_path = Some(path.clone());
    *state.shared_archive.lock().unwrap() = None;
    state.image_children.clear();
    state.expanded.clear();
    state.selected = None;
    state.image_textures.clear();
    state.map_back_image = None;
    state.map_tile_images.clear();
    state.map_obj_images.clear();
    state.current_map = None;
    state.avatar_image = None;

    let shared = state.shared_archive.clone();
    let (tx, rx) = crossbeam_channel::unbounded();
    state.load_rx = Some(rx);

    std::thread::spawn(move || {
        let result = (|| -> Result<DirTree, String> {
            let archive = WzArchive::open(&path).map_err(|e| format!("open: {e}"))?;
            let tree = archive.read_tree().map_err(|e| format!("read_tree: {e}"))?;
            *shared.lock().unwrap() = Some(std::sync::Arc::new(archive));
            Ok(tree)
        })();
        let msg = match result {
            Ok(t) => crate::state::LoadMsg::Loaded { tree: t },
            Err(e) => crate::state::LoadMsg::Failed(e),
        };
        let _ = tx.send(msg);
    });
}

