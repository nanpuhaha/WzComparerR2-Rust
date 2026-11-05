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
}

impl LifeView {
    pub fn draw_frame(&self, millis: f64) -> Option<&LifeFrame> {
        self.animation.frames.get(self.animation.frame_at(millis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
