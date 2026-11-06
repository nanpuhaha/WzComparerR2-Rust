//! `DoStringAsync` 취소 계약(디버그 훅 + 취소 플래그)을 검증한다.
//!
//! 원본은 `SetDebugHook(Count, 1)`을 걸고 취소 시 `Lua.State.Error(...)`로
//! 청크를 중단한 뒤 훅을 제거한다. 여기서는 훅 호출 횟수를 세어
//! "N번째 명령에서 중단"을 결정적으로 재현한다.

use serde::Deserialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use wz_lua::Sandbox;

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    chunk: String,
    cancel_after_hooks: u32,
    cancelled: bool,
    #[serde(default)]
    values: Vec<String>,
}

fn sandbox() -> Sandbox {
    let mut sandbox = Sandbox::new().unwrap();
    sandbox
        .init(None, None, None, None, Path::new("."))
        .unwrap();
    sandbox
}

#[test]
fn cancellation_matches_do_string_async_contract() {
    let fixture: Fixture = serde_json::from_str(include_str!("fixtures/lua_cancel.json")).unwrap();
    for case in &fixture.cases {
        let sandbox = sandbox();
        let hooks = std::cell::Cell::new(0u32);
        let limit = case.cancel_after_hooks;
        let result = sandbox.do_string_interruptible(&case.chunk, move || {
            hooks.set(hooks.get() + 1);
            hooks.get() > limit
        });
        match (case.cancelled, result) {
            (true, Err(error)) => {
                assert!(
                    error.to_string().contains("Operation cancelled."),
                    "{}: {error}",
                    case.name
                );
            }
            (false, Ok(values)) => {
                assert_eq!(values, case.values, "{}", case.name);
            }
            (want, got) => panic!("{}: cancelled={want}, got {got:?}", case.name),
        }
    }
}

#[test]
fn flag_api_and_sandbox_reuse_after_cancel() {
    let sandbox = sandbox();
    let cancel = Arc::new(AtomicBool::new(false));
    // 정상 실행.
    assert_eq!(
        sandbox
            .do_string_with_cancel("return 1+2", &cancel)
            .unwrap(),
        vec!["3"]
    );
    // 취소 후에도 샌드박스는 재사용 가능해야 한다(훅 제거 확인).
    let stop = Arc::new(AtomicBool::new(true));
    assert!(sandbox
        .do_string_with_cancel("while true do end", &stop)
        .is_err());
    assert_eq!(
        sandbox
            .do_string_with_cancel("return 'alive'", &cancel)
            .unwrap(),
        vec!["alive"]
    );
    // 전역 상태도 유지된다.
    sandbox.do_string_with_cancel("x = 41", &cancel).unwrap();
    assert_eq!(
        sandbox
            .do_string_with_cancel("return x + 1", &cancel)
            .unwrap(),
        vec!["42"]
    );
    cancel.store(false, Ordering::Relaxed);
}
