//! WZ 이미지 노드 파서 (원본 `WzComparerR2.WzLib/Wz_Image.cs`).
//!
//! # 설계: 파서가 하는 일과 하지 않는 일
//!
//! **한다**: 바이트 → 노드 트리 구조.
//! **하지 않는다**: 픽셀 디코딩, 사운드 디코딩, UOL 해석.
//!
//! `Png`/`RawData`/`Sound`/`Video` 는 **디스크립터만** 만든다
//! (오프셋 + 길이 + 메타). 실제 픽셀은 필요할 때 풀어 캐시에 넣는다
//! (리포트 ADR-1 — C# 의 이미지 단위 LRU(32) 가 못 한 분리).
//!
//! UOL 도 **미해석 상태**로 저장한다. 해석은 `wz-db` 의 `resolve()` 책임이다
//! (C# 은 `Wz_Node.cs:549-558` 에 사이클 감지가 없어 무한 루프가 가능하다).
//!
//! # 반드시 재현할 것
//!
//! | 항목 | 원본 | 위험 |
//! |---|---|---|
//! | 값 플래그 9종 | `Wz_Image.cs:489-539` | 새 플래그 추가 가능 |
//! | `0x04` float 는 **압축 int** 를 비트 패턴으로 | `WzBinaryReader.cs:81-86` | f32 로 읽으면 깨짐 |
//! | `0x09` 확장 객체는 선언 길이를 **정확히** 소비 | `Wz_Image.cs:519-527` | 안 하면 이후 노드가 어긋남 |
//! | `Canvas.dataLen` 은 비압축 i32, 플래그 포함 | `Wz_Image.cs:300` | 압축으로 읽으면 파이 |
//! | `Property` 의 `i32 n` 은 값 개수 | `Wz_Image.cs:270` | 바이트 수로 읽으면 실패 |
//! | 문자열 플래그 3종 (`0x00`/`0x01`/`0x04`) | `WzBinaryReader.cs:404-419` | `0x04` 는 8바이트 스킵 후 null |
//! | **이미지 복호화 키 선별** | `Wz_Image.cs:434-483` | 틀린 키면 첫 문자열이 깨져 전부 실패 |
//! | 객체 타입 플래그 2종 (`0x73`/`0x1B`) | `WzBinaryReader.cs:390-402` | `0x1B` 는 오프셋 역참조 |

use std::sync::Arc;

use wz_error::{Result, WzFormatError};
use wz_pool::{ReadCtx, ReadCtxExt};

use crate::crypto::Decryptor;
use crate::read::{
    apply_ascii_mask, apply_utf16_mask, read_compressed_i32, read_compressed_i64,
    read_compressed_u32,
};

/// 값 플래그 바이트 (원본 `Wz_Image.ExtractValue`).
pub mod flags {
    pub const NULL: u8 = 0x00;
    pub const SHORT: u8 = 0x02;
    pub const INT: u8 = 0x03;
    /// **압축 int** 를 읽고 IEEE-754 비트 패턴으로 해석한다.
    pub const FLOAT: u8 = 0x04;
    pub const DOUBLE: u8 = 0x05;
    pub const STRING: u8 = 0x08;
    /// `i32 objDataLen` 후 확장 객체
    pub const EXTENDED: u8 = 0x09;
    pub const BOOL: u8 = 0x0B;
    pub const UINT: u8 = 0x13;
    pub const LONG: u8 = 0x14;
}

/// 객체 타입 태그 (원본 `Wz_Image.ExtractImg`).
pub mod tags {
    pub const PROPERTY: &str = "Property";
    pub const VECTOR_2D: &str = "Shape2D#Vector2D";
    pub const CANVAS: &str = "Canvas";
    pub const CONVEX_2D: &str = "Shape2D#Convex2D";
    pub const UOL: &str = "UOL";
    pub const RAW_DATA: &str = "RawData";
    pub const SOUND_DX8: &str = "Sound_DX8";
    pub const VIDEO: &str = "Canvas#Video";
}

/// `Wz_Png` 디스크립터. 픽셀은 **없다**.
#[derive(Debug, Clone, PartialEq)]
pub struct PngRef {
    pub width: i32,
    pub height: i32,
    pub format: i32,
    pub scale: u8,
    pub pages: i32,
    pub unknown1: i32,
    /// 페이로드 길이. **플래그 바이트를 포함한 전체** (`Wz_Png.DataLength`).
    pub data_len: u32,
    /// 페이로드 시작 오프셋 (이미지 기준 상대 오프셋).
    pub offset: u64,
    /// `Canvas` 바로 앞에 붙어 있던 mini property (`0x01` 이었을 때).
    pub sub_property: Option<Arc<Node>>,
}

impl PngRef {
    /// 바이트 소스로부터 Png 페이로드의 원시 바이트 블록을 추출한다.
    ///
    /// 페이로드의 첫 바이트는 플래그이므로 1바이트를 건너뛴 실제 데이터 영역을 반환한다.
    pub fn extract_raw_bytes(&self, src: &Arc<dyn wz_pool::ByteSource>) -> wz_error::Result<Vec<u8>> {
        if self.data_len <= 1 {
            return Ok(Vec::new());
        }
        let payload_offset = self.offset.checked_add(1).ok_or_else(|| {
            wz_error::WzFormatError::Malformed {
                what: "canvas offset overflow",
                offset: self.offset,
            }
        })?;
        let payload_len = (self.data_len - 1) as usize;
        let mut buf = vec![0u8; payload_len];
        src.read_at(payload_offset, &mut buf)?;
        Ok(buf)
    }
}

/// `Wz_RawData` 디스크립터.
#[derive(Debug, Clone, PartialEq)]
pub struct RawDataRef {
    pub offset: u64,
    pub len: u32,
    pub sub_property: Option<Arc<Node>>,
}

/// `Wz_Video` (MCV0) 디스크립터.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoRef {
    pub offset: u64,
    pub len: u32,
    pub unknown: u8,
    pub sub_property: Option<Arc<Node>>,
}

/// `Wz_Sound` 디스크립터.
///
/// 원본은 `AM_MEDIA_TYPE` + (키 탐색으로 얻는) `WAVEFORMATEX` 를 풀어야 하지만,
/// 그건 **별도 관심사**다. 여기서는 `fmt_ex` 원시 바이트만 보관하고
/// 해석은 `wz-audio` 에 맡긴다 (C# 의 `TryDecryptWaveFormatEx` 는
/// BMS→KMS→GMS 순서로 시도하는 휴리스틱이라 실패할 수 있다).
#[derive(Debug, Clone, PartialEq)]
pub struct SoundRef {
    pub offset: u64,
    pub data_len: u32,
    pub duration: i32,
    /// `soundDecl`: 2 면 `fmt_ex` 가 존재
    pub sound_decl: u8,
    /// AM_MEDIA_TYPE GUID (16+16+1+1+16 바이트 구조)
    pub major_type: [u8; 16],
    pub sub_type: [u8; 16],
    pub fixed_size_samples: bool,
    pub temporal_compression: bool,
    pub format_type: [u8; 16],
    /// WAVEFORMATEX 원시 바이트 (아직 복호화 안 됨)
    pub fmt_ex: Vec<u8>,
}

/// 2D 벡터 (원본 `Wz_Vector`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Vec2 {
    pub x: i32,
    pub y: i32,
}

/// 이미지 노드 값.
///
/// # 왜 Box/Arc 이 아닌가
///
/// 스칼라는 인라인 변형에 담는다. C# 의 `object Value` 박싱이 사라진다
/// (리포트 ADR-2). 큰 것만 `Arc` 로 공유한다.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageValue {
    Null,
    Short(i16),
    Int(i32),
    Long(i64),
    Uint(u32),
    Float(f32),
    Double(f64),
    Bool(bool),
    Str(Arc<str>),
    Vector(Vec2),
    /// 미해석 UOL 경로. 해석은 `wz-db::resolve`.
    Uol(Arc<str>),
    Png(PngRef),
    RawData(RawDataRef),
    Video(VideoRef),
    Sound(Arc<SoundRef>),
    Convex(Arc<[Vec2]>),
    /// 중첩 `Property`.
    Property(Arc<Node>),
}

