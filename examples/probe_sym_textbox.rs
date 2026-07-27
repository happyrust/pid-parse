//! Gap #2: does the sheet `igTextBox` layout hold inside a `.sym` body?
//!
//! Phase 36 decoded four geometry families out of the symbol library and
//! stepped over everything else, which leaves the `0x004D` records — the
//! largest skipped family — unread. A symbol that carries its own label
//! (`FC`, `LT`, a nozzle tag) therefore draws as strokes with the lettering
//! missing, and the 40 library files with no drawable geometry at all are
//! suspected to be pure text symbols.
//!
//! The drawing-side decoder (`parsers::sheet_records`, PSM `0x004D`) reads
//! `text_length` at payload `+30` and UTF-16LE text at `+32`, with a
//! constant 68-byte overhead so that `payload_len == 68 + 2 * text_length`.
//! The two dialects have diverged before — `igLine2d`'s `payload[8..12]` is
//! 12 in a drawing and 8 in a symbol — so this checks the rule rather than
//! assuming it: every `0x004D` record in the corpus is measured against it,
//! and the decoded text is printed for eyeballing.
//!
//! Read-only: no parser, decoder or model change.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::PidParser;

const CLUSTER_MAGIC: u32 = 0x6C90_F544;
const CHAIN_START: usize = 8;
const PSM_ENVELOPE_LEN: usize = 6;
const TYPE_TEXTBOX: u16 = 0x004D;

/// Sheet dialect: 32-byte sub-header plus 36 bytes of trailing geometry.
const TEXTBOX_OVERHEAD: usize = 68;
/// Sheet dialect: `u16` character count at this payload offset.
const TEXT_LEN_AT: usize = 30;
/// Sheet dialect: UTF-16LE characters start here.
const TEXT_AT: usize = 32;
/// Sheet dialect: bytes of geometry and style after the text.
const TRAILING_LEN: usize = TEXTBOX_OVERHEAD - TEXT_AT;

const SAMPLES: usize = 25;

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

/// A candidate layout: `u16` count at `len_at`, characters right after it,
/// and `payload_len - 2 * count` bytes of fixed overhead.
#[derive(Default)]
struct Candidate {
    hits: usize,
    clean: usize,
}

#[derive(Default)]
struct Tally {
    records: usize,
    /// `payload_len == 68 + 2 * text_length` — the sheet dialect's own rule.
    consistent: usize,
    /// Length field disagrees with the payload size.
    inconsistent: usize,
    /// Payload shorter than the constant overhead.
    too_short: usize,
    /// Consistent records whose text decodes without a replacement char.
    clean_text: usize,
    /// Consistent records whose text is empty.
    empty_text: usize,
    sub_types: BTreeMap<u16, usize>,
    header_words: BTreeMap<u32, usize>,
    lengths: BTreeMap<u16, usize>,
    samples: Vec<String>,
    /// Every distinct string, with how many records carry it.
    texts: BTreeMap<String, usize>,
    /// Consistency split by `sub_type`, to see whether it discriminates.
    by_sub_type: BTreeMap<u16, (usize, usize)>,
    /// For records the sheet rule rejects: which `(len_at, overhead)` pair
    /// would explain them, and how often the text then decodes cleanly.
    candidates: BTreeMap<(usize, usize), Candidate>,
    /// Hex of the first few rejected payloads, for eyeballing.
    dumps: Vec<String>,
    /// Rejected records that exactly one `len_at` explains, keyed by offset.
    fit_offsets: BTreeMap<usize, usize>,
    single_fit: usize,
    ambiguous: usize,
    unexplained: usize,
    /// Strings recovered by the second layout, to eyeball before trusting it.
    fit_texts: BTreeMap<String, usize>,
    /// Payload sizes no offset explains, to see whether they cluster.
    unfit_lengths: Vec<usize>,
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
    println!("library: {} symbols under {}", syms.len(), root.display());

    let mut tally = Tally::default();
    // Symbols whose only content is text: no geometry family present, but at
    // least one 0x004D record. Phase 36 counted 40 bodies as empty.
    let mut text_only: Vec<(String, Vec<String>)> = Vec::new();
    let mut files_with_text = 0usize;

    for sym in &syms {
        let Ok(pkg) = PidParser::new().parse_package(sym) else {
            continue;
        };
        let short = sym
            .strip_prefix(&root)
            .unwrap_or(sym.as_path())
            .display()
            .to_string();
        let mut geometry_records = 0usize;
        let mut file_texts: Vec<String> = Vec::new();

        for (path, raw) in &pkg.streams {
            let is_sheet = path
                .rsplit('/')
                .next()
                .is_some_and(|n| n.starts_with("Sheet"));
            if !is_sheet || raw.data.len() <= CHAIN_START {
                continue;
            }
            let bytes = &raw.data;
            if u32_le(bytes, 0) != Some(CLUSTER_MAGIC) {
                continue;
            }
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
                let type_code = raw_type & 0x0FFF;
                if matches!(type_code, 0x0018 | 0x0059 | 0x0061 | 0x0084) {
                    geometry_records += 1;
                }
                if type_code == TYPE_TEXTBOX {
                    tally.records += 1;
                    let payload = &bytes[body..end];
                    record_textbox(&mut tally, &short, payload, &mut file_texts);
                }
                at = end;
            }
        }

