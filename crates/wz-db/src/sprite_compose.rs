//! CPU 스프라이트 샘플링/합성 — `SpriteBatch.Draw`의 소프트웨어 등가.
//!
//! `AnimationGraphics.Draw`가 만드는 호출 인자(소스 사각형·목적 사각형·flip·
//! 블렌드)를 실제 픽셀에 적용한다. GDI+/wgpu 없이 결정적이며, 인자는
//! [`crate::draw_math`]에서 이미 C#과 일치한다.
//!
//! 지원 블렌드: `NonPremultiplied_Hidef`(straight alpha source-over)와
//! `Opaque`. 그 외 상태는 [`ComposeError::UnsupportedBlend`]로 거부한다.

use crate::draw_math::{BlendFactor, BlendFactors};

#[derive(Debug, PartialEq, Eq)]
pub enum ComposeError {
    BadSourceSize,
    BadRect,
    UnsupportedBlend,
}

impl std::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComposeError::BadSourceSize => write!(f, "source buffer does not match size"),
            ComposeError::BadRect => write!(f, "rect outside source"),
            ComposeError::UnsupportedBlend => write!(f, "unsupported blend factors"),
        }
    }
}

impl std::error::Error for ComposeError {}

fn is_opaque(blend: &BlendFactors) -> bool {
    blend.color_src == BlendFactor::One
        && blend.color_dst == BlendFactor::Zero
        && blend.alpha_src == BlendFactor::One
        && blend.alpha_dst == BlendFactor::Zero
}

fn is_straight_alpha(blend: &BlendFactors) -> bool {
    blend.color_src == BlendFactor::SourceAlpha
        && blend.color_dst == BlendFactor::InverseSourceAlpha
        && blend.alpha_src == BlendFactor::One
        && blend.alpha_dst == BlendFactor::InverseSourceAlpha
}

/// `AtlasRect` 만큼 잘라낸 RGBA. 사각형은 소스 안에 있어야 한다.
pub fn extract_rect(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    rect: (i32, i32, i32, i32),
) -> Result<(u32, u32, Vec<u8>), ComposeError> {
    if source.len() != (source_width * source_height * 4) as usize {
        return Err(ComposeError::BadSourceSize);
    }
    let (rx, ry, rw, rh) = rect;
    if rw <= 0
        || rh <= 0
        || rx < 0
        || ry < 0
        || rx as u32 + rw as u32 > source_width
        || ry as u32 + rh as u32 > source_height
    {
        return Err(ComposeError::BadRect);
    }
    let mut out = vec![0u8; (rw * rh * 4) as usize];
    for y in 0..rh as u32 {
        let src_row = ((ry as u32 + y) * source_width + rx as u32) * 4;
        let dst_row = y * rw as u32 * 4;
        out[dst_row as usize..(dst_row + rw as u32 * 4) as usize]
            .copy_from_slice(&source[src_row as usize..(src_row + rw as u32 * 4) as usize]);
    }
    Ok((rw as u32, rh as u32, out))
}

