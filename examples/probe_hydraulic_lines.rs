//! All iglinestring2d records whose style resolves to the Hydraulic dash —
//! their vertices, so the tick phase can be measured against line start.

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
            if rec.index != 28 {
                continue;
            }
            let mm = |v: f64| v * 1000.0;
            eprint!("linestr[{i}] oid={} verts:", rec.oid);
            for (x, y) in rec.vertex_xs.iter().zip(rec.vertex_ys.iter()) {
                eprint!(" ({:.1},{:.1})", mm(*x), mm(*y));
            }
            eprintln!();
        }
    }
    Ok(())
}
