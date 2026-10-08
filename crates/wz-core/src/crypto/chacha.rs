//! MS v2 용 ChaCha20 (원본 `WzComparerR2.WzLib/Cryptography/ChaCha20CryptoTransform.cs`).
//!
//! # 왜 코어를 직접 구현했나
//!
//! `chacha20` 크레이트는 카운터를 0 으로 고정하고 `reset()` 을 노출하지 않는다.
//! 그런데 MS v2 포맷은 **두 가지 모두** 필요로 한다:
//!
//! 1. **임의 카운터 지정** — `Ms_ImageV2.cs:73` 이 `keyHashData[8..12]` 를
//!    카운터로 쓴다 (즉 0 이 아닐 수 있다).
//! 2. **64바이트마다 카운터 리셋** — `Ms_FileV2.ChaCha20Reader` 가 엔트리 테이블을
//!    읽을 때 블록 경계마다 `state[12] = 0` 으로 되돌린다 (`Ms_FileV2.cs:310-313`).
//!
//! RFC 8439 의 블록 함수는 50줄이고 [`chacha_block`] 테스트가 RFC 벡터로 검증한다.
//! 의존성 하나를 줄이고 통제권을 얻는 쪽이 이득이다.
//!
//! # 원본은 표준 ChaCha20 이 아니다 — 세 가지가 다르다
//!
//! | 항목 | 표준 (RFC 8439) | 원본 / MS v2 |
//! |---|---|---|
//! | 카운터 | 64비트 (`state[12..14]`), nonce 64비트 | **32비트** (`state[12]`), nonce 96비트 |
//! | 상태 인자 | `(key, counter, nonce)` | `(key, counter, nonce)` — 32/96 비트 |
//! | 카운터 리셋 | 없음 | **64바이트마다 `state[12] = 0`** |
//!
//! 카운터가 32비트이므로 2³² 블록(약 256 GB) 을 넘어 地址 지정할 수 없다.

use wz_error::WzCryptoError;

/// 한 번에 처리하는 바이트 (원본 `ProcessBytesAtTime = 64`, `ChaCha20CryptoTransform.cs:48`).
pub const PROCESS_BYTES_AT_TIME: usize = 64;

/// ChaCha20 상태: 상수 4 + 키 8 + 카운터 1 + 논스 3 = 16 워드.
#[derive(Clone, Copy)]
struct State([u32; 16]);

