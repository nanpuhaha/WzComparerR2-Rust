//! WZ 텍스처 포맷 정의 (원본 `WzComparerR2.WzLib/Wz_Png.cs:375-392`).
//!
//! Phase 2 전에 포맷 열거와 **레이아웃 계산 규칙**만 이식한다. 실제 픽셀 디코딩은
//! 다음 스텝.
//!
//! # `Wz_TextureFormat` — 값이 곧 디스크립터
//!
//! ```text
//! Unknown = 0      ARGB4444 = 1      ARGB8888 = 2      ARGB1555 = 257
//! RGB565  = 513    R16      = 769    DXT3      = 1026  DXT5      = 2050
//! A8      = 2304   RGBA1010102 = 2562  DXT1 = 4097   BC7 = 4098
//! RGBA32Float = 4100
//! ```
//!
//! 이 숫자를 정확히 보존해야 한다. WZ 파일에 그대로 기록돼 있기 때문이다.

/// WZ 텍스처 포맷.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum TextureFormat {
    Unknown = 0,
    /// 2 bytes/px ("form 1")
    Argb4444 = 1,
    /// 4 bytes/px ("form 2")
    Argb8888 = 2,
    /// 2 bytes/px
    Argb1555 = 257,
    /// 2 bytes/px. `Scale == 4` 면 4× 업스케일 전 지원 (원본 legacy form 517).
    Rgb565 = 513,
    /// 2 bytes/px (KMST1197+)
    R16 = 769,
    /// 16 bytes / 4×4 블록
    Dxt3 = 1026,
    /// 16 bytes / 4×4 블록
    Dxt5 = 2050,
    /// 1 byte/px (KMST1186)
    A8 = 2304,
    /// 4 bytes/px
    Rgba1010102 = 2562,
    /// 8 bytes / 4×4 블록
    Dxt1 = 4097,
    /// 16 bytes / 4×4 블록
    Bc7 = 4098,
    /// 16 bytes/px
    Rgba32Float = 4100,
}

impl TextureFormat {
    /// 숫자에서 변환. 알 수 없는 값은 `Unknown`.
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Argb4444,
            2 => Self::Argb8888,
            257 => Self::Argb1555,
            513 => Self::Rgb565,
            769 => Self::R16,
            1026 => Self::Dxt3,
            2050 => Self::Dxt5,
            2304 => Self::A8,
            2562 => Self::Rgba1010102,
            4097 => Self::Dxt1,
            4098 => Self::Bc7,
            4100 => Self::Rgba32Float,
            _ => Self::Unknown,
        }
    }

    /// 블록 압축 포맷인가 (4×4 단위).
    pub fn is_block_compressed(&self) -> bool {
        matches!(self, Self::Dxt1 | Self::Dxt3 | Self::Dxt5 | Self::Bc7)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Argb4444 => "ARGB4444",
            Self::Argb8888 => "ARGB8888",
            Self::Argb1555 => "ARGB1555",
            Self::Rgb565 => "RGB565",
            Self::R16 => "R16",
            Self::Dxt3 => "DXT3",
            Self::Dxt5 => "DXT5",
            Self::A8 => "A8",
            Self::Rgba1010102 => "RGBA1010102",
            Self::Dxt1 => "DXT1",
            Self::Bc7 => "BC7",
            Self::Rgba32Float => "RGBA32Float",
        }
    }
}

