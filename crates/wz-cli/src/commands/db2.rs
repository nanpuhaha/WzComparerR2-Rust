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

pub fn cmd_db2_music(
    wz_path: &Path,
    json: bool,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let root_name = wz_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Sound.wz");
    let mut rows = wz_db::db2_music_grid_rows(&archive, &tree, root_name);
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("{:<70} {:>10} {:>8}", "path", "bytes", "ms");
        for row in rows {
            println!(
                "{:<70} {:>10} {:>8}",
                row.path, row.data_len, row.duration_ms
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod music_tests {
    use super::*;

    #[test]
    fn db2_music_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Sound/Sound_000.wz");
        if !wz.exists() {
            return;
        }
        let archive = WzArchive::open(&wz).unwrap();
        let tree = archive.read_tree().unwrap();
        let rows = wz_db::db2_music_grid_rows(&archive, &tree, "Sound_000.wz");

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_music_sound000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 40);
    }
}

pub fn cmd_db2_damage_skins(
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
    let mut rows = wz_db::db2_damage_skin_rows(&archive, &tree, &linker);
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("{:<10} {:<28} {:>6} {:>6}", "id", "name", "icon", "sample");
        for row in rows {
            println!(
                "{:<10} {:<28} {:>6} {:>6}",
                row.id,
                row.name,
                row.icon_image.as_deref().unwrap_or(""),
                row.has_sample
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod damage_skin_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_damage_skins_match_csharp() {
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
        let string_archive = WzArchive::open(&string_wz).unwrap();
        let (arena, root_id) = Arena::from_archive(&string_archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&arena, root_id));
        let rows = wz_db::db2_damage_skin_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_damage_skins_consume000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 952);
    }
}

pub fn cmd_db2_morphs(
    wz_path: &Path,
    morph_wz: &Path,
    string_wz: &Path,
    json: bool,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_db::{Arena, StringLinker};

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let morph_archive = WzArchive::open(morph_wz)?;
    let morph_tree = morph_archive.read_tree()?;
    let string_archive = WzArchive::open(string_wz)?;
    let (arena, root) = Arena::from_archive(&string_archive);
    let mut linker = StringLinker::new();
    if !linker.load_string_wz(&arena, root) {
        return Err("StringLinker 로드 실패".into());
    }
    let mut rows =
        wz_db::db2_morph_grid_rows(&archive, &tree, &morph_archive, &morph_tree, &linker);
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!(
            "{:<10} {:<24} {:>6} {:>6} morph",
            "id", "name", "icon", "walk"
        );
        for row in rows {
            println!(
                "{:<10} {:<24} {:>6} {:>6} {}",
                row.id, row.name, row.has_icon, row.has_morph, row.morph_id
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod morph_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_morphs_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Item/Consume/Consume_000.wz");
        let morph_wz = root.join("Morph/Morph_000.wz");
        let string_wz = root.join("String/String_000.wz");
        if !wz.exists() || !morph_wz.exists() || !string_wz.exists() {
            return;
        }
        let archive = WzArchive::open(&wz).unwrap();
        let tree = archive.read_tree().unwrap();
        let morph_archive = WzArchive::open(&morph_wz).unwrap();
        let morph_tree = morph_archive.read_tree().unwrap();
        let string_archive = WzArchive::open(&string_wz).unwrap();
        let (arena, root_id) = Arena::from_archive(&string_archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&arena, root_id));
        let rows =
            wz_db::db2_morph_grid_rows(&archive, &tree, &morph_archive, &morph_tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_morphs_consume000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 234);
    }
}

pub fn cmd_db2_taming_mobs(
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
    let mut rows = wz_db::db2_taming_mob_grid_rows(&archive, &tree, &linker);
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!(
            "{:<10} {:<24} {:>4} {:>4} {:>3} islot",
            "id", "name", "lvl", "job", "tuc"
        );
        for row in rows {
            println!(
                "{:<10} {:<24} {:>4} {:>4} {:>3} {}",
                row.id, row.name, row.req_level, row.req_job, row.tuc, row.islot
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod taming_mob_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_taming_mobs_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/TamingMob/TamingMob_000.wz");
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
        let rows = wz_db::db2_taming_mob_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_taming_mobs_000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 1265);
    }
}

pub fn cmd_db2_characters(
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
    let category = wz_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str());
    let mut rows = wz_db::db2_character_grid_rows_for(&archive, &tree, &linker, category);
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!(
            "{:<10} {:<24} {:>4} {:>4} {:>3} {:<8} req",
            "id", "name", "lvl", "job", "tuc", "islot"
        );
        for row in rows {
            println!(
                "{:<10} {:<24} {:>4} {:>4} {:>3} {:<8} STR {} DEX {} INT {} LUK {} PAD {} MAD {}",
                row.id,
                row.name,
                row.req_level,
                row.req_job,
                row.tuc,
                row.islot,
                row.inc_str,
                row.inc_dex,
                row.inc_int,
                row.inc_luk,
                row.inc_pad,
                row.inc_mad
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod character_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_characters_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Coat/Coat_000.wz");
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
        let rows = wz_db::db2_character_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_coat000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 682);
    }
}

#[cfg(test)]
mod cap_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_cap_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Cap/Cap_000.wz");
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
        let rows = wz_db::db2_character_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_cap000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 3166);
    }
}

pub fn cmd_skill_summary(
    string_wz: &Path,
    limit: usize,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_db::{Arena, StringLinker};

    let string_archive = WzArchive::open(string_wz)?;
    let (arena, root) = Arena::from_archive(&string_archive);
    let mut linker = StringLinker::new();
    if !linker.load_string_wz(&arena, root) {
        return Err("StringLinker 로드 실패".into());
    }
    let mut rows: Vec<serde_json::Value> = Vec::new();
    for (id, sr) in &linker.string_skill_str {
        if let Some(desc) = sr.desc.as_deref().filter(|d| !d.is_empty()) {
            rows.push(serde_json::json!({
                "id": id,
                "kind": "desc",
                "summary": wz_db::parse_skill_summary(desc, 1),
            }));
        }
        for (i, h) in sr.skill_h.iter().enumerate() {
            if h.is_empty() {
                continue;
            }
            rows.push(serde_json::json!({
                "id": id,
                "kind": format!("h{i}"),
                "summary": wz_db::parse_skill_summary(h, 1),
            }));
        }
        if rows.len() >= limit {
            break;
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for row in rows {
            println!(
                "{}\t{}\t{}",
                row["id"].as_str().unwrap_or(""),
                row["kind"].as_str().unwrap_or(""),
                row["summary"].as_str().unwrap_or("")
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod skill_summary_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn skill_summary_matches_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let string_wz = root.join("String/String_000.wz");
        if !string_wz.exists() {
            return;
        }
        let archive = WzArchive::open(&string_wz).unwrap();
        let (arena, root_id) = Arena::from_archive(&archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&arena, root_id));

        let mut rows: Vec<serde_json::Value> = Vec::new();
        for (id, sr) in &linker.string_skill_str {
            if let Some(desc) = sr.desc.as_deref().filter(|d| !d.is_empty()) {
                rows.push(serde_json::json!({
                    "id": id,
                    "kind": "desc",
                    "summary": wz_db::parse_skill_summary(desc, 1),
                }));
            }
            for (i, h) in sr.skill_h.iter().enumerate() {
                if h.is_empty() {
                    continue;
                }
                rows.push(serde_json::json!({
                    "id": id,
                    "kind": format!("h{i}"),
                    "summary": wz_db::parse_skill_summary(h, 1),
                }));
            }
            if rows.len() >= 2000 {
                break;
            }
        }

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/skill_summary_2000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::Value::Array(rows), expected);
    }
}

pub fn cmd_item_tooltip(
    wz_path: &Path,
    image: &str,
    item_id: &str,
    string_wz: &Path,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_db::{Arena, StringLinker};

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let image_name = if image.ends_with(".img") {
        image.to_string()
    } else {
        format!("{image}.img")
    };
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some(image_name.as_str()))
        .ok_or_else(|| format!("이미지를 찾을 수 없습니다: {image_name}"))?;
    let node = archive.parse_image_at(entry.offset.ok_or("이미지 오프셋 없음")?)?;
    let item = node
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == item_id)
        .ok_or_else(|| format!("아이템을 찾을 수 없습니다: {item_id}"))?;

    let string_archive = WzArchive::open(string_wz)?;
    let (arena, root) = Arena::from_archive(&string_archive);
    let mut linker = StringLinker::new();
    if !linker.load_string_wz(&arena, root) {
        return Err("StringLinker 로드 실패".into());
    }
    let result = wz_db::build_item_tooltip(item, item_id, &linker)
        .ok_or_else(|| format!("아이템 ID 파싱 실패: {item_id}"))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("name: {}", result.name);
        println!("desc: {}", result.desc);
        for tag in &result.attributes {
            println!("tag: {tag}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod item_tooltip_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn item_tooltip_matches_csharp() {
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
        let node = archive.parse_image_at(entry.offset.unwrap()).unwrap();
        let item = node
            .object_children()
            .iter()
            .find(|c| c.name.as_ref() == "02000012")
            .unwrap();
        let string_archive = WzArchive::open(&string_wz).unwrap();
        let (arena, root_id) = Arena::from_archive(&string_archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&arena, root_id));
        let view = wz_db::build_item_tooltip(item, "02000012", &linker).unwrap();

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/item_tooltip_02000012.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&view).unwrap(), expected);
    }
}

