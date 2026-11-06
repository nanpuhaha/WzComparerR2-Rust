//! 메인 윈도우 탭/명령 수명주기와 플러그인 OnLoad 순서를 C# 참조 구현과 비교한다.
//!
//! C# 하네스는 실제 `PluginContext`와 fixture 플러그인 어셈블리를 로드해
//! `PluginEntry` 서브클래스 순서를 얻고, 탭/Enabled/OnLoad 규칙을 옮겼다.

use serde::Deserialize;
use wz_db::main_window::{plugin_on_load, CommandRegistry, TabHost};

#[derive(Deserialize)]
struct Input {
    tabs: Vec<String>,
    select: Vec<usize>,
    commands: Vec<CmdIn>,
    selection_events: Vec<bool>,
    dispatch: Vec<String>,
    on_load: Vec<(String, bool)>,
}

#[derive(Deserialize)]
struct CmdIn {
    id: String,
    requires_selection: bool,
}

#[derive(Deserialize)]
struct Expected {
    context_created: bool,
    activation_order: Vec<String>,
    tab_selection: Vec<TabSel>,
    enabled_after: Vec<EnabledAfter>,
    dispatched: Vec<DispatchOut>,
    on_load: OnLoadOut,
}

#[derive(Deserialize)]
struct TabSel {
    index: usize,
    selected: i32,
    name: Option<String>,
}

#[derive(Deserialize)]
struct EnabledAfter {
    has_selection: bool,
    changed: Vec<Change>,
    enabled: Vec<String>,
}

#[derive(Deserialize)]
struct Change {
    id: String,
    enabled: bool,
}

#[derive(Deserialize)]
struct DispatchOut {
    id: String,
    executed: bool,
}

#[derive(Deserialize)]
struct OnLoadOut {
    ran: Vec<String>,
    failures: Vec<FailureOut>,
}

#[derive(Deserialize)]
struct FailureOut {
    name: String,
    error: String,
}

#[derive(Deserialize)]
struct Activated {
    entries: Vec<ActivatedEntry>,
}

#[derive(Deserialize)]
struct ActivatedEntry {
    #[serde(rename = "type")]
    type_name: String,
}

#[test]
fn main_window_lifecycle_matches_csharp() {
    let input: Input =
        serde_json::from_str(include_str!("fixtures/main_window_input.json")).unwrap();
    let expected: Expected =
        serde_json::from_str(include_str!("fixtures/main_window.json")).unwrap();
    assert!(expected.context_created);

    // 실제 활성화 결과(`plugin_entries_activated.json`)와 하네스 순서가 같아야 한다.
    let activated: Activated =
        serde_json::from_str(include_str!("fixtures/plugin_entries_activated.json")).unwrap();
    let activated_names: Vec<String> = activated
        .entries
        .iter()
        .map(|e| e.type_name.rsplit('.').next().unwrap().to_string())
        .collect();
    assert_eq!(activated_names, expected.activation_order);

    // 탭
    let mut host = TabHost::new();
    for tab in &input.tabs {
        host.add_tab(tab);
    }
    assert_eq!(host.selected_name(), Some(input.tabs[0].as_str()));
    for (index, want) in input.select.iter().zip(expected.tab_selection.iter()) {
        assert_eq!(*index, want.index);
        host.select(*index);
        assert_eq!(
            host.selected_index().map(|i| i as i32).unwrap_or(-1),
            want.selected
        );
        assert_eq!(host.selected_name().map(str::to_string), want.name);
    }

    // 명령
    let mut registry = CommandRegistry::new();
    for command in &input.commands {
        registry.register(&command.id, command.requires_selection);
    }
    for (has, want) in input
        .selection_events
        .iter()
        .zip(expected.enabled_after.iter())
    {
        assert_eq!(*has, want.has_selection);
        let changed = registry.on_selection_changed(*has);
        let changed: Vec<(String, bool)> = changed;
        let expect_changed: Vec<(String, bool)> = want
            .changed
            .iter()
            .map(|c| (c.id.clone(), c.enabled))
            .collect();
        assert_eq!(changed, expect_changed, "changed for has={has}");
        assert_eq!(
            registry.enabled_ids(),
            want.enabled,
            "enabled for has={has}"
        );
    }
    for (id, want) in input.dispatch.iter().zip(expected.dispatched.iter()) {
        assert_eq!(id, &want.id);
        assert_eq!(registry.dispatch(id), want.executed, "dispatch {id}");
    }

    // OnLoad: fixture 결과를 그대로 쓰고, 실제 활성화 순서로 호출되는지도 확인.
    let outcomes: Vec<(String, Result<(), String>)> = input
        .on_load
        .iter()
        .map(|(name, ok)| {
            if *ok {
                (name.clone(), Ok(()))
            } else {
                (name.clone(), Err("boom".to_string()))
            }
        })
        .collect();
    let result = plugin_on_load(&outcomes);
    assert_eq!(result.ran, expected.on_load.ran);
    let failures: Vec<(String, String)> = result.failures;
    let want_failures: Vec<(String, String)> = expected
        .on_load
        .failures
        .iter()
        .map(|f| (f.name.clone(), f.error.clone()))
        .collect();
    assert_eq!(failures, want_failures);

    let activation: Vec<(String, Result<(), String>)> = expected
        .activation_order
        .iter()
        .map(|n| (n.clone(), Ok(())))
        .collect();
    assert_eq!(plugin_on_load(&activation).ran, expected.activation_order);
}
