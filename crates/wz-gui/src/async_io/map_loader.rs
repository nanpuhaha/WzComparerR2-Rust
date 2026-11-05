// wz-gui — 비동기 I/O 및 백그라운드 파싱.
//
// `docs/gui-architecture.md` §5: UI 스레드는 블로킹 I/O 를 하지 않는다.
// 워커 스레드가 결과를 채널로 보내고, UI 스레드는 `poll_load_result` 로
// 매 프레임 폴링한다.

use super::*;
use crate::state::AppState;
use wz_db::MapData;

// ── 파일 열기 ─────────────────────────────────────────────────────────

fn reactor_origin(png: &wz_core::image::PngRef) -> (i32, i32) {
    let Some(sub) = &png.sub_property else {
        return (0, 0);
    };
    for child in &sub.children {
        if child.name.as_ref() == "origin" {
            if let Some(wz_core::image::ImageValue::Vector(v)) = &child.value {
                return (v.x, v.y);
            }
        }
    }
    (0, 0)
}

pub fn load_skill_view(
    root: &std::path::Path,
    id: i32,
    ani_path: &str,
) -> Option<crate::skill::SkillView> {
    let image_name = format!("{}.img", crate::skill::skill_image_name(id));
    let base = root.join("Skill").join("_Canvas");
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = wz_core::WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(image_entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some(image_name.as_str()))
        else {
            continue;
        };
        let Some(offset) = image_entry.offset else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(offset) else {
            continue;
        };
        let skill_id = format!("{id:07}");
        let Some(target) = helper_path(&node, &["skill", &skill_id, ani_path]) else {
            continue;
        };
        let source = archive.source().clone();
        let mut frames = Vec::new();
        let mut delays = Vec::new();
        let mut children: Vec<&wz_core::image::Node> = target
            .object_children()
            .iter()
            .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
            .collect();
        children.sort_by_key(|c| c.name.as_ref().parse::<i32>().unwrap_or(0));
        if children.is_empty() {
            if let Some((w, h, rgba)) = decode_canvas_node(root, target, &source) {
                frames.push(crate::skill::SkillFrame {
                    width: w,
                    height: h,
                    rgba,
                    origin: node_origin(target),
                });
                delays.push(node_scalar_int(target, "delay").unwrap_or(100).max(20) as u32);
            }
        } else {
            for child in children {
                let Some((w, h, rgba)) = decode_canvas_node(root, child, &source) else {
                    continue;
                };
                frames.push(crate::skill::SkillFrame {
                    width: w,
                    height: h,
                    rgba,
                    origin: node_origin(child),
                });
                delays.push(node_scalar_int(child, "delay").unwrap_or(100).max(20) as u32);
            }
        }
        if frames.is_empty() {
            return None;
        }
        return Some(crate::skill::SkillView {
            id,
            x: 0,
            y: 0,
            flip: false,
            frames,
            delays,
        });
    }
    None
}

fn node_vector(node: &wz_core::image::Node, key: &str) -> Option<(i32, i32)> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == key)
        .and_then(|c| match &c.value {
            Some(wz_core::image::ImageValue::Vector(v)) => Some((v.x, v.y)),
            _ => None,
        })
}

fn node_text(node: &wz_core::image::Node, key: &str) -> Option<String> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == key)
        .and_then(|c| match &c.value {
            Some(wz_core::image::ImageValue::Str(s)) => Some(s.to_string()),
            _ => None,
        })
}

pub fn load_worldmap_view(
    root: &std::path::Path,
    name: &str,
) -> Option<crate::worldmap::WorldMapView> {
    let image_name = format!("{name}.img");
    let dir = root.join("Map").join("WorldMap");
    let (node, source) = find_image_in_dir(&dir, &image_name)?;
    Some(parse_worldmap_node(root, name, &node, &source))
}

