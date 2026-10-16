//! Symbol instances near the panel's pipe junctions (117,133)/(117,145)/(117,155)
//! and their body primitives — the arrow+leader marks come from a body.

use pid_parse::symbol_library::SymbolPrimitive as SP;
use pid_parse::{PidGraphicKind, PidParser};

const JUNCTIONS: [(f64, f64); 4] = [
    (114.8, 161.65),
    (117.1, 154.85),
    (117.1, 144.85),
    (117.35, 133.35),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = PidParser::new()
        .parse_file(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid")?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    let mut seen = std::collections::BTreeSet::new();
    for e in geometry.entities.iter() {
        if let PidGraphicKind::SymbolInstance {
            insertion,
            symbol_path,
            rotation,
            scale,
            definition,
        } = &e.kind
        {
            let (ix, iy) = (insertion.x * 1000.0, insertion.y * 1000.0);
            for (jx, jy) in JUNCTIONS {
                if (ix - jx).abs() < 12.0 && (iy - jy).abs() < 12.0 {
                    let name = symbol_path
                        .as_deref()
                        .and_then(|p| p.rsplit(['\\', '/']).next())
                        .unwrap_or("?");
                    eprintln!(
                        "sym {name:?} def={definition:?} ins=({ix:.1},{iy:.1}) s={scale:?} rot={rotation:.2} oid={:?}",
                        e.graphic_oid
                    );
                    if let Some(dr) = definition {
                        if seen.insert((dr.site, dr.sheet)) {
                            if let Some(def) = geometry
                                .symbol_definitions
                                .iter()
                                .find(|d| &d.reference == dr)
                            {
                                for (i, l) in def.sheet_layers.iter().enumerate() {
                                    eprintln!(
                                        "    layer[{i}] oid={} name={:?} displayed={:?}",
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
                                    eprintln!("    [{i:2}] {lname:14} {p:?}");
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }
    }
    Ok(())
}
