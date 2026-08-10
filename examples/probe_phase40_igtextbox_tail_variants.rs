//! Phase 40 S6 — why does `igTextBox` overhead come in three sizes?
//!
//! S3 split the 52 `inline-len-mismatch` refusals by taking `+30` at its word:
//! 24 of them imply an overhead of **exactly 100 or 108** and 20 of those read
//! as text at `+32`. The decoder assumes the overhead is a fixed 68 — 32 bytes
//! before the text and 36 after — so 100 and 108 are `68 + 32` and `68 + 40`.
//!
//! That is a testable shape rather than a mystery: if the head is unchanged
//! and the *tail* is longer, the 24 are ordinary text records with something
//! appended, and the fixed 68 is the same first-batch regularity S5 retired.
//! If instead the head differs, `+30` was never the length and the readable
//! text is a coincidence.
//!
//! So this dumps, for accepted and refused records side by side: the tail
//! length that `+30` implies, the text itself, and the tail bytes. An accepted
//! record's tail is 36 bytes by construction; the question is what the extra
//! 32 or 40 look like, and whether they repeat.
//!
//! ```powershell
//! cargo run --example probe_phase40_igtextbox_tail_variants
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_igtextbox_at, sheet_record_starts, IGTEXTBOX_PAYLOAD_OVERHEAD, PSM_ENVELOPE_LEN,
    PSM_TYPE_CODE_IGTEXTBOX,
};

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "test-file/D06.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

/// Bytes before the text in the shipped layout (`oid`, the `aux` pair,
/// `sub_type_word`, `index`, 12 undecoded bytes, then the `+30` length).
const HEAD_LEN: usize = 32;
/// Bytes after the text in the shipped layout: 3 doubles + 12 more.
const SHIPPED_TAIL_LEN: usize = IGTEXTBOX_PAYLOAD_OVERHEAD - HEAD_LEN;

fn read_u16(payload: &[u8], at: usize) -> Option<u16> {
    payload
        .get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The text `+30` claims, if it is inside the payload at all.
fn text_of(payload: &[u8], inline: u16) -> Option<String> {
    let mut units = Vec::with_capacity(inline as usize);
    for i in 0..inline as usize {
        units.push(read_u16(payload, HEAD_LEN + i * 2)?);
    }
    Some(String::from_utf16_lossy(&units))
}

fn sheet_streams(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        return Vec::new();
    };
    let names: Vec<PathBuf> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_path_buf())
        .filter(|p| p.to_string_lossy().contains("Sheet"))
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(&name) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.push((name.to_string_lossy().into_owned(), bytes));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Counts for the candidate rule, against what ships.
#[derive(Default)]
struct Candidate {
    ships: usize,
    room24: usize,
    room24_new: usize,
    room36: usize,
    room36_new: usize,
    room36_readable: usize,
}

/// Would the candidate rule take this record, given `slack` bytes required
/// after the text? Returns the text when it would.
fn candidate_takes(payload: &[u8], inline: u16, slack: usize) -> Option<String> {
    let text_end = HEAD_LEN.checked_add(2 * inline as usize)?;
    if text_end.checked_add(slack)? > payload.len() {
        return None;
    }
    for i in 0..3 {
        let at = text_end + i * 8;
        let chunk = payload.get(at..at + 8)?;
        let v = f64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
        if !v.is_finite() || v.abs() > 1e9 {
            return None;
        }
    }
    text_of(payload, inline)
}