fn parse_worldmap_node(
    root: &std::path::Path,
    name: &str,
    node: &wz_core::image::Node,
    source: &std::sync::Arc<dyn wz_pool::ByteSource>,
) -> crate::worldmap::WorldMapView {
    let display_name = helper_path(node, &["info", "WorldMap"])
        .and_then(|n| match &n.value {
            Some(wz_core::image::ImageValue::Str(s)) => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_else(|| name.to_string());
    let base_node = helper_path(node, &["BaseImg", "0"]);
    let base = base_node.and_then(|n| decode_canvas_node(root, n, source));
    let base_origin = base_node
        .and_then(|n| node_vector(n, "origin"))
        .unwrap_or((0, 0));
    let base_spot = base_node
        .and_then(|n| node_vector(n, "spot"))
        .unwrap_or((0, 0));

    let mut spots = Vec::new();
    if let Some(list) = helper_path(node, &["MapList"]) {
        let mut children: Vec<&wz_core::image::Node> = list
            .object_children()
            .iter()
            .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
            .collect();
        children.sort_by_key(|c| c.name.as_ref().parse::<i32>().unwrap_or(0));
        for spot_node in children {
            let spot = node_vector(spot_node, "spot")
                .or_else(|| {
                    Some((
                        node_scalar_int(spot_node, "x").unwrap_or(0),
                        node_scalar_int(spot_node, "y").unwrap_or(0),
                    ))
                })
                .unwrap_or((0, 0));
            let mut map_nos = Vec::new();
            if let Some(map_no) = spot_node
                .object_children()
                .iter()
                .find(|c| c.name.as_ref() == "mapNo")
            {
                for child in map_no.object_children() {
                    if let Some(value) = match &child.value {
                        Some(wz_core::image::ImageValue::Int(v)) => Some(*v),
                        Some(wz_core::image::ImageValue::Short(v)) => Some(i32::from(*v)),
                        _ => None,
                    } {
                        map_nos.push(value);
                    }
                }
            }
            spots.push(crate::worldmap::WorldMapSpot {
                x: spot.0 as f32,
                y: spot.1 as f32,
                spot_type: node_scalar_int(spot_node, "type").unwrap_or(0),
                title: node_text(spot_node, "title"),
                desc: node_text(spot_node, "desc"),
                map_nos,
            });
        }
    }

    let mut quests = Vec::new();
    if let Some(quest_root) = helper_path(node, &["QuestLimit"]) {
        let mut qnodes: Vec<&wz_core::image::Node> = Vec::new();
        if let Some(default) = quest_root
            .object_children()
            .iter()
            .find(|c| c.name.as_ref() == "default")
        {
            qnodes.push(default);
        }
        let mut numeric: Vec<&wz_core::image::Node> = quest_root
            .object_children()
            .iter()
            .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
            .collect();
        numeric.sort_by_key(|c| c.name.as_ref().parse::<i32>().unwrap_or(0));
        qnodes.extend(numeric);
        for q in qnodes {
            let list = |key: &str| {
                q.object_children()
                    .iter()
                    .find(|c| c.name.as_ref() == key)
                    .map(|n| {
                        n.object_children()
                            .iter()
                            .filter_map(|c| match &c.value {
                                Some(wz_core::image::ImageValue::Int(v)) => Some(*v),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            };
            quests.push(crate::worldmap::WorldMapQuest {
                name: q.name.to_string(),
                base: q
                    .object_children()
                    .iter()
                    .find(|c| c.name.as_ref() == "0")
                    .and_then(|n| decode_canvas_node(root, n, source)),
                open_maps: list("openMap"),
                close_maps: list("closeMap"),
                is_default: node_scalar_int(q, "default").unwrap_or(0) != 0,
                quest: node_scalar_int(q, "quest").unwrap_or(0),
                state: node_scalar_int(q, "state").unwrap_or(0),
            });
        }
    }
    let mut fogs = Vec::new();
    if let Some(fog_root) = helper_path(node, &["Fog"]) {
        let mut fnodes: Vec<&wz_core::image::Node> = fog_root
            .object_children()
            .iter()
            .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
            .collect();
        fnodes.sort_by_key(|c| c.name.as_ref().parse::<i32>().unwrap_or(0));
        for f in fnodes {
            fogs.push(crate::worldmap::WorldMapFog {
                name: f.name.to_string(),
                image: f
                    .object_children()
                    .iter()
                    .find(|c| c.name.as_ref() == "0")
                    .and_then(|n| decode_canvas_node(root, n, source)),
                quest: node_scalar_int(f, "quest").unwrap_or(0),
                state: node_scalar_int(f, "qState").unwrap_or(0),
            });
        }
    }

    crate::worldmap::WorldMapView {
        name: name.to_string(),
        display_name,
        base,
        base_origin,
        base_spot,
        spots,
        quests,
        fogs,
    }
}

fn load_map_tooltips(
    root: &std::path::Path,
    map_id: Option<i32>,
    rects: &[wz_db::MapTooltip],
) -> Vec<crate::state::MapTooltipView> {
    let mut out: Vec<crate::state::MapTooltipView> = rects
        .iter()
        .map(|t| crate::state::MapTooltipView {
            x1: t.x1,
            y1: t.y1,
            x2: t.x2,
            y2: t.y2,
            title: None,
            desc: None,
        })
        .collect();
    let Some(map_id) = map_id else {
        return out;
    };
    let Some((help, _)) = find_image_in_dir(&root.join("String"), "ToolTipHelp.img") else {
        return out;
    };
    for view in &mut out {
        let index = view_index(rects, view);
        let Some(index) = index else {
            continue;
        };
        let id = map_id.to_string();
        let idx = index.to_string();
        if let Some(node) = helper_path(&help, &["Mapobject", &id, &idx]) {
            view.title = node
                .object_children()
                .iter()
                .find(|c| c.name.as_ref() == "Title")
                .and_then(|c| match &c.value {
                    Some(wz_core::image::ImageValue::Str(s)) => Some(s.to_string()),
                    _ => None,
                });
            view.desc = node
                .object_children()
                .iter()
                .find(|c| c.name.as_ref() == "Desc")
                .and_then(|c| match &c.value {
                    Some(wz_core::image::ImageValue::Str(s)) => Some(s.to_string()),
                    _ => None,
                });
        }
    }
    out
}

fn view_index(rects: &[wz_db::MapTooltip], view: &crate::state::MapTooltipView) -> Option<i32> {
    rects
        .iter()
        .find(|r| r.x1 == view.x1 && r.y1 == view.y1 && r.x2 == view.x2 && r.y2 == view.y2)
        .map(|r| r.index)
}

fn find_map_helper(
    root: &std::path::Path,
) -> Option<(
    wz_core::image::Node,
    std::sync::Arc<dyn wz_pool::ByteSource>,
)> {
    let base = root.join("Map");
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = wz_core::WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(image_entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("MapHelper.img"))
        else {
            continue;
        };
        let Some(offset) = image_entry.offset else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(offset) else {
            continue;
        };
        return Some((node, archive.source().clone()));
    }
    None
}

fn helper_path<'a>(
    node: &'a wz_core::image::Node,
    segments: &[&str],
) -> Option<&'a wz_core::image::Node> {
    let mut cur = node;
    for segment in segments {
        cur = cur
            .object_children()
            .iter()
            .find(|c| c.name.as_ref() == *segment)?;
    }
    Some(cur)
}

fn collect_portal_animation(
    root: &std::path::Path,
    node: &wz_core::image::Node,
    source: &std::sync::Arc<dyn wz_pool::ByteSource>,
) -> crate::portal::PortalAnimation {
    let target = node
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == "portalStart")
        .unwrap_or(node);
    let mut animation = crate::portal::PortalAnimation::default();
    let mut children: Vec<&wz_core::image::Node> = target
        .object_children()
        .iter()
        .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
        .collect();
    children.sort_by_key(|c| c.name.as_ref().parse::<i32>().unwrap_or(0));
    if children.is_empty() {
        if let Some((w, h, rgba)) = decode_canvas_node(root, target, source) {
            animation.frames.push(crate::portal::PortalFrame {
                width: w,
                height: h,
                rgba,
                origin: node_origin(target),
            });
            animation
                .delays
                .push(node_scalar_int(target, "delay").unwrap_or(100).max(20) as u32);
        }
        return animation;
    }
    for child in children {
        let Some((w, h, rgba)) = decode_canvas_node(root, child, source) else {
            continue;
        };
        animation.frames.push(crate::portal::PortalFrame {
            width: w,
            height: h,
            rgba,
            origin: node_origin(child),
        });
        animation
            .delays
            .push(node_scalar_int(child, "delay").unwrap_or(100).max(20) as u32);
    }
    animation
}

fn find_obj_image(
    root: &std::path::Path,
    name: &str,
) -> Option<(
    wz_core::image::Node,
    std::sync::Arc<dyn wz_pool::ByteSource>,
)> {
    find_image_in_dir(&root.join("Map").join("Obj").join("_Canvas"), name)
}

fn collect_illuminant_animation(
    root: &std::path::Path,
    node: &wz_core::image::Node,
    source: &std::sync::Arc<dyn wz_pool::ByteSource>,
) -> crate::illuminant::IlluminantAnimation {
    let mut animation = crate::illuminant::IlluminantAnimation::default();
    let mut children: Vec<&wz_core::image::Node> = node
        .object_children()
        .iter()
        .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
        .collect();
    children.sort_by_key(|c| c.name.as_ref().parse::<i32>().unwrap_or(0));
    if children.is_empty() {
        if let Some((w, h, rgba)) = decode_canvas_node(root, node, source) {
            animation.frames.push(crate::illuminant::IlluminantFrame {
                width: w,
                height: h,
                rgba,
                origin: node_origin(node),
            });
            animation
                .delays
                .push(node_scalar_int(node, "delay").unwrap_or(100).max(20) as u32);
        }
        return animation;
    }
    for child in children {
        let Some((w, h, rgba)) = decode_canvas_node(root, child, source) else {
            continue;
        };
        animation.frames.push(crate::illuminant::IlluminantFrame {
            width: w,
            height: h,
            rgba,
            origin: node_origin(child),
        });
        animation
            .delays
            .push(node_scalar_int(child, "delay").unwrap_or(100).max(20) as u32);
    }
    animation
}

fn load_illuminant_view(
    root: &std::path::Path,
    helper: &wz_core::image::Node,
    source: &std::sync::Arc<dyn wz_pool::ByteSource>,
    cluster: &wz_db::IlluminantCluster,
) -> Option<crate::illuminant::IlluminantView> {
    let start_index = (cluster.start_point * 2).to_string();
    let end_index = (cluster.end_point * 2 + 1).to_string();
    let start_node = helper_path(helper, &["fieldGimmick", "cluster", &start_index])?;
    let end_node = helper_path(helper, &["fieldGimmick", "cluster", &end_index])?;
    let start_anim = collect_illuminant_animation(root, start_node, source);
    let end_anim = collect_illuminant_animation(root, end_node, source);
    if start_anim.frames.is_empty() && end_anim.frames.is_empty() {
        return None;
    }
    Some(crate::illuminant::IlluminantView {
        name: cluster.name.clone(),
        start: cluster.start,
        end: cluster.end,
        size: cluster.size,
        start_anim,
        end_anim,
    })
}

fn load_portal_view(
    root: &std::path::Path,
    helper: &wz_core::image::Node,
    source: &std::sync::Arc<dyn wz_pool::ByteSource>,
    portal: &wz_db::MapPortal,
) -> Option<crate::portal::PortalView> {
    let type_name = crate::portal::type_name(portal.p_type);
    let image_name = if portal.image == 0 {
        "default".to_string()
    } else {
        portal.image.to_string()
    };
    let node = helper_path(helper, &["portal", "game", type_name, &image_name])
        .or_else(|| helper_path(helper, &["portal", "game", type_name]))?;
    let animation = collect_portal_animation(root, node, source);
    if animation.frames.is_empty() {
        return None;
    }
    Some(crate::portal::PortalView {
        id: portal.id,
        portal_type: portal.p_type,
        x: portal.x,
        y: portal.y,
        shown_at_minimap: portal.shown_at_minimap || portal.enchant_portal,
        animation,
    })
}

fn load_minimap_icons(
    root: &std::path::Path,
    helper: &wz_core::image::Node,
    source: &std::sync::Arc<dyn wz_pool::ByteSource>,
) -> Option<crate::state::MinimapIcons> {
    let load = |name: &str| {
        helper_path(helper, &["minimap", name])
            .and_then(|node| decode_canvas_node(root, node, source))
    };
    Some(crate::state::MinimapIcons {
        portal: load("portal"),
        npc: load("npc"),
        mob: load("mob"),
        hidden_portal: load("hiddenportal"),
    })
}

fn find_life_image(
    root: &std::path::Path,
    npc: bool,
    id: i32,
) -> Option<(
    wz_core::image::Node,
    std::sync::Arc<dyn wz_pool::ByteSource>,
)> {
    let dir = root.join(if npc { "Npc" } else { "Mob" });
    let image = format!("{id:07}.img");
    let mut dirs = vec![dir.clone()];
    dirs.push(dir.join("_Canvas"));
    for base in dirs {
        for entry in std::fs::read_dir(base).ok()?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("wz") {
                continue;
            }
            let Ok(archive) = wz_core::WzArchive::open(&path) else {
                continue;
            };
            let Ok(tree) = archive.read_tree() else {
                continue;
            };
            let Some(image_entry) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some(image.as_str()))
            else {
                continue;
            };
            let Some(offset) = image_entry.offset else {
                continue;
            };
            let Ok(node) = archive.parse_image_at(offset) else {
                continue;
            };
            return Some((node, archive.source().clone()));
        }
    }
    None
}

