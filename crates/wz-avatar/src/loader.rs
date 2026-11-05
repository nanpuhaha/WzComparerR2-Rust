//! WZ → Avatar 로더.
//!
//! `Character/_Canvas/_Canvas_000.wz` 에서 첫 번째 이미지의 첫 프레임을 파싱해
//! 모든 파트 (body, head, hair, arm, ...) 를 로드한다.
//!
//! WZ 포맷은 origin/anchor/z/map 프로퍼티를 저장하지 않으므로,
//! 앵커/프리즘/zmap 정밀도는 C# CharaSim 참조 구현이 필요하다.
//! 현재 로더는 PoC 수준: 이미지 캔버스를 로드해 SkinLayer (width/height) +
//! Bone + ZMap + AvatarScene 을 조립하고 렌더한다.
//!
//! z-tag 는 파트 이름에서 추론한다 (body→"body", head→"head", hair→"hair",
//! arm/lHand/rHand→"arm"). zmap 순서는 MapleStory 표준 레이어 순서를 사용한다.

use std::sync::Arc;

use wz_core::image::Node as ImageNode;
use wz_core::WzArchive;
use wz_image::decode_png_ref_to_rgba;

use crate::{AvatarScene, Bone, ZMap};

use crate::zmap::{part_z_tag, STANDARD_ZMAP};

/// MapleStory 표준 z-map 순서 (Base\zmap.img 에서 로드).
/// 낮은 인덱스가 먼저 그려진다 (painter's algorithm).
/// 로드된 아바타 파트.
#[derive(Debug)]
pub struct AvatarPart {
    pub slot: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl AvatarPart {
    /// 호출자가 제공한 프리즘을 이 파트의 RGBA 에 적용한다.
    ///
    /// C# 에서도 `PrismData` 는 WZ 가 아니라 UI/호출자가 들고 있다
    /// (`AvatarPart.PrismData` 는 외부에서 설정된다). 로더는 픽셀만 만들고,
    /// 색 변환은 이 메서드로 분리한다.
    pub fn apply_prism(&mut self, data: &crate::PrismData) {
        if data.is_valid() {
            crate::apply_prism_rgba8(&mut self.rgba, data, false);
        }
    }
}

/// 로드 결과.
pub struct LoadedAvatar {
    pub parts: Vec<AvatarPart>,
    pub root_bone: Bone,
    pub zmap: Arc<ZMap>,
    pub scene: AvatarScene,
}

/// `Base/zmap.img` 에서 z-order 를 읽는다. 없으면 `None` (호출부가 표준 순서로 폴백).
fn load_zmap_order(data_root: Option<&std::path::Path>) -> Option<Vec<String>> {
    use wz_core::image::ImageValue;

    let root = data_root?;
    let base = root.join("Base");
    let candidates = [base.join("Base_000.wz"), base.join("Base.wz")];
    let path = candidates.iter().find(|p| p.exists())?;
    let archive = WzArchive::open(path).ok()?;
    let tree = archive.read_tree().ok()?;
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some("zmap.img"))?;
    let node = archive.parse_image_at(entry.offset?).ok()?;

    let mut order: Vec<String> = node.children.iter().map(|c| c.name.to_string()).collect();
    if let Some(ImageValue::Property(p)) = &node.value {
        order.extend(p.children.iter().map(|c| c.name.to_string()));
    }
    let mut seen = std::collections::HashSet::new();
    order.retain(|name| !name.is_empty() && seen.insert(name.clone()));
    if order.is_empty() {
        None
    } else {
        Some(order)
    }
}

