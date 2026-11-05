//! 메이플스토리 `.patch` 파일 헤더 파싱 및 체크섬 검증 (`WzPatcher.cs` 포트).
//!
//! 원본 `WzComparerR2/Patcher/WzPatcher.cs` 및 `Patcher/Builder/CheckSum.cs` 대응.

use std::sync::Arc;
use wz_error::{Result, WzFormatError};
use wz_pool::ByteSource;

const POLYNOMIAL: u32 = 0x04C11DB7;
const TOP_BIT: u32 = 0x80000000;

/// SFX 패처 안내문 길이의 상한 (1 MiB). 손상된 tail 로 거대한 할당을 막는다.
const MAX_NOTICE_BYTES: u64 = 1024 * 1024;

/// 압축 해제 페이로드 상한 (4 GiB). 실제 대형 패치는 1.5 GiB 수준이며,
/// 64 MiB 로 제한하면 파트 목록이 통째로 비어 버린다.
const MAX_PARTS_DECOMPRESSED_BYTES: usize = 4 * 1024 * 1024 * 1024;

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
                    let notice_len =
                        u32::from_le_bytes([tail[4], tail[5], tail[6], tail[7]]) as u64;
                    // 손상/조작된 tail 길이로 underflow 하거나 거대한 버퍼를
                    // 할당하지 않도록 파일 크기와 상한을 먼저 검사한다.
                    let overhead = 12u64
                        .checked_add(notice_len)
                        .and_then(|v| v.checked_add(block_len))
                        .ok_or(WzFormatError::Malformed {
                            what: "patch tail length overflow",
                            offset: 0,
                        })?;
                    if overhead > len {
                        return Err(WzFormatError::Malformed {
                            what: "patch tail exceeds file size",
                            offset: 0,
                        }
                        .into());
                    }
                    if notice_len > MAX_NOTICE_BYTES {
                        return Err(WzFormatError::Malformed {
                            what: "patch notice too large",
                            offset: 0,
                        }
                        .into());
                    }
                    let block_start = len - overhead;
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

        let version =
            i32::from_le_bytes([patch_head[8], patch_head[9], patch_head[10], patch_head[11]]);
        let expected_checksum = u32::from_le_bytes([
            patch_head[12],
            patch_head[13],
            patch_head[14],
            patch_head[15],
        ]);

        // T18: CRC는 압축 영역 [16, end)만 해시
        let payload_len =
            usize::try_from(patch_len - 16).map_err(|_| WzFormatError::Malformed {
                what: "patch payload too large",
                offset: patch_offset,
            })?;
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

    /// ZLIB 압축 영역을 메모리로 인플레이트한다.
    ///
    /// 원본 `WzPatcher.OpenDecompress` 와 같은 zlib/raw-deflate 폴백을 쓴다.
    pub fn decompress_payload(&self, src: &Arc<dyn ByteSource>) -> Result<Vec<u8>> {
        use flate2::read::ZlibDecoder;
        use std::io::Read;

        let mut raw = vec![
            0u8;
            usize::try_from(self.compressed_len).map_err(|_| {
                WzFormatError::Malformed {
                    what: "patch payload too large",
                    offset: self.data_start_offset,
                }
            })?
        ];
        src.read_at(self.data_start_offset, &mut raw)?;

        if raw.len() < 2 {
            return Ok(Vec::new());
        }

        let limit = MAX_PARTS_DECOMPRESSED_BYTES as u64 + 1;
        let mut decoder = ZlibDecoder::new(&raw[..]).take(limit);
        let mut decompressed = Vec::new();
        if decoder.read_to_end(&mut decompressed).is_err()
            || decompressed.len() > MAX_PARTS_DECOMPRESSED_BYTES
        {
            use flate2::read::DeflateDecoder;
            let mut def_decoder = DeflateDecoder::new(&raw[..]).take(limit);
            decompressed.clear();
            let _ = def_decoder.read_to_end(&mut decompressed);
            if decompressed.len() > MAX_PARTS_DECOMPRESSED_BYTES {
                return Err(WzFormatError::Malformed {
                    what: "patch part list too large",
                    offset: self.data_start_offset,
                }
                .into());
            }
        }
        Ok(decompressed)
    }

    /// ZLIB 인플레이트 스트림을 열어 패치 파일 내부의 파트 목록(`PatchPart`)을 파싱한다.
    ///
    /// 원본 `WzPatcher.PrePatch` (`WzPatcher.cs:171-237`) 대응:
    /// - ZLIB 헤더(0x78 0x9C 등)를 확인/건너뛰고 인플레이트 스트림을 초기화
    /// - 패치 파트 헤더(`GetFileName` + 타입별 메타데이터)를 순차 추출
    pub fn parse_parts(&self, src: &Arc<dyn ByteSource>) -> Result<Vec<PatchPart>> {
        let decompressed = self.decompress_payload(src)?;
        Ok(parse_parts_from_payload(&decompressed).0)
    }

    /// 압축 해제된 페이로드와 KMST1125 해시 테이블 여부를 함께 읽는다.
    pub fn read_patch(&self, src: &Arc<dyn ByteSource>) -> Result<PatchPayload> {
        let decompressed = self.decompress_payload(src)?;
        let (parts, is_kmst1125, old_hashes) = parse_parts_from_payload_with_hashes(&decompressed);
        Ok(PatchPayload {
            bytes: decompressed,
            parts,
            is_kmst1125,
            old_hashes,
        })
    }
}