fn collect_life_animation(
    root: &std::path::Path,
    node: &wz_core::image::Node,
    source: &std::sync::Arc<dyn wz_pool::ByteSource>,
    preferred: &[&str],
) -> crate::life::LifeAnimation {
    let mut animation = crate::life::LifeAnimation::default();
    let Some(target) = preferred.iter().find_map(|name| {
        node.object_children()
            .iter()
            .find(|c| c.name.as_ref() == *name)
    }) else {
        return animation;
    };
    let mut children: Vec<&wz_core::image::Node> = target
        .object_children()
        .iter()
        .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
        .collect();
    children.sort_by_key(|c| c.name.as_ref().parse::<i32>().unwrap_or(0));
    if children.is_empty() {
        if let Some((w, h, rgba)) = decode_canvas_node(root, target, source) {
            animation.frames.push(crate::life::LifeFrame {
                width: w,
                height: h,
                rgba,
                origin: node_origin(target),
            });
            animation
                .delays
                .push(node_scalar_int(target, "delay").unwrap_or(100).max(20) as u32);
        }
        return animation;
    }
    for child in children {
        let Some((w, h, rgba)) = decode_canvas_node(root, child, source) else {
            continue;
        };
        animation.frames.push(crate::life::LifeFrame {
            width: w,
            height: h,
            rgba,
            origin: node_origin(child),
        });
        animation
            .delays
            .push(node_scalar_int(child, "delay").unwrap_or(100).max(20) as u32);
    }
    animation
}

fn load_life_view(root: &std::path::Path, life: &wz_db::MapLife) -> Option<crate::life::LifeView> {
    let npc = matches!(life.kind(), wz_db::LifeKind::Npc);
    let mut id = life.id.parse::<i32>().unwrap_or(0);
    if id == 0 {
        return None;
    }
    let (mut node, mut source) = find_life_image(root, npc, id)?;
    for _ in 0..8 {
        let Some(link) = node
            .object_children()
            .iter()
            .find(|c| c.name.as_ref() == "info")
            .and_then(|info| {
                info.object_children()
                    .iter()
                    .find(|c| c.name.as_ref() == "link")
            })
            .and_then(|c| match &c.value {
                Some(wz_core::image::ImageValue::Int(v)) => Some(*v),
                _ => None,
            })
        else {
            break;
        };
        id = link;
        let (linked, linked_source) = find_life_image(root, npc, id)?;
        node = linked;
        source = linked_source;
    }
    let animation = collect_life_animation(root, &node, &source, &["stand", "default"]);
    let walk = collect_life_animation(root, &node, &source, &["move", "walk1"]);
    let hit = collect_life_animation(root, &node, &source, &["hit1", "hit"]);
    let die = collect_life_animation(root, &node, &source, &["die1", "die"]);
    if animation.frames.is_empty() {
        return None;
    }
    let hide_name = node
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == "info")
        .and_then(|info| {
            info.object_children()
                .iter()
                .find(|c| c.name.as_ref() == "hideName")
        })
        .and_then(|c| match &c.value {
            Some(wz_core::image::ImageValue::Int(v)) => Some(*v != 0),
            _ => None,
        })
        .unwrap_or(false);
    Some(crate::life::LifeView {
        id,
        index: life.index,
        npc,
        x: life.x,
        y: life.y,
        flip: life.flip,
        hidden: life.hide,
        name: None,
        hide_name,
        animation,
        walk,
        hit,
        die,
        state: crate::life::LifeState::Alive,
        state_started: 0.0,
        hp: 100,
        rx0: life.rx0.unwrap_or(life.x),
        rx1: life.rx1.unwrap_or(life.x),
        fh: life.fh,
        position_x: life.x as f32,
        position_y: life.y as f32,
        direction: if life.flip { -1.0 } else { 1.0 },
        moving: false,
    })
}

fn find_reactor_node(
    root: &std::path::Path,
    id: i32,
) -> Option<(
    wz_core::image::Node,
    std::sync::Arc<dyn wz_pool::ByteSource>,
)> {
    let base = root.join("Reactor").join("_Canvas");
    let image = format!("{id:07}.img");
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = wz_core::WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(image_entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some(image.as_str()))
        else {
            continue;
        };
        let Some(offset) = image_entry.offset else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(offset) else {
            continue;
        };
        return Some((node, archive.source().clone()));
    }
    None
}

fn collect_reactor_animation(
    node: &wz_core::image::Node,
    source: &std::sync::Arc<dyn wz_pool::ByteSource>,
) -> crate::reactor::ReactorAnimation {
    let mut animation = crate::reactor::ReactorAnimation::default();
    let mut children: Vec<&wz_core::image::Node> = node
        .object_children()
        .iter()
        .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
        .collect();
    children.sort_by_key(|c| c.name.as_ref().parse::<i32>().unwrap_or(0));
    if children.is_empty() {
        if let Some(png) = node.first_canvas() {
            if let Ok((w, h, rgba)) = wz_image::decode_png_ref_to_rgba(&png, source) {
                animation.frames.push(crate::reactor::ReactorFrame {
                    width: w,
                    height: h,
                    rgba,
                    origin: reactor_origin(&png),
                });
                animation
                    .delays
                    .push(node_scalar_int(node, "delay").unwrap_or(100).max(20) as u32);
            }
        }
        return animation;
    }
    for child in children {
        let Some(png) = child.first_canvas() else {
            continue;
        };
        let Ok((w, h, rgba)) = wz_image::decode_png_ref_to_rgba(&png, source) else {
            continue;
        };
        animation.frames.push(crate::reactor::ReactorFrame {
            width: w,
            height: h,
            rgba,
            origin: reactor_origin(&png),
        });
        animation
            .delays
            .push(node_scalar_int(child, "delay").unwrap_or(100).max(20) as u32);
    }
    animation
}

