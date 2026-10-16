//! Find every sheet line/polyline whose resolved style has complex_dash,
//! and every one whose librarian name mentions Hydraulic.

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = PidParser::new().parse_file(path)?;
    let table = doc.style_tables.get("/").ok_or("no root")?;
    // 1) which style ids resolve to complex_dash
    for sheet in &doc.sheet_streams {
        if sheet.path != "/Sheet6" {
            continue;
        }
        let g = sheet.geometry.as_ref().unwrap();
        let mut seen = std::collections::BTreeMap::new();
        for rec in &g.decoded_iglinestrings {
            *seen.entry(rec.index).or_insert(0usize) += 1;
        }
        for rec in &g.decoded_iglines {
            *seen.entry(rec.index).or_insert(0usize) += 1;
        }
        for (idx, n) in &seen {
            let name = table.name_of_style(*idx).unwrap_or("?");
            let r = table.resolve_line_style(*idx);
            eprintln!(
                "index={idx} n={n} name={name:?} dash={} complex={} marker={}",
                r.map(|s| s.dash.is_some()).unwrap_or(false),
                r.map(|s| s.complex_dash.is_some()).unwrap_or(false),
                r.map(|s| s.marker.is_some()).unwrap_or(false),
            );
        }
    }
    Ok(())
}
