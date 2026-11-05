//! WZ → Avatar 로더.
//!
//! `Character/_Canvas/_Canvas_000.wz` 에서 첫 번째 이미지의 첫 프레임을 파싱해
//! 모든 파트 (body, head, hair, arm, ...) 를 로드한다.
//!
//! WZ 포맷은 origin/anchor/z/map 프로퍼티를 저장하지 않으므로,
//! 앵커/프리즘/zmap 정밀도는 C# CharaSim 참조 구현이 필요하다.
//! 현재 로더는 PoC 수준: 이미지 캔버스를 로드해 SkinLayer (width/height) +
//! Bone + ZMap + AvatarScene 을 조립하고 렌더한다.
//!
//! z-tag 는 파트 이름에서 추론한다 (body→"body", head→"head", hair→"hair",
//! arm/lHand/rHand→"arm"). zmap 순서는 MapleStory 표준 레이어 순서를 사용한다.

use std::sync::Arc;

use wz_core::image::Node as ImageNode;
use wz_core::WzArchive;
use wz_image::decode_png_ref_to_rgba;

use crate::{AvatarScene, Bone, SkinLayer, ZMap};

use crate::zmap::{part_z_tag, STANDARD_ZMAP};

/// MapleStory 표준 z-map 순서 (Base\zmap.img 에서 로드).
/// 낮은 인덱스가 먼저 그려진다 (painter's algorithm).
/// 로드된 아바타 파트.
#[derive(Debug, Default, Clone)]
pub struct AvatarPart {
    pub slot: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// 아바타 기준 좌표의 최종 왼쪽 위 위치 (`FrameLayer::pos`).
    pub position: crate::Point,
    /// 원본 캔버스 origin (변환/디버그용).
    pub origin: crate::Point,
    /// 최종 z 인덱스 (GUI/씬 정렬용).
    pub z_index: i32,
    /// `info/customOrigin` — WZ child name → custom origin ID.
    pub custom_origin_map: std::collections::BTreeMap<String, String>,
}

impl AvatarPart {
    /// 호출자가 제공한 프리즘을 이 파트의 RGBA 에 적용한다.
    ///
    /// C# 에서도 `PrismData` 는 WZ 가 아니라 UI/호출자가 들고 있다
    /// (`AvatarPart.PrismData` 는 외부에서 설정된다). 로더는 픽셀만 만들고,
    /// 색 변환은 이 메서드로 분리한다.
    pub fn apply_prism(&mut self, data: &crate::PrismData) {
        if data.is_valid() {
            crate::apply_prism_rgba8(&mut self.rgba, data, false);
        }
    }
}

/// 로드 결과.
pub struct LoadedAvatar {
    pub parts: Vec<AvatarPart>,
    pub root_bone: Bone,
    pub zmap: Arc<ZMap>,
    pub scene: AvatarScene,
    /// Effect 슬롯의 전체 프레임 (자동 재생용). `parts` 에는 선택 프레임만 들어간다.
    pub effect_frames: Vec<AvatarPart>,
    pub effect_delays: Vec<u32>,
    /// 프레임별 origin/z (scene 렌더러용).
    pub effect_origins: Vec<crate::Point>,
    pub effect_z_indices: Vec<i32>,
    /// 체어 메타데이터 (`info/scale`, group taming, bodyRelMove).
    pub chair_info: Option<ChairInfo>,
    /// WZ 데이터 기반 체어 변환 (플립/회전/오프셋).
    pub transform: AvatarTransform,
    /// 체어 애니메이션 프레임 (rollingHouseChair 등). 비어 있으면 정적.
    pub chair_frames: Vec<Vec<AvatarPart>>,
    pub chair_delays: Vec<u32>,
}

/// C# `CustomChairType` — 현재 C# 원본은 `ScaleAvatarChair` 만 구현한다.
///
/// 나머지 WZ `type` 문자열(`flipXYChair`, `dummyAvatarChair`, ...)은 C# 에서
/// `Unsupported` 로 떨어지고, 동작은 `info/sitAction`, `removeBody`, `scale`,
/// `tamingMob` 같은 공통 필드로만 결정된다. 여기서는 원문을 잃지 않도록
/// `Other` 로 보존한다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CustomChairType {
    #[default]
    Unsupported,
    ScaleAvatarChair,
    Other(String),
}

impl CustomChairType {
    pub fn parse(text: &str) -> Self {
        if text.eq_ignore_ascii_case("ScaleAvatarChair") {
            Self::ScaleAvatarChair
        } else if text.is_empty() {
            Self::Unsupported
        } else {
            Self::Other(text.to_string())
        }
    }
}

/// `flipXYChairInfo` — WZ 데이터에는 있지만 C# 아바타 렌더러는 사용하지 않는다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlipXYChairInfo {
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    pub avatar_pos: crate::Point,
    pub avatar_z: i32,
    pub invisible_weapon: bool,
}

/// `rotatedSleepingBagChairInfo` — WZ 데이터에는 있지만 C# 은 사용하지 않는다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RotatedChairInfo {
    pub avatar_pos: crate::Point,
    pub rotate: i32,
    pub hide_weapon: bool,
    pub hide_nick: bool,
}

/// `dummyAvatarChair` 의 더미 아바타 한 명.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DummyAvatarEntry {
    pub taming_mob: i32,
    pub position: crate::Point,
    pub z: i32,
    pub left: bool,
}

/// `dummyAvatarChair` 의 더미 아바타 목록.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DummyChairInfo {
    pub self_entry: Option<DummyAvatarEntry>,
    pub avatars: Vec<DummyAvatarEntry>,
    pub avatar_count: usize,
    pub chat_balloon_move: crate::Point,
}

/// `rollingHouseChairInfo` — 20프레임 다층 회전 하우스.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RollingHouseInfo {
    pub parts_count: usize,
    pub part_z: Vec<i32>,
    pub pos_lv: Vec<crate::Point>,
    pub frame_count: usize,
    pub delays: Vec<u32>,
    pub min_level: i32,
    pub max_level: i32,
    pub quest_id: i32,
}

/// `part/N/part0..` 구조를 가진 체어 애니메이션 (2019rollingHouse, yeti 등).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartChairAnimationInfo {
    pub info_name: String,
    pub parts_count: usize,
    pub frame_count: usize,
    pub delays: Vec<u32>,
    pub part_z: Vec<i32>,
    pub pos_lv: Vec<crate::Point>,
}

/// `popChairInfo` / `popButtonChairInfo` — pop 수치 텍스트.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PopChairInfo {
    pub text_pos: crate::Point,
    pub text_z: i32,
    pub min: Option<i64>,
    pub max: Option<i64>,
}

/// `timeChairInfo` — 시간 텍스트.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimeChairInfo {
    pub text_pos: crate::Point,
    pub text_z: i32,
    pub max: Option<i64>,
}

/// `identityChairInfo` — 레벨/이름/직업/성별 레이어.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentityChairInfo {
    pub num_pos: crate::Point,
    pub name_pos: crate::Point,
    pub job_pos: crate::Point,
    pub gender_pos: crate::Point,
    pub layer_pos: crate::Point,
    pub layer_size: crate::Point,
    pub digits: usize,
}

/// `scoreChairInfo/scoreEffect/N` 범위.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScoreChairGroup {
    pub min: i32,
    pub max: Option<i32>,
}

/// `scoreChairInfo` — 점수 구간별 이펙트.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScoreChairInfo {
    pub groups: Vec<ScoreChairGroup>,
}

/// 호출자가 공급하는 체어 표시 값 (C# `PrismData` 처럼 WZ 밖의 값).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChairUserData {
    pub level: Option<i32>,
    pub name: Option<String>,
    pub job: Option<String>,
    pub gender: Option<String>,
    pub score: Option<i32>,
    pub pop: Option<i64>,
    pub time_minutes: Option<i64>,
}

/// `hashTagChairInfo/action/N` — 동작별 강제 포즈.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HashTagAction {
    pub action: String,
    pub frame: Option<usize>,
    pub pos: crate::Point,
}

/// `hashTagChairInfo`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HashTagChairInfo {
    pub actions: Vec<HashTagAction>,
}

/// `mirrorChairInfo`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MirrorChairInfo {
    pub avatar_pos_y: i32,
    pub invisible_weapon: bool,
}

/// `eventPointChair` — 이벤트 포인트 체어의 게임플레이 메타데이터.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventPointChairInfo {
    pub index: i32,
    pub point: i32,
    pub term: i32,
}

/// `androidChairInfo` — 안드로이드/홀로그램 효과.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AndroidChairInfo {
    pub effect_frames: usize,
    pub z: i32,
    pub forced_action: Option<String>,
    pub hide_body: bool,
    pub pos: crate::Point,
    pub reverse_flip: bool,
}

/// `petChairInfo` 의 펫 슬롯 하나.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PetChairSlot {
    pub pos: crate::Point,
    pub reverse_flip: bool,
    pub wonder_grade: i32,
    pub t_gap_min: i32,
    pub t_gap_max: i32,
}

/// `petChairInfo` — 펫 3마리 슬롯.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PetChairInfo {
    pub slots: Vec<PetChairSlot>,
}

/// `customChair` 의 알려진 필드 + 미래/미구현 필드 보존.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomChairInfo {
    pub chair_type: CustomChairType,
    pub scale_avatar: Option<i32>,
    pub self_taming_mob: Option<i32>,
    pub flip: Option<FlipXYChairInfo>,
    pub rotate: Option<RotatedChairInfo>,
    pub dummy: Option<DummyChairInfo>,
    pub rolling_house: Option<RollingHouseInfo>,
    pub android: Option<AndroidChairInfo>,
    pub pet: Option<PetChairInfo>,
    pub event_point: Option<EventPointChairInfo>,
    pub hash_tag: Option<HashTagChairInfo>,
    pub mirror: Option<MirrorChairInfo>,
    pub part_animation: Option<PartChairAnimationInfo>,
    pub identity: Option<IdentityChairInfo>,
    pub score: Option<ScoreChairInfo>,
    pub pop: Option<PopChairInfo>,
    pub time: Option<TimeChairInfo>,
    /// 중첩 Property 를 `a/b/c` 경로의 문자열로 평탄화해 보존한다.
    pub extra: std::collections::BTreeMap<String, String>,
}

fn image_scalar_string(value: &wz_core::image::ImageValue) -> Option<String> {
    use wz_core::image::ImageValue;
    match value {
        ImageValue::Int(v) => Some(v.to_string()),
        ImageValue::Short(v) => Some(v.to_string()),
        ImageValue::Str(v) => Some(v.to_string()),
        ImageValue::Vector(v) => Some(format!("{},{}", v.x, v.y)),
        ImageValue::Bool(v) => Some(v.to_string()),
        _ => None,
    }
}

fn flatten_extra(
    node: &ImageNode,
    prefix: &str,
    out: &mut std::collections::BTreeMap<String, String>,
) {
    for child in node.object_children() {
        let name = child.name.to_string();
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if let Some(value) = child.value.as_ref().and_then(image_scalar_string) {
            out.insert(path.clone(), value);
        }
        if !child.object_children().is_empty() {
            flatten_extra(child, &path, out);
        }
    }
}

fn parse_dummy_entry(node: &ImageNode) -> Option<DummyAvatarEntry> {
    let taming_mob = node_int(node, "tamingMob")?;
    Some(DummyAvatarEntry {
        taming_mob,
        position: node_vec(node, "pos").unwrap_or(crate::Point::ZERO),
        z: node_int(node, "z").unwrap_or(0),
        left: node_int(node, "left").unwrap_or(0) != 0,
    })
}

fn parse_custom_chair(node: &ImageNode) -> CustomChairInfo {
    let chair_type = node_str(node, "type")
        .map(|t| CustomChairType::parse(&t))
        .unwrap_or_default();
    let scale_avatar = node_int(node, "scaleAvatar/scale");
    let self_taming_mob = node_int(node, "self/tamingMob").filter(|v| *v != 0);

    let flip = child_node(node, "flipXYChairInfo").map(|n| FlipXYChairInfo {
        flip_horizontal: node_int(n, "flipHorizontal").unwrap_or(0) != 0,
        flip_vertical: node_int(n, "flipVertical").unwrap_or(0) != 0,
        avatar_pos: node_vec(n, "flipCharacterPos").unwrap_or(crate::Point::ZERO),
        avatar_z: node_int(n, "flipCharacterZ").unwrap_or(0),
        invisible_weapon: node_int(n, "avatarInvisibleWeapon").unwrap_or(0) != 0,
    });

    let rotate = child_node(node, "rotatedSleepingBagChairInfo").map(|n| RotatedChairInfo {
        avatar_pos: node_vec(n, "avatarPos").unwrap_or(crate::Point::ZERO),
        rotate: node_int(n, "avatarRotate").unwrap_or(0),
        // WZ 키의 오타(`avartarInvisibleWeapon`)를 그대로 따른다.
        hide_weapon: node_int(n, "avartarInvisibleWeapon")
            .or_else(|| node_int(n, "avatarInvisibleWeapon"))
            .unwrap_or(0)
            != 0,
        hide_nick: node_int(n, "forcedHideNick").unwrap_or(0) != 0,
    });

    let dummy = child_node(node, "avatar").map(|_| {
        let avatar_count = node_int(node, "avatarCount").unwrap_or(0).max(0) as usize;
        let mut avatars = Vec::new();
        let mut i = 0;
        while let Some(entry) = find_path_node(node, &format!("avatar/{i}")) {
            if let Some(parsed) = parse_dummy_entry(entry) {
                avatars.push(parsed);
            }
            i += 1;
        }
        DummyChairInfo {
            self_entry: child_node(node, "self").and_then(parse_dummy_entry),
            avatars,
            avatar_count,
            chat_balloon_move: node_vec(node, "chatBalloonMove").unwrap_or(crate::Point::ZERO),
        }
    });

    let part_animation = node.object_children().iter().find_map(|child| {
        let info_name = child.name.to_string();
        if info_name == "rollingHouseChairInfo" || !info_name.ends_with("Info") {
            return None;
        }
        let part_root = child_node(child, "part")?;
        let first_frame = part_root.object_children().first()?;
        child_node(first_frame, "part0")?;
        let parts_count = first_frame
            .object_children()
            .iter()
            .filter(|c| c.name.starts_with("part"))
            .count();
        if parts_count == 0 {
            return None;
        }
        let frame_count = part_root.object_children().len();
        let delays = (0..frame_count)
            .filter_map(|i| node_int(child, &format!("part/{i}/delay")))
            .map(|v| v.max(0) as u32)
            .collect();
        let part_z = (0..parts_count)
            .filter_map(|i| node_int(child, &format!("partZ/{i}")))
            .collect();
        let pos_lv = (1..=20)
            .filter_map(|i| node_vec(child, &format!("posLv/lv{i}")))
            .collect();
        Some(PartChairAnimationInfo {
            info_name,
            parts_count,
            frame_count,
            delays,
            part_z,
            pos_lv,
        })
    });

    let pop = child_node(node, "popChairInfo")
        .map(|n| PopChairInfo {
            text_pos: node_vec(n, "textPos").unwrap_or(crate::Point::ZERO),
            text_z: node_int(n, "textZ").unwrap_or(0),
            min: node_int(n, "popMin").map(i64::from),
            max: node_int(n, "popMax").map(i64::from),
        })
        .or_else(|| {
            child_node(node, "popButtonChairInfo").map(|n| PopChairInfo {
                text_pos: node_vec(n, "popPos").unwrap_or(crate::Point::ZERO),
                text_z: node_int(n, "z").unwrap_or(0),
                min: node_int(n, "popMin").map(i64::from),
                max: node_int(n, "popMax").map(i64::from),
            })
        });

    let time = child_node(node, "timeChairInfo").map(|n| TimeChairInfo {
        text_pos: node_vec(n, "textPos").unwrap_or(crate::Point::ZERO),
        text_z: 0,
        max: node_int(n, "timeMax").map(i64::from),
    });

    let identity = child_node(node, "identityChairInfo").map(|n| IdentityChairInfo {
        num_pos: node_vec(n, "numPos").unwrap_or(crate::Point::ZERO),
        name_pos: node_vec(n, "namePos").unwrap_or(crate::Point::ZERO),
        job_pos: node_vec(n, "jobPos").unwrap_or(crate::Point::ZERO),
        gender_pos: node_vec(n, "genderPos").unwrap_or(crate::Point::ZERO),
        layer_pos: node_vec(n, "layerPos").unwrap_or(crate::Point::ZERO),
        layer_size: node_vec(n, "layerSize").unwrap_or(crate::Point::ZERO),
        digits: child_node(n, "number")
            .map(|num| num.object_children().len())
            .unwrap_or(0),
    });

    let score = child_node(node, "scoreChairInfo").map(|n| {
        let mut groups = Vec::new();
        if let Some(effect) = child_node(n, "scoreEffect") {
            for child in effect.object_children() {
                if child.name.parse::<usize>().is_err() {
                    continue;
                }
                groups.push(ScoreChairGroup {
                    min: node_int(child, "min").unwrap_or(0),
                    max: node_int(child, "max"),
                });
            }
        }
        ScoreChairInfo { groups }
    });

    let hash_tag = child_node(node, "hashTagChairInfo").map(|n| {
        let mut actions = Vec::new();
        let Some(action_root) = child_node(n, "action") else {
            return HashTagChairInfo { actions };
        };
        for child in action_root.object_children() {
            if child.name.parse::<usize>().is_err() {
                continue;
            }
            let Some(action) = node_str(child, "forceCharacterAction") else {
                continue;
            };
            actions.push(HashTagAction {
                action,
                frame: node_int(child, "forceCharacterActionFrameIndex")
                    .filter(|v| *v >= 0)
                    .map(|v| v as usize),
                pos: node_vec(child, "pos").unwrap_or(crate::Point::ZERO),
            });
        }
        HashTagChairInfo { actions }
    });

    let mirror = child_node(node, "mirrorChairInfo").map(|n| MirrorChairInfo {
        avatar_pos_y: node_int(n, "avatarPosY").unwrap_or(0),
        invisible_weapon: node_int(n, "avatarInvisibleWeapon").unwrap_or(0) != 0,
    });

    let event_point = child_node(node, "eventPointChair").map(|n| EventPointChairInfo {
        index: node_int(n, "index").unwrap_or(0),
        point: node_int(n, "point").unwrap_or(0),
        term: node_int(n, "term").unwrap_or(0),
    });

    let android = child_node(node, "androidChairInfo").map(|n| {
        let effect_frames = child_node(n, "customEffect")
            .map(|ce| {
                ce.object_children()
                    .iter()
                    .filter(|c| matches!(c.value, Some(wz_core::image::ImageValue::Png(_))))
                    .count()
            })
            .unwrap_or(0);
        AndroidChairInfo {
            effect_frames,
            z: node_int(n, "z").unwrap_or(0),
            forced_action: node_str(n, "forcedAction").filter(|v| !v.is_empty()),
            hide_body: node_int(n, "forcedHideBody").unwrap_or(0) != 0,
            pos: node_vec(n, "pos").unwrap_or(crate::Point::ZERO),
            reverse_flip: node_int(n, "reverseFlip").unwrap_or(0) != 0,
        }
    });

    let pet = child_node(node, "petChairInfo").map(|n| {
        let mut slots = Vec::new();
        for child in n.object_children() {
            if child.name.parse::<usize>().is_err() {
                continue;
            }
            slots.push(PetChairSlot {
                pos: node_vec(child, "pos").unwrap_or(crate::Point::ZERO),
                reverse_flip: node_int(child, "reverseFlip").unwrap_or(0) != 0,
                wonder_grade: node_int(child, "wonderGrade").unwrap_or(0),
                t_gap_min: node_int(child, "randAct/tGapMin").unwrap_or(0),
                t_gap_max: node_int(child, "randAct/tGapMax").unwrap_or(0),
            });
        }
        PetChairInfo { slots }
    });

    let rolling_house = child_node(node, "rollingHouseChairInfo").map(|n| {
        let parts_count = node_int(n, "partsCount").unwrap_or(0).max(0) as usize;
        let part_z = (0..parts_count)
            .filter_map(|i| node_int(n, &format!("partZ/{i}")))
            .collect();
        let pos_lv = (1..=10)
            .filter_map(|i| node_vec(n, &format!("posLv/lv{i}")))
            .collect();
        let frame_count = child_node(n, "part")
            .map(|p| p.object_children().len())
            .unwrap_or(0);
        let delays = (0..frame_count)
            .filter_map(|i| node_int(n, &format!("part/{i}/delay")))
            .map(|v| v.max(0) as u32)
            .collect();
        RollingHouseInfo {
            parts_count,
            part_z,
            pos_lv,
            frame_count,
            delays,
            min_level: node_int(n, "minLevel").unwrap_or(0),
            max_level: node_int(n, "maxLevel").unwrap_or(0),
            quest_id: node_int(n, "partsQuestID").unwrap_or(0),
        }
    });

    let mut extra = std::collections::BTreeMap::new();
    flatten_extra(node, "", &mut extra);

    CustomChairInfo {
        chair_type,
        scale_avatar,
        self_taming_mob,
        flip,
        rotate,
        dummy,
        rolling_house,
        android,
        pet,
        event_point,
        hash_tag,
        mirror,
        part_animation,
        identity,
        score,
        pop,
        time,
        extra,
    }
}