/// `Wz_Png.GetUncompressedDataSize(format, w, h)` (원본 `:346-372`)
///
/// # BC7 함정 — 반드시 보존할 것
///
/// ```text
/// BC7 => width * (height & ~3)
/// ```
///
/// 원본 주석 (`:361-362`): *"TMST v1272, width and height for BC7 format are not
/// always multiples of 4, NX will add row padding and discard the tail rows."*
/// 즉 WZ 파일에 저장된 BC7 높이는 4의 배수가 아닐 수 있고, 디코더는
/// **4로 내림한 높이** 만큼만 해석한다. 높이를 그대로 쓰면 데이터가 어긋난다.
pub fn uncompressed_data_size(format: TextureFormat, w: u32, h: u32) -> Option<usize> {
    let (pw, ph) = (w as usize, h as usize);
    let bytes = match format {
        TextureFormat::Unknown => return None,
        // 2 bytes/px
        TextureFormat::Argb4444
        | TextureFormat::Argb1555
        | TextureFormat::Rgb565
        | TextureFormat::R16 => pw.checked_mul(ph)?.checked_mul(2)?,
        // 4 bytes/px
        TextureFormat::Argb8888 | TextureFormat::Rgba1010102 => {
            pw.checked_mul(ph)?.checked_mul(4)?
        }
        // 1 byte/px
        TextureFormat::A8 => pw.checked_mul(ph)?,
        // 16 bytes / 4×4 블록
        TextureFormat::Dxt3 | TextureFormat::Dxt5 | TextureFormat::Bc7 => pw
            .checked_add(3)?
            .checked_div(4)?
            .checked_mul(ph.checked_add(3)?.checked_div(4)?)?
            .checked_mul(16)?,
        // 8 bytes / 4×4 블록
        TextureFormat::Dxt1 => pw
            .checked_add(3)?
            .checked_div(4)?
            .checked_mul(ph.checked_add(3)?.checked_div(4)?)?
            .checked_mul(8)?,
        // 16 bytes/px
        TextureFormat::Rgba32Float => pw.checked_mul(ph)?.checked_mul(16)?,
    };
    Some(bytes)
}

/// BC7 전용 크기 규칙. `uncompressed_data_size` 가 따로 처리하지만,
/// 여기서 한 번 더 검증한다 (원본 주석을 그대로 옮긴 것).
pub fn bc7_data_size(width: u32, height: u32) -> Option<usize> {
    (width as usize).checked_mul((height & !3) as usize)
}

/// `GetUncompressedDataSize(format, scale, w, h)` (원본 `:332-344`)
///
/// scale > 1 이면 폭/높이가 scale 의 배수여야 하고, 나누어 진다.
///
/// # 함정: 이 오버로드는 `scale` 를 그대로 받는다
///
/// 원본은 여기서 `Scale` 원본값을 받는다. 하지만 **디코더가 실제로 호출하는
/// 경로는 `GetRawDataSizePerPage()`** 이고, 그것은 `ActualScale`(`1 << scale`)를
/// 넘긴다 (`Wz_Png.cs:89`):
/// ```csharp
/// GetRawDataSizePerPage() => GetUncompressedDataSize(Format, ActualScale, Width, Height);
/// ```
/// 즉 실제 데이터 크기 계산의 제수는 `1 << scale` 다.
/// [`TextureLayout::for_png`] 이 그 경로를 구현한다.
pub fn uncompressed_data_size_scaled(
    format: TextureFormat,
    scale: u32,
    w: u32,
    h: u32,
) -> Option<usize> {
    if scale <= 1 {
        return uncompressed_data_size(format, w, h);
    }
    if !w.is_multiple_of(scale) || !h.is_multiple_of(scale) {
        return None;
    }
    uncompressed_data_size(format, w / scale, h / scale)
}

/// 실제 스케일 (원본 `Wz_Png.ActualScale`, `:66`)
/// `Scale > 0` 이면 `1 << Scale`, 아니면 1.
pub fn actual_scale(scale: u32) -> u32 {
    if scale > 0 {
        1u32.checked_shl(scale).unwrap_or(1)
    } else {
        1
    }
}

/// 실제 페이지 수 (원본 `Wz_Png.ActualPages`, `:70`)
pub fn actual_pages(pages: u32) -> u32 {
    if pages > 0 {
        pages
    } else {
        1
    }
}

/// 디코딩된 이미지의 픽셀 레이아웃.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureLayout {
    /// 압축 해제 후 **페이지당** 원시 바이트 수.
    ///
    /// 제수는 `ActualScale` 다 (원본 `GetRawDataSizePerPage`, `Wz_Png.cs:89`).
    pub bytes_per_page: usize,
    /// 출력 비트맵 폭.
    pub decodable_width: u32,
    /// 출력 비트맵 높이.
    pub decodable_height: u32,
}

