//! Oracle 엄격 합성기 — `wayfinding §2-1` 의 정수 수학 소프트웨어 compositor.
//!
//! # 어디에 쓰이는가
//!
//! `wz oracle avatar` (`AvatarAction`) 만 이 모듈을 섞는다. wz-render/wgpu,
//! GUI 스타일 블렌더와는 계약이 다르다. C# 오라클 (`WzComparerR2.Oracle`) 도
//! 아래의 수학을 **문자 그대로** 복사해야 하므로 이 모듈의 사양은 "규약"이다:
//!
//! ```text
//! eff  = (sa * ea + 127) / 255
//! rgb  = (src * eff + 127) / 255 + (dst * (255 - eff) + 127) / 255
//! aout = eff + (da * (255 - eff) + 127) / 255
//! ```
//!
//! - straight alpha (premultiplied 아님 — WZ Geometry: 투명하는 영역 0).
//! - 반올림은 무조건 `(x + 127) / 255` — C#/Rust 의 `DivRound` 동일.
//! - memcpy 처럼 겹침으로 전이되지 않은 레이어는 순서대로 (양측 heme 정렬 순서대로
//!   커�배리 구분) 본 모듈은 입력 순서를 바꾸지 않는다.
use crate::Point;

/// 합성에 쓰이는 단일 레이러 소스.
///
/// `pos` 는 합성 캔버스 절대 좌표 (발생을 이미 배제한 그림 왼쪽 위).
#[derive(Debug, Clone, Copy)]
pub struct LayerSource<'a> {
    /// RGBA8, straight alpha (`width * height * 4`).
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub pos: Point,
    /// 프레임 보간 알파(0..=255). `FrameTiming::alpha_at(elapsed)` 값.
    pub alpha: u8,
}

/// 합성 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedFrame {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` RGBA8.
    pub rgba: Vec<u8>,
}

