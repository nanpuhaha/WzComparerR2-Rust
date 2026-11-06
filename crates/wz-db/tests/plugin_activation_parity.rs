//! `PluginManager.LoadPlugin`의 타입 열거 + 인스턴스화 결과를 C#과 비교한다.
//!
//! C# 하네스는 실제 `PluginContext`(provider=null)와
//! `Activator.CreateInstance(type, context)`로 `PluginEntry` 프로퍼티를 읽는다.

use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    context_created: bool,
    context_error: Option<String>,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "type")]
    type_name: String,
    name: Option<String>,
    author: Option<String>,
    version: Option<String>,
    file_version: Option<String>,
    error: Option<String>,
}

#[test]
fn plugin_activation_matches_csharp() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dll = base.join("WzComparerR2.FixturePlugin.dll");
    if !dll.exists() {
        return;
    }
    let fixture: Fixture = serde_json::from_str(
        &std::fs::read_to_string(base.join("plugin_entries_activated.json")).unwrap(),
    )
    .unwrap();
    assert!(fixture.context_created);
    assert!(fixture.context_error.is_none());

    let assembly = wz_db::read_assembly(&dll).unwrap();
    let entries = wz_db::plugin_entries(&assembly);
    assert_eq!(entries.len(), fixture.entries.len());
    for (got, want) in entries.iter().zip(fixture.entries.iter()) {
        assert_eq!(got.type_name, want.type_name);
        assert_eq!(got.name, want.name);
        assert_eq!(got.author, want.author);
        assert_eq!(got.version, want.version);
        assert_eq!(got.file_version, want.file_version);
        assert!(want.error.is_none());
    }
}
