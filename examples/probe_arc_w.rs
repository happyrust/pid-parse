use pid_parse::geometry::{build_normalized_geometry, PidGraphicKind};
use pid_parse::{ParseOptions, PidParser};
use std::path::Path;

fn main() {
    for path in std::env::args().skip(1) {
        let doc = PidParser::with_options(ParseOptions::geometry())
            .parse_file(Path::new(&path))
            .expect("parse");
        let g = build_normalized_geometry(&doc);
        for e in &g.entities {
            if let PidGraphicKind::SymbolInstance {
                insertion,
                symbol_path,
                definition,
                rotation,
                scale,
            } = &e.kind
            {
                let name = symbol_path
                    .as_deref()
                    .map(|p| p.rsplit(['\\', '/']).next().unwrap_or(p))
                    .unwrap_or("?");
                if !(name.contains("ESD") || name.contains("Solenoid") || name.contains("signal")) {
                    continue;
                }
                if let Some(d) = definition.and_then(|r| g.symbol_definition(r)) {
                    println!(
                        "{} {:?} ins=({:.1},{:.1}) rot={:.3} scale={scale:?}",
                        e.id,
                        name,
                        insertion.x * 1000.0,
                        insertion.y * 1000.0,
                        rotation
                    );
                    for (i, p) in d.primitives.iter().enumerate() {
                        if let pid_parse::symbol_library::SymbolPrimitive::Text { text, at } = p {
                            println!(
                                "    body[{i}] {text:?} at {at:?} lettering={:?}",
                                d.primitive_letterings.get(i)
                            );
                        }
                    }
                }
            }
        }
    }
}
