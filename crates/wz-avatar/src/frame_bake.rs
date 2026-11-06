//! `FrameAnimationData.ApplyFlip` / `ApplyRotation`의 CPU 베이킹.
//!
//! 원본은 `CopyTexture`(RenderTarget2D + SpriteBatch.Draw)로 텍스처를 구워
//! origin을 바꾼다. GPU를 쓰지 않고 **역방향 최근접 샘플링**으로 같은 결과를
//! 만든다.
//!
//! - flip: 같은 크기, `dest(x,y) = src(w-1-x, y)` / y 반전
//! - rotation: `rotation_bound`로 새 크기·origin을 정하고,
//!   `q = dest - newOrigin`, `v = R(-θ)·q`, `src = round(origin + v)`
//!
//! XNA `SpriteBatch`의 기본 샘플러는 선형이지만, 결정적 비교를 위해
//! **최근접**으로 고정한다(문서 참고).

use wz_db::draw_math::{flipped_origin, rotation_bound};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub origin: (i32, i32),
}

fn sample(src: &[u8], w: u32, h: u32, x: i32, y: i32) -> [u8; 4] {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
        return [0, 0, 0, 0];
    }
    let i = ((y as u32 * w + x as u32) * 4) as usize;
    [src[i], src[i + 1], src[i + 2], src[i + 3]]
}

/// `ApplyFlip` — 좌우/상하 반전 후 origin 보정.
pub fn apply_flip(
    rgba: &[u8],
    width: u32,
    height: u32,
    origin: (i32, i32),
    flip_x: bool,
    flip_y: bool,
) -> BakedFrame {
    assert_eq!(rgba.len(), (width * height * 4) as usize);
    let mut out = vec![0u8; rgba.len()];
    for y in 0..height {
        for x in 0..width {
            let sx = if flip_x { width - 1 - x } else { x };
            let sy = if flip_y { height - 1 - y } else { y };
            let s = ((sy * width + sx) * 4) as usize;
            let d = ((y * width + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
        }
    }
    BakedFrame {
        width,
        height,
        rgba: out,
        origin: flipped_origin((width as i32, height as i32), origin, flip_x, flip_y),
    }
}

/// `ApplyRotation` — 각도(도)를 `((a+360)%360)`로 정규화하고 베이킹.
pub fn apply_rotation(
    rgba: &[u8],
    width: u32,
    height: u32,
    origin: (i32, i32),
    degrees: i32,
) -> BakedFrame {
    assert_eq!(rgba.len(), (width * height * 4) as usize);
    let angle = (degrees + 360).rem_euclid(360);
    if angle == 0 {
        return BakedFrame {
            width,
            height,
            rgba: rgba.to_vec(),
            origin,
        };
    }
    let radians = angle as f32 * 0.017_453_292_f32;
    let ((new_w, new_h), new_origin) =
        rotation_bound((width as i32, height as i32), radians, origin);

    let sin = f64::from(radians).sin();
    let cos = f64::from(radians).cos();
    let mut out = vec![0u8; (new_w.max(0) as u32 * new_h.max(0) as u32 * 4) as usize];
    for dy in 0..new_h {
        for dx in 0..new_w {
            let qx = f64::from(dx - new_origin.0);
            let qy = f64::from(dy - new_origin.1);
            // R(-θ)·q
            let vx = qx * cos + qy * sin;
            let vy = -qx * sin + qy * cos;
            let sx = (f64::from(origin.0) + vx).round_ties_even() as i32;
            let sy = (f64::from(origin.1) + vy).round_ties_even() as i32;
            let pixel = sample(rgba, width, height, sx, sy);
            let d = ((dy as u32 * new_w.max(0) as u32 + dx as u32) * 4) as usize;
            out[d..d + 4].copy_from_slice(&pixel);
        }
    }
    BakedFrame {
        width: new_w.max(0) as u32,
        height: new_h.max(0) as u32,
        rgba: out,
        origin: new_origin,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite() -> Vec<u8> {
        // 2x2: red, green / blue, white
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ]
    }

    #[test]
    fn flip_horizontal_mirrors_pixels_and_origin() {
        let baked = apply_flip(&sprite(), 2, 2, (1, 1), true, false);
        assert_eq!(baked.origin, (2 - 1, 1));
        // 첫 행: 초록, 빨강
        assert_eq!(&baked.rgba[0..4], &[0, 255, 0, 255]);
        assert_eq!(&baked.rgba[4..8], &[255, 0, 0, 255]);
    }

    #[test]
    fn flip_both_mirrors_both_axes() {
        let baked = apply_flip(&sprite(), 2, 2, (1, 1), true, true);
        assert_eq!(baked.origin, (1, 1));
        assert_eq!(&baked.rgba[0..4], &[255, 255, 255, 255]);
    }

    #[test]
    fn zero_rotation_is_identity() {
        let baked = apply_rotation(&sprite(), 2, 2, (1, 1), 0);
        assert_eq!(baked.width, 2);
        assert_eq!(baked.origin, (1, 1));
        assert_eq!(baked.rgba, sprite());
        // 360도도 항등.
        assert_eq!(apply_rotation(&sprite(), 2, 2, (1, 1), 360).rgba, sprite());
    }

    #[test]
    fn rotation_90_expands_and_keeps_center_pixel() {
        // 4x2 전부 불투명한 단색으로 회전 크기만 확인.
        let mut src = vec![9u8; 4 * 2 * 4];
        for p in src.chunks_mut(4) {
            p[3] = 255;
        }
        let baked = apply_rotation(&src, 4, 2, (2, 1), 90);
        assert!(baked.width >= 2 && baked.height >= 4);
        assert_eq!(baked.rgba.len(), (baked.width * baked.height * 4) as usize);
        let opaque = baked.rgba.chunks(4).filter(|p| p[3] == 255).count();
        assert!(opaque > 0);
    }
}
