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

        /// 아바타 합성 배치 및 오프스크린 렌더링 실행 (선택적으로 출력 파일 경로 지정 가능: --render <output.png>)
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        render: Option<PathBuf>,

        /// 오프스크린 렌더링된 결과를 파일로 저장 (예: output.png 또는 output.rgba)
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },

    /// WZ 아카이브 노드 질의 검색 (이름 및 하위 속성 필터)
    Query {
        /// WZ 파일 경로
        wz: PathBuf,

        /// 대상 노드 이름 포함 패턴
        #[arg(long, default_value = "")]
        name: String,

        /// 하위 속성 조건 (예: info/reqLevel>=100, info/islot=Cp)
        #[arg(long = "prop")]
        props: Vec<String>,

        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// `Sound_DX8` 정보 확인/추출/재생
    Sound {
        /// Sound WZ 파일
        wz: PathBuf,

        /// 사운드 경로 (예: Bgm00.img/DragonDream)
        path: String,

        /// 추출 파일 경로 (확장자 생략 시 코덱에 맞춰 붙임)
        #[arg(long)]
        out: Option<PathBuf>,

        /// 추출 후 시스템 플레이어로 재생 (macOS afplay)
        #[arg(long)]
        play: bool,
    },

    /// DB2 스타일 WZ 이미지 데이터 테이블
    Db2 {
        /// WZ 파일
        wz: PathBuf,
        /// 이미지 이름 (예: 01302000.img)
        image: String,
        /// path/value 부분 문자열 필터
        #[arg(long, default_value = "")]
        filter: String,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 200)]
        limit: usize,
    },

    /// DB2 Item 그리드 (id/name/desc/icon)
    Db2Items {
        /// Item WZ 파일
        wz: PathBuf,
        /// 이미지 이름 (예: 0200.img)
        image: String,
        /// String_000.wz 경로
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 1000)]
        limit: usize,
    },

    /// DB2 Map 그리드 (id/street/map/minimap/link)
    Db2Maps {
        /// Map WZ 파일
        wz: PathBuf,
        /// String_000.wz 경로
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 1000)]
        limit: usize,
    },

    /// DB2 Mob 그리드 (id/name/stand/fly/move/hit/die/link)
    Db2Mobs {
        /// Mob WZ 파일
        wz: PathBuf,
        /// String_000.wz 경로
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 1000)]
        limit: usize,
    },

    /// DB2 Skill 그리드 (book/skill/icon/common/h)
    Db2Skills {
        /// Skill WZ 파일
        wz: PathBuf,
        /// String_000.wz 경로
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 2000)]
        limit: usize,
    },

    /// DB2 NPC 그리드 (id/name/stand/default/link)
    Db2Npcs {
        /// Npc WZ 파일
        wz: PathBuf,
        /// String_000.wz 경로
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 2000)]
        limit: usize,
    },

    /// DB2 Reactor 그리드 (id/icon/info/link)
    Db2Reactors {
        /// Reactor WZ 파일
        wz: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 2000)]
        limit: usize,
    },

    /// DB2 Music 그리드 (Bgm/PL_* Sound 경로)
    Db2Music {
        /// Sound WZ 파일
        wz: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 5000)]
        limit: usize,
    },

    /// DB2 DamageSkin 그리드
    Db2DamageSkins {
        /// Item/Consume WZ 파일
        wz: PathBuf,
        /// String_000.wz 경로
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 2000)]
        limit: usize,
    },

    /// DB2 Morph 그리드
    Db2Morphs {
        /// Item/Consume WZ 파일
        wz: PathBuf,
        /// Morph WZ 파일
        #[arg(long)]
        morph: PathBuf,
        /// String_000.wz 경로
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 2000)]
        limit: usize,
    },

    /// DB2 TamingMob 그리드
    Db2TamingMobs {
        /// Character/TamingMob WZ 파일
        wz: PathBuf,
        /// String_000.wz 경로
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 2000)]
        limit: usize,
    },

    /// DB2 Character 장비 그리드
    Db2Characters {
        /// Character/<Dir> WZ 파일
        wz: PathBuf,
        /// String_000.wz 경로
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 10000)]
        limit: usize,
    },

    /// Skill 설명 요약(`SummaryParser.GetSkillSummary`) 출력
    SkillSummary {
        /// String_000.wz 경로
        string: PathBuf,
        /// 최대 행
        #[arg(long, default_value_t = 1000)]
        limit: usize,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// ItemTooltip 이름/설명/속성 태그 출력
    ItemTooltip {
        /// Item WZ 파일
        wz: PathBuf,
        /// 이미지 이름 (예: 0200.img)
        image: String,
        /// 아이템 ID (예: 02000012)
        id: String,
        /// String_000.wz 경로
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// 장비 info 스탯 문자열 (`ItemStringHelper.GetGearPropString`)
    GearProps {
        /// Character WZ 파일
        wz: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// ItemTooltip 추가 텍스트 (레시피 유효일/사용횟수)
    ItemExtras {
        /// Data 루트
        data_root: PathBuf,
        /// Item 하위 디렉터리
        dir: String,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// ItemTooltip 캐시/기간/펫 제한 태그
    ItemCashLines {
        /// Data 루트
        data_root: PathBuf,
        /// Item 하위 디렉터리
        dir: String,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// ItemTooltip desc_leftalign/pointCost/cantAccountSharable
    ItemTextExtras {
        /// Data 루트
        data_root: PathBuf,
        /// Item 하위 디렉터리
        dir: String,
        /// String_000.wz
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// ItemTooltip 성향/tradeAvailable 라인
    ItemIncline {
        /// Data 루트
        data_root: PathBuf,
        /// Item 하위 디렉터리
        dir: String,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// PluginBase 어셈블리 메타데이터/엔트리 타입
    PluginMetadata {
        /// 어셈블리 DLL 경로
        dll: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// PluginBase 플러그인 파일 탐색 (WzComparerR2.*.dll)
    PluginFiles {
        /// Plugin 디렉터리
        dir: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// Quest 툴팁 모델 (이름/설명/수요/보상)
    QuestTooltips {
        /// Quest/QuestData WZ 파일
        wz: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 20000)]
        limit: usize,
    },

    /// Map 툴팁 텍스트 (마을/맵 이름, 설명, BGM, 몹/NPC)
    MapTooltips {
        /// Map WZ 파일
        wz: PathBuf,
        /// String_000.wz
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// OpenAPI 아바타 비트팩 해제
    AvatarUnpack {
        /// 코드 버전 (26~44, 없는 버전은 원본처럼 보정)
        version: i32,
        /// 팩 바이트 hex
        pack: String,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// Network 루프백 핸드셰이크/로그인 데모
    NetLoopback,

    /// Network 하트비트/재접속 루프백 데모
    NetReconnect,

    /// DamageSkin 모델 (툴팁/아이템 연결 필드)
    DamageSkinModel {
        /// Etc_000.wz (DamageSkin.img 포함)
        wz: PathBuf,
        /// 이미지 이름
        #[arg(long, default_value = "DamageSkin.img")]
        image: String,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// NPC/Reactor 툴팁 텍스트
    CharaTooltips {
        /// Npc WZ 파일
        npc: PathBuf,
        /// Reactor WZ 파일
        reactor: PathBuf,
        /// String_000.wz
        #[arg(long)]
        string: PathBuf,
        /// Etc_000.wz (NpcLocation.img 포함)
        #[arg(long)]
        etc: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// PluginManager.FindWz 경로 조회 (type=path 등록 후 경로)
    FindWz {
        /// `Mob=Data/Mob/...wz` 형식. 여러 번 지정 가능.
        #[arg(long = "map", value_name = "TYPE=PATH")]
        maps: Vec<String>,
        /// 조회할 전체 경로 (예: `Mob/8645262.img/stand/0`)
        path: String,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// WorldArchive/몬스터북 툴팁 텍스트
    WorldArchive {
        /// String_000.wz 경로
        string: PathBuf,
        /// Etc_002.wz (worldArchive.img 포함) 경로
        etc: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// 업적 데이터/툴팁 텍스트
    Achievements {
        /// Etc/Achievement/AchievementData WZ 파일
        wz: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 5000)]
        limit: usize,
    },

    /// 데이터 부재 항목 전수 스캔 (Skill common/level, WorldMap Fog, Familiar, BIN)
    ParityScan {
        /// Data 루트
        data_root: PathBuf,
    },

    /// DB2 Item 탭 그리드 (디렉터리 전체)
    Db2ItemTab {
        /// Data 루트
        data_root: PathBuf,
        /// Item 하위 디렉터리 (Cash/Consume/Etc/Install/Pet/Special)
        dir: String,
        /// String_000.wz 경로
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
        /// 최대 행
        #[arg(long, default_value_t = 20000)]
        limit: usize,
    },

    /// DB2 Item 탭 전체의 ItemTooltip 속성
    Db2ItemTabTooltips {
        /// Data 루트
        data_root: PathBuf,
        /// Item 하위 디렉터리 (Cash/Consume/Etc/Install/Pet/Special)
        dir: String,
        /// String_000.wz 경로
        #[arg(long)]
        string: PathBuf,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// DB2 아이콘/이미지 뷰어 (Canvas RGBA SHA-256)
    Db2Icons {
        /// WZ 파일
        wz: PathBuf,
        /// 이미지 이름
        image: String,
        /// path 부분 문자열 필터
        #[arg(long, default_value = "")]
        filter: String,
        /// JSON 출력
        #[arg(long)]
        json: bool,
    },

    /// MapleStory `.patch` 레코드 확인/적용
    Patch {
        /// .patch 파일 경로
        patch: PathBuf,

        /// 적용할 원본 클라이언트 디렉터리
        #[arg(long)]
        old: Option<PathBuf>,

        /// 패치 결과를 쓸 디렉터리
        #[arg(long)]
        out: Option<PathBuf>,

        /// 적용하지 않고 레코드만 요약
        #[arg(long)]
        info: bool,
    },

    /// C# Oracle 호환 검증용 표준 JSON 출력 하네스
    Oracle {
        #[command(subcommand)]
        action: OracleAction,
    },
}

#[derive(Subcommand, Debug)]
enum OracleAction {
    /// 노드 트리 표준 JSON 덤프
    Tree {
        /// WZ 파일 경로
        #[arg(long)]
        wz: PathBuf,

        /// 노드 이름 정규식 필터
        #[arg(long)]
        filter: Option<String>,
    },

    /// 두 WZ 파일 간의 차이점 표준 JSON 덤프
    Diff {
        /// 기준 WZ 파일 경로
        #[arg(long)]
        base: PathBuf,

        /// 변경된 WZ 파일 경로
        #[arg(long)]
        r#mod: PathBuf,
    },

    /// 이미지 내부 Canvas 픽셀 해시(SHA-256) 표준 JSON 덤프
    Image {
        /// WZ 파일 경로
        #[arg(long)]
        wz: PathBuf,

        /// 이미지/캔버스 노드 경로 (예: Character/00002000.img 또는 00002000.img/0)
        #[arg(long)]
        path: String,

        /// 출력 포맷 (sha256 등)
        #[arg(long, default_value = "sha256")]
        format: String,
    },

    /// 아이템/장비/스킬 툴팁 문자열(StringLinker) 표준 JSON 덤프
    Tooltip {
        /// String.wz 파일 경로
        #[arg(long)]
        wz: PathBuf,

        /// Item.wz 파일 경로 (Special/0910.img 등)
        #[arg(long)]
        item_wz: Option<PathBuf>,

        /// Etc.wz 파일 경로 (SetItemInfo.img / worldArchive.img)
        #[arg(long)]
        etc_wz: Option<PathBuf>,

        /// 조회 도메인 (item/eqp/skill/map/mob/npc). 생략하면 편의 순서.
        #[arg(long)]
        domain: Option<String>,

        /// 아이템/장비/스킬 ID (예: 2000000)
        #[arg(long)]
        item: i32,
    },

    /// 장비(Gear) 스탯 프로퍼티 표준 JSON 덤프
    Stats {
        /// WZ 파일 경로
        #[arg(long)]
        wz: PathBuf,

        /// 장비 ID (예: 01000000)
        #[arg(long)]
        gear: String,

        /// 특정 스탯 프로퍼티 키 (예: incSTR, reqLevel)
        #[arg(long)]
        prop: Option<String>,
    },

    /// Calculator 수식 평가 표준 JSON 덤프
    Expr {
        /// 수식 문자열 (예: "u(x/2)+10")
        #[arg(long)]
        expr: String,

        /// 인자 목록 (쉼표 구분 f64: x,y,z,w)
        #[arg(long, default_value = "")]
        args: String,
    },

    /// 패치 파일(.patch) 헤더 및 체크섬 검증 표준 JSON 덤프
    Patch {
        /// 패치 파일 경로
        #[arg(long)]
        patch: PathBuf,
    },

    /// 애니메이션 프레임 병합(정규화) 및 알파 보간 표준 JSON 덤프 (#27)
    Timeline {
        /// WZ 파일 경로
        #[arg(long)]
        wz: PathBuf,

        /// 이미지 이름 (생략하면 첫 이미지 엔트리)
        #[arg(long)]
        path: Option<String>,

        /// img 내부 액션 노드 (예: walk1)
        #[arg(long)]
        node: String,

        /// 실캔버스 해석용 _Canvas 그룹 WZ (outlink 복호화)
        #[arg(long)]
        canvas_wz: Option<PathBuf>,

        /// 알파 보간 샘플 간격(ms). 0 이면 생략
        #[arg(long, default_value_t = 0)]
        interpol_ms: u32,
    },

    /// 아바타 프레임 합성 및 RGBA8 SHA-256 표준 JSON 덤프 (#26)
    Avatar {
        /// 본체(액션) WZ 파일 경로
        #[arg(long)]
        body_wz: PathBuf,

        /// 본체 이미지 이름 (생략하면 첫 이미지 엔트리, 예: 00002000.img)
        #[arg(long)]
        body: Option<String>,

        /// 실캔버스(_outlink) 해석용 _Canvas 그룹 WZ
        #[arg(long)]
        canvas_wz: PathBuf,

        /// zmap.img 를 가진 WZ 경로
        #[arg(long)]
        zmap_wz: PathBuf,

        /// zmap 이미지 이름 (생략하면 zmap.img)
        #[arg(long)]
        zmap: Option<String>,

        /// 액션 노드 이름 (예: walk1)
        #[arg(long)]
        action: String,

        /// 프레임 인덱스
        #[arg(long, default_value_t = 0)]
        frame: u32,

        /// 진행 플랫폼 보간 지점 (ms, 0..=delay). 생략하면 프레임 시작 a0
        #[arg(long, default_value_t = 0)]
        elapsed: u32,
    },

    /// 맵 씬 물리 구조 및 발판/포탈/라이프 표준 JSON 덤프
    MapScene {
        /// WZ 파일 경로
        #[arg(long)]
        wz: PathBuf,

        /// 맵 ID 또는 이미지 경로 (예: 100000000 또는 000010000.img)
        #[arg(long)]
        map: String,
    },
}

mod commands;

use commands::*;

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
        Command::Avatar {
            demo,
            render,
            output,
        } => cmd_avatar(demo, render, output),
        Command::Query {
            wz,
            name,
            props,
            json,
        } => cmd_query(&wz, &name, &props, json),
        Command::Sound {
            wz,
            path,
            out,
            play,
        } => cmd_sound(&wz, &path, out.as_deref(), play),
        Command::Db2 {
            wz,
            image,
            filter,
            json,
            limit,
        } => cmd_db2(&wz, &image, &filter, json, limit),
        Command::Db2Items {
            wz,
            image,
            string,
            json,
            limit,
        } => cmd_db2_items(&wz, &image, &string, json, limit),
        Command::Db2Maps {
            wz,
            string,
            json,
            limit,
        } => cmd_db2_maps(&wz, &string, json, limit),
        Command::Db2Mobs {
            wz,
            string,
            json,
            limit,
        } => cmd_db2_mobs(&wz, &string, json, limit),
        Command::Db2Skills {
            wz,
            string,
            json,
            limit,
        } => cmd_db2_skills(&wz, &string, json, limit),
        Command::Db2Npcs {
            wz,
            string,
            json,
            limit,
        } => cmd_db2_npcs(&wz, &string, json, limit),
        Command::Db2Reactors { wz, json, limit } => cmd_db2_reactors(&wz, json, limit),
        Command::Db2Music { wz, json, limit } => cmd_db2_music(&wz, json, limit),
        Command::Db2DamageSkins {
            wz,
            string,
            json,
            limit,
        } => cmd_db2_damage_skins(&wz, &string, json, limit),
        Command::Db2Morphs {
            wz,
            morph,
            string,
            json,
            limit,
        } => cmd_db2_morphs(&wz, &morph, &string, json, limit),
        Command::Db2TamingMobs {
            wz,
            string,
            json,
            limit,
        } => cmd_db2_taming_mobs(&wz, &string, json, limit),
        Command::Db2Characters {
            wz,
            string,
            json,
            limit,
        } => cmd_db2_characters(&wz, &string, json, limit),
        Command::SkillSummary {
            string,
            limit,
            json,
        } => cmd_skill_summary(&string, limit, json),
        Command::ItemTooltip {
            wz,
            image,
            id,
            string,
            json,
        } => cmd_item_tooltip(&wz, &image, &id, &string, json),
        Command::GearProps { wz, json } => commands::cmd_gear_props(&wz, json),
        Command::Achievements { wz, json, limit } => commands::cmd_achievements(&wz, json, limit),
        Command::ItemExtras {
            data_root,
            dir,
            json,
        } => commands::cmd_item_extras(&data_root, &dir, json),
        Command::ItemCashLines {
            data_root,
            dir,
            json,
        } => commands::cmd_item_cash_lines(&data_root, &dir, json),
        Command::ItemTextExtras {
            data_root,
            dir,
            string,
            json,
        } => commands::cmd_item_text_extras(&data_root, &dir, &string, json),
        Command::ItemIncline {
            data_root,
            dir,
            json,
        } => commands::cmd_item_incline(&data_root, &dir, json),
        Command::PluginMetadata { dll, json } => commands::cmd_plugin_metadata(&dll, json),
        Command::PluginFiles { dir, json } => commands::cmd_plugin_files(&dir, json),
        Command::QuestTooltips { wz, json, limit } => {
            commands::cmd_quest_tooltips(&wz, json, limit)
        }
        Command::MapTooltips { wz, string, json } => commands::cmd_map_tooltips(&wz, &string, json),
        Command::AvatarUnpack {
            version,
            pack,
            json,
        } => commands::cmd_avatar_unpack(version, &pack, json),
        Command::NetLoopback => commands::cmd_net_loopback(),
        Command::NetReconnect => commands::cmd_net_reconnect(),
        Command::DamageSkinModel { wz, image, json } => {
            commands::cmd_damage_skin_model(&wz, &image, json)
        }
        Command::CharaTooltips {
            npc,
            reactor,
            string,
            etc,
            json,
        } => commands::cmd_chara_tooltips(&npc, &reactor, &string, &etc, json),
        Command::FindWz { maps, path, json } => commands::cmd_find_wz(&maps, &path, json),
        Command::WorldArchive { string, etc, json } => {
            commands::cmd_world_archive(&string, &etc, json)
        }
        Command::ParityScan { data_root } => commands::parity::cmd_parity_scan(&data_root),
        Command::Db2ItemTab {
            data_root,
            dir,
            string,
            json,
            limit,
        } => cmd_db2_item_tab(&data_root, &dir, &string, json, limit),
        Command::Db2ItemTabTooltips {
            data_root,
            dir,
            string,
            json,
        } => commands::cmd_db2_item_tab_tooltips(&data_root, &dir, &string, json),
        Command::Db2Icons {
            wz,
            image,
            filter,
            json,
        } => cmd_db2_icons(&wz, &image, &filter, json),
        Command::Patch {
            patch,
            old,
            out,
            info,
        } => cmd_patch(&patch, old.as_deref(), out.as_deref(), info),
        Command::Oracle { action } => cmd_oracle(action),
    };

    if let Err(e) = result {
        eprintln!("오류: {e}");
        std::process::exit(1);
    }
}
