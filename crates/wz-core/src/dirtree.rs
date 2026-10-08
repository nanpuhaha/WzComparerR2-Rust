//! WZ 디렉터리 트리 파서 (원본 `Wz_File.ReadDirTree` / `ReadDirTreePkg2`).
//!
//! 원본: `WzComparerR2.WzLib/Wz_File.cs:497-646`.
//!
//! # 이 모듈이 하는 일
//!
//! WZ 파일은 `DirStartPosition` 에서 시작하는 **엔트리 테이블**을 가진다.
//! 각 엔트리는 이름과 **난독화된 오프셋**만 갖고 있고, 실제 오프셋은
//! `OffsetCalc` 로 재계산해야 한다 (모듈 `offset`). 이 모듈은 그 테이블을
//! 읽어 트리를 만든다.
//!
//! # 엔트리 타입 바이트 — 세 개만 존재한다
//!
//! | 값 | 의미 |
//! |---|---|
//! | `0x02` | 이미지. 단 **이름이 i32 오프셋 참조**다 (PKG1 `Data.wz` 계열) |
//! | `0x03` | 하위 디렉터리 |
//! | `0x04` | 일반 이미지 (`.img`) |
//!
//! **캔버스도 UOL 도 없다** — 그런 타입은 없다. 둘 다 이미지 **내부** 노드다.
//! `0x00` / `0x01` 은 이미지 안에서 문자열을 읽을 때 쓰는 플래그이고,
//! 디렉터리 테이블의 타입이 아니다. 그 외 값은 오류다 (원본이 예외를 던진다).
//!
//! # PKG1 과 PKG2 의 구조가 다르다
//!
//! PKG1 은 **엔트리가 자기 안에 해시 오프셋을 갖고** 있다:
//!
//! ```text
//! [type][name][size][cs32][hashedOffset]
//! ```
//!
//! PKG2 는 **엔트리들을 먼저 전부 읽고**, 그 뒤에 해시 오프셋 표를 붙인다:
//!
//! ```text
//! [encryptedCount][entry 0][entry 1] ... [entry N]
//! [offsetCount][hashedOffset 0] ... [hashedOffset N]
//! ```
//!
//! 그 이유는 PKG2 가 **엔트리 개수 자체를 암호화**해서 표의 경계를 모르게
//! 하기 위해서다. `offsetCount` 를 복호화한 값이 `encryptedCount` 와 같아야
//! 뒤에 표가 있다는 뜻이다 (원본 `ShouldReadOffsets`).

use std::sync::Arc;

use wz_pool::{ReadCtx, ReadCtxExt};

use crate::detect::Detected;
use crate::offset::OffsetCalc;
use crate::read::{read_compressed_i32, read_compressed_i64};

/// 디렉터리 엔트리 타입 바이트.
pub mod node_type {
    /// 이미지 — 이름이 i32 오프셋 참조 (PKG1 전용).
    pub const IMAGE_NAMED_BY_REF: u8 = 0x02;
    /// 하위 디렉터리.
    pub const DIRECTORY: u8 = 0x03;
    /// 일반 이미지 (`.img`).
    pub const IMAGE: u8 = 0x04;
}

/// PKG2 엔트리 안에서 이름이 놓이는 위치.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryNamePosition {
    /// `[type][name][size][cs32]` — KMST1196–1204 기본값.
    BeforeData,
    /// `[type][size][cs32][name]` — KMST1205 / 1206.
    ///
    /// 함정: 이 두 버전은 **헤더가 163 / 353 바이트**라 KMST1201(68) 과
    /// 우연히 겹칠 수 없다. 그래도 순서가 틀어지면 이름이 길이로 읽혀
    /// 트리가 통째로 어긋난다.
    AfterData,
}

