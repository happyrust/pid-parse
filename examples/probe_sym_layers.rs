//! Decode the JSheetLayer records inside a .sym's own PSMcluster0 and
//! correlate them with the layer refs each sheet's primitives carry.
use pid_parse::parsers::sheet_layers::decode_sheet_layers;
use pid_parse::PidParser;

fn main() {
    for path in std::env::args().skip(1) {
        let package = PidParser::new()
            .parse_package(std::path::Path::new(&path))
            .expect("package");
        for (stream_path, raw) in &package.streams {
            if stream_path.rsplit('/').next() != Some("PSMcluster0") {
                continue;
            }
            let layers = decode_sheet_layers(&raw.data);
            println!("{path} :: {stream_path} ({} layers)", layers.len());
            for l in &layers {
                println!(
                    "    oid={} num={} objs={} name={:?} trailing={:#x}",
                    l.oid, l.layer_number, l.object_count, l.name, l.trailing_word
                );
            }
        }
    }
}
