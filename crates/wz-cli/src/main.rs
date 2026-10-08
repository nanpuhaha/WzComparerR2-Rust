//! `wz` — 헤드리스 CLI.
//!
//! 리포트 §4.1 원칙 9: **헤드리스 CLI 가 진짜 제품**이고 GUI 는 얇은 뷰다.
//! 모든 기능은 CLI 에서도 가능해야 한다. 덕분에:
//!
//! - GUI 이전에 로직 완성도를 올릴 수 있다
//! - 자동화/스크립팅이 된다
//! - 서버/컨테이너 환경에서도 동작한다 (GUI 불가)
//!
//! 현재 구현: `version`, `fmt` (간격), `probe` (암호 상수/키스트림 진단),
//! `corpus` (합성 코퍼스 생성).

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "wz",
    version,
    about = "WzComparerR2 — WZ 아카이브 도구 (헤드리스)",
    long_about = "MapleStory WZ 파일을 읽고 비교하는 도구.\nGUI 없이 모든 기능을 수행한다."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// 버전과 빌드 정보
    Version,

    /// 주어진 수의 포맷 정렬 (정렬이 결정적인지 확인)
    Fmt {
        /// 줄 수
        #[arg(short, long, default_value_t = 20)]
        n: usize,
    },

    /// 암호 계층 진단 — 키스트림을 hexdump 로 출력
    ///
    /// AES/SNOW2/ChaCha20/Pkg2DirStringKey 의 키스트림을 눈으로 검증한다.
    /// C# 오라클의 대응 출력과 나란히 놓고 비교하는 용도.
    Probe {
        /// 키 종류 (kms|gms|snow2|chacha20|chacha20-reset|pkg2key)
        #[arg(short, long, default_value = "kms")]
        which: String,

        /// JSON 으로 출력 (오라클 diff 용)
        ///
        /// 함정: 기본 출력은 사람이 읽는 hexdump 다. `WzComparerR2.Oracle` 와
        /// 기계적으로 diff 하려면 형식이 같아야 하므로 이 플래그가 필요하다.
        #[arg(long)]
        json: bool,
        /// 출력할 바이트 수
        #[arg(short, long, default_value_t = 64)]
        len: usize,
    },

    /// 합성 테스트 코퍼스 생성
    Corpus {
        /// 출력 디렉터리
        #[arg(short, long)]
        out: PathBuf,
    },

    /// 포맷 프로파일 감지
    Detect {
        /// WZ 파일 경로
        wz: PathBuf,

        /// JSON 으로 출력 (오라클 diff 용)
        #[arg(long)]
        json: bool,
    },

    /// 디렉터리 트리 출력
    ///
    /// `hashVersion` 을 자동 검출한 뒤 엔트리 테이블을 읽는다.
    /// `--hash-version` 을 주면 그 값을 강제한다 (오라클 대조용).
    Tree {
        /// WZ 파일 경로
        wz: PathBuf,

        /// `hashVersion` 을 강제 (0 이면 자동 검출)
        #[arg(long, default_value_t = 0)]
        hash_version: u64,

        /// 트리 전체가 아니라 루트 블록만 출력
        #[arg(long)]
        shallow: bool,

        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// 이미지 블록의 노드 트리 출력
    Image {
        /// WZ 파일 경로
        wz: PathBuf,

        /// 이미지 파일 절대 오프셋 (트리에서 얻은 값)
        #[arg(long)]
        offset: u64,

        /// `hashVersion` (트리 명령과 동일하게 자동 검출)
        #[arg(long, default_value_t = 0)]
        hash_version: u64,

        /// 최대 깊이 (0 이면 무제한)
        #[arg(long, default_value_t = 0)]
        max_depth: usize,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Version => cmd_version(),
        Command::Fmt { n } => cmd_fmt(n),
        Command::Probe { which, len, json } => cmd_probe(&which, len, json),
        Command::Corpus { out } => cmd_corpus(&out),
        Command::Detect { wz, json } => cmd_detect(&wz, json),
        Command::Tree {
            wz,
            hash_version,
            shallow,
            json,
        } => cmd_tree(&wz, hash_version, shallow, json),
        Command::Image {
            wz,
            offset,
            hash_version,
            max_depth,
        } => cmd_image(&wz, offset, hash_version, max_depth),
    };

    if let Err(e) = result {
        eprintln!("오류: {e}");
        std::process::exit(1);
    }
}