fn main() {
    let mut candidate = Candidate::default();
    // tail length implied by `+30` -> (count, samples)
    let mut refused_tails: BTreeMap<usize, (usize, Vec<String>)> = BTreeMap::new();
    let mut accepted_tails: BTreeMap<usize, usize> = BTreeMap::new();
    // Do the two head layouts agree? Compare the 12 undecoded bytes at +18.
    let mut head_18: BTreeMap<(&'static str, String), usize> = BTreeMap::new();

    for fixture in FIXTURES {
        for (name, bytes) in sheet_streams(Path::new(fixture)) {
            for at in sheet_record_starts(&bytes) {
                let Some(raw) = bytes.get(at..at + 2) else {
                    continue;
                };
                if u16::from_le_bytes([raw[0], raw[1]]) & 0x3FFF != PSM_TYPE_CODE_IGTEXTBOX {
                    continue;
                }
                let Some(b) = bytes.get(at + 2..at + 6) else {
                    continue;
                };
                let btf = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
                let Some(payload) = bytes.get(at + PSM_ENVELOPE_LEN..at + PSM_ENVELOPE_LEN + btf)
                else {
                    continue;
                };
                let Some(inline) = read_u16(payload, 30) else {
                    continue;
                };

                let accepted = decode_igtextbox_at(&bytes, at).is_some();
                if accepted {
                    candidate.ships += 1;
                }
                if candidate_takes(payload, inline, 24).is_some() {
                    candidate.room24 += 1;
                    if !accepted {
                        candidate.room24_new += 1;
                    }
                }
                if let Some(text) = candidate_takes(payload, inline, 36) {
                    candidate.room36 += 1;
                    if !accepted {
                        candidate.room36_new += 1;
                        let readable = !text.is_empty()
                            && !text.contains('\u{FFFD}')
                            && text.chars().all(|c| !c.is_control());
                        if readable {
                            candidate.room36_readable += 1;
                        }
                    }
                }
                let text_bytes = 2 * inline as usize;
                let Some(tail_start) = HEAD_LEN.checked_add(text_bytes) else {
                    continue;
                };
                if tail_start > payload.len() {
                    continue;
                }
                let tail_len = payload.len() - tail_start;

                let key = if accepted { "accepted" } else { "refused " };
                *head_18
                    .entry((key, hex(payload.get(18..30).unwrap_or_default())))
                    .or_default() += 1;

                if accepted {
                    *accepted_tails.entry(tail_len).or_default() += 1;
                    continue;
                }
                // Only the sub-population `+30` can explain: a plausible text
                // length that leaves a tail at least as long as the shipped one.
                if tail_len < SHIPPED_TAIL_LEN {
                    continue;
                }
                let entry = refused_tails.entry(tail_len).or_insert((0, Vec::new()));
                entry.0 += 1;
                if entry.1.len() < 3 {
                    let text = text_of(payload, inline).unwrap_or_default();
                    entry.1.push(format!(
                        "{name} @ 0x{at:06X} btf={btf} len={inline} text={text:?}\n      \
                         shipped tail  : {}\n      extra bytes   : {}",
                        hex(&payload[tail_start..tail_start + SHIPPED_TAIL_LEN]),
                        hex(&payload[tail_start + SHIPPED_TAIL_LEN..])
                    ));
                }
            }
        }
    }

    println!("=== candidate rule: trust `+30`, keep the structural checks ===");
    println!(
        "Today: derive the length from `btf` assuming a fixed {IGTEXTBOX_PAYLOAD_OVERHEAD}-byte\n\
         overhead, then demand `+30` agree. Candidate: take `+30` as the length and\n\
         require the payload to have room for the text and the three doubles after it,\n\
         finite and in domain — the checks that are about the record's shape rather\n\
         than about a constant measured off the first batch.\n"
    );
    println!("{:>28} {:>10} {:>10}", "", "accepted", "of which new");
    println!("{:>28} {:>10} {:>10}", "ships today", candidate.ships, 0);
    println!(
        "{:>28} {:>10} {:>10}",
        "trust +30, room for 24B", candidate.room24, candidate.room24_new
    );
    println!(
        "{:>28} {:>10} {:>10}",
        "trust +30, room for 36B", candidate.room36, candidate.room36_new
    );
    println!(
        "{:>28} {:>10} {:>10}",
        "  (of those, text reads)", candidate.room36_readable, 0
    );

    println!("\n=== tail length after the text `+30` claims ===");
    println!("shipped layout expects {SHIPPED_TAIL_LEN} bytes of tail\n");
    println!("{:>6} {:>10} {:>10}", "tail", "accepted", "refused");
    let mut all: Vec<usize> = accepted_tails.keys().copied().collect();
    all.extend(refused_tails.keys().copied());
    all.sort_unstable();
    all.dedup();
    for tail in all {
        let a = accepted_tails.get(&tail).copied().unwrap_or(0);
        let r = refused_tails.get(&tail).map_or(0, |(count, _)| *count);
        println!("{tail:>6} {a:>10} {r:>10}");
    }

    println!("\n=== the 12 undecoded head bytes at +18, accepted vs refused ===");
    let mut per_key: BTreeMap<&str, usize> = BTreeMap::new();
    for ((key, _), count) in &head_18 {
        *per_key.entry(key).or_default() += count;
    }
    for (key, total) in &per_key {
        let distinct = head_18.keys().filter(|(k, _)| k == key).count();
        println!("{key}: {total:>4} record(s), {distinct:>3} distinct head pattern(s)");
    }

    println!("\n=== samples ===");
    for (tail_len, (count, samples)) in &refused_tails {
        println!("\n-- tail {tail_len} bytes ({count} record(s)) --");
        for sample in samples {
            println!("  {sample}");
        }
    }
}
