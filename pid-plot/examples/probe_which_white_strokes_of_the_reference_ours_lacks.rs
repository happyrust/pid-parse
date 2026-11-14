//! Which strokes does a reference plot paint in the paper's colour where our
//! page paints none in it?
//!
//! The vendor's plots stroke a label's origin mark in white, where it stands
//! among the sheet's objects, and our page does the same (`write.rs`,
//! `plot_ink`). The fourth review counted 462 white pieces on the eleven
//! reference plots and found 14 with no white piece of ours at their place.
//! This lists them. For every drawing of the corpus with a plot beside it,
//! both pages are read as the order they are painted in
//! ([`read_plot_sequence`]), and each white stroked piece of the reference
//! is looked for on ours: white ink of ours at its place — a white piece
//! that passes within 0.3 mm of the reference piece's middle
//! (`PID_WHITE_NEAR_MM` for another reach). The two
//! plotters cut the same mark into different pieces (462 on the reference
//! against our 261: the vendor's driver strokes a polyline's last unit as
//! a piece of its own, and a mark a few hundredths of a millimetre long as
//! one unit), so pieces are not paired one to one and not by direction.
//!
//! One line a sheet — white pieces on the reference, on ours, and the
//! reference's with no partner — and under it one line for each of those:
//! where it lies, how long and on what pen, what our page paints at that
//! place (pieces in ink, pieces in white of another length), and the
//! primitives of our page model that pass through it, with the record each
//! was laid out from.
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_white_strokes_of_the_reference_ours_lacks -- <corpus>
//! ```
//!
//! `<corpus>` is the acceptance corpus; faces come from `PID_PLOT_FONTS` or
//! the corpus's own `fonts`. Our page is written to memory. Read-only.
//!
//! Over the eleven sheets it reads 462 white pieces on the references and
//! 261 on ours. With the reach at 0.25 mm or more, every one of the
//! reference's has white of ours at its place but those of one mark of the
//! A2 sheet (six pieces at 0.3 mm, seven at 0.25), which belongs to the
//! instrument cluster that sheet's reference shows somewhere else. Pulled in
//! to 0.2 mm, ten more pieces on six sheets are left without; at 0.17 mm,
//! 42. Every one of them is one device unit long, 0.169 mm, and lies off
//! the end of an arm of a label's origin mark — the white 0.01 mm lines of a
//! `Remarks.sym` or `Item Note & Label.sym` — which our page strokes in
//! white at the 1.270 or 2.540 mm the symbol declares and the vendor's
//! driver strokes at whole device units and then one unit more as a piece
//! of its own. So the count is the reach's, not a record's: no mark is
//! missing and none is in another colour.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_plot::fonts::{font_dirs, prepare_fonts};
use pid_plot::model::{PaperModel, PaperShape, lay_out_drawing};
use pid_plot::read::{InkSegment, PaintedStroke, read_plot_sequence};
use pid_plot::write::{PlotFont, write_plot};

/// How far from a piece's middle ink may lie and still be at its place,
/// unless `PID_WHITE_NEAR_MM` says otherwise.
const NEAR_MM: f64 = 0.3;

fn near_mm() -> f64 {
    std::env::var("PID_WHITE_NEAR_MM")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value: &f64| value.is_finite() && *value > 0.0)
        .unwrap_or(NEAR_MM)
}

fn paired_sheets(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pid"))
                && path.with_extension("pdf").is_file()
        })
        .collect();
    paths.sort();
    paths
}

fn middle(segment: &InkSegment) -> [f64; 2] {
    [
        (segment.start_mm[0] + segment.end_mm[0]) / 2.0,
        (segment.start_mm[1] + segment.end_mm[1]) / 2.0,
    ]
}

fn length(segment: &InkSegment) -> f64 {
    (segment.end_mm[0] - segment.start_mm[0]).hypot(segment.end_mm[1] - segment.start_mm[1])
}

