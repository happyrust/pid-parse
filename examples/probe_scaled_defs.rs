//! For every igSymbol2d with uniform_scale != 1: print the cached body
//! extents so we can compare template size vs placed size.

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    for entity in &geometry.entities {
        let pid_parse::PidGraphicKind::SymbolInstance {
            insertion,
            scale,
            definition,
            symbol_path,
            ..
        } = &entity.kind
        else {
            continue;
        };
        if (scale[0].abs() - 1.0).abs() < 1e-6 && (scale[1].abs() - 1.0).abs() < 1e-6 {
            continue;
        }
        let def = definition.and_then(|d| {
            geometry
                .symbol_definitions
                .iter()
                .find(|def| def.reference == d)
        });
        let bbox = def.map(|d| {
            let mut mn = (f64::MAX, f64::MAX);
            let mut mx = (f64::MIN, f64::MIN);
            for p in d.visible_primitives() {
                use pid_parse::symbol_library::SymbolPrimitive as SP;
                let mut add = |x: f64, y: f64| {
                    mn.0 = mn.0.min(x);
                    mx.0 = mx.0.max(x);
                    mn.1 = mn.1.min(y);
                    mx.1 = mx.1.max(y);
                };
                match p {
                    SP::Line { start, end } => {
                        add(start.0, start.1);
                        add(end.0, end.1);
                    }
                    SP::Circle { center, radius } => {
                        add(center.0 - radius, center.1 - radius);
                        add(center.0 + radius, center.1 + radius);
                    }
                    SP::Polyline { vertices, .. } => {
                        for (x, y) in vertices {
                            add(*x, *y);
                        }
                    }
                    SP::Arc { center, radius, .. } => {
                        add(center.0 - radius, center.1 - radius);
                        add(center.0 + radius, center.1 + radius);
                    }
                    SP::Boundary { members, .. } => {
                        for m in members {
                            if let SP::Circle { center, radius } = m {
                                add(center.0 - radius, center.1 - radius);
                                add(center.0 + radius, center.1 + radius);
                            }
                        }
                    }
                    SP::Text { at, .. } => add(at.0, at.1),
                    SP::BSpline { poles, .. } => {
                        for (x, y) in poles {
                            add(*x, *y);
                        }
                    }
                }
            }
            (
                (mx.0 - mn.0) * 1000.0,
                (mx.1 - mn.1) * 1000.0,
                mn.0 * 1000.0,
                mn.1 * 1000.0,
            )
        });
        let name = symbol_path
            .as_deref()
            .and_then(|p| p.rsplit('\\').next())
            .unwrap_or("?");
        println!(
            "oid={:?} {name:40} ins_mm=({:.1},{:.1}) scale=({:.4},{:.4}) body_mm={:?}",
            entity.graphic_oid,
            insertion.x * 1000.0,
            insertion.y * 1000.0,
            scale[0],
            scale[1],
            bbox.map(|b| format!("{:.1}x{:.1} at ({:.1},{:.1})", b.0, b.1, b.2, b.3))
        );
    }
    Ok(())
}
