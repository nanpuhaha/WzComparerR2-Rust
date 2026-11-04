# Third-party notices

이 저장소는 아래 프로젝트의 코드/문서를 참고하거나 포함합니다. 각 프로젝트의
라이선스는 원 저작자의 것입니다.

## WzComparerR2 (C# 원본)

- 저장소: https://github.com/Kagamia/WzComparerR2
- 라이선스: MIT
- Copyright (c) 2015 Kagamia Studio

이 Rust 워크스페이스는 WzComparerR2 의 WZ 포맷 해석, 아바타 합성, 렌더링
동작을 Rust 로 재구현한 것입니다. `docs/` 의 여러 문서가 원본 C# 구현의
파일/라인을 참조합니다.

## mattpocock/skills

- 저장소: https://github.com/mattpocock/skills
- 라이선스: MIT
- Copyright (c) Matt Pocock

`.agents/skills/` 아래의 에이전트 스킬 문서는 위 저장소에서 가져온 것입니다.
`pr` 스킬의 `CREDITS.md` 는 Dex Horthy 의 `show-me` 스킬 출처를 별도로
명시합니다.

## Rust 크레이트

런타임/개발 의존성의 라이선스는 `Cargo.lock` 과 `deny.toml` 의 허용 목록
(MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode)으로 관리합니다.
전체 목록은 `cargo deny check licenses` 로 확인할 수 있습니다.

## 게임 에셋

이 저장소에는 MapleStory 게임 데이터(WZ 파일, 이미지, 사운드, 폰트 등)가
포함되어 있지 않습니다. 도구를 실행하려면 사용자가 직접 정당하게 설치한
게임 클라이언트의 `Data` 디렉터리를 `WZ_DATA_ROOT` 로 지정해야 합니다.
MapleStory 및 관련 상표/저작권은 Nexon 에 있습니다.
