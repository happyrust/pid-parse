//! Dump the Wellhead Control Panel cached body (site 37311 sheet 1139)
//! primitives + their layers — the ref draws horizontal tick marks where
//! we draw diagonal dashes.

use pid_parse::symbol_library::SymbolPrimitive as SP;
use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    for def in &geometry.symbol_definitions {
        if def.reference.site != 37311 || def.reference.sheet != 1139 {
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
            let layer_oid = def.primitive_layers.get(i).copied().unwrap_or(0);
            let lname = def
                .sheet_layers
                .iter()
                .find(|l| l.oid == layer_oid)
                .and_then(|l| l.name.as_deref())
                .unwrap_or("?");
            match p {
                SP::Line { start, end } => {
                    eprintln!(
                        "  [{i:2}] L {lname:14} ({:.4},{:.4})-({:.4},{:.4})",
                        start.0, start.1, end.0, end.1
                    );
                }
                SP::Circle { center, radius } => {
                    eprintln!(
                        "  [{i:2}] C {lname:14} ({:.4},{:.4}) r={:.4}",
                        center.0, center.1, radius
                    );
                }
                SP::Polyline { vertices, .. } => {
                    eprintln!("  [{i:2}] P {lname:14} {} verts", vertices.len());
                }
                SP::Boundary { members, .. } => {
                    eprintln!("  [{i:2}] B {lname:14} {} members", members.len());
                }
                SP::Text { at, text, .. } => {
                    eprintln!("  [{i:2}] T {lname:14} ({:.4},{:.4}) {text:?}", at.0, at.1);
                }
                _ => eprintln!("  [{i:2}] ? {lname:14} {p:?}"),
            }
        }
    }
    Ok(())
}