fn cmd_version() -> Result<(), Box<dyn std::error::Error>> {
    println!("wz {}", env!("CARGO_PKG_VERSION"));
    println!("profile phase: 1 (포맷/암호 기반)");
    println!("지원: AES-ECB keystream, SNOW2, ChaCha20(32-bit counter), PKG2 dir key");
    println!("미구현: PKG1/PKG2/MS 컨테이너, 이미지 노드 (Phase 1.3~1.11 예정)");
    Ok(())
}

/// 100k 줄로 결정적 정렬 속도 확인 (리포트 §7 병렬화 대상).
fn cmd_fmt(n: usize) -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Instant;
    use wz_core::text::sort_natural;

    let mut v: Vec<String> = Vec::with_capacity(n);
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for i in 0..n {
        // 결정적 의사 난수 (여기선 `rand` 크레이트를 쓰지 않는다 — deny.toml 금지)
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        v.push(format!("img{:08x}.img/{:x}", x, i % 997));
    }

    let t0 = Instant::now();
    sort_natural(&mut v, |s| s.as_str());
    let dt = t0.elapsed();

    // 결정성 확인
    let mut w = v.clone();
    sort_natural(&mut w, |s| s.as_str());
    assert_eq!(v, w, "정렬이 멱등해야 한다");

    println!("{}개 정렬: {:?}", v.len(), dt);
    for s in v.iter().take(10) {
        println!("  {s}");
    }
    Ok(())
}

fn cmd_probe(which: &str, len: usize, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    // 오라클과 diff 할 수 있게 JSON 경로를 먼저 연다.
    if json {
        return cmd_probe_json(which, len);
    }
    use wz_core::crypto::aes::AesKeyStream;
    use wz_core::crypto::chacha::ChaCha20Stream;
    use wz_core::crypto::pkg2_key::Pkg2DirStringKey;
    use wz_core::crypto::snow2::Snow2;
    use wz_core::crypto::Decryptor;

    match which.to_ascii_lowercase().as_str() {
        "kms" | "gms" => {
            let ks = if which.eq_ignore_ascii_case("kms") {
                AesKeyStream::kms()
            } else {
                AesKeyStream::gms()
            };
            println!("# {which} AES-256-ECB 키스트림 (IV 4바이트 반복, CBC 체인)");
            hexdump(&ks.keystream_prefix(len));
        }
        "snow2" => {
            let key: [u8; 16] = std::array::from_fn(|i| i as u8);
            let mut s = Snow2::new(&key, &[]).encryptor();
            let mut buf = vec![0u8; len];
            s.transform(&mut buf);
            println!("# SNOW 2.0 키스트림 (key=00..0f, no IV, 0 입력)");
            hexdump(&buf);
        }
        "chacha20" => {
            let key: [u8; 32] = std::array::from_fn(|i| i as u8);
            let mut s = ChaCha20Stream::new(&key, &[0u8; 12]);
            let mut buf = vec![0u8; len];
            s.apply_keystream(&mut buf);
            println!("# ChaCha20 키스트림 (key=00..1f, nonce=0, counter=0)");
            hexdump(&buf);
        }
        "chacha20-reset" => {
            let key: [u8; 32] = std::array::from_fn(|i| i as u8);
            let mut s = ChaCha20Stream::entry_table_reader(&key, &[0u8; 12]);
            let mut buf = vec![0u8; len];
            s.apply_keystream(&mut buf);
            println!("# ChaCha20 (MS v2 entry_table_reader: 64바이트마다 카운터 리셋)");
            hexdump(&buf);
        }
        "pkg2key" => {
            let k = Pkg2DirStringKey::kmst1198();
            println!("# Pkg2DirStringKey(0xDEADBEEF), 8바이트 주기");
            let mut line = Vec::new();
            for i in 0..len {
                line.push(k.key_byte(i as u64));
            }
            hexdump(&line);
        }
        other => {
            return Err(format!(
                "알 수 없는 대상: {other} (kms|gms|snow2|chacha20|chacha20-reset|pkg2key)"
            )
            .into());
        }
    }
    Ok(())
}

