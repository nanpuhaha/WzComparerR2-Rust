//! `wz db2` — DB2 스타일 이미지 노드 테이블.

use std::path::Path;
use wz_core::WzArchive;

#[allow(clippy::print_literal)]
pub fn cmd_db2(
    wz_path: &Path,
    image: &str,
    filter: &str,
    json: bool,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let clean = image.trim_end_matches(".img");
    let image_name = format!("{clean}.img");
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| {
            e.name.as_deref() == Some(image_name.as_str()) || e.name.as_deref() == Some(clean)
        })
        .ok_or_else(|| format!("이미지를 찾을 수 없습니다: {image_name}"))?;
    let mut node = archive.parse_image_at(entry.offset.ok_or("이미지 오프셋 없음")?)?;
    node.name = std::sync::Arc::from(image_name.as_str());
    let mut rows = wz_db::db2_image_rows(&node);
    if !filter.is_empty() {
        rows.retain(|r| r.path.contains(filter) || r.value.contains(filter));
    }
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("{:<60} {:<10} {}", "path", "type", "value");
        for row in rows {
            println!("{:<60} {:<10} {}", row.path, row.node_type, row.value);
        }
    }
    Ok(())
}

fn collect_icons(
    node: &wz_core::image::Node,
    parent: &str,
    source: &std::sync::Arc<dyn wz_pool::ByteSource>,
    out: &mut Vec<serde_json::Value>,
) {
    use sha2::{Digest, Sha256};
    let path = if parent.is_empty() {
        node.name.to_string()
    } else {
        format!("{parent}/{}", node.name)
    };
    if let Some(wz_core::image::ImageValue::Png(png)) = &node.value {
        if let Ok((w, h, rgba)) = wz_image::decode_png_ref_to_rgba(png, source) {
            let mut hasher = Sha256::new();
            hasher.update(&rgba);
            out.push(serde_json::json!({
                "path": path,
                "width": w,
                "height": h,
                "format": png.format,
                "sha256": format!("{:x}", hasher.finalize()),
            }));
        }
    }
    for child in node.object_children() {
        collect_icons(child, &path, source, out);
    }
}

pub fn cmd_db2_icons(
    wz_path: &Path,
    image: &str,
    filter: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let clean = image.trim_end_matches(".img");
    let image_name = format!("{clean}.img");
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some(image_name.as_str()))
        .ok_or_else(|| format!("이미지를 찾을 수 없습니다: {image_name}"))?;
    let mut node = archive.parse_image_at(entry.offset.ok_or("이미지 오프셋 없음")?)?;
    node.name = std::sync::Arc::from(image_name.as_str());
    let mut rows = Vec::new();
    collect_icons(&node, "", archive.source(), &mut rows);
    rows.retain(|r| {
        filter.is_empty()
            || r.get("path")
                .and_then(|v| v.as_str())
                .map(|p| p.contains(filter))
                .unwrap_or(false)
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for row in &rows {
            println!(
                "{} {}x{} format={} {}",
                row["path"].as_str().unwrap_or(""),
                row["width"],
                row["height"],
                row["format"],
                row["sha256"].as_str().unwrap_or("")
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_match_csharp_rgba_sha() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let path = root.join("Character/Weapon/_Canvas/_Canvas_000.wz");
        if !path.exists() {
            return;
        }
        let archive = WzArchive::open(&path).unwrap();
        let tree = archive.read_tree().unwrap();
        let entry = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("01302000.img"))
            .unwrap();
        let mut node = archive.parse_image_at(entry.offset.unwrap()).unwrap();
        node.name = std::sync::Arc::from("01302000.img");
        let mut rows = Vec::new();
        collect_icons(&node, "", archive.source(), &mut rows);
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_icons_01302000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::Value::Array(rows), expected);
    }
}