/// C# `ChairPart` 의 메타데이터 중 정적 로더가 해석하는 부분.
#[derive(Debug, Clone, Default)]
pub struct ChairInfo {
    /// `info/scale` — C# `AddTamingPart` 가 아바타 전체 배율로 쓴다.
    pub scale: i32,
    pub custom_chair: Option<CustomChairInfo>,
    pub group_count: usize,
    pub group_taming_ids: Vec<i32>,
    pub group_body_rel_move: Vec<crate::Point>,
    pub body_rel_move: Option<crate::Point>,
    /// C# `SetChairDefault`: `info/sitAction` (예: `hide`).
    pub sit_action: Option<String>,
    /// C# `SetChairDefault`: `info/removeBody` → `hideBody`.
    pub remove_body: bool,
    /// C# `SetChairDefault`: `info/fixFrameIdx` 또는 `info/fixActionFrame`.
    pub fix_frame_idx: Option<i32>,
    /// C# `SetChairDefault`: `info/sitEmotion` (감정 인덱스).
    pub sit_emotion: Option<i32>,
    /// `info/tamingMob` 또는 `info/customChair/self/tamingMob`.
    pub taming_mob: Option<i32>,
    /// 위 `taming_mob` 의 메타데이터.
    pub taming: Option<TamingInfo>,
}

/// TamingMob 메타데이터 (`info/scale`, `removeBody`, `flip`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TamingInfo {
    pub id: i32,
    pub scale: i32,
    pub remove_body: bool,
    pub flip: bool,
}

/// WZ 체어 데이터 기반 변환. C# 아바타 렌더러에는 없는 확장이다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AvatarTransform {
    pub flip_x: bool,
    pub flip_y: bool,
    /// 0/90/180/270 (양수 = 시계 방향).
    pub rotate: i32,
    pub offset: crate::Point,
}

impl AvatarTransform {
    pub fn is_identity(&self) -> bool {
        !self.flip_x && !self.flip_y && self.rotate % 360 == 0 && self.offset == crate::Point::ZERO
    }
}

/// 체어 애니메이션 프레임 묶음.
#[derive(Debug, Clone, Default)]
pub struct ChairFrames {
    pub frames: Vec<Vec<AvatarPart>>,
    pub delays: Vec<u32>,
}

/// 체어가 본체에 강제하는 동작.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChairBodyAction {
    /// `sitAction=hide` / `removeBody` / taming `removeBody`.
    Hide,
    /// 동작 이름 + 고정 프레임.
    Action { name: String, frame: Option<usize> },
}

impl ChairInfo {
    /// C# `SetChairDefault` 순서를 그대로 따른다.
    ///
    /// 1. `sitAction` 이 있으면 그 동작
    /// 2. `removeBody != 0` 이면 `hideBody`
    /// 3. `ScaleAvatarChair` 이면 `stand1`
    /// 4. taming 이 없으면 `sit`
    /// 5. taming 이 있으면 `sit` (C# `SetTamingDefaultBodyAction`)
    pub fn body_action(&self) -> ChairBodyAction {
        let frame = self.fix_frame_idx.filter(|v| *v >= 0).map(|v| v as usize);
        if let Some(action) = &self.sit_action {
            if action.eq_ignore_ascii_case("hide") || action.eq_ignore_ascii_case("hideBody") {
                return ChairBodyAction::Hide;
            }
            return ChairBodyAction::Action {
                name: action.clone(),
                frame,
            };
        }
        if let Some(android) = self.custom_chair.as_ref().and_then(|c| c.android.as_ref()) {
            if android.hide_body {
                return ChairBodyAction::Hide;
            }
            if let Some(action) = &android.forced_action {
                return ChairBodyAction::Action {
                    name: action.clone(),
                    frame,
                };
            }
        }
        if let Some(tag) = self.custom_chair.as_ref().and_then(|c| c.hash_tag.as_ref()) {
            if let Some(action) = tag.actions.first() {
                return ChairBodyAction::Action {
                    name: action.action.clone(),
                    frame: action.frame,
                };
            }
        }
        if self.remove_body {
            return ChairBodyAction::Hide;
        }
        if self.taming.as_ref().is_some_and(|t| t.remove_body) {
            return ChairBodyAction::Hide;
        }
        if self
            .custom_chair
            .as_ref()
            .is_some_and(|c| c.chair_type == CustomChairType::ScaleAvatarChair)
        {
            return ChairBodyAction::Action {
                name: "stand1".into(),
                frame,
            };
        }
        ChairBodyAction::Action {
            name: "sit".into(),
            frame,
        }
    }

    /// WZ `customChair` 의 플립/회전 정보를 변환으로 만든다.
    pub fn avatar_transform(&self) -> AvatarTransform {
        let Some(custom) = &self.custom_chair else {
            return AvatarTransform::default();
        };
        if let Some(flip) = &custom.flip {
            return AvatarTransform {
                flip_x: flip.flip_horizontal,
                flip_y: flip.flip_vertical,
                rotate: 0,
                offset: flip.avatar_pos,
            };
        }
        if let Some(rot) = &custom.rotate {
            return AvatarTransform {
                flip_x: false,
                flip_y: false,
                rotate: rot.rotate,
                offset: rot.avatar_pos,
            };
        }
        AvatarTransform::default()
    }

    /// C# `CustomChairAvatarScale`: `ScaleAvatarChair` 만 `scaleAvatar.scale` 를 쓰고,
    /// 나머지 타입은 100 이다. `info/scale` 은 체어가 아니라 taming 에만 쓰인다.
    pub fn effective_scale(&self) -> i32 {
        if let Some(custom) = &self.custom_chair {
            if custom.chair_type == CustomChairType::ScaleAvatarChair {
                return custom.scale_avatar.unwrap_or(100);
            }
        }
        100
    }

    /// `flipXYChair.avatarInvisibleWeapon` / `rotatedSleepingBagChair.avartarInvisibleWeapon`.
    pub fn hides_weapon(&self) -> bool {
        self.custom_chair.as_ref().is_some_and(|c| {
            c.flip.as_ref().is_some_and(|f| f.invisible_weapon)
                || c.rotate.as_ref().is_some_and(|r| r.hide_weapon)
                || c.mirror.as_ref().is_some_and(|m| m.invisible_weapon)
        })
    }

    /// 아바타 전체에 적용할 배율. taming 이 있으면 taming `info/scale`,
    /// 없으면 체어의 `CustomChairAvatarScale`.
    pub fn avatar_scale(&self) -> i32 {
        self.taming
            .as_ref()
            .map(|t| t.scale)
            .unwrap_or_else(|| self.effective_scale())
    }
}

/// Effect 한 항목의 전체 프레임/딜레이/origin/z.
#[derive(Debug, Clone)]
pub struct EffectFrames {
    pub root_bone: Bone,
    pub parts: Vec<AvatarPart>,
    pub delays: Vec<u32>,
    pub origins: Vec<crate::Point>,
    pub z_indices: Vec<i32>,
}

fn set_named_skin_z(bone: &mut Bone, name: &str, z_index: i32) -> bool {
    if let Some(skin) = bone.skins.iter_mut().find(|s| s.name == name) {
        skin.z_index = z_index;
        return true;
    }
    for child in &mut bone.children {
        if set_named_skin_z(child, name, z_index) {
            return true;
        }
    }
    false
}

fn set_bone_z(bone: &mut Bone, z_index: i32) {
    for skin in &mut bone.skins {
        skin.z_index = z_index;
    }
    for child in &mut bone.children {
        set_bone_z(child, z_index);
    }
}

fn set_effect_skin(bone: &mut Bone, origin: crate::Point, z_index: i32) -> bool {
    if let Some(skin) = bone.skins.iter_mut().find(|s| s.name == "effect") {
        skin.origin = origin;
        skin.z_index = z_index;
        return true;
    }
    for child in &mut bone.children {
        if set_effect_skin(child, origin, z_index) {
            return true;
        }
    }
    false
}

impl LoadedAvatar {
    /// effect 프레임의 origin/z 를 bone 에 반영한다 (scene 렌더러용).
    ///
    /// GUI 파트 렌더러는 `LoadedAvatar` 대신 자체 `effect_origin` 상태로 같은
    /// origin 을 적용하지만, `AvatarScene` 을 쓰는 렌더러는 이 메서드로 맞춘다.
    pub fn apply_effect_frame(&mut self, index: usize) {
        if self.effect_frames.is_empty() {
            return;
        }
        let i = index % self.effect_frames.len();
        let origin = self
            .effect_origins
            .get(i)
            .copied()
            .unwrap_or(crate::Point::ZERO);
        let z_index = self.effect_z_indices.get(i).copied().unwrap_or(0);
        set_effect_skin(&mut self.root_bone, origin, z_index);
    }
}

/// `Base/zmap.img` 에서 z-order 를 읽는다. 없으면 `None` (호출부가 표준 순서로 폴백).
fn load_zmap_order(data_root: Option<&std::path::Path>) -> Option<Vec<String>> {
    use wz_core::image::ImageValue;

    let root = data_root?;
    let base = root.join("Base");
    let candidates = [base.join("Base_000.wz"), base.join("Base.wz")];
    let path = candidates.iter().find(|p| p.exists())?;
    let archive = WzArchive::open(path).ok()?;
    let tree = archive.read_tree().ok()?;
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some("zmap.img"))?;
    let node = archive.parse_image_at(entry.offset?).ok()?;

    let mut order: Vec<String> = node.children.iter().map(|c| c.name.to_string()).collect();
    if let Some(ImageValue::Property(p)) = &node.value {
        order.extend(p.children.iter().map(|c| c.name.to_string()));
    }
    let mut seen = std::collections::HashSet::new();
    order.retain(|name| !name.is_empty() && seen.insert(name.clone()));
    if order.is_empty() {
        None
    } else {
        Some(order)
    }
}

/// 한 이미지의 액션 프레임을 [`crate::collect_frame_assembly`] 로 조립한다.
///
/// 기존 `loader.rs` 가 파트마다 반복하던 `origin`/`map`/bone 구성을
/// `frame_layers::collect_frame_assembly` 한 곳으로 모은다.
fn load_frame_parts(
    archive: &WzArchive,
    image_name: &str,
    action_names: &[&str],
) -> Option<(Bone, Vec<AvatarPart>)> {
    load_frame_parts_at(archive, image_name, action_names, None)
}

/// `load_frame_parts` + 고정 프레임 인덱스 (C# `fixFrameIdx`).
fn load_frame_parts_at(
    archive: &WzArchive,
    image_name: &str,
    action_names: &[&str],
    frame_index: Option<usize>,
) -> Option<(Bone, Vec<AvatarPart>)> {
    use wz_core::image::ImageValue;

    let tree = archive.read_tree().ok()?;
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.node_type == 4 && e.name.as_deref() == Some(image_name))?;
    let node = archive.parse_image_at(entry.offset?).ok()?;
    let action = action_names
        .iter()
        .find_map(|name| node.children.iter().find(|c| c.name.as_ref() == *name))
        .or_else(|| {
            action_names.iter().find_map(|name| {
                let needle = name.to_ascii_lowercase();
                node.children
                    .iter()
                    .find(|c| c.name.to_ascii_lowercase().contains(&needle))
            })
        })
        .or_else(|| node.children.first())?;
    let action_children: &[ImageNode] = match &action.value {
        Some(ImageValue::Property(p)) => &p.children,
        _ => &action.children,
    };
    let frame = if action_children
        .iter()
        .any(|c| matches!(c.value, Some(ImageValue::Png(_))))
    {
        // `front` 처럼 액션 노드 자체가 프레임이고 자식이 Canvas 인 경우.
        action
    } else {
        // `stand1/0` 처럼 자식이 프레임인 경우. `fixFrameIdx` 가 있으면 그 프레임.
        action_children
            .get(frame_index.unwrap_or(0))
            .or_else(|| action_children.first())
            .unwrap_or(action)
    };

    let assembly = crate::collect_frame_assembly(frame, archive, Some(archive)).ok()?;
    let parts = assembly
        .layers
        .iter()
        .map(|layer| AvatarPart {
            slot: layer.name.clone(),
            width: layer.canvas.w,
            height: layer.canvas.h,
            rgba: layer.canvas.rgba.clone(),
            position: layer.pos,
            origin: layer.origin,
            z_index: layer.z_int.unwrap_or(0),
            custom_origin_map: Default::default(),
        })
        .collect();
    Some((assembly.root_bone, parts))
}