/// 키스트림을 JSON 으로 출력 — `WzComparerR2.Oracle probe` 와 같은 형태.
///
/// # 왜 별도 함수인가
///
/// 사람이 읽는 hexdump(`hexdump()`)와 JSON 은 같은 값을 두 형식으로 내는
/// 거지만, JSON 이 **일관된 키 이름**을 쓰면 오라클과 기계적으로 diff 할 수 있다.
/// 키 이름은 오라클(`WzComparerR2.Oracle/Oracle.cs`)과 **반드시** 같아야 한다.
fn cmd_probe_json(which: &str, len: usize) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::crypto::aes::AesKeyStream;
    use wz_core::crypto::chacha::ChaCha20Stream;
    use wz_core::crypto::pkg2_key::Pkg2DirStringKey;
    use wz_core::crypto::snow2::Snow2;
    use wz_core::crypto::Decryptor;

    let hex = |b: &[u8]| -> String {
        b.iter()
            .map(|x| format!("{x:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    };

    // 함정: 키스트림 생성 함수는 시드/IV 를 인자로 받는다. 오라클이 쓰는 것과
    // **같은 값**을 써야 출력이 일치한다.
    let value = match which.to_ascii_lowercase().as_str() {
        "kms" => Some(hex(&AesKeyStream::kms().keystream_prefix(len))),
        "gms" => Some(hex(&AesKeyStream::gms().keystream_prefix(len))),
        "bms" => Some(hex(&vec![0u8; len])), // 항등 (원본 Wz_NonOpCryptoKey)
        "snow2" => {
            let key: [u8; 16] = std::array::from_fn(|i| i as u8);
            let mut s = Snow2::new(&key, &[]).encryptor();
            let mut buf = vec![0u8; len];
            s.transform(&mut buf);
            Some(hex(&buf))
        }
        "chacha20" => {
            let key: [u8; 32] = std::array::from_fn(|i| i as u8);
            let mut s = ChaCha20Stream::new(&key, &[0u8; 12]);
            let mut buf = vec![0u8; len];
            s.apply_keystream(&mut buf);
            Some(hex(&buf))
        }
        "chacha20-reset" => {
            let key: [u8; 32] = std::array::from_fn(|i| i as u8);
            let mut s = ChaCha20Stream::entry_table_reader(&key, &[0u8; 12]);
            let mut buf = vec![0u8; len];
            s.apply_keystream(&mut buf);
            Some(hex(&buf))
        }
        "pkg2key" => {
            let k = Pkg2DirStringKey::kmst1198();
            let buf: Vec<u8> = (0..len).map(|i| k.key_byte(i as u64)).collect();
            Some(hex(&buf))
        }
        // 오라클이 `Wz_CryptoKeyType.KMST1198` 로 쓰는 것과 **같은 키**다
        // (`Wz_Crypto.cs`: `new Pkg2DirStringKey(0xDEADBEEF)`).
        // 8바이트 주기 키스트림이라 짧게 반복된다.
        "kmst1198" => {
            let k = Pkg2DirStringKey::kmst1198();
            let buf: Vec<u8> = (0..len).map(|i| k.key_byte(i as u64)).collect();
            Some(hex(&buf))
        }
        other => return Err(format!("알 수 없는 대상: {other}").into()),
    };

    let v = value.unwrap_or_default();
    println!("{{");
    println!("  \"{which}\": \"{v}\"");
    println!("}}");
    Ok(())
}
fn hexdump(data: &[u8]) {
    for (row, chunk) in data.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02X}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if (0x20..0x7F).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("{:08x}  {:<47}  |{}|", row * 16, hex.join(" "), ascii);
    }
}

