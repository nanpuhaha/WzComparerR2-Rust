//! `ByteSource` 위의 순차 커서.
//!
//! WZ 파싱은 대부분 "여기서부터 N바이트 순차 읽기"다. 커서는 절대 위치를
//! 명시적으로 관리하고, 모든 읽기가 범위를 검사되도록 만든다.

use std::sync::Arc;

use wz_error::{Result, WzError, WzIoError};

use crate::ByteSource;

/// 절대 위치를 가진 순차 커서.
///
/// `Seek`는 되감기가 아니다 — 절대 위치만 설정한다. 되감기는 바깥에서 새
/// 커서를 만드는 것으로 표현한다. C#의 `PartialStream`(seekable 스트림 위의
/// 랜덤 액세스 에뮬레이션)을 복잡한 것에서 단순한 것으로 바꾼 결과다.
#[derive(Clone)]
pub struct ReadCtx {
    src: Arc<dyn ByteSource>,
    pos: u64,
}

impl std::fmt::Debug for ReadCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadCtx")
            .field("src", &self.src.describe())
            .field("pos", &self.pos)
            .finish()
    }
}

impl ReadCtx {
    pub fn new(src: Arc<dyn ByteSource>) -> Self {
        Self { src, pos: 0 }
    }

    /// 인메모리 버퍼 위에 커서를 만든다 (테스트·합성 WZ 용).
    pub fn from_bytes(buf: impl Into<Vec<u8>>) -> Self {
        Self::new(std::sync::Arc::new(crate::MemorySource::new(buf)))
    }

    /// 절대 위치에서 시작하는 커서.
    pub fn at(src: Arc<dyn ByteSource>, pos: u64) -> Self {
        Self { src, pos }
    }

    /// `&dyn ByteSource` (Arc 로 감싸지지 않은) 위의 커서.
    ///
    /// 감지 같은 "소스를 여러 블록에 나눠 읽되 커서는 하나" 경로에서 쓴다.
    /// `Box`/`&` 를 `Arc` 로 옮기는 대신 내부에 `Arc<dyn>` 를 그대로 담는다.
    pub fn from_source(src: Arc<dyn ByteSource>) -> Self {
        Self { src, pos: 0 }
    }

    pub fn source(&self) -> &Arc<dyn ByteSource> {
        &self.src
    }

    pub fn pos(&self) -> u64 {
        self.pos
    }

    pub fn len(&self) -> Result<u64> {
        self.src.len()
    }

    /// 소스의 전체 길이가 0 인지. (`len` 과 짝을 이루는 API 일관성)
    pub fn is_empty(&self) -> bool {
        self.src.is_empty()
    }

    pub fn seek(&mut self, abs: u64) -> Result<()> {
        let total = self.src.len()?;
        if abs > total {
            return Err(WzError::Io(WzIoError::OutOfBounds {
                offset: abs,
                len: 0,
                file_len: total,
            }));
        }
        self.pos = abs;
        Ok(())
    }

    /// `n`바이트 전진. 범위를 넘으면 에러 (오프셋은 되돌리지 않는다).
    pub fn skip(&mut self, n: u64) -> Result<()> {
        self.seek(self.pos.saturating_add(n))
    }

    /// 현재 위치에서 읽고 전진.
    pub fn read(&mut self, out: &mut [u8]) -> Result<()> {
        self.src.read_at(self.pos, out)?;
        self.pos = self.pos.saturating_add(out.len() as u64);
        Ok(())
    }

    /// 버퍼를 0으로 채우고 전진. 건너뛸 패딩용.
    pub fn skip_bytes(&mut self, n: usize) -> Result<()> {
        let mut buf = vec![0u8; n];
        self.read(&mut buf)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        let mut b = [0u8; 1];
        self.read(&mut b)?;
        Ok(b[0])
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_u16_le(&mut self) -> Result<u16> {
        let mut b = [0u8; 2];
        self.read(&mut b)?;
        Ok(u16::from_le_bytes(b))
    }

    pub fn read_i16_le(&mut self) -> Result<i16> {
        Ok(self.read_u16_le()? as i16)
    }

    pub fn read_u32_le(&mut self) -> Result<u32> {
        let mut b = [0u8; 4];
        self.read(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }

    pub fn read_i32_le(&mut self) -> Result<i32> {
        Ok(self.read_u32_le()? as i32)
    }

    pub fn read_u64_le(&mut self) -> Result<u64> {
        let mut b = [0u8; 8];
        self.read(&mut b)?;
        Ok(u64::from_le_bytes(b))
    }

    pub fn read_i64_le(&mut self) -> Result<i64> {
        Ok(self.read_u64_le()? as i64)
    }

    pub fn read_f32_le(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.read_u32_le()?))
    }

    pub fn read_f64_le(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.read_u64_le()?))
    }

    /// [start, end) 구간을 읽어 준다. 커서 위치는 `end`로 이동.
    pub fn read_range(&mut self, start: u64, end: u64) -> Result<Vec<u8>> {
        if end < start {
            return Err(WzError::Io(WzIoError::OutOfBounds {
                offset: start,
                len: 0,
                file_len: self.src.len().unwrap_or(0),
            }));
        }
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];
        self.src.read_at(start, &mut buf)?;
        self.pos = end;
        Ok(buf)
    }

    /// `len`바이트를 읽어 새 커서를 만든다 (기존 커서 위치는 진행).
    pub fn split_off(&mut self, len: u64) -> Result<ReadCtx> {
        let start = self.pos;
        self.skip(len)?;
        Ok(ReadCtx::at(std::sync::Arc::clone(&self.src), start))
    }

    /// 남은 바이트 수.
    pub fn remaining(&self) -> Result<u64> {
        Ok(self.src.len()?.saturating_sub(self.pos))
    }

    /// 소스 슬라이스가 있으면 현재 위치 기준을 반환 (복사 없음).
    pub fn slice_here(&self) -> Option<&[u8]> {
        let all = self.src.as_slice()?;
        let start = usize::try_from(self.pos).ok()?;
        all.get(start..)
    }
}