/// 디렉터리 엔트리 하나의 원시 기록.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEntry {
    pub node_type: u8,
    /// 아직 해석 전 이름. `IMAGE_NAMED_BY_REF` 면 `None` (오프셋만 존재).
    pub name: Option<Arc<str>>,
    /// `IMAGE_NAMED_BY_REF` 인 경우 이름 오프셋 (i32, 해독 전).
    pub name_ref: Option<i32>,
    /// 데이터 길이. KMST1202+ 는 복호화된 값.
    pub data_len: i32,
    /// 체크섬/알 수 없음. KMST1202+ 는 복호화된 값.
    pub cs32: i32,
    /// 파일 절대 위치 (하위 디렉터리 블록의 시작점).
    pub file_pos: u64,
    /// 복호화된 실제 오프셋. `None` 이면 `file_pos` (= 검증 통과 실패).
    pub offset: Option<u64>,
}

/// 디렉터리 블록 하나 (하위 디렉터리 하나에 대응).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirBlock {
    pub entries: Vec<RawEntry>,
}

/// 디렉터리 트리 파싱 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirTree {
    /// 최상위 블록. 비어 있을 수 있다.
    pub root: DirBlock,
    /// 하위 디렉터리 블록들. `root` 의 `DIRECTORY` 엔트리 순서로 평탄화된다.
    ///
    /// 원본은 재귀 호출로 읽지만 (`Wz_File.cs:452-470`) 실제로는
    /// **깊이 우선 전위 순회**로 한.reader 를 공유한다. 그래서 평탄 리스트로
    /// 충분하다 — 순서가 재현되기만 하면 된다.
    pub blocks: Vec<DirBlock>,
}

/// 파싱 설정.
pub struct DirTreeContext<'a> {
    /// 감지된 컨테이너 (헤더 형식 판정에 쓴다).
    pub detected: &'a Detected,
    /// 오프셋 계산기. `None` 이면 오프셋을 계산하지 않는다 (디버깅용).
    pub offset_calc: Option<&'a dyn OffsetCalc>,
    /// 디렉터리 블록 시작 파일 위치.
    ///
    /// 함정: 이 값은 `dataSize` 가 **아니다**. 원본은
    /// `fileSize - DirStartPosition` 만큼을 길이로 쓴다
    /// (`Wz_File.cs:370-376`) — 저장된 `dataSize` 은 어디에도 쓰이지 않는다.
    pub dir_start: u64,
    /// PKG2 엔트리 개수 복호화기.
    pub pkg2: bool,
    /// PKG2 에서 해시 오프셋 표를 읽을지.
    pub read_offset_table: bool,
    /// PKG2 이름 위치.
    pub name_position: EntryNamePosition,
    /// PKG2 V2 이름 복호화에 필요한 입력 (`hash1`, `hashVersion`).
    ///
    /// # 왜 값을 보관하는가
    ///
    /// 키스트림은 **이름 위치마다** 다르다 (`ApplyState(filePosition)`).
    /// 첫 이름의 키 하나만 저장하면 두 번째 이름부터 틀린다. 그래서 원본
    /// 입력값을 보관해 호출 시점에 키를 만든다.
    ///
    /// `None` 이면 PKG2 V2 경로를 쓰지 않는다.
    pub pkg2_name_key_material: Option<Pkg2NameKeyMaterial>,
}

/// PKG2 V2 이름 키스트림 입력값.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pkg2NameKeyMaterial {
    pub hash1: u64,
    pub hash_version: u64,
}

// `dyn OffsetCalc` 에는 `Debug` 가 없다 (`OffsetCalc::name` 로 대신 출력한다).
impl std::fmt::Debug for DirTreeContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirTreeContext")
            .field("profile", &self.detected.profile)
            .field("offset_calc", &self.offset_calc.map(|c| c.name()))
            .field("dir_start", &self.dir_start)
            .field("pkg2", &self.pkg2)
            .field("read_offset_table", &self.read_offset_table)
            .field("name_position", &self.name_position)
            .finish()
    }
}

