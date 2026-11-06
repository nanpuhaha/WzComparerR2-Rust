//! 실제 Nexon OpenAPI 엔드포인트에서 받은 오류 봉투를 파서가 해석하는지 확인한다.
//!
//! fixture는 2026-11-06에 `https://open.api.nexon.com`에 실제로 요청해 받은
//! 본문이다(성공 경로는 API 키가 필요해 제외).
//!
//! ```text
//! GET /maplestory/v1/id?character_name=test
//!   -> 400 {"error":{"name":"OPENAPI00004","message":"Please input valid parameter"}}
//! GET /maplestory/v1/id?character_name=test  (x-nxopen-api-key: test-invalid-key)
//!   -> 400 {"error":{"name":"OPENAPI00005","message":"The apikey is not valid."}}
//! ```

use wz_openapi::{parse_api_error, ApiError, ApiErrorBody, HttpResponse, OpenApiConfig, Region};

#[test]
fn missing_key_error_parses() {
    let body = include_str!("fixtures/nexon_error_no_key.json");
    let parsed = parse_api_error(body).expect("error envelope");
    assert_eq!(parsed.name, "OPENAPI00004");
    assert_eq!(parsed.message, "Please input valid parameter");
}

#[test]
fn invalid_key_error_parses() {
    let body = include_str!("fixtures/nexon_error_bad_key.json");
    let parsed = parse_api_error(body).expect("error envelope");
    assert_eq!(parsed.name, "OPENAPI00005");
    assert_eq!(parsed.message, "The apikey is not valid.");
}

#[test]
fn non_envelope_body_is_none() {
    assert!(parse_api_error(r#"{"ocid":"abc"}"#).is_none());
    assert!(parse_api_error("not json").is_none());
}

#[test]
fn request_shape_matches_captured_endpoint() {
    // 위 curl이 실제로 호출한 URL과 클라이언트가 만드는 URL이 같아야 한다.
    let cfg = OpenApiConfig::new("test-invalid-key", Region::Kms);
    assert_eq!(
        cfg.url("/v1/id", &[("character_name", "test")]),
        "https://open.api.nexon.com/maplestory/v1/id?character_name=test"
    );
    assert_eq!(
        cfg.headers(),
        vec![(
            "x-nxopen-api-key".to_string(),
            "test-invalid-key".to_string()
        )]
    );
}

/// 400 응답을 `ApiError::Api`로 승격하는 경로(봉투 파싱) 확인.
#[test]
fn http_400_envelope_maps_to_api_error() {
    let response = HttpResponse {
        status: 400,
        body: include_str!("fixtures/nexon_error_no_key.json").to_string(),
    };
    let error = parse_api_error(&response.body).map(ApiError::Api);
    match error {
        Some(ApiError::Api(ApiErrorBody { name, .. })) => assert_eq!(name, "OPENAPI00004"),
        other => panic!("unexpected: {other:?}"),
    }
}
