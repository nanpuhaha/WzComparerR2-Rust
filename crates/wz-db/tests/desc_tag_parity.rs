//! `ReplaceDescTags`의 `#t` 링크 치환을 실제 C# `Regex` 결과와 비교한다.
//!
//! 입력은 실제 `String/Item.img` 문자열 중 `#t`가 있는 40개 + 경계 4개이고,
//! C# 하네스는 실제 `StringLinker`와 .NET `Regex`로 같은 치환을 수행한다.

use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    id: i32,
    field: String,
    text: String,
}

#[derive(Deserialize)]
struct Expected {
    id: i32,
    field: Option<String>,
    replaced: String,
}

#[test]
fn desc_tag_links_match_csharp() {
    let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
        eprintln!("skip: WZ_DATA_ROOT not set");
        return;
    };
    let string_wz = root.join("String/String_000.wz");
    if !string_wz.exists() {
        return;
    }
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/desc_tag_cases.json")).unwrap();
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/desc_tag_expected.json")).unwrap();
    assert_eq!(cases.len(), expected.len());

    let archive = wz_core::WzArchive::open(&string_wz).unwrap();
    let (arena, node) = wz_db::Arena::from_archive(&archive);
    let mut linker = wz_db::StringLinker::new();
    assert!(linker.load_string_wz(&arena, node));

    let name_of = |id: i32| {
        linker
            .string_item
            .get(&id)
            .map(|sr| sr.name.clone())
            .or_else(|| linker.string_eqp.get(&id).map(|sr| sr.name.clone()))
    };

    for (case, want) in cases.iter().zip(expected.iter()) {
        assert_eq!(case.id, want.id);
        assert_eq!(Some(case.field.as_str()), want.field.as_deref());
        let replaced = wz_db::replace_desc_tags(&case.text, &name_of);
        assert_eq!(
            replaced, want.replaced,
            "id {} field {} text {:?}",
            case.id, case.field, case.text
        );
    }
}
