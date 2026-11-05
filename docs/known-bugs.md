# 알려진 C# 버그 — 포트 시 수정해야 할 것

리포트 §12 R12. **원본 버그를 그대로 재현하지 않는다.** 고치되, 각 항목을
Rust 테스트로 고정한다.

원본 코드 위치는 공개 저장소 [`seotbeo/WzComparerR2`](https://github.com/seotbeo/WzComparerR2) 기준 (커밋 `03c18a8`).

| # | 위치 | 버그 | Rust 대응 | 상태 |
|---|---|---|---|---|
| 1 | `Wz_Node.cs:670` | 멀티페이지 PNG 덤프가 항상 0번 페이지 출력 (`ExtractPng()` 에 `page` 인자 누락) | `wz-image` 디코더가 `page` 파라미터를 받음 | Phase 2 |
| 2 | `Wz_Crypto.cs:96` | `folder.Replace(".im/", ".img")` 반환값을 버림 — 치환이 실행되지 않음 | `list.wz` 로더 자체가 레거시. `StringNameContainer` 로 대체 | Phase 2 |
| 3 | `MaplestoryCanvasVideoLoader.cs:167` | SSE4.1 경로가 `xmm1 = Shift(xmm0, 1)` — 가져야 할 값을 잘못 읽음. AVX2 경로만 정확 | 손대지 않는다. 스칼라 루프로 교체 (≈25 LOC) | Phase 6 |
| 4 | `VpxVideoDecoder.cs:76-80` | `catch`에서 HGlobal 해제 후 `throw` 하지 않아 **해제된 포인터**를 남김 | libvpx 자체를 제거. Rust `image`/ffmpeg | Phase 6 |
| 5 | `ConfigManager.cs:75-79` | `Assembly.GetCallingAssembly()` + tail-call 해저. **자체 주석이 경고**하고 있다 | 리플렉션 기반 섹션 등록을 제거. `serde` + 정적 스키마 | Phase 3 |
| 6 | `Gear.cs:28-32` | `diff` 가 **no-op setter**. `MakeTimeLimitedPropAvailable:190` 이 여기에 쓰는데 조용히 값 버림 | `diff` 필드 제거 또는 실제 구현 | Phase 4 |
| 7 | `CharaEquip.cs:219-226` | `else if (newGearType == GearType.pants)` 가 도달 불가 (앞 분기가 이미 처리) | 슬롯 충돌 매트릭스를 테이블로 재작성하면서 제거 | Phase 4 |
| 8 | `Wz_Node.cs:549-558` | `ResolveUol` 무한 루프 — **사이클 감지가 없다.** 자기참조 UOL 이 멈추지 않는다 | `resolve()` 에 깊이 상한. `WzFormatError::UolDepthExceeded` | Phase 2 |
| 9 | `Wz_Node.cs:432-439` vs `:366-370` | 문자열 인덱스는 **마지막** 중복을 유지, 선형 폴백은 **역방향** 스캔. 우연히 일치할 뿐 | 중복 이름을 전부 보관하는 `ChildrenIndex`. 경로 규칙 테스트 | Phase 2 |
| 10 | `VpxVideoDecoder` `vpx_image` | `planes_0..3` 필드 + `fixed` 로 포인터 getter 위조. ABI 버전에 취약 | libvpx FFI 제거 | Phase 6 |
| 11 | `PngEffect` / `MonogameUtils.cs:76-92` | MonoGame **사私有 필드 리플렉션** (`_d3dContext`, `_swapChain`) 으로 텍스처 교체 | wgpu 로 대체하면 필드 접근이 필요 없어짐 | Phase 6 |
| 12 | `MapRenderV1` 블록 | 3,802 LOC 이 **컴파일조차 되지 않음** (매크로 미정의) | 삭제 | ✅ Phase 0 |
| 13 | `Chat.cs` | SignalR 채팅 클라이언트가 **100% 주석 처리**. 죽은 엔드포인트 | 삭제 | ✅ Phase 0 |
| 14 | `ConfigArrayList.cs:83` | 항목마다 **무작위 GUID** 저장. 저장→불러오기→재저장 시 파일이 매번 달라짐 | 날짜순 ID(ULID) 또는 인덱스로 교체 | Phase 3 |
| 15 | `Ms_ImageV2.cs:71-72` | **`nonce[0..4]` 를 초기화하지 않은 스택 메모리로 두고**, `ChaCha20CryptoTransform.cs:170` 이 실제로 읽는다 = UB | `nonce[0..4] = 0` 으로 결정. **오라클 검증 필수** | 🔴 Phase 1 검증 필요 |
| 16 | `WzComparerR2/Entry.cs` (Avatar) | ~420 LOC 의 **개발자 디버그 하네스**가 엔트리포인트에 남아 있음 (`D:\ms\new_...` 에 GIF 기록) | 삭제 | ✅ Phase 0 |
| 17 | `GearLevelInfo.BonusProps` | Min/Max 범위와 가중치 `prob` 테이블을 **파싱만 하고 굴리지 않는다** | **결정 필요**: 게임 규칙대로 구현할지, 주석만 유지할지 | ⏸ Phase 4 |
| 18 | `Wz_Png.cs:151` | zlib 헤더로 `78 9C` **하나만** 인정. `78 01`/`78 DA` 같은 유효 헤더를 "암호화 경로"로 오인 | **원본 동작을 유지** (호환성). 주석으로 문서화 | ✅ `wz-image` 완료 |
| 19 | `MapRender/Effects/Resources/Native/*` | 게임 클라이언트에서 **추출한 DXBC 셰이더 23개** (독점 자산) | ✅ **감사 완료 — 실제 사용 3개뿐.** `docs/shader-audit.md`. 미사용 20개 + 사용 3개 모두 **번들 제외**, WGSL 재작성. `EffectCompiler` 792 LOC 삭제 대상 |

---

## 반드시 보존해야 하는 함정 (버그 아님)

의도된 동작인데 리팩터링하면서 깨뜨리기 쉬운 것들. 각 항목은 Rust 테스트로 고정돼 있다.

| # | 내용 | 원본 위치 | Rust 테스트 |
|---|---|---|---|
| T1 | **압축 정수는 LEB128/varint 가 아니다.** 이스케이프는 오직 `-128`(0x80) 하나, 그 뒤에 **고정폭** 4/8바이트 | `WzBinaryReader.cs:49-79` | `read::int::tests::not_leb128` |
| T2 | **문자열 길이 접두사는 평문**, 페이로드만 복호화되며 **키스트림 오프셋 0** 부터 | `WzBinaryReader.cs:102,112,135` | `read::string::tests::payload_decryption_starts_at_keystream_zero` |
| T3 | AES IV 는 **16바이트가 아니라 4바이트**. 16바이트로 가정하면 모든 KMS/GMS 문자열이 깨진다 | `Wz_Crypto.cs:105-106,198-201` | `crypto::aes::tests::keystream_matches_reference_implementation` |
| T4 | `Pkg2DirStringKey` 바이트 순서는 `[0,1,1,2,2,3,3,4]`. "하위 32비트 LE + 4바이트 0" 이 **아니다** | `Wz_Crypto.cs:362-375` | `crypto::pkg2_key::tests::key_byte_order_is_not_naive_le` |
| T5 | `Ms_FileV2` 시프트는 **산술 시프트** (`(sbyte)b >> 1`). `>>>` 로 쓰면 v2 파싱이 깨진다 | `Ms_FileV2.cs:75-78` | Phase 1 |
| T6 | ChaCha20 카운터 리셋은 **`Ms_FileV2` 엔트리 테이블에서만**. `Ms_ImageV2` 이미지 페이로드는 표준 증가 | `Ms_FileV2.cs:310-313` vs `Ms_ImageV2.cs:83` | `crypto::chacha::tests::two_modes_differ` |
| T7 | ChaCha20 리셋 후 **카운터를 다시 1 더하지 않는다** (0 으로 되돌린 값으로 블록 생성) | `Ms_FileV2.cs:310-313` | `crypto::chacha::tests::entry_table_reader_resets_counter_every_64_bytes` |
| T8 | MS v1 이미지는 앞 1024바이트를 **2회** 복호화 | `Ms_Image.cs:52-66` | Phase 1 |
| T9 | BC7 크기는 `w * (h & ~3)` (행 패딩 후 꼬리 행 버림) | `Wz_Png.cs:361-362` | `wz_image::format::tests::bc7_uses_floored_height` |
| T10 | 원시 데이터 크기의 제수는 `Scale` 가 아니라 **`ActualScale = 1<<Scale`** | `Wz_Png.cs:89` | `wz_image::format::tests::layout_uses_actual_scale_as_divisor` |
| T11 | PKG2 1202+ 디렉터리 문자열은 **i16** 길이 (그 이전은 i8) | `WzBinaryReader.cs:153-216` | `read::string::tests::pkg2_v1_and_v2_use_different_length_fields` |
| T12 | `Pkg2DirStringKeyV4` 는 **위치 의존** (stateful) | `Wz_Crypto.cs:479-486` | `crypto::pkg2_key::tests::v4_is_position_dependent` |
| T13 | UOL 경로는 UOL 노드의 **부모 기준 상대 경로**. 이미지 밖으로 나가면 `img.OwnerNode` 로 넘어가고 실패 시 `+ ".img"` 재시도 | `Wz_Uol.cs:37-68` | Phase 2 |
| T14 | `CharaProp.GetSum()` 은 `(base+add+...) * (100+rate+...) / 100` — **정수 나눗셈 순서 보존** | `CharaProp.cs:82-86` | Phase 4 |
| T15 | `GearPropType` 숫자값 = WZ 노드 이름 (`1..99` 일반, `100..199` 잠재, `1000+` 장비별) | `GearPropType.cs` | Phase 4 |
| T16 | Frame `delay` 기본 120, `Math.Abs` 적용 | `Frame.cs:129-132` | Phase 4 |
| T17 | GIF 알파 보간: `A0 == A1 ? A0 : round(A0*(1-p) + A1*p)` | `GifCanvas.cs:102-108` | Phase 4 |
| T18 | `.patch` CRC 는 압축 영역 `[16, end)` **만** 해시, 오프셋 12에 저장. CRC-32/BZIP2 변종 | `WzPatcher.cs:72` | Phase 3 |
| T19 | `Property` 의 `i32 n` 은 **값 개수** (바이트 수가 아님) | `Wz_Image.cs:262-432` | `corpus_builder::tests::property_count_is_value_count_not_byte_count` |
| T20 | `float`(0x04) 값은 **압축 int** 로 읽고 IEEE-754 비트 패턴으로 해석 | `WzBinaryReader.cs:81-86` | `corpus_builder::tests::float_uses_compressed_int_not_raw` |
| T21 | `Canvas.dataLen` 은 **비압축** i32 이고 **플래그 바이트를 포함** | `Wz_Image.cs:301` | `corpus_builder::tests::canvas_data_len_is_readable_from_tail` |
| T22 | 청크 암호화는 **청크마다 키스트림 오프셋 0 리셋** | `ChunkedEncryptedInputStream.cs:73-83` | `corpus_builder` chunked 경로 |
| T23 | **캔버스 zlib 스트림은 완결되지 않았을 수 있다.** 2026 실전 클라이언트(KMST1206 계열) 검증: deflate 블록이 `BFINAL=0` 로 끝나고 adler32 없이 Z_SYNC_FLUSH (`00 00 FF FF`) 로 종료. .NET `DeflateStream` 을 raw 로 쓰는 원본은 통과, `ZlibDecoder` 는 "incomplete stream" | 2026-10-30 전 코퍼스 실측 (792개 WZ / 461만 캔버스, 초기 전체 실패 → 수정 후 전부 성공) | `wz_image::decompress_zlib_compat` / `wz-image` `sync_flush_terminated_stream_is_decodable` |

---

## 추가 확인 필요 (코드에서 확정 못 함)

| # | 항목 | 이유 | 조치 |
|---|---|---|---|
| U1 | `Ms_ImageV2` `nonce[0..4] = 0` 가정이 맞는지 (#15) | 원본이 UB 라 코드로는 확정 불가 | 실제 MS v2 파일로 오라클 검증 |
| U2 | 새 클라이언트 버전의 난독화 도입 가능성 | 리포트 §12 R3 | 미지원 시 `FormatProfile` 추가가 정답. 오류 메시지가 그 방향을 안내 |
| U3 | `GearLevelInfo` 가중치 굴리기 구현 여부 (#17) | 원본이 미구현 — 의도인지 미완성인지 불명 | 게임 규칙 확인 후 결정 |
| U4 | WGSL 셰이더의 **시각적** 일치도 | 파라미터 계약은 정확히 옮겼지만 픽셀 출력이 원본과 다를 수 있음 | 원본 C# 과 스크린샷 비교 (Phase 6) |

---

## Phase 1.12 에서 **C# 원문으로 확정**된 함정 (이전 추정을 대체)

아래는 추측이 아니라 `WzComparerR2.WzLib` 원문을 직접 읽고 확인한 내용이다.
두 항목은 이전에 문서에 반대/모호하게 적혀 있었다.

### 1. UTF-16 문자열 길이 접두의 경계값은 127 이다 (128 이 아니다)

원본 리더 `WzBinaryReader.cs:126-129`:

```csharp
if (size == 127) { size = this.bReader.ReadInt32(); }
```

즉 `127` 이 **이스케이프 마커**다. 그래서:

- UTF-16 코드유닛 수 `< 127` → 접두 1바이트
- `>= 127` → 접두 `0x7F` + `i32` = **5바이트**
- 암호화는 **페이로드에서만**. 접두가 가변이므로 페이로드 오프셋도 달라진다.

**관측된 증상**: 페이로드 오프셋을 1로 고정하면 127자 넘는 문자열에서 접두 안의
`i32` 길이까지 암호화되어 파서가 `len = 177392326` 을 읽고 OutOfBounds 로 죽는다.
127 이하에서는 우연히 맞아서 **작은 문자열 테스트로는 잡히지 않는다**.
→ `wz_core::read::image_string_header_len` / `WZ_UTF16_ESCAPE_AT` 가 이를 고정한다.

같은 함정을 값 문자열 쪽도 갖는다. `0x08` (String) 값은 이름과 **같이**
`ReadImageString` 경로라 `0x00` 플래그 바이트가 앞에 붙는다.

### 2. 이 포크는 upstream 과 아키텍처가 다르다

`MapleLib/Helper/DetectVersion.cs` 와 `WzMapleVersion` enum 은 **존재하지 않는다**
(seotbeo 포크가 `Compatibility/` 로 대체). 따라서:

- `Data/Script.nx` 프로빙, `String.wz` 경로 존재 프로빙 → **없음**
- 버전 축은 하나가 아니라 **여섯 개**:
  `WzFileFormat`, `WzVersion`(int), `Wz_Capabilities`, `Wz_CryptoKeyType`,
  `Pkg2OffsetVersion`, `Pkg2EntryNameVersion`
- 레지스트리 순서는 **비단조**다. `Pkg2Kmst1198` 포맷에서는
  **1200 → 1199 → 1198** 순으로 시험한다. 오름차순 정렬하면 안 된다
  (`WzVersionProfile.cs:171-184`).

### 3. 디렉터리 엔트리 타입은 세 개뿐이다

| 값 | 의미 |
|---|---|
| `0x02` | 이미지. 이름이 `i32` 오프셋 **참조** (PKG1 `Data.wz` 계열) |
| `0x03` | 하위 디렉터리 |
| `0x04` | 일반 이미지 |

**캔버스 타입도 UOL 타입도 없다** — 둘 다 이미지 **내부** 노드다.
`0x00`/`0x01` 은 이미지 안에서 문자열을 읽을 때의 플래그지 디렉터리 타입이 아니다.
그 외 값은 원본이 예외를 던진다 (`Wz_File.cs:511,560`).

### 4. PKG1 과 PKG2 의 블록 구조가 다르다

- PKG1: 엔트리가 자기 안에 해시 오프셋을 갖는다
  `[type][name][size][cs32][hashedOffset]`
- PKG2: 엔트리를 **전부 먼저** 읽고 그 뒤에 오프셋 표가 붙는다
  `[encCount][entries...][offCount][offsets...]`
  — 엔트리 **개수 자체가 암호화**되어 표의 경계를 모르게 한다.

`offCount` 를 복호화한 값이 `encCount` 와 같을 때만 뒤에 표가 있다
(`WzFileReadContext.ShouldReadOffsets`).

### 5. 이름 마스크는 UTF-16 과 ASCII 가 **다르다**

- UTF-16: `chars[i] ^= 0xAAAA + i` — **유닛** 기준 16비트
- ASCII: `bytes[i] ^= 0xAA + i` — **바이트** 기준 8비트

둘을 같은 규칙으로 쓰면 한쪽이 깨진다. 관측: `Character/00002000.img` 가
깨진 문자열로 풀림 (상위 바이트 마스크가 1 어긋남).

### 6. `0x02` 이름 참조의 `stringOffAdd` 는 +2 와 -1 로 갈린다

`Wz_File.cs:509-510`: `encverMissing` 면 `+2`, 아니면 `-1`. 1바이트 차이가 나지만
**둘 다 원본 그대로**다. 고치지 않는다.

### 7. `DirStartPosition` 은 `dataSize` 가 아니다

원본은 `fileSize - DirStartPosition` 만큼을 길이로 쓴다 (`Wz_File.cs:370-376`).
저장된 `dataSize` 은 어디에도 쓰이지 않는다.

### 8. `Pkg2Kmst1205Hash` 는 처음에 "간단한 혼합" 으로 작성돼 있었다

Phase 1.12 에서 C# 원문(`Magic1`~`Magic8` 8개 상수)으로 교체했다. 이전 구현은
KMST1205/1206 버전 판정을 전부 틀리게 만들었다. `ComputeHash2` 안에
`RotateMultiplyXorRight30` 이 **다시 `MultiplyXorRight`(shift 30)** 를 포함하므로
두 단계를 빼먹으면 안 된다.

---

## Phase 1.13 — 버전 자동 감지와 성능

### 1. 판별의 결정적 단서는 "오프셋 구간이 겹치지 않는다" 이다

`hashVersion` 후보가 여러 개여도, 하나를 고르면 **모든 이미지 오프셋이 결정**된다.
틀린 키는 오프셋을 무작위에 가깝게 흩뿌리므로 세 조건 중 하나가 걸린다:

1. 오프셋이 파일 범위 밖
2. 오프셋이 `dirStart` 보다 앞섬
3. 두 이미지의 `[시작, 끝)` 구간이 겹침 ← 가장 강력

3번이 사실상 결정적이다. 무작위 오프셋 두 개가 겹치지 않을 확률은 0 에 가깝다.
→ `wz_core::detect_version::try_profile`

### 2. 레지스트리 순서는 **비단조** — 정렬 금지

| 포맷 | 시도 순서 |
|---|---|
| `Pkg2Kmst1198` | **1200 → 1199 → 1198** |
| `Pkg2Kmst1196` | **1197 → 1196** |

오름차순으로 정렬하면 1200 파일에서 1198 을 먼저 시도해 잘못된 키로 트리를 읽고,
"안 되네" 하고 **정답인 1200 을 건너뛴다.** 원본 `WzVersionProfile.cs:171-184`
배열 순서를 그대로 보존한다. (`profile::registry_matches_csharp_order` 가 고정)

### 3. PKG1 해시 오프셋은 **엔트리 안**에 있다

PKG2 와 구조가 다르다. 이걸 섞으면 두 번째 엔트리부터 해시 바이트를 타입으로 읽는다
(관측: `entry 1: type=0x0e`).

```text
PKG1: [count][ type name size cs32 hashedOffset ] × N
PKG2: [encCount][ type name size cs32 ] × N [encOffCount][hashedOffset] × N
```

PKG2 는 엔트리 **개수 자체를 암호화**해서 표의 경계를 모르게 한다.

### 4. 2³² 전수 스윕은 프로필당 ~18초 (12코어)

KMST1199–1201 과 KMST1200 은 `hashVersion` 역산에 `2³²` 전수 탐색이 필요하다
(C# 도 `Parallel.For(0, 256)` 로 같은 일을 한다). rayon 256청크로 구현했고
12코어에서 17.6초. 게다가 **표준 PKG2 헤더는 1201→1200→1199→1198 순**이라
최악엔 스윕을 여러 번 한다.

| 프로필 | 방식 | 실측 |
|---|---|---|
| PKG1 | `encver` 역전 | 즉시 |
| KMST1196 | 닫힌 형태 | 즉시 |
| KMST1197/1198 | 비트 백트래킹 | 즉시 |
| KMST1202–1204 | 닫힌 형태 | 즉시 |
| KMST1205/1206 | 하드코딩 문자열 | 즉시 |
| KMST1199–1201, 1200 | 2³² 스윕 | **~18초** |

대응:
- `wz tree` 가 표준 PKG2 헤더면 **사전에 경고**를 출력하고 소요 시간을 보고한다.
- `--hash-version` 을 주면 스윕을 건너뛴다 (오라클 대조용).
- 2³² 경로는 기본 테스트 스위트에서 제외했다 (`#[ignore]`).
  포함하면 검증이 느려져 사람이 스킵하고, 그러면 아예 안 돌게 된다.
  `cargo test -p wz-core -- --ignored` 로 명시 실행.

### 5. 1205/1206 은 "후보 0개" 가 **정상**이다

버전 문자열 `"v410_260106_1_A1F3C9E2"` 가 하드코딩돼 있어, `hash2` 가 그 값으로
계산한 결과와 다르면 후보를 아예 못 낸다. 이를 "후보가 1개" 라고 단정하면
정상적인 검증 실패를 오류로 취급하게 된다.
(`profile::hardcoded_versions_reject_mismatched_hash2`)

### 6. 감지 상한 200,000 은 안전망이다

잘못된 `hashVersion` 은 엔트리 개수를 임의 값으로 복호화해 낸다. 그 값으로
`Vec::with_capacity` 를 호출하면 OOM 으로 죽는다. 실제 WZ 의 최대 엔트리 수는
수천 개 수준이므로 200,000 은 넉넉한 상한이다.

---

## Phase 1.14 — C# 오라클 대조에서 확인된 차이 2건

`WzComparerR2.Oracle` + `scripts/diff-oracle.ps1` 로 실제 대조를 돌려 확인했다.

### 1. C# 의 디렉터리 엔트리 순서는 **파일 순서가 아니다**

`Wz_File.cs:497-492` 는 엔트리를 **두 패스**로 나눠 넣는다.

1. 블록을 읽으면서 **이미지(0x04)만** `parent.Nodes` 에 바로 추가하고,
   디렉터리(0x03)는 `dirs` 리스트에 모은다.
2. 그다음 `dirs` 를 순회하며 **디렉터리 노드를 나중에** 추가한다.

→ C# 출력은 항상 **이미지 → 디렉터리** 순서다.
  Rust 는 **디스크 순서 그대로** 읽는다.

| | 순서 |
|---|---|
| C# | 이미지 전부, 그다음 디렉터리 전부 |
| Rust | 디스크에 쓰인 그대로 |

**어느 쪽이 옳은가**: 포맷 자체를 보면 디스크 순서가 "원본" 이다. 원본 툴의
화면 순서를 재현하려면 Rust 도 같은 재배치를 해야 한다. 차분 테스트는
**순서 없이 집합으로** 비교하고, 순서가 다르면 알려진 구조적 차이로 보고한다
(실패로 세지 않는다).

### 2. C# 의 오프셋이 0 이면 "미해결" 이다 — 실제 오프셋 0 이 아���다

`Wz_File.cs:524-525`:

```csharp
if (this.ReadContext?.OffsetCalc != null)
    img.Offset = this.ReadContext.OffsetCalc.CalcOffset(pos, hashOffset);
```

버전 감지에 실패하면 `ReadContext` 가 null 이라 **아예 계산하지 않고 0 으로
남는다.** 즉 C# `offset = 0` 은 "파일이 0 에서 시작한다" 가 아니라
**"해결하지 못했다"** 를 뜻한다.

함정: 이걸 Rust 의 실제 오프셋과 그대로 비교하면 **거짓 실패**가 난다.
대조 스크립트는 C# 해�� 오프셋이 하나도 없을 때 비교를 **스킵**하고
"실파일을 넣어야 한다" 고 명시한다.

### 3. `new Wz_File(...)` 만으로는 트리를 못 읽는다

`Wz_Structure.LoadFile`(`Wz_Structure.cs:84-216`) 이 해야 하는 세 가지가 없다:

1. `WzPreReaders.All` 로 프로파일 사전 판독 (버전 추론)
2. `DetectCryptoKeyType` — 문자열 암호 키 타입 결정
3. `AssignDirStringReader` — 디렉터리 이름 읽기 경로 배정

이 셋을 빼면 `Wz_Crypto.Pkg1EncType` 이 `Unknown` 으로 남아
`encryption.Pkg1Keys` 가 null 이 되고, `WzBinaryReader.cs:135` 의
`decrypter.Decrypt(...)` 에서 **NullReferenceException** 이 난다.

오라클은 `Wz_Structure.LoadFile` 을 쓴다. 합성 코퍼스는 프로파일 감지를
통과하지 못하므로 `Pkg1EncType = BMS`(항등)를 명시한다.

---

## 대조 가능한 것 / 아직 못 하는 것

`scripts/diff-oracle.ps1` 로 현재 확인되는 것:

- 키스트림 `bms` / `kms` / `gms` / `kmst1198` — **바이트 단위로 C# 와 일치**
- 헤더 `fileSize` / `headerSize` / `dirStartPosition` / `signature` / `encverMissing`
- 트리 엔트리 수, 이름 집합

아직 못 하는 것:

- **오프셋** — 합성 파일에서 C# 감지가 실패해 C# 쪽이 전부 0. 실파일이 필요하다.
- **이미지 노드의 스칼라 값** — 이 포크의 `Wz_Node.Value` 는 `object` 이고
  스칼라 값 타입이 public 으로 노출되지 않아 타입 이름을 확인할 수 없다.
  `wz image` ↔ C# `Wz_Image` 노체를 나란히 보는 수동 대조로 전환해야 한다.
- `kmst1199` — C# 도 `NotSupportedException` 을 던진다 (원본도 미지원).



## random-header PKG2: 감지는 되지만 버전 감지가 실패한다

`detect()` 는 이 포맷을 인식한다 (`c87516f`, 오프셋 353 의 `0x80` 마커).
`C:/Nexon/M` 788개 중 **192개(24%)** 가 이 형태이고, 감지 결과는 C# 과
정확히 일치한다 (595 PKG1 / 192 PKG2 / 1 미분류).

하지만 `wz tree` 는 `버전 자동 감지 실패: 후보: [(1206, 0)]` 로 실패한다.
여기서 `(1206, 0)` 의 `0` 은 **후보 개수 0** 이다 (`tried.push((wz_version,
candidates.len()))`).

## 원인: `hash1` 이 헤더에 없다

### 확인된 사실 (C# 진단 출력, `_Canvas_041.wz`)

```text
pos=393 hash=0x071F1C7E name=19214.img size=393977711
calc=Pkg2OffsetCalc64V2
  headerLen   = 353
  hash1       = 0xDCC2AB20469D0574
  hashVersion = 0xB8047BAB2410606A
offset = 397
```

### `hashVersion` 은 0 이 **아니다**

`0xB8047BAB2410606A` 는 1206 프로파일의 정상값이다
(`KMST1205_1206_VERSION_STRING = "v410_260106_1_A1F3C9E2"` 에서 유도).
Rust `compute_pkg2_hash_version_1206()` 도 **똑같은 값** 을 낸다.

즉 `Wz_Structure.cs:157` 의 `CreateReadContext(header, 0, null, null)` 인 `0`
은 hashVersion 이 아니다. `CreateReadContext` 가 내부에서 hashVersion 과
hash1 을 **재유도** 한다.

### `hash1` 은 헤더에 없다

random-header PKG2 는 `Wz_File.cs:237` 에서 `hash1 = hash2 = 0` 으로 헤더를
만든다. 그런데 실제로 쓰이는 calc 의 `hash1` 은 `0xDCC2AB20469D0574` 이다.
이 값은 파일 앞 512 바이트에도 없고, 따라서 **어딘가에서 유도된다**.

그래서 `candidates_1206(hash1=0, hash2=0)` 은
`ComputeHash2(0, hv) == 0` 을 검사해 실패하고 빈 벡터를 준다.
C# 도 같은 조건을 쓰지만, C# 은 실제 hash1 을 얻은 뒤 후보를 만든다.

## 오프셋은 정확히 유도된다 (브루트포스 불필요)

세 겹으로 확인:

| 검증 | 값 |
|---|---|
| 패턴 `73 F8 FA D9 C3` 실제 위치 | 397 |
| C# 이 계산한 offset | 397 |
| `397 + size(393977711)` | 393978108 = 파일 크기 |

즉 오프셋은 `OffsetCalc.CalcOffset(pos, hashOffset)` 의 **정확한 결과** 다.
`FindAllHits` 패턴 스캔은 `CandidateImageInfos` 가 비었을 때의 백업 경로
일 뿐이고, 이 파일에서는 오프셋 표가 정상 복호화되므로 쓰이지 않는다.

## 이미지로 확인된 디렉터리 레이아웃

`EntryNamePosition = AfterData` (unknown profile 이 사전 읽기 값을 그대로
전달 — `Wz_Structure.cs:156`):

```text
353  80 + 8바이트        압축 엔트리 개수
362  04                 node_type = 이미지
363  80 + 4바이트       size
368  80 + 4바이트       cs32
373  F7 FF              int16 = -9 → 이름 9자
375..392                암호화된 UTF-16 이름 18바이트
393                      pos, 그 자리에 u32 hashOffset = 0x071F1C7E
```

이름은 `ReadPkg2DirStringV2` (`WzBinaryReader.cs:186`) 가 int16 길이 +
`size * 2` 바이트를 읽고 `firstNameKey` 로 복호화한다. 파일에 완전히 들어있다.

## 남은 작업

1. `hash1 = 0xDCC2AB20469D0574` 의 유도 경로 규명 — 이것이 마지막 관문이다
2. `hashVersion = hv_1206`, `hash1` 을 넘겨 `candidates_1206` 가 성공하도록
3. UNK 사전 읽기 / `ForcedCounts` — 위가 해결되면 필요 없을 수 있다
   (preReadResult 가 null 이면 `use8ByteKey` 도 false 가 되고 이름은
    평범한 PKG2 경로로 읽힌다)
4. 실제 192개 파일 트리 파싱 + C# 대조

`ImgNameContainer` 는 **필요 없다**. `Load()` 는 `MainForm.cs:1382` (GUI)
에서만 호출되고 `imgs.bin` 도 없어서 컨테이너가 항상 비어 있다. 그런데도
이름은 정상 복구된다 (`ItemOption.img`, `0501.img`, `40002.img` …).

## 해결: 353바이트 헤더 파서가 통째로 빠져 있었다

### 결론

`Pkg2OffsetCalc64V2` 와 `candidates_1206` 은 **그대로 옳다**.
빠져 있던 것은 단 하나 — `Wz_File.TryReadPkg2KMST1206Header`
(353바이트 random header 에서 hash1/hash2 를 뽑는 파서).

`c87516f` 에서 추가한 폴백은 오프셋 353 의 `0x80` 마커만 보고
`hash1 = hash2 = 0` 을 넣었다. 하지만 C# 은 폴백이 아니라
**KMST1206 헤더 파서** 를 먼저 시도하고, 거기서 실제 해시를 얻는다.

### 근거 (C# 진단 출력, `_Canvas_041.wz`)

```text
container:  headerType=WzPkg2Header64  headerSize=353  cap=4
            hash1=15907464988938012020   (0xDCC2AB20469D0574)
            hash2=14172149628109643592   (0xC4AD96FFDF28DF48)

tree:       calc=Pkg2OffsetCalc64V2
            headerLen=353  hash1=0xDCC2AB20469D0574
            hashVersion=0xB8047BAB2410606A
            pos=393  hash=0x071F1C7E  offset=397
```

`cap = 4` = `Wz_Capabilities.Pkg2RandomHeader64`.
폴백(`Wz_File.cs:237`) 이면 `hash1 = hash2 = 0` 이므로 이 값일 수 없다.

### 검증 (Rust, 실제 파일에서 얻은 값 그대로)

```text
real_values_reproduce_offset_397        ok   calc_offset(393, 0x071F1C7E) == 397
real_hash1_yields_single_1206_candidate ok   candidates_1206(h1, h2) == [0xB8047BAB2410606A]
```

폴백의 `hash1 = 0` 을 쓰면 `ComputeHash2(0, hv) != 0` 이라 후보가 0개가 되고
`detect_version` 이 실패한다. **원인이 정확히 이一处였다.**

### 남은 구현

`Wz_File.TryReadPkg2KMST1206Header` 를 Rust 로 이식하고
`detect_pkg2_random_header` 의 `hash1/hash2` 를 그 결과로 채운다.

관련 C# 함수 (호출 순서, `Wz_File.cs:132-162`):
1. `TryReadPkg2KMST1206Header` ← 실제 데이터는 여기서 성공
2. `TryReadPkg2KMST1205Header`
3. `TryReadPkg2KMST1204Header`
4. `TryReadPkg2KMST1202Header`
5. `TryReadPkg2KMST1201Header`
6. 353 오프셋 `0x80` 폴백 (hash1 = hash2 = 0)

이식 후 `detect_version` 이 1206 후보를 얻고 `wz tree` 가 동작할 것으로
기대한다. 검증 기준: `pkg1 12data/Skill/_Canvas/_Canvas_041.wz` 트리가
`19214.img` / offset 397 / size 393977711 로 C# 과 일치.

## 해결: PKG2 V2 이름 복호화 + 192개 파일 트리 파싱

`_Canvas_041.wz` 이름이 깨지던 문제의 원인은 **겹쳐진 함정 3개**였다.

### 함정 1: `filePos` 는 절대 위치가 아니라 **상대** 위치

C# 의 `ApplyState((ulong)reader.BaseStream.Position)` 는 `PartialStream` 위의
reader 다 (`Wz_File.cs:372`). `PartialStream.Position` 은
`baseStream.Position - offset` 이므로 (`PartialStream.cs:68`) **dirStart 기준
상대값**이다.

```text
절대 373 (이름 길이 필드) → 상대 20
```

### 함정 2: 1206 은 `V5` 가 아니라 **`V6`** 키 클래스

`Wz_Crypto.Pkg2DirStringKeyV6` 는 16바이트 키 배열을 쓴다 (`V5` 는 8바이트).

### 함정 3: 바이트 쌍이 **겹친다**

```csharp
byte[] keys = new byte[16];
Span<ushort> u16Keys = ...;
for (int i = 0; i < u16Keys.Length; i++)      // 8회
    u16Keys[i] = (ushort)(this.baseKey >> (8 * i));
```

`i` 가 2바이트씩 건너뛰므로 쌍은 `[b0,b1], [b1,b2], [b2,b3], …` 이다.
`[b0,b1], [b2,b3], [b4,b5], …` 로 짝지으면 **완전히 다른 키** 가 된다.
그리고 `i = 7` 의 `>> 56` 은 64비트라 0 → 마지막 바이트가 항상 0.

### 실측 검증 (C# 진단 로그와 1:1 일치)

```text
hash1 = 0xDCC2AB20469D0574   hashVersion = 0xB8047BAB2410606A
filePos = 20 (상대)  →  base = 0x9E5E3430C1601381
keys = 81-13-13-60-60-C1-C1-30-30-34-34-5E-5E-9E-9E-00   ← C# 출력과 동일
파일 375..392 ^ keys → UTF-16 "19214.img"                ← C# oracle 출력과 동일
```

### 길이 접두사도 다르다

| | WZ 표준 | PKG2 V2 |
|---|---|---|
| 길이 | i8 (1바이트) | **i16 (2바이트)** |
| 복호화 | 문자 마스크 `0xAAAA+i` | **V6 키스트림 XOR** |

**함정**: `int16` `-9` 의 하위 바이트 `0xF7` 을 i8 로 읽으면 우연히 `-9`
가 되어 길이가 맞아 보인다. 그래서 오류가 **이름 문자열로만** 드러나고
개수·오프셋은 정상이라 원인을 찾기 어렵다. 두 번째 이름부터 어긋난다.

### 또 하나: 64비트 PKG2 규칙 차이 2개

- 엔트리 개수는 **압축 i64** (`WzFileReadContext.cs:103`)
- 해시 오프셋 표 앞에 **카운트 필드가 없다** — `ShouldReadOffsets` 가 무조건
  `true` (`:110`). 32비트만 카운트를 읽어 비교한다

## 최종 검증: 192개 파일 전부

| 항목 | 결과 |
|---|---|
| 감지 분류 (PKG1/PKG2/미분류) | 595 / 192 / 1 — C# 일치 |
| PKG2 트리 파싱 | **181 정상** / 11 빈 스텁 / 0 오류 |
| 이름 깨짐 | **0건** |
| `_Canvas_041.wz` | `19214.img` @397 / 393977711 바이트 — C# 일치 |

11건은 362바이트 스텁 파일로 C# 도 `entries: []` 를 뱉는다 (동일 처리).

## 이미지 스칼라 검증 — 실제 WZ 파싱 성공 (2026-10-04)

`image` 명령이 실제 데이터에서 **전부 실패**하던 원인은 3개가 겹쳐 있었다.
하나만 고쳐서는 여전히 깨진다.

### 1. 이미지 복호화 키 선별 부재

WZ 헤더에는 이미지 복호화 키가 **없다**. C# 은 첫 문자열을 BMS/KMS/GMS
각각으로 풀어 보고 알려진 태그가 나오는 키를 고른다 (`Wz_Image.TryDetectEnc`,
`IsIllegalTag`). 키를 고르지 않으면 문자열이 깨져 루트 태그 비교가 실패하고
**이미지 전체**가 파싱되지 않는다.

판별 기준 태그: `Property` / `Shape2D#Vector2D` / `Canvas` /
`Shape2D#Convex2D` / `Sound_DX8` / `UOL` / `RawData`.
선별 순서는 C# 과 동일하게 **BMS → KMS → GMS**.

### 2. 문자열 길이 **부호**가 인코딩을 정한다

`ReadString` 의 규칙 (`WzBinaryReader.cs:98`):

| 길이 바이트 | 경로 | 바이트 수 | 마스크 |
|---|---|---|---|
| `n > 0` | UTF-16LE | `n * 2` | `0xAAAA + 유닛 index` |
| `n == 0` | 빈 문자열 | 0 | — |
| `n == -128` | ASCII, 뒤의 `i32` 가 길이 | `len` | `0xAA + 바이트 index` |
| `n < 0` | ASCII(cp1252) | `-n` | `0xAA + 바이트 index` |

`Item_000.wz` 루트 태그 `"Property"` 는 **ASCII 8바이트**(음수 길이)다.
UTF-16 으로 읽으면(×2) 4개 유닛으로 깨진다.

### 3. `decode_text` 의 길이 기반 추측이 치명적

짝수 바이트면 UTF-16 으로 먼저 해석하고 실패하면 ASCII 로 falls back 하던
로직이 있었다. **짝수 길이 ASCII 문자열은 100% 깨진다** — `"Property"` 뿐
아니라 이름 대부분이 여기 해당한다. 경로(부호)는 호출부에서 이미 알고 있으므로
추측은 제거하고 `decode_utf16` / `decode_ascii` 로 분리했다.

### 부수적으로 함께 고친 것

- `Decrypt(data)` 는 `Decrypt(data, 0)` 이다 — 키 오프셋은 **항상 0**.
  문자열 위치와 무관하다.
- `0x1B` 문자열 참조는 **이미지 시작 기준 상대 오프셋**이다. C# 은 이미지에
  `PartialStream` 을 씌워서 자동이라 버그가 안 드러나지만, Rust 은 파일 전체
  스트림을 쓰므로 `string_ref_offset` 에 이미지 시작 오프셋을 넘겨야 한다.
- `is_legal_tag` 는 **바이트 단위**로 읽는다. `ReadCtx::read` 는 버퍼를 전부
  채워야 성공하고 실패하면 커서도 안 움직인다. 64바이트를 한 번에 읽으면
  파일 끝에 닿는 작은 이미지에서 `used == 0` 이 되어 **모든** 키가 탈락한다.
- `ImageParser` 는 `decrypter` 를 **소유**(`Box`)한다. `&'a dyn` 로 두면
  `detect_key` 가 고른 키를 담을 수 없어 자기참조 구조체가 된다.

### 최종 검증

| 항목 | 결과 |
|---|---|
| PKG2 `Item_000.wz` @580 `ItemOption.img` | 스칼라 **24,726개** 이름·값·순서 완전 일치 |
| PKG1 `Etc_002.wz` @1123 `soulAmplify.img` | 스칼라 **143개** 완전 일치 |
| 소비 바이트 | 495,715 — C# `size` 와 일치 |
| PKG1 `_Canvas_000.wz` | 트리 1,650개, 첫 이미지 @46,267 — C# 일치 |

회귀 테스트 5개 추가(짝수 길이 ASCII / 홀수 길이 ASCII / 양수 UTF-16 /
BMS 키 선별 / 커서 원위치 복원). 전체 301 passed, clippy 0.

## 오라클이 틀리면 Rust 도 틀려 보인다 — 대조 도구의 함정

검증 도구(`Oracle.cs`)가 부정확하면 **이식 구현을 고치는 쪽으로** 흐른다.
실제로 그랬다.

- 스칼라를 `short`/`int`/`long`/`float`/`double`/`bool`/`string` **전부**에
  채우던 구현이 있었다. int `5` 하나에 7개 필드가 다 찼다.
  더 나쁜 건 float `0.5` 가 `Convert.ToInt16(0.5)` = **0**(banker's rounding)
  으로 "진짜 값이 0" 처럼 보이는데, 비교 로직이 `short` 를 먼저 집었기
  때문이다. 실제로 **73건의 가짜 불일치**가 이 원인으로 만들어졌다.
  → CLR 타입에 대응하는 필드 **하나만** 낸다.
- 재귀 조건이 `n.Value == null` 면 Canvas/Vector 아래 서브트리(Canvas 안의
  mini Property)를 통째로 버린다. `--scalars` 에서 그 아래 스칼라가 사라진다
  — 검증 중 조용히 데이터가 줄어든 것처럼 보인다. → 방출 여부만 필터하고
  재귀는 항상 한다.
- `new Wz_File(path, new Wz_Structure())` 는 `WzStructure.Node` 가 null 이라
  이미지를 못 찾는다. `LoadFile` 로 바꿔야 한다.

## 남은 작업

**주의: "WZ writer"와 "`patch.json`" 는 존재하지 않는다.**
`WzLib` 은 완전한 read-only 다 (`WzWriter`/`WzBinaryWriter` 0건).
`patch.json`/`SavePatchFile` 도 4개 브랜치 전체 `git log --all -S` 로 0건.
`Patcher/` 는 WZ 를 쓰는 게 아니라 **deflate 압축 바이너리 opcode 스트림 +
CRC-32/BZIP2** 형식의 `.patch` 를 만든다. JSON 스키마를 가정하면 존재하지
않는 것을 설계하게 된다 — `RUST_REWRITE_ARCHITECTURE.md:58-62` 가 경고.

미구현 crate (문서 §6.3~6.9 기준):

1. **`wz-db`** — 노드 arena, UOL/링크 해석, 이미지 캐시, 질의 엔진.
   우선순위 최고. `image.rs` 가 UOL 해석을 이 crate 책임으로 명시해 두었고,
   C# 에는 경로 해석기가 **두 벌** 있어(`Db2Host.GetNode` 와
   `DB2NodeExtension.GetNode`) 통합 대상이다.
2. **`wz-diff` / `wz-patch`** — 자연 정렬 + 투 포인터 병합 순회.
   `PngComparison` 3단계, `_inlink`/`_outlink` 링크 해석, CRC-32/BZIP2 정밀도.
3. **`wz-render`** 완성 — 현재 `uv()` 뿐.
4. **게임 도메인** — `wz-chara` / `wz-map` / `wz-avatar` / `wz-anim`.
5. **GUI** — eframe(egui) + wgpu (문서 §9.3).
