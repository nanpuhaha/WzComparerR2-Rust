//! `LuaConsole.LuaEnvironment`의 출력 모델(`env:Write/WriteLine`)을 검증한다.

use serde::Deserialize;
use std::path::Path;
use wz_lua::{format_dotnet, Sandbox};

#[derive(Deserialize)]
struct Fixture {
    formats: Vec<FormatCase>,
    format_errors: Vec<FormatCase>,
    env_cases: Vec<EnvCase>,
}

#[derive(Deserialize)]
struct FormatCase {
    format: String,
    args: Vec<String>,
    #[serde(default)]
    expected: String,
}

#[derive(Deserialize)]
struct EnvCase {
    script: String,
    output: String,
}

fn sandbox() -> Sandbox {
    let mut sandbox = Sandbox::new().unwrap();
    sandbox
        .init(None, None, None, None, Path::new("."))
        .unwrap();
    sandbox
}

#[test]
fn format_matches_dotnet_string_format() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/lua_env_output.json")).unwrap();
    for case in &fixture.formats {
        assert_eq!(
            format_dotnet(&case.format, &case.args).unwrap(),
            case.expected,
            "format {:?}",
            case.format
        );
    }
    for case in &fixture.format_errors {
        assert!(
            format_dotnet(&case.format, &case.args).is_err(),
            "expected error for {:?}",
            case.format
        );
    }
}

#[test]
fn env_write_and_write_line_accumulate_output() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/lua_env_output.json")).unwrap();
    for case in &fixture.env_cases {
        let sandbox = sandbox();
        sandbox.do_string(&case.script).unwrap();
        assert_eq!(
            sandbox.drain_output(),
            case.output,
            "script {:?}",
            case.script
        );
    }
}

#[test]
fn format_error_surfaces_as_lua_error() {
    let sandbox = sandbox();
    let err = sandbox
        .do_string("env:WriteLine('{1}', 'only-one')")
        .unwrap_err();
    assert!(err.to_string().contains("FormatException"), "{err}");
}

#[test]
fn json_env_fields_are_preserved() {
    let mut sandbox = Sandbox::new().unwrap();
    sandbox
        .init(
            Some(serde_json::json!({"tag": "demo"})),
            None,
            None,
            None,
            Path::new("."),
        )
        .unwrap();
    let out = sandbox.do_string("env:WriteLine(env.tag)").unwrap();
    assert!(out.is_empty());
    assert_eq!(sandbox.drain_output(), "demo\n");
}