fn cmd_corpus(out: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "합성 코퍼스 생성은 `cargo run -p corpus-builder -- --out {}` 를 쓰세요.",
        out.display()
    );
    Ok(())
}

fn cmd_detect(path: &std::path::Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::detect::detect;

    let src = wz_pool::open_file(path)?;
    let file_size = src.len()?;

    if json {
        // 함정: 사람이 읽는 출력의 **한국어 라벨** 은 콘솔 인코딩(cp949 등)으로
        // 깨질 수 있다. 스크립트가 파싱하려면 키가 ASCII 인 JSON 이 안전하다.
        //
        // `DirStartPosition` 은 오프셋 계산이 아니라 **헤더 해석** 결과다
        // (원본 `Wz_File.cs:210,219`). C# `Wz_Header.DirStartPosition` 과
        // 같은 값이어야 하므로 여기서 직접 계산한다.
        let d = detect(src.as_ref());
        let dir_start = match &d {
            Ok(d) if d.profile.starts_with("PKG1") => {
                d.header_len as i64 + if d.capabilities.encver_missing { 0 } else { 2 }
            }
            Ok(d) if d.capabilities.pkg2_random_header || d.capabilities.pkg2_random_header64 => {
                d.header_len as i64
            }
            Ok(d) => d.header_len as i64 + 8,
            Err(_) => -1,
        };

        let hex = |v: Option<u64>| match v {
            Some(h) => format!("\"{h:#018x}\""),
            None => "null".to_string(),
        };

        println!("{{");
        println!("  \"fileSize\": {file_size},");
        println!("  \"dirStartPosition\": {dir_start},");
        match &d {
            Ok(d) => {
                println!("  \"profile\": {},", serde_json::to_string(&d.profile)?);
                println!("  \"headerSize\": {},", d.header_len);
                println!("  \"capabilities\": {},", d.capabilities.bits());
                println!("  \"hash1\": {},", hex(d.hash1));
                println!("  \"hash2\": {}", hex(d.hash2));
            }
            Err(e) => {
                println!("  \"error\": {},", serde_json::to_string(&e.to_string())?);
                println!("  \"headerSize\": null,");
                println!("  \"capabilities\": null,");
                println!("  \"hash1\": null,");
                println!("  \"hash2\": null");
            }
        }
        println!("}}");
        return Ok(());
    }

    println!("파일: {}", src.describe());
    println!("크기: {file_size} 바이트");

    match detect(src.as_ref()) {
        Ok(d) => {
            println!("프로파일: {}", d.profile);
            if let Some(h) = d.hash1 {
                println!("hash1: {h:#018x}");
            }
            if let Some(h) = d.hash2 {
                println!("hash2: {h:#018x}");
            }
            Ok(())
        }
        Err(e) => Err(format!("감지 실패: {e}").into()),
    }
}

/// 감지 결과에서 만든 파싱 컨텍스트.
struct Resolved {
    detection: wz_core::detect_version::Detection,
    calc: Box<dyn wz_core::offset::OffsetCalc>,
    pkg2: bool,
    dir_start: u64,
}

