//! ALL entities with any point in x 108-128, y 150-172 — hunt the
//! arrow+leader segments drawn at the panel's top junction.

use pid_parse::{PidGraphicKind, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let near = |x: f64, y: f64| {
        (108.0..=128.0).contains(&(x * 1000.0)) && (150.0..=172.0).contains(&(y * 1000.0))
    };
    for e in geometry.entities.iter() {
        let hid = matches!(
            e.source_layer.as_ref().and_then(|l| l.displayed),
            Some(false)
        );
        let hit = match &e.kind {
            PidGraphicKind::Line { start, end } => near(start.x, start.y) || near(end.x, end.y),
            PidGraphicKind::Polyline { points, .. } => points.iter().any(|p| near(p.x, p.y)),
            PidGraphicKind::Arc { center, .. } | PidGraphicKind::Circle { center, .. } => {
                near(center.x, center.y)
            }
            PidGraphicKind::Point { position } => near(position.x, position.y),
            PidGraphicKind::Text { insertion, .. } => near(insertion.x, insertion.y),
            PidGraphicKind::Annotation { anchor, .. } => near(anchor.x, anchor.y),
            PidGraphicKind::SymbolInstance { insertion, .. } => {
                (insertion.x * 1000.0 - 117.0).abs() < 15.0
                    && (insertion.y * 1000.0 - 160.0).abs() < 15.0
            }
            _ => false,
        };
        if !hit {
            continue;
        }
        eprintln!(
            "id={} oid={:?} hid={hid} :: {:?}",
            e.id, e.graphic_oid, e.kind
        );
    }
    Ok(())
}
