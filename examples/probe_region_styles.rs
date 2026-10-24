//! Dump stroked entities intersecting a world-space rect, with resolved style.
//! Usage: probe_region_styles <drawing.pid> <x0> <y0> <x1> <y1>

use pid_parse::style_link::line_styles_for_document;
use pid_parse::{build_normalized_geometry, ParseOptions, PidGraphicKind, PidParser};

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().unwrap();
    let x0: f64 = a.next().unwrap().parse().unwrap();
    let y0: f64 = a.next().unwrap().parse().unwrap();
    let x1: f64 = a.next().unwrap().parse().unwrap();
    let y1: f64 = a.next().unwrap().parse().unwrap();
    let doc = PidParser::with_options(ParseOptions::geometry())
        .parse_file(&path)
        .unwrap();
    let index = line_styles_for_document(&doc);
    let geometry = build_normalized_geometry(&doc);
    for entity in &geometry.entities {
        let pts: Vec<(f64, f64)> = match &entity.kind {
            PidGraphicKind::Line { start, end } => vec![(start.x, start.y), (end.x, end.y)],
            PidGraphicKind::Polyline { points, .. } => points.iter().map(|p| (p.x, p.y)).collect(),
            PidGraphicKind::Circle { center, radius } => {
                vec![(center.x - radius, center.y), (center.x + radius, center.y)]
            }
            PidGraphicKind::SymbolInstance { insertion, .. } => {
                vec![(insertion.x, insertion.y)]
            }
            _ => continue,
        };
        let in_rect = pts.iter().any(|(x, y)| {
            *x * 1000.0 >= x0 && *x * 1000.0 <= x1 && *y * 1000.0 >= y0 && *y * 1000.0 <= y1
        });
        if !in_rect {
            continue;
        }
        let (stream, oid) = (entity.source.stream_path.clone(), entity.graphic_oid);
        let style = stream
            .as_ref()
            .and_then(|s| oid.map(|o| index.get(&(s.clone(), o))));
        let style_desc = style
            .flatten()
            .map(|st| {
                let [r, g, b] = st.symbology.rgb();
                format!(
                    "{:.3}mm #{r:02X}{g:02X}{b:02X}{}",
                    st.symbology.width_mm(),
                    if st.dash.is_some() { " dashed" } else { "" }
                )
            })
            .unwrap_or_else(|| "-no-style-".into());
        let kind = match &entity.kind {
            PidGraphicKind::Line { start, end } => {
                format!(
                    "Line ({:.1},{:.1})->({:.1},{:.1})",
                    start.x * 1000.0,
                    start.y * 1000.0,
                    end.x * 1000.0,
                    end.y * 1000.0
                )
            }
            PidGraphicKind::Polyline { points, closed } => {
                let xs: Vec<String> = points
                    .iter()
                    .map(|p| format!("({:.1},{:.1})", p.x * 1000.0, p.y * 1000.0))
                    .collect();
                format!(
                    "Poly{} [{}]",
                    if *closed { " closed" } else { "" },
                    xs.join(" ")
                )
            }
            PidGraphicKind::Circle { center, radius } => {
                format!(
                    "Circle ({:.1},{:.1}) r{:.1}",
                    center.x * 1000.0,
                    center.y * 1000.0,
                    radius * 1000.0
                )
            }
            PidGraphicKind::SymbolInstance {
                insertion,
                symbol_path,
                ..
            } => {
                format!(
                    "SymInst ({:.1},{:.1}) {}",
                    insertion.x * 1000.0,
                    insertion.y * 1000.0,
                    symbol_path.as_deref().unwrap_or("")
                )
            }
            _ => "?".into(),
        };
        println!(
            "{kind:60} | {style_desc:24} | {:?} {}",
            entity.id,
            entity.source.stream_path.as_deref().unwrap_or("")
        );
    }
}
