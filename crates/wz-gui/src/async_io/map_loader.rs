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

fn load_reactor_image(
    root: &std::path::Path,
    id: i32,
    x: i32,
    y: i32,
    flip: bool,
) -> Option<(i32, i32, u32, u32, Vec<u8>)> {
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
        let png = node.first_canvas()?;
        let (w, h, mut rgba) = wz_image::decode_png_ref_to_rgba(&png, archive.source()).ok()?;
        if flip {
            rgba = flip_rgba_horizontal(&rgba, w, h);
        }
        let (ox, oy) = reactor_origin(&png);
        return Some((x - ox, y - oy, w, h, rgba));
    }
    None
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

fn load_background_layer(
    root: &std::path::Path,
    background: &wz_db::BackItem,
) -> Option<crate::background::BackgroundLayer> {
    let base = root.join("Map").join("Back").join("_Canvas");
    let image_name = format!("{}.img", background.b_s);
    let no = background.no.trim_matches('/');
    let mut candidates: Vec<Vec<&str>> = Vec::new();
    if background.ani == 1 {
        candidates.push(vec!["ani", no]);
    }
    candidates.push(vec!["back", no]);
    if no.is_empty() {
        candidates.clear();
        candidates.push(Vec::new());
    }

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
    None
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
    let map = MapData::parse_from_node(&node);

    let data_root = find_data_root(state.archive_path.as_deref());
    state.data_root = data_root.clone();

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
                if let Some(image) =
                    load_reactor_image(&root, reactor.id, reactor.x, reactor.y, reactor.flip)
                {
                    reactors.push(image);
                }
            }
            let mut backgrounds = Vec::new();
            for background in &backgrounds_clone {
                if let Some(layer) = load_background_layer(&root, background) {
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
            let _ = tx.send(crate::state::MapImages {
                tiles,
                objs,
                reactors,
                backgrounds,
                particles,
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
    let map = MapData::parse_from_node(&node);
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
    fn loads_reactor_image_from_canvas() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let Some((x, y, w, h, rgba)) = load_reactor_image(&root, 1002009, -302, 253, false) else {
            return;
        };
        assert!(w > 1 && h > 1);
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        assert!(x != 0 || y != 0);
    }
}