/// 디렉터리 트리 파서.
#[derive(Debug)]
pub struct DirTreeParser<'a> {
    ctx: DirTreeContext<'a>,
    /// 재귀 깊이 제한.
    ///
    /// # 왜 필요하나
    ///
    /// 악의로 만든 파일(또는 손상된 파일)이 자기 자신을 가리키는 디렉터리
    /// 엔트리를 만들 수 있다. 원본은 재귀라 스택 오버플로로 죽지만, Rust 는
    /// 파서를 크래시 대신 에러로 돌려 GUI 가 살아남아야 한다.
    max_depth: usize,
}

impl<'a> DirTreeParser<'a> {
    pub fn new(ctx: DirTreeContext<'a>) -> Self {
        Self { ctx, max_depth: 64 }
    }

    pub fn with_max_depth(mut self, n: usize) -> Self {
        self.max_depth = n;
        self
    }

    /// 파일 전체를 파싱한다.
    pub fn parse(&self, c: &mut ReadCtx) -> Result<DirTree, DirTreeError> {
        c.seek(self.ctx.dir_start)
            .map_err(|e| DirTreeError::Io(e.to_string()))?;

        let mut blocks: Vec<DirBlock> = Vec::new();
        let mut pending: Vec<(u64, usize)> = vec![(self.ctx.dir_start, 0)];
        let file_len = c.len().unwrap_or(0);

        // 명시적 스택으로 깊이 우선 전위 순회. 원본의 재귀와 같은 순서.
        while let Some((pos, depth)) = pending.pop() {
            if depth > self.max_depth {
                return Err(DirTreeError::DepthExceeded { depth });
            }
            c.seek(pos).map_err(|e| DirTreeError::Io(e.to_string()))?;
            let block = self.read_block(c, depth)?;

            // 하위 디렉터리 오프셋을 **뒤에서부터** 넣어야 pop() 순서가
            // 진입 순서와 같아진다. 정방향으로 넣으면 마지막 자식이 먼저 나온다.
            for e in block.entries.iter().rev() {
                if e.node_type == node_type::DIRECTORY {
                    if let Some(off) = e.offset {
                        // 함정: 하위 블록 오프셋이 범위 밖이어도 **전체 파싱을
                        // 실패시키지 않는다.** 원본은 예외를 던지지만, 우리는
                        // 이미 읽은 루트를 살려야 한다 — GUI 를 죽이는 대신
                        // 부분 결과를 보여주는 게 낫다.
                        if off < file_len {
                            pending.push((off, depth + 1));
                        }
                    }
                }
            }
            blocks.push(block);
        }

        let root = if blocks.is_empty() {
            DirBlock {
                entries: Vec::new(),
            }
        } else {
            blocks.remove(0)
        };
        Ok(DirTree { root, blocks })
    }

    /// 블록 하나만 읽는다 — 하위 디렉터리를 따라가지 않는다.
    ///
    /// 큰 WZ 파일(수천 개 디렉터리)에서 전체 순회는 수 초 걸릴 수 있다.
    /// `wz tree --shallow` 는 루트 목록만 보고 싶을 때 쓴다.
    pub fn read_root_only(&self, c: &mut ReadCtx) -> Result<DirBlock, DirTreeError> {
        self.read_block(c, 0)
    }

    /// 블록 하나를 읽는다 (현재 커서 위치에서).
    fn read_block(&self, c: &mut ReadCtx, depth: usize) -> Result<DirBlock, DirTreeError> {
        let _ = depth;
        if self.ctx.pkg2 {
            self.read_block_pkg2(c)
        } else {
            self.read_block_pkg1(c)
        }
    }

    // ── PKG1 ────────────────────────────────────────────────────────────────

