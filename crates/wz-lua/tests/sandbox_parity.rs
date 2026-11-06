//! `LuaSandbox.InitLuaEnv`의 `package.path`와 스크립트 동작을 검증한다.

use serde::Deserialize;
use std::path::Path;
use wz_lua::{package_path, package_path_entries, Sandbox};

#[derive(Deserialize, Debug)]
struct Case {
    script_base_dir: String,
    plugin_base_dir: String,
    wc_r2_dir: String,
    cwd: String,
    entries: Vec<String>,
    joined: String,
}

fn opt(text: &str) -> Option<&Path> {
    if text.is_empty() {
        None
    } else {
        Some(Path::new(text))
    }
}

#[test]
fn package_path_matches_csharp() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/lua_package_path.json")).unwrap();
    for case in cases {
        let args = (
            opt(&case.script_base_dir),
            opt(&case.plugin_base_dir),
            opt(&case.wc_r2_dir),
            Path::new(&case.cwd),
        );
        assert_eq!(
            package_path_entries(args.0, args.1, args.2, args.3),
            case.entries,
            "entries for {:?}",
            case.cwd
        );
        assert_eq!(
            package_path(args.0, args.1, args.2, args.3),
            case.joined,
            "joined for {:?}",
            case.cwd
        );
    }
}

#[test]
fn require_resolves_from_script_base_dir() {
    let base = std::env::temp_dir().join(format!("wz-lua-scripts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("lua").join("mymod")).unwrap();
    std::fs::write(
        base.join("lua").join("mymod").join("init.lua"),
        "return { tag = 'module-init' }",
    )
    .unwrap();
    std::fs::write(base.join("topmod.lua"), "return { tag = 'top' }").unwrap();

    let mut sandbox = Sandbox::new().unwrap();
    sandbox
        .init(None, Some(&base), None, None, Path::new("."))
        .unwrap();
    let out = sandbox
        .do_string(
            r#"
local a = require('topmod')
local b = require('mymod')
return a.tag .. ',' .. b.tag
"#,
        )
        .unwrap();
    assert_eq!(out, vec!["top,module-init"]);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn chunk_returns_multiple_values() {
    let mut sandbox = Sandbox::new().unwrap();
    sandbox
        .init(None, None, None, None, Path::new("."))
        .unwrap();
    let out = sandbox.do_string("return 1, 'two', true, nil").unwrap();
    assert_eq!(out, vec!["1", "two", "true", "nil"]);
}

#[test]
fn runtime_error_is_reported() {
    let mut sandbox = Sandbox::new().unwrap();
    sandbox
        .init(None, None, None, None, Path::new("."))
        .unwrap();
    let err = sandbox.do_string("error('boom')").unwrap_err();
    assert!(err.to_string().contains("boom"), "{err}");
}

/// 원본 `each()`는 userdata가 아니면 `nil`을 돌려주고, Lua의 generic for는
/// 그 `nil`을 호출하려다 런타임 오류가 난다. CLR이 없는 이 구현도 같은
/// 계약(테이블이 아니면 `nil`)을 지키므로 동일하게 오류가 난다.
#[test]
fn each_returns_nil_for_non_table() {
    let mut sandbox = Sandbox::new().unwrap();
    sandbox
        .init(None, None, None, None, Path::new("."))
        .unwrap();
    // 원본도 non-table 분기에서 `return` 없이 끝나 **0개 값**을 돌려준다.
    let out = sandbox.do_string("return select('#', each(42))").unwrap();
    assert_eq!(out, vec!["0"]);

    let err = sandbox
        .do_string("for i, v in each(42) do end")
        .unwrap_err();
    assert!(err.to_string().contains("nil"), "{err}");
}
