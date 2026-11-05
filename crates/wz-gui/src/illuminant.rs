//! 조명 클러스터 시작/끝 스프라이트 (`IlluminantClusterItem.cs`).
//!
//! 원본은 `Map/Obj/sellas.img/fieldGimmick/cluster/<startPoint*2>` 와
//! `<endPoint*2+1>` 애니메이션을 시작/끝 지점에 그린다.

#[derive(Debug, Clone, Default)]
pub struct IlluminantFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub origin: (i32, i32),
}

#[derive(Debug, Clone, Default)]
pub struct IlluminantAnimation {
    pub frames: Vec<IlluminantFrame>,
    pub delays: Vec<u32>,
}

impl IlluminantAnimation {
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

#[derive(Debug, Clone, Default)]
pub struct IlluminantView {
    /// 디버그/식별용.
    #[allow(dead_code)]
    pub name: String,
    pub start: (i32, i32),
    pub end: (i32, i32),
    pub size: i32,
    pub start_anim: IlluminantAnimation,
    pub end_anim: IlluminantAnimation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_selects_by_delay() {
        let animation = IlluminantAnimation {
            frames: (0..3)
                .map(|i| IlluminantFrame {
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
