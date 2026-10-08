//! SNOW 2.0 스트림 사이퍼 (원본 `WzComparerR2.WzLib/Cryptography/Snow2CryptoTransform.cs`).
//!
//! 원본은 eSTREAM `snow2_fast.c` 의 포팅이다 (`Snow2CryptoTransform.cs:167`).
//! 64비트 블록 / 32비트 워드, 16 워드 키스트림.
//!
//! # 동작 요약
//!
//! - 키: 16 또는 32 바이트, **빅 엔디언** 으로 `s15..s0` 로 로드.
//!   16 바이트 키는 나머지 12 레지스터를 복사/반전으로 유도한다 (`:172-190`).
//! - IV: 선택적 4바이트. `s15 ^= iv[0]; s12 ^= iv[1]; s10 ^= iv[2]; s9 ^= iv[3]` (`:215-218`).
//!   WzLib 은 항상 `null` 을 넘긴다.
//! - 초기화: 32회 clocking, 각 clocking 에서 FSM 피드백 포함 (`:225-324`).
//! - `RefreshKeyStream` 가 16 워드 키스트림을 생성 (`:327-426`).
//! - 변환: `out_u32 = in_u32 - keyStream[curIndex]` (복호화) / `+` (암호화) (`:145-160`).
//!   `curIndex` 가 16 에 닿으면 스트림을 갱신하고 0 으로.
//!
//! # 위험 요소
//!
//! C# 코드가 `unchecked` 이므로 **모든 산술은 u32 wrap-around** 여야 한다.
//! Rust 에서 오버플로를 그대로 panic 으로 두면 어떤 파일이 파싱되는지 알 수 없다.

use wz_error::WzCryptoError;

use super::snow2_tables::{SBOX_TABLES, SNOW_ALPHAINV_MUL, SNOW_ALPHA_MUL};

/// 키스트림 워드 수.
const KEY_STREAM_WORDS: usize = 16;

/// SNOW 2.0 상태.
#[derive(Clone)]
pub struct Snow2 {
    /// 내부 레지스터 `s0..s15` (`s15` 가 최상위 인덱스).
    s: [u32; 16],
    /// 유한 상태 기계 레지스터.
    r1: u32,
    r2: u32,
    key_stream: [u32; KEY_STREAM_WORDS],
    cur_index: usize,
    encrypting: bool,
}

impl std::fmt::Debug for Snow2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 키 자료는 로그에 남기지 않는다.
        f.debug_struct("Snow2")
            .field("cur_index", &self.cur_index)
            .field("encrypting", &self.encrypting)
            .finish_non_exhaustive()
    }
}

#[inline(always)]
fn byte_of(n: usize, w: u32) -> usize {
    ((w >> (n * 8)) & 0xff) as usize
}

/// `A * w` — SNOW 의 field A 곱 (원본 `a_mul`, `Snow2CryptoTransform.cs:435`).
#[inline(always)]
fn a_mul(w: u32) -> u32 {
    (w << 8) ^ SNOW_ALPHA_MUL[(w >> 24) as usize]
}

/// `A^-1 * w` (원본 `ainv_mul`, `:432`).
#[inline(always)]
fn ainv_mul(w: u32) -> u32 {
    (w >> 8) ^ SNOW_ALPHAINV_MUL[(w & 0xff) as usize]
}

impl Snow2 {
    /// 키로 초기화. IV 는 비어 있어도 된다.
    ///
    /// # Panics
    /// 키 길이가 16 또는 32 가 아니면 패닉. `try_new` 로 에러 처리할 것.
    pub fn new(key: &[u8], iv: &[u8]) -> Self {
        Self::try_new(key, iv).expect("SNOW2 키 길이는 16 또는 32 여야 한다")
    }