fn apart(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// How near `point` passes to the straight piece `a`–`b`.
fn off_the_piece(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let direction = [b[0] - a[0], b[1] - a[1]];
    let squared = direction[0] * direction[0] + direction[1] * direction[1];
    if squared < 1e-18 {
        return apart(point, a);
    }
    let along = (((point[0] - a[0]) * direction[0] + (point[1] - a[1]) * direction[1]) / squared)
        .clamp(0.0, 1.0);
    apart(
        point,
        [a[0] + along * direction[0], a[1] + along * direction[1]],
    )
}

/// The primitives of the model that pass within `near` of `point`.
fn passing(model: &PaperModel, point: [f64; 2], near: f64) -> Vec<String> {
    let mut found = Vec::new();
    for primitive in &model.primitives {
        let near = match &primitive.shape {
            PaperShape::Line { start_mm, end_mm } => {
                off_the_piece(point, *start_mm, *end_mm) <= near
            }
            PaperShape::Polyline { points_mm, .. } => points_mm
                .windows(2)
                .any(|pair| off_the_piece(point, pair[0], pair[1]) <= near),
            PaperShape::Arc {
                center_mm,
                radius_mm,
                ..
            }
            | PaperShape::Circle {
                center_mm,
                radius_mm,
            } => (apart(point, *center_mm) - radius_mm).abs() <= near,
            PaperShape::Text { origin_mm, .. } => apart(point, *origin_mm) <= 2.0,
            PaperShape::Fill { .. } => false,
        };
        if !near {
            continue;
        }
        let what = match &primitive.shape {
            PaperShape::Line { .. } => "line".to_string(),
            PaperShape::Polyline { points_mm, .. } => format!("polyline of {}", points_mm.len()),
            PaperShape::Arc { .. } => "arc".to_string(),
            PaperShape::Circle { .. } => "circle".to_string(),
            PaperShape::Text { value, .. } => format!("label {value:?} (origin within 2 mm)"),
            PaperShape::Fill { .. } => "fill".to_string(),
        };
        let pen = primitive.stroke.as_ref().map_or_else(
            || "no pen".to_string(),
            |pen| format!("pen {:.4} mm colour {:?}", pen.width_mm, pen.rgb),
        );
        let record = match (&primitive.stream_path, primitive.byte_range) {
            (Some(stream), Some(range)) => format!("{stream} +{}", range[0]),
            _ => "no record".to_string(),
        };
        found.push(format!(
            "{}{what}, {pen}{} [{record}]",
            if primitive.furniture { "template " } else { "" },
            primitive
                .symbol
                .as_ref()
                .map_or_else(String::new, |symbol| format!(", of {symbol}")),
        ));
    }
    found
}

fn main() -> ExitCode {
    let Some(corpus) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: probe_which_white_strokes_of_the_reference_ours_lacks <corpus>");
        return ExitCode::from(2);
    };
    let sheets = paired_sheets(&corpus);
    if sheets.is_empty() {
        eprintln!("{}: no .pid with a plot beside it", corpus.display());
        return ExitCode::from(2);
    }
    let dirs = font_dirs(Some(&corpus));
    let near = near_mm();
    let (mut on_reference, mut on_ours, mut lacking) = (0usize, 0usize, 0usize);
    for path in &sheets {
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let reference = std::fs::read(path.with_extension("pdf"))
            .map_err(|error| error.to_string())
            .and_then(|bytes| read_plot_sequence(&bytes).map_err(|error| error.to_string()));
        let model = lay_out_drawing(path, &corpus).map_err(|error| error.to_string());
        let (reference, model) = match (reference, model) {
            (Ok(reference), Ok(model)) => (reference, model),
            (Err(error), _) | (_, Err(error)) => {
                println!("{name} | not read: {error}");
                continue;
            }
        };
        let prepared = match prepare_fonts(&[&model], &dirs) {
            Ok(prepared) => prepared,
            Err(error) => {
                println!("{name} | no faces: {error}");
                continue;
            }
        };
        let fonts: Vec<PlotFont<'_>> = prepared
            .iter()
            .map(|face| PlotFont {
                key: &face.key,
                bytes: &face.bytes,
                subset: face.subset,
            })
            .collect();
        let ours = match write_plot(&model, &fonts)
            .map_err(|error| error.to_string())
            .and_then(|pdf| read_plot_sequence(&pdf).map_err(|error| error.to_string()))
        {
            Ok(ours) => ours,
            Err(error) => {
                println!("{name} | our page: {error}");
                continue;
            }
        };
        let white = |stroke: &&PaintedStroke| stroke.paper && !stroke.filled;
        let theirs: Vec<&PaintedStroke> = reference.strokes.iter().filter(white).collect();
        let mine: Vec<&PaintedStroke> = ours.strokes.iter().filter(white).collect();
        let alone: Vec<&PaintedStroke> = theirs
            .iter()
            .copied()
            .filter(|stroke| {
                !mine.iter().any(|other| {
                    off_the_piece(
                        middle(&stroke.segment),
                        other.segment.start_mm,
                        other.segment.end_mm,
                    ) <= near
                })
            })
            .collect();
        on_reference += theirs.len();
        on_ours += mine.len();
        lacking += alone.len();
        println!(
            "{name} | white pieces: reference {}, ours {} | {} of the reference's with none of \
             ours' white at its place",
            theirs.len(),
            mine.len(),
            alone.len()
        );
        for stroke in alone {
            let at = middle(&stroke.segment);
            let here = |strokes: &[PaintedStroke], paper: bool| {
                strokes
                    .iter()
                    .filter(|other| {
                        other.paper == paper
                            && !other.filled
                            && off_the_piece(at, other.segment.start_mm, other.segment.end_mm)
                                <= near.max(NEAR_MM)
                    })
                    .map(|other| {
                        format!(
                            "{:.3} mm on {:.3} at step {}",
                            length(&other.segment),
                            other.segment.width_mm,
                            other.step
                        )
                    })
                    .collect::<Vec<_>>()
            };
            println!(
                "   ({:.3}, {:.3})-({:.3}, {:.3}) mm, {:.3} mm long on {:.3}, step {}",
                stroke.segment.start_mm[0],
                stroke.segment.start_mm[1],
                stroke.segment.end_mm[0],
                stroke.segment.end_mm[1],
                length(&stroke.segment),
                stroke.segment.width_mm,
                stroke.step
            );
            println!(
                "      reference there: white {:?}; ink {:?}",
                here(&reference.strokes, true),
                here(&reference.strokes, false)
            );
            println!(
                "      ours there: white {:?}; ink {:?}",
                here(&ours.strokes, true),
                here(&ours.strokes, false)
            );
            for primitive in passing(&model, at, near.max(NEAR_MM)) {
                println!("      model: {primitive}");
            }
        }
    }
    println!(
        "in all: {} sheet(s); white pieces: reference {on_reference}, ours {on_ours}; {lacking} \
         of the reference's with none of ours' white within {near} mm of its middle",
        sheets.len()
    );
    ExitCode::SUCCESS
}
