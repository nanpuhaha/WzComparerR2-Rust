use super::*;
pub fn cmd_oracle_tooltip(
    wz_path: &std::path::Path,
    item_id: i32,
    item_wz: Option<&std::path::Path>,
    etc_wz: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::WzArchive;
    use wz_db::{Arena, StringLinker, StringRoot, StringRoots};

    let archive = WzArchive::open(wz_path)?;
    let detection = archive.detection();
    let (arena, _) = Arena::from_archive(&archive);

    // `StringLinker.Load` 는 다섯 루트를 받는다. 아이템 툴팁에 필요한
    // Item.wz(Special/0910)와 Etc.wz(SetItemInfo/worldArchive)를 선택적으로
    // 채워, String.wz 만으로는 닿지 않던 이름/설명을 사용할 수 있게 한다.
    let item_arena = item_wz.map(load_arena).transpose()?;
    let etc_arena = etc_wz.map(load_arena).transpose()?;

    let roots = StringRoots {
        string: StringRoot {
            arena: &arena,
            node: arena.root(),
        },
        item: item_arena.as_ref().map(|a| StringRoot {
            arena: a,
            node: a.root(),
        }),
        etc: etc_arena.as_ref().map(|a| StringRoot {
            arena: a,
            node: a.root(),
        }),
        quest: None,
        reactor: None,
    };

    let mut linker = StringLinker::new();
    linker.load_roots(&roots);

    let text = linker.format_tooltip(item_id).unwrap_or_default();

    let mut doc = oracle_envelope("tooltip", detection);
    let mut res = serde_json::Map::new();
    res.insert("text".into(), serde_json::Value::String(text));
    doc.insert("result".into(), serde_json::Value::Object(res));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}

fn load_arena(path: &std::path::Path) -> Result<wz_db::Arena, wz_error::WzError> {
    let archive = wz_core::WzArchive::open(path)?;
    Ok(wz_db::Arena::from_archive(&archive).0)
}
