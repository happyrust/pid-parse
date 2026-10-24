//! What records produce the Christmas-tree donut circles?
//! scene mm (149.4, 659.3) -> source m (0.1494, 0.6593). Also: does the
//! fill index name anything at those records' style index?

use pid_parse::geometry::build_normalized_geometry;
use pid_parse::style_link::fill_styles_for_document;
use pid_parse::{ParseOptions, PidParser};
use std::path::Path;

fn main() {
    let path = Path::new(r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid");
    let doc = PidParser::with_options(ParseOptions::geometry())
        .parse_file(path)
        .expect("parse");
    let g = build_normalized_geometry(&doc);
    let fills = fill_styles_for_document(&doc);
    println!("fills: {}", fills.len());
    for e in &g.entities {
        if let pid_parse::geometry::PidGraphicKind::Circle { center, radius } = &e.kind {
            let (x, y) = (center.x * 1000.0, center.y * 1000.0);
            if (x - 149.4).abs() < 0.5 && (y - 659.3).abs() < 0.5 {
                println!(
                    "circle {} r={:.4}mm oid={:?} src={} layer={:?} conf={:?}",
                    e.id,
                    radius * 1000.0,
                    e.graphic_oid,
                    e.source.stream_path.as_deref().unwrap_or("?"),
                    e.source_layer
                        .as_ref()
                        .map(|l| (l.oid, l.name.clone(), l.displayed)),
                    e.confidence
                );
            }
        }
    }
    // raw decoded boundary records near the spot, with their style index
    for sheet in &doc.sheet_streams {
        let Some(geo) = &sheet.geometry else { continue };
        for r in &geo.decoded_igboundaries {
            // gather all numeric coords from segments, find any point near target
            let mut hit = false;
            for seg in &r.segments {
                for (x, y) in [(seg.start_x, seg.start_y), (seg.end_x, seg.end_y)] {
                    if (x * 1000.0 - 149.4).abs() < 8.0 && (y * 1000.0 - 659.3).abs() < 8.0 {
                        hit = true;
                    }
                }
            }
            if hit {
                println!(
                    "raw igboundary {} oid={} index={} layer_ref={} segs={} bytes {}..{}",
                    sheet.path,
                    r.oid,
                    r.index,
                    r.sheet_layer_ref,
                    r.segment_count,
                    r.byte_start,
                    r.byte_end
                );
                let key = (sheet.path.clone(), r.oid);
                println!("   fill={:?}", fills.get(&key));
            }
        }
    }
    println!("-- fills map --");
    for (k, v) in fills.iter().take(10) {
        println!("fill {k:?} {v:?}");
    }
}
