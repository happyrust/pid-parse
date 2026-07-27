//! Print one symbol's decoded body as CSV, so the shape can be drawn and
//! checked against what the symbol is supposed to look like.
//!
//! A valve that decodes into plausible-looking numbers can still be wrong --
//! an arc read with the wrong angle convention produces its own complement,
//! which is only obvious when you draw it. This dumps the geometry in a form
//! a plotting script can consume:
//!
//! ```text
//! line,x1,y1,x2,y2
//! circle,cx,cy,r
//! arc,cx,cy,r,start_radians,end_radians
//! poly,x1,y1,x2,y2,...
//! ```
//!
//! Usage: `cargo run --example dump_symbol_geometry -- <path-to.sym>`

use std::path::PathBuf;

use pid_parse::symbol_library::{read_symbol_geometry, SymbolPrimitive};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(arg) = std::env::args().nth(1) else {
        eprintln!("usage: dump_symbol_geometry <path-to.sym>");
        return Ok(());
    };
    let path = PathBuf::from(arg);
    let body = read_symbol_geometry(&path)?;

    for primitive in &body.primitives {
        match primitive {
            SymbolPrimitive::Line { start, end } => {
                println!("line,{},{},{},{}", start.0, start.1, end.0, end.1);
            }
            SymbolPrimitive::Circle { center, radius } => {
                println!("circle,{},{},{radius}", center.0, center.1);
            }
            SymbolPrimitive::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                println!(
                    "arc,{},{},{radius},{start_angle},{end_angle}",
                    center.0, center.1
                );
            }
            SymbolPrimitive::Polyline { vertices } => {
                let coords: Vec<String> = vertices
                    .iter()
                    .flat_map(|(x, y)| [x.to_string(), y.to_string()])
                    .collect();
                println!("poly,{}", coords.join(","));
            }
            SymbolPrimitive::Text { text, at } => {
                // Quoted last so a comma or a newline in the label cannot be
                // read as another column.
                println!("text,{},{},{text:?}", at.0, at.1);
            }
        }
    }
    eprintln!(
        "{} primitives, skipped {:?}",
        body.primitives.len(),
        body.skipped_records
    );
    Ok(())
}
