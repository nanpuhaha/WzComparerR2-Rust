use super::*;
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