/// 압축 해제된 `.patch` 페이로드와 레코드.
#[derive(Debug, Clone, Default)]
pub struct PatchPayload {
    pub bytes: Vec<u8>,
    pub parts: Vec<PatchPart>,
    pub is_kmst1125: bool,
    /// KMST1125 선행 해시 테이블 (`파일명 → 체크섬`).
    pub old_hashes: std::collections::HashMap<String, u32>,
}

fn read_i32(bytes: &[u8], cursor: &mut usize) -> Option<i32> {
    if *cursor + 4 > bytes.len() {
        return None;
    }
    let v = i32::from_le_bytes([
        bytes[*cursor],
        bytes[*cursor + 1],
        bytes[*cursor + 2],
        bytes[*cursor + 3],
    ]);
    *cursor += 4;
    Some(v)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Option<u32> {
    read_i32(bytes, cursor).map(|v| v as u32)
}

fn read_len_string(bytes: &[u8], cursor: &mut usize, max: usize) -> Option<String> {
    let len = read_i32(bytes, cursor)?;
    if len < 0 || len as usize > max || *cursor + len as usize > bytes.len() {
        return None;
    }
    let s = String::from_utf8_lossy(&bytes[*cursor..*cursor + len as usize]).to_string();
    *cursor += len as usize;
    Some(s)
}

/// 수정 레코드의 명령 스트림을 건너뛴다 (`CalcNewFileLength`).
fn skip_modify_commands(bytes: &[u8], mut cursor: usize, is_kmst1125: bool) -> (usize, usize) {
    let mut output_len = 0usize;
    while let Some(cmd) = read_i32(bytes, &mut cursor) {
        if cmd == 0 {
            break;
        }
        match (cmd as u32) >> 0x1C {
            0x08 => {
                let len = (cmd as u32 & 0x0FFF_FFFF) as usize;
                cursor = cursor.saturating_add(len);
                output_len += len;
            }
            0x0C => {
                output_len += ((cmd as u32 & 0x0FFF_FF00) >> 8) as usize;
            }
            _ => {
                cursor = cursor.saturating_add(4);
                if is_kmst1125 {
                    let _ = read_len_string(bytes, &mut cursor, 4096);
                }
                // 길이(`cmd`)는 출력 파일 길이일 뿐 패치 스트림에는 바이트가 없다.
                output_len += cmd.max(0) as usize;
            }
        }
        if cursor > bytes.len() {
            return (bytes.len(), output_len);
        }
    }
    (cursor.min(bytes.len()), output_len)
}

fn parse_parts_from_payload(payload: &[u8]) -> (Vec<PatchPart>, bool) {
    let (parts, is_kmst, _) = parse_parts_from_payload_with_hashes(payload);
    (parts, is_kmst)
}

fn parse_parts_from_payload_with_hashes(
    decompressed: &[u8],
) -> (Vec<PatchPart>, bool, std::collections::HashMap<String, u32>) {
    let mut cursor = 0usize;
    let mut parts = Vec::new();
    let mut old_hashes = std::collections::HashMap::new();
    let mut is_kmst1125 = false;

    // KMST1125 선행 파일 해시 테이블: count(i32) + [len(i32)+name+checksum(u32)]*count
    if cursor + 4 <= decompressed.len() {
        let count = i32::from_le_bytes([
            decompressed[cursor],
            decompressed[cursor + 1],
            decompressed[cursor + 2],
            decompressed[cursor + 3],
        ]);
        if count > 0 && count < 100_000 {
            let mut temp_cur = cursor + 4;
            let mut table = std::collections::HashMap::new();
            let mut valid = true;
            for _ in 0..count {
                let Some(name) = read_len_string(decompressed, &mut temp_cur, 4096) else {
                    valid = false;
                    break;
                };
                if temp_cur + 4 > decompressed.len() {
                    valid = false;
                    break;
                }
                let hash = u32::from_le_bytes([
                    decompressed[temp_cur],
                    decompressed[temp_cur + 1],
                    decompressed[temp_cur + 2],
                    decompressed[temp_cur + 3],
                ]);
                temp_cur += 4;
                table.insert(name, hash);
            }
            if valid && !table.is_empty() {
                old_hashes = table;
                is_kmst1125 = true;
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
        let mut data_offset = cursor;

        match p_type {
            PatchPartType::Create => {
                if file_name.contains('.') {
                    let Some(len) = read_i32(decompressed, &mut cursor) else {
                        break;
                    };
                    let Some(checksum) = read_u32(decompressed, &mut cursor) else {
                        break;
                    };
                    new_file_len = len.max(0) as usize;
                    new_checksum = Some(checksum);
                    data_offset = cursor;
                    cursor = cursor.saturating_add(new_file_len).min(decompressed.len());
                }
            }
            PatchPartType::Modify => {
                if !is_kmst1125 {
                    let Some(old) = read_u32(decompressed, &mut cursor) else {
                        break;
                    };
                    old_checksum = Some(old);
                }
                let Some(new) = read_u32(decompressed, &mut cursor) else {
                    break;
                };
                new_checksum = Some(new);
                data_offset = cursor;
                let (next, len) = skip_modify_commands(decompressed, cursor, is_kmst1125);
                new_file_len = len;
                cursor = next;
            }
            PatchPartType::Delete => {}
        }

        parts.push(PatchPart {
            file_name,
            part_type: p_type,
            old_checksum,
            new_checksum,
            new_file_len,
            data_offset: data_offset as u64,
        });
    }

    (parts, is_kmst1125, old_hashes)
}

/// 패치 적용 옵션.
#[derive(Debug, Clone)]
pub struct PatchApplyOptions {
    /// 생성/수정 결과와 원본 파일의 체크섬을 검증한다 (기본 true).
    pub verify_checksums: bool,
}

impl Default for PatchApplyOptions {
    fn default() -> Self {
        Self {
            verify_checksums: true,
        }
    }
}

/// 패치 적용 결과 요약.
#[derive(Debug, Clone, Default)]
pub struct PatchApplyReport {
    pub created: usize,
    pub modified: usize,
    pub deleted: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

fn safe_join(root: &std::path::Path, name: &str) -> std::path::PathBuf {
    let mut path = root.to_path_buf();
    for component in name.split(['/', '\\']) {
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." || component.contains(':') {
            return root.join("__invalid_patch_path__");
        }
        path.push(component);
    }
    path
}

fn file_checksum(path: &std::path::Path) -> std::io::Result<u32> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut crc = 0u32;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        crc = compute_patch_checksum(&buf[..n], crc);
    }
    Ok(crc)
}

fn read_exact_at(file: &mut std::fs::File, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf)?;
    Ok(buf)
}

