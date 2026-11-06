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
    fn preserves_alpha_and_size() {
        let rgba = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let out = enlarge_2x(&rgba, 2, 2);
        assert_eq!(out.len(), 4 * 4 * 4);
        assert_eq!(&out[0..4], &[1, 2, 3, 4]);
        // 마지막 행은 원본 2행째(9..16)의 확대다.
        assert_eq!(&out[3 * 16..3 * 16 + 4], &[9, 10, 11, 12]);
    }
}