#[cfg(test)]
mod shoes_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_shoes_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Shoes/Shoes_000.wz");
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
        let rows = wz_db::db2_character_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_shoes000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 1703);
    }
}

#[cfg(test)]
mod accessory_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_accessory_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Accessory/Accessory_000.wz");
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
        let rows = wz_db::db2_character_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_accessory000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 2369);
    }
}

#[cfg(test)]
mod longcoat_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_longcoat_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Longcoat/Longcoat_000.wz");
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
        let rows = wz_db::db2_character_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_longcoat000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 2493);
    }
}

#[cfg(test)]
mod glove_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_glove_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Glove/Glove_000.wz");
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
        let rows = wz_db::db2_character_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_glove000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 661);
    }
}

#[cfg(test)]
mod pants_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_pants_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Pants/Pants_000.wz");
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
        let rows = wz_db::db2_character_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_pants000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 579);
    }
}

#[cfg(test)]
mod shield_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_shield_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Shield/Shield_000.wz");
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
        let rows = wz_db::db2_character_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_shield000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 142);
    }
}

#[cfg(test)]
mod cape_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_cape_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Cape/Cape_000.wz");
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
        let rows = wz_db::db2_character_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_cape000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 1139);
    }
}

#[cfg(test)]
mod petequip_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_petequip_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/PetEquip/PetEquip_000.wz");
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
        let rows = wz_db::db2_character_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_petequip000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 511);
    }
}

#[cfg(test)]
mod weapon_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_weapon_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Weapon/Weapon_000.wz");
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
        let rows = wz_db::db2_character_grid_rows(&archive, &tree, &linker);

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_weapon000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 5655);
    }
}

#[cfg(test)]
mod hair_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_hair_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Hair/Hair_000.wz");
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
        let rows = wz_db::db2_character_grid_rows_for(&archive, &tree, &linker, Some("Hair"));

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_hair000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 9004);
    }
}

#[cfg(test)]
mod face_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_face_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Face/Face_000.wz");
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
        let rows = wz_db::db2_character_grid_rows_for(&archive, &tree, &linker, Some("Face"));

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_face000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 7424);
    }
}

#[cfg(test)]
mod mechanic_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_mechanic_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Mechanic/Mechanic_000.wz");
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
        let rows = wz_db::db2_character_grid_rows_for(&archive, &tree, &linker, Some("Mechanic"));

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_mechanic000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 23);
    }
}

#[cfg(test)]
mod dragon_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_dragon_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Dragon/Dragon_000.wz");
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
        let rows = wz_db::db2_character_grid_rows_for(&archive, &tree, &linker, Some("Dragon"));

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_dragon000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 16);
    }
}

#[cfg(test)]
mod ring_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_ring_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Character/Ring/Ring_000.wz");
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
        let rows = wz_db::db2_character_grid_rows_for(&archive, &tree, &linker, Some("Ring"));

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_characters_ring000.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&rows).unwrap(), expected);
        assert_eq!(rows.len(), 1127);
    }
}

#[cfg(test)]
mod extra_character_tests {
    use super::*;
    use wz_db::{Arena, StringLinker};

    #[test]
    fn db2_extra_characters_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let string_wz = root.join("String/String_000.wz");
        if !string_wz.exists() {
            return;
        }
        let string_archive = WzArchive::open(&string_wz).unwrap();
        let (arena, root_id) = Arena::from_archive(&string_archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&arena, root_id));

        for (category, expected_rows) in [
            ("Android", 215),
            ("ArcaneForce", 7),
            ("AuthenticForce", 8),
            ("Bits", 79),
            ("EquipBag", 2),
        ] {
            let wz = root
                .join("Character")
                .join(category)
                .join(format!("{category}_000.wz"));
            if !wz.exists() {
                continue;
            }
            let archive = WzArchive::open(&wz).unwrap();
            let tree = archive.read_tree().unwrap();
            let rows = wz_db::db2_character_grid_rows_for(&archive, &tree, &linker, Some(category));
            let expected: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures")
                        .join(format!(
                            "db2_characters_{}000.expected.json",
                            category.to_ascii_lowercase()
                        )),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(serde_json::to_value(&rows).unwrap(), expected, "{category}");
            assert_eq!(rows.len(), expected_rows, "{category}");
        }
    }
}

pub fn cmd_db2_item_tab(
    data_root: &Path,
    dir: &str,
    string_wz: &Path,
    json: bool,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let string_image = wz_db::db2_item_tab_string_image(string_wz, dir);
    if string_image.is_none() && dir != "Special" {
        return Err(format!("String/{dir}.img를 찾을 수 없습니다").into());
    }
    let mut rows = wz_db::db2_item_tab_rows(data_root, dir, string_image.as_ref());
    rows.truncate(limit);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("{:<10} {:<28} icon", "id", "name");
        for row in rows {
            println!("{:<10} {:<28} {}", row.id, row.name, row.has_icon);
        }
    }
    Ok(())
}

#[cfg(test)]
mod item_tab_tests {