impl State {
    /// RFC 8439 §2.3.1 상태 초기화.
    ///
    /// `counter` 는 **32비트** (`state[12]`) — 원본 `IvSetup` (`ChaCha20CryptoTransform.cs:169`).
    fn new(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> Self {
        let mut s = [0u32; 16];
        // "expand 32-byte k"
        s[0] = 0x6170_7865;
        s[1] = 0x3320_646e;
        s[2] = 0x7962_2d32;
        s[3] = 0x6b20_6574;
        for i in 0..8 {
            s[4 + i] =
                u32::from_le_bytes([key[i * 4], key[i * 4 + 1], key[i * 4 + 2], key[i * 4 + 3]]);
        }
        s[12] = counter; // 32비트 카운터
        for i in 0..3 {
            s[13 + i] = u32::from_le_bytes([
                nonce[i * 4],
                nonce[i * 4 + 1],
                nonce[i * 4 + 2],
                nonce[i * 4 + 3],
            ]);
        }
        Self(s)
    }
}

#[inline(always)]
fn quarter_round(x: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    x[a] = x[a].wrapping_add(x[b]);
    x[d] = (x[d] ^ x[a]).rotate_left(16);
    x[c] = x[c].wrapping_add(x[d]);
    x[b] = (x[b] ^ x[c]).rotate_left(12);
    x[a] = x[a].wrapping_add(x[b]);
    x[d] = (x[d] ^ x[a]).rotate_left(8);
    x[c] = x[c].wrapping_add(x[d]);
    x[b] = (x[b] ^ x[c]).rotate_left(7);
}

/// ChaCha20 한 블록의 키스트림 (RFC 8439 §2.3.2).
fn chacha_block(state: &State) -> [u8; 64] {
    let mut x = state.0;
    let start = x;

    // 20 라운드 = 10 회 더블 라운드
    for _ in 0..10 {
        // 열 라운드
        quarter_round(&mut x, 0, 4, 8, 12);
        quarter_round(&mut x, 1, 5, 9, 13);
        quarter_round(&mut x, 2, 6, 10, 14);
        quarter_round(&mut x, 3, 7, 11, 15);
        // 대각 라운드
        quarter_round(&mut x, 0, 5, 10, 15);
        quarter_round(&mut x, 1, 6, 11, 12);
        quarter_round(&mut x, 2, 7, 8, 13);
        quarter_round(&mut x, 3, 4, 9, 14);
    }

    let mut out = [0u8; 64];
    for i in 0..16 {
        let w = x[i].wrapping_add(start[i]);
        out[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    out
}

/// ChaCha20 스트림 암호기.
///
/// # 두 가지 모드
///
/// 원본은 MS v2 용도로 **서로 다른 두 스트림**을 쓴다. 섞으면 안 된다.
///
/// | 생성자 | 카운터 동작 | 원본 대응 |
/// |---|---|---|
/// | [`new`](Self::new) / [`new_with_counter`](Self::new_with_counter) | 블록마다 +1 (표준 RFC 8439) | `Ms_ImageV2.OpenRead` (`Ms_ImageV2.cs:83`) |
/// | [`entry_table_reader`](Self::entry_table_reader) | **64바이트마다 0 으로 리셋** | `Ms_FileV2.ChaCha20Reader` (`Ms_FileV2.cs:310-313`) |
pub struct ChaCha20Stream {
    key: [u8; 32],
    nonce: [u8; 12],
    state: State,
    /// 현재 블록의 키스트림.
    block: [u8; 64],
    /// 블록 내 오프셋.
    pos: usize,
    /// 블록 경계마다 카운터를 0 으로 되돌리는가.
    reset_each_block: bool,
}

impl std::fmt::Debug for ChaCha20Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 키/논스/카운터는 로그에 남기지 않는다.
        f.debug_struct("ChaCha20Stream")
            .field("pos", &self.pos)
            .finish_non_exhaustive()
    }
}

impl Clone for ChaCha20Stream {
    fn clone(&self) -> Self {
        // state 가 카운터를 담고 있으므로 복사는 단순하다.
        Self {
            key: self.key,
            nonce: self.nonce,
            state: self.state,
            block: self.block,
            pos: self.pos,
            reset_each_block: self.reset_each_block,
        }
    }
}

impl ChaCha20Stream {
    /// 카운터 0, **표준** 카운터 증가 (RFC 8439).
    pub fn new(key: &[u8; 32], nonce: &[u8; 12]) -> Self {
        Self::new_with_counter(key, nonce, 0)
    }

    /// 명시적 카운터로 시작, **표준** 카운터 증가.
    ///
    /// 원본 `Ms_ImageV2.cs:83` 의 `CryptoStream` 경로가 이 모드다
    /// (카운터가 `keyHashData[8..12]` 로 주어지고 이후엔 증가).
    pub fn new_with_counter(key: &[u8; 32], nonce: &[u8; 12], counter: u32) -> Self {
        Self {
            key: *key,
            nonce: *nonce,
            state: State::new(key, counter, nonce),
            block: chacha_block(&State::new(key, counter, nonce)),
            pos: 0,
            reset_each_block: false,
        }
    }

    /// MS v2 엔트리 테이블 리더 — **64바이트마다 카운터를 0 으로 리셋**.
    ///
    /// 원본 `Ms_FileV2.ChaCha20Reader` 가 하는 일이다 (`Ms_FileV2.cs:310-313`).
    /// MS v2 헤더(8바이트)와 엔트리 테이블에서 쓰인다.
    pub fn entry_table_reader(key: &[u8; 32], nonce: &[u8; 12]) -> Self {
        let mut me = Self::new(key, nonce);
        me.reset_each_block = true;
        me
    }

    /// 길이 검증 후 생성. 원본 `AllowedKeyLength = 32`, `AllowedNonceLength = 12`.
    pub fn try_new(key: &[u8], nonce: &[u8], counter: u32) -> Result<Self, WzCryptoError> {
        if key.len() != 32 {
            return Err(WzCryptoError::ChaCha20KeyLen(key.len()));
        }
        if nonce.len() != 12 {
            return Err(WzCryptoError::StringDecode {
                reason: format!("논스 길이 {} (12 여야 함)", nonce.len()),
            });
        }
        let k: [u8; 32] = key.try_into().expect("길이 확인됨");
        let n: [u8; 12] = nonce.try_into().expect("길이 확인됨");
        Ok(Self::new_with_counter(&k, &n, counter))
    }

