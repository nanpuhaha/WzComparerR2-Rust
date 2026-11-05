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
#[derive(Debug, Default, Clone)]
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
    /// Effect 슬롯의 전체 프레임 (자동 재생용). `parts` 에는 선택 프레임만 들어간다.
    pub effect_frames: Vec<AvatarPart>,
    pub effect_delays: Vec<u32>,
    /// 프레임별 origin/z (scene 렌더러용).
    pub effect_origins: Vec<crate::Point>,
    pub effect_z_indices: Vec<i32>,
    /// 체어 메타데이터 (`info/scale`, group taming, bodyRelMove).
    pub chair_info: Option<ChairInfo>,
}

/// C# `CustomChairType` — 미래 타입은 `Other` 로 보존한다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CustomChairType {
    #[default]
    Unsupported,
    ScaleAvatarChair,
    Other(String),
}

impl CustomChairType {
    pub fn parse(text: &str) -> Self {
        if text.eq_ignore_ascii_case("ScaleAvatarChair") {
            Self::ScaleAvatarChair
        } else if text.is_empty() {
            Self::Unsupported
        } else {
            Self::Other(text.to_string())
        }
    }
}

/// C# `ChairPart` 의 메타데이터 중 정적 로더가 해석하는 부분.
#[derive(Debug, Clone, Default)]
pub struct ChairInfo {
    pub scale: i32,
    pub custom_chair_type: CustomChairType,
    pub custom_scale: Option<i32>,
    pub group_count: usize,
    pub group_taming_ids: Vec<i32>,
    pub group_body_rel_move: Vec<crate::Point>,
    pub body_rel_move: Option<crate::Point>,
}

/// Effect 한 항목의 전체 프레임/딜레이/origin/z.
#[derive(Debug, Clone)]
pub struct EffectFrames {
    pub root_bone: Bone,
    pub parts: Vec<AvatarPart>,
    pub delays: Vec<u32>,
    pub origins: Vec<crate::Point>,
    pub z_indices: Vec<i32>,
}

impl ChairInfo {
    /// `ScaleAvatarChair` 이면 `scaleAvatar.scale`, 아니면 `info.scale`.
    pub fn effective_scale(&self) -> i32 {
        match self.custom_chair_type {
            CustomChairType::ScaleAvatarChair => self.custom_scale.unwrap_or(100),
            _ => self.scale,
        }
    }
}

fn set_effect_skin(bone: &mut Bone, origin: crate::Point, z_index: i32) -> bool {
    if let Some(skin) = bone.skins.iter_mut().find(|s| s.name == "effect") {
        skin.origin = origin;
        skin.z_index = z_index;
        return true;
    }
    for child in &mut bone.children {
        if set_effect_skin(child, origin, z_index) {
            return true;
        }
    }
    false
}

impl LoadedAvatar {
    /// effect 프레임의 origin/z 를 bone 에 반영한다 (scene 렌더러용).
    ///
    /// GUI 의 파트 기반 렌더러는 RGBA 만 쓰므로 origin 이 필요 없지만,
    /// `AvatarScene` 을 쓰는 렌더러는 이 메서드로 프레임별 origin 을 맞춘다.
    pub fn apply_effect_frame(&mut self, index: usize) {
        if self.effect_frames.is_empty() {
            return;
        }
        let i = index % self.effect_frames.len();
        let origin = self
            .effect_origins
            .get(i)
            .copied()
            .unwrap_or(crate::Point::ZERO);
        let z_index = self.effect_z_indices.get(i).copied().unwrap_or(0);
        set_effect_skin(&mut self.root_bone, origin, z_index);
    }
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

/// `/` 로 구분된 경로를 따라간다.
fn find_path_node<'a>(node: &'a ImageNode, path: &str) -> Option<&'a ImageNode> {
    let mut cur = node;
    for seg in path.split('/').filter(|s| !s.is_empty()) {
        cur = child_node(cur, seg)?;
    }
    Some(cur)
}