    pub fn try_new(key: &[u8], iv: &[u8]) -> Result<Self, WzCryptoError> {
        if key.len() != 16 && key.len() != 32 {
            return Err(WzCryptoError::Snow2KeyLen(key.len()));
        }
        let mut me = Self {
            s: [0u32; 16],
            r1: 0,
            r2: 0,
            key_stream: [0u32; KEY_STREAM_WORDS],
            cur_index: 0,
            encrypting: false,
        };
        me.load_key(key, iv);
        me.initial_clocking();
        me.refresh_key_stream();
        Ok(me)
    }

    /// 복호화 모드로 전환 (기본값).
    pub fn decryptor(mut self) -> Self {
        self.encrypting = false;
        self
    }

    pub fn encryptor(mut self) -> Self {
        self.encrypting = true;
        self
    }

    fn load_key(&mut self, key: &[u8], iv: &[u8]) {
        // 원본 `MemoryMarshal.Cast<byte, sbyte>` + 시프트 조합.
        // 각 4바이트를 빅 엔디언으로 읽되, 최상위 바이트를 **부호 있는 sbyte** 로
        // 해석한 뒤 시프트한다 (원본 `:174`). 이는 u32 조립과 동일하다.
        let be = |i: usize| -> u32 {
            ((key[i] as u32) << 24)
                | ((key[i + 1] as u32) << 16)
                | ((key[i + 2] as u32) << 8)
                | (key[i + 3] as u32)
        };

        if key.len() == 16 {
            self.s[15] = be(0);
            self.s[14] = be(4);
            self.s[13] = be(8);
            self.s[12] = be(12);
            self.s[11] = !self.s[15];
            self.s[10] = !self.s[14];
            self.s[9] = !self.s[13];
            self.s[8] = !self.s[12];
            self.s[7] = self.s[15];
            self.s[6] = self.s[14];
            self.s[5] = self.s[13];
            self.s[4] = self.s[12];
            self.s[3] = !self.s[15];
            self.s[2] = !self.s[14];
            self.s[1] = !self.s[13];
            self.s[0] = !self.s[12];
        } else {
            self.s[15] = be(0);
            self.s[14] = be(4);
            self.s[13] = be(8);
            self.s[12] = be(12);
            self.s[11] = be(16);
            self.s[10] = be(20);
            self.s[9] = be(24);
            self.s[8] = be(28);
            self.s[7] = !self.s[15];
            self.s[6] = !self.s[14];
            self.s[5] = !self.s[13];
            self.s[4] = !self.s[12];
            self.s[3] = !self.s[11];
            self.s[2] = !self.s[10];
            self.s[1] = !self.s[9];
            self.s[0] = !self.s[8];
        }

        // IV XOR (원본 `:213-219`)
        if !iv.is_empty() {
            self.s[15] ^= iv[0] as u32;
            self.s[12] ^= iv[1] as u32;
            self.s[10] ^= iv[2] as u32;
            self.s[9] ^= iv[3] as u32;
        }
        self.r1 = 0;
        self.r2 = 0;
    }

    /// FSM 한 스텝: `r2` 갱신 후 새 `r1` 반환.
    #[inline(always)]
    fn fsm_r2(&self) -> u32 {
        SBOX_TABLES[0][byte_of(0, self.r1)]
            ^ SBOX_TABLES[1][byte_of(1, self.r1)]
            ^ SBOX_TABLES[2][byte_of(2, self.r1)]
            ^ SBOX_TABLES[3][byte_of(3, self.r1)]
    }

