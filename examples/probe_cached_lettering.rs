//! Does a drawing's definition-cache lettering resolve against the site's
//! own StyleCluster? Prints every cached body's text primitives with their
//! lettering, for the named .pid.
use pid_parse::symbol_library::SymbolPrimitive;
use pid_parse::PidParser;

fn main() {
    let path = std::env::args().nth(1).expect("usage: <pid>");
    let doc = PidParser::new()
        .parse_file(std::path::Path::new(&path))
        .expect("parses");
    for site in &doc.jsites {
        let Some(nested) = &site.nested_geometry else {
            continue;
        };
        println!(
            "{}: {} texts, table={}",
            site.name,
            nested.texts.len(),
            doc.style_tables.contains_key(&site.path)
        );
    }
    let bodies = pid_parse::geometry::build_normalized_geometry(&doc).symbol_definitions;
    for body in &bodies {
        for (prim, _style, lettering) in body.visible_styled() {
            if let SymbolPrimitive::Text { text, at } = prim {
                println!(
                    "{:?} {:?} {:?} -> {:?}",
                    body.reference, text, at, lettering
                );
            }
        }
    }
}
