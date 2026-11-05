//! 포털 렌더링 (`PortalItem.cs`, `Map/MapHelper.img/portal/game`).

#[derive(Debug, Clone, Default)]
pub struct PortalFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub origin: (i32, i32),
}

#[derive(Debug, Clone, Default)]
pub struct PortalAnimation {
    pub frames: Vec<PortalFrame>,
    pub delays: Vec<u32>,
}

impl PortalAnimation {
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
pub struct PortalView {
    pub id: i32,
    pub portal_type: i32,
    pub x: i32,
    pub y: i32,
    /// 미니맵 아이콘 표시 여부.
    #[allow(dead_code)]
    pub shown_at_minimap: bool,
    pub animation: PortalAnimation,
}

impl PortalView {
    pub fn frame_at(&self, millis: f64) -> Option<&PortalFrame> {
        self.animation.frames.get(self.animation.frame_at(millis))
    }
}

/// 원본 `PortalItem.PortalTypes`.
pub const PORTAL_TYPES: [&str; 19] = [
    "sp", "pi", "pv", "pc", "pg", "tp", "ps", "pgi", "psi", "pcs", "ph", "psh", "pcj", "pci",
    "pci2", "pcig", "pshg", "pcc", "pcir",
];

pub fn type_name(portal_type: i32) -> &'static str {
    PORTAL_TYPES
        .get(portal_type.max(0) as usize)
        .copied()
        .unwrap_or("sp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_portal_frames_by_delay() {
        let animation = PortalAnimation {
            frames: (0..3)
                .map(|i| PortalFrame {
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

    #[test]
    fn maps_portal_type_names_like_original() {
        assert_eq!(type_name(0), "sp");
        assert_eq!(type_name(2), "pv");
        assert_eq!(type_name(7), "pgi");
        assert_eq!(type_name(999), "sp");
    }
}
