//! 키보드 카메라 이동 (`FrmMapRender2.cs:341-386`).
//!
//! 원본은 방향키 입력마다 `direction1` 을 누적하고 매 프레임 감쇠시킨다.
//! 여기서도 같은 관성 이동을 순수 함수로 분리한다.

use glam::Vec2;

pub fn step(velocity: &mut Vec2, center: &mut Vec2, dir: Vec2, boost: bool, zoom: f32) {
    if dir != Vec2::ZERO {
        let speed = if boost { 30.0 } else { 10.0 };
        *velocity += dir.normalize() * speed / zoom.max(0.01);
    }
    if velocity.length_squared() > 0.01 {
        *center += *velocity;
        *velocity *= 0.92;
    } else {
        *velocity = Vec2::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_input_moves_camera_and_decays() {
        let mut velocity = Vec2::ZERO;
        let mut center = Vec2::ZERO;
        step(&mut velocity, &mut center, Vec2::new(1.0, 0.0), false, 1.0);
        assert!(center.x > 0.0);
        let after_first = center.x;
        step(&mut velocity, &mut center, Vec2::ZERO, false, 1.0);
        assert!(center.x > after_first);
        for _ in 0..100 {
            step(&mut velocity, &mut center, Vec2::ZERO, false, 1.0);
        }
        assert_eq!(velocity, Vec2::ZERO);
    }

    #[test]
    fn ctrl_boost_moves_faster() {
        let mut v1 = Vec2::ZERO;
        let mut c1 = Vec2::ZERO;
        step(&mut v1, &mut c1, Vec2::new(0.0, -1.0), false, 1.0);
        let mut v2 = Vec2::ZERO;
        let mut c2 = Vec2::ZERO;
        step(&mut v2, &mut c2, Vec2::new(0.0, -1.0), true, 1.0);
        assert!(c2.y < c1.y);
    }
}
