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

use wz_core::WzArchive;
use wz_core::image::Node as ImageNode;
use wz_image::decode_png_ref_to_rgba;

use crate::{AvatarScene, Bone, SkinLayer, ZMap};

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

/// 로드 결과.
pub struct LoadedAvatar {
    pub parts: Vec<AvatarPart>,
    pub root_bone: Bone,
    pub zmap: Arc<ZMap>,
    pub scene: AvatarScene,
}

const STANDARD_ZMAP: &[&str] = &[
    "Ae",
    "Af",
    "Am",
    "As",
    "At",
    "Ay",
    "Bd",
    "Cp",
    "Fc",
    "Gv",
    "Hd",
    "Hr",
    "Ma",
    "Pn",
    "Ri",
    "Sd",
    "Si",
    "So",
    "Sr",
    "Tm",
    "Wg",
    "Wp",
    "Ws",
    "accessoryEar",
    "accessoryEarOverHair",
    "accessoryEye",
    "accessoryEyeBelowFace",
    "accessoryEyeOverCap",
    "accessoryEyeOverCapOverHair",
    "accessoryEyeShadow",
    "accessoryFace",
    "accessoryFaceBelowFace",
    "accessoryFaceOverFaceBelowCap",
    "accessoryOverHair",
    "arm",
    "armBelowHead",
    "armBelowHeadOverMailChest",
    "armOverHair",
    "armOverHairBelowWeapon",
    "backAccessoryEar",
    "backAccessoryFace",
    "backAccessoryFaceOverHead",
    "backAccessoryOverHead",
    "backBody",
    "backCap",
    "backCapAccessory",
    "backCapOverHair",
    "backCape",
    "backGlove",
    "backGloveWrist",
    "backHair",
    "backHairBelowCap",
    "backHairBelowCapNarrow",
    "backHairBelowCapWide",
    "backHairBelowHead",
    "backHairOverCape",
    "backHead",
    "backMailChest",
    "backMailChestAccessory",
    "backMailChestBelowPants",
    "backMailChestOverPants",
    "backMobEquipFront",
    "backMobEquipMid",
    "backMobEquipUnderSaddle",
    "backPants",
    "backPantsBelowShoes",
    "backPantsOverMailChest",
    "backPantsOverShoesBelowMailChest",
    "backSaddle",
    "backShield",
    "backShieldBelowBody",
    "backShoes",
    "backShoesBelowPants",
    "backTamingMobFront",
    "backTamingMobMid",
    "backWeapon",
    "backWeaponEffectOver",
    "backWeaponEffectUnder",
    "backWeaponOverGlove",
    "backWeaponOverGloveEffectOver",
    "backWeaponOverGloveEffectUnder",
    "backWeaponOverHead",
    "backWeaponOverHeadEffectOver",
    "backWeaponOverHeadEffectUnder",
    "backWeaponOverShield",
    "backWeaponOverShieldEffectOver",
    "backWeaponOverShieldEffectUnder",
    "backWing",
    "body",
    "cap",
    "capAccessory",
    "capAccessoryBelowAccFace",
    "capAccessoryBelowBody",
    "capBelowAccessory",
    "capOverHair",
    "capOverHand",
    "cape",
    "capeBelowBody",
    "capeBelowWeapon",
    "capeOverArm",
    "capeOverFace",
    "capeOverHead",
    "characterEnd",
    "characterStart",
    "emotionOverBody",
    "face",
    "glove",
    "gloveBelowBody",
    "gloveBelowHead",
    "gloveBelowMailArm",
    "gloveBelowWeapon",
    "gloveOverBody",
    "gloveOverHair",
    "gloveWrist",
    "gloveWristBelowBody",
    "gloveWristBelowHead",
    "gloveWristBelowMailArm",
    "gloveWristBelowWeapon",
    "gloveWristOverBody",
    "gloveWristOverHair",
    "hair",
    "hairBelowBody",
    "hairOverHead",
    "hairShade",
    "hand",
    "handBelowWeapon",
    "handOverHair",
    "head",
    "mailArm",
    "mailArmBelowHead",
    "mailArmBelowHeadOverMailChest",
    "mailArmOverHair",
    "mailArmOverHairBelowWeapon",
    "mailChest",
    "mailChestBelowPants",
    "mailChestOverHighest",
    "mailChestOverPants",
    "mailChestTop",
    "mobEquipFront",
    "mobEquipMid",
    "mobEquipRear",
    "mobEquipUnderSaddle",
    "pants",
    "pantsBelowShoes",
    "pantsOverMailChest",
    "pantsOverShoesBelowMailChest",
    "pantsOverShoesTop",
    "saddleFront",
    "saddleRear",
    "shield",
    "shieldBelowBody",
    "shieldOverHair",
    "shoes",
    "shoesOverPants",
    "shoesTop",
    "tamingMobFront",
    "tamingMobMid",
    "tamingMobRear",
    "weapon",
    "weaponBelowArm",
    "weaponBelowArmEffectOver",
    "weaponBelowArmEffectUnder",
    "weaponBelowBody",
    "weaponBelowBodyEffectOver",
    "weaponBelowBodyEffectUnder",
    "weaponEffectOver",
    "weaponEffectUnder",
    "weaponOverArm",
    "weaponOverArmBelowHead",
    "weaponOverArmBelowHeadEffectOver",
    "weaponOverArmBelowHeadEffectUnder",
    "weaponOverArmEffectOver",
    "weaponOverArmEffectUnder",
    "weaponOverBody",
    "weaponOverBodyEffectOver",
    "weaponOverBodyEffectUnder",
    "weaponOverGlove",
    "weaponOverGloveEffectOver",
    "weaponOverGloveEffectUnder",
    "weaponOverHand",
    "weaponOverHandEffectOver",
    "weaponOverHandEffectUnder",
    "weaponWristOverGlove",
    "weaponWristOverGloveEffectOver",
    "weaponWristOverGloveEffectUnder",
];


