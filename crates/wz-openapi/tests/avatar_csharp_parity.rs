//! C# `Utils.Unpack` 결과와 비교한다. 구조 자체도 C#에서 덤프했다.

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Case {
    version: i32,
    pack: String,
    rows: usize,
    sha256: String,
}

#[derive(Deserialize)]
struct Vectors {
    packs: BTreeMap<String, String>,
    cases: Vec<Case>,
}

fn hex_decode(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[test]
fn unpack_matches_csharp() {
    let vectors: Vectors = serde_json::from_str(include_str!("avatar_vectors.json")).unwrap();
    for case in &vectors.cases {
        let pack = hex_decode(&vectors.packs[&case.pack]);
        let (_, _, fields) = wz_openapi::avatar::unpack(case.version, &pack);
        assert_eq!(fields.len(), case.rows, "version {} rows", case.version);
        let canon = fields
            .iter()
            .map(|f| format!("{}\t{}\t{}", f.name, f.bits, f.value))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            sha256_hex(canon.as_bytes()),
            case.sha256,
            "version {} pack {}",
            case.version,
            case.pack
        );
    }
}
