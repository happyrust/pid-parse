//! The `igEllipticalArc2d` (`0x007E`) records a `.sym` body carries.
//!
//! `symbol_library::decode_primitive`'s `TYPE_ELLIPTICALARC` arm reads
//! `0x007E` now, so a body whose dome is stored as an elliptical arc draws
//! it as a `BSpline` primitive. This prints what the record says — centre,
//! major axis, ratio, sweep, direction flag — so the shape the library
//! reads can be compared byte-for-byte with the drawing's own cached body
//! (`src/geometry.rs`'s `elliptical_arcs` path over the same record type).
//!
//! `2to1Parametric V Drum.sym`, `Diaphragm Actuator.sym` and
//! `PCV-Self-Actuated Regulator Valve.sym` are the three drawing-used
//! symbols carrying `0x007E`; extract them from `RefData~4~681.zip` (see
//! `src/refdata_archive.rs`) to run this against each.
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
