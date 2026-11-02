use super::image::{dump_node_to_json, print_node};



pub fn print_diffs(title: &str, diffs: &[wz_diff::DiffItem]) {
    use wz_diff::DiffType;
    println!("{title} (총 {} 건)", diffs.len());
    for d in diffs {
        let tag = match d.diff_type {
            DiffType::Append => "+",
            DiffType::Delete => "-",
            DiffType::Changed => "*",
        };
        println!("[{tag}] {} ({})", d.path, d.description);
    }
}
pub fn cmd_diff(
    old_wz: &std::path::Path,
    new_wz: &std::path::Path,
    old_offset: Option<u64>,
    new_offset: Option<u64>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_diff::DiffEngine;

    let (old_off, new_off) = match (old_offset, new_offset) {
        (Some(o1), Some(o2)) => (Some(o1), Some(o2)),
        (Some(o1), None) => (Some(o1), Some(o1)),
        (None, Some(o2)) => (Some(o2), Some(o2)),
        (None, None) => (None, None),
    };

    match (old_off, new_off) {
        (Some(o1), Some(o2)) => {
            let a1 = wz_core::WzArchive::open(old_wz)?;
            let a2 = wz_core::WzArchive::open(new_wz)?;
            let n1 = a1.parse_image_at(o1)?;
            let n2 = a2.parse_image_at(o2)?;
            let diffs = DiffEngine::diff_core_nodes(Some(&n1), Some(&n2), "root");
            if json {
                println!("{}", serde_json::to_string_pretty(&diffs)?);
            } else {
                print_diffs("=== 이미지 노드 차이점 ===", &diffs);
            }
        }
        _ => {
            let a1 = wz_core::WzArchive::open(old_wz)?;
            let a2 = wz_core::WzArchive::open(new_wz)?;
            let t1 = a1.read_tree()?;
            let t2 = a2.read_tree()?;
            let diffs = DiffEngine::diff_dirtree(&t1, &t2, "root");

            if json {
                println!("{}", serde_json::to_string_pretty(&diffs)?);
            } else {
                print_diffs("=== WZ 디렉터리 트리 차이점 ===", &diffs);
            }
        }
    }
    Ok(())
}
pub fn cmd_dump(
    path: &std::path::Path,
    offset: u64,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = wz_core::WzArchive::open(path)?;
    let root = archive.parse_image_at(offset)?;

    if json {
        let dump_json = dump_node_to_json(&root);
        println!("{}", serde_json::to_string_pretty(&dump_json)?);
    } else {
        println!("=== 이미지 캔버스/노드 덤프 @{offset} ===");
        print_node(&root, 0, 0);
    }
    Ok(())
}
