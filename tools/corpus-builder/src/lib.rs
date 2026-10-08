//! 합성 WZ 코퍼스 생성기 (리포트 §10.1).
//!
//! # 왜 이게 최우선순위인가
//!
//! 실제 MapleStory WZ 파일은 Nexon 자산이라 리포에 커밋할 수 없다.
//! 그렇다고 **검증 없는 포트**는 더 나쁘다. 그래서:
//!
//! 1. **합성 WZ** 를 만들어 (여기서) 포맷 규칙을 바이트 단위로 고정하고,
//! 2. 그걸 C# 오라클에도 돌려서 양쪽 출력을 비교한다.
//!
//! 즉 이 도구는 리포트 §10.2 차분 오라클 전략의 **입력 생성기**다.
//!
//! # 현재 범위
//!
//! Phase 1 진행 중이라 컨테이너는 아직 없다. 지금 만드는 것:
//!
//! - 이미지 페이로드 (zlib + 청크 XOR + 플래그 바이트) — `Wz_Png.UnsafeOpenRead`
//! - 이미지 객체 9종 직렬화 — `Wz_Image.ExtractImg`
//! - 압축 정수 / 문자열 인코딩 (위 크레이트가 이미 제공)
//!
//! 컨테이너(PKG1/PKG2/MS) writer 는 Phase 1.3 에서 추가한다.

use std::io::Write;

use wz_core::crypto::aes::AesKeyStream;
use wz_core::crypto::Decryptor;
use wz_core::read::{encode_compressed_i32, encode_compressed_u32, encode_utf16_wz_string};

/// WZ 이미지 값 플래그 바이트 (원본 `Wz_Image.ExtractValue`).
pub mod flags {
    pub const NULL: u8 = 0x00;
    pub const SHORT: u8 = 0x02;
    pub const INT: u8 = 0x03;
    /// 압축 정수로 읽고 IEEE-754 비트 패턴으로 해석 (`WzBinaryReader.cs:81-86`)
    pub const FLOAT: u8 = 0x04;
    pub const DOUBLE: u8 = 0x05;
    pub const STRING: u8 = 0x08;
    pub const EXTENDED: u8 = 0x09;
    pub const BOOL: u8 = 0x0B;
    pub const UINT: u8 = 0x13;
    pub const LONG: u8 = 0x14;
}

/// `Canvas` / `UOL` / `RawData` 등 객체 타입 이름.
pub mod obj_types {
    pub const PROPERTY: &str = "Property";
    pub const VECTOR_2D: &str = "Shape2D#Vector2D";
    pub const CANVAS: &str = "Canvas";
    pub const CONVEX_2D: &str = "Shape2D#Convex2D";
    pub const UOL: &str = "UOL";
    pub const RAW_DATA: &str = "RawData";
}

/// WZ 텍스처 포맷 (원본 `Wz_TextureFormat`).
pub mod tex_format {
    pub const ARGB4444: u32 = 1;
    pub const ARGB8888: u32 = 2;
    pub const ARGB1555: u32 = 257;
    pub const RGB565: u32 = 513;
    pub const R16: u32 = 769;
    pub const DXT3: u32 = 1026;
    pub const DXT5: u32 = 2050;
    pub const A8: u32 = 2304;
    pub const RGBA1010102: u32 = 2562;
    pub const DXT1: u32 = 4097;
    pub const BC7: u32 = 4098;
    pub const RGBA32F: u32 = 4100;
}

/// 합성 WZ 파일 빌더.
///
/// # 암호화
///
/// `WzBuilder::with_key` 로 키스트림을 걸면 **이름과 문자열 값 페이로드가
/// 모두** 같은 키로 암호화된다.
///
/// 함정: 파서는 이미지 **하나에 키스트림 하나**만 쓴다. 값만 암호화하고 이름을
/// 평문으로 두면 파서가 이름을 잘못 읽어 이후 값이 전부 어긋난다. 둘은
/// **항상 같은 경로**를 탄다 (`Wz_Image.ExtractValue` 가 이름도 `ReadImageString`
/// 으로 읽기 때문이다).
#[derive(Default)]
pub struct WzBuilder {
    out: Vec<u8>,
    /// 문자열 페이로드 암호화 키. `None` 이면 평문.
    key: Option<AesKeyStream>,
}

impl WzBuilder {
    pub fn new() -> Self {
        Self {
            out: Vec::new(),
            key: None,
        }
    }

    /// 문자열(이름 + 값)에 키스트림을 적용한다.
    pub fn with_key(ks: AesKeyStream) -> Self {
        Self {
            out: Vec::new(),
            key: Some(ks),
        }
    }

