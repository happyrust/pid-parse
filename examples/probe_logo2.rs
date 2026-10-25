//! Which symbol defs contain Boundary prims, and of what member kinds?
//! Then: which placed instance could be the drawing border (large bbox)?

use pid_parse::symbol_library::SymbolPrimitive;
use pid_parse::{PidGraphicKind, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    for def in &geometry.symbol_definitions {
        let mut nb = 0usize;
        let mut nl = 0usize;
        let mut nc = 0usize;
        let mut prims = 0usize;
        for p in &def.primitives {
            prims += 1;
            if let SymbolPrimitive::Boundary { members, .. } = p {
                nb += 1;
                for m in members {
                    match m {
                        SymbolPrimitive::Line { .. } => nl += 1,
                        SymbolPrimitive::Circle { .. } => nc += 1,
                        _ => {}
                    }
                }
            }
        }
        if nb > 0 {
            eprintln!(
                "def {:?} prims={prims} boundaries={nb} line-mem={nl} circ-mem={nc}",
                def.reference
            );
        }
    }
    // symbol placements on the sheet — find ones near the right/bottom
    for e in &geometry.entities {
        if let PidGraphicKind::SymbolInstance {
            insertion,
            symbol_path,
            definition,
            scale,
            ..
        } = &e.kind
        {
            let (x, y) = (insertion.x * 1000.0, insertion.y * 1000.0);
            if x > 900.0 || y < 120.0 {
                println!(
                    "sym oid={:?} ins=({x:.1},{y:.1}) scale={scale:?} def={definition:?} path={:?}",
                    e.graphic_oid, symbol_path
                );
            }
        }
    }
    Ok(())
}
