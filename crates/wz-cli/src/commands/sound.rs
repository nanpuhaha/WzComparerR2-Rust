//! `wz sound` — Sound_DX8 정보/추출/재생.

use std::path::Path;

use wz_core::image::ImageValue;
use wz_core::{extract_sound, WzArchive};

pub fn cmd_sound(
    wz_path: &Path,
    sound_path: &str,
    out: Option<&Path>,
    play: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let clean = sound_path.trim_matches('/');
    let (img_name, sub) = match clean.split_once('/') {
        Some((img, sub)) => (img.to_string(), sub),
        None => {
            return Err("사운드 경로는 '<이미지>.img/<이름>' 형식이어야 합니다".into());
        }
    };
    let img_name = if img_name.ends_with(".img") {
        img_name
    } else {
        format!("{img_name}.img")
    };
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some(img_name.as_str()))
        .ok_or_else(|| format!("이미지를 찾을 수 없습니다: {img_name}"))?;
    let node = archive.parse_image_at(entry.offset.ok_or("이미지 오프셋 없음")?)?;

    let mut cur = &node;
    for segment in sub.split('/') {
        cur = cur
            .object_children()
            .iter()
            .find(|c| c.name.as_ref() == segment)
            .ok_or_else(|| format!("사운드 노드를 찾을 수 없습니다: {sound_path}"))?;
    }
    let Some(ImageValue::Sound(sound)) = &cur.value else {
        return Err(format!("Sound_DX8 노드가 아닙니다: {sound_path}").into());
    };
    let extracted = extract_sound(archive.source(), sound)?;
    println!(
        "sound: {} codec={:?} ext={} data_len={} duration_ms={} format={:?}",
        sound_path,
        extracted.codec,
        extracted.extension,
        sound.data_len,
        sound.duration,
        extracted.format
    );

    let target = match out {
        Some(path) => {
            let path = if path.extension().is_none() {
                path.with_extension(extracted.extension)
            } else {
                path.to_path_buf()
            };
            std::fs::write(&path, &extracted.bytes)?;
            println!("저장: {} ({} bytes)", path.display(), extracted.bytes.len());
            Some(path)
        }
        None if play => {
            let path = std::env::temp_dir().join(format!(
                "wz-sound-{}.{}",
                std::process::id(),
                extracted.extension
            ));
            std::fs::write(&path, &extracted.bytes)?;
            Some(path)
        }
        None => None,
    };

    if play {
        let path = target.ok_or("재생할 파일이 없습니다")?;
        #[cfg(target_os = "macos")]
        let status = std::process::Command::new("afplay").arg(&path).status()?;
        #[cfg(not(target_os = "macos"))]
        let status = std::process::Command::new("ffplay")
            .args(["-nodisp", "-autoexit"])
            .arg(&path)
            .status()?;
        println!("재생 종료: {:?}", status.code());
    }
    Ok(())
}
