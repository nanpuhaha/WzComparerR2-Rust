// wz-gui — 비동기 I/O 및 백그라운드 파싱.
//
// `docs/gui-architecture.md` §5: UI 스레드는 블로킹 I/O 를 하지 않는다.
// 워커 스레드가 결과를 채널로 보내고, UI 스레드는 `poll_load_result` 로
// 매 프레임 폴링한다.



use crate::state::AppState;
use wz_core::WzArchive;
use wz_core::dirtree::node_type as nt;
use wz_core::image::ImageValue;

// ── 파일 열기 ─────────────────────────────────────────────────────────

pub fn find_data_root(archive_path: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    // archive_path 에서 "Data" 디렉터리를 찾는다.
    if let Some(p) = archive_path {
        let mut cur = p.to_path_buf();
        while let Some(parent) = cur.parent() {
            if parent.file_name().and_then(|s| s.to_str()) == Some("Data") {
                return Some(parent.to_path_buf());
            }
            cur = parent.to_path_buf();
        }
    }
    // 폴백: 일반적인 설치 경로
    let candidates = [
        "<WZ_DATA_ROOT>",
    ];
    for c in candidates {
        let p = std::path::PathBuf::from(c);
        if p.join("Map").exists() { return Some(p); }
    }
    None
}

pub fn load_back_image(data_root: &std::path::Path, b_s: &str) -> Option<(u32, u32, Vec<u8>)> {
    load_canvas_image(data_root, "Back", b_s, "")
}

pub fn load_canvas_image(
    data_root: &std::path::Path,
    subdir: &str,
    canvas_name: &str,
    inner_path: &str,
) -> Option<(u32, u32, Vec<u8>)> {
    let canvas_dir = data_root.join("Map").join(subdir).join("_Canvas");
    let entries = std::fs::read_dir(&canvas_dir).ok()?;
    let img_name = format!("{}.img", canvas_name);
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") { continue; }
        let Ok(arch) = WzArchive::open(&path) else { continue; };
        let Ok(tree) = arch.read_tree() else { continue; };
        let e = tree.root.entries.iter().find(|e| {
            e.name.as_deref() == Some(canvas_name) || e.name.as_deref() == Some(img_name.as_str())
        });
        let Some(e) = e else { continue; };
        let Some(off) = e.offset else { continue; };
        let Ok(node) = arch.parse_image_at(off) else { continue; };

        let segs: Vec<&str> = inner_path.split('/').filter(|s| !s.is_empty()).collect();
        let target = if segs.is_empty() {
            &node
        } else {
            let mut cur = &node;
            let mut ok = true;
            for seg in &segs {
                let next = if let Some(ImageValue::Property(p)) = &cur.value {
                    p.children.iter().find(|c| c.name.as_ref() == *seg)
                        .or_else(|| cur.children.iter().find(|c| c.name.as_ref() == *seg))
                } else {
                    cur.children.iter().find(|c| c.name.as_ref() == *seg)
                };
                match next {
                    Some(n) => cur = n,
                    None => { ok = false; break; }
                }
            }
            if !ok { continue; }
            cur
        };

        let Some(pr) = target.first_canvas() else { continue; };
        let file_src = wz_pool::FileSource::open(&path).ok()?;
        let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
        if let Ok((w, h, rgba)) = wz_image::decode_png_ref_to_rgba(&pr, &src) {
            return Some((w, h, rgba));
        }
    }
    None
}

pub fn load_avatar_body(state: &mut AppState) {
    // Character/_Canvas/_Canvas_000.wz 에서 첫 번째 이미지를 로드한다.
    let canvas_path = "<WZ_DATA_ROOT>/Character/_Canvas/_Canvas_000.wz";
    let cp = std::path::PathBuf::from(canvas_path);
    let Ok(arch) = WzArchive::open(&cp) else {
        state.avatar_status = "Character _Canvas 열기 실패".into();
        return;
    };
    let Ok(tree) = arch.read_tree() else {
        state.avatar_status = "트리 읽기 실패".into();
        return;
    };
    let Some(img_entry) = tree.root.entries.iter().find(|e| e.node_type == nt::IMAGE) else {
        state.avatar_status = "이미지 없음".into();
        return;
    };
    let Some(off) = img_entry.offset else {
        state.avatar_status = "오프셋 없음".into();
        return;
    };
    let Ok(node) = arch.parse_image_at(off) else {
        state.avatar_status = "이미지 파싱 실패".into();
        return;
    };
    let Some(pr) = node.first_canvas() else {
        state.avatar_status = "캔버스 없음".into();
        return;
    };
    let file_src = match wz_pool::FileSource::open(&cp) { Ok(f) => f, Err(_) => { state.avatar_status = "파일 열기 실패".into(); return; } };
    let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
    match wz_image::decode_png_ref_to_rgba(&pr, &src) {
        Ok((w, h, rgba)) => {
            state.avatar_status = format!("Body 이미지 로드: {}x{} ({} bytes)", w, h, rgba.len());
            state.avatar_image = Some((w, h, rgba));
            state.viewport_dirty = true;
        }
        Err(e) => {
            state.avatar_status = format!("디코딩 실패: {e}");
        }
    }
}
// ── 트리 행 렌더링 ────────────────────────────────────────────────────

