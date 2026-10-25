//! Per-sheet igBoundary2d counts in W.pid, and which sheet(s) carry the title
//! block — the logo outlines we already draw must be findable as entities.

use pid_parse::{PidGraphicKind, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    // Sheet inventory: entities + boundary-ish (closed plines) per stream path
    let mut by_stream: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for e in &geometry.entities {
        let sp = e.source.stream_path.clone().unwrap_or_default();
        let closed = matches!(&e.kind, PidGraphicKind::Polyline { closed: true, .. });
        let ent = by_stream.entry(sp).or_default();
        ent.0 += 1;
        if closed {
            ent.1 += 1;
        }
    }
    for (sp, (n, c)) in &by_stream {
        eprintln!("{sp}: entities={n} closed-plines={c}");
    }
    // closed plines anywhere whose bbox lands near logo (any coord guess):
    for e in &geometry.entities {
        let PidGraphicKind::Polyline {
            points,
            closed: true,
        } = &e.kind
        else {
            continue;
        };
        let xs: Vec<f64> = points.iter().map(|p| p.x).collect();
        let ys: Vec<f64> = points.iter().map(|p| p.y).collect();
        let (x0, x1) = (
            xs.iter().cloned().fold(f64::MAX, f64::min),
            xs.iter().cloned().fold(f64::MIN, f64::max),
        );
        let (y0, y1) = (
            ys.iter().cloned().fold(f64::MAX, f64::min),
            ys.iter().cloned().fold(f64::MIN, f64::max),
        );
        // logo letterform: small, ~5x6mm -> in metres 0.005x0.006, in normalized ~0.004
        let w = x1 - x0;
        let h = y1 - y0;
        if (0.003..0.02).contains(&w) && (0.004..0.02).contains(&h) && points.len() >= 8 {
            println!(
                "closed pline oid={:?} stream={:?} bbox=({x0:.4},{y0:.4})-({x1:.4},{y1:.4}) n={}",
                e.graphic_oid,
                e.source.stream_path,
                points.len()
            );
        }
    }
    Ok(())
}