fn node_int(node: &ImageNode, path: &str) -> Option<i32> {
    match find_path_node(node, path).and_then(|n| n.value.as_ref()) {
        Some(wz_core::image::ImageValue::Int(v)) => Some(*v),
        Some(wz_core::image::ImageValue::Short(v)) => Some(i32::from(*v)),
        Some(wz_core::image::ImageValue::Str(v)) => v.parse::<i32>().ok(),
        _ => None,
    }
}

fn node_str(node: &ImageNode, path: &str) -> Option<String> {
    match find_path_node(node, path).and_then(|n| n.value.as_ref()) {
        Some(wz_core::image::ImageValue::Str(v)) => Some(v.to_string()),
        _ => None,
    }
}

fn node_vec(node: &ImageNode, path: &str) -> Option<crate::Point> {
    match find_path_node(node, path).and_then(|n| n.value.as_ref()) {
        Some(wz_core::image::ImageValue::Vector(v)) => Some(crate::Point::new(v.x, v.y)),
        _ => None,
    }
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

/// `Item/Install/<dir>` 에서 카테고리 이미지의 아이템 노드를 찾는다.
///
/// `dir` 은 `""`(메타데이터) 또는 `"_Canvas"`(픽셀) 이다.
fn find_install_item(
    data_root: &std::path::Path,
    dir: &str,
    item_id: i32,
) -> Option<(WzArchive, ImageNode)> {
    let base = data_root.join("Item").join("Install").join(dir);
    let category = format!("{:05}.img", item_id / 1000);
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
        return Some((archive, item.clone()));
    }
    None
}

/// `Item/Install/_Canvas` 의 체어 이미지를 정적 1프레임으로 로드한다.
///
/// 체어 카테고리 이미지는 `03010.img` 처럼 5자리이고, 그 자식이 아이템 ID 다.
/// `effect/0`, `0`, `default` 순으로 캔버스를 찾는다.
fn load_chair_item(
    data_root: &std::path::Path,
    item_id: i32,
) -> Option<(Bone, Vec<AvatarPart>, ChairInfo)> {
    let (archive, item) = find_install_item(data_root, "_Canvas", item_id)?;
    let meta = find_install_item(data_root, "", item_id).map(|(_, node)| node);

    let png = child_node(&item, "effect")
        .and_then(|effect| child_node(effect, "0"))
        .and_then(|frame| frame.first_canvas())
        .or_else(|| child_node(&item, "0").and_then(|frame| frame.first_canvas()))
        .or_else(|| child_node(&item, "default").and_then(|frame| frame.first_canvas()))
        .or_else(|| item.first_canvas())?;
    let (w, h, rgba) = wz_image::decode_png_ref_to_rgba(&png, archive.source()).ok()?;
    let (origin, z_index) = png_origin_z(&png);

    let info = meta
        .as_ref()
        .and_then(|node| child_node(node, "info"))
        .or_else(|| child_node(&item, "info"));
    let scale = info.and_then(|i| node_int(i, "scale")).unwrap_or(100);
    let custom = info.and_then(|i| child_node(i, "customChair"));
    let custom_chair_type = custom
        .and_then(|c| node_str(c, "type"))
        .map(|t| CustomChairType::parse(&t))
        .unwrap_or_default();
    let custom_scale = custom.and_then(|c| node_int(c, "scaleAvatar/scale"));
    let mut group_count = 0;
    let mut group_taming_ids = Vec::new();
    let mut group_body_rel_move = Vec::new();
    if let Some(info) = info {
        for group in info.object_children() {
            if !group.name.to_ascii_lowercase().contains("group") {
                continue;
            }
            for entry in group.object_children() {
                let lower = entry.name.to_ascii_lowercase();
                if !(lower.contains("sit") || lower.contains("mainuser")) {
                    continue;
                }
                group_count = entry.object_children().len();
                for member in entry.object_children() {
                    let id = node_int(member, "tamingMobM")
                        .or_else(|| node_int(member, "tamingMobF"))
                        .or_else(|| node_int(member, "tamingMob"))
                        .unwrap_or(0);
                    group_taming_ids.push(id);
                    group_body_rel_move
                        .push(node_vec(member, "bodyRelMove").unwrap_or(crate::Point::ZERO));
                }
            }
        }
    }
    let body_rel_move = child_node(&item, "0")
        .and_then(|n| node_vec(n, "bodyRelMove"))
        .or_else(|| child_node(&item, "1").and_then(|n| node_vec(n, "bodyRelMove")))
        .or_else(|| group_body_rel_move.first().copied());

    let mut skin = SkinLayer::new("chair");
    skin.z = "chair".into();
    skin.z_index = z_index;
    skin.origin = origin;
    skin.width = w;
    skin.height = h;

    let mut root = Bone::new("@chair");
    root.position = body_rel_move.unwrap_or(crate::Point::ZERO);
    root.append_bone(skin, "chair", crate::Point::ZERO);
    let part = AvatarPart {
        slot: "chair".into(),
        width: w,
        height: h,
        rgba,
        ..Default::default()
    };
    let chair_info = ChairInfo {
        scale,
        custom_chair_type,
        custom_scale,
        group_count,
        group_taming_ids,
        group_body_rel_move,
        body_rel_move,
    };
    Some((root, vec![part], chair_info))
}

