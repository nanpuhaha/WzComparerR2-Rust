use crate::OracleAction;
pub fn cmd_oracle(action: OracleAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        OracleAction::Tree { wz, filter } => cmd_oracle_tree(&wz, filter.as_deref()),
        OracleAction::Diff { base, r#mod } => cmd_oracle_diff(&base, &r#mod),
        OracleAction::Image { wz, path, format } => cmd_oracle_image(&wz, &path, &format),
        OracleAction::Tooltip { wz, item } => cmd_oracle_tooltip(&wz, item),
        OracleAction::Stats { wz, gear, prop } => cmd_oracle_stats(&wz, &gear, prop.as_deref()),
        OracleAction::Expr { expr, args } => cmd_oracle_expr(&expr, &args),
        OracleAction::Patch { patch } => cmd_oracle_patch(&patch),
        OracleAction::MapScene { wz, map } => cmd_oracle_mapscene(&wz, &map),
        OracleAction::Timeline {
            wz,
            path,
            node,
            canvas_wz,
            interpol_ms,
        } => cmd_oracle_timeline(&wz, path.as_deref(), &node, canvas_wz.as_deref(), interpol_ms),
        OracleAction::Avatar {
            body_wz,
            body,
            canvas_wz,
            zmap_wz,
            zmap,
            action,
            frame,
            elapsed,
        } => cmd_oracle_avatar(&AvatarParams {
            body_wz: &body_wz,
            body_img: body.as_deref(),
            canvas_wz: &canvas_wz,
            zmap_wz: &zmap_wz,
            zmap_img: zmap.as_deref(),
            action: &action,
            frame,
            elapsed,
        }),
    }
}
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
    let root_val =
        build_oracle_dirtree_with_archive(&tree, Some(&archive), filter_re.as_ref(), &mut node_count);

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
            img_info.insert(
                "len".into(),
                serde_json::Value::Number(e.data_len.into()),
            );
            img_info.insert(
                "cs32".into(),
                serde_json::Value::Number(e.cs32.into()),
            );
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
        item.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
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
        item.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
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
pub fn compute_png_ref_sha256(
    png_ref: &wz_core::image::PngRef,
    src: &std::sync::Arc<dyn wz_pool::ByteSource>,
) -> Result<String, Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};
    use wz_image::{decode_raw_page_to_rgba8, decompress_zlib, TextureFormat};

    let raw_payload = png_ref.extract_raw_bytes(src)?;
    let tex_fmt = TextureFormat::from_u32(png_ref.format as u32);

    let rgba_pixels = match tex_fmt {
        TextureFormat::Unknown => decompress_zlib(&raw_payload).unwrap_or(raw_payload),
        fmt => {
            // 멀티페이지(기본 0페이지) 및 RGB565 scale=16 업스케일링 규칙 적용.
            // 함정: 실패를 raw 바이트로 마스킹하면 C# 오라클과의 비교가 무의미해진다
            // — 실패는 실패로 뽑아야 한다.
            decode_raw_page_to_rgba8(
                fmt,
                png_ref.width as u32,
                png_ref.height as u32,
                png_ref.scale as u32,
                png_ref.pages as u32,
                0,
                &raw_payload,
            )
            .map(|(_, _, px)| px)?
        }
    };

    let mut hasher = Sha256::new();
    hasher.update(&rgba_pixels);
    Ok(format!("{:x}", hasher.finalize()))
}
pub fn cmd_oracle_diff(
    base_path: &std::path::Path,
    mod_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::WzArchive;
    use wz_diff::{DiffEngine, DiffType};

    let a1 = WzArchive::open(base_path)?;
    let a2 = WzArchive::open(mod_path)?;
    let t1 = a1.read_tree()?;
    let t2 = a2.read_tree()?;
    let detection = a2.detection();

    let diff_items = DiffEngine::diff_archives(&a1, &t1, &a2, &t2, "");

    let mut changed_count = 0usize;
    let mut added_count = 0usize;
    let mut removed_count = 0usize;

    let mut result_items = Vec::new();
    for d in diff_items {
        let kind = match d.diff_type {
            DiffType::Append => {
                added_count += 1;
                "append"
            }
            DiffType::Delete => {
                removed_count += 1;
                "remove"
            }
            DiffType::Changed => {
                changed_count += 1;
                "changed"
            }
        };

        let old_val = d
            .old_value
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);
        let new_val = d
            .new_value
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);

        let mut item = serde_json::Map::new();
        item.insert("path".into(), serde_json::Value::String(d.path));
        item.insert("kind".into(), serde_json::Value::String(kind.into()));
        item.insert("old".into(), old_val);
        item.insert("new".into(), new_val);
        result_items.push(serde_json::Value::Object(item));
    }

    let mut summary = serde_json::Map::new();
    summary.insert(
        "changed".into(),
        serde_json::Value::Number(changed_count.into()),
    );
    summary.insert(
        "added".into(),
        serde_json::Value::Number(added_count.into()),
    );
    summary.insert(
        "removed".into(),
        serde_json::Value::Number(removed_count.into()),
    );

    let mut doc = oracle_envelope("diff", detection);
    doc.insert("result".into(), serde_json::Value::Array(result_items));
    doc.insert("summary".into(), serde_json::Value::Object(summary));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}
