//! Dump the Christmas Tree cached-body primitives (site 37311 sheet 1015):
//! boundary centers plus min/max of all primitives, to learn the body's
//! coordinate origin and unit.

use pid_parse::symbol_library::SymbolPrimitive;
use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    for def in &geometry.symbol_definitions {
        if def.reference.site != 37311 || def.reference.sheet != 1015 {
            continue;
        }
        eprintln!("def {:?} prims={}", def.reference, def.primitives.len());
        let mut minx = f64::MAX;
        let mut maxx = f64::MIN;
        let mut miny = f64::MAX;
        let mut maxy = f64::MIN;
        let add =
            |x: f64, y: f64, minx: &mut f64, maxx: &mut f64, miny: &mut f64, maxy: &mut f64| {
                *minx = minx.min(x);
                *maxx = maxx.max(x);
                *miny = miny.min(y);
                *maxy = maxy.max(y);
            };
        for p in &def.primitives {
            match p {
                SymbolPrimitive::Line { start, end } => {
                    add(start.0, start.1, &mut minx, &mut maxx, &mut miny, &mut maxy);
                    add(end.0, end.1, &mut minx, &mut maxx, &mut miny, &mut maxy);
                }
                SymbolPrimitive::Circle { center, radius } => {
                    add(
                        center.0 - radius,
                        center.1 - radius,
                        &mut minx,
                        &mut maxx,
                        &mut miny,
                        &mut maxy,
                    );
                    add(
                        center.0 + radius,
                        center.1 + radius,
                        &mut minx,
                        &mut maxx,
                        &mut miny,
                        &mut maxy,
                    );
                }
                SymbolPrimitive::Polyline { vertices, .. } => {
                    for (x, y) in vertices {
                        add(*x, *y, &mut minx, &mut maxx, &mut miny, &mut maxy);
                    }
                }
                SymbolPrimitive::Boundary { members, .. } => {
                    for m in members {
                        if let SymbolPrimitive::Circle { center, radius } = m {
                            eprintln!(
                                "  bnd circle c=({:.6},{:.6}) r={:.6}",
                                center.0, center.1, radius
                            );
                            add(
                                center.0 - radius,
                                center.1 - radius,
                                &mut minx,
                                &mut maxx,
                                &mut miny,
                                &mut maxy,
                            );
                            add(
                                center.0 + radius,
                                center.1 + radius,
                                &mut minx,
                                &mut maxx,
                                &mut miny,
                                &mut maxy,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        eprintln!("  body bbox x[{minx:.6},{maxx:.6}] y[{miny:.6},{maxy:.6}]");
    }
    Ok(())
}
