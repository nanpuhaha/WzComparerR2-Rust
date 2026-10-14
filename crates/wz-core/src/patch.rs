//! 메이플스토리 `.patch` 파일 헤더 파싱 및 체크섬 검증 (`WzPatcher.cs` 포트).
//!
//! 원본 `WzComparerR2/Patcher/WzPatcher.cs` 및 `Patcher/Builder/CheckSum.cs` 대응.

use std::sync::Arc;
use wz_error::{Result, WzFormatError};
use wz_pool::ByteSource;

const POLYNOMIAL: u32 = 0x04C11DB7;
const TOP_BIT: u32 = 0x80000000;

/// BZIP2/CRC32 변종 체크섬 계산기 (원본 `CheckSum.cs:37-65, 184-186`).
pub fn compute_patch_checksum(data: &[u8], initial_crc: u32) -> u32 {
    let mut sbox = [0u32; 256];
    for i in 0..256u32 {
        let mut remain = i << 24;
        for _ in 0..8 {
            if (remain & TOP_BIT) != 0 {
                remain = (remain << 1) ^ POLYNOMIAL;
            } else {
                remain <<= 1;
            }
        }
        sbox[i as usize] = remain;
    }

    let mut crc = initial_crc;
    for &b in data {
        crc = (crc << 8) ^ sbox[((crc >> 24) as u8 ^ b) as usize];
    }
    crc
}

/// 패치 파일 내 각 파트의 조작 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchPartType {
    /// 0: 신규 파일 생성
    Create = 0,
    /// 1: 기존 파일 패치 변형
    Modify = 1,
    /// 2: 기존 파일 삭제
    Delete = 2,
}

/// 패치 파일 내부 파일 파트 메타데이터.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchPart {
    pub file_name: String,
    pub part_type: PatchPartType,
    pub old_checksum: Option<u32>,
    pub new_checksum: Option<u32>,
    pub new_file_len: usize,
    pub data_offset: u64,
}

/// `.patch` 파일 분할 및 헤더 파싱 결과.
#[derive(Debug, Clone)]
pub struct PatchHeader {
    pub version: i32,
    pub expected_checksum: u32,
    pub computed_checksum: u32,
    pub is_valid: bool,
    pub data_start_offset: u64,
    pub compressed_len: u64,
    pub notice_text: Option<String>,
}

