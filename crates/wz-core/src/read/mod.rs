//! WZ 바이너리 읽기 프리미티브.
//!
//! 원본 `WzComparerR2.WzLib/WzBinaryReader.cs` 의 대응.
//! 게임 규칙은 여기 없다 — 포맷 규칙만.

pub mod int;
pub mod string;

pub use int::{
    encode_compressed_i32, encode_compressed_i64, encode_compressed_u32, read_compressed_i32,
    read_compressed_i64, read_compressed_u32, read_compressed_u64,
};
pub use string::{
    apply_ascii_mask, apply_utf16_mask, encode_ascii_wz_string, encode_empty,
    encode_pkg2_dir_string, encode_pkg2_dir_string_v2, encode_utf16_wz_string,
    image_string_header_len, read_image_string, read_pkg2_dir_string, read_pkg2_dir_string_v2,
    StrEncoding, WzString, WZ_UTF16_ESCAPE_AT, WZ_UTF16_ESCAPE_MARKER,
};
