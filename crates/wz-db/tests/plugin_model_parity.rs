//! `PluginContext` 이벤트 구독과 `Invalidate/Update` 병합을 C# 참조 구현과 비교한다.
//!
//! C# 하네스는 **실제 .NET 멀티캐스트 델리게이트**와 `System.Drawing.Rectangle.Union`을
//! 사용한다(WinForms 불필요). `-=`가 마지막 일치 항목을 제거하는 동작까지 확인한다.

use serde::Deserialize;
use wz_db::plugin_events::{EventBus, InvalidRegion, InvalidateState, PluginEvent, Rect};

#[derive(Deserialize)]
struct Fixture {
    subscriptions: Vec<Sub>,
    unsubscriptions: Vec<Sub>,
    emits: Vec<String>,
    ops: Vec<Op>,
}

#[derive(Deserialize)]
struct Sub {
    event: String,
    handler: String,
}

#[derive(Deserialize)]
struct Op {
    op: String,
    #[serde(default)]
    rect: Option<[i32; 4]>,
}

#[derive(Deserialize)]
struct Expected {
    emissions: Vec<Emission>,
    paints: Vec<Paint>,
}

#[derive(Deserialize)]
struct Emission {
    #[serde(rename = "event")]
    event: String,
    handlers: Vec<u64>,
}

#[derive(Deserialize)]
struct Paint {
    whole: bool,
    #[serde(default)]
    x: Option<i32>,
    #[serde(default)]
    y: Option<i32>,
    #[serde(default)]
    w: Option<i32>,
    #[serde(default)]
    h: Option<i32>,
}

fn event(name: &str) -> PluginEvent {
    match name {
        "SelectedNode1Changed" => PluginEvent::SelectedNode1Changed,
        "SelectedNode2Changed" => PluginEvent::SelectedNode2Changed,
        "SelectedNode3Changed" => PluginEvent::SelectedNode3Changed,
        "WzOpened" => PluginEvent::WzOpened,
        "WzClosing" => PluginEvent::WzClosing,
        "MainStyleChanged" => PluginEvent::MainStyleChanged,
        other => panic!("unknown event {other}"),
    }
}

#[test]
fn plugin_events_and_invalidate_match_csharp() {
    let input: Fixture =
        serde_json::from_str(include_str!("fixtures/plugin_model_input.json")).unwrap();
    let expected: Expected =
        serde_json::from_str(include_str!("fixtures/plugin_model.json")).unwrap();

    let mut bus = EventBus::new();
    for sub in &input.subscriptions {
        bus.subscribe(event(&sub.event), sub.handler.parse().unwrap());
    }
    for sub in &input.unsubscriptions {
        bus.unsubscribe(event(&sub.event), sub.handler.parse().unwrap());
    }

    assert_eq!(input.emits.len(), expected.emissions.len());
    for (name, want) in input.emits.iter().zip(expected.emissions.iter()) {
        assert_eq!(name, &want.event);
        assert_eq!(bus.emit(event(name)), want.handlers, "emit {name}");
    }

    let mut state = InvalidateState::new();
    for op in &input.ops {
        match op.op.as_str() {
            "invalidate" => {
                let r = op.rect.expect("rect");
                state.invalidate(Some(Rect {
                    x: r[0],
                    y: r[1],
                    w: r[2],
                    h: r[3],
                }));
            }
            "invalidate_all" => state.invalidate(None),
            "update" => {
                state.update();
            }
            other => panic!("unknown op {other}"),
        }
    }

    assert_eq!(state.paints.len(), expected.paints.len());
    for (got, want) in state.paints.iter().zip(expected.paints.iter()) {
        match (got, want.whole) {
            (InvalidRegion::Whole, true) => {}
            (InvalidRegion::Rect(rect), false) => {
                assert_eq!(
                    (rect.x, rect.y, rect.w, rect.h),
                    (
                        want.x.unwrap(),
                        want.y.unwrap(),
                        want.w.unwrap(),
                        want.h.unwrap()
                    )
                );
            }
            _ => panic!("paint kind mismatch: {got:?} vs whole={}", want.whole),
        }
    }
}
