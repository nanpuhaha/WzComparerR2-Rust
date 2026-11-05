//! C# `RC4CryptoServiceProvider` 실행 결과와 비교한다.

use serde::Deserialize;

#[derive(Deserialize)]
struct Vector {
    key: String,
    data: String,
    out: String,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn rc4_matches_csharp() {
    let vectors: Vec<Vector> = serde_json::from_str(include_str!("rc4_vectors.json")).unwrap();
    for v in vectors {
        let key = hex_decode(&v.key);
        let data = hex_decode(&v.data);
        let mut rc4 = wz_network::Rc4::new(&key).unwrap();
        assert_eq!(hex(&rc4.transform(&data)), v.out, "key {}", v.key);
    }
}

fn hex_decode(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}
