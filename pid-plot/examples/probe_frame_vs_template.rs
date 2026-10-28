//! Does a sheet's border frame name the template it wears — and does the pick
//! read that name?
//!
//! A P&ID's border is a linked OLE object: the sheet carries an `igSmartFrame2d`
//! rectangle (which `build_normalized_geometry` reports as `page_dimensions_mm`)
//! and a link record naming the template file. `load_border_template` measures
//! each `.igr`'s ink extent and accepts an unnamed sheet within
//! `TEMPLATE_EXTENT_TOLERANCE_MM` (2 mm); a sheet the link names gets
//! `TEMPLATE_NAMED_TOLERANCE_MM` (10 mm), because the name already says which
//! template it is.
//!
//! This probe prints both statements side by side, per drawing: the frame
//! extent, every link's leaf name, the `.igr` the pick actually wore, and the
//! drawing's own ink with the furniture excluded — so a candidate the frame
//! almost fits can be checked against the content it would enclose.
//!
//! It settled §2.2: the two A1 pages over the 2 mm window (`…(1200td)` at
//! 843.09 × 596.09 and `N井场` at 843.46 × 596.71) both carry a `.PID` link whose
//! stem names a template on disk, and `linked_template_prefix` used to accept
//! `.igr` only — so their names were never read. Four of the 53 backup sheets
//! link `.PID` only; on the 47 sheets that wore furniture then, the link stem
//! matched the file the pick chose 47/47. Both now wear their template (49 in
//! all). See `docs/analysis/2026-10-29-the-two-a1-pages-name-their-template.md`.
//!
//! ```text
//! cargo run -p pid-plot --example probe_frame_vs_template -- <a.pid|dir> ...
//! ```

use std::path::{Path, PathBuf};

use pid_parse::{ParseOptions, PidParser, build_normalized_geometry};
use pid_plot::model::{PaperShape, lay_out_drawing};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn collect_pids(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_pids(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "pid") {
            out.push(path);
        }
    }
}

fn leaf(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

fn shape_points(shape: &PaperShape) -> Vec<[f64; 2]> {
    match shape {
        PaperShape::Line { start_mm, end_mm } => vec![*start_mm, *end_mm],
        PaperShape::Arc {
            center_mm,
            radius_mm,
            ..
        }
        | PaperShape::Circle {
            center_mm,
            radius_mm,
        } => vec![
            [center_mm[0] - radius_mm, center_mm[1] - radius_mm],
            [center_mm[0] + radius_mm, center_mm[1] + radius_mm],
        ],
        PaperShape::Polyline { points_mm, .. } => points_mm.clone(),
        PaperShape::Fill { chain_mm, .. } => chain_mm.clone(),
        PaperShape::Text { origin_mm, .. } => vec![*origin_mm],
    }
}

fn main() {
    let mut paths = Vec::new();
    for arg in std::env::args().skip(1) {
        let path = PathBuf::from(arg);
        if path.is_dir() {
            collect_pids(&path, &mut paths);
        } else {
            paths.push(path);
        }
    }
    let corpus = corpus_dir();
    println!(
        "name|page_w|page_h|frame_state|frame_w|frame_h|links|picked|picked_w|picked_h|template_prims|content_max_x|content_max_y|primitives|missing"
    );
    for path in &paths {
        let name = path.file_stem().map_or_else(
            || path.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        let Ok(doc) = PidParser::with_options(ParseOptions::geometry()).parse_file(path) else {
            println!("{name}|parse failed");
            continue;
        };
        let geometry = build_normalized_geometry(&doc);
        let (page_w, page_h) = geometry.page_dimensions_mm.unwrap_or((0.0, 0.0));
        let frames: Vec<_> = doc
            .sheet_streams
            .iter()
            .filter_map(|stream| stream.geometry.as_ref())
            .flat_map(|geometry| &geometry.decoded_igsmartframes)
            .collect();
        let frame = frames.first();
        let frame_state = frame.map_or("-".to_string(), |frame| format!("{:?}", frame.state));
        let frame_w = frame.map_or(0.0, |frame| frame.extent_width_m * 1000.0);
        let frame_h = frame.map_or(0.0, |frame| frame.extent_height_m * 1000.0);
        let links: Vec<String> = doc
            .jsites
            .iter()
            .flat_map(|site| site.ole_links.iter().map(|link| leaf(link).to_string()))
            .collect();
        let model = lay_out_drawing(path, &corpus).ok();
        let (picked, picked_w, picked_h, template_prims, missing) = model.as_ref().map_or_else(
            || ("-".to_string(), 0.0, 0.0, 0, 0),
            |model| {
                model.template.as_ref().map_or(
                    ("-".to_string(), 0.0, 0.0, 0, model.missing_symbols.len()),
                    |template| {
                        (
                            leaf(&template.path).to_string(),
                            template.extent_mm[0],
                            template.extent_mm[1],
                            template.primitives,
                            model.missing_symbols.len(),
                        )
                    },
                )
            },
        );
        let mut max = [f64::MIN; 2];
        let mut primitives = 0usize;
        if let Some(model) = &model {
            primitives = model.primitives.len();
            for primitive in &model.primitives {
                if primitive.furniture {
                    continue;
                }
                for point in shape_points(&primitive.shape) {
                    if point[0].is_finite() && point[1].is_finite() {
                        max[0] = max[0].max(point[0]);
                        max[1] = max[1].max(point[1]);
                    }
                }
            }
        }
        println!(
            "{name}|{page_w:.2}|{page_h:.2}|{frame_state}|{frame_w:.2}|{frame_h:.2}|{}|{picked}|{picked_w:.2}|{picked_h:.2}|{template_prims}|{:.2}|{:.2}|{primitives}|{missing}",
            links.join(";"),
            max[0],
            max[1]
        );
    }
}
