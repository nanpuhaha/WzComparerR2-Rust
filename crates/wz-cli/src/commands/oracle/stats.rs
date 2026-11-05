use super::*;
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
        let val = props
            .get(prop_key)
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let mut single_res = serde_json::Map::new();
        single_res.insert(
            "prop".into(),
            serde_json::Value::String(prop_key.to_string()),
        );
        single_res.insert("value".into(), val);
        doc.insert("result".into(), serde_json::Value::Object(single_res));
    } else {
        doc.insert("result".into(), serde_json::Value::Object(props));
    }

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}
