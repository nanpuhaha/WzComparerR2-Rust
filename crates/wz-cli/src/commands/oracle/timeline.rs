use super::*;
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
        action_node.object_children().iter().map(frame_timing_from).collect();
    let cycle_ms: u64 = timings.iter().map(|t| t.delay_ms as u64).sum();

    let mut frames = Vec::new();
    let mut samples = Vec::new();
    for (idx, (frame_node, timing)) in
        action_node.object_children().iter().zip(timings.iter()).enumerate()
    {
        let (layers, _move) =
            wz_avatar::collect_frame_layers(frame_node, &archive, canvas_archive.as_ref())?;

        let mut children = Vec::new();
        for ch in frame_node.object_children() {
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
        serde_json::Value::Number(action_node.object_children().len().into()),
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

pub fn frame_timing_from(frame_node: &wz_core::image::Node) -> wz_avatar::FrameTiming {
    let mut delay = None;
    let mut a0 = None;
    let mut a1 = None;
    for ch in frame_node.object_children() {
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

