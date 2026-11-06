//! `WzComparerR2.Network` 중 네트워크 없이 검증 가능한 부분.
//!
//! - `RC4CryptoServiceProvider`의 RC4 스트림 (서버/클라이언트 세션 암호화)
//! - `WcClient.EncodePack` 프레이밍 (uint16 little-endian 길이 + UTF-8 JSON)
//!
//! TCP 연결/재접속/로그인 상태 머신은 서버가 필요해 제외한다.

pub mod pack;
pub mod session;

/// 상태를 유지하는 RC4 스트림. 원본 `RC4CryptoTransform`과 동일하다.
#[derive(Clone)]
pub struct Rc4 {
    s: [u8; 256],
    i: u8,
    j: u8,
}

impl Rc4 {
    pub fn new(key: &[u8]) -> Option<Self> {
        if key.is_empty() {
            return None;
        }
        let mut s = [0u8; 256];
        for (i, slot) in s.iter_mut().enumerate() {
            *slot = i as u8;
        }
        let mut j = 0u8;
        for i in 0..256 {
            j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
            s.swap(i, j as usize);
        }
        Some(Self { s, i: 0, j: 0 })
    }

    fn next(&mut self) -> u8 {
        self.i = self.i.wrapping_add(1);
        self.j = self.j.wrapping_add(self.s[self.i as usize]);
        self.s.swap(self.i as usize, self.j as usize);
        let idx = self.s[self.i as usize].wrapping_add(self.s[self.j as usize]);
        self.s[idx as usize]
    }

    /// `TransformBlock`처럼 입력을 제자리에서 XOR 한다.
    pub fn apply(&mut self, data: &mut [u8]) {
        for byte in data {
            *byte ^= self.next();
        }
    }

    pub fn transform(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = data.to_vec();
        self.apply(&mut out);
        out
    }
}

/// `WcClient.EncodePack` — uint16 LE 길이 + UTF-8 JSON.
pub fn encode_frame(json: &[u8]) -> Option<Vec<u8>> {
    let len = u16::try_from(json.len()).ok()?;
    let mut out = Vec::with_capacity(json.len() + 2);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(json);
    Some(out)
}

/// 길이 프레임을 순서대로 꺼내는 디코더.
#[derive(Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
}

impl FrameDecoder {
    pub fn push(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// 완성된 프레임 바디를 하나 꺼낸다. 부족하면 `None`.
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        if self.buf.len() < 2 {
            return None;
        }
        let len = u16::from_le_bytes([self.buf[0], self.buf[1]]) as usize;
        if self.buf.len() < len + 2 {
            return None;
        }
        let body = self.buf[2..2 + len].to_vec();
        self.buf.drain(..2 + len);
        Some(body)
    }

    pub fn buffered(&self) -> usize {
        self.buf.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rc4_known_vector() {
        // RC4("Key", "Plaintext")의 고전 테스트 벡터.
        let mut rc4 = Rc4::new(b"Key").unwrap();
        let out = rc4.transform(b"Plaintext");
        assert_eq!(
            out,
            vec![0xBB, 0xF3, 0x16, 0xE8, 0xD9, 0x40, 0xAF, 0x0A, 0xD3]
        );
    }

    #[test]
    fn frame_round_trip_and_partial() {
        let frame = encode_frame(br#"{"a":1}"#).unwrap();
        assert_eq!(&frame[..2], &[7, 0]);
        let mut dec = FrameDecoder::default();
        dec.push(&frame[..3]);
        assert!(dec.next_frame().is_none());
        dec.push(&frame[3..]);
        assert_eq!(dec.next_frame().as_deref(), Some(&br#"{"a":1}"#[..]));
        assert_eq!(dec.buffered(), 0);
    }

    #[test]
    fn frame_rejects_oversize() {
        assert!(encode_frame(&vec![0u8; 65536]).is_none());
    }
}
