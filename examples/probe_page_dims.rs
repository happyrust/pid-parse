//! Print page_dimensions_mm + the Template meta tag for each file arg.
use pid_parse::{build_normalized_geometry, ParseOptions, PidParser};

fn main() {
    for arg in std::env::args().skip(1) {
        let doc = match PidParser::with_options(ParseOptions::geometry()).parse_file(&arg) {
            Ok(doc) => doc,
            Err(e) => {
                println!("{arg}: parse failed: {e}");
                continue;
            }
        };
        let geo = build_normalized_geometry(&doc);
        let tag = doc
            .drawing_meta
            .as_ref()
            .and_then(|m| m.tags.get("Template").cloned());
        println!(
            "{arg}\n  page_dims={:?} template_tag={:?} smartframes={}",
            geo.page_dimensions_mm,
            tag,
            doc.sheet_streams
                .iter()
                .filter_map(|s| s.geometry.as_ref())
                .map(|g| g.decoded_igsmartframes.len())
                .sum::<usize>()
        );
        for stream in &doc.sheet_streams {
            let Some(geometry) = stream.geometry.as_ref() else {
                continue;
            };
            for frame in &geometry.decoded_igsmartframes {
                println!(
                    "    frame state={:?} extent={:.2}x{:.2} mm oid={}",
                    frame.state,
                    frame.extent_width_m * 1000.0,
                    frame.extent_height_m * 1000.0,
                    frame.oid
                );
            }
        }
    }
}