impl TextureLayout {
    /// 원본 `Wz_Png` 디코딩 경로를 그대로 모사한다.
    ///
    /// # 원본의 스케일 제약 (모두 예외를 던진다)
    ///
    /// | 포맷 | 허용 `ActualScale` | 비고 |
    /// |---|---|---|
    /// | `BC7` / `DXT3` / `DXT5` | **1만** | 그 외는 `throw` (`:248`, `:286`) |
    /// | `RGB565` | 1 또는 **16** | 16 이면 4× 업스케일 (`:227`, `:234`) |
    /// | 그 외 | 1만 | (`:220` 등) |
    ///
    /// # BC7 높이 내림
    ///
    /// `new Bitmap(Width & ~3, Height & ~3, ...)` (`:288`) — TMST v1272 가
    /// 행 패딩을 넣고 꼬리 행을 버리기 때문이다. 높이를 그대로 쓰면 데이터가 어긋난다.
    pub fn for_png(format: TextureFormat, scale: u32, w: u32, h: u32) -> Option<Self> {
        let a_scale = actual_scale(scale);

        // 스케일 제약 검증 (원본의 throw 를 `None` 으로)
        let scale_ok = match format {
            TextureFormat::Bc7 | TextureFormat::Dxt3 | TextureFormat::Dxt5 => a_scale == 1,
            TextureFormat::Rgb565 => a_scale == 1 || a_scale == 16,
            _ => a_scale == 1,
        };
        if !scale_ok {
            return None;
        }

        // 원시 데이터 크기: 제수는 ActualScale
        if !w.is_multiple_of(a_scale) || !h.is_multiple_of(a_scale) {
            return None;
        }
        let bytes_per_page = uncompressed_data_size(format, w / a_scale, h / a_scale)?;

        // 출력 비트맵 크기
        let (dw, dh) = if format == TextureFormat::Bc7 {
            (w & !3, h & !3)
        } else {
            // RGB565 scale 16 은 원시 데이터를 4× 업스케일하므로 출력은 원본 크기.
            (w, h)
        };

        Some(Self {
            bytes_per_page,
            decodable_width: dw,
            decodable_height: dh,
        })
    }
}

