use std::path::Path;
use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

fn find(n: &Node, out: &mut Vec<String>) {
    if let Some(ImageValue::Str(s)) = &n.value {
        if s.as_ref() == "2024BlueholeExploration" {
            out.push(s.to_string());
        }
    }
    for c in n.object_children() {
        find(c, out);
    }
}
fn scan(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            scan(&p);
            continue;
        }
        if p.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(a) = WzArchive::open(&p) else {
            continue;
        };
        let Ok(t) = a.read_tree() else {
            continue;
        };
        for ent in &t.root.entries {
            if ent.node_type != 4 {
                continue;
            }
            let Some(off) = ent.offset else {
                continue;
            };
            let Ok(n) = a.parse_image_at(off) else {
                continue;
            };
            let mut hits = Vec::new();
            find(&n, &mut hits);
            if !hits.is_empty() {
                println!("{} {}", p.display(), ent.name.as_deref().unwrap_or(""));
            }
        }
    }
}
fn main() {
    scan(Path::new("Data/Map/Map"));
}
