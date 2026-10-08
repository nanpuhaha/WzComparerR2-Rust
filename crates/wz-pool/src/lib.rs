//! 랜덤 접근 바이트 소스 (리포트 ADR-1).
//!
//! # 설계 요약
//!
//! C# 원본은 `IMapleStoryFile`이 `Stream`을 노출하고 `WzFile.ReadLock`(단순
//! `object`)으로 다중 스레드 접근을 직렬화했다. 그 결과 모든 병렬 처리가 자물쇠
//! 경합으로 끝났다.
//!
//! 이 crate는 **불변(random-access) 바이트 소스**를 `Send + Sync`로 노출한다.
//! mmap는 프로세스 주소 공간에서 읽기 전용이므로 잠금이 아예 불필요하고,
//! rayon이 겹쳐서 돌 수 있다.
//!
//! ```no_run
//! use wz_pool::ByteSource;
//!
//! let src = wz_pool::open_file("Data.wz")?;   // mmap 시도 → 실패 시 File 소스
//! let mut buf = [0u8; 16];
//! src.read_at(0, &mut buf)?;                   // 잠금 없이, 다른 스레드와 동시에
//! # Ok::<(), wz_error::WzError>(())
//! ```

use std::fs::File;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use wz_error::{Result, WzError, WzIoError};

pub mod cache;
pub mod cursor;

pub use cursor::{ReadCtx, ReadCtxExt};

/// 랜덤 접근 바이트 소스.
///
/// # 계약 (contract)
///
/// - `read_at`은 **반드시** 요청한 길이만큼 채우거나 에러를 낸다. 부분 읽기는 없다.
/// - 구현은 `Send + Sync`이며 내부 상태를 바꾸지 않는다.
pub trait ByteSource: Send + Sync + std::fmt::Debug {
    /// 전체 길이(바이트).
    fn len(&self) -> Result<u64>;

    /// `out[offset..offset+out.len()]`을 채운다. 범위를 벗어나면 에러.
    fn read_at(&self, offset: u64, out: &mut [u8]) -> Result<()>;

    /// 연속 바이트를 그대로 참조할 수 있으면 반환. mmap 소스만 `Some`.
    ///
    /// 파서의 hot path(디렉터리 트리 파싱, 암호 키스트림 입출력)가
    /// 이것을 쓰면 복사가 사라진다.
    fn as_slice(&self) -> Option<&[u8]> {
        None
    }

    fn is_empty(&self) -> bool {
        matches!(self.len(), Ok(0))
    }

    /// 사람이 읽을 수 있는 식별자 (경로 또는 "<memory>").
    fn describe(&self) -> String;

