//! ItemTooltip/SkillTooltip이 쓰는 순수 픽셀 연산.
//!
//! 원본 `GearGraphics.EnlargeBitmap`(ItemTooltip 아이콘 확대)은 32bpp ARGB
//! 버퍼를 2배 최근접 이웃으로 늘린다. `Bitmap.LockBits`는 32bpp에서
//! stride == width*4라서 행이 연속이고, 알고리즘은 다음 두 단계다.
//!
//! 1. 원본 `(j, i)` 픽셀을 목적지 `(2j, 2i)`와 `(2j+1, 2i)`에 복사
//! 2. 목적지 행 `2i`를 행 `2i+1`로 통째로 복사
//!
//! 즉 바이트 단위 변환이며 색 공간/알파 합성과 무관하다.

/// RGBA8 버퍼를 2배로 확대한다. 반환 크기는 `(2w, 2h)`.
pub fn enlarge_2x(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let stride = w * 4;
    let dst_stride = stride * 2;
    let mut out = vec![0u8; dst_stride * h * 2];
    for i in 0..h {
        let src_row = &rgba[i * stride..(i + 1) * stride];
        let dst_row = 2 * i * dst_stride;
        for j in 0..w {
            let pixel = &src_row[j * 4..j * 4 + 4];
            let left = dst_row + 2 * j * 4;
            out[left..left + 4].copy_from_slice(pixel);
            out[left + 4..left + 8].copy_from_slice(pixel);
        }
        // 원본: 목적지 행 2i를 2i+1로 복사
        let (a, b) = out.split_at_mut(dst_row + dst_stride);
        let src = &a[dst_row..];
        b[..dst_stride].copy_from_slice(src);
    }
    out
}

/// 불투명(알파 255) 픽셀만 `dst`에 복사한다. 경계를 벗어난 픽셀은 건너뛴다.
///
/// 원본 `Graphics.DrawImage`는 알파 합성을 하지만, 여기서는 GDI+ 없이
/// 결정적으로 재현 가능한 **불투명 blit**만 지원한다. 반투명 픽셀은
/// 의도적으로 무시한다(문서 참고).
#[allow(clippy::too_many_arguments)]
pub fn blit_opaque_argb(
    dst: &mut [u8],
    dst_width: u32,
    dst_height: u32,
    src: &[u8],
    src_width: u32,
    src_height: u32,
    dx: i32,
    dy: i32,
) {
    for y in 0..src_height as i32 {
        let ty = dy + y;
        if ty < 0 || ty >= dst_height as i32 {
            continue;
        }
        for x in 0..src_width as i32 {
            let tx = dx + x;
            if tx < 0 || tx >= dst_width as i32 {
                continue;
            }
            let src_index = ((y as u32 * src_width + x as u32) * 4) as usize;
            let pixel = &src[src_index..src_index + 4];
            if pixel[3] != 255 {
                continue;
            }
            let dst_index = ((ty as u32 * dst_width + tx as u32) * 4) as usize;
            dst[dst_index..dst_index + 4].copy_from_slice(pixel);
        }
    }
}

/// ItemTooltip 아이콘 블록: 확대 아이콘을 배치 좌표에 불투명 blit한다.
///
/// 원본 `ItemTooltipRender22`의 좌표식:
/// `iconX + 6 + (1 - originX) * 2`, `picH + 6 + (33 - originY) * 2`.
pub struct IconBlock<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub origin: (i32, i32),
}

impl IconBlock<'_> {
    /// 배치 좌표.
    pub fn offset(&self, icon_x: i32, pic_h: i32) -> (i32, i32) {
        (
            icon_x + 6 + (1 - self.origin.0) * 2,
            pic_h + 6 + (33 - self.origin.1) * 2,
        )
    }
}

/// `canvas_width x canvas_height` 투명 ARGB 버퍼에 아이콘 블록을 그린다.
pub fn compose_icon_block(
    icon: &IconBlock<'_>,
    canvas_width: u32,
    canvas_height: u32,
    icon_x: i32,
    pic_h: i32,
) -> Vec<u8> {
    let enlarged = enlarge_2x(icon.rgba, icon.width, icon.height);
    let mut canvas = vec![0u8; (canvas_width * canvas_height * 4) as usize];
    let (dx, dy) = icon.offset(icon_x, pic_h);
    blit_opaque_argb(
        &mut canvas,
        canvas_width,
        canvas_height,
        &enlarged,
        icon.width * 2,
        icon.height * 2,
        dx,
        dy,
    );
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_each_pixel_and_row() {
        // 2x1: 빨강, 초록
        let rgba = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let out = enlarge_2x(&rgba, 2, 1);
        assert_eq!(out.len(), 4 * 2 * 4);
        assert_eq!(&out[0..4], &[255, 0, 0, 255]);
        assert_eq!(&out[4..8], &[255, 0, 0, 255]);
        assert_eq!(&out[8..12], &[0, 255, 0, 255]);
        assert_eq!(&out[12..16], &[0, 255, 0, 255]);
        // 두 번째 목적지 행은 첫 행의 복사
        assert_eq!(&out[16..32], &out[0..16]);
    }

    #[test]
    fn opaque_blit_skips_translucent_and_clips() {
        // 2x1 소스: 불투명 빨강, 반투명 초록
        let src = vec![255, 0, 0, 255, 0, 255, 0, 128];
        let mut dst = vec![0u8; 4 * 2 * 4];
        blit_opaque_argb(&mut dst, 4, 2, &src, 2, 1, 1, 0);
        assert_eq!(&dst[4..8], &[255, 0, 0, 255]);
        // 반투명 픽셀은 복사되지 않는다.
        assert_eq!(&dst[8..12], &[0, 0, 0, 0]);
        // 경계 밖은 무시된다.
        // 1x1 ARGB 캔버스
        let mut clipped = vec![0u8; 16];
        blit_opaque_argb(&mut clipped, 1, 1, &src, 2, 1, 3, 0);
        assert_eq!(clipped, vec![0u8; 16]);
    }

    #[test]
    fn icon_block_uses_itemtooltip_formula() {
        let rgba = vec![10, 20, 30, 255];
        let icon = IconBlock {
            rgba: &rgba,
            width: 1,
            height: 1,
            origin: (-3, 30),
        };
        assert_eq!(icon.offset(15, 7), (15 + 6 + 8, 7 + 6 + 6));
        let canvas = compose_icon_block(&icon, 40, 40, 15, 7);
        let index = ((19 * 40 + 29) * 4) as usize;
        assert_eq!(&canvas[index..index + 4], &[10, 20, 30, 255]);
    }

    #[test]
    fn preserves_alpha_and_size() {
        let rgba = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let out = enlarge_2x(&rgba, 2, 2);
        assert_eq!(out.len(), 4 * 4 * 4);
        assert_eq!(&out[0..4], &[1, 2, 3, 4]);
        // 마지막 행은 원본 2행째(9..16)의 확대다.
        assert_eq!(&out[3 * 16..3 * 16 + 4], &[9, 10, 11, 12]);
    }
}