/// 헤더 감지 → 프로파일 판별 → 오프셋 계산기.
///
/// # 왜 자동 감지가 기본인가
///
/// `hashVersion` 을 모르면 어떤 오프셋도 복호화할 수 없다. C# 의
/// `WzVersionVerifier` 와 같은 접근을 쓴다: 후보를 나열하고 **오프셋이
/// 겹치지 않는 것**으로 하나를 고른다.
///
/// `--hash-version` 은 그 과정을 건너뛰고 강제한다. 오라클 대조나
/// 성능 측정용이다 — 틀린 값을 넣으면 조용히 엉뚱한 트리가 나오므로
/// 검증 단계에서 걸러진다.
fn resolve(
    src: &std::sync::Arc<dyn wz_pool::ByteSource>,
    hash_version: u64,
) -> Result<Resolved, Box<dyn std::error::Error>> {
    use wz_core::detect_version::{detect_version, Options};

    // 함정: KMST1199–1201 과 KMST1200 의 `hashVersion` 은 2³² 전수 탐색이라
    // **프로필당 ~18초**다 (12코어 기준, rayon 256청크). 게다가 표준 PKG2 헤더는
    // 1201→1200→1199→1198 순으로 시도하므로 최악엔 스윕을 여러 번 한다.
    //
    // 사용자가 멈추지 않은 것처럼 보이지 않게 미리 알린다.
    let detected0 = wz_core::detect::detect(src.as_ref()).map_err(|e| format!("감지 실패: {e}"))?;
    let expensive = detected0.profile == "PKG2_STANDARD_HEADER";
    if expensive && hash_version == 0 {
        eprintln!(
            "참고: 이 파일은 표준 PKG2 헤더라 `hashVersion` 역산을 시도합니다.\n\
             KMST1199–1201 은 2^32 전수 탐색이라 수십 초 걸릴 수 있습니다.\n\
             --hash-version 을 주면 건너뜁니다."
        );
    }

    let opts = Options {
        max_depth: 1,
        hash_version,
    };
    let t0 = std::time::Instant::now();
    let detection = detect_version(src, opts).map_err(|e| format!("버전 자동 감지 실패: {e}"))?;
    if expensive {
        eprintln!("  · 감지 소요 {:?}", t0.elapsed());
    }

    let calc = detection.profile.offset_calc(
        detection.detected.header_len,
        detection.hash_version,
        detection.detected.hash1.unwrap_or(0),
    );
    let is_pkg1 = detection.profile.format == wz_core::profile::WzFileFormat::Pkg1;
    let dir_start = if is_pkg1 {
        detection.detected.header_len as u64
            + if detection.detected.capabilities.encver_missing {
                0
            } else {
                2
            }
    } else if detection.detected.capabilities.pkg2_random_header
        || detection.detected.capabilities.pkg2_random_header64
    {
        detection.detected.header_len as u64
    } else {
        detection.detected.header_len as u64 + 8
    };

    Ok(Resolved {
        calc,
        pkg2: !is_pkg1,
        dir_start,
        detection,
    })
}
fn cmd_tree(
    path: &std::path::Path,
    hash_version: u64,
    shallow: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::dirtree::*;

    let src = wz_pool::open_file_arc(path)?;
    let r = resolve(&src, hash_version)?;

    if !json {
        println!("파일: {}", src.describe());
        println!(
            "프로파일: {} / KMST{}",
            r.detection.reason, r.detection.profile.wz_version
        );
        println!("hashVersion: {:#018x}", r.detection.hash_version);
        println!("headerSize: {}", r.detection.detected.header_len);
        println!("dirStart: {}", r.dir_start);
    }

    /// PKG2 V2 이름 키스트림 입력값을 감지 결과에서 뽑는다.
    ///
    /// # 왜 프로파일별로 가를까
    ///
    /// PKG2 V2 문자열(`ReadPkg2DirStringV2`)을 쓰는 프로파일은 1205/1206 뿐이다
    /// (`WzVersionProfile.cs:505,650` -> `allNamesUseV2: true`). 다른 프로파일은
    /// WZ 표준 문자열을 쓰므로 키스트림을 만들면 안 된다 — 길이 접두사가 1바이트로
    /// 달라서 **전부** 깨진다.
    fn pkg2_name_key_material(
        d: &wz_core::detect_version::Detection,
    ) -> Option<Pkg2NameKeyMaterial> {
        let v = d.profile.wz_version;
        if !(v == 1205 || v == 1206) {
            return None;
        }
        Some(Pkg2NameKeyMaterial {
            hash1: d.detected.hash1?,
            hash_version: d.hash_version,
        })
    }

    let ctx = DirTreeContext {
        detected: &r.detection.detected,
        offset_calc: Some(r.calc.as_ref()),
        dir_start: r.dir_start,
        pkg2: r.pkg2,
        read_offset_table: r.pkg2,
        name_position: match r.detection.profile.name_position {
            wz_core::profile::EntryNamePosition::AfterData => EntryNamePosition::AfterData,
            wz_core::profile::EntryNamePosition::BeforeData => EntryNamePosition::BeforeData,
        },
        // PKG2 V2 이름 키스트림 입력값 (1205/1206 프로파일).
        //
        // # 함정: 키는 **위치마다** 다르다
        //
        // `Pkg2DirStringKeyV5.ApplyState(filePosition)` 이 이름마다 호출되므로
        // 키 하나만 만들어 두면 두 번째 이름부터 틀린다. 원본 입력값을 넘겨
        // 호출 시점에 계산하게 한다. 위치는 절대 오프셋이 아니라 `dirStart`
        // 기준 **상대값**이다 (디렉터리는 `PartialStream` 위에서 읽힌다).
        pkg2_name_key_material: pkg2_name_key_material(&r.detection),
    };
    let mut c = wz_pool::ReadCtx::new(src.clone());
    let parser = DirTreeParser::new(ctx);
    let tree = if shallow {
        let mut c2 = wz_pool::ReadCtx::new(src.clone());
        c2.seek(r.dir_start)?;
        DirTree {
            root: parser.read_root_only(&mut c2)?,
            blocks: Vec::new(),
        }
    } else {
        parser.parse(&mut c)?
    };

    if json {
        let out = tree_to_json(&tree);
        println!("{out}");
    } else {
        print_tree(&tree, 0);
    }
    Ok(())
}

