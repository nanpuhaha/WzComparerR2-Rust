use super::*;
pub fn cmd_oracle_tree(
    path: &std::path::Path,
    filter_pattern: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    use regex::Regex;
    use wz_core::WzArchive;

    let archive = WzArchive::open(path)?;
    let tree = archive.read_tree()?;
    let detection = archive.detection();

    let filter_re = if let Some(pat) = filter_pattern {
        Some(Regex::new(pat)?)
    } else {
        None
    };

    let mut node_count = 0usize;
    let root_val = build_oracle_dirtree_with_archive(
        &tree,
        Some(&archive),
        filter_re.as_ref(),
        &mut node_count,
    );

    let mut doc = oracle_envelope("tree", detection);
    let mut res_map = serde_json::Map::new();
    res_map.insert("root".into(), root_val);
    res_map.insert(
        "node_count".into(),
        serde_json::Value::Number(node_count.into()),
    );
    doc.insert("result".into(), serde_json::Value::Object(res_map));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}

pub fn build_oracle_dirtree_with_archive(
    tree: &wz_core::dirtree::DirTree,
    archive: Option<&wz_core::WzArchive>,
    filter: Option<&regex::Regex>,
    node_count: &mut usize,
) -> serde_json::Value {
    *node_count += 1; // root itself

    let mut block_cursor = 0;
    let children = build_dir_block_children(
        &tree.root,
        &tree.blocks,
        &mut block_cursor,
        archive,
        filter,
        node_count,
    );

    let mut root_obj = serde_json::Map::new();
    root_obj.insert("name".into(), serde_json::Value::String("".into()));
    root_obj.insert("type".into(), serde_json::Value::String("Directory".into()));
    root_obj.insert("children".into(), serde_json::Value::Array(children));
    serde_json::Value::Object(root_obj)
}

pub fn build_dir_block_children(
    block: &wz_core::dirtree::DirBlock,
    blocks: &[wz_core::dirtree::DirBlock],
    block_cursor: &mut usize,
    archive: Option<&wz_core::WzArchive>,
    filter: Option<&regex::Regex>,
    node_count: &mut usize,
) -> Vec<serde_json::Value> {
    use wz_core::dirtree::node_type;
    use wz_core::text::sort_natural;

    let mut entries = Vec::new();

    for e in &block.entries {
        let name = e.name.as_deref().unwrap_or("");
        if let Some(re) = filter {
            if !re.is_match(name) && e.node_type != node_type::DIRECTORY {
                continue;
            }
        }

        *node_count += 1;
        let mut entry_obj = serde_json::Map::new();
        entry_obj.insert("name".into(), serde_json::Value::String(name.to_string()));

        if e.node_type == node_type::DIRECTORY {
            entry_obj.insert("type".into(), serde_json::Value::String("Directory".into()));
            if let Some(sub_block) = blocks.get(*block_cursor) {
                *block_cursor += 1;
                let sub_children = build_dir_block_children(
                    sub_block,
                    blocks,
                    block_cursor,
                    archive,
                    filter,
                    node_count,
                );
                entry_obj.insert("children".into(), serde_json::Value::Array(sub_children));
            } else {
                entry_obj.insert("children".into(), serde_json::Value::Array(Vec::new()));
            }
        } else {
            entry_obj.insert("type".into(), serde_json::Value::String("Image".into()));
            let mut img_info = serde_json::Map::new();
            // `offset` 은 **의도적으로 넣지 않는다**. C# `Wz_File.ReadDirTree` 는
            // `ReadContext.OffsetCalc` 가 있을 때만 `img.Offset` 을 채우는데 그
            // 컨텍스트는 PKG2 경로에서만 설정된다 — PKG1 에서는 항상 0 이라
            // 오라클이 키 자체를 비운다 (#28 리포트 ②). 한쪽만 낼 수 있는 필드를
            // 비교 대상에 두면 `tree` 게이트가 영원히 빨간불이 된다.
            // 사람이 보는 `wz tree` 는 별도 경로(`@<offset>`)라 영향이 없다.
            img_info.insert("len".into(), serde_json::Value::Number(e.data_len.into()));
            img_info.insert("cs32".into(), serde_json::Value::Number(e.cs32.into()));
            entry_obj.insert("image".into(), serde_json::Value::Object(img_info));

            if let (Some(arch), Some(off)) = (archive, e.offset) {
                if let Ok(img_root) = arch.parse_image_at(off) {
                    let img_children =
                        build_oracle_image_children(&img_root, arch.source(), node_count);
                    entry_obj.insert("children".into(), serde_json::Value::Array(img_children));
                }
            }
        }

        entries.push(entry_obj);
    }

    // natural_cmp 정렬 적용
    sort_natural(&mut entries, |item| {
        item.get("name").and_then(|v| v.as_str()).unwrap_or("")
    });

    entries.into_iter().map(serde_json::Value::Object).collect()
}

