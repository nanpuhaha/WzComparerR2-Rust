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
    pub set_name: Option<String>,
    pub set_complete: Option<i32>,
    pub set_effects: Vec<(i32, Vec<String>)>,
    pub potentials: Vec<Vec<String>>,
    pub max_star: Option<i32>,
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

fn find_image(root: &std::path::Path, subdir: &str, image_name: &str) -> Option<Node> {
    let dir = root.join(subdir);
    for base in [dir.clone(), dir.join("_Canvas")] {
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
                .find(|e| e.name.as_deref() == Some(image_name))
            else {
                continue;
            };
            let Some(offset) = image_entry.offset else {
                continue;
            };
            if let Ok(node) = archive.parse_image_at(offset) {
                return Some(node);
            }
        }
    }
    None
}

type SetData = (String, i32, Vec<(i32, Vec<String>)>);

fn set_tooltip(data_root: &std::path::Path, set_id: i32) -> Option<SetData> {
    let root = find_image(data_root, "Etc", "SetItemInfo.img")?;
    let set = child(&root, &set_id.to_string())?;
    let name = child(set, "setItemName")
        .and_then(|n| match &n.value {
            Some(ImageValue::Str(s)) => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_else(|| set_id.to_string());
    let complete = child(set, "completeCount")
        .and_then(|n| match &n.value {
            Some(ImageValue::Short(v)) => Some(i32::from(*v)),
            Some(ImageValue::Int(v)) => Some(*v),
            _ => None,
        })
        .unwrap_or(0);
    let mut effects = Vec::new();
    if let Some(effect_root) = child(set, "Effect") {
        let mut nodes: Vec<&Node> = effect_root
            .object_children()
            .iter()
            .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
            .collect();
        nodes.sort_by_key(|c| c.name.as_ref().parse::<i32>().unwrap_or(0));
        for node in nodes {
            let count = node.name.as_ref().parse::<i32>().unwrap_or(0);
            let mut lines = Vec::new();
            const SET_ORDER: &[&str] = &[
                "incPAD",
                "incMAD",
                "incAllStat",
                "incMHP",
                "incMMP",
                "incSTR",
                "incDEX",
                "incINT",
                "incLUK",
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
            ];
            for key in SET_ORDER {
                let stat = node
                    .object_children()
                    .iter()
                    .find(|c| c.name.as_ref() == *key);
                if let Some(value) = stat.and_then(|c| match &c.value {
                    Some(ImageValue::Int(v)) => Some(i64::from(*v)),
                    Some(ImageValue::Short(v)) => Some(i64::from(*v)),
                    Some(ImageValue::Long(v)) => Some(*v),
                    _ => None,
                }) {
                    if let Some(line) = prop_line(key, value) {
                        lines.push(line);
                    }
                }
            }
            if !lines.is_empty() {
                effects.push((count, lines));
            }
        }
    }
    Some((name, complete, effects))
}

fn potential_lines(data_root: &std::path::Path, code: i32, level: i32) -> Vec<String> {
    let Some(root) = find_image(data_root, "Item", "ItemOption.img") else {
        return Vec::new();
    };
    let Some(option) = child(&root, &format!("{code:06}")) else {
        return Vec::new();
    };
    let Some(level_node) = child(option, "level").and_then(|l| child(l, &level.to_string())) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    for stat in level_node.object_children() {
        if let Some(value) = int(level_node, stat.name.as_ref()) {
            if let Some(line) = prop_line(stat.name.as_ref(), value) {
                lines.push(line);
            }
        }
    }
    lines
}

fn max_star(info: &Node) -> i32 {
    let req_level = int(info, "reqLevel").unwrap_or(0) as i32;
    let tuc = int(info, "tuc").unwrap_or(0);
    let cash = int(info, "cash").unwrap_or(0) != 0;
    let only_upgrade = int(info, "onlyUpgrade").unwrap_or(0) != 0;
    let superior = int(info, "superiorEqp").unwrap_or(0) != 0;
    if tuc <= 0 || cash || only_upgrade {
        return 0;
    }
    const STAR: &[[i32; 3]] = &[
        [0, 5, 3],
        [95, 8, 5],
        [108, 10, 8],
        [118, 15, 10],
        [128, 20, 12],
        [138, 30, 15],
    ];
    let mut value = 0;
    for row in STAR {
        if req_level >= row[0] {
            value = if superior { row[2] } else { row[1] };
        } else {
            break;
        }
    }
    value
}

/// 장비 한 개의 툴팁 텍스트/아이콘을 만든다.
pub fn build_gear_tooltip(
    data_root: &std::path::Path,
    slot: &str,
    item_id: i32,
    title: &str,
) -> Option<GearTooltip> {
    build_gear_tooltip_full(data_root, slot, item_id, title, &[])
}

/// 잠재 옵션까지 포함한 gear tooltip.
pub fn build_gear_tooltip_full(
    data_root: &std::path::Path,
    slot: &str,
    item_id: i32,
    title: &str,
    potentials: &[(i32, i32)],
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
    let set = int(info, "setItemID")
        .filter(|id| *id > i32::MIN as i64)
        .and_then(|id| set_tooltip(data_root, id as i32));
    let mut potential_sections = Vec::new();
    for (code, level) in potentials {
        let lines = potential_lines(data_root, *code, *level);
        if !lines.is_empty() {
            potential_sections.push(lines);
        }
    }
    let star = max_star(info);
    let set_name = set.as_ref().map(|s| s.0.clone());
    let set_complete = set.as_ref().map(|s| s.1);
    let set_effects = set.map(|s| s.2).unwrap_or_default();
    if let Some(name) = &set_name {
        lines.push(format!("{name} ({}세트)", set_complete.unwrap_or(0)));
        for (count, effect_lines) in &set_effects {
            lines.push(format!("{count}세트 효과"));
            lines.extend(effect_lines.iter().cloned());
        }
    }
    for section in &potential_sections {
        lines.push("잠재능력".into());
        lines.extend(section.iter().cloned());
    }
    if let Some(star) = (star > 0).then_some(star) {
        lines.push(format!("최대 스타포스 : {star}"));
    }
    Some(GearTooltip {
        title: title.to_string(),
        lines,
        icon,
        set_name,
        set_complete,
        set_effects,
        potentials: potential_sections,
        max_star: (star > 0).then_some(star),
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
        let full =
            build_gear_tooltip_full(&root, "Ring1", 1114302, "결속의 반지", &[(1, 1)]).unwrap();
        assert_eq!(full.set_name.as_deref(), Some("결속의 반지"));
        assert_eq!(full.set_complete, Some(5));
        assert!(full.set_effects.iter().any(|(n, _)| *n == 4));
        assert!(full.lines.iter().any(|l| l == "4세트 효과"));
        assert!(full.lines.iter().any(|l| l == "공격력 : +2"));
        let set_expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/set_minus1.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            serde_json::json!({
                "name": full.set_name,
                "complete": full.set_complete,
                "effects": full.set_effects.iter().map(|(count, lines)| serde_json::json!({
                    "count": count, "lines": lines
                })).collect::<Vec<_>>(),
            }),
            set_expected
        );
        let potential_expected: Vec<String> = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/potential_1_1.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(full.potentials[0], potential_expected);
        assert!(full.max_star.is_none());
        let (w, h, rgba) = render_gear_tooltip(&root, &tooltip).unwrap();
        assert!(w > 100 && h > 20);
        assert!(rgba.chunks(4).any(|p| p[3] != 0));
    }
}
