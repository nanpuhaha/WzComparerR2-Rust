//! Nexon OpenAPI HTTP 계층 — 요청 URL/헤더 구성과 응답 파싱.
//!
//! 실제 엔드포인트는 `https://open.api.nexon.com` 이고 헤더는
//! `x-nxopen-api-key` 다(원본은 `MapleStory.OpenAPI` NuGet 패키지가 전송).
//! 이 모듈은 전송을 [`Transport`]로 분리해, 테스트에서는 로컬 목 HTTP 서버를
//! 쓰고 실제 HTTPS는 외부 클라이언트를 주입하도록 한다.

use serde::{Deserialize, Serialize};

/// 리전별 경로 세그먼트.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Kms,
    Msea,
    Tms,
}

impl Region {
    pub fn segment(self) -> &'static str {
        match self {
            Region::Kms => "maplestory",
            Region::Msea => "maplestorysea",
            Region::Tms => "maplestorytw",
        }
    }
}

/// 요청 설정. `base_url`은 목 서버 테스트를 위해 바꿀 수 있다.
#[derive(Debug, Clone)]
pub struct OpenApiConfig {
    pub base_url: String,
    pub api_key: String,
    pub region: Region,
}

impl OpenApiConfig {
    pub fn new(api_key: impl Into<String>, region: Region) -> Self {
        Self {
            base_url: "https://open.api.nexon.com".to_string(),
            api_key: api_key.into(),
            region,
        }
    }

    /// `{base}/{segment}{path}` (+ 쿼리).
    pub fn url(&self, path: &str, query: &[(&str, &str)]) -> String {
        let mut url = format!(
            "{}/{}{}",
            self.base_url.trim_end_matches('/'),
            self.region.segment(),
            path
        );
        if !query.is_empty() {
            url.push('?');
            url.push_str(
                &query
                    .iter()
                    .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
                    .collect::<Vec<_>>()
                    .join("&"),
            );
        }
        url
    }

    /// 원본이 붙이는 인증 헤더.
    pub fn headers(&self) -> Vec<(String, String)> {
        vec![("x-nxopen-api-key".to_string(), self.api_key.clone())]
    }
}

/// `application/x-www-form-urlencoded` 규칙의 최소 퍼센트 인코딩.
pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Debug)]
pub enum TransportError {
    Io(String),
    Malformed(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::Io(e) => write!(f, "io: {e}"),
            TransportError::Malformed(e) => write!(f, "malformed: {e}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// HTTP GET 전송. 실제 HTTPS는 이 트레이트 구현을 주입한다.
pub trait Transport {
    fn get(
        &mut self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<HttpResponse, TransportError>;
}

/// `http://` 만 지원하는 표준 TCP 전송(로컬 목 서버/테스트용).
pub struct TcpTransport;

impl Transport for TcpTransport {
    fn get(
        &mut self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<HttpResponse, TransportError> {
        use std::io::{Read, Write};
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| TransportError::Malformed(format!("not http: {url}")))?;
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let host = authority.split(':').next().unwrap_or(authority);
        let addr = if authority.contains(':') {
            authority.to_string()
        } else {
            format!("{authority}:80")
        };
        let mut stream =
            std::net::TcpStream::connect(&addr).map_err(|e| TransportError::Io(e.to_string()))?;
        let mut request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\n");
        for (name, value) in headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("Connection: close\r\n\r\n");
        stream
            .write_all(request.as_bytes())
            .map_err(|e| TransportError::Io(e.to_string()))?;
        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .map_err(|e| TransportError::Io(e.to_string()))?;
        let text = String::from_utf8_lossy(&raw).to_string();
        let (head, body) = text
            .split_once("\r\n\r\n")
            .ok_or_else(|| TransportError::Malformed("missing header terminator".into()))?;
        let status = head
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .ok_or_else(|| TransportError::Malformed("missing status line".into()))?;
        Ok(HttpResponse {
            status,
            body: body.to_string(),
        })
    }
}

/// `error` 봉투.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub name: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: ApiErrorBody,
}

/// 응답 본문에서 `error` 봉투를 꺼낸다. 봉투가 없으면 `None`.
pub fn parse_api_error(body: &str) -> Option<ApiErrorBody> {
    serde_json::from_str::<ErrorEnvelope>(body)
        .ok()
        .map(|envelope| envelope.error)
}

#[derive(Debug)]
pub enum ApiError {
    /// Nexon API 오류 봉투(`OPENAPI00004` 등).
    Api(ApiErrorBody),
    /// HTTP 실패지만 봉투가 없는 경우.
    Status(u16, String),
    Json(String),
    Transport(TransportError),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Api(e) => write!(f, "{}: {}", e.name, e.message),
            ApiError::Status(code, body) => write!(f, "http {code}: {body}"),
            ApiError::Json(e) => write!(f, "json: {e}"),
            ApiError::Transport(e) => write!(f, "transport: {e}"),
        }
    }
}

impl std::error::Error for ApiError {}

/// `/v1/character/basic` 응답.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterBasic {
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub character_name: Option<String>,
    #[serde(default)]
    pub world_name: Option<String>,
    #[serde(default)]
    pub character_class: Option<String>,
    #[serde(default)]
    pub character_class_level: Option<String>,
    #[serde(default)]
    pub character_level: Option<i32>,
    #[serde(default)]
    pub character_exp: Option<i64>,
    #[serde(default)]
    pub character_exp_rate: Option<String>,
    #[serde(default)]
    pub character_gender: Option<String>,
    #[serde(default)]
    pub character_popularity: Option<i32>,
    /// `...?look=...` 를 포함한 이미지 URL. 아바타 코드 추출에 쓴다.
    #[serde(default)]
    pub character_image: Option<String>,
}