/// RGBA8 버퍼를 최근접 이웃으로 `factor` 배 확대/축소한다.
fn resize_rgba_nearest(src: &[u8], w: u32, h: u32, factor: f32) -> (u32, u32, Vec<u8>) {
    let nw = ((w as f32 * factor).round() as u32).max(1);
    let nh = ((h as f32 * factor).round() as u32).max(1);
    if nw == w && nh == h {
        return (w, h, src.to_vec());
    }
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        let sy = ((y as f32 / factor) as u32).min(h.saturating_sub(1));
        for x in 0..nw {
            let sx = ((x as f32 / factor) as u32).min(w.saturating_sub(1));
            let src_off = ((sy * w + sx) * 4) as usize;
            let dst_off = ((y * nw + x) * 4) as usize;
            out[dst_off..dst_off + 4].copy_from_slice(&src[src_off..src_off + 4]);
        }
    }
    (nw, nh, out)
}

fn is_scaled_slot(slot: &str) -> bool {
    !matches!(slot, "chair" | "effect" | "taming") && !slot.starts_with("tamingmob")
}

fn scale_bone_tree(bone: &mut Bone, factor: f32, scaled: &std::collections::HashSet<String>) {
    let has_scaled = bone.skins.iter().any(|s| scaled.contains(&s.name));
    if has_scaled {
        bone.position = crate::Point::new(
            (bone.position.x as f32 * factor).round() as i32,
            (bone.position.y as f32 * factor).round() as i32,
        );
        for skin in &mut bone.skins {
            if scaled.contains(&skin.name) {
                skin.origin = crate::Point::new(
                    (skin.origin.x as f32 * factor).round() as i32,
                    (skin.origin.y as f32 * factor).round() as i32,
                );
                skin.offset = crate::Point::new(
                    (skin.offset.x as f32 * factor).round() as i32,
                    (skin.offset.y as f32 * factor).round() as i32,
                );
                skin.width = (skin.width as f32 * factor).round().max(1.0) as u32;
                skin.height = (skin.height as f32 * factor).round().max(1.0) as u32;
            }
        }
    }
    for child in &mut bone.children {
        scale_bone_tree(child, factor, scaled);
    }
}