pub fn cmd_oracle_image(
    wz_path: &std::path::Path,
    node_path: &str,
    format_opt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::WzArchive;

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let detection = archive.detection();

    // 경로 파싱: <dir/.../file.img>/<inner_path> 형태 분리
    let normalized = node_path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

    // .img 확장자를 가진 세그먼트 위치 탐색
    let img_idx = parts.iter().position(|p| p.to_lowercase().ends_with(".img"));

    let (img_entry, sub_path_parts) = if let Some(idx) = img_idx {
        let img_name = parts[idx];
        let found = find_image_entry_in_dirtree(&tree, img_name)
            .ok_or_else(|| format!("디렉터리 트리에서 이미지를 찾을 수 없습니다: {img_name}"))?;
        (found, &parts[idx + 1..])
    } else {
        // 경로 전체에서 첫 번째 .img 검색 또는 첫 번째 이미지 사용
        let found = tree
            .root
            .entries
            .iter()
            .find(|e| e.node_type == wz_core::dirtree::node_type::IMAGE)
            .cloned()
            .ok_or("WZ 아카이브에 이미지가 없습니다")?;
        (found, &parts[..])
    };

    let img_offset = img_entry.offset.ok_or("이미지 오프셋이 유효하지 않습니다")?;
    let img_root = archive.parse_image_at(img_offset)?;

    // 서브 노드 탐색
    let target_node = find_sub_node(&img_root, sub_path_parts)?;

    // 노드에서 PngRef 추출 (현재 노드 또는 첫 번째 Canvas 자식)
    let png_ref = extract_png_ref(target_node)
        .ok_or_else(|| format!("경로에서 Canvas/Png 노드를 찾을 수 없습니다: {node_path}"))?;

    let hash_hex = if format_opt.eq_ignore_ascii_case("sha256") {
        compute_png_ref_sha256(&png_ref, archive.source())?
    } else {
        let raw_payload = png_ref.extract_raw_bytes(archive.source())?;
        let decompressed = wz_image::decompress_zlib(&raw_payload)?;
        format!("len:{}", decompressed.len())
    };

    let mut doc = oracle_envelope("image", detection);

    let mut res = serde_json::Map::new();
    res.insert("w".into(), serde_json::Value::Number(png_ref.width.into()));
    res.insert("h".into(), serde_json::Value::Number(png_ref.height.into()));
    res.insert(
        "format".into(),
        serde_json::Value::Number(png_ref.format.into()),
    );
    res.insert(
        "scale".into(),
        serde_json::Value::Number(png_ref.scale.into()),
    );
    res.insert(
        "pages".into(),
        serde_json::Value::Number(png_ref.pages.into()),
    );
    res.insert("sha256".into(), serde_json::Value::String(hash_hex));
    doc.insert("result".into(), serde_json::Value::Object(res));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}
pub fn find_image_entry_in_dirtree(
    tree: &wz_core::dirtree::DirTree,
    img_name: &str,
) -> Option<wz_core::dirtree::RawEntry> {
    for e in &tree.root.entries {
        if e.name
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case(img_name))
            .unwrap_or(false)
        {
            return Some(e.clone());
        }
    }
    for b in &tree.blocks {
        for e in &b.entries {
            if e.name
                .as_deref()
                .map(|s| s.eq_ignore_ascii_case(img_name))
                .unwrap_or(false)
            {
                return Some(e.clone());
            }
        }
    }
    None
}
pub fn find_sub_node<'a>(
    root: &'a wz_core::image::Node,
    parts: &[&str],
) -> Result<&'a wz_core::image::Node, Box<dyn std::error::Error>> {
    let mut current = root;
    for part in parts {
        if let Some(child) = current.children.iter().find(|c| c.name.as_ref() == *part) {
            current = child;
        } else if let Some(wz_core::image::ImageValue::Property(p)) = &current.value {
            if let Some(child) = p.children.iter().find(|c| c.name.as_ref() == *part) {
                current = child;
            } else {
                return Err(format!("자식 노드를 찾을 수 없습니다: {part}").into());
            }
        } else {
            return Err(format!("자식 노드를 찾을 수 없습니다: {part}").into());
        }
    }
    Ok(current)
}
pub fn extract_png_ref(node: &wz_core::image::Node) -> Option<wz_core::image::PngRef> {
    if let Some(wz_core::image::ImageValue::Png(p)) = &node.value {
        return Some(p.clone());
    }
    for ch in &node.children {
        if let Some(p) = extract_png_ref(ch) {
            return Some(p);
        }
    }
    if let Some(wz_core::image::ImageValue::Property(p)) = &node.value {
        if let Some(res) = extract_png_ref(p) {
            return Some(res);
        }
    }
    None
}
pub fn cmd_oracle_tooltip(
    wz_path: &std::path::Path,
    item_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::WzArchive;
    use wz_db::StringLinker;

    let archive = WzArchive::open(wz_path)?;
    let detection = archive.detection();
    let (arena, _) = wz_db::Arena::from_archive(&archive);

    let mut linker = StringLinker::new();
    linker.load_string_wz(&arena, arena.root());

    let text = linker
        .format_tooltip(item_id)
        .unwrap_or_default();

    let mut doc = oracle_envelope("tooltip", detection);
    let mut res = serde_json::Map::new();
    res.insert("text".into(), serde_json::Value::String(text));
    doc.insert("result".into(), serde_json::Value::Object(res));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}