impl ImageValue {
    /// 사람이 읽을 타입 이름 (오라클 JSON 스키마용).
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Short(_) => "Short",
            Self::Int(_) => "Int",
            Self::Long(_) => "Long",
            Self::Uint(_) => "Uint",
            Self::Float(_) => "Float",
            Self::Double(_) => "Double",
            Self::Bool(_) => "Bool",
            Self::Str(_) => "String",
            Self::Vector(_) => "Vector",
            Self::Uol(_) => "UOL",
            Self::Png(_) => "Canvas",
            Self::RawData(_) => "RawData",
            Self::Video(_) => "Video",
            Self::Sound(_) => "Sound",
            Self::Convex(_) => "Convex",
            Self::Property(_) => "Property",
        }
    }

    /// 디버그/로그용 짧은 표현.
    pub fn brief(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Short(v) => format!("{v}"),
            Self::Int(v) => format!("{v}"),
            Self::Long(v) => format!("{v}"),
            Self::Uint(v) => format!("{v}"),
            Self::Float(v) => format!("{v}"),
            Self::Double(v) => format!("{v}"),
            Self::Bool(v) => format!("{v}"),
            Self::Str(v) => format!("{v:?}"),
            Self::Vector(v) => format!("({}, {})", v.x, v.y),
            Self::Uol(v) => format!("{v:?}"),
            Self::Png(p) => format!("{}x{} fmt={}", p.width, p.height, p.format),
            Self::RawData(r) => format!("{} bytes @{}", r.len, r.offset),
            Self::Video(v) => format!("{} bytes @{}", v.len, v.offset),
            Self::Sound(s) => format!("{} bytes @{}", s.data_len, s.offset),
            Self::Convex(p) => format!("{} points", p.len()),
            Self::Property(p) => format!("{} children", p.children.len()),
        }
    }
}

/// WZ 노드 (이미지 루트 포함).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    pub name: Arc<str>,
    pub value: Option<ImageValue>,
    /// 순서 보존. 중복 이름이 **존재할 수 있다** (리포트 §3.2 S-9 —
    /// C# 은 인덱스로 마지막 것만, 선형 폴백으로 역방향을 유지해 우연히 일치시킨다).
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self {
            name: name.into(),
            value: None,
            children: Vec::new(),
        }
    }

    /// 이름으로 자식을 찾는다. **중복 시 첫 번째.**
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| &*c.name == name)
    }

    /// 이름 경로로 내려간다 (`a/b/c`).
    pub fn find(&self, path: &str) -> Option<&Node> {
        let mut cur = self;
        for part in path.split('/').filter(|s| !s.is_empty()) {
            cur = cur.child(part)?;
        }
        Some(cur)
    }

    /// 전체 노드 수 (자기 포함).
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(|c| c.count()).sum::<usize>()
    }
}

/// 이미지 파서.
///
/// # 상태
///
/// `string_ref_offset` — `0x1B`/`0x01` 플래그의 역참조 기준 오프셋.
/// 원본 `WzBinaryReader.StringReferenceOffsetBytes` (`WzBinaryReader.cs:25`).
/// 주어진 키로 첫 문자열을 풀었을 때 알려진 태그가 나오는지 확인한다.
///
/// 원본 `Wz_Image.IsIllegalTag` (`Wz_Image.cs:465`). 이름이 "Illegal" 인
/// 역설적인데, 반환값 `true` 가 **합법** 키라는 뜻이다.
///
/// # 함정: 판별에 성공하면 커서를 원위치로 되돌린다
///
/// 호출자가 같은 `ReadCtx` 를 이어서 쓰므로 판별용으로 소비한 바이트를
/// 그대로 두면 파싱이 어긋난다.
fn is_legal_tag(
    c: &mut ReadCtx,
    image_offset: u64,
    key: Option<&dyn Decryptor>,
    known: &[&str],
) -> bool {
    let save = c.pos();
    c.seek(image_offset).ok();

    // 판별에는 태그 문자열 하나면 충분하다.
    //
    // 함정: `ReadCtx::read` 는 **버퍼를 전부 채워야** 성공하고, 실패하면
    // 커서도 안 옮긴다. 그래서 64바이트를 한 번에 읽으면 이미지 끝에 닿는
    // 작은 파일에서 `used == 0` 이 되어 **모든** 키가 탈락한다.
    // 바이트 단위로 읽어 "읽은 만큼" 을 쓴다.
    const PROBE: usize = 64;
    let mut buf = [0u8; PROBE];
    let mut used = 0usize;
    while used < PROBE {
        match c.read_u8() {
            Ok(b) => {
                buf[used] = b;
                used += 1;
            }
            Err(_) => break,
        }
    }
    c.seek(save).ok();
    if used == 0 {
        return false;
    }

    // 길이는 판별에 필요 없다 — 태그 문자열 자체만 본다.
    let mut probe = Probe {
        decrypter: key,
        pos: 0,
        data: &buf[..used],
    };
    let Some(tag) = probe.read_tag_string() else {
        return false;
    };
    known.contains(&tag.as_str())
}

/// 태그 문자열 하나를 읽는 최소 파서.
///
/// 전체 `ImageParser` 를 쓰면 재귀적으로 상태를 만들어 버리고, 판별 목적에는
/// 과하다. 플래그 2종(`0x73` 인라인, `0x1B` 참조)만 처리하면 충분하다.
struct Probe<'a> {
    decrypter: Option<&'a dyn Decryptor>,
    pos: usize,
    data: &'a [u8],
}

