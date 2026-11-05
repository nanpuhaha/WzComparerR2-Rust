//! `wz patch` — MapleStory `.patch` 확인 및 적용.

use std::path::Path;
use std::sync::Arc;

use wz_core::patch::{apply_patch, PatchApplyOptions, PatchHeader, PatchPartType};

pub fn cmd_patch(
    patch_path: &Path,
    old_root: Option<&Path>,
    out_root: Option<&Path>,
    info_only: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let src: Arc<dyn wz_pool::ByteSource> = wz_pool::open_file(patch_path)?.into();
    let header = PatchHeader::parse(&src)?;
    println!(
        "patch: version={} checksum=0x{:08X} valid={} compressed={} notice={}",
        header.version,
        header.expected_checksum,
        header.is_valid,
        header.compressed_len,
        header.notice_text.as_deref().unwrap_or("")
    );

    let payload = header.read_patch(&src)?;
    let mut created = 0;
    let mut modified = 0;
    let mut deleted = 0;
    for part in &payload.parts {
        match part.part_type {
            PatchPartType::Create => created += 1,
            PatchPartType::Modify => modified += 1,
            PatchPartType::Delete => deleted += 1,
        }
    }
    println!(
        "records: create={created} modify={modified} delete={deleted} kmst1125={} hashes={} payload={} bytes",
        payload.is_kmst1125,
        payload.old_hashes.len(),
        payload.bytes.len()
    );

    if info_only {
        for part in &payload.parts {
            println!(
                "{:?}\t{}\toffset={} len={} new=0x{:08X}",
                part.part_type,
                part.file_name,
                part.data_offset,
                part.new_file_len,
                part.new_checksum.unwrap_or(0)
            );
        }
        return Ok(());
    }

    let (Some(old_root), Some(out_root)) = (old_root, out_root) else {
        println!("적용하려면 --old <원본 디렉터리> --out <출력 디렉터리> 를 지정하세요");
        return Ok(());
    };
    let report = apply_patch(&payload, old_root, out_root, &PatchApplyOptions::default())?;
    println!(
        "적용: 생성 {} 수정 {} 삭제 {} 건너뜀 {} 오류 {}",
        report.created,
        report.modified,
        report.deleted,
        report.skipped,
        report.errors.len()
    );
    for error in report.errors.iter().take(20) {
        eprintln!("  {error}");
    }
    if !report.errors.is_empty() {
        return Err(format!("{}개 파일 적용 실패", report.errors.len()).into());
    }
    Ok(())
}
