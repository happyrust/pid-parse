//! Does any label in this corpus have more than one line?
//!
//! `JStyleTextPara +66` reads `{0.0: 8, 1.0: 332, 1.5: 36}` across the corpus
//! and the shape of those values — single and one-and-a-half — is what a line
//! spacing multiple looks like. The read order
//! (`style.dll!sub_100337A0`) is native-reader, so the *field* is not in
//! doubt. See `docs/analysis/2026-08-13-text-para-layout-and-justification.md`.
//!
//! But line spacing only moves a glyph when there is a second line to push
//! away from the first. **Nobody has measured whether this corpus has one.**
//! Wiring a field whose every consumer is a single-line label changes nothing
//! on screen and adds a value to carry, a test to keep green, and a reader who
//! believes it matters.
//!
//! So this probe measures the denominator before anything is wired:
//!
//! 1. how many `igTextBox` labels carry a line break in their text;
//! 2. which break code unit they use, if any;
//! 3. a falsifier that does not trust the decoder — a scan of **every**
//!    `0x004D` payload, accepted or refused, for a break code unit anywhere in
//!    it. A zero there cannot be explained away by the decoder's refusals;
//! 4. the cross-tab that actually decides the question: the `+66` value each
//!    label's paragraph style states, against whether that label is multi-line.
//!    Spacing `1.5` sitting only on single-line labels is a field with no
//!    consumer in this corpus.
//!
//! # The measurement口径, and how it could be wrong
//!
//! "Multi-line" here means **the decoded text contains a line-breaking code
//! point** — `U+000A`, `U+000D`, `U+000B`, `U+000C`, `U+0085`, `U+2028`,
//! `U+2029`. Every other C0 control is counted separately so that a break
//! spelled some way this list does not know about still shows up rather than
//! being silently filed as single-line.
//!
//! That口径 has a known way of being wrong. Shape 3 of `igTextBox` carries
//! `A + B` formatting runs after the text (see
//! `docs/analysis/2026-08-13-igtextbox-has-three-shapes.md` §five), and a
//! vendor that models rich text as runs can also model a paragraph break as a
//! run rather than as a character in the string. If that is what shape 3 does,
//! a label could be visually multi-line with no break code unit anywhere in
//! its text, and this probe would report it as single-line. The shape
//! cross-tab below is printed for exactly that reason: it says how much of the
//! corpus sits behind that uncertainty.
//!
//! ```powershell
//! cargo run --example probe_text_multiline_census
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use pid_parse::parsers::sheet_records::{
    decode_igtextbox_at, decode_igtextboxes, sheet_record_starts, PSM_ENVELOPE_LEN,
    PSM_TYPE_CODE_IGTEXTBOX,
};
use pid_parse::style_link::{
    stylecluster_path_for_sheet, DocumentStyleTable, PSM_TYPE_CODE_JSTYLE_TEXT_PARA,
};

/// Payload offset of the line spacing multiple inside a `JStyleTextPara`,
/// the field this probe exists to decide about. Level: native-reader.
const TEXT_PARA_LINE_SPACING_AT: usize = 66;

/// Bytes of PSM envelope before a style record's payload, for turning a
/// [`pid_parse::style_link::StyleRecord::byte_range`] into a payload offset.
const STYLE_ENVELOPE_LEN: usize = 6;

/// Code points that end a line. Anything here makes a label multi-line.
const BREAKS: [char; 7] = [
    '\u{000A}', '\u{000D}', '\u{000B}', '\u{000C}', '\u{0085}', '\u{2028}', '\u{2029}',
];

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "test-file/D06.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

/// Every stream in the file, by path, so sheets and their own document's
/// `StyleCluster` can be paired without re-opening the compound file.
///
/// A fixture that fails to open is fatal rather than skipped. This probe
/// exists to establish a denominator, and a fixture that quietly contributes
/// zero labels does not lower the count honestly — it lowers it invisibly.
fn streams(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let file = std::fs::File::open(path)
        .unwrap_or_else(|error| panic!("{} exists but did not open: {error}", path.display()));
    let mut cfb = cfb::CompoundFile::open(file).unwrap_or_else(|error| {
        panic!(
            "{} did not read as a compound file: {error}",
            path.display()
        )
    });
    let names: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(name.as_str()) else {
            continue;
        };
        let mut bytes = Vec::new();
        if stream.read_to_end(&mut bytes).is_ok() {
            out.insert(name, bytes);
        }
    }
    out
}

fn is_sheet(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .unwrap_or_default()
        .starts_with("Sheet")
}

