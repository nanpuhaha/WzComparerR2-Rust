//! 버전 자동 감지 — 후보를 시도하고 오프셋으로 검증한다.
//!
//! 원본: `WzVersionVerifier.TryDetect` / `ValidateOffsets`
//! (`WzComparerR2.WzLib/Compatibility/WzVersionVerifier.cs:582-765`).
//!
//! # 핵심 아이디어
//!
//! 헤더의 `hash1`/`hash2` 로는 `hashVersion` 이 **여러 개**일 수 있다. 하지만
//! `hashVersion` 을 하나 고르면 **모든 이미지 오프셋이 결정**된다. 틀린 키는
//! 오프셋을 무작위에 가깝게 흩뿌리므로 세 가지 중 하나가 반드시 걸린다:
//!
//! 1. 오프셋이 파일 범위를 벗어난다
//! 2. 오프셋이 디렉터리 영역보다 앞선다
//! 3. **두 이미지의 바이트 구간이 겹친다** ← 가장 강력하다
//!
//! 세 번째가 결정적 판별자다. 무작위 오프셋 두 개가 겹치지 않을 확률은 사실상
//! 0 이므로, 올바른 `hashVersion` 은 유일하게 통과한다.
//!
//! # 원본과 다른 점 (의도적)
//!
//! 원본은 재귀적 `WzPreReadProvider` 로 "암호화 안 된 디렉터리 뼈대"를 먼저
//! 긁어낸 뒤 여러 해석을 시도한다. 여기서는 루트 블록 하나만 검증한다. 파일
//! 하나를 열기 전에 전체 트리를 복호화하는 것은 비싸고, 루트에서 판별이 안 되면
//! 그 파일은 어느 프로파일로도 안 열리기 때문이다.

use wz_pool::{ByteSource, ReadCtx};

use crate::detect::{DetectError, Detected};
use crate::profile::{candidates_for, Profile, WzFileFormat};

/// 감지 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    /// 확정된 프로파일.
    pub profile: &'static Profile,
    /// 확정된 `hashVersion`.
    pub hash_version: u64,
    /// 감지된 헤더 (전문 그대로).
    pub detected: Detected,
    /// 루트 블록의 엔트리 수 (PKG1 은 평문, PKG2 는 복호화 결과).
    pub root_entry_count: i32,
    /// 검증에 통과한 이미지 구간 수.
    pub validated_images: usize,
    /// 왜 이 프로파일을 선택했는지 (디버깅/사용자 표시용).
    pub reason: String,
}

/// 감지 실패 사유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionError {
    /// 헤더 감지 실패.
    Header(DetectError),
    /// 이 헤더 길이에 대응하는 프로파일이 없다.
    UnknownHeaderLen { header_len: u32 },
    /// 후보를 모두 시도했으나 통과한 것이 없다.
    NoCandidate {
        /// 시도한 (버전, 후보 수) 목록
        tried: Vec<(u32, usize)>,
    },
    /// 사용자가 `hashVersion` 을 강제했는데 검증이 실패했다.
    Rejected { wz_version: u32 },
}

impl std::fmt::Display for DetectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Header(e) => write!(f, "헤더 감지 실패: {e}"),
            Self::UnknownHeaderLen { header_len } => {
                write!(f, "알 수 없는 헤더 길이: {header_len}")
            }
            Self::NoCandidate { tried } => {
                write!(f, "통과하는 프로파일이 없다. 시도: {tried:?}")
            }
            Self::Rejected { wz_version } => {
                write!(f, "KMST{wz_version} 은 오프셋 검증에서 탈락했다")
            }
        }
    }
}

impl std::error::Error for DetectionError {}

impl From<DetectError> for DetectionError {
    fn from(e: DetectError) -> Self {
        Self::Header(e)
    }
}

/// 감지 옵션.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// 검사할 하위 디렉터리 깊이. 0 이면 루트 블록만.
    ///
    /// 함정: 깊이 1 만으로도 판별은 충분하다. 전체를 긁으면 큰 파일에서
    /// 수 초 걸리고, 잘못된 후보마다 그 비용을 다시 치른다.
    pub max_depth: usize,
    /// `hashVersion` 을 강제 (0 이면 자동)
    pub hash_version: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_depth: 1,
            hash_version: 0,
        }
    }
}