struct ModifyCursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> ModifyCursor<'a> {
    fn read_i32(&mut self) -> Result<i32> {
        let value = read_i32(self.bytes, &mut self.pos).ok_or(WzFormatError::Malformed {
            what: "patch modify command truncated",
            offset: self.pos as u64,
        })?;
        Ok(value)
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.pos + len > self.bytes.len() {
            return Err(WzFormatError::Malformed {
                what: "patch modify payload truncated",
                offset: self.pos as u64,
            }
            .into());
        }
        let slice = &self.bytes[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }
    fn read_string(&mut self) -> Result<String> {
        let len = self.read_i32()?;
        if len < 0 || len as usize > 4096 {
            return Err(WzFormatError::Malformed {
                what: "patch dependency name invalid",
                offset: self.pos as u64,
            }
            .into());
        }
        let bytes = self.take(len as usize)?;
        Ok(String::from_utf8_lossy(bytes).to_string())
    }
}

/// `.patch` 페이로드를 원본 디렉터리에 적용해 출력 디렉터리에 쓴다.
///
/// 원본 `WzPatcher.CreateNewFile` / `RebuildFile` 의 생성·수정·삭제 흐름을
/// 그대로 옮긴 것이다. 수정 레코드의 세 가지 명령(패치 복사, 바이트 채움,
/// 원본 구간 복사)과 KMST1125 의존 파일명까지 처리한다.
pub fn apply_patch(
    payload: &PatchPayload,
    old_root: &std::path::Path,
    out_root: &std::path::Path,
    options: &PatchApplyOptions,
) -> Result<PatchApplyReport> {
    use std::io::Write;

    let mut report = PatchApplyReport::default();
    std::fs::create_dir_all(out_root).map_err(|_| WzFormatError::Malformed {
        what: "cannot create patch output directory",
        offset: 0,
    })?;

    for part in &payload.parts {
        if part.file_name.is_empty() {
            report.skipped += 1;
            continue;
        }
        let target = safe_join(out_root, &part.file_name);
        match part.part_type {
            PatchPartType::Create => {
                if part.new_file_len == 0 && !part.file_name.contains('.') {
                    if let Err(e) = std::fs::create_dir_all(&target) {
                        report.errors.push(format!("{}: {e}", part.file_name));
                    } else {
                        report.created += 1;
                    }
                    continue;
                }
                let start = part.data_offset as usize;
                let end = start.saturating_add(part.new_file_len);
                if end > payload.bytes.len() {
                    report
                        .errors
                        .push(format!("{}: 생성 데이터 범위 초과", part.file_name));
                    continue;
                }
                let data = &payload.bytes[start..end];
                if options.verify_checksums {
                    if let Some(expected) = part.new_checksum {
                        let actual = compute_patch_checksum(data, 0);
                        if actual != expected {
                            report.errors.push(format!(
                                "{}: 생성 체크섬 불일치 (0x{expected:08X} != 0x{actual:08X})",
                                part.file_name
                            ));
                            continue;
                        }
                    }
                }
                if let Some(parent) = target.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&target, data) {
                    Ok(()) => report.created += 1,
                    Err(e) => report.errors.push(format!("{}: {e}", part.file_name)),
                }
            }
            PatchPartType::Delete => {
                let result = if target.is_dir() {
                    std::fs::remove_dir_all(&target)
                } else {
                    std::fs::remove_file(&target)
                };
                match result {
                    Ok(()) => report.deleted += 1,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => report.skipped += 1,
                    Err(e) => report.errors.push(format!("{}: {e}", part.file_name)),
                }
            }
            PatchPartType::Modify => {
                let old_path = safe_join(old_root, &part.file_name);
                if !old_path.exists() {
                    report
                        .errors
                        .push(format!("{}: 원본 파일 없음", part.file_name));
                    continue;
                }
                if options.verify_checksums {
                    if let Some(expected) = part.old_checksum {
                        match file_checksum(&old_path) {
                            Ok(actual) if actual != expected => {
                                report.errors.push(format!(
                                    "{}: 원본 체크섬 불일치 (0x{expected:08X} != 0x{actual:08X})",
                                    part.file_name
                                ));
                                continue;
                            }
                            Ok(_) => {}
                            Err(e) => {
                                report.errors.push(format!("{}: {e}", part.file_name));
                                continue;
                            }
                        }
                    }
                }
                let start = part.data_offset as usize;
                if start > payload.bytes.len() {
                    report
                        .errors
                        .push(format!("{}: 명령 오프셋 범위 초과", part.file_name));
                    continue;
                }
                let mut cursor = ModifyCursor {
                    bytes: &payload.bytes[start..],
                    pos: 0,
                };
                let mut old_file = match std::fs::File::open(&old_path) {
                    Ok(f) => f,
                    Err(e) => {
                        report.errors.push(format!("{}: {e}", part.file_name));
                        continue;
                    }
                };
                if let Some(parent) = target.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let mut out = match std::fs::File::create(&target) {
                    Ok(f) => f,
                    Err(e) => {
                        report.errors.push(format!("{}: {e}", part.file_name));
                        continue;
                    }
                };
                let mut crc = 0u32;
                let mut failed = None;
                loop {
                    let cmd = match cursor.read_i32() {
                        Ok(cmd) => cmd,
                        Err(e) => {
                            failed = Some(format!("{}: {e}", part.file_name));
                            break;
                        }
                    };
                    if cmd == 0 {
                        break;
                    }
                    let kind = (cmd as u32) >> 0x1C;
                    let result: Result<()> = (|| {
                        match kind {
                            0x08 => {
                                let len = (cmd as u32 & 0x0FFF_FFFF) as usize;
                                let block = cursor.take(len)?;
                                out.write_all(block).map_err(|_| WzFormatError::Malformed {
                                    what: "patch write failed",
                                    offset: 0,
                                })?;
                                crc = compute_patch_checksum(block, crc);
                            }
                            0x0C => {
                                let len = ((cmd as u32 & 0x0FFF_FF00) >> 8) as usize;
                                let fill = (cmd as u32 & 0xFF) as u8;
                                let buf = vec![fill; len];
                                out.write_all(&buf).map_err(|_| WzFormatError::Malformed {
                                    what: "patch fill write failed",
                                    offset: 0,
                                })?;
                                crc = compute_patch_checksum(&buf, crc);
                            }
                            _ => {
                                let len = cmd.max(0) as usize;
                                let start_pos =
                                    cursor.read_i32().map_err(|_| WzFormatError::Malformed {
                                        what: "patch old-copy offset invalid",
                                        offset: 0,
                                    })?;
                                let from_name = if payload.is_kmst1125 {
                                    cursor.read_string()?
                                } else {
                                    part.file_name.clone()
                                };
                                let from_path = safe_join(old_root, &from_name);
                                if from_path != old_path {
                                    old_file = std::fs::File::open(&from_path).map_err(|_| {
                                        WzFormatError::Malformed {
                                            what: "patch dependency file missing",
                                            offset: 0,
                                        }
                                    })?;
                                }
                                let block =
                                    read_exact_at(&mut old_file, start_pos.max(0) as u64, len)
                                        .map_err(|_| WzFormatError::Malformed {
                                            what: "patch old-copy read failed",
                                            offset: 0,
                                        })?;
                                out.write_all(&block)
                                    .map_err(|_| WzFormatError::Malformed {
                                        what: "patch old-copy write failed",
                                        offset: 0,
                                    })?;
                                crc = compute_patch_checksum(&block, crc);
                            }
                        }
                        Ok(())
                    })();
                    if let Err(e) = result {
                        failed = Some(format!("{}: {e}", part.file_name));
                        break;
                    }
                }
                if let Some(err) = failed {
                    let _ = std::fs::remove_file(&target);
                    report.errors.push(err);
                    continue;
                }
                out.flush().map_err(|_| WzFormatError::Malformed {
                    what: "patch flush failed",
                    offset: 0,
                })?;
                if options.verify_checksums {
                    if let Some(expected) = part.new_checksum {
                        if crc != expected {
                            report.errors.push(format!(
                                "{}: 수정 체크섬 불일치 (0x{expected:08X} != 0x{crc:08X})",
                                part.file_name
                            ));
                            let _ = std::fs::remove_file(&target);
                            continue;
                        }
                    }
                }
                report.modified += 1;
            }
        }
    }
    Ok(report)
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

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("wz-patch-test-{}-{tag}", std::process::id()))
    }

    #[test]
    fn apply_create_and_modify_records() {
        let root = temp_dir("apply");
        let old_root = root.join("old");
        let out_root = root.join("out");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&old_root).unwrap();
        std::fs::write(old_root.join("old.bin"), b"ABCXYZ").unwrap();

        let created = b"hello world";
        let mut bytes = created.to_vec();
        let create_checksum = compute_patch_checksum(created, 0);
        let modify_offset = bytes.len();
        let fill_cmd = (0x0Cu32 << 28) | (2u32 << 8) | u32::from(b'X');
        bytes.extend_from_slice(&fill_cmd.to_le_bytes());
        let copy_patch_cmd = (0x08u32 << 28) | 5u32;
        bytes.extend_from_slice(&copy_patch_cmd.to_le_bytes());
        bytes.extend_from_slice(b"PATCH");
        bytes.extend_from_slice(&4i32.to_le_bytes());
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        let expected = b"XXPATCHCXYZ";
        let modify_checksum = compute_patch_checksum(expected, 0);

        let payload = PatchPayload {
            bytes,
            is_kmst1125: false,
            old_hashes: Default::default(),
            parts: vec![
                PatchPart {
                    file_name: "new.bin".into(),
                    part_type: PatchPartType::Create,
                    old_checksum: None,
                    new_checksum: Some(create_checksum),
                    new_file_len: created.len(),
                    data_offset: 0,
                },
                PatchPart {
                    file_name: "old.bin".into(),
                    part_type: PatchPartType::Modify,
                    old_checksum: None,
                    new_checksum: Some(modify_checksum),
                    new_file_len: 0,
                    data_offset: modify_offset as u64,
                },
            ],
        };
        let report = apply_patch(
            &payload,
            &old_root,
            &out_root,
            &PatchApplyOptions::default(),
        )
        .unwrap();
        assert_eq!(report.created, 1);
        assert_eq!(report.modified, 1);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(std::fs::read(out_root.join("new.bin")).unwrap(), created);
        assert_eq!(std::fs::read(out_root.join("old.bin")).unwrap(), expected);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn real_patch_created_files_verify() {
        let Some(path) = std::env::var_os("WZ_PATCH_PATH").map(std::path::PathBuf::from) else {
            return;
        };
        if !path.exists() {
            return;
        }
        let src: Arc<dyn ByteSource> =
            Arc::new(wz_pool::FileSource::open(&path).expect("open real patch"));
        let header = PatchHeader::parse(&src).expect("header");
        assert!(header.is_valid);
        let payload = header.read_patch(&src).expect("read patch");
        assert!(payload.is_kmst1125);
        let created: Vec<_> = payload
            .parts
            .iter()
            .filter(|p| p.part_type == PatchPartType::Create && p.new_file_len > 0)
            .collect();
        assert_eq!(created.len(), 43);
        let total_modified: usize = payload
            .parts
            .iter()
            .filter(|p| p.part_type == PatchPartType::Modify)
            .map(|p| p.new_file_len)
            .sum();
        assert_eq!(total_modified, 42_701_118_706);
        let ng = payload
            .parts
            .iter()
            .find(|p| p.file_name == "NGClient64.aes")
            .expect("NGClient64.aes");
        assert_eq!(ng.new_file_len, 12_825_088);
        for part in created {
            let start = part.data_offset as usize;
            let data = &payload.bytes[start..start + part.new_file_len];
            if let Some(expected) = part.new_checksum {
                assert_eq!(
                    compute_patch_checksum(data, 0),
                    expected,
                    "{}",
                    part.file_name
                );
            }
        }
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