fn load_reactor_view(
    root: &std::path::Path,
    reactor: &wz_db::MapReactor,
) -> Option<crate::reactor::ReactorView> {
    let mut id = reactor.id;
    let (mut node, mut source) = find_reactor_node(root, id)?;
    if let Some(info) = node
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == "info")
    {
        if let Some(link) = info
            .object_children()
            .iter()
            .find(|c| c.name.as_ref() == "link")
            .and_then(|c| match &c.value {
                Some(wz_core::image::ImageValue::Int(v)) => Some(*v),
                _ => None,
            })
        {
            id = link;
            let (linked, linked_source) = find_reactor_node(root, link)?;
            node = linked;
            source = linked_source;
        }
    }

    let mut view = crate::reactor::ReactorView {
        index: reactor.index,
        id,
        x: reactor.x,
        y: reactor.y,
        flip: reactor.flip,
        ..Default::default()
    };
    let mut stages: Vec<(i32, &wz_core::image::Node)> = node
        .object_children()
        .iter()
        .filter_map(|c| c.name.as_ref().parse::<i32>().ok().map(|i| (i, c)))
        .collect();
    stages.sort_by_key(|(i, _)| *i);
    for (stage, stage_node) in stages {
        let animation = collect_reactor_animation(stage_node, &source);
        if !animation.is_empty() {
            view.stages.insert(stage, animation);
        }
        if let Some(hit) = stage_node
            .object_children()
            .iter()
            .find(|c| c.name.as_ref() == "hit")
        {
            let hit_animation = collect_reactor_animation(hit, &source);
            if !hit_animation.is_empty() {
                view.hits.insert(stage, hit_animation);
            }
        }
    }
    if view.stages.is_empty() && view.hits.is_empty() {
        return None;
    }
    Some(view)
}

fn find_particle_desc_node(root: &std::path::Path, name: &str) -> Option<wz_core::image::Node> {
    let base = root.join("Effect");
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = wz_core::WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(image_entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("particle.img"))
        else {
            continue;
        };
        let Some(offset) = image_entry.offset else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(offset) else {
            continue;
        };
        if let Some(found) = node
            .object_children()
            .iter()
            .find(|c| c.name.as_ref() == name)
        {
            return Some(found.clone());
        }
    }
    None
}

fn walk_node<'a>(
    node: &'a wz_core::image::Node,
    segments: &[&str],
) -> Option<&'a wz_core::image::Node> {
    if segments.is_empty() {
        return Some(node);
    }
    let head = segments[0];
    let next = node
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == head)
        .or_else(|| match &node.value {
            Some(wz_core::image::ImageValue::Png(png)) => {
                png.sub_property.as_ref().and_then(|sub| {
                    sub.object_children()
                        .iter()
                        .find(|c| c.name.as_ref() == head)
                })
            }
            _ => None,
        })?;
    walk_node(next, &segments[1..])
}

fn load_particle_texture(root: &std::path::Path, outlink: &str) -> Option<(u32, u32, Vec<u8>)> {
    let (image, sub) = outlink.split_once(".img/")?;
    let image_name = format!("{}.img", image.rsplit('/').next()?);
    let segments: Vec<&str> = sub.split('/').filter(|s| !s.is_empty()).collect();
    let base = root.join("Effect").join("_Canvas");
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = wz_core::WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(image_entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some(image_name.as_str()))
        else {
            continue;
        };
        let Some(offset) = image_entry.offset else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(offset) else {
            continue;
        };
        let target = walk_node(&node, &segments)?;
        let png = target.first_canvas()?;
        if let Ok((w, h, rgba)) = wz_image::decode_png_ref_to_rgba(&png, archive.source()) {
            return Some((w, h, rgba));
        }
    }
    None
}

fn node_scalar_int(node: &wz_core::image::Node, key: &str) -> Option<i32> {
    let child = node
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == key)?;
    match &child.value {
        Some(wz_core::image::ImageValue::Int(v)) => Some(*v),
        Some(wz_core::image::ImageValue::Short(v)) => Some(i32::from(*v)),
        Some(wz_core::image::ImageValue::Long(v)) => Some(*v as i32),
        _ => None,
    }
}

fn decode_node_frame(
    node: &wz_core::image::Node,
    archive: &wz_core::WzArchive,
) -> Option<(u32, u32, Vec<u8>)> {
    let png = node.first_canvas()?;
    wz_image::decode_png_ref_to_rgba(&png, archive.source()).ok()
}

fn collect_frames(
    node: &wz_core::image::Node,
    archive: &wz_core::WzArchive,
    out: &mut Vec<(u32, u32, Vec<u8>)>,
    delays: &mut Vec<u32>,
) {
    if let Some(frame) = decode_node_frame(node, archive) {
        out.push(frame);
        delays.push(node_scalar_int(node, "delay").unwrap_or(100).max(20) as u32);
        return;
    }
    for child in node.object_children() {
        collect_frames(child, archive, out, delays);
    }
}

fn node_origin(node: &wz_core::image::Node) -> (i32, i32) {
    if let Some(origin) = node
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == "origin")
        .and_then(|c| match &c.value {
            Some(wz_core::image::ImageValue::Vector(v)) => Some((v.x, v.y)),
            _ => None,
        })
    {
        return origin;
    }
    node.first_canvas()
        .map(|png| reactor_origin(&png))
        .unwrap_or((0, 0))
}

fn png_outlink(png: &wz_core::image::PngRef) -> Option<String> {
    let sub = png.sub_property.as_ref()?;
    sub.object_children()
        .iter()
        .find(|c| c.name.as_ref() == "_outlink")
        .and_then(|c| match &c.value {
            Some(wz_core::image::ImageValue::Str(s)) => Some(s.to_string()),
            _ => None,
        })
}

fn decode_canvas_node(
    root: &std::path::Path,
    node: &wz_core::image::Node,
    source: &std::sync::Arc<dyn wz_pool::ByteSource>,
) -> Option<(u32, u32, Vec<u8>)> {
    if let Some(outlink) = node
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == "_outlink")
        .and_then(|c| match &c.value {
            Some(wz_core::image::ImageValue::Str(s)) => Some(s.to_string()),
            _ => None,
        })
    {
        return decode_outlink_canvas(root, &outlink, 0);
    }
    let png = node.first_canvas()?;
    if let Some(outlink) = png_outlink(&png) {
        return decode_outlink_canvas(root, &outlink, 0);
    }
    wz_image::decode_png_ref_to_rgba(&png, source).ok()
}

fn decode_outlink_canvas(
    root: &std::path::Path,
    outlink: &str,
    depth: usize,
) -> Option<(u32, u32, Vec<u8>)> {
    if depth > 8 {
        return None;
    }
    let (image_part, sub) = outlink.split_once(".img/")?;
    let image_name = format!("{}.img", image_part.rsplit('/').next()?);
    let dir = root.join(image_part).parent()?.to_path_buf();
    let (image, source) = find_image_in_dir(&dir, &image_name)?;
    let segments: Vec<&str> = sub.split('/').filter(|s| !s.is_empty()).collect();
    let target = walk_node(&image, &segments)?;
    if let Some(next) = target
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == "_outlink")
        .and_then(|c| match &c.value {
            Some(wz_core::image::ImageValue::Str(s)) => Some(s.to_string()),
            _ => None,
        })
    {
        return decode_outlink_canvas(root, &next, depth + 1);
    }
    let png = target.first_canvas()?;
    if let Some(next) = png_outlink(&png) {
        return decode_outlink_canvas(root, &next, depth + 1);
    }
    wz_image::decode_png_ref_to_rgba(&png, &source).ok()
}

fn find_image_in_dir(
    dir: &std::path::Path,
    image_name: &str,
) -> Option<(
    wz_core::image::Node,
    std::sync::Arc<dyn wz_pool::ByteSource>,
)> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = wz_core::WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(image_entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some(image_name))
        else {
            continue;
        };
        let Some(offset) = image_entry.offset else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(offset) else {
            continue;
        };
        return Some((node, archive.source().clone()));
    }
    None
}

