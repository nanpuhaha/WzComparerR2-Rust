//! 실전 코퍼스 벌크 파싱 검증.
//!
//! ```text
//! cargo run -p wz-cli --example bulk_scan -- <데이터 루트>
//! ```
//!
//! - `WzArchive::open` (헤더/프로파일 감지)
//! - 디렉터리 트리 전체 순회
//! - 모든 이미지 엔트리 `parse_image_at` (이미지 내부 노드/값/Property 재귀)
//! - 모든 Canvas(PngRef): zlib 해제 + 0페이지 RGBA8 디코딩 시도
//!
//! 각 파일마다 stdout 에 JSON-line 을 뽑는다. 실패는 stderr 에 집계한다.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Stats {
    nodes: u64,
    canvases: u64,
    sounds: u64,
    videos: u64,
    rawdatas: u64,
    uols: u64,
    decoded_canvas: u64,
    /// 알려진 포맷인데 디코딩 실패 → 진짜 실패
    canvas_decode_fail: u64,
    /// Unknown 포맷 (원본도 해석 규칙이 결정적이지 않음) → 격리 집계
    canvas_undecodable_unknown: u64,
    failures: Vec<String>,
}

impl Stats {
    fn merge(&mut self, o: Stats) {
        self.nodes += o.nodes;
        self.canvases += o.canvases;
        self.sounds += o.sounds;
        self.videos += o.videos;
        self.rawdatas += o.rawdatas;
        self.uols += o.uols;
        self.decoded_canvas += o.decoded_canvas;
        self.canvas_decode_fail += o.canvas_decode_fail;
        self.canvas_undecodable_unknown += o.canvas_undecodable_unknown;
        self.failures.extend(o.failures);
    }
}

fn walk_node(node: &wz_core::image::Node, src: &Arc<dyn wz_pool::ByteSource>, stats: &mut Stats) {
    stats.nodes += 1;

    if let Some(v) = &node.value {
        match v {
            wz_core::image::ImageValue::Png(p) => {
                stats.canvases += 1;
                canvas_check(p, src, stats);
            }
            wz_core::image::ImageValue::Sound(_) => stats.sounds += 1,
            wz_core::image::ImageValue::Video(_) => stats.videos += 1,
            wz_core::image::ImageValue::RawData(_) => stats.rawdatas += 1,
            wz_core::image::ImageValue::Uol(_) => stats.uols += 1,
            _ => {}
        }
    }

    for ch in &node.children {
        walk_node(ch, src, stats);
    }
    if let Some(wz_core::image::ImageValue::Property(inner)) = &node.value {
        for ch in &inner.children {
            walk_node(ch, src, stats);
        }
    }
}

fn canvas_check(
    png_ref: &wz_core::image::PngRef,
    src: &Arc<dyn wz_pool::ByteSource>,
    stats: &mut Stats,
) {
    use wz_image::{decode_raw_page_to_rgba8, decompress_zlib, TextureFormat};

    if png_ref.width == 0 || png_ref.height == 0 || png_ref.data_len <= 1 {
        return;
    }
    let raw_payload = match png_ref.extract_raw_bytes(src) {
        Ok(b) => b,
        Err(e) => {
            stats.failures.push(format!("canvas-extract: {e}"));
            return;
        }
    };
    let tex_fmt = TextureFormat::from_u32(png_ref.format as u32);
    let result = match tex_fmt {
        TextureFormat::Unknown => {
            // unknown 포맷: zlib 해제 가능성만 확인. 성패를 실패로 세지 않음.
            match decompress_zlib(&raw_payload) {
                Ok(_) => stats.decoded_canvas += 1,
                Err(_) => stats.canvas_undecodable_unknown += 1,
            }
            return;
        }
        fmt => decode_raw_page_to_rgba8(
            fmt,
            png_ref.width as u32,
            png_ref.height as u32,
            png_ref.scale as u32,
            png_ref.pages as u32,
            0,
            &raw_payload,
        )
        .map(|_| ()),
    };
    match result {
        Ok(()) => stats.decoded_canvas += 1,
        Err(_) => stats.canvas_decode_fail += 1,
    }
}

