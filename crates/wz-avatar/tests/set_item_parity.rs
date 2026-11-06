//! `SetItem.CreateFromNode`을 실제 C# 실행 결과와 비교한다.
//!
//! 입력은 실제 `Etc/SetItemInfo.img`의 644개 세트와 `Item/ItemOption.img`이고,
//! C# 하네스가 실제 `SetItem.CreateFromNode` + `Potential.CreateFromNode`를 실행해
//! 구조 전체(파트/이펙트/옵션/몹/액티브스킬/시간보너스)를 덤프한다.

use serde_json::Value;
use std::path::{Path, PathBuf};
use wz_avatar::{create_set_item_from_node, set_item_json};
use wz_core::WzArchive;

/// 이미지 노드를 찾아 반환한다(테스트 전용).
fn load_image(root: &Path, subdir: &str, name: &str) -> Option<wz_core::image::Node> {
    let dir = root.join(subdir);
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("wz"))
        .collect();
    files.sort();
    for path in files {
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some(name))
        else {
            continue;
        };
        let Some(offset) = entry.offset else { continue };
        if let Ok(image) = archive.parse_image_at(offset) {
            return Some(image);
        }
    }
    None
}

#[test]
fn set_items_match_csharp() {
    let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(PathBuf::from) else {
        eprintln!("skip: WZ_DATA_ROOT not set");
        return;
    };
    let (Some(set_root), Some(option_root)) = (
        load_image(&root, "Etc", "SetItemInfo.img"),
        load_image(&root, "Item", "ItemOption.img"),
    ) else {
        eprintln!("skip: SetItemInfo.img / ItemOption.img not found");
        return;
    };

    let expected: Value = serde_json::from_str(include_str!("fixtures/set_items_expected.json"))
        .expect("fixture json");
    let expected_rows = expected["rows"].as_array().expect("rows");

    let mut rows: Vec<Value> = Vec::new();
    for node in set_root.object_children() {
        let set = create_set_item_from_node(node, Some(&option_root)).expect("set");
        rows.push(set_item_json(&set));
    }
    assert_eq!(
        rows.len(),
        expected_rows.len(),
        "set count mismatch ({} vs {})",
        rows.len(),
        expected_rows.len()
    );

    for (got, want) in rows.iter().zip(expected_rows.iter()) {
        assert_eq!(got, want, "set {}", got["set_item_id"]);
    }
}
