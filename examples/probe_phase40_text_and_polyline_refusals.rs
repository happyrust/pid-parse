//! Phase 40 S3 — which rule refuses the 53 records that are left?
//!
//! S1 found 141 chain-resident records their own family's decoder refused in
//! silence. S5 took 88 of them: `igLine2d` refused on `aux_hi == 12`, a rule
//! this crate invented for a PSM envelope field the native reader discards.
//!
//! The remaining 53 are `igTextBox` (`0x004D`) and `igLineString2d`
//! (`0x0084`), and they cannot share that cause — neither family validates
//! `aux_hi` at all (the polyline decoder only rejects it above `0xFFFF`). So
//! this is a different illness, and the first question is the same one S1
//! asked: **which single rule fires, and how many populations are there?**
//!
//! Every rule below mirrors the shipped decoder in its order, so "first rule
//! to fire" here is the reason the shipped decoder said no.
//!
//! ```powershell
//! cargo run --example probe_phase40_text_and_polyline_refusals
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::model::SHEET_RECORD_FAMILIES;
use pid_parse::parsers::sheet_records::{
    decode_iglinestring_at, decode_igtextbox_at, sheet_record_starts,
    IGLINESTRING2D_MIN_PAYLOAD_LEN, IGTEXTBOX_PAYLOAD_OVERHEAD, PSM_ENVELOPE_LEN,
    PSM_TYPE_CODE_IGLINESTRING2D, PSM_TYPE_CODE_IGTEXTBOX,
};

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "test-file/D06.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

/// Private in the crate; mirrored here so the attribution matches the shipped
/// decoder rule for rule.
const IGTEXTBOX_MAX_TEXT_LENGTH: u16 = 1024;
const IGLINESTRING2D_MAX_VERTEX_COUNT: u32 = 10_000;
const IGLINESTRING2D_FORM_MAX: u8 = 6;
const DOMAIN_LIMIT: f64 = 1e9;

/// Which rule said no, plus the numbers that make it readable.
struct Verdict {
    rule: &'static str,
    detail: String,
}

