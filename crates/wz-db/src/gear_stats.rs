//! 장비 스탯 추출 (`CharaSim` 포트).
//!
//! `wz-cli` 의 `extract_gear_stats_from_node` 를 wz-db 로 이동한 deep module.
//! CLI 와 GUI 가 같은 함수를 쓰도록 한다 (아키텍처 감사 §3.3 deepening).

use wz_core::image::ImageValue;
use wz_core::image::Node;
use wz_core::text::sort_natural;

/// 노드의 `info` 프로퍼티에서 스칼라 스탯을 추출한다.
///
/// - `info` 자식이 있으면 그 children 을 사용한다.
/// - `info` 가 없으면 node 자체의 children 을 사용한다.
/// - 복합값(Property, Canvas 등) 은 건너뛴다 (스칼라만).
/// - 키는 natural sort 순서로 정렬된다.
pub fn extract_gear_props(node: &Node) -> serde_json::Map<String, serde_json::Value> {
    let info_node = node
        .children
        .iter()
        .find(|c| c.name.as_ref() == "info")
        .or_else(|| {
            if let Some(ImageValue::Property(inner)) = &node.value {
                inner.children.iter().find(|c| c.name.as_ref() == "info")
            } else {
                None
            }
        })
        .unwrap_or(node);

    let mut children: Vec<&Node> = info_node.children.iter().collect();
    if let Some(ImageValue::Property(inner)) = &info_node.value {
        children.extend(inner.children.iter());
    }
    sort_natural(&mut children, |c| c.name.as_ref());

    let mut props = serde_json::Map::new();
    for ch in children {
        let val = match &ch.value {
            Some(ImageValue::Short(v)) => serde_json::Value::Number((*v).into()),
            Some(ImageValue::Int(v)) => serde_json::Value::Number((*v).into()),
            Some(ImageValue::Long(v)) => serde_json::Value::Number((*v).into()),
            Some(ImageValue::Uint(v)) => serde_json::Value::Number((*v).into()),
            Some(ImageValue::Float(v)) => serde_json::Number::from_f64(*v as f64)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Some(ImageValue::Double(v)) => serde_json::Number::from_f64(*v)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Some(ImageValue::Bool(v)) => serde_json::Value::Bool(*v),
            Some(ImageValue::Str(s) | ImageValue::Uol(s)) => {
                serde_json::Value::String(s.to_string())
            }
            _ => continue,
        };
        props.insert(ch.name.to_string(), val);
    }
    props
}