/// 스프라이트를 목적 사각형에 합성한다.
///
/// - `dest`는 `(x, y, w, h)`이고 크기가 다르면 최근접 이웃으로 샘플링한다.
/// - `flip_h`/`flip_v`는 스프라이트 내부 좌표를 뒤집는다.
/// - `alpha`는 `frame.A0`(0~255)로 소스 알파에 곱한다.
#[allow(clippy::too_many_arguments)]
pub fn compose_sprite(
    dst: &mut [u8],
    dst_width: u32,
    dst_height: u32,
    sprite: &[u8],
    sprite_width: u32,
    sprite_height: u32,
    dest: (i32, i32, i32, i32),
    flip_h: bool,
    flip_v: bool,
    alpha: u8,
    blend: &BlendFactors,
) -> Result<(), ComposeError> {
    if dst.len() != (dst_width * dst_height * 4) as usize
        || sprite.len() != (sprite_width * sprite_height * 4) as usize
    {
        return Err(ComposeError::BadSourceSize);
    }
    let opaque = is_opaque(blend);
    if !opaque && !is_straight_alpha(blend) {
        return Err(ComposeError::UnsupportedBlend);
    }
    let (dx, dy, dw, dh) = dest;
    if dw <= 0 || dh <= 0 {
        return Ok(());
    }

    for oy in 0..dh {
        let ty = dy + oy;
        if ty < 0 || ty >= dst_height as i32 {
            continue;
        }
        // 목적 y → 스프라이트 y (flip 반영)
        let sy = (oy as u32 * sprite_height) / dh as u32;
        let sy = if flip_v { sprite_height - 1 - sy } else { sy };
        for ox in 0..dw {
            let tx = dx + ox;
            if tx < 0 || tx >= dst_width as i32 {
                continue;
            }
            let sx = (ox as u32 * sprite_width) / dw as u32;
            let sx = if flip_h { sprite_width - 1 - sx } else { sx };
            let si = ((sy * sprite_width + sx) * 4) as usize;
            let src = [sprite[si], sprite[si + 1], sprite[si + 2], sprite[si + 3]];
            if src[3] == 0 {
                continue;
            }
            let di = ((ty as u32 * dst_width + tx as u32) * 4) as usize;
            if opaque {
                dst[di..di + 4].copy_from_slice(&src);
                continue;
            }
            let sa = (u32::from(src[3]) * u32::from(alpha) + 127) / 255;
            let da = u32::from(dst[di + 3]);
            let inv = 255 - sa;
            let out_a = (sa + (da * inv + 127) / 255).min(255);
            for c in 0..3 {
                let num = u32::from(src[c]) * sa + (u32::from(dst[di + c]) * da * inv + 127) / 255;
                dst[di + c] = ((num + out_a / 2) / out_a.max(1)).min(255) as u8;
            }
            dst[di + 3] = out_a as u8;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_math::{non_premultiplied_hidef, opaque_blend};

    #[test]
    fn extract_takes_sub_rect() {
        // 3x2, 각 픽셀 값 = index
        let mut src = vec![0u8; 3 * 2 * 4];
        for i in 0..6 {
            src[i * 4] = i as u8;
        }
        let (w, h, out) = extract_rect(&src, 3, 2, (1, 0, 2, 2)).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(out.len(), 2 * 2 * 4);
        assert_eq!((out[0], out[4], out[8], out[12]), (1, 2, 4, 5));
        assert!(extract_rect(&src, 3, 2, (2, 0, 2, 1)).is_err());
    }

    #[test]
    fn opaque_copies_and_flips() {
        let sprite: Vec<u8> = (0..2u8).flat_map(|v| [v, 0, 0, 255]).collect();
        let mut dst = vec![0u8; 2 * 2 * 4];
        compose_sprite(
            &mut dst,
            2,
            2,
            &sprite,
            2,
            1,
            (0, 0, 2, 1),
            true,
            false,
            255,
            &opaque_blend(),
        )
        .unwrap();
        assert_eq!((dst[0], dst[4]), (1, 0));
    }

    #[test]
    fn translates_by_destination_and_clips() {
        let sprite = vec![9u8, 9, 9, 255];
        let mut dst = vec![0u8; 3 * 3 * 4];
        compose_sprite(
            &mut dst,
            3,
            3,
            &sprite,
            1,
            1,
            (2, 2, 1, 1),
            false,
            false,
            255,
            &opaque_blend(),
        )
        .unwrap();
        let i = ((2 * 3 + 2) * 4) as usize;
        assert_eq!(&dst[i..i + 4], &[9, 9, 9, 255]);
        // 완전히 밖이면 아무 것도 쓰지 않는다.
        let mut dst = vec![0u8; 4];
        compose_sprite(
            &mut dst,
            1,
            1,
            &sprite,
            1,
            1,
            (5, 5, 1, 1),
            false,
            false,
            255,
            &opaque_blend(),
        )
        .unwrap();
        assert_eq!(dst, vec![0u8; 4]);
    }

    #[test]
    fn straight_alpha_blends_with_alpha_scale() {
        let sprite = vec![255u8, 0, 0, 128];
        let mut dst = vec![255u8, 255, 255, 255];
        compose_sprite(
            &mut dst,
            1,
            1,
            &sprite,
            1,
            1,
            (0, 0, 1, 1),
            false,
            false,
            255,
            &non_premultiplied_hidef(),
        )
        .unwrap();
        assert_eq!(dst[3], 255);
        assert!(dst[0] >= 250);
        assert!(dst[1] > 100 && dst[1] < 155);
        // alpha 0이면 변화 없음.
        let mut dst = vec![10u8, 20, 30, 40];
        compose_sprite(
            &mut dst,
            1,
            1,
            &sprite,
            1,
            1,
            (0, 0, 1, 1),
            false,
            false,
            0,
            &non_premultiplied_hidef(),
        )
        .unwrap();
        assert_eq!(dst, vec![10, 20, 30, 40]);
    }

    #[test]
    fn rejects_unregistered_blend() {
        let why = crate::draw_math::multiply_rgb();
        let mut dst = vec![0u8; 4];
        assert_eq!(
            compose_sprite(
                &mut dst,
                1,
                1,
                &[1, 2, 3, 255],
                1,
                1,
                (0, 0, 1, 1),
                false,
                false,
                255,
                &why
            ),
            Err(ComposeError::UnsupportedBlend)
        );
    }
}