/// 파일을 열어볼 프로파일을 찾는다.
///
/// # 왜 `Arc<dyn ByteSource>` 를 받는가
///
/// `Arc` 는 `'static` 이라 빌려온 `&dyn` 을 담을 수 없다. 감지는 여러 블록을
/// 훑는 동안 **소스 하나**를 계속 재사용해야 하므로 `Arc` 로 소유하는 편이
/// 맞고, 그 결과 호출자는 `open_file_arc` / `Arc::new(MemorySource)` 를 쓴다.
/// `Arc<T>` 를 `Arc<dyn ByteSource>` 로 바꿔 준다.
#[cfg(test)]
fn arc_src<T: ByteSource + 'static>(v: T) -> std::sync::Arc<dyn ByteSource> {
    std::sync::Arc::new(v)
}
pub fn detect_version(
    src: &std::sync::Arc<dyn ByteSource>,
    opts: Options,
) -> Result<Detection, DetectionError> {
    let detected = crate::detect::detect(src.as_ref())?;
    let header_len = detected.header_len;
    // ── 1. 포맷 후보 좁히기 ──
    let formats: Vec<WzFileFormat> = if detected.profile.starts_with("PKG1") {
        vec![WzFileFormat::Pkg1]
    } else if detected.capabilities.pkg2_random_header {
        match WzFileFormat::from_random_header_len(header_len) {
            Some(f) => vec![f],
            None => return Err(DetectionError::UnknownHeaderLen { header_len }),
        }
    } else if detected.capabilities.pkg2_random_header64 {
        // 함정: 1202 와 1204 는 **헤더 길이로** 갈린다 (150 vs 200).
        // 1205/1206 도 각각 163/353 으로 유일하다.
        match WzFileFormat::from_random_header_len(header_len) {
            Some(f) => vec![f],
            None => return Err(DetectionError::UnknownHeaderLen { header_len }),
        }
    } else {
        // 표준 PKG2 헤더. `hash1`/`hash2` 로는 1196/1198/1201 이 안 갈린다.
        vec![
            WzFileFormat::Pkg2Kmst1201,
            WzFileFormat::Pkg2Kmst1198,
            WzFileFormat::Pkg2Kmst1196,
        ]
    };

    let hash1 = detected.hash1.unwrap_or(0);
    let hash2 = detected.hash2.unwrap_or(0);
    let file_len = src.len().unwrap_or(0);
    let mut tried: Vec<(u32, usize)> = Vec::new();
    // 후보마다 새 커서를 만들지 않고 하나를 공유한다 — 블록 오프셋으로 seek 하므로
    // 안전하며, `ByteSource` 가 `Arc` 로 감싸지지 않은 `&dyn` 여도 동작한다.
    let mut c = ReadCtx::from_source(src.clone());

    for format in formats {
        // 레지스트리 순서를 그대로 지킨다 (비단조!).
        for profile in candidates_for(format) {
            let candidates: Vec<u64> = if opts.hash_version != 0 {
                vec![opts.hash_version]
            } else if format == WzFileFormat::Pkg1 {
                pkg1_candidates(&detected)
            } else {
                profile.hash_version_candidates(hash1, hash2)
            };

            tried.push((profile.wz_version, candidates.len()));

            for hv in candidates {
                if let Some(d) = try_profile(
                    &mut c,
                    &detected,
                    profile,
                    hv,
                    file_len,
                    &opts,
                    dir_start_position(&detected),
                ) {
                    return Ok(d);
                }
            }
        }
    }

    if opts.hash_version != 0 {
        // 강제 모드에서는 실패를 명시해야 한다.
        return Err(DetectionError::NoCandidate { tried });
    }
    Err(DetectionError::NoCandidate { tried })
}

/// PKG1 `hashVersion` 후보.
fn pkg1_candidates(d: &Detected) -> Vec<u64> {
    if d.capabilities.encver_missing {
        return verify_pkg1_777();
    }
    // `encver` 바이트는 `hashVersion` 의 반전이다. 헤더에서 직접 읽는다.
    match read_encver(d) {
        Some(encver) => crate::verify::pkg1_hash_version_candidates(encver)
            .into_iter()
            .map(|(_, hv)| u64::from(hv))
            .collect(),
        None => Vec::new(),
    }
}