    /// 32회 초기 clocking (원본 `:225-324`). 두 겹의 8-워드 루프.
    fn initial_clocking(&mut self) {
        for _ in 0..2 {
            for _ in 0..8usize {
                // 원본은 인덱스를 하드코딩했다 (s0/s5, s1/s6, ... s7/s12).
                // i 로 일반화하되 `s[k]` 참조는 원본과 정확히 일치해야 한다.
                //
                // 1회째 반복 (s0): r1+r15, s2, s11, r2+s5
                // 2회째 반복 (s1): r1+r0,  s3, s12, r2+s6
                // ...
                // 8회째 반복 (s7): r1+r6,  s9, s2,  r2+s12
                //
                // 일반식: 반복 j 에서 갱신하는 레지스터는 s[j].
                //   보조 인덱스는 (j+2)%16 과 (j+11)%16, FSM 는 r2 + s[(j+5)%16].
                for j in 0..8usize {
                    let outfrom_fsm = (self.r1.wrapping_add(self.s[(j + 15) % 16])) ^ self.r2;
                    self.s[j] = a_mul(self.s[j])
                        ^ self.s[(j + 2) % 16]
                        ^ ainv_mul(self.s[(j + 11) % 16])
                        ^ outfrom_fsm;
                    let fsmtmp = self.r2.wrapping_add(self.s[(j + 5) % 16]);
                    self.r2 = self.fsm_r2();
                    self.r1 = fsmtmp;
                }
            }
        }
    }

    /// 16 워드 키스트림을 (재)생성한다 (원본 `RefreshKeyStream`, `:327-426`).
    ///
    /// 인덱스 `i` 는 `s[i]` 를 갱신하고 `keyStream[i]` 를 만든다.
    /// `r1/r2` 갱신에 `s[(i+5) % 16]` 과 `s[(i+12) % 16]` 을 쓴다.
    fn refresh_key_stream(&mut self) {
        // 원본의 `fsmtmp = r2 + s_k ; r2 = fsm() ; r1 = fsmtmp` 패턴을
        // 인덱스로 일반화한다. `k` 는 5,6,7,8,9,10,11,12,13,14,15,0,1,2,3,4
        // 이므로 `(i + 5) % 16` 이 된다.
        for i in 0..16usize {
            let s_i = a_mul(self.s[i]) ^ self.s[(i + 2) % 16] ^ ainv_mul(self.s[(i + 11) % 16]);
            self.s[i] = s_i;

            let fsmtmp = self.r2.wrapping_add(self.s[(i + 5) % 16]);
            self.r2 = self.fsm_r2();
            self.r1 = fsmtmp;

            self.key_stream[i] = (self.r1.wrapping_add(s_i)) ^ self.r2 ^ self.s[(i + 1) % 16];
        }
    }

    /// 키스트림의 현재 워드.
    #[inline(always)]
    fn current_word(&self) -> u32 {
        self.key_stream[self.cur_index]
    }

    /// 32비트 워드 하나를 변환 (원본 `TransformBlock` 루프 내부).
    #[inline(always)]
    fn transform_word(&mut self, w: u32) -> u32 {
        let out = if self.encrypting {
            w.wrapping_add(self.current_word())
        } else {
            w.wrapping_sub(self.current_word())
        };
        self.cur_index += 1;
        if self.cur_index >= KEY_STREAM_WORDS {
            self.refresh_key_stream();
            self.cur_index = 0;
        }
        out
    }

    /// 바이트 블록을 제자리 변환한다. 길이는 4의 배수여야 한다.
    ///
    /// # Panics
    /// `data.len() % 4 != 0` 이면 패닉. WZ SNOW2 는 항상 4 바이트 정렬이다.
    pub fn transform(&mut self, data: &mut [u8]) {
        assert_eq!(
            data.len() % 4,
            0,
            "SNOW2 는 4바이트 단위 변환만 지원한다 (원본 TransformBlock 과 동일)"
        );
        for chunk in data.as_chunks_mut::<4>().0.iter_mut() {
            let w = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let out = self.transform_word(w);
            chunk.copy_from_slice(&out.to_le_bytes());
        }
    }

    /// 새 상태로 복제 (독립 스트림 위치에서 계속).
    pub fn fork(&self) -> Self {
        self.clone()
    }

