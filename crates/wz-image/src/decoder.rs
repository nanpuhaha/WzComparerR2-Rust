//! WZ 텍스처 데이터 디컴프레션, RGBA8 픽셀 디코더 및 무손실 PNG 인코더.
//!
//! 원본 `WzComparerR2.WzLib/Wz_Png.cs` 및 `Utilities/ImageCodec.cs` 기반의 픽셀 언패킹 구현.

use std::io::{Read, Write};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use wz_error::{Result, WzImageError};
use wz_core::image::PngRef;

use crate::bc7::unpack_bc7;
use crate::format::{
    actual_scale, bc7_data_size, uncompressed_data_size, TextureFormat, TextureLayout,
};

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

/// 원본 `Wz_Png.UnsafeOpenRead` (`Wz_Png.cs:141-166`) 호환 압축 해제.
///
/// # 함정: 스트림이 **완결되지 않았을 수 있다** (BFINAL=0, Z_SYNC_FLUSH 종료)
///
/// 실전 2026 클라이언트 (KMST777/1206 계열) 검증 결과, 캔버스 페이로드의
/// deflate 블록은 `BFINAL=0` 로 끝난다 — **마지막 블록 플래그와 adler32 가
/// 없다.** .NET `DeflateStream` 은 남은 입력이 끝나면 그대로 돌려주므로
/// 통과하지만, `flate2::ZlibDecoder` 는 "incomplete stream" 로 실패한다.
/// 원본이 `ZlibDecoder` 가 아닌 **raw `DeflateStream`** 을 쓰는 이유가 이것이다
/// (`78 9C` 헤더를 직접 소비한 뒤 raw deflate 로 이어서 읽는다).
///
/// 따라서 여기서는 C# 과 동일하게:
/// 1. 엄격 경로([`decompress_zlib`])를 먼저 시도한다 — 완결 스트림은 그대로 통과.
/// 2. 실패 시 C# 경로 재현: `78 9C` 헤더 2바이트를 소비한 뒤 **raw inflate**,
///    입력이 소진되면 그 시점까지 해제한 바이트를 그대로 쓴다.
///    (`expected` 보다 적게 나오면 진짜 오류.)
///
/// `expected` 는 요청 페이지들의 압축 해제 후 바이트 수다.
/// `78 9C` 가 아닌 스트림은 원본이 청크 암호화 경로로 보내므로 여기서 오류로
/// 마무리한다 (EncKeys 유도가 아직 `wz_image` 에는 없다).
pub fn decompress_zlib_compat(compressed: &[u8], expected: usize) -> Result<Vec<u8>> {
    if let Ok(v) = decompress_zlib(compressed) {
        if v.len() >= expected {
            return Ok(v);
        }
    }

    if compressed.len() < 2 || compressed[..2] != [0x78, 0x9C] {
        return Err(WzImageError::Inflate {
            offset: 0,
            source: std::io::Error::other("zlib 헤더 78 9C 가 아니다 — 청크 암호화 경로 미구현"),
        }
        .into());
    }
    raw_inflate_tolerant(&compressed[2..], expected)
}

/// raw deflate 해제 — 입력이 소진되면 **거기까지의 출력을 정상으로 취급한다.**
///
/// `.NET DeflateStream.Read` 의 동작 재현: 스트림 종료자(BFINAL=1 블록)가
/// 없어도 이미 해제한 바이트는 유효하다. (Z_SYNC_FLUSH 가 낸
/// `00 00 FF FF` 는 별도의 데이터가 아니라 빈 stored 블록이므로
/// 디코더가 자연스럽게 소비한다.)
fn raw_inflate_tolerant(input: &[u8], expected: usize) -> Result<Vec<u8>> {
    use flate2::{Decompress, FlushDecompress, Status};

    let mut d = Decompress::new(false);
    let mut out = vec![0u8; expected.max(1) + 64];
    let mut in_pos = 0usize;
    let mut out_pos = 0usize;

    while in_pos < input.len() && out_pos < expected {
        if out_pos == out.len() {
            out.resize(out.len() * 2, 0);
        }
        let status = d
            .decompress(&input[in_pos..], &mut out[out_pos..], FlushDecompress::None)
            .map_err(|e| WzImageError::Inflate {
                offset: in_pos as u64,
                source: std::io::Error::other(e),
            })?;
        let produced = d.total_out() as usize - out_pos;
        out_pos += produced;
        in_pos += d.total_in() as usize - in_pos;

        if status == Status::StreamEnd
            || in_pos >= input.len()   // 입력 소진: .NET 처럼 여기까지의 바이트를 쓴다
            || produced == 0           // 진행 없는 BufError → 데이터 오류
        {
            break;
        }
    }
    out.truncate(out_pos);
    Ok(out)
}