/// Hair/Face 아이템의 믹스 컬러 변형을 로드해 `parts` 에 블렌드한다.
///
/// C# `AvatarPart.LoadMixNodes` / `AvatarCanvas.MixBitmaps` 대응.
fn apply_mix_to_item(
    archive: &WzArchive,
    item_id: i32,
    dir: &str,
    parts: &mut [AvatarPart],
    mix_color: i32,
    opacity: i32,
) {
    if parts.is_empty() || opacity <= 0 || !(0..=8).contains(&mix_color) {
        return;
    }
    let (base_color, variant_id) = match dir {
        "Hair" => {
            let base = item_id % 10;
            (base, item_id / 10 * 10 + mix_color)
        }
        "Face" => {
            let base = item_id / 100 % 10;
            (
                base,
                item_id / 1000 * 1000 + item_id % 100 + mix_color * 100,
            )
        }
        _ => return,
    };
    if base_color == mix_color {
        return;
    }
    let image_name = format!("{:08}.img", variant_id);
    let Some((_, mix_parts)) =
        load_item_frame(archive, Some(&image_name), &["default", "stand1"], dir)
    else {
        return;
    };
    for (base, mix) in parts.iter_mut().zip(mix_parts.iter()) {
        crate::mix_rgba8(&mut base.rgba, &mix.rgba, opacity);
    }
}

/// 두 bone 트리를 합친다. 같은 이름의 bone 은 스킨을 합치고, 없으면 자식으로 붙인다.
fn merge_bone_tree(dst: &mut Bone, src: Bone) {
    dst.skins.extend(src.skins);
    for child in src.children {
        if let Some(existing) = dst.find_child_mut(&child.name) {
            merge_bone_tree(existing, child);
        } else {
            dst.children.push(child);
        }
    }
}

/// 이름으로 직접 자식을 찾는다.
fn child_node<'a>(node: &'a ImageNode, name: &str) -> Option<&'a ImageNode> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

/// `/` 로 구분된 경로를 따라간다.
fn find_path_node<'a>(node: &'a ImageNode, path: &str) -> Option<&'a ImageNode> {
    let mut cur = node;
    for seg in path.split('/').filter(|s| !s.is_empty()) {
        cur = child_node(cur, seg)?;
    }
    Some(cur)
}

fn node_int(node: &ImageNode, path: &str) -> Option<i32> {
    match find_path_node(node, path).and_then(|n| n.value.as_ref()) {
        Some(wz_core::image::ImageValue::Int(v)) => Some(*v),
        Some(wz_core::image::ImageValue::Short(v)) => Some(i32::from(*v)),
        Some(wz_core::image::ImageValue::Str(v)) => v.parse::<i32>().ok(),
        Some(wz_core::image::ImageValue::Bool(v)) => Some(i32::from(*v)),
        _ => None,
    }
}

fn node_str(node: &ImageNode, path: &str) -> Option<String> {
    match find_path_node(node, path).and_then(|n| n.value.as_ref()) {
        Some(wz_core::image::ImageValue::Str(v)) => Some(v.to_string()),
        _ => None,
    }
}

fn node_vec(node: &ImageNode, path: &str) -> Option<crate::Point> {
    match find_path_node(node, path).and_then(|n| n.value.as_ref()) {
        Some(wz_core::image::ImageValue::Vector(v)) => Some(crate::Point::new(v.x, v.y)),
        _ => None,
    }
}

/// `Effect/_Canvas` 에서 특정 이미지를 가진 아카이브를 찾는다.
fn find_effect_canvas_archive(data_root: &std::path::Path, image_name: &str) -> Option<WzArchive> {
    let dir = data_root.join("Effect").join("_Canvas");
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        if tree
            .root
            .entries
            .iter()
            .any(|e| e.name.as_deref() == Some(image_name))
        {
            return Some(archive);
        }
    }
    None
}

/// Canvas mini-property 에서 `origin`/`z` 를 읽는다.
fn png_origin_z(png: &wz_core::image::PngRef) -> (crate::Point, i32) {
    use wz_core::image::ImageValue;

    let mut origin = crate::Point::ZERO;
    let mut z_index = 0;
    if let Some(sub) = &png.sub_property {
        for ch in &sub.children {
            match (ch.name.as_ref(), &ch.value) {
                ("origin", Some(ImageValue::Vector(v))) => {
                    origin = crate::Point::new(v.x, v.y);
                }
                ("z", Some(ImageValue::Int(v))) => z_index = *v,
                _ => {}
            }
        }
    }
    (origin, z_index)
}

/// `Item/Install/<dir>` 에서 카테고리 이미지의 아이템 노드를 찾는다.
///
/// `dir` 은 `""`(메타데이터) 또는 `"_Canvas"`(픽셀) 이다.
fn find_install_item(
    data_root: &std::path::Path,
    dir: &str,
    item_id: i32,
) -> Option<(WzArchive, ImageNode)> {
    let base = data_root.join("Item").join("Install").join(dir);
    // 카테고리 이미지 이름은 5자리(`03018.img`) 또는 6자리(`030156.img`)다.
    let categories = [
        format!("{:05}.img", item_id / 1000),
        format!("{:06}.img", item_id / 100),
        format!("{:04}.img", item_id / 10000),
    ];
    let item_name = format!("{:08}", item_id);
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        for category in &categories {
            let Some(cat_entry) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some(category.as_str()))
            else {
                continue;
            };
            let Ok(node) = archive.parse_image_at(cat_entry.offset?) else {
                continue;
            };
            let Some(item) = child_node(&node, &item_name) else {
                continue;
            };
            return Some((archive, item.clone()));
        }
    }
    None
}

/// `Character/TamingMob` 메타데이터에서 taming mob 의 `info` 를 찾는다.
fn find_taming_meta(data_root: &std::path::Path, item_id: i32) -> Option<ImageNode> {
    let base = data_root.join("Character").join("TamingMob");
    let image = format!("{:08}.img", item_id);
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(item_entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some(image.as_str()))
        else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(item_entry.offset?) else {
            continue;
        };
        return Some(node);
    }
    None
}

/// C# `AddTamingPart`: `info/scale`, `info/removeBody`, `info/flip`.
pub fn load_taming_info(data_root: &std::path::Path, item_id: i32) -> Option<TamingInfo> {
    let node = find_taming_meta(data_root, item_id)?;
    let info = child_node(&node, "info")?;
    Some(TamingInfo {
        id: item_id,
        scale: node_int(info, "scale").unwrap_or(100),
        remove_body: node_int(info, "removeBody").unwrap_or(0) != 0,
        flip: node_int(info, "flip").unwrap_or(0) != 0,
    })
}

fn parse_chair_info(item: &ImageNode, info: &ImageNode) -> ChairInfo {
    let custom_chair = child_node(info, "customChair").map(parse_custom_chair);
    let taming_mob = node_int(info, "tamingMob")
        .filter(|v| *v != 0)
        .or_else(|| custom_chair.as_ref().and_then(|c| c.self_taming_mob));

    let mut group_count = 0;
    let mut group_taming_ids = Vec::new();
    let mut group_body_rel_move = Vec::new();
    for group in info.object_children() {
        if !group.name.to_ascii_lowercase().contains("group") {
            continue;
        }
        for entry in group.object_children() {
            let lower = entry.name.to_ascii_lowercase();
            if !(lower.contains("sit") || lower.contains("mainuser")) {
                continue;
            }
            group_count = entry.object_children().len();
            for member in entry.object_children() {
                let id = node_int(member, "tamingMobM")
                    .or_else(|| node_int(member, "tamingMobF"))
                    .or_else(|| node_int(member, "tamingMob"))
                    .unwrap_or(0);
                group_taming_ids.push(id);
                group_body_rel_move
                    .push(node_vec(member, "bodyRelMove").unwrap_or(crate::Point::ZERO));
            }
        }
    }
    let body_rel_move = child_node(item, "0")
        .and_then(|n| node_vec(n, "bodyRelMove"))
        .or_else(|| child_node(item, "1").and_then(|n| node_vec(n, "bodyRelMove")))
        .or_else(|| group_body_rel_move.first().copied());

    ChairInfo {
        scale: node_int(info, "scale").unwrap_or(100),
        custom_chair,
        group_count,
        group_taming_ids,
        group_body_rel_move,
        body_rel_move,
        sit_action: node_str(info, "sitAction").filter(|v| !v.is_empty()),
        remove_body: node_int(info, "removeBody").unwrap_or(0) != 0,
        fix_frame_idx: node_int(info, "fixFrameIdx").or_else(|| node_int(info, "fixActionFrame")),
        sit_emotion: node_int(info, "sitEmotion"),
        taming_mob,
        taming: None,
    }
}

/// C# `AddChairPart` 의 메타데이터 부분을 먼저 읽는다.
///
/// 본체 액션을 체어 정보로 결정하려면 픽셀 로드보다 먼저 필요하다.
pub fn load_chair_info(data_root: &std::path::Path, item_id: i32) -> Option<ChairInfo> {
    let (_, meta) = find_install_item(data_root, "", item_id)?;
    let info = child_node(&meta, "info")?;
    let mut chair = parse_chair_info(&meta, info);
    if let Some(id) = chair.taming_mob {
        chair.taming = load_taming_info(data_root, id);
    }
    Some(chair)
}

/// 5x7 비트맵 글리프 (숫자/`-`/`:`/공백).
fn glyph_rows(ch: char) -> Option<[u8; 7]> {
    let rows: [u8; 7] = match ch {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        ':' => [0, 0b00100, 0b00100, 0, 0b00100, 0b00100, 0],
        ' ' => [0; 7],
        _ => return None,
    };
    Some(rows)
}

/// 숫자/기호 텍스트를 RGBA 로 그린다. 검은 그림자 + 흰 글자.
fn draw_text_rgba(text: &str, scale: u32) -> Option<(u32, u32, Vec<u8>)> {
    if text.is_empty() {
        return None;
    }
    let scale = scale.max(1);
    let cell_w = 5 * scale + scale;
    let w = (text.chars().count() as u32) * cell_w + scale;
    let h = 7 * scale + scale;
    let mut out = vec![0u8; (w * h * 4) as usize];
    let mut put = |x: i32, y: i32, color: [u8; 4]| {
        if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
            return;
        }
        let off = ((y as u32 * w + x as u32) * 4) as usize;
        out[off..off + 4].copy_from_slice(&color);
    };
    for (i, ch) in text.chars().enumerate() {
        let Some(rows) = glyph_rows(ch) else {
            continue;
        };
        let ox = i as i32 * cell_w as i32;
        for (ry, row) in rows.iter().enumerate() {
            for cx in 0..5 {
                if row & (1 << (4 - cx)) == 0 {
                    continue;
                }
                for sy in 0..scale as i32 {
                    for sx in 0..scale as i32 {
                        let x = ox + cx * scale as i32 + sx;
                        let y = ry as i32 * scale as i32 + sy;
                        put(x + scale as i32, y + scale as i32, [0, 0, 0, 255]);
                        put(x, y, [255, 255, 255, 255]);
                    }
                }
            }
        }
    }
    Some((w, h, out))
}

fn text_chair_part(
    slot: &str,
    text: &str,
    pos: crate::Point,
    z: i32,
    scale: u32,
) -> Option<(Bone, Vec<AvatarPart>)> {
    let (w, h, rgba) = draw_text_rgba(text, scale)?;
    let mut skin = SkinLayer::new(slot);
    skin.z = "text".into();
    skin.z_index = z;
    skin.origin = crate::Point::ZERO;
    skin.width = w;
    skin.height = h;
    let mut root = Bone::new("@text");
    root.append_bone(skin, slot, pos);
    let part = AvatarPart {
        slot: slot.to_string(),
        width: w,
        height: h,
        rgba,
        position: pos,
        origin: crate::Point::ZERO,
        z_index: z,
        custom_origin_map: Default::default(),
    };
    Some((root, vec![part]))
}

fn load_pop_text(info: &PopChairInfo, pop: i64) -> Option<(Bone, Vec<AvatarPart>)> {
    let mut value = pop;
    if let Some(min) = info.min {
        value = value.max(min);
    }
    if let Some(max) = info.max {
        value = value.min(max);
    }
    text_chair_part(
        "pop_text",
        &value.to_string(),
        info.text_pos,
        info.text_z,
        2,
    )
}

fn load_time_text(info: &TimeChairInfo, minutes: i64) -> Option<(Bone, Vec<AvatarPart>)> {
    let mut value = minutes.max(0);
    if let Some(max) = info.max {
        value = value.min(max);
    }
    let text = format!("{:02}:{:02}", value / 60, value % 60);
    text_chair_part("time_text", &text, info.text_pos, info.text_z, 2)
}

/// 메타데이터 Canvas 또는 `_outlink` 대상 픽셀을 해석한다.
fn decode_chair_canvas(
    node: &ImageNode,
    meta_archive: &WzArchive,
    canvas_archive: &WzArchive,
) -> Option<(u32, u32, Vec<u8>)> {
    if let Some(link) = node_str(node, "_outlink") {
        let resolved = crate::frame_layers::resolve_outlink(canvas_archive, &link).ok()?;
        return Some((resolved.w, resolved.h, resolved.rgba));
    }
    let png = node.first_canvas()?;
    wz_image::decode_png_ref_to_rgba(&png, meta_archive.source()).ok()
}

/// `identityChairInfo/number/0..9` 로 레벨 숫자를 조립한다.
fn load_identity_digits(
    data_root: &std::path::Path,
    item_id: i32,
    identity: &IdentityChairInfo,
    level: i32,
) -> Option<(Bone, Vec<AvatarPart>)> {
    if level < 0 {
        return None;
    }
    let (meta_archive, meta_item) = find_install_item(data_root, "", item_id)?;
    let (canvas_archive, _) = find_install_item(data_root, "_Canvas", item_id)?;
    let mut digits = std::collections::BTreeMap::new();
    for d in 0..10 {
        let path = format!("info/customChair/identityChairInfo/number/{d}");
        let Some(node) = find_path_node(&meta_item, &path) else {
            continue;
        };
        if let Some(decoded) = decode_chair_canvas(node, &meta_archive, &canvas_archive) {
            digits.insert(d, decoded);
        }
    }
    if digits.is_empty() {
        return None;
    }

    let mut root = Bone::new("@identity");
    let mut parts = Vec::new();
    let mut cursor = 0i32;
    for (i, ch) in level.to_string().chars().enumerate() {
        let Some(d) = ch.to_digit(10) else {
            continue;
        };
        let Some((w, h, rgba)) = digits.get(&d) else {
            continue;
        };
        let pos = crate::Point::new(identity.num_pos.x + cursor, identity.num_pos.y);
        let slot = format!("identity_digit_{i}");
        let mut skin = SkinLayer::new(&slot);
        skin.z = "identity".into();
        skin.z_index = 1;
        skin.origin = crate::Point::ZERO;
        skin.width = *w;
        skin.height = *h;
        root.append_bone(skin, &slot, pos);
        parts.push(AvatarPart {
            slot,
            width: *w,
            height: *h,
            rgba: rgba.clone(),
            position: pos,
            origin: crate::Point::ZERO,
            z_index: 1,
            custom_origin_map: Default::default(),
        });
        cursor += *w as i32;
    }
    if parts.is_empty() {
        return None;
    }
    Some((root, parts))
}

/// `scoreChairInfo/scoreEffect` 에서 점수 구간에 맞는 이펙트 프레임을 로드한다.
fn load_score_chair_effect(
    data_root: &std::path::Path,
    item_id: i32,
    score: i32,
) -> Option<(Bone, Vec<AvatarPart>, ChairFrames)> {
    let (meta_archive, meta_item) = find_install_item(data_root, "", item_id)?;
    let (canvas_archive, _) = find_install_item(data_root, "_Canvas", item_id)?;
    let root_node = find_path_node(&meta_item, "info/customChair/scoreChairInfo/scoreEffect")?;
    let group = root_node.object_children().iter().find(|group| {
        let min = node_int(group, "min").unwrap_or(0);
        let max = node_int(group, "max");
        score >= min && max.is_none_or(|m| score <= m)
    })?;
    let effect = child_node(group, "effect")?;
    let z = node_int(effect, "z").unwrap_or(-1);

    let mut frames = Vec::new();
    let mut delays = Vec::new();
    for child in effect.object_children() {
        if child.name.parse::<usize>().is_err() {
            continue;
        }
        let Some((w, h, rgba)) = decode_chair_canvas(child, &meta_archive, &canvas_archive) else {
            continue;
        };
        let origin = node_vec(child, "origin").unwrap_or(crate::Point::ZERO);
        frames.push(vec![AvatarPart {
            slot: "score_effect".into(),
            width: w,
            height: h,
            rgba,
            position: crate::Point::new(-origin.x, -origin.y),
            origin,
            z_index: z,
            custom_origin_map: Default::default(),
        }]);
        delays.push(100);
    }
    let first = frames.first()?.clone();
    let mut root = Bone::new("@score");
    let mut skin = SkinLayer::new("score_effect");
    skin.z = "score".into();
    skin.z_index = z;
    skin.origin = crate::Point::ZERO;
    skin.width = first[0].width;
    skin.height = first[0].height;
    root.append_bone(skin, "score_effect", crate::Point::ZERO);
    Some((root, first, ChairFrames { frames, delays }))
}

