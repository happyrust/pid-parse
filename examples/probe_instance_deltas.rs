use pid_parse::geometry::{build_normalized_geometry, PidGraphicKind};
use pid_parse::symbol_library::SymbolPrimitive;
use pid_parse::{ParseOptions, PidParser};
use std::path::Path;

fn prim_extent(p: &SymbolPrimitive, acc: &mut (f64, f64, f64, f64)) {
    let mut push = |x: f64, y: f64| {
        acc.0 = acc.0.min(x);
        acc.1 = acc.1.min(y);
        acc.2 = acc.2.max(x);
        acc.3 = acc.3.max(y);
    };
    match p {
        SymbolPrimitive::Line { start, end, .. } => {
            push(start.0, start.1);
            push(end.0, end.1);
        }
        SymbolPrimitive::Polyline { vertices, .. } => {
            for v in vertices {
                push(v.0, v.1);
            }
        }
        SymbolPrimitive::Circle { center, radius } => {
            push(center.0 - radius, center.1 - radius);
            push(center.0 + radius, center.1 + radius);
        }
        SymbolPrimitive::Arc { center, radius, .. } => {
            push(center.0 - radius, center.1 - radius);
            push(center.0 + radius, center.1 + radius);
        }
        SymbolPrimitive::Text { at, .. } => push(at.0, at.1),
        SymbolPrimitive::BSpline { poles, .. } => {
            for v in poles {
                push(v.0, v.1);
            }
        }
        SymbolPrimitive::Boundary { members, .. } => {
            for m in members {
                prim_extent(m, acc);
            }
        }
    }
}

fn main() {
    for path in std::env::args().skip(1) {
        let doc = PidParser::with_options(ParseOptions::geometry())
            .parse_file(Path::new(&path))
            .expect("parse");
        let g = build_normalized_geometry(&doc);
        let tag = Path::new(&path).file_name().unwrap().to_string_lossy();
        println!("=== {}", tag.chars().take(10).collect::<String>());
        for e in &g.entities {
            let PidGraphicKind::SymbolInstance {
                symbol_path,
                definition,
                scale,
                ..
            } = &e.kind
            else {
                continue;
            };
            let Some(d) = definition.and_then(|r| g.symbol_definition(r)) else {
                continue;
            };
            let overridden: Vec<_> = d
                .variables
                .iter()
                .filter(|v| {
                    v.instance_value_m
                        .is_some_and(|iv| (iv - v.value_m).abs() > 1e-9)
                })
                .collect();
            if overridden.is_empty() {
                continue;
            }
            let name = symbol_path
                .as_deref()
                .map(|p| p.rsplit(['\\', '/']).next().unwrap_or(p))
                .unwrap_or("?");
            let mut acc = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for p in &d.primitives {
                prim_extent(p, &mut acc);
            }
            println!(
                " {name} scale=({:.3},{:.3}) body=({:.1}..{:.1} x {:.1}..{:.1})mm",
                scale[0],
                scale[1],
                acc.0 * 1000.0,
                acc.2 * 1000.0,
                acc.1 * 1000.0,
                acc.3 * 1000.0
            );
            for v in overridden {
                println!(
                    "    {} : default={:.1}mm instance={:.1}mm",
                    v.name,
                    v.value_m * 1000.0,
                    v.instance_value_m.unwrap() * 1000.0
                );
            }
        }
    }
}
