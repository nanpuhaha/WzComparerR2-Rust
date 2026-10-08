use serde::{Deserialize, Serialize};
use wz_core::image::Node as WzCoreNode;
use wz_db::{Arena, NodeId};

/// Type of difference between two nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffType {
    Append,
    Delete,
    Changed,
}

/// A diff result item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffItem {
    pub path: String,
    pub diff_type: DiffType,
    pub description: String,
}

/// Rayon-powered tree comparator
pub struct DiffEngine;

impl DiffEngine {
    /// Compare two `wz_core::image::Node` trees by adapting them into Arena storage
    pub fn diff_core_nodes(
        old_node: Option<&WzCoreNode>,
        new_node: Option<&WzCoreNode>,
        path: &str,
    ) -> Vec<DiffItem> {
        let (old_arena, old_id) = match old_node {
            Some(n) => {
                let (a, root) = Arena::from_wz_node(n);
                (Some(a), Some(root))
            }
            None => (None, None),
        };

        let (new_arena, new_id) = match new_node {
            Some(n) => {
                let (a, root) = Arena::from_wz_node(n);
                (Some(a), Some(root))
            }
            None => (None, None),
        };

        let empty = Arena::new();
        Self::diff_arena_nodes(
            old_arena.as_ref().unwrap_or(&empty),
            new_arena.as_ref().unwrap_or(&empty),
            old_id,
            new_id,
            path,
        )
    }

    /// Compare two `wz-db` Arena nodes recursively
    pub fn diff_arena_nodes(
        old_arena: &Arena,
        new_arena: &Arena,
        old_id: Option<NodeId>,
        new_id: Option<NodeId>,
        path: &str,
    ) -> Vec<DiffItem> {
        match (old_id, new_id) {
            (None, Some(nid)) => {
                let mut items = vec![DiffItem {
                    path: path.to_string(),
                    diff_type: DiffType::Append,
                    description: format!("Added node: {}", path),
                }];
                for &cid in new_arena.children(nid) {
                    if let Some(c_name) = new_arena.name(cid) {
                        let subpath = format!("{}/{}", path, c_name);
                        items.extend(Self::diff_arena_nodes(
                            old_arena,
                            new_arena,
                            None,
                            Some(cid),
                            &subpath,
                        ));
                    }
                }
                items
            }
            (Some(oid), None) => {
                let mut items = vec![DiffItem {
                    path: path.to_string(),
                    diff_type: DiffType::Delete,
                    description: format!("Deleted node: {}", path),
                }];
                for &cid in old_arena.children(oid) {
                    if let Some(c_name) = old_arena.name(cid) {
                        let subpath = format!("{}/{}", path, c_name);
                        items.extend(Self::diff_arena_nodes(
                            old_arena,
                            new_arena,
                            Some(cid),
                            None,
                            &subpath,
                        ));
                    }
                }
                items
            }
            (Some(oid), Some(nid)) => {
                let mut items = Vec::new();
                let o_val = old_arena.value(oid);
                let n_val = new_arena.value(nid);

                if o_val != n_val {
                    items.push(DiffItem {
                        path: path.to_string(),
                        diff_type: DiffType::Changed,
                        description: format!("Modified value at {}", path),
                    });
                }

                // Match children by name
                let mut child_names: Vec<String> = Vec::new();
                for &cid in old_arena.children(oid) {
                    if let Some(name) = old_arena.name(cid) {
                        if !child_names.iter().any(|n| n == name) {
                            child_names.push(name.to_string());
                        }
                    }
                }
                for &cid in new_arena.children(nid) {
                    if let Some(name) = new_arena.name(cid) {
                        if !child_names.iter().any(|n| n == name) {
                            child_names.push(name.to_string());
                        }
                    }
                }

                for name in &child_names {
                    let o_child = old_arena.find_child(oid, name);
                    let n_child = new_arena.find_child(nid, name);
                    let subpath = format!("{}/{}", path, name);
                    items.extend(Self::diff_arena_nodes(
                        old_arena, new_arena, o_child, n_child, &subpath,
                    ));
                }

                items
            }
            (None, None) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_core::image::ImageValue;

    #[test]
    fn test_core_node_diff() {
        let mut old_root = WzCoreNode::new("Character");
        let mut old_child = WzCoreNode::new("00002000.img");
        old_child.value = Some(ImageValue::Int(100));
        old_root.children.push(old_child);

        let mut new_root = WzCoreNode::new("Character");
        let mut new_child1 = WzCoreNode::new("00002000.img");
        new_child1.value = Some(ImageValue::Int(200)); // changed
        let mut new_child2 = WzCoreNode::new("00002001.img");
        new_child2.value = Some(ImageValue::Int(300)); // added
        new_root.children.push(new_child1);
        new_root.children.push(new_child2);

        let diffs = DiffEngine::diff_core_nodes(Some(&old_root), Some(&new_root), "Character");
        assert_eq!(diffs.len(), 2);

        let changed = diffs
            .iter()
            .find(|d| d.path == "Character/00002000.img")
            .unwrap();
        assert_eq!(changed.diff_type, DiffType::Changed);

        let added = diffs
            .iter()
            .find(|d| d.path == "Character/00002001.img")
            .unwrap();
        assert_eq!(added.diff_type, DiffType::Append);
    }

    #[test]
    fn test_arena_diff() {
        let old_arena = Arena::with_root("Base");
        let old_root_id = old_arena.root();

        let mut new_arena = Arena::with_root("Base");
        let new_root_id = new_arena.root();
        new_arena.add_node(new_root_id, "zmap.img", None);

        let diffs = DiffEngine::diff_arena_nodes(
            &old_arena,
            &new_arena,
            Some(old_root_id),
            Some(new_root_id),
            "Base",
        );

        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].diff_type, DiffType::Append);
        assert_eq!(diffs[0].path, "Base/zmap.img");
    }
}
