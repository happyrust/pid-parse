//! Every decoded `igTextBox` the sheet carries near one paper point, so a
//! label the reference plots but our paper table lacks can be told apart
//! from one we draw at the wrong size.
//!
//! ```text
//! cargo run -p pid-parse --example probe_sheet_texts_near -- \
//!     "<drawing.pid>" <x_mm> <y_mm> [radius_mm]
//! ```

use pid_parse::{PidGraphicKind, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: <drawing.pid> <x_mm> <y_mm> [radius]");
    let x: f64 = args.next().expect("x").parse()?;
    let y: f64 = args.next().expect("y").parse()?;
    let radius: f64 = args.next().map_or(30.0, |value| value.parse().unwrap());
    let doc = PidParser::new().parse_file(std::path::Path::new(&path))?;
    let geometry = pid_parse::build_normalized_geometry(&doc);
    for entity in &geometry.entities {
        let PidGraphicKind::Text {
            insertion,
            value,
            height,
            anchor_code,
            text_box_width,
            ..
        } = &entity.kind
        else {
            continue;
        };
        let at = [insertion.x * 1000.0, insertion.y * 1000.0];
        let distance = (at[0] - x).hypot(at[1] - y);
        if distance > radius {
            continue;
        }
        println!(
            "{value:?} at ({:.2},{:.2}) h {:.3} code {:#04x} box {text_box_width:?} oid {:?} stream {:?} dist {:.2}",
            at[0],
            at[1],
            height * 1000.0,
            anchor_code,
            entity.graphic_oid,
            entity.source.stream_path,
            distance
        );
    }
    Ok(())
}
