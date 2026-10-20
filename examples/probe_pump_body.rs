//! Dump the Metering Pump cached-body primitives (from W.pid) and where
//! they land on the page under both scale hypotheses.

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
            rotation,
            ..
        } = &entity.kind
        else {
            continue;
        };
        if symbol_path
            .as_deref()
            .is_none_or(|p| !p.contains("Metering Pump"))
        {
            continue;
        }
        eprintln!(
            "pump oid={:?} ins=({:.4},{:.4}) rot={rot:.3} scale=({:.4},{:.4}) def={definition:?}",
            entity.graphic_oid,
            insertion.x,
            insertion.y,
            scale[0],
            scale[1],
            rot = rotation
        );
        let Some(d) = definition else { continue };
        let Some(def) = geometry
            .symbol_definitions
            .iter()
            .find(|def| def.reference == *d)
        else {
            continue;
        };
        for (i, p) in def.primitives.iter().enumerate() {
            eprintln!("  prim[{i}] {p:?}");
        }
        break;
    }
    Ok(())
}
