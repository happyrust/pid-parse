//! Which cached symbol bodies contain concentric circle pairs (the
//! annulus markers the reference PDF fills solid), and what does the
//! body's own `primitive_styles` say for those circles — a resolved line
//! style or `None` (a fill-family index)?

use pid_parse::geometry::build_normalized_geometry;
use pid_parse::symbol_library::SymbolPrimitive;
use pid_parse::{ParseOptions, PidParser};
use std::path::Path;

fn main() {
    for arg in std::env::args().skip(1) {
        let path = Path::new(&arg);
        let doc = PidParser::with_options(ParseOptions::geometry())
            .parse_file(path)
            .expect("parse");
        let g = build_normalized_geometry(&doc);
        println!("=== {}", path.file_name().unwrap().to_string_lossy());
        println!("defs: {}", g.symbol_definitions.len());
        for def in &g.symbol_definitions {
            // collect circle centres+radii
            let circles: Vec<(usize, f64, f64, f64)> = def
                .primitives
                .iter()
                .enumerate()
                .filter_map(|(i, p)| match p {
                    SymbolPrimitive::Circle { center, radius } => {
                        Some((i, center.0, center.1, *radius))
                    }
                    _ => None,
                })
                .collect();
            let mut concentric = Vec::new();
            for a in &circles {
                for b in &circles {
                    if a.0 < b.0
                        && (a.1 - b.1).abs() < 1e-6
                        && (a.2 - b.2).abs() < 1e-6
                        && (a.3 - b.3).abs() > 1e-6
                    {
                        concentric.push((a.0, b.0));
                    }
                }
            }
            if concentric.is_empty() {
                continue;
            }
            println!(
                "def site={} sheet={} prims={} concentric_pairs={:?} layers={:?}",
                def.reference.site,
                def.reference.sheet,
                def.primitives.len(),
                concentric,
                def.sheet_layers
                    .iter()
                    .map(|l| (l.oid, l.name.clone(), l.displayed))
                    .collect::<Vec<_>>()
            );
            for (i, p) in def.primitives.iter().enumerate() {
                let style = def.primitive_styles.get(i).and_then(|s| s.as_ref());
                let layer_oid = def.primitive_layers.get(i).copied().unwrap_or(0);
                let layer = def
                    .sheet_layers
                    .iter()
                    .find(|l| l.oid == layer_oid)
                    .map(|l| format!("{:?} disp={:?}", l.name, l.displayed));
                let desc = match p {
                    SymbolPrimitive::Circle { center, radius } => {
                        format!("Circle c=({:.4},{:.4}) r={:.5}", center.0, center.1, radius)
                    }
                    other => format!("{other:?}"),
                };
                let desc = if desc.len() > 90 {
                    format!("{}…", &desc[..90])
                } else {
                    desc
                };
                println!(
                    "   [{i}] layer={:?} style={:?} {}",
                    layer.map(|s| s.chars().take(40).collect::<String>()),
                    style.map(|s| (s.rgb, s.width_mm, s.dash_mm.len())),
                    desc
                );
            }
        }
    }
}