fn verify_pkg1_777() -> Vec<u64> {
    crate::verify::pkg1_fixed_candidate()
        .into_iter()
        .map(|(_, hv)| u64::from(hv))
        .collect()
}

/// PKG1 `encver` 바이트를 헤더에서 읽는다.
///
/// 감지 단계(`detect_pkg1`) 가 이미 읽어 `Detected.encver` 에 보관한다.
/// 여기서는 그 값을 쓴다.
///
/// # 함정: 이게 `None` 이면 PKG1 은 후보가 0 개가 된다
///
/// `hashVersion` 은 `encver` 의 반전이라 값이 없으면 후보를 만들 방법이 없다.
/// `encver_missing` 인 경우만 `777` 고정값을 쓰고, 나머지는 반드시 값이 있다.
fn read_encver(d: &Detected) -> Option<u32> {
    d.encver
}

/// 프로파일 하나를 실제로 시도한다. 검증 통과 시 `Some`.
#[allow(clippy::too_many_arguments)]
fn try_profile(
    c: &mut ReadCtx,
    detected: &Detected,
    profile: &'static Profile,
    hash_version: u64,
    file_len: u64,
    opts: &Options,
    dir_start: u64,
) -> Option<Detection> {
    // 함정: PKG1 에는 `hash1` 이 **없다.** `?` 로 걸면 PKG1 이 무조건 탈락한다.
    // 오프셋 계산기는 PKG2 에서만 `hash1` 을 쓴다.
    let calc = profile.offset_calc(
        detected.header_len,
        hash_version,
        detected.hash1.unwrap_or(0),
    );
    let is_pkg1 = profile.format == WzFileFormat::Pkg1;

    let mut sizes: Vec<(u64, u64)> = Vec::new();
    let mut entry_count = 0i32;
    let mut visited = vec![dir_start];
    let mut queue = std::collections::VecDeque::from([(dir_start, 0usize)]);

    while let Some((pos, depth)) = queue.pop_front() {
        if depth > opts.max_depth {
            break;
        }
        c.seek(pos).ok()?;
        let block = read_block(
            c,
            calc.as_ref(),
            is_pkg1,
            profile.name_position,
            calc.is_wide() && !is_pkg1,
        )?;

        if depth == 0 {
            entry_count = block.len() as i32;
        }

        for e in block {
            let Some(off) = e.offset else { continue };
            match e.node_type {
                // 이미지: [시작, 끝) 구간이 실제로 파일 안에 있어야 하고
                // 다른 이미지와 겹치면 안 된다.
                0x04 | 0x02 => {
                    if e.data_len < 0 {
                        return None;
                    }
                    let end = off + e.data_len as u64;
                    // 함정: 오버플로. 잘못된 키면 off 와 dataLen 이 튀어서
                    // 더하기가 wrap 하면 "범위 안" 으로 통과해 버릴 수 있다.
                    if end < off {
                        return None;
                    }
                    if off < dir_start || end > file_len {
                        return None;
                    }
                    sizes.push((off, end));
                }
                // 디렉터리: 파일 범위 안이어야 한다.
                0x03 => {
                    if off < dir_start || off >= file_len {
                        return None;
                    }
                    if !visited.contains(&off) {
                        visited.push(off);
                        if depth < opts.max_depth {
                            queue.push_back((off, depth + 1));
                        }
                    }
                }
                _ => return None,
            }
        }
    }

    // ★ 결정적 판별: 겹침 검사
    //
    // 오름차순 정렬 후 인접 구간이 겹치면 탈락. 샘플이 2개 미만이면 이 검사는
    // 아무 효과가 없으므로, 그 경우 다른 조건(개수/범위)만으로 통과시킨다.
    if sizes.len() >= 2 {
        sizes.sort_unstable();
        for w in sizes.windows(2) {
            if w[0].1 > w[1].0 {
                return None;
            }
        }
    }

    Some(Detection {
        profile,
        hash_version,
        detected: detected.clone(),
        root_entry_count: entry_count,
        validated_images: sizes.len(),
        reason: format!(
            "{} (엔트리 {}개, 이미지 {}개 검증)",
            profile.format.as_str(),
            entry_count,
            sizes.len()
        ),
    })
}

