pub mod avatar;
pub mod common;
pub mod diff;
pub mod expr;
pub mod image;
pub mod mapscene;
pub mod patch;
pub mod stats;
pub mod timeline;
pub mod tooltip;
pub mod tree;

pub use avatar::*;
pub use common::*;
pub use diff::*;
pub use expr::*;
pub use image::*;
pub use mapscene::*;
pub use patch::*;
pub use stats::*;
pub use timeline::*;
pub use tooltip::*;
pub use tree::*;

use crate::OracleAction;

pub fn cmd_oracle(action: OracleAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        OracleAction::Tree { wz, filter } => cmd_oracle_tree(&wz, filter.as_deref()),
        OracleAction::Diff { base, r#mod } => cmd_oracle_diff(&base, &r#mod),
        OracleAction::Image { wz, path, format } => cmd_oracle_image(&wz, &path, &format),
        OracleAction::Tooltip {
            wz,
            item_wz,
            etc_wz,
            domain,
            item,
        } => cmd_oracle_tooltip(
            &wz,
            item,
            item_wz.as_deref(),
            etc_wz.as_deref(),
            domain.as_deref(),
        ),
        OracleAction::Stats { wz, gear, prop } => cmd_oracle_stats(&wz, &gear, prop.as_deref()),
        OracleAction::Expr { expr, args } => cmd_oracle_expr(&expr, &args),
        OracleAction::Patch { patch } => cmd_oracle_patch(&patch),
        OracleAction::MapScene { wz, map } => cmd_oracle_mapscene(&wz, &map),
        OracleAction::Timeline {
            wz,
            path,
            node,
            canvas_wz,
            interpol_ms,
        } => cmd_oracle_timeline(
            &wz,
            path.as_deref(),
            &node,
            canvas_wz.as_deref(),
            interpol_ms,
        ),
        OracleAction::Avatar {
            body_wz,
            body,
            canvas_wz,
            zmap_wz,
            zmap,
            action,
            frame,
            elapsed,
        } => cmd_oracle_avatar(&AvatarParams {
            body_wz: &body_wz,
            body_img: body.as_deref(),
            canvas_wz: &canvas_wz,
            zmap_wz: &zmap_wz,
            zmap_img: zmap.as_deref(),
            action: &action,
            frame,
            elapsed,
        }),
    }
}
