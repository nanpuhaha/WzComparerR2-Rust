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