fn load_minimap_canvas(
    root: &std::path::Path,
    map_node: &wz_core::image::Node,
) -> Option<(u32, u32, Vec<u8>)> {
    let mini = map_node
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == "miniMap")?;
    let canvas = mini
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == "canvas")?;
    if let Some(outlink) = canvas
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == "_outlink")
        .and_then(|c| match &c.value {
            Some(wz_core::image::ImageValue::Str(s)) => Some(s.to_string()),
            _ => None,
        })
    {
        return decode_outlink_canvas(root, &outlink, 0);
    }
    let png = canvas.first_canvas()?;
    if let Some(outlink) = png_outlink(&png) {
        return decode_outlink_canvas(root, &outlink, 0);
    }
    let source = map_node_source(root, map_node)?;
    wz_image::decode_png_ref_to_rgba(&png, &source).ok()
}

fn map_node_source(
    _root: &std::path::Path,
    _node: &wz_core::image::Node,
) -> Option<std::sync::Arc<dyn wz_pool::ByteSource>> {
    None
}

fn water_shader_kind(node: &wz_core::image::Node) -> Option<crate::water::WaterKind> {
    let shader = node
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == "shader")
        .and_then(|c| match &c.value {
            Some(wz_core::image::ImageValue::Str(s)) => Some(s.to_string()),
            _ => None,
        })?;
    if shader.contains("waterBack") {
        Some(crate::water::WaterKind::Back)
    } else if shader.contains("waterFront") {
        Some(crate::water::WaterKind::Front)
    } else {
        None
    }
}

fn load_water_layer(
    root: &std::path::Path,
    background: &wz_db::BackItem,
) -> Option<crate::water::WaterLayer> {
    let image_name = format!("{}.img", background.b_s);
    let no = background.no.trim_matches('/');
    let dirs = [
        root.join("Map").join("Back"),
        root.join("Map").join("Back").join("_Canvas"),
    ];
    for base in dirs {
        for entry in std::fs::read_dir(base).ok()?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("wz") {
                continue;
            }
            let Ok(archive) = wz_core::WzArchive::open(&path) else {
                continue;
            };
            let Ok(tree) = archive.read_tree() else {
                continue;
            };
            let Some(image_entry) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some(image_name.as_str()))
            else {
                continue;
            };
            let Some(offset) = image_entry.offset else {
                continue;
            };
            let Ok(node) = archive.parse_image_at(offset) else {
                continue;
            };
            let Some(target) = walk_node(&node, &["back", no]) else {
                continue;
            };
            let Some(kind) = water_shader_kind(target) else {
                continue;
            };
            let source = archive.source().clone();
            let mut frames = Vec::new();
            let mut delays = Vec::new();
            let mut frame_nodes: Vec<&wz_core::image::Node> = target
                .object_children()
                .iter()
                .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
                .collect();
            frame_nodes.sort_by_key(|c| c.name.as_ref().parse::<i32>().unwrap_or(0));
            if frame_nodes.is_empty() {
                frame_nodes.push(target);
            }
            for frame in frame_nodes {
                let mut textures = Vec::new();
                let mut texture_nodes: Vec<&wz_core::image::Node> = frame
                    .object_children()
                    .iter()
                    .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
                    .collect();
                texture_nodes.sort_by_key(|c| c.name.as_ref().parse::<i32>().unwrap_or(0));
                if texture_nodes.is_empty() {
                    if let Some(texture) = decode_canvas_node(root, frame, &source) {
                        textures.push(texture);
                    }
                } else {
                    for texture_group in texture_nodes {
                        let mut texture_indices: Vec<&wz_core::image::Node> = texture_group
                            .object_children()
                            .iter()
                            .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
                            .collect();
                        texture_indices
                            .sort_by_key(|c| c.name.as_ref().parse::<i32>().unwrap_or(0));
                        if texture_indices.is_empty() {
                            if let Some(texture) = decode_canvas_node(root, texture_group, &source)
                            {
                                textures.push(texture);
                            }
                        } else {
                            for index in texture_indices {
                                if let Some(texture) = decode_canvas_node(root, index, &source) {
                                    textures.push(texture);
                                }
                            }
                        }
                    }
                }
                if !textures.is_empty() {
                    frames.push(crate::water::WaterFrame { textures });
                    delays.push(node_scalar_int(frame, "delay").unwrap_or(100).max(20) as u32);
                }
            }
            if frames.is_empty() {
                return None;
            }
            let (w, h) = frames
                .first()
                .and_then(|f| f.textures.first())
                .map(|(w, h, _)| (*w, *h))
                .unwrap_or((1, 1));
            return Some(crate::water::WaterLayer {
                background: crate::background::BackgroundLayer {
                    x: background.x,
                    y: background.y,
                    rx: background.rx,
                    ry: background.ry,
                    cx: background.cx,
                    cy: background.cy,
                    tile_mode: background.tile_mode,
                    flip: background.flip,
                    alpha: background.alpha.clamp(0, 255) as u8,
                    front: background.front,
                    frames: vec![(w, h, Vec::new())],
                    delays: vec![100],
                },
                kind,
                frames,
                delays,
            });
        }
    }
    None
}

fn load_background_layer(
    root: &std::path::Path,
    background: &wz_db::BackItem,
) -> Option<crate::background::BackgroundLayer> {
    let image_name = format!("{}.img", background.b_s);
    let no = background.no.trim_matches('/');
    let dirs = [
        root.join("Map").join("Back"),
        root.join("Map").join("Back").join("_Canvas"),
    ];
    let mut candidates: Vec<Vec<&str>> = Vec::new();
    if background.ani == 1 {
        candidates.push(vec!["ani", no]);
    }
    candidates.push(vec!["back", no]);
    if no.is_empty() {
        candidates.clear();
        candidates.push(Vec::new());
    }

    for base in dirs {
        for entry in std::fs::read_dir(base).ok()?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("wz") {
                continue;
            }
            let Ok(archive) = wz_core::WzArchive::open(&path) else {
                continue;
            };
            let Ok(tree) = archive.read_tree() else {
                continue;
            };
            let Some(image_entry) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some(image_name.as_str()))
            else {
                continue;
            };
            let Some(offset) = image_entry.offset else {
                continue;
            };
            let Ok(node) = archive.parse_image_at(offset) else {
                continue;
            };
            for segments in &candidates {
                let Some(target) = walk_node(&node, segments) else {
                    continue;
                };
                if water_shader_kind(target).is_some() {
                    continue;
                }
                let mut frames = Vec::new();
                let mut delays = Vec::new();
                collect_frames(target, &archive, &mut frames, &mut delays);
                if frames.is_empty() {
                    continue;
                }
                return Some(crate::background::BackgroundLayer {
                    x: background.x,
                    y: background.y,
                    rx: background.rx,
                    ry: background.ry,
                    cx: background.cx,
                    cy: background.cy,
                    tile_mode: background.tile_mode,
                    flip: background.flip,
                    alpha: background.alpha.clamp(0, 255) as u8,
                    front: background.front,
                    frames,
                    delays,
                });
            }
        }
    }
    None
}

fn map_id_from_path(path: &str) -> Option<i32> {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".img")
        .parse::<i32>()
        .ok()
}

