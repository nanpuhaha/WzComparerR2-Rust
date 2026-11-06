//! DB2 그리드 BIN 컨테이너 (`DataGrid.SaveBin`/`LoadBin`).
//!
//! 형식(`BinaryWriter` 기준):
//! ```text
//! int32 columns
//! int32 rows
//! rows x columns:
//!   string tag        // "str" | "Bitmap" (7비트 길이 + UTF-8)
//!   str    -> string value
//!   Bitmap -> int32 length, byte[length]
//! ```
//!
//! 원본의 두 가지 특이점:
//! 1. `SaveBin`은 `string`/`Bitmap` 셀만 쓰고, `LoadBin`은 모든 셀에 대해
//!    태그를 읽는다. 그 외 타입이 섞이면 스트림이 어긋난다.
//! 2. `LoadBin`은 `for (i = 0; i <= m - 2; i++)`라서 **마지막 행을 읽지 않는다**.
//!
//! 또한 원본은 PNG를 `MemoryStream.GetBuffer()`로 써서 용량(capacity)만큼
//! 0 패딩이 붙는다. capacity는 GDI+가 스트림에 쓰는 청크 크기에 따라 달라져
//! 플랫폼 의존이므로, 여기서는 **정확한 페이로드 길이**를 쓴다(문서 참고).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinCell {
    Str(String),
    Bitmap(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2BinGrid {
    pub columns: i32,
    pub rows: Vec<Vec<BinCell>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BinError {
    Truncated,
    BadTag(String),
    CellsMismatch { row: usize, expected: i32 },
}

impl std::fmt::Display for BinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinError::Truncated => write!(f, "truncated"),
            BinError::BadTag(tag) => write!(f, "bad cell tag: {tag}"),
            BinError::CellsMismatch { row, expected } => {
                write!(f, "row {row} has != {expected} cells")
            }
        }
    }
}

impl std::error::Error for BinError {}