pub fn cmd_db2_items(
    wz_path: &Path,
    image: &str,
    string_wz: &Path,
    json: bool,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_db::{Arena, StringLinker};

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let clean = image.trim_end_matches(".img");
    let image_name = format!("{clean}.img");
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some(image_name.as_str()))
        .ok_or_else(|| format!("이미지를 찾을 수 없습니다: {image_name}"))?;
    let mut node = archive.parse_image_at(entry.offset.ok_or("이미지 오프셋 없음")?)?;
    node.name = std::sync::Arc::from(image_name.as_str());

    let string_archive = WzArchive::open(string_wz)?;
    let (arena, root) = Arena::from_archive(&string_archive);
    let mut linker = StringLinker::new();
    if !linker.load_string_wz(&arena, root) {
        return Err("StringLinker 로드 실패".into());
    }

    let mut rows = wz_db::db2_item_grid_rows(&node, &linker);
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("{:<10} {:<24} desc", "id", "name");
        for row in rows {
            println!(
                "{:<10} {:<24} {}",
                row.id,
                row.name,
                row.desc.replace('\n', " ")
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod item_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_items_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Item/Consume/Consume_000.wz");
        let string_wz = root.join("String/String_000.wz");
        if !wz.exists() || !string_wz.exists() {
            return;
        }
        let archive = WzArchive::open(&wz).unwrap();
        let tree = archive.read_tree().unwrap();
        let entry = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("0200.img"))
            .unwrap();
        let mut node = archive.parse_image_at(entry.offset.unwrap()).unwrap();
        node.name = std::sync::Arc::from("0200.img");

        let string_archive = WzArchive::open(&string_wz).unwrap();
        let (arena, root_id) = Arena::from_archive(&string_archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&arena, root_id));
        let rows = wz_db::db2_item_grid_rows(&node, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_items_0200.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 530);
    }
}

pub fn cmd_db2_maps(
    wz_path: &Path,
    string_wz: &Path,
    json: bool,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_db::{Arena, StringLinker};

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let string_archive = WzArchive::open(string_wz)?;
    let (arena, root) = Arena::from_archive(&string_archive);
    let mut linker = StringLinker::new();
    if !linker.load_string_wz(&arena, root) {
        return Err("StringLinker 로드 실패".into());
    }
    let mut rows = wz_db::db2_map_grid_rows(&archive, &tree, &linker);
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!(
            "{:<10} {:<16} {:<24} {:<6} link",
            "id", "street", "map", "mini"
        );
        for row in rows {
            println!(
                "{:<10} {:<16} {:<24} {:<6} {:?}",
                row.id, row.street_name, row.map_name, row.has_minimap, row.link
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod map_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_maps_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Map/Map/Map0/Map0_000.wz");
        let string_wz = root.join("String/String_000.wz");
        if !wz.exists() || !string_wz.exists() {
            return;
        }
        let archive = WzArchive::open(&wz).unwrap();
        let tree = archive.read_tree().unwrap();
        let string_archive = WzArchive::open(&string_wz).unwrap();
        let (arena, root_id) = Arena::from_archive(&string_archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&arena, root_id));
        let rows = wz_db::db2_map_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_maps_map0.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 87);
    }
}

pub fn cmd_db2_mobs(
    wz_path: &Path,
    string_wz: &Path,
    json: bool,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_db::{Arena, StringLinker};

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let string_archive = WzArchive::open(string_wz)?;
    let (arena, root) = Arena::from_archive(&string_archive);
    let mut linker = StringLinker::new();
    if !linker.load_string_wz(&arena, root) {
        return Err("StringLinker 로드 실패".into());
    }
    let mut rows = wz_db::db2_mob_grid_rows(&archive, &tree, &linker);
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!(
            "{:<8} {:<24} {:>5} {:>4} {:>5} {:>3} {:>3} link",
            "id", "name", "stand", "fly", "move", "hit", "die"
        );
        for row in rows {
            println!(
                "{:<8} {:<24} {:>5} {:>4} {:>5} {:>3} {:>3} {:?}",
                row.id,
                row.name,
                row.has_stand,
                row.has_fly,
                row.has_move,
                row.has_hit,
                row.has_die,
                row.link
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod mob_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_mobs_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Mob/_Canvas/_Canvas_011.wz");
        let string_wz = root.join("String/String_000.wz");
        if !wz.exists() || !string_wz.exists() {
            return;
        }
        let archive = WzArchive::open(&wz).unwrap();
        let tree = archive.read_tree().unwrap();
        let string_archive = WzArchive::open(&string_wz).unwrap();
        let (arena, root_id) = Arena::from_archive(&string_archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&arena, root_id));
        let rows = wz_db::db2_mob_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_mobs_canvas011.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 55);
    }
}

