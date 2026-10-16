//! Do the pipe lines ending at the panel junctions carry a line-terminator
//! style? If the style names a JStyleLineTerminator, ref's ⊢ marks may be
//! end-of-line glyphs rather than point markers.

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = PidParser::new().parse_file(path)?;
    let table = doc.style_tables.get("/").ok_or("no root style table")?;
    for sheet in &doc.sheet_streams {
        if sheet.path != "/Sheet6" {
            continue;
        }
        let g = sheet.geometry.as_ref().unwrap();
        for (i, rec) in g.decoded_iglinestrings.iter().enumerate() {
            // records ending near the junctions x~114-118, y~130-162
            let mm = |v: f64| v * 1000.0;
            let near = rec
                .vertex_xs
                .iter()
                .zip(rec.vertex_ys.iter())
                .any(|(x, y)| (mm(*x) - 116.0).abs() < 4.0 && (mm(*y) - 146.0).abs() < 34.0);
            if !near {
                continue;
            }
            let style = table.resolve_line_style(rec.index);
            let dash_desc = style
                .map(|s| format!("{:?}", s.dash))
                .unwrap_or_else(|| "?".to_string());
            eprintln!(
                "linestr[{i}] oid={} idx={} verts={} end=({:.1},{:.1}) style_id={:?} dash={} marker={} name={:?}",
                rec.oid,
                rec.index,
                rec.vertex_xs.len(),
                mm(*rec.vertex_xs.last().unwrap()),
                mm(*rec.vertex_ys.last().unwrap()),
                style.map(|s| s.style_id),
                dash_desc,
                style.and_then(|s| s.marker).map_or("none".to_string(), |m| format!(
                    "sym={} term={} status={:?} strokes={}",
                    m.point_symbol_id,
                    m.line_terminator_id,
                    m.status,
                    m.strokes().len()
                )),
                style.and_then(|s| table.name_of_style(s.style_id)),
            );
            if let Some(m) = style.and_then(|s| s.marker) {
                for s in m.strokes() {
                    eprintln!(
                        "    stroke ({:.4},{:.4})-({:.4},{:.4})",
                        s.start.0, s.start.1, s.end.0, s.end.1
                    );
                }
            }
        }
    }
    Ok(())
}
