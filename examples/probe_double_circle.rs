//! Full primitive + layer dump of every cached body holding a circle.
use pid_parse::symbol_library::SymbolPrimitive;
use pid_parse::PidParser;

fn main() {
    let path = std::env::args().nth(1).expect("usage: <pid>");
    let doc = PidParser::new()
        .parse_file(std::path::Path::new(&path))
        .expect("parses");
    let geom = pid_parse::geometry::build_normalized_geometry(&doc);
    println!("{} cached defs", geom.symbol_definitions.len());
    for body in &geom.symbol_definitions {
        let has_circle = body.primitives.iter().any(|p| {
            matches!(
                p,
                SymbolPrimitive::Circle { .. } | SymbolPrimitive::Arc { .. }
            )
        });
        if !has_circle {
            continue;
        }
        println!("{:?}", body.reference);
        println!("  sheet_layers: {:?}", body.sheet_layers);
        for (i, p) in body.primitives.iter().enumerate() {
            let layer = body.primitive_layers.get(i);
            println!("    [{i}] layer={layer:?} {p:?}");
        }
    }
}
