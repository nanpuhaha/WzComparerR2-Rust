//! `PluginManager.FindWz` 경로 조회.
//!
//! 원본은 `Wz_Type` 별로 열린 `Wz_File`을 고른 뒤 `fullPath`의 나머지
//! 세그먼트를 따라 내려가고, 중간에 `.img`를 만나면 추출해서 계속한다
//! (`MainForm.CharaSimLoader_WzFileFinding` + `Wz_Node.FindNodeByPath`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use wz_core::dirtree::DirTree;
use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundNode {
    /// 실제로 찾은 노드 이름.
    pub name: String,
    /// `null`/`int:3`/`string:abc`/`canvas`/`sound`/`other`.
    pub value: String,
    pub children: usize,
    /// 중간에 지나간 세그먼트 수.
    pub depth: usize,
}

/// 타입 이름 → 열린 WZ 파일.
#[derive(Default)]
pub struct PluginWzSet {
    files: BTreeMap<String, (WzArchive, DirTree, String)>,
}

impl PluginWzSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// `Wz_Type` 이름(대소문자 무시)으로 WZ 파일을 등록한다.
    pub fn add(&mut self, wz_type: &str, path: &Path) -> Result<(), wz_error::WzError> {
        let archive = WzArchive::open(path)?;
        let tree = archive.read_tree()?;
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        self.files
            .insert(wz_type.to_ascii_lowercase(), (archive, tree, file_name));
        Ok(())
    }

    pub fn find(&self, full_path: &str) -> Option<FoundNode> {
        let parts: Vec<&str> = full_path
            .split(['/', '\\'])
            .filter(|s| !s.is_empty())
            .collect();
        let (wz_type, rest) = parts.split_first()?;
        let (archive, tree, file_name) = self.files.get(&wz_type.to_ascii_lowercase())?;
        if rest.is_empty() {
            return Some(FoundNode {
                name: file_name.clone(),
                value: "other".into(),
                children: tree.root.entries.len(),
                depth: 0,
            });
        }

        let mut current: Option<Node> = None;
        for (index, part) in rest.iter().enumerate() {
            let node = match &current {
                None => {
                    let entry = tree
                        .root
                        .entries
                        .iter()
                        .find(|e| e.name.as_deref() == Some(*part))?;
                    if entry.node_type == 4 {
                        let mut parsed = archive.parse_image_at(entry.offset?).ok()?;
                        parsed.name = std::sync::Arc::from(*part);
                        parsed
                    } else {
                        // 디렉터리는 이 클라이언트의 분할 WZ 루트에서 바로
                        // 내려갈 수 없다. 찾은 것으로만 보고한다.
                        return Some(FoundNode {
                            name: (*part).to_string(),
                            value: "other".into(),
                            children: 0,
                            depth: index + 1,
                        });
                    }
                }
                Some(parent) => {
                    let found = parent
                        .object_children()
                        .iter()
                        .find(|c| c.name.as_ref() == *part)?;
                    if let Some(ImageValue::Uol(target)) = &found.value {
                        // UOL은 문자열 링크만 기록한다.
                        return Some(FoundNode {
                            name: found.name.to_string(),
                            value: format!("string:{target}"),
                            children: found.object_children().len(),
                            depth: index + 1,
                        });
                    }
                    found.clone()
                }
            };

            if index + 1 == rest.len() {
                return Some(FoundNode {
                    name: node.name.to_string(),
                    value: classify(&node),
                    children: node.object_children().len(),
                    depth: index + 1,
                });
            }
            current = Some(node);
        }
        None
    }
}

fn classify(node: &Node) -> String {
    match &node.value {
        None => "null".to_string(),
        Some(ImageValue::Int(v)) => format!("int:{v}"),
        Some(ImageValue::Short(v)) => format!("int:{v}"),
        Some(ImageValue::Long(v)) => format!("int:{v}"),
        Some(ImageValue::Uint(v)) => format!("int:{v}"),
        Some(ImageValue::Float(v)) => format!("float:{v}"),
        Some(ImageValue::Double(v)) => format!("float:{v}"),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => format!("string:{s}"),
        Some(ImageValue::Null) | Some(ImageValue::Property(_)) => "null".to_string(),
        Some(ImageValue::Png(_)) => "canvas".to_string(),
        Some(ImageValue::Sound(_)) => "sound".to_string(),
        _ => "other".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_scalars() {
        let mut node = Node::new("x");
        node.value = Some(ImageValue::Int(7));
        assert_eq!(classify(&node), "int:7");
        node.value = Some(ImageValue::Str("a".into()));
        assert_eq!(classify(&node), "string:a");
        assert_eq!(classify(&Node::new("y")), "null");
    }
}
