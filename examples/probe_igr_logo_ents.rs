//! Entity census of the .igr logo zone (x 695-711, y 71-81 mm on A1):
//! which kinds, which layers?

use pid_parse::{build_normalized_geometry, ParseOptions, PidGraphicKind, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A1.igr";
    let parsed = PidParser::with_options(ParseOptions::geometry()).parse_file(path)?;
    let geo = build_normalized_geometry(&parsed);
    let mut kinds = std::collections::BTreeMap::new();
    for e in &geo.entities {
        let (x0, y0, x1, y1) = bbox(e);
        let cx = (x0 + x1) / 2.0;
        let cy = (y0 + y1) / 2.0;
        if !(0.69..=0.72).contains(&cx) || !(0.070..=0.082).contains(&cy) {
            continue;
        }
        let kind = match &e.kind {
            PidGraphicKind::Line { .. } => "Line",
            PidGraphicKind::Polyline { closed, .. } => {
                if *closed {
                    "PolylineClosed"
                } else {
                    "Polyline"
                }
            }
            PidGraphicKind::Arc { .. } => "Arc",
            PidGraphicKind::Point { .. } => "Point",
            PidGraphicKind::Text { .. } => "Text",
            PidGraphicKind::SymbolInstance { .. } => "Symbol",
            _ => "Other",
        };
        let layer = e
            .source_layer
            .as_ref()
            .and_then(|l| l.name.clone())
            .unwrap_or_else(|| "?".to_string());
        *kinds.entry((kind, layer)).or_insert(0) += 1;
    }
    for ((k, l), n) in kinds {
        println!("{n:3} {k:16} layer={l}");
    }
    for (path, layers) in &parsed.sheet_layers {
        for l in layers {
            println!(
                "layer stream={path} name={:?} displayed={:?} oid={:#x}",
                l.name, l.displayed, l.oid
            );
        }
    }
    Ok(())
}

fn bbox(e: &pid_parse::PidGraphicEntity) -> (f64, f64, f64, f64) {
    let mut x0 = f64::MAX;
    let mut y0 = f64::MAX;
    let mut x1 = f64::MIN;
    let mut y1 = f64::MIN;
    let mut add = |x: f64, y: f64| {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    };
    match &e.kind {
        PidGraphicKind::Line { start, end } => {
            add(start.x, start.y);
            add(end.x, end.y);
        }
        PidGraphicKind::Polyline { points, .. } => {
            for p in points {
                add(p.x, p.y);
            }
        }
        PidGraphicKind::Point { position } => add(position.x, position.y),
        PidGraphicKind::Arc { center, radius, .. } => {
            add(center.x - radius, center.y - radius);
            add(center.x + radius, center.y + radius);
        }
        _ => return (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
    }
    (x0, y0, x1, y1)
}
