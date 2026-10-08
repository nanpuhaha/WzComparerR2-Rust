//! 문자열 정렬/비교.
//!
//! 현재는 자연 정렬만 (Phase 4 에서 MS 마크업 파서와 CJK 레이아웃이 추가된다).

pub mod natural;

pub use natural::{natural_cmp, natural_key, sort_natural};