/// 파트 이름 → z-tag 추론.
fn part_z_tag(part_name: &str) -> String {
    let lower = part_name.to_ascii_lowercase();
    if lower.contains("body") {
        "body".into()
    } else if lower.contains("head") {
        "head".into()
    } else if lower.contains("hair") {
        "hair".into()
    } else if lower.contains("arm") || lower.contains("hand") {
        "arm".into()
    } else if lower.contains("cap") {
        "cap".into()
    } else if lower.contains("face") {
        "face".into()
    } else if lower.contains("weapon") {
        "weapon".into()
    } else if lower.contains("cape") {
        "cape".into()
    } else if lower.contains("coat") {
        "coat".into()
    } else if lower.contains("pants") {
        "pants".into()
    } else if lower.contains("shoe") {
        "shoes".into()
    } else if lower.contains("glove") {
        "glove".into()
    } else {
        part_name.to_string()
    }
}

/// 파트 이름 → 앵커 이름 추론.
fn part_anchor(_part_name: &str) -> &'static str {
    "navel"
}

pub fn load_from_character_wz(char_wz_path: &std::path::Path) -> Option<LoadedAvatar> {
    let canvas_path = char_wz_path.parent()?.join("_Canvas").join("_Canvas_000.wz");
    let arch = WzArchive::open(&canvas_path).ok()?;
    let tree = arch.read_tree().ok()?;

    let img_entry = tree.root.entries.iter().find(|e| e.node_type == 4)?;
    let off = img_entry.offset?;
    let node = arch.parse_image_at(off).ok()?;

    // 표준 액션 우선 선택 (stand1 > alert > 첫 번째)
    let action = node.children.iter().find(|c| c.name.as_ref() == "stand1")
        .or_else(|| node.children.iter().find(|c| c.name.as_ref() == "alert"))
        .or_else(|| node.children.first())
        .or_else(|| {
            if let Some(wz_core::image::ImageValue::Property(p)) = &node.value {
                p.children.iter().find(|c| c.name.as_ref() == "stand1")
                    .or_else(|| p.children.iter().find(|c| c.name.as_ref() == "alert"))
                    .or_else(|| p.children.first())
            } else {
                None
            }
        })?;
    // 첫 프레임
    let frame = action.children.first().or_else(|| {
        if let Some(wz_core::image::ImageValue::Property(p)) = &action.value {
            p.children.first()
        } else {
            None
        }
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
            if let Some(ImageValue::Png(p)) = &n.value {
                return Some(p.clone());
            }
            for c in &n.children {
                if let Some(p) = find_png(c) {
                    return Some(p);
                }
            }
            if let Some(ImageValue::Property(p)) = &n.value {
                if let Some(res) = find_png(p) {
                    return Some(res);
                }
            }
            None
        }

        // z-tag: WZ 노드에 "z" 자식이 있으면 사용, 없으면 이름 기반 추론
        let z_tag = if let Some(wz_core::image::ImageValue::Property(p)) = &pn.value {
            // Property 내부에서 z 찾기
            p.children.iter().find(|c| c.name.as_ref() == "z")
                .and_then(|z| match &z.value {
                    Some(wz_core::image::ImageValue::Str(s)) => Some((**s).to_string()),
                    Some(wz_core::image::ImageValue::Int(_v)) => None, // int z → z_index
                    _ => None,
                })
                .unwrap_or_else(|| part_z_tag(pn.name.as_ref()))
        } else {
            // Png 노드: 형제 노드에서 z 찾기 (부모 프레임에서)
            part_z_tag(pn.name.as_ref())
        };

        // z_index: WZ 노드에 "z" 자식이 Int 이면 사용
        let z_index = if let Some(wz_core::image::ImageValue::Property(p)) = &pn.value {
            p.children.iter().find(|c| c.name.as_ref() == "z")
                .and_then(|z| match &z.value {
                    Some(wz_core::image::ImageValue::Int(v)) => Some(*v),
                    _ => None,
                })
                .unwrap_or(-2)
        } else {
            -2
        };

        let Some(pr) = find_png(pn) else { continue; };
        let file_src = wz_pool::FileSource::open(&canvas_path).ok()?;
        let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
        let Ok((w, h, rgba)) = decode_png_ref_to_rgba(&pr, &src) else {
            continue;
        };

        let slot_name = pn.name.to_string();
        let anchor = part_anchor(&slot_name);

        let mut skin = SkinLayer::new(slot_name.clone());
        skin.z = z_tag;
        skin.z_index = z_index;
        skin.width = w;
        skin.height = h;
        root.append_bone(skin, anchor, crate::Point::new(0, 0));

        parts.push(AvatarPart {
            slot: slot_name,
            width: w,
            height: h,
            rgba,
        });
    }

    // 2. Head 이미지 (00012000.img) 에서 head/face/hair 파트 로드
    let head_img_name = "00012000.img";
    if let Some(head_entry) = tree.root.entries.iter().find(|e| {
        e.node_type == 4 && e.name.as_deref() == Some(head_img_name)
    }) {
        if let Some(off) = head_entry.offset {
            if let Ok(head_node) = arch.parse_image_at(off) {
                // front 액션 우선
                let head_action = head_node.children.iter().find(|c| c.name.as_ref() == "front")
                    .or_else(|| head_node.children.iter().find(|c| c.name.as_ref() == "back"))
                    .or_else(|| head_node.children.first());

                if let Some(ha) = head_action {
                    // head action 의 자식이 Canvas 이면 직접 로드
                    let mut head_parts: Vec<&ImageNode> = ha.children.iter().collect();
                    if let Some(wz_core::image::ImageValue::Property(p)) = &ha.value {
                        head_parts.extend(p.children.iter());
                    }

                    for pn in head_parts {
                        fn find_png2(n: &ImageNode) -> Option<wz_core::image::PngRef> {
                            use wz_core::image::ImageValue;
                            if let Some(ImageValue::Png(p)) = &n.value { return Some(p.clone()); }
                            for c in &n.children { if let Some(p) = find_png2(c) { return Some(p); } }
                            if let Some(ImageValue::Property(p)) = &n.value { if let Some(res) = find_png2(p) { return Some(res); } }
                            None
                        }
                        let Some(pr) = find_png2(pn) else { continue; };
                        let file_src = wz_pool::FileSource::open(&canvas_path).ok()?;
                        let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
                        let Ok((w, h, rgba)) = decode_png_ref_to_rgba(&pr, &src) else { continue; };

                        let slot_name = pn.name.to_string();
                        let z_tag = part_z_tag(&slot_name);

                        let mut skin = SkinLayer::new(slot_name.clone());
                        skin.z = z_tag;
                        skin.width = w;
                        skin.height = h;
                        root.append_bone(skin, "navel", crate::Point::new(0, 0));

                        parts.push(AvatarPart { slot: slot_name, width: w, height: h, rgba });
                    }
                }
            }
        }
    }

    // 3. 장비 아이템 로딩 (PoC: Cap 1000000)
    let char_root = char_wz_path.parent().unwrap_or(char_wz_path);
    let cap_canvas_path = char_root.join("Cap").join("_Canvas").join("_Canvas_000.wz");
    if cap_canvas_path.exists() {
        if let Ok(cap_arch) = WzArchive::open(&cap_canvas_path) {
            if let Ok(cap_tree) = cap_arch.read_tree() {
                // 아이템 ID 1000000 → 파일명 01000000.img
                let item_name = "01000000.img";
                if let Some(entry) = cap_tree.root.entries.iter().find(|e| {
                    e.node_type == 4 && e.name.as_deref() == Some(item_name)
                }) {
                    if let Some(off) = entry.offset {
                        if let Ok(item_node) = cap_arch.parse_image_at(off) {
                            if let Some(png) = extract_first_canvas(&item_node) {
                                let file_src = wz_pool::FileSource::open(&cap_canvas_path).ok();
                                if let Some(fs) = file_src {
                                    let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(fs);
                                    if let Ok((w, h, rgba)) = decode_png_ref_to_rgba(&png, &src) {
                                        let slot_name = "cap".to_string();
                                        let z_tag = part_z_tag(&slot_name);
                                        let mut skin = SkinLayer::new(slot_name.clone());
                                        skin.z = z_tag;
                                        skin.width = w;
                                        skin.height = h;
                                        root.append_bone(skin, "navel", crate::Point::new(0, 0));
                                        parts.push(AvatarPart { slot: slot_name, width: w, height: h, rgba });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 4. 추가 장비 슬롯 로딩 (PoC: 각 슬롯의 첫 번째 아이템)
    let char_root = char_wz_path.parent().unwrap_or(char_wz_path);
    let slots = ["Coat", "Pants", "Shoes", "Glove", "Cape"];
    for slot_name in &slots {
        let canvas_path = char_root.join(slot_name).join("_Canvas").join("_Canvas_000.wz");
        if !canvas_path.exists() { continue; }
        if let Ok(arch) = WzArchive::open(&canvas_path) {
            if let Ok(tree) = arch.read_tree() {
                if let Some(entry) = tree.root.entries.first() {
                    if let Some(off) = entry.offset {
                        if let Ok(node) = arch.parse_image_at(off) {
                            if let Some(png) = extract_first_canvas(&node) {
                                if let Ok(fs) = wz_pool::FileSource::open(&canvas_path) {
                                    let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(fs);
                                    if let Ok((w, h, rgba)) = decode_png_ref_to_rgba(&png, &src) {
                                        let z_tag = part_z_tag(slot_name);
                                        let mut skin = SkinLayer::new(slot_name.to_lowercase());
                                        skin.z = z_tag;
                                        skin.width = w;
                                        skin.height = h;
                                        root.append_bone(skin, "navel", crate::Point::new(0, 0));
                                        parts.push(AvatarPart { slot: slot_name.to_lowercase(), width: w, height: h, rgba });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 5. Weapon 슬롯 로딩 (PoC: _Canvas_000.wz 첫 아이템)
    let weapon_canvas_path = char_root.join("Weapon").join("_Canvas").join("_Canvas_000.wz");
    if weapon_canvas_path.exists() {
        if let Ok(arch) = WzArchive::open(&weapon_canvas_path) {
            if let Ok(tree) = arch.read_tree() {
                if let Some(entry) = tree.root.entries.first() {
                    if let Some(off) = entry.offset {
                        if let Ok(node) = arch.parse_image_at(off) {
                            // stand1 액션의 첫 프레임에서 canvas 추출
                            let mut weapon_png = None;
                            let stand1 = node.children.iter().find(|c| c.name.as_ref() == "stand1");
                            if let Some(s1) = stand1 {
                                if let Some(frame) = s1.children.first() {
                                    weapon_png = extract_first_canvas(frame);
                                }
                            }
                            if weapon_png.is_none() {
                                weapon_png = extract_first_canvas(&node);
                            }
                            if let Some(png) = weapon_png {
                                if let Ok(fs) = wz_pool::FileSource::open(&weapon_canvas_path) {
                                    let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(fs);
                                    if let Ok((w, h, rgba)) = decode_png_ref_to_rgba(&png, &src) {
                                        let mut skin = SkinLayer::new("weapon".to_string());
                                        skin.z = "weapon".to_string();
                                        skin.width = w;
                                        skin.height = h;
                                        root.append_bone(skin, "navel", crate::Point::new(0, 0));
                                        parts.push(AvatarPart { slot: "weapon".to_string(), width: w, height: h, rgba });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 6. 나머지 슬롯 로딩 (PoC: 각 슬롯의 첫 번째 아이템)
    let remaining_slots = [
        ("Accessory", "accessory"),
        ("Face", "face"),
        ("Hair", "hair"),
        ("Ring", "ring"),
        ("PetEquip", "petequip"),
        ("TamingMob", "tamingmob"),
    ];
    for (dir_name, slot_name) in &remaining_slots {
        let canvas_path = char_root.join(dir_name).join("_Canvas").join("_Canvas_000.wz");
        if !canvas_path.exists() { continue; }
        if let Ok(arch) = WzArchive::open(&canvas_path) {
            if let Ok(tree) = arch.read_tree() {
                if let Some(entry) = tree.root.entries.first() {
                    if let Some(off) = entry.offset {
                        if let Ok(node) = arch.parse_image_at(off) {
                            if let Some(png) = extract_first_canvas(&node) {
                                if let Ok(fs) = wz_pool::FileSource::open(&canvas_path) {
                                    let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(fs);
                                    if let Ok((w, h, rgba)) = decode_png_ref_to_rgba(&png, &src) {
                                        let z_tag = part_z_tag(slot_name);
                                        let mut skin = SkinLayer::new(slot_name.to_string());
                                        skin.z = z_tag;
                                        skin.width = w;
                                        skin.height = h;
                                        root.append_bone(skin, "navel", crate::Point::new(0, 0));
                                        parts.push(AvatarPart { slot: slot_name.to_string(), width: w, height: h, rgba });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if parts.is_empty() {
        return None;
    }

    // 표준 MapleStory z-map 순서 사용
    let mut zmap_order: Vec<String> = STANDARD_ZMAP.iter().map(|s| s.to_string()).collect();
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
pub fn extract_first_canvas(node: &wz_core::image::Node) -> Option<wz_core::image::PngRef> {
    use wz_core::image::ImageValue;
    // 직접 Canvas
    if let Some(ImageValue::Png(p)) = &node.value {
        return Some(p.clone());
    }
    // Property → 첫 번째 자식 재귀
    if let Some(ImageValue::Property(p)) = &node.value {
        for child in &p.children {
            if let Some(png) = extract_first_canvas(child) {
                return Some(png);
            }
        }
    }
    // children 직접 탐색
    for child in &node.children {
        if let Some(png) = extract_first_canvas(child) {
            return Some(png);
        }
    }
    None
}


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
    let entry = tree.root.entries.iter().find(|e| {
        e.node_type == 4 && e.name.as_deref() == Some(file_name.as_str())
    })?;
    let off = entry.offset?;
    let node = arch.parse_image_at(off).ok()?;
    let png = extract_first_canvas(&node)?;
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
    if !canvas_path.exists() { return None; }

    let arch = WzArchive::open(&canvas_path).ok()?;
    let tree = arch.read_tree().ok()?;

    let mut parts: Vec<AvatarPart> = Vec::new();
    let mut root = Bone::new("@root");

    // Body: 0000{skin}00.img (skin=2000 → 00002000.img)
    let body_img_name = format!("{:08}.img", body_skin);
    if let Some(entry) = tree.root.entries.iter().find(|e| {
        e.node_type == 4 && e.name.as_deref() == Some(body_img_name.as_str())
    }) {
        if let Some(off) = entry.offset {
            if let Ok(node) = arch.parse_image_at(off) {
                // stand1 액션 우선
                let action = node.children.iter().find(|c| c.name.as_ref() == "stand1")
                    .or_else(|| node.children.iter().find(|c| c.name.as_ref() == "alert"))
                    .or_else(|| node.children.first());
                if let Some(act) = action {
                    let frame = act.children.first();
                    if let Some(f) = frame {
                        let mut part_nodes: Vec<&wz_core::image::Node> = f.children.iter().collect();
                        if let Some(wz_core::image::ImageValue::Property(p)) = &f.value {
                            part_nodes.extend(p.children.iter());
                        }
                        for pn in part_nodes {
                            fn find_png(n: &wz_core::image::Node) -> Option<wz_core::image::PngRef> {
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
                            let z_tag = part_z_tag(&slot_name);
                            let mut skin = SkinLayer::new(slot_name.clone());
                            skin.z = z_tag;
                            skin.width = w;
                            skin.height = h;
                            root.append_bone(skin, "navel", crate::Point::new(0, 0));
                            parts.push(AvatarPart { slot: slot_name, width: w, height: h, rgba });
                        }
                    }
                }
            }
        }
    }

    // Head 로드
    // Head is in the same canvas file (00012000.img)
    if let Some(head_entry) = tree.root.entries.iter().find(|e| {
        e.node_type == 4 && e.name.as_deref() == Some("00012000.img")
    }) {
        if let Some(off) = head_entry.offset {
            if let Ok(head_node) = arch.parse_image_at(off) {
                let head_action = head_node.children.iter().find(|c| c.name.as_ref() == "front")
                    .or_else(|| head_node.children.iter().find(|c| c.name.as_ref() == "back"))
                    .or_else(|| head_node.children.first());
                if let Some(ha) = head_action {
                    let mut head_parts: Vec<&wz_core::image::Node> = ha.children.iter().collect();
                    if let Some(wz_core::image::ImageValue::Property(p)) = &ha.value {
                        head_parts.extend(p.children.iter());
                    }
                    for pn in head_parts {
                        fn find_png2(n: &wz_core::image::Node) -> Option<wz_core::image::PngRef> {
                            use wz_core::image::ImageValue;
                            if let Some(ImageValue::Png(p)) = &n.value { return Some(p.clone()); }
                            for c in &n.children { if let Some(p) = find_png2(c) { return Some(p); } }
                            if let Some(ImageValue::Property(p)) = &n.value { if let Some(res) = find_png2(p) { return Some(res); } }
                            None
                        }
                        let Some(pr) = find_png2(pn) else { continue; };
                        let file_src = wz_pool::FileSource::open(&canvas_path).ok()?;
                        let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
                        let Ok((w, h, rgba)) = decode_png_ref_to_rgba(&pr, &src) else { continue; };
                        let slot_name = pn.name.to_string();
                        let z_tag = part_z_tag(&slot_name);
                        let mut skin = SkinLayer::new(slot_name.clone());
                        skin.z = z_tag;
                        skin.width = w;
                        skin.height = h;
                        root.append_bone(skin, "navel", crate::Point::new(0, 0));
                        parts.push(AvatarPart { slot: slot_name, width: w, height: h, rgba });
                    }
                }
            }
        }
    }

    // Equipment 로드
    for (slot, item_id) in equipment {
        if let Some(dir) = slot_dir_name(slot) {
            let equip_canvas_path = data_root.join("Character").join(dir).join("_Canvas").join("_Canvas_000.wz");
            if equip_canvas_path.exists() {
                if let Some((_, w, h, rgba)) = load_equipment_item(&equip_canvas_path, *item_id) {
                    let z_tag = part_z_tag(slot);
                    let mut skin = SkinLayer::new(slot.to_lowercase());
                    skin.z = z_tag;
                    skin.width = w;
                    skin.height = h;
                    root.append_bone(skin, "navel", crate::Point::new(0, 0));
                    parts.push(AvatarPart { slot: slot.to_lowercase(), width: w, height: h, rgba });
                }
            }
        }
    }

    if parts.is_empty() { return None; }

    let mut zmap_order: Vec<String> = STANDARD_ZMAP.iter().map(|s| s.to_string()).collect();
    for p in &parts {
        let zt = part_z_tag(&p.slot);
        if !zmap_order.contains(&zt) { zmap_order.push(zt); }
    }
    let zmap = std::sync::Arc::new(ZMap::new(zmap_order));
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

        // z-tag 검증: body/head/hair/arm 중 하나 이상 있어야 한다
        let z_tags: Vec<&str> = av
            .root_bone
            .children
            .iter()
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
    fn load_avatar_with_equipment_works() {
        let data_root = std::path::Path::new("<WZ_DATA_ROOT>");
        if !data_root.exists() { return; }
        let equipment = vec![
            ("Cap".to_string(), 1000000),
            ("Coat".to_string(), 1040000),
        ];
        let av = load_avatar_with_equipment(data_root, 2000, &equipment);
        if let Some(av) = av {
            assert!(!av.parts.is_empty(), "at least one part loaded");
            // body + head + cap + coat
            assert!(av.parts.len() >= 4, "expected >= 4 parts, got {}", av.parts.len());
            let batch = av.scene.build_batch(&av.root_bone);
            assert!(batch.layer_count > 0);
        }
    }

    #[test]
    fn load_equipment_item_works() {
        let data_root = std::path::Path::new("<WZ_DATA_ROOT>");
        if !data_root.exists() { return; }
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
        let coat_path = data_root.join("Coat").join("_Canvas").join("_Canvas_000.wz");
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
        let char_wz = "<WZ_DATA_ROOT>/Character/Character.wz";
        if !std::path::Path::new(char_wz).exists() {
            return;
        }
        let av = load_from_character_wz(std::path::Path::new(char_wz))
            .expect("loader should return Some on real Character.wz");
        // 최소 8 파트 (body, arm, head, ear variants, cap)
        assert!(av.parts.len() >= 14, "expected >= 14 parts, got {}", av.parts.len());
        // body 파트가 있어야 한다
        assert!(av.parts.iter().any(|p| p.slot == "body"), "body part missing");
        // head 파트가 있어야 한다
        assert!(av.parts.iter().any(|p| p.slot == "head"), "head part missing");
        // cap 파트가 있어야 한다
        assert!(av.parts.iter().any(|p| p.slot == "cap"), "cap part missing");
        // zmap 이 표준 순서를 포함해야 한다
        assert!(av.zmap.index_of("body").is_some(), "zmap missing body");
        assert!(av.zmap.index_of("head").is_some(), "zmap missing head");
        // batch 렌더링이 동작해야 한다
        let batch = av.scene.build_batch(&av.root_bone);
        assert!(batch.layer_count >= av.parts.len(), "layer_count < parts count");
        assert!(batch.vertex_count() > 0, "no vertices generated");
    }
}
