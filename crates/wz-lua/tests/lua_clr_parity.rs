//! 원본 `LuaSandbox.InitLuaEnv` 프렐류드를 실제 NLua에서 실행한 결과와 비교한다.
//!
//! C# 하네스는 원문 프렐류드(`luanet.import_type`/`get_method_bysig`)를
//! NLua로 그대로 돌려 CLR `IEnumerable`을 순회한다. Rust는 같은 계약을
//! userdata 컬렉션으로 제공한다.

use serde::Deserialize;
use std::path::Path;
use wz_lua::{Item, Sandbox};

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    kind: String,
    iterator_count: String,
    ok: bool,
    value: Option<String>,
}

fn case_items(name: &str) -> Vec<Item> {
    match name {
        "list_int" => vec![Item::Int(10), Item::Int(20), Item::Int(30)],
        "list_str" => vec![Item::Str("가".into()), Item::Str("나".into())],
        "array_int" => vec![Item::Int(1), Item::Int(2), Item::Int(3)],
        "empty_list" => Vec::new(),
        other => panic!("unknown case {other}"),
    }
}

#[test]
fn nlua_clr_bridge_contract_matches() {
    let fixture: Fixture = serde_json::from_str(include_str!("fixtures/lua_clr.json")).unwrap();
    assert!(fixture.cases.len() >= 6);

    for case in fixture.cases {
        let mut sandbox = Sandbox::new().unwrap();
        sandbox
            .init(None, None, None, None, Path::new("."))
            .unwrap();

        let arg = match case.kind.as_str() {
            "clr" => {
                let handle = sandbox.collection(case_items(&case.name)).unwrap();
                sandbox.set_global(&case.name, handle).unwrap();
                case.name.clone()
            }
            "table" => "{1,2,3}".to_string(),
            _ => "nil".to_string(),
        };

        let count = sandbox
            .do_string(&format!("return select('#', each({arg}))"))
            .unwrap();
        assert_eq!(
            count.first().map(String::as_str),
            Some(case.iterator_count.as_str()),
            "iterator count for {}",
            case.name
        );

        let result = sandbox
            .do_string(&format!(
                r#"
local ok, acc = pcall(function()
  local out = ''
  for i, v in each({arg}) do out = out .. i .. '=' .. tostring(v) .. ';' end
  return out
end)
return tostring(ok) .. '|' .. tostring(acc)
"#
            ))
            .unwrap();
        let joined = result.first().cloned().unwrap_or_default();
        let (ok, value) = joined.split_once('|').unwrap_or((joined.as_str(), ""));
        assert_eq!(
            ok == "true",
            case.ok,
            "ok flag for {} (value {value})",
            case.name
        );
        if case.ok {
            assert_eq!(
                value,
                case.value.as_deref().unwrap_or(""),
                "iterated pairs for {}",
                case.name
            );
        }
    }
}