/// `part/N/part0..` 구조의 체어 애니메이션을 로드한다.
fn load_part_chair_item(
    data_root: &std::path::Path,
    item_id: i32,
    info: &ChairInfo,
) -> Option<(Bone, Vec<AvatarPart>, ChairFrames)> {
    let anim = info.custom_chair.as_ref()?.part_animation.as_ref()?;
    let (meta_archive, meta_item) = find_install_item(data_root, "", item_id)?;
    let (canvas_archive, canvas_item) = find_install_item(data_root, "_Canvas", item_id)?;
    let base = format!("info/customChair/{}/part", anim.info_name);
    let frame_root = find_path_node(&meta_item, &base)?;
    let canvas_root = find_path_node(&canvas_item, &base);
    let offset = anim.pos_lv.first().copied().unwrap_or(crate::Point::ZERO);

    let mut frames = Vec::new();
    let mut delays = Vec::new();
    let mut first_bone = None;
    for f in 0..anim.frame_count {
        let Some(frame_node) = child_node(frame_root, &f.to_string()) else {
            continue;
        };
        let canvas_frame = canvas_root.and_then(|root| child_node(root, &f.to_string()));
        let Some((bone, parts)) = build_rolling_frame(
            frame_node,
            canvas_frame,
            anim.parts_count,
            &anim.part_z,
            &meta_archive,
            &canvas_archive,
            offset,
        ) else {
            continue;
        };
        if first_bone.is_none() {
            first_bone = Some(bone);
        }
        frames.push(parts);
        let delay = anim
            .delays
            .get(f)
            .copied()
            .or_else(|| node_int(frame_node, "delay").map(|v| v.max(0) as u32))
            .unwrap_or(120)
            .max(20);
        delays.push(delay);
    }
    let first = frames.first()?.clone();
    Some((first_bone?, first, ChairFrames { frames, delays }))
}

/// `androidChairInfo/customEffect` 프레임을 로드한다.
///
/// 메타데이터 캔버스는 1x1 + `_outlink` 이고, 실제 픽셀은 `_Canvas` 아카이브에
/// 있다. 각 프레임은 단일 파트(`android_effect`)로 취급한다.
fn load_android_chair_item(
    data_root: &std::path::Path,
    item_id: i32,
    info: &ChairInfo,
) -> Option<(Bone, Vec<AvatarPart>, ChairFrames)> {
    let (_, meta_item) = find_install_item(data_root, "", item_id)?;
    let (canvas_archive, _) = find_install_item(data_root, "_Canvas", item_id)?;
    let android = info.custom_chair.as_ref()?.android.as_ref()?;
    let effect = find_path_node(&meta_item, "info/customChair/androidChairInfo/customEffect")?;

    let mut frames = Vec::new();
    let mut delays = Vec::new();
    let mut first_bone = None;
    for i in 0..android.effect_frames {
        let Some(node) = child_node(effect, &i.to_string()) else {
            continue;
        };
        let Some(outlink) = node_str(node, "_outlink") else {
            continue;
        };
        let Ok(resolved) = crate::frame_layers::resolve_outlink(&canvas_archive, &outlink) else {
            continue;
        };
        let mut rgba = resolved.rgba;
        if android.reverse_flip {
            rgba = flip_rgba_horizontal(&rgba, resolved.w, resolved.h);
        }
        if first_bone.is_none() {
            let mut root = Bone::new("@android");
            let mut skin = SkinLayer::new("android_effect");
            skin.z = "android".into();
            skin.z_index = android.z;
            skin.origin = crate::Point::ZERO;
            skin.width = resolved.w;
            skin.height = resolved.h;
            root.position = android.pos;
            root.append_bone(skin, "android_effect", crate::Point::ZERO);
            first_bone = Some(root);
        }
        frames.push(vec![AvatarPart {
            slot: "android_effect".into(),
            width: resolved.w,
            height: resolved.h,
            rgba,
            position: android.pos,
            origin: crate::Point::ZERO,
            z_index: android.z,
            custom_origin_map: Default::default(),
        }]);
        // WZ 에 delay 가 없어서 100ms 고정.
        delays.push(100);
    }
    let first = frames.first()?.clone();
    Some((first_bone?, first, ChairFrames { frames, delays }))
}

/// 한 rolling-house 프레임의 8개 파츠를 조립한다.
fn build_rolling_frame(
    frame_node: &ImageNode,
    canvas_frame: Option<&ImageNode>,
    parts_count: usize,
    part_z: &[i32],
    meta_archive: &WzArchive,
    canvas_archive: &WzArchive,
    offset: crate::Point,
) -> Option<(Bone, Vec<AvatarPart>)> {
    let mut children = Vec::new();
    for i in 0..parts_count {
        let Some(part) = child_node(frame_node, &format!("part{i}")) else {
            continue;
        };
        // `_Canvas` 에 실제로 존재하는 첫 프레임 인덱스를 고른다.
        // 일부 파츠는 `0` 이 없고 `1` 부터 시작한다.
        let frame_key = canvas_frame
            .and_then(|root| child_node(root, &format!("part{i}")))
            .and_then(|p| p.object_children().first())
            .map(|c| c.name.to_string())
            .or_else(|| part.object_children().first().map(|c| c.name.to_string()));
        let Some(frame_key) = frame_key else {
            continue;
        };
        let Some(canvas) = child_node(part, &frame_key) else {
            continue;
        };
        let mut canvas = canvas.clone();
        canvas.name = format!("part{i}").into();
        children.push(canvas);
    }
    if children.is_empty() {
        return None;
    }
    let synthetic = ImageNode {
        name: "rollingHouseFrame".into(),
        value: None,
        children,
    };
    let assembly =
        crate::collect_frame_assembly(&synthetic, meta_archive, Some(canvas_archive)).ok()?;

    let mut root = assembly.root_bone;
    root.position = crate::Point::new(-offset.x, -offset.y);

    let mut parts = Vec::new();
    for layer in &assembly.layers {
        let idx = layer
            .name
            .strip_prefix("part")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let z = part_z
            .get(idx)
            .copied()
            .unwrap_or_else(|| layer.z_int.unwrap_or(0));
        set_named_skin_z(&mut root, &layer.name, z);
        parts.push(AvatarPart {
            slot: format!("rolling_part{idx}"),
            width: layer.canvas.w,
            height: layer.canvas.h,
            rgba: layer.canvas.rgba.clone(),
            position: crate::Point::new(layer.pos.x - offset.x, layer.pos.y - offset.y),
            origin: layer.origin,
            z_index: z,
            custom_origin_map: Default::default(),
        });
    }
    Some((root, parts))
}

/// `rollingHouseChairInfo` 의 모든 애니메이션 프레임을 로드한다.
///
/// 메타데이터 캔버스는 1x1 + `_outlink` 이고, 실제 픽셀은 `_Canvas` 아카이브에
/// 있다. `collect_frame_assembly` 가 outlink 를 해석하도록 두 아카이브를 함께 넘긴다.
fn load_rolling_house_item(
    data_root: &std::path::Path,
    item_id: i32,
    info: &ChairInfo,
) -> Option<(Bone, Vec<AvatarPart>, ChairFrames)> {
    let (meta_archive, meta_item) = find_install_item(data_root, "", item_id)?;
    let (canvas_archive, canvas_item) = find_install_item(data_root, "_Canvas", item_id)?;
    let rolling = info.custom_chair.as_ref()?.rolling_house.as_ref()?;
    let rh_node = find_path_node(&meta_item, "info/customChair/rollingHouseChairInfo")?;
    let frame_root = find_path_node(rh_node, "part")?;
    let canvas_root = find_path_node(&canvas_item, "info/customChair/rollingHouseChairInfo/part");
    let offset = rolling
        .pos_lv
        .first()
        .copied()
        .unwrap_or(crate::Point::ZERO);

    let mut frames = Vec::new();
    let mut delays = Vec::new();
    let mut first_bone = None;
    for f in 0..rolling.frame_count {
        let Some(frame_node) = child_node(frame_root, &f.to_string()) else {
            continue;
        };
        let canvas_frame = canvas_root.and_then(|root| child_node(root, &f.to_string()));
        let Some((bone, parts)) = build_rolling_frame(
            frame_node,
            canvas_frame,
            rolling.parts_count,
            &rolling.part_z,
            &meta_archive,
            &canvas_archive,
            offset,
        ) else {
            continue;
        };
        if first_bone.is_none() {
            first_bone = Some(bone);
        }
        frames.push(parts);
        delays.push(node_int(frame_node, "delay").unwrap_or(120).max(20) as u32);
    }
    let first = frames.first()?.clone();
    Some((first_bone?, first, ChairFrames { frames, delays }))
}

/// `Item/Install/_Canvas` 의 체어 이미지를 정적 1프레임으로 로드한다.
///
/// 체어 카테고리 이미지는 `03010.img` 처럼 5자리이고, 그 자식이 아이템 ID 다.
/// `effect/0`, `0`, `default` 순으로 캔버스를 찾는다.
fn load_chair_item(
    data_root: &std::path::Path,
    item_id: i32,
    info: &ChairInfo,
) -> Option<(Bone, Vec<AvatarPart>, Option<ChairFrames>)> {
    if info
        .custom_chair
        .as_ref()
        .and_then(|c| c.android.as_ref())
        .is_some()
    {
        return load_android_chair_item(data_root, item_id, info)
            .map(|(bone, parts, frames)| (bone, parts, Some(frames)));
    }
    if info
        .custom_chair
        .as_ref()
        .and_then(|c| c.rolling_house.as_ref())
        .is_some()
    {
        return load_rolling_house_item(data_root, item_id, info)
            .map(|(bone, parts, frames)| (bone, parts, Some(frames)));
    }
    if info
        .custom_chair
        .as_ref()
        .and_then(|c| c.part_animation.as_ref())
        .is_some()
    {
        return load_part_chair_item(data_root, item_id, info)
            .map(|(bone, parts, frames)| (bone, parts, Some(frames)));
    }
    let (archive, item) = find_install_item(data_root, "_Canvas", item_id)?;

    let png = child_node(&item, "effect")
        .and_then(|effect| child_node(effect, "0"))
        .and_then(|frame| frame.first_canvas())
        .or_else(|| child_node(&item, "0").and_then(|frame| frame.first_canvas()))
        .or_else(|| child_node(&item, "default").and_then(|frame| frame.first_canvas()))
        .or_else(|| item.first_canvas())?;
    let (w, h, rgba) = wz_image::decode_png_ref_to_rgba(&png, archive.source()).ok()?;
    let (origin, z_index) = png_origin_z(&png);

    let mut skin = SkinLayer::new("chair");
    skin.z = "chair".into();
    skin.z_index = z_index;
    skin.origin = origin;
    skin.width = w;
    skin.height = h;

    let mut root = Bone::new("@chair");
    root.position = info.body_rel_move.unwrap_or(crate::Point::ZERO);
    root.append_bone(skin, "chair", crate::Point::ZERO);
    let part = AvatarPart {
        slot: "chair".into(),
        width: w,
        height: h,
        rgba,
        position: crate::Point::new(
            info.body_rel_move.unwrap_or(crate::Point::ZERO).x - origin.x,
            info.body_rel_move.unwrap_or(crate::Point::ZERO).y - origin.y,
        ),
        origin,
        z_index,
        ..Default::default()
    };
    Some((root, vec![part], None))
}

/// RGBA8 버퍼를 최근접 이웃으로 `factor` 배 확대/축소한다.
fn resize_rgba_nearest(src: &[u8], w: u32, h: u32, factor: f32) -> (u32, u32, Vec<u8>) {
    let nw = ((w as f32 * factor).round() as u32).max(1);
    let nh = ((h as f32 * factor).round() as u32).max(1);
    if nw == w && nh == h {
        return (w, h, src.to_vec());
    }
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        let sy = ((y as f32 / factor) as u32).min(h.saturating_sub(1));
        for x in 0..nw {
            let sx = ((x as f32 / factor) as u32).min(w.saturating_sub(1));
            let src_off = ((sy * w + sx) * 4) as usize;
            let dst_off = ((y * nw + x) * 4) as usize;
            out[dst_off..dst_off + 4].copy_from_slice(&src[src_off..src_off + 4]);
        }
    }
    (nw, nh, out)
}

fn is_scaled_slot(slot: &str) -> bool {
    !matches!(slot, "chair" | "effect" | "taming") && !slot.starts_with("tamingmob")
}

fn scale_bone_tree(bone: &mut Bone, factor: f32, scaled: &std::collections::HashSet<String>) {
    let has_scaled = bone.skins.iter().any(|s| scaled.contains(&s.name));
    if has_scaled {
        bone.position = crate::Point::new(
            (bone.position.x as f32 * factor).round() as i32,
            (bone.position.y as f32 * factor).round() as i32,
        );
        for skin in &mut bone.skins {
            if scaled.contains(&skin.name) {
                skin.origin = crate::Point::new(
                    (skin.origin.x as f32 * factor).round() as i32,
                    (skin.origin.y as f32 * factor).round() as i32,
                );
                skin.offset = crate::Point::new(
                    (skin.offset.x as f32 * factor).round() as i32,
                    (skin.offset.y as f32 * factor).round() as i32,
                );
                skin.width = (skin.width as f32 * factor).round().max(1.0) as u32;
                skin.height = (skin.height as f32 * factor).round().max(1.0) as u32;
            }
        }
    }
    for child in &mut bone.children {
        scale_bone_tree(child, factor, scaled);
    }
}

