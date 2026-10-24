//! Dump ALL primitives of the Christmas Tree cached body (site 37311 sheet 1015),
//! grouped by kind, in mm.

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
        for (i, p) in def.primitives.iter().enumerate() {
            match p {
                SymbolPrimitive::Line { start, end } => {
                    let (x0, y0) = (start.0 * 1000.0, start.1 * 1000.0);
                    let (x1, y1) = (end.0 * 1000.0, end.1 * 1000.0);
                    println!(
                        "{i:3} line ({x0:.2},{y0:.2})->({x1:.2},{y1:.2}) len={:.2}",
                        ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt()
                    );
                }
                SymbolPrimitive::Circle { center, radius } => {
                    println!(
                        "{i:3} circle c=({:.2},{:.2}) r={:.2}",
                        center.0 * 1000.0,
                        center.1 * 1000.0,
                        radius * 1000.0
                    );
                }
                SymbolPrimitive::Polyline { vertices, .. } => {
                    let xs: Vec<String> = vertices
                        .iter()
                        .map(|(x, y)| format!("({:.1},{:.1})", x * 1000.0, y * 1000.0))
                        .collect();
                    println!("{i:3} polyline n={} {}", vertices.len(), xs.join(" "));
                }
                SymbolPrimitive::Boundary { members, fill } => {
                    println!("{i:3} boundary fill={fill:?} members={}", members.len());
                    for m in members {
                        match m {
                            SymbolPrimitive::Circle { center, radius } => println!(
                                "      circ c=({:.2},{:.2}) r={:.2}",
                                center.0 * 1000.0,
                                center.1 * 1000.0,
                                radius * 1000.0
                            ),
                            SymbolPrimitive::Line { start, end } => println!(
                                "      line ({:.2},{:.2})->({:.2},{:.2})",
                                start.0 * 1000.0,
                                start.1 * 1000.0,
                                end.0 * 1000.0,
                                end.1 * 1000.0
                            ),
                            SymbolPrimitive::Polyline { vertices, .. } => {
                                let xs: Vec<String> = vertices
                                    .iter()
                                    .map(|(x, y)| format!("({:.1},{:.1})", x * 1000.0, y * 1000.0))
                                    .collect();
                                println!("      pline {}", xs.join(" "));
                            }
                            _ => println!("      {:?}", std::mem::discriminant(m)),
                        }
                    }
                }
                SymbolPrimitive::BSpline { poles, .. } => {
                    let xs: Vec<String> = poles
                        .iter()
                        .map(|(x, y)| format!("({:.2},{:.2})", x * 1000.0, y * 1000.0))
                        .collect();
                    println!("{i:3} bspline n={} {}", poles.len(), xs.join(" "));
                }
                other => println!("{i:3} {:?}", std::mem::discriminant(other)),
            }
        }
    }
    Ok(())
}
