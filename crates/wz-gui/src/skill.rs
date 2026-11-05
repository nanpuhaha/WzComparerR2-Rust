//! 스킬 애니메이션 미리보기 (`MapRender.SummonSkill`).
//!
//! 맵 데이터에는 스킬이 직접 들어가지 않고, 원본은 `Skill/<img>.img/skill/
//! <id>/<ani>` 노드를 읽어 장면에 소환한다. 여기서도 같은 경로를 로드해
//! 지정한 위치/flip 으로 렌더한다.

#[derive(Debug, Clone, Default)]
pub struct SkillFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub origin: (i32, i32),
}

#[derive(Debug, Clone, Default)]
pub struct SkillView {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub flip: bool,
    pub frames: Vec<SkillFrame>,
    pub delays: Vec<u32>,
}

impl SkillView {
    pub fn frame_at(&self, millis: f64) -> Option<&SkillFrame> {
        if self.frames.is_empty() {
            return None;
        }
        if self.frames.len() == 1 {
            return self.frames.first();
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
                return self.frames.get(i);
            }
            t -= d;
        }
        self.frames.first()
    }
}

/// 원본 `SummonSkill` 의 이미지 이름 규칙.
pub fn skill_image_name(id: i32) -> String {
    if (80_000_000..90_000_000).contains(&id) {
        (id / 100).to_string()
    } else {
        (id / 10_000).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_name_matches_original() {
        assert_eq!(skill_image_name(1001005), "100");
        assert_eq!(skill_image_name(80001234), "800012");
    }

    #[test]
    fn selects_frames_by_delay() {
        let skill = SkillView {
            frames: (0..3)
                .map(|i| SkillFrame {
                    width: 1,
                    height: 1,
                    rgba: vec![i; 4],
                    origin: (0, 0),
                })
                .collect(),
            delays: vec![100, 100, 200],
            ..Default::default()
        };
        assert_eq!(skill.frame_at(0.0).unwrap().rgba[0], 0);
        assert_eq!(skill.frame_at(150.0).unwrap().rgba[0], 1);
        assert_eq!(skill.frame_at(250.0).unwrap().rgba[0], 2);
        assert_eq!(skill.frame_at(450.0).unwrap().rgba[0], 0);
    }
}