/// Show a label on one line, with the breaks made visible.
fn escaped(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:04X}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn main() {
    // Decoder-accepted side.
    let mut accepted = 0usize;
    let mut multiline = 0usize;
    let mut break_units: BTreeMap<String, usize> = BTreeMap::new();
    let mut other_controls: BTreeMap<String, usize> = BTreeMap::new();
    let mut lines_hist: BTreeMap<usize, usize> = BTreeMap::new();
    let mut by_shape: BTreeMap<u16, (usize, usize)> = BTreeMap::new();
    let mut by_spacing: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut unresolved_spacing = 0usize;
    let mut samples: Vec<String> = Vec::new();
    // Per fixture, so a count can be attributed to a drawing rather than to
    // "the corpus" -- two of the six fixtures are the same drawing published
    // twice, and a total that does not say so overstates the evidence.
    let mut per_fixture: Vec<(String, usize, usize, usize)> = Vec::new();
    // Every label whose paragraph states the non-default spacing, so the
    // ones that are *not* multi-line can be looked at rather than assumed.
    let mut wide_spacing_labels: Vec<String> = Vec::new();

    // Raw side: every 0x004D on the chain, decoder or no decoder.
    let mut chain_records = 0usize;
    let mut chain_with_break_unit = 0usize;

    let mut skipped: Vec<&str> = Vec::new();
    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            skipped.push(fixture);
            continue;
        }
        let all = streams(path);
        assert!(
            all.keys().any(|name| is_sheet(name)),
            "{fixture} opened but holds no Sheet stream"
        );
        let (before_accepted, before_multi, before_wide) =
            (accepted, multiline, wide_spacing_labels.len());
        for (name, bytes) in all.iter().filter(|(name, _)| is_sheet(name)) {
            // Raw chain walk first: it does not depend on the decoder
            // accepting anything, so a zero here is the strongest form of
            // the negative result.
            for at in sheet_record_starts(bytes) {
                let Some(head) = bytes.get(at..at + 2) else {
                    continue;
                };
                if u16::from_le_bytes([head[0], head[1]]) & 0x3FFF != PSM_TYPE_CODE_IGTEXTBOX {
                    continue;
                }
                let Some(len_bytes) = bytes.get(at + 2..at + 6) else {
                    continue;
                };
                let btf =
                    u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]])
                        as usize;
                let Some(payload) = bytes.get(at + PSM_ENVELOPE_LEN..at + PSM_ENVELOPE_LEN + btf)
                else {
                    continue;
                };
                chain_records += 1;
                // Text lives at an even payload offset in all three shapes,
                // so even-aligned code units are the ones a label could be
                // made of. This over-counts — a double's bytes can spell
                // 0x000A — which is what makes a zero here meaningful.
                let has_break_unit = (0..payload.len().saturating_sub(1)).step_by(2).any(|at| {
                    let unit = u32::from(u16::from_le_bytes([payload[at], payload[at + 1]]));
                    char::from_u32(unit).is_some_and(|c| BREAKS.contains(&c))
                });
                if has_break_unit {
                    chain_with_break_unit += 1;
                }
                let _ = decode_igtextbox_at(bytes, at);
            }

            // Accepted side, joined to the paragraph style each label names.
            let style_bytes = all.get(&stylecluster_path_for_sheet(name));
            let table = style_bytes.map_or_else(DocumentStyleTable::default, |b| {
                DocumentStyleTable::from_stylecluster_bytes(b)
            });

            for record in decode_igtextboxes(bytes) {
                accepted += 1;
                let breaks = record.text.chars().filter(|c| BREAKS.contains(c)).count();
                let is_multi = breaks > 0;
                if is_multi {
                    multiline += 1;
                    if samples.len() < 8 {
                        samples.push(escaped(&record.text));
                    }
                }
                *lines_hist.entry(breaks + 1).or_default() += 1;
                for ch in record.text.chars() {
                    if BREAKS.contains(&ch) {
                        *break_units
                            .entry(format!("U+{:04X}", ch as u32))
                            .or_default() += 1;
                    } else if ch.is_control() {
                        *other_controls
                            .entry(format!("U+{:04X}", ch as u32))
                            .or_default() += 1;
                    }
                }

                let shape = by_shape.entry(record.text_sub_type).or_default();
                shape.0 += 1;
                shape.1 += usize::from(is_multi);

                // The paragraph style this label names, and the +66 it states.
                // `style_link` does not surface the field, so it is read out
                // of the record's own bytes via its byte range.
                let spacing = table.get(record.index).and_then(|style| {
                    if style.type_code != PSM_TYPE_CODE_JSTYLE_TEXT_PARA {
                        return None;
                    }
                    let payload_at = style.byte_range.start + STYLE_ENVELOPE_LEN;
                    let at = payload_at + TEXT_PARA_LINE_SPACING_AT;
                    let raw = style_bytes?.get(at..at + 8)?;
                    Some(f64::from_le_bytes([
                        raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                    ]))
                });
                match spacing {
                    Some(value) => {
                        let slot = by_spacing.entry(format!("{value:.3}")).or_default();
                        slot.0 += 1;
                        slot.1 += usize::from(is_multi);
                        if (value - 1.0).abs() > 1e-9 {
                            wide_spacing_labels.push(format!(
                                "{value:.3}  lines={}  {}",
                                breaks + 1,
                                escaped(&record.text)
                            ));
                        }
                    }
                    None => unresolved_spacing += 1,
                }
            }
        }
        per_fixture.push((
            fixture.rsplit('/').next().unwrap_or(fixture).to_string(),
            accepted - before_accepted,
            multiline - before_multi,
            wide_spacing_labels.len() - before_wide,
        ));
    }

    println!("=== igTextBox labels: how many have more than one line? ===");
    assert!(
        skipped.is_empty(),
        "these fixtures are missing, so the denominator below is not the corpus: {skipped:?}"
    );
    println!("chain 0x004D records:  {chain_records}");
    println!("decoder-accepted:      {accepted}");
    println!();

    println!("--- accepted text, breaks counted as code points ---");
    let pct = if accepted == 0 {
        0.0
    } else {
        100.0 * multiline as f64 / accepted as f64
    };
    println!("multi-line records:    {multiline} / {accepted}  ({pct:.2}%)");
    println!("break code points seen: {break_units:?}");
    println!("other control chars:    {other_controls:?}");
    println!("lines per record:       {lines_hist:?}");
    if !samples.is_empty() {
        println!("samples:");
        for sample in &samples {
            println!("  {sample}");
        }
    }

    println!("\n--- by shape (+18); shape 3 is the rich-text one ---");
    println!("{:>6} {:>10} {:>12}", "shape", "records", "multi-line");
    for (shape, (total, multi)) in &by_shape {
        println!("{shape:>6} {total:>10} {multi:>12}");
    }

    println!("\n--- falsifier: a break code unit anywhere in the payload ---");
    println!("(over-counts: a double's bytes can spell one. A zero cannot.)");
    println!("payloads containing one: {chain_with_break_unit} / {chain_records}");

    println!("\n--- +66 line spacing, against multi-line ---");
    println!("{:>10} {:>10} {:>12}", "+66", "labels", "multi-line");
    for (value, (total, multi)) in &by_spacing {
        println!("{value:>10} {total:>10} {multi:>12}");
    }
    println!("labels whose paragraph style did not resolve: {unresolved_spacing}");
    if !wide_spacing_labels.is_empty() {
        println!("\nevery label whose paragraph states a non-default spacing:");
        for label in &wide_spacing_labels {
            println!("  {label}");
        }
    }

    // The same view the ratchet asserts over: only the labels whose height
    // resolves, reached through the public API rather than by re-reading the
    // bytes. The counts differ from the census above because a label whose
    // character style states an unusable height resolves to nothing at all,
    // and the ratchet has to pin what a consumer actually receives.
    println!("\n--- as `text_heights_for_file` hands it to a consumer ---");
    let mut resolved_spacing: BTreeMap<String, usize> = BTreeMap::new();
    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        let index = pid_parse::style_link::text_heights_for_file(path).expect("fixture opens");
        for style in index.values() {
            let key = style
                .line_spacing
                .map_or_else(|| "unstated".to_string(), |value| format!("{value:.3}"));
            *resolved_spacing.entry(key).or_default() += 1;
        }
    }
    println!("{resolved_spacing:?}");
    println!("total: {}", resolved_spacing.values().sum::<usize>());

    // The index is keyed by (stream, oid), so two labels sharing an oid within
    // one sheet collapse into one entry and the second silently inherits the
    // first's style. Counting labels and counting index entries then disagree,
    // and a ratchet that pins one while quoting the other is off by however
    // many duplicates there are. Measure it rather than assume it is zero.
    println!("\n--- do two labels ever share one (stream, oid)? ---");
    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        let mut seen: BTreeMap<(String, u32), usize> = BTreeMap::new();
        for (name, bytes) in streams(path).iter().filter(|(name, _)| is_sheet(name)) {
            for record in decode_igtextboxes(bytes) {
                *seen.entry((name.clone(), record.oid)).or_default() += 1;
            }
        }
        let duplicated: Vec<_> = seen.iter().filter(|(_, n)| **n > 1).collect();
        if !duplicated.is_empty() {
            println!(
                "  {}: {duplicated:?}",
                fixture.rsplit('/').next().unwrap_or(fixture)
            );
        }
    }

    println!("\n--- per fixture ---");
    println!(
        "{:>44} {:>8} {:>11} {:>10}",
        "fixture", "labels", "multi-line", "spacing≠1"
    );
    for (name, labels, multi, wide) in &per_fixture {
        println!("{name:>44} {labels:>8} {multi:>11} {wide:>10}");
    }

    println!("\n=== verdict ===");
    if multiline == 0 && chain_with_break_unit == 0 {
        println!(
            "No label in this corpus has a second line, and no igTextBox payload\n\
             even contains a break code unit -- which the decoder's refusals\n\
             cannot explain away, because that scan does not go through it.\n\
             +66 therefore has no consumer here: wiring it would carry a value\n\
             from the file to the renderer that no drawing can act on.\n\
             Register the negative and stop.\n\
             \n\
             The one door this leaves open is shape 3's formatting runs, which\n\
             could in principle spell a paragraph break without a character.\n\
             The shape table above says how many records that is."
        );
    } else if multiline == 0 {
        println!(
            "No accepted label is multi-line, but {chain_with_break_unit} payload(s) do contain a\n\
             break code unit. Before registering a negative, find out whether\n\
             those are refused text records or merely doubles that spell one."
        );
    } else {
        println!(
            "{multiline} label(s) are multi-line, so +66 has a real consumer. Wire it,\n\
             and pin the measured distribution above rather than a guessed one."
        );
    }
}
