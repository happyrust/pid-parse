//! What a linked `.igr` border template carries: page extent, entity census,
//! layers, and every label with its insertion point and resolved height.
//!
//! ```text
//! cargo run --example probe_igr_template -- <a.igr|dir> [more ...]
//! ```
//!
//! The template is an OLE/IGDS drawing in its own right, so it parses through
//! the same [`PidParser`] as a `.pid`. Read-only.

use std::path::{Path, PathBuf};

use pid_parse::style_link::text_styles_for_document;
use pid_parse::{build_normalized_geometry, ParseOptions, PidGraphicKind, PidParser};

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut children: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        children.sort();
        for child in children {
            collect(&child, out);
        }
    } else if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("igr"))
    {
        out.push(path.to_path_buf());
    }
}

fn bbox(points: &[[f64; 2]]) -> Option<([f64; 2], [f64; 2])> {
    let mut min = [f64::MAX; 2];
    let mut max = [f64::MIN; 2];
    let mut seen = false;
    for point in points {
        if point[0].is_finite() && point[1].is_finite() {
            min[0] = min[0].min(point[0]);
            min[1] = min[1].min(point[1]);
            max[0] = max[0].max(point[0]);
            max[1] = max[1].max(point[1]);
            seen = true;
        }
    }
    seen.then_some((min, max))
}

fn main() {
    let mut files = Vec::new();
    for arg in std::env::args().skip(1) {
        collect(Path::new(&arg), &mut files);
    }
    if files.is_empty() {
        eprintln!("usage: probe_igr_template <a.igr|dir> [more ...]");
        std::process::exit(2);
    }
    for file in &files {
        println!("\n========== {}", file.display());
        let doc = match PidParser::with_options(ParseOptions::geometry()).parse_file(file) {
            Ok(doc) => doc,
            Err(error) => {
                println!("  parse failed: {error}");
                continue;
            }
        };
        let geometry = build_normalized_geometry(&doc);
        println!(
            "  page {:?} m  entities {}  symbol definitions {}",
            geometry.page_dimensions_mm,
            geometry.entities.len(),
            geometry.symbol_definitions.len()
        );
        let styles = text_styles_for_document(&doc);
        let mut kinds: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        let mut points: Vec<[f64; 2]> = Vec::new();
        let mut texts = Vec::new();
        let mut layers: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for entity in &geometry.entities {
            let kind = match &entity.kind {
                PidGraphicKind::Line { start, end } => {
                    points.push([start.x * 1000.0, start.y * 1000.0]);
                    points.push([end.x * 1000.0, end.y * 1000.0]);
                    "Line"
                }
                PidGraphicKind::Polyline { points: chain, .. } => {
                    points.extend(chain.iter().map(|p| [p.x * 1000.0, p.y * 1000.0]));
                    "Polyline"
                }
                PidGraphicKind::Arc { center, radius, .. } => {
                    points.push([(center.x - radius) * 1000.0, (center.y - radius) * 1000.0]);
                    points.push([(center.x + radius) * 1000.0, (center.y + radius) * 1000.0]);
                    "Arc"
                }
                PidGraphicKind::Circle { center, radius } => {
                    points.push([(center.x - radius) * 1000.0, (center.y - radius) * 1000.0]);
                    points.push([(center.x + radius) * 1000.0, (center.y + radius) * 1000.0]);
                    "Circle"
                }
                PidGraphicKind::Point { position } => {
                    points.push([position.x * 1000.0, position.y * 1000.0]);
                    "Point"
                }
                PidGraphicKind::Text {
                    insertion,
                    height,
                    value,
                    ..
                } => {
                    let at = [insertion.x * 1000.0, insertion.y * 1000.0];
                    points.push(at);
                    let resolved = entity
                        .graphic_oid
                        .zip(entity.source.stream_path.clone())
                        .and_then(|(oid, stream)| styles.get(&(stream, oid)).cloned());
                    texts.push((
                        at,
                        Some(height * 1000.0),
                        resolved
                            .as_ref()
                            .and_then(|style| style.effective())
                            .map(|height| height.height_m * 1000.0),
                        value.clone(),
                        resolved
                            .as_ref()
                            .and_then(|style| style.effective())
                            .and_then(|height| height.font_name.clone()),
                    ));
                    "Text"
                }
                PidGraphicKind::SymbolInstance { .. } => "Symbol",
                _ => "Other",
            };
            *kinds.entry(kind).or_default() += 1;
            let layer = entity
                .source_layer
                .as_ref()
                .and_then(|layer| layer.name.clone())
                .unwrap_or_else(|| "?".to_string());
            *layers.entry(layer).or_default() += 1;
        }
        println!("  kinds {kinds:?}");
        println!("  layers {layers:?}");
        if let Some((min, max)) = bbox(&points) {
            println!(
                "  ink bbox ({:.2}, {:.2}) .. ({:.2}, {:.2}) mm",
                min[0], min[1], max[0], max[1]
            );
        }
        println!("  texts {}", texts.len());
        for (at, height, resolved, value, font) in texts.iter().take(40) {
            println!(
                "    ({:8.2},{:8.2}) h {:?} resolved {:?} {:?} {:?}",
                at[0], at[1], height, resolved, value, font
            );
        }
    }
}
