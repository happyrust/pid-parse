//! Find the source record for the junction connector (vertical line
//! ~(114.8,161.7)-(114.8,168.4)) and print its style resolution.

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = PidParser::new().parse_file(path)?;
    let table = doc.style_tables.get("/").ok_or("no root")?;
    for sheet in &doc.sheet_streams {
        if sheet.path != "/Sheet6" {
            continue;
        }
        let g = sheet.geometry.as_ref().unwrap();
        for (i, rec) in g.decoded_iglines.iter().enumerate() {
            let (x0, y0, x1, y1) = (
                rec.start_x * 1000.0,
                rec.start_y * 1000.0,
                rec.end_x * 1000.0,
                rec.end_y * 1000.0,
            );
            let near = |x: f64, y: f64| (x - 114.8).abs() < 0.3 && (y - 161.7).abs() < 0.3;
            if near(x0, y0) || near(x1, y1) {
                let name = table.name_of_style(rec.index).unwrap_or("?");
                let r = table.resolve_line_style(rec.index);
                eprintln!(
                    "line[{i}] ({x0:.2},{y0:.2})->({x1:.2},{y1:.2}) index={} name={name:?} dash={} complex={} marker={}",
                    rec.index,
                    r.map(|s| s.dash.is_some()).unwrap_or(false),
                    r.map(|s| s.complex_dash.is_some()).unwrap_or(false),
                    r.map(|s| s.marker.is_some()).unwrap_or(false),
                );
            }
        }
        for (i, rec) in g.decoded_iglinestrings.iter().enumerate() {
            let first = (
                rec.vertex_xs.first().copied().unwrap_or(0.0) * 1000.0,
                rec.vertex_ys.first().copied().unwrap_or(0.0) * 1000.0,
            );
            if (first.0 - 114.8).abs() < 0.3 && (first.1 - 161.7).abs() < 0.3 {
                let name = table.name_of_style(rec.index).unwrap_or("?");
                let r = table.resolve_line_style(rec.index);
                eprintln!(
                    "linestr[{i}] start={first:?} index={} name={name:?} dash={} complex={} marker={} form={} scope={}",
                    rec.index,
                    r.map(|s| s.dash.is_some()).unwrap_or(false),
                    r.map(|s| s.complex_dash.is_some()).unwrap_or(false),
                    r.map(|s| s.marker.is_some()).unwrap_or(false),
                    rec.form, rec.scope,
                );
            }
        }
    }
    Ok(())
}
