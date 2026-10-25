//! Every entity whose coordinates cannot possibly be on the page, with the
//! record it came from — the evidence needed to tell a misdecoded field from a
//! real off-sheet reference.
//!
//! ```text
//! cargo run -p pid-parse --example probe_absurd_geometry -- "<file.pid>" [limit_m]
//! ```
//!
//! `limit_m` defaults to 1000 (metres): a P&ID sheet is at most a few metres
//! across, so anything past that is a misread double rather than a coordinate.
//! Read-only.

use std::path::PathBuf;

use pid_parse::{PidGraphicKind, PidParser};

fn point_is_absurd(point: [f64; 2], limit: f64) -> bool {
    !point[0].is_finite()
        || !point[1].is_finite()
        || point[0].abs() > limit
        || point[1].abs() > limit
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pid = std::env::args()
        .nth(1)
        .expect("usage: <file.pid> [limit_m]");
    let limit: f64 = std::env::args()
        .nth(2)
        .map_or(1000.0, |value| value.parse().expect("limit_m"));
    let document = PidParser::new().parse_file(PathBuf::from(&pid).as_path())?;
    let geometry = pid_parse::build_normalized_geometry(&document);
    let mut found = 0usize;
    for entity in &geometry.entities {
        let points: Vec<[f64; 2]> = match &entity.kind {
            PidGraphicKind::Line { start, end } => vec![[start.x, start.y], [end.x, end.y]],
            PidGraphicKind::Point { position } => vec![[position.x, position.y]],
            PidGraphicKind::Polyline { points, .. } => {
                points.iter().map(|point| [point.x, point.y]).collect()
            }
            PidGraphicKind::Text { insertion, .. } => vec![[insertion.x, insertion.y]],
            PidGraphicKind::SymbolInstance { insertion, .. } => vec![[insertion.x, insertion.y]],
            _ => Vec::new(),
        };
        if !points.iter().any(|point| point_is_absurd(*point, limit)) {
            continue;
        }
        found += 1;
        println!(
            "{} {:?} from {} bytes {:?} kind {:?} oid {:?}",
            entity.id,
            entity.confidence,
            entity.source.stream_path.as_deref().unwrap_or("-"),
            entity.source.byte_range,
            entity.source.record_kind,
            entity.graphic_oid,
        );
        println!("    points {points:?}");
        if let Some(note) = entity.source.note.as_deref() {
            println!("    note {note}");
        }
    }
    println!("{found} absurd entities of {}", geometry.entities.len());
    Ok(())
}