pub fn cmd_oracle_stats(
    wz_path: &std::path::Path,
    gear_id: &str,
    prop_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::WzArchive;

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let detection = archive.detection();

    let clean = gear_id.trim_end_matches(".img");
    let padded = if let Ok(num) = clean.parse::<u32>() {
        format!("{num:08}.img")
    } else {
        format!("{clean}.img")
    };
    let raw_name = format!("{clean}.img");

    let entry = find_image_entry_in_dirtree(&tree, &padded)
        .or_else(|| find_image_entry_in_dirtree(&tree, &raw_name))
        .ok_or_else(|| format!("장비 이미지를 찾을 수 없습니다: {gear_id}"))?;
    let off = entry.offset.ok_or("장비 이미지 오프셋이 없습니다")?;
    let img_root = archive.parse_image_at(off)?;

    let props = wz_db::extract_gear_props(&img_root);

    let mut doc = oracle_envelope("stats", detection);
    if let Some(prop_key) = prop_filter {
        let val = props.get(prop_key).cloned().unwrap_or(serde_json::Value::Null);
        let mut single_res = serde_json::Map::new();
        single_res.insert("prop".into(), serde_json::Value::String(prop_key.to_string()));
        single_res.insert("value".into(), val);
        doc.insert("result".into(), serde_json::Value::Object(single_res));
    } else {
        doc.insert("result".into(), serde_json::Value::Object(props));
    }

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}
pub fn cmd_oracle_expr(
    expr: &str,
    args_str: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<f64> = if args_str.trim().is_empty() {
        Vec::new()
    } else {
        args_str
            .split(',')
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect()
    };

    let evaluated = wz_db::evaluate_expression(expr, &args)?;

    let mut doc = serde_json::Map::new();
    doc.insert("schema_version".into(), serde_json::Value::Number(1.into()));
    doc.insert("op".into(), serde_json::Value::String("expr".into()));
    let mut res = serde_json::Map::new();
    res.insert("expr".into(), serde_json::Value::String(expr.to_string()));
    if let Some(num) = serde_json::Number::from_f64(evaluated) {
        res.insert("value".into(), serde_json::Value::Number(num));
    } else {
        res.insert("value".into(), serde_json::Value::Null);
    }
    doc.insert("result".into(), serde_json::Value::Object(res));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}
pub fn cmd_oracle_patch(
    patch_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;
    use wz_core::patch::{PatchHeader, PatchPartType};

    let src: Arc<dyn wz_pool::ByteSource> = wz_pool::open_file(patch_path)?.into();
    let header = PatchHeader::parse(&src)?;
    let parts = header.parse_parts(&src).unwrap_or_default();

    let mut doc = serde_json::Map::new();
    doc.insert("schema_version".into(), serde_json::Value::Number(1.into()));
    doc.insert("op".into(), serde_json::Value::String("patch".into()));
    let mut res = serde_json::Map::new();
    res.insert("version".into(), serde_json::Value::Number(header.version.into()));
    res.insert(
        "expected_checksum".into(),
        serde_json::Value::Number(header.expected_checksum.into()),
    );
    res.insert(
        "computed_checksum".into(),
        serde_json::Value::Number(header.computed_checksum.into()),
    );
    res.insert("is_valid".into(), serde_json::Value::Bool(header.is_valid));
    res.insert(
        "compressed_len".into(),
        serde_json::Value::Number(header.compressed_len.into()),
    );
    if let Some(notice) = header.notice_text {
        res.insert("notice".into(), serde_json::Value::String(notice));
    }

    let part_items: Vec<serde_json::Value> = parts
        .iter()
        .map(|p| {
            let type_str = match p.part_type {
                PatchPartType::Create => "create",
                PatchPartType::Modify => "modify",
                PatchPartType::Delete => "delete",
            };
            serde_json::json!({
                "file_name": p.file_name,
                "type": type_str,
                "old_checksum": p.old_checksum,
                "new_checksum": p.new_checksum,
                "new_file_len": p.new_file_len,
            })
        })
        .collect();
    res.insert("parts".into(), serde_json::Value::Array(part_items));
    doc.insert("result".into(), serde_json::Value::Object(res));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}
