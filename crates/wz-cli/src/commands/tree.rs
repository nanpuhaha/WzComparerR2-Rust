pub fn cmd_tree(
    path: &std::path::Path,
    hash_version: u64,
    shallow: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::WzArchive;

    let archive = WzArchive::open_with_hash_version(path, hash_version)?;

    if !json {
        println!("파일: {}", archive.source().describe());
        println!(
            "프로파일: {} / KMST{}",
            archive.detection().reason,
            archive.detection().profile.wz_version
        );
        println!("hashVersion: {:#018x}", archive.detection().hash_version);
        println!("headerSize: {}", archive.detection().detected.header_len);
    }

    let tree = archive.read_tree_with_mode(shallow)?;

    if json {
        let out = tree_to_json(&tree);
        println!("{out}");
    } else {
        print_tree(&tree, 0);
    }
    Ok(())
}
pub fn print_tree(tree: &wz_core::dirtree::DirTree, indent: usize) {
    use wz_core::dirtree::node_type;
    let pad = "  ".repeat(indent);
    println!("{}./", pad);
    for e in &tree.root.entries {
        let kind = match e.node_type {
            node_type::DIRECTORY => "dir ",
            node_type::IMAGE_NAMED_BY_REF => "img*",
            _ => "img ",
        };
        let name = e.name.as_deref().unwrap_or("<참조>");
        match e.offset {
            Some(off) => println!("{pad}  {kind} {name}  @{off} ({} 바이트)", e.data_len),
            None => println!("{pad}  {kind} {name}  @? (오프셋 미해결)"),
        }
    }
    for b in &tree.blocks {
        println!("{}├─ 하위 블록 ({} 엔트리)", pad, b.entries.len());
        for e in &b.entries {
            let kind = match e.node_type {
                node_type::DIRECTORY => "dir ",
                node_type::IMAGE_NAMED_BY_REF => "img*",
                _ => "img ",
            };
            let name = e.name.as_deref().unwrap_or("<참조>");
            match e.offset {
                Some(off) => println!("{pad}│  {kind} {name}  @{off}"),
                None => println!("{pad}│  {kind} {name}  @?"),
            }
        }
    }
}
pub fn tree_to_json(tree: &wz_core::dirtree::DirTree) -> String {
    use wz_core::dirtree::node_type;
    let block_json = |b: &wz_core::dirtree::DirBlock| {
        let items: Vec<String> = b
            .entries
            .iter()
            .map(|e| {
                let ty = match e.node_type {
                    node_type::DIRECTORY => "dir",
                    node_type::IMAGE_NAMED_BY_REF => "image_ref",
                    _ => "image",
                };
                format!(
                    r#"{{"type":"{ty}","name":{},"offset":{},"len":{},"cs32":{}}}"#,
                    serde_json::to_string(e.name.as_deref().unwrap_or("")).unwrap(),
                    e.offset.map(|o| o.to_string()).unwrap_or("null".into()),
                    e.data_len,
                    e.cs32
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    };
    let blocks: Vec<String> = tree.blocks.iter().map(block_json).collect();
    format!(
        r#"{{"root":{},"subdirs":[{}]}}"#,
        block_json(&tree.root),
        blocks.join(",")
    )
}
