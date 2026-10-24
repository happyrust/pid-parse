//! Diagonal vs tick marks at the panel's right edge: what records produce
//! the small strokes at x ~117-125, y ~128-165?

use pid_parse::{PidGraphicKind, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    for e in geometry.entities.iter() {
        let hid = matches!(
            e.source_layer.as_ref().and_then(|l| l.displayed),
            Some(false)
        );
        match &e.kind {
            PidGraphicKind::Line { start, end } => {
                let (x0, y0, x1, y1) = (
                    start.x * 1000.0,
                    start.y * 1000.0,
                    end.x * 1000.0,
                    end.y * 1000.0,
                );
                let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                if (112.0..=128.0).contains(&cx) && (125.0..=170.0).contains(&cy) {
                    eprintln!(
                        "L ({:.2},{:.2})-({:.2},{:.2}) layer={:?} hid={} oid={:?}",
                        x0,
                        y0,
                        x1,
                        y1,
                        e.source_layer.as_ref().and_then(|l| l.name.as_deref()),
                        hid,
                        e.graphic_oid
                    );
                }
            }
            PidGraphicKind::Polyline { points, .. } => {
                for w in points.windows(2) {
                    let (x0, y0, x1, y1) = (
                        w[0].x * 1000.0,
                        w[0].y * 1000.0,
                        w[1].x * 1000.0,
                        w[1].y * 1000.0,
                    );
                    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                    if (112.0..=128.0).contains(&cx) && (125.0..=170.0).contains(&cy) {
                        eprintln!(
                            "P ({:.2},{:.2})-({:.2},{:.2}) layer={:?} hid={} oid={:?} id={}",
                            x0,
                            y0,
                            x1,
                            y1,
                            e.source_layer.as_ref().and_then(|l| l.name.as_deref()),
                            hid,
                            e.graphic_oid,
                            e.id
                        );
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