    /// PKG1 블록 (`Wz_File.cs:497-546`).
    ///
    /// ```text
    /// [compressed int count]
    /// count × [ type(1) | name | size | cs32 | hashedOffset(4 LE) ]
    /// ```
    ///
    /// 함정: `count` 는 **평문**이다 (PKG2 와 다르다).
    fn read_block_pkg1(&self, c: &mut ReadCtx) -> Result<DirBlock, DirTreeError> {
        let count = read_compressed_i32(c).map_err(DirTreeError::from)?;
        if count < 0 {
            return Err(DirTreeError::NegativeCount { count });
        }
        let count = usize::try_from(count).map_err(|_| DirTreeError::CountTooLarge { count })?;

        let mut entries = Vec::with_capacity(count.min(4096));
        for _ in 0..count {
            let node_type = c.read_u8().map_err(DirTreeError::io)?;
            let (name, name_ref) = self.read_entry_name_pkg1(c, node_type)?;
            let data_len = read_compressed_i32(c).map_err(DirTreeError::from)?;
            let cs32 = read_compressed_i32(c).map_err(DirTreeError::from)?;
            // `pos` 는 hashedOffset **바로 앞** 절대 위치
            let file_pos = c.pos();
            let hashed_offset = c.read_u32_le().map_err(DirTreeError::io)?;
            let offset = self
                .ctx
                .offset_calc
                .map(|oc| oc.calc_offset(file_pos as u32, hashed_offset) as u64);

            entries.push(RawEntry {
                node_type,
                name,
                name_ref,
                data_len,
                cs32,
                file_pos,
                offset,
            });
        }
        Ok(DirBlock { entries })
    }

    /// PKG1 엔트리 이름 (`Wz_File.cs:504-514`).
    ///
    /// # 함정: 타입 0x02 는 이름을 **읽지 않는다**
    ///
    /// 대신 i32 오프셋을 읽고 그 위치로 넘어가 이름을 찾는다 (KMST1125
    /// `Data.wz` 계열의 이름 중복 제거 방식).
    ///
    /// # 함정: `stringOffAdd` 는 +2 와 -1 이 **다르다**
    ///
    /// `encver` 가 있으면 `-1`, 없으면 `+2`. 1바이트 차이가 나지만 둘 다
    /// 원본 그대로다 — 고치지 않는다.
    fn read_entry_name_pkg1(
        &self,
        c: &mut ReadCtx,
        node_type: u8,
    ) -> Result<(Option<Arc<str>>, Option<i32>), DirTreeError> {
        match node_type {
            node_type::IMAGE_NAMED_BY_REF => {
                let raw = c.read_i32_le().map_err(DirTreeError::io)?;
                let add = if self.ctx.detected.capabilities.encver_missing {
                    2
                } else {
                    -1
                };
                let rel = i64::from(raw) + i64::from(add);
                let at = (rel + self.ctx.dir_start as i64).max(0) as u64;
                let name = self.read_name_at(c, at)?;
                Ok((Some(name), Some(raw)))
            }
            node_type::IMAGE | node_type::DIRECTORY => {
                let n = self.read_name_here(c)?;
                Ok((Some(n), None))
            }
            other => Err(DirTreeError::UnknownNodeType { node_type: other }),
        }
    }

    // ── PKG2 ────────────────────────────────────────────────────────────────

