//! Phase 36-A: is a `.sym` symbol library file the same container as a `.pid`?
//!
//! `pid-parse` resolves every `igSymbol2d` placement to a UNC path into a
//! SmartPlant reference share (`...\Piping\Valves\Angle\2-Way Angle Globe
//! Valve.sym`). Two of those `.sym` files are checked in under
//! `test-file/symbols/`, and both open with the CFB signature, so the
//! question is whether the *inside* is also shared: same cluster magic, same
//! 16-byte cluster header, same 6-byte PSM record envelope.
//!
//! If it is, the symbol bodies are already reachable with the decoders the
//! sheet side has been using since Phase 35-A, and drawing a real symbol is a
//! matter of wiring, not of a second reverse-engineering campaign.
//!
//! This probe walks every stream, and for the `Sheet*` streams it dumps the
//! header, walks the record chain, and hex-dumps the bytes so the layout can
//! be read directly. `Circle.sym` is the control: one circle, so whatever
//! `igCircle2d` looks like has to be visible in 57 bytes.
//!
//! Read-only: no parser, schema, or model change.

use std::path::{Path, PathBuf};

use pid_parse::PidParser;

const CLUSTER_MAGIC: u32 = 0x6C90_F544;
const CLUSTER_HEADER_LEN: usize = 16;
const PSM_ENVELOPE_LEN: usize = 6;
const MAX_DUMP: usize = 1024;

fn symbols_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test-file")
        .join("symbols")
}

fn collect_syms(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_syms(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("sym"))
        {
            out.push(path);
        }
    }
}

fn u16_le(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2)
        .and_then(|s| <[u8; 2]>::try_from(s).ok())
        .map(u16::from_le_bytes)
}

fn u32_le(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map(u32::from_le_bytes)
}

fn f64_le(b: &[u8], at: usize) -> Option<f64> {
    b.get(at..at + 8)
        .and_then(|s| <[u8; 8]>::try_from(s).ok())
        .map(f64::from_le_bytes)
}

fn hex_dump(bytes: &[u8], limit: usize) {
    let shown = bytes.len().min(limit);
    for (row, chunk) in bytes[..shown].chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("      {:04x}  {:<47}  |{}|", row * 16, hex.join(" "), ascii);
    }
    if bytes.len() > shown {
        println!("      ... {} more bytes", bytes.len() - shown);
    }
}

/// Walk the 6-byte PSM envelope chain from `start`, reporting how far it gets.
fn walk_records(bytes: &[u8], start: usize) {
    let mut at = start;
    let mut n = 0;
    while at + PSM_ENVELOPE_LEN <= bytes.len() {
        let Some(raw_type) = u16_le(bytes, at) else {
            break;
        };
        let Some(btf) = u32_le(bytes, at + 2) else {
            break;
        };
        if raw_type == 0 {
            println!("      [{n}] @{at:#06x} type=0x0000 -- stop (zero type)");
            break;
        }
        let body = at + PSM_ENVELOPE_LEN;
        let end = body.saturating_add(btf as usize);
        if end > bytes.len() {
            println!(
                "      [{n}] @{at:#06x} type={:#06x} btf={btf} -- OVERRUN (needs {end}, have {})",
                raw_type & 0x0FFF,
                bytes.len()
            );
            break;
        }
        let payload = &bytes[body..end];
        // f64s in a symbol body are local design units, so a symbol drawn
        // around its own origin lands in a small band -- print them all and
        // let the values say whether the walk is real.
        let floats: Vec<String> = (0..payload.len().saturating_sub(7))
            .step_by(8)
            .filter_map(|o| f64_le(payload, o))
            .filter(|v| v.is_finite() && (*v == 0.0 || v.abs() > 1e-9))
            .map(|v| format!("{v:.6}"))
            .collect();
        println!(
            "      [{n}] @{at:#06x} type={:#06x} flags={:#x} btf={btf} payload={} f64s=[{}]",
            raw_type & 0x0FFF,
            raw_type >> 12,
            payload.len(),
            floats.join(", ")
        );
        at = end;
        n += 1;
    }
    if at == bytes.len() {
        println!("      chain ends exactly at stream end ({n} records) -- CLEAN");
    } else {
        println!(
            "      chain stopped at {at:#06x} of {:#06x} ({} bytes left, {n} records)",
            bytes.len(),
            bytes.len() - at
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = symbols_root();
    let mut syms = Vec::new();
    collect_syms(&root, &mut syms);
    syms.sort();
    if syms.is_empty() {
        println!("no .sym fixtures under {}; skipping", root.display());
        return Ok(());
    }

    for sym in &syms {
        let name = sym
            .strip_prefix(&root)
            .unwrap_or(sym.as_path())
            .display()
            .to_string();
        println!("\n================ {name} ================");
        let pkg = match PidParser::new().parse_package(sym) {
            Ok(p) => p,
            Err(e) => {
                println!("  parse FAILED: {e}");
                continue;
            }
        };
        println!("  parsed OK: {} streams", pkg.streams.len());

        println!("  -- streams --");
        for (path, raw) in &pkg.streams {
            let magic = u32_le(&raw.data, 0);
            let tag = match magic {
                Some(m) if m == CLUSTER_MAGIC => " <cluster-magic>",
                _ => "",
            };
            println!("    {:>7}  {path}{tag}", raw.data.len());
        }

        for (path, raw) in &pkg.streams {
            let is_sheet = path
                .rsplit('/')
                .next()
                .is_some_and(|n| n.starts_with("Sheet"));
            if !is_sheet || raw.data.is_empty() {
                continue;
            }
            println!("\n  -- {path} ({} bytes) --", raw.data.len());
            let data = &raw.data;
            match (
                u32_le(data, 0),
                u32_le(data, 4),
                u16_le(data, 8),
                u32_le(data, 10),
                u16_le(data, 14),
            ) {
                (Some(magic), Some(count), Some(stype), Some(body_len), Some(flags))
                    if magic == CLUSTER_MAGIC =>
                {
                    println!(
                        "      header: records={count} stream_type={stype:#06x} body_len={body_len} flags={flags:#06x}"
                    );
                    println!(
                        "      arithmetic: len={} header={CLUSTER_HEADER_LEN} rest={}",
                        data.len(),
                        data.len() - CLUSTER_HEADER_LEN
                    );
                    walk_records(data, CLUSTER_HEADER_LEN);
                }
                _ => println!("      no cluster header (magic mismatch or too short)"),
            }
            hex_dump(data, MAX_DUMP);
        }
    }

    Ok(())
}
