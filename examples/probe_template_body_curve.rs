//! What body does a border template cache for the symbol it places?
//!
//! `probe_igr_template` counts entities; this one goes into the nested
//! `LdcSite` storages and prints what each body is made of, family by family
//! and record by record, in the body's own metres. A placement names one
//! `JSheet` of one storage (`igSymbol2d` tail), so the question "did the print
//! use the cached body or the `.sym` library's" starts here: if the cache holds
//! a curve family the printer could not reproduce chord for chord, the two
//! bodies are not the same shape.
//!
//! ```text
//! cargo run --example probe_template_body_curve -- <a.igr> [more ...]
//! ```
//!
//! Read-only.

use std::path::Path;

use pid_parse::symbol_library::SymbolPrimitive;
use pid_parse::{ParseOptions, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut files: Vec<String> = std::env::args().skip(1).collect();
    if files.is_empty() {
        files.push(r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A0.igr".into());
        files.push(r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A1.igr".into());
    }
    for path in files {
        let path = Path::new(&path);
        println!("\n========== {}", path.display());
        let doc = PidParser::with_options(ParseOptions::geometry()).parse_file(path)?;
        for (index, site) in doc.jsites.iter().enumerate() {
            let Some(nested) = site.nested_geometry.as_ref() else {
                continue;
            };
            if nested.is_empty() {
                continue;
            }
            println!(
                "\n  [{index}] {} path {}  circles {} arcs {} lines {} polylines {} texts {} \
                 rects {} bsplines {} ellipses {} boundaries {} dims {}",
                site.name,
                site.path,
                nested.circles.len(),
                nested.arcs.len(),
                nested.lines.len(),
                nested.polylines.len(),
                nested.texts.len(),
                nested.rectangles.len(),
                nested.bsplines.len(),
                nested.elliptical_arcs.len(),
                nested.boundaries.len(),
                nested.dimensions.len(),
            );
            println!("      sheets {:?}", nested.sheets);
            for definition in &nested.definitions {
                println!(
                    "      body sheet {} manager {} layers {:?}",
                    definition.sheet_oid, definition.manager_oid, definition.layers
                );
            }
            // Micrometres, not metres: a body's radii are whole microns (1450 µm,
            // 1050 µm) and a four-decimal metre print rounds 1450 to "0.0015"
            // and 1050 to "0.0011", which reads like different geometry.
            for arc in &nested.arcs {
                println!(
                    "      arc   oid {} layer {} centre ({:.1},{:.1}) µm  r {:.1} µm  \
                     {:.6} -> {:.6} rad  flag {}",
                    arc.oid,
                    arc.sheet_layer_ref,
                    arc.center_x * 1e6,
                    arc.center_y * 1e6,
                    arc.radius * 1e6,
                    arc.start_angle,
                    arc.end_angle,
                    arc.flag,
                );
            }
            for bspline in &nested.bsplines {
                println!(
                    "      bspl  oid {} layer {} degree {} poles {}",
                    bspline.oid,
                    bspline.sheet_layer_ref,
                    bspline.degree(),
                    bspline.poles().len(),
                );
            }
            for curve in &nested.elliptical_arcs {
                println!(
                    "      ellip oid {} layer {} centre ({:.1},{:.1}) µm  major {:.1} µm  \
                     ratio {:.6}",
                    curve.oid,
                    curve.sheet_layer_ref,
                    curve.center_x * 1e6,
                    curve.center_y * 1e6,
                    curve.major_x.hypot(curve.major_y) * 1e6,
                    curve.ratio,
                );
            }
        }
        // Every visible primitive of every embedded body, as the renderer sees
        // it: this is what a placement can draw without the `.sym`.
        let geometry = pid_parse::build_normalized_geometry(&doc);
        println!(
            "\n  symbol definitions: {}",
            geometry.symbol_definitions.len()
        );
        for definition in &geometry.symbol_definitions {
            println!(
                "    sheet {} primitives {} (visible {})",
                definition.reference.sheet,
                definition.primitives.len(),
                definition.visible_primitives().count()
            );
            for (kind, line) in definition
                .primitives
                .iter()
                .zip(primitive_lines(&definition.primitives))
            {
                let _ = kind;
                println!("      {line}");
            }
        }
    }
    Ok(())
}

fn primitive_lines(primitives: &[SymbolPrimitive]) -> Vec<String> {
    primitives.iter().map(describe).collect()
}

fn describe(primitive: &SymbolPrimitive) -> String {
    match primitive {
        SymbolPrimitive::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            clockwise,
        } => format!(
            "arc   centre ({:.1},{:.1}) µm  r {:.1} µm  {start_angle:.6} -> \
             {end_angle:.6} rad  cw {clockwise}",
            center.0 * 1e6,
            center.1 * 1e6,
            radius * 1e6
        ),
        SymbolPrimitive::Circle { center, radius } => format!(
            "circle centre ({:.1},{:.1}) µm  r {:.1} µm",
            center.0 * 1e6,
            center.1 * 1e6,
            radius * 1e6
        ),
        SymbolPrimitive::Line { start, end } => format!(
            "line  ({:.1},{:.1}) -> ({:.1},{:.1}) µm",
            start.0 * 1e6,
            start.1 * 1e6,
            end.0 * 1e6,
            end.1 * 1e6
        ),
        SymbolPrimitive::Polyline {
            vertices,
            is_closed,
        } => format!(
            "poly  {} points closed {is_closed} {:?} µm",
            vertices.len(),
            vertices
                .iter()
                .map(|point| (point.0 * 1e6, point.1 * 1e6))
                .collect::<Vec<_>>()
        ),
        SymbolPrimitive::BSpline { poles, knots, .. } => format!(
            "bspl  degree {} poles {:?} µm  knots {}",
            knots.len().saturating_sub(poles.len()).saturating_sub(1),
            poles
                .iter()
                .map(|pole| (pole.0 * 1e6, pole.1 * 1e6))
                .collect::<Vec<_>>(),
            knots.len()
        ),
        other => format!("{other:?}"),
    }
}
