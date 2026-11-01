#!/usr/bin/env bash
#
# scripts/differential.sh — Rust `wz oracle` 와 C# `WzComparerR2.Oracle` 의 차분 실행.
#
# 사용: scripts/differential.sh <op> [args...]
#   <op> 뒤의 인자는 **양쪽에 그대로** 전달된다.
#   도구 옵션(--csharp-repo 등)은 반드시 <op> **앞에** 둔다. 뒤에 두면 오라클로 넘겨간다.
#
#   scripts/differential.sh --csharp-repo /path/to/WzComparerR2 tree --wz A.wz
#   scripts/differential.sh --csharp-repo /path/to/WzComparerR2 expr --expr "1/3 + 2*3"
#
# 환경 변수:
#   WZ_CSHARP_REPO   C# 저장소 루트 (= --csharp-repo)
#   WZ_CSHARP_DLL    빌드된 WzComparerR2.Oracle.dll (= --csharp-dll)
#   WZ_BIN           빌드된 `wz` 실행 파일 (= --wz-bin)
#   WZ_DIFF_OUT      산출물 디렉터리 (= --out-dir)
#
# 종료 코드: 0 일치 / 1 불일치 / 2 도구 사용법 오류 / 3 비교 불가 / 4 하네스 오류
#
# Windows 에서는 Git Bash 나 WSL 로 돌린다:
#   "C:\Program Files\Git\bin\bash.exe" scripts/differential.sh --help
#
# 이 파일은 **껍데기다**. 판정 로직(정규화·판정·진단·종료 코드)은 전부
# `tools/oracle-diff`(Rust) 에 있고 거기에 인라인 테스트가 붙어 있다. 셸에는
# "저장소 루트를 찾아 실행 파일을 exec" 말고 할 일이 없다.
set -euo pipefail

script_dir="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH='' cd -- "$script_dir/.." && pwd)"

bin="$repo_root/target/debug/oracle-diff"
if [ ! -x "$bin" ] && [ -x "$bin.exe" ]; then
  bin="$bin.exe"
fi

if [ ! -x "$bin" ]; then
  echo "oracle-diff: 실행 파일이 없어 빌드합니다 (cargo build -p oracle-diff) …" >&2
  if ! command -v cargo >/dev/null 2>&1; then
    echo "oracle-diff: cargo 를 찾을 수 없습니다. Rust 툴체인을 PATH 에 넣으세요." >&2
    exit 4
  fi
  if ! cargo build --quiet --manifest-path "$repo_root/Cargo.toml" -p oracle-diff >&2; then
    echo "oracle-diff: 빌드 실패" >&2
    exit 4
  fi
fi

exec "$bin" "$@"