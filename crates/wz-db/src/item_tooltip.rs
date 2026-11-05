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
            tags.push("월드 내 나의 캐릭터 간 1회 이동 가능 (이동 후 교환불가)".to_string());
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
        tags.push("1회 교환 가능 (거래 후 월드 내 나의 캐릭터 간 이동만 가능)".to_string());
    }
    if value(info, "mintable").unwrap_or(0) != 0 {
        tags.push("민팅 가능".to_string());
    }
    tags
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
    fn trade_block_tag() {
        let item = item_with(&[("tradeBlock", 1)]);
        assert_eq!(item_attribute_lines(&item, 2_000_012), vec!["교환 불가"]);
    }

    #[test]
    fn account_share_tags() {
        let item = item_with(&[("accountSharable", 1), ("sharableOnce", 1)]);
        assert_eq!(
            item_attribute_lines(&item, 2_000_012),
            vec!["월드 내 나의 캐릭터 간 1회 이동 가능 (이동 후 교환불가)"]
        );
    }
}
