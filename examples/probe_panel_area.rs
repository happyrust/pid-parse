//! What sheet-level entities sit inside the panel rect (104-117, 130-162)?

use pid_parse::{PidGraphicKind, PidParser};

fn hidden(e: &pid_parse::PidGraphicEntity) -> bool {
    matches!(
        e.source_layer.as_ref().and_then(|l| l.displayed),
        Some(false)
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    // Scene coords are metres.
    for e in geometry.entities.iter() {
        match &e.kind {
            PidGraphicKind::Line { start, end } => {
                let (x0, y0) = (start.x * 1000.0, start.y * 1000.0);
                let (x1, y1) = (end.x * 1000.0, end.y * 1000.0);
                let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                if (100.0..=122.0).contains(&cx) && (128.0..=165.0).contains(&cy) {
                    eprintln!(
                        "L ({:.1},{:.1})-({:.1},{:.1}) d={:.1}mm layer={:?} hid={} oid={:?}",
                        x0,
                        y0,
                        x1,
                        y1,
                        (x1 - x0).hypot(y1 - y0),
                        e.source_layer.as_ref().and_then(|l| l.name.as_deref()),
                        hidden(e),
                        e.graphic_oid
                    );
                }
            }
            PidGraphicKind::Polyline { points, closed } => {
                for w in points.windows(2) {
                    let (x0, y0) = (w[0].x * 1000.0, w[0].y * 1000.0);
                    let (x1, y1) = (w[1].x * 1000.0, w[1].y * 1000.0);
                    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                    if (100.0..=122.0).contains(&cx) && (128.0..=165.0).contains(&cy) {
                        eprintln!(
                            "P seg ({:.1},{:.1})-({:.1},{:.1}) d={:.1}mm closed={} layer={:?} hid={} oid={:?}",
                            x0,
                            y0,
                            x1,
                            y1,
                            (x1 - x0).hypot(y1 - y0),
                            closed,
                            e.source_layer.as_ref().and_then(|l| l.name.as_deref()),
                            hidden(e),
                            e.graphic_oid
                        );
                    }
                }
            }
            _ => {}
        }
    }
    eprintln!("--- symbols in region ---");
    for e in geometry.entities.iter() {
        if let PidGraphicKind::SymbolInstance {
            insertion,
            symbol_path,
            rotation,
            scale,
            definition,
        } = &e.kind
        {
            let (x, y) = (insertion.x * 1000.0, insertion.y * 1000.0);
            if (60.0..=140.0).contains(&x) && (100.0..=200.0).contains(&y) {
                eprintln!(
                    "sym path={symbol_path:?} def={definition:?} ins=({x:.1},{y:.1}) s={scale:?} rot={rotation:.2} hid={}",
                    hidden(e)
                );
            }
        }
    }
    Ok(())
}