pub fn cmd_db2_skills(
    wz_path: &Path,
    string_wz: &Path,
    json: bool,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_db::{Arena, StringLinker};

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let string_archive = WzArchive::open(string_wz)?;
    let (arena, root) = Arena::from_archive(&string_archive);
    let mut linker = StringLinker::new();
    if !linker.load_string_wz(&arena, root) {
        return Err("StringLinker 로드 실패".into());
    }
    let mut rows = wz_db::db2_skill_grid_rows(&archive, &tree, &linker);
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!(
            "{:<10} {:<6} {:<8} {:<24} {:>4} h",
            "id", "type", "book", "name", "icon"
        );
        for row in rows {
            println!(
                "{:<10} {:<6} {:<8} {:<24} {:>4} {}",
                row.id, row.row_type, row.book, row.name, row.has_icon, row.h_count
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod skill_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_skills_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Skill/_Canvas/_Canvas_000.wz");
        let string_wz = root.join("String/String_000.wz");
        if !wz.exists() || !string_wz.exists() {
            return;
        }
        let archive = WzArchive::open(&wz).unwrap();
        let tree = archive.read_tree().unwrap();
        let string_archive = WzArchive::open(&string_wz).unwrap();
        let (arena, root_id) = Arena::from_archive(&string_archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&arena, root_id));
        let rows = wz_db::db2_skill_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_skills_canvas000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 165);
    }
}

pub fn cmd_db2_npcs(
    wz_path: &Path,
    string_wz: &Path,
    json: bool,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_db::{Arena, StringLinker};

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let string_archive = WzArchive::open(string_wz)?;
    let (arena, root) = Arena::from_archive(&string_archive);
    let mut linker = StringLinker::new();
    if !linker.load_string_wz(&arena, root) {
        return Err("StringLinker 로드 실패".into());
    }
    let mut rows = wz_db::db2_npc_grid_rows(&archive, &tree, &linker);
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!(
            "{:<8} {:<24} {:>5} {:>7} link",
            "id", "name", "stand", "default"
        );
        for row in rows {
            println!(
                "{:<8} {:<24} {:>5} {:>7} {:?}",
                row.id, row.name, row.has_stand_0, row.has_default_0, row.link
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod npc_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_npcs_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Npc/_Canvas/_Canvas_000.wz");
        let string_wz = root.join("String/String_000.wz");
        if !wz.exists() || !string_wz.exists() {
            return;
        }
        let archive = WzArchive::open(&wz).unwrap();
        let tree = archive.read_tree().unwrap();
        let string_archive = WzArchive::open(&string_wz).unwrap();
        let (arena, root_id) = Arena::from_archive(&string_archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&arena, root_id));
        let rows = wz_db::db2_npc_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_npcs_canvas000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 943);
    }
}

pub fn cmd_db2_reactors(
    wz_path: &Path,
    json: bool,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let mut rows = wz_db::db2_reactor_grid_rows(&archive, &tree);
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("{:<8} {:>5} {:>5} link", "id", "icon", "info");
        for row in rows {
            println!(
                "{:<8} {:>5} {:>5} {:?}",
                row.id, row.has_icon, row.has_info, row.link
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod reactor_tests {
    use super::*;

    #[test]
    fn db2_reactors_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Reactor/_Canvas/_Canvas_000.wz");
        if !wz.exists() {
            return;
        }
        let archive = WzArchive::open(&wz).unwrap();
        let tree = archive.read_tree().unwrap();
        let rows = wz_db::db2_reactor_grid_rows(&archive, &tree);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_reactors_canvas000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 441);
    }
}
