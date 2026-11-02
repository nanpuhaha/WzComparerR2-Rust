


pub fn cmd_image(
    path: &std::path::Path,
    offset: u64,
    hash_version: u64,
    max_depth: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::WzArchive;

    let archive = WzArchive::open_with_hash_version(path, hash_version)?;
    let root = archive.parse_image_at(offset)?;

    println!("파일: {}", archive.source().describe());
    println!(
        "프로파일: {} / KMST{}",
        archive.detection().reason,
        archive.detection().profile.wz_version
    );
    println!("hashVersion: {:#018x}", archive.detection().hash_version);
    println!("이미지 오프셋: {offset}");
    println!();
    print_node(&root, 0, max_depth);
    Ok(())
}
pub fn print_node(node: &wz_core::image::Node, depth: usize, max_depth: usize) {
    if max_depth != 0 && depth > max_depth {
        return;
    }
    let pad = "  ".repeat(depth);
    let ty = node
        .value
        .as_ref()
        .map(|v| v.type_name())
        .unwrap_or("Property");

    // 함정: 중첩 `Property` 는 `children` 이 아니라 `value` 안에 들어간다
    // (`parse_extended` → `ImageValue::Property`). 여기서 재귀하지 않으면
    // 스칼라가 전부 안 보인다 — 검증 단계에서 가장 흔한 착각 원인.
    if let Some(wz_core::image::ImageValue::Property(inner)) = &node.value {
        println!(
            "{pad}{}: Property ({} 자식)",
            node.name,
            inner.children.len()
        );
        for ch in &inner.children {
            print_node(ch, depth + 1, max_depth);
        }
        return;
    }

    let detail = match &node.value {
        Some(wz_core::image::ImageValue::Str(s)) => format!(" = {s:?}"),
        Some(wz_core::image::ImageValue::Short(v)) => format!(" = {v}"),
        Some(wz_core::image::ImageValue::Int(v)) => format!(" = {v}"),
        Some(wz_core::image::ImageValue::Float(v)) => format!(" = {v}"),
        Some(wz_core::image::ImageValue::Double(v)) => format!(" = {v}"),
        Some(wz_core::image::ImageValue::Long(v)) => format!(" = {v}"),
        Some(wz_core::image::ImageValue::Bool(v)) => format!(" = {v}"),
        Some(wz_core::image::ImageValue::Null) | None => String::new(),
        Some(wz_core::image::ImageValue::Png(p)) => {
            format!(
                " = {}x{} fmt={} scale={} pages={} ({} 바이트)",
                p.width, p.height, p.format, p.scale, p.pages, p.data_len
            )
        }
        Some(wz_core::image::ImageValue::Uol(s)) => format!(" = {s}"),
        Some(wz_core::image::ImageValue::Vector(v)) => format!(" = ({}, {})", v.x, v.y),
        _ => String::new(),
    };

    if node.children.is_empty() {
        println!("{pad}{}: {ty}{detail}", node.name);
    } else {
        println!(
            "{pad}{}: {ty}{detail} ({} 자식)",
            node.name,
            node.children.len()
        );
        for ch in &node.children {
            print_node(ch, depth + 1, max_depth);
        }
    }
}
pub fn dump_node_to_json(node: &wz_core::image::Node) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("name".into(), serde_json::Value::String(node.name.to_string()));

    if let Some(wz_core::image::ImageValue::Property(inner)) = &node.value {
        map.insert("type".into(), serde_json::Value::String("Property".into()));
        let children: Vec<serde_json::Value> = inner.children.iter().map(dump_node_to_json).collect();
        map.insert("children".into(), serde_json::Value::Array(children));
    } else {
        if let Some(val) = &node.value {
            map.insert("type".into(), serde_json::Value::String(val.type_name().into()));
            match val {
                wz_core::image::ImageValue::Str(s) => {
                    map.insert("value".into(), serde_json::Value::String(s.to_string()));
                }
                wz_core::image::ImageValue::Short(v) => {
                    map.insert("value".into(), serde_json::Value::Number((*v).into()));
                }
                wz_core::image::ImageValue::Int(v) => {
                    map.insert("value".into(), serde_json::Value::Number((*v).into()));
                }
                wz_core::image::ImageValue::Long(v) => {
                    map.insert("value".into(), serde_json::Value::Number((*v).into()));
                }
                wz_core::image::ImageValue::Float(v) => {
                    if let Some(n) = serde_json::Number::from_f64(*v as f64) {
                        map.insert("value".into(), serde_json::Value::Number(n));
                    }
                }
                wz_core::image::ImageValue::Double(v) => {
                    if let Some(n) = serde_json::Number::from_f64(*v) {
                        map.insert("value".into(), serde_json::Value::Number(n));
                    }
                }
                wz_core::image::ImageValue::Png(p) => {
                    let mut p_map = serde_json::Map::new();
                    p_map.insert("width".into(), serde_json::Value::Number(p.width.into()));
                    p_map.insert("height".into(), serde_json::Value::Number(p.height.into()));
                    p_map.insert("format".into(), serde_json::Value::Number(p.format.into()));
                    p_map.insert("scale".into(), serde_json::Value::Number(p.scale.into()));
                    p_map.insert("pages".into(), serde_json::Value::Number(p.pages.into()));
                    p_map.insert("data_len".into(), serde_json::Value::Number(p.data_len.into()));
                    map.insert("png".into(), serde_json::Value::Object(p_map));
                }
                wz_core::image::ImageValue::Uol(s) => {
                    map.insert("uol".into(), serde_json::Value::String(s.to_string()));
                }
                wz_core::image::ImageValue::Vector(v) => {
                    let mut v_map = serde_json::Map::new();
                    v_map.insert("x".into(), serde_json::Value::Number(v.x.into()));
                    v_map.insert("y".into(), serde_json::Value::Number(v.y.into()));
                    map.insert("vector".into(), serde_json::Value::Object(v_map));
                }
                _ => {}
            }
        }
        if !node.children.is_empty() {
            let children: Vec<serde_json::Value> = node.children.iter().map(dump_node_to_json).collect();
            map.insert("children".into(), serde_json::Value::Array(children));
        }
    }

    serde_json::Value::Object(map)
}