    /// PKG2 블록 (`Wz_File.cs:548-646`).
    ///
    /// # 함정: 엔트리 개수가 암호화되어 있다
    ///
    /// `offsetCalc` 이 없으면 개수를 못 읽는다 — 원본은 이때 0 을 반환한다.
    fn read_block_pkg2(&self, c: &mut ReadCtx) -> Result<DirBlock, DirTreeError> {
        // # 함정: 64비트 PKG2 의 개수는 **압축 i64** 다
        //
        // `Pkg2DirTreeReadRule64.ReadEntryCount` 는 `ReadCompressedInt64()` 이다
        // (`WzFileReadContext.cs:103`). i32 로 읽으면 `0x80` escape 가 4바이트
        // vs 8바이트로 어긋나 **엔트리 전체가 1바이트씩 밀린다.**
        //
        // 실측 (`_Canvas_041.wz`): 9바이트로 읽어야 첫 `nodeType` 이 362 에서
        // 0x04 로 맞아떨어진다.
        let count = match self.ctx.offset_calc {
            Some(oc) if oc.is_wide() => {
                oc.decrypt_entry_count_i64(read_compressed_i64(c).map_err(DirTreeError::from)?)
            }
            Some(oc) => i64::from(
                oc.decrypt_entry_count_i32(read_compressed_i32(c).map_err(DirTreeError::from)?),
            ),
            None => 0,
        };
        if count < 0 {
            return Err(DirTreeError::NegativeCount {
                count: count as i32,
            });
        }
        let count = usize::try_from(count).map_err(|_| DirTreeError::CountTooLarge {
            count: count as i32,
        })?;

        let mut entries: Vec<RawEntry> = Vec::with_capacity(count.min(4096));
        for _ in 0..count {
            let node_type = c.read_u8().map_err(DirTreeError::io)?;
            if node_type != node_type::IMAGE && node_type != node_type::DIRECTORY {
                return Err(DirTreeError::UnknownNodeType { node_type });
            }
            let mut entry = RawEntry {
                node_type,
                name: None,
                name_ref: None,
                data_len: 0,
                cs32: 0,
                file_pos: 0,
                offset: None,
            };

            // 이름 위치는 버전마다 다르다.
            if self.ctx.name_position == EntryNamePosition::BeforeData {
                entry.name = Some(self.read_name_here(c)?);
            }

            // `size` 의 **절대 위치**를 기억한다 — 길이 복호화 키에 쓰인다.
            let size_pos = c.pos();
            let mut data_len = read_compressed_i32(c).map_err(DirTreeError::from)?;
            let cs_pos = c.pos();
            let mut cs32 = read_compressed_i32(c).map_err(DirTreeError::from)?;

            if let Some(oc) = self.ctx.offset_calc {
                if let Some(l) = oc.calc_length(size_pos as u32, data_len) {
                    data_len = l;
                }
                if let Some(l) = oc.calc_length(cs_pos as u32, cs32) {
                    cs32 = l;
                }
            }

            if self.ctx.name_position == EntryNamePosition::AfterData {
                entry.name = Some(self.read_name_here(c)?);
            }

            entry.data_len = data_len;
            entry.cs32 = cs32;
            entries.push(entry);
        }

        // 해시 오프셋 표.
        //
        // 함정: 표가 **있다고 가정하면 안 된다.** `offsetCount` 를 복호화한 값이
        // `count` 와 같을 때만 표가 뒤따른다 (원본 `ShouldReadOffsets`).
        // 안 맞아도 원본은 개수를 **0** 이 아니라 비교 결과로 처리하므로,
        // 여기서는 표를 건너뛰고 오프셋 미해결 상태로 둔다.
        if self.ctx.read_offset_table && count > 0 {
            // # 함정: 64비트는 표 앞에 **카운트 필드가 없다**
            //
            // `Pkg2DirTreeReadRule64.ShouldReadOffsets` 는 무조건 `true` 를 반환하며
            // 아무것도 읽지 않는다 (`WzFileReadContext.cs:110`). 32비트만 카운트를
            // 읽어 엔트리 개수와 비교한다.
            //
            // 실측 (`Cash_000.wz`, 45개 이미지): 엔트리 45개를 다 읽은 뒤 위치에서
            // 바로 u32 해시 오프셋 45개가 이어진다. 카운트 필드를 넣으면 1바이트씩
            // 밀려 모든 해시가 깨진다.
            let wide = self.ctx.offset_calc.map(|oc| oc.is_wide()).unwrap_or(false);
            let matches = if wide {
                true
            } else {
                let enc_off_count = read_compressed_i32(c).map_err(DirTreeError::from)?;
                self.ctx
                    .offset_calc
                    .map(|oc| oc.decrypt_entry_count_i32(enc_off_count) == count as i32)
                    .unwrap_or(false)
            };
            if matches {
                for entry in entries.iter_mut() {
                    let file_pos = c.pos();
                    let hashed = c.read_u32_le().map_err(DirTreeError::io)?;
                    entry.file_pos = file_pos;
                    entry.offset = self
                        .ctx
                        .offset_calc
                        .map(|oc| oc.calc_offset(file_pos as u32, hashed) as u64);
                }
            } else {
                for entry in entries.iter_mut() {
                    entry.file_pos = c.pos();
                }
            }
        } else {
            let pos = c.pos();
            for entry in entries.iter_mut() {
                entry.file_pos = pos;
            }
        }

        Ok(DirBlock { entries })
    }

