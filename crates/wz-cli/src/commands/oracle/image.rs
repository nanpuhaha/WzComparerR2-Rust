use super::*;
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
    let img_idx = parts
        .iter()
        .position(|p| p.to_lowercase().ends_with(".img"));

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

    let img_offset = img_entry
        .offset
        .ok_or("이미지 오프셋이 유효하지 않습니다")?;
    let img_root = archive.parse_image_at(img_offset)?;

    // 서브 노드 탐색
    let target_node = find_sub_node(&img_root, sub_path_parts)?;

    // 노드에서 PngRef 추출 (현재 노드 또는 첫 번째 Canvas 자식)
    let png_ref = target_node
        .first_canvas()
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