/// `/v1/id` 응답.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterId {
    #[serde(default)]
    pub ocid: Option<String>,
}

pub struct OpenApiClient<T: Transport> {
    pub config: OpenApiConfig,
    transport: T,
}

impl<T: Transport> OpenApiClient<T> {
    pub fn new(config: OpenApiConfig, transport: T) -> Self {
        Self { config, transport }
    }

    fn parse<R: for<'de> Deserialize<'de>>(&mut self, url: &str) -> Result<R, ApiError> {
        let headers = self.config.headers();
        let response = self
            .transport
            .get(url, &headers)
            .map_err(ApiError::Transport)?;
        if !(200..300).contains(&response.status) {
            if let Some(error) = parse_api_error(&response.body) {
                return Err(ApiError::Api(error));
            }
            return Err(ApiError::Status(response.status, response.body));
        }
        serde_json::from_str(&response.body).map_err(|e| ApiError::Json(e.to_string()))
    }

    /// `/v1/id?character_name=` — 캐릭터명 → OCID.
    pub fn character_ocid(&mut self, character_name: &str) -> Result<String, ApiError> {
        let url = self
            .config
            .url("/v1/id", &[("character_name", character_name)]);
        let id: CharacterId = self.parse(&url)?;
        id.ocid
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ApiError::Status(200, "missing ocid".into()))
    }

    /// `/v1/character/basic?ocid=`
    pub fn character_basic(&mut self, ocid: &str) -> Result<CharacterBasic, ApiError> {
        let url = self.config.url("/v1/character/basic", &[("ocid", ocid)]);
        self.parse(&url)
    }
}

/// `/v1/id` 응답 본문 파서(성공 경로).
pub fn parse_character_id_json(body: &str) -> Result<CharacterId, ApiError> {
    serde_json::from_str(body).map_err(|e| ApiError::Json(e.to_string()))
}

/// `/v1/character/basic` 응답 본문 파서(성공 경로).
pub fn parse_character_basic_json(body: &str) -> Result<CharacterBasic, ApiError> {
    serde_json::from_str(body).map_err(|e| ApiError::Json(e.to_string()))
}

/// `character_image` URL의 `look/<DATA>`에서 아바타 코드를 꺼낸다.
pub fn avatar_code_from_image_url(url: &str) -> Option<&str> {
    let index = url.find("look/")?;
    let rest = &url[index + "look/".len()..];
    let end = rest
        .find(|c: char| !c.is_ascii_uppercase())
        .unwrap_or(rest.len());
    (end > 0).then_some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_region_urls() {
        let cfg = OpenApiConfig::new("KEY", Region::Kms);
        assert_eq!(
            cfg.url("/v1/character/basic", &[("ocid", "abc")]),
            "https://open.api.nexon.com/maplestory/v1/character/basic?ocid=abc"
        );
        let cfg = OpenApiConfig::new("KEY", Region::Tms);
        assert!(cfg.url("/v1/id", &[]).ends_with("/maplestorytw/v1/id"));
        assert_eq!(
            cfg.headers(),
            vec![("x-nxopen-api-key".into(), "KEY".into())]
        );
    }

    #[test]
    fn encodes_korean_names() {
        assert_eq!(percent_encode("홍길동"), "%ED%99%8D%EA%B8%B8%EB%8F%99");
        assert_eq!(percent_encode("a b-c_d.e~f"), "a%20b-c_d.e~f");
    }

    #[test]
    fn extracts_avatar_code() {
        assert_eq!(
            avatar_code_from_image_url("https://open.api.nexus.com/character/look/ABCDEF?x=1"),
            Some("ABCDEF")
        );
        assert_eq!(avatar_code_from_image_url("no-code"), None);
    }
}