    /// 블록 경계에서 카운터를 0 으로 되돌린다 (원본 `Ms_FileV2.cs:310-313`).
    ///
    /// `state[12] = 0` 만 바꾸고 논스는 그대로 둔다.
    #[inline]
    fn reset_counter(&mut self) {
        self.state.0[12] = 0;
    }

    /// 다음 블록으로 넘어간다.
    ///
    /// # 함정
    ///
    /// 리셋 모드에서는 카운터를 **0 으로 되돌린 뒤 그 값으로 블록을 만든다.**
    /// 리셋 후 다시 1 을 더하면 모든 블록이 카운터 1 이 되어 리셋이 무의미해진다
    /// (실제로 처음 구현에서 이 버그가 났고 테스트가 잡았다).
    #[inline]
    fn advance_block(&mut self) {
        if self.reset_each_block {
            self.reset_counter();
        } else {
            self.state.0[12] = self.state.0[12].wrapping_add(1);
        }
        self.block = chacha_block(&self.state);
        self.pos = 0;
    }

    /// 키스트림을 제자리 적용한다.
    ///
    /// 길이가 64의 배수가 아니어도 동작한다.
    /// `entry_table_reader` 모드에서는 블록 경계에서 카운터가 0 으로 리셋된다.
    pub fn apply_keystream(&mut self, data: &mut [u8]) {
        for byte in data.iter_mut() {
            if self.pos == PROCESS_BYTES_AT_TIME {
                self.advance_block();
            }
            *byte ^= self.block[self.pos];
            self.pos += 1;
        }
    }

    /// 새 버퍼에 복호화 결과를 쓴다.
    pub fn decrypt_into(&mut self, src: &[u8], dst: &mut [u8]) {
        assert_eq!(src.len(), dst.len(), "복호화 길이 불일치");
        dst.copy_from_slice(src);
        self.apply_keystream(dst);
    }

    pub fn counter(&self) -> u32 {
        self.state.0[12]
    }

    /// 카운터를 지정하고 블록 위치(0)부터 다시 시작.
    pub fn reposition(&mut self, counter: u32) {
        self.state.0[12] = counter;
        self.block = chacha_block(&self.state);
        self.pos = 0;
    }

    /// 키/논스 접근자 (파생 로직 테스트용).
    pub fn key(&self) -> &[u8; 32] {
        &self.key
    }
    pub fn nonce(&self) -> &[u8; 12] {
        &self.nonce
    }
}

/// FNV-1a 32비트 (offset basis `0x811C9DC5`, prime `0x01000193`).
///
/// 원본 `Ms_ImageV2.cs:30-34`:
/// ```csharp
/// uint keyHash = 0x811C9DC5;
/// foreach (var c in KeySalt) keyHash = (keyHash ^ c) * 0x1000193;
/// ```
pub fn fnv1a32_over_chars(key_salt: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    // 원본은 `string` 을 `char` 로 순회하므로 UTF-16 코드유닛을 쓴다.
    for u in key_salt.encode_utf16() {
        h = (h ^ u as u32).wrapping_mul(0x0100_0193);
    }
    h
}

/// `keyHashDigits` — `keyHash.ToString().Select(v => (byte)(v - '0'))`
///
/// 원본 `Ms_ImageV2.cs:38`. **ASCII 코드가 아니라 숫자 값**이다.
pub fn key_hash_digits(key_hash: u32) -> Vec<u8> {
    key_hash
        .to_string()
        .bytes()
        .map(|b| b.wrapping_sub(b'0'))
        .collect()
}

/// `Ms_ImageV2` / `Ms_FileV2` 가 쓰는 키/논스/카운터 유도.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageKey {
    pub chacha_key: [u8; 32],
    pub chacha_nonce: [u8; 12],
    pub chacha_counter: u32,
}