/// 프로토콜 확장으로 자주 쓰는 조합을 노출한다.
pub trait ReadCtxExt {
    fn read_exact_slice(&mut self, len: usize) -> Result<Vec<u8>>;
    fn peek_u8(&self) -> Result<u8>;
}

impl ReadCtxExt for ReadCtx {
    fn read_exact_slice(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.read(&mut buf)?;
        Ok(buf)
    }

    /// 커서를 움직이지 않고 1바이트를 본다.
    fn peek_u8(&self) -> Result<u8> {
        let mut b = [0u8; 1];
        self.src.read_at(self.pos, &mut b)?;
        Ok(b[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemorySource;
    use std::sync::Arc;

    #[test]
    fn reads_little_endian_scalars() {
        // 01 02 03 04 05 06 07 08 09 0A
        let src = MemorySource::new(vec![
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
        ]);
        let mut c = ReadCtx::new(Arc::new(src));

        // bytes[0..2] = 01 02 → LE u16 = 0x0201
        assert_eq!(c.read_u16_le().unwrap(), 0x0201);
        assert_eq!(c.pos(), 2);
        // bytes[2..6] = 03 04 05 06 → LE u32 = 0x06050403
        assert_eq!(c.read_u32_le().unwrap(), 0x0605_0403);
        assert_eq!(c.pos(), 6);
        // bytes[6..8] = 07 08 → LE u16 = 0x0807
        assert_eq!(c.read_u16_le().unwrap(), 0x0807);
        assert_eq!(c.pos(), 8);

        // signed 해석 (각각 새 커서 — i32 가 4바이트를 모두 소비한다)
        let mut c2 = ReadCtx::from_bytes(vec![0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(c2.read_i32_le().unwrap(), -1);
        let mut c3 = ReadCtx::from_bytes(vec![0xFF, 0xFF]);
        assert_eq!(c3.read_i16_le().unwrap(), -1);
        let mut c4 = ReadCtx::from_bytes(vec![0xFF; 8]);
        assert_eq!(c4.read_i64_le().unwrap(), -1);
    }

    #[test]
    fn seek_is_absolute_not_relative() {
        let src: Arc<dyn ByteSource> = Arc::new(MemorySource::new((0..64).collect::<Vec<u8>>()));
        let mut c = ReadCtx::new(src);
        c.seek(32).unwrap();
        assert_eq!(c.pos(), 32);
        let mut b = [0u8; 4];
        c.read(&mut b).unwrap();
        assert_eq!(b, [32, 33, 34, 35]);
        assert_eq!(c.pos(), 36);

        c.seek(10).unwrap();
        assert_eq!(c.pos(), 10);
    }

    #[test]
    fn reads_past_end_error_not_panic() {
        let mut c = ReadCtx::from_bytes(vec![1, 2, 3]);
        c.seek(1).unwrap();
        let mut big = [0u8; 8];
        assert!(c.read(&mut big).is_err());
    }

    #[test]
    fn from_bytes_matches_arc_memory_source() {
        let a = ReadCtx::from_bytes(vec![1, 2, 3, 4]);
        let b = ReadCtx::new(Arc::new(MemorySource::new(vec![1, 2, 3, 4])));
        assert_eq!(a.pos(), b.pos());
        assert_eq!(a.len().unwrap(), b.len().unwrap());
    }

    #[test]
    fn split_off_carries_source() {
        let src: Arc<dyn ByteSource> = Arc::new(MemorySource::new((0..64).collect::<Vec<u8>>()));
        let mut c = ReadCtx::new(src);
        c.seek(16).unwrap();
        let mut sub = c.split_off(8).unwrap();
        assert_eq!(c.pos(), 24);
        assert_eq!(sub.pos(), 16);

        let mut b = [0u8; 4];
        sub.read(&mut b).unwrap();
        assert_eq!(b, [16, 17, 18, 19]);
    }

    #[test]
    fn slice_here_avoids_copy() {
        let src = MemorySource::new((0..64).collect::<Vec<u8>>());
        let mut c = ReadCtx::new(std::sync::Arc::new(src));
        c.seek(60).unwrap();
        assert_eq!(c.slice_here().unwrap().len(), 4);
        assert_eq!(c.slice_here().unwrap()[0], 60);
    }

    #[test]
    fn peek_does_not_advance() {
        let src: Arc<dyn ByteSource> = Arc::new(MemorySource::new(vec![7, 8, 9]));
        let mut c = ReadCtx::new(src);
        assert_eq!(c.peek_u8().unwrap(), 7);
        assert_eq!(c.pos(), 0);
        assert_eq!(c.read_u8().unwrap(), 7);
        assert_eq!(c.pos(), 1);
    }

    #[test]
    fn remaining_and_read_range() {
        let src: Arc<dyn ByteSource> = Arc::new(MemorySource::new((0..100u8).collect::<Vec<u8>>()));
        let mut c = ReadCtx::new(src);
        assert_eq!(c.remaining().unwrap(), 100);
        c.seek(90).unwrap();
        assert_eq!(c.remaining().unwrap(), 10);
        let r = c.read_range(90, 95).unwrap();
        assert_eq!(r, vec![90, 91, 92, 93, 94]);
        assert_eq!(c.pos(), 95);
    }
}
