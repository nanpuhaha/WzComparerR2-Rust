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

    /// 두 WZ 파일 또는 이미지 비교 (Diff)
    Diff {
        /// 이전 버전 WZ 파일
        old_wz: PathBuf,

        /// 새 버전 WZ 파일
        new_wz: PathBuf,

        /// 이전 이미지 오프셋 (지정 시 단일 이미지 diff)
        #[arg(long)]
        old_offset: Option<u64>,

        /// 새 이미지 오프셋 (지정 시 단일 이미지 diff)
        #[arg(long)]
        new_offset: Option<u64>,

        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// 이미지 내부 리소스 / 캔버스 덤프
    Dump {
        /// WZ 파일 경로
        wz: PathBuf,

        /// 이미지 오프셋
        #[arg(long)]
        offset: u64,

        /// JSON 구조 덤프
        #[arg(long)]
        json: bool,
    },

    /// 29개 아바타 슬롯 및 렌더 앵커 파이프라인 정보
    Avatar {
        /// 앵커/ZMap 정렬 합성 데모 실행
        #[arg(long)]
        demo: bool,

        /// 아바타 합성 배치 및 렌더링 파이프라인 검증 (오프스크린 렌더)
        #[arg(long)]
        render: bool,

        /// 오프스크린 렌더링된 결과를 파일로 저장 (예: output.rgba 또는 output.raw)
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
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
        Command::Diff {
            old_wz,
            new_wz,
            old_offset,
            new_offset,
            json,
        } => cmd_diff(&old_wz, &new_wz, old_offset, new_offset, json),
        Command::Dump { wz, offset, json } => cmd_dump(&wz, offset, json),
        Command::Avatar { demo, render, output } => cmd_avatar(demo, render, output),
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


fn cmd_tree(
    path: &std::path::Path,
    hash_version: u64,
    shallow: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::WzArchive;

    let archive = WzArchive::open_with_hash_version(path, hash_version)?;

    if !json {
        println!("파일: {}", archive.source().describe());
        println!(
            "프로파일: {} / KMST{}",
            archive.detection().reason,
            archive.detection().profile.wz_version
        );
        println!("hashVersion: {:#018x}", archive.detection().hash_version);
        println!("headerSize: {}", archive.detection().detected.header_len);
    }

    let tree = archive.read_tree_with_mode(shallow)?;

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
    use wz_core::WzArchive;

    let archive = WzArchive::open_with_hash_version(path, hash_version)?;
    let root = archive.parse_image_at(offset)?;

    println!("파일: {}", archive.source().describe());
    println!(
        "프로파일: {} / KMST{}",
        archive.detection().reason,
        archive.detection().profile.wz_version
    );
    println!("hashVersion: {:#018x}", archive.detection().hash_version);
    println!("이미지 오프셋: {offset}");
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

    if node.children.is_empty() {
        println!("{pad}{}: {ty}{detail}", node.name);
    } else {
        println!(
            "{pad}{}: {ty}{detail} ({} 자식)",
            node.name,
            node.children.len()
        );
        for ch in &node.children {
            print_node(ch, depth + 1, max_depth);
        }
    }
}

fn dump_node_to_json(node: &wz_core::image::Node) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("name".into(), serde_json::Value::String(node.name.to_string()));

    if let Some(wz_core::image::ImageValue::Property(inner)) = &node.value {
        map.insert("type".into(), serde_json::Value::String("Property".into()));
        let children: Vec<serde_json::Value> = inner.children.iter().map(dump_node_to_json).collect();
        map.insert("children".into(), serde_json::Value::Array(children));
    } else {
        if let Some(val) = &node.value {
            map.insert("type".into(), serde_json::Value::String(val.type_name().into()));
            match val {
                wz_core::image::ImageValue::Str(s) => {
                    map.insert("value".into(), serde_json::Value::String(s.to_string()));
                }
                wz_core::image::ImageValue::Short(v) => {
                    map.insert("value".into(), serde_json::Value::Number((*v).into()));
                }
                wz_core::image::ImageValue::Int(v) => {
                    map.insert("value".into(), serde_json::Value::Number((*v).into()));
                }
                wz_core::image::ImageValue::Long(v) => {
                    map.insert("value".into(), serde_json::Value::Number((*v).into()));
                }
                wz_core::image::ImageValue::Float(v) => {
                    if let Some(n) = serde_json::Number::from_f64(*v as f64) {
                        map.insert("value".into(), serde_json::Value::Number(n));
                    }
                }
                wz_core::image::ImageValue::Double(v) => {
                    if let Some(n) = serde_json::Number::from_f64(*v) {
                        map.insert("value".into(), serde_json::Value::Number(n));
                    }
                }
                wz_core::image::ImageValue::Png(p) => {
                    let mut p_map = serde_json::Map::new();
                    p_map.insert("width".into(), serde_json::Value::Number(p.width.into()));
                    p_map.insert("height".into(), serde_json::Value::Number(p.height.into()));
                    p_map.insert("format".into(), serde_json::Value::Number(p.format.into()));
                    p_map.insert("scale".into(), serde_json::Value::Number(p.scale.into()));
                    p_map.insert("pages".into(), serde_json::Value::Number(p.pages.into()));
                    p_map.insert("data_len".into(), serde_json::Value::Number(p.data_len.into()));
                    map.insert("png".into(), serde_json::Value::Object(p_map));
                }
                wz_core::image::ImageValue::Uol(s) => {
                    map.insert("uol".into(), serde_json::Value::String(s.to_string()));
                }
                wz_core::image::ImageValue::Vector(v) => {
                    let mut v_map = serde_json::Map::new();
                    v_map.insert("x".into(), serde_json::Value::Number(v.x.into()));
                    v_map.insert("y".into(), serde_json::Value::Number(v.y.into()));
                    map.insert("vector".into(), serde_json::Value::Object(v_map));
                }
                _ => {}
            }
        }
        if !node.children.is_empty() {
            let children: Vec<serde_json::Value> = node.children.iter().map(dump_node_to_json).collect();
            map.insert("children".into(), serde_json::Value::Array(children));
        }
    }

    serde_json::Value::Object(map)
}

fn print_diffs(title: &str, diffs: &[wz_diff::DiffItem]) {
    use wz_diff::DiffType;
    println!("{title} (총 {} 건)", diffs.len());
    for d in diffs {
        let tag = match d.diff_type {
            DiffType::Append => "+",
            DiffType::Delete => "-",
            DiffType::Changed => "*",
        };
        println!("[{tag}] {} ({})", d.path, d.description);
    }
}

fn cmd_diff(
    old_wz: &std::path::Path,
    new_wz: &std::path::Path,
    old_offset: Option<u64>,
    new_offset: Option<u64>,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_diff::DiffEngine;

    let (old_off, new_off) = match (old_offset, new_offset) {
        (Some(o1), Some(o2)) => (Some(o1), Some(o2)),
        (Some(o1), None) => (Some(o1), Some(o1)),
        (None, Some(o2)) => (Some(o2), Some(o2)),
        (None, None) => (None, None),
    };

    match (old_off, new_off) {
        (Some(o1), Some(o2)) => {
            let a1 = wz_core::WzArchive::open(old_wz)?;
            let a2 = wz_core::WzArchive::open(new_wz)?;
            let n1 = a1.parse_image_at(o1)?;
            let n2 = a2.parse_image_at(o2)?;
            let diffs = DiffEngine::diff_core_nodes(Some(&n1), Some(&n2), "root");
            if json {
                println!("{}", serde_json::to_string_pretty(&diffs)?);
            } else {
                print_diffs("=== 이미지 노드 차이점 ===", &diffs);
            }
        }
        _ => {
            let a1 = wz_core::WzArchive::open(old_wz)?;
            let a2 = wz_core::WzArchive::open(new_wz)?;
            let t1 = a1.read_tree()?;
            let t2 = a2.read_tree()?;
            let diffs = DiffEngine::diff_dirtree(&t1, &t2, "root");

            if json {
                println!("{}", serde_json::to_string_pretty(&diffs)?);
            } else {
                print_diffs("=== WZ 디렉터리 트리 차이점 ===", &diffs);
            }
        }
    }
    Ok(())
}

fn cmd_dump(
    path: &std::path::Path,
    offset: u64,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = wz_core::WzArchive::open(path)?;
    let root = archive.parse_image_at(offset)?;

    if json {
        let dump_json = dump_node_to_json(&root);
        println!("{}", serde_json::to_string_pretty(&dump_json)?);
    } else {
        println!("=== 이미지 캔버스/노드 덤프 @{offset} ===");
        print_node(&root, 0, 0);
    }
    Ok(())
}

fn cmd_avatar(
    demo: bool,
    render: bool,
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;
    use wz_avatar::{AvatarSlot, Bone, Point, SkinLayer, ZMap};

    println!("=== 29개 MapleStory 아바타 슬롯 ===");
    for i in 0..AvatarSlot::TOTAL_SLOTS {
        if let Some(slot) = AvatarSlot::from_index(i) {
            println!("  [{:02}] {:?}", i, slot);
        }
    }

    let zmap_order = vec![
        "backHair".into(),
        "body".into(),
        "arm".into(),
        "head".into(),
        "hair".into(),
    ];
    let zmap = Arc::new(ZMap::new(zmap_order));
    let mut root = Bone::new("@root");

    let mut body_skin = SkinLayer::new("body");
    body_skin.z = "body".into();
    body_skin.width = 32;
    body_skin.height = 32;
    root.append_bone(body_skin, "navel", Point::new(10, 20));

    let mut head_skin = SkinLayer::new("head");
    head_skin.z = "head".into();
    head_skin.width = 44;
    head_skin.height = 44;
    root.append_bone(head_skin, "navel", Point::new(0, 10));

    let scene = wz_avatar::AvatarScene::new(zmap);

    if demo {
        println!();
        println!("=== 아바타 Z-Map 레이어 조합 & 앵커 정렬 데모 ===");
        let layers = scene.canvas.generate_layers(&root);
        println!("생성된 레이어 수: {}", layers.len());
        for (i, layer) in layers.iter().enumerate() {
            println!(
                "  레이어 #{i}: {} (pos: {:?}, size: {}x{}, prio: {})",
                layer.name, layer.position, layer.width, layer.height, layer.z_index
            );
        }
        let batch = scene.build_batch(&root);
        println!(
            "생성된 GPU 렌더 배치: 정점 {} 개, 인덱스 {} 개 (삼각형 {} 개)",
            batch.vertex_count(),
            batch.index_count(),
            batch.index_count() / 3
        );
    }

    let should_render = render || output.is_some();
    if should_render {
        println!();
        println!("=== wz-render GPU 오프스크린 렌더 파이프라인 검증 ===");
        let mut packer = wz_render::AtlasShelfPacker::new(512, 512);
        let slot = packer
            .allocate(64, 64)
            .ok_or("아틀라스 텍스처 슬롯 할당 실패")?;
        println!(
            "아틀라스 할당 성공: GpuKey({}), Rect [x:{}, y:{}, w:{}, h:{}]",
            slot.key.0, slot.rect.x, slot.rect.y, slot.rect.w, slot.rect.h
        );

        let render_w = 256u32;
        let render_h = 256u32;
        let camera = wz_render::Camera2D::new(render_w as f32, render_h as f32);
        let uniforms = camera.build_uniforms(0.0, 0.0);
        println!(
            "2D 직교 카메라 유니폼 생성 완료: 뷰포트 [{}, {}], 줌 {}",
            camera.viewport_size.x, camera.viewport_size.y, camera.zoom
        );

        let mut sprite_batch = wz_render::SpriteBatch::new();
        let uv = slot.rect.uv(512, 512);
        sprite_batch.push_quad(
            slot.key,
            [64.0, 64.0, 64.0, 64.0],
            0.0,
            uv,
            [1.0, 1.0, 1.0, 1.0],
        );
        sprite_batch.finish();
        println!(
            "스프라이트 배치 빌드 완료: 정점 {}개, 인덱스 {}개, 드로우 커맨드 {}개",
            sprite_batch.vertices.len(),
            sprite_batch.indices.len(),
            sprite_batch.commands.len()
        );

        match wz_render::RenderContext::new_headless() {
            Ok(mut ctx) => {
                let dummy_pixels = vec![255u8; 64 * 64 * 4];
                ctx.create_texture(slot.key, 64, 64, &dummy_pixels)?;
                ctx.update_buffers(&uniforms, &sprite_batch);
                println!("wgpu 렌더 컨텍스트 초기화 및 GPU 버퍼/텍스처 업로드 성공 (Headless)");

                let clear_color = [0.1, 0.1, 0.15, 1.0];
                let pixels = ctx.render_to_rgba(render_w, render_h, &sprite_batch, &uniforms, clear_color)?;
                println!(
                    "오프스크린 렌더링 및 픽셀 리드백 성공: {}x{} (총 {} 바이트)",
                    render_w,
                    render_h,
                    pixels.len()
                );

                if let Some(out_path) = output {
                    std::fs::write(&out_path, &pixels)?;
                    println!("렌더링된 RGBA 원시 버퍼 저장 완료: {}", out_path.display());
                }
            }
            Err(e) => {
                println!("(안내) 호환되는 GPU 어댑터가 없는 환경이므로 실제 디바이스 생성을 건너뜁니다: {e}");
            }
        }
    }
    Ok(())
}
