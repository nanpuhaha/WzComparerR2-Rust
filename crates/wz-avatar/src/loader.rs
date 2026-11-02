//! WZ → Avatar 로더.
//!
//! `Character/_Canvas/_Canvas_000.wz` 에서 첫 번째 이미지의 첫 프레임을 파싱해
//! 모든 파트 (body, head, hair, arm, ...) 를 로드한다.
//!
//! WZ 포맷은 origin/anchor/z/map 프로퍼티를 저장하지 않으므로,
//! 앵커/프리즘/zmap 정밀도는 C# CharaSim 참조 구현이 필요하다.
//! 현재 로더는 PoC 수준: 이미지 캔버스를 로드해 SkinLayer (width/height) +
//! Bone + ZMap + AvatarScene 을 조립하고 렌더한다.

use std::sync::Arc;

use wz_core::WzArchive;
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



pub fn load_from_character_wz(char_wz_path: &std::path::Path) -> Option<LoadedAvatar> {
    // Character.wz 는 컨테이너 인덱스다. 실제 이미지는 _Canvas/_Canvas_000.wz 에 있다.
    let canvas_path = char_wz_path.parent()?.join("_Canvas").join("_Canvas_000.wz");
    let arch = WzArchive::open(&canvas_path).ok()?;
    let tree = arch.read_tree().ok()?;

    let img_entry = tree.root.entries.iter().find(|e| e.node_type == 4)?;
    let off = img_entry.offset?;
    let node = arch.parse_image_at(off).ok()?;

    // 첫 액션 찾기
    let action = node.children.first().or_else(|| {
        if let Some(wz_core::image::ImageValue::Property(p)) = &node.value {
            p.children.first()
        } else { None }
    })?;
    // 첫 프레임
    let frame = action.children.first().or_else(|| {
        if let Some(wz_core::image::ImageValue::Property(p)) = &action.value {
            p.children.first()
        } else { None }
    })?;

    // 프레임의 모든 파트 (body, head, hair, arm, etc.) 로드
    let mut part_nodes: Vec<&ImageNode> = frame.children.iter().collect();
    if let Some(wz_core::image::ImageValue::Property(p)) = &frame.value {
        part_nodes.extend(p.children.iter());
    }

    let mut parts: Vec<AvatarPart> = Vec::new();
    let mut root = Bone::new("@root");

    for pn in part_nodes {
        fn find_png(n: &ImageNode) -> Option<wz_core::image::PngRef> {
            use wz_core::image::ImageValue;
            if let Some(ImageValue::Png(p)) = &n.value { return Some(p.clone()); }
            for c in &n.children { if let Some(p) = find_png(c) { return Some(p); } }
            if let Some(ImageValue::Property(p)) = &n.value { if let Some(res) = find_png(p) { return Some(res); } }
            None
        }
        let Some(pr) = find_png(pn) else { continue; };
        let file_src = wz_pool::FileSource::open(&canvas_path).ok()?;
        let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
        let Ok((w, h, rgba)) = decode_png_ref_to_rgba(&pr, &src) else { continue; };

        let slot_name = pn.name.to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_returns_parts_on_real_character_wz() {
        let char_wz = "<WZ_DATA_ROOT>/Character/Character.wz";
        if !std::path::Path::new(char_wz).exists() {
            return;
        }
        let av = load_from_character_wz(std::path::Path::new(char_wz))
            .expect("loader should return Some on real Character.wz");
        assert!(!av.parts.is_empty(), "at least one part loaded");
        for p in &av.parts {
            assert!(p.width > 0 && p.height > 0);
            assert_eq!(p.rgba.len(), (p.width * p.height * 4) as usize);
        }
        let batch = av.scene.build_batch(&av.root_bone);
        assert!(batch.layer_count > 0);
        assert!(batch.vertex_count() > 0);
    }
}
