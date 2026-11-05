# Public release checklist — v0.1

이 문서는 저장소를 public 으로 전환하기 전에 확인한 항목과, 사람이 마지막으로
눈으로 확인해야 할 항목을 구분한다.

## 완료 (2026-11-05)

- [x] `LICENSE` (MIT + Kagamia Studio 원본 고지)
- [x] `THIRD-PARTY-NOTICES.md` (원본 WzComparerR2, Rust 크레이트)
- [x] `README.md`, `SECURITY.md`
- [x] 게임 에셋 미포함 — `Data`/`*.log` gitignore, `WZ_DATA_ROOT` 만 사용
- [x] `.scratch/`, `.agents/skills/` 추적 및 히스토리 제거
- [x] 하드코딩된 `/Users/...` 경로 제거
- [x] `Cargo.lock` 및 `crates/wz-gui/Cargo.lock` 커밋
- [x] CI: `cargo fmt --check`, `cargo test --workspace`, `cargo clippy -D warnings`,
      `cargo deny check`, `cargo audit`, GUI build/clippy
- [x] GitHub Actions 커밋 SHA 핀 고정, `permissions: contents: read`
- [x] Dependabot 설정 (cargo ×2, actions; PR 3개 제한 + minor/patch 그룹)
- [x] Dependabot **vulnerability alerts** 및 **automated security fixes** 활성화
- [x] `main` 브랜치 보호: force push / branch deletion 금지, conversation resolution
- [x] 저장소 설명/topics 설정, wiki 비활성화
- [x] 이전 저장소 `WzComparerR2-Rust-archive` 를 private 로 보관
- [x] 새 저장소 `main` 히스토리에서 `.scratch`/`.agents`/`Data`/`wzdiag.log`/`jwseo` 0건
- [x] `GLOSSARY.md` + `docs/adr/0001~0006`
- [x] wayfinder 맵/티켓 로컬 기록 (`.scratch/wayfinder/`)

## 공개 직전 수동 확인

- [x] 열린 Dependabot PR 검토: `crc32fast`/`memmap2` 패치 PR #3 머지, 메이저 범프 8개는 v0.1.0 이후로 닫음
- [x] `README` 의 기능 설명이 현재 코드와 일치하는지 최종 확인
- [x] `docs/known-bugs.md`/`docs/oracle-spec.md` 의 핀 커밋(`bb98c25…`)이 실제 C# 커밋과 일치하는지 확인
- [x] `WzComparerR2-Rust-archive` 는 private 로 보관 유지 (이슈 32개/PR #30 보존)
- [x] GitHub 저장소 visibility 를 `public` 으로 전환
- [x] `v0.1.0` 태그/릴리스 노트 생성 (`d9c4984`)

## 공개 후

- [x] `cargo install --path crates/wz-cli --debug --locked` 스모크 테스트: `wz 0.1.0`, `oracle expr 1+1 → 2`, `tree` 동작 확인
- [ ] 외부 기여 가이드(이슈/PR 템플릿) 필요 여부 결정
- [ ] wayfinder 맵을 GitHub 이슈로 옮길지 결정
