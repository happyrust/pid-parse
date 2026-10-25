//! End-to-end check on the W drawing: resolve line style 28 ("Hydraulic")
//! and print the complex dash + glyph strokes.

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = PidParser::new().parse_file(path)?;
    let table = doc.style_tables.get("/").ok_or("no root style table")?;
    for sid in [27u32, 28, 52, 53, 26] {
        match table.resolve_line_style(sid) {
            Some(r) => eprintln!(
                "style {sid}: dash={:?} complex_dash={:?} marker={:?}",
                r.dash.is_some(),
                r.complex_dash.is_some(),
                r.marker.is_some()
            ),
            None => eprintln!("style {sid}: resolve_line_style -> None"),
        }
    }
    let r = table.resolve_line_style(28).unwrap();
    if let Some(cd) = r.complex_dash {
        eprintln!(
            "complex: component={} point_symbol={} period={:.5}m ({:.3}mm) strokes={}",
            cd.component_id,
            cd.point_symbol_id,
            cd.period_m,
            cd.period_mm(),
            cd.strokes().len()
        );
        for s in cd.strokes() {
            eprintln!(
                "  stroke ({:.4},{:.4})-({:.4},{:.4}) m",
                s.start.0, s.start.1, s.end.0, s.end.1
            );
        }
    }
    Ok(())
}
