//! `PluginContext.AddRibbonBar` 배치 모델을 C# 참조 구현과 비교한다.
//!
//! DevComponents RibbonBar는 macOS에서 만들 수 없어 C# 하네스가 순서/좌표
//! 규칙을 옮겼고, `PluginContext` 자체는 실제로 생성해 사용 가능함을 기록한다.

use serde::Deserialize;
use wz_db::plugin_ribbon::{RibbonBar, RibbonModel, RibbonPanel};

#[derive(Deserialize)]
struct Fixture {
    panels: Vec<PanelIn>,
    calls: Vec<Call>,
}

#[derive(Deserialize)]
struct PanelIn {
    tab: String,
    bars: Vec<BarIn>,
}

#[derive(Deserialize)]
struct BarIn {
    text: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Deserialize)]
struct Call {
    tab: String,
    text: String,
    width: i32,
}

#[derive(Deserialize)]
struct Expected {
    context_created: bool,
    results: Vec<ResultOut>,
    panels: Vec<PanelOut>,
}

#[derive(Deserialize)]
struct ResultOut {
    tab: String,
    text: String,
    error: Option<String>,
    x: Option<i32>,
    y: Option<i32>,
    width: Option<i32>,
    height: Option<i32>,
}

#[derive(Deserialize)]
struct PanelOut {
    tab: String,
    bars: Vec<BarOut>,
}

#[derive(Deserialize)]
struct BarOut {
    text: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[test]
fn ribbon_placement_matches_csharp() {
    let input: Fixture =
        serde_json::from_str(include_str!("fixtures/plugin_ribbon_input.json")).unwrap();
    let expected: Expected =
        serde_json::from_str(include_str!("fixtures/plugin_ribbon.json")).unwrap();
    assert!(expected.context_created, "PluginContext must exist");

    let mut model = RibbonModel::new(
        input
            .panels
            .iter()
            .map(|p| RibbonPanel {
                tab_name: p.tab.clone(),
                bars: p
                    .bars
                    .iter()
                    .map(|b| RibbonBar {
                        text: b.text.clone(),
                        x: b.x,
                        y: b.y,
                        width: b.width,
                        height: b.height,
                    })
                    .collect(),
            })
            .collect(),
    );

    for (call, want) in input.calls.iter().zip(expected.results.iter()) {
        assert_eq!(call.tab, want.tab);
        assert_eq!(call.text, want.text);
        match model.add_bar(&call.tab, &call.text, call.width) {
            Ok(bar) => {
                assert!(want.error.is_none(), "unexpected error for {}", call.text);
                assert_eq!(
                    (bar.x, bar.y, bar.width, bar.height, Some(bar.text.as_str())),
                    (
                        want.x.unwrap(),
                        want.y.unwrap(),
                        want.width.unwrap(),
                        want.height.unwrap(),
                        Some(want.text.as_str())
                    ),
                    "bar placement for {}",
                    call.text
                );
            }
            Err(err) => {
                assert_eq!(
                    err.to_string(),
                    want.error.clone().expect("expected error"),
                    "error for {}",
                    call.text
                );
            }
        }
    }

    assert_eq!(model.panels.len(), expected.panels.len());
    for (panel, want) in model.panels.iter().zip(expected.panels.iter()) {
        assert_eq!(panel.tab_name, want.tab);
        let got: Vec<_> = panel
            .bars
            .iter()
            .map(|b| (b.text.as_str(), b.x, b.y, b.width, b.height))
            .collect();
        let expected_bars: Vec<_> = want
            .bars
            .iter()
            .map(|b| (b.text.as_str(), b.x, b.y, b.width, b.height))
            .collect();
        assert_eq!(got, expected_bars, "panel {}", panel.tab_name);
    }
}