    // ── 이름 읽기 ───────────────────────────────────────────────────────────

    /// 현재 커서 위치에서 이름 하나를 읽는다.
    ///
    /// `i8` 길이 접두:
    /// - `> 0` → UTF-16LE, `n * 2` 바이트. `n == 127` 이면 뒤의 i32 가 실제 길이.
    /// - `< 0` → ASCII, `-n` 바이트. `-128` 이면 뒤의 i32 가 실제 길이.
    /// - `== 0` → 빈 문자열.
    ///
    /// 함정: 마스크는 `(byte)(0xAA + i)`, 암호화는 **페이로드만** 한다
    /// (`WzBinaryReader.cs:98-150`).
    fn read_name_here(&self, c: &mut ReadCtx) -> Result<Arc<str>, DirTreeError> {
        // PKG2 V2 는 길이·암호화 방식이 WZ 표준과 완전히 다르므로 분기한다.
        //
        // 키스트림이 있는 경우에만 V2 경로를 탄다. 1205/1206 프로파일이 이에 해당.
        if self.ctx.pkg2_name_key_material.is_some() {
            return self.read_pkg2_v2_name(c);
        }
        let start = c.pos();
        self.read_name_body(c, start)
    }

    /// 절대 위치에서 이름을 읽고 커서를 원위치로 되돌린다
    /// (`WzBinaryReader.ReadStringAt`, `:421-432`).
    fn read_name_at(&self, c: &mut ReadCtx, at: u64) -> Result<Arc<str>, DirTreeError> {
        let save = c.pos();
        c.seek(at).map_err(|e| DirTreeError::Io(e.to_string()))?;
        let r = self.read_name_body(c, at);
        c.seek(save).map_err(|e| DirTreeError::Io(e.to_string()))?;
        r
    }

