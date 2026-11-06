//! fixture 어셈블리의 C# `Assembly.GetExportedTypes()` 결과와 비교한다.

use serde::Deserialize;

#[derive(Deserialize, PartialEq, Debug)]
struct Expected {
    name: String,
    version: Option<String>,
    title: Option<String>,
    company: Option<String>,
    informational: Option<String>,
    file_version: Option<String>,
    plugin_types: Vec<String>,
}

#[test]
fn plugin_metadata_matches_csharp() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let dll = base.join("WzComparerR2.FixturePlugin.dll");
    if !dll.exists() {
        return;
    }
    let expected: Expected = serde_json::from_str(
        &std::fs::read_to_string(base.join("plugin_entries.expected.json")).unwrap(),
    )
    .unwrap();
    let assembly = wz_db::read_assembly(&dll).unwrap();
    let plugin_types = wz_db::plugin_types(&assembly);
    let actual = Expected {
        name: assembly.name,
        version: assembly.version,
        title: assembly.title,
        company: assembly.company,
        informational: assembly.informational_version,
        file_version: assembly.file_version,
        plugin_types,
    };
    assert_eq!(actual, expected);
}
