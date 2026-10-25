use pid_parse::geometry::{build_normalized_geometry, PidGraphicKind};
use pid_parse::symbol_library::SymbolPrimitive;
use pid_parse::{ParseOptions, PidParser};
use std::path::Path;

fn main() {
    for path in std::env::args().skip(1) {
        let doc = PidParser::with_options(ParseOptions::geometry())
            .parse_file(Path::new(&path))
            .expect("parse");
        let g = build_normalized_geometry(&doc);
        let tag = Path::new(&path).file_name().unwrap().to_string_lossy();
        for e in &g.entities {
            if let PidGraphicKind::SymbolInstance {
                insertion,
                symbol_path,
                definition,
                rotation,
                scale,
            } = &e.kind
            {
                if scale[1] >= 0.0 {
                    continue;
                }
                let name = symbol_path
                    .as_deref()
                    .map(|p| p.rsplit(['\\', '/']).next().unwrap_or(p))
                    .unwrap_or("?");
                let Some(d) = definition.and_then(|r| g.symbol_definition(r)) else {
                    continue;
                };
                let texts: Vec<String> = d
                    .primitives
                    .iter()
                    .filter_map(|p| {
                        if let SymbolPrimitive::Text { text, .. } = p {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if texts.is_empty() {
                    continue;
                }
                println!(
                    "{} {:?} ins=({:.1},{:.1}) rot={:.3} texts={texts:?}",
                    tag.chars().take(8).collect::<String>(),
                    name,
                    insertion.x * 1000.0,
                    insertion.y * 1000.0,
                    rotation
                );
            }
        }
    }
}