    /// PKG2 V2 디렉트리 이름 읽기 (`WzBinaryReader.ReadPkg2DirStringV2`).
    ///
    /// # WZ 표준과 완전히 다르다
    ///
    /// | | WZ 표준 | PKG2 V2 |
    /// |---|---|---|
    /// | 길이 | i8 (1바이트) | **i16 (2바이트)** |
    /// | 복호화 | 문자 마스크 `0xAAAA+i` | **V6 키스트림 XOR** |
    ///
    /// # 함정: 첫 1바이트가 우연히 맞아 보인다
    ///
    /// `int16` `-9` 의 하위 바이트 `0xF7` 을 i8 로 읽으면 `-9` → 길이 9 로
    /// **그럴듯하게** 나온다. 그래서 오류가 이름 문자열로만 드러나고, 개수나
    /// 오프셋은 정상이라 원인을 찾기 어렵다.
    ///
    /// 길이 필드 위치가 키스트림 기준 위치다 — C# 은 길이를 **읽기 전에**
    /// `ApplyState(reader.BaseStream.Position)` 을 호출하므로, 키를 만들 때는
    /// 길이 필드 위치(데이터 시작 아님)를 쓴다.
    fn read_pkg2_v2_name(&self, c: &mut ReadCtx) -> Result<Arc<str>, DirTreeError> {
        // 키 계산 기준 위치 — 데이터 시작이 아니라 **길이 필드** 위치.
        //
        // C# 은 `PartialStream` 위에서 읽으므로 이 값은 `dir_start` 기준 상대값이다.
        // 실측 (`_Canvas_041.wz`): 이름 데이터는 절대 375, `dirStart` 353,
        // 길이 필드는 상대 20 → `pkg2_dir_string_keys_v6(h1, hv, 20)`.
        //
        // 키스트림은 위치마다 달라지므로 호출 시점에 만들어야 한다. 컨텍스트의
        // `pkg2_name_key` 는 **첫 이름** 기준 키이므로 여러 이름을 읽으면 틀린다.
        //
        // # 함정: 위치를 상대/절대 중 무엇으로 줘야 하는가
        //
        // `Wz_Crypto.Pkg2DirStringKeyV5.ApplyState` 는 `reader.BaseStream.Position`
        // 을 받는데 그 reader 는 `PartialStream` 위에 있다. 즉 **상대**다.
        let rel_pos = c.pos().saturating_sub(self.ctx.dir_start);
        let mat = self
            .ctx
            .pkg2_name_key_material
            .ok_or(DirTreeError::NoPkg2NameKey)?;
        let keys = crate::offset::pkg2_dir_string_keys_v6(mat.hash1, mat.hash_version, rel_pos);

        let raw_len = c.read_i16_le().map_err(DirTreeError::io)?;
        if raw_len == 0 {
            return Ok(Arc::from(""));
        }
        if raw_len > 0 {
            return Err(DirTreeError::StringTooLong {
                len: i64::from(raw_len),
            });
        }
        let count =
            usize::try_from(-i32::from(raw_len)).map_err(|_| DirTreeError::StringTooLong {
                len: i64::from(raw_len),
            })?;
        // 이름은 최대 0x10000 코드유닛. 더 크면 파싱이 아니라 손상이다.
        if count > 0x10000 {
            return Err(DirTreeError::StringTooLong { len: count as i64 });
        }

        let mut buf = vec![0u8; count * 2];
        let bytes = c.read_exact_slice(buf.len()).map_err(DirTreeError::io)?;
        buf.copy_from_slice(&bytes);

        Ok(Arc::from(
            crate::offset::decode_pkg2_dir_string_v2(&keys, &mut buf)
                .as_deref()
                .unwrap_or(""),
        ))
    }

    fn read_name_body(&self, c: &mut ReadCtx, _at: u64) -> Result<Arc<str>, DirTreeError> {
        let n = c.read_i8().map_err(DirTreeError::io)?;
        let count: usize = if n == 0 {
            0
        } else if n > 0 {
            if n == 127 {
                let v = c.read_i32_le().map_err(DirTreeError::io)?;
                usize::try_from(v).map_err(|_| DirTreeError::StringTooLong { len: v as i64 })?
            } else {
                n as usize
            }
        } else if n == -128 {
            let v = c.read_i32_le().map_err(DirTreeError::io)?;
            usize::try_from(v).map_err(|_| DirTreeError::StringTooLong { len: v as i64 })?
        } else {
            (-(n as i32)) as usize
        };

        if count == 0 {
            return Ok(Arc::from(""));
        }

        if n > 0 {
            // UTF-16LE
            //
            // # 함정: 마스크가 16비트 단위다
            //
            // 원본 `MathHelper.ApplyWzStringCharMask` 는
            // `chars[i] ^= 0xAAAA + i` 다 — **유니코드 유닛** 기준.
            // ASCII 경로처럼 바이트마다 `0xAA + i` 하면 한 글의 상위 바이트가
            // 잘못 풀린다 (관측: `Character/00002000.img` → 깨진 문자열).
            let mut buf = c
                .read_exact_slice(count * 2)
                .map_err(|e| DirTreeError::Io(e.to_string()))?;
            for (i, chunk) in buf.as_chunks_mut::<2>().0.iter_mut().enumerate() {
                let u = u16::from_le_bytes(*chunk);
                let masked = u ^ (0xAAAAu16.wrapping_add(i as u16));
                chunk.copy_from_slice(&masked.to_le_bytes());
            }
            let units: Vec<u16> = buf
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| u16::from_le_bytes(*p))
                .collect();
            String::from_utf16(&units)
                .map(Arc::from)
                .map_err(|_| DirTreeError::InvalidUtf16)
        } else {
            // ASCII (cp1252 근사)
            //
            // 함정: 여기는 **바이트** 단위 `0xAA + i` 다 (원본
            // `MathHelper.DecodeWzStringAscii`). UTF-16 과 규칙이 다르다.
            let buf = c
                .read_exact_slice(count)
                .map_err(|e| DirTreeError::Io(e.to_string()))?;
            let mut s = String::with_capacity(count);
            for (i, &b) in buf.iter().enumerate() {
                s.push(char::from(b ^ (0xAAu8).wrapping_add(i as u8)));
            }
            Ok(Arc::from(s.as_str()))
        }
    }
}

