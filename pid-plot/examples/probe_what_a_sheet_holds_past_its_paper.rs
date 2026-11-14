//! What does a sheet hold past the paper it states?
//!
//! A sheet's `0x0114 JSheet` record states its paper, and the eleven
//! reference plots are printed on exactly that paper
//! (`docs/analysis/2026-11-11-a-sheet-states-its-paper.md`). A border frame
//! may place its template off the paper's corner — the nine
//! `CPECCHB-SQ-A2` sheets by (0.388, 0.566) mm, `N井场` by (2.46, 2.71) mm —
//! and what the template then holds past the paper's top and right edges is
//! on no plot. This prints, for every drawing it is given:
//!
//! | column | what |
//! |---|---|
//! | `paper` | the paper the sheet's record states, millimetres |
//! | `page` | the page the layout writes ([`PaperModel::resolved_page_mm`]) |
//! | `frame` | where the border frame puts the template: its offset, and the far corner the template reaches there |
//! | `past` | primitives with a point past the paper by more than the slack: the template's, the drawing's |
//! | `off` | of those, the ones no point of which is on the paper |
//!
//! and under it one line for each such primitive: whose it is, what it is,
//! how far past each edge it reaches, the record it was laid out from, and
//! — for a stroke — which of its pieces lie wholly off the paper. A label is
//! read by its insertion point, the only place its shape carries.
//!
//! ```text
//! cargo run -p pid-plot --example probe_what_a_sheet_holds_past_its_paper -- <corpus> <drawing.pid | directory> [...]
//! ```
//!
//! `<corpus>` is the directory the drawings are laid out against, as their
//! gate lays them out: the package for the plant backup, `test-file` for
//! the fixtures, the acceptance corpus for the reference sheets. A
//! directory is walked for `.pid` files. `PID_PAST_THE_PAPER_SLACK_MM`
//! (default 0.01) is the slack. Read-only.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_plot::model::{PaperModel, PaperPrimitive, PaperShape, lay_out_drawing};

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for entry in entries {
            collect(&entry, out);
        }
    } else if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pid"))
    {
        out.push(path.to_path_buf());
    }
}

/// The paper every sheet of the drawing that states one agrees on,
/// `[width, height]` in millimetres.
fn stated_paper_mm(pid: &Path) -> Option<[f64; 2]> {
    let doc = pid_parse::PidParser::new().parse_file(pid).ok()?;
    let mut papers: Vec<[f64; 2]> = doc
        .sheet_display_order
        .values()
        .filter_map(|order| order.paper)
        .map(|paper| [paper.width_m * 1000.0, paper.height_m * 1000.0])
        .collect();
    papers.dedup();
    match papers.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

/// The chain of points a shape's ink runs through: a line's two ends, a
/// polyline's vertices and its closing edge, the box of a circle or an arc,
/// a fill's ring, a label's insertion point.
fn chain(shape: &PaperShape) -> Vec<[f64; 2]> {
    match shape {
        PaperShape::Line { start_mm, end_mm } => vec![*start_mm, *end_mm],
        PaperShape::Polyline { points_mm, closed } => {
            let mut points = points_mm.clone();
            if *closed && points.len() >= 2 {
                points.push(points[0]);
            }
            points
        }
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
        PaperShape::Fill { chain_mm, .. } => chain_mm.clone(),
        PaperShape::Text { origin_mm, .. } => vec![*origin_mm],
    }
}

/// How far `point` lies past the paper: `[left, bottom, right, top]`,
/// positive outside.
fn past(point: [f64; 2], paper: [f64; 2]) -> [f64; 4] {
    [
        -point[0],
        -point[1],
        point[0] - paper[0],
        point[1] - paper[1],
    ]
}

fn kind(shape: &PaperShape) -> String {
    match shape {
        PaperShape::Line { .. } => "line".into(),
        PaperShape::Polyline { points_mm, closed } => format!(
            "polyline of {} point(s){}",
            points_mm.len(),
            if *closed { ", closed" } else { "" }
        ),
        PaperShape::Arc { .. } => "arc".into(),
        PaperShape::Circle { .. } => "circle".into(),
        PaperShape::Fill { chain_mm, .. } => format!("fill of {} point(s)", chain_mm.len()),
        PaperShape::Text { value, .. } => format!("label {value:?}"),
    }
}

/// One primitive that reaches past the paper.
struct Past<'a> {
    primitive: &'a PaperPrimitive,
    /// The furthest any of its points lies past each edge.
    reach: [f64; 4],
    /// No point of it is on the paper.
    off: bool,
    /// Its straight pieces, and how many of them have both ends past the
    /// same edge.
    pieces: (usize, usize),
}

