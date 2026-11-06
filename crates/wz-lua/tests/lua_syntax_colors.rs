//! `Lua.xshd` 색상 이름 → RGB 매핑을 fixture로 고정한다.

use serde::Deserialize;
use std::collections::BTreeMap;
use wz_lua::{color_rgb, HighlightKind};

#[derive(Deserialize)]
struct Fixture {
    colors: BTreeMap<String, String>,
}

fn kind_by_name(name: &str) -> HighlightKind {
    match name {
        "Comment" => HighlightKind::Comment,
        "String" => HighlightKind::String,
        "Number" => HighlightKind::Number,
        "Punctuation" => HighlightKind::Punctuation,
        "Keyword" => HighlightKind::Keyword,
        "AccessKeyword" => HighlightKind::AccessKeyword,
        other => panic!("unknown kind {other}"),
    }
}

#[test]
fn color_mapping_matches_lua_xshd() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/lua_syntax_colors.json")).unwrap();
    for (name, hex) in &fixture.colors {
        let (r, g, b) = color_rgb(kind_by_name(name)).expect("color");
        assert_eq!(format!("{r:02X}{g:02X}{b:02X}"), *hex, "{name}");
    }
    // 색이 없는 종류는 `None`.
    for kind in [
        HighlightKind::Identifier,
        HighlightKind::Whitespace,
        HighlightKind::Other,
    ] {
        assert_eq!(color_rgb(kind), None);
    }
}
