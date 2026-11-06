//! DB2 BIN 컨테이너를 C# `BinaryWriter`/`BinaryReader` 결과와 비교한다.
//!
//! C# 하네스는 WinForms `DataGridView` 대신 행/셀 값을 직접 쓰지만
//! 직렬화는 실제 `BinaryWriter`를 쓴다(7비트 문자열 길이, LE int32).

use serde::Deserialize;
use wz_db::{BinCell, Db2BinGrid};

#[derive(Deserialize)]
struct Fixture {
    columns: i32,
    declared_rows: i32,
    bin_base64: String,
    parsed_rows: Vec<Vec<String>>,
    parsed_row_count: usize,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/db2_bin.json")).unwrap()
}

fn b64_decode(text: &str) -> Vec<u8> {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0u32;
    for byte in text.bytes().filter(|b| *b != b'=') {
        let value = T.iter().position(|c| *c == byte).expect("base64") as u32;
        acc = (acc << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

fn b64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn sample() -> Db2BinGrid {
    Db2BinGrid {
        columns: 2,
        rows: vec![
            vec![BinCell::Str("가".into()), BinCell::Bitmap(vec![1, 2, 3])],
            vec![BinCell::Str("나".into()), BinCell::Bitmap(vec![0xFF; 300])],
            vec![BinCell::Str("다".into()), BinCell::Bitmap(Vec::new())],
        ],
    }
}

#[test]
fn bytes_match_csharp_binary_writer() {
    let f = fixture();
    let grid = sample();
    assert_eq!(grid.columns, f.columns);
    assert_eq!(grid.rows.len() as i32, f.declared_rows);
    let bytes = grid.write_bin().unwrap();
    let expected = b64_decode(&f.bin_base64);
    assert_eq!(b64_encode(&bytes), f.bin_base64, "byte layout");
    assert_eq!(bytes, expected);
}

#[test]
fn read_matches_csharp_loadbin() {
    let f = fixture();
    let bytes = b64_decode(&f.bin_base64);
    let grid = Db2BinGrid::read_bin(&bytes).unwrap();
    assert_eq!(
        grid.rows.len(),
        f.parsed_row_count,
        "LoadBin drops last row"
    );
    for (row, want) in grid.rows.iter().zip(f.parsed_rows.iter()) {
        let got: Vec<String> = row
            .iter()
            .map(|cell| match cell {
                BinCell::Str(s) => s.clone(),
                BinCell::Bitmap(b) => b.iter().map(|v| format!("{v:02x}")).collect(),
            })
            .collect();
        assert_eq!(&got, want);
    }
}

#[test]
fn read_all_recovers_every_row() {
    let f = fixture();
    let bytes = b64_decode(&f.bin_base64);
    let (grid, declared) = Db2BinGrid::read_bin_all(&bytes).unwrap();
    assert_eq!(declared, f.declared_rows);
    assert_eq!(grid, sample());
}
