//! For JSite37311 sheet 1015 (Christmas Tree 02 cached body): list the
//! igDimension records, resolve each `measured_oid` to its igLine2d, and
//! print dim_value / stored_length — the parametric scale candidate.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let doc = pid_parse::PidParser::new().parse_file(path)?;
    for site in &doc.jsites {
        if site.name != "JSite37311" {
            continue;
        }
        let Some(n) = &site.nested_geometry else {
            continue;
        };
        println!("lines in site: {}", n.lines.len());
        for d in &n.dimensions {
            let measured = n.lines.iter().find(|l| l.oid == d.measured_oid);
            let len = measured.map(|l| {
                let dx = l.end_x - l.start_x;
                let dy = l.end_y - l.start_y;
                (dx * dx + dy * dy).sqrt()
            });
            println!(
                "dim oid={} parent={} kind={} value_m={:.6} measured_oid={} marker={:#06x} line={:?} len={:?} ratio={:?}",
                d.oid,
                d.parent_ref,
                d.kind,
                d.value_m,
                d.measured_oid,
                d.measured_marker,
                measured.map(|l| ((l.start_x, l.start_y), (l.end_x, l.end_y))),
                len,
                len.map(|l| d.value_m / l)
            );
        }
    }
    Ok(())
}
