//! 로컬 목 HTTP 서버로 OpenAPI 요청 구성/응답 파싱을 검증한다.
//!
//! 실제 `https://open.api.nexon.com` 호출은 API 키가 필요하므로,
//! 여기서는 같은 경로/헤더/JSON 스키마를 흉내 내는 서버를 띄운다.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use wz_openapi::http::{
    avatar_code_from_image_url, ApiError, CharacterBasic, OpenApiClient, OpenApiConfig, Region,
    TcpTransport,
};

struct Recorded {
    request_line: String,
    headers: Vec<String>,
}

fn serve_once(listener: &TcpListener, status: u16, body: &str) -> Recorded {
    let (stream, _) = listener.accept().unwrap();
    let recorded = read_request(&stream);
    respond(stream, status, body);
    recorded
}

fn read_request(stream: &TcpStream) -> Recorded {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap() == 0 {
            break;
        }
        let trimmed = line.trim_end().to_string();
        if trimmed.is_empty() {
            break;
        }
        headers.push(trimmed);
    }
    Recorded {
        request_line: request_line.trim_end().to_string(),
        headers,
    }
}

fn respond(mut stream: TcpStream, status: u16, body: &str) {
    let response = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        if status == 200 { "OK" } else { "Bad Request" },
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}

#[test]
fn ocid_and_character_basic_round_trip() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");

    let server = thread::spawn(move || {
        let first = serve_once(&listener, 200, r#"{"ocid":"ocid-123"}"#);
        let second = serve_once(
            &listener,
            200,
            r#"{
              "date": "2026-11-06T00:00:00+09:00",
              "character_name": "홍길동",
              "world_name": "스카니아",
              "character_class": "히어로",
              "character_class_level": "6",
              "character_level": 285,
              "character_exp": 123456789,
              "character_exp_rate": "12.345",
              "character_gender": "남",
              "character_popularity": 42,
              "character_image": "https://open.api.nexon.com/character/look/ABCDEFGHIJ?x=1&y=2"
            }"#,
        );
        vec![first, second]
    });

    let mut config = OpenApiConfig::new("mock-key", Region::Kms);
    config.base_url = base_url;
    let mut client = OpenApiClient::new(config, TcpTransport);

    let ocid = client.character_ocid("홍길동").unwrap();
    assert_eq!(ocid, "ocid-123");

    let basic: CharacterBasic = client.character_basic(&ocid).unwrap();
    assert_eq!(basic.character_name.as_deref(), Some("홍길동"));
    assert_eq!(basic.world_name.as_deref(), Some("스카니아"));
    assert_eq!(basic.character_class.as_deref(), Some("히어로"));
    assert_eq!(basic.character_level, Some(285));
    assert_eq!(basic.character_exp, Some(123456789));
    assert_eq!(basic.character_popularity, Some(42));
    assert_eq!(
        avatar_code_from_image_url(basic.character_image.as_deref().unwrap()),
        Some("ABCDEFGHIJ")
    );

    let recorded = server.join().unwrap();
    // 요청 URL: 리전 세그먼트 + 경로 + 쿼리(UTF-8 퍼센트 인코딩)
    assert_eq!(
        recorded[0].request_line,
        "GET /maplestory/v1/id?character_name=%ED%99%8D%EA%B8%B8%EB%8F%99 HTTP/1.1"
    );
    assert_eq!(
        recorded[1].request_line,
        "GET /maplestory/v1/character/basic?ocid=ocid-123 HTTP/1.1"
    );
    // 인증 헤더가 두 요청 모두에 실린다.
    assert!(recorded[0]
        .headers
        .iter()
        .any(|h| h.eq_ignore_ascii_case("x-nxopen-api-key: mock-key")));
    assert!(recorded[1]
        .headers
        .iter()
        .any(|h| h.eq_ignore_ascii_case("x-nxopen-api-key: mock-key")));
}

#[test]
fn api_error_envelope_is_surfaced() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        serve_once(
            &listener,
            400,
            r#"{"error":{"name":"OPENAPI00004","message":"Please input valid parameter"}}"#,
        )
    });

    let mut config = OpenApiConfig::new("mock-key", Region::Msea);
    config.base_url = format!("http://{addr}");
    let mut client = OpenApiClient::new(config, TcpTransport);

    let err = client.character_ocid("없는캐릭터").unwrap_err();
    match err {
        ApiError::Api(body) => {
            assert_eq!(body.name, "OPENAPI00004");
            assert_eq!(body.message, "Please input valid parameter");
        }
        other => panic!("unexpected error: {other}"),
    }

    let recorded = server.join().unwrap();
    assert!(recorded
        .request_line
        .starts_with("GET /maplestorysea/v1/id?"));
}
