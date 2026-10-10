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

/// High-performance tree comparator
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

    /// Compare two `wz_core::dirtree::DirTree` structures by building unified Arena representations.
    pub fn diff_dirtree(
        old_tree: &wz_core::dirtree::DirTree,
        new_tree: &wz_core::dirtree::DirTree,
        root_name: &str,
    ) -> Vec<DiffItem> {
        let (old_arena, old_root) = Self::dirtree_to_arena(old_tree, root_name);
        let (new_arena, new_root) = Self::dirtree_to_arena(new_tree, root_name);
        Self::diff_arena_nodes(
            &old_arena,
            &new_arena,
            Some(old_root),
            Some(new_root),
            root_name,
        )
    }

    /// Converts a `DirTree` into a hierarchical `Arena` representation.
    pub fn dirtree_to_arena(tree: &wz_core::dirtree::DirTree, root_name: &str) -> (Arena, NodeId) {
        let mut arena = Arena::with_root(root_name);
        let root = arena.root();
        let mut block_cursor = 0;
        Self::ingest_dir_block(
            &mut arena,
            root,
            &tree.root,
            &tree.blocks,
            &mut block_cursor,
        );
        (arena, root)
    }

    fn ingest_dir_block(
        arena: &mut Arena,
        parent: NodeId,
        block: &wz_core::dirtree::DirBlock,
        blocks: &[wz_core::dirtree::DirBlock],
        block_cursor: &mut usize,
    ) {
        use wz_core::dirtree::node_type;
        use wz_db::NodeValue;
        for e in &block.entries {
            let name = e.name.clone().unwrap_or_else(|| std::sync::Arc::from("<ref>"));
            if e.node_type == node_type::DIRECTORY {
                let dir_id = arena.add_node(parent, name, None);
                if let Some(sub_block) = blocks.get(*block_cursor) {
                    *block_cursor += 1;
                    Self::ingest_dir_block(arena, dir_id, sub_block, blocks, block_cursor);
                }
            } else {
                arena.add_node(parent, name, Some(NodeValue::Int(e.data_len)));
            }
        }
    }

    /// Recursively collects an entire subtree as Append or Delete items
    fn collect_subtree(
        arena: &Arena,
        id: NodeId,
        diff_type: DiffType,
        path: &str,
    ) -> Vec<DiffItem> {
        let desc = match diff_type {
            DiffType::Append => format!("Added node: {}", path),
            DiffType::Delete => format!("Deleted node: {}", path),
            DiffType::Changed => format!("Modified value at {}", path),
        };
        let mut items = vec![DiffItem {
            path: path.to_string(),
            diff_type,
            description: desc,
        }];
        for &cid in arena.children(id) {
            if let Some(c_name) = arena.name(cid) {
                let subpath = if path.is_empty() {
                    c_name.to_string()
                } else {
                    format!("{}/{}", path, c_name)
                };
                items.extend(Self::collect_subtree(arena, cid, diff_type, &subpath));
            }
        }
        items
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
            (None, Some(nid)) => Self::collect_subtree(new_arena, nid, DiffType::Append, path),
            (Some(oid), None) => Self::collect_subtree(old_arena, oid, DiffType::Delete, path),
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

                // Match children by borrowed &str name without heap allocation
                let mut child_names: Vec<&str> = Vec::new();
                fn collect_names<'a>(arena: &'a Arena, id: NodeId, names: &mut Vec<&'a str>) {
                    for &cid in arena.children(id) {
                        if let Some(name) = arena.name(cid) {
                            if !names.contains(&name) {
                                names.push(name);
                            }
                        }
                    }
                }

                collect_names(old_arena, oid, &mut child_names);
                collect_names(new_arena, nid, &mut child_names);

                for name in child_names {
                    let o_child = old_arena.find_child(oid, name);
                    let n_child = new_arena.find_child(nid, name);
                    let subpath = if path.is_empty() {
                        name.to_string()
                    } else {
                        format!("{}/{}", path, name)
                    };
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

    #[test]
    fn test_dirtree_diff() {
        use wz_core::dirtree::{node_type, DirBlock, DirTree, RawEntry};

        let old_tree = DirTree {
            root: DirBlock {
                entries: vec![RawEntry {
                    node_type: node_type::IMAGE,
                    name: Some("item.img".into()),
                    name_ref: None,
                    data_len: 1024,
                    cs32: 0,
                    file_pos: 0,
                    offset: Some(100),
                }],
            },
            blocks: vec![],
        };

        let new_tree = DirTree {
            root: DirBlock {
                entries: vec![
                    RawEntry {
                        node_type: node_type::IMAGE,
                        name: Some("item.img".into()),
                        name_ref: None,
                        data_len: 2048, // changed size
                        cs32: 0,
                        file_pos: 0,
                        offset: Some(100),
                    },
                    RawEntry {
                        node_type: node_type::IMAGE,
                        name: Some("mob.img".into()), // added
                        name_ref: None,
                        data_len: 512,
                        cs32: 0,
                        file_pos: 0,
                        offset: Some(200),
                    },
                ],
            },
            blocks: vec![],
        };

        let diffs = DiffEngine::diff_dirtree(&old_tree, &new_tree, "Data.wz");
        assert_eq!(diffs.len(), 2);

        let changed = diffs.iter().find(|d| d.path == "Data.wz/item.img").unwrap();
        assert_eq!(changed.diff_type, DiffType::Changed);

        let added = diffs.iter().find(|d| d.path == "Data.wz/mob.img").unwrap();
        assert_eq!(added.diff_type, DiffType::Append);
    }

    #[test]
    fn test_nested_dirtree_diff() {
        use wz_core::dirtree::{node_type, DirBlock, DirTree, RawEntry};

        // Old: Dir "Map" containing "Map0"
        let old_tree = DirTree {
            root: DirBlock {
                entries: vec![RawEntry {
                    node_type: node_type::DIRECTORY,
                    name: Some("Map".into()),
                    name_ref: None,
                    data_len: 0,
                    cs32: 0,
                    file_pos: 0,
                    offset: Some(10),
                }],
            },
            blocks: vec![DirBlock {
                entries: vec![RawEntry {
                    node_type: node_type::IMAGE,
                    name: Some("Map0.img".into()),
                    name_ref: None,
                    data_len: 100,
                    cs32: 0,
                    file_pos: 0,
                    offset: Some(20),
                }],
            }],
        };

        // New: Dir "Map" containing "Map0" (changed) and "Map1" (added)
        let new_tree = DirTree {
            root: DirBlock {
                entries: vec![RawEntry {
                    node_type: node_type::DIRECTORY,
                    name: Some("Map".into()),
                    name_ref: None,
                    data_len: 0,
                    cs32: 0,
                    file_pos: 0,
                    offset: Some(10),
                }],
            },
            blocks: vec![DirBlock {
                entries: vec![
                    RawEntry {
                        node_type: node_type::IMAGE,
                        name: Some("Map0.img".into()),
                        name_ref: None,
                        data_len: 150,
                        cs32: 0,
                        file_pos: 0,
                        offset: Some(20),
                    },
                    RawEntry {
                        node_type: node_type::IMAGE,
                        name: Some("Map1.img".into()),
                        name_ref: None,
                        data_len: 200,
                        cs32: 0,
                        file_pos: 0,
                        offset: Some(30),
                    },
                ],
            }],
        };

        let diffs = DiffEngine::diff_dirtree(&old_tree, &new_tree, "Map.wz");
        assert_eq!(diffs.len(), 2);

        let map0 = diffs.iter().find(|d| d.path == "Map.wz/Map/Map0.img").unwrap();
        assert_eq!(map0.diff_type, DiffType::Changed);

        let map1 = diffs.iter().find(|d| d.path == "Map.wz/Map/Map1.img").unwrap();
        assert_eq!(map1.diff_type, DiffType::Append);
    }
}
