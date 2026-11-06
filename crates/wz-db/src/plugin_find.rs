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

/// `PluginManager.GetPluginFiles` — `Plugin/` 아래 `WzComparerR2.*.dll`.
///
/// 원본은 `Directory.GetFiles(dir, "WzComparerR2.*.dll", AllDirectories)`를
/// 쓰므로 하위 디렉터리까지 포함한다. 순서는 OS 열거 순서라 보장이 없어
/// 여기서는 정렬해 돌려준다.
pub fn plugin_files(plugin_dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![plugin_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let lower = name.to_ascii_lowercase();
            if lower.starts_with("wzcomparerr2.") && lower.ends_with(".dll") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// 플러그인 메타데이터 (`PluginEntry`의 기본 프로퍼티).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub file_name: String,
    /// `AssemblyTitleAttribute.Title`
    pub name: Option<String>,
    /// `AssemblyCompanyAttribute.Company`
    pub author: Option<String>,
    /// `AssemblyName.Version`
    pub version: Option<String>,
    /// `AssemblyInformationalVersionAttribute` 우선, 없으면 `AssemblyFileVersion`
    pub file_version: Option<String>,
}

impl PluginMetadata {
    /// `PluginEntry`의 기본 프로퍼티 규칙.
    ///
    /// - `Name`  = `AssemblyTitle`
    /// - `Author` = `AssemblyCompany`
    /// - `FileVersion` = `InformationalVersion`이 비어있지 않으면 그 값,
    ///   아니면 `AssemblyFileVersion`
    #[allow(clippy::too_many_arguments)]
    pub fn from_attributes(
        file_name: impl Into<String>,
        title: Option<String>,
        company: Option<String>,
        version: Option<String>,
        informational_version: Option<String>,
        file_version: Option<String>,
    ) -> Self {
        let file_version = informational_version
            .filter(|s| !s.is_empty())
            .or(file_version);
        Self {
            file_name: file_name.into(),
            name: title,
            author: company,
            version,
            file_version,
        }
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

#[cfg(test)]
mod plugin_file_tests {
    use super::*;

    fn touch(dir: &std::path::Path, rel: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, b"x").unwrap();
    }

    #[test]
    fn metadata_prefers_informational_version() {
        let meta = PluginMetadata::from_attributes(
            "WzComparerR2.X.dll",
            Some("플러그인".into()),
            Some("작성자".into()),
            Some("1.2.3.4".into()),
            Some("1.2.3-beta".into()),
            Some("1.2.3.0".into()),
        );
        assert_eq!(meta.name.as_deref(), Some("플러그인"));
        assert_eq!(meta.author.as_deref(), Some("작성자"));
        assert_eq!(meta.file_version.as_deref(), Some("1.2.3-beta"));
        let meta = PluginMetadata::from_attributes(
            "x.dll",
            None,
            None,
            None,
            Some(String::new()),
            Some("9.9.9.9".into()),
        );
        assert_eq!(meta.file_version.as_deref(), Some("9.9.9.9"));
    }

    #[test]
    fn finds_pattern_recursively() {
        let base = std::env::temp_dir().join(format!("wz-plugin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        touch(&base, "WzComparerR2.A.dll");
        touch(&base, "sub/WzComparerR2.B.dll");
        touch(&base, "Other.dll");
        touch(&base, "WzComparerR2.C.txt");
        let found = plugin_files(&base);
        assert_eq!(found.len(), 2, "{found:?}");
        let _ = std::fs::remove_dir_all(&base);
    }
}
