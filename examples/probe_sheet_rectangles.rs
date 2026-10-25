//! Every decoded `igRectangle2d` (`0x0020`) record a drawing's sheet streams
//! carry, with the extent it spans, so a page frame stored as a rectangle can
//! be told from a symbol body's box.
//!
//! ```text
//! cargo run -p pid-parse --example probe_sheet_rectangles -- "<file>"
//! ```
//!
//! Read-only.

use std::path::PathBuf;

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .expect("usage: probe_sheet_rectangles <file>");
    let document = PidParser::new().parse_file(PathBuf::from(&path).as_path())?;
    let mut total = 0usize;
    for sheet in &document.sheet_streams {
        let Some(geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        let rectangles = &geometry.decoded_igrectangles;
        if rectangles.is_empty() {
            continue;
        }
        total += rectangles.len();
        println!("{}: {} rectangles", sheet.path, rectangles.len());
        for record in rectangles {
            println!(
                "  oid {} layer {} origin ({:.3},{:.3}) size ({:.3},{:.3}) \
                 rotation {:.4} edges {:?}",
                record.oid,
                record.sheet_layer_ref,
                record.origin_x,
                record.origin_y,
                record.width,
                record.height(),
                record.rotation,
                record.edges,
            );
        }
    }
    println!("total {total}");
    Ok(())
}
