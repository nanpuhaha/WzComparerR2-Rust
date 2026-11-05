//! 리액터 stage/hit 상태 (`ReactorItem.cs`).
//!
//! 원본은 `StateMachineAnimator` 로 stage 전환을 처리한다. 여기서는 같은
//! 프레임 데이터를 보관하고, 클릭 시 현재 stage 의 `hit` 을 재생한 뒤
//! 다음 stage 로 넘어가는 상태를 순수 함수로 계산한다.

use std::collections::BTreeMap;

/// 리액터 프레임 한 장.
#[derive(Debug, Clone, Default)]
pub struct ReactorFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub origin: (i32, i32),
}

/// stage 하나의 반복/hit 애니메이션.
#[derive(Debug, Clone, Default)]
pub struct ReactorAnimation {
    pub frames: Vec<ReactorFrame>,
    pub delays: Vec<u32>,
}

impl ReactorAnimation {
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
    /// 경과 시간(ms)에 해당하는 프레임 인덱스.
    pub fn frame_at(&self, millis: f64) -> usize {
        if self.frames.len() <= 1 {
            return 0;
        }
        let total: u32 = self
            .delays
            .iter()
            .copied()
            .take(self.frames.len())
            .sum::<u32>()
            .max(1);
        let mut t = (millis.max(0.0) as u64 % total as u64) as u32;
        for (i, delay) in self.delays.iter().take(self.frames.len()).enumerate() {
            let d = (*delay).max(1);
            if t < d {
                return i;
            }
            t -= d;
        }
        0
    }
    pub fn total_delay(&self) -> u32 {
        self.delays
            .iter()
            .copied()
            .take(self.frames.len())
            .sum::<u32>()
            .max(1)
    }
}

/// 맵에 배치된 리액터 하나.
#[derive(Debug, Clone, Default)]
pub struct ReactorView {
    pub index: i32,
    /// 리액터 사운드 경로(`Sound/Reactor.img/<id>/<stage>`)용.
    #[allow(dead_code)]
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub flip: bool,
    pub stages: BTreeMap<i32, ReactorAnimation>,
    /// `{stage}/hit`.
    pub hits: BTreeMap<i32, ReactorAnimation>,
    pub stage: i32,
    /// hit 재생이 끝나면 이동할 stage.
    pub pending_stage: Option<i32>,
    pub hit_stage: Option<i32>,
    pub hit_started: f64,
}

/// 렌더할 프레임 선택 결과.
pub struct ReactorDraw<'a> {
    pub frame: &'a ReactorFrame,
    pub animation: &'a ReactorAnimation,
    pub hit: bool,
}

impl ReactorView {
    pub fn current_animation(&self, time: f64) -> Option<(&ReactorAnimation, bool)> {
        if let Some(hit_stage) = self.hit_stage {
            if let Some(hit) = self.hits.get(&hit_stage) {
                if !hit.is_empty() {
                    let elapsed = (time - self.hit_started).max(0.0) * 1000.0;
                    if elapsed < hit.total_delay() as f64 {
                        return Some((hit, true));
                    }
                }
            }
        }
        self.stages.get(&self.stage).map(|a| (a, false))
    }

    pub fn draw_at(&self, time: f64) -> Option<ReactorDraw<'_>> {
        let (animation, hit) = self.current_animation(time)?;
        if animation.is_empty() {
            return None;
        }
        let elapsed = if hit {
            (time - self.hit_started).max(0.0) * 1000.0
        } else {
            time * 1000.0
        };
        let frame = animation.frames.get(animation.frame_at(elapsed))?;
        Some(ReactorDraw {
            frame,
            animation,
            hit,
        })
    }

    /// 클릭 시 `NextStage = Stage + 1` 을 적용한다 (`SceneRendering.cs:383`).
    ///
    /// 성공하면 재생할 사운드의 `(reactor id, 현재 stage)` 를 돌려준다.
    pub fn click(&mut self, time: f64) -> Option<(i32, i32)> {
        let next = if let Some(pending) = self.pending_stage {
            pending
        } else {
            self.stage + 1
        };
        if !self.stages.contains_key(&next) {
            return None;
        }
        let sound = (self.id, self.stage);
        if self
            .hits
            .get(&self.stage)
            .map(|h| !h.is_empty())
            .unwrap_or(false)
        {
            self.hit_stage = Some(self.stage);
            self.hit_started = time;
            self.pending_stage = Some(next);
        } else {
            self.stage = next;
            self.pending_stage = None;
        }
        Some(sound)
    }

    /// hit 이 끝났으면 다음 stage 로 전환한다. 전환 여부를 돌려준다.
    pub fn tick(&mut self, time: f64) -> bool {
        let Some(hit_stage) = self.hit_stage else {
            return false;
        };
        let duration = self
            .hits
            .get(&hit_stage)
            .map(|h| h.total_delay() as f64 / 1000.0)
            .unwrap_or(0.0);
        if time - self.hit_started >= duration {
            if let Some(next) = self.pending_stage.take() {
                self.stage = next;
            }
            self.hit_stage = None;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(n: u8) -> ReactorFrame {
        ReactorFrame {
            width: 2,
            height: 2,
            rgba: vec![n; 16],
            origin: (1, 1),
        }
    }
    fn anim(frames: usize, delay: u32) -> ReactorAnimation {
        ReactorAnimation {
            frames: (0..frames).map(|i| frame(i as u8)).collect(),
            delays: vec![delay; frames],
        }
    }
    fn view() -> ReactorView {
        let mut stages = BTreeMap::new();
        stages.insert(0, anim(2, 100));
        stages.insert(1, anim(2, 200));
        let mut hits = BTreeMap::new();
        hits.insert(0, anim(2, 50));
        ReactorView {
            stages,
            hits,
            ..Default::default()
        }
    }

    #[test]
    fn click_plays_hit_then_advances_stage() {
        let mut v = view();
        assert_eq!(v.click(10.0), Some((0, 0)));
        assert_eq!(v.stage, 0);
        assert_eq!(v.hit_stage, Some(0));
        assert!(v.draw_at(10.02).unwrap().hit);
        assert!(!v.tick(10.05));
        assert!(v.tick(10.11));
        assert_eq!(v.stage, 1);
        assert_eq!(v.hit_stage, None);
        assert!(!v.draw_at(10.2).unwrap().hit);
    }

    #[test]
    fn click_without_frames_does_nothing() {
        let mut v = view();
        v.stage = 1;
        assert_eq!(v.click(0.0), None);
        assert_eq!(v.stage, 1);
    }

    #[test]
    fn frame_selection_wraps() {
        let a = anim(3, 100);
        assert_eq!(a.frame_at(0.0), 0);
        assert_eq!(a.frame_at(150.0), 1);
        assert_eq!(a.frame_at(250.0), 2);
        assert_eq!(a.frame_at(300.0), 0);
    }
}
