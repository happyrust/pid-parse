//! Inspect CPE-SQ-A0.igr: boundary records, fill resolution, entity census at
//! the logo spot (right-bottom of the title block).

use pid_parse::{ParseOptions, PidGraphicKind, PidParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A0.igr";
    let parsed = PidParser::with_options(ParseOptions::geometry()).parse_file(path)?;
    let geometry = pid_parse::build_normalized_geometry(&parsed);
    eprintln!(
        "entities={} page={:?}",
        geometry.entities.len(),
        geometry.page_dimensions_mm
    );

    // fill index: which (stream,oid) resolve?
    let fills = pid_parse::style_link::fill_styles_for_document(&parsed);
    eprintln!("fills resolved: {}", fills.len());
    for (k, f) in fills.iter().take(20) {
        eprintln!("  fill {k:?} rgb={:?}", f.rgb());
    }

    // closed plines (boundary rings) near logo region — sheet metres:
    // logo at page mm x~1043-1057 y~73-80 -> guess normalized or metres; print ALL
    // small closed plines w<0.02 with bbox
    let mut nring = 0;
    for e in &geometry.entities {
        let PidGraphicKind::Polyline {
            points,
            closed: true,
        } = &e.kind
        else {
            continue;
        };
        let xs: Vec<f64> = points.iter().map(|p| p.x).collect();
        let ys: Vec<f64> = points.iter().map(|p| p.y).collect();
        let (x0, x1) = (
            xs.iter().cloned().fold(f64::MAX, f64::min),
            xs.iter().cloned().fold(f64::MIN, f64::max),
        );
        let (y0, y1) = (
            ys.iter().cloned().fold(f64::MAX, f64::min),
            ys.iter().cloned().fold(f64::MIN, f64::max),
        );
        let w = x1 - x0;
        let h = y1 - y0;
        nring += 1;
        // logo-sized rings: print ones with w,h in plausible letterform span
        if w < 0.02 && h < 0.02 && points.len() >= 6 {
            println!(
                "ring oid={:?} n={} bbox=({x0:.5},{y0:.5})-({x1:.5},{y1:.5}) fill_join={}",
                e.graphic_oid,
                points.len(),
                fills.contains_key(&(
                    e.source.stream_path.clone().unwrap_or_default(),
                    e.graphic_oid.unwrap_or(0)
                ))
            );
        }
    }
    eprintln!("closed rings total={nring}");
    // dropped/refused census
    for d in &geometry.dropped_graphic_records {
        eprintln!("dropped {:?} {}", d.rad_class_name, d.count);
    }
    for r in &geometry.refused_graphic_records {
        eprintln!("refused {:?} {}", r.rad_class_name, r.count);
    }
    Ok(())
}
