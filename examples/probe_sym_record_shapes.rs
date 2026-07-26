//! Phase 36-C: what geometry does each `.sym` PSM type code carry?
//!
//! The corpus scan (`probe_sym_corpus_scan.rs`) walked 1134 symbol sheet
//! streams and every record chain landed exactly on the last byte, so the
//! record boundaries are known. What the records *mean* is not: only
//! `0x0018` (igLine2d), `0x0059` (igCircle2d), `0x005E` (igPoint2d) and
//! `0x004D` (igTextBox) are named by the existing sheet families, and those
//! four are 68% of the corpus.
//!
//! This probe reads the rest positionally. For each type code it prints the
//! payload after the 18-byte positional header as f64s, plus a value-domain
//! summary across the whole corpus. Geometry gives itself away: coordinates
//! cluster in the symbol's own small extent, radii are positive and small,
//! and angles fall in [-2pi, 2pi].
//!
//! Read-only: no parser, schema, or model change.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::PidParser;

const CLUSTER_MAGIC: u32 = 0x6C90_F544;
const CHAIN_START: usize = 8;
const PSM_ENVELOPE_LEN: usize = 6;
const PAYLOAD_HEADER_LEN: usize = 18;
const SAMPLES_PER_TYPE: usize = 4;

fn symbols_root() -> PathBuf {
    let full = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test-file")
        .join("symbols-full");
    if full.is_dir() {
        return full;
    }
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

/// Per-slot value domain: what the f64 at a given offset looks like across
/// every record of one type code.
#[derive(Default)]
struct Slot {
    n: usize,
    min: f64,
    max: f64,
    /// Values that land in [-2pi, 2pi] -- an angle would be all of them.
    angle_like: usize,
    /// Strictly positive values -- a radius would be all of them.
    positive: usize,
}

impl Slot {
    fn add(&mut self, v: f64) {
        if self.n == 0 {
            self.min = v;
            self.max = v;
        }
        self.n += 1;
        self.min = self.min.min(v);
        self.max = self.max.max(v);
        if v.abs() <= std::f64::consts::TAU + 1e-9 {
            self.angle_like += 1;
        }
        if v > 0.0 {
            self.positive += 1;
        }
    }
}

#[derive(Default)]
struct TypeInfo {
    count: usize,
    /// Only fixed-size types get slot statistics; a variable payload has no
    /// stable offset to compare across records.
    fixed_len: Option<usize>,
    mixed_len: bool,
    slots: BTreeMap<usize, Slot>,
    samples: Vec<String>,
    header_words: BTreeMap<u32, usize>,
    sub_types: BTreeMap<u16, usize>,
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

    let mut types: BTreeMap<u16, TypeInfo> = BTreeMap::new();

    for sym in &syms {
        let Ok(pkg) = PidParser::new().parse_package(sym) else {
            continue;
        };
        let short = sym
            .strip_prefix(&root)
            .unwrap_or(sym.as_path())
            .display()
            .to_string();
        for (path, raw) in &pkg.streams {
            let is_sheet = path
                .rsplit('/')
                .next()
                .is_some_and(|n| n.starts_with("Sheet"));
            if !is_sheet || raw.data.len() <= CHAIN_START {
                continue;
            }
            if u32_le(&raw.data, 0) != Some(CLUSTER_MAGIC) {
                continue;
            }
            let bytes = &raw.data;
            let mut at = CHAIN_START;
            while at + PSM_ENVELOPE_LEN <= bytes.len() {
                let (Some(raw_type), Some(btf)) = (u16_le(bytes, at), u32_le(bytes, at + 2)) else {
                    break;
                };
                if raw_type == 0 {
                    break;
                }
                let body = at + PSM_ENVELOPE_LEN;
                let end = body.saturating_add(btf as usize);
                if end > bytes.len() {
                    break;
                }
                let payload = &bytes[body..end];
                let info = types.entry(raw_type & 0x0FFF).or_default();
                info.count += 1;
                match info.fixed_len {
                    None if !info.mixed_len => info.fixed_len = Some(payload.len()),
                    Some(l) if l != payload.len() => {
                        info.fixed_len = None;
                        info.mixed_len = true;
                    }
                    _ => {}
                }
                if payload.len() >= PAYLOAD_HEADER_LEN {
                    if let Some(w) = u32_le(payload, 8) {
                        *info.header_words.entry(w).or_default() += 1;
                    }
                    if let Some(w) = u16_le(payload, 12) {
                        *info.sub_types.entry(w).or_default() += 1;
                    }
                    let mut off = PAYLOAD_HEADER_LEN;
                    while off + 8 <= payload.len() {
                        if let Some(v) = f64_le(payload, off) {
                            if v.is_finite() {
                                info.slots.entry(off).or_default().add(v);
                            }
                        }
                        off += 8;
                    }
                }
                if info.samples.len() < SAMPLES_PER_TYPE {
                    let floats: Vec<String> = (PAYLOAD_HEADER_LEN..payload.len())
                        .step_by(8)
                        .filter(|o| o + 8 <= payload.len())
                        .filter_map(|o| f64_le(payload, o))
                        .map(|v| format!("{v:.6}"))
                        .collect();
                    let tail = payload.len() - PAYLOAD_HEADER_LEN;
                    info.samples.push(format!(
                        "{short} btf={} f64s={} rem={} [{}]",
                        payload.len(),
                        tail / 8,
                        tail % 8,
                        floats.join(", ")
                    ));
                }
                at = end;
            }
        }
    }

    let mut rows: Vec<(&u16, &TypeInfo)> = types.iter().collect();
    rows.sort_by_key(|(_, info)| Reverse(info.count));

    for (code, info) in rows {
        let named = match code {
            0x0018 => " = igLine2d",
            0x0059 => " = igCircle2d",
            0x005E => " = igPoint2d",
            0x004D => " = igTextBox",
            0x0084 => " = igLineString2d",
            0x0013 => " = igBoundary2d",
            0x00CE => " = igSymbol2d",
            _ => "",
        };
        let shape = match info.fixed_len {
            Some(l) => format!("fixed payload {l}"),
            None => "variable payload".to_string(),
        };
        println!(
            "\n===== type {code:#06x}{named}  ({} records, {shape}) =====",
            info.count
        );

        let hw: Vec<String> = {
            let mut v: Vec<(&u32, &usize)> = info.header_words.iter().collect();
            v.sort_by_key(|(_, count)| Reverse(**count));
            v.iter().take(4).map(|(k, n)| format!("{k}x{n}")).collect()
        };
        let st: Vec<String> = {
            let mut v: Vec<(&u16, &usize)> = info.sub_types.iter().collect();
            v.sort_by_key(|(_, count)| Reverse(**count));
            v.iter()
                .take(4)
                .map(|(k, n)| format!("{:#06x}x{n}", k))
                .collect()
        };
        println!("  payload[8..12] : {}", hw.join(" "));
        println!("  sub_type[12..14]: {}", st.join(" "));

        if info.fixed_len.is_some() {
            println!("  f64 slots (offset from payload start):");
            for (off, slot) in &info.slots {
                let angle = 100.0 * slot.angle_like as f64 / slot.n.max(1) as f64;
                let pos = 100.0 * slot.positive as f64 / slot.n.max(1) as f64;
                println!(
                    "    +{off:<3} n={:<6} min={:>14.6} max={:>14.6}  |v|<=2pi:{angle:>5.1}%  v>0:{pos:>5.1}%",
                    slot.n, slot.min, slot.max
                );
            }
        }
        println!("  samples:");
        for s in &info.samples {
            println!("    {s}");
        }
    }

    Ok(())
}
