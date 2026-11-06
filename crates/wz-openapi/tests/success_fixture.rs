//! 성공(200) 응답 fixture로 파싱 경로를 검증한다.
//!
//! 실제 `https://open.api.nexon.com` 200 응답은 Nexon API 키가 필요해
//! 캡처하지 못했다. fixture는 공식 응답 스키마(문서화된 필드)를 따른다.
//! 요청 URL/헤더와 오류 봉투는 실서버로 검증되어 있다.

use wz_openapi::http::{
    avatar_code_from_image_url, parse_character_basic_json, parse_character_id_json, ApiError,
};

#[test]
fn character_basic_200_fixture_parses_all_fields() {
    let basic = parse_character_basic_json(include_str!("fixtures/nexon_character_basic_200.json"))
        .expect("valid 200 body");

    assert_eq!(basic.date.as_deref(), Some("2026-11-06T00:00:00+09:00"));
    assert_eq!(basic.character_name.as_deref(), Some("홍길동"));
    assert_eq!(basic.world_name.as_deref(), Some("스카니아"));
    assert_eq!(basic.character_class.as_deref(), Some("히어로"));
    assert_eq!(basic.character_class_level.as_deref(), Some("6"));
    assert_eq!(basic.character_level, Some(285));
    assert_eq!(basic.character_exp, Some(123_456_789_012));
    assert_eq!(basic.character_exp_rate.as_deref(), Some("12.345"));
    assert_eq!(basic.character_gender.as_deref(), Some("남"));
    assert_eq!(basic.character_popularity, Some(42));
    let image = basic.character_image.as_deref().unwrap();
    assert_eq!(
        avatar_code_from_image_url(image),
        Some("ABCDEFGHIJKLMNOPQRSTUVWXYZ")
    );
}

#[test]
fn character_id_200_fixture_parses_ocid() {
    let id = parse_character_id_json(r#"{"ocid":"ocid-123"}"#).unwrap();
    assert_eq!(id.ocid.as_deref(), Some("ocid-123"));
}

#[test]
fn success_parser_rejects_malformed_body() {
    assert!(matches!(
        parse_character_basic_json("not json"),
        Err(ApiError::Json(_))
    ));
}
