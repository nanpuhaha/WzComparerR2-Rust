// wz-gui — 비동기 I/O 및 백그라운드 파싱.
//
// `docs/gui-architecture.md` §5: UI 스레드는 블로킹 I/O 를 하지 않는다.
// 워커 스레드가 결과를 채널로 보내고, UI 스레드는 `poll_load_result` 로
// 매 프레임 폴링한다.

use crate::state::AppState;
use std::path::{Path, PathBuf};
use wz_core::WzArchive;
use wz_db::{Arena, StringLinker, StringRoot, StringRoots};

// ── 파일 열기 ─────────────────────────────────────────────────────────

/// `Data/<dir>/<stem>` 후보 중 처음 존재하는 WZ 를 찾는다.
///
/// 현대 클라이언트는 `String_000.wz` 처럼 분할 파일을 쓰고, 구형은
/// `String.wz` 하나를 쓴다. 둘 다 시도한다.
fn find_wz(data_root: &Path, dir: &str, stems: &[&str]) -> Option<PathBuf> {
    let base = data_root.join(dir);
    stems
        .iter()
        .map(|stem| base.join(stem))
        .find(|p| p.exists())
}

fn load_arena(path: &Path) -> Result<Arena, String> {
    let archive = WzArchive::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    Ok(Arena::from_archive(&archive).0)
}

pub fn load_string_linker(state: &mut AppState) {
    let Some(data_root) = state.data_root.clone() else {
        state.string_linker_status = "data_root 가 설정되지 않았습니다".into();
        return;
    };

    let string_wz = match find_wz(&data_root, "String", &["String_000.wz", "String.wz"]) {
        Some(p) => p,
        None => {
            state.string_linker_status =
                format!("String WZ 없음: {}", data_root.join("String").display());
            return;
        }
    };
    let item_wz = find_wz(&data_root, "Item", &["Item_000.wz", "Item.wz"]);
    let etc_wz = find_wz(&data_root, "Etc", &["Etc_000.wz", "Etc.wz"]);
    let quest_wz = find_wz(&data_root, "Quest", &["Quest_000.wz", "Quest.wz"]);
    let reactor_wz = find_wz(&data_root, "Reactor", &["Reactor_000.wz", "Reactor.wz"]);

    let mut missing = Vec::new();
    if item_wz.is_none() {
        missing.push("Item");
    }
    if etc_wz.is_none() {
        missing.push("Etc");
    }
    if quest_wz.is_none() {
        missing.push("Quest");
    }
    if reactor_wz.is_none() {
        missing.push("Reactor");
    }

    let loaded = (|| -> Result<StringLinker, String> {
        let string_arena = load_arena(&string_wz)?;
        let item_arena = item_wz.as_deref().map(load_arena).transpose()?;
        let etc_arena = etc_wz.as_deref().map(load_arena).transpose()?;
        let quest_arena = quest_wz.as_deref().map(load_arena).transpose()?;
        let reactor_arena = reactor_wz.as_deref().map(load_arena).transpose()?;

        let roots = StringRoots {
            string: StringRoot {
                arena: &string_arena,
                node: string_arena.root(),
            },
            item: item_arena.as_ref().map(|a| StringRoot {
                arena: a,
                node: a.root(),
            }),
            etc: etc_arena.as_ref().map(|a| StringRoot {
                arena: a,
                node: a.root(),
            }),
            quest: quest_arena.as_ref().map(|a| StringRoot {
                arena: a,
                node: a.root(),
            }),
            reactor: reactor_arena.as_ref().map(|a| StringRoot {
                arena: a,
                node: a.root(),
            }),
        };

        let mut linker = StringLinker::new();
        linker.load_roots(&roots);
        Ok(linker)
    })();

    match loaded {
        Ok(linker) => {
            let count = linker.string_item.len()
                + linker.string_eqp.len()
                + linker.string_skill.len()
                + linker.string_map.len()
                + linker.string_mob.len()
                + linker.string_npc.len();
            state.string_linker_status = if missing.is_empty() {
                format!("StringLinker 로드 완료 ({count} entries, 5 roots)")
            } else {
                format!(
                    "StringLinker 로드 완료 ({count} entries) — 누락: {}",
                    missing.join(", ")
                )
            };
            state.string_linker = Some(linker);
        }
        Err(e) => {
            state.string_linker_status = format!("StringLinker 로드 실패: {e}");
        }
    }
}

// ── 아바타 장비 로딩 ─────────────────────────────────────────────────