/// `DirStartPosition`.
fn dir_start_position(d: &Detected) -> u64 {
    if d.profile.starts_with("PKG1") {
        d.header_len as u64 + if d.capabilities.encver_missing { 0 } else { 2 }
    } else if d.capabilities.pkg2_random_header || d.capabilities.pkg2_random_header64 {
        d.header_len as u64
    } else {
        d.header_len as u64 + 8
    }
}

/// 블록 읽기용 최소 엔트리.
struct RawEntryLite {
    node_type: u8,
    data_len: i32,
    offset: Option<u64>,
}

/// 루트 블록을 읽는다. 실패하면 `None` (이 후보는 탈락).
fn read_block(
    c: &mut ReadCtx,
    calc: &dyn crate::offset::OffsetCalc,
    is_pkg1: bool,
    name_pos: crate::profile::EntryNamePosition,
    // PKG2 V2 이름인지 (64비트 PKG2 = 1205/1206).
    //
    // PKG2 V2 는 길이가 int16 이라 WZ 표준(i8) 경로와 **호환되지 않는다**.
    // 첫 이름은 하위 바이트가 우연히 맞아 보이지만 이후가 모두 어긋난다.
    pkg2_v2_name: bool,
) -> Option<Vec<RawEntryLite>> {
    use crate::read::{read_compressed_i32, read_compressed_i64};

    // 64비트 PKG2 는 압축 **i64** 이다 (`WzFileReadContext.cs:103`).
    // i32 로 읽으면 0x80 escape 가 4바이트 vs 8바이트로 어긋나
    // 엔트리 전체가 밀린다.
    let count = if is_pkg1 {
        read_compressed_i32(c).ok()?
    } else if calc.is_wide() {
        calc.decrypt_entry_count_i64(read_compressed_i64(c).ok()?) as i32
    } else {
        calc.decrypt_entry_count_i32(read_compressed_i32(c).ok()?)
    };
    // 엔트리 개수는 합리적이어야 한다. 0 은 빈 디렉터리로 허용.
    //
    // 200,000 상한은 정상이 아니라 **안전망**이다. 잘못된 `hashVersion` 은
    // 개수를 아무 값이나 복호화해 내고, 그 값으로 `Vec::with_capacity` 를 호출하면
    // OOM 으로 죽는다. 실제 WZ 의 최대 엔트리 수는 수천 개 수준이다.
    if !(0..=200_000).contains(&count) {
        return None;
    }
    let count = count as usize;

    let mut out = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        let node_type = c.read_u8().ok()?;
        if name_pos == crate::profile::EntryNamePosition::BeforeData {
            if pkg2_v2_name {
                skip_pkg2_v2_name(c)?
            } else {
                read_name(c)?
            }
        }
        let size_pos = c.pos();
        let mut data_len = read_compressed_i32(c).ok()?;
        // `cs32` 는 검증에 쓰지 않는다 (엔트리 수/오프셋만 쓴다) — 그래서
        // 길이 복호화를 해도 버린다. 다만 **읽기는 해야 한다** (커서 진행).
        let _cs32 = read_compressed_i32(c).ok()?;
        if let Some(l) = calc.calc_length(size_pos as u32, data_len) {
            data_len = l;
        }
        if name_pos == crate::profile::EntryNamePosition::AfterData {
            if pkg2_v2_name {
                skip_pkg2_v2_name(c)?
            } else {
                read_name(c)?
            }
        }

        // ★ PKG1 과 PKG2 의 결정적 차이
        //
        // PKG1: 해시 오프셋이 **엔트리 안**에 있다 → 여기서 바로 읽는다.
        // PKG2: 엔트리가 전부 끝난 뒤에 표가 따로 온다 → 아래에서 읽는다.
        //
        // 이걸 섞으면 PKG1 의 두 번째 엔트리부터 해시 바이트를 타입으로 읽는다
        // (관측: `entry 1: type=0x0e`).
        if is_pkg1 {
            let file_pos = c.pos();
            let hashed = c.read_u32_le().ok()?;
            out.push(RawEntryLite {
                node_type,
                data_len,
                offset: Some(calc.calc_offset(file_pos as u32, hashed) as u64),
            });
        } else {
            out.push(RawEntryLite {
                node_type,
                data_len,
                offset: None,
            });
        }
    }

    if !is_pkg1 {
        // 64비트는 표 앞에 카운트 필드가 없다 (`ShouldReadOffsets` 가 무조건
        // true, `WzFileReadContext.cs:110`). 카운트를 읽으면 1바이트씩 밀려
        // 모든 해시 오프셋이 깨진다.
        let have_table = if calc.is_wide() {
            true
        } else {
            let enc_off_count = read_compressed_i32(c).ok()?;
            calc.decrypt_entry_count_i32(enc_off_count) == count as i32
        };
        if have_table {
            for e in out.iter_mut() {
                let file_pos = c.pos();
                let hashed = c.read_u32_le().ok()?;
                e.offset = Some(calc.calc_offset(file_pos as u32, hashed) as u64);
            }
        }
    }
    Some(out)
}

