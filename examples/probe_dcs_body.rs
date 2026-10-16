//! Dump `DCS Func Access in Prime Loc.sym` cached body (site 37311 sheet
//! 1435): primitives, layers, and styles — ours draws arrow+leader, ref
//! draws a ⊢ tick.

use pid_parse::symbol_library::SymbolPrimitive as SP;
use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    for def in &geometry.symbol_definitions {
        if def.reference.site != 37311 || def.reference.sheet != 1435 {
            continue;
        }
        eprintln!("def {:?} prims={}", def.reference, def.primitives.len());
        for (i, l) in def.sheet_layers.iter().enumerate() {
            eprintln!(
                "  layer[{i}] oid={} name={:?} displayed={:?}",
                l.oid, l.name, l.displayed
            );
        }
        for (i, p) in def.primitives.iter().enumerate() {
            let lay = def.primitive_layers.get(i).copied().unwrap_or(0);
            let lname = def
                .sheet_layers
                .iter()
                .find(|l| l.oid == lay)
                .and_then(|l| l.name.as_deref())
                .unwrap_or("?");
            let sty = def.primitive_styles.get(i).and_then(|s| s.as_ref());
            match p {
                SP::Line { start, end } => eprintln!(
                    "  [{i:2}] L {lname:12} ({:.4},{:.4})-({:.4},{:.4}) sty={sty:?}",
                    start.0, start.1, end.0, end.1
                ),
                SP::Circle { center, radius } => eprintln!(
                    "  [{i:2}] C {lname:12} ({:.4},{:.4}) r={:.4} sty={sty:?}",
                    center.0, center.1, radius
                ),
                SP::Arc { center, radius, start_angle, end_angle } => eprintln!(
                    "  [{i:2}] A {lname:12} ({:.4},{:.4}) r={:.4} a={start_angle:.0}..{end_angle:.0} sty={sty:?}",
                    center.0, center.1, radius
                ),
                SP::Polyline { vertices, is_closed } => {
                    eprintln!("  [{i:2}] P {lname:12} {}v closed={is_closed} sty={sty:?}", vertices.len());
                    for v in vertices {
                        eprintln!("        ({:.4},{:.4})", v.0, v.1);
                    }
                }
                SP::Text { at, text, .. } => eprintln!(
                    "  [{i:2}] T {lname:12} ({:.4},{:.4}) {text:?} sty={sty:?}",
                    at.0, at.1
                ),
                other => eprintln!("  [{i:2}] ? {lname:12} {other:?}"),
            }
        }
    }
    Ok(())
}
