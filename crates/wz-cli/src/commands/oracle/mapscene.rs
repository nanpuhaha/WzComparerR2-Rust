use super::*;
pub fn cmd_oracle_mapscene(
    wz_path: &std::path::Path,
    map_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::text::sort_natural;
    use wz_core::WzArchive;
    use wz_db::MapData;

    let archive = WzArchive::open(wz_path)?;
    let tree = archive.read_tree()?;
    let detection = archive.detection();

    let clean = map_id.trim_end_matches(".img");
    let padded = if let Ok(num) = clean.parse::<u32>() {
        format!("{num:09}.img")
    } else {
        format!("{clean}.img")
    };
    let raw_name = format!("{clean}.img");

    let entry = find_image_entry_in_dirtree(&tree, &padded)
        .or_else(|| find_image_entry_in_dirtree(&tree, &raw_name))
        .or_else(|| {
            // Map0/000010000.img 형식 등 서브트리 경로 매칭 시도
            if let Some(pos) = clean.rfind('/') {
                let sub = &clean[pos + 1..];
                find_image_entry_in_dirtree(&tree, &format!("{sub}.img"))
            } else {
                None
            }
        })
        .ok_or_else(|| format!("맵 이미지를 찾을 수 없습니다: {map_id}"))?;

    let off = entry.offset.ok_or("맵 이미지 오프셋이 없습니다")?;
    let img_root = archive.parse_image_at(off)?;
    let map_data = MapData::parse_from_node(&img_root);

    let mut doc = oracle_envelope("mapscene", detection);

    let mut res = serde_json::Map::new();

    // info
    let mut info_map = serde_json::Map::new();
    if let Some(id) = map_data.info.map_id {
        info_map.insert("map_id".into(), serde_json::Value::Number(id.into()));
    }
    if let Some(bgm) = &map_data.info.bgm {
        info_map.insert("bgm".into(), serde_json::Value::String(bgm.clone()));
    }
    if let Some(mark) = &map_data.info.map_mark {
        info_map.insert("map_mark".into(), serde_json::Value::String(mark.clone()));
    }
    if let Some(ret) = map_data.info.return_map {
        info_map.insert("return_map".into(), serde_json::Value::Number(ret.into()));
    }
    if let Some(f_ret) = map_data.info.forced_return {
        info_map.insert(
            "forced_return".into(),
            serde_json::Value::Number(f_ret.into()),
        );
    }
    info_map.insert("town".into(), serde_json::Value::Bool(map_data.info.town));
    info_map.insert("swim".into(), serde_json::Value::Bool(map_data.info.swim));
    info_map.insert("fly".into(), serde_json::Value::Bool(map_data.info.fly));
    if let Some(vr) = map_data.info.vrect {
        let mut vr_map = serde_json::Map::new();
        vr_map.insert("left".into(), serde_json::Value::Number(vr.left.into()));
        vr_map.insert("top".into(), serde_json::Value::Number(vr.top.into()));
        vr_map.insert("right".into(), serde_json::Value::Number(vr.right.into()));
        vr_map.insert("bottom".into(), serde_json::Value::Number(vr.bottom.into()));
        info_map.insert("vrect".into(), serde_json::Value::Object(vr_map));
    }
    res.insert("info".into(), serde_json::Value::Object(info_map));

    // footholds (ID 오름차순 정렬)
    let mut sorted_fhs = map_data.footholds.clone();
    sorted_fhs.sort_by_key(|f| f.id);
    let fh_items: Vec<serde_json::Value> = sorted_fhs
        .iter()
        .map(|f| {
            serde_json::json!({
                "id": f.id,
                "layer": f.layer,
                "group": f.group,
                "x1": f.x1,
                "y1": f.y1,
                "x2": f.x2,
                "y2": f.y2,
                "prev": f.prev,
                "next": f.next,
                "piece": f.piece,
                "is_wall": f.is_wall(),
                "is_flat": f.is_flat(),
            })
        })
        .collect();
    res.insert("footholds".into(), serde_json::Value::Array(fh_items));

    // portals (포탈 이름 natural_cmp 및 ID 오름차순)
    let mut sorted_portals = map_data.portals.clone();
    sorted_portals.sort_by(
        |a, b| match wz_core::text::natural_cmp(&a.p_name, &b.p_name) {
            std::cmp::Ordering::Equal => a.id.cmp(&b.id),
            other => other,
        },
    );
    let portal_items: Vec<serde_json::Value> = sorted_portals
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "pn": p.p_name,
                "pt": p.p_type,
                "x": p.x,
                "y": p.y,
                "tm": p.to_map,
                "tn": p.to_name,
                "script": p.script,
            })
        })
        .collect();
    res.insert("portals".into(), serde_json::Value::Array(portal_items));

    // life (ID natural_cmp 정렬)
    let mut sorted_life = map_data.life.clone();
    sort_natural(&mut sorted_life, |l| &l.id);
    let life_items: Vec<serde_json::Value> = sorted_life
        .iter()
        .map(|l| {
            serde_json::json!({
                "id": l.id,
                "type": l.life_type,
                "x": l.x,
                "y": l.y,
                "cy": l.cy,
                "fh": l.fh,
                "flip": l.flip,
                "hide": l.hide,
            })
        })
        .collect();
    res.insert("life".into(), serde_json::Value::Array(life_items));

    doc.insert("result".into(), serde_json::Value::Object(res));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}