/// 한 이미지의 액션 프레임을 [`crate::collect_frame_assembly`] 로 조립한다.
///
/// 기존 `loader.rs` 가 파트마다 반복하던 `origin`/`map`/bone 구성을
/// `frame_layers::collect_frame_assembly` 한 곳으로 모은다.
fn load_frame_parts(
    archive: &WzArchive,
    image_name: &str,
    action_names: &[&str],
) -> Option<(Bone, Vec<AvatarPart>)> {
    use wz_core::image::ImageValue;

    let tree = archive.read_tree().ok()?;
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.node_type == 4 && e.name.as_deref() == Some(image_name))?;
    let node = archive.parse_image_at(entry.offset?).ok()?;
    let action = action_names
        .iter()
        .find_map(|name| node.children.iter().find(|c| c.name.as_ref() == *name))
        .or_else(|| node.children.first())?;
    let action_children: &[ImageNode] = match &action.value {
        Some(ImageValue::Property(p)) => &p.children,
        _ => &action.children,
    };
    let frame = if action_children
        .iter()
        .any(|c| matches!(c.value, Some(ImageValue::Png(_))))
    {
        // `front` 처럼 액션 노드 자체가 프레임이고 자식이 Canvas 인 경우.
        action
    } else {
        // `stand1/0` 처럼 첫 자식이 프레임인 경우.
        action_children.first().unwrap_or(action)
    };

    let assembly = crate::collect_frame_assembly(frame, archive, Some(archive)).ok()?;
    let parts = assembly
        .layers
        .iter()
        .map(|layer| AvatarPart {
            slot: layer.name.clone(),
            width: layer.canvas.w,
            height: layer.canvas.h,
            rgba: layer.canvas.rgba.clone(),
        })
        .collect();
    Some((assembly.root_bone, parts))
}

/// 두 bone 트리를 합친다. 같은 이름의 bone 은 스킨을 합치고, 없으면 자식으로 붙인다.
fn merge_bone_tree(dst: &mut Bone, src: Bone) {
    dst.skins.extend(src.skins);
    for child in src.children {
        if let Some(existing) = dst.find_child_mut(&child.name) {
            merge_bone_tree(existing, child);
        } else {
            dst.children.push(child);
        }
    }
}

/// `_Canvas` 아이템 이미지 하나를 `collect_frame_assembly` 로 조립한다.
///
/// `image_name` 이 `None` 이면 아카이브의 첫 이미지를 쓴다. 장비/무기 이미지는
/// `default`/`stand1`/`walk1` 액션을 가지며, 액션 자체가 프레임인 경우와
/// `0` 같은 자식이 프레임인 경우를 모두 처리한다.
fn load_item_frame(
    archive: &WzArchive,
    image_name: Option<&str>,
    action_names: &[&str],
    slot: &str,
) -> Option<(Bone, Vec<AvatarPart>)> {
    use wz_core::image::ImageValue;

    let tree = archive.read_tree().ok()?;
    let entry = match image_name {
        Some(name) => tree
            .root
            .entries
            .iter()
            .find(|e| e.node_type == 4 && e.name.as_deref() == Some(name)),
        None => tree.root.entries.iter().find(|e| e.node_type == 4),
    }?;
    let node = archive.parse_image_at(entry.offset?).ok()?;
    let action = action_names
        .iter()
        .find_map(|name| node.children.iter().find(|c| c.name.as_ref() == *name))
        .or_else(|| node.children.iter().find(|c| c.name.as_ref() == "default"))
        .or_else(|| node.children.first())
        .unwrap_or(&node);
    let action_children: &[ImageNode] = match &action.value {
        Some(ImageValue::Property(p)) => &p.children,
        _ => &action.children,
    };
    let frame = if action_children
        .iter()
        .any(|c| matches!(c.value, Some(ImageValue::Png(_))))
    {
        action
    } else {
        action_children.first().unwrap_or(action)
    };

    let assembly = crate::collect_frame_assembly(frame, archive, Some(archive)).ok()?;
    let base_slot = slot.to_lowercase();
    let parts = assembly
        .layers
        .iter()
        .enumerate()
        .map(|(i, layer)| {
            let name = if i == 0 {
                base_slot.clone()
            } else {
                format!("{base_slot}_{}", layer.name)
            };
            AvatarPart {
                slot: name,
                width: layer.canvas.w,
                height: layer.canvas.h,
                rgba: layer.canvas.rgba.clone(),
            }
        })
        .collect();
    Some((assembly.root_bone, parts))
}

