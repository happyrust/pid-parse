//! Does CPE-SQ-A0.igr carry a StyleCluster, and does index 24 resolve a fill?

use pid_parse::{ParseOptions, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A0.igr";
    let parsed = PidParser::with_options(ParseOptions::geometry()).parse_file(path)?;
    for (stream, t) in &parsed.style_tables {
        eprintln!("table stream={stream} records={}", t.records().len());
        for r in t.records() {
            eprintln!(
                "  id={} type={:#06x} name={:?}",
                r.style_id, r.type_code, r.name
            );
        }
        eprintln!(
            "resolve_fill(24) = {:?}",
            t.resolve_fill(24).map(|f| f.rgb())
        );
        eprintln!(
            "resolve_line(24) = {:?}",
            t.resolve_line_style(24)
                .map(|l| (l.symbology.width_mm(), l.dash.is_some()))
        );
    }
    Ok(())
}
