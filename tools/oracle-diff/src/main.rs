//! 차분 러너 실행부.
//!
//! 판정 로직은 전부 `oracle_diff` 라이브러리에 있고, 이 파일은 배선만 한다.
//! 왜 셸이 아니라 Rust 인지는 `lib.rs` 맨 위 문서를 본다.
//!
//! ```text
//! scripts/differential.sh  →  oracle-diff <op> [args...]
//!                              ├─ wz   oracle <op> [args...]
//!                              └─ dotnet WzComparerR2.Oracle.dll <op> [args...]
//!                                 ↓ 둘 다 key-sort 정규화
//!                                 ↓ compare() → verdict → 리포트 → 종료 코드
//! ```

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use oracle_diff::{
    compare, exit_code, render_report, resolve_normalizer, ArtifactPaths, Normalizer,
    NormalizerMode, Outcome, ReportContext, SideResult, Verdict, JQ_INSTALL_HINT,
};

/// Rust 쪽 오라클이 있는 디렉터리/파일 이름.
const RUST_ORACLE_BIN: &str = "wz";
/// C# 오라클 프로젝트 디렉터리.
const CSHARP_ORACLE_DIR: &str = "WzComparerR2.Oracle";
/// C# 오라클 어셈블리 파일 이름.
const CSHARP_ORACLE_DLL: &str = "WzComparerR2.Oracle.dll";

const USAGE: &str = "\
oracle-diff — Rust `wz oracle` 와 C# `WzComparerR2.Oracle` 의 차분 판정기

사용법:
  oracle-diff [도구 옵션] <op> [args...]
  scripts/differential.sh <op> [args...]      # 스펙이 정한 진입점

  <op> 뒤의 인자는 **양쪽에 그대로** 전달된다. 도구 옵션은 반드시 <op> 앞에 둔다.
  스펙의 `--format json` 은 어느 쪽도 받지 않는다(`wz oracle image --format` 는 해시
  알고리즘 지정용이다). JSON 이 기본 출력이다.

도구 옵션 (모두 <op> 앞에):
  --csharp-repo <경로>     C# 저장소 체크아웃. 없으면 WZ_CSHARP_REPO 를 본다.
  --csharp-dll  <경로>     빌드된 WzComparerR2.Oracle.dll. 없으면 WZ_CSHARP_DLL.
  --wz-bin      <경로>     빌드된 `wz` 실행 파일. 없으면 WZ_BIN.
  --normalizer  <모드>     auto(기본) | jq | builtin
  --out-dir     <경로>     산출물 디렉터리. 없으면 WZ_DIFF_OUT.
  --replay      <경로>     저장된 산출물로만 다시 판정한다. 양쪽을 실행하지 않는다.
  --keep                    일치해도 산출물을 남긴다.
  --no-build-csharp         C# 오라클을 자동으로 빌드하지 않는다.
  -q, --quiet               리포트를 stdout 이 아니라 산출물 파일에만 쓴다.

환경 변수:
  WZ_CSHARP_REPO, WZ_CSHARP_DLL, WZ_BIN, WZ_DIFF_OUT, WZ_DIFF_NORMALIZER

종료 코드:
  0  일치            1  불일치          2  도구 사용법 오류
  3  비교 불가       4  하네스 오류
";

// ──────────────────────────────────────────────────────────────────── CLI

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
enum NormalizerFlag {
    /// `jq` 가 있으면 `jq -S .`, 없으면 내장 정규화기(경고와 함께).
    #[default]
    Auto,
    /// 스펙 그대로. `jq` 가 없으면 설치 방법을 안내하고 중단한다.
    Jq,
    /// 내장 정규화기를 강제.
    Builtin,
}

impl From<NormalizerFlag> for NormalizerMode {
    fn from(value: NormalizerFlag) -> Self {
        match value {
            NormalizerFlag::Auto => Self::Auto,
            NormalizerFlag::Jq => Self::Jq,
            NormalizerFlag::Builtin => Self::Builtin,
        }
    }
}

#[derive(Debug, Default, clap::Parser)]
#[command(
    about = "Rust/C# 오라클 차분 판정기",
    long_about = USAGE,
    disable_help_flag = false
)]
struct Cli {
    /// 비교할 op (tree image tooltip stats expr diff mapscene patch avatar timeline).
    #[arg(value_name = "OP")]
    op: Option<String>,

    /// 양쪽 오라클에 그대로 넘길 인자.
    #[arg(value_name = "ARGS", trailing_var_arg = true, allow_hyphen_values = true)]
    passthrough: Vec<String>,

