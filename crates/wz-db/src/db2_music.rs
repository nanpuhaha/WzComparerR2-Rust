//! DB2 Music 그리드 (`DB2Form.LoadMusic/AddMusicRows`).
//!
//! `Sound.wz`의 `Bgm*` / `PL_3*` / `PL_B*` / `PL_C*` / `PL_M*` 이미지 아래에
//! 있는 모든 `Sound_DX8` 노드를 재귀적으로 나열한다. 행 값은 원본
//! `GetPath()`와 같은 `/` 구분 경로다.

use serde::{Deserialize, Serialize};
use wz_core::dirtree::DirTree;
use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2MusicRow {
    pub path: String,
    pub data_len: u32,
    pub duration_ms: i32,
}

fn is_music_image(name: &str) -> bool {
    name.starts_with("Bgm")
        || name.starts_with("PL_3")
        || name.starts_with("PL_B")
        || name.starts_with("PL_C")
        || name.starts_with("PL_M")
}

fn collect_sounds(node: &Node, path: &str, rows: &mut Vec<Db2MusicRow>) {
    for child in node.object_children() {
        let child_path = format!("{path}/{}", child.name);
        match &child.value {
            Some(ImageValue::Sound(sound)) => rows.push(Db2MusicRow {
                path: child_path,
                data_len: sound.data_len,
                duration_ms: sound.duration,
            }),
            _ => collect_sounds(child, &child_path, rows),
        }
    }
}

/// 한 Sound WZ 의 음악 노드를 DB2 Music 행으로 만든다.
///
/// `root_name`은 원본 `GetPath()`의 최상위 경로(보통 WZ 파일명)다.
pub fn music_grid_rows(archive: &WzArchive, tree: &DirTree, root_name: &str) -> Vec<Db2MusicRow> {
    let mut rows = Vec::new();
    for entry in &tree.root.entries {
        if entry.node_type != 4 {
            continue;
        }
        let Some(name) = entry.name.as_deref() else {
            continue;
        };
        if !is_music_image(name) {
            continue;
        }
        let Some(offset) = entry.offset else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(offset) else {
            continue;
        };
        collect_sounds(&node, &format!("{root_name}/{name}"), &mut rows);
    }
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use wz_core::image::SoundRef;

    #[test]
    fn recursive_sound_paths() {
        let mut root = Node::new("Bgm00.img");
        let mut group = Node::new("sub");
        let mut sound = Node::new("track");
        sound.value = Some(ImageValue::Sound(Arc::new(SoundRef {
            offset: 0,
            data_len: 123,
            duration: 456,
            sound_decl: 2,
            major_type: [0; 16],
            sub_type: [0; 16],
            fixed_size_samples: false,
            temporal_compression: false,
            format_type: [0; 16],
            fmt_ex: Vec::new(),
        })));
        group.children.push(sound);
        root.children.push(group);

        let mut rows = Vec::new();
        collect_sounds(&root, "Sound_000.wz/Bgm00.img", &mut rows);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, "Sound_000.wz/Bgm00.img/sub/track");
        assert_eq!(rows[0].data_len, 123);
        assert_eq!(rows[0].duration_ms, 456);
    }

    #[test]
    fn music_image_filter_matches_original() {
        assert!(is_music_image("Bgm00.img"));
        assert!(is_music_image("PL_MONAD.img"));
        assert!(!is_music_image("AchievementEff.img"));
    }
}