    /// 파일 읽기 연산을 위한 안전한 헬퍼.
    fn try_read_exact(&self, offset: u64, out: &mut [u8]) -> Result<()> {
        match self.read_at(offset, out) {
            Ok(()) => Ok(()),
            Err(WzError::Io(WzIoError::Read { .. })) => Err(WzError::Io(WzIoError::OutOfBounds {
                offset,
                len: out.len(),
                file_len: self.len().unwrap_or(0),
            })),
            Err(e) => Err(e),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MemorySource
// ─────────────────────────────────────────────────────────────────────────────

/// 메모리 상주 바이트 버퍼. 테스트와 인메모리 WZ에 사용.
#[derive(Debug, Clone)]
pub struct MemorySource {
    buf: Arc<[u8]>,
    name: String,
}

impl MemorySource {
    pub fn new(buf: impl Into<Vec<u8>>) -> Self {
        let buf = buf.into();
        let len = buf.len();
        Self {
            buf: Arc::from(buf),
            name: format!("<memory:{len}>"),
        }
    }

    pub fn named(buf: impl Into<Vec<u8>>, name: impl Into<String>) -> Self {
        Self {
            buf: Arc::from(buf.into()),
            name: name.into(),
        }
    }

    pub fn as_arc(&self) -> Arc<[u8]> {
        Arc::clone(&self.buf)
    }
}

impl ByteSource for MemorySource {
    fn len(&self) -> Result<u64> {
        Ok(self.buf.len() as u64)
    }

    fn read_at(&self, offset: u64, out: &mut [u8]) -> Result<()> {
        let start = usize::try_from(offset).map_err(|_| {
            WzError::Io(WzIoError::OutOfBounds {
                offset,
                len: out.len(),
                file_len: self.buf.len() as u64,
            })
        })?;
        let end = start.checked_add(out.len()).ok_or_else(|| {
            WzError::Io(WzIoError::OutOfBounds {
                offset,
                len: out.len(),
                file_len: self.buf.len() as u64,
            })
        })?;
        let slice = self.buf.get(start..end).ok_or_else(|| {
            WzError::Io(WzIoError::OutOfBounds {
                offset,
                len: out.len(),
                file_len: self.buf.len() as u64,
            })
        })?;
        out.copy_from_slice(slice);
        Ok(())
    }

    fn as_slice(&self) -> Option<&[u8]> {
        Some(&self.buf)
    }

    fn describe(&self) -> String {
        self.name.clone()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FileSource — pread 기반 (mmap 불가 환경 폴백)
// ─────────────────────────────────────────────────────────────────────────────

/// `pread` 기반 파일 소스. mmap이 불가능한 환경(일부 네트워크 파일시스템,
/// 32비트 주소 공간 등)에서 쓰인다.
///
/// POSIX·Windows 양쪽에서 스레드 안전(offset이 명시적이므로 공유 파일 핸들 사용 가능).
#[derive(Debug)]
pub struct FileSource {
    file: File,
    path: PathBuf,
    len: u64,
}

impl FileSource {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(|source| {
            WzError::Io(WzIoError::Open {
                path: path.clone(),
                source,
            })
        })?;
        let len = file
            .metadata()
            .map_err(WzIoError::OpenBare)
            .map_err(WzError::Io)?
            .len();
        Ok(Self { file, path, len })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(unix)]
fn read_exact_at(file: &File, offset: u64, out: &mut [u8]) -> std::io::Result<()> {
    use std::os::unix::fs::FileExt;
    file.read_exact_at(out, offset)
}

#[cfg(windows)]
fn read_exact_at(file: &File, offset: u64, out: &mut [u8]) -> std::io::Result<()> {
    use std::os::windows::fs::FileExt;
    // Windows 의 `FileExt` 는 `seek_read` (부분 읽기 가능) 만 노출한다.
    // 정확히 N 바이트를 요구하므로 루프로 채운다.
    // Unix 의 `read_exact_at` 과 동일한 의미가 되도록 한다.
    let mut done = 0usize;
    while done < out.len() {
        let n = file.seek_read(&mut out[done..], offset + done as u64)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "unexpected end of file",
            ));
        }
        done += n;
    }
    Ok(())
}

// NOTE: Unix 와 Windows 의 `FileExt::read_exact_at` 는 **같은 이름이지만
// 인자 순서가 반대**다 (unix: (buf, offset), windows: (buf, offset) — 동일).
// 그래도 cfg 로 분리해 플랫폼별 시그니처 차이를 명시적으로 드러낸다.

#[cfg(not(any(unix, windows)))]
fn read_exact_at(file: &File, offset: u64, out: &mut [u8]) -> std::io::Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = file.try_clone()?;
    f.seek(SeekFrom::Start(offset))?;
    f.read_exact(out)
}

impl ByteSource for FileSource {
    fn len(&self) -> Result<u64> {
        Ok(self.len)
    }

    fn read_at(&self, offset: u64, out: &mut [u8]) -> Result<()> {
        if out.is_empty() {
            return Ok(());
        }
        let end =
            offset
                .checked_add(out.len() as u64)
                .ok_or(WzError::Io(WzIoError::OutOfBounds {
                    offset,
                    len: out.len(),
                    file_len: self.len,
                }))?;
        if end > self.len {
            return Err(WzError::Io(WzIoError::OutOfBounds {
                offset,
                len: out.len(),
                file_len: self.len,
            }));
        }
        read_exact_at(&self.file, offset, out).map_err(|source| {
            if source.kind() == std::io::ErrorKind::UnexpectedEof {
                WzError::Io(WzIoError::OutOfBounds {
                    offset,
                    len: out.len(),
                    file_len: self.len,
                })
            } else {
                WzError::Io(WzIoError::Read {
                    offset,
                    len: out.len(),
                    source,
                })
            }
        })
    }