fn past_the_paper<'a>(model: &'a PaperModel, paper: [f64; 2], slack: f64) -> Vec<Past<'a>> {
    let mut out = Vec::new();
    for primitive in &model.primitives {
        let points: Vec<[f64; 2]> = chain(&primitive.shape)
            .into_iter()
            .filter(|point| point[0].is_finite() && point[1].is_finite())
            .collect();
        if points.is_empty() {
            continue;
        }
        let mut reach = [f64::NEG_INFINITY; 4];
        // The least any point lies past each edge: a shape is off the paper
        // when even its nearest point is past one.
        let mut least = [f64::INFINITY; 4];
        for point in &points {
            let here = past(*point, paper);
            for edge in 0..4 {
                reach[edge] = reach[edge].max(here[edge]);
                least[edge] = least[edge].min(here[edge]);
            }
        }
        if reach.iter().all(|value| *value <= slack) {
            continue;
        }
        let off = least.iter().any(|value| *value > slack);
        let stroked = matches!(
            primitive.shape,
            PaperShape::Line { .. } | PaperShape::Polyline { .. }
        );
        let pieces = if stroked {
            let total = points.len().saturating_sub(1);
            let beyond = points
                .windows(2)
                .filter(|pair| {
                    let (a, b) = (past(pair[0], paper), past(pair[1], paper));
                    (0..4).any(|edge| a[edge] > slack && b[edge] > slack)
                })
                .count();
            (total, beyond)
        } else {
            (0, 0)
        };
        out.push(Past {
            primitive,
            reach,
            off,
            pieces,
        });
    }
    out
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(corpus) = args.next().map(PathBuf::from) else {
        eprintln!(
            "usage: probe_what_a_sheet_holds_past_its_paper <corpus> <drawing.pid | directory> [...]"
        );
        return ExitCode::from(2);
    };
    let mut drawings = Vec::new();
    for arg in args {
        collect(Path::new(&arg), &mut drawings);
    }
    if drawings.is_empty() {
        eprintln!("no drawing given");
        return ExitCode::from(2);
    }
    let slack = std::env::var("PID_PAST_THE_PAPER_SLACK_MM")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.01);
    println!(
        "drawing | paper | page | page - paper | frame: offset, template reaches | past the paper by over {slack} mm: template, drawing | off the paper: template, drawing"
    );
    let (mut differ, mut with_ink_past) = (0usize, 0usize);
    for pid in &drawings {
        let name = pid
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let model = match lay_out_drawing(pid, &corpus) {
            Ok(model) => model,
            Err(error) => {
                println!("{name} | does not lay out: {error}");
                continue;
            }
        };
        let page = model.resolved_page_mm();
        let Some(paper) = stated_paper_mm(pid) else {
            println!("{name} | states no paper | page {page:?}");
            continue;
        };
        let frame = model.template.as_ref().map_or_else(
            || "no template".to_string(),
            |template| {
                format!(
                    "({:.3}, {:.3}), ({:.3}, {:.3})",
                    template.offset_mm[0],
                    template.offset_mm[1],
                    template.extent_mm[0] * template.scale + template.offset_mm[0],
                    template.extent_mm[1] * template.scale + template.offset_mm[1]
                )
            },
        );
        let found = past_the_paper(&model, paper, slack);
        let count = |furniture: bool, off: bool| {
            found
                .iter()
                .filter(|past| past.primitive.furniture == furniture && (!off || past.off))
                .count()
        };
        let (page_text, delta) = page.map_or_else(
            || ("none".to_string(), "-".to_string()),
            |page| {
                (
                    format!("{:.4} x {:.4}", page[0], page[1]),
                    format!("({:+.4}, {:+.4})", page[0] - paper[0], page[1] - paper[1]),
                )
            },
        );
        if page.is_some_and(|page| {
            (page[0] - paper[0]).abs() > 1e-6 || (page[1] - paper[1]).abs() > 1e-6
        }) {
            differ += 1;
        }
        if !found.is_empty() {
            with_ink_past += 1;
        }
        println!(
            "{name} | {} x {} | {page_text} | {delta} | {frame} | {}, {} | {}, {}",
            paper[0],
            paper[1],
            count(true, false),
            count(false, false),
            count(true, true),
            count(false, true)
        );
        for past in &found {
            let edges = ["left", "bottom", "right", "top"];
            let reach: Vec<String> = (0..4)
                .filter(|edge| past.reach[*edge] > slack)
                .map(|edge| format!("{} {:.3}", edges[edge], past.reach[edge]))
                .collect();
            let record = match (&past.primitive.stream_path, past.primitive.byte_range) {
                (Some(stream), Some(range)) => format!("{stream} +{}", range[0]),
                _ => "no record".to_string(),
            };
            let pieces = if past.pieces.0 > 0 {
                format!(
                    "; {} of its {} piece(s) wholly past an edge",
                    past.pieces.1, past.pieces.0
                )
            } else {
                String::new()
            };
            let pen = past
                .primitive
                .stroke
                .as_ref()
                .map_or_else(String::new, |pen| format!(", pen {:.4} mm", pen.width_mm));
            println!(
                "      {} {}{}{}: past {} mm; {}{}{}",
                if past.primitive.furniture {
                    "template"
                } else {
                    "drawing"
                },
                kind(&past.primitive.shape),
                pen,
                past.primitive
                    .symbol
                    .as_ref()
                    .map_or_else(String::new, |symbol| format!(" of {symbol}")),
                reach.join(", "),
                if past.off {
                    "off the paper"
                } else {
                    "crosses the edge"
                },
                pieces,
                format_args!(" [{record}]")
            );
        }
    }
    println!(
        "in all: {} drawing(s); {differ} whose page is not the paper; {with_ink_past} with a primitive past the paper",
        drawings.len()
    );
    ExitCode::SUCCESS
}
