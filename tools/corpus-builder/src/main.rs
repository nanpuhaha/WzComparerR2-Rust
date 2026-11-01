//! `corpus-builder` CLI.
//!
//! 합성 WZ 코퍼스를 생성해 파일/JSON 매니페스트로 출력한다.
//!
//! ```text
//! cargo run -p corpus-builder -- --out ../tests/corpus/generated --json
//! ```
//!
//! 출력이 **결정적** 이어야 한다 (리포트 §4.1 원칙 6) — 같은 커밋에서 항상
//! 같은 바이트가 나오므로 골든 파일을 커밋할 수 있다.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use corpus_builder::{tex_format, texture_corpus, ContainerHeader, WzBuilder};
use wz_core::crypto::aes::AesKeyStream;

fn main() {
    if let Err(e) = run() {
        eprintln!("오류: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = PathBuf::from("../tests/corpus/generated");
    let mut manifest_json = false;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" | "-o" => {
                out = args.next().ok_or("--out 에 값이 필요합니다")?.into();
            }
            "--json" => manifest_json = true,
            "-h" | "--help" => {
                println!("corpus-builder [--out <dir>] [--json]");
                println!("  합성 WZ 텍스처 코퍼스를 생성한다 (결정적).");
                return Ok(());
            }
            other => return Err(format!("알 수 없는 인자: {other}").into()),
        }
    }

    fs::create_dir_all(&out)?;
    let cases = texture_corpus();

    let mut manifest: Vec<(String, usize, String)> = Vec::new();
    for case in &cases {
        let bytes = case.build();
        let name = format!("{}.bin", case.id());
        fs::write(out.join(&name), &bytes)?;
        manifest.push((name, bytes.len(), case.id()));
    }

    // 스칼라 값 노드 (압축 정수 / 문자열 경로 커버)
    {
        let mut b = WzBuilder::new();
        let k = b.root_property_begin();
        b.value_int("zero", 0);
        b.value_int("one", 1);
        b.value_int("neg_one", -1);
        b.value_int("i127", 127);
        b.value_int("i128", 128);
        b.value_int("imin", i32::MIN);
        b.value_int("imax", i32::MAX);
        b.value_int("big", 100_000);
        b.root_property_end(k, 8);
        let bytes = b.into_vec();
        fs::write(out.join("values_int.bin"), &bytes)?;
        manifest.push(("values_int.bin".into(), bytes.len(), "scalars/int".into()));
    }
    {
        // KMS 암호화 경로 — 이름과 값이 **같은 키**로 암호화된다
        let mut b = WzBuilder::with_key(AesKeyStream::kms());
        let k = b.root_property_begin();
        b.value_string("empty", "");
        b.value_string("ascii", "ascii only");
        b.value_string("korean", "한글 문자열");
        b.value_string("emoji", "🎮 emoji surrogate pair");
        b.value_string("long_128plus", &"x".repeat(300));
        b.value_string("kms_plain", "KMS 암호화 대상");
        b.value_string("kms_128plus", &"y".repeat(200));
        b.root_property_end(k, 7);
        let bytes = b.into_vec();
        fs::write(out.join("values_string_kms.bin"), &bytes)?;
        manifest.push((
            "values_string_kms.bin".into(),
            bytes.len(),
            "scalars/string+kms".into(),
        ));
    }
    {
        // 평문 문자열 (키 없음) — BMS/무암호화 프로파일 대응
        let mut b = WzBuilder::new();
        let k = b.root_property_begin();
        b.value_string("empty", "");
        b.value_string("ascii", "ascii only");
        b.value_string("korean", "한글 문자열");
        b.value_string("emoji", "🎮 emoji surrogate pair");
        b.value_string("long_128plus", &"x".repeat(300));
        b.root_property_end(k, 5);
        let bytes = b.into_vec();
        fs::write(out.join("values_string.bin"), &bytes)?;
        manifest.push((
            "values_string.bin".into(),
            bytes.len(),
            "scalars/string".into(),
        ));
    }
    {
        let mut b = WzBuilder::new();
        let k = b.root_property_begin();
        b.value_float("f_one", 1.0);
        b.value_float("f_neg_half", -0.5);
        b.value_double("d_pi", std::f64::consts::PI);
        b.value_bool("b_true", true);
        b.value_bool("b_false", false);
        b.value_long("l_min", i64::MIN);
        b.value_long("l_max", i64::MAX);
        b.value_null("null");
        b.root_property_end(k, 8);
        let bytes = b.into_vec();
        fs::write(out.join("values_misc.bin"), &bytes)?;
        manifest.push(("values_misc.bin".into(), bytes.len(), "scalars/misc".into()));
    }
    {
        let mut b = WzBuilder::with_key(AesKeyStream::kms());
        let k = b.root_property_begin();
        // 중첩 Property
        let m = b.property_begin("nested");
        b.value_int("leaf", 0x0BAD_C0DEu32 as i32);
        b.property_end(m, 1);
        // UOL
        b.uol("link_rel", "../00002000.img/info");
        b.uol("link_up", "../../String/Item.img");
        // Vector
        b.vector2d("vec", -5, 300);
        // RawData — ver 0 / ver 1 + mini property / ver 1 + 없음
        b.raw_data("raw_v0", &0xDEAD_BEEFu32.to_le_bytes(), 0);
        b.raw_data_with_sub(
            "raw_v1",
            &0x0102_0304u32.to_le_bytes(),
            1,
            Some(("magic", 7)),
        );
        b.raw_data("raw_v1b", &[1, 2, 3], 1);
        b.root_property_end(k, 6);
        let bytes = b.into_vec();
        fs::write(out.join("values_objects.bin"), &bytes)?;
        manifest.push((
            "values_objects.bin".into(),
            bytes.len(),
            "scalars/objects".into(),
        ));
    }

    // 디렉터리 트리 파일 — `wz tree` 명령 대상
    {
        use wz_core::offset::OffsetCalc;
        let hv = wz_core::verify::compute_pkg1_hash_version(777);
        let calc = wz_core::offset::Pkg1OffsetCalc::new(60, hv);

        // PKG1 헤더: magic + dataSize(i64) + headerSize(i32) + copyright
        //
        // 함정: `encver` 감지기는 `headerSize` 위치의 2바이트를 본다. KMST1132+
        // 파일은 `encver` 가 없고 그 자리에 곧바로 루트 블록(압축 개수)이 온다.
        // 합성 파일도 **그대로** 만들어야 감지가 `PKG1_ENCVER_MISSING` 로 나온다 —
        // 0으로 채우면 `encver = 0` 으로 읽혀 구버전으로 오인된다.
        let base = 60usize;
        let root_entries = vec![
            corpus_builder::DirEntrySpec::dir("Character"),
            corpus_builder::DirEntrySpec::image("base.img", 100),
            corpus_builder::DirEntrySpec::dir("Item"),
            corpus_builder::DirEntrySpec::image("Effect.img", 200),
        ];
        let (root_block, root_pos) =
            corpus_builder::dir_block_pkg1_with_positions(&root_entries, &[0; 4]);
        let char_at = base + root_block.len() + 64;
        let (char_block, char_pos) = {
            let e = [
                corpus_builder::DirEntrySpec::image("00002000.img", 50),
                corpus_builder::DirEntrySpec::image("00002001.img", 60),
            ];
            corpus_builder::dir_block_pkg1_with_positions(&e, &[0, 0])
        };
        let item_at = char_at + char_block.len() + 64;
        let (item_block, item_pos) = {
            let e = [corpus_builder::DirEntrySpec::image("100.img", 70)];
            corpus_builder::dir_block_pkg1_with_positions(&e, &[0])
        };
        let total = item_at + item_block.len();

        let hash_of = |pos: &[usize], block_at: usize, targets: &[u32]| -> Vec<u32> {
            pos.iter()
                .zip(targets)
                .map(|(&p, &t)| calc.encode_offset((block_at + p) as u32, t))
                .collect()
        };

        // 함정: 이미지의 대상 오프셋이 **파일 안에** 있어야 감지 검증이 통과한다.
        // `0` 이나 임의 값을 넣으면 `off < dirStart` 에 걸린다. 그래서 블록 뒤에
        // 실제 페이로드를 배치하고 그 시작점을 목표로 삼는다.
        let mut file = vec![0u8; total];
        let mut image_targets = std::collections::HashMap::new();
        let mut cursor = total;
        for (name, len) in [
            ("base.img", 100usize),
            ("Effect.img", 200),
            ("00002000.img", 50),
            ("00002001.img", 60),
            ("100.img", 70),
        ] {
            file.resize(cursor + len, 0x5A);
            image_targets.insert(name, cursor as u32);
            cursor += len;
        }

        let (root_block, _) = corpus_builder::dir_block_pkg1_with_positions(
            &[
                corpus_builder::DirEntrySpec::dir("Character"),
                corpus_builder::DirEntrySpec::image("base.img", 100),
                corpus_builder::DirEntrySpec::dir("Item"),
                corpus_builder::DirEntrySpec::image("Effect.img", 200),
            ],
            &hash_of(
                &root_pos,
                base,
                &[
                    char_at as u32,
                    image_targets["base.img"],
                    item_at as u32,
                    image_targets["Effect.img"],
                ],
            ),
        );
        let (char_block, _) = corpus_builder::dir_block_pkg1_with_positions(
            &[
                corpus_builder::DirEntrySpec::image("00002000.img", 50),
                corpus_builder::DirEntrySpec::image("00002001.img", 60),
            ],
            &hash_of(
                &char_pos,
                char_at,
                &[image_targets["00002000.img"], image_targets["00002001.img"]],
            ),
        );
        let (item_block, _) = corpus_builder::dir_block_pkg1_with_positions(
            &[corpus_builder::DirEntrySpec::image("100.img", 70)],
            &hash_of(&item_pos, item_at, &[image_targets["100.img"]]),
        );

        file[base..base + root_block.len()].copy_from_slice(&root_block);
        file[char_at..char_at + char_block.len()].copy_from_slice(&char_block);
        file[item_at..item_at + item_block.len()].copy_from_slice(&item_block);

        // PKG1 헤더: magic + dataSize(i64) + headerSize(i32) + copyright(44바이트)
        file[0..4].copy_from_slice(b"PKG1");
        file[12..16].copy_from_slice(&60i32.to_le_bytes());
        for b in file[16..60].iter_mut() {
            *b = b' ';
        }

        // dataSize 는 파일 크기 − headerSize 로 계산 (페이로드 추가분 반영)
        let data_size = (file.len() - 60) as i64;
        file[4..12].copy_from_slice(&data_size.to_le_bytes());

        fs::write(out.join("dirtree_pkg1.wz"), &file)?;
        manifest.push(("dirtree_pkg1.wz".into(), file.len(), "dirtree/pkg1".into()));
    }

    // ── 실제 이미지 본문을 가진 PKG1 컨테이너 ────────────────────────────
    //
    // `dirtree_pkg1.wz` 의 이미지 오프셋은 0x5A 패딩을 가리킨다 — 디렉터리
    // 트리 파싱만 검증할 수 있고 `image`/`stats` 오라클은 돌릴 수 없다
    // (#28 KNOWN GAP 4). 여기서는 진짜 이미지 본문을 넣은 컨테이너를 만든다.
    //
    // 레이아웃은 **연속**이다 (하위 블록 사이 64바이트 패딩 없음) — 실제 WZ 와
    // 같은 배치라 C# `Wz_File.GetDirTree` 의 순차 읽기가 어긋나지 않는다.
    {
        use wz_core::offset::OffsetCalc;
        let hv = wz_core::verify::compute_pkg1_hash_version(777);
        let calc = wz_core::offset::Pkg1OffsetCalc::new(60, hv);

        // canvas.img: root Property { caption:int, canvas:Canvas }
        let canvas_body = {
            let mut b = WzBuilder::new();
            let root = b.root_property_begin();
            b.value_int("caption", 1);
            // 4x4 ARGB8888 — 픽셀 값은 결정적이기만 하면 된다.
            let texels: Vec<u8> = (0..4 * 4 * 4)
                .map(|i| (i as u8).wrapping_mul(17).wrapping_add(3))
                .collect();
            b.canvas_plain("canvas", 4, 4, tex_format::ARGB8888, 0, 1, &texels);
            b.root_property_end(root, 2);
            b.into_vec()
        };

        // 01000000.img: root Property { info:Property { ... } } — oracle stats
        let gear_body = {
            let mut b = WzBuilder::new();
            let root = b.root_property_begin();
            let info = b.property_begin("info");
            b.value_int("reqLevel", 10);
            b.value_int("reqJob", 0);
            b.value_int("incSTR", 3);
            b.value_int("incDEX", 1);
            b.value_int("incINT", 0);
            b.value_int("incLUK", 2);
            b.value_int("tuc", 7);
            b.value_int("price", 1000);
            b.value_string("islot", "Cp");
            b.value_string("vslot", "Cp");
            b.property_end(info, 10);
            b.root_property_end(root, 1);
            b.into_vec()
        };

        let bodies: [(&str, &[u8]); 2] = [
            ("canvas.img", &canvas_body),
            ("01000000.img", &gear_body),
        ];
        let entries: Vec<corpus_builder::DirEntrySpec> = bodies
            .iter()
            .map(|(name, body)| corpus_builder::DirEntrySpec::image(name, body.len() as i32))
            .collect();

        // 1차: 블록 길이와 각 오프셋 필드 위치를 얻는다 (해시 값은 길이에 영향 없음)
        let (root_block, positions) =
            corpus_builder::dir_block_pkg1_with_positions(&entries, &[0; 2]);
        let base = 60usize;
        let mut targets = Vec::with_capacity(bodies.len());
        let mut cursor = base + root_block.len();
        for (_, body) in &bodies {
            targets.push(cursor as u32);
            cursor += body.len();
        }

        // 2차: 실제 이미지 오프셋으로 해시 오프셋을 채운다
        let hashed: Vec<u32> = positions
            .iter()
            .zip(&targets)
            .map(|(&pos, &target)| calc.encode_offset((base + pos) as u32, target))
            .collect();
        let (root_block, _) = corpus_builder::dir_block_pkg1_with_positions(&entries, &hashed);

        let mut file = vec![0u8; cursor];
        file[base..base + root_block.len()].copy_from_slice(&root_block);
        let mut at = base + root_block.len();
        for (_, body) in &bodies {
            file[at..at + body.len()].copy_from_slice(body);
            at += body.len();
        }

        file[0..4].copy_from_slice(b"PKG1");
        let data_size = (file.len() - base) as i64;
        file[4..12].copy_from_slice(&data_size.to_le_bytes());
        file[12..16].copy_from_slice(&(base as i32).to_le_bytes());
        for b in file[16..base].iter_mut() {
            *b = b' ';
        }

        fs::write(out.join("props_pkg1.wz"), &file)?;
        manifest.push((
            "props_pkg1.wz".into(),
            file.len(),
            "props/pkg1".into(),
        ));
    }

    // 컨테이너 헤더 (감지 계층 테스트용 — 전체 writer 는 Phase 1.3)
    {
        let containers = [
            ContainerHeader::pkg1_encver_missing(&[0x01, 0x02, 0x03, 0x04]),
            ContainerHeader::pkg1_with_encver(&[0x01, 0x02, 0x03, 0x04]),
            ContainerHeader::pkg2(0x1122_3344, 0x5566_7788, &[0x01, 0x02, 0x03, 0x04]),
        ];
        for (i, c) in containers.iter().enumerate() {
            let name = format!("container_{i}_{}.bin", c.expected_profile);
            fs::write(out.join(&name), &c.bytes)?;
            println!("  컨테이너: {name} ({} 바이트)", c.bytes.len());
        }
    }

    // 결정성 검증용 체크섬 (고정값이어야 한다 — 바뀌면 회귀)
    let mut total = 0u64;
    for (_, len, _) in &manifest {
        total = total.wrapping_add(*len as u64);
    }

    println!("출력 디렉터리: {}", out.display());
    println!("파일 {}개, 총 {} 바이트", manifest.len(), total);
    println!("결정성 체크섬 (합계): {total:#018x}");

    if manifest_json {
        let json: Vec<_> = manifest
            .iter()
            .map(|(n, l, id)| serde_json::json!({ "file": n, "bytes": l, "id": id }))
            .collect();
        let path = out.join("manifest.json");
        let mut f = fs::File::create(&path)?;
        f.write_all(serde_json::to_string_pretty(&json)?.as_bytes())?;
        println!("매니페스트: {}", path.display());
    }

    Ok(())
}
