//! Find the cached body that contains a "PG" text in a corpus .pid and
//! print its layer table + per-primitive layer refs.
use pid_parse::symbol_library::SymbolPrimitive;
use pid_parse::PidParser;

fn main() {
    for path in std::env::args().skip(1) {
        let doc = PidParser::new()
            .parse_file(std::path::Path::new(&path))
            .expect("parses");
        let geom = pid_parse::geometry::build_normalized_geometry(&doc);
        println!("== {path}");
        for body in &geom.symbol_definitions {
            let has_pg = body
                .primitives
                .iter()
                .any(|p| matches!(p, SymbolPrimitive::Text { text, .. } if text.trim() == "PG"));
            if !has_pg {
                continue;
            }
            println!(
                "body {:?} ({} prims)",
                body.reference,
                body.primitives.len()
            );
            for layer in &body.sheet_layers {
                println!(
                    "  layer oid={:<6} shown={:?} name={:?}",
                    layer.oid, layer.displayed, layer.name
                );
            }
            for (i, prim) in body.primitives.iter().enumerate() {
                let layer = body.primitive_layers.get(i).copied().unwrap_or(0);
                let desc = match prim {
                    SymbolPrimitive::Text { text, .. } => format!("text {text:?}"),
                    SymbolPrimitive::Circle { radius, .. } => format!("circle r={radius:.6}"),
                    other => format!("{other:?}").chars().take(40).collect(),
                };
                println!("  prim[{i}] layer={layer:<6} {desc}");
            }
        }
    }
}
