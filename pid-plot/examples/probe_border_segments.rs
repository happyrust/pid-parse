//! The segments a plot file reads that sit in the page's outermost band, so a
//! page frame can be told from drawing content.
//!
//! ```text
//! cargo run -p pid-plot --example probe_border_segments -- "<plot.pdf>" [band_mm]
//! ```
//!
//! Read-only. The reader drops a stroke that runs along the sheet frame
//! (`read::sheet_edge_mm`), so the reference's two frame rectangles never reach
//! this list — the rows that do reach it are the title block and the signature
//! column, which `template_masks` then masks out.

use std::path::PathBuf;

use pid_plot::read::read_plot;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .expect("usage: <plot.pdf> [band_mm]");
    let band: f64 = std::env::args()
        .nth(2)
        .map_or(1.0, |value| value.parse().expect("band_mm"));
    let ink = read_plot(&std::fs::read(PathBuf::from(&path))?)?;
    let [width, height] = ink.page_mm;
    let mut rows = Vec::new();
    for segment in &ink.segments {
        let (dx, dy) = (
            segment.end_mm[0] - segment.start_mm[0],
            segment.end_mm[1] - segment.start_mm[1],
        );
        let length = (dx * dx + dy * dy).sqrt();
        let near = [segment.start_mm, segment.end_mm].iter().any(|point| {
            point[0] < band
                || point[1] < band
                || point[0] > width - band
                || point[1] > height - band
        });
        if near {
            rows.push((length, segment.start_mm, segment.end_mm, segment.width_mm));
        }
    }
    rows.sort_by(|left, right| right.0.total_cmp(&left.0));
    let total: f64 = rows.iter().map(|row| row.0).sum();
    println!(
        "page {width:.3}x{height:.3} band {band:.2} mm: {} of {} segments, {total:.1} mm",
        rows.len(),
        ink.segments.len()
    );
    for (length, start, end, stroke) in rows.iter().take(16) {
        println!(
            "  {length:8.3}  ({:8.3},{:8.3}) -> ({:8.3},{:8.3})  stroke {stroke:.3}",
            start[0], start[1], end[0], end[1]
        );
    }
    Ok(())
}