/// RGBA8 픽셀 버퍼(`src_w * src_h * 4` 바이트)를 가로 `scale_x`배, 세로 `scale_y`배 최근접 이웃 확장한다.
///
/// 원본 `ImageCodec.ScalePixels` (`ImageCodec.cs:97-126`) 대응.
pub fn scale_pixels_rgba8(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    scale_x: u32,
    scale_y: u32,
) -> Vec<u8> {
    let sw = src_w as usize;
    let sh = src_h as usize;
    let sx = scale_x as usize;
    let sy = scale_y as usize;
    let dst_w = sw * sx;
    let dst_h = sh * sy;
    let dst_stride = dst_w * 4;
    let src_stride = sw * 4;
    let mut out = vec![0u8; dst_stride * dst_h];

    for y in 0..sh {
        let src_row_start = y * src_stride;
        let dst_row0_start = (y * sy) * dst_stride;
        for x in 0..sw {
            let px_off = src_row_start + x * 4;
            let px = &src[px_off..px_off + 4];
            for s in 0..sx {
                let dst_off = dst_row0_start + (x * sx + s) * 4;
                out[dst_off..dst_off + 4].copy_from_slice(px);
            }
        }
        let first_row = out[dst_row0_start..dst_row0_start + dst_stride].to_vec();
        for s in 1..sy {
            let dup_start = dst_row0_start + s * dst_stride;
            out[dup_start..dup_start + dst_stride].copy_from_slice(&first_row);
        }
    }

    out
}

/// 멀티페이지 및 스케일(`RGB565` scale=16 포함) 규칙을 반영해 특정 `page`의 RGBA8 픽셀과 출력 크기(`(width, height, rgba)`)를 반환한다.
///
/// 원본 `Wz_Png.ExtractPng(int page)` (`Wz_Png.cs:173-315`) 대응.
pub fn decode_page_to_rgba8(
    format: TextureFormat,
    width: u32,
    height: u32,
    scale: u32,
    pages: u32,
    page: u32,
    raw_all_pages: &[u8],
) -> Result<(u32, u32, Vec<u8>)> {
    let effective_page = if pages > 0 {
        if page >= pages {
            return Err(WzImageError::PageOutOfRange {
                requested: page,
                available: pages,
            }
            .into());
        }
        page as usize
    } else {
        0
    };

    let layout = TextureLayout::for_png(format, scale, width, height).ok_or(
        WzImageError::SizeOverflow {
            w: width,
            h: height,
            format: format as u32,
            scale,
        },
    )?;

    let bytes_per_page = if format == TextureFormat::Bc7 {
        bc7_data_size(width, height).ok_or(WzImageError::SizeOverflow {
            w: width,
            h: height,
            format: format as u32,
            scale,
        })?
    } else {
        layout.bytes_per_page
    };

    let start = effective_page
        .checked_mul(bytes_per_page)
        .ok_or(WzImageError::SizeOverflow {
            w: width,
            h: height,
            format: format as u32,
            scale,
        })?;
    let end = start
        .checked_add(bytes_per_page)
        .ok_or(WzImageError::SizeOverflow {
            w: width,
            h: height,
            format: format as u32,
            scale,
        })?;
    if raw_all_pages.len() < end {
        return Err(WzImageError::SizeOverflow {
            w: width,
            h: height,
            format: format as u32,
            scale,
        }
        .into());
    }
    let page_slice = &raw_all_pages[start..end];

    let a_scale = actual_scale(scale);
    if format == TextureFormat::Rgb565 && a_scale == 16 {
        let raw_w = width / 16;
        let raw_h = height / 16;
        let small_rgba = decode_to_rgba8(TextureFormat::Rgb565, raw_w, raw_h, page_slice)?;
        let upscaled = scale_pixels_rgba8(&small_rgba, raw_w, raw_h, 16, 16);
        return Ok((layout.decodable_width, layout.decodable_height, upscaled));
    }

    let rgba = decode_to_rgba8(format, width, height, page_slice)?;
    Ok((layout.decodable_width, layout.decodable_height, rgba))
}

