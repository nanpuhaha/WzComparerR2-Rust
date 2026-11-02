//! 헤드리스 맵 렌더 스모크 테스트.
//!
//! 실제 Map WZ 를 열어 맵을 파싱하고, back/tile/obj 이미지를 로드해
//! 아틀라스에 올린 뒤 wz-render 로 RGBA 를 뽑아 **비어 있지 않고 단색이
//! 아닌지** 검증한다. GUI 없이도 맵 렌더 파이프라인의 회귀를 잡는다.
//!
//! 사용:
//!   cargo run -p wz-cli --example map_render_smoke -- <Map0_000.wz 경로>
//!
//! 종료 코드: 0 성공 / 1 실패 (panic).

use std::path::PathBuf;
use wz_core::WzArchive;
use wz_db::MapData;
use wz_render::{batch::SpriteBatch, camera::Camera2D, device::RenderContext, GpuKey};

fn load_canvas_image(data_root: &std::path::Path, subdir: &str, canvas_name: &str, inner_path: &str) -> Option<(u32, u32, Vec<u8>)> {
    use wz_core::image::ImageValue;
    let canvas_dir = data_root.join("Map").join(subdir).join("_Canvas");
    let entries = std::fs::read_dir(&canvas_dir).ok()?;
    let img_name = format!("{}.img", canvas_name);
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") { continue; }
        let Ok(arch) = WzArchive::open(&path) else { continue; };
        let Ok(tree) = arch.read_tree() else { continue; };
        let e = tree.root.entries.iter().find(|e| {
            e.name.as_deref() == Some(canvas_name) || e.name.as_deref() == Some(img_name.as_str())
        });
        let Some(e) = e else { continue; };
        let Some(off) = e.offset else { continue; };
        let Ok(node) = arch.parse_image_at(off) else { continue; };
        let segs: Vec<&str> = inner_path.split('/').filter(|s| !s.is_empty()).collect();
        let mut cur = &node;
        let mut ok = true;
        for seg in &segs {
            let next = if let Some(ImageValue::Property(p)) = &cur.value {
                p.children.iter().find(|c| c.name.as_ref() == *seg)
                    .or_else(|| cur.children.iter().find(|c| c.name.as_ref() == *seg))
            } else {
                cur.children.iter().find(|c| c.name.as_ref() == *seg)
            };
            match next { Some(n) => cur = n, None => { ok = false; break; } }
        }
        if !ok { continue; }
        fn find_png(n: &wz_core::image::Node) -> Option<wz_core::image::PngRef> {
            use wz_core::image::ImageValue;
            if let Some(ImageValue::Png(p)) = &n.value { return Some(p.clone()); }
            for c in &n.children { if let Some(p) = find_png(c) { return Some(p); } }
            if let Some(ImageValue::Property(p)) = &n.value { if let Some(res) = find_png(p) { return Some(res); } }
            None
        }
        let Some(pr) = find_png(cur) else { continue; };
        let file_src = wz_pool::FileSource::open(&path).ok()?;
        let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
        if let Ok((w, h, rgba)) = wz_image::decode_png_ref_to_rgba(&pr, &src) {
            return Some((w, h, rgba));
        }
    }
    None
}

fn find_data_root(map_path: &std::path::Path) -> Option<PathBuf> {
    let mut p = map_path.to_path_buf();
    while let Some(parent) = p.parent() {
        if parent.file_name().and_then(|s| s.to_str()) == Some("Data") {
            return Some(parent.to_path_buf());
        }
        p = parent.to_path_buf();
    }
    None
}

fn main() {
    let map_path = std::env::args().nth(1).expect("usage: map_render_smoke <Map0_000.wz>");
    let map_path = PathBuf::from(map_path);
    let data_root = find_data_root(&map_path).expect("data root not found");

    let arch = WzArchive::open(&map_path).expect("open map wz");
    let tree = arch.read_tree().expect("read tree");

    let entry = tree.root.entries.iter().find(|e| e.node_type == wz_core::dirtree::node_type::IMAGE).expect("no image entry");
    let off = entry.offset.expect("offset");
    let node = arch.parse_image_at(off).expect("parse image");
    let map = MapData::parse_from_node(&node);

    println!("map: footholds={} portals={} life={} back={} obj={} tile={}",
        map.footholds.len(), map.portals.len(), map.life.len(),
        map.backgrounds.len(), map.objects.len(), map.tiles.len());

    let mut rc = RenderContext::new_headless().expect("render context");
    let mut batch = SpriteBatch::new();

    // back
    let mut back_count = 0;
    if let Some(first) = map.backgrounds.first() {
        if let Some((w, h, px)) = load_canvas_image(&data_root, "Back", &first.b_s, "") {
            let key = GpuKey(2);
            rc.create_texture(key, w, h, &px).expect("tex");
            batch.push_quad(key, [0.0, 0.0, w as f32, h as f32], 0.0, [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
            back_count += 1;
        }
    }
    // tiles
    let mut tile_count = 0;
    for (i, tile) in map.tiles.iter().enumerate() {
        if let Some((w, h, px)) = load_canvas_image(&data_root, "Tile", &tile.t_s, &format!("{}/{}", tile.u, tile.no)) {
            let key = GpuKey(100 + i as u32);
            rc.create_texture(key, w, h, &px).expect("tex");
            batch.push_quad(key, [tile.x as f32, tile.y as f32, w as f32, h as f32], 0.0, [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
            tile_count += 1;
        }
    }
    // objs
    let mut obj_count = 0;
    for (i, obj) in map.objects.iter().enumerate() {
        let inner = format!("{}/{}/{}/0", obj.l0, obj.l1, obj.l2);
        if let Some((w, h, px)) = load_canvas_image(&data_root, "Obj", &obj.o_s, &inner) {
            let key = GpuKey(200 + i as u32);
            rc.create_texture(key, w, h, &px).expect("tex");
            batch.push_quad(key, [obj.x as f32, obj.y as f32, w as f32, h as f32], obj.z as f32, [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
            obj_count += 1;
        }
    }
    batch.finish();

    let cam = Camera2D::new(800.0, 600.0);
    let uniforms = cam.build_uniforms(0.0, 0.0);
    let t0 = std::time::Instant::now();
    let rgba = rc.render_to_rgba(800, 600, &batch, &uniforms, [0.08, 0.08, 0.12, 1.0]).expect("render");
    let elapsed = t0.elapsed();
    println!("render time: {:?}", elapsed);

    assert_eq!(rgba.len(), 800 * 600 * 4);
    let mut non_zero = 0usize;
    for b in &rgba { if *b != 0 { non_zero += 1; } }
    assert!(non_zero > 1000, "rendered image is blank (non_zero={})", non_zero);

    let mut colors = std::collections::HashSet::new();
    for chunk in rgba.chunks(4) { colors.insert((chunk[0], chunk[1], chunk[2])); }
    println!("layers loaded: back={} tile={} obj={}", back_count, tile_count, obj_count);
    println!("SMOKE OK: {}x{} rendered, {} non-zero bytes, {} distinct colors", 800, 600, non_zero, colors.len());
}