impl Probe<'_> {
    /// `0x73` (인라인 문자열) 또는 `0x1B` (참조) 를 읽는다.
    ///
    /// # 함정: 참조(`0x1B`) 는 판별용 버퍼 밖을 가리킨다
    ///
    /// 판별은 이미지 앞 64바이트만 봤으므로 참조 대상이 범위 밖이다.
    /// 이 경우 `None` 을 돌려 그 키를 탈락시킨다 — 실제로는
    /// `Wz_Image` 루트 태그는 항상 `0x73` 인라인이다.
    fn read_tag_string(&mut self) -> Option<String> {
        let flag = *self.data.get(self.pos)?;
        self.pos += 1;
        match flag {
            0x73 => self.read_str(),
            _ => None,
        }
    }

    /// `i8` 길이 접두사 + 마스킹된 바이트를 읽는다 (원본 `ReadString`).
    ///
    /// # 가장 큰 함정: **부호가 문자종류를 정한다**
    ///
    /// | 길이 바이트 | 경로 | 바이트 수 | 마스크 |
    /// |---|---|---|---|
    /// | `n > 0` | UTF-16LE | `n * 2` | `0xAAAA + 유닛 index` |
    /// | `n == 0` | 빈 문자열 | 0 | — |
    /// | `n == -128` | ASCII, 뒤의 `i32` 가 길이 | `len` | `0xAA + 바이트 index` |
    /// | `n < 0` | ASCII(cp1252) | `-n` | `0xAA + 바이트 index` |
    ///
    /// 음수를 UTF-16 으로 읽으면 (×2) 태그 판별이 100% 실패한다.
    /// `Item_000.wz @580` 의 루트 태그는 `len = -8` 인 **ASCII** 경로다.
    fn read_str(&mut self) -> Option<String> {
        let n = *self.data.get(self.pos)? as i8;
        self.pos += 1;

        if n == 0 {
            return Some(String::new());
        }

        // 음수 = ASCII 경로. -128 은 뒤의 i32 를 길이로 쓴다.
        if n < 0 {
            let count = if n == -128 {
                let v = self.i32_at(self.pos)?;
                self.pos += 4;
                if v < 0 {
                    return None;
                }
                v as usize
            } else {
                (-(n as i32)) as usize
            };
            let end = self.pos.checked_add(count)?;
            let raw = self.data.get(self.pos..end)?;
            let mut bytes = raw.to_vec();
            self.decrypt(&mut bytes);
            crate::read::string::apply_ascii_mask(&mut bytes);
            return Some(bytes.iter().map(|&b| b as char).collect());
        }

        // 양수 = UTF-16LE 경로.
        let count = if n == 127 {
            let v = self.i32_at(self.pos)?;
            self.pos += 4;
            if v < 0 {
                return None;
            }
            v as usize
        } else {
            n as usize
        };
        let need = count.checked_mul(2)?;
        let end = self.pos.checked_add(need)?;
        let raw = self.data.get(self.pos..end)?;
        let mut bytes = raw.to_vec();
        self.decrypt(&mut bytes);
        crate::read::string::apply_utf16_mask(&mut bytes);

        let mut out = String::with_capacity(count);
        for c in bytes.as_chunks::<2>().0.iter() {
            let u = u16::from_le_bytes(*c);
            out.push(char::from_u32(u32::from(u)).unwrap_or('\u{FFFD}'));
        }
        Some(out)
    }

    /// 키 오프셋은 **항상 0** 이다 (원본 `Decrypt(data)` → `Decrypt(data, 0)`).
    fn decrypt(&self, bytes: &mut [u8]) {
        if let Some(d) = self.decrypter {
            d.decrypt_at(bytes, 0);
        }
    }

    ///little-endian `i32` 를 읽는다 (길이 확장용).
    fn i32_at(&self, at: usize) -> Option<i32> {
        let b = self.data.get(at..at.checked_add(4)?)?;
        Some(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

#[derive(Debug)]
pub struct ImageParser {
    // 함정: `&'a dyn Decryptor` 로 두면 `detect_key` 가 고른
    // `Box<dyn Decryptor>` 를 담을 수 없다 (자기참조 구조체가 된다).
    // 그래서 소유하는 `Box` 로 둔다.
    decrypter: Option<Box<dyn Decryptor>>,
    /// `0x01`/`0x1B` 역참조 기준 (원본 `StringReferenceOffsetBytes`).
    string_ref_offset: u64,
    /// 문자열 풀 (역참조 재사용). 리포트 §7 — 절대 오프셋 기준.
    pool: std::collections::HashMap<u64, Arc<str>>,
    /// 파싱된 총 바이트 (검증/진단용).
    bytes_consumed: u64,
}

impl ImageParser {
    pub fn new(decrypter: Option<Box<dyn Decryptor>>) -> Self {
        Self {
            decrypter,
            string_ref_offset: 0,
            pool: std::collections::HashMap::new(),
            bytes_consumed: 0,
        }
    }

    pub fn with_string_ref_offset(mut self, off: u64) -> Self {
        self.string_ref_offset = off;
        self
    }
    /// 이미지의 실제 복호화 키를 **휴리스틱으로** 골라 설정한다.
    ///
    /// # 함정: 판별은 반드시 이미지 **시작 오프셋** 에서 한다
    ///
    /// `PartialStream` 처럼 이미지만 잘라 붙인 스트림이면 오프셋 0 이 되므로,
    /// 호출자가 원본 파일 오프셋을 넘겨야 한다.
    ///
    /// 원본 `Wz_Image.TryDetectEnc` (`Wz_Image.cs:434`) 이스식이다.
    /// 헤더에 키 정보가 없어 **첫 문자열로 판별**한다.
    ///
    /// # 함정: 키를 안 고르면 전부 깨진다
    ///
    /// `Item_000.wz` 는 첫 문자열이 `73 F8 FA D9 ...` 로 암호화돼 있다.
    /// 키를BMS/KMS/GMS 중 어느 것으로도 풀지 않으면 문자열이 깨지고
    /// `"Property"` 태그 비교가 실패해 **이미지 전체가** 파싱되지 않는다.
    ///
    /// # 함정: 선별은 파일당 한 번, 이미지 시작 오프셋에서
    ///
    /// 판별용으로 이미지 데이터를 16바이트만 읽는다. 전체를 복호화하면 안 된다.
    ///
    /// # 판별 순서
    ///
    /// `Pkg1EncType`(구조체에 설정된 값)이 먼저이고, 그다음
    /// **BMS → KMS → GMS** 순이다 (`Wz_Image.cs:450`).BMS 는 no-op 이라
    /// 대부분 여기서 걸린다.
    ///
    /// # 알려진 태그
    ///
    /// `IsIllegalTag` (`Wz_Image.cs:465`) 가 받아들이는 문자열. 이 목록에
    /// 없으면 그 키는 오답이다.
    pub fn detect_key(
        mut self,
        c: &mut ReadCtx,
        image_offset: u64,
        declared: Option<crate::crypto::aes::KeyKind>,
    ) -> Result<Self> {
        const KNOWN: [&str; 7] = [
            "Property",
            "Shape2D#Vector2D",
            "Canvas",
            "Shape2D#Convex2D",
            "Sound_DX8",
            "UOL",
            "RawData",
        ];

        // 구조체에 선언된 키가 우선한다 (C# `TryDetectEnc` 선두 분기).
        if let Some(kind) = declared {
            let ks = crate::crypto::aes::key_stream(kind);
            if is_legal_tag(c, image_offset, Some(ks.as_ref()), &KNOWN) {
                self.decrypter = Some(ks);
                return Ok(self);
            }
        }

        // BMS → KMS → GMS 순서로 시도한다 (C# 과 동일한 순서).
        for kind in [
            crate::crypto::aes::KeyKind::Bms,
            crate::crypto::aes::KeyKind::Kms,
            crate::crypto::aes::KeyKind::Gms,
        ] {
            let ks = crate::crypto::aes::key_stream(kind);
            if is_legal_tag(c, image_offset, Some(ks.as_ref()), &KNOWN) {
                self.decrypter = Some(ks);
                return Ok(self);
            }
        }

        Err(WzFormatError::UnsupportedProfile {
            profile: "image key detection".into(),
            path_hint: format!(
                "BMS/KMS/GMS 중 어느 것으로도 첫 문자열이 알려진 태그가 되지 않음 (오프셋 {})",
                image_offset
            ),
        }
        .into())
    }

    /// 이미지 루트를 파싱한다 (전체 이미지 = 루트 노드 1개).
    ///
    /// 원본 `Wz_Image.TryExtract` 이 `ExtractImg` → 루트 이름 "" 로 만든다.
    pub fn parse_image_root(&mut self, c: &mut ReadCtx) -> Result<Node> {
        let start = c.pos();
        let children = self.parse_object_children(c)?;
        self.bytes_consumed = c.pos().saturating_sub(start);
        Ok(Node {
            name: Arc::from(""),
            value: None,
            children,
        })
    }

    pub fn bytes_consumed(&self) -> u64 {
        self.bytes_consumed
    }

    // ── 문자열 ─────────────────────────────────────────────────────────────

    /// `ReadImageString` — 플래그 3종.
    ///
    /// | 플래그 | 의미 |
    /// |---|---|
    /// | `0x00` | 바로 뒤에 일반 문자열 |
    /// | `0x01` | `i32` 오프셋 + 그 위치의 문자열 (역참조) |
    /// | `0x04` | 8바이트 스킵 후 `null` |
    fn read_image_string(&mut self, c: &mut ReadCtx) -> Result<Option<Arc<str>>> {
        let flag_pos = c.pos();
        let flag = c.read_u8()?;
        match flag {
            0x00 => Ok(Some(Arc::from(self.read_plain_string(c)?.as_str()))),
            0x01 => {
                let rel = c.read_i32_le()?;
                let at = (rel as i64 + self.string_ref_offset as i64).max(0) as u64;
                Ok(Some(self.read_string_at(c, at)?))
            }
            0x04 => {
                // 원본 `WzBinaryReader.cs:413-415` — 8바이트 스킵 후 null
                c.skip_bytes(8)?;
                Ok(None)
            }
            _ => Err(WzFormatError::Malformed {
                what: "image string flag",
                offset: flag_pos,
            }
            .into()),
        }
    }

    /// 절대 오프셋의 문자열을 읽는다 (역참조).
    fn read_string_at(&mut self, c: &mut ReadCtx, at: u64) -> Result<Arc<str>> {
        if let Some(hit) = self.pool.get(&at) {
            return Ok(Arc::clone(hit));
        }
        let saved = c.pos();
        c.seek(at)?;
        let s = self.read_plain_string(c)?;
        c.seek(saved)?;
        let s: Arc<str> = Arc::from(s.as_str());
        self.pool.insert(at, Arc::clone(&s));
        Ok(s)
    }

    /// 3종 인코딩 문자열 (길이 접두사 + XOR 마스킹 + 선택적 복호화).
    fn read_plain_string(&mut self, c: &mut ReadCtx) -> Result<String> {
        let start = c.pos();
        let len = c.read_i8()?;

        // 함정: 경로(ASCII/UTF-16)를 **기억해야 한다**. 바이트 길이만 보고
        // UTF-16 으로 추측하면 짝수 바이트 ASCII 문자열을 전부 깨뜨린다
        // (`"Property"` = ASCII 8바이트 → 4개 유닛으로 오해).
        // `decode_text` 의 길이 기반 추측은 이 때문에 위험하다.
        let (raw, utf16) = if len == 0 {
            (Vec::new(), false)
        } else if len > 0 {
            // UTF-16LE
            let n = if len == 127 {
                let v = c.read_i32_le()?;
                usize::try_from(v).map_err(|_| wz_error::WzCryptoError::StringDecode {
                    reason: format!("UTF-16 길이 {v}"),
                })?
            } else {
                len as usize
            };
            let mut buf = c.read_exact_slice(n * 2)?;
            if let Some(d) = self.decrypter.as_deref() {
                d.decrypt_at(&mut buf, 0);
            }
            apply_utf16_mask(&mut buf);
            (buf, true)
        } else {
            // 8비트 (cp1252)
            let n = if len == -128 {
                let v = c.read_i32_le()?;
                usize::try_from(v).map_err(|_| wz_error::WzCryptoError::StringDecode {
                    reason: format!("ASCII 길이 {v}"),
                })?
            } else {
                (-(len as i32)) as usize
            };
            let mut buf = c.read_exact_slice(n)?;
            if let Some(d) = self.decrypter.as_deref() {
                d.decrypt_at(&mut buf, 0);
            }
            apply_ascii_mask(&mut buf);
            (buf, false)
        };

        let text = if utf16 {
            decode_utf16(&raw)
        } else {
            // ASCII 는 바이트를 latin-1/cp1252 로 그대로 문자로 본다.
            // 짝수 길이여도 UTF-16 이 아니다 (위 함정).
            decode_ascii(&raw)
        };
        // 문자열 풀은 역참조 대상 오프셋에 대해서만 유효하므로
        // 시작 오프셋을 기억해 둔다.
        self.pool
            .entry(start)
            .or_insert_with(|| Arc::from(text.as_str()));
        Ok(text)
    }

    // ── 값 ────────────────────────────────────────────────────────────────

    /// `ExtractValue` — 이름 + 플래그 + 값.
    fn parse_value(&mut self, c: &mut ReadCtx) -> Result<Node> {
        let name_pos = c.pos();
        let name = self.read_image_string(c)?.ok_or(WzFormatError::Malformed {
            what: "property name",
            offset: name_pos,
        })?;

        let flag_pos = c.pos();
        let flag = c.read_u8()?;
        let value = match flag {
            flags::NULL => ImageValue::Null,
            flags::SHORT => ImageValue::Short(c.read_i16_le()?),
            flags::INT => ImageValue::Int(read_compressed_i32(c)?),
            flags::UINT => ImageValue::Uint(read_compressed_u32(c)?),
            flags::LONG => ImageValue::Long(read_compressed_i64(c)?),
            flags::BOOL => ImageValue::Bool(c.read_i16_le()? != 0),
            // 함정: 압축 int 를 읽고 IEEE-754 비트 패턴으로 해석한다.
            // 4바이트 raw 가 아니다 (`WzBinaryReader.cs:81-86`).
            flags::FLOAT => {
                let bits = read_compressed_i32(c)?;
                ImageValue::Float(f32::from_bits(bits as u32))
            }
            flags::DOUBLE => ImageValue::Double(c.read_f64_le()?),
            flags::STRING => {
                let s = self.read_image_string(c)?;
                match s {
                    Some(s) => ImageValue::Str(s),
                    None => ImageValue::Null,
                }
            }
            flags::EXTENDED => {
                // `i32 objDataLen` + 확장 객체, 길이 **정확히** 소비해야 한다
                let obj_len = c.read_i32_le()?;
                let eob = c.pos().saturating_add_signed(obj_len as i64);
                let v = self.parse_extended(c)?;
                if c.pos() != eob {
                    return Err(WzFormatError::ObjectLengthMismatch {
                        declared: obj_len as u32,
                        actual: c.pos(),
                        offset: flag_pos,
                    }
                    .into());
                }
                v
            }
            other => {
                let _ = other;
                return Err(WzFormatError::Malformed {
                    what: "value flag",
                    offset: flag_pos,
                }
                .into());
            }
        };

        Ok(Node {
            name,
            value: Some(value),
            children: Vec::new(),
        })
    }

    // ── 확장 객체 ─────────────────────────────────────────────────────────

    /// `ExtractImg` — 태그에 따라 객체를 파싱한다.
    ///
    /// **반환값의 의미**: 태그가 `Property` 면 `ImageValue::Property`,
    /// 그 외에는 "이 노드에 붙는 값" 이며 호출자가 지정한 노드에 넣는다.
    fn parse_extended(&mut self, c: &mut ReadCtx) -> Result<ImageValue> {
        let tag = self.read_object_type_name(c)?;

        Ok(match &*tag {
            tags::PROPERTY => {
                let node = self.parse_mini_property(c)?;
                ImageValue::Property(Arc::new(node))
            }
            tags::VECTOR_2D => {
                let x = read_compressed_i32(c)?;
                let y = read_compressed_i32(c)?;
                ImageValue::Vector(Vec2 { x, y })
            }
            tags::CANVAS => ImageValue::Png(self.parse_canvas(c)?),
            tags::CONVEX_2D => ImageValue::Convex(Arc::from(self.parse_convex(c)?)),
            tags::UOL => {
                c.skip_bytes(1)?; // 원본 `Wz_Image.cs:389`
                let path = self.read_image_string(c)?.ok_or(WzFormatError::Malformed {
                    what: "UOL path",
                    offset: c.pos(),
                })?;
                ImageValue::Uol(path)
            }
            tags::RAW_DATA => {
                let (offset, len, sub) = self.parse_raw_data(c)?;
                ImageValue::RawData(RawDataRef {
                    offset,
                    len,
                    sub_property: sub,
                })
            }
            tags::VIDEO => {
                let (offset, len, unknown, sub) = self.parse_video(c)?;
                ImageValue::Video(VideoRef {
                    offset,
                    len,
                    unknown,
                    sub_property: sub,
                })
            }
            tags::SOUND_DX8 => ImageValue::Sound(Arc::from(self.parse_sound(c)?)),
            other => {
                return Err(WzFormatError::UnsupportedProfile {
                    profile: other.to_string(),
                    path_hint: "이미지 객체 태그".into(),
                }
                .into());
            }
        })
    }

    /// `ReadImageObjectTypeName` — 플래그 2종.
    fn read_object_type_name(&mut self, c: &mut ReadCtx) -> Result<Arc<str>> {
        let flag_pos = c.pos();
        let flag = c.read_u8()?;
        match flag {
            0x73 => Ok(Arc::from(self.read_plain_string(c)?.as_str())),
            0x1B => {
                let rel = c.read_i32_le()?;
                let at = (rel as i64 + self.string_ref_offset as i64).max(0) as u64;
                Ok(self.read_string_at(c, at)?)
            }
            _ => Err(WzFormatError::Malformed {
                what: "object type flag",
                offset: flag_pos,
            }
            .into()),
        }
    }

    /// `Property` 본문 — `skip 2; i32 entries; entries × value`.
    ///
    /// # 함정
    ///
    /// `i32 entries` 는 **값 개수**다. 바이트 수가 아니다.
    fn parse_mini_property(&mut self, c: &mut ReadCtx) -> Result<Node> {
        c.skip_bytes(2)?; // 원본 `Wz_Image.cs:269`
        let entries = read_compressed_i32(c)?;
        let entries = usize::try_from(entries).map_err(|_| WzFormatError::Malformed {
            what: "property entry count",
            offset: c.pos(),
        })?;
        let mut children = Vec::with_capacity(entries.min(4096));
        for _ in 0..entries {
            children.push(self.parse_value(c)?);
        }
        Ok(Node {
            name: Arc::from(""),
            value: None,
            children,
        })
    }

    /// `Canvas` 본문 (`Wz_Image.cs:281-303`).
    fn parse_canvas(&mut self, c: &mut ReadCtx) -> Result<PngRef> {
        c.skip_bytes(1)?; // 원본 `:282`
        let sub = if c.read_u8()? == 0x01 {
            Some(Arc::from(self.parse_mini_property(c)?))
        } else {
            None
        };

        let width = read_compressed_i32(c)?;
        let height = read_compressed_i32(c)?;
        let format = read_compressed_i32(c)?;
        let scale = c.read_u8()?;
        let pages = read_compressed_i32(c)?; // KMST1186
        let unknown1 = read_compressed_i32(c)?; // KMST1198
        c.skip_bytes(2)?; // "TBD" (원본 `:299`)
                          // 함정: dataLen 은 **비압축** i32 이다
        let data_len = c.read_i32_le()?;
        // 페이로드 오프셋 (원본 `(uint)reader.BaseStream.Position`)
        let offset = c.pos();

        if data_len < 0 {
            return Err(WzFormatError::Malformed {
                what: "canvas dataLen",
                offset,
            }
            .into());
        }
        // 함정: 페이로드를 **건너뛰어야** 한다.
        // 원본 `Wz_Image.cs:302` 의 `reader.SkipBytes(dataLen)`.
        // 이게 없으면 `0x09` 의 길이 검증(`:523-527`)이 실패해 모든 Canvas
        // 프로퍼티가 깨진다. 픽셀은 나중에 읽으므로 스킵만 한다.
        c.skip_bytes(data_len as usize)?;
        Ok(PngRef {
            width,
            height,
            format,
            scale,
            pages,
            unknown1,
            data_len: data_len as u32,
            offset,
            sub_property: sub,
        })
    }

    /// `Shape2D#Convex2D` (`Wz_Image.cs:305-322`).
    ///
    /// 각 점이 `ExtractImg` 로 파싱되는데, 전부 `Vector2D` 여야 한다.
    fn parse_convex(&mut self, c: &mut ReadCtx) -> Result<Vec<Vec2>> {
        let entries = read_compressed_i32(c)?;
        let entries = usize::try_from(entries).map_err(|_| WzFormatError::Malformed {
            what: "convex point count",
            offset: c.pos(),
        })?;
        let mut points = Vec::with_capacity(entries.min(4096));
        for _ in 0..entries {
            match self.parse_extended(c)? {
                ImageValue::Vector(v) => points.push(v),
                other => {
                    // 원본 `Wz_Image.cs:317-318` — "Convex2D contains non vector2D items."
                    return Err(WzFormatError::UnsupportedProfile {
                        profile: format!("Convex2D/{other:?}"),
                        path_hint: "점은 Vector2D 여야 한다".into(),
                    }
                    .into());
                }
            }
        }
        Ok(points)
    }

    /// `RawData` (`Wz_Image.cs:393-410`).
    fn parse_raw_data(&mut self, c: &mut ReadCtx) -> Result<(u64, u32, Option<Arc<Node>>)> {
        let ver = c.read_u8()?;
        let sub = if ver == 1 {
            // KMST1177
            if c.read_u8()? == 0x01 {
                Some(Arc::from(self.parse_mini_property(c)?))
            } else {
                None
            }
        } else {
            None
        };
        let len = read_compressed_i32(c)?;
        let offset = c.pos();
        if len < 0 {
            return Err(WzFormatError::Malformed {
                what: "rawdata len",
                offset,
            }
            .into());
        }
        c.skip_bytes(len as usize)?;
        Ok((offset, len as u32, sub))
    }

    /// `Canvas#Video` (`Wz_Image.cs:412-427`).
    fn parse_video(&mut self, c: &mut ReadCtx) -> Result<(u64, u32, u8, Option<Arc<Node>>)> {
        c.skip_bytes(1)?;
        // KMST1188
        let sub = if c.read_u8()? == 0x01 {
            Some(Arc::from(self.parse_mini_property(c)?))
        } else {
            None
        };
        let unknown = c.read_u8()?;
        let len = read_compressed_i32(c)?;
        let offset = c.pos();
        if len < 0 {
            return Err(WzFormatError::Malformed {
                what: "video len",
                offset,
            }
            .into());
        }
        c.skip_bytes(len as usize)?;
        Ok((offset, len as u32, unknown, sub))
    }

    /// `Sound_DX8` (`Wz_Image.cs:324-386`).
    ///
    /// `WAVEFORMATEX` 는 **복호화하지 않고** 원시 바이트로 보관한다.
    /// 원본의 `TryDecryptWaveFormatEx` 는 BMS→KMS→GMS 순서로 키를 시도하는
    /// 휴리스틱이라 실패할 수 있고, 그건 파서가 아니라 오디오 계층의 책임이다.
    fn parse_sound(&mut self, c: &mut ReadCtx) -> Result<SoundRef> {
        let ver = c.read_u8()?;
        if ver == 1 {
            // KMST1184
            if c.read_u8()? == 0x01 {
                let _ = self.parse_mini_property(c)?;
            }
        }
        let data_len = read_compressed_i32(c)?;
        let duration = read_compressed_i32(c)?;
        let sound_decl = c.read_u8()?;

        let mut major_type = [0u8; 16];
        c.read(&mut major_type)?;
        let mut sub_type = [0u8; 16];
        c.read(&mut sub_type)?;
        let fixed_size_samples = c.read_u8()? != 0;
        let temporal_compression = c.read_u8()? != 0;
        let mut format_type = [0u8; 16];
        c.read(&mut format_type)?;

        let mut fmt_ex = Vec::new();
        if sound_decl == 2 {
            let n = read_compressed_i32(c)?;
            if n < 0 {
                return Err(WzFormatError::Malformed {
                    what: "waveformatex len",
                    offset: c.pos(),
                }
                .into());
            }
            fmt_ex = c.read_exact_slice(n as usize)?;
        }

        if data_len < 0 {
            return Err(WzFormatError::Malformed {
                what: "sound dataLen",
                offset: c.pos(),
            }
            .into());
        }
        let offset = c.pos();
        c.skip_bytes(data_len as usize)?;

        Ok(SoundRef {
            offset,
            data_len: data_len as u32,
            duration,
            sound_decl,
            major_type,
            sub_type,
            fixed_size_samples,
            temporal_compression,
            format_type,
            fmt_ex,
        })
    }

    /// `Property` 태그를 **노드 목록**으로 (이미지 루트용).
    fn parse_object_children(&mut self, c: &mut ReadCtx) -> Result<Vec<Node>> {
        let tag = self.read_object_type_name(c)?;
        match &*tag {
            tags::PROPERTY => Ok(self.parse_mini_property(c)?.children),
            other => Err(WzFormatError::UnsupportedProfile {
                profile: other.to_string(),
                path_hint: format!("이미지 루트는 Property 여야 한다 (시작 위치 {})", c.pos()),
            }
            .into()),
        }
    }
}

/// 8비트/16비트 바이트열 → `String`.
///
/// UTF-16LE 바이트 → 문자열.
fn decode_utf16(raw: &[u8]) -> String {
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    String::from_utf16_lossy(&units)
}

/// cp1252 바이트 → 문자열.
///
/// # 함정: 짝수 길이라도 UTF-16 이 아니다
///
/// 길이가 짝수라는 이유로 UTF-16 으로 추측하면 ASCII 문자열이 깨진다.
/// 경로 정보(길이 부호)는 **호출부에서** 알고 있으므로 여기서 추측하지 않는다.
fn decode_ascii(raw: &[u8]) -> String {
    raw.iter().map(|&b| b as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(bytes: Vec<u8>) -> ReadCtx {
        ReadCtx::from_bytes(bytes)
    }

    /// ASCII 경로 문자열(음수 길이)을 인코딩한다.
    ///
    /// `WzBinaryReader.ReadString` 의 음수 분기 — 바이트 수 그대로 읽고
    /// `0xAA + i` 마스크를 쓴다.
    fn enc_ascii(s: &str) -> Vec<u8> {
        let mut v = vec![(-(s.len() as i8)) as u8];
        for (i, b) in s.bytes().enumerate() {
            v.push(b ^ (0xAAu32.wrapping_add(i as u32) & 0xFF) as u8);
        }
        v
    }

    /// 회귀: 짝수 바이트 ASCII 문자열이 UTF-16 으로 오해되면 안 된다.
    ///
    /// `Item_000.wz` 의 루트 태그 `"Property"` 가 이 경우다 (ASCII 8바이트).
    /// 길이만 보고 인코딩을 추측하면 4개 유닛으로 깨져 루트 태그 비교가
    /// 실패하고 이미지 전체가 파싱되지 않는다.
    #[test]
    fn even_length_ascii_is_not_utf16() {
        let mut p = ImageParser::new(None);
        let mut c = ctx(enc_ascii("Property"));
        assert_eq!(p.read_plain_string(&mut c).unwrap(), "Property");
    }

    /// 회귀: 홀수 바이트 ASCII 도 정상 (양수 길이 UTF-16 과 구분된다).
    #[test]
    fn odd_length_ascii_string() {
        let mut p = ImageParser::new(None);
        let mut c = ctx(enc_ascii("abc"));
        assert_eq!(p.read_plain_string(&mut c).unwrap(), "abc");
    }

    /// 회귀: UTF-16 경로는 여전히 UTF-16 으로 읽어야 한다.
    #[test]
    fn positive_length_is_utf16() {
        let mut p = ImageParser::new(None);
        let mut c = ctx(enc_str("Property"));
        assert_eq!(p.read_plain_string(&mut c).unwrap(), "Property");
    }

    /// 회귀: 이미지 키 선별이 BMS 를 고른다.
    ///
    /// 루트 태그가BMS(항등) 로만 `Property` 가 되는 이미지에서,
    /// 키를 고르지 않으면 문자열이 깨져 파싱이 전부 실패한다.
    #[test]
    fn detect_key_picks_bms_for_plain_property() {
        let mut image = vec![0x73u8];
        image.extend_from_slice(&enc_ascii("Property"));
        let mut c = ctx(image);
        let parser = ImageParser::new(None)
            .detect_key(&mut c, 0, None)
            .expect("BMS 로 풀리므로 선별 성공");
        // 선별이 성공하면 루트 태그를 그대로 읽을 수 있다.
        let mut c = ctx({
            let mut v = vec![0x73u8];
            v.extend_from_slice(&enc_ascii("Property"));
            v
        });
        let mut parser = parser;
        assert_eq!(&*parser.read_object_type_name(&mut c).unwrap(), "Property");
    }

    /// 회정: 키 선별은 파일/스트림 위치를 원위치로 되돌린다.
    ///
    /// 판별용으로 소비한 바이트를 남기면 뒤 파싱이 어긋난다.
    #[test]
    fn detect_key_restores_cursor() {
        let mut image = vec![0xAA, 0xBB, 0xCC];
        image.push(0x73);
        image.extend_from_slice(&enc_ascii("Property"));
        let mut c = ctx(image);
        c.seek(3).unwrap();
        let _ = ImageParser::new(None).detect_key(&mut c, 3, None).unwrap();
        assert_eq!(c.pos(), 3, "선별 후 커서 원위치");
    }

    /// **프로퍼티 이름** 인코딩 — `read_image_string` 이 플래그 바이트를
    /// 먼저 읽으므로 `0x00` (인라인) 접두사가 필수다.
    fn name(s: &str) -> Vec<u8> {
        let mut v = vec![0x00u8];
        v.extend_from_slice(&enc_str(s));
        v
    }
    /// 문자열을 평문으로 인코딩 (테스트 헬퍼).
    fn enc_str(s: &str) -> Vec<u8> {
        let units: Vec<u16> = s.encode_utf16().collect();
        let mut out = vec![units.len() as u8];
        for (i, u) in units.iter().enumerate() {
            let mask = (0xAAAAu32.wrapping_add(i as u32) & 0xFFFF) as u16;
            out.extend_from_slice(&(u ^ mask).to_le_bytes());
        }
        out
    }

    /// `i32` 값 하나.
    fn value_int(nm: &str, v: i32) -> Vec<u8> {
        let mut b = name(nm);
        b.push(flags::INT);
        b.extend_from_slice(&crate::read::encode_compressed_i32(v));
        b
    }

    // ── 기본 값 ────────────────────────────────────────────────────────────

    /// `name(nm) + 플래그 + 값` 을 만드는 헬퍼.
    fn prop(nm: &str, f: u8, payload: &[u8]) -> Vec<u8> {
        let mut v = name(nm);
        v.push(f);
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn parses_all_scalar_flags() {
        // 원본 `Wz_Image.ExtractValue` 의 플래그 9종을 전부 한 번씩
        // (원본 `Wz_Image.cs:489-539`)
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes()); // skip 2
        b.extend_from_slice(&crate::read::encode_compressed_i32(10)); // 10개 값

        // 0x02 Short
        b.extend(prop("short", flags::SHORT, &(-300i16).to_le_bytes()));
        // 0x00 Null
        b.extend(prop("null", flags::NULL, &[]));
        // 0x03 Int
        b.extend(prop(
            "int",
            flags::INT,
            &crate::read::encode_compressed_i32(12345),
        ));
        // 0x04 Float — 압축 int 를 비트 패턴으로
        b.extend(prop(
            "float",
            flags::FLOAT,
            &crate::read::encode_compressed_i32(3.5f32.to_bits() as i32),
        ));
        // 0x05 Double
        b.extend(prop(
            "double",
            flags::DOUBLE,
            &std::f64::consts::PI.to_bits().to_le_bytes(),
        ));
        // 0x08 String
        let mut sv = name("string");
        sv.push(flags::STRING);
        let mut s0 = vec![0x00u8];
        s0.extend_from_slice(&enc_str("hello"));
        sv.extend_from_slice(&s0);
        b.extend(sv);
        // 0x0B Bool
        b.extend(prop("bool", flags::BOOL, &1i16.to_le_bytes()));
        // 0x13 Uint
        b.extend(prop(
            "uint",
            flags::UINT,
            &crate::read::encode_compressed_u32(4_000_000_000),
        ));
        // 0x14 Long
        b.extend(prop(
            "long",
            flags::LONG,
            &crate::read::encode_compressed_i64(i64::MIN),
        ));
        // 0x09 Extended (Property)
        {
            let mut inner = vec![0x73u8];
            inner.extend_from_slice(&enc_str("Property"));
            inner.extend_from_slice(&2u16.to_le_bytes());
            inner.extend_from_slice(&crate::read::encode_compressed_i32(1));
            inner.extend(value_int("leaf", 7));
            b.extend(prop("ext", flags::EXTENDED, &{
                let mut v = (inner.len() as i32).to_le_bytes().to_vec();
                v.extend_from_slice(&inner);
                v
            }));
        }

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        assert_eq!(c.pos(), c.len().unwrap(), "정확히 끝까지 소비");

        assert_eq!(root.children.len(), 10);
        let by = |n: &str| root.child(n).unwrap().value.clone().unwrap();
        assert_eq!(by("short"), ImageValue::Short(-300));
        assert_eq!(by("null"), ImageValue::Null);
        assert_eq!(by("int"), ImageValue::Int(12345));
        assert_eq!(by("float"), ImageValue::Float(3.5));
        assert_eq!(by("double"), ImageValue::Double(std::f64::consts::PI));
        assert_eq!(by("bool"), ImageValue::Bool(true));
        assert_eq!(by("uint"), ImageValue::Uint(4_000_000_000));
        assert_eq!(by("long"), ImageValue::Long(i64::MIN));
        match by("string") {
            ImageValue::Str(s) => assert_eq!(&*s, "hello"),
            ref o => panic!("String 여야 한다: {o:?}"),
        }
        match by("ext") {
            ImageValue::Property(n) => {
                assert_eq!(n.child("leaf").unwrap().value, Some(ImageValue::Int(7)))
            }
            ref o => panic!("Property 여야 한다: {o:?}"),
        }
    }

    #[test]
    fn float_uses_compressed_int_bit_pattern() {
        // 함정 검증: 0x04 는 압축 int 다. 4바이트 raw 가 아니다.
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("f"));
        b.push(flags::FLOAT);
        let bits = 3.5f32.to_bits() as i32;
        b.extend_from_slice(&crate::read::encode_compressed_i32(bits));

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        match root.child("f").unwrap().value.as_ref().unwrap() {
            ImageValue::Float(v) => assert_eq!(*v, 3.5),
            other => panic!("Float 여야 한다: {other:?}"),
        }
    }

    #[test]
    fn double_is_raw_le() {
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("d"));
        b.push(flags::DOUBLE);
        b.extend_from_slice(&std::f64::consts::PI.to_bits().to_le_bytes());

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        match root.child("d").unwrap().value.as_ref().unwrap() {
            ImageValue::Double(v) => assert_eq!(*v, std::f64::consts::PI),
            other => panic!("Double 여야 한다: {other:?}"),
        }
    }

    // ── 확장 객체 ───────────────────────────────────────────────────────────

    #[test]
    fn parses_property_nested() {
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("sub"));
        b.push(flags::EXTENDED);
        // 길이는 나중에 패치
        let len_pos = b.len();
        b.extend_from_slice(&0i32.to_le_bytes());
        // 내부는 "Property" 태그 + skip2 + entries
        let mut inner = Vec::new();
        inner.push(0x73);
        inner.extend_from_slice(&enc_str("Property"));
        inner.extend_from_slice(&2u16.to_le_bytes());
        inner.extend_from_slice(&crate::read::encode_compressed_i32(1));
        inner.extend(value_int("leaf", 7));
        b.extend_from_slice(&inner);
        let body_len = (b.len() - (len_pos + 4)) as i32;
        b[len_pos..len_pos + 4].copy_from_slice(&body_len.to_le_bytes());

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();

        match root.child("sub").unwrap().value.as_ref().unwrap() {
            ImageValue::Property(n) => {
                assert_eq!(n.children.len(), 1);
                assert_eq!(n.child("leaf").unwrap().value, Some(ImageValue::Int(7)));
            }
            other => panic!("Property 여야 한다: {other:?}"),
        }
    }

    #[test]
    fn extended_object_length_mismatch_is_error() {
        // 선언 길이를 실제보다 크게 조작 → 반드시 에러 (C# `Wz_Image.cs:523-527`)
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("v"));
        b.push(flags::EXTENDED);
        b.extend_from_slice(&999i32.to_le_bytes()); // 실제보다 큰 길이
        b.push(0x73);
        b.extend_from_slice(&enc_str("Shape2D#Vector2D"));
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend_from_slice(&crate::read::encode_compressed_i32(2));

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let err = p.parse_image_root(&mut c).unwrap_err();
        assert!(
            matches!(
                err,
                wz_error::WzError::Format(wz_error::WzFormatError::ObjectLengthMismatch { .. })
            ),
            "길이 불일치가 검출돼야 한다: {err}"
        );
    }

    #[test]
    fn parses_vector2d() {
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("v"));
        b.push(flags::EXTENDED);
        let inner_len_pos = {
            let mut inner = Vec::new();
            inner.push(0x73);
            inner.extend_from_slice(&enc_str("Shape2D#Vector2D"));
            inner.extend_from_slice(&crate::read::encode_compressed_i32(-5));
            inner.extend_from_slice(&crate::read::encode_compressed_i32(300));
            inner.len()
        };
        b.extend_from_slice(&(inner_len_pos as i32).to_le_bytes());
        b.push(0x73);
        b.extend_from_slice(&enc_str("Shape2D#Vector2D"));
        b.extend_from_slice(&crate::read::encode_compressed_i32(-5));
        b.extend_from_slice(&crate::read::encode_compressed_i32(300));

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        assert_eq!(
            root.child("v").unwrap().value,
            Some(ImageValue::Vector(Vec2 { x: -5, y: 300 }))
        );
    }

    #[test]
    fn parses_uol_unresolved() {
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("link"));
        b.push(flags::EXTENDED);
        let mut inner = Vec::new();
        inner.push(0x73);
        inner.extend_from_slice(&enc_str("UOL"));
        inner.push(0); // skip 1
                       // UOL 경로도 `ReadImageString` 이므로 0x00 플래그 접두가 필요하다
                       // (원본 `Wz_Image.cs:390`)
        let mut uol = vec![0x00u8];
        uol.extend_from_slice(&enc_str("../other.img/info"));
        inner.extend_from_slice(&uol);
        let n = inner.len() as i32;
        b.extend_from_slice(&n.to_le_bytes());
        b.extend_from_slice(&inner);

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        match root.child("link").unwrap().value.as_ref().unwrap() {
            ImageValue::Uol(v) => assert_eq!(&**v, "../other.img/info"),
            other => panic!("UOL 여야 한다: {other:?}"),
        }
    }

    #[test]
    fn parses_convex_with_points() {
        let mut pts = Vec::new();
        for (x, y) in [(0i32, 0i32), (10, 0), (10, 10), (0, 10)] {
            let mut v = vec![0x73u8];
            v.extend_from_slice(&enc_str("Shape2D#Vector2D"));
            v.extend_from_slice(&crate::read::encode_compressed_i32(x));
            v.extend_from_slice(&crate::read::encode_compressed_i32(y));
            pts.push(v);
        }

        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("c"));
        b.push(flags::EXTENDED);
        let mut inner = vec![0x73u8];
        inner.extend_from_slice(&enc_str("Shape2D#Convex2D"));
        inner.extend_from_slice(&crate::read::encode_compressed_i32(4));
        for p in &pts {
            inner.extend_from_slice(p);
        }
        let n = inner.len() as i32;
        b.extend_from_slice(&n.to_le_bytes());
        b.extend_from_slice(&inner);

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        match root.child("c").unwrap().value.as_ref().unwrap() {
            ImageValue::Convex(v) => {
                assert_eq!(v.len(), 4);
                assert_eq!(v[3], Vec2 { x: 0, y: 10 });
            }
            other => panic!("Convex 여야 한다: {other:?}"),
        }
    }

    #[test]
    fn convex_rejects_non_vector() {
        // 원본 `Wz_Image.cs:317-318` — Convex2D 는 Vector2D 만 허용
        let mut inner = vec![0x73u8];
        inner.extend_from_slice(&enc_str("Shape2D#Convex2D"));
        inner.extend_from_slice(&crate::read::encode_compressed_i32(1));
        inner.push(0x73);
        inner.extend_from_slice(&enc_str("UOL"));
        inner.push(0);
        inner.extend_from_slice(&enc_str("x"));

        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("c"));
        b.push(flags::EXTENDED);
        b.extend_from_slice(&(inner.len() as i32).to_le_bytes());
        b.extend_from_slice(&inner);

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        assert!(p.parse_image_root(&mut c).is_err());
    }

    #[test]
    fn parses_canvas_descriptor_only() {
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("img"));
        b.push(flags::EXTENDED);
        let mut inner = Vec::new();
        inner.push(0x73);
        inner.extend_from_slice(&enc_str("Canvas"));
        inner.push(0); // skip 1
        inner.push(0x00); // mini property 없음
        inner.extend_from_slice(&crate::read::encode_compressed_i32(64));
        inner.extend_from_slice(&crate::read::encode_compressed_i32(32));
        inner.extend_from_slice(&crate::read::encode_compressed_i32(2)); // ARGB8888
        inner.push(0); // scale
        inner.extend_from_slice(&crate::read::encode_compressed_i32(1)); // pages
        inner.extend_from_slice(&crate::read::encode_compressed_i32(0)); // unknown1
        inner.extend_from_slice(&2u16.to_le_bytes()); // skip 2
        inner.extend_from_slice(&16i32.to_le_bytes()); // dataLen (비압축!)
        inner.extend_from_slice(&[0x5A; 16]); // 페이로드 — 길이가 dataLen 과 같아야 한다
                                              // **inner 를 완성한 뒤** 길이를 재야 한다 (본문 길이 = i32 4바이트
                                              //  바로 뒤부터 끝까지)
        let n = inner.len() as i32;
        b.extend_from_slice(&n.to_le_bytes());
        b.extend_from_slice(&inner);

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        assert_eq!(c.pos(), c.len().unwrap());
        match root.child("img").unwrap().value.as_ref().unwrap() {
            ImageValue::Png(v) => {
                assert_eq!((v.width, v.height), (64, 32));
                assert_eq!(v.format, 2);
                assert_eq!(v.data_len, 16);
                assert!(v.sub_property.is_none());
            }
            other => panic!("Canvas 여야 한다: {other:?}"),
        }
    }

    #[test]
    fn parses_rawdata_and_skips_payload() {
        let mut inner = Vec::new();
        inner.push(0x73);
        inner.extend_from_slice(&enc_str("RawData"));
        inner.push(0); // ver
        inner.extend_from_slice(&crate::read::encode_compressed_i32(8));
        inner.extend_from_slice(&[0xAA; 8]);

        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("r"));
        b.push(flags::EXTENDED);
        b.extend_from_slice(&(inner.len() as i32).to_le_bytes());
        b.extend_from_slice(&inner);

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        match root.child("r").unwrap().value.as_ref().unwrap() {
            ImageValue::RawData(v) => assert_eq!(v.len, 8),
            other => panic!("RawData 여야 한다: {other:?}"),
        }
        // 페이로드를 건너뛰었으므로 정확히 끝까지 소비돼야 한다
        assert_eq!(c.pos(), c.len().unwrap());
    }

    #[test]
    fn parses_video() {
        let mut inner = Vec::new();
        inner.push(0x73);
        inner.extend_from_slice(&enc_str("Canvas#Video"));
        inner.push(0); // skip 1
        inner.push(0x00); // sub 없음
        inner.push(0x7F); // unknown
        inner.extend_from_slice(&crate::read::encode_compressed_i32(4));
        inner.extend_from_slice(&[1, 2, 3, 4]);

        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("v"));
        b.push(flags::EXTENDED);
        b.extend_from_slice(&(inner.len() as i32).to_le_bytes());
        b.extend_from_slice(&inner);

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        match root.child("v").unwrap().value.as_ref().unwrap() {
            ImageValue::Video(v) => {
                assert_eq!(v.len, 4);
                assert_eq!(v.unknown, 0x7F);
            }
            other => panic!("Video 여야 한다: {other:?}"),
        }
    }

    #[test]
    fn parses_sound_dx8_and_keeps_raw_fmt() {
        let mut inner = Vec::new();
        inner.push(0x73);
        inner.extend_from_slice(&enc_str("Sound_DX8"));
        inner.push(0); // ver
        inner.extend_from_slice(&crate::read::encode_compressed_i32(6)); // dataLen
        inner.extend_from_slice(&crate::read::encode_compressed_i32(120)); // duration
        inner.push(2); // soundDecl → fmt_ex 존재
        inner.extend_from_slice(&[1u8; 16]); // majorType
        inner.extend_from_slice(&[2u8; 16]); // subType
        inner.push(1); // fixedSizeSamples
        inner.push(0); // temporalCompression
        inner.extend_from_slice(&[3u8; 16]); // formatType
        inner.extend_from_slice(&crate::read::encode_compressed_i32(4)); // fmtExLen
        inner.extend_from_slice(&[0xFF, 0xFF, 0x00, 0x00]); // fmtEx (원시)
        inner.extend_from_slice(&[9u8; 6]); // 사운드 페이로드

        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("snd"));
        b.push(flags::EXTENDED);
        b.extend_from_slice(&(inner.len() as i32).to_le_bytes());
        b.extend_from_slice(&inner);

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        match root.child("snd").unwrap().value.as_ref().unwrap() {
            ImageValue::Sound(v) => {
                assert_eq!(v.data_len, 6);
                assert_eq!(v.duration, 120);
                assert_eq!(v.sound_decl, 2);
                assert_eq!(v.fmt_ex, vec![0xFF, 0xFF, 0x00, 0x00], "원시 보존");
                assert_eq!(&v.major_type, &[1u8; 16]);
            }
            other => panic!("Sound 여야 한다: {other:?}"),
        }
        assert_eq!(c.pos(), c.len().unwrap());
    }

    // ── 문자열 플래그 ──────────────────────────────────────────────────────

    #[test]
    fn string_flag_04_returns_null_after_8_bytes() {
        // 원본 `WzBinaryReader.cs:413-415`
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("n"));
        b.push(flags::STRING);
        b.push(0x04);
        b.extend_from_slice(&[0u8; 8]);

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        assert_eq!(root.child("n").unwrap().value, Some(ImageValue::Null));
        assert_eq!(c.pos(), c.len().unwrap(), "8바이트를 정확히 소비");
    }

    #[test]
    fn string_flag_01_backreference() {
        // 0x01: i32 오프셋 → 그 위치의 문자열
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(2));
        // 0 번째: 원본 문자열을 바이트열에 먼저 심어 둔다
        let target_pos = b.len() + 1; // 0x00 이름 플래그 바로 다음 = 길이 바이트
        b.extend(value_int("first", 1));
        // 1 번째: 역참조
        b.extend(name("second"));
        b.push(flags::STRING);
        b.push(0x01);
        b.extend_from_slice(&(target_pos as i32).to_le_bytes());

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        assert_eq!(root.child("first").unwrap().value, Some(ImageValue::Int(1)));
        // 역참조로 첫 번째 프로퍼티 이름("first") 을 가져와야 한다
        match root.child("second").unwrap().value.as_ref().unwrap() {
            ImageValue::Str(s) => assert_eq!(&**s, "first", "역참조로 이름 문자열을 가져온다"),
            other => panic!("String 여야 한다: {other:?}"),
        }
    }

    #[test]
    fn object_type_flag_1b_backreference() {
        // `0x1B` : `i32` 오프셋 → 그 위치의 **객체 타입 이름**
        // (원본 `WzBinaryReader.cs:397-398`)
        //
        // 실제 WZ 에서 역참조 대상은 **앞에서 이미 읽힌 객체 타입 이름**이다
        // (문자열 풀이 절대 파일 오프셋을 키로 캐싱한다).
        // 그래서 먼저 실제 Vector2D 객체를 하나 넣고, 그 `0x73` 위치를 가리킨다.
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(2));

        // (1) 실제 Vector2D — 여기 등록된다.
        //
        // `0x1B` 오프셋은 **0x73 태그 플래그 바로 다음**(길이 바이트)을 가리킨다.
        // 원본 `ReadStringAt` 이 `ReadString`(= 길이 바이트부터) 을 호출하기 때문
        // (`WzBinaryReader.cs:397-398`, `:421-432`).
        let vec_obj_pos = b.len() + name("a").len() + 1 + 4 + 1;
        b.extend(name("a"));
        b.push(flags::EXTENDED);
        let inner1 = {
            let mut v = vec![0x73u8];
            v.extend_from_slice(&enc_str("Shape2D#Vector2D"));
            v.extend_from_slice(&crate::read::encode_compressed_i32(11));
            v.extend_from_slice(&crate::read::encode_compressed_i32(22));
            v
        };
        b.extend_from_slice(&(inner1.len() as i32).to_le_bytes());
        b.extend_from_slice(&inner1);

        // (2) 0x1B 로 역참조
        b.extend(name("b"));
        b.push(flags::EXTENDED);
        let inner2 = {
            let mut v = vec![0x1Bu8];
            v.extend_from_slice(&(vec_obj_pos as i32).to_le_bytes());
            v.extend_from_slice(&crate::read::encode_compressed_i32(33));
            v.extend_from_slice(&crate::read::encode_compressed_i32(44));
            v
        };
        b.extend_from_slice(&(inner2.len() as i32).to_le_bytes());
        b.extend_from_slice(&inner2);

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let root = p.parse_image_root(&mut c).unwrap();
        assert_eq!(c.pos(), c.len().unwrap());
        assert_eq!(
            root.child("a").unwrap().value,
            Some(ImageValue::Vector(Vec2 { x: 11, y: 22 }))
        );
        assert_eq!(
            root.child("b").unwrap().value,
            Some(ImageValue::Vector(Vec2 { x: 33, y: 44 })),
            "0x1B 역참조로 Vector2D 태그를 재사용해야 한다"
        );
    }

    // ── 오류 처리 ──────────────────────────────────────────────────────────

    #[test]
    fn unknown_value_flag_is_error() {
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("x"));
        b.push(0x7F); // 미지원 플래그

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let err = p.parse_image_root(&mut c).unwrap_err();
        assert!(matches!(
            err,
            wz_error::WzError::Format(wz_error::WzFormatError::Malformed { .. })
        ));
    }

    #[test]
    fn unknown_object_tag_is_error_with_tag_name() {
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(1));
        b.extend(name("x"));
        b.push(flags::EXTENDED);
        let mut inner = vec![0x73u8];
        inner.extend_from_slice(&enc_str("Unknown#Tag"));
        b.extend_from_slice(&(inner.len() as i32).to_le_bytes());
        b.extend_from_slice(&inner);

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        let msg = p.parse_image_root(&mut c).unwrap_err().to_string();
        assert!(
            msg.contains("Unknown#Tag"),
            "태그명이 에러에 나와야 한다: {msg}"
        );
    }

    #[test]
    fn truncated_input_is_error_not_panic() {
        for n in 0..40usize {
            let mut b = vec![0x73];
            b.extend_from_slice(&enc_str("Property"));
            b.extend_from_slice(&2u16.to_le_bytes());
            b.extend_from_slice(&crate::read::encode_compressed_i32(3));
            b.truncate(b.len().min(20 + n)); // 다양한 절단 지점
            let mut c = ctx(b);
            let mut p = ImageParser::new(None);
            let _ = p.parse_image_root(&mut c); // 패닉만 없으면 OK
        }
    }

    #[test]
    fn negative_entry_count_is_error() {
        let mut b = Vec::new();
        b.push(0x73);
        b.extend_from_slice(&enc_str("Property"));
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&crate::read::encode_compressed_i32(-1));

        let mut c = ctx(b);
        let mut p = ImageParser::new(None);
        assert!(p.parse_image_root(&mut c).is_err(), "음수 개수는 거부");
    }

    // ── 노드 유틸 ──────────────────────────────────────────────────────────

    #[test]
    fn node_find_by_path() {
        let n = Node {
            name: Arc::from(""),
            value: None,
            children: vec![
                Node {
                    name: Arc::from("a"),
                    value: Some(ImageValue::Int(1)),
                    children: vec![Node {
                        name: Arc::from("b"),
                        value: Some(ImageValue::Int(2)),
                        children: vec![],
                    }],
                },
                Node {
                    name: Arc::from("c"),
                    value: Some(ImageValue::Int(3)),
                    children: vec![],
                },
            ],
        };
        assert_eq!(n.find("a").unwrap().value, Some(ImageValue::Int(1)));
        assert_eq!(n.find("a/b").unwrap().value, Some(ImageValue::Int(2)));
        assert_eq!(n.find("c").unwrap().value, Some(ImageValue::Int(3)));
        assert!(n.find("a/x").is_none());
        assert!(n.find("zzz").is_none());
        assert_eq!(n.count(), 4, "root + a + a/b + c");
    }

    #[test]
    fn duplicate_names_are_kept() {
        // 리포트 §3.2 S-9: C# 은 중복을 "마지막 유지/역방향 스캔" 으로
        // 우연히 일치시켰다. 우리는 **전부** 보존하고 첫 번째를 반환한다.
        let n = Node {
            name: Arc::from(""),
            value: None,
            children: vec![Node::new("dup"), Node::new("dup")],
        };
        assert_eq!(n.children.len(), 2, "중복 이름이 보존된다");
        assert_eq!(n.child("dup").unwrap().name.as_ref(), "dup");
    }

    #[test]
    fn type_names_are_stable() {
        // 오라클 JSON 스키마가 이 이름에 의존한다
        assert_eq!(ImageValue::Null.type_name(), "Null");
        assert_eq!(ImageValue::Int(0).type_name(), "Int");
        assert_eq!(ImageValue::Uol(Arc::from("x")).type_name(), "UOL");
        assert_eq!(
            ImageValue::Property(Arc::new(Node::default())).type_name(),
            "Property"
        );
    }
}
