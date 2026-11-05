//! C# `NexonOpenAPI.Utils` 실행 결과와 비교한다.
//!
//! fixture는 `tools/csharp-worldmap openapi <text>`의 출력이다.

use serde::Deserialize;

#[derive(Deserialize)]
struct Vector {
    text: String,
    decrypt: Option<String>,
    decrypt_len: usize,
    check_ver: i32,
    item_id: Option<String>,
}

#[test]
fn decrypt_and_helpers_match_csharp() {
    let vectors: Vec<Vector> = serde_json::from_str(include_str!("openapi_vectors.json")).unwrap();
    for v in vectors {
        let dec = wz_openapi::decrypt(&v.text);
        match (&v.decrypt, &dec) {
            (None, None) => {}
            (Some(hex), Some(bytes)) => {
                let got: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                assert_eq!(&got, hex, "decrypt {}", v.text);
                assert_eq!(bytes.len(), v.decrypt_len);
                assert_eq!(
                    wz_openapi::check_ver(bytes),
                    v.check_ver,
                    "check_ver {}",
                    v.text
                );
            }
            _ => panic!("decrypt mismatch for {}", v.text),
        }
        assert_eq!(
            wz_openapi::get_item_id(&v.text),
            v.item_id,
            "item_id {}",
            v.text
        );
    }
}
