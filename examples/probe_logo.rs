//! What's at the CPE-logo spot (x 1040-1060, y 70-85 mm) in W.pid entities?

use pid_parse::{PidGraphicKind, PidParser};

fn bbox(e: &pid_parse::PidGraphicEntity) -> Option<(f64, f64, f64, f64)> {
    let mut minx = f64::MAX;
    let mut miny = f64::MAX;
    let mut maxx = f64::MIN;
    let mut maxy = f64::MIN;
    let mut any = false;
    let mut add = |x: f64, y: f64| {
        minx = minx.min(x);
        maxx = maxx.max(x);
        miny = miny.min(y);
        maxy = maxy.max(y);
        any = true;
    };
    match &e.kind {
        PidGraphicKind::Line { start, end } => {
            add(start.x * 1000.0, start.y * 1000.0);
            add(end.x * 1000.0, end.y * 1000.0);
        }
        PidGraphicKind::Polyline { points, .. } => {
            for p in points {
                add(p.x * 1000.0, p.y * 1000.0);
            }
        }
        PidGraphicKind::Circle { center, radius } => {
            add((center.x - radius) * 1000.0, (center.y - radius) * 1000.0);
            add((center.x + radius) * 1000.0, (center.y + radius) * 1000.0);
        }
        PidGraphicKind::Point { position } => {
            add(position.x * 1000.0, position.y * 1000.0);
        }
        PidGraphicKind::Text { insertion, .. } => {
            add(insertion.x * 1000.0, insertion.y * 1000.0);
        }
        PidGraphicKind::SymbolInstance { insertion, .. } => {
            add(insertion.x * 1000.0, insertion.y * 1000.0);
        }
        _ => {}
    }
    any.then_some((minx, miny, maxx, maxy))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let mut hits = 0;
    for e in &geometry.entities {
        let Some((x0, y0, x1, y1)) = bbox(e) else {
            continue;
        };
        // logo region sheet mm y-up (coords are metres -> *1000)
        if x1 < 1035.0 || x0 > 1065.0 || y1 < 68.0 || y0 > 88.0 {
            continue;
        }
        hits += 1;
        let k = match &e.kind {
            PidGraphicKind::Line { .. } => "line",
            PidGraphicKind::Polyline { .. } => "pline",
            PidGraphicKind::Circle { .. } => "circle",
            PidGraphicKind::Point { .. } => "point",
            PidGraphicKind::Text { value, .. } => "text",
            PidGraphicKind::SymbolInstance { symbol_path, .. } => "symbol",
            PidGraphicKind::Annotation { .. } => "annot",
            PidGraphicKind::Arc { .. } => "arc",
            PidGraphicKind::Unknown { .. } => "unknown",
            _ => "?",
        };
        if hits < 40 {
            let extra = match &e.kind {
                PidGraphicKind::Text { value, .. } => format!(" {value:?}"),
                PidGraphicKind::SymbolInstance { symbol_path, .. } => format!(" {symbol_path:?}"),
                _ => String::new(),
            };
            println!(
                "{k:7} oid={:?} bbox=({x0:.1},{y0:.1})-({x1:.1},{y1:.1}){extra}",
                e.graphic_oid
            );
        }
    }
    eprintln!("hits={hits} total entities={}", geometry.entities.len());
    Ok(())
}