/// 여러 레이어의 좌표를 통합한 바운딩 박스와 최소 `-min` 교정.
///
/// 레이어가 0 개면 `None`.
pub fn bounding_box(layers: &[LayerSource<'_>]) -> Option<(Point, u32, u32)> {
    if layers.is_empty() {
        return None;
    }
    let mut min = Point::new(i32::MAX, i32::MAX);
    let mut max = Point::new(i32::MIN, i32::MIN);
    for l in layers {
        min.x = min.x.min(l.pos.x);
        min.y = min.y.min(l.pos.y);
        max.x = max.x.max(l.pos.x + l.width as i32);
        max.y = max.y.max(l.pos.y + l.height as i32);
    }
    let w = (max.x - min.x).max(0) as u32;
    let h = (max.y - min.y).max(0) as u32;
    Some((min, w, h))
}

fn blend(px_out: &mut [u8], src: [u8; 4], eff: u32) {
    let d = eff;
    // C#/Rust 공용 반올림 상수: +127, /255
    let div255 = |v: u32| (v + 127) / 255;
    let rest = 255 - d;
    let s = [
        div255(src[0] as u32 * d),
        div255(src[1] as u32 * d),
        div255(src[2] as u32 * d),
    ];
    let da = d + div255(px_out[3] as u32 * rest);
    let mut out_rgb = [0u8; 3];
    for i in 0..3 {
        let out = s[i] + div255(px_out[i] as u32 * rest);
        out_rgb[i] = out.min(255) as u8;
    }
    px_out[..3].copy_from_slice(&out_rgb);
    px_out[3] = da.min(255) as u8;
}

/// 레이어 순서대로 straight-alpha source-over 합성.
///
/// 캔버스 크기는 [`bounding_box`] 로 잡는다 (레이어 0개면 오류).
pub fn compose(layers: &[LayerSource<'_>]) -> Result<ComposedFrame, ComposeError> {
    let (min, w, h) = bounding_box(layers).ok_or(ComposeError::NoLayers)?;
    let stride = w as usize * 4;
    let mut rgba = vec![0u8; stride * h as usize];

    for l in layers {
        if l.rgba.len() < (l.width as usize) * (l.height as usize) * 4 {
            return Err(ComposeError::PixelTooShort {
                want: (l.width as usize) * (l.height as usize) * 4,
                got: l.rgba.len(),
            });
        }
        let src_stride = l.width as usize * 4;
        for sy in 0..l.height as i32 {
            let dy = l.pos.y + sy - min.y;
            if dy < 0 || dy >= h as i32 {
                continue;
            }
            for sx in 0..l.width as i32 {
                let dx = l.pos.x + sx - min.x;
                if dx < 0 || dx >= w as i32 {
                    continue;
                }
                let so = (sy as usize * src_stride) + (sx as usize * 4);
                let s = [l.rgba[so], l.rgba[so + 1], l.rgba[so + 2], l.rgba[so + 3]];
                if s[3] == 0 {
                    continue;
                }
                // eff = (sa * ea + 127) / 255
                let eff = (s[3] as u32 * l.alpha as u32 + 127) / 255;
                if eff == 0 {
                    continue;
                }
                let do_ = (dy as usize * stride) + (dx as usize * 4);
                blend(&mut rgba[do_..do_ + 4], s, eff);
            }
        }
    }
    Ok(ComposedFrame {
        width: w,
        height: h,
        rgba,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposeError {
    NoLayers,
    PixelTooShort { want: usize, got: usize },
}

impl std::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComposeError::NoLayers => write!(f, "합성할 레이어가 없다"),
            ComposeError::PixelTooShort { want, got } => {
                write!(f, "레이어 픽셀이 짧다: {got}/{want} 바이트")
            }
        }
    }
}

impl std::error::Error for ComposeError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
        [r, g, b, a]
    }

    #[test]
    fn compose_no_layers() {
        assert_eq!(compose(&[]).unwrap_err(), ComposeError::NoLayers);
    }

    #[test]
    fn compose_single_full_layer_offsets_to_origin() {
        let red = [255u8, 0, 0, 255];
        let pixels: Vec<u8> = red.iter().copied().cycle().take(2 * 2 * 4).collect();
        let layers = [LayerSource {
            rgba: &pixels,
            width: 2,
            height: 2,
            pos: Point::new(0, 0),
            alpha: 255,
        }];
        let out = compose(&layers).unwrap();
        assert_eq!((out.width, out.height), (2, 2));
        assert_eq!(out.rgba.len(), 2 * 2 * 4);
        assert_eq!(
            &out.rgba[0..4],
            &[255, 0, 0, 255],
            "alpha=255 완중 레이어는 픽셀 그대로"
        );
    }

    #[test]
    fn compose_alpha_over_integers_matches_pinned_formula() {
        // 규정 수학 대입: red(255,0,0,255) over blue(0,0,255,255) with ea=255
        // eff = (255*255 + 127)/255 = 255, rest = 0 → 수칙: out = (src*255+127)/255
        let src = px(255, 0, 0, 255);
        let dst = px(0, 0, 255, 255);
        let mut c = dst;
        blend(&mut c, src, 255);
        assert_eq!(c, [255, 0, 0, 255]);
    }

    #[test]
    fn compose_half_alpha_duplicates_rounding() {
        // 레지 255, ea=128 → eff = (255*128+127)/255 = 128.
        // src as 255*.rgb = 128 (div255=(255*128+127)/255 = 128)
        let src = px(255, 255, 255, 255);
        let dst = px(0, 0, 0, 0);
        let mut c = dst;
        blend(&mut c, src, 128);
        assert_eq!(c[0], 128);
        assert_eq!(c[3], 128);
    }

    #[test]
    fn compose_clip_skips_outside() {
        // 3x3 흰색 불투명 사각형을 캔버스 바로 밖에 절반 걸쳐 그린다 —
        // 캔버스는 유니온 바운딩(3x3)이고 벗어난 부분은 클립된다.
        let image: Vec<u8> = std::iter::repeat([255u8, 255, 255, 255])
            .flatten()
            .take(3 * 3 * 4)
            .collect();
        let layers = [LayerSource {
            rgba: &image,
            width: 3,
            height: 3,
            pos: Point::new(0, 0),
            alpha: 255,
        }];
        let out = compose(&layers).unwrap();
        assert_eq!((out.width, out.height), (3, 3));
        assert_eq!(out.rgba.len(), 9 * 4);
        assert!(out.rgba.chunks(4).all(|px| px == [255, 255, 255, 255]));
    }

    #[test]
    fn compose_layer_short_pixel_is_error() {
        let layers = [LayerSource {
            rgba: &[0u8],
            width: 2,
            height: 2,
            pos: Point::new(0, 0),
            alpha: 255,
        }];
        assert!(matches!(
            compose(&layers).unwrap_err(),
            ComposeError::PixelTooShort { .. }
        ));
    }
}
