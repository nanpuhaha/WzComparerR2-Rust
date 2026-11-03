use super::*;
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