/// `Character/<dir>/_Canvas/_Canvas_000.wz` 를 열어 아이템 이미지를 조립한다.
fn load_item_canvas(
    char_root: &std::path::Path,
    dir: &str,
    image_name: &str,
    action_names: &[&str],
) -> Option<(Bone, Vec<AvatarPart>)> {
    let path = char_root.join(dir).join("_Canvas").join("_Canvas_000.wz");
    if !path.exists() {
        return None;
    }
    let archive = WzArchive::open(&path).ok()?;
    load_item_frame(&archive, Some(image_name), action_names, dir)
}

/// `_Canvas` 아카이브의 첫 이미지를 조립한다 (PoC 장비/무기 슬롯).
fn load_first_item_canvas(
    char_root: &std::path::Path,
    dir: &str,
    action_names: &[&str],
) -> Option<(Bone, Vec<AvatarPart>)> {
    let path = char_root.join(dir).join("_Canvas").join("_Canvas_000.wz");
    if !path.exists() {
        return None;
    }
    let archive = WzArchive::open(&path).ok()?;
    load_item_frame(&archive, None, action_names, dir)
}

pub fn load_from_character_wz(char_wz_path: &std::path::Path) -> Option<LoadedAvatar> {
    load_from_character_wz_with_skin(char_wz_path, 2000)
}

pub fn load_from_character_wz_with_skin(
    char_wz_path: &std::path::Path,
    body_skin: i32,
) -> Option<LoadedAvatar> {
    let data_root = char_wz_path
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());
    let canvas_path = char_wz_path
        .parent()?
        .join("_Canvas")
        .join("_Canvas_000.wz");
    let arch = WzArchive::open(&canvas_path).ok()?;

    let (mut root, mut parts) = load_frame_parts(
        &arch,
        &format!("{:08}.img", body_skin),
        &["stand1", "alert"],
    )?;

    if let Some((head_root, head_parts)) =
        load_frame_parts(&arch, "00012000.img", &["front", "back"])
    {
        root.skins.extend(head_root.skins);
        root.children.extend(head_root.children);
        parts.extend(head_parts);
    }

    // 3. 장비 아이템 로딩 (Cap 1000000)
    let char_root = char_wz_path.parent().unwrap_or(char_wz_path);
    if let Some((item_root, item_parts)) =
        load_item_canvas(char_root, "Cap", "01000000.img", &["default", "stand1"])
    {
        merge_bone_tree(&mut root, item_root);
        parts.extend(item_parts);
    }

    // 4. 추가 장비 슬롯 로딩 (각 슬롯의 첫 번째 아이템)
    for slot_name in ["Coat", "Pants", "Shoes", "Glove", "Cape"] {
        if let Some((item_root, item_parts)) =
            load_first_item_canvas(char_root, slot_name, &["default", "stand1"])
        {
            merge_bone_tree(&mut root, item_root);
            parts.extend(item_parts);
        }
    }

    // 5. Weapon 슬롯 로딩 (첫 아이템)
    if let Some((weapon_root, weapon_parts)) =
        load_first_item_canvas(char_root, "Weapon", &["stand1", "walk1", "default"])
    {
        merge_bone_tree(&mut root, weapon_root);
        parts.extend(weapon_parts);
    }

    if parts.is_empty() {
        return None;
    }

    // `Base/zmap.img` 가 있으면 그 순서를, 없으면 표준 MapleStory 순서를 쓴다.
    let mut zmap_order: Vec<String> = load_zmap_order(data_root.as_deref())
        .unwrap_or_else(|| STANDARD_ZMAP.iter().map(|s| s.to_string()).collect());
    // 로드된 파트의 z-tag 가 표준에 없으면 추가
    for p in &parts {
        let zt = part_z_tag(&p.slot);
        if !zmap_order.contains(&zt) {
            zmap_order.push(zt);
        }
    }
    let zmap = Arc::new(ZMap::new(zmap_order));
    let scene = AvatarScene::new(zmap.clone());

    Some(LoadedAvatar {
        parts,
        root_bone: root,
        zmap,
        scene,
    })
}

