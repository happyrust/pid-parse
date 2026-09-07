//! Evidence probe for a closure flag on `.sym` `igLineString2d` records.
//!
//! A closed vertex run (first vertex repeated as the last) is observable. This
//! probe groups that observation by the two bytes immediately after the vertex
//! count (`form`, `scope`) across the complete local symbol corpus. A decoder
//! field is justified only if one value separates closed from open runs.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

const TYPE_POLYLINE: u16 = 0x0084;

fn roots() -> Vec<PathBuf> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if !args.is_empty() {
        return args;
    }
    if let Some(value) = std::env::var_os("PID_SYMBOL_LIBRARY") {
        return std::env::split_paths(&value).collect();
    }
    let full = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file/symbols-full");
    vec![full]
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        eprintln!("warning: symbol-library root not found: {}", dir.display());
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("sym"))
        {
            out.push(path);
        }
    }
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn f64_at(bytes: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn main() {
    let roots = roots();
    let mut syms = Vec::new();
    for root in &roots {
        collect(root, &mut syms);
    }

    let mut files = 0usize;
    let mut records = 0usize;
    let mut closed = 0usize;
    let mut by_form_scope: BTreeMap<(u8, u8, bool), usize> = BTreeMap::new();
    let mut closed_samples = Vec::new();
    for path in &syms {
        let Ok(file) = std::fs::File::open(path) else {
            continue;
        };
        let Ok(mut compound) = cfb::CompoundFile::open(file) else {
            continue;
        };
        files += 1;
        let streams: Vec<_> = compound
            .walk()
            .filter(cfb::Entry::is_stream)
            .filter(|entry| entry.name().starts_with("Sheet"))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for stream_path in streams {
            let Ok(mut stream) = compound.open_stream(&stream_path) else {
                continue;
            };
            let mut bytes = Vec::new();
            if stream.read_to_end(&mut bytes).is_err() {
                continue;
            }
            for at in pid_parse::parsers::sheet_records::sheet_record_starts(&bytes) {
                let Some(type_word) = bytes.get(at..at + 2) else {
                    continue;
                };
                let type_code = u16::from_le_bytes(type_word.try_into().unwrap()) & 0x3fff;
                if type_code != TYPE_POLYLINE {
                    continue;
                }
                let Some(payload_len) = u32_at(&bytes, at + 2).map(|n| n as usize) else {
                    continue;
                };
                let Some(payload) = bytes.get(at + 6..at + 6 + payload_len) else {
                    continue;
                };
                let Some(count) = u32_at(payload, 18).map(|n| n as usize) else {
                    continue;
                };
                if count < 2 || payload.len() != 24 + count * 16 {
                    continue;
                }
                let Some(first) = f64_at(payload, 24).zip(f64_at(payload, 32)) else {
                    continue;
                };
                let last_at = 24 + (count - 1) * 16;
                let Some(last) = f64_at(payload, last_at).zip(f64_at(payload, last_at + 8)) else {
                    continue;
                };
                let is_closed =
                    (first.0 - last.0).abs() <= 1e-12 && (first.1 - last.1).abs() <= 1e-12;
                records += 1;
                closed += usize::from(is_closed);
                if is_closed && closed_samples.len() < 4 {
                    closed_samples.push(path.display().to_string());
                }
                *by_form_scope
                    .entry((payload[22], payload[23], is_closed))
                    .or_default() += 1;
            }
        }
    }

    println!("symbol files: {files}");
    println!("polylines: {records}; geometrically closed: {closed}");
    let form_is_conclusive = records > 0
        && closed > 0
        && by_form_scope.iter().all(|((form, _, is_closed), _)| {
            (*form == 2 && *is_closed) || (*form == 1 && !*is_closed)
        });
    for ((form, scope, is_closed), count) in by_form_scope {
        println!("form={form} scope={scope} closed={is_closed}: {count}");
    }
    for sample in closed_samples {
        println!("closed sample: {sample}");
    }
    if form_is_conclusive {
        println!("verdict: form=2 is_closed, form=1 is open on the complete local corpus");
    } else if closed == 0 {
        println!("verdict: corpus contains no closed symbol polyline; no closure field is emitted");
    } else {
        println!("verdict: closed runs exist; correlate form/scope before adding a decoder field");
    }
}