        if !file_texts.is_empty() {
            files_with_text += 1;
            if geometry_records == 0 {
                text_only.push((short, file_texts));
            }
        }
    }

    report(&tally, files_with_text, &text_only);
    Ok(())
}

fn record_textbox(tally: &mut Tally, short: &str, payload: &[u8], file_texts: &mut Vec<String>) {
    if let Some(w) = u32_le(payload, 8) {
        *tally.header_words.entry(w).or_default() += 1;
    }
    let sub_type = u16_le(payload, 12).unwrap_or(u16::MAX);
    *tally.sub_types.entry(sub_type).or_default() += 1;
    if payload.len() < TEXTBOX_OVERHEAD {
        tally.too_short += 1;
        tally.by_sub_type.entry(sub_type).or_default().1 += 1;
        sweep_candidates(tally, payload);
        return;
    }
    let derived = (payload.len() - TEXTBOX_OVERHEAD) / 2;
    let Some(declared) = u16_le(payload, TEXT_LEN_AT) else {
        tally.too_short += 1;
        return;
    };
    *tally.lengths.entry(declared).or_default() += 1;
    if !(payload.len() - TEXTBOX_OVERHEAD).is_multiple_of(2) || declared as usize != derived {
        tally.inconsistent += 1;
        tally.by_sub_type.entry(sub_type).or_default().1 += 1;
        sweep_candidates(tally, payload);
        if tally.dumps.len() < 6 {
            tally.dumps.push(format!(
                "{short} payload={} declared@30={declared} derived={derived}\n      {}",
                payload.len(),
                hex(&payload[..payload.len().min(80)])
            ));
        }
        return;
    }
    tally.consistent += 1;
    tally.by_sub_type.entry(sub_type).or_default().0 += 1;

    let mut chars = Vec::with_capacity(declared as usize);
    for i in 0..declared as usize {
        let Some(c) = u16_le(payload, TEXT_AT + i * 2) else {
            break;
        };
        chars.push(c);
    }
    let text = String::from_utf16_lossy(&chars);
    if text.is_empty() {
        tally.empty_text += 1;
    } else if !text.contains('\u{fffd}') {
        tally.clean_text += 1;
    }
    *tally.texts.entry(text.clone()).or_default() += 1;
    if !text.is_empty() {
        file_texts.push(text.clone());
    }

    if tally.samples.len() < SAMPLES {
        let after = TEXT_AT + declared as usize * 2;
        let tail: Vec<String> = (0..3)
            .filter_map(|i| f64_le(payload, after + i * 8))
            .map(|v| format!("{v:.5}"))
            .collect();
        tally.samples.push(format!(
            "{short} len={declared:<3} text={text:?} tail=[{}]",
            tail.join(", ")
        ));
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

/// Text that reads as a label rather than as reinterpreted binary.
fn plausible(text: &str) -> bool {
    !text.is_empty()
        && !text.contains('\u{fffd}')
        && text
            .chars()
            .all(|c| !c.is_control() && c != '\u{0}' && (c.is_ascii() || c as u32 > 0x2000))
}

/// Read the `u16` at `len_at` as a character count and return the text it
/// would introduce, if the payload has room for it.
fn text_at(payload: &[u8], len_at: usize) -> Option<(usize, String)> {
    let count = u16_le(payload, len_at)? as usize;
    let start = len_at + 2;
    let end = start.checked_add(count * 2)?;
    if count == 0 || end > payload.len() {
        return None;
    }
    let mut chars = Vec::with_capacity(count);
    for i in 0..count {
        chars.push(u16_le(payload, start + i * 2)?);
    }
    Some((end, String::from_utf16_lossy(&chars)))
}

/// For a record the sheet rule rejects: which `u16` could be the character
/// count?
///
/// The sheet dialect's own invariant is that the text is followed by exactly
/// [`TRAILING_LEN`] bytes of geometry and style, so `payload_len - text_end`
/// is the discriminator: a `len_at` that leaves exactly that much behind is
/// the count, and any other reading is a coincidence. Both the trailing size
/// and the offset are tallied so the assumption can be checked rather than
/// assumed.
fn sweep_candidates(tally: &mut Tally, payload: &[u8]) {
    let mut exact: Vec<usize> = Vec::new();
    for len_at in (0..payload.len().min(72)).step_by(2) {
        let Some((end, text)) = text_at(payload, len_at) else {
            continue;
        };
        let trailing = payload.len() - end;
        let entry = tally.candidates.entry((len_at, trailing)).or_default();
        entry.hits += 1;
        if plausible(&text) {
            entry.clean += 1;
            if trailing == TRAILING_LEN {
                exact.push(len_at);
            }
        }
    }
    match exact.len() {
        0 => {
            tally.unexplained += 1;
            if tally.unfit_lengths.len() < 400 {
                tally.unfit_lengths.push(payload.len());
            }
        }
        1 => {
            tally.single_fit += 1;
            *tally.fit_offsets.entry(exact[0]).or_default() += 1;
            if let Some((_, text)) = text_at(payload, exact[0]) {
                *tally.fit_texts.entry(text).or_default() += 1;
            }
        }
        _ => tally.ambiguous += 1,
    }
}

fn report(tally: &Tally, files_with_text: usize, text_only: &[(String, Vec<String>)]) {
    println!("\n== 0x004D igTextBox records in the .sym corpus ==");
    println!("  total            {}", tally.records);
    println!(
        "  layout holds     {} ({:.1}%)",
        tally.consistent,
        100.0 * tally.consistent as f64 / tally.records.max(1) as f64
    );
    println!("  length mismatch  {}", tally.inconsistent);
    println!("  payload < 68 B   {}", tally.too_short);
    println!("  text decodes     {}", tally.clean_text);
    println!("  text empty       {}", tally.empty_text);
    println!("  files with text  {files_with_text}");

    let top = |m: &BTreeMap<u16, usize>, n: usize| -> String {
        let mut v: Vec<(&u16, &usize)> = m.iter().collect();
        v.sort_by_key(|(_, c)| Reverse(**c));
        v.iter()
            .take(n)
            .map(|(k, c)| format!("{k:#06x}x{c}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    println!("  sub_type[12..14] {}", top(&tally.sub_types, 6));
    let mut hw: Vec<(&u32, &usize)> = tally.header_words.iter().collect();
    hw.sort_by_key(|(_, c)| Reverse(**c));
    println!(
        "  payload[8..12]   {}",
        hw.iter()
            .take(6)
            .map(|(k, c)| format!("{k}x{c}"))
            .collect::<Vec<_>>()
            .join(" ")
    );

    println!("\n== consistency by sub_type ==");
    for (sub, (ok, bad)) in &tally.by_sub_type {
        println!("  {sub:#06x}  layout holds {ok:<5} rejected {bad}");
    }

    let rejected = tally.inconsistent + tally.too_short;
    println!("\n== rejected records, tested against a fixed {TRAILING_LEN}-byte trailing ==");
    println!("  exactly one offset fits  {}", tally.single_fit);
    println!("  several offsets fit      {}", tally.ambiguous);
    println!("  no offset fits           {}", tally.unexplained);
    println!("  (of {rejected} rejected)");
    println!("  offsets that fit uniquely:");
    let mut fits: Vec<(&usize, &usize)> = tally.fit_offsets.iter().collect();
    fits.sort_by_key(|(_, c)| Reverse(**c));
    for (len_at, count) in fits.iter().take(12) {
        println!(
            "    count@+{len_at:<3} text@+{:<3} {count} records",
            *len_at + 2
        );
    }

    println!("\n== strings the second layout recovers ==");
    let mut ft: Vec<(&String, &usize)> = tally.fit_texts.iter().collect();
    ft.sort_by_key(|(_, c)| Reverse(**c));
    for (text, count) in ft.iter().take(20) {
        println!("  {count:>4}x {text:?}");
    }
    println!("  ({} distinct)", tally.fit_texts.len());

    let mut sizes: BTreeMap<usize, usize> = BTreeMap::new();
    for l in &tally.unfit_lengths {
        *sizes.entry(*l).or_default() += 1;
    }
    let mut sv: Vec<(&usize, &usize)> = sizes.iter().collect();
    sv.sort_by_key(|(_, c)| Reverse(**c));
    println!("\n== payload sizes no offset explains ==");
    println!(
        "  {}",
        sv.iter()
            .take(14)
            .map(|(l, c)| format!("{l}B x{c}"))
            .collect::<Vec<_>>()
            .join("  ")
    );

    println!("\n== every (offset, trailing) reading, by how often the text looks like a label ==");
    let mut cands: Vec<(&(usize, usize), &Candidate)> = tally
        .candidates
        .iter()
        .filter(|(_, c)| c.clean > 0)
        .collect();
    cands.sort_by_key(|(_, c)| Reverse(c.clean));
    for ((len_at, trailing), c) in cands.iter().take(12) {
        println!(
            "  count@+{len_at:<3} text@+{:<3} trailing={trailing:<4} readings {:<5} label-like {}",
            len_at + 2,
            c.hits,
            c.clean
        );
    }

    println!("\n== rejected payloads, first 80 bytes ==");
    for d in &tally.dumps {
        println!("  {d}");
    }

    println!("\n== samples ==");
    for s in &tally.samples {
        println!("  {s}");
    }

    println!("\n== most common strings ==");
    let mut texts: Vec<(&String, &usize)> = tally.texts.iter().collect();
    texts.sort_by_key(|(_, c)| Reverse(**c));
    for (text, count) in texts.iter().take(25) {
        println!("  {count:>4}x {text:?}");
    }
    println!("  ({} distinct strings)", tally.texts.len());

    println!(
        "\n== symbols with text but no geometry ({} files) ==",
        text_only.len()
    );
    for (name, texts) in text_only.iter().take(20) {
        println!("  {name} -> {texts:?}");
    }
}