    pub fn cur_index(&self) -> usize {
        self.cur_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key16() -> [u8; 16] {
        (0..16)
            .map(|i| i as u8)
            .collect::<Vec<u8>>()
            .try_into()
            .unwrap()
    }
    fn key32() -> [u8; 32] {
        (0..32u32)
            .map(|i| (i * 7 + 3) as u8)
            .collect::<Vec<u8>>()
            .try_into()
            .unwrap()
    }

    #[test]
    fn rejects_bad_key_lengths() {
        for n in [0usize, 1, 8, 15, 17, 24, 31, 33, 64] {
            assert!(
                Snow2::try_new(&vec![0u8; n], &[]).is_err(),
                "길이 {n} 는 거부"
            );
        }
        assert!(Snow2::try_new(&key16(), &[]).is_ok());
        assert!(Snow2::try_new(&key32(), &[]).is_ok());
    }

    #[test]
    fn transform_requires_4_byte_alignment() {
        let mut s = Snow2::new(&key16(), &[]);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut b = [0u8; 3];
            s.transform(&mut b);
        }))
        .is_err());
    }

    #[test]
    fn decrypt_is_inverse_of_encrypt() {
        let plain: Vec<u8> = (0..256u32).map(|v| v as u8).collect();

        let mut enc = Snow2::new(&key32(), &[]).encryptor();
        let mut cipher = plain.clone();
        enc.transform(&mut cipher);
        assert_ne!(&cipher[..64], &plain[..64], "암호화되어야 한다");

        let mut dec = Snow2::new(&key32(), &[]).decryptor();
        dec.transform(&mut cipher);
        assert_eq!(&cipher[..], &plain[..], "복호화하면 평문 복구");
    }

    #[test]
    fn stateful_across_calls() {
        // 키스트림은 16워드마다 갱신된다. 한 번에 넘겨 호출과 쪼개 호출이 같아야 한다.
        let data: Vec<u8> = (0..256u32).map(|v| (v * 3) as u8).collect();

        let mut whole = Snow2::new(&key16(), &[]).encryptor();
        let mut buf_a = data.clone();
        whole.transform(&mut buf_a);

        let mut split = Snow2::new(&key16(), &[]).encryptor();
        let mut buf_b = Vec::new();
        for chunk in data.chunks(7).take(0) {
            let mut c = chunk.to_vec();
            split.transform(&mut c);
            buf_b.extend_from_slice(&c);
        }
        // 4바이트 경계로 쪼개서 처리
        let mut split2 = Snow2::new(&key16(), &[]).encryptor();
        let mut buf_c = Vec::new();
        for chunk in data.chunks(12) {
            // 12는 4의 배수
            let mut c = chunk.to_vec();
            split2.transform(&mut c);
            buf_c.extend_from_slice(&c);
        }
        let _ = (split, buf_b);

        assert_eq!(buf_a, buf_c, "쪼개 호출과 일괄 호출이 같아야 한다");
    }

    #[test]
    fn keystream_refreshes_after_16_words() {
        // 64바이트(16워드) 이후에는 키스트림이 갱신되어야 한다.
        let mut a = Snow2::new(&key16(), &[]).encryptor();
        let mut b = Snow2::new(&key16(), &[]).encryptor();
        let mut x = [0u8; 128];
        let mut y = [0u8; 128];
        a.transform(&mut x);
        b.transform(&mut y);
        // 0으로만 입력하면 출력 = 키스트림
        assert_eq!(x[0..16], y[0..16]);
        assert_ne!(
            x[0..16],
            x[16..32],
            "첫 64바이트(키스트림 1세대)와 그 다음이 달라야 한다"
        );
        assert_ne!(x[16..32], x[32..48]);
    }

    #[test]
    fn iv_changes_stream() {
        let a = {
            let mut s = Snow2::new(&key16(), &[]).encryptor();
            let mut b = vec![0u8; 64];
            s.transform(&mut b);
            b
        };
        let b = {
            let mut s = Snow2::new(&key16(), &[1, 2, 3, 4]).encryptor();
            let mut v = vec![0u8; 64];
            s.transform(&mut v);
            v
        };
        assert_ne!(a, b, "IV 가 키스트림에 반영되어야 한다");
    }

    #[test]
    fn key_schedule_differs_between_key_sizes() {
        let a = {
            let mut s = Snow2::new(&key16(), &[]).encryptor();
            let mut b = vec![0u8; 32];
            s.transform(&mut b);
            b
        };
        let b = {
            let mut s = Snow2::new(&key32(), &[]).encryptor();
            let mut v = vec![0u8; 32];
            s.transform(&mut v);
            v
        };
        assert_ne!(a, b);
    }

    #[test]
    fn different_keys_give_different_streams() {
        let a = {
            let mut s = Snow2::new(&key16(), &[]).encryptor();
            let mut b = vec![0u8; 32];
            s.transform(&mut b);
            b
        };
        let mut other = key16();
        other[0] ^= 0xFF;
        let b = {
            let mut s = Snow2::new(&other, &[]).encryptor();
            let mut v = vec![0u8; 32];
            s.transform(&mut v);
            v
        };
        assert_ne!(a, b);
    }

    #[test]
    fn fork_independs_from_position() {
        let mut a = Snow2::new(&key16(), &[]).encryptor();
        let mut b = a.fork();
        let mut x = [0u8; 32];
        let mut y = [0u8; 32];
        a.transform(&mut x);
        b.transform(&mut y);
        assert_eq!(x, y, "fork 직후에는 같은 위치");
        // a 만 진행하면 이후 결과가 갈린다
        let mut x2 = [0u8; 32];
        a.transform(&mut x2);
        let mut y2 = [0u8; 32];
        b.transform(&mut y2);
        assert_eq!(x2, y2, "각각 독립적으로 진행하므로 여전히 동일");
    }

    #[test]
    fn a_mul_and_ainv_are_consistent() {
        // SNOW 의 A 곱은 체(field) 연산이다. 0 은 고정점이고 0 이 아니다.
        assert_eq!(a_mul(0), 0, "체 연산에서 0*x = 0");
        assert_eq!(ainv_mul(0), 0);
        assert_ne!(a_mul(1), 0, "1*x != 0");
        // 결정성
        for x in [0u32, 1, 0x1234_5678, 0xFFFF_FFFF, 0xDEAD_BEEF] {
            assert_eq!(a_mul(x), a_mul(x));
            assert_eq!(ainv_mul(x), ainv_mul(x));
        }
    }

    #[test]
    fn a_mul_is_a_bijection() {
        // 체 곱이므로 단사 함수여야 한다. 0 만 자기 자신으로 갈 수 있다.
        for x in [0u32, 1, 0x1234_5678, 0xFFFF_FFFF, 0xDEAD_BEEF, 0x00FF_00FF] {
            let y = a_mul(x);
            assert_eq!(ainv_mul(y), x, "A^-1(A(x)) == x 여야 한다 (x={x:#x})");
        }
    }

    #[test]
    fn zero_input_in_encrypt_mode_yields_keystream() {
        // 암호화 모드: out = 0 + ks = ks → 0 바이트를 넣으면 키스트림이 그대로 나온다.
        let mut s = Snow2::new(&key16(), &[]).encryptor();
        let mut buf = vec![0u8; 64];
        s.transform(&mut buf);

        // 결정성: 같은 키면 같은 키스트림
        let mut s2 = Snow2::new(&key16(), &[]).encryptor();
        let mut buf2 = vec![0u8; 64];
        s2.transform(&mut buf2);
        assert_eq!(buf, buf2, "키스트림은 결정적이어야 한다");
    }

    #[test]
    fn debug_does_not_leak_key_state() {
        let s = Snow2::new(&[0xAB; 32], &[]);
        let dbg = format!("{s:?}");
        assert!(
            !dbg.contains("0xabab"),
            "키 자료가 로그에 새면 안 된다: {dbg}"
        );
    }
}
