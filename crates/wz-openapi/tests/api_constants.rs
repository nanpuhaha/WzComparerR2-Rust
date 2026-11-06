//! 사용하는 경로/헤더가 원본 `MapleStory.OpenAPI` 패키지의 상수와 맞는지 확인한다.
//!
//! fixture는 NuGet 패키지 DLL에서 추출한 문자열이다(스크립트는
//! `docs/parity-inventory.md` 참고).

use serde::Deserialize;

#[derive(Deserialize)]
struct Constants {
    package: String,
    base_url: String,
    header: Option<String>,
    region_segments: Vec<String>,
    paths: Vec<String>,
}

fn constants() -> Constants {
    serde_json::from_str(include_str!("fixtures/nexon_api_paths.json")).unwrap()
}

#[test]
fn used_paths_and_header_exist_in_original_package() {
    let c = constants();
    assert!(c.package.contains("MapleStory.OpenAPI"));
    assert!(c.base_url.contains("open.api.nexon.com"));
    assert_eq!(c.header.as_deref(), Some("x-nxopen-api-key"));
    for path in ["/v1/id", "/v1/character/basic"] {
        assert!(
            c.paths.iter().any(|p| p == path),
            "{path} not found in {:?}",
            c.paths.len()
        );
    }
    for segment in ["maplestory", "maplestorysea", "maplestorytw"] {
        assert!(c.region_segments.iter().any(|s| s == segment), "{segment}");
    }
}

#[test]
fn region_segments_match_client() {
    use wz_openapi::http::{OpenApiConfig, Region};
    let c = constants();
    for (region, segment) in [
        (Region::Kms, "maplestory"),
        (Region::Msea, "maplestorysea"),
        (Region::Tms, "maplestorytw"),
    ] {
        assert_eq!(region.segment(), segment);
        assert!(c.region_segments.iter().any(|s| s == segment));
        let cfg = OpenApiConfig::new("k", region);
        assert!(cfg
            .url("/v1/id", &[])
            .contains(&format!("/{segment}/v1/id")));
    }
}
