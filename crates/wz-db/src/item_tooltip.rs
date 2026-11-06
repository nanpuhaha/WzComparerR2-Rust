//! ItemTooltip 속성 태그 (`ItemTooltipRender2.GetItemAttributeString`).
//!
//! 원본은 `Item.CreateFromNode`의 `Props`를 읽어 거래/소지 제한 태그를
//! 고정 순서로 만든다. 여기서는 WZ `info` 노드에서 같은 값을 직접 읽는다.

use crate::linker::StringLinker;
use serde::{Deserialize, Serialize};
use wz_core::image::{ImageValue, Node};

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn value(node: &Node, key: &str) -> Option<i64> {
    child(node, key).and_then(|c| match &c.value {
        Some(ImageValue::Int(v)) => Some(i64::from(*v)),
        Some(ImageValue::Short(v)) => Some(i64::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v),
        Some(ImageValue::Uint(v)) => Some(i64::from(*v)),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.parse().ok(),
        _ => None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemTooltipView {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub auto_desc: String,
    pub attributes: Vec<String>,
}

/// ItemTooltip의 이름/설명/속성 태그를 만든다.
pub fn build_item_tooltip(
    item: &Node,
    item_id: &str,
    linker: &StringLinker,
) -> Option<ItemTooltipView> {
    let id: i32 = item_id.parse().ok()?;
    let sr = linker.string_item.get(&id);
    Some(ItemTooltipView {
        id: item_id.to_string(),
        name: sr.map(|sr| sr.name.clone()).unwrap_or_default(),
        desc: sr.and_then(|sr| sr.desc.clone()).unwrap_or_default(),
        auto_desc: sr.and_then(|sr| sr.auto_desc.clone()).unwrap_or_default(),
        attributes: item_attribute_lines(item, id),
    })
}

/// `ItemStringHelper.GetItemCoreSpecString` — 코어 아이템 설명.
///
/// `kind`는 `ItemCoreSpecType` 이름(`Ctrl_mobLv` 등)이다.
pub fn item_core_spec_string(kind: &str, value: i32, desc: Option<&str>) -> Option<String> {
    let has_coda = desc
        .filter(|d| !d.is_empty())
        .and_then(|d| d.chars().last())
        .map(|c| {
            let code = c as u32;
            (0xAC00..=0xD7A3).contains(&code) && !(code - 0xAC00).is_multiple_of(28)
        })
        .unwrap_or(false);
    let with_coda = |text: &str| {
        if has_coda {
            format!("{text}으로")
        } else {
            format!("{text}로")
        }
    };
    Some(match kind {
        "Ctrl_mobLv" if value != 0 => format!("몬스터 레벨 {value} 증가"),
        "Ctrl_mobHPRate" if value != 0 => format!("몬스터 HP {value}% 증가"),
        "Ctrl_mobRate" if value != 0 => format!("몬스터 개체 수 {value}% 증가"),
        "Ctrl_mobRateSpecial" if value != 0 => format!("몬스터 개체 수 {value}% 추가 증가"),
        "Ctrl_change_Mob" => format!("{} 몬스터 이미지 변경", with_coda(desc?)),
        "Ctrl_change_BGM" => format!("{} 배경 음악 변경", with_coda(desc?)),
        "Ctrl_change_BackGrnd" => format!("{} 배경 이미지 변경", with_coda(desc?)),
        "Ctrl_partyExp" if value != 0 => format!("파티 경험치 {value}% 증가"),
        "Ctrl_partyExpSpecial" if value != 0 => format!("파티 경험치 {value}% 추가 증가"),
        "Ctrl_addMob" if value != 0 => format!("{}, 링크{value} 지역에 추가", desc?),
        "Ctrl_dropRate" if value != 0 => format!("드롭률 {value}% 증가"),
        "Ctrl_dropRateSpecial" if value != 0 => format!("드롭률 {value}% 추가 증가"),
        "Ctrl_dropRate_Herb" if value != 0 => format!("약초 드롭률 {value}% 증가"),
        "Ctrl_dropRate_Mineral" if value != 0 => format!("광물 드롭률 {value}% 증가"),
        "Ctrl_dropRareEquip" if value != 0 => "장비 아이템이 미확인 상태로 드롭".to_string(),
        "Ctrl_reward" | "Ctrl_addMission" => desc?.to_string(),
        _ => return None,
    })
}

/// `ItemStringHelper.GetSkillReqAmount` — 스킬 요구량 라벨.
///
/// 스킬 ID의 만 단위(`skillID / 10000`)로 동물 계열을 구분한다. 원본은
/// 11200/11210/11211/11212 외에는 `？？`로 표시한다.
pub fn skill_req_amount(skill_id: i32, req_amount: i32) -> String {
    let animal = match skill_id / 10000 {
        11200 => "巨熊",
        11210 => "雪豹",
        11211 => "猛禽",
        11212 => "猫咪",
        _ => "？？",
    };
    format!("[需要{animal}技能点: {req_amount}]")
}

/// `GetItemAttributeString`과 같은 순서/문구의 태그 목록.
pub fn item_attribute_lines(item: &Node, item_id: i32) -> Vec<String> {
    let info = child(item, "info").unwrap_or(item);
    let mut tags = Vec::new();
    if value(info, "only").unwrap_or(0) != 0 {
        tags.push("중복 소지 불가".to_string());
    }
    if value(info, "quest").unwrap_or(0) != 0 {
        tags.push("퀘스트 아이템".to_string());
    }
    if value(info, "pquest").unwrap_or(0) != 0 {
        tags.push("파티 퀘스트 아이템".to_string());
    }
    if value(info, "tradeBlock").unwrap_or(0) != 0 {
        tags.push("교환 불가".to_string());
    }
    if value(info, "useTradeBlock").unwrap_or(0) != 0 {
        tags.push("사용시 교환 불가".to_string());
    } else if matches!(item_id / 10000, 501 | 502 | 516) {
        tags.push("교환 불가".to_string());
    }
    if value(info, "accountSharable").unwrap_or(0) != 0 {
        if value(info, "exp_minLev").unwrap_or(0) != 0 {
            tags.push("사용시 교환 불가".to_string());
        }
        if value(info, "sharableOnce").unwrap_or(0) != 0 {
            tags.push("월드 내 나의 캐릭터 간 1회 이동 가능\n(이동 후 교환불가)".to_string());
        } else {
            tags.push("월드 내 나의 캐릭터 간 이동만 가능".to_string());
        }
    }
    if value(info, "exchangeableOnce").unwrap_or(0) != 0 {
        tags.push("1회 교환가능 (사용 또는 거래 후 교환불가)".to_string());
    }
    if let Some(multi_pet) = value(info, "multiPet") {
        tags.push(if multi_pet == 0 {
            "일반펫 (다른 일반펫과 중복 사용불가)".to_string()
        } else {
            "멀티펫 (다른 펫과 최대 3개 중복 사용가능)".to_string()
        });
    } else if item_id / 10000 == 500 {
        tags.push("일반펫 (다른 일반펫과 중복 사용불가)".to_string());
    }
    if value(info, "accountSharableAfterExchange").unwrap_or(0) != 0 {
        tags.push("1회 교환 가능\n(거래 후 월드 내 나의 캐릭터 간 이동만 가능)".to_string());
    }
    if value(info, "mintable").unwrap_or(0) != 0 {
        tags.push("민팅 가능".to_string());
    }
    tags
}

/// `ItemStringHelper.GetItemCategoryName` — 아이템 분류 한글명.
///
/// 원본 `Item.ItemType` 이름을 그대로 받는다(`Consume`, `Install`, `Etc`,
/// `Cash`, `Pet`). 그 외는 `None`.
pub fn item_category_name(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "Consume" => "소비",
        "Install" => "설치",
        "Etc" => "기타",
        "Cash" => "캐시",
        "Pet" => "펫",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_with(pairs: &[(&str, i32)]) -> Node {
        let mut item = Node::new("02000012");
        let mut info = Node::new("info");
        for (key, value) in pairs {
            let mut c = Node::new(*key);
            c.value = Some(ImageValue::Int(*value));
            info.children.push(c);
        }
        item.children.push(info);
        item
    }

    #[test]
    fn skill_req_amount_uses_ten_thousands() {
        assert_eq!(skill_req_amount(112_000_000, 7), "[需要巨熊技能点: 7]");
        assert_eq!(skill_req_amount(112_100_000, 1), "[需要雪豹技能点: 1]");
        assert_eq!(skill_req_amount(112_130_000, 2), "[需要？？技能点: 2]");
        // 11200/10000 == 1이라 소형 ID는 매칭되지 않는다(원본과 동일).
        assert_eq!(skill_req_amount(11_200, 3), "[需要？？技能点: 3]");
    }

    #[test]
    fn trade_block_tag() {
        let item = item_with(&[("tradeBlock", 1)]);
        assert_eq!(item_attribute_lines(&item, 2_000_012), vec!["교환 불가"]);
    }

    #[test]
    fn account_share_tags() {
        let item = item_with(&[("accountSharable", 1), ("sharableOnce", 1)]);
        assert_eq!(
            item_attribute_lines(&item, 2_000_012),
            vec!["월드 내 나의 캐릭터 간 1회 이동 가능\n(이동 후 교환불가)"]
        );
    }
}
