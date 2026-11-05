//! C# `ItemStringHelper.GetItemCoreSpecString` 실행 결과와 비교한다.

use serde::Deserialize;

#[derive(Deserialize)]
struct Row {
    kind: String,
    value: i32,
    desc: Option<String>,
    text: Option<String>,
}

#[test]
fn core_specs_match_csharp() {
    let rows: Vec<Row> = serde_json::from_str(include_str!("core_specs.json")).unwrap();
    for row in rows {
        let got = wz_db::item_core_spec_string(&row.kind, row.value, row.desc.as_deref());
        assert_eq!(
            got, row.text,
            "kind={} value={} desc={:?}",
            row.kind, row.value, row.desc
        );
    }
}
