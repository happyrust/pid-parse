//! Hunt the sine-wave / zig-zag ink seen in the exported PDF around
//! scene mm (x 169.8-181.6, y 649.3-662) of drawing 2: dump every
//! polyline-ish entity with >=10 vertices and every entity whose
//! raw bbox intersects the target under both m->mm and mm-as-is.

use pid_parse::geometry::{build_normalized_geometry, PidGraphicKind};
use pid_parse::{ParseOptions, PidParser};
use std::path::Path;

fn bbox(pts: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (x, y) in pts {
        x0 = x0.min(*x);
        y0 = y0.min(*y);
        x1 = x1.max(*x);
        y1 = y1.max(*y);
    }
    (x0, y0, x1, y1)
}

fn main() {
    let path = Path::new(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid");
    let doc = PidParser::with_options(ParseOptions::geometry())
        .parse_file(path)
        .expect("parse");
    let g = build_normalized_geometry(&doc);
    println!("entities: {}", g.entities.len());
    for e in &g.entities {
        if let PidGraphicKind::SymbolInstance {
            insertion,
            symbol_path,
            rotation,
            scale,
            definition,
        } = &e.kind
        {
            let (sx, sy) = (insertion.x * 1000.0, insertion.y * 1000.0);
            if sx > 150.0 && sx < 200.0 && sy > 630.0 && sy < 680.0 {
                println!(
                    "SYMINS {} at mm({:.1},{:.1}) rot {:.2} scale {:?} path {:?} def {:?} layer {:?}",
                    e.id,
                    sx,
                    sy,
                    rotation,
                    scale,
                    symbol_path,
                    definition,
                    e.source_layer
                        .as_ref()
                        .map(|l| (l.oid, l.name.clone(), l.displayed))
                );
                if let Some(dref) = definition {
                    if let Some(def) = g.symbol_definition(*dref) {
                        for (i, p) in def.primitives.iter().enumerate() {
                            println!("   body[{i}] {p:?}");
                        }
                    }
                }
            }
            continue;
        }
        let pts: Vec<(f64, f64)> = match &e.kind {
            PidGraphicKind::Polyline { points, .. } => points.iter().map(|p| (p.x, p.y)).collect(),
            PidGraphicKind::Line { start, end } => vec![(start.x, start.y), (end.x, end.y)],
            _ => continue,
        };
        let (x0, y0, x1, y1) = bbox(&pts);
        let big = pts.len() >= 10;
        // target box in mm; try raw==mm and raw*1000==mm
        let hit_mm = x1 > 169.0 && x0 < 182.0 && y1 > 648.0 && y0 < 663.0;
        let hit_m = x1 * 1000.0 > 169.0
            && x0 * 1000.0 < 182.0
            && y1 * 1000.0 > 648.0
            && y0 * 1000.0 < 663.0;
        // the solid vertical L: scene x 110-163, y 648-715
        let hit_l = x1 * 1000.0 > 110.0
            && x0 * 1000.0 < 163.0
            && y1 * 1000.0 > 648.0
            && y0 * 1000.0 < 715.0
            && (x1 - x0) * 1000.0 + (y1 - y0) * 1000.0 > 50.0;
        if big || hit_mm || hit_m || hit_l {
            let layer = e
                .source_layer
                .as_ref()
                .map(|l| format!("oid={} name={:?} disp={:?}", l.oid, l.name, l.displayed))
                .unwrap_or_else(|| "none".into());
            println!(
                "{} n={} raw [{:.4},{:.4} - {:.4},{:.4}] mm [{:.1},{:.1} - {:.1},{:.1}] layer {} kind {:?} src {} oid {:?}",
                e.id,
                pts.len(),
                x0,
                y0,
                x1,
                y1,
                x0 * 1000.0,
                y0 * 1000.0,
                x1 * 1000.0,
                y1 * 1000.0,
                layer,
                std::mem::discriminant(&e.kind),
                e.source.stream_path.as_deref().unwrap_or("?"),
                e.graphic_oid
            );
            if big && pts.len() <= 60 {
                print!("   verts:");
                for (x, y) in pts.iter().take(30) {
                    print!(" ({:.3},{:.3})", x * 1000.0, y * 1000.0);
                }
                println!();
            }
        }
    }
}
