//! The `igEllipticalArc2d` (`0x007E`) records a `.sym` body carries.
//!
//! [`pid_parse::symbol_library::read_symbol_geometry`] skips `0x007E`, so a
//! body whose dome is stored as an elliptical arc draws without it. This
//! prints what the record says — centre, radii, sweep — so the shape the
//! library asks for can be compared with the drawing's own cached body.
//!
//! ```text
//! cargo run -p pid-parse --example probe_sym_elliptical_arcs -- "<symbol.sym>"
//! ```

use std::path::Path;

use pid_parse::parsers::sheet_records::decode_igellipticalarcs;
use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("usage: <symbol.sym>");
    let package = PidParser::new().parse_package(Path::new(&path))?;
    for (stream_path, raw) in &package.streams {
        if !stream_path
            .rsplit('/')
            .next()
            .unwrap_or("")
            .starts_with("Sheet")
        {
            continue;
        }
        for arc in decode_igellipticalarcs(&raw.data) {
            println!("{stream_path}: {arc:?}");
        }
    }
    Ok(())
}
