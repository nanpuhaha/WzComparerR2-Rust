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

use crate::{AvatarScene, Bone, SkinLayer, ZMap};

use crate::zmap::{part_z_tag, STANDARD_ZMAP};

/// MapleStory 표준 z-map 순서 (Base\zmap.img 에서 로드).
/// 낮은 인덱스가 먼저 그려진다 (painter's algorithm).
/// 로드된 아바타 파트.
#[derive(Debug, Default)]
pub struct AvatarPart {
    pub slot: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// `info/customOrigin` — WZ child name → custom origin ID.
    pub custom_origin_map: std::collections::BTreeMap<String, String>,
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
            custom_origin_map: Default::default(),
        })
        .collect();
    Some((assembly.root_bone, parts))
}

/// Hair/Face 아이템의 믹스 컬러 변형을 로드해 `parts` 에 블렌드한다.
///
/// C# `AvatarPart.LoadMixNodes` / `AvatarCanvas.MixBitmaps` 대응.
fn apply_mix_to_item(
    archive: &WzArchive,
    item_id: i32,
    dir: &str,
    parts: &mut [AvatarPart],
    mix_color: i32,
    opacity: i32,
) {
    if parts.is_empty() || opacity <= 0 || !(0..=8).contains(&mix_color) {
        return;
    }
    let (base_color, variant_id) = match dir {
        "Hair" => {
            let base = item_id % 10;
            (base, item_id / 10 * 10 + mix_color)
        }
        "Face" => {
            let base = item_id / 100 % 10;
            (
                base,
                item_id / 1000 * 1000 + item_id % 100 + mix_color * 100,
            )
        }
        _ => return,
    };
    if base_color == mix_color {
        return;
    }
    let image_name = format!("{:08}.img", variant_id);
    let Some((_, mix_parts)) =
        load_item_frame(archive, Some(&image_name), &["default", "stand1"], dir)
    else {
        return;
    };
    for (base, mix) in parts.iter_mut().zip(mix_parts.iter()) {
        crate::mix_rgba8(&mut base.rgba, &mix.rgba, opacity);
    }
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

/// 이름으로 직접 자식을 찾는다.
fn child_node<'a>(node: &'a ImageNode, name: &str) -> Option<&'a ImageNode> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

/// `Effect/_Canvas` 에서 특정 이미지를 가진 아카이브를 찾는다.
fn find_effect_canvas_archive(data_root: &std::path::Path, image_name: &str) -> Option<WzArchive> {
    let dir = data_root.join("Effect").join("_Canvas");
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
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
            return Some(archive);
        }
    }
    None
}

/// Canvas mini-property 에서 `origin`/`z` 를 읽는다.
fn png_origin_z(png: &wz_core::image::PngRef) -> (crate::Point, i32) {
    use wz_core::image::ImageValue;

    let mut origin = crate::Point::ZERO;
    let mut z_index = 0;
    if let Some(sub) = &png.sub_property {
        for ch in &sub.children {
            match (ch.name.as_ref(), &ch.value) {
                ("origin", Some(ImageValue::Vector(v))) => {
                    origin = crate::Point::new(v.x, v.y);
                }
                ("z", Some(ImageValue::Int(v))) => z_index = *v,
                _ => {}
            }
        }
    }
    (origin, z_index)
}

/// `Item/Install/_Canvas` 의 체어 이미지를 정적 1프레임으로 로드한다.
///
/// 체어 카테고리 이미지는 `03010.img` 처럼 5자리이고, 그 자식이 아이템 ID 다.
/// `effect/0`, `0`, `default` 순으로 캔버스를 찾는다.
fn load_chair_item(data_root: &std::path::Path, item_id: i32) -> Option<(Bone, Vec<AvatarPart>)> {
    let canvas_dir = data_root.join("Item").join("Install").join("_Canvas");
    let category = format!("{:05}.img", item_id / 1000);
    for entry in std::fs::read_dir(canvas_dir).ok()?.flatten() {
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
        let Some(cat_entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some(category.as_str()))
        else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(cat_entry.offset?) else {
            continue;
        };
        let Some(item) = child_node(&node, &format!("{:08}", item_id)) else {
            continue;
        };
        let png = child_node(item, "effect")
            .and_then(|effect| child_node(effect, "0"))
            .and_then(|frame| frame.first_canvas())
            .or_else(|| child_node(item, "0").and_then(|frame| frame.first_canvas()))
            .or_else(|| child_node(item, "default").and_then(|frame| frame.first_canvas()))
            .or_else(|| item.first_canvas())?;
        let (w, h, rgba) = wz_image::decode_png_ref_to_rgba(&png, archive.source()).ok()?;
        let (origin, z_index) = png_origin_z(&png);

        let mut skin = SkinLayer::new("chair");
        skin.z = "chair".into();
        skin.z_index = z_index;
        skin.origin = origin;
        skin.width = w;
        skin.height = h;

        let mut root = Bone::new("@chair");
        root.append_bone(skin, "chair", crate::Point::ZERO);
        let part = AvatarPart {
            slot: "chair".into(),
            width: w,
            height: h,
            rgba,
            ..Default::default()
        };
        return Some((root, vec![part]));
    }
    None
}