/// 이름을 건너뛴다 — 검증에는 이름이 필요 없다.
/// PKG2 V2 이름의 **바이트 수만** 건너뛴다.
///
/// # 왜 복호화가 필요 없는가
///
/// 버전 검증은 이름 **내용**이 아니라 이름이 소비한 **바이트 수** 로 커서를
/// 움직여야 한다. 키스트림을 계산할 필요도 없다 — int16 길이만 읽고
/// `size * 2` 만큼 건너뛰면 충분하다.
///
/// # 함정: 길이가 **int16** 이다
///
/// WZ 표준은 i8 이다. `int16` `-9` 의 하위 바이트 `0xF7` 을 i8 로 읽으면
/// 우연히 `-9` 가 되어 첫 이름은 맞아 보이고, **두 번째 이름부터** 어긋난다.
/// 검증이 "이름 개수만 맞는 파일"에서만 실패하는 이유가 이것이다.
///
/// # 함정: 0 은 빈 문자열
///
/// `size == 0` 이면 데이터가 없다. `size > 0` 이면 V2 규칙 위반이다
/// (`WzBinaryReader.cs:208`).
fn skip_pkg2_v2_name(c: &mut ReadCtx) -> Option<()> {
    let raw = c.read_i16_le().ok()?;
    if raw >= 0 {
        // 0 = 빈 문자열. 양수는 이 규칙에서 나오지 않는다.
        return (raw == 0).then_some(());
    }
    let n = usize::try_from(-i32::from(raw)).ok()?;
    if n > 0x10000 {
        return None;
    }
    c.skip((n * 2) as u64).ok()
}

