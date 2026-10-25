//! Dump the 0x0057 Top ViewFilterSet records of a .sym's PSMcluster0:
//! which sheet each governs, and the (name, number) -> displayed answers.
use std::path::Path;

use pid_parse::parsers::view_filter_sets::decode_view_filter_sets;
use pid_parse::PidParser;

fn main() {
    for arg in std::env::args().skip(1) {
        let package = PidParser::new()
            .parse_package(Path::new(&arg))
            .expect("compound file");
        println!(
            "== {}",
            Path::new(&arg).file_name().unwrap().to_string_lossy()
        );
        for (stream_path, raw) in &package.streams {
            if stream_path.rsplit('/').next() != Some("PSMcluster0") {
                continue;
            }
            for set in decode_view_filter_sets(&raw.data) {
                println!(
                    "  set oid={} sheet_ref={} active={}",
                    set.oid, set.sheet_ref, set.active_layer_number
                );
                for layer in &set.layers {
                    println!(
                        "    {:?} num={} shown={:?} locatable={:?}",
                        layer.name,
                        layer.layer_number,
                        set.is_displayed(layer.layer_number),
                        set.is_locatable(layer.layer_number)
                    );
                }
            }
        }
    }
}
