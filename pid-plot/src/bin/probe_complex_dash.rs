//! Dump sheet entities whose resolved style stamps a complex dash, with
//! their authored point order and the resolved period/glyph.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use pid_parse::style_link::line_styles_for_document;
use pid_parse::{PidGraphicKind, PidParser, build_normalized_geometry};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: probe_complex_dash <drawing.pid> [x_min x_max y_min y_max mm]");
        return ExitCode::from(2);
    };
    let win: Vec<f64> = args.by_ref().take(4).filter_map(|a| a.parse().ok()).collect();
    let doc = match PidParser::new().parse_file(PathBuf::from(&path).as_path()) {
        Ok(doc) => doc,
        Err(error) => {
            eprintln!("{path}: {error}");
            return ExitCode::from(1);
        }
    };
    let styles = line_styles_for_document(&doc);
    let geometry = build_normalized_geometry(&doc);
    for entity in &geometry.entities {
        let Some(stream) = entity.source.stream_path.as_ref() else {
            continue;
        };
        let Some(oid) = entity.graphic_oid else { continue };
        let Some(style) = styles.get(&(stream.clone(), oid)) else {
            continue;
        };
        let Some(complex) = style.complex_dash.as_ref() else {
            continue;
        };
        let pts: Vec<[f64; 2]> = match &entity.kind {
            PidGraphicKind::Line { start, end } => {
                vec![[start.x * 1000.0, start.y * 1000.0], [end.x * 1000.0, end.y * 1000.0]]
            }
            PidGraphicKind::Polyline { points, .. } => points
                .iter()
                .map(|p| [p.x * 1000.0, p.y * 1000.0])
                .collect(),
            _ => continue,
        };
        if win.len() == 4 {
            let (x0, x1, y0, y1) = (win[0], win[1], win[2], win[3]);
            if !pts.iter().any(|p| p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1) {
                continue;
            }
        }
        println!(
            "{} oid={} period={:.3}mm strokes={} pts={:?}",
            entity.source.record_id.as_deref().unwrap_or("?"),
            oid,
            complex.period_mm(),
            complex.strokes().len(),
            pts
        );
        for (i, m) in complex.strokes().iter().enumerate() {
            println!(
                "    mark{i}: ({:.4},{:.4}) -> ({:.4},{:.4}) m",
                m.start.0, m.start.1, m.end.0, m.end.1
            );
        }
    }
    ExitCode::SUCCESS
}