fn read_name(c: &mut ReadCtx) -> Option<()> {
    let n = c.read_i8().ok()?;
    let count: usize = if n == 0 {
        0
    } else if n > 0 {
        if n == 127 {
            let v = c.read_i32_le().ok()?;
            if v < 0 {
                return None;
            }
            v as usize
        } else {
            n as usize
        }
    } else if n == -128 {
        let v = c.read_i32_le().ok()?;
        if v < 0 {
            return None;
        }
        v as usize
    } else {
        (-(n as i32)) as usize
    };
    // ASCII ��� UTF-16 폭이 다르다.
    let bytes = if n > 0 { count * 2 } else { count };
    if bytes > 64 * 1024 * 1024 {
        return None;
    }
    c.skip(bytes as u64).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_pool::MemorySource;

    /// PKG1 루트 블록을 만들고 각 엔트리의 `hashedOffset` **상대 위치**를 돌려준다.
    ///
    /// # 함정: 압축 정수를 1바이트로 덮어쓰면 안 된다
    ///
    /// `200` 을 `0xC8` 한 바이트로 쓰면 리더는 `-56` 으로 읽는다 (부호 확장).
    /// 위치 계산도 이 때문에 틀어지므로 반드시 `encode_compressed_i32` 을 쓴다.
    fn pkg1_block(entries: &[(&str, u8, i32)]) -> (Vec<u8>, Vec<usize>) {
        use crate::read::encode_compressed_i32;
        let mut block = Vec::new();
        let mut positions = Vec::new();
        block.extend_from_slice(&encode_compressed_i32(entries.len() as i32));
        for (name, ty, len) in entries {
            block.push(*ty);
            let units: Vec<u16> = name.encode_utf16().collect();
            block.push(units.len() as u8);
            for (i, u) in units.iter().enumerate() {
                block.extend_from_slice(&(u ^ (0xAAAAu16.wrapping_add(i as u16))).to_le_bytes());
            }
            block.extend_from_slice(&encode_compressed_i32(*len));
            block.extend_from_slice(&encode_compressed_i32(0));
            positions.push(block.len());
            block.extend_from_slice(&0u32.to_le_bytes());
        }
        (block, positions)
    }

    fn pkg1_header(mut file: Vec<u8>) -> Vec<u8> {
        let mut head = vec![0u8; 60];
        head[0..4].copy_from_slice(b"PKG1");
        head[12..16].copy_from_slice(&60i32.to_le_bytes());
        for b in head[16..60].iter_mut() {
            *b = b' ';
        }
        file.splice(0..0, head);
        let data_size = (file.len() - 60) as i64;
        file[4..12].copy_from_slice(&data_size.to_le_bytes());
        file
    }

    /// `hashed` 가 전부 0 인 파일 (잘못된 키 시뮬레이션용).
    fn pkg1_file_zero_hashes(entries: &[(&str, u8, i32)]) -> Vec<u8> {
        let (block, _) = pkg1_block(entries);
        pkg1_header(block)
    }

    /// 올바른 `hashedOffset` 를 계산한 PKG1 파일.
    ///
    /// # 두 가지가 동시에 맞아야 한다
    ///
    /// 1. `hashedOffset` 가 정확히 그 오프셋으로 **복호화**되게 계산되어야 하고
    /// 2. 그 오프셋이 **파일 안에** 있어야 한다 (`end <= file_len`).
    ///
    /// 해시를 임의 값(0)으로 두면 1이 깨지고, 임의 오프셋을 넣으면 2가 깨진다.
    /// 둘 다 깨지면 **감지가 실패해야 하고**, 그게 정답이다 — 그래서
    /// `zero_hashed_offsets_fail_detection` 이 짝이 된다.
    ///
    /// 이미지는 블록 **뒤에** 실제 페이로드를 두고 그 시작점을 목표로 삼는다.
    fn pkg1_file_valid(entries: &[(&str, u8, i32, i32)]) -> Vec<u8> {
        use crate::offset::OffsetCalc;
        let hv = crate::verify::compute_pkg1_hash_version(777);
        let calc = crate::offset::Pkg1OffsetCalc::new(60, hv);
        let dir_start = 60usize;
        let specs: Vec<(&str, u8, i32)> = entries.iter().map(|(n, t, l, _)| (*n, *t, *l)).collect();
        let (mut block, positions) = pkg1_block(&specs);

        // 이미지 페이로드를 블록 뒤에 배치한다.
        let mut file = pkg1_header(block.clone());
        let mut targets = Vec::new();
        for (_, _, _, want_len) in entries {
            let at = file.len();
            let want = *want_len as usize;
            if file.len() < at + want {
                file.resize(at + want, 0xAB);
            }
            targets.push(at as u32);
        }

        for (i, _) in entries.iter().enumerate() {
            let h = calc.encode_offset((dir_start + positions[i]) as u32, targets[i]);
            block[positions[i]..positions[i] + 4].copy_from_slice(&h.to_le_bytes());
        }
        file[dir_start..dir_start + block.len()].copy_from_slice(&block);
        file
    }

    #[test]
    fn detects_pkg1_encver_missing() {
        let f = pkg1_file_valid(&[
            ("base.img", 0x04, 100, 100),
            ("Map.img", 0x04, 200, 200),
            ("Item.img", 0x04, 300, 300),
        ]);
        let src = arc_src(MemorySource::new(f));
        let d = detect_version(&src, Options::default()).expect("감지");
        assert_eq!(d.profile.format, WzFileFormat::Pkg1);
        assert_eq!(d.profile.wz_version, 777);
        assert_eq!(
            d.hash_version as u32,
            crate::verify::compute_pkg1_hash_version(777)
        );
        assert_eq!(d.validated_images, 3, "이미지 3개 검증");
    }

    #[test]
    fn rejects_unknown_magic() {
        let src = arc_src(MemorySource::new(vec![0u8; 64]));
        assert!(matches!(
            detect_version(&src, Options::default()),
            Err(DetectionError::Header(_))
        ));
    }

    #[test]
    fn wrong_hash_version_is_rejected() {
        // 함정: 틀린 hashVersion 은 오프셋을 무작위로 흩뿌려 겹침 검사에 걸린다.
        let f = pkg1_file_valid(&[
            ("a.img", 0x04, 100, 100),
            ("b.img", 0x04, 200, 200),
            ("c.img", 0x04, 300, 300),
        ]);
        let src = arc_src(MemorySource::new(f));
        let ok = detect_version(&src, Options::default()).expect("정답은 통과");
        let good = ok.hash_version;
        assert_ne!(good, 0);

        let bad = detect_version(
            &src,
            Options {
                max_depth: 1,
                hash_version: good.wrapping_add(1),
            },
        );
        assert!(
            bad.is_err(),
            "틀린 hashVersion 이 통과하면 안 된다 (검증 규칙 확인 필요)"
        );
    }

    #[test]
    fn zero_hashed_offsets_fail_detection() {
        // 함정: 해시를 0 으로 두면 감지가 실패해야 한다. 이 테스트가
        // "검증이 실제로 작동한다" 를 증명한다 — 항상 통과하면 무의미하다.
        let f = pkg1_file_zero_hashes(&[
            ("a.img", 0x04, 100),
            ("b.img", 0x04, 200),
            ("c.img", 0x04, 300),
        ]);
        let src = arc_src(MemorySource::new(f));
        assert!(
            detect_version(&src, Options::default()).is_err(),
            "해시 0 은 잘못된 키 → 탈락해야 한다"
        );
    }

    #[test]
    fn dir_start_matches_capabilities() {
        let mut d = Detected {
            profile: "PKG1_ENCVER_MISSING".into(),
            hash1: None,
            hash2: None,
            header_len: 60,
            encver: None,
            capabilities: crate::detect::Capabilities {
                encver_missing: true,
                ..Default::default()
            },
        };
        assert_eq!(dir_start_position(&d), 60, "encver 없음 → headerSize");
        d.capabilities.encver_missing = false;
        assert_eq!(dir_start_position(&d), 62, "encver 있음 → +2");
        d.profile = "PKG2_STANDARD_HEADER".into();
        assert_eq!(dir_start_position(&d), 68, "표준 PKG2 → +8");
        d.capabilities.pkg2_random_header64 = true;
        assert_eq!(dir_start_position(&d), 60, "랜덤 헤더 → headerSize");
    }

    #[test]
    fn name_skip_handles_all_length_forms() {
        // UTF-16 직접 (1바이트 접두)
        let mut c = ReadCtx::from_bytes(vec![3, 0, 0, 0, 0, 0, 0xAA]);
        assert!(read_name(&mut c).is_some());
        // ASCII (음수 접두)
        let mut c = ReadCtx::from_bytes(vec![0xFD, 1, 2, 3]);
        assert!(read_name(&mut c).is_some());
        // 빈 문자열
        let mut c = ReadCtx::from_bytes(vec![0]);
        assert!(read_name(&mut c).is_some());
        // 이스케이프 (127 + i32)
        let mut c = ReadCtx::from_bytes({
            let mut v = vec![127u8];
            v.extend_from_slice(&2i32.to_le_bytes());
            v.extend_from_slice(&[0; 4]);
            v
        });
        assert!(read_name(&mut c).is_some());
        // 음수 i32 길이는 거부
        let mut c = ReadCtx::from_bytes({
            let mut v = vec![127u8];
            v.extend_from_slice(&(-5i32).to_le_bytes());
            v
        });
        assert!(read_name(&mut c).is_none(), "음수 길이 거부");
    }
}
