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
            let PidGraphicKind::SymbolInstance {
                insertion,
                symbol_path,
                definition,
                ..
            } = &e.kind
            else {
                continue;
            };
            let name = symbol_path
                .as_deref()
                .map(|p| p.rsplit(['\\', '/']).next().unwrap_or(p))
                .unwrap_or("?");
            if !(name.contains("V Drum")
                || name.contains("Black Box")
                || name.contains("Electric Motor"))
            {
                continue;
            }
            let Some(d) = definition.and_then(|r| g.symbol_definition(r)) else {
                continue;
            };
            let ovr = d.variables.iter().any(|v| {
                v.instance_value_m
                    .is_some_and(|iv| (iv - v.value_m).abs() > 1e-9)
            });
            if !ovr {
                continue;
            }
            // page mm: insertion metres * page height 841 (landscape A0) or 594
            println!(
                "{} {name} ins_m=({:.4},{:.4})",
                Path::new(&path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .chars()
                    .take(8)
                    .collect::<String>(),
                insertion.x,
                insertion.y
            );
        }
    }
}