pub fn cmd_oracle_mapscene(
    wz_path: &std::path::Path,
    map_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::text::sort_natural;
    use wz_core::WzArchive;
    use wz_db::MapData;

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let detection = archive.detection();

    let clean = map_id.trim_end_matches(".img");
    let padded = if let Ok(num) = clean.parse::<u32>() {
        format!("{num:09}.img")
    } else {
        format!("{clean}.img")
    };
    let raw_name = format!("{clean}.img");

    let entry = find_image_entry_in_dirtree(&tree, &padded)
        .or_else(|| find_image_entry_in_dirtree(&tree, &raw_name))
        .or_else(|| {
            // Map0/000010000.img 형식 등 서브트리 경로 매칭 시도
            if let Some(pos) = clean.rfind('/') {
                let sub = &clean[pos + 1..];
                find_image_entry_in_dirtree(&tree, &format!("{sub}.img"))
            } else {
                None
            }
        })
        .ok_or_else(|| format!("맵 이미지를 찾을 수 없습니다: {map_id}"))?;

    let off = entry.offset.ok_or("맵 이미지 오프셋이 없습니다")?;
    let img_root = archive.parse_image_at(off)?;
    let map_data = MapData::parse_from_node(&img_root);

    let mut doc = oracle_envelope("mapscene", detection);

    let mut res = serde_json::Map::new();

    // info
    let mut info_map = serde_json::Map::new();
    if let Some(id) = map_data.info.map_id {
        info_map.insert("map_id".into(), serde_json::Value::Number(id.into()));
    }
    if let Some(bgm) = &map_data.info.bgm {
        info_map.insert("bgm".into(), serde_json::Value::String(bgm.clone()));
    }
    if let Some(mark) = &map_data.info.map_mark {
        info_map.insert("map_mark".into(), serde_json::Value::String(mark.clone()));
    }
    if let Some(ret) = map_data.info.return_map {
        info_map.insert("return_map".into(), serde_json::Value::Number(ret.into()));
    }
    if let Some(f_ret) = map_data.info.forced_return {
        info_map.insert(
            "forced_return".into(),
            serde_json::Value::Number(f_ret.into()),
        );
    }
    info_map.insert("town".into(), serde_json::Value::Bool(map_data.info.town));
    info_map.insert("swim".into(), serde_json::Value::Bool(map_data.info.swim));
    info_map.insert("fly".into(), serde_json::Value::Bool(map_data.info.fly));
    if let Some(vr) = map_data.info.vrect {
        let mut vr_map = serde_json::Map::new();
        vr_map.insert("left".into(), serde_json::Value::Number(vr.left.into()));
        vr_map.insert("top".into(), serde_json::Value::Number(vr.top.into()));
        vr_map.insert("right".into(), serde_json::Value::Number(vr.right.into()));
        vr_map.insert("bottom".into(), serde_json::Value::Number(vr.bottom.into()));
        info_map.insert("vrect".into(), serde_json::Value::Object(vr_map));
    }
    res.insert("info".into(), serde_json::Value::Object(info_map));

    // footholds (ID 오름차순 정렬)
    let mut sorted_fhs = map_data.footholds.clone();
    sorted_fhs.sort_by_key(|f| f.id);
    let fh_items: Vec<serde_json::Value> = sorted_fhs
        .iter()
        .map(|f| {
            serde_json::json!({
                "id": f.id,
                "layer": f.layer,
                "group": f.group,
                "x1": f.x1,
                "y1": f.y1,
                "x2": f.x2,
                "y2": f.y2,
                "prev": f.prev,
                "next": f.next,
                "piece": f.piece,
                "is_wall": f.is_wall(),
                "is_flat": f.is_flat(),
            })
        })
        .collect();
    res.insert("footholds".into(), serde_json::Value::Array(fh_items));

    // portals (포탈 이름 natural_cmp 및 ID 오름차순)
    let mut sorted_portals = map_data.portals.clone();
    sorted_portals.sort_by(|a, b| {
        match wz_core::text::natural_cmp(&a.p_name, &b.p_name) {
            std::cmp::Ordering::Equal => a.id.cmp(&b.id),
            other => other,
        }
    });
    let portal_items: Vec<serde_json::Value> = sorted_portals
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "pn": p.p_name,
                "pt": p.p_type,
                "x": p.x,
                "y": p.y,
                "tm": p.to_map,
                "tn": p.to_name,
                "script": p.script,
            })
        })
        .collect();
    res.insert("portals".into(), serde_json::Value::Array(portal_items));

    // life (ID natural_cmp 정렬)
    let mut sorted_life = map_data.life.clone();
    sort_natural(&mut sorted_life, |l| &l.id);
    let life_items: Vec<serde_json::Value> = sorted_life
        .iter()
        .map(|l| {
            serde_json::json!({
                "id": l.id,
                "type": l.life_type,
                "x": l.x,
                "y": l.y,
                "cy": l.cy,
                "fh": l.fh,
                "flip": l.flip,
                "hide": l.hide,
            })
        })
        .collect();
    res.insert("life".into(), serde_json::Value::Array(life_items));

    doc.insert("result".into(), serde_json::Value::Object(res));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}
