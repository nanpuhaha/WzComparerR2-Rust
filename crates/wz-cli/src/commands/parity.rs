//! 클라이언트 데이터 존재 여부 전수 스캔.
//!
//! 원본 C# 기능 중 데이터가 없어 검증할 수 없는 항목(Skill `common`/`level`,
//! WorldMap `QuestLimit`/`Fog`, Familiar, `.BIN`)을 실제 `Data` 트리에서
//! 다시 세어 근거를 남긴다. `weather`는 원본 소스에 구현 자체가 없다.

use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use wz_core::image::Node;
use wz_core::WzArchive;

fn wz_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("wz") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn find_files(data_root: &Path, predicate: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![data_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if predicate(name) {
                    out.push(path);
                }
            }
        }
    }
    out.sort();
    out
}

fn image_nodes(archive: &WzArchive) -> Vec<Node> {
    let Ok(tree) = archive.read_tree() else {
        return Vec::new();
    };
    tree.root
        .entries
        .iter()
        .filter(|e| e.node_type == 4)
        .filter_map(|e| e.offset)
        .filter_map(|offset| archive.parse_image_at(offset).ok())
        .collect()
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn count_skill_nodes(node: &Node, stats: &mut (u64, u64, u64)) {
    for c in node.object_children() {
        if c.name.as_ref() == "common" {
            stats.1 += 1;
        }
        if c.name.as_ref() == "level" {
            stats.2 += 1;
        }
        count_skill_nodes(c, stats);
    }
}

fn scan_skills(root: &Path) -> (u64, u64, u64) {
    // (skill nodes, common nodes, level nodes)
    let mut stats = (0, 0, 0);
    for path in wz_files(root) {
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        for image in image_nodes(&archive) {
            let Some(skill_root) = child(&image, "skill") else {
                continue;
            };
            for skill in skill_root.object_children() {
                stats.0 += 1;
                count_skill_nodes(skill, &mut stats);
            }
        }
    }
    stats
}

fn count_names(node: &Node, counts: &mut BTreeMap<String, u64>) {
    for c in node.object_children() {
        let name = c.name.to_string();
        if matches!(name.as_str(), "QuestLimit" | "Fog" | "fog" | "fogColor") {
            *counts.entry(name).or_insert(0) += 1;
        }
        count_names(c, counts);
    }
}

fn scan_mob(root: &Path) -> (u64, u64, u64, u64) {
    // (images, info nodes, level, elemAttr)
    let mut stats = (0, 0, 0, 0);
    for path in wz_files(root) {
        if path.file_name().and_then(|s| s.to_str()) == Some("Mob.wz") {
            continue;
        }
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        for image in image_nodes(&archive) {
            stats.0 += 1;
            let Some(info) = child(&image, "info") else {
                continue;
            };
            stats.1 += 1;
            if child(info, "level").is_some() {
                stats.2 += 1;
            }
            if child(info, "elemAttr").is_some() {
                stats.3 += 1;
            }
        }
    }
    stats
}

fn scan_item_extra(root: &Path) -> (u64, u64, u64, u64) {
    // (items, recipe spec, exp_minLev, pet interact)
    let mut stats = (0, 0, 0, 0);
    let Ok(dirs) = std::fs::read_dir(root.join("Item")) else {
        return stats;
    };
    for dir in dirs.flatten() {
        let dir = dir.path();
        if !dir.is_dir() {
            continue;
        }
        for path in wz_files(&dir) {
            let Ok(archive) = WzArchive::open(&path) else {
                continue;
            };
            for image in image_nodes(&archive) {
                for item in image.object_children() {
                    stats.0 += 1;
                    if let Some(spec) = child(item, "spec") {
                        if child(spec, "recipeValidDay").is_some()
                            || child(spec, "recipeUseCount").is_some()
                        {
                            stats.1 += 1;
                        }
                    }
                    if let Some(info) = child(item, "info") {
                        if child(info, "exp_minLev").is_some() {
                            stats.2 += 1;
                        }
                    }
                    if child(item, "interact").is_some() {
                        stats.3 += 1;
                    }
                }
            }
        }
    }
    stats
}

fn scan_worldmap(root: &Path) -> (u64, BTreeMap<String, u64>) {
    let mut images = 0;
    let mut counts = BTreeMap::new();
    for path in wz_files(root) {
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        for image in image_nodes(&archive) {
            images += 1;
            count_names(&image, &mut counts);
        }
    }
    (images, counts)
}

/// `wz parity-scan` 구현.
pub fn cmd_parity_scan(data_root: &Path) -> Result<(), Box<dyn Error>> {
    println!("Data root: {}", data_root.display());

    let (skills, commons, levels) = scan_skills(&data_root.join("Skill"));
    println!("Skill: {skills} skill nodes, {commons} common nodes, {levels} level nodes");

    let (worldmap_images, worldmap_counts) = scan_worldmap(&data_root.join("Map/WorldMap"));
    println!("WorldMap: {worldmap_images} images");
    for (name, count) in &worldmap_counts {
        println!("WorldMap node {name}: {count}");
    }
    if worldmap_counts.is_empty() {
        println!("WorldMap node QuestLimit/Fog: 0");
    }

    let (mob_images, mob_info, mob_level, mob_elem) = scan_mob(&data_root.join("Mob"));
    println!(
        "Mob: {mob_images} images, {mob_info} info nodes, {mob_level} level, {mob_elem} elemAttr"
    );

    let (items, recipes, exp_limits, pet_interacts) = scan_item_extra(data_root);
    println!(
        "Item: {items} items, {recipes} recipe specs, {exp_limits} exp_minLev, {pet_interacts} pet interact"
    );

    let achievement = find_files(data_root, |name| {
        name.to_ascii_lowercase().contains("achievement")
    });
    println!("Achievement files: {}", achievement.len());
    let recipe = find_files(data_root, |name| name.eq_ignore_ascii_case("Recipe.img"));
    println!("Recipe.img files: {}", recipe.len());

    let familiar = find_files(data_root, |name| {
        name.to_ascii_lowercase().contains("familiar")
    });
    println!("Familiar files: {}", familiar.len());

    let bins = find_files(data_root, |name| {
        name.to_ascii_uppercase().ends_with(".BIN")
    });
    println!("BIN files: {}", bins.len());

    println!("weather: 원본 C# 소스에 weather 구현 없음");
    Ok(())
}
