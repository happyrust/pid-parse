//! Print the vertex runs of every style-28 (Hydraulic) linestring in W.

use pid_parse::PidParser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = PidParser::new().parse_file(path)?;
    for sheet in &doc.sheet_streams {
        if sheet.path != "/Sheet6" {
            continue;
        }
        let g = sheet.geometry.as_ref().unwrap();
        for (i, rec) in g.decoded_iglinestrings.iter().enumerate() {
            if rec.index != 28 {
                continue;
            }
            let vs: Vec<String> = rec
                .vertex_xs
                .iter()
                .zip(&rec.vertex_ys)
                .map(|(x, y)| format!("({:.1},{:.1})", x * 1000.0, y * 1000.0))
                .collect();
            eprintln!("linestr[{i}] {}", vs.join(" -> "));
        }
        for (i, rec) in g.decoded_iglines.iter().enumerate() {
            if rec.index != 28 {
                continue;
            }
            eprintln!(
                "line[{i}] ({:.1},{:.1})->({:.1},{:.1})",
                rec.start_x * 1000.0,
                rec.start_y * 1000.0,
                rec.end_x * 1000.0,
                rec.end_y * 1000.0
            );
        }
    }
    Ok(())
}