/// 장비 슬롯 → 디렉토리 이름 매핑.
pub fn slot_dir_name(slot: &str) -> Option<&'static str> {
    match slot {
        "Cap" => Some("Cap"),
        "Coat" => Some("Coat"),
        "Longcoat" => Some("Longcoat"),
        "Pants" => Some("Pants"),
        "Shoes" => Some("Shoes"),
        "Glove" => Some("Glove"),
        "Cape" => Some("Cape"),
        "Weapon" => Some("Weapon"),
        "Shield" | "SubWeapon" => Some("Shield"),
        "FaceAccessory" => Some("Accessory"),
        _ => None,
    }
}

/// 아이템 ID → 슬롯 이름 추론 (MapleStory 8자리 ID 규칙).
pub fn item_id_to_slot(id: i32) -> Option<&'static str> {
    let prefix = id / 100000;
    match prefix {
        100 => Some("Cap"),
        104 => Some("Coat"),
        105 => Some("Longcoat"),
        106 => Some("Pants"),
        107 => Some("Shoes"),
        108 => Some("Glove"),
        109 => Some("Shield"),
        110 => Some("Cape"),
        130 | 131 | 132 | 133 | 134 | 136 | 137 | 138 | 140..=159 => Some("Weapon"),
        _ => None,
    }
}

/// 장비 아이템 이미지에서 첫 번째 Canvas 를 추출한다.
/// WZ 구조: item.img → action (default/backDefault) → [frame] → Canvas
/// 장비 아이템을 로드하여 (slot_name, width, height, rgba) 를 반환한다.
/// wz_path: Character/{Slot}/_Canvas/_Canvas_000.wz 경로
/// item_id: 장비 아이템 ID (8자리)
pub fn load_equipment_item(
    wz_path: &std::path::Path,
    item_id: i32,
) -> Option<(String, u32, u32, Vec<u8>)> {
    use wz_core::WzArchive;
    let arch = WzArchive::open(wz_path).ok()?;
    let tree = arch.read_tree().ok()?;
    let file_name = format!("{:08}.img", item_id);
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.node_type == 4 && e.name.as_deref() == Some(file_name.as_str()))?;
    let off = entry.offset?;
    let node = arch.parse_image_at(off).ok()?;
    let png = node.first_canvas()?;
    let file_src = wz_pool::FileSource::open(wz_path).ok()?;
    let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
    let (w, h, rgba) = decode_png_ref_to_rgba(&png, &src).ok()?;
    let slot = item_id_to_slot(item_id).unwrap_or("Unknown").to_string();
    Some((slot, w, h, rgba))
}

