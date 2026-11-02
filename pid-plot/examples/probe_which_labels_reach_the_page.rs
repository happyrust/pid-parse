//! Which labels a drawing records reach the page, and where do the others stop?
//!
//! The A2 reference plot read as "`TICA` letters as `TT` here, and the
//! `H=-15℃` line is not drawn at all" until the three stages were asked
//! separately: the record decoded out of the `igTextBox`, the graphic entity
//! built from it, and the paper primitive laid out from that. Every label this
//! page was said to be missing was present at all three — the gap was its
//! *position*, not its existence
//! (`docs/analysis/2026-11-02-the-a2-labels-were-already-drawn.md`).
//!
//! ```text
//! cargo run -p pid-plot --example probe_which_labels_reach_the_page -- \
//!     "<drawing.pid>" "TICA,-15,TT" ["<corpus>"]
//! ```
//!
//! Read-only: parse, layout, print.

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pid_parse::{PidGraphicKind, PidParser, build_normalized_geometry};
use pid_plot::model::{PaperShape, lay_out_drawing};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let (Some(pid), Some(needles)) = (args.next(), args.next()) else {
        eprintln!(
            "usage: probe_which_labels_reach_the_page <drawing.pid> <needle[,needle…]> [corpus]"
        );
        return ExitCode::from(2);
    };
    let corpus = args
        .next()
        .map(PathBuf::from)
        .or_else(|| env::var_os("PID_PARSE_SELF_CHECK_DIR").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(r"D:\work\cad\PID自研软件验证"));
    let needles: Vec<String> = needles
        .split(',')
        .map(str::trim)
        .filter(|needle| !needle.is_empty())
        .map(str::to_string)
        .collect();
    let matches = |value: &str| needles.iter().any(|needle| value.contains(needle));

    let doc = match PidParser::new().parse_file(Path::new(&pid)) {
        Ok(doc) => doc,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };

    println!("== decoded igTextBox records");
    for stream in &doc.sheet_streams {
        let Some(geometry) = stream.geometry.as_ref() else {
            continue;
        };
        for record in &geometry.decoded_igtextboxes {
            if !matches(&record.text) {
                continue;
            }
            println!(
                "  {}\toid {}\tlayer {}\t{:?}\tat [{:.3},{:.3}] m\tbytes {}..{}",
                stream.path,
                record.oid,
                record.sheet_layer_ref,
                record.text,
                record.trailing_double_1,
                record.trailing_double_2,
                record.byte_start,
                record.byte_end,
            );
        }
    }

    let geometry = build_normalized_geometry(&doc);
    println!("\n== text entities");
    for entity in &geometry.entities {
        let PidGraphicKind::Text {
            insertion, value, ..
        } = &entity.kind
        else {
            continue;
        };
        if !matches(value) {
            continue;
        }
        println!(
            "  {}\t{:?}\tat [{:.3},{:.3}] m\tlayer {:?} displayed {:?}\tstream {:?}",
            entity.id,
            value,
            insertion.x,
            insertion.y,
            entity
                .source_layer
                .as_ref()
                .and_then(|layer| layer.name.clone()),
            entity
                .source_layer
                .as_ref()
                .and_then(|layer| layer.displayed),
            entity.source.stream_path,
        );
    }

    let model = match lay_out_drawing(Path::new(&pid), &corpus) {
        Ok(model) => model,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    println!("\n== paper text primitives");
    for primitive in &model.primitives {
        let PaperShape::Text {
            origin_mm,
            height_mm,
            value,
            rotation,
            anchor,
            line,
            ..
        } = &primitive.shape
        else {
            continue;
        };
        if !matches(value) {
            continue;
        }
        println!(
            "  {:?}\tat [{:.3},{:.3}] mm\th {height_mm:?}\trot {:.0}°\tanchor {anchor:?}\tline {line:?}",
            value,
            origin_mm[0],
            origin_mm[1],
            rotation.to_degrees(),
        );
    }
    ExitCode::SUCCESS
}
