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
//! A row whose record names a line style in the symbol's own `StyleCluster`
//! carries a trailing `@RRGGBB:WW` token, `WW` being the width in hundredths
//! of a millimetre — the same shape OpenCADStudio's drawing-level dump uses,
//! so one plotting script reads either. No token means the record's style
//! index named something with no line symbology.
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

    for styled in &body.primitives {
        // The colour and width the symbol states for this stroke, appended so
        // a plot can draw them; absent where the record's style index names
        // no line style. Same trailing-token shape the drawing-level dump
        // uses, so one plotting script reads either.
        let style = styled.style.map_or_else(String::new, |style| {
            let [r, g, b] = style.rgb;
            format!(
                ",@{r:02X}{g:02X}{b:02X}:{}",
                (style.width_mm * 100.0).round() as i64
            )
        });
        match &styled.primitive {
            SymbolPrimitive::Line { start, end } => {
                println!("line,{},{},{},{}{style}", start.0, start.1, end.0, end.1);
            }
            SymbolPrimitive::Circle { center, radius } => {
                println!("circle,{},{},{radius}{style}", center.0, center.1);
            }
            SymbolPrimitive::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                println!(
                    "arc,{},{},{radius},{start_angle},{end_angle}{style}",
                    center.0, center.1
                );
            }
            SymbolPrimitive::Polyline { vertices } => {
                let coords: Vec<String> = vertices
                    .iter()
                    .flat_map(|(x, y)| [x.to_string(), y.to_string()])
                    .collect();
                println!("poly,{}{style}", coords.join(","));
            }
            SymbolPrimitive::Text { text, at } => {
                // Height and rotation are zero because the record carries
                // neither; the columns are there so a drawing-level dump can
                // use the same row shape. Quoted so a comma or a newline in
                // the label cannot be read as another column.
                println!("text,{},{},0,0,{text:?}{style}", at.0, at.1);
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
