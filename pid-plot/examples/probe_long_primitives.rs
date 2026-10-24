//! The longest paper primitives of one drawing, with the stream, byte range and
//! symbol each came from, so a stroke that runs across the sheet can be traced
//! back to the record that asked for it.
//!
//! ```text
//! cargo run -p pid-plot --example probe_long_primitives -- "<drawing.pid>" [min_mm]
//! ```
//!
//! Read-only.

use std::path::PathBuf;

use pid_plot::model::{PaperShape, lay_out_drawing};

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn length_of(shape: &PaperShape) -> f64 {
    match shape {
        PaperShape::Line { start_mm, end_mm } => {
            (end_mm[0] - start_mm[0]).hypot(end_mm[1] - start_mm[1])
        }
        PaperShape::Polyline { points_mm, .. } => points_mm
            .windows(2)
            .map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]))
            .sum(),
        PaperShape::Arc {
            radius_mm,
            start_angle,
            end_angle,
            ..
        } => {
            let sweep = (end_angle - start_angle).abs().min(std::f64::consts::TAU);
            radius_mm * sweep
        }
        _ => 0.0,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pid = std::env::args()
        .nth(1)
        .expect("usage: <drawing.pid> [min_mm]");
    let min_mm: f64 = std::env::args()
        .nth(2)
        .map_or(100.0, |value| value.parse().expect("min_mm"));
    let model = lay_out_drawing(&PathBuf::from(&pid), &corpus_dir())?;
    let mut rows: Vec<(f64, usize, String)> = model
        .primitives
        .iter()
        .enumerate()
        .filter_map(|(index, primitive)| {
            let length = length_of(&primitive.shape);
            if length < min_mm {
                return None;
            }
            let where_from = format!(
                "{} {} {:?} {:?}",
                primitive.confidence,
                primitive.stream_path.as_deref().unwrap_or("-"),
                primitive.byte_range,
                primitive.symbol.as_deref().unwrap_or("-"),
            );
            Some((length, index, where_from))
        })
        .collect();
    rows.sort_by(|left, right| right.0.total_cmp(&left.0));
    println!(
        "{} primitives, {} over {min_mm} mm",
        model.primitives.len(),
        rows.len()
    );
    for (length, index, where_from) in rows.iter().take(20) {
        let shape = &model.primitives[*index].shape;
        let ends = match shape {
            PaperShape::Line { start_mm, end_mm } => {
                format!(
                    "({:.1},{:.1}) -> ({:.1},{:.1})",
                    start_mm[0], start_mm[1], end_mm[0], end_mm[1]
                )
            }
            PaperShape::Polyline { points_mm, .. } => points_mm.first().map_or_else(
                || "-".to_string(),
                |first| {
                    let last = points_mm.last().expect("non-empty");
                    format!(
                        "poly {} points ({:.1},{:.1}) -> ({:.1},{:.1})",
                        points_mm.len(),
                        first[0],
                        first[1],
                        last[0],
                        last[1]
                    )
                },
            ),
            other => format!("{other:?}"),
        };
        println!("  {length:8.1} mm  #{index}  {ends}  [{where_from}]");
    }
    Ok(())
}