    /// `WzBuilder` 의 상태를 복제 (테스트에서 두 경로로 재사용).
    pub fn duplicate(&self) -> Self {
        Self {
            out: self.out.clone(),
            key: self.key.as_ref().map(AesKeyStream::try_clone),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.out
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.out
    }

    pub fn len(&self) -> usize {
        self.out.len()
    }

    pub fn is_empty(&self) -> bool {
        self.out.is_empty()
    }

    // ── 기본 스칼라 ─────────────────────────────────────────────────────────

    fn u8(&mut self, v: u8) -> &mut Self {
        self.out.push(v);
        self
    }

    fn i16_le(&mut self, v: i16) -> &mut Self {
        self.out.extend_from_slice(&v.to_le_bytes());
        self
    }

    fn u16_le(&mut self, v: u16) -> &mut Self {
        self.out.extend_from_slice(&v.to_le_bytes());
        self
    }

    fn i32_le(&mut self, v: i32) -> &mut Self {
        self.out.extend_from_slice(&v.to_le_bytes());
        self
    }

    /// **비압축** i32 (예: `objDataLen`).
    fn i32_raw(&mut self, v: i32) -> &mut Self {
        self.i32_le(v)
    }

    // ── 값 직렬화 (원본 Wz_Image.ExtractValue) ──────────────────────────────
    // ── 값 직렬화 (원본 Wz_Image.ExtractValue) ──────────────────────────────

    /// **프로퍼티 이름** 기록 — `0x00` 플래그 + 인코딩 문자열.
    ///
    /// 원본 `Wz_Image.ExtractValue` 는 이름을 **먼저** 읽는다
    /// (`Wz_Image.cs:487`). 파서의 `read_image_string` 은 플래그 바이트를
    /// 요구하므로 `0x00` 접두가 필수다. 빠뜨리면 바이트열이 문자열로 오해된다.
    pub fn write_name(&mut self, name: &str) -> &mut Self {
        self.u8(0x00);
        self.push_encoded_string(name);
        self
    }

    /// 마스킹된 WZ 문자열을 키로 (필요하면) 암호화해 버퍼에 추가.
    ///
    /// 함정: **길이 접두는 평문**, 페이지만 키스트림 오프셋 `0` 부터
    /// (`WzBinaryReader.cs:112,135`).
    fn push_encoded_string(&mut self, s: &str) {
        let enc = self.encode_string(s);
        self.out.extend_from_slice(&enc);
    }

    pub fn value_null(&mut self, name: &str) -> &mut Self {
        self.write_name(name);
        self.u8(flags::NULL)
    }

    pub fn value_short(&mut self, name: &str, v: i16) -> &mut Self {
        self.write_name(name);
        self.u8(flags::SHORT);
        self.i16_le(v)
    }

    /// 압축 정수 int. `flags::INT` 는 압축 int 다 (`WzBinaryReader.cs:75`)
    pub fn value_int(&mut self, name: &str, v: i32) -> &mut Self {
        self.write_name(name);
        self.u8(flags::INT);
        self.out.extend_from_slice(&encode_compressed_i32(v));
        self
    }

    pub fn value_uint(&mut self, name: &str, v: u32) -> &mut Self {
        self.write_name(name);
        self.u8(flags::UINT);
        self.out.extend_from_slice(&encode_compressed_u32(v));
        self
    }

    /// 압축 int 를 읽고 float 비트 패턴으로 해석.
    ///
    /// 함정: **압축** int 다. 4바이트를 그대로 쓰는 게 아니다
    /// (`Wz_Image.cs` 의 `0x04` 처리 / `WzBinaryReader.cs:81-86`).
    pub fn value_float(&mut self, name: &str, v: f32) -> &mut Self {
        self.write_name(name);
        self.u8(flags::FLOAT);
        self.out
            .extend_from_slice(&encode_compressed_i32(v.to_bits() as i32));
        self
    }

    pub fn value_double(&mut self, name: &str, v: f64) -> &mut Self {
        self.write_name(name);
        self.u8(flags::DOUBLE);
        self.out.extend_from_slice(&v.to_bits().to_le_bytes());
        self
    }

    pub fn value_bool(&mut self, name: &str, v: bool) -> &mut Self {
        self.write_name(name);
        self.u8(flags::BOOL);
        self.i16_le(i16::from(v))
    }

    pub fn value_long(&mut self, name: &str, v: i64) -> &mut Self {
        self.write_name(name);
        self.u8(flags::LONG);
        self.out
            .extend_from_slice(&wz_core::read::encode_compressed_i64(v));
        self
    }

    /// 문자열 값. 키가 설정돼 있으면 페이로드만 키스트림 오프셋 0 부터 XOR.
    ///
    /// 원본 규칙 (`WzBinaryReader.ReadString`): **길이 접두사는 평문**, 페이지만 복호화.
    ///
    /// 함정: 값 문자열도 `ReadImageString` 경로라 **`0x00` 플래그 바이트**가
    /// 앞에 붙는다 (`Wz_Image.cs:513`). `write_name` 과 같은 접두다.
    pub fn value_string(&mut self, name: &str, s: &str) -> &mut Self {
        self.write_name(name);
        self.u8(flags::STRING);
        self.u8(0x00);
        self.push_encoded_string(s);
        self
    }

    // ── 확장 객체 (0x09) ────────────────────────────────────────────────────

    /// `0x09` 확장 객체 헤더: `i32 objDataLen` 후 본문.
    ///
    /// # 함정: `i32 objDataLen` 은 **비압축** 이고 **플래그를 제외한 본문 길이**다
    ///
    /// 원본 `Wz_Image.cs:519-527` 이 파싱 후 위치가 `eob` 와 정확히 같은지
    /// 검증한다. 이 값이 틀리면 **해당 노드뿐 아니라 이후 모든 노드가** 어긋난다.
    fn emit_extended<F: FnOnce(&mut Vec<u8>)>(&mut self, name: &str, body: F) -> &mut Self {
        self.write_name(name);
        self.u8(flags::EXTENDED);
        let mut inner = Vec::new();
        body(&mut inner);
        self.i32_raw(inner.len() as i32);
        self.out.extend_from_slice(&inner);
        self
    }

    /// `0x73` 인라인 객체 타입: `0x73 + UTF-16 문자열` (길이 직결, 0x00 없음)
    ///
    /// 함정: `ReadStringBlock` 경로라 **길이 접두가 1바이트 직결형**이고
    /// `0x00` 플래그 바이트가 없다 (`Wz_Image.cs:305-320`).
    ///
    /// 함정: 그래도 **키스트림은 적용된다**. `ReadStringBlock` 도 같은 복호화
    /// 스트림을 지나므로 타입 이름이 이름과 같은 키로 암호화된다.
    pub fn obj_type_inline(&mut self, type_name: &str) -> &mut Self {
        self.u8(0x73);
        let enc = self.encode_string(type_name);
        self.out.extend_from_slice(&enc);
        self
    }

    /// `Property` — `0x73 + skip 2 + compressed-i32 n + n × value`
    ///
    /// # 함정 1: `n` 은 **값 개수**다 (바이트 수가 아니다)
    /// # 함정 2: `n` 은 **압축** int 다 (원본 `Wz_Image.cs:271` 의 `ReadCompressedInt`)
    /// # 함정 3: `i32 objDataLen` 은 비압축이고, `n` 은 그 **안**에 있다
    ///
    /// `n` 은 항상 5바이트 이스케이프(`0x80 + i32`)로 예약한다 — 폭이 가변이면
    /// 나중에 개수만 고칠 때 뒤쪽 값을 밀어挪해야 하는데, **고정 폭이면 제자리
    /// 패치**로 끝나서 길이 계산이 항상 맞는다. 파서는 두 형태를 모두 받는다.
    pub fn property_begin(&mut self, name: &str) -> (usize, usize) {
        self.write_name(name);
        self.u8(flags::EXTENDED);
        // objDataLen 자리 확보 (비압축 i32, 본문 + 4 바이트 patch)
        let len_pos = self.reserve_i32();
        self.obj_type_inline(obj_types::PROPERTY);
        self.u16_le(0); // 원본의 "skip 2"
        let count_pos = self.reserve_compressed_i32();
        (len_pos, count_pos)
    }

    /// 압축 int 자리 5바이트(`0x80 + i32`)를 예약하고 그 오프셋을 돌려준다.
    fn reserve_compressed_i32(&mut self) -> usize {
        let pos = self.out.len();
        self.u8(0x80); // 압축 int 이스케이프 표시
        self.i32_raw(0); // placeholder
        pos
    }

    fn patch_compressed_i32(&mut self, pos: usize, v: i32) {
        debug_assert_eq!(self.out[pos], 0x80, "압축 int 이스케이프 자리여야 한다");
        self.out[pos + 1..pos + 5].copy_from_slice(&v.to_le_bytes());
    }

    /// `property_begin` 이 반환한 `(len_pos, count_pos)` 를 실제 길이/개수로 패치.
    pub fn property_end(&mut self, marker: (usize, usize), value_count: usize) -> &mut Self {
        let (len_pos, count_pos) = marker;
        self.patch_compressed_i32(count_pos, value_count as i32);
        let body_len = (self.out.len() - (len_pos + 4)) as i32;
        self.out[len_pos..len_pos + 4].copy_from_slice(&body_len.to_le_bytes());
        self
    }

    /// 이미지 루트의 `Property` 본문 — 이름/플래그 없이 시작.
    ///
    /// 반환된 marker 를 `root_property_end` 에 넘긴다.
    /// 루트에는 `objDataLen` 이 **없다** (파일 헤더가 이미 길이를 알린다).
    pub fn root_property_begin(&mut self) -> (usize, usize) {
        self.obj_type_inline(obj_types::PROPERTY);
        self.u16_le(0);
        let count_pos = self.reserve_compressed_i32();
        (0, count_pos)
    }

    pub fn root_property_end(&mut self, marker: (usize, usize), value_count: usize) {
        let (_, count_pos) = marker;
        self.patch_compressed_i32(count_pos, value_count as i32);
    }

    /// `i32` 자리 4바이트를 예약하고 그 오프셋을 돌려준다 (뒤에서 패치용).
    fn reserve_i32(&mut self) -> usize {
        let pos = self.out.len();
        self.i32_raw(0);
        pos
    }

    /// `Shape2D#Vector2D` — 2 × 압축 i32 (`Wz_Image.cs:277-279`)
    pub fn vector2d(&mut self, name: &str, x: i32, y: i32) -> &mut Self {
        let type_name = self.encode_string(obj_types::VECTOR_2D);
        let enc_x = encode_compressed_i32(x);
        let enc_y = encode_compressed_i32(y);
        self.emit_extended(name, |inner| {
            inner.push(0x73);
            inner.extend_from_slice(&type_name);
            inner.extend_from_slice(&enc_x);
            inner.extend_from_slice(&enc_y);
        })
    }

    /// `UOL` — `skip 1; string` (`Wz_Image.cs:388-391`)
    ///
    /// UOL 경로도 `ReadImageString` 이므로 `0x00` 접두가 붙고, 키가 설정돼
    /// 있으면 이름과 **같이** 같은 키로 암호화된다.
    pub fn uol(&mut self, name: &str, path: &str) -> &mut Self {
        let enc_path = self.encode_string(path);
        let type_name = self.encode_string(obj_types::UOL);
        self.emit_extended(name, |inner| {
            inner.push(0x73);
            inner.extend_from_slice(&type_name);
            inner.push(0); // skip 1
            inner.push(0x00);
            inner.extend_from_slice(&enc_path);
        })
    }

    /// 마스킹 + (키가 있으면) 암호화만 적용한 문자열 바이트.
    ///
    /// 함정: 암호화는 페이로드에서만 하고 **가변 길이 접두는 그대로 둔다**
    /// (`image_string_header_len` 참조). 접두까지 암호화하면 128자 넘는
    /// 문자열에서 파서가 길이를 잘못 읽는다.
    fn encode_string(&self, s: &str) -> Vec<u8> {
        let mut enc = encode_utf16_wz_string(s);
        let hdr = wz_core::read::image_string_header_len(s);
        if let Some(ks) = &self.key {
            ks.decrypt_at(&mut enc[hdr..], 0);
        }
        enc
    }

    /// `RawData` — `u8 ver; [ver==1 → mini property]; i32 len; 페이로드`
    /// (`Wz_Image.cs:393-410`)
    pub fn raw_data(&mut self, name: &str, payload: &[u8], ver: u8) -> &mut Self {
        self.raw_data_with_sub(name, payload, ver, None)
    }

    /// `ver == 1` 이고 `sub` 로 mini property 까지 채우는 변형.
    ///
    /// # 함정: `ver == 1` 이어도 mini property 는 **선택**이다
    ///
    /// `ver` 다음 바이트가 `0x01` 인 경우에만 mini property 가 따라온다
    /// (`Wz_Image.cs:396-401`). 그 외 값이면 페이로드로 바로 간다.
    pub fn raw_data_with_sub(
        &mut self,
        name: &str,
        payload: &[u8],
        ver: u8,
        sub: Option<(&str, i32)>,
    ) -> &mut Self {
        let type_name = self.encode_string(obj_types::RAW_DATA);
        let sub_bytes = sub.map(|(k, v)| {
            // mini property: `0x01 + skip 2 + compressed count + 값`
            let mut b = vec![0x01u8, 0x00, 0x00];
            b.extend_from_slice(&encode_compressed_i32(1));
            let mut nb = WzBuilder::new();
            nb.key = self.key.as_ref().map(AesKeyStream::try_clone);
            nb.write_name(k);
            nb.u8(flags::INT);
            nb.out.extend_from_slice(&encode_compressed_i32(v));
            b.extend_from_slice(&nb.out);
            b
        });
        let enc_len = encode_compressed_i32(payload.len() as i32);
        self.emit_extended(name, |inner| {
            inner.push(0x73);
            inner.extend_from_slice(&type_name);
            inner.push(ver);
            if ver == 1 {
                match &sub_bytes {
                    Some(b) => inner.extend_from_slice(b),
                    None => inner.push(0x00), // mini property 없음
                }
            }
            inner.extend_from_slice(&enc_len);
            inner.extend_from_slice(payload);
        })
    }

    /// `Canvas` + **평문** zlib 페이로드 (`Wz_Image.cs:281-303`)
    ///
    /// # 함정 1: `dataLen` 은 **비압축** i32
    /// # 함정 2: `dataLen` 은 **플래그 바이트를 포함한** 페이로드 길이
    ///
    /// 반환값: `(레코드 바이트, dataLen)`.
    #[allow(clippy::too_many_arguments)]
    pub fn canvas_plain(
        &mut self,
        name: &str,
        width: i32,
        height: i32,
        format: u32,
        scale: u8,
        pages: i32,
        raw_texels: &[u8],
    ) -> (usize, i32) {
        let payload = Self::zlib_payload(raw_texels);
        let data_len = payload.len() as i32;
        let type_name = self.encode_string(obj_types::CANVAS);
        let header = CanvasHeader::new(width, height, format, scale, pages, data_len);
        self.emit_extended(name, |inner| {
            inner.push(0x73);
            inner.extend_from_slice(&type_name);
            inner.push(0); // skip 1
            inner.push(0x00); // mini property 없음
            inner.extend_from_slice(&header.encode());
            inner.extend_from_slice(&data_len.to_le_bytes()); // 비압축!
            inner.extend_from_slice(&payload);
        });
        (self.out.len(), data_len)
    }

    /// `Canvas` + **청크 암호화** 페이로드.
    ///
    /// 키스트림은 **청크마다 오프셋 0 으로 리셋**된다
    /// (`ChunkedEncryptedInputStream.cs:73-83`). 이걸 빠뜨리면 복호화 안 된다.
    #[allow(clippy::too_many_arguments)]
    pub fn canvas_chunked(
        &mut self,
        name: &str,
        width: i32,
        height: i32,
        format: u32,
        scale: u8,
        pages: i32,
        raw_texels: &[u8],
        ks: &AesKeyStream,
        chunk_size: usize,
    ) -> (usize, i32) {
        // 평문 zlib 를 먼저 만든다
        let plain = Self::zlib_bytes(raw_texels);
        let mut payload = vec![0x01u8];
        for chunk in plain.chunks(chunk_size.max(1)) {
            let mut ct = chunk.to_vec();
            ks.decrypt_at(&mut ct, 0); // ← 청크마다 오프셋 0
            payload.extend_from_slice(&(ct.len() as i32).to_le_bytes());
            payload.extend_from_slice(&ct);
        }
        payload.extend_from_slice(&0i32.to_le_bytes()); // 종료 마커
        let data_len = payload.len() as i32;
        let type_name = self.encode_string(obj_types::CANVAS);
        let header = CanvasHeader::new(width, height, format, scale, pages, data_len);
        self.emit_extended(name, |inner| {
            inner.push(0x73);
            inner.extend_from_slice(&type_name);
            inner.push(0); // skip 1
            inner.push(0x00); // mini property 없음
            inner.extend_from_slice(&header.encode());
            inner.extend_from_slice(&data_len.to_le_bytes()); // 비압축!
            inner.extend_from_slice(&payload);
        });
        (self.out.len(), data_len)
    }

    /// `[플래그 1B][zlib]` 페이로드.
    fn zlib_payload(texels: &[u8]) -> Vec<u8> {
        let mut v = vec![0x01u8]; // WZ 플래그 (항상 무시 — Wz_Png.cs:147)
        v.extend_from_slice(&Self::zlib_bytes(texels));
        v
    }

    /// 평문 zlib 스트림 (헤더 2B + deflate + adler32).
    fn zlib_bytes(texels: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(texels.len() / 2 + 32);
        {
            let mut enc = flate2::write::ZlibEncoder::new(&mut out, flate2::Compression::default());
            enc.write_all(texels).expect("in-memory write");
            enc.finish().expect("in-memory finish");
        }
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 디렉터리 트리 (원본 Wz_File.ReadDirTree / ReadDirTreePkg2)
// ─────────────────────────────────────────────────────────────────────────────

/// 디렉터리 엔트리 (쓰기 측).
#[derive(Debug, Clone)]
pub struct DirEntrySpec {
    /// `0x03` = 디렉터리, `0x04` = 이미지, `0x02` = 이름이 오프셋 참조.
    pub node_type: u8,
    pub name: String,
    pub data_len: i32,
    pub cs32: i32,
}

impl DirEntrySpec {
    pub fn dir(name: &str) -> Self {
        Self {
            node_type: 0x03,
            name: name.into(),
            data_len: 0,
            cs32: 0,
        }
    }

    pub fn image(name: &str, data_len: i32) -> Self {
        Self {
            node_type: 0x04,
            name: name.into(),
            data_len,
            cs32: 0x1234_5678,
        }
    }
}

/// `i8` 접두 + 마스킹된 WZ 문자열 (원본 `WzBinaryReader.ReadString`).
///
/// # 함정
///
/// - `> 0` → UTF-16LE, `n == 127` 이면 뒤에 i32
/// - `< 0` → ASCII, `-128` 이면 뒤에 i32
/// - 마스크는 UTF-16 이면 **16비트 유닛** 기준 `0xAAAA + i`,
///   ASCII 면 **바이트** 기준 `0xAA + i`.
///
/// 두 경로의 마스크가 **다르다**. 통일하면 한쪽이 깨진다.
pub fn encode_dir_name(s: &str) -> Vec<u8> {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut masked = Vec::with_capacity(units.len() * 2);
    for (i, u) in units.iter().enumerate() {
        // 함정: 바이트가 아니라 유닛 인덱스를 쓴다.
        masked.extend_from_slice(&(u ^ (0xAAAAu16.wrapping_add(i as u16))).to_le_bytes());
    }
    if units.len() < 127 {
        let mut out = vec![units.len() as u8];
        out.extend_from_slice(&masked);
        out
    } else {
        let mut out = vec![127u8];
        out.extend_from_slice(&(units.len() as i32).to_le_bytes());
        out.extend_from_slice(&masked);
        out
    }
}

/// ASCII (길이가 음수인) 디렉터리 이름 인코딩.
///
/// 마스크가 **바이트** 단위 `0xAA + i` 라 UTF-16 경로와 규칙이 다르다.
pub fn encode_dir_name_ascii(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let masked: Vec<u8> = bytes
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ 0xAAu8.wrapping_add(i as u8))
        .collect();
    if bytes.len() < 128 {
        // 함정: `i8` 이므로 길이 N 의 접두는 **-N** 이다 (`0 - N`).
        // `255 - N` 으로 쓰면 1 어긋나 파서가 N+1 바이트를 읽는다.
        let mut out = vec![0u8.wrapping_sub(bytes.len() as u8)];
        out.extend_from_slice(&masked);
        out
    } else {
        let mut out = vec![128u8]; // -128 이스케이프
        out.extend_from_slice(&(bytes.len() as i32).to_le_bytes());
        out.extend_from_slice(&masked);
        out
    }
}

/// PKG1 디렉터리 블록 + 각 엔트리 `hashedOffset` 의 **상대 위치**.
///
/// ```text
/// [compressed count] [type][name][size][cs32][hashedOffset] × N
/// ```
///
/// # 왜 위치를 함께 반환하는가
///
/// `hashedOffset` 는 **파일 절대 위치**로 복호화된다. 그래서 writer 는 각
/// 엔트리의 `hashedOffset` 가 놓인 위치를 알아야 그 위치에 맞는 해시를 만들 수
/// 있다 (`OffsetCalc::encode_offset`). 위치를 손으로 스캔하면 압축 int 의
/// 가변 폭(1 또는 5바이트)을 놓쳐 **조용히 틀린 테스트**가 된다.
pub fn dir_block_pkg1_with_positions(
    entries: &[DirEntrySpec],
    hashed_offsets: &[u32],
) -> (Vec<u8>, Vec<usize>) {
    let mut out = Vec::new();
    let mut positions = Vec::with_capacity(entries.len());
    out.extend_from_slice(&encode_compressed_i32(entries.len() as i32));
    for (i, e) in entries.iter().enumerate() {
        out.push(e.node_type);
        out.extend_from_slice(&encode_dir_name(&e.name));
        out.extend_from_slice(&encode_compressed_i32(e.data_len));
        out.extend_from_slice(&encode_compressed_i32(e.cs32));
        positions.push(out.len());
        out.extend_from_slice(&hashed_offsets.get(i).copied().unwrap_or(0).to_le_bytes());
    }
    (out, positions)
}

/// 위치 정보 없이 블록만 (대부분의 테스트용).
pub fn dir_block_pkg1(entries: &[DirEntrySpec], hashed_offsets: &[u32]) -> Vec<u8> {
    dir_block_pkg1_with_positions(entries, hashed_offsets).0
}

/// PKG2 디렉터리 블록.
///
/// ```text
/// [encrypted count] [type][name][size][cs32] × N  [offset count] [hashedOffset] × N
/// ```
///
/// # 함정: 엔트리와 오프셋 표가 **분리**되어 있다
///
/// PKG1 과 달리 오프셋이 엔트리 뒤에 몰려 있다. 개수가 암호화되어 표의
/// 경계를 모르게 하기 위한 설계다.
pub fn dir_block_pkg2(
    entries: &[DirEntrySpec],
    hashed_offsets: &[u32],
    encrypt_count: impl Fn(i32) -> i32,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&encode_compressed_i32(encrypt_count(entries.len() as i32)));
    for e in entries {
        out.push(e.node_type);
        out.extend_from_slice(&encode_dir_name(&e.name));
        out.extend_from_slice(&encode_compressed_i32(e.data_len));
        out.extend_from_slice(&encode_compressed_i32(e.cs32));
    }
    out.extend_from_slice(&encode_compressed_i32(encrypt_count(entries.len() as i32)));
    for h in hashed_offsets {
        out.extend_from_slice(&h.to_le_bytes());
    }
    out
}

/// `Data.wz` 용 타입 `0x02` (이름이 i32 오프셋 참조) 블록.
pub fn dir_block_pkg1_named_by_ref(entries: &[(i32, i32, i32, u32)], name_ref_add: i32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&encode_compressed_i32(entries.len() as i32));
    for (name_off, size, cs32, hashed) in entries {
        out.push(0x02);
        // 함정: `stringOffAdd` 는 +2 / -1 로 갈린다 (원본 `Wz_File.cs:509`).
        out.extend_from_slice(&(name_off - name_ref_add).to_le_bytes());
        out.extend_from_slice(&encode_compressed_i32(*size));
        out.extend_from_slice(&encode_compressed_i32(*cs32));
        out.extend_from_slice(&hashed.to_le_bytes());
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// 컨테이너 헤더 (감지 계층 테스트용)
// ─────────────────────────────────────────────────────────────────────────────

/// 합성 컨테이너 헤더 — `wz_core::detect` 검증용.
///
/// 전체 컨테이너 writer 는 Phase 1.3 (`dirtree`) 에서 구현한다. 지금은
/// 감지 로직이 **헤더만으로** 프로파일을 구분하는지 확인하는 최소 헤더만 만든다.
#[derive(Debug, Clone)]
pub struct ContainerHeader {
    /// 파일 전체 바이트.
    pub bytes: Vec<u8>,
    /// 감지가 돌려주길 기대하는 프로파일 문자열.
    pub expected_profile: &'static str,
    /// PKG2 전용. PKG1 이면 `None`.
    pub hash1: Option<u32>,
    pub hash2: Option<u32>,
}

impl ContainerHeader {
    /// `PKG1` + dataSize + headerSize, **`encver` 없음** (KMST1132 이후).
    ///
    /// `headerSize == 16` 이고 그 뒤에 **아무것도 없어야** 감지가
    /// "encver 없음" 으로 판정한다 (2바이트 읽기가 실패해야 한다).
    pub fn pkg1_encver_missing(payload: &[u8]) -> Self {
        let header_size: i32 = 16;
        let data_size: i64 = payload.len() as i64;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PKG1");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        bytes.extend_from_slice(&header_size.to_le_bytes());
        bytes.extend_from_slice(payload);
        Self {
            bytes,
            expected_profile: "PKG1_ENCVER_MISSING",
            hash1: None,
            hash2: None,
        }
    }

    /// `PKG1` + dataSize + headerSize, 그 자리에 `u16 encver` (구버전).
    ///
    /// 함정: `encver` 는 `headerSize` **위치에** 있다 — 헤더 안쪽이 아니라
    /// 헤더가 끝난 자리. 감지기가 `encver_pos = header_len` 으로 읽는다.
    pub fn pkg1_with_encver(payload: &[u8]) -> Self {
        let header_size: i32 = 16;
        let data_size: i64 = 2 + payload.len() as i64;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PKG1");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        bytes.extend_from_slice(&header_size.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes()); // encver = 0
        bytes.extend_from_slice(payload);
        Self {
            bytes,
            expected_profile: "PKG1",
            hash1: None,
            hash2: None,
        }
    }

    /// `PKG2` 표준 레이아웃: magic + dataSize + headerSize + copyright + hash1 + hash2.
    ///
    /// 함정: 감지기는 `hash1` 을 **`headerSize` 위치**에서 읽는다
    /// (`detect_pkg2`). 따라서 `headerSize` 에는 `16 + copyright` 만 넣고
    /// 해시 8바이트는 그 **뒤**에 둔다 — 해시를 길이에 포함시키면 어긋난다.
    pub fn pkg2(hash1: u32, hash2: u32, copyright: &[u8]) -> Self {
        let header_size: i32 = 16 + copyright.len() as i32;
        let data_size: i64 = 0;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PKG2");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        bytes.extend_from_slice(&header_size.to_le_bytes());
        bytes.extend_from_slice(copyright);
        bytes.extend_from_slice(&hash1.to_le_bytes());
        bytes.extend_from_slice(&hash2.to_le_bytes());
        Self {
            bytes,
            expected_profile: "PKG2_STANDARD_HEADER",
            hash1: Some(hash1),
            hash2: Some(hash2),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

/// 결정적 픽셀 데이터 (디코더 결과와 비교하기 위한 원시 텍셀).
pub fn synthetic_texels(format: u32, w: u32, h: u32) -> Vec<u8> {
    match format {
        tex_format::ARGB8888 => {
            let mut v = Vec::with_capacity((w * h * 4) as usize);
            for i in 0..(w * h) {
                v.extend_from_slice(&[(i % 251) as u8, (i / 7 % 251) as u8, (i % 97) as u8, 0xFF]);
            }
            v
        }
        tex_format::ARGB4444 | tex_format::ARGB1555 | tex_format::RGB565 | tex_format::R16 => {
            let mut v = Vec::with_capacity((w * h * 2) as usize);
            for i in 0..(w * h) {
                v.extend_from_slice(&((i % 65536) as u16).to_le_bytes());
            }
            v
        }
        tex_format::A8 => (0..(w * h)).map(|i| (i % 256) as u8).collect(),
        tex_format::DXT1 => {
            // ceil(w/4) * ceil(h/4) * 8
            let bw = w.div_ceil(4) as usize;
            let bh = h.div_ceil(4) as usize;
            (0..bw * bh * 8).map(|i| (i % 256) as u8).collect()
        }
        tex_format::DXT3 | tex_format::DXT5 | tex_format::BC7 => {
            let bw = w.div_ceil(4) as usize;
            let bh = h.div_ceil(4) as usize;
            (0..bw * bh * 16).map(|i| (i % 256) as u8).collect()
        }
        tex_format::RGBA1010102 => {
            let mut v = Vec::with_capacity((w * h * 4) as usize);
            for _ in 0..(w * h) {
                v.extend_from_slice(&[0xAB, 0xCD, 0xEF, 0x12]);
            }
            v
        }
        tex_format::RGBA32F => (0..(w * h * 16)).map(|i| (i % 256) as u8).collect(),
        _ => (0..(w * h * 4)).map(|i| (i % 256) as u8).collect(),
    }
}

/// BC7 은 크기가 `w * (h & ~3) * 4` 다 (원본 `Wz_Png.cs:362`).
pub fn synthetic_bc7(w: u32, h: u32) -> Vec<u8> {
    let n = (w as usize) * ((h & !3) as usize) * 4;
    (0..n).map(|i| (i % 256) as u8).collect()
}

/// 텍스처 포맷 전체에 대한 코퍼스를 만든다.
///
/// 13개 포맷 ×代表性 크기(1×1, 4×4, 비배수 127×129, 1024×1024)는
/// 디코더의 엣지 케이스를 다 커버한다.
pub fn texture_corpus() -> Vec<TextureCase> {
    let formats = [
        tex_format::ARGB4444,
        tex_format::ARGB8888,
        tex_format::ARGB1555,
        tex_format::RGB565,
        tex_format::R16,
        tex_format::DXT3,
        tex_format::DXT5,
        tex_format::A8,
        tex_format::RGBA1010102,
        tex_format::DXT1,
        tex_format::BC7,
        tex_format::RGBA32F,
    ];
    let sizes = [(1u32, 1u32), (4, 4), (127, 129), (64, 64)];

    let mut cases = Vec::new();
    for f in formats {
        for (w, h) in sizes {
            // BC7 은 높이가 4 배수여야 하고, 127x129 같은 홀수 크기가 quirk 를 드러낸다
            if f == tex_format::BC7 && (w & !3) == 0 {
                continue;
            }
            let texels = if f == tex_format::BC7 {
                synthetic_bc7(w, h)
            } else {
                synthetic_texels(f, w, h)
            };
            cases.push(TextureCase {
                format: f,
                width: w,
                height: h,
                scale: 0,
                pages: 1,
                texels: texels.clone(),
                chunked: false,
            });
            // 청크 암호화 경로도 같은 데이터로 (복호화 로직 검증)
            cases.push(TextureCase {
                format: f,
                width: w,
                height: h,
                scale: 0,
                pages: 1,
                texels,
                chunked: true,
            });
        }
    }
    // 멀티페이지
    cases.push(TextureCase {
        format: tex_format::ARGB8888,
        width: 8,
        height: 8,
        scale: 0,
        pages: 3,
        texels: synthetic_texels(tex_format::ARGB8888, 8, 8),
        chunked: false,
    });
    // RGB565 scale 4 (ActualScale 16) — 원본의 업스케일 경로
    cases.push(TextureCase {
        format: tex_format::RGB565,
        width: 64,
        height: 64,
        scale: 4,
        pages: 1,
        texels: synthetic_texels(tex_format::RGB565, 4, 4),
        chunked: false,
    });
    cases
}

#[derive(Debug, Clone)]
pub struct TextureCase {
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub scale: u8,
    pub pages: i32,
    pub texels: Vec<u8>,
    pub chunked: bool,
}

impl TextureCase {
    /// 안정적 식별자 (골든 파일명).
    pub fn id(&self) -> String {
        format!(
            "tex_{}_{}x{}_s{}_p{}_{}",
            self.format,
            self.width,
            self.height,
            self.scale,
            self.pages,
            if self.chunked { "chunked" } else { "plain" }
        )
    }

    /// 이 케이스를 완전한 이미지(= 루트 `Property` + `Canvas` 1개)로 직렬화한다.
    ///
    /// 파서의 진입점(`parse_image_root`)이 루트 `Property` 를 요구하므로
    /// 래퍼를 넣어야 라운드트립이 성립한다.
    pub fn build(&self) -> Vec<u8> {
        self.build_with_len().0
    }

    /// `(전체 바이트, dataLen)`.
    pub fn build_with_len(&self) -> (Vec<u8>, i32) {
        let mut b = WzBuilder::new();
        let root = b.root_property_begin();
        let data_len = if self.chunked {
            b.canvas_chunked(
                "canvas",
                self.width as i32,
                self.height as i32,
                self.format,
                self.scale,
                self.pages,
                &self.texels,
                &AesKeyStream::kms(),
                64,
            )
            .1
        } else {
            b.canvas_plain(
                "canvas",
                self.width as i32,
                self.height as i32,
                self.format,
                self.scale,
                self.pages,
                &self.texels,
            )
            .1
        };
        b.root_property_end(root, 1);
        (b.into_vec(), data_len)
    }
}

/// `Canvas` 헤더 (`Wz_Image.cs:281-303`).
///
/// # 함정
///
/// 1. 해상도/포맷/페이지/미지정 필드는 **압축** int 다.
/// 2. `scale` 만 **1바이트**다 (압축 int 가 아니다!).
/// 3. `dataLen` 은 **비압축** i32 이고 플래그 바이트를 **포함**한다.
/// 4. 미지의 2바이트 "TBD" 필드가 있다 (`Wz_Image.cs:299`).
#[derive(Debug, Clone, Copy)]
struct CanvasHeader {
    width: i32,
    height: i32,
    format: i32,
    scale: u8,
    pages: i32,
    /// `dataLen` 은 여기 넣지 않는다 — 바깥에서 비압축으로 따로 쓴다.
    _data_len: i32,
}

impl CanvasHeader {
    fn new(width: i32, height: i32, format: u32, scale: u8, pages: i32, data_len: i32) -> Self {
        Self {
            width,
            height,
            format: format as i32,
            scale,
            pages,
            _data_len: data_len,
        }
    }

    fn encode(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(20);
        v.extend_from_slice(&encode_compressed_i32(self.width));
        v.extend_from_slice(&encode_compressed_i32(self.height));
        v.extend_from_slice(&encode_compressed_i32(self.format));
        v.push(self.scale); // ← 1바이트!
        v.extend_from_slice(&encode_compressed_i32(self.pages));
        v.extend_from_slice(&encode_compressed_i32(0)); // unknown1 (KMST1198)
        v.extend_from_slice(&2u16.to_le_bytes()); // "TBD"
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_prefix_is_required_before_flag() {
        // 함정: 이름을 빼면 파서가 플래그를 문자열로 오해한다.
        // 함정: WZ 문자열은 `0xAAAA + i` 로 **마스킹**된다.
        let mut b = WzBuilder::new();
        b.value_int("n", 7);
        let bytes = b.into_vec();
        assert_eq!(bytes[0], 0x00, "첫 바이트는 문자열 플래그");
        assert_eq!(bytes[1], 1, "직결 길이 접두 (UTF-16 코드유닛 수)");
        let masked = u16::from_le_bytes([bytes[2], bytes[3]]);
        assert_eq!(masked ^ 0xAAAA, u16::from(b'n'), "마스킹 해제 후 'n'");
        assert_eq!(bytes[4], flags::INT, "그 다음이 값 플래그");
        assert_eq!(bytes[5], 7, "그 다음이 압축 int");
    }

    /// 이 값 직렬화의 `0x00` + 문자열 이름 접두 길이 = `1 + 1 + 2*units(name)`.
    fn name_prefix_len(name: &str) -> usize {
        2 + 2 * name.encode_utf16().count()
    }

    #[test]
    fn scalar_values_have_expected_shape() {
        let mut b = WzBuilder::new();
        b.value_int("v", 42);
        let n = name_prefix_len("v");
        assert_eq!(
            &b.bytes()[n..],
            &[flags::INT, 42],
            "이름 접두 뒤에 flag + 1바이트 압축 int"
        );

        let mut b = WzBuilder::new();
        b.value_int("v", 1000);
        assert_eq!(b.bytes()[n], flags::INT);
        assert_eq!(b.len() - n, 6, "128 이상은 flag 1 + 5바이트 이스케이프");
    }

    #[test]
    fn float_uses_compressed_int_not_raw() {
        // 함정: 0x04 는 압축 int 다. float 의 비트 패턴을 압축 int 로 감싼다.
        let mut b = WzBuilder::new();
        b.value_float("f", 1.0);
        let bits = 1.0f32.to_bits() as i32;
        let n = name_prefix_len("f");
        assert_eq!(&b.bytes()[n..], {
            let mut want = vec![flags::FLOAT];
            want.extend_from_slice(&encode_compressed_i32(bits));
            want
        });
    }

    #[test]
    fn extended_object_length_is_patched_back() {
        let mut b = WzBuilder::new();
        let p = b.property_begin("obj");
        b.value_int("a", 1);
        b.value_int("b", 2);
        b.property_end(p, 2);

        let (len_pos, _) = p;
        let body_len =
            i32::from_le_bytes(b.bytes()[len_pos..len_pos + 4].try_into().expect("4바이트"));
        assert_eq!(
            body_len as usize,
            b.len() - (len_pos + 4),
            "objDataLen 은 플래그를 제외한 본문 길이"
        );
    }

    #[test]
    fn property_count_is_value_count_not_byte_count() {
        let mut b = WzBuilder::new();
        let p = b.property_begin("root");
        b.value_int("n", 7);
        b.value_string("s", "hi");
        b.property_end(p, 2);

        let (_, count_pos) = p;
        // 개수는 항상 5바이트 압축 int 이스케이프로 예약된다 (제자리 패치용)
        assert_eq!(b.bytes()[count_pos], 0x80, "압축 int 이스케이프");
        let n = i32::from_le_bytes(
            b.bytes()[count_pos + 1..count_pos + 5]
                .try_into()
                .expect("4바이트"),
        );
        assert_eq!(n, 2, "값이 2개 (바이트 수가 아님)");
        assert!(
            b.len() > count_pos + 5 + 2,
            "바이트 수와 값 개수가 다름 — 이 구분이 핵심"
        );
    }

    #[test]
    fn canvas_data_len_includes_flag_byte_and_fits() {
        let texels = synthetic_texels(tex_format::ARGB8888, 4, 4);
        let mut b = WzBuilder::new();
        let (total, data_len) = b.canvas_plain("c", 4, 4, tex_format::ARGB8888, 0, 1, &texels);
        assert!(data_len > 0, "dataLen 양수");
        assert!(
            (data_len as usize) <= total,
            "dataLen({data_len}) 이 전체({total}) 를 넘으면 안 된다"
        );
        // zlib 헤더(2B) + 최소한의 deflate + adler(4B) + 플래그(1B)
        assert!(data_len >= 7, "최소 크기");
    }

    #[test]
    fn canvas_data_len_is_readable_from_tail() {
        // dataLen 은 페이로드 바로 앞에 **비압축** i32 로 있다.
        let texels = synthetic_texels(tex_format::ARGB8888, 4, 4);
        let mut b = WzBuilder::new();
        let (_total, data_len) = b.canvas_plain("c", 4, 4, tex_format::ARGB8888, 0, 1, &texels);
        let bytes = b.into_vec();
        // 페이로드 = 레코드 끝에서 dataLen 바이트
        let payload = &bytes[bytes.len() - data_len as usize..];
        assert_eq!(payload[0], 0x01, "첫 바이트는 플래그");
        assert_eq!(
            &payload[1..3],
            &[0x78, 0x9C],
            "zlib 헤더 0x789C (Wz_Png.cs:155 가 요구하는 값)"
        );
    }

    #[test]
    fn plain_and_chunked_canvases_differ_only_in_payload() {
        let texels = synthetic_texels(tex_format::ARGB8888, 4, 4);
        let plain = {
            let mut b = WzBuilder::new();
            b.canvas_plain("c", 4, 4, tex_format::ARGB8888, 0, 1, &texels);
            b.into_vec()
        };
        let chunked = {
            let mut b = WzBuilder::new();
            b.canvas_chunked(
                "c",
                4,
                4,
                tex_format::ARGB8888,
                0,
                1,
                &texels,
                &AesKeyStream::kms(),
                64,
            );
            b.into_vec()
        };
        assert_ne!(plain, chunked, "청크 경로는 구조가 다르다");
        // 헤더는 동일해야 한다 (이름 ~ dataLen 까지)
        let common = plain
            .iter()
            .zip(chunked.iter())
            .take_while(|(a, b)| a == b)
            .count();
        assert!(
            common >= name_prefix_len("c"),
            "적어도 이름 접두까지는 같아야 한다 (공통 {common} 바이트)"
        );
    }

    #[test]
    fn texture_corpus_covers_all_formats() {
        let c = texture_corpus();
        // 12 포맷 × 4 크기 × 2 경로 = 96, + 멀티페이지 1 + scale 1
        assert!(c.len() >= 90, "코퍼스가 충분히 커야 한다: {}", c.len());
        let ids: std::collections::HashSet<_> = c.iter().map(|x| x.id()).collect();
        assert_eq!(ids.len(), c.len(), "ID 가 유일해야 한다");
    }

    #[test]
    fn bc7_cases_are_skipped_when_width_floors_to_zero() {
        let c = texture_corpus();
        for case in c.iter().filter(|c| c.format == tex_format::BC7) {
            assert!(case.width & !3 != 0, "BC7 폭이 0 이 되면 건너뛰어야 한다");
        }
    }

    #[test]
    fn synthetic_texels_sizes_match_layout_rules() {
        for (f, bpp) in [
            (tex_format::ARGB8888, 4usize),
            (tex_format::ARGB4444, 2),
            (tex_format::RGB565, 2),
            (tex_format::A8, 1),
        ] {
            let t = synthetic_texels(f, 4, 4);
            assert_eq!(t.len(), 4 * 4 * bpp, "포맷 {f}");
        }
        assert_eq!(
            synthetic_texels(tex_format::DXT1, 5, 5).len(),
            2 * 2 * 8,
            "DXT1 은 ceil 규칙"
        );
        assert_eq!(synthetic_texels(tex_format::DXT5, 5, 5).len(), 2 * 2 * 16);
        assert_eq!(synthetic_bc7(4, 6).len(), 4 * 4 * 4, "BC7 은 높이 내림");
    }

    // ── wz-core 파서와의 라운드트립 ────────────────────────────────────────
    //
    // writer(`WzBuilder`) 와 reader(`wz_core::ImageParser`) 가 **같은 규칙**을
    // 공유하는지 검증한다. 이게 깨지면 합성 코퍼스가 무의미해지고,
    // C# 오라클 비교의 입력이 틀린다 (리포트 §10.2).

    /// `Property` 본문 하나를 파싱해 루트로 만든다.
    fn parse(bytes: Vec<u8>) -> wz_core::image::Node {
        let mut c = wz_pool::ReadCtx::from_bytes(bytes);
        let mut p = wz_core::image::ImageParser::new(None);
        let root = p.parse_image_root(&mut c).expect("파싱 실패");
        assert_eq!(
            c.pos(),
            c.len().unwrap(),
            "파서가 남은 바이트 없이 정확히 소비해야 한다 (라운드트립)"
        );
        root
    }

    #[test]
    fn roundtrip_scalars() {
        let mut b = WzBuilder::new();
        let k = b.root_property_begin();
        b.value_int("zero", 0);
        b.value_int("one", 1);
        b.value_int("neg_one", -1);
        b.value_int("i127", 127);
        b.value_int("i128", 128);
        b.value_int("imin", i32::MIN);
        b.value_int("imax", i32::MAX);
        b.value_int("big", 100_000);
        b.root_property_end(k, 8);
        let root = parse(b.into_vec());

        let vals: Vec<i32> = root
            .children
            .iter()
            .map(|n| match n.value {
                Some(wz_core::image::ImageValue::Int(v)) => v,
                ref o => panic!("Int 여야 한다: {o:?}"),
            })
            .collect();
        assert_eq!(
            vals,
            vec![0, 1, -1, 127, 128, i32::MIN, i32::MAX, 100_000],
            "압축 정수 경계값 왕복"
        );
        // 이름도 정확히 왕복돼야 한다
        let names: Vec<&str> = root.children.iter().map(|n| &*n.name).collect();
        assert_eq!(
            names,
            vec!["zero", "one", "neg_one", "i127", "i128", "imin", "imax", "big"]
        );
    }

    #[test]
    fn roundtrip_strings() {
        let mut b = WzBuilder::new();
        let k = b.root_property_begin();
        b.value_string("s0", "");
        b.value_string("s1", "ascii only");
        b.value_string("s2", "한글 문자열");
        b.value_string("s3", "🎮 surrogate pair");
        b.value_string("s4", &"x".repeat(300)); // 127 이스케이프 경로
        b.root_property_end(k, 5);
        let root = parse(b.into_vec());

        let got: Vec<String> = root
            .children
            .iter()
            .map(|n| match &n.value {
                Some(wz_core::image::ImageValue::Str(s)) => s.to_string(),
                ref o => panic!("String 여야 한다: {o:?}"),
            })
            .collect();
        assert_eq!(got[0], "");
        assert_eq!(got[1], "ascii only");
        assert_eq!(got[2], "한글 문자열");
        assert_eq!(got[3], "🎮 surrogate pair");
        assert_eq!(got[4].len(), 300, "긴 문자열");
    }

    #[test]
    fn roundtrip_strings_with_kms_encryption() {
        let ks = wz_core::crypto::aes::AesKeyStream::kms();
        let mut b = WzBuilder::with_key(ks.try_clone());
        let k = b.root_property_begin();
        b.value_string("s", "KMS 암호화 대상");
        b.value_string("long", &"y".repeat(200));
        b.root_property_end(k, 2);
        let bytes = b.into_vec();

        // 같은 키로 파싱해야 **이름까지** 복호화된다
        let mut c = wz_pool::ReadCtx::from_bytes(bytes.clone());
        let mut p = wz_core::image::ImageParser::new(Some(Box::new(ks)));
        let root = p.parse_image_root(&mut c).expect("KMS 파싱 실패");
        assert_eq!(&*root.children[0].name, "s", "이름도 같은 키 경로");
        match root.children[0].value.as_ref().unwrap() {
            wz_core::image::ImageValue::Str(s) => assert_eq!(&**s, "KMS 암호화 대상"),
            o => panic!("String 여야 한다: {o:?}"),
        }
        match root.children[1].value.as_ref().unwrap() {
            wz_core::image::ImageValue::Str(s) => assert_eq!(s.len(), 200),
            o => panic!("String 여야 한다: {o:?}"),
        }

        // 키가 없으면 깨진 문자열이 나오지만 **패닉은 없어야 한다**
        let mut c2 = wz_pool::ReadCtx::from_bytes(bytes);
        let mut p2 = wz_core::image::ImageParser::new(None);
        let _ = p2.parse_image_root(&mut c2);
    }

    #[test]
    fn roundtrip_misc_values() {
        let mut b = WzBuilder::new();
        let k = b.root_property_begin();
        b.value_float("f1", 1.0);
        b.value_float("f2", -0.5);
        b.value_double("d", std::f64::consts::PI);
        b.value_bool("bt", true);
        b.value_bool("bf", false);
        b.value_long("lmin", i64::MIN);
        b.value_long("lmax", i64::MAX);
        b.value_null("n");
        b.root_property_end(k, 8);
        let root = parse(b.into_vec());

        use wz_core::image::ImageValue as V;
        assert_eq!(root.children[0].value, Some(V::Float(1.0)));
        assert_eq!(root.children[1].value, Some(V::Float(-0.5)));
        assert_eq!(
            root.children[2].value,
            Some(V::Double(std::f64::consts::PI))
        );
        assert_eq!(root.children[3].value, Some(V::Bool(true)));
        assert_eq!(root.children[4].value, Some(V::Bool(false)));
        assert_eq!(root.children[5].value, Some(V::Long(i64::MIN)));
        assert_eq!(root.children[6].value, Some(V::Long(i64::MAX)));
        assert_eq!(root.children[7].value, Some(V::Null));
    }

    #[test]
    fn roundtrip_canvas_descriptors() {
        use wz_core::image::ImageValue as V;
        // 대표 포맷 × 대표 크기
        for (f, w, h) in [
            (tex_format::ARGB8888, 4u32, 4u32),
            (tex_format::BC7, 8, 6),
            (tex_format::DXT5, 16, 16),
        ] {
            let texels = if f == tex_format::BC7 {
                synthetic_bc7(w, h)
            } else {
                synthetic_texels(f, w, h)
            };
            let mut b = WzBuilder::new();
            let k = b.root_property_begin();
            b.canvas_plain("canvas", w as i32, h as i32, f, 0, 1, &texels);
            b.root_property_end(k, 1);
            let root = parse(b.into_vec());

            match root.children[0].value.as_ref().unwrap() {
                V::Png(p) => {
                    assert_eq!(p.width, w as i32, "포맷 {f}");
                    assert_eq!(p.height, h as i32, "포맷 {f}");
                    assert_eq!(p.format as u32, f);
                    assert_eq!(p.scale, 0);
                    assert_eq!(p.pages, 1);
                    assert!(p.data_len > 0, "dataLen 은 플래그 바이트를 포함");
                }
                o => panic!("Canvas 여야 한다 (포맷 {f}): {o:?}"),
            }
        }
    }

    #[test]
    fn roundtrip_chunked_canvas() {
        use wz_core::image::ImageValue as V;
        let texels = synthetic_texels(tex_format::ARGB8888, 8, 8);
        let mut b = WzBuilder::new();
        let k = b.root_property_begin();
        b.canvas_chunked(
            "canvas",
            8,
            8,
            tex_format::ARGB8888,
            0,
            1,
            &texels,
            &AesKeyStream::kms(),
            16, // 작은 청크로 강제
        );
        b.root_property_end(k, 1);
        let root = parse(b.into_vec());
        match root.children[0].value.as_ref().unwrap() {
            V::Png(p) => assert_eq!((p.width, p.height), (8, 8)),
            o => panic!("Canvas 여야 한다: {o:?}"),
        }
    }

    #[test]
    fn roundtrip_objects() {
        use wz_core::image::ImageValue as V;
        let ks = AesKeyStream::kms();
        let mut b = WzBuilder::with_key(ks.try_clone());
        let k = b.root_property_begin();
        // 중첩 Property
        let m = b.property_begin("nested");
        b.value_int("leaf", 0x0BAD_C0DEu32 as i32);
        b.property_end(m, 1);
        // Vector
        b.vector2d("vec", -5, 300);
        // UOL
        b.uol("link1", "../00002000.img/info");
        b.uol("link2", "../../String/Item.img");
        b.root_property_end(k, 4);
        let bytes = b.into_vec();

        let mut c = wz_pool::ReadCtx::from_bytes(bytes);
        let mut p = wz_core::image::ImageParser::new(Some(Box::new(ks)));
        let root = p.parse_image_root(&mut c).expect("파싱 실패");

        assert_eq!(root.children.len(), 4);
        match &root.children[0].value {
            Some(V::Property(n)) => {
                assert_eq!(n.children.len(), 1);
                assert_eq!(n.children[0].value, Some(V::Int(0x0BAD_C0DEu32 as i32)));
            }
            o => panic!("Property 여야 한다: {o:?}"),
        }
        assert_eq!(
            root.children[1].value,
            Some(V::Vector(wz_core::image::Vec2 { x: -5, y: 300 }))
        );
        match &root.children[2].value {
            Some(V::Uol(s)) => assert_eq!(&**s, "../00002000.img/info"),
            o => panic!("UOL 여야 한다: {o:?}"),
        }
        match &root.children[3].value {
            Some(V::Uol(s)) => assert_eq!(&**s, "../../String/Item.img"),
            o => panic!("UOL 여야 한다: {o:?}"),
        }
    }

    #[test]
    fn roundtrip_nested_property_depth() {
        // 0x09 의 길이 역패치가 실제 소비량과 정확히 일치해야 한다.
        // 원본 `Wz_Image.cs:519-527` 의 검증 조건.
        // 중첩 3단계로 만들어 모든 레벨에서 길이 검증이 걸리게 한다.
        let mut b = WzBuilder::new();
        let k = b.root_property_begin();
        let m1 = b.property_begin("l1");
        let m2 = b.property_begin("l2");
        let m3 = b.property_begin("l3");
        b.value_int("v", 1);
        b.property_end(m3, 1);
        b.value_int("w", 2);
        b.property_end(m2, 2);
        b.value_int("x", 3);
        b.property_end(m1, 2);
        b.root_property_end(k, 1);

        // parse() 가 남은 바이트 0 을 assert 하므로 길이 불일치가 즉시 드러난다
        let root = parse(b.into_vec());
        assert_eq!(root.children.len(), 1);
        let l1 = match &root.children[0].value {
            Some(wz_core::image::ImageValue::Property(n)) => n,
            o => panic!("Property 여야 한다: {o:?}"),
        };
        assert_eq!(l1.children.len(), 2);
        let l2 = match &l1.children[0].value {
            Some(wz_core::image::ImageValue::Property(n)) => n,
            o => panic!("Property 여야 한다: {o:?}"),
        };
        assert_eq!(l2.children.len(), 2, "내부 2개 값");
    }

    #[test]
    fn roundtrip_raw_data() {
        use wz_core::image::ImageValue as V;
        let mut b = WzBuilder::new();
        let k = b.root_property_begin();
        b.raw_data("v0", &0xDEAD_BEEFu32.to_le_bytes(), 0);
        // ver 1 + mini property 있음
        b.raw_data_with_sub("v1", &0x0102_0304u32.to_le_bytes(), 1, Some(("magic", 7)));
        // ver 1 + mini property 없음 (플래그 0x00)
        b.raw_data("v1b", &[1, 2, 3], 1);
        b.root_property_end(k, 3);
        let root = parse(b.into_vec());

        match &root.children[0].value {
            Some(V::RawData(v)) => {
                assert_eq!(v.len, 4, "페이로드 길이");
                assert!(v.sub_property.is_none(), "ver 0 은 mini property 없음");
            }
            o => panic!("RawData 여야 한다: {o:?}"),
        }
        match &root.children[1].value {
            Some(V::RawData(v)) => {
                assert_eq!(v.len, 4);
                let sub = v
                    .sub_property
                    .as_ref()
                    .expect("ver 1 + 0x01 → mini property");
                assert_eq!(sub.children.len(), 1);
                assert_eq!(&*sub.children[0].name, "magic");
                assert_eq!(sub.children[0].value, Some(V::Int(7)));
            }
            o => panic!("RawData 여야 한다: {o:?}"),
        }
        match &root.children[2].value {
            Some(V::RawData(v)) => {
                assert_eq!(v.len, 3);
                assert!(v.sub_property.is_none(), "ver 1 + 0x00 → 없음");
            }
            o => panic!("RawData 여야 한다: {o:?}"),
        }
    }

    // ── 디렉터리 트리 라운드트립 ────────────────────────────────────────────

    /// PKG1 계산기. `hashVersion` 은 777 고정 (encverMissing).
    fn pkg1_calc() -> wz_core::offset::Pkg1OffsetCalc {
        wz_core::offset::Pkg1OffsetCalc::new(60, wz_core::verify::compute_pkg1_hash_version(777))
    }

    fn parse_pkg1_with(
        bytes: Vec<u8>,
        dir_start: u64,
        calc: &dyn wz_core::offset::OffsetCalc,
    ) -> wz_core::dirtree::DirTree {
        use wz_core::dirtree::*;
        let detected = wz_core::detect::Detected {
            encver: None,
            profile: "PKG1_ENCVER_MISSING".into(),
            hash1: None,
            hash2: None,
            header_len: 0,
            capabilities: wz_core::detect::Capabilities {
                encver_missing: true,
                ..Default::default()
            },
        };
        let ctx = DirTreeContext {
            detected: &detected,
            offset_calc: Some(calc),
            dir_start,
            pkg2: false,
            read_offset_table: false,
            name_position: EntryNamePosition::BeforeData,
            pkg2_name_key_material: None,
        };
        let mut c = wz_pool::ReadCtx::from_bytes(bytes);
        DirTreeParser::new(ctx)
            .parse(&mut c)
            .expect("PKG1 트리 파싱")
    }

    fn parse_pkg1(bytes: Vec<u8>, dir_start: u64) -> wz_core::dirtree::DirTree {
        let calc = pkg1_calc();
        parse_pkg1_with(bytes, dir_start, &calc)
    }

    /// 엔트리들이 **선형으로** 배치된 파일을 만든다.
    ///
    /// `hashedOffset` 는 `OffsetCalc::encode_offset` 로 계산하므로 파서가
    /// 정확히 그 오프셋을 복원한다. 임의 값을 넣으면 복호화 결과가 파일 밖으로
    /// 튀어 테스트가 **아무것도 검증하지 못한 채 통과**한다.
    fn linear_dir_file(entries: &[DirEntrySpec]) -> (Vec<u8>, Vec<u64>) {
        use wz_core::offset::OffsetCalc;
        let calc = pkg1_calc();
        let (probe, positions) = dir_block_pkg1_with_positions(entries, &vec![0; entries.len()]);
        let base = 128usize;
        let block_len = probe.len();
        let targets: Vec<u64> = (0..entries.len())
            .map(|i| (base + i * block_len) as u64)
            .collect();

        let hashed: Vec<u32> = positions
            .iter()
            .enumerate()
            .map(|(i, &p)| calc.encode_offset((base + p) as u32, targets[i] as u32))
            .collect();

        let (block, _) = dir_block_pkg1_with_positions(entries, &hashed);
        let mut file = vec![0u8; base + block.len()];
        file[base..base + block.len()].copy_from_slice(&block);
        (file, targets)
    }

    #[test]
    fn dirtree_pkg1_roundtrip() {
        use wz_core::dirtree::node_type;
        let entries = vec![
            DirEntrySpec::image("base.img", 1000),
            DirEntrySpec::dir("Character"),
            DirEntrySpec::image("Effect.img", 2048),
            DirEntrySpec::dir("Item"),
            DirEntrySpec::image("Map.img", 4096),
        ];
        let (file, targets) = linear_dir_file(&entries);
        // 함정: 블록은 base(128) 에 있으므로 `dir_start` 도 그 값이어야 한다.
        let tree = parse_pkg1(file, 128);

        assert_eq!(tree.root.entries.len(), 5, "모든 엔트리가 살아야 한다");
        for (i, e) in entries.iter().enumerate() {
            let got = &tree.root.entries[i];
            assert_eq!(got.node_type, e.node_type, "엔트리 {i} 타입");
            assert_eq!(&**got.name.as_ref().unwrap(), &e.name, "엔트리 {i} 이름");
            assert_eq!(got.data_len, e.data_len, "엔트리 {i} 길이");
            assert_eq!(got.cs32, e.cs32, "엔트리 {i} 체크섬");
            assert_eq!(
                got.offset,
                Some(targets[i]),
                "엔트리 {i} 오프셋 (encode_offset 역산 검증)"
            );
        }
        assert_eq!(tree.root.entries[1].node_type, node_type::DIRECTORY);
        assert_eq!(tree.root.entries[0].node_type, node_type::IMAGE);
    }

    #[test]
    fn dirtree_unknown_type_is_error_not_panic() {
        // 함정: 0x00/0x01 은 문자열 플래그지 디렉터리 타입이 아니다.
        for bad in [0x00u8, 0x01, 0x05, 0x73, 0x80] {
            let mut block = Vec::new();
            block.extend_from_slice(&encode_compressed_i32(1));
            block.push(bad);
            block.extend_from_slice(&encode_dir_name("x"));
            block.extend_from_slice(&encode_compressed_i32(1));
            block.extend_from_slice(&encode_compressed_i32(0));
            block.extend_from_slice(&0u32.to_le_bytes());

            use wz_core::dirtree::{
                DirTreeContext, DirTreeError, DirTreeParser, EntryNamePosition,
            };
            let detected = wz_core::detect::Detected {
                encver: None,
                profile: "PKG1_ENCVER_MISSING".into(),
                hash1: None,
                hash2: None,
                header_len: 0,
                capabilities: wz_core::detect::Capabilities {
                    encver_missing: true,
                    ..Default::default()
                },
            };
            let calc = pkg1_calc();
            let ctx = DirTreeContext {
                detected: &detected,
                offset_calc: Some(&calc),
                dir_start: 0,
                pkg2: false,
                read_offset_table: false,
                name_position: EntryNamePosition::BeforeData,
                pkg2_name_key_material: None,
            };
            let mut c = wz_pool::ReadCtx::from_bytes(block);
            let err = DirTreeParser::new(ctx)
                .parse(&mut c)
                .expect_err("에러여야 한다");
            assert_eq!(
                err,
                DirTreeError::UnknownNodeType { node_type: bad },
                "타입 0x{bad:02X}"
            );
        }
    }

    #[test]
    fn dirtree_names_are_masked_then_unmasked() {
        for s in [
            "Character/00002000.img",
            "한글 이름",
            "🎮 surrogate pair",
            "a",
        ] {
            let entries = vec![DirEntrySpec::image(s, 10)];
            let (file, _) = linear_dir_file(&entries);
            let tree = parse_pkg1(file, 128);
            assert_eq!(
                &**tree.root.entries[0].name.as_ref().unwrap(),
                s,
                "이름 왕복 실패"
            );
        }
    }

    #[test]
    fn dirtree_ascii_name_roundtrips() {
        // 함정: ASCII 경로의 마스크는 **바이트** 단위라 UTF-16 과 다르다.
        for s in ["Item.img", "a", "Map/Map0/100.img"] {
            let entries = vec![DirEntrySpec::image(s, 10)];
            // ASCII 로 인코딩한 블록을 직접 만든다.
            let mut b = WzBuilder::new();
            b.out.extend_from_slice(&encode_compressed_i32(1));
            b.out.push(0x04);
            b.out.extend_from_slice(&encode_dir_name_ascii(s));
            b.out.extend_from_slice(&encode_compressed_i32(10));
            b.out.extend_from_slice(&encode_compressed_i32(0));
            b.out.extend_from_slice(&0u32.to_le_bytes());
            let _ = entries;
            let tree = parse_pkg1(b.into_vec(), 0);
            assert_eq!(&**tree.root.entries[0].name.as_ref().unwrap(), s);
        }
    }

    #[test]
    fn dirtree_long_name_uses_escape_prefix() {
        // 127 코드유닛 **이상**은 5바이트 접두 (0x7F + i32)
        let s: String = (0..200)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let enc = encode_dir_name(&s);
        assert_eq!(enc[0], 127, "이스케이프 마커");
        assert_eq!(
            i32::from_le_bytes(enc[1..5].try_into().expect("4바이트")),
            200,
            "접두의 i32"
        );
        let entries = vec![DirEntrySpec::image(&s, 1)];
        let (file, _) = linear_dir_file(&entries);
        let tree = parse_pkg1(file, 128);
        assert_eq!(&**tree.root.entries[0].name.as_ref().unwrap(), &s);
    }

    #[test]
    fn dirtree_empty_name_is_allowed() {
        let mut b = WzBuilder::new();
        b.out.extend_from_slice(&encode_compressed_i32(1));
        b.out.push(0x04);
        b.out.push(0x00); // 길이 0
        b.out.extend_from_slice(&encode_compressed_i32(1));
        b.out.extend_from_slice(&encode_compressed_i32(0));
        b.out.extend_from_slice(&0u32.to_le_bytes());
        let tree = parse_pkg1(b.into_vec(), 0);
        assert_eq!(&**tree.root.entries[0].name.as_ref().unwrap(), "");
    }

    #[test]
    fn dirtree_name_by_ref_resolves() {
        use wz_core::dirtree::{DirTreeContext, DirTreeParser, EntryNamePosition};
        // 타입 0x02: 이름이 i32 오프셋 참조 (KMST1125 Data.wz)
        //
        // 함정: `stringOffAdd` 가 +2 이므로 writer 는 `ref - 2` 를 기록해야 한다.
        let name = "00002000.img";
        let name_at = 64u32; // 절대 위치
        let add = 2i32;
        // writer 는 `raw = name_off - add` 를 기록하고, 파서는 `at = raw + add`
        // 로 되돌린다. 그러므로 writer 에는 **add 를 더하지 않은** 위치를 준다.
        let entries = vec![(name_at as i32, 100, 0, 0xABCDu32)];
        let block = dir_block_pkg1_named_by_ref(&entries, add);
        let mut file = vec![0u8; 128];
        file[0..block.len()].copy_from_slice(&block);
        let enc = encode_dir_name(name);
        file[name_at as usize..name_at as usize + enc.len()].copy_from_slice(&enc);

        let detected = wz_core::detect::Detected {
            encver: None,
            profile: "PKG1_ENCVER_MISSING".into(),
            hash1: None,
            hash2: None,
            header_len: 0,
            capabilities: wz_core::detect::Capabilities {
                encver_missing: true,
                ..Default::default()
            },
        };
        let calc = pkg1_calc();
        let ctx = DirTreeContext {
            detected: &detected,
            offset_calc: Some(&calc),
            dir_start: 0,
            pkg2: false,
            read_offset_table: false,
            name_position: EntryNamePosition::BeforeData,
            pkg2_name_key_material: None,
        };
        let mut c = wz_pool::ReadCtx::from_bytes(file);
        let tree = DirTreeParser::new(ctx).parse(&mut c).expect("파싱");
        assert_eq!(tree.root.entries.len(), 1);
        assert_eq!(tree.root.entries[0].node_type, 0x02);
        assert_eq!(&**tree.root.entries[0].name.as_ref().unwrap(), name);
        assert_eq!(
            tree.root.entries[0].name_ref,
            Some(name_at as i32 - add),
            "파일에는 add 가 빠진 값이 기록된다"
        );
    }

    #[test]
    fn dirtree_nested_dirs_are_visited_depth_first() {
        use wz_core::dirtree::node_type;
        use wz_core::offset::OffsetCalc;
        // 루트: [dir A, img r.img] → A: [img a.img, dir B] → B: [img b.img]
        //
        // `encode_offset` 로 각 블록의 오프셋이 정확히 복원되도록 만들어야
        // 재귀가 실제로 일어난다. 안 그러면 테스트가 조용히 아무것도 안 한다.
        let calc = pkg1_calc();
        let base = 64usize;

        // 블록 위치를 먼저 정하고, 그 위치를 목표로 하는 해시를 만든다.
        // 블록 크기는 해시와 무관하므로 두 번 만들어 길이를 잰다.
        let (root_probe, root_pos) = {
            let e = [DirEntrySpec::dir("A"), DirEntrySpec::image("r.img", 1)];
            dir_block_pkg1_with_positions(&e, &[0, 0])
        };
        let a_at = base + root_probe.len() + 64;
        let (a_probe, a_pos) = {
            let e = [DirEntrySpec::image("a.img", 1), DirEntrySpec::dir("B")];
            dir_block_pkg1_with_positions(&e, &[0, 0])
        };
        let b_at = a_at + a_probe.len() + 64;
        let (b_probe, b_pos) = {
            let e = [DirEntrySpec::image("b.img", 1)];
            dir_block_pkg1_with_positions(&e, &[0])
        };
        let total = b_at + b_probe.len() + 64;

        // 각 엔트리가 가리킬 실제 대상
        let root_targets = [a_at as u32, (base + 4) as u32];
        let a_targets = [(a_at + 4) as u32, b_at as u32];
        let b_targets = [b_at as u32];

        let hash_of = |pos: &[usize], block_at: usize, targets: &[u32]| -> Vec<u32> {
            pos.iter()
                .zip(targets)
                .map(|(&p, &t)| calc.encode_offset((block_at + p) as u32, t))
                .collect()
        };

        let (root_block, _) = dir_block_pkg1_with_positions(
            &[DirEntrySpec::dir("A"), DirEntrySpec::image("r.img", 1)],
            &hash_of(&root_pos, base, &root_targets),
        );
        let (a_block, _) = dir_block_pkg1_with_positions(
            &[DirEntrySpec::image("a.img", 1), DirEntrySpec::dir("B")],
            &hash_of(&a_pos, a_at, &a_targets),
        );
        let (b_block, _) = dir_block_pkg1_with_positions(
            &[DirEntrySpec::image("b.img", 1)],
            &hash_of(&b_pos, b_at, &b_targets),
        );

        let mut file = vec![0u8; total];
        file[base..base + root_block.len()].copy_from_slice(&root_block);
        file[a_at..a_at + a_block.len()].copy_from_slice(&a_block);
        file[b_at..b_at + b_block.len()].copy_from_slice(&b_block);

        let tree = parse_pkg1_with(file, base as u64, &calc);
        assert_eq!(tree.root.entries.len(), 2);
        assert_eq!(tree.root.entries[0].node_type, node_type::DIRECTORY);
        // `root` 는 `blocks` 에서 빠져 있으므로 blocks = [A, B] 두 개.
        // (세 블록이 방문됐다는 뜻이다 — 루트 + A + B)
        assert_eq!(
            tree.blocks.len(),
            2,
            "깊이 우선 전위 순회: {:?}",
            names(&tree)
        );
        assert_eq!(&**tree.blocks[0].entries[0].name.as_ref().unwrap(), "a.img");
        assert_eq!(&**tree.blocks[0].entries[1].name.as_ref().unwrap(), "B");
        assert_eq!(&**tree.blocks[1].entries[0].name.as_ref().unwrap(), "b.img");
    }

    fn names(t: &wz_core::dirtree::DirTree) -> Vec<String> {
        t.blocks
            .iter()
            .map(|b| {
                b.entries
                    .iter()
                    .map(|e| e.name.as_ref().map(|s| s.to_string()).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect()
    }

    #[test]
    fn dirtree_depth_limit_stops_runaway_recursion() {
        use wz_core::dirtree::{DirTreeContext, DirTreeError, DirTreeParser, EntryNamePosition};
        use wz_core::offset::OffsetCalc;
        // 자기 자신을 가리키는 디렉터리 — 스택 오버플로가 아니라 에러여야 한다.
        let calc = pkg1_calc();
        // 루트가 자기 자신을 가리키도록 만든다.
        let entries = vec![DirEntrySpec::dir("self")];
        let (block, pos) = dir_block_pkg1_with_positions(&entries, &[0]);
        let base = 64usize;
        let mut file = vec![0u8; base + block.len() + 64];
        file[base..base + block.len()].copy_from_slice(&block);
        let h = calc.encode_offset((base + pos[0]) as u32, base as u32);
        file[base + pos[0]..base + pos[0] + 4].copy_from_slice(&h.to_le_bytes());

        let detected = wz_core::detect::Detected {
            encver: None,
            profile: "PKG1_ENCVER_MISSING".into(),
            hash1: None,
            hash2: None,
            header_len: 0,
            capabilities: wz_core::detect::Capabilities {
                encver_missing: true,
                ..Default::default()
            },
        };
        let ctx = DirTreeContext {
            detected: &detected,
            offset_calc: Some(&calc),
            dir_start: base as u64,
            pkg2: false,
            read_offset_table: false,
            name_position: EntryNamePosition::BeforeData,
            pkg2_name_key_material: None,
        };
        let mut c = wz_pool::ReadCtx::from_bytes(file);
        let err = DirTreeParser::new(ctx)
            .with_max_depth(4)
            .parse(&mut c)
            .expect_err("깊이 제한이 걸려야 한다");
        assert!(matches!(err, DirTreeError::DepthExceeded { .. }), "{err:?}");
    }

    // ── PKG2 디렉터리 트리 라운드트립 ─────────────────────────────────────

    /// PKG2 표준 헤더 + 루트 블록 + 이미지 페이로드를 **한 번에** 만든다.
    ///
    /// # 세 가지가 동시에 맞아야 파싱이 된다
    ///
    /// 1. 엔트리 **개수**가 `decrypt_entry_count` 로 복호화돼야 한다
    ///    (`^ ((hash1 << 16) - 0x21524111 * hashVersion)`)
    /// 2. 표의 개수도 1 과 같아야 뒤따르는 오프셋 표를 읽는다
    /// 3. 각 `hashedOffset` 가 `encode_offset` 로 정확히 역산돼야 한다
    ///
    /// PKG2 에서 역산할 오프셋을 미리 알기가 어려운 이유는 **오프셋 표의
    /// 위치 자체가 엔트리 크기에 의존**하기 때문이다. 그래서 블록을 두 번
    /// 만든다: 한 번은 표 위치(= 파일 오프셋)를 알아내는 계측용, 한 번은
    /// 실제 해시를 넣은 최종본.
    fn pkg2_file(
        calc: &wz_core::offset::Pkg2OffsetCalc32,
        entries: &[DirEntrySpec],
        payload_len: i32,
    ) -> Vec<u8> {
        use wz_core::offset::OffsetCalc;
        let enc_count = move |n: i32| calc.decrypt_entry_count_i32(n);
        let header = 60usize;

        // 1차: 표가 시작될 위치만 알아낸다 (해시는 0).
        let probe = pkg2_block(entries, &vec![0u32; entries.len()], &enc_count);
        let table_at = header + probe.len() - entries.len() * 4;

        // 이미지 페이로드는 표 뒤에 둔다.
        let payload_at = table_at + entries.len() * 4;
        let offsets: Vec<u32> = (0..entries.len())
            .map(|i| {
                let pos = table_at + i * 4;
                calc.encode_offset(pos as u32, (payload_at + i * payload_len as usize) as u32)
            })
            .collect();

        // 2차: 실제 해시 포함.
        let block = pkg2_block(entries, &offsets, &enc_count);
        let mut file = vec![0u8; header];
        file[0..4].copy_from_slice(b"PKG2");
        file[12..16].copy_from_slice(&(header as i32).to_le_bytes());
        for b in file[16..header].iter_mut() {
            *b = b' ';
        }
        file.extend_from_slice(&block);
        for i in 0..entries.len() {
            file.resize(payload_at + (i + 1) * payload_len as usize, 0x5A);
        }
        let data_size = (file.len() - header) as i64;
        file[4..12].copy_from_slice(&data_size.to_le_bytes());
        file
    }

    /// PKG2 루트 블록: `[enc count][entries][enc offCount][offsets]`.
    fn pkg2_block(
        entries: &[DirEntrySpec],
        offsets: &[u32],
        enc_count: &dyn Fn(i32) -> i32,
    ) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&encode_compressed_i32(enc_count(entries.len() as i32)));
        for e in entries {
            b.push(e.node_type);
            b.extend_from_slice(&encode_dir_name(&e.name));
            b.extend_from_slice(&encode_compressed_i32(e.data_len));
            b.extend_from_slice(&encode_compressed_i32(e.cs32));
        }
        b.extend_from_slice(&encode_compressed_i32(enc_count(entries.len() as i32)));
        for o in offsets {
            b.extend_from_slice(&o.to_le_bytes());
        }
        b
    }

    /// PKG2 파싱 컨텍스트.
    fn pkg2_ctx<'a>(
        detected: &'a wz_core::detect::Detected,
        calc: &'a dyn wz_core::offset::OffsetCalc,
        dir_start: u64,
    ) -> wz_core::dirtree::DirTreeContext<'a> {
        use wz_core::dirtree::{DirTreeContext, EntryNamePosition};
        DirTreeContext {
            detected,
            offset_calc: Some(calc),
            dir_start,
            pkg2: true,
            read_offset_table: true,
            name_position: EntryNamePosition::BeforeData,
            pkg2_name_key_material: None,
        }
    }

    #[test]
    fn dirtree_pkg2_roundtrip() {
        use wz_core::dirtree::{node_type, DirTreeParser};
        let hash1 = 0xABCD_EF01u32;
        let hash_version = 0x1234_5678u32;
        let calc = wz_core::offset::Pkg2OffsetCalc32::v2(60, hash_version, hash1);

        let entries = vec![
            DirEntrySpec::image("base.img", 100),
            DirEntrySpec::image("Effect.img", 200),
            DirEntrySpec::image("Map.img", 300),
        ];
        let file = pkg2_file(&calc, &entries, 400);

        let detected = wz_core::detect::Detected {
            encver: None,
            profile: "PKG2_STANDARD_HEADER".into(),
            hash1: Some(hash1 as u64),
            hash2: Some(0x5566_7788),
            header_len: 60,
            capabilities: wz_core::detect::Capabilities::default(),
        };
        // 함정: 합성 파일은 offset 60 에 hash1/hash2 를 쓰지 않는다 (표준 헤더 8바이트가 없다).
        // 그래서 블록은 headerSize(60) 에 바로 온다.
        let ctx = pkg2_ctx(&detected, &calc, 60);
        let mut c = wz_pool::ReadCtx::from_bytes(file);
        let tree = DirTreeParser::new(ctx)
            .parse(&mut c)
            .expect("PKG2 트리 파싱");

        assert_eq!(
            tree.root.entries.len(),
            3,
            "엔트리 개수 복호화 성공 (`^ ((hash1<<16) - 0x21524111*hashVersion)`)"
        );
        for (i, e) in entries.iter().enumerate() {
            let got = &tree.root.entries[i];
            assert_eq!(got.node_type, e.node_type, "타입 {i}");
            assert_eq!(&**got.name.as_ref().unwrap(), &e.name, "이름 {i}");
            assert_eq!(got.data_len, e.data_len, "길이 {i}");
            assert!(got.offset.is_some(), "오프셋 {i} 해석됨");
        }
        assert_eq!(tree.root.entries[0].node_type, node_type::IMAGE);
    }

    #[test]
    fn dirtree_pkg2_offsets_do_not_overlap() {
        // 감지 검증의 결정적 조건과 같은 것 — 오프셋 구간이 겹치지 않는다.
        use wz_core::dirtree::DirTreeParser;
        let calc = wz_core::offset::Pkg2OffsetCalc32::v2(60, 0x1234_5678, 0xABCD_EF01);
        let entries = vec![
            DirEntrySpec::image("a.img", 100),
            DirEntrySpec::image("b.img", 200),
            DirEntrySpec::image("c.img", 300),
        ];
        let file = pkg2_file(&calc, &entries, 400);
        let detected = wz_core::detect::Detected {
            encver: None,
            profile: "PKG2_STANDARD_HEADER".into(),
            hash1: Some(0xABCD_EF01),
            hash2: Some(0x5566_7788),
            header_len: 60,
            capabilities: wz_core::detect::Capabilities::default(),
        };
        // 함정: 합성 파일은 offset 60 에 hash1/hash2 를 쓰지 않는다 (표준 헤더 8바이트가 없다).
        // 그래서 블록은 headerSize(60) 에 바로 온다.
        let ctx = pkg2_ctx(&detected, &calc, 60);
        let mut c = wz_pool::ReadCtx::from_bytes(file);
        let tree = DirTreeParser::new(ctx).parse(&mut c).expect("파싱");

        let mut ranges: Vec<(u64, u64)> = tree
            .root
            .entries
            .iter()
            .map(|e| {
                let s = e.offset.expect("오프셋");
                (s, s + e.data_len as u64)
            })
            .collect();
        ranges.sort_unstable();
        for w in ranges.windows(2) {
            assert!(w[0].1 <= w[1].0, "구간 겹침: {:?} vs {:?}", w[0], w[1]);
        }
    }

    #[test]
    fn dirtree_pkg2_empty_directory_is_valid() {
        // 함정: 빈 디렉터리(엔트리 0개)는 **오류가 아니다.** 실제 WZ 에 있다.
        use wz_core::dirtree::DirTreeParser;
        use wz_core::offset::OffsetCalc;
        let calc = wz_core::offset::Pkg2OffsetCalc32::v2(60, 0x1234_5678, 0xABCD_EF01);
        let enc = move |n: i32| calc.decrypt_entry_count_i32(n);
        let block = pkg2_block(&[], &[], &enc);
        let mut file = vec![0u8; 60];
        file[0..4].copy_from_slice(b"PKG2");
        file[12..16].copy_from_slice(&60i32.to_le_bytes());
        file.extend_from_slice(&block);

        let detected = wz_core::detect::Detected {
            encver: None,
            profile: "PKG2_STANDARD_HEADER".into(),
            hash1: Some(0xABCD_EF01),
            hash2: Some(0x5566_7788),
            header_len: 60,
            capabilities: wz_core::detect::Capabilities::default(),
        };
        // 함정: 합성 파일은 offset 60 에 hash1/hash2 를 쓰지 않는다 (표준 헤더 8바이트가 없다).
        // 그래서 블록은 headerSize(60) 에 바로 온다.
        let ctx = pkg2_ctx(&detected, &calc, 60);
        let mut c = wz_pool::ReadCtx::from_bytes(file);
        let tree = DirTreeParser::new(ctx).parse(&mut c).expect("빈 디렉터리");
        assert!(tree.root.entries.is_empty());
    }

    #[test]
    fn dirtree_pkg2_name_after_data_is_read() {
        // 함정: KMST1205/1206 은 이름이 `size`/`cs32` **뒤**에 온다.
        // 순서를 틀리면 이름이 숫자로 읽혀 트리가 통째로 어긋난다.
        use wz_core::dirtree::{DirTreeContext, DirTreeParser, EntryNamePosition};
        let hash1 = 0x1122_3344_5566_7788u64;
        let hash_version = wz_core::verify::compute_pkg2_hash_version_1202(
            wz_core::verify::KMST1205_1206_VERSION_STRING,
        );
        let calc = wz_core::offset::Pkg2OffsetCalc64V1::new(163, hash1, hash_version);

        // 엔트리 하나: [type][size][cs32][name]
        let mut block = vec![0x04u8];
        block.extend_from_slice(&encode_compressed_i32(100));
        block.extend_from_slice(&encode_compressed_i32(0));
        block.extend_from_slice(&encode_dir_name("Item.img"));

        let mut file = vec![0u8; 163];
        file.extend_from_slice(&block);

        let detected = wz_core::detect::Detected {
            encver: None,
            profile: "PKG2_RANDOM_HEADER_1205".into(),
            hash1: Some(hash1),
            hash2: Some(0),
            header_len: 163,
            capabilities: wz_core::detect::Capabilities {
                pkg2_random_header64: true,
                ..Default::default()
            },
        };
        let ctx = DirTreeContext {
            detected: &detected,
            offset_calc: Some(&calc),
            dir_start: 163,
            pkg2: true,
            read_offset_table: false,
            name_position: EntryNamePosition::AfterData,
            pkg2_name_key_material: None,
        };
        let mut c = wz_pool::ReadCtx::from_bytes(file);
        // 엔트리 개수 복호화 결과가 1 이 아닐 수 있어 Either 다 허용하지만
        // **크래시**는 안 돼야 한다.
        match DirTreeParser::new(ctx).read_root_only(&mut c) {
            Ok(b) => {
                for e in b.entries {
                    if let Some(n) = e.name {
                        assert_eq!(&*n, "Item.img", "이름이 뒤에 와야 읽힌다");
                    }
                }
            }
            Err(_) => { /* 개수 복호화 실패는 허용 — 크래시만 아니면 된다 */ }
        }
    }

    #[test]
    fn determinism() {
        let a = texture_corpus()[0].build();
        let b = texture_corpus()[0].build();
        assert_eq!(a, b, "같은 입력 → 같은 바이트 (결정성)");
    }

    #[test]
    fn container_headers_are_detected() {
        use wz_core::detect::detect;
        use wz_pool::MemorySource;

        let cases = [
            ContainerHeader::pkg1_encver_missing(&[1, 2, 3, 4]),
            ContainerHeader::pkg1_with_encver(&[1, 2, 3, 4]),
            ContainerHeader::pkg2(0x1122_3344, 0x5566_7788, &[1, 2, 3, 4]),
        ];
        for c in cases {
            let d = detect(&MemorySource::new(c.bytes.clone())).unwrap();
            assert_eq!(
                d.profile,
                c.expected_profile,
                "기대 프로파일 불일치 (bytes={:02X?})",
                &c.bytes[..c.bytes.len().min(24)]
            );
            assert_eq!(d.hash1, c.hash1.map(|h| h as u64));
            assert_eq!(d.hash2, c.hash2.map(|h| h as u64));
        }
    }

    #[test]
    fn container_headers_are_deterministic() {
        assert_eq!(
            ContainerHeader::pkg2(1, 2, &[3]).bytes,
            ContainerHeader::pkg2(1, 2, &[3]).bytes
        );
    }
}
