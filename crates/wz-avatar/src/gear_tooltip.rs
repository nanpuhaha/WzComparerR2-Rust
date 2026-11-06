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

/// 원본 `ItemStringHelper.GetGearPropString`(signFlag=0) 그대로.
///
/// `reqLevel`/`tuc`/`incACC`/`incEVA`/`incMDD`는 원본에서 주석 처리되어
/// `null`을 돌려준다. 툴팁 전용 라벨은 [`tooltip_stat_line`]이 담당한다.
pub fn gear_prop_line(key: &str, value: i64) -> Option<String> {
    let sign = if value > 0 { "+" } else { "" };
    let plain = |label: &str| Some(format!("{label} : {sign}{value}"));
    let percent = |label: &str| Some(format!("{label} : {sign}{value}%"));
    Some(match key {
        "incSTR" => plain("STR")?,
        "incSTRr" => percent("STR")?,
        "incDEX" => plain("DEX")?,
        "incDEXr" => percent("DEX")?,
        "incINT" => plain("INT")?,
        "incINTr" => percent("INT")?,
        "incLUK" => plain("LUK")?,
        "incLUKr" => percent("LUK")?,
        "incAllStat" => plain("올스탯")?,
        "statR" => percent("올스탯")?,
        "incMHP" => plain("최대 HP")?,
        "incMHPr" => percent("최대 HP")?,
        "incMMP" => plain("최대 MP")?,
        "incMMPr" => percent("최대 MP")?,
        "incMDF" => plain("MaxDF")?,
        "incPAD" | "incWAT" => plain("공격력")?,
        "incPADr" => percent("공격력")?,
        "incMAD" => plain("마력")?,
        "incMADr" => percent("마력")?,
        "incPDD" => plain("방어력")?,
        "incPDDr" => percent("방어력")?,
        "incSpeed" => plain("이동속도")?,
        "incJump" => plain("점프력")?,
        "incCraft" => plain("손재주")?,
        "damR" | "incDAMr" => percent("데미지")?,
        "incCr" | "incCRT" => percent("크리티컬 확률")?,
        "incCDr" | "incCRD" => percent("크리티컬 데미지")?,
        "knockback" => format!("직접 타격시 {value}%의 확률로 넉백"),
        "incPVPDamage" => format!("대난투 시 추가 공격력 {sign} {value}"),
        "incPQEXPr" => format!("파티퀘스트 경험치 {value}% 증가"),
        "incEXPr" => percent("추가 경험치")?,
        "incBDR" | "bdR" => format!("보스 몬스터 공격 시 데미지 +{value}%"),
        "incIMDR" | "imdR" => format!("몬스터 방어율 무시 : +{value}%"),
        "reduceReq" => format!("착용 레벨 감소 : - {value}"),
        "nbdR" => format!("일반 몬스터 공격 시 데미지 : +{value}%"),
        "only" => zero_none(value, "중복 소지 불가")?,
        "tradeBlock" => zero_none(value, "교환 불가")?,
        "equipTradeBlock" => zero_none(value, "장착 시 교환 불가")?,
        "accountSharable" => zero_none(value, "월드 내 나의 캐릭터 간 이동만 가능")?,
        "sharableOnce" => {
            zero_none(value, "월드 내 나의 캐릭터 간 1회 이동 가능\n(이동 후 교환불가)")?
        }
        "onlyEquip" => zero_none(value, "중복 장착 불가")?,
        "notExtend" => zero_none(value, "유효기간 연장 불가")?,
        "accountSharableAfterExchange" => zero_none(
            value,
            "1회 교환 가능\n(거래 후 월드 내 나의 캐릭터 간 이동만 가능)",
        )?,
        "noPrism" => zero_none(value, "염색하기 불가")?,
        "mintable" => zero_none(value, "민팅 가능")?,
        "tradeAvailable" => match value {
            1 => "#c카르마의 가위 또는 실버 카르마의 가위를 사용하면 1회 교환이 가능하게 할 수 있습니다.#".into(),
            2 => "#c플래티넘 카르마의 가위를 사용하면 1회 교환이 가능하게 할 수 있습니다.#".into(),
            _ => return None,
        },
        "accountShareTag" if value == 1 => {
            "#c쉐어 네임 텍을 사용하면 월드 내 나의 캐릭터 간 1회 이동할 수 있습니다.#".into()
        }
        "noPotential" => zero_none(value, "잠재능력 설정 불가")?,
        "fixedPotential" => zero_none(value, "잠재능력 재설정 불가")?,
        "superiorEqp" => {
            zero_none(value, "아이템 강화 성공시 더욱 높은 효과를 받을 수 있습니다.")?
        }
        "nActivatedSocket" => zero_none(value, "#c可以镶嵌星岩#")?,
        "jokerToSetItem" => zero_none(
            value,
            "#c3개 이상 착용하고 있는 모든 세트 아이템에 포함되는 럭키 아이템! (단, 2개 이상의 럭키 아이템 착용 시 1개만 효과 적용.)#",
        )?,
        "abilityTimeLimited" => zero_none(value, "기간 한정 능력치")?,
        "blockGoldHammer" => zero_none(value, "황금망치 사용 불가")?,
        "colorvar" => zero_none(value, "#c该装备可通过染色颜料来变更颜色.#")?,
        "cantRepair" => zero_none(value, "수리 불가")?,
        "noLookChange" => zero_none(value, "훈장 신비의 모루 사용 불가")?,
        "reissueBan" => zero_none(value, "재발급 불가")?,
        "incAllStat_incMHP25" => {
            format!("올스탯: {sign}{value}, 최대 HP : {sign}{}", value * 25)
        }
        "incAllStat_incMHP50_incMMP50" => format!(
            "올스탯: {sign}{value}, 최대 HP / 최대 MP : {sign}{}",
            value * 50
        ),
        "incMHP_incMMP" => format!("최대 HP / 최대 MP : {sign}{value}"),
        "incMHPr_incMMPr" => format!("최대 HP / 최대 MP : {sign}{value}%"),
        "incPAD_incMAD" | "incAD" => format!("공격력 / 마력 : {sign}{value}"),
        "incPDD_incMDD" => format!("방어력 : {sign}{value}"),
        "incARC" => format!("ARC : {sign}{value}"),
        "incAUT" => format!("AUT : {sign}{value}"),
        "Etuc" => format!("익셉셔널 강화가 가능합니다. (최대 : {value}회)"),
        "CuttableCount" => format!("가위 사용 가능 횟수：{value}회"),
        _ => return None,
    })
}