pub fn children_of(node: &wz_core::image::Node) -> &[wz_core::image::Node] {
    if !node.children.is_empty() {
        return &node.children;
    }
    match &node.value {
        Some(wz_core::image::ImageValue::Property(inner)) => &inner.children,
        _ => &[],
    }
}
pub struct ResolvedCanvas {
    w: u32,
    h: u32,
    sha256: String,
    /// avatar 합성용 RGBA8 — timeline 은 해시만 쓴다.
    rgba: Vec<u8>,
}
pub fn split_outlink(raw: &str) -> Option<(&str, Vec<&str>)> {
    let segs: Vec<&str> = raw.split('/').filter(|s| !s.is_empty()).collect();
    let img_idx = segs
        .iter()
        .rposition(|s| s.to_ascii_lowercase().ends_with(".img"))?;
    Some((segs[img_idx], segs[img_idx + 1..].to_vec()))
}
pub fn decode_canvas_pixels(
    png: &wz_core::image::PngRef,
    src: &std::sync::Arc<dyn wz_pool::ByteSource>,
) -> Result<ResolvedCanvas, Box<dyn std::error::Error>> {
    use wz_image::TextureFormat;

    let raw_payload = png.extract_raw_bytes(src)?;
    let fmt = TextureFormat::from_u32(png.format as u32);
    let rgba = match fmt {
        TextureFormat::Unknown => {
            // 미해석 포맷: zlib 해제만 시도, 실패 시 검정 픽셀로 진행한다.
            // (크래시로 검증을 중단하는 것보다 "미해석" 표시가 낫다.)
            match wz_image::decompress_zlib(&raw_payload) {
                Ok(v) => v,
                Err(_) => vec![0u8; (png.width.max(1) * png.height.max(1) * 4) as usize],
            }
        }
        _ => wz_image::decode_raw_page_to_rgba8(
            fmt,
            png.width as u32,
            png.height as u32,
            png.scale as u32,
            png.pages as u32,
            0,
            &raw_payload,
        )?
        .2,
    };
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&rgba);
    Ok(ResolvedCanvas {
        w: png.width as u32,
        h: png.height as u32,
        sha256: format!("{:x}", hasher.finalize()),
        rgba,
    })
}
pub fn resolve_outlink(
    canvas_archive: &wz_core::WzArchive,
    outlink: &str,
) -> Result<ResolvedCanvas, Box<dyn std::error::Error>> {
    let (img_name, sub) = split_outlink(outlink)
        .ok_or(format!("outlink 안에 .img 세그먼트가 없다: {outlink}"))?;
    let tree = canvas_archive.read_tree()?;
    let entry = find_image_entry_in_dirtree(&tree, img_name)
        .ok_or(format!("_Canvas WZ 에 이미지가 없다: {outlink}"))?;
    let off = entry.offset.ok_or("outlink 이미지 오프셋 미해결")?;
    let root = canvas_archive.parse_image_at(off)?;
    let target = find_sub_node(&root, &sub)?;
    let png = extract_png_ref(target)
        .ok_or(format!("outlink 경로가 Canvas 가 아니다: {outlink}"))?;
    decode_canvas_pixels(&png, canvas_archive.source())
}
pub struct FrameLayer {
    name: String,
    canvas: ResolvedCanvas,
    origin: wz_avatar::Point,
    /// z 문자열 (본체 더미의 `z` 가 문자열이었다)
    z: Option<String>,
    /// `z` 가 int 로 표현된 경우 (C# `skin.ZIndex`)
    z_int: Option<i32>,
    outlink: Option<String>,
}
pub fn collect_frame_layers(
    frame_node: &wz_core::image::Node,
    archive: &wz_core::WzArchive,
    canvas_archive: Option<&wz_core::WzArchive>,
) -> Result<(Vec<FrameLayer>, Option<wz_avatar::Point>), Box<dyn std::error::Error>> {
    let src = archive.source();
    let mut layers: Vec<FrameLayer> = Vec::new();

    for child in children_of(frame_node) {
        let Some(wz_core::image::ImageValue::Png(png)) = &child.value else {
            continue;
        };
        let sub = png.sub_property.as_ref();
        let mut origin = wz_avatar::Point::new(0, 0);
        let mut z: Option<String> = None;
        let mut z_int: Option<i32> = None;
        if let Some(sub) = sub {
            for ch in &sub.children {
                match (ch.name.as_ref(), &ch.value) {
                    ("origin", Some(wz_core::image::ImageValue::Vector(v))) => {
                        origin = wz_avatar::Point::new(v.x, v.y)
                    }
                    ("z", Some(wz_core::image::ImageValue::Str(s))) => {
                        z = Some(s.to_string())
                    }
                    ("z", Some(wz_core::image::ImageValue::Int(i))) => z_int = Some(*i),
                    _ => {}
                }
            }
        }
        let outlink = sub.and_then(|s| {
            s.children.iter().find_map(|c| match &c.value {
                Some(wz_core::image::ImageValue::Str(v))
                    if c.name.as_ref() == "_outlink" => Some(v.to_string()),
                _ => None,
            })
        });

        let canvas = match (&outlink, canvas_archive) {
            (Some(link), Some(c_archive)) => resolve_outlink(c_archive, link)?,
            (Some(_), None) => {
                return Err("outlink 캔버스가 있지만 canvas-wz 가 제공되지 않았다".into());
            }
            (None, _) => decode_canvas_pixels(png, src)?,
        };
        layers.push(FrameLayer {
            name: child.name.to_string(),
            canvas,
            origin,
            z,
            z_int,
            outlink,
        });
    }

    // 프레임 수준 `move` — 원본 C# `ActionFrame.Move`
    let move_vec = children_of(frame_node).iter().find_map(|ch| {
        match (ch.name.as_ref(), &ch.value) {
            ("move", Some(wz_core::image::ImageValue::Vector(v))) => {
                Some(wz_avatar::Point::new(v.x, v.y))
            }
            _ => None,
        }
    });

    Ok((layers, move_vec))
}
pub fn frame_timing_from(frame_node: &wz_core::image::Node) -> wz_avatar::FrameTiming {
    let mut delay = None;
    let mut a0 = None;
    let mut a1 = None;
    for ch in children_of(frame_node) {
        let iv = match &ch.value {
            Some(wz_core::image::ImageValue::Int(i)) => Some(*i),
            _ => None,
        };
        match (ch.name.as_ref(), iv) {
            ("delay", Some(i)) => delay = Some(i),
            ("a0", Some(i)) => a0 = Some(i),
            ("a1", Some(i)) => a1 = Some(i),
            _ => {}
        }
    }
    wz_avatar::FrameTiming::from_raw(delay, a0, a1)
}
pub fn avatar_z_priority(zmap_len: usize, zmap: &wz_avatar::ZMap, layer: &FrameLayer) -> i32 {
    let count = zmap_len as i32;
    if let Some(z) = &layer.z {
        zmap.index_of(z).map(|i| i.min(i32::MAX as usize) as i32).unwrap_or(count)
    } else if let Some(zi) = layer.z_int {
        if zi < 0 {
            count - zi
        } else {
            -1 - zi
        }
    } else {
        count
    }
}
pub fn scalar_value_json(
    name: &str,
    value: &wz_core::image::ImageValue,
) -> serde_json::Value {
    use wz_core::image::ImageValue as V;

    let mut m = serde_json::Map::new();
    m.insert("name".into(), serde_json::Value::String(name.to_string()));
    m.insert(
        "type".into(),
        serde_json::Value::String(value.type_name().into()),
    );
    match value {
        V::Str(s) => m.insert("value".into(), serde_json::Value::String(s.to_string())),
        V::Short(v) => m.insert("value".into(), serde_json::Value::Number((*v).into())),
        V::Int(v) => m.insert("value".into(), serde_json::Value::Number((*v).into())),
        V::Long(v) => m.insert("value".into(), serde_json::Value::Number((*v).into())),
        V::Uint(v) => {
            m.insert("value".into(), serde_json::Value::Number((*v as u64).into()))
        }
        V::Float(v) => m.insert(
            "value".into(),
            serde_json::Number::from_f64(*v as f64)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
        ),
        V::Double(v) => m.insert(
            "value".into(),
            serde_json::Number::from_f64(*v)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
        ),
        V::Bool(b) => m.insert("value".into(), serde_json::Value::Bool(*b)),
        V::Null => m.insert("value".into(), serde_json::Value::Null),
        V::Vector(v) => m.insert(
            "value".into(),
            serde_json::Value::Array(vec![
                serde_json::Value::Number(v.x.into()),
                serde_json::Value::Number(v.y.into()),
            ]),
        ),
        _ => None,
    };
    serde_json::Value::Object(m)
}
pub fn cmd_oracle_timeline(
    wz_path: &std::path::Path,
    img_name: Option<&str>,
    node_path: &str,
    canvas_wz_path: Option<&std::path::Path>,
    interpol_ms: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::WzArchive;

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let entry = match img_name {
        Some(name) => find_image_entry_in_dirtree(&tree, name)
            .ok_or(format!("디렉터리 트리에서 이미지를 찾을 수 없습니다: {name}"))?,
        None => tree
            .root
            .entries
            .iter()
            .chain(tree.blocks.iter().flat_map(|b| &b.entries))
            .find(|e| e.node_type == wz_core::dirtree::node_type::IMAGE)
            .cloned()
            .ok_or("WZ 아카이브에 이미지가 없습니다")?,
    };
    let off = entry.offset.ok_or("이미지 오프셋이 유효하지 않습니다")?;
    let img_root = archive.parse_image_at(off)?;

    let parts: Vec<&str> = node_path.split('/').filter(|s| !s.is_empty()).collect();
    let action_node = find_sub_node(&img_root, &parts)?;

    let canvas_archive = match canvas_wz_path {
        Some(p) => Some(WzArchive::open(p)?),
        None => None,
    };

    let timings: Vec<wz_avatar::FrameTiming> =
        children_of(action_node).iter().map(frame_timing_from).collect();
    let cycle_ms: u64 = timings.iter().map(|t| t.delay_ms as u64).sum();

    let mut frames = Vec::new();
    let mut samples = Vec::new();
    for (idx, (frame_node, timing)) in
        children_of(action_node).iter().zip(timings.iter()).enumerate()
    {
        let (layers, _move) =
            collect_frame_layers(frame_node, &archive, canvas_archive.as_ref())?;

        let mut children = Vec::new();
        for ch in children_of(frame_node) {
            match &ch.value {
                Some(wz_core::image::ImageValue::Png(_)) => {
                    let layer = layers
                        .iter()
                        .find(|l| l.name == ch.name.as_ref())
                        .ok_or("프레임 자식과 레이어가 맞지 않는다")?;
                    children.push(serde_json::json!({
                        "name": ch.name.as_ref(),
                        "type": "canvas",
                        "outlink": layer.outlink,
                        "w": layer.canvas.w,
                        "h": layer.canvas.h,
                        "sha256": layer.canvas.sha256,
                    }));
                }
                Some(v) => children.push(scalar_value_json(ch.name.as_ref(), v)),
                None => children.push(serde_json::json!({
                    "name": ch.name.as_ref(),
                    "type": "property",
                })),
            }
        }

        let mut frame_json = serde_json::Map::new();
        frame_json.insert("frame".into(), serde_json::Value::Number(idx.into()));
        frame_json.insert(
            "delay_ms".into(),
            serde_json::Value::Number(timing.delay_ms.into()),
        );
        frame_json.insert("a0".into(), serde_json::Value::Number(timing.a0.into()));
        frame_json.insert("a1".into(), serde_json::Value::Number(timing.a1.into()));
        frame_json.insert("children".into(), serde_json::Value::Array(children));
        frames.push(serde_json::Value::Object(frame_json));

        if interpol_ms > 0 {
            for elapsed in (0..=timing.delay_ms).step_by(interpol_ms.max(1) as usize) {
                samples.push(serde_json::json!({
                    "frame": idx,
                    "elapsed_ms": elapsed,
                    "alpha": timing.alpha_at(elapsed),
                }));
            }
        }
    }

    let mut doc = oracle_envelope("timeline", archive.detection());
    let mut res = serde_json::Map::new();
    res.insert(
        "node".into(),
        serde_json::Value::String(node_path.to_string()),
    );
    res.insert(
        "frame_count".into(),
        serde_json::Value::Number(children_of(action_node).len().into()),
    );
    res.insert("cycle_ms".into(), serde_json::Value::Number(cycle_ms.into()));
    res.insert("frames".into(), serde_json::Value::Array(frames));
    if interpol_ms > 0 {
        res.insert("samples".into(), serde_json::Value::Array(samples));
    }
    doc.insert("result".into(), serde_json::Value::Object(res));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}
