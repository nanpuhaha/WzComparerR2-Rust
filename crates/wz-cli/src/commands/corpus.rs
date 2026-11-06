pub fn cmd_version() -> Result<(), Box<dyn std::error::Error>> {
    println!("wz {}", env!("CARGO_PKG_VERSION"));
    println!("지원: PKG1/PKG2/MS 컨테이너, AES/SNOW2/ChaCha20");
    println!("      이미지 트리/Canvas/UOL, WZ/DXBC/BC7 디코딩");
    println!("      StringLinker(5 roots), 아바타/맵 데이터, C# 오라클 차분 하네스");
    println!(
        "미구현: WinForms UI, Common Controls 그래픽(Device), \
         ItemTooltip 텍스트 래스터/GDI+ 감마 합성"
    );
    println!(
        "일부: OpenAPI helpers/HTTP(목 서버 검증, 실 키 필요), Network \
         RC4/프레임/패킷 JSON/로그인·재접속 루프백, PluginBase FindWz/파일 탐색/PE 메타데이터/인스턴스화/리본 주입/이벤트·무효화/탭·명령 수명주기, \
         LuaConsole 샌드박스/CLR 브리지/델리게이트/문법 강조/에디터 패널/거터/콘솔 실행/출력·이력/취소, TextRenderer 레이아웃 코어/아이콘 확대·블록 합성 픽셀/입력 히트테스트/드로잉 기하/툴팁 세로 배치/서식 파서/#t 링크/속성 문자열/부호 변형/2열 포맷/점선·분류명/스킬 요구량/가격 표기/축약/몹 스킬명/소형 포맷터/오프스크린 렌더/스프라이트 기하/CPU 합성/GPU 오프스크린/프레임 베이킹"
    );
    println!("데이터 없음: Skill common, WorldMap QuestLimit/Fog, Familiar, BIN");
    println!("원본 없음: weather (원본 C# 소스에 구현 없음)");
    Ok(())
}
pub fn cmd_fmt(n: usize) -> Result<(), Box<dyn std::error::Error>> {
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
pub fn cmd_probe(which: &str, len: usize, json: bool) -> Result<(), Box<dyn std::error::Error>> {
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
pub fn cmd_probe_json(which: &str, len: usize) -> Result<(), Box<dyn std::error::Error>> {
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
pub fn hexdump(data: &[u8]) {
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
pub fn cmd_corpus(out: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "합성 코퍼스 생성은 `cargo run -p corpus-builder -- --out {}` 를 쓰세요.",
        out.display()
    );
    Ok(())
}
pub fn cmd_detect(path: &std::path::Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
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
