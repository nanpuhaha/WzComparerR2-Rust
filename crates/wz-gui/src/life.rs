//! 맵 라이프(Mob/NPC) 정적 애니메이션 (`LifeItem.cs`).
//!
//! 원본은 AI/컨트롤러로 이동까지 시뮬레이션하지만, 맵 뷰어에 필요한
//! `stand` 애니메이션·flip·origin·hide 는 같은 데이터로 재현한다.

#[derive(Debug, Clone, Default)]
pub struct LifeFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub origin: (i32, i32),
}

#[derive(Debug, Clone, Default)]
pub struct LifeAnimation {
    pub frames: Vec<LifeFrame>,
    pub delays: Vec<u32>,
}

impl LifeAnimation {
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
}

/// 맵에 배치된 라이프 하나.
#[derive(Debug, Clone, Default)]
pub struct LifeView {
    pub id: i32,
    pub index: i32,
    pub npc: bool,
    pub x: i32,
    pub y: i32,
    pub flip: bool,
    pub hidden: bool,
    /// NPC 이름 (`info/hideName == 1` 이면 표시하지 않음).
    pub name: Option<String>,
    pub hide_name: bool,
    pub animation: LifeAnimation,
    /// 이동 애니메이션 (`move`/`walk1`).
    pub walk: LifeAnimation,
    /// `rx0`/`rx1` 이동 범위.
    pub rx0: i32,
    pub rx1: i32,
    /// 발판 ID.
    pub fh: i32,
    /// 현재 월드 위치 (AI).
    pub position_x: f32,
    pub position_y: f32,
    /// -1 = 왼쪽, 1 = 오른쪽.
    pub direction: f32,
    pub moving: bool,
}

impl LifeView {
    /// 현재 상태(`stand`/`move`)에 맞는 프레임.
    pub fn draw_frame(&self, millis: f64) -> Option<&LifeFrame> {
        let animation = if self.moving && !self.walk.frames.is_empty() {
            &self.walk
        } else {
            &self.animation
        };
        animation.frames.get(animation.frame_at(millis))
    }

    /// 원본 `BehaviorController` 의 이동을 단순화한 발판/범위 AI.
    ///
    /// 몹은 `rx0..rx1` 사이를 왕복하고, 발판이 있으면 x에 따른 y를 보간한다.
    /// NPC는 제자리에 선다.
    pub fn tick(
        &mut self,
        dt: f64,
        millis: f64,
        footholds: &std::collections::HashMap<i32, wz_db::Foothold>,
    ) {
        if self.npc || self.hidden {
            self.moving = false;
            return;
        }
        let (lo, hi) = if self.rx0 < self.rx1 {
            (self.rx0, self.rx1)
        } else {
            (self.rx1, self.rx0)
        };
        if hi <= lo {
            self.moving = false;
            self.position_x = self.x as f32;
            self.position_y = self.y as f32;
            return;
        }
        self.moving = true;
        let speed = 60.0; // px/s, 원본 LifeInfo.speed 를 단순화
        self.position_x += self.direction * speed * dt as f32;
        if self.position_x >= hi as f32 {
            self.position_x = hi as f32;
            self.direction = -1.0;
        } else if self.position_x <= lo as f32 {
            self.position_x = lo as f32;
            self.direction = 1.0;
        }
        self.flip = self.direction < 0.0;
        if let Some(fh) = footholds.get(&self.fh) {
            if fh.x1 == fh.x2 {
                self.position_y = fh.y1 as f32;
            } else {
                let t = ((self.position_x - fh.x1 as f32) / (fh.x2 - fh.x1) as f32).clamp(0.0, 1.0);
                self.position_y = fh.y1 as f32 + (fh.y2 - fh.y1) as f32 * t;
            }
        }
        // walk 프레임이 없으면 stand 로 대체한다.
        if self.walk.frames.is_empty() {
            let _ = millis;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mob_patrols_between_rx_bounds() {
        let mut life = LifeView {
            x: 0,
            y: 0,
            rx0: -100,
            rx1: 100,
            direction: 1.0,
            position_x: 0.0,
            position_y: 0.0,
            ..Default::default()
        };
        let footholds = std::collections::HashMap::new();
        life.tick(1.0, 0.0, &footholds);
        assert!((life.position_x - 60.0).abs() < 0.01);
        life.position_x = 95.0;
        life.tick(1.0, 0.0, &footholds);
        assert_eq!(life.position_x, 100.0);
        assert_eq!(life.direction, -1.0);
        assert!(life.flip);
        life.tick(10.0, 0.0, &footholds);
        assert_eq!(life.position_x, -100.0);
        assert_eq!(life.direction, 1.0);
    }

    #[test]
    fn npc_does_not_move() {
        let mut life = LifeView {
            npc: true,
            rx0: -100,
            rx1: 100,
            ..Default::default()
        };
        life.tick(1.0, 0.0, &std::collections::HashMap::new());
        assert!(!life.moving);
        assert_eq!(life.position_x, 0.0);
    }

    #[test]
    fn foothold_interpolates_y() {
        let mut life = LifeView {
            rx0: 0,
            rx1: 200,
            fh: 1,
            position_x: 50.0,
            direction: 1.0,
            ..Default::default()
        };
        let mut footholds = std::collections::HashMap::new();
        footholds.insert(
            1,
            wz_db::Foothold {
                id: 1,
                x1: 0,
                y1: 100,
                x2: 200,
                y2: 0,
                ..Default::default()
            },
        );
        life.tick(0.0, 0.0, &footholds);
        assert!((life.position_y - 75.0).abs() < 0.01);
    }

    #[test]
    fn selects_frames_by_delay() {
        let animation = LifeAnimation {
            frames: (0..3)
                .map(|i| LifeFrame {
                    width: 1,
                    height: 1,
                    rgba: vec![i; 4],
                    origin: (0, 0),
                })
                .collect(),
            delays: vec![100, 100, 200],
        };
        assert_eq!(animation.frame_at(0.0), 0);
        assert_eq!(animation.frame_at(150.0), 1);
        assert_eq!(animation.frame_at(250.0), 2);
        assert_eq!(animation.frame_at(450.0), 0);
    }
}