/// 체어의 `scale`/`scaleAvatar.scale` 을 아바타 파트와 bone 에 적용한다.
fn apply_avatar_scale(root: &mut Bone, parts: &mut [AvatarPart], scale: i32) {
    if scale <= 0 || scale == 100 {
        return;
    }
    let factor = scale as f32 / 100.0;
    let scaled: std::collections::HashSet<String> = parts
        .iter()
        .filter(|p| is_scaled_slot(&p.slot))
        .map(|p| p.slot.clone())
        .collect();
    for part in parts.iter_mut() {
        if !scaled.contains(&part.slot) {
            continue;
        }
        let (w, h, rgba) = resize_rgba_nearest(&part.rgba, part.width, part.height, factor);
        part.width = w;
        part.height = h;
        part.rgba = rgba;
    }
    scale_bone_tree(root, factor, &scaled);
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

/// C# `CustomOriginMap`(originValue → childName) + `CustomOrigin`(childName → Point)
/// 두 단계를 거쳐 skin origin 을 덮어쓴다.
fn apply_custom_origin_map(
    root: &mut Bone,
    parts: &[AvatarPart],
    selected: &std::collections::BTreeMap<String, crate::Point>,
) {
    if selected.is_empty() {
        return;
    }
    for part in parts {
        let mut effective = std::collections::BTreeMap::new();
        for (origin_value, child_name) in &part.custom_origin_map {
            if let Some(point) = selected.get(child_name) {
                effective.insert(origin_value.clone(), *point);
            }
        }
        if !effective.is_empty() {
            apply_custom_origins(root, &effective);
        }
    }
}

/// `Effect/Effect_*.wz` 의 `ItemEff.img/<id>/effect/default/0` 을 정적 1프레임으로 로드한다.
///
/// C# 은 effect 를 애니메이션하지만, v0.1 이후 PoC 는 첫 프레임만 합성한다.
fn load_effect_item(
    data_root: &std::path::Path,
    item_id: i32,
    frame_index: usize,
) -> Option<EffectFrames> {
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
    if frames.is_empty() {
        return None;
    }
    let canvas_archive = find_effect_canvas_archive(data_root, "ItemEff.img")?;
    let effect_z = child_node(effect, "z")
        .and_then(|n| match &n.value {
            Some(ImageValue::Int(v)) => Some(*v),
            _ => None,
        })
        .unwrap_or(0);

    let selected_index = frame_index % frames.len();
    let mut parts = Vec::new();
    let mut delays = Vec::new();
    let mut origins = Vec::new();
    let mut z_indices = Vec::new();
    let mut selected_meta = None;

    for (i, frame) in frames.iter().enumerate() {
        let png = frame.first_canvas()?;
        let outlink = png.sub_property.as_ref().and_then(|sub| {
            sub.children.iter().find_map(|c| match &c.value {
                Some(ImageValue::Str(v)) if c.name.as_ref() == "_outlink" => Some(v.to_string()),
                _ => None,
            })
        })?;
        let resolved = crate::resolve_outlink(&canvas_archive, &outlink).ok()?;
        let (origin, frame_z) = png_origin_z(&png);
        let z_index = if frame_z != 0 { frame_z } else { effect_z };
        let delay = png
            .sub_property
            .as_ref()
            .and_then(|sub| {
                sub.children
                    .iter()
                    .find_map(|c| match (&c.name.as_ref(), &c.value) {
                        (&"delay", Some(ImageValue::Int(v))) => Some((*v).max(0) as u32),
                        _ => None,
                    })
            })
            .unwrap_or(0);
        delays.push(delay);
        origins.push(origin);
        z_indices.push(z_index);
        if i == selected_index {
            selected_meta = Some((resolved.w, resolved.h, origin, z_index));
        }
        parts.push(AvatarPart {
            slot: "effect".into(),
            width: resolved.w,
            height: resolved.h,
            rgba: resolved.rgba,
            ..Default::default()
        });
    }

    let (w, h, origin, z_index) = selected_meta?;
    let mut skin = SkinLayer::new("effect");
    skin.z = "effect".into();
    skin.z_index = z_index;
    skin.origin = origin;
    skin.width = w;
    skin.height = h;

    let mut root = Bone::new("@effect");
    root.append_bone(skin, "effect", crate::Point::ZERO);
    Some(EffectFrames {
        root_bone: root,
        parts,
        delays,
        origins,
        z_indices,
    })
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

/// `Character/TamingMob/_Canvas` 에서 taming mob 한 마리를 로드한다.
fn load_taming_item(data_root: &std::path::Path, item_id: i32) -> Option<(Bone, Vec<AvatarPart>)> {
    let dir = data_root
        .join("Character")
        .join("TamingMob")
        .join("_Canvas");
    let image = format!("{:08}.img", item_id);
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
            .any(|e| e.name.as_deref() == Some(image.as_str()))
        {
            return load_item_frame(
                &archive,
                Some(&image),
                &["sit", "stand1", "default"],
                "TamingMob",
            );
        }
    }
    None
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
        effect_frames: Vec::new(),
        effect_delays: Vec::new(),
        effect_origins: Vec::new(),
        effect_z_indices: Vec::new(),
        chair_info: None,
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
    let mut effect_frames: Vec<AvatarPart> = Vec::new();
    let mut effect_delays: Vec<u32> = Vec::new();
    let mut effect_origins: Vec<crate::Point> = Vec::new();
    let mut effect_z_indices: Vec<i32> = Vec::new();
    let mut chair_info: Option<ChairInfo> = None;
    for (slot, item_id) in equipment {
        if let Some(dir) = slot_dir_name(slot) {
            if dir == "Effect" {
                if let Some(effect) = load_effect_item(data_root, *item_id, mix.effect_frame) {
                    let EffectFrames {
                        root_bone,
                        parts: frame_parts,
                        delays,
                        origins,
                        z_indices,
                    } = effect;
                    merge_bone_tree(&mut root, root_bone);
                    let selected = mix.effect_frame.min(frame_parts.len().saturating_sub(1));
                    if let Some(part) = frame_parts.get(selected).cloned() {
                        parts.push(part);
                    }
                    effect_frames = frame_parts;
                    effect_delays = delays;
                    effect_origins = origins;
                    effect_z_indices = z_indices;
                }
                continue;
            }
            if dir == "Chair" {
                if let Some((item_root, item_parts, info)) = load_chair_item(data_root, *item_id) {
                    chair_info = Some(info);
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
            let actions: &[&str] = if dir == "TamingMob" {
                &["sit", "stand1", "default"]
            } else {
                &["default", "stand1"]
            };
            if let Some((mut item_root, mut item_parts)) =
                load_item_frame(&archive, Some(&image_name), actions, dir)
            {
                let mix_setting = match dir {
                    "Hair" => mix.hair,
                    "Face" => mix.face,
                    _ => None,
                };
                if let Some((color, opacity)) = mix_setting {
                    apply_mix_to_item(&archive, *item_id, dir, &mut item_parts, color, opacity);
                }
                apply_custom_origin_map(&mut item_root, &item_parts, &mix.custom_origins);
                merge_bone_tree(&mut root, item_root);
                parts.extend(item_parts);
            }
        }
    }

    if let Some(info) = &chair_info {
        for (i, id) in info.group_taming_ids.iter().enumerate() {
            if *id == 0 {
                continue;
            }
            if let Some((mut taming_root, mut taming_parts)) = load_taming_item(data_root, *id) {
                taming_root.position = info
                    .group_body_rel_move
                    .get(i)
                    .copied()
                    .unwrap_or(crate::Point::ZERO);
                if i > 0 {
                    for part in &mut taming_parts {
                        part.slot = format!("{}_{}", part.slot, i);
                    }
                }
                merge_bone_tree(&mut root, taming_root);
                parts.extend(taming_parts);
            }
        }
        apply_avatar_scale(&mut root, &mut parts, info.effective_scale());
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
        effect_frames,
        effect_delays,
        effect_origins,
        effect_z_indices,
        chair_info,
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
    fn group_chair_loads_taming_members() {
        let Some(data_root) = data_root() else {
            return;
        };
        let equipment = vec![("Chair".to_string(), 3016200)];
        if let Some(av) = load_avatar_with_equipment(&data_root, 2000, &equipment) {
            let info = av.chair_info.as_ref().expect("chair info");
            assert!(info.group_count >= 2, "group chair count");
            assert!(
                info.group_taming_ids.iter().any(|id| *id != 0),
                "group taming ids"
            );
            assert!(av.parts.iter().any(|p| p.slot == "chair"), "chair missing");
            assert!(
                av.parts.iter().any(|p| p.slot.starts_with("tamingmob")),
                "group taming parts missing"
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
        assert!(base.effect_frames.len() >= 2, "effect frames missing");
        assert_eq!(base.effect_frames.len(), base.effect_delays.len());
        assert!(
            base.effect_delays.iter().any(|d| *d > 0),
            "effect delays missing"
        );
    }
}

#[cfg(test)]
mod custom_origin_map_tests {
    use super::*;

    #[test]
    fn custom_origin_map_uses_child_name_then_selected_point() {
        let mut root = Bone::new("@root");
        root.append_bone(SkinLayer::new("default"), "navel", crate::Point::ZERO);

        let mut part = AvatarPart::default();
        // C# `CustomOriginMap`: originValue ("default") → childName ("cap").
        part.custom_origin_map
            .insert("default".to_string(), "cap".to_string());

        let mut selected = std::collections::BTreeMap::new();
        selected.insert("cap".to_string(), crate::Point::new(7, 9));
        apply_custom_origin_map(&mut root, &[part], &selected);

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
mod scale_tests {
    use super::*;

    #[test]
    fn avatar_scale_resizes_matching_parts_and_bones() {
        let mut root = Bone::new("@root");
        let mut skin = SkinLayer::new("body");
        skin.origin = crate::Point::new(2, 4);
        skin.width = 2;
        skin.height = 2;
        root.append_bone(skin, "navel", crate::Point::ZERO);

        let mut parts = vec![AvatarPart {
            slot: "body".into(),
            width: 2,
            height: 2,
            rgba: vec![255; 16],
            ..Default::default()
        }];
        apply_avatar_scale(&mut root, &mut parts, 150);

        assert_eq!(parts[0].width, 3);
        assert_eq!(parts[0].height, 3);
        assert_eq!(parts[0].rgba.len(), 36);
        let skin = root
            .children
            .iter()
            .flat_map(|b| b.skins.iter())
            .find(|s| s.name == "body")
            .expect("body skin");
        assert_eq!(skin.width, 3);
        assert_eq!(skin.origin, crate::Point::new(3, 6));
    }
}

#[cfg(test)]
mod effect_origin_tests {
    use super::*;

    #[test]
    fn set_effect_skin_updates_origin_and_z() {
        let mut root = Bone::new("@root");
        root.append_bone(SkinLayer::new("effect"), "effect", crate::Point::ZERO);
        assert!(set_effect_skin(&mut root, crate::Point::new(3, 4), 2));
        let skin = root
            .children
            .iter()
            .flat_map(|b| b.skins.iter())
            .find(|s| s.name == "effect")
            .expect("effect skin");
        assert_eq!(skin.origin, crate::Point::new(3, 4));
        assert_eq!(skin.z_index, 2);
    }
}

#[cfg(test)]
mod custom_chair_type_tests {
    use super::*;

    #[test]
    fn parses_known_and_unknown_types() {
        assert_eq!(
            CustomChairType::parse("scaleavatarchair"),
            CustomChairType::ScaleAvatarChair
        );
        assert_eq!(CustomChairType::parse(""), CustomChairType::Unsupported);
        assert!(matches!(
            CustomChairType::parse("FutureChair"),
            CustomChairType::Other(v) if v == "FutureChair"
        ));
    }

    #[test]
    fn effective_scale_only_uses_scale_avatar_for_scale_type() {
        let mut info = ChairInfo {
            scale: 110,
            custom_scale: Some(80),
            custom_chair_type: CustomChairType::ScaleAvatarChair,
            ..Default::default()
        };
        assert_eq!(info.effective_scale(), 80);
        info.custom_chair_type = CustomChairType::Other("FutureChair".into());
        assert_eq!(info.effective_scale(), 110);
    }
}