fn scan_file(path: &Path) -> Result<Stats, Box<dyn std::error::Error>> {
    use wz_core::dirtree::node_type::{DIRECTORY, IMAGE, IMAGE_NAMED_BY_REF};
    use wz_core::WzArchive;

    let archive = WzArchive::open(path)?;
    let src = archive.source().clone();
    let mut stats = Stats::default();
    let tree = archive.read_tree_with_mode(false)?;

    let entries = tree
        .root
        .entries
        .iter()
        .chain(tree.blocks.iter().flat_map(|b| &b.entries));

    for e in entries {
        if e.node_type == DIRECTORY || (e.node_type != IMAGE && e.node_type != IMAGE_NAMED_BY_REF) {
            continue;
        }
        let Some(off) = e.offset else {
            stats.failures.push(format!(
                "entry-offset-unresolved: {}",
                e.name.as_deref().unwrap_or("?")
            ));
            continue;
        };
        let img_root = match archive.parse_image_at(off) {
            Ok(n) => n,
            Err(err) => {
                stats
                    .failures
                    .push(format!("image {}: {err}", e.name.as_deref().unwrap_or("?")));
                continue;
            }
        };
        let mut local = Stats::default();
        walk_node(&img_root, &src, &mut local);
        stats.merge(local);
    }

    Ok(stats)
}

fn collect_wz(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_wz(&p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("wz") {
            out.push(p);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args().nth(1).expect("usage: bulk_scan <root>");

    let mut files = Vec::new();
    collect_wz(Path::new(&root), &mut files);
    files.sort();
    files.shrink_to_fit();
    eprintln!("files: {}", files.len());

    let done = Arc::new(AtomicUsize::new(0));
    let failures: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let totals = Arc::new(Mutex::new(Stats::default()));
    let files = Arc::new(files);

    for f in files.iter() {
        let disp = f.display();
        match scan_file(f) {
            Ok(mut stats) => {
                for e in &stats.failures {
                    failures.lock().unwrap().push(format!("{disp}: {e}"));
                }
                let json = serde_json::json!({
                    "file": disp.to_string(),
                    "nodes": stats.nodes,
                    "canvases": stats.canvases,
                    "decoded": stats.decoded_canvas,
                    "decode_fail": stats.canvas_decode_fail,
                    "unknown_fmt": stats.canvas_undecodable_unknown,
                    "sound": stats.sounds,
                    "video": stats.videos,
                    "rawdata": stats.rawdatas,
                    "uol": stats.uols,
                    "failures": stats.failures.len(),
                });
                println!("{json}");
                totals.lock().unwrap().merge(std::mem::take(&mut stats));
            }
            Err(e) => {
                failures
                    .lock()
                    .unwrap()
                    .push(format!("{disp}: archive: {e}"));
            }
        }
        let n = done.fetch_add(1, Ordering::SeqCst) + 1;
        if n.is_multiple_of(100) {
            eprintln!("progress: {n}");
        }
    }

    let t = totals.lock().unwrap();
    eprintln!();
    eprintln!("=== SUMMARY ===");
    eprintln!("nodes: {}", t.nodes);
    eprintln!(
        "canvases: {} (decoded {} / decode_fail {} / unknown_fmt {})",
        t.canvases, t.decoded_canvas, t.canvas_decode_fail, t.canvas_undecodable_unknown
    );
    eprintln!(
        "sounds: {} (구조만 검증), videos: {}, rawdata: {}, uol: {}",
        t.sounds, t.videos, t.rawdatas, t.uols
    );
    let fails = failures.lock().unwrap();
    eprintln!("failures: {}", fails.len());
    for f in fails.iter().take(80) {
        eprintln!("  FAIL {f}");
    }
    Ok(())
}
