//! Whether the coordinates an `EndpointPair` names fit the sheet at some unit
//! scale — the question behind "these look like noise": a raw f64 pair read
//! without its unit conversion lands thousands of times too far out, and the
//! right divisor would put it back on the paper.
//!
//! ```text
//! cargo run -p pid-parse --example probe_endpoint_pair_scale -- "<file.pid>"
//! ```
//!
//! Read-only.

use std::path::PathBuf;

use pid_parse::{PidGraphicKind, PidParser, SheetRecordKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pid = std::env::args().nth(1).expect("usage: <file.pid>");
    let document = PidParser::new().parse_file(PathBuf::from(&pid).as_path())?;
    let geometry = pid_parse::build_normalized_geometry(&document);
    let Some((width_mm, height_mm)) = geometry.page_dimensions_mm else {
        println!("{pid}: the drawing states no page size");
        return Ok(());
    };
    // The page in metres: the model's own unit for entity coordinates.
    let page = [width_mm / 1000.0, height_mm / 1000.0];
    // Candidate divisors from the raw double to metres.
    let scales: [(&str, f64); 5] = [
        ("x1 (metres)", 1.0),
        ("x1e-2", 1e-2),
        ("x1e-3 (mm)", 1e-3),
        ("x1e-6 (µm)", 1e-6),
        ("x1/300in", 25.4e-3 / 300.0),
    ];
    let mut counts = [0usize; 5];
    let mut pairs = 0usize;
    let mut spans = Vec::new();
    for entity in &geometry.entities {
        if entity.source.record_kind != Some(SheetRecordKind::EndpointPair) {
            continue;
        }
        let PidGraphicKind::Line { start, end } = &entity.kind else {
            continue;
        };
        pairs += 1;
        let points = [[start.x, start.y], [end.x, end.y]];
        for (index, (_, scale)) in scales.iter().enumerate() {
            let fits = points.iter().all(|point| {
                let x = point[0] * scale;
                let y = point[1] * scale;
                (0.0..=page[0]).contains(&x) && (0.0..=page[1]).contains(&y)
            });
            if fits {
                counts[index] += 1;
            }
        }
        spans.push(((start.x, start.y), (end.x, end.y)));
    }
    println!(
        "{pid}: {} EndpointPair lines, page {width_mm:.1}x{height_mm:.1} mm",
        pairs
    );
    for (index, (label, _)) in scales.iter().enumerate() {
        println!(
            "  {label:14} both ends inside the page: {}/{pairs}",
            counts[index]
        );
    }
    spans.sort_by(|left, right| right.0 .0.abs().total_cmp(&left.0 .0.abs()));
    for ((ax, ay), (bx, by)) in spans.iter().take(8) {
        println!(
            "  ({ax:.1},{ay:.1}) -> ({bx:.1},{by:.1})   /1000 = ({:.1},{:.1}) -> ({:.1},{:.1})",
            ax / 1000.0,
            ay / 1000.0,
            bx / 1000.0,
            by / 1000.0
        );
    }
    Ok(())
}