fn print_tree(tree: &wz_core::dirtree::DirTree, indent: usize) {
    use wz_core::dirtree::node_type;
    let pad = "  ".repeat(indent);
    println!("{}./", pad);
    for e in &tree.root.entries {
        let kind = match e.node_type {
            node_type::DIRECTORY => "dir ",
            node_type::IMAGE_NAMED_BY_REF => "img*",
            _ => "img ",
        };
        let name = e.name.as_deref().unwrap_or("<참조>");
        match e.offset {
            Some(off) => println!("{pad}  {kind} {name}  @{off} ({} 바이트)", e.data_len),
            None => println!("{pad}  {kind} {name}  @? (오프셋 미해결)"),
        }
    }
    for b in &tree.blocks {
        println!("{}├─ 하위 블록 ({} 엔트리)", pad, b.entries.len());
        for e in &b.entries {
            let kind = match e.node_type {
                node_type::DIRECTORY => "dir ",
                node_type::IMAGE_NAMED_BY_REF => "img*",
                _ => "img ",
            };
            let name = e.name.as_deref().unwrap_or("<참조>");
            match e.offset {
                Some(off) => println!("{pad}│  {kind} {name}  @{off}"),
                None => println!("{pad}│  {kind} {name}  @?"),
            }
        }
    }
}

