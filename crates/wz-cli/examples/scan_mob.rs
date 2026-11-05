use std::path::Path;
use wz_core::image::Node;
use wz_core::WzArchive;

fn names(n: &Node, out: &mut Vec<String>) {
    for c in n.object_children() {
        out.push(c.name.to_string());
    }
}
fn main() {
    let base = Path::new("Data/Mob/_Canvas");
    for e in std::fs::read_dir(base).unwrap().flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(a) = WzArchive::open(&p) else { continue };
        let Ok(t) = a.read_tree() else { continue };
        for ent in &t.root.entries {
            if ent.node_type != 4 {
                continue;
            }
            let Some(off) = ent.offset else { continue };
            let Ok(n) = a.parse_image_at(off) else {
                continue;
            };
            let mut v = Vec::new();
            names(&n, &mut v);
            let has_atk = v.iter().any(|x| x.starts_with("attack"));
            let has_skill = v.iter().any(|x| x.starts_with("skill"));
            let has_fly = v.iter().any(|x| x == "fly");
            if (has_atk || has_skill) && has_fly {
                println!(
                    "{} {} atk={} skill={} fly={} names={:?}",
                    p.display(),
                    ent.name.as_deref().unwrap_or(""),
                    has_atk,
                    has_skill,
                    has_fly,
                    v
                );
                return;
            }
        }
    }
}
