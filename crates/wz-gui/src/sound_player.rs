//! GUI 사운드 재생 — WZ `Sound_DX8` 을 추출해 시스템 플레이어로 재생.
//!
//! 코덱/포맷 해석은 `wz-core::sound` 가 담당하고, 여기서는 임시 파일과
//! 프로세스 수명만 관리한다.

use std::path::{Path, PathBuf};

use crate::state::AppState;
use wz_core::image::ImageValue;
use wz_core::{extract_sound, WzArchive};

fn open_sound(
    wz: &Path,
    sound_path: &str,
) -> Result<wz_core::ExtractedSound, Box<dyn std::error::Error>> {
    let archive = WzArchive::open(wz)?;
    let tree = archive.read_tree()?;
    let clean = sound_path.trim_matches('/');
    let (img, sub) = clean
        .split_once('/')
        .ok_or("사운드 경로는 '<이미지>.img/<이름>' 형식이어야 합니다")?;
    let img_name = if img.ends_with(".img") {
        img.to_string()
    } else {
        format!("{img}.img")
    };
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some(img_name.as_str()))
        .ok_or_else(|| format!("이미지 없음: {img_name}"))?;
    let node = archive.parse_image_at(entry.offset.ok_or("이미지 오프셋 없음")?)?;
    let mut cur = &node;
    for segment in sub.split('/') {
        cur = cur
            .object_children()
            .iter()
            .find(|c| c.name.as_ref() == segment)
            .ok_or_else(|| format!("사운드 없음: {sound_path}"))?;
    }
    let Some(ImageValue::Sound(sound)) = &cur.value else {
        return Err(format!("Sound_DX8 아님: {sound_path}").into());
    };
    Ok(extract_sound(archive.source(), sound)?)
}

fn find_sound_archive(data_root: &Path, image_name: &str) -> Option<PathBuf> {
    let base = data_root.join("Sound");
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        if tree
            .root
            .entries
            .iter()
            .any(|e| e.name.as_deref() == Some(image_name))
        {
            return Some(path);
        }
    }
    None
}

fn play_extracted(
    state: &mut AppState,
    extracted: &wz_core::ExtractedSound,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(mut child) = state.sound_child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "wz-gui-sound-{}-{}.{}",
        std::process::id(),
        stamp,
        extracted.extension
    ));
    std::fs::write(&path, &extracted.bytes)?;
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("afplay");
        c.arg(&path);
        c
    };
    #[cfg(not(target_os = "macos"))]
    let mut command = {
        let mut c = std::process::Command::new("ffplay");
        c.args(["-nodisp", "-autoexit"]).arg(&path);
        c
    };
    let child = command.spawn()?;
    state.sound_child = Some(child);
    state.sound_status = format!(
        "재생 중: {label} ({:?}, {} bytes)",
        extracted.codec,
        extracted.bytes.len()
    );
    Ok(())
}

/// 지정한 Sound WZ 의 사운드를 재생한다.
pub fn play_from_archive(
    state: &mut AppState,
    wz: &Path,
    sound_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let extracted = open_sound(wz, sound_path)?;
    play_extracted(state, &extracted, sound_path)
}

/// 현재 맵의 `info/bgm` 을 Data/Sound 에서 찾아 재생한다.
pub fn play_map_bgm(state: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
    let Some(map) = &state.current_map else {
        return Err("맵이 로드되지 않았습니다".into());
    };
    let Some(bgm) = &map.info.bgm else {
        return Err("맵에 bgm 이 없습니다".into());
    };
    if bgm.is_empty() {
        return Err("맵에 bgm 이 없습니다".into());
    }
    let (image, sub) = bgm.split_once('/').ok_or("bgm 경로 형식이 아닙니다")?;
    let image_name = if image.ends_with(".img") {
        image.to_string()
    } else {
        format!("{image}.img")
    };
    let root = state
        .data_root
        .clone()
        .ok_or("Data 루트를 찾을 수 없습니다")?;
    let wz = find_sound_archive(&root, &image_name)
        .ok_or_else(|| format!("Sound WZ 에서 {image_name} 을 찾을 수 없습니다"))?;
    play_from_archive(state, &wz, &format!("{image_name}/{sub}"))
}

/// 현재 재생 프로세스를 중지한다.
pub fn stop(state: &mut AppState) {
    if let Some(mut child) = state.sound_child.take() {
        let _ = child.kill();
        let _ = child.wait();
        state.sound_status = "정지".into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_bgm_archive_and_extracts() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(PathBuf::from) else {
            return;
        };
        let Some(wz) = find_sound_archive(&root, "Bgm00.img") else {
            return;
        };
        let extracted = open_sound(&wz, "Bgm00.img/DragonDream").expect("extract");
        assert_eq!(extracted.codec, wz_core::SoundCodec::Mp3);
        assert_eq!(extracted.extension, "mp3");
        assert_eq!(extracted.bytes.len(), 1_261_192);
        assert_eq!(
            extracted.format.as_ref().map(|f| f.samples_per_sec),
            Some(22050)
        );
    }
}
