//! What remains undecoded in the .igr after the boundary-variant fix?

use pid_parse::{build_normalized_geometry, ParseOptions, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in [
        r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A0.igr",
        r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A1.igr",
    ] {
        let parsed = PidParser::with_options(ParseOptions::geometry()).parse_file(path)?;
        let geo = build_normalized_geometry(&parsed);
        println!("{path}");
        for r in &geo.refused_graphic_records {
            println!(
                "  refused stream={} type={:#06x} class={:?} n={}",
                r.stream_path, r.type_code, r.rad_class_name, r.count
            );
        }
        for r in &geo.dropped_graphic_records {
            println!(
                "  dropped stream={} type={:#06x} class={:?} n={}",
                r.stream_path, r.type_code, r.rad_class_name, r.count
            );
        }
        println!("  entities={}", geo.entities.len());
    }
    Ok(())
}
