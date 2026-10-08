//! WzComparerR2 전 크레이트 공통 에러 타입.
//!
//! # 왜 독립 크레이트인가
//!
//! 리포트 ADR-2 / §3 S-6: C# 원본은 실패를 `null`로 흘려보내서 "데이터가 없다"와
//! "복호화에 실패했다"를 구분하지 못했고, 일부는 `catch {}`로 조용히 삼켰다.
//! 여기서는 모든 실패 경로가 `Result`로 강제되고, 원인이 구조화되어 남는다.
//!
//! GUI는 이 값을 **표시할 수는 있지만 발생시키지는 않는다** (리포트 §4.1 원칙 5).

use std::path::PathBuf;

/// WZ 파일 I/O 계층에서 발생한 오류.
#[derive(Debug, thiserror::Error)]
pub enum WzIoError {
    #[error("파일 접근 실패: {path}")]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("파일 접근 실패: {0}")]
    OpenBare(#[source] std::io::Error),

    #[error("읽기 범위를 벗어뜀: offset={offset} len={len} (파일 크기 {file_len})")]
    OutOfBounds {
        offset: u64,
        len: usize,
        file_len: u64,
    },

    #[error("읽기 실패 (offset={offset}, len={len})")]
    Read {
        offset: u64,
        len: usize,
        #[source]
        source: std::io::Error,
    },

    #[error("mmap 실패: {0} — 파일 소스로 폴백하지 못했습니다")]
    Mmap(#[source] std::io::Error),
}

/// 컨테이너/이미지 바이트 파싱 중 발생한 오류.
///
/// `#[non_exhaustive]`가 아니다 — 매칭이 누락되도록 강제하기 위해서다.
#[derive(Debug, thiserror::Error)]
pub enum WzFormatError {
    /// 매직 바이트가 어떤 알려진 포맷과도 일치하지 않음.
    #[error("알 수 없는 파일 포맷: 매직 {magic:02X?} (오프셋 {offset})")]
    UnknownMagic { magic: [u8; 4], offset: u64 },

    /// 지원하지 않는 포맷 세대. 새 클라이언트가 새 난독화를 도입한 경우.
    ///
    /// 리포트 §12 R3: 이 경우 크레이트를 고치는 게 아니라 `FormatProfile`을
    /// 추가해야 한다. 오류 메시지가 그 방향을 명시한다.
    #[error("지원하지 않는 포맷 프로파일: {profile} (파일 {path_hint})")]
    UnsupportedProfile { profile: String, path_hint: String },

    /// 헤더 레이아웃이 구조적으로 맞지 않음.
    #[error("잘못된 {what}: offset={offset}")]
    Malformed { what: &'static str, offset: u64 },

    /// 확장 객체(중첩 Property 등)가 선언된 길이만큼 소비되지 않음.
    ///
    /// C# `Wz_Image.cs:519-527`가 검증하던 조건. 반드시 재현할 것.
    #[error("확장 객체 길이 불일치: 선언={declared} 실제={actual} (offset={offset})")]
    ObjectLengthMismatch {
        declared: u32,
        actual: u64,
        offset: u64,
    },

    /// UOL 해석이 깊이 상한을 초과. C# `Wz_Node.cs:549-558`에 사이클 감지가
    /// 없어 자기참조 UOL이 무한 루프를 만든다 (리포트 §14.4 #18).
    #[error("UOL 해석 깊이 초과 (상한 {limit}), 순환 참조 가능성: {path}")]
    UolDepthExceeded { limit: usize, path: String },

    #[error("UOL 해석 실패: {path}")]
    UolUnresolved { path: String },

    #[error("{0}")]
    Crypto(#[from] WzCryptoError),
}

#[derive(Debug, thiserror::Error)]
pub enum WzCryptoError {
    #[error("알 수 없는 암호 키 종류: {0}")]
    UnknownKeyType(u8),

    #[error("SNOW2 키 길이 오류: {0} (16 또는 32 바이트여야 함)")]
    Snow2KeyLen(usize),

    #[error("ChaCha20 키 길이 오류: {0} (32 바이트여야 함)")]
    ChaCha20KeyLen(usize),

    #[error("해시 검증 실패: 기대 {expected}, 실제 {actual}")]
    HashMismatch { expected: u64, actual: u64 },

    #[error("문자열 복호화 결과가 잘못된 인코딩입니다: {reason}")]
    StringDecode { reason: String },
}

#[derive(Debug, thiserror::Error)]
pub enum WzImageError {
    #[error("지원하지 않는 텍스처 포맷: {0} (코드 {1})")]
    UnsupportedFormat(String, u32),

    #[error("zlib 헤더 불일치: 기대 0x789C, 실제 {0:02X?}")]
    ZlibHeader([u8; 2]),

    #[error("압축 해제 실패 (offset={offset})")]
    Inflate {
        offset: u64,
        #[source]
        source: std::io::Error,
    },

    #[error("페이지 범위 초과: 요청 {requested}, 보유 {available}")]
    PageOutOfRange { requested: u32, available: u32 },

    #[error("이미지 크기 오버플로: {w}x{h} (format {format}, scale {scale})")]
    SizeOverflow {
        w: u32,
        h: u32,
        format: u32,
        scale: u32,
    },
}

/// WZ 관련 전체 오류. 도메인/앱 레이어가 쓰는 배합 타입.
#[derive(Debug, thiserror::Error)]
pub enum WzError {
    #[error(transparent)]
    Io(#[from] WzIoError),

    #[error(transparent)]
    Format(#[from] WzFormatError),

    #[error(transparent)]
    Image(#[from] WzImageError),

    /// 직접 `?` 로 쓸 수 있도록 배합 변형에도 크립토 에러를 노출한다.
    #[error(transparent)]
    Crypto(#[from] WzCryptoError),

    /// 노드 경로로 찾지 못함. "없다"와 "실패했다"를 구분하기 위한 명시적 변형.
    #[error("WZ 노드를 찾을 수 없음: {0}")]
    NotFound(String),

    #[error("WZ 파일이 열려 있지 않음: {0}")]
    NotOpen(String),

    /// 진행 중 취소 (diff/패치의 장시간 작업).
    #[error("작업이 취소됨")]
    Cancelled,
}

pub type Result<T, E = WzError> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_informative() {
        // 진단 가능성이 이 enum의 존재 이유다 (ADR-2).
        let e = WzError::Format(WzFormatError::ObjectLengthMismatch {
            declared: 100,
            actual: 96,
            offset: 0x1234,
        });
        let s = e.to_string();
        assert!(s.contains("100"), "{s}");
        assert!(s.contains("96"), "{s}");
        assert!(s.contains("4660"), "{s}"); // 0x1234
    }

    #[test]
    fn source_chain_is_preserved() {
        use std::error::Error;
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let e = WzError::Io(WzIoError::Open {
            path: PathBuf::from("Data.wz"),
            source: io,
        });
        assert!(e.source().is_some(), "I/O 원인 소스가 보존되어야 한다");
    }

    #[test]
    fn not_found_is_distinct_from_failure() {
        // S-6의 핵심 요구: 두 상태가 타입 수준에서 구분돼야 한다.
        let nf: WzError = WzError::NotFound("Character/00002000.img".into());
        let fmt: WzError = WzFormatError::Malformed {
            what: "header",
            offset: 0,
        }
        .into();
        assert!(matches!(nf, WzError::NotFound(_)));
        assert!(!matches!(fmt, WzError::NotFound(_)));
    }
}
