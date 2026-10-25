//! Dump the 51 Line entities in the .igr logo zone — are they the
//! letterform outlines (duplicating the boundary fills) or other detail?

use pid_parse::{build_normalized_geometry, ParseOptions, PidGraphicKind, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A1.igr";
    let parsed = PidParser::with_options(ParseOptions::geometry()).parse_file(path)?;
    let geo = build_normalized_geometry(&parsed);
    for e in &geo.entities {
        let PidGraphicKind::Line { start, end } = &e.kind else {
            continue;
        };
        let cx = (start.x + end.x) / 2.0;
        let cy = (start.y + end.y) / 2.0;
        if !(0.69..=0.72).contains(&cx) || !(0.070..=0.082).contains(&cy) {
            continue;
        }
        println!(
            "  ({:.4},{:.4})->({:.4},{:.4}) oid={:?} record={:?}",
            start.x * 1000.0,
            start.y * 1000.0,
            end.x * 1000.0,
            end.y * 1000.0,
            e.graphic_oid,
            e.source.record_id
        );
    }
    Ok(())
}
