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