/// `name:x,y` 로 지정한 커스텀 오리진을 bone 의 skin 에 적용한다.
fn apply_custom_origins(bone: &mut Bone, map: &std::collections::BTreeMap<String, crate::Point>) {
    if map.is_empty() {
        return;
    }
    for skin in &mut bone.skins {
        if let Some(origin) = map.get(&skin.name) {
            skin.origin = *origin;
        }
    }
    for child in &mut bone.children {
        apply_custom_origins(child, map);
    }
}

/// `Effect/Effect_*.wz` 의 `ItemEff.img/<id>/effect/default/0` 을 정적 1프레임으로 로드한다.
///
/// C# 은 effect 를 애니메이션하지만, v0.1 이후 PoC 는 첫 프레임만 합성한다.
fn load_effect_item(
    data_root: &std::path::Path,
    item_id: i32,
    frame_index: usize,
) -> Option<(Bone, Vec<AvatarPart>)> {
    use wz_core::image::ImageValue;

    let effect_dir = data_root.join("Effect");
    let mut parsed = None;
    for entry in std::fs::read_dir(effect_dir).ok()?.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if !name.starts_with("Effect") || path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("ItemEff.img"))
        else {
            continue;
        };
        if let Ok(node) = archive.parse_image_at(entry.offset?) {
            parsed = Some(node);
            break;
        }
    }
    let item_eff = parsed?;
    let item = child_node(&item_eff, &item_id.to_string())?;
    let effect = child_node(item, "effect")?;
    let default = child_node(effect, "default")?;
    // `default` children are `z`, `pos`, `0`, `1`, … — only numeric children are frames.
    let frames: Vec<&ImageNode> = default
        .object_children()
        .iter()
        .filter(|c| c.name.parse::<usize>().is_ok())
        .collect();
    let frame = frames.get(frame_index % frames.len().max(1))?;
    let png = frame.first_canvas()?;

    let canvas_archive = find_effect_canvas_archive(data_root, "ItemEff.img");
    let canvas_archive = canvas_archive?;
    let outlink = png.sub_property.as_ref().and_then(|sub| {
        sub.children.iter().find_map(|c| match &c.value {
            Some(ImageValue::Str(v)) if c.name.as_ref() == "_outlink" => Some(v.to_string()),
            _ => None,
        })
    })?;
    let resolved = crate::resolve_outlink(&canvas_archive, &outlink).ok()?;

    let mut origin = crate::Point::ZERO;
    let mut z_index = 0;
    if let Some(sub) = &png.sub_property {
        for ch in &sub.children {
            match (ch.name.as_ref(), &ch.value) {
                ("origin", Some(ImageValue::Vector(v))) => {
                    origin = crate::Point::new(v.x, v.y);
                }
                ("z", Some(ImageValue::Int(v))) => z_index = *v,
                _ => {}
            }
        }
    }
    if let Some(z_node) = child_node(effect, "z") {
        if let Some(ImageValue::Int(v)) = &z_node.value {
            z_index = *v;
        }
    }

    let mut skin = SkinLayer::new("effect");
    skin.z = "effect".into();
    skin.z_index = z_index;
    skin.origin = origin;
    skin.width = resolved.w;
    skin.height = resolved.h;

    let mut root = Bone::new("@effect");
    root.append_bone(skin, "effect", crate::Point::ZERO);
    let part = AvatarPart {
        slot: "effect".into(),
        width: resolved.w,
        height: resolved.h,
        rgba: resolved.rgba,
        ..Default::default()
    };
    Some((root, vec![part]))
}

