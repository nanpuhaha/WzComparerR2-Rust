//! 애니메이션 프레임 딜레이 정규화 및 알파 보간(`a0..a1`).
//!
//! `docs/known-bugs.md` T16/T17 및 C# `Frame.cs` / `Gif.CreateFromNode` 대응:
//! - `delay == 0` 이면 기본값 `120`ms를 사용한다.
//! - 음수 `delay`는 `abs()`로 양수 정규화하여 타임라인 역행을 방지한다.
//! - 프레임 시작 알파(`a0`, 기본 255)와 종료 알파(`a1`, 기본 255)를 시간 `t` (`0..=delay`)에서 선형 보간한다.

/// 단일 애니메이션 프레임의 타이밍 및 투명도 파라미터.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTiming {
    /// 정규화된 프레임 지속 시간(ms, 항상 > 0).
    pub delay_ms: u32,
    /// 프레임 시작 알파 (0..=255).
    pub a0: u8,
    /// 프레임 종료 알파 (0..=255).
    pub a1: u8,
}

impl Default for FrameTiming {
    fn default() -> Self {
        Self {
            delay_ms: 120,
            a0: 255,
            a1: 255,
        }
    }
}

impl FrameTiming {
    /// 원본 WZ의 `delay` 값을 정규화하여 [`FrameTiming`]을 생성한다.
    ///
    /// - `raw_delay == 0` -> `120`
    /// - `raw_delay < 0` -> `raw_delay.unsigned_abs()`
    pub fn from_raw(raw_delay: Option<i32>, a0: Option<i32>, a1: Option<i32>) -> Self {
        let delay_ms = match raw_delay.unwrap_or(0) {
            0 => 120,
            d => d.unsigned_abs(),
        };
        let clamp_u8 = |v: i32| v.clamp(0, 255) as u8;
        let a0_val = a0.map(clamp_u8).unwrap_or(255);
        let a1_val = a1.map(clamp_u8).unwrap_or(a0.map(clamp_u8).unwrap_or(255));
        Self {
            delay_ms,
            a0: a0_val,
            a1: a1_val,
        }
    }

    /// 프레임 내 경과 시간 `elapsed_ms` (`0..=delay_ms`)에서의 보간된 알파(`0..=255`)를 계산한다.
    pub fn alpha_at(&self, elapsed_ms: u32) -> u8 {
        if self.delay_ms == 0 || self.a0 == self.a1 {
            return self.a0;
        }
        let t = (elapsed_ms.min(self.delay_ms) as f32) / (self.delay_ms as f32);
        let a = (self.a0 as f32) * (1.0 - t) + (self.a1 as f32) * t;
        a.round().clamp(0.0, 255.0) as u8
    }

    /// 정규화된 실수 알파(`0.0..=1.0`)를 반환한다.
    pub fn alpha_f32_at(&self, elapsed_ms: u32) -> f32 {
        (self.alpha_at(elapsed_ms) as f32) / 255.0
    }
}
