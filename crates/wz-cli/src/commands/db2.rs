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
