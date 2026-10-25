//! Resolve the line style of the L-shaped polyline at scene mm
//! (117-166, 651-712) of drawing 2 — the one the reference PDF draws
//! dashed and we draw solid. Also census: how many iglinestring records
//! resolve to a dashed style at all.

use pid_parse::style_link::line_styles_for_document;
use pid_parse::{ParseOptions, PidParser};
use std::path::Path;

fn main() {
    let path = Path::new(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid");
    let doc = PidParser::with_options(ParseOptions::geometry())
        .parse_file(path)
        .expect("parse");
    let styles = line_styles_for_document(&doc);
    let mut dashed = 0usize;
    let mut total = 0usize;
    let mut dash_pats = std::collections::BTreeMap::new();
    for ((stream, oid), style) in &styles {
        total += 1;
        if let Some(d) = &style.dash {
            dashed += 1;
            *dash_pats
                .entry(format!("{:?}", d.segments_mm()))
                .or_insert(0usize) += 1;
            if *stream == "/Sheet6" && dashed <= 25 {
                println!(
                    "dashed oid={oid} segs={:?} sym={:?}",
                    d.segments_mm(),
                    style.symbology
                );
            }
        }
    }
    println!("styles total={total} dashed={dashed}");
    for (p, n) in &dash_pats {
        println!("  pattern {p} x{n}");
    }
    // the L record: find in sheet decoded iglinestrings by geometry
    for sheet in &doc.sheet_streams {
        let Some(geometry) = &sheet.geometry else {
            continue;
        };
        for r in &geometry.decoded_iglinestrings {
            let vs: Vec<(f64, f64)> = r
                .vertex_xs
                .iter()
                .zip(r.vertex_ys.iter())
                .map(|(x, y)| (x * 1000.0, y * 1000.0))
                .collect();
            let xs: Vec<f64> = vs.iter().map(|v| v.0).collect();
            let ys: Vec<f64> = vs.iter().map(|v| v.1).collect();
            let (x0, x1) = (
                xs.iter().cloned().fold(f64::MAX, f64::min),
                xs.iter().cloned().fold(f64::MIN, f64::max),
            );
            let (y0, y1) = (
                ys.iter().cloned().fold(f64::MAX, f64::min),
                ys.iter().cloned().fold(f64::MIN, f64::max),
            );
            if x1 > 110.0 && x0 < 166.0 && y1 > 648.0 && y0 < 715.0 && (x1 - x0) + (y1 - y0) > 50.0
            {
                println!(
                    "L-ish record in {}: oid={} index={} layer_ref={} n={} verts {:?}",
                    sheet.path,
                    r.oid,
                    r.index,
                    r.sheet_layer_ref,
                    vs.len(),
                    vs
                );
                let resolved = styles.get(&(sheet.path.clone(), r.oid));
                println!("  resolved: {resolved:?}");
            }
        }
    }
}
