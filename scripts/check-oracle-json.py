#!/usr/bin/env python3
"""`wz oracle` JSON 자기검증 — C# 오라클 없이 돌릴 수 있는 최소 계약 확인.

이 스크립트가 검사하는 것은 **계약의 형태**이지 값의 정합성이 아니다. 값이
C# 오라클과 같은지는 `scripts/differential.sh` 가 판정한다. 여기서 보는 것은:

  * envelope 이 스펙대로인가 (`schema_version`, `op`, `result`)
  * 합성 코퍼스(`props_pkg1.wz`)에서 기대한 필드가 실제로 나오는가
  * `image` 의 해시와 `stats` 의 스탯처럼 **코퍼스가 고정한 값**이 나오는가

CI 에서는 `rust-gates` 잡이 이걸 돌린다. 여기서 깨지면 차분을 돌릴 이유가 없다.
로컬에서도 같은 방식으로 돌린다:

    python3 scripts/check-oracle-json.py image target/image.json
"""

import json
import sys

# 합성 코퍼스 `props_pkg1.wz` 가 고정한 값들 (tools/corpus-builder/src/main.rs).
GEAR_INC_STR = 3
EXPR_7_OVER_2 = 3.5

IMAGE_KEYS = {"w", "h", "format", "scale", "pages", "sha256"}


def fail(op: str, message: str) -> None:
    print(f"{op}: {message}", file=sys.stderr)
    sys.exit(1)


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("사용: check-oracle-json.py <op> <json-file>", file=sys.stderr)
        return 2

    op, path = argv[1], argv[2]
    try:
        with open(path, encoding="utf-8") as fp:
            doc = json.load(fp)
    except (OSError, json.JSONDecodeError) as exc:
        fail(op, f"JSON 을 읽을 수 없다: {exc}")

    if doc.get("schema_version") != 1:
        fail(op, f"schema_version 이 1 이 아니다: {doc.get('schema_version')!r}")
    if doc.get("op") != op:
        fail(op, f"op 키가 다르다: {doc.get('op')!r}")
    result = doc.get("result")
    if not isinstance(result, dict):
        fail(op, "result 가 객체가 아니다")

    if op == "tree":
        if result.get("node_count", 0) <= 0:
            fail(op, f"node_count 가 0 이다: {result.get('node_count')!r}")
        if "root" not in result:
            fail(op, "root 가 없다")
    elif op == "image":
        missing = IMAGE_KEYS - set(result)
        if missing:
            fail(op, f"필드 누락: {sorted(missing)}")
        if result["w"] <= 0 or result["h"] <= 0:
            fail(op, f"크기가 0 이다: {result['w']}x{result['h']}")
        if not isinstance(result["sha256"], str) or len(result["sha256"]) != 64:
            fail(op, f"sha256 형식이 아니다: {result['sha256']!r}")
    elif op == "stats":
        if result.get("incSTR") != GEAR_INC_STR:
            fail(op, f"incSTR 이 {GEAR_INC_STR} 이 아니다: {result.get('incSTR')!r}")
    elif op == "expr":
        if result.get("value") != EXPR_7_OVER_2:
            fail(op, f"7/2 값이 {EXPR_7_OVER_2} 가 아니다: {result.get('value')!r}")
    else:
        # 새 op 가 생기면 여기서 알려준다 — 조용히 통과시키지 않는다.
        fail(op, "이 스크립트가 모르는 op 다")

    print(f"{op}: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
