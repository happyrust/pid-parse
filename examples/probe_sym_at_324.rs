use pid_parse::geometry::{build_normalized_geometry, PidGraphicKind};
use pid_parse::symbol_library::{read_symbol_geometry, SymbolPrimitive};
use pid_parse::{ParseOptions, PidParser};
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let f = Path::new(
        r"D:\work\cad\PID自研软件验证\Ref\Symbols\Piping\Valves\2 Way Common\Plug Valve Type 3.sym",
    );
    let geo = read_symbol_geometry(f)?;
    println!("== .sym boundaries ==");
    for sp in &geo.primitives {
        if let SymbolPrimitive::Boundary { members, fill } = &sp.primitive {
            println!("  members={} fill={:?}", members.len(), fill.map(|f| f.rgb));
        }
    }
    println!("== cached body (HE) ==");
    let parsed = PidParser::with_options(ParseOptions::geometry())
        .parse_file(r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid")?;
    let g = build_normalized_geometry(&parsed);
    for e in &g.entities {
        if let PidGraphicKind::SymbolInstance {
            insertion,
            definition,
            ..
        } = &e.kind
        {
            let (px, py) = (insertion.x * 1000.0, insertion.y * 1000.0);
            if ((px - 491.2).abs() < 5.0 && (py - 351.4).abs() < 5.0)
                || ((px - 700.4).abs() < 5.0 && (py - 202.9).abs() < 5.0)
            {
                if let Some(b) = definition.and_then(|d| g.symbol_definition(d)) {
                    for p in &b.primitives {
                        if let SymbolPrimitive::Boundary { members, fill } = p {
                            println!(
                                "  ins=({:.1},{:.1}) members={} fill={:?}",
                                px,
                                py,
                                members.len(),
                                fill.map(|f| f.rgb)
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
