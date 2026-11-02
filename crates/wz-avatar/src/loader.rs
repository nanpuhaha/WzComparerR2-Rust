//! WZ → Avatar 로더.
//!
//! `Character.wz` 의 슬롯 디렉터리(Body, Cap, Hair, ...) 에서 첫 번째
//! `.img` 를 찾아 이미지를 디코딩하고, `Bone`/`SkinLayer`/`ZMap` 으로
//! 조립한다. GUI 아바타 패널과 CLI `cmd_avatar` 가 공유한다.

use std::sync::Arc;

use wz_core::WzArchive;
use wz_core::dirtree::node_type as nt;
use wz_core::image::Node as ImageNode;
use wz_image::decode_png_ref_to_rgba;

use crate::{AvatarScene, Bone, SkinLayer, ZMap};

/// 로드된 아바타 파트.
#[derive(Debug)]
pub struct AvatarPart {
    pub slot: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// 로드 결과.
pub struct LoadedAvatar {
    pub parts: Vec<AvatarPart>,
    pub root_bone: Bone,
    pub zmap: Arc<ZMap>,
    pub scene: AvatarScene,
}

/// `Character.wz` 에서 아바타 파트를 로드한다.
///
/// 각 슬롯 디렉터리의 **첫 번째** `.img` 만 로드한다 (PoC).
/// 반환값이 `None` 이면 WZ 를 열 수 없거나 Body 이미지가 없다.
pub fn load_from_character_wz(char_wz_path: &std::path::Path) -> Option<LoadedAvatar> {
    // Character.wz 는 438 바이트짜리 컨테이너 인덱스다. 실제 이미지는
    // Character/_Canvas/_Canvas_000.wz 에 있다.
    let canvas_path = char_wz_path.parent()?.join("_Canvas").join("_Canvas_000.wz");
    let arch = WzArchive::open(&canvas_path).ok()?;
    let tree = arch.read_tree().ok()?;

    let mut parts: Vec<AvatarPart> = Vec::new();
    let mut root = Bone::new("@root");

    // _Canvas_000.wz 의 처음 5개 이미지를 슬롯으로 로드한다 (PoC).
    for (i, img_entry) in tree.root.entries.iter().filter(|e| e.node_type == nt::IMAGE).take(5).enumerate() {
        let Some(off) = img_entry.offset else { continue; };
        let Ok(node) = arch.parse_image_at(off) else { continue; };
        let slot_name = format!("Slot{}", i);

        fn find_png(n: &ImageNode) -> Option<wz_core::image::PngRef> {
            use wz_core::image::ImageValue;
            if let Some(ImageValue::Png(p)) = &n.value { return Some(p.clone()); }
            for c in &n.children { if let Some(p) = find_png(c) { return Some(p); } }
            if let Some(ImageValue::Property(p)) = &n.value { if let Some(res) = find_png(p) { return Some(res); } }
            None
        }
        let Some(pr) = find_png(&node) else { continue; };
        let file_src = wz_pool::FileSource::open(&canvas_path).ok()?;
        let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
        let Ok((w, h, rgba)) = decode_png_ref_to_rgba(&pr, &src) else { continue; };

        let mut skin = SkinLayer::new(slot_name.clone());
        skin.z = slot_name.clone();
        skin.width = w;
        skin.height = h;
        root.append_bone(skin, "navel", crate::Point::new(0, 0));

        parts.push(AvatarPart { slot: slot_name, width: w, height: h, rgba });
    }

    if parts.is_empty() {
        return None;
    }

    let zmap_order: Vec<String> = parts.iter().map(|p| p.slot.clone()).collect();
    let zmap = Arc::new(ZMap::new(zmap_order));
    let scene = AvatarScene::new(zmap.clone());

    Some(LoadedAvatar { parts, root_bone: root, zmap, scene })
}
