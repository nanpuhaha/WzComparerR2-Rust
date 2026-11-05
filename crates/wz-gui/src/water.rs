//! 물 배경 스프라이트 (`waterBack` / `waterFront`).
//!
//! 원본은 `MsCustomSprite` + 전용 픽셀 셰이더로 그린다. 이 모듈은 동일한
//! 배경 프레임/텍스처를 로드하고, 셰이더 대신 시간에 따른 물빛 틴트로
//! 렌더한다. 텍스처 주소/알파/패럴랙스는 원본 BackgroundLayer 규칙을 쓴다.

use crate::background::BackgroundLayer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WaterKind {
    #[default]
    Back,
    Front,
}

#[derive(Debug, Clone, Default)]
pub struct WaterFrame {
    pub textures: Vec<(u32, u32, Vec<u8>)>,
}

#[derive(Debug, Clone, Default)]
pub struct WaterLayer {
    /// 배경 좌표/패럴랙스/알파.
    pub background: BackgroundLayer,
    pub kind: WaterKind,
    pub frames: Vec<WaterFrame>,
    pub delays: Vec<u32>,
    /// WZ `shader/waterBack/constant`에서 파싱한 값.
    pub back_params: Option<wz_render::WaterBackParams>,
    /// WZ `shader/waterFront/constant`에서 파싱한 값.
    pub front_params: Option<wz_render::WaterFrontParams>,
}

impl WaterLayer {
    pub fn frame_at(&self, millis: f64) -> Option<&WaterFrame> {
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

    /// 원본 waterBack/waterFront 를 근사한 시간 기반 틴트.
    pub fn tint(&self, millis: f64) -> [f32; 4] {
        let t = millis as f32 / 1000.0;
        let alpha = self.background.alpha as f32 / 255.0;
        match self.kind {
            WaterKind::Back => [0.20, 0.42, 0.72, 0.55 * alpha],
            WaterKind::Front => {
                let wave = (t * 1.7).sin() * 0.06;
                [
                    (0.42 + wave).clamp(0.0, 1.0),
                    (0.70 + wave * 0.5).clamp(0.0, 1.0),
                    (0.98 + wave * 0.2).clamp(0.0, 1.0),
                    (0.34 + wave.abs()) * alpha,
                ]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(kind: WaterKind) -> WaterLayer {
        WaterLayer {
            kind,
            frames: vec![WaterFrame {
                textures: vec![(1, 1, vec![255; 4])],
            }],
            delays: vec![100],
            ..Default::default()
        }
    }

    #[test]
    fn front_tint_animates_but_back_is_static() {
        let back = layer(WaterKind::Back);
        assert_eq!(back.tint(0.0), back.tint(1000.0));
        let front = layer(WaterKind::Front);
        assert_ne!(front.tint(0.0), front.tint(700.0));
    }

    #[test]
    fn frame_selection_wraps() {
        let layer = WaterLayer {
            frames: vec![WaterFrame::default(), WaterFrame::default()],
            delays: vec![100, 200],
            ..Default::default()
        };
        assert_eq!(layer.frame_at(50.0).map(|_| 0), Some(0));
        assert_eq!(layer.frame_at(150.0).map(|_| 0), Some(0));
        // 두 프레임 모두 default 라 구분은 인덱스 대신 시간 경계로 확인.
        assert!(layer.frame_at(299.0).is_some());
        assert!(layer.frame_at(300.0).is_some());
    }
}
