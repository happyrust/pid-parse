//! Resolve every igPoint2d near the hydraulic junctions (x~114-118mm) and
//! print the style id / resolved marker its record names.

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
        for (i, rec) in g.decoded_igpoints.iter().enumerate() {
            let (x, y) = (rec.x * 1000.0, rec.y * 1000.0);
            if !(110.0..=125.0).contains(&x) {
                continue;
            }
            let name = table.name_of_style(rec.index).unwrap_or("?");
            let resolved = table.resolve_line_style(rec.index);
            eprintln!(
                "point[{i}] ({x:.1},{y:.1}) index={:?} name={name:?} -> symbology={:?} dash={} marker={} complex={}",
                rec.index,
                resolved.map(|r| (r.style_id, r.symbology.width_mm(), r.symbology.colour)),
                resolved.map(|r| r.dash.is_some()).unwrap_or(false),
                resolved.map(|r| r.marker.is_some()).unwrap_or(false),
                resolved.map(|r| r.complex_dash.is_some()).unwrap_or(false),
            );
        }
    }
    Ok(())
}
