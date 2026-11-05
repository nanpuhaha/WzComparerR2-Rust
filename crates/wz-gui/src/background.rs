//! 맵 배경 레이어 렌더링 계산 (`BackItem.cs`, `GetMeshBack`).
//!
//! 텍스처/프레임 로딩은 `async_io::map_loader` 가 담당하고, 여기서는
//! 카메라 비율 스크롤, 시간 스크롤, 타일 범위를 순수 함수로 계산한다.

/// `Map/Back/<bS>.img` 에서 읽은 배경 한 개.
#[derive(Debug, Clone, Default)]
pub struct BackgroundLayer {
    pub x: i32,
    pub y: i32,
    pub rx: i32,
    pub ry: i32,
    pub cx: i32,
    pub cy: i32,
    pub tile_mode: i32,
    pub flip: bool,
    pub alpha: u8,
    pub front: bool,
    /// 각 프레임 `(width, height, rgba)`.
    pub frames: Vec<(u32, u32, Vec<u8>)>,
    pub delays: Vec<u32>,
}

impl BackgroundLayer {
    pub fn width(&self) -> u32 {
        self.frames.first().map(|f| f.0).unwrap_or(0)
    }
    pub fn height(&self) -> u32 {
        self.frames.first().map(|f| f.1).unwrap_or(0)
    }
    pub fn frame_at(&self, millis: f64) -> usize {
        if self.frames.len() <= 1 {
            return 0;
        }
        let total: u32 = if self.delays.len() == self.frames.len() {
            self.delays.iter().copied().sum()
        } else {
            self.frames.len() as u32 * 100
        }
        .max(1);
        let mut t = (millis.max(0.0) as u64 % total as u64) as u32;
        for (i, delay) in self.delays.iter().enumerate().take(self.frames.len()) {
            let delay = (*delay).max(1);
            if t < delay {
                return i;
            }
            t -= delay;
        }
        0
    }
}

/// TileMode 비트 (`TileMode.cs`).
pub const TILE_HORIZONTAL: i32 = 1;
pub const TILE_VERTICAL: i32 = 2;
pub const TILE_BOTH: i32 = 3;
pub const SCROLL_HORIZONTAL: i32 = 4;
pub const SCROLL_VERTICAL: i32 = 5;

pub fn effective_tile_size(layer: &BackgroundLayer) -> (i32, i32) {
    let mut cx = layer.cx;
    let mut cy = layer.cy;
    if (layer.tile_mode & TILE_BOTH) == TILE_BOTH && (cx == 0 || cy == 0) {
        let w = layer.width() as i32;
        let h = layer.height() as i32;
        if cx == 0 {
            cx = w;
        }
        if cy == 0 {
            cy = h;
        }
    }
    (cx, cy)
}

fn scroll_offset(rate: i32, explicit_distance: i32, tile_distance: i32, millis: f64) -> f32 {
    let reference = if explicit_distance != 0 {
        explicit_distance.abs()
    } else {
        100
    };
    let mut offset = rate as f64 * reference as f64 * millis / 20_000.0;
    if tile_distance > 0 {
        offset %= tile_distance as f64;
    }
    offset as f32
}

/// 배경의 월드 위치 (`GetMeshBack` 좌표 계산).
pub fn background_position(
    layer: &BackgroundLayer,
    camera_center: [f32; 2],
    millis: f64,
) -> [f32; 2] {
    let (cx, cy) = effective_tile_size(layer);
    let mut x = layer.x as f32;
    let mut y = layer.y as f32;
    if matches!(layer.tile_mode, SCROLL_HORIZONTAL | 6) {
        x += scroll_offset(layer.rx, layer.cx.abs().max(1), cx, millis);
    } else {
        x += camera_center[0] * (100.0 + layer.rx as f32) / 100.0;
    }
    if matches!(layer.tile_mode, SCROLL_VERTICAL | 7) {
        y += scroll_offset(layer.ry, layer.cy.abs().max(1), cy, millis);
    } else {
        y += camera_center[1] * (100.0 + layer.ry as f32) / 100.0;
    }
    [x.floor(), y.floor()]
}

/// 화면에 그릴 타일 인덱스 범위 `(left, top, right, bottom)` (배타적).
pub fn tile_range(
    layer: &BackgroundLayer,
    position: [f32; 2],
    camera_center: [f32; 2],
    viewport: [f32; 2],
    zoom: f32,
) -> (i32, i32, i32, i32) {
    let (cx, cy) = effective_tile_size(layer);
    let half_w = viewport[0] / zoom.max(0.01) * 0.5;
    let half_h = viewport[1] / zoom.max(0.01) * 0.5;
    let left = camera_center[0] - half_w;
    let right = camera_center[0] + half_w;
    let top = camera_center[1] - half_h;
    let bottom = camera_center[1] + half_h;
    let w = layer.width() as f32;
    let h = layer.height() as f32;

    let l = if (layer.tile_mode & TILE_HORIZONTAL) != 0 && cx > 0 {
        (((left - position[0] - w) / cx as f32).floor() as i32) - 1
    } else {
        0
    };
    let r = if (layer.tile_mode & TILE_HORIZONTAL) != 0 && cx > 0 {
        (((right - position[0]) / cx as f32).ceil() as i32) + 2
    } else {
        1
    };
    let t = if (layer.tile_mode & TILE_VERTICAL) != 0 && cy > 0 {
        (((top - position[1] - h) / cy as f32).floor() as i32) - 1
    } else {
        0
    };
    let b = if (layer.tile_mode & TILE_VERTICAL) != 0 && cy > 0 {
        (((bottom - position[1]) / cy as f32).ceil() as i32) + 2
    } else {
        1
    };
    (l, t, r, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer() -> BackgroundLayer {
        BackgroundLayer {
            x: 10,
            y: 20,
            rx: -15,
            ry: -15,
            cx: 2000,
            cy: 0,
            tile_mode: 1,
            alpha: 255,
            frames: vec![(100, 50, vec![0; 100 * 50 * 4])],
            delays: vec![100],
            ..Default::default()
        }
    }

    #[test]
    fn camera_ratio_scrolls_non_tile_axis() {
        let l = layer();
        let pos = background_position(&l, [100.0, 200.0], 0.0);
        assert_eq!(pos, [10.0 + 85.0, 20.0 + 170.0]);
    }

    #[test]
    fn tile_range_covers_visible_world() {
        let l = layer();
        let (lft, top, rgt, bot) = tile_range(&l, [0.0, 0.0], [0.0, 0.0], [800.0, 600.0], 1.0);
        assert!(lft <= -1 && rgt >= 2);
        assert_eq!((top, bot), (0, 1));
    }

    #[test]
    fn scroll_mode_advances_with_time() {
        let mut l = layer();
        l.tile_mode = SCROLL_HORIZONTAL;
        l.rx = -10;
        l.cx = 100;
        let a = background_position(&l, [0.0, 0.0], 0.0);
        let b = background_position(&l, [0.0, 0.0], 1000.0);
        assert!(a[0] != b[0]);
    }

    #[test]
    fn frame_selection_uses_delays() {
        let mut l = layer();
        l.frames.push((1, 1, vec![0; 4]));
        l.delays = vec![100, 200];
        assert_eq!(l.frame_at(50.0), 0);
        assert_eq!(l.frame_at(150.0), 1);
        assert_eq!(l.frame_at(350.0), 0);
    }
}
