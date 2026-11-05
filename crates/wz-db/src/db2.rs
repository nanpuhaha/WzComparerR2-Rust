//! DB2 스타일 노드 데이터 테이블.
//!
//! 원본 `DB2Form`의 39개 전용 그리드 전체는 아니지만, 선택한 WZ 이미지의
//! 노드를 `path/name/type/value` 행으로 평탄화해 검색/비교 가능한 뷰를 만든다.

use serde::{Deserialize, Serialize};
use wz_core::image::{ImageValue, Node};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2Row {
    pub path: String,
    pub name: String,
    pub node_type: String,
    pub value: String,
    pub children: usize,
}

fn value_text(value: Option<&ImageValue>) -> String {
    match value {
        None => String::new(),
        Some(ImageValue::Null) => "null".into(),
        Some(ImageValue::Short(v)) => v.to_string(),
        Some(ImageValue::Int(v)) => v.to_string(),
        Some(ImageValue::Long(v)) => v.to_string(),
        Some(ImageValue::Uint(v)) => v.to_string(),
        Some(ImageValue::Float(v)) => v.to_string(),
        Some(ImageValue::Double(v)) => v.to_string(),
        Some(ImageValue::Bool(v)) => v.to_string(),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.to_string(),
        Some(ImageValue::Vector(v)) => format!("{},{}", v.x, v.y),
        Some(ImageValue::Png(p)) => {
            format!(
                "{}x{} format={} len={}",
                p.width, p.height, p.format, p.data_len
            )
        }
        Some(ImageValue::RawData(r)) => format!("raw len={}", r.len),
        Some(ImageValue::Video(v)) => format!("video len={}", v.len),
        Some(ImageValue::Sound(s)) => {
            format!("sound len={} duration={}", s.data_len, s.duration)
        }
        Some(ImageValue::Convex(points)) => format!("convex {}", points.len()),
        Some(ImageValue::Property(_)) => String::new(),
    }
}

fn flatten_node(node: &Node, parent: &str, out: &mut Vec<Db2Row>) {
    let path = if parent.is_empty() {
        node.name.to_string()
    } else {
        format!("{parent}/{}", node.name)
    };
    out.push(Db2Row {
        path: path.clone(),
        name: node.name.to_string(),
        node_type: match &node.value {
            None | Some(ImageValue::Property(_)) => "Directory".into(),
            Some(v) => v.type_name().to_string(),
        },
        value: value_text(node.value.as_ref()),
        children: node.object_children().len()
            + match &node.value {
                Some(ImageValue::Png(png)) => png
                    .sub_property
                    .as_ref()
                    .map(|sub| sub.object_children().len())
                    .unwrap_or(0),
                _ => 0,
            },
    });
    for child in node.object_children() {
        flatten_node(child, &path, out);
    }
    if let Some(ImageValue::Png(png)) = &node.value {
        if let Some(sub) = &png.sub_property {
            for child in sub.object_children() {
                flatten_node(child, &path, out);
            }
        }
    }
}

/// 이미지 루트를 DB2 행 목록으로 평탄화한다.
pub fn image_rows(root: &Node) -> Vec<Db2Row> {
    let mut rows = Vec::new();
    flatten_node(root, "", &mut rows);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_paths_and_values() {
        let mut root = Node::new("img");
        let mut info = Node::new("info");
        let mut level = Node::new("level");
        level.value = Some(ImageValue::Int(120));
        info.children.push(level);
        root.children.push(info);

        let rows = image_rows(&root);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].path, "img/info");
        assert_eq!(rows[2].node_type, "Int");
        assert_eq!(rows[2].value, "120");
    }
}
