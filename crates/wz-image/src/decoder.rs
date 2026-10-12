//! WZ 텍스처 데이터 디컴프레션, RGBA8 픽셀 디코더 및 무손실 PNG 인코더.
//!
//! 원본 `WzComparerR2.WzLib/Wz_Png.cs` 및 `Utilities/ImageCodec.cs` 기반의 픽셀 언패킹 구현.

use std::io::{Read, Write};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use wz_error::{Result, WzImageError};

use crate::format::{uncompressed_data_size, TextureFormat};

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

/// RGBA8 픽셀 버퍼(`width * height * 4` 바이트)를 표준 무손실 PNG 바이트 스트림으로 인코딩한다.
pub fn encode_png_rgba8(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    // 1. PNG 시그니처 (8 바이트)
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let write_chunk = |out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(chunk_type);
        out.extend_from_slice(data);
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(chunk_type);
        hasher.update(data);
        out.extend_from_slice(&hasher.finalize().to_be_bytes());
    };

    // 2. IHDR 청크 (13 바이트: width, height, bit_depth=8, color_type=6(RGBA), comp=0, filter=0, interlace=0)
    let mut ihdr = [0u8; 13];
    ihdr[0..4].copy_from_slice(&width.to_be_bytes());
    ihdr[4..8].copy_from_slice(&height.to_be_bytes());
    ihdr[8] = 8; // bit depth: 8
    ihdr[9] = 6; // color type: 6 (Truecolor with alpha, RGBA)
    ihdr[10] = 0;
    ihdr[11] = 0;
    ihdr[12] = 0;
    write_chunk(&mut out, b"IHDR", &ihdr);

    // 3. IDAT 청크 (행마다 필터 바이트 0(None) + RGBA 행 데이터 ZLIB 압축)
    let row_bytes = (width as usize) * 4;
    let mut raw_scanlines = Vec::with_capacity((row_bytes + 1) * (height as usize));
    for y in 0..(height as usize) {
        raw_scanlines.push(0u8); // Filter type 0 (None)
        let start = y * row_bytes;
        let end = (start + row_bytes).min(rgba.len());
        if start < end {
            raw_scanlines.extend_from_slice(&rgba[start..end]);
            if end - start < row_bytes {
                raw_scanlines.resize(raw_scanlines.len() + (row_bytes - (end - start)), 0);
            }
        } else {
            raw_scanlines.resize(raw_scanlines.len() + row_bytes, 0);
        }
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    let _ = encoder.write_all(&raw_scanlines);
    let idat_data = encoder.finish().unwrap_or_default();
    write_chunk(&mut out, b"IDAT", &idat_data);

    // 4. IEND 청크
    write_chunk(&mut out, b"IEND", &[]);

    out
}

fn rgb565_to_rgba(val: u16) -> [u8; 4] {
    let b = (val & 0x1F) as u8;
    let g = ((val >> 5) & 0x3F) as u8;
    let r = ((val >> 11) & 0x1F) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
        255,
    ]
}

fn expand_dxt_color_table(c0: u16, c1: u16) -> [[u8; 4]; 4] {
    let col0 = rgb565_to_rgba(c0);
    let col1 = rgb565_to_rgba(c1);
    if c0 > c1 {
        let col2 = [
            ((col0[0] as u16 * 2 + col1[0] as u16 + 1) / 3) as u8,
            ((col0[1] as u16 * 2 + col1[1] as u16 + 1) / 3) as u8,
            ((col0[2] as u16 * 2 + col1[2] as u16 + 1) / 3) as u8,
            255,
        ];
        let col3 = [
            ((col0[0] as u16 + col1[0] as u16 * 2 + 1) / 3) as u8,
            ((col0[1] as u16 + col1[1] as u16 * 2 + 1) / 3) as u8,
            ((col0[2] as u16 + col1[2] as u16 * 2 + 1) / 3) as u8,
            255,
        ];
        [col0, col1, col2, col3]
    } else {
        let col2 = [
            ((col0[0] as u16 + col1[0] as u16) / 2) as u8,
            ((col0[1] as u16 + col1[1] as u16) / 2) as u8,
            ((col0[2] as u16 + col1[2] as u16) / 2) as u8,
            255,
        ];
        [col0, col1, col2, [0, 0, 0, 255]]
    }
}

