//! Dump the .igr igArc2d entities in the logo zone — angular fields.

use pid_parse::{build_normalized_geometry, ParseOptions, PidGraphicKind, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A1.igr";
    let parsed = PidParser::with_options(ParseOptions::geometry()).parse_file(path)?;
    let geo = build_normalized_geometry(&parsed);
    for e in &geo.entities {
        let PidGraphicKind::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            clockwise,
        } = &e.kind
        else {
            continue;
        };
        if !(0.69..=0.72).contains(&center.x) || !(0.070..=0.082).contains(&center.y) {
            continue;
        }
        println!(
            "  c=({:.4},{:.4}) r={:.4} ang=({:.4}..{:.4}) {clockwise} oid={:?} rec={:?}",
            center.x * 1000.0,
            center.y * 1000.0,
            radius * 1000.0,
            start_angle,
            end_angle,
            e.graphic_oid,
            e.source.record_id
        );
    }
    Ok(())
}