fn read_u32(payload: &[u8], at: usize) -> Option<u32> {
    payload
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_u16(payload: &[u8], at: usize) -> Option<u16> {
    payload
        .get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn read_f64(payload: &[u8], at: usize) -> Option<f64> {
    payload
        .get(at..at + 8)
        .map(|b| f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
}

/// Re-run `IgTextBoxDecoder`'s rules in order; report the first that fires.
fn attribute_textbox(bytes: &[u8], off: usize, btf: usize) -> Verdict {
    if btf < IGTEXTBOX_PAYLOAD_OVERHEAD {
        return Verdict {
            rule: "btf-below-overhead",
            detail: format!("btf={btf} < {IGTEXTBOX_PAYLOAD_OVERHEAD}"),
        };
    }
    if !(btf - IGTEXTBOX_PAYLOAD_OVERHEAD).is_multiple_of(2) {
        return Verdict {
            rule: "btf-odd-text-bytes",
            detail: format!("btf={btf}, (btf-{IGTEXTBOX_PAYLOAD_OVERHEAD}) is odd"),
        };
    }
    let derived = ((btf - IGTEXTBOX_PAYLOAD_OVERHEAD) / 2) as u16;
    if derived > IGTEXTBOX_MAX_TEXT_LENGTH {
        return Verdict {
            rule: "text-too-long",
            detail: format!("derived={derived} > {IGTEXTBOX_MAX_TEXT_LENGTH}"),
        };
    }
    let Some(payload) = bytes.get(off + PSM_ENVELOPE_LEN..off + PSM_ENVELOPE_LEN + btf) else {
        return Verdict {
            rule: "truncated",
            detail: format!("payload of {btf} runs past the stream"),
        };
    };
    let Some(inline) = read_u16(payload, 30) else {
        return Verdict {
            rule: "truncated",
            detail: "no room for the inline length at +30".to_string(),
        };
    };
    if inline != derived {
        return Verdict {
            rule: "inline-len-mismatch",
            detail: format!("inline(+30)={inline} vs derived-from-btf={derived}"),
        };
    }
    let text_end = 32 + (inline as usize) * 2;
    if text_end + 36 > payload.len() {
        return Verdict {
            rule: "no-room-for-tail",
            detail: format!("text_end={text_end}, +36 > payload {}", payload.len()),
        };
    }
    for i in 0..3 {
        let Some(v) = read_f64(payload, text_end + i * 8) else {
            return Verdict {
                rule: "truncated-tail",
                detail: format!("trailing double {i} runs past the payload"),
            };
        };
        if !v.is_finite() {
            return Verdict {
                rule: "nonfinite-trailing",
                detail: format!("trailing[{i}] is not finite"),
            };
        }
        if v.abs() > DOMAIN_LIMIT {
            return Verdict {
                rule: "trailing-out-of-domain",
                detail: format!("trailing[{i}]={v:e}"),
            };
        }
    }
    Verdict {
        rule: "NO-RULE-FIRED",
        detail: "probe and decoder disagree — the probe is wrong".to_string(),
    }
}

/// Re-run `IgLineString2dDecoder`'s rules in order; report the first to fire.
fn attribute_linestring(bytes: &[u8], off: usize, btf: usize) -> Verdict {
    if btf < IGLINESTRING2D_MIN_PAYLOAD_LEN {
        return Verdict {
            rule: "btf-below-minimum",
            detail: format!("btf={btf} < {IGLINESTRING2D_MIN_PAYLOAD_LEN}"),
        };
    }
    if !(btf - 24).is_multiple_of(16) {
        return Verdict {
            rule: "btf-not-vertex-aligned",
            detail: format!("btf={btf}, (btf-24) % 16 = {}", (btf - 24) % 16),
        };
    }
    let computed_vc = ((btf - 24) / 16) as u32;
    if !(2..=IGLINESTRING2D_MAX_VERTEX_COUNT).contains(&computed_vc) {
        return Verdict {
            rule: "vertex-count-out-of-range",
            detail: format!("computed_vc={computed_vc}"),
        };
    }
    let Some(payload) = bytes.get(off + PSM_ENVELOPE_LEN..off + PSM_ENVELOPE_LEN + btf) else {
        return Verdict {
            rule: "truncated",
            detail: format!("payload of {btf} runs past the stream"),
        };
    };
    let aux_hi = read_u32(payload, 8).unwrap_or_default();
    if aux_hi > 0xFFFF {
        return Verdict {
            rule: "aux-hi-above-0xFFFF",
            detail: format!("aux_hi={aux_hi}"),
        };
    }
    let inline_vc = read_u32(payload, 18).unwrap_or_default();
    if inline_vc != computed_vc {
        return Verdict {
            rule: "inline-vc-mismatch",
            detail: format!("inline(+18)={inline_vc} vs derived-from-btf={computed_vc}"),
        };
    }
    let form = payload[22];
    let scope = payload[23];
    if form > IGLINESTRING2D_FORM_MAX {
        return Verdict {
            rule: "form-out-of-range",
            detail: format!("form={form} > {IGLINESTRING2D_FORM_MAX}"),
        };
    }
    if scope > 4 && scope != 6 {
        return Verdict {
            rule: "scope-out-of-range",
            detail: format!("scope={scope}"),
        };
    }
    let mut first = (0.0f64, 0.0f64);
    let mut all_same = true;
    for i in 0..computed_vc as usize {
        let pos = 24 + i * 16;
        let (Some(x), Some(y)) = (read_f64(payload, pos), read_f64(payload, pos + 8)) else {
            return Verdict {
                rule: "truncated-vertices",
                detail: format!("vertex {i} runs past the payload"),
            };
        };
        if !x.is_finite() || !y.is_finite() {
            return Verdict {
                rule: "nonfinite-vertex",
                detail: format!("vertex {i} is not finite"),
            };
        }
        if x.abs() > DOMAIN_LIMIT || y.abs() > DOMAIN_LIMIT {
            return Verdict {
                rule: "vertex-out-of-domain",
                detail: format!("vertex {i} = ({x:e}, {y:e})"),
            };
        }
        if i == 0 {
            first = (x, y);
        } else if (x - first.0).abs() > 1e-12 || (y - first.1).abs() > 1e-12 {
            all_same = false;
        }
    }
    if all_same {
        return Verdict {
            rule: "degenerate-all-vertices-equal",
            detail: format!(
                "{computed_vc} vertices, all ({:.4}, {:.4})",
                first.0, first.1
            ),
        };
    }
    Verdict {
        rule: "NO-RULE-FIRED",
        detail: "probe and decoder disagree — the probe is wrong".to_string(),
    }
}

/// Take `+30` at its word and see what it implies. A record whose real text
/// length is `inline` needs `btf - 2*inline` bytes of overhead; if that number
/// clusters and the bytes at `+32` read as text, the fixed overhead is the
/// wrong assumption rather than the field.
fn record_implied_overhead(
    bytes: &[u8],
    at: usize,
    btf: usize,
    out: &mut BTreeMap<i64, (usize, usize, String)>,
) {
    let Some(payload) = bytes.get(at + PSM_ENVELOPE_LEN..at + PSM_ENVELOPE_LEN + btf) else {
        return;
    };
    let Some(inline) = read_u16(payload, 30) else {
        return;
    };
    let implied = btf as i64 - 2 * i64::from(inline);
    let mut chars = Vec::new();
    let mut readable = inline > 0;
    for i in 0..inline as usize {
        match read_u16(payload, 32 + i * 2) {
            Some(unit) => chars.push(unit),
            None => {
                readable = false;
                break;
            }
        }
    }
    let text = String::from_utf16_lossy(&chars);
    // "Reads as text" is deliberately crude: printable, no replacement chars.
    if readable {
        readable = !text.contains('\u{FFFD}')
            && text
                .chars()
                .all(|c| !c.is_control() && (c.is_ascii_graphic() || c == ' ' || !c.is_ascii()));
    }
    let entry = out.entry(implied).or_insert((0, 0, String::new()));
    entry.0 += 1;
    if readable {
        entry.1 += 1;
        if entry.2.is_empty() {
            entry.2 = text;
        }
    }
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

fn main() {
    // family -> rule -> (refused by the family, of those visible to the
    // shipped census). The two differ: the census calls a record refused only
    // when no family's decoded range covers its start, so a record another
    // family's bytes overlap never reaches the warning.
    let mut by_rule: BTreeMap<(&str, &str), (usize, usize)> = BTreeMap::new();
    let mut samples: BTreeMap<(&str, &str), Vec<String>> = BTreeMap::new();
    let mut accepted: BTreeMap<&str, usize> = BTreeMap::new();
    let mut refused_per_stream: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    // Which family's decoded range swallows a refused record's start byte.
    let mut covered_by: BTreeMap<String, usize> = BTreeMap::new();
    // For the inline-length mismatches: `btf - 2*inline` -> (count, how many
    // read as plausible text at +32, one sample).
    let mut implied_overhead: BTreeMap<i64, (usize, usize, String)> = BTreeMap::new();

    for fixture in FIXTURES {
        for (name, bytes) in sheet_streams(Path::new(fixture)) {
            let claimed: Vec<(&str, std::ops::Range<usize>)> = SHEET_RECORD_FAMILIES
                .iter()
                .flat_map(|family| {
                    (family.decoded_ranges)(&bytes)
                        .into_iter()
                        .map(move |range| (family.name, range))
                })
                .collect();

            for at in sheet_record_starts(&bytes) {
                let Some(raw) = bytes.get(at..at + 2) else {
                    continue;
                };
                let type_code = u16::from_le_bytes([raw[0], raw[1]]) & 0x3FFF;
                let Some(btf) = read_u32(&bytes, at + 2).map(|v| v as usize) else {
                    continue;
                };

                let (family, refused, verdict) = match type_code {
                    PSM_TYPE_CODE_IGTEXTBOX => (
                        "igTextBox",
                        decode_igtextbox_at(&bytes, at).is_none(),
                        attribute_textbox as fn(&[u8], usize, usize) -> Verdict,
                    ),
                    PSM_TYPE_CODE_IGLINESTRING2D => (
                        "igLineString2d",
                        decode_iglinestring_at(&bytes, at).is_none(),
                        attribute_linestring as fn(&[u8], usize, usize) -> Verdict,
                    ),
                    _ => continue,
                };

                if !refused {
                    *accepted.entry(family).or_default() += 1;
                    continue;
                }

                let coverer = claimed
                    .iter()
                    .find(|(_, range)| range.contains(&at))
                    .map(|(name, range)| (*name, range.clone()));
                let census_visible = coverer.is_none();
                if let Some((name, range)) = &coverer {
                    *covered_by
                        .entry(format!("{name} (starts 0x{:06X})", range.start))
                        .or_default() += 1;
                }
                let v = verdict(&bytes, at, btf);
                if v.rule == "inline-len-mismatch" {
                    record_implied_overhead(&bytes, at, btf, &mut implied_overhead);
                }
                let tally = by_rule.entry((family, v.rule)).or_default();
                tally.0 += 1;
                if census_visible {
                    tally.1 += 1;
                    let entry = refused_per_stream
                        .entry(format!("{fixture} {name}"))
                        .or_default();
                    if family == "igTextBox" {
                        entry.0 += 1;
                    } else {
                        entry.1 += 1;
                    }
                }
                let bucket = samples.entry((family, v.rule)).or_default();
                if bucket.len() < 4 {
                    let seen = if census_visible {
                        "reported"
                    } else {
                        "covered "
                    };
                    bucket.push(format!(
                        "[{seen}] {name} @ 0x{at:06X} btf={btf} — {}",
                        v.detail
                    ));
                }
            }
        }
    }

    println!("=== which rule refuses, by family ===");
    println!(
        "{:<16} {:<30} {:>8} {:>10}",
        "family", "rule", "refused", "reported"
    );
    for ((family, rule), (refused, reported)) in &by_rule {
        println!("{family:<16} {rule:<30} {refused:>8} {reported:>10}");
    }
    println!(
        "\n`refused` is what the family's own decoder rejects; `reported` is the\n\
         subset the shipped census warns about — the rest have their start byte\n\
         inside some other family's decoded range, so nothing names them."
    );

    println!("\n=== accepted today, for scale ===");
    for (family, count) in &accepted {
        println!("{family:<16} {count:>4}");
    }

    println!("\n=== reported refusals by stream (text, polyline) ===");
    for (stream, (text, poly)) in &refused_per_stream {
        if *text > 0 || *poly > 0 {
            println!("{text:>3} text  {poly:>3} poly   {stream}");
        }
    }

    println!("\n=== if +30 is the real text length, what overhead does btf imply ===");
    println!(
        "The decoder derives the length from `btf` assuming a fixed {IGTEXTBOX_PAYLOAD_OVERHEAD}-byte\n\
         overhead and refuses when +30 disagrees. If +30 is right and the overhead is\n\
         variable, `btf - 2*inline` is that record's real overhead and the text at +32\n\
         should read as text. If +30 is garbage, neither holds."
    );
    println!(
        "{:>9} {:>7} {:>11}  sample text at +32",
        "implied", "count", "text-ok"
    );
    for (implied, (count, ok, sample)) in &implied_overhead {
        println!("{implied:>9} {count:>7} {ok:>7}/{count:<3}  {sample:?}");
    }

    println!("\n=== who covers the refusals nothing reports ===");
    let mut families: BTreeMap<&str, usize> = BTreeMap::new();
    for (key, count) in &covered_by {
        let family = key.split_whitespace().next().unwrap_or(key);
        *families.entry(family).or_default() += count;
    }
    for (family, count) in &families {
        println!("{family:<20} covers {count:>4} refused record start(s)");
    }

    println!("\n=== samples ===");
    for ((family, rule), bucket) in &samples {
        println!("\n{family} / {rule}:");
        for sample in bucket {
            println!("  {sample}");
        }
    }
}