/// 이전 API 호환용 별칭.
pub fn layout_for(format: TextureFormat, scale: u32, w: u32, h: u32) -> Option<TextureLayout> {
    TextureLayout::for_png(format, scale, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_codes_are_exact() {
        // 이 값들이 WZ 파일 바이트에 기록된다. 틀리면 모든 파일이 깨진다.
        assert_eq!(TextureFormat::Unknown as u32, 0);
        assert_eq!(TextureFormat::Argb4444 as u32, 1);
        assert_eq!(TextureFormat::Argb8888 as u32, 2);
        assert_eq!(TextureFormat::Argb1555 as u32, 257);
        assert_eq!(TextureFormat::Rgb565 as u32, 513);
        assert_eq!(TextureFormat::R16 as u32, 769);
        assert_eq!(TextureFormat::Dxt3 as u32, 1026);
        assert_eq!(TextureFormat::Dxt5 as u32, 2050);
        assert_eq!(TextureFormat::A8 as u32, 2304);
        assert_eq!(TextureFormat::Rgba1010102 as u32, 2562);
        assert_eq!(TextureFormat::Dxt1 as u32, 4097);
        assert_eq!(TextureFormat::Bc7 as u32, 4098);
        assert_eq!(TextureFormat::Rgba32Float as u32, 4100);
    }

    #[test]
    fn format_roundtrip_from_u32() {
        for f in [
            TextureFormat::Unknown,
            TextureFormat::Argb4444,
            TextureFormat::Argb8888,
            TextureFormat::Argb1555,
            TextureFormat::Rgb565,
            TextureFormat::R16,
            TextureFormat::Dxt3,
            TextureFormat::Dxt5,
            TextureFormat::A8,
            TextureFormat::Rgba1010102,
            TextureFormat::Dxt1,
            TextureFormat::Bc7,
            TextureFormat::Rgba32Float,
        ] {
            assert_eq!(TextureFormat::from_u32(f as u32), f, "{:?}", f.name());
        }
    }

    #[test]
    fn unknown_codes_degrade_to_unknown() {
        for v in [3u32, 100, 1000, 4096, 4099, u32::MAX] {
            assert_eq!(
                TextureFormat::from_u32(v),
                TextureFormat::Unknown,
                "코드 {v}"
            );
        }
    }

    #[test]
    fn uncompressed_sizes_match_reference() {
        // 원본 Wz_Png.GetUncompressedDataSize 를 손으로 옮긴 기대값
        assert_eq!(
            uncompressed_data_size(TextureFormat::Argb8888, 4, 4),
            Some(4 * 4 * 4)
        );
        assert_eq!(
            uncompressed_data_size(TextureFormat::Argb4444, 10, 10),
            Some(200)
        );
        assert_eq!(uncompressed_data_size(TextureFormat::A8, 8, 8), Some(64));
        assert_eq!(
            uncompressed_data_size(TextureFormat::Rgba32Float, 2, 2),
            Some(2 * 2 * 16)
        );
        // 블록 압축: ceil(w/4) * ceil(h/4) * bytes
        assert_eq!(uncompressed_data_size(TextureFormat::Dxt1, 4, 4), Some(8));
        assert_eq!(
            uncompressed_data_size(TextureFormat::Dxt5, 8, 8),
            Some(2 * 2 * 16)
        );
        // 4 의 배수가 아니면 올림
        assert_eq!(
            uncompressed_data_size(TextureFormat::Dxt1, 5, 5),
            Some(2 * 2 * 8)
        );
        assert_eq!(uncompressed_data_size(TextureFormat::Unknown, 4, 4), None);
    }

    #[test]
    fn bc7_uses_floored_height() {
        // 함정 검증: 원본은 BC7 에만 `h & ~3` 을 적용한다
        assert_eq!(bc7_data_size(4, 8), Some(4 * 8));
        assert_eq!(bc7_data_size(4, 6), Some(4 * 4), "높이 6 → 4로 내림");
        assert_eq!(bc7_data_size(4, 7), Some(4 * 4));
        assert_eq!(bc7_data_size(8, 127), Some(8 * 124));

        // DXT5 는 ceil 규칙을 그대로 쓴다 (내림이 아님)
        assert_eq!(
            uncompressed_data_size(TextureFormat::Dxt5, 4, 6),
            Some(2 * 16),
            "DXT5 는 ceil 규칙"
        );
    }

    #[test]
    fn scale_divides_dimensions() {
        assert_eq!(
            uncompressed_data_size_scaled(TextureFormat::Argb8888, 1, 16, 16),
            Some(16 * 16 * 4)
        );
        // scale 2 → 8x8 로 축소
        assert_eq!(
            uncompressed_data_size_scaled(TextureFormat::Argb8888, 2, 16, 16),
            Some(8 * 8 * 4)
        );
        // 배수가 아니면 None
        assert_eq!(
            uncompressed_data_size_scaled(TextureFormat::Argb8888, 2, 15, 16),
            None,
            "15 는 2 의 배수가 아니다"
        );
    }

    #[test]
    fn actual_scale_and_pages() {
        assert_eq!(actual_scale(0), 1);
        assert_eq!(actual_scale(1), 2);
        assert_eq!(actual_scale(2), 4);
        assert_eq!(
            actual_scale(4),
            16,
            "RGB565 scale 16 경로 (원본 legacy form 517)"
        );

        assert_eq!(actual_pages(0), 1);
        assert_eq!(actual_pages(1), 1);
        assert_eq!(actual_pages(3), 3);
    }

    #[test]
    fn layout_floors_bc7_dimensions() {
        // 원본 `new Bitmap(Width & ~3, Height & ~3, ...)` (Wz_Png.cs:288)
        let l = layout_for(TextureFormat::Bc7, 0, 8, 6).unwrap();
        assert_eq!(l.decodable_width, 8);
        assert_eq!(l.decodable_height, 4, "BC7 높이는 4 배수로 내림");

        let l2 = layout_for(TextureFormat::Argb8888, 0, 8, 6).unwrap();
        assert_eq!(l2.decodable_height, 6, "BC7 아니면 내림하지 않는다");
    }

    #[test]
    fn layout_rejects_unsupported_scale_combinations() {
        // 원본은 BC7/DXT3/DXT5 에서 scale != 1 이면 throw (Wz_Png.cs:248, 286)
        assert!(layout_for(TextureFormat::Bc7, 1, 16, 16).is_none());
        assert!(layout_for(TextureFormat::Dxt3, 1, 16, 16).is_none());
        assert!(layout_for(TextureFormat::Dxt5, 1, 16, 16).is_none());
        assert!(layout_for(TextureFormat::Dxt1, 1, 16, 16).is_none());

        // 그 외 포맷도 scale 1 만 지원
        assert!(layout_for(TextureFormat::Argb8888, 1, 16, 16).is_none());
        assert!(layout_for(TextureFormat::A8, 2, 16, 16).is_none());
    }

    #[test]
    fn rgb565_allows_scale_16() {
        // 원본 `case RGB565 when ActualScale == 1 || ActualScale == 16` (Wz_Png.cs:227)
        // scale 4 → ActualScale 16
        let l = layout_for(TextureFormat::Rgb565, 4, 16, 16).unwrap();
        // 원시 데이터는 1/16 크기 (16/16 = 1)
        assert_eq!(l.bytes_per_page, 2);
        // 출력은 원본 크기 (4× 업스케일)
        assert_eq!(l.decodable_width, 16);
        assert_eq!(l.decodable_height, 16);

        // scale 2 → ActualScale 4 는 미지원
        assert!(layout_for(TextureFormat::Rgb565, 2, 16, 16).is_none());
    }

    #[test]
    fn layout_uses_actual_scale_as_divisor() {
        // 함정 검증: 제수는 Scale 가 아니라 ActualScale(=1<<Scale) 다.
        // (원본 GetRawDataSizePerPage 가 ActualScale 를 넘긴다 — Wz_Png.cs:89)
        let l = layout_for(TextureFormat::Rgb565, 4, 64, 64).unwrap();
        // 64/16 = 4 → 4*4*2 = 32 바이트 (Scale=4 로 나누면 64*64*2=8192 가 된다)
        assert_eq!(l.bytes_per_page, 4 * 4 * 2);
    }

    #[test]
    fn scaled_overload_uses_raw_scale() {
        // C# 오버로드 자체는 Scale 를 그대로 받는다 (Wz_Png.cs:332)
        assert_eq!(
            uncompressed_data_size_scaled(TextureFormat::Argb8888, 2, 16, 16),
            Some(8 * 8 * 4)
        );
        // 이 경로는 디코더가 호출하지 않는다 — 레이아웃 계산과 구분된다.
        assert_ne!(
            uncompressed_data_size_scaled(TextureFormat::Argb8888, 1, 16, 16),
            layout_for(TextureFormat::Argb8888, 1, 16, 16).map(|l| l.bytes_per_page)
        );
    }

    #[test]
    fn layout_rejects_non_divisible_dimensions() {
        // 16 은 ActualScale 4 의 배수가 아니다
        assert!(layout_for(TextureFormat::Rgb565, 4, 30, 30).is_none());
        assert!(layout_for(TextureFormat::Rgb565, 4, 32, 32).is_some());
    }

    #[test]
    fn block_compressed_flag() {
        assert!(TextureFormat::Dxt1.is_block_compressed());
        assert!(TextureFormat::Dxt3.is_block_compressed());
        assert!(TextureFormat::Dxt5.is_block_compressed());
        assert!(TextureFormat::Bc7.is_block_compressed());
        assert!(!TextureFormat::Argb8888.is_block_compressed());
        assert!(!TextureFormat::A8.is_block_compressed());
    }

    #[test]
    fn overflow_is_none_not_panic() {
        // u32::MAX 폭/높이는 오버플로해야 한다. panic 이면 안 된다.
        assert_eq!(
            uncompressed_data_size(TextureFormat::Argb8888, u32::MAX, u32::MAX),
            None
        );
        // usize 는 64비트에서 1.8e19 이므로 이건 실제로 넘지 않는다.
        assert_eq!(
            uncompressed_data_size(TextureFormat::Rgba32Float, 1_000_000, 1_000_000),
            Some(1_000_000 * 1_000_000 * 16),
            "1e6 x 1e6 x 16 = 1.6e13 은 usize 에 들어간다"
        );
        // 32비트 usize 환경에서는 오버플로해야 한다 (checked_mul 사용 확인)
        #[cfg(target_pointer_width = "32")]
        assert_eq!(
            uncompressed_data_size(TextureFormat::Rgba32Float, 1_000_000, 1_000_000),
            None
        );
    }
}
