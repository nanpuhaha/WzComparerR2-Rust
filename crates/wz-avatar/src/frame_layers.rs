
#[derive(Clone)]
pub struct ResolvedCanvas {
    pub w: u32,
    pub h: u32,
    pub sha256: String,
    /// avatar 합성용 RGBA8 — timeline 은 해시만 쓴다.
    pub rgba: Vec<u8>,
}


#[derive(Clone)]
pub struct FrameLayer {
    pub name: String,
    pub canvas: ResolvedCanvas,
    pub origin: crate::Point,
    pub pos: crate::Point,
    pub z: Option<String>,
    pub z_int: Option<i32>,
    pub outlink: Option<String>,
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
    let png = target.first_canvas()
        .ok_or(format!("outlink 경로가 Canvas 가 아니다: {outlink}"))?;
    decode_canvas_pixels(&png, canvas_archive.source())
}

pub fn collect_frame_layers(
    frame_node: &wz_core::image::Node,
    archive: &wz_core::WzArchive,
    canvas_archive: Option<&wz_core::WzArchive>,
) -> Result<(Vec<FrameLayer>, Option<crate::Point>), Box<dyn std::error::Error>> {
    let src = archive.source();

    #[derive(Clone)]
    struct PartMeta {
        name: String,
        canvas: ResolvedCanvas,
        origin: crate::Point,
        z: Option<String>,
        z_int: Option<i32>,
        outlink: Option<String>,
        maps: Vec<(String, crate::Point)>,
    }

    let mut parts: Vec<PartMeta> = Vec::new();
    for child in frame_node.object_children() {
        let Some(wz_core::image::ImageValue::Png(png)) = &child.value else {
            continue;
        };
        let sub = png.sub_property.as_ref();
        let mut origin = crate::Point::new(0, 0);
        let mut z: Option<String> = None;
        let mut z_int: Option<i32> = None;
        let mut maps: Vec<(String, crate::Point)> = Vec::new();
        if let Some(sub) = sub {
            for ch in &sub.children {
                match (ch.name.as_ref(), &ch.value) {
                    ("origin", Some(wz_core::image::ImageValue::Vector(v))) => {
                        origin = crate::Point::new(v.x, v.y)
                    }
                    ("z", Some(wz_core::image::ImageValue::Str(s))) => {
                        z = Some(s.to_string())
                    }
                    ("z", Some(wz_core::image::ImageValue::Int(i))) => z_int = Some(*i),
                    ("map", Some(wz_core::image::ImageValue::Property(p))) => {
                        for m in &p.children {
                            if let Some(wz_core::image::ImageValue::Vector(v)) = &m.value {
                                maps.push((m.name.to_string(), crate::Point::new(v.x, v.y)));
                            }
                        }
                    }
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
        parts.push(PartMeta {
            name: child.name.to_string(),
            canvas,
            origin,
            z,
            z_int,
            outlink,
            maps,
        });
    }

    #[derive(Clone)]
    struct BoneNode {
        name: String,
        parent: Option<usize>,
        position: crate::Point,
        skins: Vec<usize>,
        skin_offsets: Vec<crate::Point>,
        skin_origins: Vec<crate::Point>,
    }

    let mut nodes: Vec<BoneNode> = Vec::new();
    nodes.push(BoneNode {
        name: "@root".into(),
        parent: None,
        position: crate::Point::ZERO,
        skins: Vec::new(),
        skin_offsets: Vec::new(),
        skin_origins: Vec::new(),
    });

    for (layer_idx, part) in parts.iter().enumerate() {
        if part.maps.is_empty() {
            nodes[0].skins.push(layer_idx);
            nodes[0].skin_offsets.push(crate::Point::ZERO);
            nodes[0].skin_origins.push(part.origin);
            continue;
        }
        let mut parent_idx: Option<usize> = None;
        for (map_name, map_origin) in &part.maps {
            if map_name == "muzzle" {
                continue;
            }
            if parent_idx.is_none() {
                if let Some(existing) = nodes.iter().position(|b| b.name == *map_name) {
                    let joint_name = format!("@{}_{}", map_name, part.name);
                    let idx = nodes.len();
                    nodes.push(BoneNode {
                        name: joint_name,
                        parent: Some(existing),
                        position: crate::Point::new(-map_origin.x, -map_origin.y),
                        skins: vec![layer_idx],
                        skin_offsets: vec![crate::Point::ZERO],
                        skin_origins: vec![part.origin],
                    });
                    parent_idx = Some(idx);
                } else {
                    let idx = nodes.len();
                    nodes.push(BoneNode {
                        name: map_name.clone(),
                        parent: Some(0),
                        position: *map_origin,
                        skins: vec![layer_idx],
                        skin_offsets: vec![crate::Point::new(-map_origin.x, -map_origin.y)],
                        skin_origins: vec![part.origin],
                    });
                    parent_idx = Some(idx);
                }
            } else {
                let bone_idx = if let Some(existing) = nodes.iter().position(|b| b.name == *map_name) {
                    existing
                } else {
                    let i = nodes.len();
                    nodes.push(BoneNode {
                        name: map_name.clone(),
                        parent: parent_idx,
                        position: *map_origin,
                        skins: Vec::new(),
                        skin_offsets: Vec::new(),
                        skin_origins: Vec::new(),
                    });
                    i
                };
                nodes[bone_idx].parent = parent_idx;
                nodes[bone_idx].position = *map_origin;
            }
        }
    }

    let move_vec = frame_node.object_children().iter().find_map(|ch| {
        match (ch.name.as_ref(), &ch.value) {
            ("move", Some(wz_core::image::ImageValue::Vector(v))) => {
                Some(crate::Point::new(v.x, v.y))
            }
            _ => None,
        }
    });

    let mut layer_pos: Vec<crate::Point> = vec![crate::Point::ZERO; parts.len()];

    fn recurse(
        node_idx: usize,
        accum: crate::Point,
        nodes: &[BoneNode],
        layer_pos: &mut [crate::Point],
        move_vec: Option<crate::Point>,
    ) {
        let node = &nodes[node_idx];
        let cur = crate::Point::new(accum.x + node.position.x, accum.y + node.position.y);
        for (i, &li) in node.skins.iter().enumerate() {
            let off = node.skin_offsets[i];
            let origin = node.skin_origins[i];
            let mut pos = crate::Point::new(
                cur.x + off.x - origin.x,
                cur.y + off.y - origin.y,
            );
            if let Some(mv) = move_vec {
                pos.x += mv.x;
                pos.y += mv.y;
            }
            layer_pos[li] = pos;
        }
        for i in 0..nodes.len() {
            if nodes[i].parent == Some(node_idx) {
                recurse(i, cur, nodes, layer_pos, move_vec);
            }
        }
    }
    recurse(0, crate::Point::ZERO, &nodes, &mut layer_pos, move_vec);

    let mut layers: Vec<FrameLayer> = Vec::new();
    for (idx, part) in parts.into_iter().enumerate() {
        layers.push(FrameLayer {
            name: part.name,
            canvas: part.canvas,
            origin: part.origin,
            pos: layer_pos[idx],
            z: part.z,
            z_int: part.z_int,
            outlink: part.outlink,
        });
    }

    Ok((layers, move_vec))
}

pub fn avatar_z_priority(zmap_len: usize, zmap: &crate::ZMap, layer: &FrameLayer) -> i32 {
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
