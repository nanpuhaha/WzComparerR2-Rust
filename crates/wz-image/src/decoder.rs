//! WZ 텍스처 데이터 디컴프레션 및 RGBA8 픽셀 디코더.
//!
//! 원본 `WzComparerR2.WzLib/Wz_Png.cs` 기반의 픽셀 언패킹 구현.

use std::io::Read;
use flate2::read::ZlibDecoder;
use wz_error::{Result, WzImageError};

use crate::format::TextureFormat;

/// ZLIB 스트림을 압축 해제한다.
pub fn decompress_zlib(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(compressed);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| WzImageError::Inflate {
            offset: 0,
            source: e,
        })?;
    Ok(decompressed)
}

/// 텍스처 디코더 인터페이스 구조체.
pub struct TextureDecoder;

impl TextureDecoder {
    /// 원시 픽셀 데이터를 표준 RGBA8 (4 바이트/픽셀, [R, G, B, A]) 버퍼로 변환한다.
    #[inline]
    pub fn decode_rgba8(
        format: TextureFormat,
        width: u32,
        height: u32,
        raw: &[u8],
    ) -> Result<Vec<u8>> {
        decode_to_rgba8(format, width, height, raw)
    }

    /// ZLIB 스트림을 압축 해제한다.
    #[inline]
    pub fn decompress_zlib(compressed: &[u8]) -> Result<Vec<u8>> {
        decompress_zlib(compressed)
    }
}

/// 원시 픽셀 데이터를 표준 RGBA8 (4 바이트/픽셀, [R, G, B, A]) 버퍼로 변환한다.
pub fn decode_to_rgba8(
    format: TextureFormat,
    width: u32,
    height: u32,
    raw: &[u8],
) -> Result<Vec<u8>> {
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| WzImageError::SizeOverflow {
            w: width,
            h: height,
            format: format as u32,
            scale: 1,
        })?;
    let mut output = vec![0u8; pixel_count * 4];

    match format {
        TextureFormat::Argb8888 => {
            // 원본 C# Bitmap 메모리: B, G, R, A 순서
            if raw.len() < pixel_count * 4 {
                return Err(WzImageError::SizeOverflow {
                    w: width,
                    h: height,
                    format: format as u32,
                    scale: 1,
                }.into());
            }
            for i in 0..pixel_count {
                let src = i * 4;
                let dst = i * 4;
                let b = raw[src];
                let g = raw[src + 1];
                let r = raw[src + 2];
                let a = raw[src + 3];
                output[dst] = r;
                output[dst + 1] = g;
                output[dst + 2] = b;
                output[dst + 3] = a;
            }
        }
        TextureFormat::Argb4444 => {
            // 2 바이트/픽셀: [B:4 G:4, R:4 A:4] (리틀 엔디안)
            if raw.len() < pixel_count * 2 {
                return Err(WzImageError::SizeOverflow {
                    w: width,
                    h: height,
                    format: format as u32,
                    scale: 1,
                }.into());
            }
            for i in 0..pixel_count {
                let src = i * 2;
                let dst = i * 4;
                let low = raw[src];
                let high = raw[src + 1];

                let b = low & 0x0F;
                let g = (low >> 4) & 0x0F;
                let r = high & 0x0F;
                let a = (high >> 4) & 0x0F;

                output[dst] = (r << 4) | r;
                output[dst + 1] = (g << 4) | g;
                output[dst + 2] = (b << 4) | b;
                output[dst + 3] = (a << 4) | a;
            }
        }
        TextureFormat::Argb1555 => {
            // 1비트 A, 5비트 R, 5비트 G, 5비트 B
            if raw.len() < pixel_count * 2 {
                return Err(WzImageError::SizeOverflow {
                    w: width,
                    h: height,
                    format: format as u32,
                    scale: 1,
                }.into());
            }
            for i in 0..pixel_count {
                let src = i * 2;
                let dst = i * 4;
                let val = u16::from_le_bytes([raw[src], raw[src + 1]]);

                let b = (val & 0x1F) as u8;
                let g = ((val >> 5) & 0x1F) as u8;
                let r = ((val >> 10) & 0x1F) as u8;
                let a = if (val & 0x8000) != 0 { 255 } else { 0 };

                output[dst] = (r << 3) | (r >> 2);
                output[dst + 1] = (g << 3) | (g >> 2);
                output[dst + 2] = (b << 3) | (b >> 2);
                output[dst + 3] = a;
            }
        }
        TextureFormat::Rgb565 => {
            // 5비트 R, 6비트 G, 5비트 B, 알파=255
            if raw.len() < pixel_count * 2 {
                return Err(WzImageError::SizeOverflow {
                    w: width,
                    h: height,
                    format: format as u32,
                    scale: 1,
                }.into());
            }
            for i in 0..pixel_count {
                let src = i * 2;
                let dst = i * 4;
                let val = u16::from_le_bytes([raw[src], raw[src + 1]]);

                let b = (val & 0x1F) as u8;
                let g = ((val >> 5) & 0x3F) as u8;
                let r = ((val >> 11) & 0x1F) as u8;

                output[dst] = (r << 3) | (r >> 2);
                output[dst + 1] = (g << 2) | (g >> 4);
                output[dst + 2] = (b << 3) | (b >> 2);
                output[dst + 3] = 255;
            }
        }
        TextureFormat::A8 => {
            // 1 바이트/픽셀: 흰색 + A채널
            if raw.len() < pixel_count {
                return Err(WzImageError::SizeOverflow {
                    w: width,
                    h: height,
                    format: format as u32,
                    scale: 1,
                }.into());
            }
            for i in 0..pixel_count {
                let dst = i * 4;
                let a = raw[i];
                output[dst] = 255;
                output[dst + 1] = 255;
                output[dst + 2] = 255;
                output[dst + 3] = a;
            }
        }
        _ => {
            return Err(WzImageError::UnsupportedFormat(
                format.name().to_string(),
                format as u32,
            ).into());
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argb4444_decoding() {
        // [B:0xF, G:0x0], [R:0xA, A:0xF] => R=0xAA, G=0x00, B=0xFF, A=0xFF
        let raw = vec![0x0F, 0xFA];
        let rgba = decode_to_rgba8(TextureFormat::Argb4444, 1, 1, &raw).expect("decode");
        assert_eq!(rgba, vec![0xAA, 0x00, 0xFF, 0xFF]);
    }

    #[test]
    fn test_argb8888_decoding() {
        // B, G, R, A = 10, 20, 30, 40
        let raw = vec![10, 20, 30, 40];
        let rgba = decode_to_rgba8(TextureFormat::Argb8888, 1, 1, &raw).expect("decode");
        assert_eq!(rgba, vec![30, 20, 10, 40]);
    }

    #[test]
    fn test_rgb565_decoding() {
        // R=31(max), G=63(max), B=31(max) => White (val = 0xFFFF)
        let raw = vec![0xFF, 0xFF];
        let rgba = decode_to_rgba8(TextureFormat::Rgb565, 1, 1, &raw).expect("decode");
        assert_eq!(rgba, vec![255, 255, 255, 255]);
    }

    #[test]
    fn test_a8_decoding() {
        let raw = vec![128];
        let rgba = decode_to_rgba8(TextureFormat::A8, 1, 1, &raw).expect("decode");
        assert_eq!(rgba, vec![255, 255, 255, 128]);
    }
}