pub fn build_oracle_image_children(
    node: &wz_core::image::Node,
    src: &std::sync::Arc<dyn wz_pool::ByteSource>,
    node_count: &mut usize,
) -> Vec<serde_json::Value> {
    use wz_core::image::ImageValue;
    use wz_core::text::sort_natural;

    let mut raw_children: Vec<&wz_core::image::Node> = Vec::new();
    if let Some(ImageValue::Property(inner)) = &node.value {
        for ch in &inner.children {
            raw_children.push(ch);
        }
    }
    for ch in &node.children {
        raw_children.push(ch);
    }

    let mut items = Vec::with_capacity(raw_children.len());
    for ch in raw_children {
        items.push(build_oracle_image_node(ch, src, node_count));
    }

    sort_natural(&mut items, |item| {
        item.get("name").and_then(|v| v.as_str()).unwrap_or("")
    });

    items.into_iter().map(serde_json::Value::Object).collect()
}

pub fn build_oracle_image_node(
    node: &wz_core::image::Node,
    src: &std::sync::Arc<dyn wz_pool::ByteSource>,
    node_count: &mut usize,
) -> serde_json::Map<String, serde_json::Value> {
    use wz_core::image::ImageValue;

    *node_count += 1;
    let mut map = serde_json::Map::new();
    map.insert(
        "name".into(),
        serde_json::Value::String(node.name.to_string()),
    );

    match &node.value {
        Some(ImageValue::Property(_)) | None => {
            map.insert("type".into(), serde_json::Value::String("Property".into()));
            let children = build_oracle_image_children(node, src, node_count);
            map.insert("children".into(), serde_json::Value::Array(children));
        }
        Some(ImageValue::Png(png_ref)) => {
            map.insert("type".into(), serde_json::Value::String("Canvas".into()));
            let sha_hex = compute_png_ref_sha256(png_ref, src).unwrap_or_default();
            let mut canvas_obj = serde_json::Map::new();
            canvas_obj.insert("w".into(), serde_json::Value::Number(png_ref.width.into()));
            canvas_obj.insert("h".into(), serde_json::Value::Number(png_ref.height.into()));
            canvas_obj.insert(
                "format".into(),
                serde_json::Value::Number(png_ref.format.into()),
            );
            canvas_obj.insert(
                "scale".into(),
                serde_json::Value::Number(png_ref.scale.into()),
            );
            canvas_obj.insert(
                "pages".into(),
                serde_json::Value::Number(png_ref.pages.into()),
            );
            canvas_obj.insert("sha256".into(), serde_json::Value::String(sha_hex));
            map.insert("canvas".into(), serde_json::Value::Object(canvas_obj));

            let mut sub_children = build_oracle_image_children(node, src, node_count);
            if let Some(sub_prop) = &png_ref.sub_property {
                sub_children.extend(build_oracle_image_children(sub_prop, src, node_count));
            }
            if !sub_children.is_empty() {
                map.insert("children".into(), serde_json::Value::Array(sub_children));
            }
        }
        Some(ImageValue::Uol(u)) => {
            map.insert("type".into(), serde_json::Value::String("UOL".into()));
            map.insert("value".into(), serde_json::Value::String(u.to_string()));
        }
        Some(ImageValue::Short(v)) => {
            map.insert("type".into(), serde_json::Value::String("Short".into()));
            map.insert("value".into(), serde_json::Value::Number((*v).into()));
        }
        Some(ImageValue::Int(v)) => {
            map.insert("type".into(), serde_json::Value::String("Int".into()));
            map.insert("value".into(), serde_json::Value::Number((*v).into()));
        }
        Some(ImageValue::Long(v)) => {
            map.insert("type".into(), serde_json::Value::String("Long".into()));
            map.insert("value".into(), serde_json::Value::Number((*v).into()));
        }
        Some(ImageValue::Uint(v)) => {
            map.insert("type".into(), serde_json::Value::String("UInt".into()));
            map.insert("value".into(), serde_json::Value::Number((*v).into()));
        }
        Some(ImageValue::Float(v)) => {
            map.insert("type".into(), serde_json::Value::String("Float".into()));
            map.insert(
                "value".into(),
                serde_json::Number::from_f64(*v as f64)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
            );
        }
        Some(ImageValue::Double(v)) => {
            map.insert("type".into(), serde_json::Value::String("Double".into()));
            map.insert(
                "value".into(),
                serde_json::Number::from_f64(*v)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
            );
        }
        Some(ImageValue::Bool(v)) => {
            map.insert("type".into(), serde_json::Value::String("Bool".into()));
            map.insert("value".into(), serde_json::Value::Bool(*v));
        }
        Some(ImageValue::Str(s)) => {
            map.insert("type".into(), serde_json::Value::String("String".into()));
            map.insert("value".into(), serde_json::Value::String(s.to_string()));
        }
        Some(ImageValue::Vector(v)) => {
            map.insert("type".into(), serde_json::Value::String("Vector".into()));
            map.insert(
                "value".into(),
                serde_json::Value::Array(vec![
                    serde_json::Value::Number(v.x.into()),
                    serde_json::Value::Number(v.y.into()),
                ]),
            );
        }
        Some(ImageValue::Null) => {
            map.insert("type".into(), serde_json::Value::String("Null".into()));
            map.insert("value".into(), serde_json::Value::Null);
        }
        Some(other) => {
            map.insert(
                "type".into(),
                serde_json::Value::String(format!("{other:?}")),
            );
        }
    }

    map
}
