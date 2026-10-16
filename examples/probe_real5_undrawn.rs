//! Read-only census: what the five real drawings still leave undrawn after
//! the text-box / igArc2d decoder work. Per file prints the refused
//! (shape the family decoder walked over and refused), skipped (retired
//! 0x8000-bit records the native reader also skips), and the evidence
//! items that decode but land as inferred/probe-only rather than drawn.

use pid_parse::geometry::{build_normalized_geometry, PidGeometryConfidence};
use pid_parse::{ParseOptions, PidParser};
use std::collections::BTreeMap;
use std::path::Path;

fn main() {
    let dir = Path::new(r"D:\work\cad\PID自研软件验证");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("read dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "pid").unwrap_or(false))
        .collect();
    files.sort();
    for path in files {
        let doc = match PidParser::with_options(ParseOptions::geometry()).parse_file(&path) {
            Ok(doc) => doc,
            Err(e) => {
                println!("{}: parse failed: {e}", path.display());
                continue;
            }
        };
        let g = build_normalized_geometry(&doc);
        println!(
            "\n========== {}",
            path.file_name().unwrap().to_string_lossy()
        );
        for r in &g.refused_graphic_records {
            println!(
                "  REFUSED  0x{:04X} {:>3}  {}  ({})",
                r.type_code,
                r.count,
                r.rad_class_name.as_deref().unwrap_or("?"),
                r.stream_path
            );
        }
        for s in &g.skipped_graphic_records {
            println!(
                "  SKIPPED  0x{:04X} {:>3}  {}  ({})",
                s.type_code,
                s.count,
                s.rad_class_name.as_deref().unwrap_or("?"),
                s.stream_path
            );
        }
        let mut non_decoded: BTreeMap<(String, String), usize> = BTreeMap::new();
        let mut conf: BTreeMap<String, usize> = BTreeMap::new();
        for e in &g.entities {
            *conf.entry(format!("{:?}", e.confidence)).or_insert(0) += 1;
            if e.confidence != PidGeometryConfidence::Decoded {
                *non_decoded
                    .entry((
                        format!("{:?}", e.confidence),
                        format!("{:?}", e.source.record_kind),
                    ))
                    .or_insert(0) += 1;
            }
        }
        println!("  confidence: {conf:?}");
        for ((c, k), n) in non_decoded {
            println!("  undrawn-evidence  {c:>9}  {k:<28} x{n}");
        }
    }
}