/// 체어의 `scale`/`scaleAvatar.scale` 을 아바타 파트와 bone 에 적용한다.
fn apply_avatar_scale(root: &mut Bone, parts: &mut [AvatarPart], scale: i32) {
    if scale <= 0 || scale == 100 {
        return;
    }
    let factor = scale as f32 / 100.0;
    let scaled: std::collections::HashSet<String> = parts
        .iter()
        .filter(|p| is_scaled_slot(&p.slot))
        .map(|p| p.slot.clone())
        .collect();
    for part in parts.iter_mut() {
        if !scaled.contains(&part.slot) {
            continue;
        }
        let (w, h, rgba) = resize_rgba_nearest(&part.rgba, part.width, part.height, factor);
        part.width = w;
        part.height = h;
        part.position = crate::Point::new(
            (part.position.x as f32 * factor).round() as i32,
            (part.position.y as f32 * factor).round() as i32,
        );
        part.origin = crate::Point::new(
            (part.origin.x as f32 * factor).round() as i32,
            (part.origin.y as f32 * factor).round() as i32,
        );
        part.rgba = rgba;
    }
    scale_bone_tree(root, factor, &scaled);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransformOp {
    FlipX,
    FlipY,
    Rotate90,
    Rotate180,
    Rotate270,
}

fn transform_ops(transform: AvatarTransform) -> Vec<TransformOp> {
    let mut ops = Vec::new();
    if transform.flip_x {
        ops.push(TransformOp::FlipX);
    }
    if transform.flip_y {
        ops.push(TransformOp::FlipY);
    }
    match transform.rotate.rem_euclid(360) {
        90 => ops.push(TransformOp::Rotate90),
        180 => ops.push(TransformOp::Rotate180),
        270 => ops.push(TransformOp::Rotate270),
        _ => {}
    }
    ops
}

fn flip_rgba_horizontal(src: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    for y in 0..h {
        for x in 0..w {
            let sx = w - 1 - x;
            let s = ((y * w + sx) * 4) as usize;
            let d = ((y * w + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    out
}

fn flip_rgba_vertical(src: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    for y in 0..h {
        let sy = h - 1 - y;
        for x in 0..w {
            let s = ((sy * w + x) * 4) as usize;
            let d = ((y * w + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    out
}

fn rotate_rgba_180(src: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    for y in 0..h {
        let sy = h - 1 - y;
        for x in 0..w {
            let sx = w - 1 - x;
            let s = ((sy * w + sx) * 4) as usize;
            let d = ((y * w + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    out
}

/// 시계 방향 90도. 결과 크기는 `h x w`.
fn rotate_rgba_cw(src: &[u8], w: u32, h: u32) -> Vec<u8> {
    let nw = h;
    let nh = w;
    let mut out = vec![0u8; src.len()];
    for y in 0..nh {
        for x in 0..nw {
            // dst(x,y) = src(y, h-1-x)
            let sx = y;
            let sy = h - 1 - x;
            let s = ((sy * w + sx) * 4) as usize;
            let d = ((y * nw + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    out
}

/// 반시계 방향 90도. 결과 크기는 `h x w`.
fn rotate_rgba_ccw(src: &[u8], w: u32, h: u32) -> Vec<u8> {
    let nw = h;
    let nh = w;
    let mut out = vec![0u8; src.len()];
    for y in 0..nh {
        for x in 0..nw {
            // dst(x,y) = src(w-1-y, x)
            let sx = w - 1 - y;
            let sy = x;
            let s = ((sy * w + sx) * 4) as usize;
            let d = ((y * nw + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    out
}

fn transform_part_op(part: &mut AvatarPart, op: TransformOp) {
    match op {
        TransformOp::FlipX => {
            part.rgba = flip_rgba_horizontal(&part.rgba, part.width, part.height);
            part.position.x = -part.position.x - part.width as i32;
            part.origin.x = part.width as i32 - part.origin.x;
        }
        TransformOp::FlipY => {
            part.rgba = flip_rgba_vertical(&part.rgba, part.width, part.height);
            part.position.y = -part.position.y - part.height as i32;
            part.origin.y = part.height as i32 - part.origin.y;
        }
        TransformOp::Rotate90 => {
            part.rgba = rotate_rgba_cw(&part.rgba, part.width, part.height);
            let h = part.height as i32;
            part.position = crate::Point::new(-part.position.y - h, part.position.x);
            part.origin = crate::Point::new(h - part.origin.y, part.origin.x);
            std::mem::swap(&mut part.width, &mut part.height);
        }
        TransformOp::Rotate180 => {
            part.rgba = rotate_rgba_180(&part.rgba, part.width, part.height);
            part.position = crate::Point::new(
                -part.position.x - part.width as i32,
                -part.position.y - part.height as i32,
            );
            part.origin = crate::Point::new(
                part.width as i32 - part.origin.x,
                part.height as i32 - part.origin.y,
            );
        }
        TransformOp::Rotate270 => {
            part.rgba = rotate_rgba_ccw(&part.rgba, part.width, part.height);
            let w = part.width as i32;
            part.position = crate::Point::new(part.position.y, -part.position.x - w);
            part.origin = crate::Point::new(part.origin.y, w - part.origin.x);
            std::mem::swap(&mut part.width, &mut part.height);
        }
    }
}

fn transform_bone_op(bone: &mut Bone, op: TransformOp) {
    match op {
        TransformOp::FlipX => {
            bone.position.x = -bone.position.x;
            for skin in &mut bone.skins {
                skin.offset.x = -skin.offset.x;
                skin.origin.x = skin.width as i32 - skin.origin.x;
            }
        }
        TransformOp::FlipY => {
            bone.position.y = -bone.position.y;
            for skin in &mut bone.skins {
                skin.offset.y = -skin.offset.y;
                skin.origin.y = skin.height as i32 - skin.origin.y;
            }
        }
        TransformOp::Rotate90 => {
            bone.position = crate::Point::new(-bone.position.y, bone.position.x);
            for skin in &mut bone.skins {
                skin.offset = crate::Point::new(-skin.offset.y, skin.offset.x);
                skin.origin = crate::Point::new(skin.height as i32 - skin.origin.y, skin.origin.x);
                std::mem::swap(&mut skin.width, &mut skin.height);
            }
        }
        TransformOp::Rotate180 => {
            bone.position = crate::Point::new(-bone.position.x, -bone.position.y);
            for skin in &mut bone.skins {
                skin.offset = crate::Point::new(-skin.offset.x, -skin.offset.y);
                skin.origin = crate::Point::new(
                    skin.width as i32 - skin.origin.x,
                    skin.height as i32 - skin.origin.y,
                );
            }
        }
        TransformOp::Rotate270 => {
            bone.position = crate::Point::new(bone.position.y, -bone.position.x);
            for skin in &mut bone.skins {
                skin.offset = crate::Point::new(skin.offset.y, -skin.offset.x);
                skin.origin = crate::Point::new(skin.origin.y, skin.width as i32 - skin.origin.x);
                std::mem::swap(&mut skin.width, &mut skin.height);
            }
        }
    }
    for child in &mut bone.children {
        transform_bone_op(child, op);
    }
}

/// WZ 체어 변환을 파트와 bone 트리에 동시에 적용한다.
pub fn apply_avatar_transform(
    root: &mut Bone,
    parts: &mut [AvatarPart],
    transform: AvatarTransform,
) {
    if transform.is_identity() {
        return;
    }
    for op in transform_ops(transform) {
        for part in parts.iter_mut() {
            transform_part_op(part, op);
        }
        transform_bone_op(root, op);
    }
    if transform.offset != crate::Point::ZERO {
        for part in parts.iter_mut() {
            part.position = crate::Point::new(
                part.position.x + transform.offset.x,
                part.position.y + transform.offset.y,
            );
        }
        root.position = crate::Point::new(
            root.position.x + transform.offset.x,
            root.position.y + transform.offset.y,
        );
    }
}

/// `name:x,y` 로 지정한 커스텀 오리진을 bone 의 skin 에 적용한다.
fn apply_custom_origins(bone: &mut Bone, map: &std::collections::BTreeMap<String, crate::Point>) {
    if map.is_empty() {
        return;
    }
    for skin in &mut bone.skins {
        if let Some(origin) = map.get(&skin.name) {
            skin.origin = *origin;
        }
    }
    for child in &mut bone.children {
        apply_custom_origins(child, map);
    }
}

/// C# `CustomOriginMap`(originValue → childName) + `CustomOrigin`(childName → Point)
/// 두 단계를 거쳐 skin origin 을 덮어쓴다.
fn apply_custom_origin_map(
    root: &mut Bone,
    parts: &[AvatarPart],
    selected: &std::collections::BTreeMap<String, crate::Point>,
) {
    if selected.is_empty() {
        return;
    }
    for part in parts {
        let mut effective = std::collections::BTreeMap::new();
        for (origin_value, child_name) in &part.custom_origin_map {
            if let Some(point) = selected.get(child_name) {
                effective.insert(origin_value.clone(), *point);
            }
        }
        if !effective.is_empty() {
            apply_custom_origins(root, &effective);
        }
    }
}

/// `Effect/Effect_*.wz` 의 `ItemEff.img/<id>/effect/default/0` 을 정적 1프레임으로 로드한다.
///
/// C# 은 effect 를 애니메이션하지만, v0.1 이후 PoC 는 첫 프레임만 합성한다.
fn load_effect_item(
    data_root: &std::path::Path,
    item_id: i32,
    frame_index: usize,
) -> Option<EffectFrames> {
    use wz_core::image::ImageValue;

    let effect_dir = data_root.join("Effect");
    let mut parsed = None;
    for entry in std::fs::read_dir(effect_dir).ok()?.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if !name.starts_with("Effect") || path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("ItemEff.img"))
        else {
            continue;
        };
        if let Ok(node) = archive.parse_image_at(entry.offset?) {
            parsed = Some(node);
            break;
        }
    }
    let item_eff = parsed?;
    let item = child_node(&item_eff, &item_id.to_string())?;
    let effect = child_node(item, "effect")?;
    let default = child_node(effect, "default")?;
    // `default` children are `z`, `pos`, `0`, `1`, … — only numeric children are frames.
    let frames: Vec<&ImageNode> = default
        .object_children()
        .iter()
        .filter(|c| c.name.parse::<usize>().is_ok())
        .collect();
    if frames.is_empty() {
        return None;
    }
    let canvas_archive = find_effect_canvas_archive(data_root, "ItemEff.img")?;
    let effect_z = child_node(effect, "z")
        .and_then(|n| match &n.value {
            Some(ImageValue::Int(v)) => Some(*v),
            _ => None,
        })
        .unwrap_or(0);

    let selected_index = frame_index % frames.len();
    let mut parts = Vec::new();
    let mut delays = Vec::new();
    let mut origins = Vec::new();
    let mut z_indices = Vec::new();
    let mut selected_meta = None;

    for (i, frame) in frames.iter().enumerate() {
        let png = frame.first_canvas()?;
        let outlink = png.sub_property.as_ref().and_then(|sub| {
            sub.children.iter().find_map(|c| match &c.value {
                Some(ImageValue::Str(v)) if c.name.as_ref() == "_outlink" => Some(v.to_string()),
                _ => None,
            })
        })?;
        let resolved = crate::resolve_outlink(&canvas_archive, &outlink).ok()?;
        let (origin, frame_z) = png_origin_z(&png);
        let z_index = if frame_z != 0 { frame_z } else { effect_z };
        let delay = png
            .sub_property
            .as_ref()
            .and_then(|sub| {
                sub.children
                    .iter()
                    .find_map(|c| match (&c.name.as_ref(), &c.value) {
                        (&"delay", Some(ImageValue::Int(v))) => Some((*v).max(0) as u32),
                        _ => None,
                    })
            })
            .unwrap_or(0);
        delays.push(delay);
        origins.push(origin);
        z_indices.push(z_index);
        if i == selected_index {
            selected_meta = Some((resolved.w, resolved.h, origin, z_index));
        }
        parts.push(AvatarPart {
            slot: "effect".into(),
            width: resolved.w,
            height: resolved.h,
            rgba: resolved.rgba,
            position: crate::Point::new(-origin.x, -origin.y),
            origin,
            z_index,
            ..Default::default()
        });
    }

    let (w, h, origin, z_index) = selected_meta?;
    let mut skin = SkinLayer::new("effect");
    skin.z = "effect".into();
    skin.z_index = z_index;
    skin.origin = origin;
    skin.width = w;
    skin.height = h;

    let mut root = Bone::new("@effect");
    root.append_bone(skin, "effect", crate::Point::ZERO);
    Some(EffectFrames {
        root_bone: root,
        parts,
        delays,
        origins,
        z_indices,
    })
}

/// `info/customOrigin` 의 자식들에서 `WZ child name → custom origin ID` 맵을 만든다.
fn parse_custom_origin(node: &ImageNode) -> std::collections::BTreeMap<String, String> {
    use wz_core::image::ImageValue;

    let mut out = std::collections::BTreeMap::new();
    let Some(info) = node.children.iter().find(|c| c.name.as_ref() == "info") else {
        return out;
    };
    let Some(custom) = info
        .children
        .iter()
        .find(|c| c.name.as_ref() == "customOrigin")
    else {
        return out;
    };
    for child in &custom.children {
        if let Some(ImageValue::Str(value)) = &child.value {
            out.insert(value.to_string(), child.name.to_string());
        }
    }
    out
}

/// `_Canvas` 아이템 이미지 하나를 `collect_frame_assembly` 로 조립한다.
///
/// `image_name` 이 `None` 이면 아카이브의 첫 이미지를 쓴다. 장비/무기 이미지는
/// `default`/`stand1`/`walk1` 액션을 가지며, 액션 자체가 프레임인 경우와
/// `0` 같은 자식이 프레임인 경우를 모두 처리한다.
fn load_item_frame(
    archive: &WzArchive,
    image_name: Option<&str>,
    action_names: &[&str],
    slot: &str,
) -> Option<(Bone, Vec<AvatarPart>)> {
    use wz_core::image::ImageValue;

    let tree = archive.read_tree().ok()?;
    let entry = match image_name {
        Some(name) => tree
            .root
            .entries
            .iter()
            .find(|e| e.node_type == 4 && e.name.as_deref() == Some(name)),
        None => tree.root.entries.iter().find(|e| e.node_type == 4),
    }?;
    let node = archive.parse_image_at(entry.offset?).ok()?;
    let custom_origin = parse_custom_origin(&node);
    let action = action_names
        .iter()
        .find_map(|name| node.children.iter().find(|c| c.name.as_ref() == *name))
        .or_else(|| node.children.iter().find(|c| c.name.as_ref() == "default"))
        .or_else(|| node.children.first())
        .unwrap_or(&node);
    let action_children: &[ImageNode] = match &action.value {
        Some(ImageValue::Property(p)) => &p.children,
        _ => &action.children,
    };
    let frame = if action_children
        .iter()
        .any(|c| matches!(c.value, Some(ImageValue::Png(_))))
    {
        action
    } else {
        action_children.first().unwrap_or(action)
    };

    let assembly = crate::collect_frame_assembly(frame, archive, Some(archive)).ok()?;
    let base_slot = slot.to_lowercase();
    let parts = assembly
        .layers
        .iter()
        .enumerate()
        .map(|(i, layer)| {
            let name = if i == 0 {
                base_slot.clone()
            } else {
                format!("{base_slot}_{}", layer.name)
            };
            AvatarPart {
                slot: name,
                width: layer.canvas.w,
                height: layer.canvas.h,
                rgba: layer.canvas.rgba.clone(),
                position: layer.pos,
                origin: layer.origin,
                z_index: layer.z_int.unwrap_or(0),
                custom_origin_map: custom_origin.clone(),
            }
        })
        .collect();
    Some((assembly.root_bone, parts))
}

/// `Character/TamingMob/_Canvas` 에서 taming mob 한 마리를 로드한다.
fn load_taming_item(data_root: &std::path::Path, item_id: i32) -> Option<(Bone, Vec<AvatarPart>)> {
    let dir = data_root
        .join("Character")
        .join("TamingMob")
        .join("_Canvas");
    let image = format!("{:08}.img", item_id);
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        if tree
            .root
            .entries
            .iter()
            .any(|e| e.name.as_deref() == Some(image.as_str()))
        {
            return load_item_frame(
                &archive,
                Some(&image),
                &["sit", "stand1", "default"],
                "TamingMob",
            );
        }
    }
    None
}

/// `Character/<dir>/_Canvas/_Canvas_000.wz` 를 열어 아이템 이미지를 조립한다.
fn load_item_canvas(
    char_root: &std::path::Path,
    dir: &str,
    image_name: &str,
    action_names: &[&str],
) -> Option<(Bone, Vec<AvatarPart>)> {
    let path = char_root.join(dir).join("_Canvas").join("_Canvas_000.wz");
    if !path.exists() {
        return None;
    }
    let archive = WzArchive::open(&path).ok()?;
    load_item_frame(&archive, Some(image_name), action_names, dir)
}

/// `_Canvas` 아카이브의 첫 이미지를 조립한다 (PoC 장비/무기 슬롯).
fn load_first_item_canvas(
    char_root: &std::path::Path,
    dir: &str,
    action_names: &[&str],
) -> Option<(Bone, Vec<AvatarPart>)> {
    let path = char_root.join(dir).join("_Canvas").join("_Canvas_000.wz");
    if !path.exists() {
        return None;
    }
    let archive = WzArchive::open(&path).ok()?;
    load_item_frame(&archive, None, action_names, dir)
}

pub fn load_from_character_wz(char_wz_path: &std::path::Path) -> Option<LoadedAvatar> {
    load_from_character_wz_with_skin(char_wz_path, 2000)
}

pub fn load_from_character_wz_with_skin(
    char_wz_path: &std::path::Path,
    body_skin: i32,
) -> Option<LoadedAvatar> {
    let data_root = char_wz_path
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());
    let canvas_path = char_wz_path
        .parent()?
        .join("_Canvas")
        .join("_Canvas_000.wz");
    let arch = WzArchive::open(&canvas_path).ok()?;

    let (mut root, mut parts) = load_frame_parts(
        &arch,
        &format!("{:08}.img", body_skin),
        &["stand1", "stand", "alert"],
    )?;

    if let Some((head_root, head_parts)) =
        load_frame_parts(&arch, "00012000.img", &["front", "back"])
    {
        root.skins.extend(head_root.skins);
        root.children.extend(head_root.children);
        parts.extend(head_parts);
    }

    // 3. 장비 아이템 로딩 (Cap 1000000)
    let char_root = char_wz_path.parent().unwrap_or(char_wz_path);
    if let Some((item_root, item_parts)) =
        load_item_canvas(char_root, "Cap", "01000000.img", &["default", "stand1"])
    {
        merge_bone_tree(&mut root, item_root);
        parts.extend(item_parts);
    }

    // 4. 추가 장비 슬롯 로딩 (각 슬롯의 첫 번째 아이템)
    for slot_name in ["Coat", "Pants", "Shoes", "Glove", "Cape"] {
        if let Some((item_root, item_parts)) =
            load_first_item_canvas(char_root, slot_name, &["default", "stand1"])
        {
            merge_bone_tree(&mut root, item_root);
            parts.extend(item_parts);
        }
    }

    // 5. Weapon 슬롯 로딩 (첫 아이템)
    if let Some((weapon_root, weapon_parts)) =
        load_first_item_canvas(char_root, "Weapon", &["stand1", "walk1", "default"])
    {
        merge_bone_tree(&mut root, weapon_root);
        parts.extend(weapon_parts);
    }

    if parts.is_empty() {
        return None;
    }

    // `Base/zmap.img` 가 있으면 그 순서를, 없으면 표준 MapleStory 순서를 쓴다.
    let mut zmap_order: Vec<String> = load_zmap_order(data_root.as_deref())
        .unwrap_or_else(|| STANDARD_ZMAP.iter().map(|s| s.to_string()).collect());
    // 로드된 파트의 z-tag 가 표준에 없으면 추가
    for p in &parts {
        let zt = part_z_tag(&p.slot);
        if !zmap_order.contains(&zt) {
            zmap_order.push(zt);
        }
    }
    let zmap = Arc::new(ZMap::new(zmap_order));
    let scene = AvatarScene::new(zmap.clone());

    Some(LoadedAvatar {
        parts,
        root_bone: root,
        zmap,
        scene,
        effect_frames: Vec::new(),
        effect_delays: Vec::new(),
        effect_origins: Vec::new(),
        effect_z_indices: Vec::new(),
        chair_info: None,
        transform: AvatarTransform::default(),
        chair_frames: Vec::new(),
        chair_delays: Vec::new(),
    })
}

/// 장비 슬롯 → 디렉토리 이름 매핑.
pub fn slot_dir_name(slot: &str) -> Option<&'static str> {
    match slot {
        "Cap" => Some("Cap"),
        "Coat" => Some("Coat"),
        "Longcoat" => Some("Longcoat"),
        "Pants" => Some("Pants"),
        "Shoes" => Some("Shoes"),
        "Glove" => Some("Glove"),
        "Cape" => Some("Cape"),
        "Weapon" => Some("Weapon"),
        "Shield" | "SubWeapon" => Some("Shield"),
        "FaceAccessory" => Some("Accessory"),
        "Hair" => Some("Hair"),
        "Face" => Some("Face"),
        "Taming" => Some("TamingMob"),
        "Effect" => Some("Effect"),
        "Chair" => Some("Chair"),
        _ => None,
    }
}

/// 아이템 ID → 슬롯 이름 추론 (MapleStory 8자리 ID 규칙).
pub fn item_id_to_slot(id: i32) -> Option<&'static str> {
    let prefix = id / 100000;
    match prefix {
        100 => Some("Cap"),
        104 => Some("Coat"),
        105 => Some("Longcoat"),
        106 => Some("Pants"),
        107 => Some("Shoes"),
        108 => Some("Glove"),
        109 => Some("Shield"),
        110 => Some("Cape"),
        130 | 131 | 132 | 133 | 134 | 136 | 137 | 138 | 140..=159 => Some("Weapon"),
        _ => None,
    }
}

/// 장비 아이템 이미지에서 첫 번째 Canvas 를 추출한다.
/// WZ 구조: item.img → action (default/backDefault) → [frame] → Canvas
/// 장비 아이템을 로드하여 (slot_name, width, height, rgba) 를 반환한다.
/// wz_path: Character/{Slot}/_Canvas/_Canvas_000.wz 경로
/// item_id: 장비 아이템 ID (8자리)
pub fn load_equipment_item(
    wz_path: &std::path::Path,
    item_id: i32,
) -> Option<(String, u32, u32, Vec<u8>)> {
    use wz_core::WzArchive;
    let arch = WzArchive::open(wz_path).ok()?;
    let tree = arch.read_tree().ok()?;
    let file_name = format!("{:08}.img", item_id);
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.node_type == 4 && e.name.as_deref() == Some(file_name.as_str()))?;
    let off = entry.offset?;
    let node = arch.parse_image_at(off).ok()?;
    let png = node.first_canvas()?;
    let file_src = wz_pool::FileSource::open(wz_path).ok()?;
    let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
    let (w, h, rgba) = decode_png_ref_to_rgba(&png, &src).ok()?;
    let slot = item_id_to_slot(item_id).unwrap_or("Unknown").to_string();
    Some((slot, w, h, rgba))
}

/// 장비 아이템 ID 목록으로 아바타를 로드한다.
/// data_root: MapleStory Data 디렉터리 경로
/// equipment: (slot_name, item_id) 쌍의 목록
/// Hair/Face 믹스 컬러와 특수 레이어 설정.
///
/// WZ/UI 가 공급한다 (C# `AvatarPart.MixColor`/`MixOpacity`, `AvatarCanvas.CustomOrigin`).
#[derive(Debug, Clone, Default)]
pub struct MixOptions {
    pub hair: Option<(i32, i32)>,
    pub face: Option<(i32, i32)>,
    /// `Effect` 슬롯의 정적 프레임 인덱스 (프레임 수로 modulo).
    pub effect_frame: usize,
    /// `name:x,y` 로 지정한 커스텀 오리진. `SkinLayer.name` 과 매칭한다.
    pub custom_origins: std::collections::BTreeMap<String, crate::Point>,
    /// identity/score 체어에 표시할 호출자 값.
    pub chair: ChairUserData,
}

pub fn load_avatar_with_equipment(
    data_root: &std::path::Path,
    body_skin: i32,
    equipment: &[(String, i32)],
) -> Option<LoadedAvatar> {
    load_avatar_with_equipment_and_mix(data_root, body_skin, equipment, &MixOptions::default())
}

pub fn load_avatar_with_equipment_and_mix(
    data_root: &std::path::Path,
    body_skin: i32,
    equipment: &[(String, i32)],
    mix: &MixOptions,
) -> Option<LoadedAvatar> {
    // Body 로드 (0000{skin}00.img)
    let body_dir = data_root.join("Character");
    let canvas_dir = body_dir.join("_Canvas");
    let canvas_path = canvas_dir.join("_Canvas_000.wz");
    if !canvas_path.exists() {
        return None;
    }

    let arch = WzArchive::open(&canvas_path).ok()?;

    let chair_info = equipment.iter().find_map(|(slot, item_id)| {
        (slot == "Chair")
            .then(|| load_chair_info(data_root, *item_id))
            .flatten()
    });
    let chair_action = chair_info.as_ref().map(|c| c.body_action());
    let body_image = format!("{:08}.img", body_skin);
    let (mut root, mut parts) = match &chair_action {
        Some(ChairBodyAction::Hide) => (Bone::new("@root"), Vec::new()),
        Some(ChairBodyAction::Action { name, frame }) => {
            let actions = [name.as_str(), "sit", "stand1", "stand"];
            load_frame_parts_at(&arch, &body_image, &actions, *frame)?
        }
        None => load_frame_parts(&arch, &body_image, &["stand1", "stand", "alert"])?,
    };

    // C# `hideBody`: bodyAction 이 null 이면 LinkPlayerParts 가 body+head 를 모두 숨긴다.
    if !matches!(&chair_action, Some(ChairBodyAction::Hide)) {
        if let Some((head_root, head_parts)) =
            load_frame_parts(&arch, "00012000.img", &["front", "back"])
        {
            root.skins.extend(head_root.skins);
            root.children.extend(head_root.children);
            parts.extend(head_parts);
        }
    }

    // Equipment 로드
    let char_root = data_root.join("Character");
    let mut effect_frames: Vec<AvatarPart> = Vec::new();
    let mut effect_delays: Vec<u32> = Vec::new();
    let mut effect_origins: Vec<crate::Point> = Vec::new();
    let mut effect_z_indices: Vec<i32> = Vec::new();
    let mut chair_info = chair_info;
    let mut chair_frames: Vec<Vec<AvatarPart>> = Vec::new();
    let mut chair_delays: Vec<u32> = Vec::new();
    for (slot, item_id) in equipment {
        if let Some(dir) = slot_dir_name(slot) {
            if dir == "Effect" {
                if let Some(effect) = load_effect_item(data_root, *item_id, mix.effect_frame) {
                    let EffectFrames {
                        root_bone,
                        parts: frame_parts,
                        delays,
                        origins,
                        z_indices,
                    } = effect;
                    merge_bone_tree(&mut root, root_bone);
                    let selected = mix.effect_frame.min(frame_parts.len().saturating_sub(1));
                    if let Some(part) = frame_parts.get(selected).cloned() {
                        parts.push(part);
                    }
                    effect_frames = frame_parts;
                    effect_delays = delays;
                    effect_origins = origins;
                    effect_z_indices = z_indices;
                }
                continue;
            }
            if dir == "Chair" {
                if chair_info.is_none() {
                    chair_info = load_chair_info(data_root, *item_id);
                }
                // C# 은 `info/tamingMob` 이 있어도 ChairPart 와 TamingPart 를
                // **둘 다** 추가한다 (removeChairPart 는 이전 체어만 지운다).
                if let Some(info) = &chair_info {
                    if let Some((item_root, item_parts, frames)) =
                        load_chair_item(data_root, *item_id, info)
                    {
                        if let Some(frames) = frames {
                            chair_frames = frames.frames;
                            chair_delays = frames.delays;
                        }
                        merge_bone_tree(&mut root, item_root);
                        parts.extend(item_parts);
                    }
                    if let Some(custom) = &info.custom_chair {
                        if let (Some(identity), Some(level)) =
                            (custom.identity.as_ref(), mix.chair.level)
                        {
                            if let Some((digit_root, digit_parts)) =
                                load_identity_digits(data_root, *item_id, identity, level)
                            {
                                merge_bone_tree(&mut root, digit_root);
                                parts.extend(digit_parts);
                            }
                        }
                        if custom.score.is_some() {
                            if let Some(score) = mix.chair.score {
                                if let Some((score_root, score_parts, frames)) =
                                    load_score_chair_effect(data_root, *item_id, score)
                                {
                                    chair_frames = frames.frames;
                                    chair_delays = frames.delays;
                                    merge_bone_tree(&mut root, score_root);
                                    parts.extend(score_parts);
                                }
                            }
                        }
                        if let (Some(pop_info), Some(pop)) = (custom.pop.as_ref(), mix.chair.pop) {
                            if let Some((text_root, text_parts)) = load_pop_text(pop_info, pop) {
                                merge_bone_tree(&mut root, text_root);
                                parts.extend(text_parts);
                            }
                        }
                        if let (Some(time_info), Some(minutes)) =
                            (custom.time.as_ref(), mix.chair.time_minutes)
                        {
                            if let Some((text_root, text_parts)) =
                                load_time_text(time_info, minutes)
                            {
                                merge_bone_tree(&mut root, text_root);
                                parts.extend(text_parts);
                            }
                        }
                    }
                }
                continue;
            }
            if dir == "Weapon" && chair_info.as_ref().is_some_and(|c| c.hides_weapon()) {
                continue;
            }
            let path = char_root.join(dir).join("_Canvas").join("_Canvas_000.wz");
            if !path.exists() {
                continue;
            }
            let Ok(archive) = WzArchive::open(&path) else {
                continue;
            };
            let image_name = format!("{:08}.img", item_id);
            let actions: &[&str] = if dir == "TamingMob" {
                &["sit", "stand1", "default"]
            } else {
                &["default", "stand1"]
            };
            if let Some((mut item_root, mut item_parts)) =
                load_item_frame(&archive, Some(&image_name), actions, dir)
            {
                let mix_setting = match dir {
                    "Hair" => mix.hair,
                    "Face" => mix.face,
                    _ => None,
                };
                if let Some((color, opacity)) = mix_setting {
                    apply_mix_to_item(&archive, *item_id, dir, &mut item_parts, color, opacity);
                }
                apply_custom_origin_map(&mut item_root, &item_parts, &mix.custom_origins);
                merge_bone_tree(&mut root, item_root);
                parts.extend(item_parts);
            }
        }
    }

    if let Some(info) = &chair_info {
        let has_group = !info.group_taming_ids.is_empty();
        if has_group {
            for (i, id) in info.group_taming_ids.iter().enumerate() {
                if *id == 0 {
                    continue;
                }
                if let Some((mut taming_root, mut taming_parts)) = load_taming_item(data_root, *id)
                {
                    let offset = info
                        .group_body_rel_move
                        .get(i)
                        .copied()
                        .unwrap_or(crate::Point::ZERO);
                    taming_root.position = offset;
                    for part in &mut taming_parts {
                        part.position = crate::Point::new(
                            part.position.x + offset.x,
                            part.position.y + offset.y,
                        );
                        if i > 0 {
                            part.slot = format!("{}_{}", part.slot, i);
                        }
                    }
                    merge_bone_tree(&mut root, taming_root);
                    parts.extend(taming_parts);
                }
            }
        } else if let Some(taming) = &info.taming {
            // 단일 taming 체어 — C# 은 ChairPart 와 TamingPart 를 함께 추가한다.
            if let Some((mut taming_root, mut taming_parts)) =
                load_taming_item(data_root, taming.id)
            {
                let offset = info.body_rel_move.unwrap_or(crate::Point::ZERO);
                taming_root.position = offset;
                for part in &mut taming_parts {
                    part.position =
                        crate::Point::new(part.position.x + offset.x, part.position.y + offset.y);
                }
                merge_bone_tree(&mut root, taming_root);
                parts.extend(taming_parts);
            }
        }

        // C# `fAvatarScale`: 선택된 taming 의 `info/scale` 이 아바타 전체 배율이다.
        let scale = if has_group {
            info.group_taming_ids
                .iter()
                .find(|id| **id != 0)
                .and_then(|id| load_taming_info(data_root, *id))
                .map(|t| t.scale)
                .unwrap_or(100)
        } else if let Some(taming) = &info.taming {
            taming.scale
        } else {
            info.effective_scale()
        };
        apply_avatar_scale(&mut root, &mut parts, scale);

        // WZ `dummyAvatarChair`: `customChair/avatar/*` 의 더미 아바타를 추가한다.
        // C# 은 이 데이터를 렌더링하지 않으므로, 이 경로는 WZ 스키마 기반 확장이다.
        if let Some(dummy) = info.custom_chair.as_ref().and_then(|c| c.dummy.as_ref()) {
            for (i, entry) in dummy.avatars.iter().enumerate() {
                if entry.taming_mob == 0 || Some(entry.taming_mob) == info.taming_mob {
                    continue;
                }
                let Some((mut taming_root, mut taming_parts)) =
                    load_taming_item(data_root, entry.taming_mob)
                else {
                    continue;
                };
                if entry.left {
                    let flip = AvatarTransform {
                        flip_x: true,
                        ..Default::default()
                    };
                    apply_avatar_transform(&mut taming_root, &mut taming_parts, flip);
                }
                taming_root.position = entry.position;
                set_bone_z(&mut taming_root, entry.z);
                for part in &mut taming_parts {
                    part.position = crate::Point::new(
                        part.position.x + entry.position.x,
                        part.position.y + entry.position.y,
                    );
                    part.z_index = entry.z;
                    part.slot = format!("dummy{i}_{}", part.slot);
                }
                merge_bone_tree(&mut root, taming_root);
                parts.extend(taming_parts);
            }
        }
    }

    // WZ 체어 변환(플립/회전) — C# 원본에는 없는 데이터 기반 확장.
    let transform = chair_info
        .as_ref()
        .map(|c| c.avatar_transform())
        .unwrap_or_default();
    apply_avatar_transform(&mut root, &mut parts, transform);

    if parts.is_empty() {
        return None;
    }

    // `Base/zmap.img` 가 있으면 그 순서를, 없으면 표준 MapleStory 순서를 쓴다.
    let mut zmap_order: Vec<String> = load_zmap_order(Some(data_root))
        .unwrap_or_else(|| STANDARD_ZMAP.iter().map(|s| s.to_string()).collect());
    for p in &parts {
        let zt = part_z_tag(&p.slot);
        if !zmap_order.contains(&zt) {
            zmap_order.push(zt);
        }
    }
    let zmap = std::sync::Arc::new(ZMap::new(zmap_order));
    let scene = AvatarScene::new(zmap.clone());

    Some(LoadedAvatar {
        parts,
        root_bone: root,
        zmap,
        scene,
        effect_frames,
        effect_delays,
        effect_origins,
        effect_z_indices,
        chair_info,
        transform,
        chair_frames,
        chair_delays,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 실제 게임 데이터가 있는 환경에서만 도는 테스트용 루트.
    ///
    /// `WZ_DATA_ROOT`(예: `.../Nexon/Maple/Data`)를 설정한 경우에만 테스트가
    /// 실제 파일을 읽고, 없으면 조용히 skip 한다. 소스에 사용자 경로를 넣지 않는다.
    fn wz_data_root() -> Option<std::path::PathBuf> {
        std::env::var_os("WZ_DATA_ROOT")
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists())
    }

    #[test]
    fn loader_returns_parts_on_real_character_wz() {
        let Some(root) = wz_data_root() else { return };
        let char_wz = root.join("Character").join("Character.wz");
        if !char_wz.exists() {
            return;
        }
        let av = load_from_character_wz(&char_wz)
            .expect("loader should return Some on real Character.wz");
        assert!(!av.parts.is_empty(), "at least one part loaded");
        for p in &av.parts {
            assert!(p.width > 0 && p.height > 0);
            assert_eq!(p.rgba.len(), (p.width * p.height * 4) as usize);
        }
        let batch = av.scene.build_batch(&av.root_bone);
        assert!(batch.layer_count > 0);
        assert!(batch.vertex_count() > 0);

        // z-tag 검증: body/head/hair/arm 중 하나 이상 있어야 한다
        // (map 이 없는 스킨은 root bone 에 직접 붙을 수 있으므로 root 도 포함한다.)
        let z_tags: Vec<&str> = std::iter::once(&av.root_bone)
            .chain(av.root_bone.children.iter())
            .flat_map(|b| b.skins.iter().map(|s| s.z.as_str()))
            .collect();
        assert!(!z_tags.is_empty(), "at least one z-tag assigned");
    }

    #[test]
    fn z_tag_mapping() {
        assert_eq!(part_z_tag("body"), "body");
        assert_eq!(part_z_tag("head"), "head");
        assert_eq!(part_z_tag("hair"), "hair");
        assert_eq!(part_z_tag("arm"), "arm");
        assert_eq!(part_z_tag("lHand"), "arm");
        assert_eq!(part_z_tag("rHand"), "arm");
    }

    #[test]
    fn load_avatar_with_equipment_custom_ids() {
        let Some(data_root) = wz_data_root() else {
            return;
        };
        // Load with specific equipment IDs
        let equipment = vec![
            ("Cap".to_string(), 1000002),
            ("Coat".to_string(), 1040002),
            ("Pants".to_string(), 1060000),
            ("Shoes".to_string(), 1070000),
            ("Glove".to_string(), 1080000),
            ("Cape".to_string(), 1102000),
        ];
        let av = load_avatar_with_equipment(&data_root, 2000, &equipment);
        if let Some(av) = av {
            // Should have body + head + 6 equipment = at least 8 parts
            assert!(
                av.parts.len() >= 8,
                "expected >= 8 parts, got {}",
                av.parts.len()
            );
            let batch = av.scene.build_batch(&av.root_bone);
            assert!(batch.layer_count > 0);
            assert!(batch.vertex_count() > 0);
        }
    }

    #[test]
    fn load_avatar_with_equipment_works() {
        let Some(data_root) = wz_data_root() else {
            return;
        };
        let equipment = vec![("Cap".to_string(), 1000000), ("Coat".to_string(), 1040000)];
        let av = load_avatar_with_equipment(&data_root, 2000, &equipment);
        if let Some(av) = av {
            assert!(!av.parts.is_empty(), "at least one part loaded");
            // body + head + cap + coat
            assert!(
                av.parts.len() >= 4,
                "expected >= 4 parts, got {}",
                av.parts.len()
            );
            let batch = av.scene.build_batch(&av.root_bone);
            assert!(batch.layer_count > 0);
            assert!(av.parts.iter().any(|p| p.slot == "cap"), "cap missing");
            assert!(av.parts.iter().any(|p| p.slot == "coat"), "coat missing");
        }
    }

    #[test]
    fn hair_mix_color_changes_hair_pixels() {
        let Some(data_root) = wz_data_root() else {
            return;
        };
        let equipment = vec![("Hair".to_string(), 30000)];
        let base = load_avatar_with_equipment_and_mix(
            &data_root,
            2000,
            &equipment,
            &MixOptions::default(),
        );
        let mixed = load_avatar_with_equipment_and_mix(
            &data_root,
            2000,
            &equipment,
            &MixOptions {
                hair: Some((1, 100)),
                face: None,
                ..Default::default()
            },
        );
        let (Some(base), Some(mixed)) = (base, mixed) else {
            return;
        };
        let Some(base_hair) = base.parts.iter().find(|p| p.slot == "hair") else {
            return;
        };
        let Some(mixed_hair) = mixed.parts.iter().find(|p| p.slot == "hair") else {
            return;
        };
        assert_ne!(
            base_hair.rgba, mixed_hair.rgba,
            "mix color should change hair pixels"
        );
    }

    #[test]
    fn load_equipment_item_works() {
        let Some(data_root) = wz_data_root() else {
            return;
        };
        // Cap item 1000000
        let cap_path = data_root.join("Cap").join("_Canvas").join("_Canvas_000.wz");
        if cap_path.exists() {
            let result = load_equipment_item(&cap_path, 1000000);
            assert!(result.is_some(), "cap item should load");
            let (slot, w, h, rgba) = result.unwrap();
            assert_eq!(slot, "Cap");
            assert!(w > 0 && h > 0);
            assert_eq!(rgba.len(), (w * h * 4) as usize);
        }
        // Coat item 1040000
        let coat_path = data_root
            .join("Coat")
            .join("_Canvas")
            .join("_Canvas_000.wz");
        if coat_path.exists() {
            let result = load_equipment_item(&coat_path, 1040000);
            assert!(result.is_some(), "coat item should load");
            let (slot, w, h, _rgba) = result.unwrap();
            assert_eq!(slot, "Coat");
            assert!(w > 0 && h > 0);
        }
    }

    #[test]
    fn loader_loads_expected_parts() {
        let Some(root) = wz_data_root() else { return };
        let char_wz = root.join("Character").join("Character.wz");
        if !char_wz.exists() {
            return;
        }
        let av = load_from_character_wz(&char_wz)
            .expect("loader should return Some on real Character.wz");
        // 최소 8 파트 (body, arm, head, ear variants, cap)
        assert!(
            av.parts.len() >= 14,
            "expected >= 14 parts, got {}",
            av.parts.len()
        );
        // body 파트가 있어야 한다
        assert!(
            av.parts.iter().any(|p| p.slot == "body"),
            "body part missing"
        );
        // head 파트가 있어야 한다
        assert!(
            av.parts.iter().any(|p| p.slot == "head"),
            "head part missing"
        );
        // cap 파트가 있어야 한다
        assert!(av.parts.iter().any(|p| p.slot == "cap"), "cap part missing");
        // zmap 이 표준 순서를 포함해야 한다
        assert!(av.zmap.index_of("body").is_some(), "zmap missing body");
        assert!(av.zmap.index_of("head").is_some(), "zmap missing head");
        // batch 렌더링이 동작해야 한다
        let batch = av.scene.build_batch(&av.root_bone);
        assert!(
            batch.layer_count >= av.parts.len(),
            "layer_count < parts count"
        );
        assert!(batch.vertex_count() > 0, "no vertices generated");
    }
}

#[cfg(test)]
mod prism_tests {
    use super::*;

    #[test]
    fn avatar_part_applies_caller_supplied_prism() {
        let mut part = AvatarPart {
            slot: "cap".into(),
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 255],
            ..Default::default()
        };
        let prism = crate::PrismData::new(0, 120, 100, 100);
        part.apply_prism(&prism);
        assert_ne!(part.rgba, vec![255, 0, 0, 255]);
    }
}

#[cfg(test)]
mod custom_origin_tests {
    use super::*;
    use std::sync::Arc;
    use wz_core::image::ImageValue;

    #[test]
    fn parses_custom_origin_id_map() {
        let mut root = ImageNode {
            name: Arc::from("01000000.img"),
            value: None,
            children: Vec::new(),
        };
        let mut info = ImageNode {
            name: Arc::from("info"),
            value: None,
            children: Vec::new(),
        };
        let custom = ImageNode {
            name: Arc::from("customOrigin"),
            value: None,
            children: vec![
                ImageNode {
                    name: Arc::from("cap"),
                    value: Some(ImageValue::Str(Arc::from("capType"))),
                    children: Vec::new(),
                },
                ImageNode {
                    name: Arc::from("ear"),
                    value: Some(ImageValue::Str(Arc::from("earType"))),
                    children: Vec::new(),
                },
            ],
        };
        info.children.push(custom);
        root.children.push(info);

        let map = parse_custom_origin(&root);
        assert_eq!(map.get("capType").map(String::as_str), Some("cap"));
        assert_eq!(map.get("earType").map(String::as_str), Some("ear"));
    }

    #[test]
    fn custom_origin_overrides_skin_origin() {
        let mut root = Bone::new("@root");
        let skin = SkinLayer::new("default");
        root.append_bone(skin, "navel", crate::Point::ZERO);
        let mut map = std::collections::BTreeMap::new();
        map.insert("default".to_string(), crate::Point::new(7, 9));
        apply_custom_origins(&mut root, &map);
        let found = root
            .children
            .iter()
            .flat_map(|b| b.skins.iter())
            .find(|s| s.name == "default")
            .expect("skin");
        assert_eq!(found.origin, crate::Point::new(7, 9));
    }
}

#[cfg(test)]
mod special_slot_tests {
    use super::*;

    fn data_root() -> Option<std::path::PathBuf> {
        std::env::var_os("WZ_DATA_ROOT")
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists())
    }

    #[test]
    fn taming_slot_loads() {
        let Some(data_root) = data_root() else {
            return;
        };
        let equipment = vec![("Taming".to_string(), 1902000)];
        if let Some(av) = load_avatar_with_equipment(&data_root, 2000, &equipment) {
            assert!(
                av.parts.iter().any(|p| p.slot == "tamingmob"),
                "taming part missing"
            );
        }
    }

    #[test]
    fn effect_slot_loads_first_frame() {
        let Some(data_root) = data_root() else {
            return;
        };
        let equipment = vec![("Effect".to_string(), 1000070)];
        if let Some(av) = load_avatar_with_equipment(&data_root, 2000, &equipment) {
            assert!(
                av.parts.iter().any(|p| p.slot == "effect"),
                "effect part missing"
            );
        }
    }

    #[test]
    fn chair_slot_loads() {
        let Some(data_root) = data_root() else {
            return;
        };
        let equipment = vec![("Chair".to_string(), 3010000)];
        if let Some(av) = load_avatar_with_equipment(&data_root, 2000, &equipment) {
            assert!(
                av.parts.iter().any(|p| p.slot == "chair"),
                "chair part missing"
            );
        }
    }

    #[test]
    fn group_chair_loads_taming_members() {
        let Some(data_root) = data_root() else {
            return;
        };
        let equipment = vec![("Chair".to_string(), 3016200)];
        if let Some(av) = load_avatar_with_equipment(&data_root, 2000, &equipment) {
            let info = av.chair_info.as_ref().expect("chair info");
            assert!(info.group_count >= 2, "group chair count");
            assert!(
                info.group_taming_ids.iter().any(|id| *id != 0),
                "group taming ids"
            );
            assert!(av.parts.iter().any(|p| p.slot == "chair"), "chair missing");
            assert!(
                av.parts.iter().any(|p| p.slot.starts_with("tamingmob")),
                "group taming parts missing"
            );
        }
    }

    #[test]
    fn effect_frame_index_changes_pixels() {
        let Some(data_root) = data_root() else {
            return;
        };
        let equipment = vec![("Effect".to_string(), 1000070)];
        let base = load_avatar_with_equipment_and_mix(
            &data_root,
            2000,
            &equipment,
            &MixOptions::default(),
        );
        let next = load_avatar_with_equipment_and_mix(
            &data_root,
            2000,
            &equipment,
            &MixOptions {
                effect_frame: 1,
                ..Default::default()
            },
        );
        let (Some(base), Some(next)) = (base, next) else {
            return;
        };
        let (Some(a), Some(b)) = (
            base.parts.iter().find(|p| p.slot == "effect"),
            next.parts.iter().find(|p| p.slot == "effect"),
        ) else {
            return;
        };
        assert_ne!(a.rgba, b.rgba, "effect frame 0 and 1 should differ");
        assert!(base.effect_frames.len() >= 2, "effect frames missing");
        assert_eq!(base.effect_frames.len(), base.effect_delays.len());
        assert!(
            base.effect_delays.iter().any(|d| *d > 0),
            "effect delays missing"
        );
    }
}

#[cfg(test)]
mod custom_origin_map_tests {
    use super::*;

    #[test]
    fn custom_origin_map_uses_child_name_then_selected_point() {
        let mut root = Bone::new("@root");
        root.append_bone(SkinLayer::new("default"), "navel", crate::Point::ZERO);

        let mut part = AvatarPart::default();
        // C# `CustomOriginMap`: originValue ("default") → childName ("cap").
        part.custom_origin_map
            .insert("default".to_string(), "cap".to_string());

        let mut selected = std::collections::BTreeMap::new();
        selected.insert("cap".to_string(), crate::Point::new(7, 9));
        apply_custom_origin_map(&mut root, &[part], &selected);

        let found = root
            .children
            .iter()
            .flat_map(|b| b.skins.iter())
            .find(|s| s.name == "default")
            .expect("skin");
        assert_eq!(found.origin, crate::Point::new(7, 9));
    }
}

#[cfg(test)]
mod scale_tests {
    use super::*;

    #[test]
    fn avatar_scale_resizes_matching_parts_and_bones() {
        let mut root = Bone::new("@root");
        let mut skin = SkinLayer::new("body");
        skin.origin = crate::Point::new(2, 4);
        skin.width = 2;
        skin.height = 2;
        root.append_bone(skin, "navel", crate::Point::ZERO);

        let mut parts = vec![AvatarPart {
            slot: "body".into(),
            width: 2,
            height: 2,
            rgba: vec![255; 16],
            ..Default::default()
        }];
        apply_avatar_scale(&mut root, &mut parts, 150);

        assert_eq!(parts[0].width, 3);
        assert_eq!(parts[0].height, 3);
        assert_eq!(parts[0].rgba.len(), 36);
        let skin = root
            .children
            .iter()
            .flat_map(|b| b.skins.iter())
            .find(|s| s.name == "body")
            .expect("body skin");
        assert_eq!(skin.width, 3);
        assert_eq!(skin.origin, crate::Point::new(3, 6));
    }
}

#[cfg(test)]
mod effect_origin_tests {
    use super::*;

    #[test]
    fn set_effect_skin_updates_origin_and_z() {
        let mut root = Bone::new("@root");
        root.append_bone(SkinLayer::new("effect"), "effect", crate::Point::ZERO);
        assert!(set_effect_skin(&mut root, crate::Point::new(3, 4), 2));
        let skin = root
            .children
            .iter()
            .flat_map(|b| b.skins.iter())
            .find(|s| s.name == "effect")
            .expect("effect skin");
        assert_eq!(skin.origin, crate::Point::new(3, 4));
        assert_eq!(skin.z_index, 2);
    }
}

#[cfg(test)]
mod custom_chair_type_tests {
    use super::*;

    #[test]
    fn parses_known_and_unknown_types() {
        assert_eq!(
            CustomChairType::parse("scaleavatarchair"),
            CustomChairType::ScaleAvatarChair
        );
        assert_eq!(CustomChairType::parse(""), CustomChairType::Unsupported);
        assert!(matches!(
            CustomChairType::parse("FutureChair"),
            CustomChairType::Other(v) if v == "FutureChair"
        ));
    }

    #[test]
    fn effective_scale_only_uses_scale_avatar_for_scale_type() {
        let mut info = ChairInfo {
            scale: 110,
            custom_chair: Some(CustomChairInfo {
                chair_type: CustomChairType::ScaleAvatarChair,
                scale_avatar: Some(80),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(info.effective_scale(), 80);
        info.custom_chair = Some(CustomChairInfo {
            chair_type: CustomChairType::Other("FutureChair".into()),
            scale_avatar: Some(80),
            ..Default::default()
        });
        assert_eq!(info.effective_scale(), 100);
    }

    #[test]
    fn parse_custom_chair_preserves_unknown_fields() {
        use wz_core::image::{ImageValue, Node};

        let node = Node {
            name: "customChair".into(),
            value: None,
            children: vec![
                Node {
                    name: "type".into(),
                    value: Some(ImageValue::Str("FutureChair".into())),
                    children: vec![],
                },
                Node {
                    name: "scaleAvatar".into(),
                    value: None,
                    children: vec![Node {
                        name: "scale".into(),
                        value: Some(ImageValue::Int(130)),
                        children: vec![],
                    }],
                },
                Node {
                    name: "newFlag".into(),
                    value: Some(ImageValue::Int(7)),
                    children: vec![],
                },
            ],
        };

        let info = parse_custom_chair(&node);
        assert_eq!(
            info.chair_type,
            CustomChairType::Other("FutureChair".into())
        );
        assert_eq!(info.scale_avatar, Some(130));
        assert_eq!(info.extra.get("newFlag").map(String::as_str), Some("7"));
    }
}

#[cfg(test)]
mod chair_pose_tests {
    use super::*;

    fn data_root() -> Option<std::path::PathBuf> {
        std::env::var_os("WZ_DATA_ROOT")
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists())
    }

    #[test]
    fn chair_switches_body_to_sit_pose() {
        let Some(data_root) = data_root() else {
            return;
        };
        let equipment = vec![("Chair".to_string(), 3016200)];
        let with_chair = load_avatar_with_equipment(&data_root, 2000, &equipment);
        let without_chair = load_avatar_with_equipment(&data_root, 2000, &[]);
        let (Some(with_chair), Some(without_chair)) = (with_chair, without_chair) else {
            return;
        };
        let (Some(sit), Some(stand)) = (
            with_chair.parts.iter().find(|p| p.slot == "body"),
            without_chair.parts.iter().find(|p| p.slot == "body"),
        ) else {
            return;
        };
        assert_ne!(
            sit.rgba, stand.rgba,
            "chair should switch the body to the sit action"
        );
    }
}

#[cfg(test)]
mod chair_custom_type_tests {
    use super::*;

    fn data_root() -> Option<std::path::PathBuf> {
        std::env::var_os("WZ_DATA_ROOT")
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists())
    }

    fn load_chair(id: i32) -> Option<LoadedAvatar> {
        let root = data_root()?;
        load_avatar_with_equipment(&root, 2000, &[("Chair".to_string(), id)])
    }

    fn chair_info(av: &LoadedAvatar) -> &ChairInfo {
        av.chair_info.as_ref().expect("chair info")
    }

    fn has_slot(av: &LoadedAvatar, slot: &str) -> bool {
        av.parts.iter().any(|p| p.slot == slot)
    }

    #[test]
    fn scale_avatar_chair_uses_scale_avatar_and_hides_body() {
        let Some(av) = load_chair(3018429) else {
            return;
        };
        let info = chair_info(&av);
        assert_eq!(
            info.custom_chair.as_ref().unwrap().chair_type,
            CustomChairType::ScaleAvatarChair
        );
        assert_eq!(info.effective_scale(), 300);
        // C# `SetChairDefault`: `removeBody != 0` 이 `ScaleAvatarChair` 보다 먼저다.
        assert!(!has_slot(&av, "body"), "removeBody chair must hide body");
        assert!(has_slot(&av, "chair"));
    }

    #[test]
    fn rotated_sleeping_bag_chair_hides_body() {
        let Some(av) = load_chair(3018018) else {
            return;
        };
        let info = chair_info(&av);
        assert_eq!(info.sit_action.as_deref(), Some("hide"));
        assert!(!has_slot(&av, "body"));
        let rotate = info.custom_chair.as_ref().unwrap().rotate.as_ref().unwrap();
        assert_eq!(rotate.rotate, 90);
        assert_eq!(rotate.avatar_pos, crate::Point::new(-26, -23));
        assert!(rotate.hide_weapon);
        assert!(rotate.hide_nick);
        assert!(info.hides_weapon());
        assert_eq!(av.transform.rotate, 90);
        assert_eq!(av.transform.offset, crate::Point::new(-26, -23));
    }

    #[test]
    fn dummy_avatar_chair_parses_entries_and_loads_self_taming() {
        let Some(av) = load_chair(3018853) else {
            return;
        };
        let info = chair_info(&av);
        let custom = info.custom_chair.as_ref().unwrap();
        let dummy = custom.dummy.as_ref().unwrap();
        assert_eq!(dummy.avatar_count, 4);
        assert_eq!(dummy.avatars.len(), 4);
        assert_eq!(dummy.self_entry.as_ref().unwrap().taming_mob, 1983797);
        assert_eq!(info.taming_mob, Some(1983797));
        assert_eq!(info.taming.as_ref().unwrap().id, 1983797);
        assert!(!has_slot(&av, "body"));
        assert!(has_slot(&av, "chair"));
        assert!(av.parts.iter().any(|p| p.slot.starts_with("tamingmob")));
        for i in 0..4 {
            assert!(
                av.parts
                    .iter()
                    .any(|p| p.slot.starts_with(&format!("dummy{i}_"))),
                "dummy {i} parts missing"
            );
        }
    }

    #[test]
    fn flip_xy_chair_parses_flip_and_keeps_sit_body() {
        let Some(av) = load_chair(3019313) else {
            return;
        };
        let info = chair_info(&av);
        let flip = info.custom_chair.as_ref().unwrap().flip.as_ref().unwrap();
        assert!(flip.flip_horizontal);
        assert!(flip.flip_vertical);
        assert_eq!(flip.avatar_pos, crate::Point::new(-17, -111));
        assert_eq!(flip.avatar_z, 0);
        assert!(flip.invisible_weapon);
        assert!(info.hides_weapon());
        assert!(av.transform.flip_x);
        assert!(av.transform.flip_y);
        assert_eq!(av.transform.offset, crate::Point::new(-17, -111));
        // flipXYChair 는 removeBody/sitAction 이 없으므로 C# 은 sit 을 쓴다.
        assert!(has_slot(&av, "body"));
        assert!(has_slot(&av, "chair"));
    }

    fn load_chair_with(id: i32, chair: ChairUserData) -> Option<LoadedAvatar> {
        let root = data_root()?;
        load_avatar_with_equipment_and_mix(
            &root,
            2000,
            &[("Chair".to_string(), id)],
            &MixOptions {
                chair,
                ..Default::default()
            },
        )
    }

    #[test]
    fn pop_chair_renders_value_text() {
        let Some(av) = load_chair_with(
            3015603,
            ChairUserData {
                pop: Some(1234),
                ..Default::default()
            },
        ) else {
            return;
        };
        let info = chair_info(&av);
        assert!(info
            .custom_chair
            .as_ref()
            .and_then(|c| c.pop.as_ref())
            .is_some());
        let part = av
            .parts
            .iter()
            .find(|p| p.slot == "pop_text")
            .expect("pop_text part");
        assert!(part.width > 1 && part.height > 1);
        assert!(part.rgba.chunks(4).any(|p| p[3] != 0));
    }

    #[test]
    fn time_chair_renders_time_text() {
        let Some(av) = load_chair_with(
            3018442,
            ChairUserData {
                time_minutes: Some(90),
                ..Default::default()
            },
        ) else {
            return;
        };
        let info = chair_info(&av);
        assert!(info
            .custom_chair
            .as_ref()
            .and_then(|c| c.time.as_ref())
            .is_some());
        let part = av
            .parts
            .iter()
            .find(|p| p.slot == "time_text")
            .expect("time_text part");
        assert!(part.width > 1 && part.height > 1);
        assert!(part.rgba.chunks(4).any(|p| p[3] != 0));
    }

    #[test]
    fn identity_chair_renders_level_digits() {
        let Some(av) = load_chair_with(
            3018108,
            ChairUserData {
                level: Some(200),
                ..Default::default()
            },
        ) else {
            return;
        };
        let info = chair_info(&av);
        let identity = info
            .custom_chair
            .as_ref()
            .and_then(|c| c.identity.as_ref())
            .expect("identity info");
        assert_eq!(identity.digits, 10);
        for i in 0..3 {
            assert!(
                av.parts
                    .iter()
                    .any(|p| p.slot == format!("identity_digit_{i}")),
                "digit {i} missing"
            );
        }
        let digit0 = av
            .parts
            .iter()
            .find(|p| p.slot == "identity_digit_0")
            .unwrap();
        let digit1 = av
            .parts
            .iter()
            .find(|p| p.slot == "identity_digit_1")
            .unwrap();
        let digit2 = av
            .parts
            .iter()
            .find(|p| p.slot == "identity_digit_2")
            .unwrap();
        assert_eq!(digit0.position, crate::Point::new(58, 43));
        assert_eq!(digit1.position.x, 58 + digit0.width as i32);
        assert_eq!(
            digit2.position.x,
            58 + digit0.width as i32 + digit1.width as i32
        );
    }

    #[test]
    fn score_chair_selects_group_by_score() {
        let Some(av) = load_chair_with(
            3018256,
            ChairUserData {
                score: Some(2500),
                ..Default::default()
            },
        ) else {
            return;
        };
        let info = chair_info(&av);
        let score = info
            .custom_chair
            .as_ref()
            .and_then(|c| c.score.as_ref())
            .expect("score info");
        assert_eq!(score.groups.len(), 6);
        assert_eq!(score.groups[1].min, 2000);
        assert_eq!(score.groups[1].max, Some(3999));
        assert_eq!(av.chair_frames.len(), 8);
        assert_eq!(av.chair_delays.len(), 8);
        assert!(av.parts.iter().any(|p| p.slot == "score_effect"));
    }

    #[test]
    fn part_animation_chairs_load_frames() {
        for id in [3018467, 3018634] {
            let Some(av) = load_chair(id) else {
                continue;
            };
            let info = chair_info(&av);
            let anim = info
                .custom_chair
                .as_ref()
                .and_then(|c| c.part_animation.as_ref())
                .unwrap_or_else(|| panic!("part animation missing for {id}"));
            assert!(anim.parts_count > 0, "id {id}");
            assert!(anim.frame_count > 1, "id {id}");
            assert_eq!(av.chair_frames.len(), anim.frame_count, "id {id}");
            assert_eq!(av.chair_delays.len(), anim.frame_count, "id {id}");
            assert!(av.parts.iter().any(|p| p.slot.starts_with("rolling_part")));
        }
    }

    #[test]
    fn hash_tag_chair_forces_first_action() {
        let Some(av) = load_chair(3018079) else {
            return;
        };
        let info = chair_info(&av);
        let tag = info
            .custom_chair
            .as_ref()
            .and_then(|c| c.hash_tag.as_ref())
            .expect("hash tag info");
        assert!(!tag.actions.is_empty());
        assert_eq!(tag.actions[0].action, "stand1");
        assert_eq!(tag.actions[0].frame, Some(2));
        assert_eq!(tag.actions[0].pos, crate::Point::new(27, -105));
        assert_eq!(
            info.body_action(),
            ChairBodyAction::Action {
                name: "stand1".into(),
                frame: Some(2)
            }
        );
        assert!(has_slot(&av, "body"));
    }

    #[test]
    fn mirror_chair_parses_position_and_hides_weapon() {
        let Some(av) = load_chair(3019098) else {
            return;
        };
        let info = chair_info(&av);
        let mirror = info
            .custom_chair
            .as_ref()
            .and_then(|c| c.mirror.as_ref())
            .expect("mirror info");
        assert_eq!(mirror.avatar_pos_y, -28);
        assert!(mirror.invisible_weapon);
        assert!(info.hides_weapon());
        assert!(has_slot(&av, "body"));
    }

    #[test]
    fn android_chair_loads_custom_effect_frames() {
        let Some(av) = load_chair(3018020) else {
            return;
        };
        let info = chair_info(&av);
        let android = info
            .custom_chair
            .as_ref()
            .and_then(|c| c.android.as_ref())
            .expect("android info");
        assert_eq!(android.effect_frames, 16);
        assert!(android.hide_body);
        assert_eq!(android.forced_action.as_deref(), Some("sit"));
        assert_eq!(android.pos, crate::Point::new(-121, 6));
        assert!(android.reverse_flip);
        assert_eq!(av.chair_frames.len(), 16);
        assert_eq!(av.chair_delays, vec![100; 16]);
        assert!(!has_slot(&av, "body"));
        let part = av
            .parts
            .iter()
            .find(|p| p.slot == "android_effect")
            .expect("android_effect part");
        assert!(part.width > 1 && part.height > 1);
    }

    #[test]
    fn pet_chair_parses_slots_and_loads_taming() {
        let Some(av) = load_chair(3018179) else {
            return;
        };
        let info = chair_info(&av);
        let pet = info
            .custom_chair
            .as_ref()
            .and_then(|c| c.pet.as_ref())
            .expect("pet info");
        assert_eq!(pet.slots.len(), 3);
        assert_eq!(pet.slots[0].pos, crate::Point::new(-108, 0));
        assert!(pet.slots[0].reverse_flip);
        assert_eq!(pet.slots[0].wonder_grade, 6);
        assert_eq!(pet.slots[1].pos, crate::Point::new(-6, 50));
        assert_eq!(pet.slots[2].pos, crate::Point::new(18, -149));
        assert_eq!(info.taming_mob, Some(1983457));
        assert!(av.parts.iter().any(|p| p.slot.starts_with("tamingmob")));
    }

    #[test]
    fn rolling_house_chair_loads_multilayer_parts() {
        let Some(av) = load_chair(3018010) else {
            return;
        };
        let info = chair_info(&av);
        let rolling = info
            .custom_chair
            .as_ref()
            .and_then(|c| c.rolling_house.as_ref())
            .expect("rolling house info");
        assert_eq!(rolling.parts_count, 8);
        assert_eq!(rolling.part_z.len(), 8);
        assert_eq!(rolling.pos_lv.len(), 10);
        assert!(rolling.frame_count >= 1);
        assert_eq!(info.custom_chair.as_ref().unwrap().self_taming_mob, None);
        assert_eq!(av.chair_frames.len(), rolling.frame_count);
        assert_eq!(av.chair_delays.len(), rolling.frame_count);
        assert!(av.chair_frames.iter().all(|f| !f.is_empty()));

        for i in 0..8 {
            let part = av
                .parts
                .iter()
                .find(|p| p.slot == format!("rolling_part{i}"))
                .unwrap_or_else(|| panic!("rolling_part{i} missing"));
            assert!(part.width > 1 && part.height > 1, "part{i} is empty");
        }
        assert!(!has_slot(&av, "chair"), "base 1x1 chair should be replaced");
    }

    #[test]
    fn event_point_chair_hides_body_and_loads_taming() {
        let Some(av) = load_chair(3018093) else {
            return;
        };
        let info = chair_info(&av);
        let event = info
            .custom_chair
            .as_ref()
            .and_then(|c| c.event_point.as_ref())
            .expect("event point info");
        assert_eq!((event.index, event.point, event.term), (1, 3, 10000));
        assert_eq!(info.taming_mob, Some(1983422));
        assert!(!has_slot(&av, "body"));
        assert!(has_slot(&av, "chair"));
        assert!(av.parts.iter().any(|p| p.slot.starts_with("tamingmob")));
    }

    #[test]
    fn body_action_follows_csharp_precedence() {
        let mut info = ChairInfo {
            custom_chair: Some(CustomChairInfo {
                chair_type: CustomChairType::ScaleAvatarChair,
                scale_avatar: Some(150),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            info.body_action(),
            ChairBodyAction::Action {
                name: "stand1".into(),
                frame: None
            }
        );

        info.remove_body = true;
        assert_eq!(info.body_action(), ChairBodyAction::Hide);

        info.sit_action = Some("hide".into());
        assert_eq!(info.body_action(), ChairBodyAction::Hide);

        info.sit_action = Some("sit".into());
        sanity_check_action(&info.body_action(), "sit");

        info.sit_action = None;
        info.remove_body = false;
        info.taming = Some(TamingInfo {
            id: 1,
            scale: 100,
            remove_body: true,
            flip: false,
        });
        assert_eq!(info.body_action(), ChairBodyAction::Hide);

        info.taming = None;
        info.custom_chair = None;
        sanity_check_action(&info.body_action(), "sit");
    }

    fn sanity_check_action(action: &ChairBodyAction, expected: &str) {
        match action {
            ChairBodyAction::Action { name, .. } => assert_eq!(name, expected),
            other => panic!("expected action {expected}, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod avatar_transform_tests {
    use super::*;

    fn part_2x1() -> AvatarPart {
        AvatarPart {
            slot: "test".into(),
            width: 2,
            height: 1,
            rgba: vec![10, 0, 0, 255, 20, 0, 0, 255],
            position: crate::Point::new(5, 7),
            origin: crate::Point::new(0, 0),
            z_index: 0,
            ..Default::default()
        }
    }

    #[test]
    fn horizontal_flip_mirrors_pixels_and_position() {
        let mut root = Bone::new("@root");
        let mut parts = vec![part_2x1()];
        apply_avatar_transform(
            &mut root,
            &mut parts,
            AvatarTransform {
                flip_x: true,
                ..Default::default()
            },
        );
        assert_eq!(parts[0].rgba[0..4], [20, 0, 0, 255]);
        assert_eq!(parts[0].rgba[4..8], [10, 0, 0, 255]);
        assert_eq!(parts[0].position, crate::Point::new(-7, 7));
        assert_eq!(parts[0].origin, crate::Point::new(2, 0));
    }

    #[test]
    fn rotate_90_swaps_dimensions_and_position() {
        let mut root = Bone::new("@root");
        let mut parts = vec![part_2x1()];
        apply_avatar_transform(
            &mut root,
            &mut parts,
            AvatarTransform {
                rotate: 90,
                ..Default::default()
            },
        );
        assert_eq!((parts[0].width, parts[0].height), (1, 2));
        assert_eq!(parts[0].position, crate::Point::new(-8, 5));
        assert_eq!(parts[0].origin, crate::Point::new(1, 0));
        assert_eq!(parts[0].rgba[0..4], [10, 0, 0, 255]);
        assert_eq!(parts[0].rgba[4..8], [20, 0, 0, 255]);
    }

    #[test]
    fn transform_offset_is_applied_to_parts_and_root() {
        let mut root = Bone::new("@root");
        root.position = crate::Point::new(1, 2);
        let mut parts = vec![part_2x1()];
        apply_avatar_transform(
            &mut root,
            &mut parts,
            AvatarTransform {
                offset: crate::Point::new(10, -3),
                ..Default::default()
            },
        );
        assert_eq!(parts[0].position, crate::Point::new(15, 4));
        assert_eq!(root.position, crate::Point::new(11, -1));
    }

    #[test]
    fn rotate_bone_swaps_skin_dimensions_and_origin() {
        let mut root = Bone::new("@root");
        let mut skin = SkinLayer::new("body");
        skin.width = 2;
        skin.height = 1;
        skin.origin = crate::Point::new(0, 0);
        root.append_bone(skin, "body", crate::Point::new(5, 7));
        let mut parts = Vec::new();
        apply_avatar_transform(
            &mut root,
            &mut parts,
            AvatarTransform {
                rotate: 90,
                ..Default::default()
            },
        );
        let skin = &root.children[0].skins[0];
        assert_eq!((skin.width, skin.height), (1, 2));
        assert_eq!(skin.origin, crate::Point::new(1, 0));
        assert_eq!(root.children[0].position, crate::Point::new(-7, 5));
    }
}
