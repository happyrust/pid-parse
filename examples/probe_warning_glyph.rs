//! Dump the resolved marker glyph strokes for styles 56 (lsWarning) and 21
//! (lsOk) in W — the junction igPoint2d styles.

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = PidParser::new().parse_file(path)?;
    let table = doc.style_tables.get("/").ok_or("no root style table")?;
    for id in [56u32, 21] {
        let Some(rs) = table.resolve_line_style(id) else {
            eprintln!("style {id}: unresolved");
            continue;
        };
        eprintln!(
            "=== style {id} status={:?}",
            rs.marker.as_ref().map(|m| m.status)
        );
        if let Some(m) = &rs.marker {
            for (i, s) in m.strokes().iter().enumerate() {
                eprintln!(
                    "  stroke[{i}] ({:.3},{:.3})->({:.3},{:.3}) mm",
                    s.start.0 * 1000.0,
                    s.start.1 * 1000.0,
                    s.end.0 * 1000.0,
                    s.end.1 * 1000.0
                );
            }
        }
    }
    Ok(())
}