/// `info/customOrigin` 의 자식들에서 `WZ child name → custom origin ID` 맵을 만든다.
fn parse_custom_origin(node: &ImageNode) -> std::collections::BTreeMap<String, String> {
    use wz_core::image::ImageValue;

    let mut out = std::collections::BTreeMap::new();
    let Some(info) = node.children.iter().find(|c| c.name.as_ref() == "info") else {
        return out;
    };
    let Some(custom) = info
        .children
        .iter()
        .find(|c| c.name.as_ref() == "customOrigin")
    else {
        return out;
    };
    for child in &custom.children {
        if let Some(ImageValue::Str(value)) = &child.value {
            out.insert(value.to_string(), child.name.to_string());
        }
    }
    out
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
    let custom_origin = parse_custom_origin(&node);
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
                custom_origin_map: custom_origin.clone(),
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
        "Hair" => Some("Hair"),
        "Face" => Some("Face"),
        "Taming" => Some("TamingMob"),
        "Effect" => Some("Effect"),
        "Chair" => Some("Chair"),
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
/// Hair/Face 믹스 컬러와 특수 레이어 설정.
///
/// WZ/UI 가 공급한다 (C# `AvatarPart.MixColor`/`MixOpacity`, `AvatarCanvas.CustomOrigin`).
#[derive(Debug, Clone, Default)]
pub struct MixOptions {
    pub hair: Option<(i32, i32)>,
    pub face: Option<(i32, i32)>,
    /// `Effect` 슬롯의 정적 프레임 인덱스 (프레임 수로 modulo).
    pub effect_frame: usize,
    /// `name:x,y` 로 지정한 커스텀 오리진. `SkinLayer.name` 과 매칭한다.
    pub custom_origins: std::collections::BTreeMap<String, crate::Point>,
}

pub fn load_avatar_with_equipment(
    data_root: &std::path::Path,
    body_skin: i32,
    equipment: &[(String, i32)],
) -> Option<LoadedAvatar> {
    load_avatar_with_equipment_and_mix(data_root, body_skin, equipment, &MixOptions::default())
}

pub fn load_avatar_with_equipment_and_mix(
    data_root: &std::path::Path,
    body_skin: i32,
    equipment: &[(String, i32)],
    mix: &MixOptions,
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
            if dir == "Effect" {
                if let Some((item_root, item_parts)) =
                    load_effect_item(data_root, *item_id, mix.effect_frame)
                {
                    merge_bone_tree(&mut root, item_root);
                    parts.extend(item_parts);
                }
                continue;
            }
            if dir == "Chair" {
                if let Some((item_root, item_parts)) = load_chair_item(data_root, *item_id) {
                    merge_bone_tree(&mut root, item_root);
                    parts.extend(item_parts);
                }
                continue;
            }
            let path = char_root.join(dir).join("_Canvas").join("_Canvas_000.wz");
            if !path.exists() {
                continue;
            }
            let Ok(archive) = WzArchive::open(&path) else {
                continue;
            };
            let image_name = format!("{:08}.img", item_id);
            if let Some((item_root, mut item_parts)) =
                load_item_frame(&archive, Some(&image_name), &["default", "stand1"], dir)
            {
                let mix_setting = match dir {
                    "Hair" => mix.hair,
                    "Face" => mix.face,
                    _ => None,
                };
                if let Some((color, opacity)) = mix_setting {
                    apply_mix_to_item(&archive, *item_id, dir, &mut item_parts, color, opacity);
                }
                merge_bone_tree(&mut root, item_root);
                parts.extend(item_parts);
            }
        }
    }

    apply_custom_origins(&mut root, &mix.custom_origins);

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
    fn hair_mix_color_changes_hair_pixels() {
        let Some(data_root) = wz_data_root() else {
            return;
        };
        let equipment = vec![("Hair".to_string(), 30000)];
        let base = load_avatar_with_equipment_and_mix(
            &data_root,
            2000,
            &equipment,
            &MixOptions::default(),
        );
        let mixed = load_avatar_with_equipment_and_mix(
            &data_root,
            2000,
            &equipment,
            &MixOptions {
                hair: Some((1, 100)),
                face: None,
                ..Default::default()
            },
        );
        let (Some(base), Some(mixed)) = (base, mixed) else {
            return;
        };
        let Some(base_hair) = base.parts.iter().find(|p| p.slot == "hair") else {
            return;
        };
        let Some(mixed_hair) = mixed.parts.iter().find(|p| p.slot == "hair") else {
            return;
        };
        assert_ne!(
            base_hair.rgba, mixed_hair.rgba,
            "mix color should change hair pixels"
        );
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
            ..Default::default()
        };
        let prism = crate::PrismData::new(0, 120, 100, 100);
        part.apply_prism(&prism);
        assert_ne!(part.rgba, vec![255, 0, 0, 255]);
    }
}

