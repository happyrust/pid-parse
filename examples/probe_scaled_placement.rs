//! For every non-unit-scale igSymbol2d in W, compute where the scaled body
//! lands in page mm and print — verifies `page = ins + s·M·body`.

use pid_parse::symbol_library::SymbolPrimitive as SP;
use pid_parse::{PidGraphicKind, PidParser};

fn add(acc: &mut (f64, f64, f64, f64), x: f64, y: f64) {
    acc.0 = acc.0.min(x);
    acc.1 = acc.1.min(y);
    acc.2 = acc.2.max(x);
    acc.3 = acc.3.max(y);
}

fn prim_bounds(p: &SP, acc: &mut (f64, f64, f64, f64)) {
    match p {
        SP::Line { start, end } => {
            add(acc, start.0, start.1);
            add(acc, end.0, end.1);
        }
        SP::Circle { center, radius } => {
            add(acc, center.0 - radius, center.1 - radius);
            add(acc, center.0 + radius, center.1 + radius);
        }
        SP::Arc { center, radius, .. } => {
            add(acc, center.0 - radius, center.1 - radius);
            add(acc, center.0 + radius, center.1 + radius);
        }
        SP::Polyline { vertices, .. }
        | SP::BSpline {
            poles: vertices, ..
        } => {
            for (x, y) in vertices {
                add(acc, *x, *y);
            }
        }
        SP::Text { at, .. } => add(acc, at.0, at.1),
        SP::Boundary { members, .. } => {
            for m in members {
                prim_bounds(m, acc);
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    for e in geometry.entities.iter() {
        if let PidGraphicKind::SymbolInstance {
            insertion,
            symbol_path,
            rotation,
            scale,
            definition,
        } = &e.kind
        {
            if (scale[0].abs() - 1.0).abs() < 1e-6 && (scale[1].abs() - 1.0).abs() < 1e-6 {
                continue;
            }
            let name = symbol_path
                .as_deref()
                .and_then(|p| p.rsplit(['\\', '/']).next())
                .unwrap_or("?");
            eprintln!(
                "{name:38} def={definition:?} ins=({:.1},{:.1})mm scale=({:.4},{:.4}) rot={:.3} oid={:?}",
                insertion.x * 1000.0,
                insertion.y * 1000.0,
                scale[0],
                scale[1],
                rotation,
                e.graphic_oid
            );
            if let Some(def_ref) = definition {
                if let Some(def) = geometry
                    .symbol_definitions
                    .iter()
                    .find(|d| &d.reference == def_ref)
                {
                    let mut acc = (
                        f64::INFINITY,
                        f64::INFINITY,
                        f64::NEG_INFINITY,
                        f64::NEG_INFINITY,
                    );
                    for p in def.visible_primitives() {
                        prim_bounds(p, &mut acc);
                    }
                    let (x0, y0, x1, y1) = acc;
                    let (sx, sy) = (scale[0], scale[1]);
                    let px = |v: f64| insertion.x + v * sx;
                    let py = |v: f64| insertion.y + v * sy;
                    eprintln!(
                        "    body=({:.4},{:.4})-({:.4},{:.4})m -> page=({:.1},{:.1})-({:.1},{:.1})mm",
                        x0,
                        y0,
                        x1,
                        y1,
                        px(x0).min(px(x1)) * 1000.0,
                        py(y0).min(py(y1)) * 1000.0,
                        px(x0).max(px(x1)) * 1000.0,
                        py(y0).max(py(y1)) * 1000.0
                    );
                }
            }
        }
    }
    Ok(())
}