/// 장비 아이템 ID 목록으로 아바타를 로드한다.
/// data_root: MapleStory Data 디렉터리 경로
/// equipment: (slot_name, item_id) 쌍의 목록
pub fn load_avatar_with_equipment(
    data_root: &std::path::Path,
    body_skin: i32,
    equipment: &[(String, i32)],
) -> Option<LoadedAvatar> {
    // Body 로드 (0000{skin}00.img)
    let body_dir = data_root.join("Character");
    let canvas_dir = body_dir.join("_Canvas");
    let canvas_path = canvas_dir.join("_Canvas_000.wz");
    if !canvas_path.exists() {
        return None;
    }

    let arch = WzArchive::open(&canvas_path).ok()?;

    let (mut root, mut parts) = load_frame_parts(
        &arch,
        &format!("{:08}.img", body_skin),
        &["stand1", "alert"],
    )?;

    if let Some((head_root, head_parts)) =
        load_frame_parts(&arch, "00012000.img", &["front", "back"])
    {
        root.skins.extend(head_root.skins);
        root.children.extend(head_root.children);
        parts.extend(head_parts);
    }

    // Equipment 로드
    let char_root = data_root.join("Character");
    for (slot, item_id) in equipment {
        if let Some(dir) = slot_dir_name(slot) {
            let image_name = format!("{:08}.img", item_id);
            if let Some((item_root, item_parts)) =
                load_item_canvas(&char_root, dir, &image_name, &["default", "stand1"])
            {
                merge_bone_tree(&mut root, item_root);
                parts.extend(item_parts);
            }
        }
    }

    if parts.is_empty() {
        return None;
    }

    // `Base/zmap.img` 가 있으면 그 순서를, 없으면 표준 MapleStory 순서를 쓴다.
    let mut zmap_order: Vec<String> = load_zmap_order(Some(data_root))
        .unwrap_or_else(|| STANDARD_ZMAP.iter().map(|s| s.to_string()).collect());
    for p in &parts {
        let zt = part_z_tag(&p.slot);
        if !zmap_order.contains(&zt) {
            zmap_order.push(zt);
        }
    }
    let zmap = std::sync::Arc::new(ZMap::new(zmap_order));
    let scene = AvatarScene::new(zmap.clone());

    Some(LoadedAvatar {
        parts,
        root_bone: root,
        zmap,
        scene,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 실제 게임 데이터가 있는 환경에서만 도는 테스트용 루트.
    ///
    /// `WZ_DATA_ROOT`(예: `.../Nexon/Maple/Data`)를 설정한 경우에만 테스트가
    /// 실제 파일을 읽고, 없으면 조용히 skip 한다. 소스에 사용자 경로를 넣지 않는다.
    fn wz_data_root() -> Option<std::path::PathBuf> {
        std::env::var_os("WZ_DATA_ROOT")
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists())
    }

    #[test]
    fn loader_returns_parts_on_real_character_wz() {
        let Some(root) = wz_data_root() else { return };
        let char_wz = root.join("Character").join("Character.wz");
        if !char_wz.exists() {
            return;
        }
        let av = load_from_character_wz(&char_wz)
            .expect("loader should return Some on real Character.wz");
        assert!(!av.parts.is_empty(), "at least one part loaded");
        for p in &av.parts {
            assert!(p.width > 0 && p.height > 0);
            assert_eq!(p.rgba.len(), (p.width * p.height * 4) as usize);
        }
        let batch = av.scene.build_batch(&av.root_bone);
        assert!(batch.layer_count > 0);
        assert!(batch.vertex_count() > 0);

        // z-tag 검증: body/head/hair/arm 중 하나 이상 있어야 한다
        // (map 이 없는 스킨은 root bone 에 직접 붙을 수 있으므로 root 도 포함한다.)
        let z_tags: Vec<&str> = std::iter::once(&av.root_bone)
            .chain(av.root_bone.children.iter())
            .flat_map(|b| b.skins.iter().map(|s| s.z.as_str()))
            .collect();
        assert!(!z_tags.is_empty(), "at least one z-tag assigned");
    }

    #[test]
    fn z_tag_mapping() {
        assert_eq!(part_z_tag("body"), "body");
        assert_eq!(part_z_tag("head"), "head");
        assert_eq!(part_z_tag("hair"), "hair");
        assert_eq!(part_z_tag("arm"), "arm");
        assert_eq!(part_z_tag("lHand"), "arm");
        assert_eq!(part_z_tag("rHand"), "arm");
    }

    #[test]
    fn load_avatar_with_equipment_custom_ids() {
        let Some(data_root) = wz_data_root() else {
            return;
        };
        // Load with specific equipment IDs
        let equipment = vec![
            ("Cap".to_string(), 1000002),
            ("Coat".to_string(), 1040002),
            ("Pants".to_string(), 1060000),
            ("Shoes".to_string(), 1070000),
            ("Glove".to_string(), 1080000),
            ("Cape".to_string(), 1102000),
        ];
        let av = load_avatar_with_equipment(&data_root, 2000, &equipment);
        if let Some(av) = av {
            // Should have body + head + 6 equipment = at least 8 parts
            assert!(
                av.parts.len() >= 8,
                "expected >= 8 parts, got {}",
                av.parts.len()
            );
            let batch = av.scene.build_batch(&av.root_bone);
            assert!(batch.layer_count > 0);
            assert!(batch.vertex_count() > 0);
        }
    }

    #[test]
    fn load_avatar_with_equipment_works() {
        let Some(data_root) = wz_data_root() else {
            return;
        };
        let equipment = vec![("Cap".to_string(), 1000000), ("Coat".to_string(), 1040000)];
        let av = load_avatar_with_equipment(&data_root, 2000, &equipment);
        if let Some(av) = av {
            assert!(!av.parts.is_empty(), "at least one part loaded");
            // body + head + cap + coat
            assert!(
                av.parts.len() >= 4,
                "expected >= 4 parts, got {}",
                av.parts.len()
            );
            let batch = av.scene.build_batch(&av.root_bone);
            assert!(batch.layer_count > 0);
            assert!(av.parts.iter().any(|p| p.slot == "cap"), "cap missing");
            assert!(av.parts.iter().any(|p| p.slot == "coat"), "coat missing");
        }
    }

    #[test]
    fn load_equipment_item_works() {
        let Some(data_root) = wz_data_root() else {
            return;
        };
        // Cap item 1000000
        let cap_path = data_root.join("Cap").join("_Canvas").join("_Canvas_000.wz");
        if cap_path.exists() {
            let result = load_equipment_item(&cap_path, 1000000);
            assert!(result.is_some(), "cap item should load");
            let (slot, w, h, rgba) = result.unwrap();
            assert_eq!(slot, "Cap");
            assert!(w > 0 && h > 0);
            assert_eq!(rgba.len(), (w * h * 4) as usize);
        }
        // Coat item 1040000
        let coat_path = data_root
            .join("Coat")
            .join("_Canvas")
            .join("_Canvas_000.wz");
        if coat_path.exists() {
            let result = load_equipment_item(&coat_path, 1040000);
            assert!(result.is_some(), "coat item should load");
            let (slot, w, h, _rgba) = result.unwrap();
            assert_eq!(slot, "Coat");
            assert!(w > 0 && h > 0);
        }
    }

    #[test]
    fn loader_loads_expected_parts() {
        let Some(root) = wz_data_root() else { return };
        let char_wz = root.join("Character").join("Character.wz");
        if !char_wz.exists() {
            return;
        }
        let av = load_from_character_wz(&char_wz)
            .expect("loader should return Some on real Character.wz");
        // 최소 8 파트 (body, arm, head, ear variants, cap)
        assert!(
            av.parts.len() >= 14,
            "expected >= 14 parts, got {}",
            av.parts.len()
        );
        // body 파트가 있어야 한다
        assert!(
            av.parts.iter().any(|p| p.slot == "body"),
            "body part missing"
        );
        // head 파트가 있어야 한다
        assert!(
            av.parts.iter().any(|p| p.slot == "head"),
            "head part missing"
        );
        // cap 파트가 있어야 한다
        assert!(av.parts.iter().any(|p| p.slot == "cap"), "cap part missing");
        // zmap 이 표준 순서를 포함해야 한다
        assert!(av.zmap.index_of("body").is_some(), "zmap missing body");
        assert!(av.zmap.index_of("head").is_some(), "zmap missing head");
        // batch 렌더링이 동작해야 한다
        let batch = av.scene.build_batch(&av.root_bone);
        assert!(
            batch.layer_count >= av.parts.len(),
            "layer_count < parts count"
        );
        assert!(batch.vertex_count() > 0, "no vertices generated");
    }
}

#[cfg(test)]
mod prism_tests {
    use super::*;

    #[test]
    fn avatar_part_applies_caller_supplied_prism() {
        let mut part = AvatarPart {
            slot: "cap".into(),
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 255],
        };
        let prism = crate::PrismData::new(0, 120, 100, 100);
        part.apply_prism(&prism);
        assert_ne!(part.rgba, vec![255, 0, 0, 255]);
    }
}