impl PatchHeader {
    /// 바이트 소스에서 패치 파일 헤더 및 체크섬(`[16, end)` 영역)을 검증한다.
    ///
    /// 원본 `WzPatcher.OpenDecompress` (`WzPatcher.cs:60-90`) 및 `docs/known-bugs.md` T18 대응:
    /// 패치 블록의 오프셋 8: `ver(i32)`, 오프셋 12: `checkSum0(u32)`, 오프셋 16 이후가 압축 페이로드.
    pub fn parse(src: &Arc<dyn ByteSource>) -> Result<Self> {
        let len = src.len()?;
        if len < 20 {
            return Err(WzFormatError::Malformed {
                what: "patch file too short",
                offset: 0,
            }
            .into());
        }

        // SFX EXE 패처인지 확인 (오프셋 0이 "MZ" (0x5A4D) 인 경우 파일 후미 분할)
        let mut header_buf = [0u8; 16];
        src.read_at(0, &mut header_buf)?;

        let (patch_offset, patch_len, notice) = if header_buf[0] == 0x4D && header_buf[1] == 0x5A {
            // 파일 끝에서 12바이트 전 (0xf2f7fbf3 매직 확인)
            if len >= 16 {
                let mut tail = [0u8; 12];
                src.read_at(len - 12, &mut tail)?;
                let magic = u32::from_le_bytes([tail[8], tail[9], tail[10], tail[11]]);
                if magic == 0xF2F7FBF3 {
                    let block_len = u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]) as u64;
                    let notice_len = u32::from_le_bytes([tail[4], tail[5], tail[6], tail[7]]) as u64;
                    let block_start = len - 12 - notice_len - block_len;
                    let mut notice_bytes = vec![0u8; notice_len as usize];
                    src.read_at(block_start + block_len, &mut notice_bytes)?;
                    let notice_str = String::from_utf8_lossy(&notice_bytes).to_string();
                    (block_start, block_len, Some(notice_str))
                } else {
                    (0, len, None)
                }
            } else {
                (0, len, None)
            }
        } else {
            (0, len, None)
        };

        if patch_len < 16 {
            return Err(WzFormatError::Malformed {
                what: "patch header too short",
                offset: patch_offset,
            }
            .into());
        }

        let mut patch_head = [0u8; 16];
        src.read_at(patch_offset, &mut patch_head)?;

        let version = i32::from_le_bytes([patch_head[8], patch_head[9], patch_head[10], patch_head[11]]);
        let expected_checksum = u32::from_le_bytes([patch_head[12], patch_head[13], patch_head[14], patch_head[15]]);

        // T18: CRC는 압축 영역 [16, end)만 해시
        let payload_len = (patch_len - 16) as usize;
        let mut payload = vec![0u8; payload_len];
        src.read_at(patch_offset + 16, &mut payload)?;
        let computed_checksum = compute_patch_checksum(&payload, 0);

        Ok(Self {
            version,
            expected_checksum,
            computed_checksum,
            is_valid: expected_checksum == computed_checksum,
            data_start_offset: patch_offset + 16,
            compressed_len: payload_len as u64,
            notice_text: notice,
        })
    }

    /// ZLIB 인플레이트 스트림을 열어 패치 파일 내부의 파트 목록(`PatchPart`)을 파싱한다.
    ///
    /// 원본 `WzPatcher.PrePatch` (`WzPatcher.cs:171-237`) 대응:
    /// - ZLIB 헤더(0x78 0x9C 등)를 확인/건너뛰고 인플레이트 스트림을 초기화
    /// - 패치 파트 헤더(`GetFileName` + 타입별 메타데이터)를 순차 추출
    pub fn parse_parts(&self, src: &Arc<dyn ByteSource>) -> Result<Vec<PatchPart>> {
        use flate2::read::ZlibDecoder;
        use std::io::Read;

        let mut raw = vec![0u8; self.compressed_len as usize];
        src.read_at(self.data_start_offset, &mut raw)?;

        if raw.len() < 2 {
            return Ok(Vec::new());
        }
        let slice = &raw[..];

        let mut decoder = ZlibDecoder::new(slice);
        let mut decompressed = Vec::new();
        if decoder.read_to_end(&mut decompressed).is_err() {
            // raw deflate fallback
            use flate2::read::DeflateDecoder;
            let mut def_decoder = DeflateDecoder::new(slice);
            decompressed.clear();
            let _ = def_decoder.read_to_end(&mut decompressed);
        }

        if decompressed.is_empty() {
            return Ok(Vec::new());
        }

        let mut cursor = 0usize;
        let mut parts = Vec::new();

        // KMST1125 파일 해시 테이블 건너뛰기/확인
        if cursor + 4 <= decompressed.len() {
            let count = i32::from_le_bytes([
                decompressed[cursor],
                decompressed[cursor + 1],
                decompressed[cursor + 2],
                decompressed[cursor + 3],
            ]);
            if count > 0 && count < 10000 {
                // hash list heuristic: try parse
                let mut temp_cur = cursor + 4;
                let mut valid_table = true;
                for _ in 0..count {
                    if temp_cur + 4 > decompressed.len() {
                        valid_table = false;
                        break;
                    }
                    let str_len = i32::from_le_bytes([
                        decompressed[temp_cur],
                        decompressed[temp_cur + 1],
                        decompressed[temp_cur + 2],
                        decompressed[temp_cur + 3],
                    ]) as usize;
                    temp_cur += 4;
                    if str_len > 1024 || temp_cur + str_len + 4 > decompressed.len() {
                        valid_table = false;
                        break;
                    }
                    temp_cur += str_len + 4;
                }
                if valid_table {
                    cursor = temp_cur;
                }
            }
        }

        while cursor < decompressed.len() {
            let mut name_bytes = Vec::new();
            let mut patch_type_byte = None;
            while cursor < decompressed.len() {
                let b = decompressed[cursor];
                cursor += 1;
                if b <= 2 {
                    patch_type_byte = Some(b);
                    break;
                }
                name_bytes.push(b);
            }

            let p_type = match patch_type_byte {
                Some(0) => PatchPartType::Create,
                Some(1) => PatchPartType::Modify,
                Some(2) => PatchPartType::Delete,
                _ => break,
            };

            let file_name = String::from_utf8_lossy(&name_bytes).to_string();
            let mut old_checksum = None;
            let mut new_checksum = None;
            let mut new_file_len = 0usize;

            match p_type {
                PatchPartType::Create => {
                    if file_name.contains('.') && cursor + 8 <= decompressed.len() {
                        new_file_len = i32::from_le_bytes([
                            decompressed[cursor],
                            decompressed[cursor + 1],
                            decompressed[cursor + 2],
                            decompressed[cursor + 3],
                        ]) as usize;
                        cursor += 4;
                        new_checksum = Some(u32::from_le_bytes([
                            decompressed[cursor],
                            decompressed[cursor + 1],
                            decompressed[cursor + 2],
                            decompressed[cursor + 3],
                        ]));
                        cursor += 4;
                        // skip payload
                        cursor = cursor.saturating_add(new_file_len).min(decompressed.len());
                    }
                }
                PatchPartType::Modify => {
                    if cursor + 8 <= decompressed.len() {
                        old_checksum = Some(u32::from_le_bytes([
                            decompressed[cursor],
                            decompressed[cursor + 1],
                            decompressed[cursor + 2],
                            decompressed[cursor + 3],
                        ]));
                        cursor += 4;
                        new_checksum = Some(u32::from_le_bytes([
                            decompressed[cursor],
                            decompressed[cursor + 1],
                            decompressed[cursor + 2],
                            decompressed[cursor + 3],
                        ]));
                        cursor += 4;
                    }
                }
                PatchPartType::Delete => {}
            }

            parts.push(PatchPart {
                file_name,
                part_type: p_type,
                old_checksum,
                new_checksum,
                new_file_len,
                data_offset: cursor as u64,
            });
        }

        Ok(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_pool::MemorySource;

    #[test]
    fn test_patch_checksum_and_header_parsing() {
        // 더미 패치 데이터 구성:
        // [0..8]: 매직/예약 (8바이트)
        // [8..12]: ver = 100
        // [12..16]: checksum
        // [16..]: 압축 페이로드 데이터
        let payload = b"MapleStoryPatchTestDataPayload1234567890";
        let computed_crc = compute_patch_checksum(payload, 0);

        let mut data = vec![0u8; 16];
        data[8..12].copy_from_slice(&100i32.to_le_bytes());
        data[12..16].copy_from_slice(&computed_crc.to_le_bytes());
        data.extend_from_slice(payload);

        let src: Arc<dyn ByteSource> = Arc::new(MemorySource::new(data));
        let header = PatchHeader::parse(&src).expect("valid patch header");

        assert_eq!(header.version, 100);
        assert_eq!(header.expected_checksum, computed_crc);
        assert_eq!(header.computed_checksum, computed_crc);
        assert!(header.is_valid);
        assert_eq!(header.compressed_len, payload.len() as u64);
    }

    #[test]
    fn test_patch_parse_parts_decompression() {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        // 더미 파트 데이터 구성:
        // [0..4] type: 0 (Create)
        // [4..] string: "Sound/Bgm00.img" (utf16 or len-prefixed)
        // [..] old_len: 4바이트
        // [..] new_len: 4바이트 (100)
        // [..] payload: 100바이트
        let mut raw_parts = Vec::new();
        // Part 1: Type 0 (Create), name "Base.wz"
        let name1 = "Base.wz";
        raw_parts.extend_from_slice(name1.as_bytes());
        raw_parts.push(0u8); // type 0 = Create
        raw_parts.extend_from_slice(&10i32.to_le_bytes()); // new_file_len
        raw_parts.extend_from_slice(&12345u32.to_le_bytes()); // new_checksum
        raw_parts.extend_from_slice(&[0xAA; 10]); // payload

        // ZLIB 압축
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw_parts).unwrap();
        let compressed_payload = encoder.finish().unwrap();

        let computed_crc = compute_patch_checksum(&compressed_payload, 0);

        let mut data = vec![0u8; 16];
        data[8..12].copy_from_slice(&200i32.to_le_bytes());
        data[12..16].copy_from_slice(&computed_crc.to_le_bytes());
        data.extend_from_slice(&compressed_payload);

        let src: Arc<dyn ByteSource> = Arc::new(MemorySource::new(data));
        let header = PatchHeader::parse(&src).expect("valid patch header");
        let parts = header.parse_parts(&src).expect("parse parts");

        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].file_name, "Base.wz");
        assert_eq!(parts[0].part_type, PatchPartType::Create);
        assert_eq!(parts[0].new_file_len, 10);
    }
}