    fn canonical(rows: &[wz_db::Db2ItemTabRow]) -> String {
        rows.iter()
            .map(|r| {
                format!(
                    "{}\t{}\t{}\t{}",
                    r.id,
                    r.name,
                    r.desc,
                    if r.has_icon { 1 } else { 0 }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn db2_item_tabs_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let string_wz = root.join("String/String_000.wz");
        if !root.join("Item").exists() || !string_wz.exists() {
            return;
        }
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_item_tabs.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();

        for dir in ["Cash", "Consume", "Etc", "Install", "Pet", "Special"] {
            if !root.join("Item").join(dir).exists() {
                continue;
            }
            let image = wz_db::db2_item_tab_string_image(&string_wz, dir);
            let rows = wz_db::db2_item_tab_rows(&root, dir, image.as_ref());
            let want = &expected[dir];
            assert_eq!(
                rows.len() as u64,
                want["rows"].as_u64().unwrap(),
                "{dir} rows"
            );
            assert_eq!(
                sha256_hex(canonical(&rows).as_bytes()),
                want["sha256"].as_str().unwrap(),
                "{dir} hash"
            );
        }
    }
}

pub fn cmd_gear_props(wz_path: &Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let mut seen = std::collections::HashSet::new();
    let mut rows = Vec::new();
    for entry in &tree.root.entries {
        if entry.node_type != 4 {
            continue;
        }
        let Some(offset) = entry.offset else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(offset) else {
            continue;
        };
        let Some(info) = node
            .object_children()
            .iter()
            .find(|c| c.name.as_ref() == "info")
        else {
            continue;
        };
        for c in info.object_children() {
            let value = match &c.value {
                Some(wz_core::image::ImageValue::Int(v)) => i64::from(*v),
                Some(wz_core::image::ImageValue::Short(v)) => i64::from(*v),
                Some(wz_core::image::ImageValue::Long(v)) => *v,
                Some(wz_core::image::ImageValue::Uint(v)) => i64::from(*v),
                Some(wz_core::image::ImageValue::Str(text))
                | Some(wz_core::image::ImageValue::Uol(text)) => match text.trim().parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                },
                _ => continue,
            };
            let key = c.name.to_string();
            if !seen.insert((key.clone(), value)) {
                continue;
            }
            rows.push(serde_json::json!({
                "key": key,
                "value": value,
                "text": wz_avatar::gear_prop_line(&key, value),
            }));
        }
    }
    rows.sort_by(|a, b| {
        let ka = a["key"].as_str().unwrap_or("");
        let kb = b["key"].as_str().unwrap_or("");
        ka.cmp(kb).then(
            a["value"]
                .as_i64()
                .unwrap_or(0)
                .cmp(&b["value"].as_i64().unwrap_or(0)),
        )
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for row in &rows {
            println!(
                "{}\t{}\t{}",
                row["key"].as_str().unwrap_or(""),
                row["value"],
                row["text"].as_str().unwrap_or("null")
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod gear_props_tests {
    use super::*;

    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    fn collect(path: &Path) -> Vec<(String, i64, Option<String>)> {
        let archive = WzArchive::open(path).unwrap();
        let tree = archive.read_tree().unwrap();
        let mut seen = std::collections::HashSet::new();
        let mut rows = Vec::new();
        for entry in &tree.root.entries {
            if entry.node_type != 4 {
                continue;
            }
            let Some(offset) = entry.offset else {
                continue;
            };
            let Ok(node) = archive.parse_image_at(offset) else {
                continue;
            };
            let Some(info) = node
                .object_children()
                .iter()
                .find(|c| c.name.as_ref() == "info")
            else {
                continue;
            };
            for c in info.object_children() {
                let value = match &c.value {
                    Some(wz_core::image::ImageValue::Int(v)) => i64::from(*v),
                    Some(wz_core::image::ImageValue::Short(v)) => i64::from(*v),
                    Some(wz_core::image::ImageValue::Long(v)) => *v,
                    Some(wz_core::image::ImageValue::Uint(v)) => i64::from(*v),
                    Some(wz_core::image::ImageValue::Str(text))
                    | Some(wz_core::image::ImageValue::Uol(text)) => match text.trim().parse() {
                        Ok(v) => v,
                        Err(_) => continue,
                    },
                    _ => continue,
                };
                let key = c.name.to_string();
                if !seen.insert((key.clone(), value)) {
                    continue;
                }
                rows.push((key.clone(), value, wz_avatar::gear_prop_line(&key, value)));
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        rows
    }

    #[test]
    fn gear_props_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/gear_props.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        for cat in [
            "Coat",
            "Weapon",
            "Cap",
            "Accessory",
            "Face",
            "Ring",
            "TamingMob",
        ] {
            let path = root
                .join("Character")
                .join(cat)
                .join(format!("{cat}_000.wz"));
            if !path.exists() {
                continue;
            }
            let rows = collect(&path);
            let canon = rows
                .iter()
                .map(|(k, v, t)| format!("{k}\t{v}\t{}", t.as_deref().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(
                rows.len() as u64,
                expected[cat]["pairs"].as_u64().unwrap(),
                "{cat} pairs"
            );
            assert_eq!(
                sha256_hex(canon.as_bytes()),
                expected[cat]["sha256"].as_str().unwrap(),
                "{cat} hash"
            );
        }
    }
}

pub fn cmd_db2_item_tab_tooltips(
    data_root: &Path,
    dir: &str,
    string_wz: &Path,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let string_image = wz_db::db2_item_tab_string_image(string_wz, dir);
    if string_image.is_none() && dir != "Special" {
        return Err(format!("String/{dir}.img를 찾을 수 없습니다").into());
    }
    let rows = wz_db::db2_item_tab_tooltips(data_root, dir, string_image.as_ref());
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for row in &rows {
            if row.attributes.is_empty() {
                continue;
            }
            println!("{}\t{}\t{:?}", row.id, row.name, row.attributes);
        }
    }
    Ok(())
}

#[cfg(test)]
mod item_tab_tooltip_tests {
    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn db2_item_tab_tooltips_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let string_wz = root.join("String/String_000.wz");
        if !root.join("Item").exists() || !string_wz.exists() {
            return;
        }
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/db2_item_tab_tooltips.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        for dir in ["Cash", "Consume", "Etc", "Install", "Pet", "Special"] {
            if !root.join("Item").join(dir).exists() {
                continue;
            }
            let image = wz_db::db2_item_tab_string_image(&string_wz, dir);
            let rows = wz_db::db2_item_tab_tooltips(&root, dir, image.as_ref());
            let canon = rows
                .iter()
                .map(|r| {
                    format!(
                        "{}\t{}\t{}\t{}",
                        r.id,
                        r.name,
                        r.desc,
                        r.attributes.join("|")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let want = &expected[dir];
            assert_eq!(
                rows.len() as u64,
                want["rows"].as_u64().unwrap(),
                "{dir} rows"
            );
            assert_eq!(
                sha256_hex(canon.as_bytes()),
                want["sha256"].as_str().unwrap(),
                "{dir} hash"
            );
        }
    }
}

pub fn cmd_achievements(
    wz_path: &Path,
    json: bool,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let mut rows = Vec::new();
    let mut names = std::collections::HashMap::new();
    for entry in &tree.root.entries {
        if entry.node_type != 4 {
            continue;
        }
        let Some(offset) = entry.offset else {
            continue;
        };
        let Ok(mut node) = archive.parse_image_at(offset) else {
            continue;
        };
        if let Some(name) = entry.name.as_deref() {
            node.name = std::sync::Arc::from(name);
        }
        if let Some(ac) = wz_db::Achievement::from_node(&node) {
            names.insert(ac.id, ac.name.clone());
            rows.push(ac);
        }
    }
    rows.sort_by_key(|a| a.id);
    rows.truncate(limit);
    let rows: Vec<_> = rows
        .into_iter()
        .map(|ac| {
            let lines = ac.tooltip_lines(&names);
            serde_json::json!({
                "id": ac.id,
                "name": ac.name,
                "desc": ac.desc,
                "score": ac.score,
                "difficulty": ac.difficulty,
                "ui_form": ac.ui_form,
                "block": ac.block,
                "main_category": ac.main_category,
                "sub_category": ac.sub_category,
                "prior_ids": ac.prior_ids,
                "missions": ac.missions,
                "rewards": ac.rewards,
                "lines": lines,
            })
        })
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for row in &rows {
            println!("{}", row["id"]);
            for line in row["lines"].as_array().into_iter().flatten() {
                println!("  {}", line.as_str().unwrap_or(""));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod achievement_tests {
    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    fn rec() -> char {
        char::from(0x1e)
    }

    #[test]
    fn achievements_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let path = root.join("Etc/Achievement/AchievementData/AchievementData_000.wz");
        if !path.exists() {
            return;
        }
        let archive = wz_core::WzArchive::open(&path).unwrap();
        let tree = archive.read_tree().unwrap();
        let mut rows = Vec::new();
        for entry in &tree.root.entries {
            if entry.node_type != 4 {
                continue;
            }
            let Some(offset) = entry.offset else {
                continue;
            };
            let Ok(mut node) = archive.parse_image_at(offset) else {
                continue;
            };
            if let Some(name) = entry.name.as_deref() {
                node.name = std::sync::Arc::from(name);
            }
            if let Some(ac) = wz_db::Achievement::from_node(&node) {
                rows.push(ac);
            }
        }
        rows.sort_by_key(|a| a.id);
        let names: std::collections::HashMap<i32, String> =
            rows.iter().map(|a| (a.id, a.name.clone())).collect();
        let rec = rec();
        let canon = rows
            .iter()
            .map(|a| {
                let lines = a.tooltip_lines(&names);
                [
                    a.id.to_string(),
                    a.name.clone(),
                    a.desc.clone(),
                    a.score.to_string(),
                    a.difficulty.clone().unwrap_or_default(),
                    a.ui_form.clone().unwrap_or_default(),
                    a.block.clone().unwrap_or_default(),
                    a.main_category.clone(),
                    a.sub_category.clone().unwrap_or_default(),
                    a.prior_ids
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                    a.missions.join("|"),
                    a.rewards
                        .iter()
                        .map(|r| format!("{}:{}", r.id, r.desc))
                        .collect::<Vec<_>>()
                        .join("|"),
                    lines.join("|"),
                ]
                .join(&rec.to_string())
            })
            .collect::<Vec<_>>()
            .join(&rec.to_string());

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/achievements.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len() as u64, expected["rows"].as_u64().unwrap());
        assert_eq!(
            sha256_hex(canon.as_bytes()),
            expected["sha256"].as_str().unwrap()
        );
    }
}

pub fn cmd_world_archive(
    string_wz: &Path,
    etc_wz: &Path,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_db::{Arena, StringLinker};

    let string_archive = WzArchive::open(string_wz)?;
    let (string_arena, string_root) = Arena::from_archive(&string_archive);
    let mut linker = StringLinker::new();
    if !linker.load_string_wz(&string_arena, string_root) {
        return Err("StringLinker 로드 실패".into());
    }
    let etc_archive = WzArchive::open(etc_wz)?;
    let (etc_arena, etc_root) = Arena::from_archive(&etc_archive);
    linker.load_etc_wz(&etc_arena, etc_root);

    let mut rows = Vec::new();
    for (id, sr) in &linker.string_world_archive_mob {
        let monster_book = linker
            .string_monster_book
            .get(id)
            .and_then(|sr| sr.desc.clone());
        rows.push(serde_json::json!({
            "kind": "mob",
            "id": id,
            "desc": sr.desc,
            "monster_book": monster_book,
            "lines": wz_db::world_archive_mob_lines(sr.desc.as_deref(), monster_book.as_deref()),
        }));
    }
    let npc_image = string_arena
        .children(string_root)
        .iter()
        .copied()
        .find(|&c| string_arena.name(c) == Some("Npc.img"))
        .map(|c| string_arena.descend_image(c));
    let mut npc_quotes = Vec::new();
    for (id, sr) in &linker.string_world_archive_npc {
        let quote = npc_image
            .and_then(|root| {
                string_arena
                    .children(root)
                    .iter()
                    .copied()
                    .find(|&c| string_arena.name(c) == Some(id.to_string().as_str()))
            })
            .map(|node| wz_db::npc_quote_parse(&string_arena, node, &linker))
            .filter(|q| {
                !q.n.is_empty()
                    || !q.f.is_empty()
                    || !q.w.is_empty()
                    || !q.d.is_empty()
                    || !q.special.is_empty()
            })
            .map(|q| wz_db::npc_quote_message(&q))
            .filter(|m| !m.is_empty());
        rows.push(serde_json::json!({
            "kind": "npc",
            "id": id,
            "desc": sr.desc,
            "quote": quote,
            "lines": wz_db::world_archive_npc_lines(sr.desc.as_deref(), quote.as_deref()),
        }));
        npc_quotes.push(quote);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for row in &rows {
            println!("{} {}", row["kind"].as_str().unwrap_or(""), row["id"]);
            for line in row["lines"].as_array().into_iter().flatten() {
                println!("  {}", line.as_str().unwrap_or(""));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod world_archive_tests {
    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn world_archive_matches_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let string_wz = root.join("String/String_000.wz");
        let etc_wz = root.join("Etc/Etc_002.wz");
        if !string_wz.exists() || !etc_wz.exists() {
            return;
        }
        use wz_db::{Arena, StringLinker};
        let string_archive = wz_core::WzArchive::open(&string_wz).unwrap();
        let (string_arena, string_root) = Arena::from_archive(&string_archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&string_arena, string_root));
        let etc_archive = wz_core::WzArchive::open(&etc_wz).unwrap();
        let (etc_arena, etc_root) = Arena::from_archive(&etc_archive);
        linker.load_etc_wz(&etc_arena, etc_root);

        let rec = char::from(0x1e);
        let mut rows: Vec<String> = Vec::new();
        let mut count = 0u64;
        for (id, sr) in &linker.string_world_archive_mob {
            let book = linker
                .string_monster_book
                .get(id)
                .and_then(|s| s.desc.clone());
            let lines = wz_db::world_archive_mob_lines(sr.desc.as_deref(), book.as_deref());
            rows.push(
                [
                    "mob".to_string(),
                    id.to_string(),
                    sr.desc.clone().unwrap_or_default(),
                    String::new(),
                    book.unwrap_or_default(),
                    lines.join("|"),
                ]
                .join(&rec.to_string()),
            );
            count += 1;
        }
        let npc_image = string_arena
            .children(string_root)
            .iter()
            .copied()
            .find(|&c| string_arena.name(c) == Some("Npc.img"))
            .map(|c| string_arena.descend_image(c));
        for (id, sr) in &linker.string_world_archive_npc {
            let quote = npc_image
                .and_then(|root| {
                    string_arena
                        .children(root)
                        .iter()
                        .copied()
                        .find(|&c| string_arena.name(c) == Some(id.to_string().as_str()))
                })
                .map(|node| wz_db::npc_quote_parse(&string_arena, node, &linker))
                .filter(|q| {
                    !q.n.is_empty()
                        || !q.f.is_empty()
                        || !q.w.is_empty()
                        || !q.d.is_empty()
                        || !q.special.is_empty()
                })
                .map(|q| wz_db::npc_quote_message(&q))
                .filter(|m| !m.is_empty());
            let lines = wz_db::world_archive_npc_lines(sr.desc.as_deref(), quote.as_deref());
            rows.push(
                [
                    "npc".to_string(),
                    id.to_string(),
                    sr.desc.clone().unwrap_or_default(),
                    quote.unwrap_or_default(),
                    String::new(),
                    lines.join("|"),
                ]
                .join(&rec.to_string()),
            );
            count += 1;
        }
        let canon = rows.join(&rec.to_string());
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/world_archive.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(count, expected["rows"].as_u64().unwrap());
        assert_eq!(
            sha256_hex(canon.as_bytes()),
            expected["sha256"].as_str().unwrap()
        );
    }
}

pub fn cmd_find_wz(
    maps: &[String],
    path: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut set = wz_db::PluginWzSet::new();
    for entry in maps {
        let Some((wz_type, file)) = entry.split_once('=') else {
            return Err(format!("잘못된 --map 형식: {entry}").into());
        };
        set.add(wz_type, Path::new(file))?;
    }
    let found = set.find(path);
    if json {
        println!("{}", serde_json::to_string_pretty(&found)?);
    } else if let Some(node) = found {
        println!(
            "{} value={} children={} depth={}",
            node.name, node.value, node.children, node.depth
        );
    } else {
        println!("not found");
    }
    Ok(())
}

#[cfg(test)]
mod findwz_tests {
    #[test]
    fn findwz_matches_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let mappings = [
            ("Mob", "Mob/_Canvas/_Canvas_011.wz"),
            ("Npc", "Npc/_Canvas/_Canvas_000.wz"),
            ("Skill", "Skill/_Canvas/_Canvas_000.wz"),
            ("Reactor", "Reactor/_Canvas/_Canvas_000.wz"),
            ("String", "String/String_000.wz"),
        ];
        for (_, rel) in mappings {
            if !root.join(rel).exists() {
                return;
            }
        }
        let mut set = wz_db::PluginWzSet::new();
        for (kind, rel) in mappings {
            set.add(kind, &root.join(rel)).unwrap();
        }
        let expected: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/findwz.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        for row in expected {
            let path = row["path"].as_str().unwrap();
            let found = set.find(path);
            assert_eq!(found.is_some(), row["found"].as_bool().unwrap(), "{path}");
            if let Some(node) = found {
                assert_eq!(node.name, row["name"].as_str().unwrap(), "{path} name");
                assert_eq!(node.value, row["value"].as_str().unwrap(), "{path} value");
                assert_eq!(
                    node.children as u64,
                    row["children"].as_u64().unwrap(),
                    "{path} children"
                );
            }
        }
    }
}

pub fn cmd_chara_tooltips(
    npc_wz: &Path,
    reactor_wz: &Path,
    string_wz: &Path,
    etc_wz: &Path,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_db::{Arena, StringLinker};

    let string_archive = WzArchive::open(string_wz)?;
    let (string_arena, string_root) = Arena::from_archive(&string_archive);
    let mut linker = StringLinker::new();
    if !linker.load_string_wz(&string_arena, string_root) {
        return Err("StringLinker 로드 실패".into());
    }
    let etc_archive = WzArchive::open(etc_wz)?;
    let etc_tree = etc_archive.read_tree()?;
    let location_image = etc_tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some("NpcLocation.img"))
        .and_then(|e| e.offset)
        .and_then(|offset| etc_archive.parse_image_at(offset).ok())
        .unwrap_or_else(|| wz_core::image::Node::new("NpcLocation.img"));

    let map_name = |id: i32| linker.string_map.get(&id).map(|sr| sr.name.clone());

    let npc_archive = WzArchive::open(npc_wz)?;
    let npc_tree = npc_archive.read_tree()?;
    let mut rows = Vec::new();
    for entry in &npc_tree.root.entries {
        if entry.node_type != 4 {
            continue;
        }
        let Some(offset) = entry.offset else {
            continue;
        };
        let Ok(mut node) = npc_archive.parse_image_at(offset) else {
            continue;
        };
        let Some(name) = entry.name.as_deref() else {
            continue;
        };
        node.name = std::sync::Arc::from(name);
        let Some(id) = name
            .strip_suffix(".img")
            .and_then(|s| s.parse::<i32>().ok())
        else {
            continue;
        };
        let locations = wz_db::npc_locations(&location_image, id);
        let npc_name = linker.string_npc.get(&id).map(|sr| sr.name.clone());
        rows.push(serde_json::json!({
            "kind": "npc",
            "id": id,
            "name": npc_name,
            "lines": wz_db::npc_lines(npc_name.as_deref(), id, &locations, &map_name),
        }));
    }

    let reactor_archive = WzArchive::open(reactor_wz)?;
    let reactor_tree = reactor_archive.read_tree()?;
    for entry in &reactor_tree.root.entries {
        if entry.node_type != 4 {
            continue;
        }
        let Some(offset) = entry.offset else {
            continue;
        };
        let Ok(mut node) = reactor_archive.parse_image_at(offset) else {
            continue;
        };
        let Some(name) = entry.name.as_deref() else {
            continue;
        };
        node.name = std::sync::Arc::from(name);
        if let Some(tooltip) = wz_db::reactor_from_node(&node) {
            rows.push(serde_json::json!({
                "kind": "reactor",
                "id": tooltip.id,
                "name": tooltip.name,
                "action": tooltip.action,
                "clip_count": tooltip.clip_count,
                "lines": tooltip.lines,
            }));
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for row in &rows {
            println!("{} {}", row["kind"].as_str().unwrap_or(""), row["id"]);
            for line in row["lines"].as_array().into_iter().flatten() {
                println!("  {}", line.as_str().unwrap_or(""));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod chara_tooltip_tests {
    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn chara_tooltips_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let npc = root.join("Npc/Npc_000.wz");
        let reactor = root.join("Reactor/_Canvas/_Canvas_000.wz");
        let string_wz = root.join("String/String_000.wz");
        let etc = root.join("Etc/Etc_000.wz");
        for path in [&npc, &reactor, &string_wz, &etc] {
            if !path.exists() {
                return;
            }
        }
        use wz_db::{Arena, StringLinker};
        let string_archive = wz_core::WzArchive::open(&string_wz).unwrap();
        let (string_arena, string_root) = Arena::from_archive(&string_archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&string_arena, string_root));
        let etc_archive = wz_core::WzArchive::open(&etc).unwrap();
        let etc_tree = etc_archive.read_tree().unwrap();
        let location_image = etc_tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("NpcLocation.img"))
            .and_then(|e| e.offset)
            .and_then(|o| etc_archive.parse_image_at(o).ok())
            .unwrap_or_else(|| wz_core::image::Node::new("NpcLocation.img"));
        let map_name = |id: i32| linker.string_map.get(&id).map(|sr| sr.name.clone());

        let rec = char::from(0x1e);
        let mut rows: Vec<String> = Vec::new();
        let npc_archive = wz_core::WzArchive::open(&npc).unwrap();
        for entry in &npc_archive.read_tree().unwrap().root.entries {
            if entry.node_type != 4 {
                continue;
            }
            let Some(offset) = entry.offset else { continue };
            let Ok(mut node) = npc_archive.parse_image_at(offset) else {
                continue;
            };
            let Some(name) = entry.name.as_deref() else {
                continue;
            };
            node.name = std::sync::Arc::from(name);
            let Some(id) = name.strip_suffix(".img").and_then(|s| s.parse().ok()) else {
                continue;
            };
            let locations = wz_db::npc_locations(&location_image, id);
            let npc_name = linker.string_npc.get(&id).map(|sr| sr.name.clone());
            rows.push(
                [
                    "npc".to_string(),
                    id.to_string(),
                    npc_name.clone().unwrap_or_default(),
                    String::new(),
                    String::new(),
                    wz_db::npc_lines(npc_name.as_deref(), id, &locations, &map_name).join("|"),
                ]
                .join(&rec.to_string()),
            );
        }
        let reactor_archive = wz_core::WzArchive::open(&reactor).unwrap();
        for entry in &reactor_archive.read_tree().unwrap().root.entries {
            if entry.node_type != 4 {
                continue;
            }
            let Some(offset) = entry.offset else { continue };
            let Ok(mut node) = reactor_archive.parse_image_at(offset) else {
                continue;
            };
            let Some(name) = entry.name.as_deref() else {
                continue;
            };
            node.name = std::sync::Arc::from(name);
            let Some(t) = wz_db::reactor_from_node(&node) else {
                continue;
            };
            rows.push(
                [
                    "reactor".to_string(),
                    t.id.to_string(),
                    t.name.unwrap_or_default(),
                    t.action.unwrap_or_default(),
                    t.clip_count.to_string(),
                    t.lines.join("|"),
                ]
                .join(&rec.to_string()),
            );
        }
        let canon = rows.join(&rec.to_string());
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/chara_tooltips.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len() as u64, expected["rows"].as_u64().unwrap());
        assert_eq!(
            sha256_hex(canon.as_bytes()),
            expected["sha256"].as_str().unwrap()
        );
    }
}

pub fn cmd_damage_skin_model(
    wz_path: &Path,
    image: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let root = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some(image))
        .and_then(|e| e.offset)
        .and_then(|o| archive.parse_image_at(o).ok())
        .ok_or_else(|| format!("이미지를 찾을 수 없습니다: {image}"))?;
    let mut rows: Vec<wz_db::DamageSkin> = root
        .object_children()
        .iter()
        .filter_map(wz_db::DamageSkin::from_node)
        .collect();
    rows.sort_by_key(|s| s.id);
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for skin in &rows {
            println!(
                "{} desc={:?} extract={} sample={} sections={}",
                skin.id,
                skin.desc,
                skin.extract_id,
                skin.has_sample,
                skin.sections.len()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod damage_skin_model_tests {
    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn damage_skin_models_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Etc/Etc_000.wz");
        if !wz.exists() {
            return;
        }
        let archive = wz_core::WzArchive::open(&wz).unwrap();
        let tree = archive.read_tree().unwrap();
        let root_node = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("DamageSkin.img"))
            .and_then(|e| e.offset)
            .and_then(|o| archive.parse_image_at(o).ok())
            .unwrap();
        let mut skins: Vec<wz_db::DamageSkin> = root_node
            .object_children()
            .iter()
            .filter_map(wz_db::DamageSkin::from_node)
            .collect();
        skins.sort_by_key(|s| s.id);
        let rec = char::from(0x1e);
        let field = char::from(0x1f);
        let canon = skins
            .iter()
            .map(|s| {
                let secs = s
                    .sections
                    .iter()
                    .map(|(k, v)| {
                        [
                            k.clone(),
                            v.keys.join("|"),
                            v.number_space.to_string(),
                            if v.bottom_fix {
                                "1".into()
                            } else {
                                "0".to_string()
                            },
                            v.dot_space.to_string(),
                        ]
                        .join(&field.to_string())
                    })
                    .collect::<Vec<_>>()
                    .join(";");
                [
                    s.id.to_string(),
                    s.desc.clone().unwrap_or_default(),
                    s.extract_id.to_string(),
                    s.custom_type.clone().unwrap_or_default(),
                    if s.has_sample {
                        "1".into()
                    } else {
                        "0".to_string()
                    },
                    secs,
                ]
                .join(&rec.to_string())
            })
            .collect::<Vec<_>>()
            .join(&rec.to_string());

        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/damage_skins.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(skins.len() as u64, expected["rows"].as_u64().unwrap());
        assert_eq!(
            sha256_hex(canon.as_bytes()),
            expected["sha256"].as_str().unwrap()
        );
    }
}

pub fn cmd_net_loopback() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use wz_network::pack::Packet;
    use wz_network::session::{read_frame, write_frame, ClientSession, ClientState};
    use wz_network::Rc4;

    fn b64(data: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for c in data.chunks(3) {
            let n = (u32::from(c[0]) << 16)
                | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                | u32::from(*c.get(2).unwrap_or(&0));
            out.push(T[(n >> 18) as usize & 63] as char);
            out.push(T[(n >> 12) as usize & 63] as char);
            out.push(if c.len() > 1 {
                T[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            out.push(if c.len() > 2 {
                T[n as usize & 63] as char
            } else {
                '='
            });
        }
        out
    }

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let server = std::thread::spawn(move || -> Result<(), String> {
        let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
        let mut read = None;
        let mut write = None;
        let req = read_frame(&mut stream, &mut read).map_err(|e| e.to_string())?;
        let req = req.ok_or("no CryptReq")?;
        if !String::from_utf8_lossy(&req).contains("\"$type\":\"1\"") {
            return Err("expected CryptReq".into());
        }
        let resp = format!(
            "{{\"$type\":\"2\",\"KeyEncryptedS2C\":\"{}\",\"KeyEncryptedC2S\":\"{}\"}}",
            b64(b"s2c-key"),
            b64(b"c2s-key")
        );
        write_frame(&mut stream, &mut write, resp.as_bytes()).map_err(|e| e.to_string())?;
        read = Rc4::new(b"c2s-key");
        write = Rc4::new(b"s2c-key");
        let login = read_frame(&mut stream, &mut read).map_err(|e| e.to_string())?;
        let login = login.ok_or("no LoginReq")?;
        if !String::from_utf8_lossy(&login).contains("\"WcID\":\"wc-01\"") {
            return Err("login mismatch".into());
        }
        write_frame(
            &mut stream,
            &mut write,
            br#"{"$type":"4","SessionID":"sess-42"}"#,
        )
        .map_err(|e| e.to_string())?;
        stream.flush().map_err(|e| e.to_string())?;
        Ok(())
    });

    let stream = TcpStream::connect(addr)?;
    let mut client = ClientSession::new(stream);
    println!("state {:?}", client.state);
    client.send_crypt_req(&[1, 0, 1], &[9, 9])?;
    println!("send PackCryptReq -> {:?}", client.state);
    let body = client.recv_packet()?.ok_or("no CryptResp")?;
    let identity = |data: &[u8]| Some(data.to_vec());
    if !client.handle_crypt_resp(&body, &identity)? {
        return Err("CryptResp handling failed".into());
    }
    println!(
        "recv PackCryptResp: s2c={:?} c2s={:?} -> {:?}",
        String::from_utf8_lossy(client.server_key.as_deref().unwrap_or_default()),
        String::from_utf8_lossy(client.client_key.as_deref().unwrap_or_default()),
        client.state
    );
    client.send_login("wc-01", "테스터")?;
    println!("send PackLoginReq -> {:?}", client.state);
    let body = client.recv_packet()?.ok_or("no LoginResp")?;
    if !client.handle_login_resp(&body) {
        return Err("LoginResp handling failed".into());
    }
    println!("recv PackLoginResp -> {:?}", client.state);
    server.join().map_err(|_| "server panicked")??;
    println!(
        "OK: {}",
        if client.state == ClientState::LoggedIn {
            "login flow complete"
        } else {
            "unexpected state"
        }
    );
    let _ = Packet::Heartbeat;
    Ok(())
}

pub fn cmd_avatar_unpack(
    version: i32,
    pack_hex: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let pack: Vec<u8> = (0..pack_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&pack_hex[i..i + 2], 16))
        .collect::<Result<_, _>>()?;
    let (effective, unknown, fields) = wz_openapi::avatar::unpack(version, &pack);
    if json {
        let rows: Vec<_> = fields
            .iter()
            .map(|f| serde_json::json!({"name": f.name, "bits": f.bits, "value": f.value}))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "version": effective,
                "unknown": unknown,
                "fields": rows,
            }))?
        );
    } else {
        println!(
            "version {effective} (unknown={unknown}) fields={}",
            fields.len()
        );
        for f in fields.iter().take(12) {
            println!("  {}\t{}\t{}", f.name, f.bits, f.value);
        }
    }
    Ok(())
}

pub fn cmd_map_tooltips(
    wz_path: &Path,
    string_wz: &Path,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_db::{Arena, StringLinker};

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let string_archive = WzArchive::open(string_wz)?;
    let (string_arena, string_root) = Arena::from_archive(&string_archive);
    let mut linker = StringLinker::new();
    if !linker.load_string_wz(&string_arena, string_root) {
        return Err("StringLinker 로드 실패".into());
    }

    let mut rows = Vec::new();
    for entry in &tree.root.entries {
        if entry.node_type != 4 {
            continue;
        }
        let Some(offset) = entry.offset else {
            continue;
        };
        let Ok(mut node) = archive.parse_image_at(offset) else {
            continue;
        };
        let Some(name) = entry.name.as_deref() else {
            continue;
        };
        node.name = std::sync::Arc::from(name);
        let Some(map) = wz_db::MapTooltipFields::from_node(&node) else {
            continue;
        };
        let sr = linker
            .string_map
            .get(&map.id)
            .or_else(|| map.link.and_then(|link| linker.string_map.get(&link)));
        let street = sr.and_then(|sr| sr.street_name.clone());
        let map_name = sr.and_then(|sr| sr.map_name.clone());
        let desc = sr
            .and_then(|sr| sr.values.get("mapDesc").cloned())
            .filter(|s| !s.is_empty());
        let text = map.text(
            street,
            map_name,
            desc,
            &|id| linker.string_mob.get(&id).map(|sr| sr.name.clone()),
            &|id| linker.string_npc.get(&id).map(|sr| sr.name.clone()),
        );
        rows.push(serde_json::json!({
            "id": map.id,
            "link": map.link,
            "barrier": map.barrier,
            "barrier_arc": map.barrier_arc,
            "barrier_aut": map.barrier_aut,
            "map_mark": map.map_mark,
            "bgm": map.bgm,
            "street": text.street,
            "map_name": text.map_name,
            "id_text": text.id_text,
            "desc": text.desc,
            "bgm_text": text.bgm,
            "mobs": text.mobs,
            "npcs": text.npcs,
        }));
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for row in &rows {
            println!(
                "{} {} / {} {}",
                row["id"], row["street"], row["map_name"], row["id_text"]
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod map_tooltip_tests {
    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn map_tooltips_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Map/Map/Map0/Map0_000.wz");
        let string_wz = root.join("String/String_000.wz");
        if !wz.exists() || !string_wz.exists() {
            return;
        }
        use wz_db::{Arena, StringLinker};
        let archive = wz_core::WzArchive::open(&wz).unwrap();
        let tree = archive.read_tree().unwrap();
        let string_archive = wz_core::WzArchive::open(&string_wz).unwrap();
        let (string_arena, string_root) = Arena::from_archive(&string_archive);
        let mut linker = StringLinker::new();
        assert!(linker.load_string_wz(&string_arena, string_root));

        let rec = char::from(0x1e);
        let mut rows = Vec::new();
        for entry in &tree.root.entries {
            if entry.node_type != 4 {
                continue;
            }
            let Some(offset) = entry.offset else { continue };
            let Ok(mut node) = archive.parse_image_at(offset) else {
                continue;
            };
            let Some(name) = entry.name.as_deref() else {
                continue;
            };
            node.name = std::sync::Arc::from(name);
            let Some(map) = wz_db::MapTooltipFields::from_node(&node) else {
                continue;
            };
            let sr = linker
                .string_map
                .get(&map.id)
                .or_else(|| map.link.and_then(|l| linker.string_map.get(&l)));
            let text = map.text(
                sr.and_then(|s| s.street_name.clone()),
                sr.and_then(|s| s.map_name.clone()),
                sr.and_then(|s| s.values.get("mapDesc").cloned())
                    .filter(|s| !s.is_empty()),
                &|id| linker.string_mob.get(&id).map(|s| s.name.clone()),
                &|id| linker.string_npc.get(&id).map(|s| s.name.clone()),
            );
            let mobs = text
                .mobs
                .iter()
                .map(|(id, n)| format!("{id}:{n}"))
                .collect::<Vec<_>>()
                .join("|");
            let npcs = text
                .npcs
                .iter()
                .map(|(id, n)| format!("{id}:{n}"))
                .collect::<Vec<_>>()
                .join("|");
            rows.push(
                [
                    map.id.to_string(),
                    map.link.map(|v| v.to_string()).unwrap_or_default(),
                    map.barrier.to_string(),
                    map.barrier_arc.to_string(),
                    map.barrier_aut.to_string(),
                    map.map_mark.clone().unwrap_or_default(),
                    map.bgm.clone().unwrap_or_default(),
                    text.street,
                    text.map_name,
                    text.id_text,
                    text.desc.unwrap_or_default(),
                    mobs,
                    npcs,
                ]
                .join(&rec.to_string()),
            );
        }
        let canon = rows.join(&rec.to_string());
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/map_tooltips.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len() as u64, expected["rows"].as_u64().unwrap());
        assert_eq!(
            sha256_hex(canon.as_bytes()),
            expected["sha256"].as_str().unwrap()
        );
    }
}

pub fn cmd_quest_tooltips(
    wz_path: &Path,
    json: bool,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let mut rows = Vec::new();
    for entry in &tree.root.entries {
        if entry.node_type != 4 {
            continue;
        }
        let Some(offset) = entry.offset else {
            continue;
        };
        let Ok(mut node) = archive.parse_image_at(offset) else {
            continue;
        };
        let Some(name) = entry.name.as_deref() else {
            continue;
        };
        node.name = std::sync::Arc::from(name);
        if let Some(quest) = wz_db::QuestTooltip::from_node(&node) {
            rows.push(quest);
        }
    }
    rows.sort_by_key(|q| q.id);
    rows.truncate(limit);
    if json {
        let rows: Vec<_> = rows
            .into_iter()
            .map(|q| {
                let lines = q.summary_lines();
                serde_json::json!({
                    "id": q.id,
                    "name": q.name,
                    "desc": q.desc,
                    "blocked": q.blocked,
                    "lvmin": q.lvmin,
                    "lvmax": q.lvmax,
                    "lv_limit": q.lv_limit,
                    "recommend_except": q.recommend_except,
                    "demand_base": q.demand_base,
                    "check1_npc": q.check1_npc,
                    "check1_items": q.check1_items,
                    "check1_infoex": q.check1_infoex,
                    "act1_reward": q.act1_reward,
                    "lines": lines,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for q in &rows {
            println!("{} {}", q.id, q.name.clone().unwrap_or_default());
        }
    }
    Ok(())
}

#[cfg(test)]
mod quest_tooltip_tests {
    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn quest_tooltips_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let wz = root.join("Quest/QuestData/QuestData_000.wz");
        if !wz.exists() {
            return;
        }
        let archive = wz_core::WzArchive::open(&wz).unwrap();
        let tree = archive.read_tree().unwrap();
        let mut quests = Vec::new();
        for entry in &tree.root.entries {
            if entry.node_type != 4 {
                continue;
            }
            let Some(offset) = entry.offset else { continue };
            let Ok(mut node) = archive.parse_image_at(offset) else {
                continue;
            };
            let Some(name) = entry.name.as_deref() else {
                continue;
            };
            node.name = std::sync::Arc::from(name);
            if let Some(q) = wz_db::QuestTooltip::from_node(&node) {
                quests.push(q);
            }
        }
        quests.sort_by_key(|q| q.id);

        let rec = char::from(0x1e);
        let field = char::from(0x1f);
        let canon = quests
            .iter()
            .map(|q| {
                let aw = &q.act1_reward;
                let reward = [
                    aw.exp.to_string(),
                    aw.meso.to_string(),
                    aw.pop.to_string(),
                    aw.pet_tameness.to_string(),
                    aw.attr_exps
                        .iter()
                        .map(|(k, v)| format!("{k}:{v}"))
                        .collect::<Vec<_>>()
                        .join(";"),
                    aw.items
                        .iter()
                        .map(|i| {
                            format!(
                                "{}:{}:{}:{}:{}",
                                i.id,
                                i.count,
                                i.prob
                                    .map(|p| p.to_string())
                                    .unwrap_or_else(|| "None".into()),
                                i.job,
                                i.gender
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(";"),
                    aw.sps
                        .iter()
                        .map(|s| {
                            format!(
                                "{}:{}",
                                s.sp,
                                s.jobs
                                    .iter()
                                    .map(|j| j.to_string())
                                    .collect::<Vec<_>>()
                                    .join(",")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(";"),
                ]
                .join(&field.to_string());
                [
                    q.id.to_string(),
                    q.name.clone().unwrap_or_default(),
                    q.desc
                        .iter()
                        .map(|d| d.clone().unwrap_or_default())
                        .collect::<Vec<_>>()
                        .join("|"),
                    if q.blocked {
                        "1".into()
                    } else {
                        "0".to_string()
                    },
                    q.lvmin.to_string(),
                    q.lvmax.to_string(),
                    if q.lv_limit {
                        "1".into()
                    } else {
                        "0".to_string()
                    },
                    if q.recommend_except {
                        "1".into()
                    } else {
                        "0".to_string()
                    },
                    q.demand_base.clone().unwrap_or_default(),
                    q.check1_npc.to_string(),
                    q.check1_items
                        .iter()
                        .map(|i| format!("{}:{}:{}", i.key, i.id, i.count))
                        .collect::<Vec<_>>()
                        .join(";"),
                    q.check1_infoex
                        .iter()
                        .map(|(k, v)| format!("{k}:{}", if *v { 1 } else { 0 }))
                        .collect::<Vec<_>>()
                        .join(";"),
                    reward,
                ]
                .join(&rec.to_string())
            })
            .collect::<Vec<_>>()
            .join(&rec.to_string());
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/quest_tooltips.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(quests.len() as u64, expected["rows"].as_u64().unwrap());
        assert_eq!(
            sha256_hex(canon.as_bytes()),
            expected["sha256"].as_str().unwrap()
        );
    }
}

pub fn cmd_plugin_files(dir: &Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let files: Vec<String> = wz_db::plugin_files(dir)
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_string))
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&files)?);
    } else {
        for f in files {
            println!("{f}");
        }
    }
    Ok(())
}

pub fn cmd_item_extras(
    data_root: &Path,
    dir: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut paths: Vec<_> = std::fs::read_dir(data_root.join("Item").join(dir))
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("wz"))
        .collect();
    paths.sort();
    let mut rows = Vec::new();
    for path in paths {
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        for entry in &tree.root.entries {
            if entry.node_type != 4 {
                continue;
            }
            let Some(offset) = entry.offset else {
                continue;
            };
            let Ok(image) = archive.parse_image_at(offset) else {
                continue;
            };
            for item in image.object_children() {
                if let Some(spec) = wz_db::RecipeSpec::from_item(item) {
                    let lines = spec.lines();
                    if lines.is_empty() {
                        continue;
                    }
                    rows.push(serde_json::json!({
                        "id": item.name.to_string(),
                        "valid_day": spec.valid_day,
                        "use_count": spec.use_count,
                        "lines": lines,
                    }));
                }
            }
        }
    }
    rows.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for row in &rows {
            println!("{} {:?}", row["id"], row["lines"]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod item_extra_tests {
    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn item_recipe_lines_match_csharp() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let dir = root.join("Item/Consume");
        if !dir.exists() {
            return;
        }
        let mut paths: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("wz"))
            .collect();
        paths.sort();
        let mut rows = Vec::new();
        for path in paths {
            let Ok(archive) = wz_core::WzArchive::open(&path) else {
                continue;
            };
            let Ok(tree) = archive.read_tree() else {
                continue;
            };
            for entry in &tree.root.entries {
                if entry.node_type != 4 {
                    continue;
                }
                let Some(offset) = entry.offset else { continue };
                let Ok(image) = archive.parse_image_at(offset) else {
                    continue;
                };
                for item in image.object_children() {
                    let Some(spec) = wz_db::RecipeSpec::from_item(item) else {
                        continue;
                    };
                    let lines = spec.lines();
                    if lines.is_empty() {
                        continue;
                    }
                    rows.push((item.name.to_string(), spec, lines));
                }
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        let rec = char::from(0x1e);
        let canon = rows
            .iter()
            .map(|(id, spec, lines)| {
                [
                    id.clone(),
                    spec.valid_day.map(|v| v.to_string()).unwrap_or_default(),
                    spec.use_count.map(|v| v.to_string()).unwrap_or_default(),
                    lines.join("|"),
                ]
                .join(&rec.to_string())
            })
            .collect::<Vec<_>>()
            .join(&rec.to_string());
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/item_extras.expected.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len() as u64, expected["rows"].as_u64().unwrap());
        assert_eq!(
            sha256_hex(canon.as_bytes()),
            expected["sha256"].as_str().unwrap()
        );
    }
}

pub fn cmd_net_reconnect() -> Result<(), Box<dyn std::error::Error>> {
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;
    use wz_network::pack::Packet;
    use wz_network::session::{read_frame, write_frame, Backoff, ReconnectingClient};

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let server = std::thread::spawn(move || -> Result<(), String> {
        {
            let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
            let mut read = None;
            let mut write = None;
            write_frame(&mut stream, &mut write, Packet::Heartbeat.json().as_bytes())
                .map_err(|e| e.to_string())?;
            let echo = read_frame(&mut stream, &mut read)
                .map_err(|e| e.to_string())?
                .ok_or("no heartbeat echo")?;
            if !String::from_utf8_lossy(&echo).contains("\"$type\":\"0\"") {
                return Err("heartbeat echo mismatch".into());
            }
        }
        let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
        let mut write = None;
        write_frame(
            &mut stream,
            &mut write,
            br#"{"$type":"4","SessionID":"after-reconnect"}"#,
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    });

    let mut client = ReconnectingClient::new(
        move || TcpStream::connect(addr),
        Backoff {
            initial: Duration::from_millis(1),
            factor: 1.0,
            max: Duration::from_millis(1),
            max_attempts: 3,
        },
    );
    client.connect()?;
    println!("connected (attempts={})", client.attempts);
    let packet = client.next_packet()?.ok_or("no packet")?;
    println!(
        "heartbeat echoed, then reconnect #{}; packet={}",
        client.reconnects,
        String::from_utf8_lossy(&packet)
    );
    server.join().map_err(|_| "server panicked")??;
    println!("OK: heartbeat echo + reconnect");
    Ok(())
}
