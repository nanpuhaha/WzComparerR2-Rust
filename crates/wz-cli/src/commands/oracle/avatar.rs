use super::*;
pub struct AvatarParams<'a> {
    pub body_wz: &'a std::path::Path,
    pub body_img: Option<&'a str>,
    pub canvas_wz: &'a std::path::Path,
    pub zmap_wz: &'a std::path::Path,
    pub zmap_img: Option<&'a str>,
    pub action: &'a str,
    pub frame: u32,
    pub elapsed: u32,
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
    let zmap_order: Vec<String> = zmap_root.object_children()
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
    let action_frames = action_node.object_children();
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
    let (mut layers, _move_vec) =
        wz_avatar::collect_frame_layers(frame_node, &body_archive, Some(&canvas_archive))?;

    // 렌더 순서: z 규칙 오름차순 (stable — 같은 우선순위는 트리 순서 유지)
    layers.sort_by_key(|l| wz_avatar::avatar_z_priority(zmap_len, &zmap, l));

    let sources: Vec<wz_avatar::LayerSource<'_>> = layers
        .iter()
        .map(|l| wz_avatar::LayerSource {
            rgba: &l.canvas.rgba,
            width: l.canvas.w,
            height: l.canvas.h,
            pos: l.pos,
            alpha,
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
                "z_index": wz_avatar::avatar_z_priority(zmap_len, &zmap, l),
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

