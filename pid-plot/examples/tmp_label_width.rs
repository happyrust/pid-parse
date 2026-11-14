//! Compare one label's ink between a reference plot and our own.
//!
//! ```text
//! cargo run -p pid-plot --example tmp_label_width -- "<reference.pdf>" "<ours.pdf>" "<needle>"
//! ```

use std::path::PathBuf;

use pid_plot::read::read_plot;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reference = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: <ref.pdf> <ours.pdf> <needle>"),
    );
    let ours = PathBuf::from(
        std::env::args()
            .nth(2)
            .expect("usage: <ref.pdf> <ours.pdf> <needle>"),
    );
    let needle = std::env::args()
        .nth(3)
        .expect("usage: <ref.pdf> <ours.pdf> <needle>");
    for (what, path) in [("reference", reference), ("ours", ours)] {
        let ink = read_plot(&std::fs::read(&path)?)?;
        println!("== {what}: {} spans", ink.spans.len());
        for span in ink.spans.iter().filter(|span| span.text.contains(&needle)) {
            println!(
                "  {:?} advance={:.4} origin=[{:.3}, {:.3}] height={:.4}",
                span.text, span.advance_mm, span.origin_mm[0], span.origin_mm[1], span.height_mm
            );
        }
    }
    Ok(())
}
