//! `AnimationControl`의 마우스 히트테스트/드래그/휠 줌 계산.
//!
//! WinForms 이벤트 객체 대신 좌표/델타를 입력으로 받는다. 위험한 부분은
//! C# 의미론에 의존하므로 그대로 옮겼다.
//!
//! - `Math.Round(float)`는 **은행가 반올림**(MidpointRounding.ToEven)
//! - `Rectangle.Contains`는 `x <= px < x+w` (오른쪽/아래 경계 제외)
//! - 휠 줌은 `e.Delta / WHEEL_DELTA`가 **정수 나눗셈**이라 120 미만은 0틱

/// `Math.Round(float)` 상당 — 0.5는 짝수로.
pub fn round_half_even(value: f32) -> i32 {
    (value as f64).round_ties_even() as i32
}

/// `AnimationControl.GetItemAt`의 사각형.
///
/// `position`은 아이템 위치, `bound`는 `Measure()`가 준 (x, y, w, h),
/// `scale`은 `GlobalScale`.
pub fn item_rect(
    position: (i32, i32),
    bound: (i32, i32, i32, i32),
    scale: f32,
) -> (i32, i32, i32, i32) {
    (
        round_half_even(position.0 as f32 + bound.0 as f32 * scale),
        round_half_even(position.1 as f32 + bound.1 as f32 * scale),
        round_half_even(bound.2 as f32 * scale),
        round_half_even(bound.3 as f32 * scale),
    )
}

/// `Rectangle.Contains(x, y)` 상당(오른쪽/아래 경계 제외).
pub fn rect_contains(rect: (i32, i32, i32, i32), x: i32, y: i32) -> bool {
    x >= rect.0 && x < rect.0 + rect.2 && y >= rect.1 && y < rect.1 + rect.3
}

/// 히트테스트 대상: `(Position, Measure())`.
pub type HitItem = ((i32, i32), (i32, i32, i32, i32));

/// `GetItemAt`/`GetItemsAt` — 뒤에서부터(위에 있는 것 먼저) 히트한 인덱스.
pub fn hit_items(items: &[HitItem], scale: f32, x: i32, y: i32) -> Vec<usize> {
    let mut out = Vec::new();
    for index in (0..items.len()).rev() {
        let (position, bound) = items[index];
        if rect_contains(item_rect(position, bound, scale), x, y) {
            out.push(index);
        }
    }
    out
}

/// `MouseMove`의 드래그 위치: `cur - mouseDown + start`.
pub fn drag_position(mouse_down: (i32, i32), start: (i32, i32), current: (i32, i32)) -> (i32, i32) {
    (
        current.0 - mouse_down.0 + start.0,
        current.1 - mouse_down.1 + start.1,
    )
}

/// `MouseDragSaveEnabled` 판정: 드래그 박스(`mouseDown`, `dragSize`) 밖인가.
pub fn outside_drag_box(
    mouse_down: (i32, i32),
    drag_size: (i32, i32),
    current: (i32, i32),
) -> bool {
    !rect_contains(
        (mouse_down.0, mouse_down.1, drag_size.0, drag_size.1),
        current.0,
        current.1,
    )
}

/// `MouseWheel` + `GlobalScale` 세터(0.1~10 클램프).
pub fn wheel_scale(old_scale: f32, delta: i32) -> f32 {
    const WHEEL_DELTA: i32 = 120;
    // C#: `float wheelTicks = e.Delta / WHEEL_DELTA;` → 정수 나눗셈
    let ticks = (delta / WHEEL_DELTA) as f32;
    let new_scale = old_scale * (1.0 + 0.1 * ticks);
    // 100%를 가로지르면 정확히 1로 고정
    let crossed = (old_scale - 1.0).partial_cmp(&0.0) != Some(std::cmp::Ordering::Equal)
        && (new_scale - 1.0).partial_cmp(&0.0) != Some(std::cmp::Ordering::Equal)
        && ((old_scale < 1.0) != (new_scale < 1.0));
    let new_scale = if crossed { 1.0 } else { new_scale };
    new_scale.clamp(0.1, 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_half_to_even() {
        assert_eq!(round_half_even(1.5), 2);
        assert_eq!(round_half_even(2.5), 2);
        assert_eq!(round_half_even(3.5), 4);
        assert_eq!(round_half_even(-1.5), -2);
        assert_eq!(round_half_even(1.4), 1);
    }

    #[test]
    fn rect_uses_exclusive_right_bottom() {
        let rect = item_rect((0, 0), (0, 0, 10, 10), 1.0);
        assert!(rect_contains(rect, 0, 0));
        assert!(rect_contains(rect, 9, 9));
        assert!(!rect_contains(rect, 10, 9));
        assert!(!rect_contains(rect, 9, 10));
    }

    #[test]
    fn hit_test_order_is_top_down() {
        let items = [((0, 0), (0, 0, 100, 100)), ((10, 10), (0, 0, 100, 100))];
        assert_eq!(hit_items(&items, 1.0, 20, 20), vec![1, 0]);
        assert_eq!(hit_items(&items, 1.0, 5, 5), vec![0]);
    }

    #[test]
    fn drag_is_delta_plus_start() {
        assert_eq!(drag_position((10, 20), (5, 6), (13, 29)), (8, 15));
    }

    #[test]
    fn drag_box_and_wheel() {
        assert!(!outside_drag_box((10, 20), (4, 4), (13, 23)));
        assert!(outside_drag_box((10, 20), (4, 4), (14, 24)));
        // 120 미만 delta는 정수 나눗셈으로 0틱
        assert!((wheel_scale(1.0, 60) - 1.0).abs() < 1e-6);
        assert!((wheel_scale(1.0, 120) - 1.1).abs() < 1e-6);
        // 100%를 가로지르면 1로 고정
        assert!((wheel_scale(0.95, 120) - 1.0).abs() < 1e-6);
        // 상한 클램프
        assert!((wheel_scale(9.9, 240) - 10.0).abs() < 1e-6);
    }
}
