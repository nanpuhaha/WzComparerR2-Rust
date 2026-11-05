//! DB2 Skill 그리드 (`DB2Form.LoadSkill`).
//!
//! `Skill/<book>.img`마다 책 행을 만들고, `skill/<id>`마다 스킬 행을 만든다.
//! 이름/설명은 `StringLinker`의 `Skill.img` 값을 쓴다.

use crate::linker::StringLinker;
use serde::{Deserialize, Serialize};
use wz_core::dirtree::DirTree;
use wz_core::image::Node;
use wz_core::WzArchive;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2SkillRow {
    /// 책 ID(`100`) 또는 스킬 ID(`1001005`).
    pub id: String,
    /// `book` 또는 `skill`.
    pub row_type: String,
    /// 소속 책 ID (책 행이면 자기 자신).
    pub book: String,
    pub name: String,
    pub desc: String,
    pub has_icon: bool,
    pub has_common: bool,
    /// `String/Skill.img/<id>`의 `h` 또는 `h1..hn` 개수.
    pub h_count: usize,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

/// 한 Skill WZ 의 책/스킬을 DB2 Skill 그리드 행으로 만든다.
pub fn skill_grid_rows(
    archive: &WzArchive,
    tree: &DirTree,
    linker: &StringLinker,
) -> Vec<Db2SkillRow> {
    let mut rows = Vec::new();
    let mut books = Vec::new();
    for entry in &tree.root.entries {
        if entry.node_type != 4 {
            continue;
        }
        let Some(name) = entry.name.as_deref() else {
            continue;
        };
        let Some(book) = name.strip_suffix(".img") else {
            continue;
        };
        if book.is_empty() || !book.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Some(offset) = entry.offset else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(offset) else {
            continue;
        };
        books.push((book.to_string(), node));
    }
    books.sort_by(|a, b| a.0.cmp(&b.0));

    for (book, node) in books {
        let book_sr = linker.string_skill_str.get(&book);
        rows.push(Db2SkillRow {
            id: book.clone(),
            row_type: "book".into(),
            book: book.clone(),
            name: book_sr
                .and_then(|sr| sr.values.get("bookName").cloned())
                .unwrap_or_default(),
            desc: String::new(),
            has_icon: child(&node, "info")
                .and_then(|info| child(info, "icon"))
                .is_some(),
            has_common: false,
            h_count: 0,
        });

        let Some(skill_root) = child(&node, "skill") else {
            continue;
        };
        let mut skills: Vec<&Node> = skill_root
            .object_children()
            .iter()
            .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
            .collect();
        skills.sort_by_key(|c| c.name.parse::<i32>().unwrap_or(0));
        for skill in skills {
            let id = skill.name.to_string();
            let sr = id
                .parse::<i32>()
                .ok()
                .and_then(|n| linker.string_skill.get(&n));
            rows.push(Db2SkillRow {
                id,
                row_type: "skill".into(),
                book: book.clone(),
                name: sr.map(|sr| sr.name.clone()).unwrap_or_default(),
                desc: sr.and_then(|sr| sr.desc.clone()).unwrap_or_default(),
                has_icon: child(skill, "icon").is_some(),
                has_common: child(skill, "common").is_some(),
                h_count: sr.map(|sr| sr.skill_h.len()).unwrap_or(0),
            });
        }
    }

    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_and_skill_flags() {
        let mut book = Node::new("100.img");
        let mut info = Node::new("info");
        info.children.push(Node::new("icon"));
        book.children.push(info);
        let mut skill_root = Node::new("skill");
        let mut skill = Node::new("1001005");
        skill.children.push(Node::new("icon"));
        skill.children.push(Node::new("common"));
        assert!(child(&skill, "common").is_some());
        skill_root.children.push(skill);
        book.children.push(skill_root);

        assert!(child(&book, "info")
            .and_then(|i| child(i, "icon"))
            .is_some());
    }
}