    #[arg(long, value_name = "PATH")]
    csharp_repo: Option<PathBuf>,

    #[arg(long, value_name = "PATH")]
    csharp_dll: Option<PathBuf>,

    #[arg(long, value_name = "PATH")]
    wz_bin: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = NormalizerFlag::Auto)]
    normalizer: NormalizerFlag,

    #[arg(long, value_name = "PATH")]
    out_dir: Option<PathBuf>,

    #[arg(long, value_name = "PATH")]
    replay: Option<PathBuf>,

    #[arg(long)]
    keep: bool,

    #[arg(long)]
    no_build_csharp: bool,

    #[arg(long, short)]
    quiet: bool,
}

// ──────────────────────────────────────────────────────────────── 경로 탐색

/// `PATH` 변수 문자열에서 실행 파일을 찾는다.
///
/// 환경변수를 직접 건드리지 않고 문자열을 받는 이유는 이게 순수 함수라서
/// 테스트가 병렬 실행 중 환경에 영향 주지 않기 때문이다.
fn find_in_path(name: &str, path_var: &str) -> Option<PathBuf> {
    let exe_suffixes: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    for dir in std::env::split_paths(path_var) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        for suffix in exe_suffixes {
            let candidate = dir.join(format!("{name}{suffix}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// `jq` 가 이 머신에 있는지.
fn jq_available() -> bool {
    find_in_path("jq", &std::env::var("PATH").unwrap_or_default()).is_some()
}

/// 현재 실행 파일에서 저장소 루트까지 올라간다.
///
/// `target/debug/oracle-diff(.exe)` 에서 두 단계 위가 저장소 루트다. 빌드 산출물이
/// 다른 곳에 있으면 `None` 이고, 그래도 동작해야 하므로 실패로 보지 않는다.
fn repo_root_from(exe: &Path) -> Option<PathBuf> {
    exe.parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|root| root.join("Cargo.toml").is_file() && root.join("crates/wz-cli/Cargo.toml").is_file())
        .map(Path::to_path_buf)
}

/// 빌드된 `wz` 실행 파일 경로를 만든다.
fn rust_oracle_candidate(root: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        format!("{RUST_ORACLE_BIN}.exe")
    } else {
        RUST_ORACLE_BIN.to_string()
    };
    root.join("target").join("debug").join(name)
}

/// C# 오라클 프로젝트 파일.
fn csharp_project(repo: &Path) -> PathBuf {
    repo.join(CSHARP_ORACLE_DIR)
        .join("WzComparerR2.Oracle.csproj")
}

/// 저장소 안에서 빌드된 오라클 dll 을 찾는다.
///
/// `bin` 아래를 재귀 탐색하되 `Debug` + `net8.0-windows` 조합을 가장 선호한다.
fn locate_csharp_dll(repo: &Path) -> Option<PathBuf> {
    let bin_root = repo.join(CSHARP_ORACLE_DIR).join("bin");
    let mut best: Option<(u8, PathBuf)> = None;
    let mut stack = vec![bin_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.file_name() == Some(OsStr::new(CSHARP_ORACLE_DLL)) {
                let text = path.to_string_lossy().to_ascii_lowercase();
                let mut score = 0u8;
                if text.contains("debug") {
                    score += 2;
                }
                if text.contains("net8.0-windows") {
                    score += 1;
                }
                let better = best.as_ref().map(|(s, _)| score > *s).unwrap_or(true);
                if better {
                    best = Some((score, path));
                }
            }
        }
    }
    best.map(|(_, path)| path)
}

// ────────────────────────────────────────────────────────────── 명령 조립

/// Rust 쪽 실행 명령.
fn rust_command(wz_bin: &Path, op: &str, args: &[String]) -> Vec<String> {
    let mut cmd = vec![wz_bin.display().to_string(), "oracle".into(), op.to_string()];
    cmd.extend(args.iter().cloned());
    cmd
}

/// C# 쪽 실행 명령.
fn csharp_command(dll: &Path, op: &str, args: &[String]) -> Vec<String> {
    let mut cmd = vec![
        dotnet_exe().display().to_string(),
        dll.display().to_string(),
        op.to_string(),
    ];
    cmd.extend(args.iter().cloned());
    cmd
}

fn dotnet_exe() -> PathBuf {
    find_in_path("dotnet", &std::env::var("PATH").unwrap_or_default())
        .unwrap_or_else(|| PathBuf::from("dotnet"))
}

/// 명령을 돌려 표준 출력/에러/종료 코드를 모은다.
fn capture(command: &[String]) -> SideResult {
    let Some((program, rest)) = command.split_first() else {
        return SideResult::failed(-1, "빈 명령");
    };
    match Command::new(program).args(rest).output() {
        Ok(out) => SideResult {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        },
        Err(e) => SideResult::failed(-1, format!("`{program}` 실행 실패: {e}")),
    }
}

/// 저장된 산출물 한 벌을 읽는다. 산출물 디렉터리와 리플레이 디렉터리는 같은 형식이다.
fn read_sides(dir: &Path) -> Result<(SideResult, SideResult), String> {
    let read = |stem: &str| -> Result<SideResult, String> {
        let stdout = std::fs::read_to_string(dir.join(format!("{stem}.json")))
            .map_err(|e| format!("{} 읽기 실패: {e}", dir.join(format!("{stem}.json")).display()))?;
        let stderr = std::fs::read_to_string(dir.join(format!("{stem}.stderr"))).unwrap_or_default();
        let code = std::fs::read_to_string(dir.join(format!("{stem}.code")))
            .ok()
            .and_then(|raw| raw.trim().parse::<i32>().ok())
            .unwrap_or(0);
        Ok(SideResult {
            code,
            stdout,
            stderr,
        })
    };
    Ok((read("rust")?, read("csharp")?))
}

/// 산출물 파일 이름 목록. 리플레이가 읽는 이름과 정확히 같아야 한다.
///
/// 이 목록이 계약이다 — `write_artifacts` 는 이 순서대로 쓰고 `read_sides` 는
/// 앞부분을 읽는다. 산출물 디렉터리 하나가 곧 리플레이 디렉터리 하나다.
const ARTIFACT_FILES: &[&str] = &[
    "rust.json",
    "rust.stderr",
    "rust.code",
    "rust.normalized.json",
    "csharp.json",
    "csharp.stderr",
    "csharp.code",
    "csharp.normalized.json",
    "report.txt",
];

fn write_artifacts(dir: &Path, rust: &SideResult, csharp: &SideResult, verdict: &Verdict, report: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for name in ARTIFACT_FILES {
        let body: Option<String> = match *name {
            "rust.json" => Some(rust.stdout.clone()),
            "rust.stderr" => Some(rust.stderr.clone()),
            "rust.code" => Some(rust.code.to_string()),
            "csharp.json" => Some(csharp.stdout.clone()),
            "csharp.stderr" => Some(csharp.stderr.clone()),
            "csharp.code" => Some(csharp.code.to_string()),
            "rust.normalized.json" => verdict.rust_normalized.clone(),
            "csharp.normalized.json" => verdict.csharp_normalized.clone(),
            "report.txt" => Some(report.to_string()),
            _ => None,
        };
        if let Some(body) = body {
            std::fs::write(dir.join(name), body)?;
        }
    }
    Ok(())
}

fn default_out_dir(repo_root: Option<&Path>, op: &str) -> PathBuf {
    if let Ok(from_env) = std::env::var("WZ_DIFF_OUT") {
        if !from_env.trim().is_empty() {
            return PathBuf::from(from_env);
        }
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let base = repo_root
        .map(Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir);
    base.join("target")
        .join("oracle-diff")
        .join(format!("{op}-{stamp}"))
}

/// C# 저장소 HEAD 를 짧게 읽는다. 알 수 없으면 `None` — 판정과는 무관하다.
fn csharp_head(repo: Option<&Path>) -> Option<String> {
    let repo = repo?;
    let out = Command::new("git")
        .args(["-C", &repo.display().to_string(), "rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let head = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if head.is_empty() {
        None
    } else {
        Some(head)
    }
}

// ──────────────────────────────────────────────────────────────────── run

/// 전체 실행. 종료 코드를 돌려준다.
fn run(cli: &Cli) -> i32 {
    let Some(op) = cli.op.as_deref() else {
        println!("{USAGE}");
        return exit_code(Outcome::ToolUsage);
    };

    let repo_root = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(repo_root_from)
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .filter(|cwd| cwd.join("Cargo.toml").is_file())
        });

    // 정규화기를 먼저 정한다 — jq 를 요구하는 모드에서 jq 가 없으면
    // 수 분간 빌드한 뒤에 실패하면 안 된다.
    let normalizer = match resolve_normalizer(cli.normalizer.into(), jq_available()) {
        Ok(n) => n,
        Err(message) => {
            eprintln!("oracle-diff: {message}");
            eprintln!("스펙이 요구하는 `jq -S .` 정규화를 건너뛰면 모든 비교가 틀린 이유로 실패한다.");
            eprintln!("내장 정규화기로 진행하려면 `--normalizer builtin` 또는 `--normalizer auto` 를 쓰세요.");
            return exit_code(Outcome::HarnessError);
        }
    };
    let normalizer_warning = match (normalizer, cli.normalizer) {
        (Normalizer::Builtin, NormalizerFlag::Auto) => Some(format!(
            "jq 를 찾지 못해 내장 정규화기를 사용합니다 (폴백).\n{JQ_INSTALL_HINT}"
        )),
        _ => None,
    };

    let normalize = |raw: &str| -> Result<String, String> {
        match normalizer {
            Normalizer::Builtin => oracle_diff::normalize_builtin(raw),
            Normalizer::Jq => normalize_with_jq(raw),
        }
    };

    // 실행 계획: 리플레이면 양쪽을 돌리지 않는다.
    let (rust, csharp, rust_cmd, csharp_cmd, csharp_repo_dir) = match &cli.replay {
        Some(dir) => match read_sides(dir) {
            Ok((rust, csharp)) => (
                rust,
                csharp,
                vec!["<replay>".to_string()],
                vec!["<replay>".to_string()],
                None,
            ),
            Err(message) => {
                eprintln!("oracle-diff: 리플레이 실패: {message}");
                return exit_code(Outcome::HarnessError);
            }
        },
        None => match resolve_oracles(cli, repo_root.as_deref()) {
            Ok(plan) => (
                plan.rust,
                plan.csharp,
                plan.rust_command,
                plan.csharp_command,
                plan.csharp_repo,
            ),
            Err(message) => {
                eprintln!("oracle-diff: {message}");
                return exit_code(Outcome::HarnessError);
            }
        },
    };

    let verdict = compare(op, &rust, &csharp, &normalize);
    let outcome = verdict.comparison.outcome();

    let out_dir = cli
        .out_dir
        .clone()
        .unwrap_or_else(|| default_out_dir(repo_root.as_deref(), op));
    let keep = cli.keep || outcome != Outcome::Match;
    let head = csharp_head(csharp_repo_dir.as_deref());
    let mut written: Option<ArtifactPaths> = None;

    if keep {
        let ctx = ReportContext {
            op,
            argv: &cli.passthrough,
            rust_command: &rust_cmd,
            csharp_command: &csharp_cmd,
            normalizer: Some(normalizer),
            normalizer_warning: normalizer_warning.as_deref(),
            csharp_head: head.as_deref(),
            artifacts: None,
        };
        let report = render_report(&verdict, &ctx);
        match write_artifacts(&out_dir, &rust, &csharp, &verdict, &report) {
            Ok(()) => {
                written = Some(ArtifactPaths {
                    directory: out_dir.display().to_string(),
                })
            }
            Err(e) => {
                eprintln!("oracle-diff: 산출물 저장 실패 ({}): {e}", out_dir.display());
            }
        }
    }

    let ctx = ReportContext {
        op,
        argv: &cli.passthrough,
        rust_command: &rust_cmd,
        csharp_command: &csharp_cmd,
        normalizer: Some(normalizer),
        normalizer_warning: normalizer_warning.as_deref(),
        csharp_head: head.as_deref(),
        artifacts: written.as_ref(),
    };
    let report = render_report(&verdict, &ctx);

    if cli.quiet {
        println!("{}", verdict.comparison.headline());
        if let Some(paths) = &written {
            println!("report          : {}/report.txt", paths.directory);
        }
    } else {
        println!("{report}");
    }

    exit_code(outcome)
}

/// `jq -S .` 정규화. 스펙이 요구하는 경로.
fn normalize_with_jq(raw: &str) -> Result<String, String> {
    use std::io::Write;
    let mut child = Command::new("jq")
        .args(["-S", "."])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("jq 실행 실패: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("jq stdin 열기 실패")?
        .write_all(raw.as_bytes())
        .map_err(|e| format!("jq 에 입력 전송 실패: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("jq 대기 실패: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "jq 실패: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// 양쪽을 돌린 결과와 "무엇을 어떻게 돌렸는지".
struct Execution {
    rust: SideResult,
    csharp: SideResult,
    rust_command: Vec<String>,
    csharp_command: Vec<String>,
    csharp_repo: Option<PathBuf>,
}

/// 양쪽 오라클을 찾거나(또는 빌드하고) 실행 계획을 만든다.
fn resolve_oracles(cli: &Cli, repo_root: Option<&Path>) -> Result<Execution, String> {
    // C# 저장소
    let csharp_repo = cli
        .csharp_repo
        .clone()
        .or_else(|| non_empty_env("WZ_CSHARP_REPO").map(PathBuf::from));

    // Rust 실행 파일
    let wz_bin = match cli
        .wz_bin
        .clone()
        .or_else(|| non_empty_env("WZ_BIN").map(PathBuf::from))
    {
        Some(explicit) => {
            if !explicit.is_file() {
                return Err(format!("`wz` 실행 파일이 없습니다: {}", explicit.display()));
            }
            explicit
        }
        None => {
            let candidate = repo_root.map(rust_oracle_candidate).ok_or_else(missing_repo_hint)?;
            if !candidate.is_file() {
                eprintln!("oracle-diff: `wz` 를 빌드합니다 (cargo build -p wz-cli --bin wz) …");
                let status = Command::new("cargo")
                    .args(["build", "-p", "wz-cli", "--bin", RUST_ORACLE_BIN])
                    .status()
                    .map_err(|e| format!("cargo 실행 실패: {e}"))?;
                if !status.success() {
                    return Err("`cargo build -p wz-cli --bin wz` 가 실패했습니다".to_string());
                }
            }
            candidate
        }
    };

    // C# dll
    let dll = match cli
        .csharp_dll
        .clone()
        .or_else(|| non_empty_env("WZ_CSHARP_DLL").map(PathBuf::from))
    {
        Some(explicit) => {
            if !explicit.is_file() {
                return Err(format!("C# 오라클 dll 이 없습니다: {}", explicit.display()));
            }
            explicit
        }
        None => locate_or_build_csharp(cli, csharp_repo.as_deref())?,
    };

    let op = cli.op.as_deref().unwrap_or_default();
    let rust_cmd = rust_command(&wz_bin, op, &cli.passthrough);
    let csharp_cmd = csharp_command(&dll, op, &cli.passthrough);

    Ok(Execution {
        rust: capture(&rust_cmd),
        csharp: capture(&csharp_cmd),
        rust_command: rust_cmd,
        csharp_command: csharp_cmd,
        csharp_repo,
    })
}

/// C# 오라클 dll 을 찾고, 없으면(허용되면) 빌드해서 다시 찾는다.
fn locate_or_build_csharp(cli: &Cli, csharp_repo: Option<&Path>) -> Result<PathBuf, String> {
    let repo = csharp_repo.ok_or_else(missing_csharp_hint)?;
    let project = csharp_project(repo);
    if !project.is_file() {
        return Err(format!(
            "C# 오라클 프로젝트가 없습니다: {}\n--csharp-repo 는 `{}` 가 들어 있는 저장소 루트여야 합니다.",
            project.display(),
            CSHARP_ORACLE_DIR
        ));
    }
    if let Some(found) = locate_csharp_dll(repo) {
        return Ok(found);
    }
    if cli.no_build_csharp {
        return Err(format!(
            "빌드된 {CSHARP_ORACLE_DLL} 을 찾지 못했습니다 (--no-build-csharp).\n\
             빌드하려면: dotnet build {} -c Debug",
            project.display()
        ));
    }
    eprintln!("oracle-diff: C# 오라클을 빌드합니다 (dotnet build … -c Debug) …");
    let status = Command::new("dotnet")
        .args(["build", &project.display().to_string(), "-c", "Debug"])
        .status()
        .map_err(|e| format!("dotnet 실행 실패: {e}"))?;
    if !status.success() {
        return Err("C# 오라클 빌드가 실패했습니다".to_string());
    }
    locate_csharp_dll(repo)
        .ok_or_else(|| format!("빌드 후에도 {CSHARP_ORACLE_DLL} 을 찾지 못했습니다"))
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn missing_repo_hint() -> String {
    "`wz` 빌드 대상을 찾지 못했습니다. --wz-bin 으로 빌드된 실행 파일을 넘기거나,\n\
     저장소 안에서 실행하세요 (target/debug/wz 가 있으면 자동으로 빌드하지 않습니다)."
        .to_string()
}

fn missing_csharp_hint() -> String {
    "C# 오라클 위치를 알 수 없습니다. 저장소 경로를 **하드코딩하지 않습니다** — 아래 둘 중 하나로 주세요.\n  \
     --csharp-repo <C# 저장소 루트>   (또는 WZ_CSHARP_REPO)\n  \
     --csharp-dll  <빌드된 {}>   (또는 WZ_CSHARP_DLL)\n\
     핀 커밋과 재핀 절차는 `docs/oracle-spec.md` 의 `## C# 참조 핀` 절에 있습니다."
        .replace("{}", CSHARP_ORACLE_DLL)
        .to_string()
}

fn main() {
    let cli = <Cli as clap::Parser>::parse();
    std::process::exit(run(&cli));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 테스트 전용 임시 디렉터리. 어떤 크레이트도 dev-dependency 를 쓰지 않으므로
    /// `tempfile` 대신 표준 라이브러리만 쓴다.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oracle-diff-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("임시 디렉터리 생성");
        dir
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    // ── 경로 탐색 ────────────────────────────────────────────────────

    #[test]
    fn find_in_path_finds_a_file_that_exists_and_skips_names_without_one() {
        let dir = scratch("find-in-path");
        write(&dir.join("jq.exe"), "x");
        assert_eq!(
            find_in_path("jq", &dir.display().to_string()),
            Some(dir.join(if cfg!(windows) { "jq.exe" } else { "jq" }))
        );
        assert_eq!(find_in_path("nope", &dir.display().to_string()), None);
    }

    #[test]
    fn find_in_path_ignores_empty_entries() {
        assert_eq!(find_in_path("jq", ""), None);
        assert_eq!(find_in_path("jq", "::"), None);
    }

    #[test]
    fn repo_root_is_two_levels_above_the_debug_binary() {
        let root = scratch("repo-root");
        write(&root.join("Cargo.toml"), "[workspace]\n");
        write(&root.join("crates/wz-cli/Cargo.toml"), "[package]\n");
        write(&root.join("target/debug/oracle-diff.exe"), "");
        assert_eq!(repo_root_from(&root.join("target/debug/oracle-diff.exe")), Some(root));
    }

    #[test]
    fn repo_root_is_none_when_the_marker_files_are_missing() {
        assert_eq!(repo_root_from(Path::new("/nowhere/target/debug/wz")), None);
    }

    #[test]
    fn csharp_dll_lookup_prefers_debug_net8_windows() {
        let repo = scratch("dll-lookup");
        write(&repo.join("WzComparerR2.Oracle/bin/Release/net8.0").join(CSHARP_ORACLE_DLL), "x");
        write(
            &repo.join("WzComparerR2.Oracle/bin/Debug/net8.0-windows").join(CSHARP_ORACLE_DLL),
            "x",
        );
        let found = locate_csharp_dll(&repo).expect("찾아야 한다");
        assert!(found.to_string_lossy().contains("net8.0-windows"), "{found:?}");
        assert!(found.to_string_lossy().contains("Debug"), "{found:?}");
    }

    #[test]
    fn csharp_dll_lookup_is_none_when_nothing_is_built() {
        let repo = scratch("dll-missing");
        assert_eq!(locate_csharp_dll(&repo), None);
    }

    #[test]
    fn csharp_project_path_points_at_the_harness_project() {
        let project = csharp_project(Path::new("/csharp"));
        assert!(project.ends_with("WzComparerR2.Oracle/WzComparerR2.Oracle.csproj"), "{project:?}");
    }

    // ── 명령 조립 ────────────────────────────────────────────────────

    #[test]
    fn rust_command_inserts_oracle_between_binary_and_op() {
        let cmd = rust_command(
            Path::new("/bin/wz"),
            "expr",
            &["--expr".to_string(), "1/3".to_string()],
        );
        assert_eq!(cmd[1], "oracle");
        assert_eq!(cmd[2], "expr");
        assert_eq!(&cmd[3..], &["--expr".to_string(), "1/3".to_string()]);
    }

    #[test]
    fn csharp_command_does_not_inject_a_format_flag() {
        // 스펙의 `--format json` 은 어느 쪽도 받지 않는다. 주입하면 image op 이 깨진다.
        let cmd = csharp_command(
            Path::new("/csharp/WzComparerR2.Oracle.dll"),
            "image",
            &["--format".to_string(), "sha256".to_string()],
        );
        assert!(cmd[1].ends_with(CSHARP_ORACLE_DLL));
        assert_eq!(cmd[2], "image");
        assert_eq!(&cmd[3..], &["--format".to_string(), "sha256".to_string()]);
    }

    #[test]
    fn both_commands_receive_identical_argument_tails() {
        let args = vec!["--wz".to_string(), "A.wz".to_string()];
        let rust = rust_command(Path::new("/bin/wz"), "tree", &args);
        let csharp = csharp_command(Path::new("/o.dll"), "tree", &args);
        assert_eq!(rust[3..], csharp[3..]);
    }

    // ── 리플레이 ─────────────────────────────────────────────────────

    #[test]
    fn replay_reads_both_sides_with_their_exit_codes() {
        let dir = scratch("replay-read");
        write(&dir.join("rust.json"), r#"{"a":1}"#);
        write(&dir.join("rust.code"), "0\n");
        write(&dir.join("csharp.json"), "");
        write(&dir.join("csharp.stderr"), "not-implemented: nope");
        write(&dir.join("csharp.code"), "3");

        let (rust, csharp) = read_sides(&dir).unwrap();
        assert_eq!(rust.code, 0);
        assert_eq!(rust.stdout, r#"{"a":1}"#);
        assert_eq!(csharp.code, 3);
        assert!(csharp.stderr.contains("not-implemented"));
    }

    #[test]
    fn replay_reports_a_missing_side_instead_of_silently_treating_it_as_empty() {
        let dir = scratch("replay-missing");
        write(&dir.join("csharp.json"), "{}");
        let err = read_sides(&dir).unwrap_err();
        assert!(err.contains("rust.json"), "{err}");
    }

    #[test]
    fn artifacts_are_written_in_exactly_the_names_replay_reads() {
        let dir = scratch("artifacts");
        let rust = SideResult::ok(r#"{"a":1}"#);
        let csharp = SideResult::failed(3, "nope");
        let verdict = compare("diff", &rust, &csharp, &oracle_diff::normalize_builtin);
        write_artifacts(&dir, &rust, &csharp, &verdict, "report").unwrap();

        for name in ARTIFACT_FILES {
            if name.ends_with(".normalized.json") {
                continue;
            }
            assert!(dir.join(name).is_file(), "{name} 가 없다");
        }
        let (replayed_rust, replayed_csharp) = read_sides(&dir).unwrap();
        assert_eq!(replayed_rust.stdout, rust.stdout);
        assert_eq!(replayed_csharp.code, 3);
    }

    // ── run() ────────────────────────────────────────────────────────

    #[test]
    fn run_without_an_op_is_a_tool_usage_error() {
        let dir = scratch("run-no-op");
        let cli = Cli {
            replay: Some(dir.clone()),
            ..Default::default()
        };
        assert_eq!(run(&cli), 2);
    }

    #[test]
    fn run_on_replay_of_a_csharp_stub_returns_three_not_zero() {
        let dir = scratch("run-stub");
        write(&dir.join("rust.json"), "{}");
        write(&dir.join("rust.code"), "0");
        write(&dir.join("csharp.json"), "");
        write(&dir.join("csharp.stderr"), "not-implemented: WzFileComparer unreachable");
        write(&dir.join("csharp.code"), "3");

        let out = scratch("run-stub-out");
        let cli = Cli {
            op: Some("diff".into()),
            normalizer: NormalizerFlag::Builtin,
            out_dir: Some(out.clone()),
            replay: Some(dir),
            quiet: true,
            ..Default::default()
        };
        assert_eq!(run(&cli), 3, "스텁은 통과도 실패도 아니다");
        assert!(out.join("report.txt").is_file(), "리포트가 남아야 한다");
        let report = std::fs::read_to_string(out.join("report.txt")).unwrap();
        assert!(report.contains("NOT COMPARABLE"), "{report}");
    }

    #[test]
    fn run_on_replay_of_a_mismatch_returns_one_and_saves_both_sides() {
        let dir = scratch("run-mismatch");
        write(&dir.join("rust.json"), r#"{"result":{"value":6.333333333333333}}"#);
        write(&dir.join("rust.code"), "0");
        write(&dir.join("csharp.json"), r#"{"result":{"value":6.333333333333334}}"#);
        write(&dir.join("csharp.code"), "0");

        let out = scratch("run-mismatch-out");
        let cli = Cli {
            op: Some("expr".into()),
            normalizer: NormalizerFlag::Builtin,
            out_dir: Some(out.clone()),
            replay: Some(dir),
            quiet: true,
            ..Default::default()
        };
        assert_eq!(run(&cli), 1);
        assert!(out.join("rust.normalized.json").is_file());
        assert!(out.join("csharp.normalized.json").is_file());
        let report = std::fs::read_to_string(out.join("report.txt")).unwrap();
        assert!(report.contains("MISMATCH"), "{report}");
        assert!(report.contains("/result/value"), "{report}");
    }

    #[test]
    fn run_on_replay_of_equal_json_returns_zero_and_writes_nothing() {
        let dir = scratch("run-match");
        write(&dir.join("rust.json"), r#"{"b":1,"a":2}"#);
        write(&dir.join("rust.code"), "0");
        write(&dir.join("csharp.json"), r#"{"a":2,"b":1}"#);
        write(&dir.join("csharp.code"), "0");

        let out = scratch("run-match-out");
        let cli = Cli {
            op: Some("expr".into()),
            normalizer: NormalizerFlag::Builtin,
            out_dir: Some(out.clone()),
            replay: Some(dir),
            quiet: true,
            ..Default::default()
        };
        assert_eq!(run(&cli), 0, "키 순서만 다른 것은 정규화 후 일치");
        assert!(!out.join("report.txt").exists(), "일치할 때는 산출물을 남기지 않는다");
    }

    #[test]
    fn run_keep_writes_artifacts_even_on_a_match() {
        let dir = scratch("run-keep");
        write(&dir.join("rust.json"), "{}");
        write(&dir.join("rust.code"), "0");
        write(&dir.join("csharp.json"), "{}");
        write(&dir.join("csharp.code"), "0");

        let out = scratch("run-keep-out");
        let cli = Cli {
            op: Some("expr".into()),
            normalizer: NormalizerFlag::Builtin,
            out_dir: Some(out.clone()),
            replay: Some(dir),
            keep: true,
            quiet: true,
            ..Default::default()
        };
        assert_eq!(run(&cli), 0);
        assert!(out.join("report.txt").is_file());
    }

    #[test]
    fn run_tooltip_match_is_three_because_it_is_advisory() {
        let dir = scratch("run-tooltip");
        write(&dir.join("rust.json"), r#"{"result":{"text":"a"}}"#);
        write(&dir.join("rust.code"), "0");
        write(&dir.join("csharp.json"), r#"{"result":{"text":"a"}}"#);
        write(&dir.join("csharp.code"), "0");

        let out = scratch("run-tooltip-out");
        let cli = Cli {
            op: Some("tooltip".into()),
            normalizer: NormalizerFlag::Builtin,
            out_dir: Some(out.clone()),
            replay: Some(dir),
            quiet: true,
            ..Default::default()
        };
        assert_eq!(run(&cli), 3, "통과로 보고되면 안 된다");
        let report = std::fs::read_to_string(out.join("report.txt")).unwrap();
        assert!(report.contains("ADVISORY MATCH"), "{report}");
    }

    #[test]
    fn run_unknown_op_is_two() {
        let dir = scratch("run-unknown");
        write(&dir.join("rust.json"), "{}");
        write(&dir.join("rust.code"), "0");
        write(&dir.join("csharp.json"), "{}");
        write(&dir.join("csharp.code"), "0");

        let cli = Cli {
            op: Some("bogus".into()),
            normalizer: NormalizerFlag::Builtin,
            out_dir: Some(scratch("run-unknown-out")),
            replay: Some(dir),
            quiet: true,
            ..Default::default()
        };
        assert_eq!(run(&cli), 2);
    }

    #[test]
    fn run_rust_failure_is_four_and_names_the_stderr() {
        let dir = scratch("run-rust-fail");
        write(&dir.join("rust.json"), "");
        write(&dir.join("rust.stderr"), "오류: 파일 접근 실패");
        write(&dir.join("rust.code"), "1");
        write(&dir.join("csharp.json"), "{}");
        write(&dir.join("csharp.code"), "0");

        let out = scratch("run-rust-fail-out");
        let cli = Cli {
            op: Some("tree".into()),
            normalizer: NormalizerFlag::Builtin,
            out_dir: Some(out.clone()),
            replay: Some(dir),
            quiet: true,
            ..Default::default()
        };
        assert_eq!(run(&cli), 4, "비교가 일어나지 않았으므로 불일치가 아니다");
        let report = std::fs::read_to_string(out.join("report.txt")).unwrap();
        assert!(report.contains("RUST FAILURE"), "{report}");
        assert!(report.contains("파일 접근 실패"), "{report}");
    }

    #[test]
    fn missing_csharp_location_names_both_ways_to_supply_it() {
        let hint = missing_csharp_hint();
        assert!(hint.contains("--csharp-repo"), "{hint}");
        assert!(hint.contains("WZ_CSHARP_REPO"), "{hint}");
        assert!(hint.contains(CSHARP_ORACLE_DLL), "{hint}");
    }
}