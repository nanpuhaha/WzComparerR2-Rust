# 데이터 부재 항목 (전수 스캔)

`wz parity-scan <Data>`는 원본 기능 중 클라이언트 데이터가 없어 비교 검증이
불가능한 항목을 실제 데이터 트리에서 다시 센다. 재현 명령:

```
cargo run -q -p wz-cli -- parity-scan Data
```

## 2026-11-06 실행 결과

```
Data root: Data
Skill: 7055 skill nodes, 0 common nodes, 3 level nodes
WorldMap: 168 images
WorldMap node QuestLimit/Fog: 0
Mob: 3524 images, 227 info nodes, 0 level, 0 elemAttr
Achievement files: 6
Recipe.img files: 0
Familiar files: 0
BIN files: 0
weather: 원본 C# 소스에 weather 구현 없음
```

## 해석

| 항목 | 근거 | 상태 |
|---|---|---|
| Skill `common`/`level` 수치 치환 | skill 7,055개 중 `common` 0개 | 데이터 없음. resolver 실패 경로 20,629행은 C#과 일치 |
| WorldMap `QuestLimit`/`Fog` | WorldMap 168 이미지에 노드 0개 | 데이터 없음 (fixture 단위 테스트만 존재) |
| Familiar | `Character/Familiar`, `Etc/FamiliarInfo.img` 파일 0개 | 데이터 없음 |
| BIN | `.BIN` 파일 0개 | 데이터 없음 (포맷 파서만 구현) |
| weather | 원본 `WzComparerR2`, `WzComparerR2.Common`, `WzComparerR2.WzLib`에 `weather` 문자열 없음 | 원본 기능 아님 |
| MobTooltip 스탯 | Mob 3,524 이미지 중 info 227개, `level`/`elemAttr` 0개 | 스탯/속성 라인은 데이터 없음. 이름/ID는 StringMob으로 검증 가능 |
| RecipeTooltip | `Recipe.img` 0개 | 데이터 없음 |
| WorldArchiveTooltip | `Etc_002.wz/worldArchive.img` 894KB, 1,845 항목 (NPC 대사 767) | 구현·검증 완료 (`wz world-archive`) |
| AchievementTooltip | Achievement 파일 6개 | 구현·검증 완료 (1,579개, `wz achievements`) |

이 항목들은 구현 누락이 아니라 검증 대상 데이터가 없는 경우이며, 데이터가
추가되면 `wz parity-scan` 수치가 바뀌므로 즉시 드러난다.
