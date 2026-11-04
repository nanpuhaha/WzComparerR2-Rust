# WzComparerR2-Rust

MapleStory WZ 아카이브를 읽고, 비교하고, 렌더링하는 Rust 워크스페이스입니다.
C# 원본 [WzComparerR2](https://github.com/Kagamia/WzComparerR2)의 동작을
Rust 로 재구현하는 것을 목표로 합니다.

> **게임 에셋은 포함되어 있지 않습니다.** 이 저장소는 파서/렌더러 코드만
> 배포합니다. 실행하려면 사용자가 정당하게 보유한 게임 클라이언트의 `Data`
> 디렉터리가 필요합니다. MapleStory 및 관련 상표/저작권은 Nexon 에 있습니다.

## 구성

| 크레이트 | 역할 |
|---|---|
| `wz-core` | WZ 아카이브, 암호(AES/SNOW2/ChaCha20), 이미지 트리, 버전 감지 |
| `wz-image` | PNG/DXBC/BMP 디코딩, BC7 언팩 |
| `wz-db` | 노드 아레나, 문자열 링커, 캐릭터 스탯, 맵 데이터 |
| `wz-avatar` | 아바타 파트 로드, zmap, 프리즘, 프레임 합성 |
| `wz-render` | wgpu 기반 스프라이트/맵 렌더러 |
| `wz-diff` | 두 WZ 트리의 구조적 비교 |
| `wz-cli` (`wz`) | 헤드리스 CLI — 모든 기능의 기준 구현 |
| `wz-gui` | egui/eframe 데스크톱 GUI (PoC 단계) |
| `tools/*` | 합성 코퍼스 빌더, C# 오라클 차분 하네스 |

## 요구 사항

- Rust **1.90+** (`rust-toolchain.toml` 참고)
- Linux/macOS/Windows. GUI는 wgpu 백엔드(Vulkan/Metal/DX12)를 사용합니다.

## 빌드 및 테스트

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

GUI:

```bash
cargo build --manifest-path crates/wz-gui/Cargo.toml
```

## 게임 데이터 설정

소스에 절대 경로를 하드코딩하지 않습니다. 실제 WZ 파일을 읽는 테스트와 GUI
아바타 기능은 다음 환경 변수를 사용합니다.

```bash
export WZ_DATA_ROOT="/path/to/Nexon/Maple/Data"
```

- 미설정 시 실제 데이터가 필요한 테스트는 자동으로 skip 됩니다.
- GUI는 `WZ_DATA_ROOT` 가 없으면 상태 표시줄에 설정 안내를 출력합니다.

## 사용 예시

```bash
# 포맷/버전 감지
cargo run -p wz-cli -- detect "$WZ_DATA_ROOT/Base/Base_000.wz"

# 디렉터리 트리
cargo run -p wz-cli -- tree "$WZ_DATA_ROOT/Character/Character_000.wz"

# 이미지 추출
cargo run -p wz-cli -- image "$WZ_DATA_ROOT/Character/Character_000.wz" --out out.png

# 두 아카이브 비교
cargo run -p wz-cli -- diff old.wz new.wz
```

전체 명령은 `cargo run -p wz-cli -- --help` 로 확인하세요.

## 문서

- [`docs/known-bugs.md`](docs/known-bugs.md) — 원본 C# 버그와 포팅 시 수정 사항
- [`docs/oracle-spec.md`](docs/oracle-spec.md) — C# 오라클 차분 검증 스펙
- [`docs/differential.md`](docs/differential.md) — 차분 스크립트 사용법
- [`docs/gui-architecture.md`](docs/gui-architecture.md) — GUI 설계
- [`docs/architecture-audit.md`](docs/architecture-audit.md) — 모듈 구조 감사

## 보안

취약점 제보는 공개 이슈 대신 저장소 소유자에게 비공개로 연락해 주세요.
[`SECURITY.md`](SECURITY.md) 를 참고하세요.

## 라이선스

MIT. 원본 WzComparerR2 의 저작권 고지를 포함합니다. 자세한 내용은
[`LICENSE`](LICENSE) 와 [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md) 를
참고하세요.
