//! Every normalized entity whose geometry reaches the panel junction
//! (x 110-125, y 155-170): what kind emits the arrow+leader?

use pid_parse::{PidGraphicKind, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let near = |x: f64, y: f64| {
        (110.0..=125.0).contains(&(x * 1000.0)) && (150.0..=170.0).contains(&(y * 1000.0))
    };
    for e in geometry.entities.iter() {
        let hid = matches!(
            e.source_layer.as_ref().and_then(|l| l.displayed),
            Some(false)
        );
        let hit = match &e.kind {
            PidGraphicKind::Line { start, end } => near(start.x, start.y) || near(end.x, end.y),
            PidGraphicKind::Polyline { points, .. } => points.iter().any(|p| near(p.x, p.y)),
            PidGraphicKind::Arc { center, radius, .. } => {
                near(center.x - radius, center.y) || near(center.x + radius, center.y)
            }
            PidGraphicKind::Circle { center, radius } => {
                near(center.x - radius, center.y) || near(center.x + radius, center.y)
            }
            PidGraphicKind::Annotation { anchor, .. } => {
                (anchor.x * 1000.0 - 117.0).abs() < 30.0 && (anchor.y * 1000.0 - 160.0).abs() < 30.0
            }
            _ => false,
        };
        if !hit {
            continue;
        }
        let desc = match &e.kind {
            PidGraphicKind::Line { start, end } => format!(
                "Line ({:.1},{:.1})-({:.1},{:.1})",
                start.x * 1000.0,
                start.y * 1000.0,
                end.x * 1000.0,
                end.y * 1000.0
            ),
            PidGraphicKind::Polyline { points, .. } => format!(
                "Polyline {} pts bbox=({:.1},{:.1})-({:.1},{:.1})",
                points.len(),
                points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min) * 1000.0,
                points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min) * 1000.0,
                points.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max) * 1000.0,
                points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max) * 1000.0,
            ),
            PidGraphicKind::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => format!(
                "Arc c=({:.1},{:.1}) r={:.1} a={:.0}..{:.0}",
                center.x * 1000.0,
                center.y * 1000.0,
                radius * 1000.0,
                start_angle,
                end_angle
            ),
            PidGraphicKind::Circle { center, radius } => format!(
                "Circle c=({:.1},{:.1}) r={:.1}",
                center.x * 1000.0,
                center.y * 1000.0,
                radius * 1000.0
            ),
            PidGraphicKind::Annotation {
                anchor,
                rotation_angle,
                secondary_radius,
                note,
            } => format!(
                "Annotation anchor=({:.3},{:.3}) rot={:.1} r2={:?} note={:?}",
                anchor.x * 1000.0,
                anchor.y * 1000.0,
                rotation_angle,
                secondary_radius,
                note
            ),
            other => format!("{other:?}"),
        };
        eprintln!(
            "{desc} | hid={hid} oid={:?} id={} kind_rec={:?}",
            e.graphic_oid,
            e.id,
            e.kind.decoded_sheet_record_kind()
        );
    }
    Ok(())
}