/// 압축 해제 전의 ZLIB raw 페이로드를 받아 압축 해제 후 특정 `page`를 RGBA8로 디코딩한다.
pub fn decode_raw_page_to_rgba8(
    format: TextureFormat,
    width: u32,
    height: u32,
    scale: u32,
    pages: u32,
    page: u32,
    raw_compressed: &[u8],
) -> Result<(u32, u32, Vec<u8>)> {
    // 요청 페이지 범위만큼 필요한 압축 해제 후 바이트 수.
    let layout = TextureLayout::for_png(format, scale, width, height).ok_or(
        WzImageError::SizeOverflow {
            w: width,
            h: height,
            format: format as u32,
            scale,
        },
    )?;
    let bytes_per_page = if format == TextureFormat::Bc7 {
        bc7_data_size(width, height).ok_or(WzImageError::SizeOverflow {
            w: width,
            h: height,
            format: format as u32,
            scale,
        })?
    } else {
        layout.bytes_per_page
    };
    let total_pages = if pages > 0 {
        pages as usize
    } else {
        1
    };
    let total_pages_usize = bytes_per_page
        .checked_mul(total_pages)
        .ok_or(WzImageError::SizeOverflow {
            w: width,
            h: height,
            format: format as u32,
            scale,
        })?;

    let decompressed = match decompress_zlib_compat(raw_compressed, total_pages_usize) {
        Ok(v) => v,
        Err(e) => {
            // 합성 코퍼스처럼 data 가 "압축이 완전 풀린" 상태로 저장된 경우도
            // 있다 — raw 로 간주해 디코딩을 다시 시도한다.
            if raw_compressed.len() >= total_pages_usize {
                raw_compressed.to_vec()
            } else {
                return Err(e);
            }
        }
    };
    decode_page_to_rgba8(format, width, height, scale, pages, page, &decompressed)
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
    if format == TextureFormat::Bc7 {
        let out_w = (width & !3) as usize;
        let out_h = (height & !3) as usize;
        let expected_raw =
            bc7_data_size(width, height).ok_or(WzImageError::SizeOverflow {
                w: width,
                h: height,
                format: format as u32,
                scale: 1,
            })?;
        if raw.len() < expected_raw {
            return Err(WzImageError::SizeOverflow {
                w: width,
                h: height,
                format: format as u32,
                scale: 1,
            }
            .into());
        }
        let mut output = vec![0u8; out_w * out_h * 4];
        // C# ImageCodec.BC7ToRGBA32 (ImageCodec.cs:192-211) 대응:
        // BC7 압축 블록 행 스트라이드는 `this.Width * 4` 바이트(4개 픽셀 행 당 width * 4 바이트 raw 블록).
        let src_stride = (width as usize) * 4;
        let dst_stride = out_w * 4;
        for (by, y) in (0..out_h).step_by(4).enumerate() {
            let row_start = by * src_stride;
            for (bx, x) in (0..out_w).step_by(4).enumerate() {
                let block_start = row_start + bx * 16;
                let mut block = [0u8; 16];
                block.copy_from_slice(&raw[block_start..block_start + 16]);
                let pixels = unpack_bc7(&block);
                for py in 0..4 {
                    let dst_row_off = (y + py) * dst_stride + x * 4;
                    for px in 0..4 {
                        let dst_off = dst_row_off + px * 4;
                        output[dst_off..dst_off + 4].copy_from_slice(&pixels[py * 4 + px]);
                    }
                }
            }
        }
        return Ok(output);
    }

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

    #[test]
    fn test_bc7_mode6_decoding_and_tail_row_truncation() {
        // Mode 6 (first byte bit 6 set = 0x40), all endpoints = 0x7F and pbits = 1 -> RGBA = (255, 255, 255, 255)
        let lo: u64 = (1u64 << 6) | (!0u64 << 7);
        let hi: u64 = 1u64; // p1 = 1, all weight indices = 0
        let mut block = [0u8; 16];
        block[0..8].copy_from_slice(&lo.to_le_bytes());
        block[8..16].copy_from_slice(&hi.to_le_bytes());

        // height=6 (4의 배수가 아닌 BC7 높이 -> 4로 절삭되어 4x4 = 64바이트 출력)
        let rgba = decode_to_rgba8(TextureFormat::Bc7, 4, 6, &block).expect("decode bc7");
        assert_eq!(rgba.len(), 4 * 4 * 4);
        assert_eq!(&rgba[0..4], &[255, 255, 255, 255]);
    }

    #[test]
    fn test_multipage_and_rgb565_scale16() {
        // 2페이지 1x1 ARGB8888: page 0 = 파랑, page 1 = 빨강
        let raw_pages = vec![
            255, 0, 0, 255, // page 0: BGRA(255,0,0,255) -> RGBA(0,0,255,255)
            0, 0, 255, 255, // page 1: BGRA(0,0,255,255) -> RGBA(255,0,0,255)
        ];
        let (w0, h0, p0) =
            decode_page_to_rgba8(TextureFormat::Argb8888, 1, 1, 0, 2, 0, &raw_pages).unwrap();
        assert_eq!((w0, h0), (1, 1));
        assert_eq!(p0, vec![0, 0, 255, 255]);

        let (_, _, p1) =
            decode_page_to_rgba8(TextureFormat::Argb8888, 1, 1, 0, 2, 1, &raw_pages).unwrap();
        assert_eq!(p1, vec![255, 0, 0, 255]);

        // 범위 초과 페이지 요청 시 에러
        assert!(decode_page_to_rgba8(TextureFormat::Argb8888, 1, 1, 0, 2, 2, &raw_pages).is_err());

        // RGB565 scale=4 (ActualScale=16): 16x16 출력에 1x1 (2바이트) 원시 데이터만 사용
        let raw_565 = vec![0xFF, 0xFF];
        let (sw, sh, scaled) =
            decode_page_to_rgba8(TextureFormat::Rgb565, 16, 16, 4, 1, 0, &raw_565).unwrap();
        assert_eq!((sw, sh), (16, 16));
        assert_eq!(scaled.len(), 16 * 16 * 4);
        assert!(scaled.iter().all(|&b| b == 255));
    }

    /// 실전 2026 클라이언트 (KMST 계열) 금고 회귀 테스트.
    ///
    /// 실측 바이트 (`Item/Cash/Cash_000.wz`, 1x1 ARGB4444 캔버스, dataLen=11):
    /// ```text
    /// 78 9c 62 60 00 00 00 00 ff ff
    /// ```
    /// `BFINAL=0` — 마지막 deflate 블록 플래그와 adler32 가 없는 **비완결** 스트림
    /// (Z_SYNC_FLUSH 종료). .NET `DeflateStream` 기준으로는 디코딩 2바이트가 나온다.
    #[test]
    fn sync_flush_terminated_stream_is_decodable() {
        let raw = [0x78, 0x9C, 0x62, 0x60, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF];

        // 완결 스트림이 아니므로 strict 경로는 실패해야 한다.
        assert!(decompress_zlib(&raw).is_err());

        // compat 경로: raw deflate 로 2바이트(1x1 ARGB4444) 해제.
        let out = decompress_zlib_compat(&raw, 2).unwrap();
        assert_eq!(out, vec![0x00, 0x00]);

        // 디코더 전체 경로: 1x1 ARGB4444 → RGBA8 투명 픽셀.
        let (_, _, rgba) =
            decode_raw_page_to_rgba8(TextureFormat::Argb4444, 1, 1, 0, 0, 0, &raw).unwrap();
        assert_eq!(rgba, vec![0, 0, 0, 0]);
    }

    /// 완결 스트림은 strict 경로 그대로 통과해야 한다 (레거시 WZ 호환).
    #[test]
    fn complete_stream_prefers_strict_path() {
        let complete = {
            let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
            e.write_all(&[0x00, 0x00]).unwrap();
            e.finish().unwrap()
        };
        assert_eq!(
            decompress_zlib_compat(&complete, 2).unwrap(),
            decompress_zlib(&complete).unwrap()
        );
    }
}


/// `PngRef` + 바이트 소스 → RGBA8 픽셀.
///
/// GUI/CLI 가 `extract_raw_bytes` + `TextureFormat::from_u32` + `decode_raw_page_to_rgba8`
/// 를 매번 조합하지 않도록 한 곳에 가둔다 (아키텍처 감사 §3.3 deepening).
pub fn decode_png_ref_to_rgba(
    png_ref: &PngRef,
    src: &std::sync::Arc<dyn wz_pool::ByteSource>,
) -> wz_error::Result<(u32, u32, Vec<u8>)> {
    let raw_payload = png_ref.extract_raw_bytes(src)?;

    let tex_fmt = TextureFormat::from_u32(png_ref.format as u32);
    match tex_fmt {
        TextureFormat::Unknown => Ok((png_ref.width as u32, png_ref.height as u32, raw_payload)),
        fmt => {
            let (w, h, px) = decode_raw_page_to_rgba8(
                fmt,
                png_ref.width as u32,
                png_ref.height as u32,
                png_ref.scale as u32,
                png_ref.pages as u32,
                0,
                &raw_payload,
            )?;
            Ok((w, h, px))
        }
    }
}
