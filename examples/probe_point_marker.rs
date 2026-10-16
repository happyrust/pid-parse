//! Resolve the point markers at the panel junctions: which terminator glyph
//! does each igPoint2d's style name, and what strokes does it hold?

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = PidParser::new().parse_file(path)?;
    let table = doc.style_tables.get("/").ok_or("no root style table")?;
    let mut n_points = 0usize;
    let mut n_markers = 0usize;
    for sheet in &doc.sheet_streams {
        if sheet.path != "/Sheet6" {
            continue;
        }
        let g = sheet.geometry.as_ref().unwrap();
        for rec in &g.decoded_igpoints {
            n_points += 1;
            let style = table.resolve_line_style(rec.index);
            let mm = |v: f64| v * 1000.0;
            let near_panel = (mm(rec.x) - 114.0).abs() < 8.0 && (mm(rec.y) - 155.0).abs() < 25.0;
            if let Some(marker) = style.and_then(|s| s.marker) {
                n_markers += 1;
                if near_panel {
                    eprintln!(
                        "pt oid={} idx={} at=({:.2},{:.2})mm -> marker sym={} term={} status={:?} strokes={}",
                        rec.oid,
                        rec.index,
                        mm(rec.x),
                        mm(rec.y),
                        marker.point_symbol_id,
                        marker.line_terminator_id,
                        marker.status,
                        marker.strokes().len()
                    );
                    for s in marker.strokes() {
                        eprintln!(
                            "    ({:.4},{:.4})-({:.4},{:.4})m",
                            s.start.0, s.start.1, s.end.0, s.end.1
                        );
                    }
                }
            } else if near_panel {
                eprintln!(
                    "pt oid={} idx={} at=({:.2},{:.2})mm -> style_id={:?} marker=None",
                    rec.oid,
                    rec.index,
                    mm(rec.x),
                    mm(rec.y),
                    style.map(|s| s.style_id)
                );
            }
        }
    }
    eprintln!("points={n_points} with-marker={n_markers}");
    Ok(())
}
