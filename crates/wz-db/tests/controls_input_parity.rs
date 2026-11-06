//! `AnimationControl`의 히트테스트/드래그/휠 계산을 C# 참조 구현과 비교한다.
//!
//! C# 쪽은 `Math.Round`(은행가 반올림)와 `System.Drawing.Rectangle.Contains`를
//! 실제 .NET API로 실행한다.

use serde::Deserialize;
use wz_db::controls_input::{
    drag_position, hit_items, item_rect, outside_drag_box, wheel_scale, HitItem,
};

const ITEMS: [HitItem; 3] = [
    ((0, 0), (0, 0, 100, 50)),
    ((20, 10), (-5, -5, 30, 30)),
    ((-3, 7), (2, 2, 10, 10)),
];

#[derive(Deserialize)]
struct Fixture {
    rects: Vec<RectCase>,
    hits: Vec<HitCase>,
    ties: Vec<TieCase>,
    drags: Vec<DragCase>,
    drag_boxes: Vec<BoxCase>,
    wheels: Vec<WheelCase>,
}

#[derive(Deserialize)]
struct RectCase {
    px: i32,
    py: i32,
    bx: i32,
    by: i32,
    bw: i32,
    bh: i32,
    scale: f32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[derive(Deserialize)]
struct HitCase {
    scale: f32,
    x: i32,
    y: i32,
    indices: Vec<usize>,
}

#[derive(Deserialize)]
struct TieCase {
    bx: i32,
    scale: f32,
    x: i32,
}

#[derive(Deserialize)]
struct DragCase {
    mouse_down: [i32; 2],
    start: [i32; 2],
    current: [i32; 2],
    position: [i32; 2],
}

#[derive(Deserialize)]
struct BoxCase {
    drag_size: [i32; 2],
    current: [i32; 2],
    inside: bool,
}

#[derive(Deserialize)]
struct WheelCase {
    old_scale: f32,
    delta: i32,
    new_scale: f32,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/controls_input.json")).unwrap()
}

#[test]
fn hit_test_rects_match_csharp() {
    for c in fixture().rects {
        let got = item_rect((c.px, c.py), (c.bx, c.by, c.bw, c.bh), c.scale);
        assert_eq!(
            got,
            (c.x, c.y, c.w, c.h),
            "pos ({},{}) bound ({},{},{},{}) scale {}",
            c.px,
            c.py,
            c.bx,
            c.by,
            c.bw,
            c.bh,
            c.scale
        );
    }
}

#[test]
fn hit_order_matches_csharp() {
    for c in fixture().hits {
        assert_eq!(
            hit_items(&ITEMS, c.scale, c.x, c.y),
            c.indices,
            "scale {} point ({},{})",
            c.scale,
            c.x,
            c.y
        );
    }
}

#[test]
fn half_even_ties_match_csharp() {
    for c in fixture().ties {
        assert_eq!(
            item_rect((0, 0), (c.bx, 0, 0, 0), c.scale).0,
            c.x,
            "{} * {} (banker's rounding)",
            c.bx,
            c.scale
        );
    }
}

#[test]
fn drag_positions_match_csharp() {
    for c in fixture().drags {
        assert_eq!(
            drag_position(
                (c.mouse_down[0], c.mouse_down[1]),
                (c.start[0], c.start[1]),
                (c.current[0], c.current[1])
            ),
            (c.position[0], c.position[1])
        );
    }
}

#[test]
fn drag_box_matches_csharp() {
    for c in fixture().drag_boxes {
        assert_eq!(
            !outside_drag_box(
                (10, 20),
                (c.drag_size[0], c.drag_size[1]),
                (c.current[0], c.current[1])
            ),
            c.inside,
            "drag_size {:?} current {:?}",
            c.drag_size,
            c.current
        );
    }
}

#[test]
fn wheel_scale_matches_csharp() {
    for c in fixture().wheels {
        let got = wheel_scale(c.old_scale, c.delta);
        assert!(
            (got - c.new_scale).abs() < 1e-6,
            "old {} delta {} -> {} (want {})",
            c.old_scale,
            c.delta,
            got,
            c.new_scale
        );
    }
}
