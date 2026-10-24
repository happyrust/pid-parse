//! What a title-block field's `igTextBox` record says about where it letters.
//!
//! The boxed records whose text starts with `DWG-` all carry the same
//! 45.307 × 3.689 mm box, and the reference plots start most of them at the
//! box's left edge. Three of them start at the box's right edge instead
//! (`box_right − string_width`), and the record's own fields are the only
//! place a difference could hide. This probe prints one row per matching
//! record so the exceptions can be diffed against the followers: the anchor
//! code, the box, the insertion the box is measured from, the paragraph
//! style the record names, where the record sits — and, with `--tail`, the
//! dwords of the kind-2 placement tail that no reader has named yet.
//!
//! ```text
//! cargo run -p pid-parse --example probe_title_block_letters -- \
//!     "<drawing.pid>" ["DWG-"] [--tail]
//! ```
//!
//! Read-only: no parser, schema or model change.

use std::collections::HashMap;
use std::env;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use pid_parse::{build_normalized_geometry, PidGraphicKind, PidParser};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(pid) = args.next() else {
        eprintln!("usage: probe_title_block_letters <drawing.pid> [prefix] [--tail]");
        return ExitCode::from(2);
    };
    let mut prefix = "DWG-".to_string();
    let mut tail = false;
    for argument in args {
        if argument == "--tail" {
            tail = true;
        } else {
            prefix = argument;
        }
    }
    let doc = match PidParser::new().parse_file(PathBuf::from(&pid).as_path()) {
        Ok(doc) => doc,
        Err(error) => {
            eprintln!("{pid}: {error}");
            return ExitCode::from(1);
        }
    };
    let geometry = build_normalized_geometry(&doc);
    let mut streams: HashMap<String, Vec<u8>> = HashMap::new();
    if tail {
        let Ok(file) = std::fs::File::open(&pid) else {
            eprintln!("{pid}: cannot open for the tail read");
            return ExitCode::from(1);
        };
        let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
            eprintln!("{pid}: not a compound file");
            return ExitCode::from(1);
        };
        let names: Vec<PathBuf> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for name in names {
            let Ok(mut stream) = cfb.open_stream(&name) else {
                continue;
            };
            let mut bytes = Vec::new();
            if stream.read_to_end(&mut bytes).is_ok() {
                streams.insert(name.to_string_lossy().to_string(), bytes);
            }
        }
    }
    println!(
        "text\tinsertion_mm\tanchor_code\tbox_mm\theight_m\trotation_deg\tlayer\toid\tstream\tbytes"
    );
    for entity in &geometry.entities {
        let PidGraphicKind::Text {
            insertion,
            value,
            height,
            rotation,
            anchor_code,
            text_box_width,
        } = &entity.kind
        else {
            continue;
        };
        if !value.trim().starts_with(&prefix) {
            continue;
        }
        println!(
            "{value:?}\t[{:.2},{:.2}]\t0x{anchor_code:02X}\t{}\t{height:.4}\t{:.1}\t{}\t{}\t{}\t{}",
            insertion.x * 1000.0,
            insertion.y * 1000.0,
            text_box_width.map_or("none".to_string(), |width| format!("{:.3}", width * 1000.0)),
            rotation.to_degrees(),
            entity
                .source_layer
                .as_ref()
                .map_or("none".to_string(), |layer| format!("{layer:?}")),
            entity
                .graphic_oid
                .map_or("none".to_string(), |oid| oid.to_string()),
            entity.source.stream_path.as_deref().unwrap_or("none"),
            entity
                .source
                .byte_range
                .as_ref()
                .map_or("none".to_string(), |range| {
                    format!("{}..{}", range.start, range.end)
                }),
        );
        if !tail {
            continue;
        }
        let (Some(path), Some(range)) =
            (entity.source.stream_path.clone(), entity.source.byte_range)
        else {
            continue;
        };
        let Some(bytes) = streams.get(&path) else {
            println!("    tail: stream {path} not readable");
            continue;
        };
        let Some(record) = bytes.get(range.start..range.end) else {
            println!("    tail: range {range:?} outside {path}");
            continue;
        };
        // 6-byte PSM envelope, then the payload the parser decodes.
        let Some(payload) = record.get(6..) else {
            continue;
        };
        let Some(tail_bytes) = payload
            .len()
            .checked_sub(68)
            .and_then(|at| payload.get(at..))
        else {
            println!("    tail: payload {} shorter than 68", payload.len());
            continue;
        };
        let dword = |at: usize| {
            tail_bytes
                .get(at..at + 4)
                .map(|slice| u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
                .map_or("----".to_string(), |word| format!("{word:#010x}"))
        };
        let hex: Vec<String> = tail_bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        println!(
            "    payload {}  +32 {}  +36 {}  +56 {}  +60 {}  +64 {}",
            payload.len(),
            dword(32),
            dword(36),
            dword(56),
            dword(60),
            dword(64)
        );
        println!("    tail {}", hex.join(""));
    }
    ExitCode::SUCCESS
}