fn tree_to_json(tree: &wz_core::dirtree::DirTree) -> String {
    use wz_core::dirtree::node_type;
    let block_json = |b: &wz_core::dirtree::DirBlock| {
        let items: Vec<String> = b
            .entries
            .iter()
            .map(|e| {
                let ty = match e.node_type {
                    node_type::DIRECTORY => "dir",
                    node_type::IMAGE_NAMED_BY_REF => "image_ref",
                    _ => "image",
                };
                format!(
                    r#"{{"type":"{ty}","name":{},"offset":{},"len":{},"cs32":{}}}"#,
                    serde_json::to_string(e.name.as_deref().unwrap_or("")).unwrap(),
                    e.offset.map(|o| o.to_string()).unwrap_or("null".into()),
                    e.data_len,
                    e.cs32
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    };
    let blocks: Vec<String> = tree.blocks.iter().map(block_json).collect();
    format!(
        r#"{{"root":{},"subdirs":[{}]}}"#,
        block_json(&tree.root),
        blocks.join(",")
    )
}

fn cmd_image(
    path: &std::path::Path,
    offset: u64,
    hash_version: u64,
    max_depth: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::image::ImageParser;

    let src = wz_pool::open_file_arc(path)?;
    let r = resolve(&src, hash_version)?;
    let len = src.len()?;
    if offset >= len {
        return Err(format!("오프셋 {offset} 가 파일 크기 {len} 를 넘는다").into());
    }

    let mut c = wz_pool::ReadCtx::new(src.clone());
    c.seek(offset)?;
    // 함정: 복호화기를 넘기지 않으면 문자열이 깨져 루트 태그(`Property`)
    // 비교가 실패한다. 파일·오프셋을 주면 BMS/KMS/GMS 중 맞는 키를 고른다.
    //
    // `declared` 는 구조체에 키가 명시된 경우(`Wz_Structure.Pkg1EncType`)
    // 뿐이라 CLI 에서는 `None` 이다.
    //
    // 함정: `string_ref_offset` = 이미지 시작 오프셋이어야 한다.
    // `0x1B` 문자열 참조는 **이미지 기준 상대 오프셋**인데, C# 은 이미지에
    // PartialStream 을 씌워서 그게 자동으로 맞춰진다. Rust 은 파일 전체
    // 스트림을 쓰므로 기준을 넘겨야 하고, 안 넘기면 파일 앞부분을 문자열로
    // 읽어 엉뚱한 태그가 나온다.
    let mut parser = ImageParser::new(None)
        .detect_key(&mut c, offset, None)?
        .with_string_ref_offset(offset);
    let root = parser
        .parse_image_root(&mut c)
        .map_err(|e| format!("이미지 파싱 실패 @{offset}: {e}"))?;

    println!("파일: {}", src.describe());
    println!(
        "프로파일: {} / KMST{}",
        r.detection.reason, r.detection.profile.wz_version
    );
    println!("hashVersion: {:#018x}", r.detection.hash_version);
    println!("이미지 오프셋: {offset}");
    println!("소비 바이트: {}", parser.bytes_consumed());
    println!();
    print_node(&root, 0, max_depth);
    Ok(())
}

fn print_node(node: &wz_core::image::Node, depth: usize, max_depth: usize) {
    if max_depth != 0 && depth > max_depth {
        return;
    }
    let pad = "  ".repeat(depth);
    let ty = node
        .value
        .as_ref()
        .map(|v| v.type_name())
        .unwrap_or("Property");

    // 함정: 중첩 `Property` 는 `children` 이 아니라 `value` 안에 들어간다
    // (`parse_extended` → `ImageValue::Property`). 여기서 재귀하지 않으면
    // 스칼라가 전부 안 보인다 — 검증 단계에서 가장 흔한 착각 원인.
    if let Some(wz_core::image::ImageValue::Property(inner)) = &node.value {
        println!(
            "{pad}{}: Property ({} 자식)",
            node.name,
            inner.children.len()
        );
        for ch in &inner.children {
            print_node(ch, depth + 1, max_depth);
        }
        return;
    }

    if node.children.is_empty() {
        let detail = match &node.value {
            Some(wz_core::image::ImageValue::Str(s)) => format!(" = {s:?}"),
            Some(wz_core::image::ImageValue::Short(v)) => format!(" = {v}"),
            Some(wz_core::image::ImageValue::Int(v)) => format!(" = {v}"),
            Some(wz_core::image::ImageValue::Float(v)) => format!(" = {v}"),
            Some(wz_core::image::ImageValue::Double(v)) => format!(" = {v}"),
            Some(wz_core::image::ImageValue::Long(v)) => format!(" = {v}"),
            Some(wz_core::image::ImageValue::Bool(v)) => format!(" = {v}"),
            Some(wz_core::image::ImageValue::Null) | None => String::new(),
            Some(wz_core::image::ImageValue::Png(p)) => {
                format!(
                    " = {}x{} fmt={} scale={} pages={} ({} 바이트)",
                    p.width, p.height, p.format, p.scale, p.pages, p.data_len
                )
            }
            Some(wz_core::image::ImageValue::Uol(s)) => format!(" = {s}"),
            Some(wz_core::image::ImageValue::Vector(v)) => format!(" = ({}, {})", v.x, v.y),
            _ => String::new(),
        };
        println!("{pad}{}: {ty}{detail}", node.name);
    } else {
        println!("{pad}{}: {ty} ({} 자식)", node.name, node.children.len());
        for ch in &node.children {
            print_node(ch, depth + 1, max_depth);
        }
    }
}