/// `Ms_ImageV2.OpenRead` 의 키 유도를 그대로 옮긴 것 (`Ms_ImageV2.cs:27-73`).
///
/// # 원본의 미정의 동작 (UB) — 여기서 어떻게 처리했나
///
/// 원본 `Ms_ImageV2.cs:71-72` 는
/// ```csharp
/// Span<byte> nonce = stackalloc byte[12];
/// keyHashData.Slice(0, 8).CopyTo(nonce.Slice(4));   // nonce[0..4] 는 미기입!
/// ```
/// 이고 `IvSetup` 은 `state[13] = U8To32Little(nonce, 0)` 로 **nonce[0..4] 를
/// 실제로 사용한다** (`ChaCha20CryptoTransform.cs:170`).
/// 즉 원본은 **초기화되지 않은 스택 메모리**를 키스트림에 넣는다.
///
/// UB 를 그대로 재현할 수는 없다. MS v2 파일이 실제로 정상 복호화되고 같은
/// 랜덤 생성기가 파일을 쓰고 있으므로, 실전에서는 `nonce[0..4] == 0` 이라고
/// 보는 것이 유일하게 타당한 가정이다. 여기서는 **0 을 넣는다.**
///
/// > **오라클 검증 필요**: 실제 MS v2 파일로 이 가정이 맞는지 반드시 확인한다.
pub fn derive_image_key(key_salt: &str, entry_name: &str, entry_key: &[u8; 16]) -> ImageKey {
    let key_hash = fnv1a32_over_chars(key_salt);
    let key_hash2 = key_hash >> 1;
    let key_hash3 = key_hash2 ^ 0x6C;

    let digits = key_hash_digits(key_hash);
    let d = digits.len().max(1);

    // imgKey[32] (원본 `:44-51`)
    let name_chars: Vec<u16> = entry_name.encode_utf16().collect();
    let nlen = name_chars.len().max(1);
    let mut img_key = [0u8; 32];
    for (i, slot) in img_key.iter_mut().enumerate() {
        let ch = name_chars[i % nlen] as u32;
        let t = (digits[i % d] % 2) as u32
            + entry_key[(digits[(i + 2) % d] as usize + i) % 16] as u32
            + ((digits[(i + 1) % d] as usize + i) % 5) as u32;
        *slot = (i as u32).wrapping_add(ch.wrapping_mul(t)) as u8;
    }

    // XOR obscure (원본 `:52-55`)
    for (k, o) in img_key
        .iter_mut()
        .zip(super::consts::CHACHA20_KEY_OBSCURE.iter())
    {
        *k ^= *o;
    }

    // keyHashData[12] (원본 `:58-62`)
    let mut key_hash_data = [0u8; 12];
    key_hash_data[0..4].copy_from_slice(&key_hash.to_le_bytes());
    key_hash_data[4..8].copy_from_slice(&key_hash2.to_le_bytes());
    key_hash_data[8..12].copy_from_slice(&key_hash3.to_le_bytes());

    // 원본 `for (int i = 0, a = 0, b = 0, c = 90, d = 0; i < 12; ++i)`
    // → a,b,c,d 는 **루프 밖에서 선언되고 누적된다**. 매 반복마다 재초기화하지 않는다.
    let mut a: u8 = 0;
    let mut b: u8 = 0;
    let mut c: u8 = 90;
    let mut d: u8 = 0;
    for (i, slot) in key_hash_data.iter_mut().enumerate() {
        let mix = d
            .wrapping_add(11u8.wrapping_mul((i / 11) as u8))
            .wrapping_add(c ^ ((i >> 2) as u8))
            .wrapping_add(a ^ b);
        *slot ^= mix;
        d = d.wrapping_sub(1);
        a = a.wrapping_add(8);
        b = b.wrapping_add(17);
        c = c.wrapping_add(43);
    }

    // nonce[0..4] = 0 (원본 UB — 위 문서 참고), nonce[4..12] = keyHashData[0..8]
    let mut nonce = [0u8; 12];
    nonce[4..12].copy_from_slice(&key_hash_data[0..8]);

    // counter = u32 LE from keyHashData[8..12] (원본 `:73`)
    let counter = u32::from_le_bytes([
        key_hash_data[8],
        key_hash_data[9],
        key_hash_data[10],
        key_hash_data[11],
    ]);

    ImageKey {
        chacha_key: img_key,
        chacha_nonce: nonce,
        chacha_counter: counter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> [u8; 32] {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() {
            *b = i as u8;
        }
        k
    }
    fn nonce() -> [u8; 12] {
        [0xA5; 12]
    }

    // ── RFC 8439 벡터 ───────────────────────────────────────────────────────

    #[test]
    fn rfc8439_block0_vector() {
        // rfc8439 §2.3.2: key=0x00..1f, counter=1, nonce=00:00:00:09:00:00:00:4a:00:00:00:00
        let k: [u8; 32] = core::array::from_fn(|i| i as u8);
        let n: [u8; 12] = [0, 0, 0, 0x09, 0, 0, 0, 0x4a, 0, 0, 0, 0];
        let block = chacha_block(&State::new(&k, 1, &n));
        assert_eq!(
            &block[..16],
            &[
                0x10, 0xf1, 0xe7, 0xe4, 0xd1, 0x3b, 0x59, 0x15, 0x50, 0x0f, 0xdd, 0x1f, 0xa3, 0x20,
                0x71, 0xc4
            ],
            "RFC 8439 §2.3.2 키스트림"
        );
    }

    #[test]
    fn stream_blocks_match_independent_block_calls() {
        // 128바이트를 0 으로 채워 스트림에 넣으면
        //   buf[0..64]  = chacha_block(counter)
        //   buf[64..128] = chacha_block(counter+1)
        // 이어야 한다. 카운터 증가 로직을 독립 검증한다.
        //
        // (RFC 8439 §2.4.2 는 평문이 "Ladies and Gentlemen..." 이므로
        //  0-평문 스트림에는 쓸 수 없다. 핵심 검증은 §2.3.2 블록 벡터가 담당한다.)
        let k: [u8; 32] = core::array::from_fn(|i| i as u8);
        let n: [u8; 12] = [0xA5; 12];

        let mut s = ChaCha20Stream::new_with_counter(&k, &n, 5);
        let mut buf = [0u8; 128];
        s.apply_keystream(&mut buf);

        let expect_5 = chacha_block(&State::new(&k, 5, &n));
        let expect_6 = chacha_block(&State::new(&k, 6, &n));
        assert_eq!(&buf[..64], &expect_5[..], "첫 블록 = 카운터 5");
        assert_eq!(&buf[64..], &expect_6[..], "둘째 블록 = 카운터 6");
    }

    #[test]
    fn counter_overflow_wraps() {
        // 32비트 카운터이므로 u32::MAX 다음은 0 (RFC 와 동일).
        let k: [u8; 32] = [0; 32];
        let n: [u8; 12] = [0; 12];
        let mut s = ChaCha20Stream::new_with_counter(&k, &n, u32::MAX);
        let mut buf = [0u8; 64];
        s.apply_keystream(&mut buf);
        s.apply_keystream(&mut buf);
        assert_eq!(s.counter(), 0, "카운터가 wrap 해야 한다");
    }

    #[test]
    fn counter_is_32_bit_and_nonce_is_96_bit() {
        // state[12] 만 카운터, state[13..16] 이 논스 — 32/96 비트 배치.
        let k = [0u8; 32];
        let n = [0u8; 12];
        let s = State::new(&k, 0xDEAD_BEEF, &n);
        assert_eq!(s.0[12], 0xDEAD_BEEF, "카운터는 state[12]");
        assert_eq!(s.0[13..16], [0, 0, 0], "논스는 state[13..16]");
        // 카운터 2³²-1 이 표현 가능해야 한다
        let s2 = State::new(&k, u32::MAX, &n);
        assert_eq!(s2.0[12], u32::MAX);
    }

    #[test]
    fn consecutive_blocks_increment_counter() {
        // 카운터 증가는 "다음 블록이 필요할 때" 일어난다 (지연).
        let mut s = ChaCha20Stream::new(&key(), &nonce());
        assert_eq!(s.counter(), 0);
        let mut buf = [0u8; 64];
        s.apply_keystream(&mut buf);
        assert_eq!(s.counter(), 0, "첫 블록은 카운터 0");
        s.apply_keystream(&mut buf);
        assert_eq!(s.counter(), 1, "둘째 블록은 카운터 1");
        s.apply_keystream(&mut buf);
        assert_eq!(s.counter(), 2, "셋째 블록은 카운터 2");
    }

    // ── MS v2 규칙 ──────────────────────────────────────────────────────────

    #[test]
    fn standard_mode_does_not_reset_counter() {
        // `Ms_ImageV2` 경로: CryptoStream 이므로 카운터가 계속 증가한다.
        let mut s = ChaCha20Stream::new(&key(), &nonce());
        let mut two = [0u8; 128];
        s.apply_keystream(&mut two);
        assert_ne!(
            &two[..64],
            &two[64..],
            "표준 모드에서는 두 블록의 키스트림이 달라야 한다"
        );
    }

    #[test]
    fn entry_table_reader_resets_counter_every_64_bytes() {
        // `Ms_FileV2.ChaCha20Reader` 경로: 블록마다 state[12] = 0
        let mut s = ChaCha20Stream::entry_table_reader(&key(), &nonce());
        let mut two = [0u8; 128];
        s.apply_keystream(&mut two);
        assert_eq!(
            &two[..64],
            &two[64..],
            "리셋 모드에서는 64바이트마다 카운터가 0 이라 두 블록이 같다"
        );
    }

    #[test]
    fn two_modes_differ() {
        let mut a = ChaCha20Stream::new(&key(), &nonce());
        let mut b = ChaCha20Stream::entry_table_reader(&key(), &nonce());
        let mut x = [0u8; 128];
        let mut y = [0u8; 128];
        a.apply_keystream(&mut x);
        b.apply_keystream(&mut y);
        assert_eq!(&x[..64], &y[..64], "첫 블록은 같다");
        assert_ne!(&x[64..], &y[64..], "둘째 블록부터 갈라진다");
    }

    #[test]
    fn chunked_calls_equal_single_call() {
        let data: Vec<u8> = (0..200u32).map(|v| v as u8).collect();
        let mut a = ChaCha20Stream::new(&key(), &nonce());
        let mut whole = data.clone();
        a.apply_keystream(&mut whole);

        let mut b = ChaCha20Stream::new(&key(), &nonce());
        let mut chunked = Vec::new();
        for chunk in data.chunks(7) {
            let mut c = chunk.to_vec();
            b.apply_keystream(&mut c);
            chunked.extend_from_slice(&c);
        }
        assert_eq!(whole, chunked, "쪼개 호출과 일괄 호출이 같아야 한다");
    }

    #[test]
    fn chunking_across_block_boundary() {
        let data: Vec<u8> = (0..70u32).map(|v| v as u8).collect();
        let mut a = ChaCha20Stream::new(&key(), &nonce());
        let mut whole = data.clone();
        a.apply_keystream(&mut whole);

        let mut b = ChaCha20Stream::new(&key(), &nonce());
        let mut first = data[..60].to_vec();
        b.apply_keystream(&mut first);
        let mut second = data[60..].to_vec();
        b.apply_keystream(&mut second);
        first.extend_from_slice(&second);
        assert_eq!(whole, first);
    }

    #[test]
    fn explicit_counter_changes_stream() {
        let mut a = ChaCha20Stream::new_with_counter(&key(), &nonce(), 0);
        let mut b = ChaCha20Stream::new_with_counter(&key(), &nonce(), 7);
        let (mut x, mut y) = ([0u8; 64], [0u8; 64]);
        a.apply_keystream(&mut x);
        b.apply_keystream(&mut y);
        assert_ne!(x, y);
    }

    #[test]
    fn reposition_restarts_from_counter() {
        let data: Vec<u8> = (0..32u32).map(|v| v as u8).collect();
        let mut a = ChaCha20Stream::new_with_counter(&key(), &nonce(), 3);
        let mut first = data.clone();
        a.apply_keystream(&mut first);
        a.reposition(3);
        let mut again = data.clone();
        a.apply_keystream(&mut again);
        assert_eq!(first, again);
    }

    #[test]
    fn decrypt_roundtrip() {
        let plain: Vec<u8> = (0..128u32).map(|v| v as u8).collect();
        let mut enc = ChaCha20Stream::new(&key(), &nonce());
        let mut cipher = plain.clone();
        enc.apply_keystream(&mut cipher);
        let mut dec = ChaCha20Stream::new(&key(), &nonce());
        let mut out = vec![0u8; 128];
        dec.decrypt_into(&cipher, &mut out);
        assert_eq!(out, plain);
    }

    #[test]
    fn try_new_validates_lengths() {
        assert!(ChaCha20Stream::try_new(&[0u8; 32], &[0u8; 12], 0).is_ok());
        assert!(ChaCha20Stream::try_new(&[0u8; 16], &[0u8; 12], 0).is_err());
        assert!(ChaCha20Stream::try_new(&[0u8; 32], &[0u8; 16], 0).is_err());
    }

    #[test]
    fn empty_input_does_not_advance_state() {
        let mut s = ChaCha20Stream::new(&key(), &nonce());
        let mut empty: [u8; 0] = [];
        s.apply_keystream(&mut empty);
        assert_eq!(s.counter(), 0, "빈 호출이 카운터를 올리면 안 된다");
        let mut one = [0u8; 1];
        let expect = {
            let mut a = ChaCha20Stream::new(&key(), &nonce());
            let mut v = [0u8; 1];
            a.apply_keystream(&mut v);
            v[0]
        };
        s.apply_keystream(&mut one);
        assert_eq!(one[0], expect);
    }

    #[test]
    fn clone_is_independent() {
        let mut a = ChaCha20Stream::new_with_counter(&key(), &nonce(), 5);
        let mut b = a.clone();
        let (mut x, mut y) = ([0u8; 64], [0u8; 64]);
        a.apply_keystream(&mut x);
        b.apply_keystream(&mut y);
        assert_eq!(x, y);
        // 이후 각각 독립적으로 진행
        a.reposition(9);
        b.reposition(2);
        let (mut x2, mut y2) = ([0u8; 64], [0u8; 64]);
        a.apply_keystream(&mut x2);
        b.apply_keystream(&mut y2);
        assert_ne!(x2, y2);
    }

    // ── FNV / 이미지 키 ─────────────────────────────────────────────────────

    #[test]
    fn fnv1a_matches_known_vectors() {
        assert_eq!(fnv1a32_over_chars(""), 0x811C_9DC5);
        assert_eq!(fnv1a32_over_chars("a"), 0xE40C_292C);
        assert_eq!(fnv1a32_over_chars("foobar"), 0xBF9C_F968);
    }

    #[test]
    fn key_hash_digits_are_numeric_not_ascii() {
        // 원본 `(byte)(v - '0')` → '1' 은 1 (0x31 이 아님)
        assert_eq!(key_hash_digits(123), vec![1, 2, 3]);
        assert_eq!(key_hash_digits(0), vec![0]);
        assert_eq!(key_hash_digits(10), vec![1, 0]);
    }

    #[test]
    fn image_key_derivation_is_deterministic() {
        assert_eq!(
            derive_image_key("Base", "00002000.img", &[0u8; 16]),
            derive_image_key("Base", "00002000.img", &[0u8; 16])
        );
    }

    #[test]
    fn image_key_responds_to_all_inputs() {
        let base = derive_image_key("Base", "img.img", &[0u8; 16]);
        assert_ne!(
            base.chacha_key,
            derive_image_key("Other", "img.img", &[0u8; 16]).chacha_key
        );
        assert_ne!(
            base.chacha_key,
            derive_image_key("Base", "other.img", &[0u8; 16]).chacha_key
        );
        let mut ek = [0u8; 16];
        ek[3] = 0xAB;
        assert_ne!(
            base.chacha_key,
            derive_image_key("Base", "img.img", &ek).chacha_key
        );
    }

    #[test]
    fn image_nonce_prefix_is_zero() {
        // 원본 UB 를 0 으로 결정 (문서 참고)
        let k = derive_image_key("salt", "name", &[1u8; 16]);
        assert_eq!(&k.chacha_nonce[..4], &[0u8; 4]);
    }

    #[test]
    fn image_key_obscure_is_applied() {
        let (salt, name) = ("test-salt", "00002000.img");
        let ek = [0x11u8; 16];
        let k = derive_image_key(salt, name, &ek);

        // XOR 전 값을 다시 계산하고 obscure 를 되돌린다
        let kh = fnv1a32_over_chars(salt);
        let digits = key_hash_digits(kh);
        let d = digits.len();
        let chars: Vec<u16> = name.encode_utf16().collect();
        let mut raw = [0u8; 32];
        for (i, slot) in raw.iter_mut().enumerate() {
            let ch = chars[i % chars.len()] as u32;
            let t = (digits[i % d] % 2) as u32
                + ek[(digits[(i + 2) % d] as usize + i) % 16] as u32
                + ((digits[(i + 1) % d] as usize + i) % 5) as u32;
            *slot = (i as u32).wrapping_add(ch.wrapping_mul(t)) as u8;
        }
        for (i, b) in raw.iter_mut().enumerate() {
            *b ^= super::super::consts::CHACHA20_KEY_OBSCURE[i];
        }
        assert_eq!(k.chacha_key, raw);
    }

    #[test]
    fn debug_does_not_leak_key_material() {
        let s = ChaCha20Stream::new(&[0xAB; 32], &[0xCD; 12]);
        let dbg = format!("{s:?}");
        assert!(!dbg.contains("0xab"), "{dbg}");
        assert!(!dbg.contains("0xcd"), "{dbg}");
    }
}