pub fn load_map_by_path(state: &mut AppState) {
    if state.tree.is_none() {
        state.map_status = "WZ 파일을 먼저 열어 주세요".into();
        return;
    }
    let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
        state.map_status = "아카이브 없음".into();
        return;
    };
    let path = state.map_path.clone();
    let Some(off) = archive.find_entry_offset(&path) else {
        state.map_status = format!("엔트리 없음: {path}");
        return;
    };
    let Ok(node) = archive.parse_image_at(off) else {
        state.map_status = "이미지 파싱 실패".into();
        return;
    };
    let mut map = MapData::parse_from_node(&node);
    if map.info.map_id.is_none() {
        map.info.map_id = map_id_from_path(&state.map_path);
    }

    let data_root = find_data_root(state.archive_path.as_deref());
    state.data_root = data_root.clone();

    state.map_minimap = data_root
        .as_deref()
        .and_then(|root| load_minimap_canvas(root, &node))
        .and_then(|(w, h, rgba)| map.mini_map.clone().map(|info| (w, h, rgba, info)));
    state.map_tooltips = data_root
        .as_deref()
        .map(|root| load_map_tooltips(root, map.info.map_id, &map.tooltips))
        .unwrap_or_default();
    state.map_minimap_texture = None;
    state.map_minimap_icons = data_root.as_deref().and_then(|root| {
        let (helper, source) = find_map_helper(root)?;
        load_minimap_icons(root, &helper, &source)
    });
    state.map_minimap_icon_textures = None;

    let back_img = if let (Some(root), Some(first)) = (data_root.as_ref(), map.backgrounds.first())
    {
        load_back_image(root, &first.b_s)
    } else {
        None
    };
    state.map_back_image = back_img;

    // 타일/오브젝트 이미지는 워커 스레드에서 로드한다 (UI 블로킹 방지).
    if let Some(root) = data_root.clone() {
        let tiles_clone = map.tiles.clone();
        let objs_clone = map.objects.clone();
        let reactors_clone = map.reactors.clone();
        let backgrounds_clone = map.backgrounds.clone();
        let particles_clone = map.particles.clone();
        let life_clone = map.life.clone();
        let portals_clone = map.portals.clone();
        let clusters_clone = map.illuminant_clusters.clone();
        let (tx, rx) = crossbeam_channel::unbounded();
        state.map_images_rx = Some(rx);
        std::thread::spawn(move || {
            let mut tiles = Vec::new();
            for tile in tiles_clone.iter() {
                if let Some((w, h, rgba)) =
                    load_canvas_image(&root, "Tile", &tile.t_s, &format!("{}/{}", tile.u, tile.no))
                {
                    tiles.push((tile.x, tile.y, w, h, rgba));
                }
            }
            let mut objs = Vec::new();
            for obj in &objs_clone {
                let inner = format!("{}/{}/{}/0", obj.l0, obj.l1, obj.l2);
                if let Some((w, h, rgba)) = load_canvas_image(&root, "Obj", &obj.o_s, &inner) {
                    objs.push((obj.x, obj.y, w, h, rgba));
                }
            }
            let mut reactors = Vec::new();
            for reactor in &reactors_clone {
                if let Some(view) = load_reactor_view(&root, reactor) {
                    reactors.push(view);
                }
            }
            let mut backgrounds = Vec::new();
            let mut waters = Vec::new();
            for background in &backgrounds_clone {
                if let Some(water) = load_water_layer(&root, background) {
                    waters.push(water);
                } else if let Some(layer) = load_background_layer(&root, background) {
                    backgrounds.push(layer);
                }
            }
            let mut particles = Vec::new();
            for particle in &particles_clone {
                let Some(desc_node) = find_particle_desc_node(&root, &particle.name) else {
                    continue;
                };
                let desc = wz_db::particle::parse_particle_desc(&desc_node);
                let Some(outlink) = desc.texture.outlink.clone() else {
                    continue;
                };
                let Some(texture) = load_particle_texture(&root, &outlink) else {
                    continue;
                };
                particles.push(crate::particles::MapParticleLayer {
                    origin: desc.texture.origin.unwrap_or((0, 0)),
                    desc,
                    texture,
                    points: particle.points.clone(),
                    z: particle.z,
                    rx: particle.rx,
                    ry: particle.ry,
                });
            }
            let mut lifes = Vec::new();
            for life in &life_clone {
                if let Some(view) = load_life_view(&root, life) {
                    lifes.push(view);
                }
            }
            let mut portals = Vec::new();
            if let Some((helper, helper_source)) = find_map_helper(&root) {
                for portal in &portals_clone {
                    if let Some(view) = load_portal_view(&root, &helper, &helper_source, portal) {
                        portals.push(view);
                    }
                }
            }
            let mut illuminants = Vec::new();
            if let Some((sellas, sellas_source)) = find_obj_image(&root, "sellas.img") {
                for cluster in &clusters_clone {
                    if let Some(view) =
                        load_illuminant_view(&root, &sellas, &sellas_source, cluster)
                    {
                        illuminants.push(view);
                    }
                }
            }
            let _ = tx.send(crate::state::MapImages {
                tiles,
                objs,
                reactors,
                backgrounds,
                waters,
                particles,
                lifes,
                portals,
                illuminants,
            });
        });
    }

    state.map_background_time = 0.0;
    state.map_background_last = 0.0;
    state.map_particle_layers.clear();
    state.map_particle_time = 0.0;
    state.map_particle_last = 0.0;
    state.map_status = format!(
        "map_id={:?} bgm={:?} town={} fh={} portals={} life={} back={} obj={} tile={} reactor={} particle={} (back_img={})",
        map.info.map_id, map.info.bgm, map.info.town,
        map.footholds.len(), map.portals.len(), map.life.len(),
        map.backgrounds.len(), map.objects.len(), map.tiles.len(),
        map.reactors.len(), map.particles.len(),
        state.map_back_image.is_some()
    );
    state.current_map = Some(map);
    state.viewport_dirty = true;
    state.viewport_map_dirty = true;
}

