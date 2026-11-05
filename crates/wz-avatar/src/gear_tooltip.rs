//! CharaSimControl gear tooltip의 텍스트/아이콘 합성.
//!
//! 원본 `ItemStringHelper.GetGearPropString` 의 주요 장비 스탯 라벨을 옮기고,
//! 기존 `render_tooltip_bitmap` 으로 게임 폰트+아이콘 비트맵을 만든다.

use std::sync::Arc;
use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

#[derive(Debug, Clone, Default)]
pub struct GearTooltip {
    pub title: String,
    pub lines: Vec<String>,
    pub icon: Option<(u32, u32, Vec<u8>)>,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn int(node: &Node, key: &str) -> Option<i64> {
    child(node, key).and_then(|c| match &c.value {
        Some(ImageValue::Int(v)) => Some(i64::from(*v)),
        Some(ImageValue::Short(v)) => Some(i64::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v),
        Some(ImageValue::Uint(v)) => Some(i64::from(*v)),
        _ => None,
    })
}

fn signed(_key: &str, label: &str, value: i64, percent: bool) -> String {
    let sign = if value > 0 { "+" } else { "" };
    let suffix = if percent { "%" } else { "" };
    format!("{label} : {sign}{value}{suffix}")
}

/// 원본 `ItemStringHelper.GetGearPropString` 의 주요 라벨.
fn prop_line(key: &str, value: i64) -> Option<String> {
    Some(match key {
        "incSTR" => signed(key, "STR", value, false),
        "incDEX" => signed(key, "DEX", value, false),
        "incINT" => signed(key, "INT", value, false),
        "incLUK" => signed(key, "LUK", value, false),
        "incAllStat" | "statR" => signed(key, "올스탯", value, false),
        "incMHP" => signed(key, "최대 HP", value, false),
        "incMMP" => signed(key, "최대 MP", value, false),
        "incPAD" => signed(key, "공격력", value, false),
        "incMAD" => signed(key, "마력", value, false),
        "incPDD" => signed(key, "방어력", value, false),
        "incMDD" => signed(key, "마법방어력", value, false),
        "incACC" => signed(key, "명중치", value, false),
        "incEVA" => signed(key, "회피치", value, false),
        "incSpeed" => signed(key, "이동속도", value, false),
        "incJump" => signed(key, "점프력", value, false),
        "incCraft" => signed(key, "손재주", value, false),
        "incDAMr" | "damR" => signed(key, "데미지", value, true),
        "incCr" => signed(key, "크리티컬 확률", value, true),
        "incCDr" => signed(key, "크리티컬 데미지", value, true),
        "incBDR" | "bdR" => format!("보스 몬스터 공격 시 데미지 +{value}%"),
        "incIMDR" | "imdR" => format!("몬스터 방어율 무시 : +{value}%"),
        "incEXPr" => signed(key, "추가 경험치", value, true),
        "tuc" => format!("업그레이드 가능 횟수 : {value}"),
        "reqLevel" => format!("착용 레벨 : {value}"),
        "only" if value != 0 => "중복 소지 불가".into(),
        "tradeBlock" if value != 0 => "교환 불가".into(),
        "equipTradeBlock" if value != 0 => "장착 시 교환 불가".into(),
        "accountSharable" if value != 0 => "월드 내 나의 캐릭터 간 이동만 가능".into(),
        "onlyEquip" if value != 0 => "중복 장착 불가".into(),
        "notExtend" if value != 0 => "유효기간 연장 불가".into(),
        "noPotential" if value != 0 => "잠재능력 설정 불가".into(),
        "fixedPotential" if value != 0 => "잠재능력 재설정 불가".into(),
        "superiorEqp" if value != 0 => {
            "아이템 강화 성공시 더욱 높은 효과를 받을 수 있습니다.".into()
        }
        "blockGoldHammer" if value != 0 => "황금망치 사용 불가".into(),
        "cantRepair" if value != 0 => "수리 불가".into(),
        _ => return None,
    })
}

const PROP_ORDER: &[&str] = &[
    "reqLevel",
    "incSTR",
    "incDEX",
    "incINT",
    "incLUK",
    "incAllStat",
    "statR",
    "incMHP",
    "incMMP",
    "incPAD",
    "incMAD",
    "incPDD",
    "incMDD",
    "incACC",
    "incEVA",
    "incSpeed",
    "incJump",
    "incCraft",
    "incDAMr",
    "incCr",
    "incCDr",
    "incBDR",
    "incIMDR",
    "incEXPr",
    "tuc",
    "only",
    "tradeBlock",
    "equipTradeBlock",
    "accountSharable",
    "onlyEquip",
    "notExtend",
    "noPotential",
    "fixedPotential",
    "superiorEqp",
    "blockGoldHammer",
    "cantRepair",
];

fn find_item_image(
    data_root: &std::path::Path,
    slot: &str,
    item_id: i32,
) -> Option<(Node, Arc<dyn wz_pool::ByteSource>)> {
    let dir = crate::loader::slot_dir_name(slot)?;
    let image = format!("{item_id:08}.img");
    let base = data_root.join("Character").join(dir);
    for base in [base.clone(), base.join("_Canvas")] {
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
            let Some(image_entry) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some(image.as_str()))
            else {
                continue;
            };
            let Some(offset) = image_entry.offset else {
                continue;
            };
            let Ok(node) = archive.parse_image_at(offset) else {
                continue;
            };
            return Some((node, archive.source().clone()));
        }
    }
    None
}

/// 장비 한 개의 툴팁 텍스트/아이콘을 만든다.
pub fn build_gear_tooltip(
    data_root: &std::path::Path,
    slot: &str,
    item_id: i32,
    title: &str,
) -> Option<GearTooltip> {
    let (node, source) = find_item_image(data_root, slot, item_id)?;
    let info = child(&node, "info").unwrap_or(&node);
    let mut lines = Vec::new();
    for key in PROP_ORDER {
        if let Some(value) = int(info, key) {
            if let Some(line) = prop_line(key, value) {
                lines.push(line);
            }
        }
    }
    let icon = child(info, "icon")
        .and_then(|n| n.first_canvas())
        .and_then(|png| wz_image::decode_png_ref_to_rgba(&png, &source).ok());
    Some(GearTooltip {
        title: title.to_string(),
        lines,
        icon,
    })
}

/// 툴팁을 게임 폰트 비트맵으로 렌더한다.
pub fn render_gear_tooltip(
    data_root: &std::path::Path,
    tooltip: &GearTooltip,
) -> Option<(u32, u32, Vec<u8>)> {
    crate::loader::render_tooltip_bitmap(
        data_root,
        &tooltip.title,
        &tooltip.lines,
        tooltip.icon.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> Option<std::path::PathBuf> {
        std::env::var_os("WZ_DATA_ROOT")
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists())
    }

    #[test]
    fn weapon_tooltip_lines_and_bitmap() {
        let Some(root) = root() else { return };
        let tooltip = build_gear_tooltip(&root, "Weapon", 1302000, "테스트 검").unwrap();
        assert!(tooltip.lines.iter().any(|l| l == "공격력 : +15"));
        assert!(tooltip
            .lines
            .iter()
            .any(|l| l == "업그레이드 가능 횟수 : 8"));
        assert!(tooltip.icon.is_some());
        let (w, h, rgba) = render_gear_tooltip(&root, &tooltip).unwrap();
        assert!(w > 100 && h > 20);
        assert!(rgba.chunks(4).any(|p| p[3] != 0));
    }
}
