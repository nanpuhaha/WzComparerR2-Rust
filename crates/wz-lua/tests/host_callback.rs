//! NLua 델리게이트 마샬링 계약을 `wz-lua`에서 재현한다.
//!
//! fixture는 원본과 같은 NLua **1.7.3**으로 실제 실행한 결과다.
//! - `GlobalFindNodeFunction`(2-파라미터, 기본값 있음)는 핸들러도 **정확히
//!   2-파라미터**여야 바인딩된다. 원본 1-파라미터 핸들러는
//!   `ArgumentException: Cannot bind to the target method...`로 실패한다.
//! - `out` 파라미터 델리게이트는 try/catch로 잡히지 않고 프로세스가 abort된다.

use serde::Deserialize;
use std::path::Path;
use wz_lua::Sandbox;

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
    out_param: OutParam,
}

#[derive(Deserialize)]
struct Case {
    case: String,
    value: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct OutParam {
    exit_code: i32,
    note: String,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/lua_delegate.json")).unwrap()
}

fn script_for(case: &str) -> Option<&'static str> {
    Some(match case {
        "find_null" => {
            "local n = host_call(function(path) return nil end, 'Mob/2.img')\n\
             if n == nil then return '(null)' end\n\
             return tostring(n)"
        }
        "two_arg" => {
            "return host_call(function(a, b) return a .. ':' .. tostring(b) end, 'x', 7)"
        }
        "two_arg_neg" => {
            "return host_call(function(a, b) return a .. ':' .. tostring(b) end, '', -3)"
        }
        "two_arg_arity" => {
            "return host_call(function(a, b, c) return tostring(a) .. tostring(b) .. tostring(c) end, 'x', 7)"
        }
        _ => return None,
    })
}

#[test]
fn callback_marshalling_matches_nlua() {
    let mut checked = 0;
    for case in fixture().cases {
        // 원본 1-파라미터 핸들러는 NLua 바인딩 단계에서 실패한다.
        if case.case == "find_null_original_handler" {
            let error = case.error.expect("binding error recorded");
            assert!(
                error.contains("Cannot bind to the target method"),
                "unexpected error: {error}"
            );
            continue;
        }
        let Some(script) = script_for(&case.case) else {
            continue;
        };
        assert!(
            case.error.is_none(),
            "{}: {}",
            case.case,
            case.error.unwrap()
        );

        let mut sandbox = Sandbox::new().unwrap();
        sandbox
            .init(None, None, None, None, Path::new("."))
            .unwrap();
        let out = sandbox.do_string(script).unwrap();
        assert_eq!(
            out.first().map(String::as_str),
            case.value.as_deref(),
            "case {}",
            case.case
        );
        checked += 1;
    }
    assert_eq!(checked, 4, "four executable cases");
}

/// Lua 함수를 직접 호출해 첫 반환값만 쓰는지 확인(다중 반환 무시).
#[test]
fn call_first_return_uses_only_first_value() {
    let mut sandbox = Sandbox::new().unwrap();
    sandbox
        .init(None, None, None, None, Path::new("."))
        .unwrap();
    let out = sandbox
        .do_string(
            r#"
local f = function(a, b) return a + b, a * b, 'ignored' end
local first = host_call(f, 3, 4)
return tostring(first), tostring(f(3, 4))
"#,
        )
        .unwrap();
    assert_eq!(out[0], "7");
}

/// out-param 델리게이트는 재현 불가(프로세스 abort)임을 fixture로 고정한다.
#[test]
fn out_param_delegate_is_documented_as_abort() {
    let out = fixture().out_param;
    assert_eq!(out.exit_code, 134, "SIGABRT");
    assert!(out.note.contains("abort"));
}