pub fn load_map_from_selection(state: &mut AppState) {
    let Some(sel) = state.selected.clone() else {
        return;
    };
    let Some(tree) = &state.tree else {
        return;
    };
    let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
        return;
    };
    let name = sel.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
    let Some(e) = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some(name))
    else {
        state.map_status = format!("엔트리 없음: {name}");
        return;
    };
    let Some(off) = e.offset else {
        state.map_status = "오프셋 없음".into();
        return;
    };
    let Ok(node) = archive.parse_image_at(off) else {
        state.map_status = "이미지 파싱 실패".into();
        return;
    };
    let mut map = MapData::parse_from_node(&node);
    if map.info.map_id.is_none() {
        map.info.map_id = name.trim_end_matches(".img").parse::<i32>().ok();
    }
    state.map_status = format!(
        "map_id={:?} bgm={:?} town={} fh={} portals={} life={}",
        map.info.map_id,
        map.info.bgm,
        map.info.town,
        map.footholds.len(),
        map.portals.len(),
        map.life.len()
    );
    // map_path 를 갱신하고 load_map_by_path 로 이미지 로딩을 위임한다.
    state.map_path = name.to_string();
    state.current_map = Some(map);
    // 이미지 로딩은 load_map_by_path 의 워커가 처리한다.
    // (map_path 가 설정되어 있으므로 그대로 호출)
    load_map_by_path(state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_real_map_tooltips() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let path = root
            .join("Map")
            .join("Map")
            .join("Map0")
            .join("Map0_000.wz");
        if !path.exists() {
            return;
        }
        let archive = wz_core::WzArchive::open(&path).unwrap();
        let tree = archive.read_tree().unwrap();
        let entry = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("000050000.img"))
            .expect("000050000.img");
        let node = archive.parse_image_at(entry.offset.unwrap()).unwrap();
        let map = wz_db::MapData::parse_from_node(&node);
        assert_eq!(map.tooltips.len(), 1);
        let views = load_map_tooltips(&root, Some(50000), &map.tooltips);
        assert_eq!(views.len(), 1);
        assert!(
            views
                .iter()
                .any(|v| v.title.as_deref().is_some_and(|t| !t.is_empty())),
            "{views:?}"
        );
    }

    #[test]
    fn parses_synthetic_worldmap_quest_and_fog() {
        let mut root = wz_core::image::Node::new("Synthetic.img");
        let mut info = wz_core::image::Node::new("info");
        let mut world_name = wz_core::image::Node::new("WorldMap");
        world_name.value = Some(wz_core::image::ImageValue::Str("Synthetic".into()));
        info.children.push(world_name);
        root.children.push(info);

        let mut qroot = wz_core::image::Node::new("QuestLimit");
        let mut quest = wz_core::image::Node::new("default");
        for (key, value) in [("quest", 123), ("state", 2), ("default", 1)] {
            let mut n = wz_core::image::Node::new(key);
            n.value = Some(wz_core::image::ImageValue::Int(value));
            quest.children.push(n);
        }
        let mut open = wz_core::image::Node::new("openMap");
        for value in [3, 5] {
            let mut n = wz_core::image::Node::new(value.to_string());
            n.value = Some(wz_core::image::ImageValue::Int(value));
            open.children.push(n);
        }
        let mut close = wz_core::image::Node::new("closeMap");
        let mut n = wz_core::image::Node::new("0");
        n.value = Some(wz_core::image::ImageValue::Int(9));
        close.children.push(n);
        quest.children.push(open);
        quest.children.push(close);
        qroot.children.push(quest);
        root.children.push(qroot);

        let mut froot = wz_core::image::Node::new("Fog");
        let mut fog = wz_core::image::Node::new("0");
        for (key, value) in [("quest", 77), ("qState", 1)] {
            let mut n = wz_core::image::Node::new(key);
            n.value = Some(wz_core::image::ImageValue::Int(value));
            fog.children.push(n);
        }
        froot.children.push(fog);
        root.children.push(froot);

        let mut mlist = wz_core::image::Node::new("MapList");
        let mut spot = wz_core::image::Node::new("0");
        let mut pos = wz_core::image::Node::new("spot");
        pos.value = Some(wz_core::image::ImageValue::Vector(wz_core::image::Vec2 {
            x: 11,
            y: 22,
        }));
        spot.children.push(pos);
        let mut map_no = wz_core::image::Node::new("mapNo");
        let mut no = wz_core::image::Node::new("0");
        no.value = Some(wz_core::image::ImageValue::Int(100000000));
        map_no.children.push(no);
        spot.children.push(map_no);
        mlist.children.push(spot);
        root.children.push(mlist);

        let source: std::sync::Arc<dyn wz_pool::ByteSource> =
            std::sync::Arc::new(wz_pool::MemorySource::new(vec![0u8; 4]));
        let view = parse_worldmap_node(std::path::Path::new("."), "Synthetic", &root, &source);
        assert_eq!(view.display_name, "Synthetic");
        assert_eq!(view.quests.len(), 1);
        assert_eq!(view.quests[0].open_maps, vec![3, 5]);
        assert_eq!(view.quests[0].close_maps, vec![9]);
        assert_eq!(view.quests[0].quest, 123);
        assert!(view.quests[0].is_default);
        assert_eq!(view.fogs.len(), 1);
        assert_eq!(view.fogs[0].quest, 77);
        assert_eq!(view.fogs[0].state, 1);
        assert_eq!(view.spots.len(), 1);
        assert_eq!(view.spots[0].map_nos, vec![100_000_000]);
    }

    #[test]
    fn loads_real_worldmap() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let names = crate::worldmap::list_worldmaps(&root);
        assert!(names.iter().any(|n| n == "WorldMap000"), "{names:?}");
        let view = load_worldmap_view(&root, "WorldMap000").expect("worldmap view");
        assert_eq!(view.display_name, "WorldMap000");
        assert!(view.base.is_some());
        assert_eq!(view.spots.len(), 7);
        assert!(!view.spots[0].map_nos.is_empty());
    }

    #[test]
    fn loads_real_skill_animation() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let view = load_skill_view(&root, 1_001_005, "effect").expect("skill effect");
        assert_eq!(view.id, 1_001_005);
        assert!(!view.frames.is_empty());
        assert!(view.frames[0].width > 1);
        assert!(view
            .frames
            .iter()
            .any(|f| f.rgba.chunks(4).any(|p| p[3] != 0)));
    }

    #[test]
    fn loads_real_water_back_front_layers() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let path = root
            .join("Map")
            .join("Map")
            .join("Map9")
            .join("Map9_001.wz");
        if !path.exists() {
            return;
        }
        let archive = wz_core::WzArchive::open(&path).unwrap();
        let tree = archive.read_tree().unwrap();
        let entry = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("993258900.img"))
            .expect("993258900.img");
        let node = archive.parse_image_at(entry.offset.unwrap()).unwrap();
        let map = wz_db::MapData::parse_from_node(&node);
        let back = map
            .backgrounds
            .iter()
            .find(|b| b.b_s == "2024BlueholeExploration" && b.no == "14")
            .expect("waterBack bg");
        let front = map
            .backgrounds
            .iter()
            .find(|b| b.b_s == "2024BlueholeExploration" && b.no == "15")
            .expect("waterFront bg");

        let back_layer = load_water_layer(&root, back).expect("waterBack layer");
        assert_eq!(back_layer.kind, crate::water::WaterKind::Back);
        assert!(!back_layer.frames.is_empty());
        assert!(back_layer.frames[0].textures[0].0 > 1);

        let front_layer = load_water_layer(&root, front).expect("waterFront layer");
        assert_eq!(front_layer.kind, crate::water::WaterKind::Front);
        assert!(!front_layer.frames.is_empty());
        assert!(front_layer.frames[0].textures[0].0 > 1);
        assert_ne!(front_layer.tint(0.0), front_layer.tint(700.0));

        // 실제 waterFront 텍스처로 WGSL 파이프라인 픽셀 출력 검증.
        if let Ok(mut rc) = wz_render::RenderContext::new_headless() {
            let frame = &front_layer.frames[0];
            let mut keys = Vec::new();
            for (i, (w, h, rgba)) in frame.textures.iter().enumerate() {
                let key = wz_render::GpuKey(900 + i as u32);
                rc.create_texture(key, *w, *h, rgba).unwrap();
                keys.push(key);
            }
            let camera = wz_render::Camera2D::new(64.0, 64.0);
            let uniforms = camera.build_uniforms(0.0, 0.0);
            let params = wz_render::WaterFrontParams {
                vp: uniforms.vp,
                vp_inv: uniforms.vp_inv,
                resolution_time: [64.0, 64.0, 0.5, 0.02],
                ..Default::default()
            };
            let front_pixels = rc
                .render_water_to_rgba(
                    64,
                    64,
                    wz_render::shader::MaplePixelShader::WaterFront,
                    &params.to_bytes(),
                    &keys,
                    [0.0, 0.0, 64.0, 64.0],
                    &uniforms,
                )
                .expect("waterFront render");
            assert!(front_pixels.chunks(4).any(|p| p[3] != 0));

            let back_params = wz_render::WaterBackParams {
                min_y: 0.0,
                max_y: 64.0,
                ..Default::default()
            };
            let back_pixels = rc
                .render_water_to_rgba(
                    64,
                    64,
                    wz_render::shader::MaplePixelShader::WaterBack,
                    &back_params.to_bytes(),
                    &[],
                    [0.0, 0.0, 64.0, 64.0],
                    &uniforms,
                )
                .expect("waterBack render");
            assert!(back_pixels.chunks(4).any(|p| p[3] != 0));

            // 993258900: waterBack은 scene 뒤, waterFront는 scene 앞.
            let scene: Vec<u8> = (0..64 * 64).flat_map(|_| [40u8, 50, 60, 128]).collect();
            let behind = crate::compose::compose(&scene, Some(&back_pixels), None);
            let both = crate::compose::compose(&scene, Some(&back_pixels), Some(&front_pixels));
            assert_ne!(behind, both, "waterFront must be in front of the scene");
            assert_ne!(behind, scene, "waterBack must be behind the scene");
        }
    }

    #[test]
    fn loads_real_illuminant_cluster_sprites() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let Some((sellas, source)) = find_obj_image(&root, "sellas.img") else {
            return;
        };
        let cluster = wz_db::IlluminantCluster {
            name: "test".into(),
            start: (10, 20),
            end: (100, 200),
            size: 24,
            start_point: 0,
            end_point: 0,
            ..Default::default()
        };
        let view =
            load_illuminant_view(&root, &sellas, &source, &cluster).expect("illuminant view");
        assert!(!view.start_anim.frames.is_empty());
        assert!(!view.end_anim.frames.is_empty());
        assert!(view
            .start_anim
            .frames
            .iter()
            .any(|f| f.rgba.chunks(4).any(|p| p[3] != 0)));
    }

    #[test]
    fn loads_real_portal_and_minimap_icons() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let Some((helper, source)) = find_map_helper(&root) else {
            return;
        };
        let portal = wz_db::MapPortal {
            id: 3,
            p_name: "out00".into(),
            p_type: 2,
            x: 544,
            y: 423,
            ..Default::default()
        };
        let view = load_portal_view(&root, &helper, &source, &portal).expect("portal view");
        assert!(!view.animation.frames.is_empty());
        assert!(view.animation.frames[0].width > 1);
        assert!(view
            .animation
            .frames
            .iter()
            .any(|f| f.rgba.chunks(4).any(|p| p[3] != 0)));

        let icons = load_minimap_icons(&root, &helper, &source).expect("icons");
        for (name, icon) in [
            ("portal", &icons.portal),
            ("npc", &icons.npc),
            ("mob", &icons.mob),
            ("hidden", &icons.hidden_portal),
        ] {
            let (w, h, rgba) = icon.as_ref().unwrap_or_else(|| panic!("{name} icon"));
            assert!(*w > 1 && *h > 1, "{name}: {w}x{h}");
            assert_eq!(rgba.len(), (*w * *h * 4) as usize);
        }
    }

    #[test]
    fn loads_real_npc_and_mob_frames() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let npc = wz_db::MapLife {
            index: 0,
            id: "1541000".into(),
            life_type: "n".into(),
            x: -590,
            y: 230,
            flip: true,
            ..Default::default()
        };
        let Some(view) = load_life_view(&root, &npc) else {
            return;
        };
        assert!(view.npc);
        assert!(view.flip);
        assert!(!view.animation.frames.is_empty());
        assert!(view.animation.frames[0].width > 1);
        assert!(view
            .animation
            .frames
            .iter()
            .any(|f| f.rgba.chunks(4).any(|p| p[3] != 0)));

        let mob = wz_db::MapLife {
            index: 1,
            id: "100100".into(),
            life_type: "m".into(),
            x: 0,
            y: 0,
            ..Default::default()
        };
        if let Some(view) = load_life_view(&root, &mob) {
            assert!(!view.npc);
            assert!(!view.animation.frames.is_empty());
            assert!(!view.walk.frames.is_empty(), "mob move frames");
            assert!(!view.hit.frames.is_empty(), "mob hit frames");
            assert!(!view.die.frames.is_empty(), "mob die frames");
        }

        // 실제 맵 발판 + 실제 몹 프레임으로 AI 이동 검증.
        let map_path = root
            .join("Map")
            .join("Map")
            .join("Map0")
            .join("Map0_000.wz");
        if map_path.exists() {
            let archive = wz_core::WzArchive::open(&map_path).unwrap();
            let tree = archive.read_tree().unwrap();
            if let Some(entry) = tree
                .root
                .entries
                .iter()
                .find(|e| e.name.as_deref() == Some("000020000.img"))
            {
                let node = archive.parse_image_at(entry.offset.unwrap()).unwrap();
                let map = wz_db::MapData::parse_from_node(&node);
                let real = map
                    .life
                    .iter()
                    .find(|l| l.life_type == "m")
                    .expect("mob life");
                let mut patrol = wz_db::MapLife {
                    id: "100100".into(),
                    life_type: "m".into(),
                    x: real.x,
                    y: real.y,
                    fh: real.fh,
                    rx0: Some(real.x - 50),
                    rx1: Some(real.x + 50),
                    ..Default::default()
                };
                let mut view = load_life_view(&root, &patrol).expect("mob view");
                let before = view.position_x;
                view.tick(1.0, 0.0, &map.footholds_by_id);
                assert!((view.position_x - before).abs() > 1.0);
                assert!(view.moving);
                // 발판이 있으면 y가 발판 위로 보정된다.
                if map.footholds_by_id.contains_key(&patrol.fh) {
                    assert!((view.position_y - real.y as f32).abs() < 200.0);
                }
                patrol.rx0 = None;
                patrol.rx1 = None;
            }
        }
    }

    #[test]
    fn loads_real_minimap_canvas() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let path = root
            .join("Map")
            .join("Map")
            .join("Map0")
            .join("Map0_000.wz");
        if !path.exists() {
            return;
        }
        let archive = wz_core::WzArchive::open(&path).unwrap();
        let tree = archive.read_tree().unwrap();
        let entry = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("000010000.img"))
            .expect("000010000.img");
        let node = archive.parse_image_at(entry.offset.unwrap()).unwrap();
        let map = wz_db::MapData::parse_from_node(&node);
        let info = map.mini_map.expect("miniMap info");
        assert_eq!(info.width, 2313);
        assert_eq!(info.height, 1592);
        assert_eq!(info.center_x, 1338);
        assert_eq!(info.mag, 4);
        let (w, h, rgba) = load_minimap_canvas(&root, &node).expect("minimap canvas");
        assert!(w > 1 && h > 1);
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        assert!(rgba.chunks(4).any(|p| p[3] != 0));
    }

    #[test]
    fn loads_background_frames_with_no_and_tile_mode() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let path = root
            .join("Map")
            .join("Map")
            .join("Map0")
            .join("Map0_000.wz");
        if !path.exists() {
            return;
        }
        let archive = wz_core::WzArchive::open(&path).unwrap();
        let tree = archive.read_tree().unwrap();
        let entry = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("000010000.img"))
            .expect("000010000.img");
        let node = archive.parse_image_at(entry.offset.unwrap()).unwrap();
        let map = wz_db::MapData::parse_from_node(&node);

        let layer = load_background_layer(&root, &map.backgrounds[1]).expect("tile background");
        assert_eq!(layer.cx, 0);
        assert_eq!(layer.tile_mode, 4);
        assert_eq!(layer.rx, -1);
        assert!(!layer.frames.is_empty());
        assert!(layer.width() > 0 && layer.height() > 0);

        let tiled = load_background_layer(&root, &map.backgrounds[7]).expect("cx background");
        assert_eq!(tiled.cx, 1200);
        assert_eq!(tiled.tile_mode, 1);
    }

    #[test]
    fn loads_particle_desc_and_texture() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let name = "adventure2022_tree_burn_00_Fog2";
        let Some(node) = find_particle_desc_node(&root, name) else {
            return;
        };
        let desc = wz_db::particle::parse_particle_desc(&node);
        assert_eq!(desc.total_particle, 10);
        assert_eq!(desc.life, 2.0);
        let outlink = desc.texture.outlink.clone().expect("outlink");
        let Some((w, h, rgba)) = load_particle_texture(&root, &outlink) else {
            return;
        };
        assert!(w > 1 && h > 1);
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        let layer = crate::particles::MapParticleLayer {
            desc,
            texture: (w, h, rgba),
            origin: (0, 0),
            points: vec![(-825, -167)],
            z: 0,
            rx: -100,
            ry: -100,
        };
        let sprites = crate::particles::sample_layer(&layer, 0.5, [0.0, 0.0]);
        assert_eq!(sprites.len(), 10);
    }

    #[test]
    fn loads_reactor_stages_and_hit() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let reactor = wz_db::MapReactor {
            index: 0,
            id: 1002009,
            x: -302,
            y: 253,
            ..Default::default()
        };
        let Some(view) = load_reactor_view(&root, &reactor) else {
            return;
        };
        assert_eq!(view.id, 1002009);
        // 이 리액터는 stage 0/3 이 hit 프레임만 가진다.
        assert!(view.hits.contains_key(&0));
        let hit = &view.hits[&0];
        assert_eq!(hit.frames.len(), 2);
        assert_eq!((hit.frames[0].width, hit.frames[0].height), (64, 45));
        assert!(hit.frames[0].rgba.chunks(4).any(|p| p[3] != 0));

        let visible = wz_db::MapReactor {
            index: 1,
            id: 100000,
            x: 0,
            y: 0,
            ..Default::default()
        };
        if let Some(view) = load_reactor_view(&root, &visible) {
            let stage0 = view.stages.get(&0).expect("stage 0");
            assert_eq!((stage0.frames[0].width, stage0.frames[0].height), (29, 35));
        }
    }
}
