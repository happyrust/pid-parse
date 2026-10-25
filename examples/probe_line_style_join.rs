//! How much of a drawing's line work reaches a line style?
//!
//! `pid-plot` writes every stroke at one width and one colour, while
//! `SmartPlant`'s own plots carry three widths and the palette's colours. Before
//! the writer can draw styled strokes, the join has to be worth drawing from:
//! [`line_styles_for_document`] resolves `igLine2d` / `igLineString2d` /
//! `igPoint2d` and the symbol placements by `(stream, oid)`, and this probe
//! counts how many of the geometry's stroked entities that actually covers,
//! plus the palette it lands on.
//!
//! Usage: `cargo run --example probe_line_style_join -- <drawing.pid>…`

use std::collections::BTreeMap;

use pid_parse::style_link::line_styles_for_document;
use pid_parse::{build_normalized_geometry, ParseOptions, PidGraphicKind, PidParser};

fn main() {
    for path in std::env::args().skip(1) {
        let doc = match PidParser::with_options(ParseOptions::geometry()).parse_file(&path) {
            Ok(doc) => doc,
            Err(error) => {
                println!("{path}: parse failed: {error}");
                continue;
            }
        };
        let index = line_styles_for_document(&doc);
        let geometry = build_normalized_geometry(&doc);
        let mut stroked = 0usize;
        let mut resolved = 0usize;
        let mut dashes = 0usize;
        let mut palette: BTreeMap<String, usize> = BTreeMap::new();
        for entity in &geometry.entities {
            if !matches!(
                entity.kind,
                PidGraphicKind::Line { .. }
                    | PidGraphicKind::Polyline { .. }
                    | PidGraphicKind::Arc { .. }
                    | PidGraphicKind::Circle { .. }
                    | PidGraphicKind::SymbolInstance { .. }
            ) {
                continue;
            }
            stroked += 1;
            let (Some(stream), Some(oid)) = (entity.source.stream_path.clone(), entity.graphic_oid)
            else {
                continue;
            };
            let Some(style) = index.get(&(stream, oid)) else {
                continue;
            };
            resolved += 1;
            if style.dash.is_some() {
                dashes += 1;
            }
            let [r, g, b] = style.symbology.rgb();
            *palette
                .entry(format!(
                    "{:.3}mm #{r:02X}{g:02X}{b:02X}",
                    style.symbology.width_mm()
                ))
                .or_default() += 1;
        }
        println!(
            "{path}\n  stroked entities {stroked}, resolved to a line style {resolved}, of those dashed {dashes}"
        );
        for (palette, count) in &palette {
            println!("    {count:5} {palette}");
        }
    }
}