fn write_7bit_len(out: &mut Vec<u8>, mut value: usize) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn write_string(out: &mut Vec<u8>, text: &str) {
    write_7bit_len(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

fn write_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], BinError> {
        let end = self.pos.checked_add(n).ok_or(BinError::Truncated)?;
        let slice = self.data.get(self.pos..end).ok_or(BinError::Truncated)?;
        self.pos = end;
        Ok(slice)
    }

    fn i32(&mut self) -> Result<i32, BinError> {
        let bytes = self.take(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn string(&mut self) -> Result<String, BinError> {
        let mut shift = 0u32;
        let mut len = 0usize;
        loop {
            let byte = self.take(1)?[0];
            len |= ((byte & 0x7F) as usize)
                .checked_shl(shift)
                .ok_or(BinError::Truncated)?;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 35 {
                return Err(BinError::Truncated);
            }
        }
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| BinError::Truncated)
    }
}

impl Db2BinGrid {
    /// `SaveBin` 레이아웃 그대로 직렬화한다.
    pub fn write_bin(&self) -> Result<Vec<u8>, BinError> {
        let mut out = Vec::new();
        write_i32(&mut out, self.columns);
        write_i32(&mut out, self.rows.len() as i32);
        for (row_index, row) in self.rows.iter().enumerate() {
            if row.len() as i32 != self.columns {
                return Err(BinError::CellsMismatch {
                    row: row_index,
                    expected: self.columns,
                });
            }
            for cell in row {
                match cell {
                    BinCell::Str(text) => {
                        write_string(&mut out, "str");
                        write_string(&mut out, text);
                    }
                    BinCell::Bitmap(data) => {
                        write_string(&mut out, "Bitmap");
                        write_i32(&mut out, data.len() as i32);
                        out.extend_from_slice(data);
                    }
                }
            }
        }
        Ok(out)
    }

    /// `LoadBin`과 동일하게 **마지막 행을 건너뛰고** 읽는다.
    pub fn read_bin(bytes: &[u8]) -> Result<Self, BinError> {
        let (grid, declared) = Self::read_bin_all(bytes)?;
        let keep = declared.saturating_sub(1) as usize;
        let mut grid = grid;
        grid.rows.truncate(keep);
        Ok(grid)
    }

    /// 같은 스트림을 마지막 행까지 읽는다(원본 버그를 피하는 경로).
    pub fn read_bin_all(bytes: &[u8]) -> Result<(Self, i32), BinError> {
        let mut reader = Reader {
            data: bytes,
            pos: 0,
        };
        let columns = reader.i32()?;
        let declared = reader.i32()?;
        let mut rows = Vec::new();
        for row_index in 0..declared.max(0) as usize {
            let mut row = Vec::with_capacity(columns.max(0) as usize);
            for _ in 0..columns.max(0) {
                let tag = reader.string()?;
                match tag.as_str() {
                    "str" => row.push(BinCell::Str(reader.string()?)),
                    "Bitmap" => {
                        let len = reader.i32()?;
                        if len < 0 {
                            return Err(BinError::Truncated);
                        }
                        row.push(BinCell::Bitmap(reader.take(len as usize)?.to_vec()));
                    }
                    other => return Err(BinError::BadTag(other.to_string())),
                }
            }
            let _ = row_index;
            rows.push(row);
        }
        Ok((Db2BinGrid { columns, rows }, declared))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Db2BinGrid {
        Db2BinGrid {
            columns: 2,
            rows: vec![
                vec![BinCell::Str("가".into()), BinCell::Bitmap(vec![1, 2, 3])],
                vec![BinCell::Str("나".into()), BinCell::Bitmap(vec![0xFF; 300])],
            ],
        }
    }

    #[test]
    fn header_and_string_encoding() {
        let grid = Db2BinGrid {
            columns: 1,
            rows: vec![vec![BinCell::Str("ab".into())]],
        };
        let bytes = grid.write_bin().unwrap();
        assert_eq!(&bytes[0..4], &1i32.to_le_bytes());
        assert_eq!(&bytes[4..8], &1i32.to_le_bytes());
        // "str" 7비트 길이(3) + UTF-8, 그 다음 "ab"
        assert_eq!(&bytes[8..12], &[3, b's', b't', b'r']);
        assert_eq!(&bytes[12..15], &[2, b'a', b'b']);
    }

    #[test]
    fn bitmap_cell_has_length_prefix() {
        let grid = Db2BinGrid {
            columns: 1,
            rows: vec![vec![BinCell::Bitmap(vec![9, 8])]],
        };
        let bytes = grid.write_bin().unwrap();
        // 7비트 길이(6) + "Bitmap"
        assert_eq!(&bytes[8..15], &[6, b'B', b'i', b't', b'm', b'a', b'p']);
        assert_eq!(&bytes[15..19], &2i32.to_le_bytes());
        assert_eq!(&bytes[19..21], &[9, 8]);
    }

    #[test]
    fn loadbin_drops_last_row_like_original() {
        let grid = sample();
        let bytes = grid.write_bin().unwrap();
        let (all, declared) = Db2BinGrid::read_bin_all(&bytes).unwrap();
        assert_eq!(declared, 2);
        assert_eq!(all, grid);
        let restored = Db2BinGrid::read_bin(&bytes).unwrap();
        assert_eq!(restored.rows.len(), 1, "LoadBin drops the last row");
        assert_eq!(restored.rows[0], grid.rows[0]);
    }

    #[test]
    fn rejects_short_and_unknown() {
        assert!(Db2BinGrid::read_bin(&[1, 0, 0, 0]).is_err());
        let mut bytes = Db2BinGrid {
            columns: 1,
            rows: vec![vec![BinCell::Str("x".into())]],
        }
        .write_bin()
        .unwrap();
        bytes[8] = 3;
        bytes[9] = b'z';
        bytes[10] = b'z';
        bytes[11] = b'z';
        assert_eq!(
            Db2BinGrid::read_bin(&bytes).unwrap_err(),
            BinError::BadTag("zzz".into())
        );
    }
}