pub struct AvatarParams<'a> {
    body_wz: &'a std::path::Path,
    body_img: Option<&'a str>,
    canvas_wz: &'a std::path::Path,
    zmap_wz: &'a std::path::Path,
    zmap_img: Option<&'a str>,
    action: &'a str,
    frame: u32,
    elapsed: u32,
}
pub fn cmd_oracle_avatar(params: &AvatarParams) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::WzArchive;

    let AvatarParams {
        body_wz,
        body_img,
        canvas_wz: canvas_wz_path,
        zmap_wz,
        zmap_img,
        action,
        frame: frame_idx,
        elapsed,
    } = *params;
    let body_archive = WzArchive::open(body_wz)?;
    let canvas_archive = WzArchive::open(canvas_wz_path)?;
    let zmap_archive = WzArchive::open(zmap_wz)?;

    let tree = zmap_archive.read_tree()?;
    let zname = zmap_img.unwrap_or("zmap.img");
    let zentry = find_image_entry_in_dirtree(&tree, zname)
        .ok_or(format!("zmap WZ 에서 이미지를 찾지 못했다: {zname}"))?;
    let zoff = zentry.offset.ok_or("zmap 이미지 오프셋 미해결")?;
    let zmap_root = zmap_archive.parse_image_at(zoff)?;
    let zmap_order: Vec<String> = children_of(&zmap_root)
        .iter()
        .map(|c| match &c.value {
            Some(wz_core::image::ImageValue::Str(s)) => s.to_string(),
            _ => c.name.to_string(),
        })
        .collect();
    let zmap_len = zmap_order.len();
    let zmap = wz_avatar::ZMap::new(zmap_order);

    let body_tree = body_archive.read_tree()?;
    let body_entry = match body_img {
        Some(name) => find_image_entry_in_dirtree(&body_tree, name)
            .ok_or(format!("본체 WZ 에 이미지가 없다: {name}"))?,
        None => body_tree
            .root
            .entries
            .iter()
            .chain(body_tree.blocks.iter().flat_map(|b| &b.entries))
            .find(|e| e.node_type == wz_core::dirtree::node_type::IMAGE)
            .cloned()
            .ok_or("본체 WZ 에 이미지가 없다")?,
    };
    let body_off = body_entry.offset.ok_or("본체 이미지 오프셋 미해결")?;
    let body_root = body_archive.parse_image_at(body_off)?;

    let parts: Vec<&str> = action.split('/').filter(|s| !s.is_empty()).collect();
    let action_node = find_sub_node(&body_root, &parts)?;
    let action_frames = children_of(action_node);
    if action_frames.is_empty() {
        return Err(format!("액션 노드에 프레임이 없다: {action}").into());
    }
    let frame_node = action_frames
        .get(frame_idx as usize)
        .ok_or(format!(
            "프레임 {frame_idx} 없다 (전체 {})",
            action_frames.len()
        ))?;

    let timing = frame_timing_from(frame_node);
    let alpha = timing.alpha_at(elapsed.min(timing.delay_ms));
    let (mut layers, move_vec) =
        collect_frame_layers(frame_node, &body_archive, Some(&canvas_archive))?;

    // 렌더 순서: z 규칙 오름차순 (stable — 같은 우선순위는 트리 순서 유지)
    layers.sort_by_key(|l| avatar_z_priority(zmap_len, &zmap, l));

    let sources: Vec<wz_avatar::LayerSource<'_>> = layers
        .iter()
        .map(|l| {
            let mut pos = wz_avatar::Point::new(-l.origin.x, -l.origin.y);
            if let Some(mv) = move_vec {
                pos = wz_avatar::Point::new(pos.x + mv.x, pos.y + mv.y);
            }
            wz_avatar::LayerSource {
                rgba: &l.canvas.rgba,
                width: l.canvas.w,
                height: l.canvas.h,
                pos,
                alpha,
            }
        })
        .collect();
    let composed = wz_avatar::compose(&sources)?;

    let mut doc = oracle_envelope("avatar", body_archive.detection());
    let mut res = serde_json::Map::new();
    res.insert(
        "image".into(),
        serde_json::json!({ "w": composed.width, "h": composed.height }),
    );
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&composed.rgba);
    res.insert(
        "sha256".into(),
        serde_json::Value::String(format!("{:x}", hasher.finalize())),
    );
    let layer_json: Vec<serde_json::Value> = layers
        .iter()
        .map(|l| {
            let z_json = match (&l.z, &l.z_int) {
                (Some(z), _) => serde_json::Value::String(z.clone()),
                (None, Some(zi)) => serde_json::Value::Number((*zi).into()),
                (None, None) => serde_json::Value::Null,
            };
            serde_json::json!({
                "name": l.name,
                "z": z_json,
                "z_index": avatar_z_priority(zmap_len, &zmap, l),
                "outlink": l.outlink,
                "w": l.canvas.w,
                "h": l.canvas.h,
                "origin": [l.origin.x, l.origin.y],
                "sha256": l.canvas.sha256,
            })
        })
        .collect();
    res.insert("layers".into(), serde_json::Value::Array(layer_json));
    res.insert(
        "frames".into(),
        serde_json::json!({
            "delay_ms": timing.delay_ms,
            "a0": timing.a0,
            "a1": timing.a1,
        }),
    );
    doc.insert("result".into(), serde_json::Value::Object(res));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}

pub fn oracle_envelope(
    op: &str,
    detection: &wz_core::detect_version::Detection,
) -> serde_json::Map<String, serde_json::Value> {
    let hash1_str = detection
        .detected
        .hash1
        .map(|h| format!("{:#010x}", h))
        .unwrap_or_else(|| "0x00000000".into());
    let hash2_str = detection
        .detected
        .hash2
        .map(|h| format!("{:#010x}", h))
        .unwrap_or_else(|| "0x00000000".into());

    let mut doc = serde_json::Map::new();
    doc.insert("schema_version".into(), serde_json::Value::Number(1.into()));
    doc.insert("op".into(), serde_json::Value::String(op.into()));
    doc.insert(
        "profile".into(),
        serde_json::Value::String(detection.detected.profile.to_string()),
    );
    doc.insert("hash1".into(), serde_json::Value::String(hash1_str));
    doc.insert("hash2".into(), serde_json::Value::String(hash2_str));
    doc
}
