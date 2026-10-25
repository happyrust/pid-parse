//! Resolve the style ids a .sym's text records name, through the .sym's own
//! StyleCluster — what JStyleTextChar do they reach (height, font, colour)?
use pid_parse::style_link::DocumentStyleTable;

fn main() {
    for arg in std::env::args().skip(1) {
        println!("=== {arg}");
        let Ok(mut cfb) = cfb::open(&arg) else {
            println!("not cfb");
            continue;
        };
        let streams: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|e| e.path().to_string_lossy().into_owned())
            .filter(|n| n.contains("StyleCluster"))
            .collect();
        for sp in streams {
            let mut bytes = Vec::new();
            use std::io::Read;
            let Ok(mut s) = cfb.open_stream(&sp) else {
                continue;
            };
            if s.read_to_end(&mut bytes).is_err() {
                continue;
            }
            let table = DocumentStyleTable::from_stylecluster_bytes(&bytes);
            println!("  {sp}: {} records", table.len());
            for id in 0..=4096u32 {
                if let Some(r) = table.get(id) {
                    if r.type_code == 0x002C || r.type_code == 0x002D {
                        println!("    id {id} type 0x{:04X}", r.type_code);
                        if let Some(res) = table.resolve_text_height(id) {
                            println!("        resolve_text_height -> height_m={} font={:?} align={:?} rgb={:?} spacing={:?}",
                                res.height_m, res.font_name, res.alignment,
                                res.rgb(), res.line_spacing);
                        }
                    }
                }
            }
        }
    }
}