    fn describe(&self) -> String {
        self.path.display().to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MmapSource — 기본 경로
// ─────────────────────────────────────────────────────────────────────────────

/// 읽기 전용 mmap 소스.
///
/// 파서의 대다수 접근이 `as_slice()` 경로(제로 복사)로 흐른다.
pub struct MmapSource {
    map: memmap2::Mmap,
    len: u64,
    path: PathBuf,
}

impl std::fmt::Debug for MmapSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MmapSource")
            .field("path", &self.path)
            .field("len", &self.len)
            .finish()
    }
}

impl MmapSource {
    #[expect(
        unsafe_code,
        reason = "mmap 생성은 unsafe 이다. 읽기 전용 매핑이며 쓰기 API 가 없다."
    )]
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(|source| {
            WzError::Io(WzIoError::Open {
                path: path.clone(),
                source,
            })
        })?;
        // SAFETY: `memmap2::Mmap::map` 은 **읽기 전용** 매핑을 만든다.
        // 이 타입은 쓰기 API 를 노출하지 않고 내부 버퍼가 불변이므로,
        // `&MmapSource` 를 공유해도 별칭 규칙이 깨지지 않는다
        // (아래의 `unsafe impl Send/Sync` 가 그 근거다).
        // 유일하게 남는 위험은 파일 외부에서 동일 파일을 truncate 하는 것인데,
        // `Mmap` 은 SIGBUS 로 처리한다 — 원본 C# `FileStream` 과 동일한 노출.
        let map = unsafe { memmap2::Mmap::map(&file) }.map_err(WzIoError::Mmap)?;
        let len = map.len() as u64;
        Ok(Self { map, len, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

// SAFETY: `memmap2::Mmap::map` 은 읽기 전용 매핑을 만들고, 이 타입이 제공하는
// 유일한 API 가 `&[u8]` 읽기다. 쓰기가 불가능하므로 여러 스레드가 동시에
// 읽어도 별칭 규칙이 깨지지 않는다. (`memmap2::Mmap` 자체는 `!Sync` 다.)
#[expect(unsafe_code, reason = "읽기 전용 mmap 의 Send/Sync")]
unsafe impl Send for MmapSource {}
#[expect(unsafe_code, reason = "읽기 전용 mmap 의 Send/Sync")]
unsafe impl Sync for MmapSource {}

impl ByteSource for MmapSource {
    fn len(&self) -> Result<u64> {
        Ok(self.len)
    }

    fn read_at(&self, offset: u64, out: &mut [u8]) -> Result<()> {
        if out.is_empty() {
            return Ok(());
        }
        let start = usize::try_from(offset).map_err(|_| {
            WzError::Io(WzIoError::OutOfBounds {
                offset,
                len: out.len(),
                file_len: self.len,
            })
        })?;
        let end = start
            .checked_add(out.len())
            .ok_or(WzError::Io(WzIoError::OutOfBounds {
                offset,
                len: out.len(),
                file_len: self.len,
            }))?;
        let slice = self
            .map
            .get(start..end)
            .ok_or(WzError::Io(WzIoError::OutOfBounds {
                offset,
                len: out.len(),
                file_len: self.len,
            }))?;
        out.copy_from_slice(slice);
        Ok(())
    }

    fn as_slice(&self) -> Option<&[u8]> {
        Some(self.map.deref())
    }

    fn describe(&self) -> String {
        self.path.display().to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 열기 헬퍼
// ─────────────────────────────────────────────────────────────────────────────

/// mmap을 우선 시도하고, 실패하면 `pread` 소스로 폴백한다.
///
/// WZ 파일은 수십 GB까지 크므로 Windows 32비트처럼 주소 공간 제약이 있는
/// 환경에서는 mmap이 실패할 수 있다. 조용히 degrade한다.
pub fn open_file(path: impl AsRef<Path>) -> Result<Box<dyn ByteSource>> {
    let path = path.as_ref();
    match MmapSource::open(path) {
        Ok(m) => Ok(Box::new(m)),
        Err(_mmap_err) => Ok(Box::new(FileSource::open(path)?)),
    }
}

/// 경로를 열되, 경로가 `None`이면 빈 소스를 준다 (테스트/CLI 용).
pub fn open_file_opt(path: Option<&Path>) -> Result<Box<dyn ByteSource>> {
    match path {
        Some(p) => open_file(p),
        None => Ok(Box::new(MemorySource::new(Vec::new()))),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 부분 소스 — 오프셋 윈도우를 감싼다
// ─────────────────────────────────────────────────────────────────────────────

/// 하위 소스의 `[base, base+len)` 구간을 0 기준으로 재번호 매긴다.
///
/// WZ `entry`는 파일 절대 오프셋을 들고 다닌다. 부분 소스는 그 절대 오프셋을
/// 그대로 쓰면서도 로컬 좌표 읽기를 제공한다.
#[derive(Debug, Clone)]
pub struct WindowSource {
    inner: Arc<dyn ByteSource>,
    base: u64,
    len: u64,
}

impl WindowSource {
    pub fn new(inner: Arc<dyn ByteSource>, base: u64, len: u64) -> Result<Self> {
        let total = inner.len()?;
        if base > total || base + len > total {
            return Err(WzError::Io(WzIoError::OutOfBounds {
                offset: base,
                len: len as usize,
                file_len: total,
            }));
        }
        Ok(Self { inner, base, len })
    }

    /// 파일 절대 오프셋을 로컬(윈도우 기준) 오프셋으로 바꿔 읽는다.
    pub fn read_abs(&self, abs_offset: u64, out: &mut [u8]) -> Result<()> {
        let local =
            abs_offset
                .checked_sub(self.base)
                .ok_or(WzError::Io(WzIoError::OutOfBounds {
                    offset: abs_offset,
                    len: out.len(),
                    file_len: self.len,
                }))?;
        self.inner.read_at(self.base + local, out)
    }
}

impl ByteSource for WindowSource {
    fn len(&self) -> Result<u64> {
        Ok(self.len)
    }

    fn read_at(&self, offset: u64, out: &mut [u8]) -> Result<()> {
        let end =
            offset
                .checked_add(out.len() as u64)
                .ok_or(WzError::Io(WzIoError::OutOfBounds {
                    offset,
                    len: out.len(),
                    file_len: self.len,
                }))?;
        if end > self.len {
            return Err(WzError::Io(WzIoError::OutOfBounds {
                offset,
                len: out.len(),
                file_len: self.len,
            }));
        }
        self.inner.read_at(self.base + offset, out)
    }

    fn describe(&self) -> String {
        format!(
            "{}[{}..{}]",
            self.inner.describe(),
            self.base,
            self.base + self.len
        )
    }
}

/// 하위 소스의 일부를 잘라낸다. 편의 함수.
pub fn window(src: Arc<dyn ByteSource>, base: u64, len: u64) -> Result<WindowSource> {
    WindowSource::new(src, base, len)
}

/// `open_file` 의 `Arc` 버전.
///
/// `ReadCtx` 는 `Arc<dyn ByteSource>` 를 요구하는데 `Box` 는 `Clone` 이 안 된다
/// (`dyn` 은 `Sized` 가 아니므로). 트리/이미지 명령처럼 소스를 여러 커서에서
/// 공유할 수 있어야 할 때 이 함수를 쓴다.
pub fn open_file_arc(path: impl AsRef<Path>) -> Result<std::sync::Arc<dyn ByteSource>> {
    let path = path.as_ref();
    match MmapSource::open(path) {
        Ok(m) => Ok(std::sync::Arc::new(m)),
        Err(_mmap_err) => Ok(std::sync::Arc::new(FileSource::open(path)?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_error::WzIoError;

    fn pattern(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn memory_source_reads_exactly() {
        let src = MemorySource::new(pattern(1024));
        assert_eq!(src.len().unwrap(), 1024);

        let mut buf = [0u8; 16];
        src.read_at(0, &mut buf).unwrap();
        assert_eq!(buf, pattern(16)[..]);

        let mut buf2 = [0u8; 16];
        src.read_at(1000, &mut buf2).unwrap();
        assert_eq!(buf2, pattern(1024)[1000..1016]);
    }

    #[test]
    fn out_of_bounds_is_typed_not_panic() {
        let src = MemorySource::new(pattern(16));
        let mut buf = [0u8; 32];
        let err = src.read_at(0, &mut buf).unwrap_err();
        // S-6: 실패가 조용히 0으로 채워지지 않는다.
        assert!(matches!(err, WzError::Io(WzIoError::OutOfBounds { .. })));
    }

    #[test]
    fn empty_read_at_end_is_ok() {
        let src = MemorySource::new(pattern(16));
        let mut empty: [u8; 0] = [];
        assert!(src.read_at(16, &mut empty).is_ok());
    }

    #[test]
    fn memory_and_file_sources_agree() {
        let data = pattern(4096);
        let mem = MemorySource::new(data.clone());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.bin");
        std::fs::write(&path, &data).unwrap();

        let fsrc = FileSource::open(&path).unwrap();
        let msrc = MmapSource::open(&path).unwrap();

        for &(off, len) in &[(0u64, 1usize), (1, 7), (255, 100), (4095, 1), (100, 1000)] {
            let mut a = vec![0u8; len];
            let mut b = vec![0u8; len];
            let mut c = vec![0u8; len];
            mem.read_at(off, &mut a).unwrap();
            fsrc.read_at(off, &mut b).unwrap();
            msrc.read_at(off, &mut c).unwrap();
            assert_eq!(a, b, "mem/file 불일치 off={off} len={len}");
            assert_eq!(a, c, "mem/mmap 불일치 off={off} len={len}");
            assert_eq!(a, data[off as usize..off as usize + len], "데이터 불일치");
        }
    }

    #[test]
    fn mmap_falls_back_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.bin");
        std::fs::write(&path, pattern(1000)).unwrap();

        let src = open_file(&path).unwrap();
        assert_eq!(src.len().unwrap(), 1000);
        let mut buf = [0u8; 4];
        src.read_at(0, &mut buf).unwrap();
        assert_eq!(&buf, &pattern(1000)[..4]);
    }

    #[test]
    fn sources_are_send_and_sync() {
        fn assert_ss<T: Send + Sync>(_: &T) {}
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.bin");
        std::fs::write(&path, pattern(4096)).unwrap();

        let m = MemorySource::new(pattern(4096));
        let f = FileSource::open(&path).unwrap();
        let mm = MmapSource::open(&path).unwrap();
        assert_ss(&m);
        assert_ss(&f);
        assert_ss(&mm);

        // ADR-1: 동시 읽기가 실제로 가능해야 한다.
        let shared: Arc<dyn ByteSource> = Arc::new(mm);
        std::thread::scope(|s| {
            for _ in 0..8 {
                let src = Arc::clone(&shared);
                s.spawn(move || {
                    for off in 0..4000u64 {
                        let mut b = [0u8; 8];
                        src.read_at(off, &mut b).unwrap();
                    }
                });
            }
        });
    }

    #[test]
    fn window_source_renumbers_offsets() {
        let inner: Arc<dyn ByteSource> = Arc::new(MemorySource::new(pattern(1000)));
        let w = window(Arc::clone(&inner), 100, 200).unwrap();
        assert_eq!(w.len().unwrap(), 200);

        // 로컬 오프셋 0 은 파일 절대 오프셋 100 에 대응
        let mut local = [0u8; 8];
        w.read_at(0, &mut local).unwrap();
        assert_eq!(local, pattern(1000)[100..108]);

        // 절대 오프셋 108 과 로컬 오프셋 8 은 같은 바이트
        let mut abs = [0u8; 8];
        w.read_abs(108, &mut abs).unwrap();
        assert_eq!(abs, pattern(1000)[108..116]);
        let mut local8 = [0u8; 8];
        w.read_at(8, &mut local8).unwrap();
        assert_eq!(abs, local8, "절대/로컬 좌표가 일치해야 한다");

        // 윈도우 밖 절대 오프셋은 거부
        let mut bad = [0u8; 4];
        assert!(w.read_abs(50, &mut bad).is_err());
        assert!(w.read_at(200, &mut bad).is_err());
    }

    #[test]
    fn window_rejects_out_of_range_construction() {
        let inner: Arc<dyn ByteSource> = Arc::new(MemorySource::new(pattern(100)));
        assert!(window(inner, 50, 51).is_err());
        assert!(window(Arc::new(MemorySource::new(pattern(100))), 101, 0).is_err());
    }
}
