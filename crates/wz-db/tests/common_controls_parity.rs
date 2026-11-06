//! `Common/Controls` + `Animation` 순수 로직을 C# 참조 구현과 비교한다.
//!
//! 원본 타입은 SharpDX 의존으로 macOS에서 로드되지 않아, 하네스가 원본
//! 메서드 본문을 그대로 옮겨 컴파일한 결과를 fixture로 쓴다.

use serde::Deserialize;
use wz_db::common_controls as cc;

#[derive(Deserialize)]
struct Fixture {
    timelines: Vec<TimelineCase>,
    probes: Vec<ProbeCase>,
    gif_cases: Vec<GifCase>,
    item_time_cases: Vec<ItemTimeCase>,
    max_length_cases: Vec<MaxLengthCase>,
    time_offset_cases: Vec<OffsetCase>,
    reset_cases: Vec<ResetCase>,
    scale_cases: Vec<ScaleCase>,
}

#[derive(Deserialize)]
struct TimelineCase {
    delays: Vec<i32>,
    timeline: Vec<i32>,
}

#[derive(Deserialize)]
struct ProbeCase {
    delays: Vec<i32>,
    t: i32,
    index: i32,
    progress: f32,
}

#[derive(Deserialize)]
struct GifCase {
    len: Vec<i32>,
    ani: Vec<bool>,
    preferred: i32,
    max: Option<i32>,
    timeline: Vec<i32>,
}

#[derive(Deserialize)]
struct Pair {
    start: i32,
    end: i32,
}

#[derive(Deserialize)]
struct ItemTimeCase {
    start: i32,
    length: i32,
    end: i32,
    pair: Pair,
}

#[derive(Deserialize)]
struct MaxLengthCase {
    pairs: Vec<Pair>,
    max: i32,
}

#[derive(Deserialize)]
struct OffsetCase {
    ms: i32,
    last: i32,
    off: i32,
    max_len: i32,
    interval: i32,
    value: i32,
}

#[derive(Deserialize)]
struct ResetCase {
    cur: i32,
    max: i32,
    margin: i32,
    reset: bool,
}

#[derive(Deserialize)]
struct ScaleCase {
    input: f32,
    output: f32,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/anim_timeline.json")).unwrap()
}

#[test]
fn create_timeline_matches_csharp() {
    for case in fixture().timelines {
        assert_eq!(
            cc::create_timeline(&case.delays),
            case.timeline,
            "delays {:?}",
            case.delays
        );
    }
}

#[test]
fn process_from_timeline_matches_csharp() {
    for case in fixture().probes {
        let timeline = cc::create_timeline(&case.delays);
        let (index, progress) = cc::get_process_from_timeline(&timeline, case.t).unwrap();
        assert_eq!(
            index as i32, case.index,
            "delays {:?} t {}",
            case.delays, case.t
        );
        assert!(
            (progress - case.progress).abs() < 1e-6,
            "delays {:?} t {} progress {} vs {}",
            case.delays,
            case.t,
            progress,
            case.progress
        );
    }
}

#[test]
fn gif_timeline_matches_csharp() {
    for case in fixture().gif_cases {
        let frames: Vec<cc::KeyFrame> = case
            .len
            .iter()
            .zip(case.ani.iter())
            .map(|(length, animated)| cc::KeyFrame {
                length: *length,
                animated: *animated,
            })
            .collect();
        assert_eq!(
            cc::gif_timeline(&frames, case.preferred, case.max),
            case.timeline,
            "len {:?} ani {:?} preferred {} max {:?}",
            case.len,
            case.ani,
            case.preferred,
            case.max
        );
    }
}

#[test]
fn item_ranges_match_csharp() {
    let f = fixture();
    for case in f.item_time_cases {
        assert_eq!(
            cc::add_item_times(case.start, case.length, case.end),
            (case.pair.start, case.pair.end),
            "start {} length {} end {}",
            case.start,
            case.length,
            case.end
        );
    }
    for case in f.max_length_cases {
        let pairs: Vec<(i32, i32)> = case.pairs.iter().map(|p| (p.start, p.end)).collect();
        assert_eq!(cc::max_length(&pairs), case.max, "{pairs:?}");
    }
}

#[test]
fn time_offset_and_reset_match_csharp() {
    let f = fixture();
    for case in f.time_offset_cases {
        assert_eq!(
            cc::update_time_offset(case.ms, case.last, case.off, case.max_len, case.interval),
            case.value,
            "ms {} last {} off {} max {} interval {}",
            case.ms,
            case.last,
            case.off,
            case.max_len,
            case.interval
        );
    }
    for case in f.reset_cases {
        assert_eq!(
            cc::should_reset(case.cur, case.max, case.margin),
            case.reset,
            "cur {} max {} margin {}",
            case.cur,
            case.max,
            case.margin
        );
    }
    for case in f.scale_cases {
        assert!(
            (cc::clamp_scale(case.input) - case.output).abs() < 1e-6,
            "scale {} vs {}",
            case.input,
            case.output
        );
    }
}