fn expand_dxt_color_indices(bytes: &[u8]) -> [usize; 16] {
    let mut indices = [0usize; 16];
    for j in 0..4 {
        let b = bytes[j];
        indices[j * 4] = (b & 0x03) as usize;
        indices[j * 4 + 1] = ((b >> 2) & 0x03) as usize;
        indices[j * 4 + 2] = ((b >> 4) & 0x03) as usize;
        indices[j * 4 + 3] = ((b >> 6) & 0x03) as usize;
    }
    indices
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
        .ok_or(WzImageError::SizeOverflow {
            w: width,
            h: height,
            format: format as u32,
            scale: 1,
        })?;
    let expected_raw = uncompressed_data_size(format, width, height).ok_or(
        WzImageError::UnsupportedFormat(format.name().to_string(), format as u32),
    )?;
    if raw.len() < expected_raw {
        return Err(WzImageError::SizeOverflow {
            w: width,
            h: height,
            format: format as u32,
            scale: 1,
        }
        .into());
    }

    let mut output = vec![0u8; pixel_count * 4];

    match format {
        TextureFormat::Argb8888 => {
            // 원본 C# Bitmap 메모리: B, G, R, A 순서
            for i in 0..pixel_count {
                let src = i * 4;
                let dst = i * 4;
                output[dst] = raw[src + 2];
                output[dst + 1] = raw[src + 1];
                output[dst + 2] = raw[src];
                output[dst + 3] = raw[src + 3];
            }
        }
        TextureFormat::Argb4444 => {
            // 2 바이트/픽셀: [B:4 G:4, R:4 A:4] (리틀 엔디안)
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
            for i in 0..pixel_count {
                let src = i * 2;
                let dst = i * 4;
                let val = u16::from_le_bytes([raw[src], raw[src + 1]]);
                let rgba = rgb565_to_rgba(val);
                output[dst..dst + 4].copy_from_slice(&rgba);
            }
        }
        TextureFormat::R16 => {
            // 16비트 R 채널 -> 상위 8비트를 R 채널로 매핑, G=0, B=0, A=255
            for i in 0..pixel_count {
                let src = i * 2;
                let dst = i * 4;
                let r16 = u16::from_le_bytes([raw[src], raw[src + 1]]);
                output[dst] = (r16 >> 8) as u8;
                output[dst + 1] = 0;
                output[dst + 2] = 0;
                output[dst + 3] = 255;
            }
        }
        TextureFormat::A8 => {
            // 1 바이트/픽셀: 흰색 + A채널
            for (i, &a) in raw.iter().enumerate().take(pixel_count) {
                let dst = i * 4;
                output[dst] = 255;
                output[dst + 1] = 255;
                output[dst + 2] = 255;
                output[dst + 3] = a;
            }
        }
        TextureFormat::Rgba1010102 => {
            // ImageCodec.R10G10B10A2ToBGRA32: r=bits[0..10], g=bits[10..20], b=bits[20..30], a=bits[30..32]
            for i in 0..pixel_count {
                let src = i * 4;
                let dst = i * 4;
                let px = u32::from_le_bytes([raw[src], raw[src + 1], raw[src + 2], raw[src + 3]]);
                let r8 = ((px & 0x3FF) >> 2) as u8;
                let g8 = (((px >> 10) & 0x3FF) >> 2) as u8;
                let b8 = (((px >> 20) & 0x3FF) >> 2) as u8;
                let a8 = (((px >> 30) & 0x03) * 85) as u8;
                output[dst] = r8;
                output[dst + 1] = g8;
                output[dst + 2] = b8;
                output[dst + 3] = a8;
            }
        }
        TextureFormat::Rgba32Float => {
            // 16 바이트/픽셀: [f32; 4] (R, G, B, A in 0.0..=1.0)
            for i in 0..pixel_count {
                let src = i * 16;
                let dst = i * 4;
                for c in 0..4 {
                    let f = f32::from_le_bytes([
                        raw[src + c * 4],
                        raw[src + c * 4 + 1],
                        raw[src + c * 4 + 2],
                        raw[src + c * 4 + 3],
                    ]);
                    output[dst + c] = (f.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                }
            }
        }
        TextureFormat::Dxt1 => {
            let blocks_x = (width as usize).div_ceil(4);
            let blocks_y = (height as usize).div_ceil(4);
            let mut block_offset = 0;
            for by in 0..blocks_y {
                for bx in 0..blocks_x {
                    let block = &raw[block_offset..block_offset + 8];
                    block_offset += 8;
                    let c0 = u16::from_le_bytes([block[0], block[1]]);
                    let c1 = u16::from_le_bytes([block[2], block[3]]);
                    let colors = expand_dxt_color_table(c0, c1);
                    let indices = expand_dxt_color_indices(&block[4..8]);
                    for py in 0..4 {
                        let y = by * 4 + py;
                        if y >= height as usize {
                            continue;
                        }
                        for px in 0..4 {
                            let x = bx * 4 + px;
                            if x >= width as usize {
                                continue;
                            }
                            let dst = (y * (width as usize) + x) * 4;
                            let c = colors[indices[py * 4 + px]];
                            output[dst..dst + 4].copy_from_slice(&c);
                        }
                    }
                }
            }
        }
        TextureFormat::Dxt3 => {
            let blocks_x = (width as usize).div_ceil(4);
            let blocks_y = (height as usize).div_ceil(4);
            let mut block_offset = 0;
            for by in 0..blocks_y {
                for bx in 0..blocks_x {
                    let block = &raw[block_offset..block_offset + 16];
                    block_offset += 16;
                    let mut alphas = [0u8; 16];
                    for j in 0..8 {
                        let lo = block[j] & 0x0F;
                        let hi = (block[j] >> 4) & 0x0F;
                        alphas[j * 2] = lo | (lo << 4);
                        alphas[j * 2 + 1] = hi | (hi << 4);
                    }
                    let c0 = u16::from_le_bytes([block[8], block[9]]);
                    let c1 = u16::from_le_bytes([block[10], block[11]]);
                    let colors = expand_dxt_color_table(c0, c1);
                    let indices = expand_dxt_color_indices(&block[12..16]);
                    for py in 0..4 {
                        let y = by * 4 + py;
                        if y >= height as usize {
                            continue;
                        }
                        for px in 0..4 {
                            let x = bx * 4 + px;
                            if x >= width as usize {
                                continue;
                            }
                            let idx = py * 4 + px;
                            let dst = (y * (width as usize) + x) * 4;
                            let c = colors[indices[idx]];
                            output[dst] = c[0];
                            output[dst + 1] = c[1];
                            output[dst + 2] = c[2];
                            output[dst + 3] = alphas[idx];
                        }
                    }
                }
            }
        }
        TextureFormat::Dxt5 => {
            let blocks_x = (width as usize).div_ceil(4);
            let blocks_y = (height as usize).div_ceil(4);
            let mut block_offset = 0;
            for by in 0..blocks_y {
                for bx in 0..blocks_x {
                    let block = &raw[block_offset..block_offset + 16];
                    block_offset += 16;
                    let a0 = block[0];
                    let a1 = block[1];
                    let mut alpha_table = [0u8; 8];
                    alpha_table[0] = a0;
                    alpha_table[1] = a1;
                    if a0 > a1 {
                        for (i, slot) in alpha_table.iter_mut().enumerate().skip(2) {
                            *slot =
                                (((8 - i as u16) * a0 as u16 + (i as u16 - 1) * a1 as u16 + 3) / 7)
                                    as u8;
                        }
                    } else {
                        for (i, slot) in alpha_table.iter_mut().enumerate().take(6).skip(2) {
                            *slot =
                                (((6 - i as u16) * a0 as u16 + (i as u16 - 1) * a1 as u16 + 2) / 5)
                                    as u8;
                        }
                        alpha_table[6] = 0;
                        alpha_table[7] = 255;
                    }
                    let mut alpha_indices = [0usize; 16];
                    for half in 0..2 {
                        let b_off = 2 + half * 3;
                        let flags = (block[b_off] as u32)
                            | ((block[b_off + 1] as u32) << 8)
                            | ((block[b_off + 2] as u32) << 16);
                        for j in 0..8 {
                            alpha_indices[half * 8 + j] = ((flags >> (j * 3)) & 0x07) as usize;
                        }
                    }
                    let c0 = u16::from_le_bytes([block[8], block[9]]);
                    let c1 = u16::from_le_bytes([block[10], block[11]]);
                    let colors = expand_dxt_color_table(c0, c1);
                    let indices = expand_dxt_color_indices(&block[12..16]);
                    for py in 0..4 {
                        let y = by * 4 + py;
                        if y >= height as usize {
                            continue;
                        }
                        for px in 0..4 {
                            let x = bx * 4 + px;
                            if x >= width as usize {
                                continue;
                            }
                            let idx = py * 4 + px;
                            let dst = (y * (width as usize) + x) * 4;
                            let c = colors[indices[idx]];
                            output[dst] = c[0];
                            output[dst + 1] = c[1];
                            output[dst + 2] = c[2];
                            output[dst + 3] = alpha_table[alpha_indices[idx]];
                        }
                    }
                }
            }
        }
        _ => {
            return Err(WzImageError::UnsupportedFormat(
                format.name().to_string(),
                format as u32,
            )
            .into());
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

    #[test]
    fn test_dxt5_and_png_encoding() {
        // 4x4 블록 1개 (16바이트): 알파 255 고정, 색상 0xFFFF (흰색)
        let mut block = [0u8; 16];
        block[0] = 255;
        block[1] = 255;
        block[8] = 0xFF;
        block[9] = 0xFF;
        let rgba = decode_to_rgba8(TextureFormat::Dxt5, 4, 4, &block).expect("decode dxt5");
        assert_eq!(rgba.len(), 64);
        assert_eq!(&rgba[0..4], &[255, 255, 255, 255]);

        let png = encode_png_rgba8(4, 4, &rgba);
        assert_eq!(&png[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        assert!(png.windows(4).any(|w| w == b"IHDR"));
        assert!(png.windows(4).any(|w| w == b"IDAT"));
        assert!(png.windows(4).any(|w| w == b"IEND"));
    }
}