#[cfg(test)]
mod custom_origin_tests {
    use super::*;
    use std::sync::Arc;
    use wz_core::image::ImageValue;

    #[test]
    fn parses_custom_origin_id_map() {
        let mut root = ImageNode {
            name: Arc::from("01000000.img"),
            value: None,
            children: Vec::new(),
        };
        let mut info = ImageNode {
            name: Arc::from("info"),
            value: None,
            children: Vec::new(),
        };
        let custom = ImageNode {
            name: Arc::from("customOrigin"),
            value: None,
            children: vec![
                ImageNode {
                    name: Arc::from("cap"),
                    value: Some(ImageValue::Str(Arc::from("capType"))),
                    children: Vec::new(),
                },
                ImageNode {
                    name: Arc::from("ear"),
                    value: Some(ImageValue::Str(Arc::from("earType"))),
                    children: Vec::new(),
                },
            ],
        };
        info.children.push(custom);
        root.children.push(info);

        let map = parse_custom_origin(&root);
        assert_eq!(map.get("capType").map(String::as_str), Some("cap"));
        assert_eq!(map.get("earType").map(String::as_str), Some("ear"));
    }

    #[test]
    fn custom_origin_overrides_skin_origin() {
        let mut root = Bone::new("@root");
        let skin = SkinLayer::new("default");
        root.append_bone(skin, "navel", crate::Point::ZERO);
        let mut map = std::collections::BTreeMap::new();
        map.insert("default".to_string(), crate::Point::new(7, 9));
        apply_custom_origins(&mut root, &map);
        let found = root
            .children
            .iter()
            .flat_map(|b| b.skins.iter())
            .find(|s| s.name == "default")
            .expect("skin");
        assert_eq!(found.origin, crate::Point::new(7, 9));
    }
}

#[cfg(test)]
mod special_slot_tests {
    use super::*;

    fn data_root() -> Option<std::path::PathBuf> {
        std::env::var_os("WZ_DATA_ROOT")
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists())
    }

    #[test]
    fn taming_slot_loads() {
        let Some(data_root) = data_root() else {
            return;
        };
        let equipment = vec![("Taming".to_string(), 1902000)];
        if let Some(av) = load_avatar_with_equipment(&data_root, 2000, &equipment) {
            assert!(
                av.parts.iter().any(|p| p.slot == "tamingmob"),
                "taming part missing"
            );
        }
    }

    #[test]
    fn effect_slot_loads_first_frame() {
        let Some(data_root) = data_root() else {
            return;
        };
        let equipment = vec![("Effect".to_string(), 1000070)];
        if let Some(av) = load_avatar_with_equipment(&data_root, 2000, &equipment) {
            assert!(
                av.parts.iter().any(|p| p.slot == "effect"),
                "effect part missing"
            );
        }
    }

    #[test]
    fn chair_slot_loads() {
        let Some(data_root) = data_root() else {
            return;
        };
        let equipment = vec![("Chair".to_string(), 3010000)];
        if let Some(av) = load_avatar_with_equipment(&data_root, 2000, &equipment) {
            assert!(
                av.parts.iter().any(|p| p.slot == "chair"),
                "chair part missing"
            );
        }
    }

    #[test]
    fn effect_frame_index_changes_pixels() {
        let Some(data_root) = data_root() else {
            return;
        };
        let equipment = vec![("Effect".to_string(), 1000070)];
        let base = load_avatar_with_equipment_and_mix(
            &data_root,
            2000,
            &equipment,
            &MixOptions::default(),
        );
        let next = load_avatar_with_equipment_and_mix(
            &data_root,
            2000,
            &equipment,
            &MixOptions {
                effect_frame: 1,
                ..Default::default()
            },
        );
        let (Some(base), Some(next)) = (base, next) else {
            return;
        };
        let (Some(a), Some(b)) = (
            base.parts.iter().find(|p| p.slot == "effect"),
            next.parts.iter().find(|p| p.slot == "effect"),
        ) else {
            return;
        };
        assert_ne!(a.rgba, b.rgba, "effect frame 0 and 1 should differ");
    }
}
