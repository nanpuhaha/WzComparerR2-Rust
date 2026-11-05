//! `WzVirtualNode` 병합 (`WzComparerR2/Comparer/WzVirtualNode.cs`).
//!
//! Base WZ 처럼 여러 WZ 파일에 나뉜 동일 트리를 하나의 가상 노드로 합친다.
//! 값/디렉터리 충돌 시 원본과 동일하게 오류를 낸다.

use wz_core::image::{ImageValue, Node};

#[derive(Debug, Clone, PartialEq)]
pub struct VirtualNode {
    pub name: String,
    /// 링크된 원본 값들 (없으면 None).
    pub values: Vec<Option<String>>,
    pub children: Vec<VirtualNode>,
}

fn value_signature(value: Option<&ImageValue>) -> Option<String> {
    Some(match value? {
        ImageValue::Null => "null".into(),
        ImageValue::Short(v) => v.to_string(),
        ImageValue::Int(v) => v.to_string(),
        ImageValue::Long(v) => v.to_string(),
        ImageValue::Uint(v) => v.to_string(),
        ImageValue::Float(v) => v.to_string(),
        ImageValue::Double(v) => v.to_string(),
        ImageValue::Bool(v) => v.to_string(),
        ImageValue::Str(s) | ImageValue::Uol(s) => s.to_string(),
        ImageValue::Vector(v) => format!("{},{}", v.x, v.y),
        ImageValue::Png(p) => format!("png:{}x{}:{}:{}", p.width, p.height, p.format, p.data_len),
        ImageValue::RawData(r) => format!("raw:{}", r.len),
        ImageValue::Video(v) => format!("video:{}", v.len),
        ImageValue::Sound(s) => format!("sound:{}", s.data_len),
        ImageValue::Convex(v) => format!("convex:{}", v.len()),
        ImageValue::Property(_) => "property".into(),
    })
}

fn merge_nodes(nodes: &[&Node]) -> Result<VirtualNode, String> {
    let first = nodes.first().ok_or("empty virtual node")?;
    let mut out = VirtualNode {
        name: first.name.to_string(),
        values: nodes
            .iter()
            .map(|n| value_signature(n.value.as_ref()))
            .collect(),
        children: Vec::new(),
    };
    for node in nodes {
        for child in node.object_children() {
            if let Some(existing) = out
                .children
                .iter_mut()
                .find(|c| c.name == child.name.as_ref())
            {
                let has_value = child.value.is_some();
                let has_dir = !child.object_children().is_empty();
                let existing_has_value = existing.values.iter().any(|v| v.is_some());
                let existing_has_dir = !existing.children.is_empty();
                if !has_value && !existing_has_value {
                    // 둘 다 디렉터리: 재귀 병합
                    merge_into(existing, child);
                } else if !has_dir && !existing_has_dir {
                    existing.values.push(value_signature(child.value.as_ref()));
                } else {
                    return Err(format!("WZ merge conflict at {}", child.name));
                }
            } else {
                out.children.push(merge_subtree(child));
            }
        }
    }
    Ok(out)
}

fn merge_into(existing: &mut VirtualNode, node: &Node) {
    existing.values.push(value_signature(node.value.as_ref()));
    for child in node.object_children() {
        if let Some(target) = existing
            .children
            .iter_mut()
            .find(|c| c.name == child.name.as_ref())
        {
            merge_into(target, child);
        } else {
            existing.children.push(merge_subtree(child));
        }
    }
}

fn merge_subtree(node: &Node) -> VirtualNode {
    VirtualNode {
        name: node.name.to_string(),
        values: vec![value_signature(node.value.as_ref())],
        children: node.object_children().iter().map(merge_subtree).collect(),
    }
}

/// 여러 원본 트리를 하나의 가상 루트로 병합한다.
pub fn merge(nodes: &[&Node]) -> Result<VirtualNode, String> {
    merge_nodes(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_core::image::Node;

    #[test]
    fn merges_disjoint_children() {
        let mut a = Node::new("Base");
        let mut info = Node::new("info");
        info.value = Some(ImageValue::Int(1));
        a.children.push(info);

        let mut b = Node::new("Base");
        let mut info = Node::new("info");
        info.value = Some(ImageValue::Int(1));
        b.children.push(info);
        let mut extra = Node::new("extra");
        extra.value = Some(ImageValue::Int(2));
        b.children.push(extra);

        let merged = merge(&[&a, &b]).unwrap();
        assert_eq!(merged.children.len(), 2);
        assert_eq!(merged.children[0].values.len(), 2);
    }

    #[test]
    fn merges_same_named_directories_recursively() {
        let mut a = Node::new("Base");
        let mut dir = Node::new("dir");
        let mut x = Node::new("x");
        x.value = Some(ImageValue::Int(1));
        dir.children.push(x);
        a.children.push(dir);

        let mut b = Node::new("Base");
        let mut dir = Node::new("dir");
        let mut x = Node::new("x");
        x.value = Some(ImageValue::Int(1));
        dir.children.push(x);
        let mut y = Node::new("y");
        y.value = Some(ImageValue::Int(2));
        dir.children.push(y);
        b.children.push(dir);

        let merged = merge(&[&a, &b]).unwrap();
        let dir = &merged.children[0];
        assert_eq!(dir.values.len(), 2);
        assert_eq!(dir.children.len(), 2);
    }

    #[test]
    fn conflicting_directory_and_value_errors() {
        let mut a = Node::new("Base");
        let mut child = Node::new("x");
        child.value = Some(ImageValue::Int(1));
        a.children.push(child);

        let mut b = Node::new("Base");
        let mut child = Node::new("x");
        child.children.push(Node::new("y"));
        b.children.push(child);

        assert!(merge(&[&a, &b]).is_err());
    }
}