fn zero_none(value: i64, text: &str) -> Option<String> {
    if value == 0 {
        None
    } else {
        Some(text.to_string())
    }
}

/// 툴팁 표시용 확장 라벨. 원본 `GetGearPropString`이 주석 처리한 항목과
/// 착용 레벨/업그레이드 횟수를 함께 보여준다.
pub fn tooltip_stat_line(key: &str, value: i64) -> Option<String> {
    let sign = if value > 0 { "+" } else { "" };
    Some(match key {
        "reqLevel" => format!("착용 레벨 : {value}"),
        "tuc" => format!("업그레이드 가능 횟수 : {value}"),
        "incACC" => format!("명중치 : {sign}{value}"),
        "incEVA" => format!("회피치 : {sign}{value}"),
        "incMDD" => format!("마법방어력 : {sign}{value}"),
        _ => gear_prop_line(key, value)?,
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
                    if let Some(line) = tooltip_stat_line(key, value) {
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
            if let Some(line) = tooltip_stat_line(stat.name.as_ref(), value) {
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
            if let Some(line) = tooltip_stat_line(key, value) {
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

/// 원본 `ItemStringHelper.GetItemPropString` — 아이템 공통 속성 문자열.
///
/// `GetGearPropString`을 쓰는 항목은 [`gear_prop_line`]에 위임한다.
/// 값이 0일 때 `null`을 돌려주는 항목이 많으므로 `Option`을 반환한다.
pub fn item_prop_string(key: &str, value: i64) -> Option<String> {
    Some(match key {
        "tradeBlock" => gear_prop_line("tradeBlock", value)?,
        "useTradeBlock" => {
            if value == 0 {
                return None;
            }
            "사용시 교환 불가".to_string()
        }
        "tradeAvailable" => gear_prop_line("tradeAvailable", value)?,
        "only" => gear_prop_line("only", value)?,
        "accountSharable" => gear_prop_line("accountSharable", value)?,
        "sharableOnce" => gear_prop_line("sharableOnce", value)?,
        "exchangeableOnce" => {
            if value == 0 {
                return None;
            }
            "1회 교환가능 (사용 또는 거래 후 교환불가)".to_string()
        }
        "accountSharableAfterExchange" => gear_prop_line("accountSharableAfterExchange", value)?,
        "quest" => {
            if value == 0 {
                return None;
            }
            "퀘스트 아이템".to_string()
        }
        "pquest" => {
            if value == 0 {
                return None;
            }
            "파티 퀘스트 아이템".to_string()
        }
        // 원본은 이 항목만 값이 0/1 모두 문자열을 돌려준다.
        "multiPet" => {
            if value == 0 {
                "일반펫 (다른 일반펫과 중복 사용불가)".to_string()
            } else {
                "멀티펫 (다른 펫과 최대 3개 중복 사용가능)".to_string()
            }
        }
        "permanent" => {
            if value == 0 {
                return None;
            }
            "마법의 시간이 끝나지 않는 미라클 펫입니다.".to_string()
        }
        "mintable" => gear_prop_line("mintable", value)?,
        _ => return None,
    })
}

/// `ItemTooltipRender22.GetItemTopAttributeString`의 항목 순서대로 태그를 만든다.
///
/// `props`는 `(키, 값)` 목록이고, `item_id`/`is_pet`은 원본의 특수 분기에 쓴다.
pub fn item_top_attribute_lines(
    props: &std::collections::BTreeMap<String, i64>,
    item_id: i32,
    is_pet: bool,
) -> Vec<String> {
    let get = |key: &str| props.get(key).copied().unwrap_or(0);
    let mut tags = Vec::new();
    for key in ["quest", "pquest", "tradeBlock"] {
        if get(key) != 0 {
            if let Some(text) = item_prop_string(key, get(key)) {
                tags.push(text);
            }
        }
    }
    if get("useTradeBlock") != 0 {
        if let Some(text) = item_prop_string("useTradeBlock", get("useTradeBlock")) {
            tags.push(text);
        }
    } else if matches!(item_id / 10000, 501 | 502 | 516) {
        if let Some(text) = item_prop_string("tradeBlock", 1) {
            tags.push(text);
        }
    }
    if get("accountSharable") != 0 {
        if get("exp_minLev") != 0 {
            if let Some(text) = item_prop_string("useTradeBlock", 1) {
                tags.push(text);
            }
        }
        if get("sharableOnce") != 0 {
            if let Some(text) = item_prop_string("sharableOnce", get("sharableOnce")) {
                tags.push(text);
            }
        } else if let Some(text) = item_prop_string("accountSharable", get("accountSharable")) {
            tags.push(text);
        }
    }
    if get("exchangeableOnce") != 0 {
        if let Some(text) = item_prop_string("exchangeableOnce", get("exchangeableOnce")) {
            tags.push(text);
        }
    }
    if props.contains_key("multiPet") {
        if let Some(text) = item_prop_string("multiPet", get("multiPet")) {
            tags.push(text);
        }
    } else if is_pet {
        if let Some(text) = item_prop_string("multiPet", 0) {
            tags.push(text);
        }
    }
    if get("accountSharableAfterExchange") != 0 {
        if let Some(text) = item_prop_string(
            "accountSharableAfterExchange",
            get("accountSharableAfterExchange"),
        ) {
            tags.push(text);
        }
    }
    if props.contains_key("mintable") {
        if let Some(text) = item_prop_string("mintable", get("mintable")) {
            tags.push(text);
        }
    }
    tags
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