/// 디렉터리 트리 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirTreeError {
    /// 엔트리 타입 바이트가 `0x02`/`0x03`/`0x04` 가 아니다.
    UnknownNodeType { node_type: u8 },
    /// 엔트리 개수가 음수다. 대개 키가 틀렸다는 뜻이다.
    NegativeCount { count: i32 },
    /// 엔트리 개수가 너무 커서 `usize` 로 변환할 수 없다.
    CountTooLarge { count: i32 },
    /// 재귀 깊이 제한 초과 (자기 참조 디렉터리).
    DepthExceeded { depth: usize },
    /// 문자열 길이가 음수 또는 범위 밖.
    StringTooLong { len: i64 },
    /// PKG2 V2 이름 키스트림이 없다.
    ///
    /// `DirTreeContext.pkg2_name_key` 가 `None` 인데 PKG2 V2 이름 경로를
    /// 타는 경우다. 감지 단계에서 키스트림을 만들지 못한 상황이며,
    /// 조용히 표준 경로로 fallback 하면 **잘못된 이름**이 나온다.
    NoPkg2NameKey,
    /// UTF-16 코드유닛이 잘못된 쌍을 이룬다.
    InvalidUtf16,
    /// 압축 정수 읽기 실패.
    CompressedInt(String),
    /// I/O 실패.
    Io(String),
}

impl DirTreeError {
    fn io(e: wz_error::WzError) -> Self {
        Self::Io(e.to_string())
    }
}

impl std::fmt::Display for DirTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownNodeType { node_type } => {
                write!(f, "알 수 없는 디렉터리 타입 0x{node_type:02X}")
            }
            Self::NegativeCount { count } => write!(f, "엔트리 개수 음수: {count}"),
            Self::CountTooLarge { count } => write!(f, "엔트리 개수 과다: {count}"),
            Self::DepthExceeded { depth } => write!(f, "디렉터리 깊이 초과: {depth}"),
            Self::StringTooLong { len } => write!(f, "문자열 길이 범위 밖: {len}"),
            Self::NoPkg2NameKey => write!(
                f,
                "PKG2 V2 이름 키스트림이 없다 (감지 단계에서 키를 만들지 못함)"
            ),
            Self::InvalidUtf16 => write!(f, "UTF-16 코드유닛 오류"),
            Self::CompressedInt(e) => write!(f, "압축 정수 오류: {e}"),
            Self::Io(e) => write!(f, "I/O 오류: {e}"),
        }
    }
}

impl std::error::Error for DirTreeError {}

impl From<wz_error::WzError> for DirTreeError {
    fn from(e: wz_error::WzError) -> Self {
        Self::Io(e.to_string())
    }
}
