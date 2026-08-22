//! Close the `igTextBox` byte account: tail kind 2's extra bytes and the
//! formatting runs behind shape 3.
//!
//! Two structures were left open on 2026-08-13. Both come out of the same
//! native chain, so one probe checks both.
//!
//! `radsrvitem.dll` reads an `igTextBox` with `IGDSFactoryText::Load`
//! (`sub_56498C00`). The text sink is the class's second base at `this + 20`;
//! the placement tail is handed to `slot +24` of an
//! `IGDSFactoryTextPointRectShape` parked at `this + 88`
//! (`sub_56498780`). The matching sizers -- `IGDSFactoryText::GetSize`
//! (`sub_56498AB0`) and `IGDSFactoryTextPointRectShape::GetSize`
//! (`sub_56497AE0`) -- state the record length as a sum:
//!
//! ```text
//!   bytes_to_follow = 22            common header
//!                   + body_len      per shape
//!                   + tail_len      per tail kind
//!
//!   body_len   shape 1 :  2 + 2*count
//!              shape 2 : 10 + 2*count
//!              shape 3 :  6 + 2*count + 8*(A + B)
//!
//!   tail_len   kind 1  : 36
//!              kind 2  : 68 + 8 * popcount(flags & 0x3F80_0000)
//! ```
//!
//! `flags` is the `u32` at `tail + 36` -- the first dword past the 36 bytes
//! kind 1 stops at. Its seven high bits each gate one optional 8-byte field,
//! which is where "32 or 40 extra bytes" came from: 68 - 36 = 32 with no bit
//! set, 76 - 36 = 40 with one.
//!
//! The runs are the other half. `Load` walks `A + B` entries of 8 bytes past
//! the text, `(u16 length, u16 selector, u32 value)`, dispatching selector 1
//! to sink slot `+68` and selector 2 to slot `+72`; the sinks stamp the
//! selector back in themselves (`sub_564978B0` writes `1`, `sub_56497930`
//! writes `2`). `GetSize` refuses a record whose selector-1 lengths do not
//! sum to the character count, and derives the shape word from the run
//! counts: no runs is shape 1, exactly one selector-1 run is shape 2,
//! anything else is shape 3. So shape 2's "count packed with `0x10000`" at
//! `+22` was never a marker -- it is one run entry whose selector happens to
//! sit in the high half.
//!
//! What this probe measures:
//!
//! 1. **the byte account closes** -- `22 + body_len + tail_len` equals the
//!    payload length exactly, on every record, with nothing left over;
//! 2. the distribution of tail lengths, flag words and optional fields;
//! 3. that the runs partition the text the way the native sizer demands, and
//!    that the run counts reproduce the shape word.
//!
//! ```powershell
//! cargo run --example probe_igtextbox_tail_and_runs
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::{
    decode_igtextbox_at, sheet_record_starts, PSM_ENVELOPE_LEN, PSM_TYPE_CODE_IGTEXTBOX,
};

/// Payload offset of the shape discriminator (record byte 24).
const SHAPE_AT: usize = 18;
/// Payload offset of the tail-kind tag (record byte 26).
const TAIL_KIND_AT: usize = 20;
/// Payload offset where the body starts, after the 22-byte common header.
const BODY_START: usize = 22;
/// Bytes a kind-1 tail occupies: 2 doubles of insertion, 2 of direction, and
/// a dword. `IGDSFactoryTextPointRectShape::GetSize` returns exactly this.
const TAIL_KIND1_LEN: usize = 36;
/// Fixed part of a kind-2 tail, before any optional field.
const TAIL_KIND2_BASE_LEN: usize = 68;
/// Offset within the tail of the flag word that gates the optional fields.
const TAIL_FLAGS_AT: usize = 36;
/// The seven bits of the flag word that each add an 8-byte field. The native
/// "is this kind 2" test (`sub_56497A10`) checks this same mask.
const TAIL_OPTIONAL_MASK: u32 = 0x3F80_0000;
/// The optional fields in the order the native reader consumes them.
const TAIL_OPTIONAL_BITS: [u32; 7] = [
    0x0080_0000,
    0x1000_0000,
    0x2000_0000,
    0x0100_0000,
    0x0200_0000,
    0x0400_0000,
    0x0800_0000,
];

/// Payload offset of a style record's own id, shared by every style family.
const STYLE_ID_AT: usize = 14;
/// Bytes of stream header before the first record in a style stream.
const STREAM_HEADER: usize = 8;
/// PSM type of the paragraph style the text record names through `+14`.
const PSM_TEXT_PARA_STYLE: u16 = 0x002D;
/// PSM type of the character style a paragraph style points at.
const PSM_TEXT_CHAR_STYLE: u16 = 0x002C;
/// Where a `JStyleTextPara` names its character style.
const TEXT_PARA_CHAR_REF_AT: usize = 38;

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/\u{5DE5}\u{827A}\u{7BA1}\u{9053}\u{53CA}\u{4EEA}\u{8868}\u{6D41}\u{7A0B}-1.pid",
    "test-file/D06.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let s = d.get(at..at + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let s = d.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn f64_at(d: &[u8], at: usize) -> Option<f64> {
    let s = d.get(at..at + 8)?;
    Some(f64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

/// Sheet streams of one fixture, or why it could not be read.
///
/// The usual probe idiom swallows an open failure and returns no streams,
/// which reads downstream as "this fixture has no records" -- indistinguishable
/// from a fixture that is genuinely empty. That is how a corpus of 260 quietly
/// becomes a corpus of 201 when something else on the machine happens to hold
/// a `.pid` open. A count nobody can tell is short is worse than an error.
fn sheet_streams(path: &Path) -> Result<Vec<Vec<u8>>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut cfb = cfb::CompoundFile::open(file).map_err(|e| e.to_string())?;
    let names: Vec<PathBuf> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_path_buf())
        .filter(|p| p.to_string_lossy().contains("Sheet"))
        .collect();
    let mut out = Vec::new();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(&name) else {
            continue;
        };
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        out.push(bytes);
    }
    Ok(out)
}

/// What a `JStyleTextChar` actually puts on the page. Comparing these rather
/// than the style ids is what says whether two ids would render differently.
#[derive(Clone, PartialEq)]
struct Lettering {
    height_m: f64,
    colour: u32,
    font: String,
}

/// One style record, reduced to what this probe needs.
struct StyleRec {
    type_code: u16,
    /// `JStyleTextPara +38`, the character style it defaults to.
    char_ref: Option<u32>,
    /// Set for `JStyleTextChar` only.
    lettering: Option<Lettering>,
}

/// Height, colour and font name out of a `JStyleTextChar`, using the offsets
/// the native read order pinned on 2026-08-13.
fn lettering_of(payload: &[u8]) -> Option<Lettering> {
    let height_m = f64_at(payload, 42)?;
    let colour = u32_at(payload, 34)?;
    let count = u16_at(payload, 68)? as usize;
    if payload.len() != 70 + 2 * count {
        return None;
    }
    let mut units = Vec::with_capacity(count);
    for i in 0..count {
        units.push(u16_at(payload, 70 + i * 2)?);
    }
    Some(Lettering {
        height_m,
        colour,
        font: String::from_utf16_lossy(&units),
    })
}

/// Every style record's id (`payload +14`) mapped to the PSM type that
/// declares it, so a run's `u32` can be tested against the style tables
/// instead of just tallied.
fn style_ids(path: &Path) -> BTreeMap<u32, StyleRec> {
    let mut out = BTreeMap::new();
    let Ok(file) = std::fs::File::open(path) else {
        return out;
    };
    let Ok(mut cfb) = cfb::CompoundFile::open(file) else {
        return out;
    };
    let names: Vec<PathBuf> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|e| e.path().to_path_buf())
        .filter(|p| p.to_string_lossy().to_lowercase().contains("style"))
        .collect();
    for name in names {
        let Ok(mut stream) = cfb.open_stream(&name) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() {
            continue;
        }
        let mut at = STREAM_HEADER;
        while at + PSM_ENVELOPE_LEN <= data.len() {
            let (Some(type_word), Some(btf)) = (u16_at(&data, at), u32_at(&data, at + 2)) else {
                break;
            };
            let end = at + PSM_ENVELOPE_LEN + btf as usize;
            if type_word & 0x3FFF == 0 || btf == 0 || end > data.len() {
                break;
            }
            let payload = &data[at + PSM_ENVELOPE_LEN..end];
            let type_code = type_word & 0x3FFF;
            if let Some(id) = u32_at(payload, STYLE_ID_AT) {
                // A `JStyleTextPara` names its character style at +38; that is
                // the second hop the shipped decoder already walks.
                let char_ref = if type_code == PSM_TEXT_PARA_STYLE {
                    u32_at(payload, TEXT_PARA_CHAR_REF_AT)
                } else {
                    None
                };
                let lettering = if type_code == PSM_TEXT_CHAR_STYLE {
                    lettering_of(payload)
                } else {
                    None
                };
                out.insert(
                    id,
                    StyleRec {
                        type_code,
                        char_ref,
                        lettering,
                    },
                );
            }
            at = end;
        }
    }
    out
}

/// One formatting run as it sits on disk.
#[derive(Clone, Copy)]
struct Run {
    length: u16,
    selector: u16,
    value: u32,
}

/// The body as the native Load reads it: character count, where the text
/// starts, the run counts, and how long the body runs.
struct Body {
    count: u16,
    text_at: usize,
    runs_at: usize,
    a: u16,
    b: u16,
    len: usize,
}

/// `IGDSFactoryText::Load`'s three body shapes, and where its `GetSize`
/// counterpart says each one ends.
fn body_of(payload: &[u8], shape: u16) -> Option<Body> {
    match shape {
        1 => {
            let count = u16_at(payload, BODY_START)?;
            Some(Body {
                count,
                text_at: BODY_START + 2,
                runs_at: 0,
                a: 0,
                b: 0,
                len: 2 + 2 * count as usize,
            })
        }
        2 => {
            let count = u16_at(payload, BODY_START + 8)?;
            Some(Body {
                count,
                text_at: BODY_START + 10,
                // The single run sits *before* the count, at the body start.
                runs_at: BODY_START,
                a: 1,
                b: 0,
                len: 10 + 2 * count as usize,
            })
        }
        3 => {
            let a = u16_at(payload, BODY_START)?;
            let b = u16_at(payload, BODY_START + 2)?;
            let count = u16_at(payload, BODY_START + 4)?;
            let text_at = BODY_START + 6;
            Some(Body {
                count,
                text_at,
                runs_at: text_at + 2 * count as usize,
                a,
                b,
                len: 6 + 2 * count as usize + 8 * (a as usize + b as usize),
            })
        }
        _ => None,
    }
}

fn runs_of(payload: &[u8], body: &Body) -> Option<Vec<Run>> {
    let total = body.a as usize + body.b as usize;
    let mut out = Vec::with_capacity(total);
    for i in 0..total {
        let at = body.runs_at + i * 8;
        out.push(Run {
            length: u16_at(payload, at)?,
            selector: u16_at(payload, at + 2)?,
            value: u32_at(payload, at + 4)?,
        });
    }
    Some(out)
}

/// What `IGDSFactoryTextPointRectShape::GetSize` returns for this tail.
fn tail_len(payload: &[u8], tail_at: usize, kind: u16) -> Option<usize> {
    match kind {
        1 => Some(TAIL_KIND1_LEN),
        2 => {
            let flags = u32_at(payload, tail_at + TAIL_FLAGS_AT)?;
            let extra = (flags & TAIL_OPTIONAL_MASK).count_ones() as usize;
            Some(TAIL_KIND2_BASE_LEN + 8 * extra)
        }
        _ => None,
    }
}

/// The shape word `IGDSFactoryText::GetSize` derives from the run counts.
fn shape_from_runs(a: u16, b: u16) -> u16 {
    if a == 0 && b == 0 {
        1
    } else if a == 1 && b == 0 {
        2
    } else {
        3
    }
}

fn bump<K: Ord>(map: &mut BTreeMap<K, usize>, key: K) {
    *map.entry(key).or_default() += 1;
}

#[derive(Default)]
struct Tally {
    total: usize,
    per_fixture: Vec<(&'static str, usize, usize, usize)>,
    unreadable: Vec<(&'static str, String)>,
    shape_kind: BTreeMap<(u16, u16), usize>,
    accepted: usize,
    account_closes: usize,
    account_misses: Vec<String>,
    tail_lens: BTreeMap<usize, usize>,
    extra_past_placement: BTreeMap<usize, usize>,
    flag_words: BTreeMap<u32, usize>,
    optional_counts: BTreeMap<usize, usize>,
    trailer_dword: BTreeMap<u32, usize>,
    rect_a: BTreeMap<String, usize>,
    rect_b: BTreeMap<String, usize>,
    tail_dwords: BTreeMap<(u32, u32, u32), usize>,
    ab: BTreeMap<(u16, u16), usize>,
    selectors: BTreeMap<u16, usize>,
    run_lengths_sum_ok: usize,
    run_lengths_sum_checked: usize,
    selector_order_ok: usize,
    shape_rule_ok: usize,
    run_values: BTreeMap<u32, usize>,
    run_value_is_own_index: usize,
    run_value_total: usize,
    multi_run_records: usize,
    run_samples: Vec<(String, Vec<Run>)>,
    /// `(selector, PSM type the value resolves to)`; type `0` means the value
    /// is not a style id in this file at all.
    run_points_at: BTreeMap<(u16, u16), usize>,
    run_equals_own_index: BTreeMap<u16, usize>,
    own_index_points_at: BTreeMap<u16, usize>,
    one_hop_agrees: usize,
    one_hop_differs: usize,
    one_hop_unresolved: usize,
    lettering_same: usize,
    lettering_differs: usize,
    lettering_unresolved: usize,
    lettering_examples: Vec<String>,
}

/// Name the style families the run values land in.
fn psm_name(type_code: u16) -> String {
    match type_code {
        0 => "not a style id in this file".to_owned(),
        PSM_TEXT_CHAR_STYLE => "0x002C JStyleTextChar".to_owned(),
        PSM_TEXT_PARA_STYLE => "0x002D JStyleTextPara".to_owned(),
        other => format!("{other:#06x} other style family"),
    }
}

/// The label itself, so a multi-run record can be read against its runs.
fn text_of(payload: &[u8], at: usize, count: u16) -> String {
    let mut units = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        match u16_at(payload, at + i * 2) {
            Some(unit) => units.push(unit),
            None => break,
        }
    }
    String::from_utf16_lossy(&units)
}

fn tidy(x: f64) -> String {
    if x == 0.0 {
        "0".to_owned()
    } else {
        format!("{x:.6}")
    }
}

fn main() {
    let mut t = Tally::default();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            continue;
        }
        let before = t.total;
        let styles = style_ids(path);
        let streams = match sheet_streams(path) {
            Ok(streams) => streams,
            Err(why) => {
                t.unreadable.push((fixture, why));
                continue;
            }
        };
        let stream_bytes: usize = streams.iter().map(Vec::len).sum();
        let stream_count = streams.len();
        for bytes in streams {
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
                let (Some(shape), Some(kind)) =
                    (u16_at(payload, SHAPE_AT), u16_at(payload, TAIL_KIND_AT))
                else {
                    continue;
                };
                t.total += 1;
                bump(&mut t.shape_kind, (shape, kind));
                if decode_igtextbox_at(&bytes, at).is_some() {
                    t.accepted += 1;
                }

                let Some(body) = body_of(payload, shape) else {
                    t.account_misses
                        .push(format!("shape {shape} is not one of 1/2/3"));
                    continue;
                };
                let tail_at = BODY_START + body.len;
                let Some(tail) = tail_len(payload, tail_at, kind) else {
                    t.account_misses
                        .push(format!("tail kind {kind} is not one of 1/2"));
                    continue;
                };

                // (1) the byte account.
                let stated = BODY_START + body.len + tail;
                if stated == payload.len() {
                    t.account_closes += 1;
                } else if t.account_misses.len() < 8 {
                    t.account_misses.push(format!(
                        "shape {shape} kind {kind}: payload {} but 22 + {} + {tail} = {stated}",
                        payload.len(),
                        body.len
                    ));
                }
                bump(&mut t.tail_lens, tail);
                bump(&mut t.extra_past_placement, tail - TAIL_KIND1_LEN);

                // (2) what the tail carries.
                if let Some(word) = u32_at(payload, tail_at + 32) {
                    bump(&mut t.trailer_dword, word);
                }
                if kind == 2 {
                    if let Some(flags) = u32_at(payload, tail_at + TAIL_FLAGS_AT) {
                        bump(&mut t.flag_words, flags);
                        bump(
                            &mut t.optional_counts,
                            (flags & TAIL_OPTIONAL_MASK).count_ones() as usize,
                        );
                    }
                    if let (Some(x), Some(y)) =
                        (f64_at(payload, tail_at + 40), f64_at(payload, tail_at + 48))
                    {
                        bump(&mut t.rect_a, tidy(x));
                        bump(&mut t.rect_b, tidy(y));
                    }
                    if let (Some(p), Some(q), Some(r)) = (
                        u32_at(payload, tail_at + 56),
                        u32_at(payload, tail_at + 60),
                        u32_at(payload, tail_at + 64),
                    ) {
                        bump(&mut t.tail_dwords, (p, q, r));
                    }
                }

                // (3) the runs.
                bump(&mut t.ab, (body.a, body.b));
                if shape_from_runs(body.a, body.b) == shape {
                    t.shape_rule_ok += 1;
                }
                let Some(runs) = runs_of(payload, &body) else {
                    continue;
                };
                if runs.len() > 1 {
                    t.multi_run_records += 1;
                    if t.run_samples.len() < 6 {
                        t.run_samples
                            .push((text_of(payload, body.text_at, body.count), runs.clone()));
                    }
                }
                let own_index = u32_at(payload, 14).unwrap_or(0);
                let mut sum_first_kind = 0usize;
                let mut order_ok = true;
                let own = styles.get(&own_index);
                bump(
                    &mut t.own_index_points_at,
                    own.map_or(0, |rec| rec.type_code),
                );
                // The shipped decoder resolves the character style in two hops:
                // record `+14` names a paragraph style, whose `+38` names the
                // character style. Shape 2's single run carries a character
                // style id outright, so the two can be compared -- first as
                // ids, then as the lettering they actually produce.
                let two_hop = own.and_then(|rec| rec.char_ref);
                if shape == 2 {
                    match (runs.first(), two_hop) {
                        (Some(run), Some(two_hop)) => {
                            if run.value == two_hop {
                                t.one_hop_agrees += 1;
                            } else {
                                t.one_hop_differs += 1;
                                let from_run =
                                    styles.get(&run.value).and_then(|r| r.lettering.as_ref());
                                let from_para =
                                    styles.get(&two_hop).and_then(|r| r.lettering.as_ref());
                                match (from_run, from_para) {
                                    (Some(a), Some(b)) if a == b => t.lettering_same += 1,
                                    (Some(a), Some(b)) => {
                                        t.lettering_differs += 1;
                                        if t.lettering_examples.len() < 6 {
                                            t.lettering_examples.push(format!(
                                                "run {:?} {:.4}mm #{:06X}  vs  para {:?} {:.4}mm #{:06X}",
                                                a.font,
                                                a.height_m * 1000.0,
                                                a.colour,
                                                b.font,
                                                b.height_m * 1000.0,
                                                b.colour
                                            ));
                                        }
                                    }
                                    _ => t.lettering_unresolved += 1,
                                }
                            }
                        }
                        _ => t.one_hop_unresolved += 1,
                    }
                }
                for (i, run) in runs.iter().enumerate() {
                    bump(&mut t.selectors, run.selector);
                    bump(&mut t.run_values, run.value);
                    t.run_value_total += 1;
                    bump(
                        &mut t.run_points_at,
                        (
                            run.selector,
                            styles.get(&run.value).map_or(0, |rec| rec.type_code),
                        ),
                    );
                    if run.value == own_index {
                        bump(&mut t.run_equals_own_index, run.selector);
                        t.run_value_is_own_index += 1;
                    }
                    // The Save side writes all selector-1 runs, then all
                    // selector-2 runs, so position and selector must agree.
                    let want = if i < body.a as usize { 1 } else { 2 };
                    if run.selector != want {
                        order_ok = false;
                    }
                    if run.selector == 1 {
                        sum_first_kind += run.length as usize;
                    }
                }
                if order_ok {
                    t.selector_order_ok += 1;
                }
                if body.a > 0 {
                    t.run_lengths_sum_checked += 1;
                    if sum_first_kind == body.count as usize {
                        t.run_lengths_sum_ok += 1;
                    }
                }
            }
        }
        t.per_fixture
            .push((fixture, t.total - before, stream_count, stream_bytes));
    }

    report(&t);
}

fn report(t: &Tally) {
    println!("=== igTextBox: native read order, byte account, formatting runs ===");
    println!(
        "chain records: {}   (decoder accepts {})",
        t.total, t.accepted
    );
    for (fixture, n, streams, bytes) in &t.per_fixture {
        println!("  {n:>4} records  {streams:>2} sheet streams  {bytes:>8} bytes  {fixture}");
    }
    for (fixture, why) in &t.unreadable {
        println!("  UNREAD                                        {fixture}: {why}");
    }
    if !t.unreadable.is_empty() {
        println!(
            "\n  {} fixture(s) could not be opened, so the counts below are short\n\
             by whatever they hold. Do not quote them as corpus-wide.",
            t.unreadable.len()
        );
    }
    println!();

    println!("--- shape (+18) x tail kind (+20) ---");
    println!("{:>6} {:>10} {:>10}", "shape", "tail kind", "records");
    for ((shape, kind), n) in &t.shape_kind {
        println!("{shape:>6} {kind:>10} {n:>10}");
    }

    println!("\n--- byte account: payload == 22 + body_len + tail_len ---");
    println!("closes exactly: {}/{}", t.account_closes, t.total);
    for miss in &t.account_misses {
        println!("  MISS {miss}");
    }
    if t.account_misses.is_empty() && t.account_closes == t.total {
        println!("  no slack anywhere: every payload byte is spoken for.");
    }

    println!("\n--- tail length (native GetSize) ---");
    for (len, n) in &t.tail_lens {
        println!("  {len:>4} bytes : {n}");
    }
    println!("  bytes past the 36-byte placement block:");
    for (extra, n) in &t.extra_past_placement {
        println!("    +{extra:<3} : {n}");
    }

    println!("\n--- kind-2 flag word (tail +36) ---");
    for (flags, n) in &t.flag_words {
        let bits: Vec<String> = TAIL_OPTIONAL_BITS
            .iter()
            .filter(|bit| flags & *bit != 0)
            .map(|bit| format!("{bit:#010x}"))
            .collect();
        let shown = if bits.is_empty() {
            "no optional field".to_owned()
        } else {
            bits.join(" ")
        };
        println!("  {flags:#010x} : {n:>3}   optional: {shown}");
    }
    println!("  optional fields per record:");
    for (count, n) in &t.optional_counts {
        println!("    {count} : {n}");
    }

    println!("\n--- kind-2 rectangle doubles (tail +40 / +48) ---");
    println!("  +40: {:?}", t.rect_a);
    println!("  +48: {:?}", t.rect_b);
    println!("  dwords at tail +56/+60/+64:");
    for ((p, q, r), n) in &t.tail_dwords {
        println!("    ({p:#010x}, {q:#010x}, {r:#010x}) : {n}");
    }

    println!("\n--- dword at tail +32 (present in both kinds) ---");
    for (word, n) in t.trailer_dword.iter().take(12) {
        println!("  {word:#010x} : {n}");
    }
    if t.trailer_dword.len() > 12 {
        println!("  ... {} distinct values in all", t.trailer_dword.len());
    }

    println!("\n--- formatting runs ---");
    println!("  (A, B) run counts:");
    for ((a, b), n) in &t.ab {
        println!("    A={a} B={b} : {n}");
    }
    println!(
        "  shape word reproduced from (A, B): {}/{}",
        t.shape_rule_ok, t.total
    );
    println!("  selectors seen: {:?}", t.selectors);
    println!(
        "  selector matches position (all A then all B): {}/{}",
        t.selector_order_ok, t.total
    );
    println!(
        "  selector-1 lengths sum to the character count: {}/{}",
        t.run_lengths_sum_ok, t.run_lengths_sum_checked
    );
    println!(
        "  records carrying more than one run: {}",
        t.multi_run_records
    );
    println!(
        "  run values equal to the record's own index (+14): {}/{}",
        t.run_value_is_own_index, t.run_value_total
    );
    println!("  distinct run values: {}", t.run_values.len());
    for (value, n) in t.run_values.iter().take(12) {
        println!("    {value:#010x} : {n}");
    }

    println!("\n--- what the run value names, per selector ---");
    for ((selector, type_code), n) in &t.run_points_at {
        println!(
            "  selector {selector} -> {:<24} : {n}",
            psm_name(*type_code)
        );
    }
    println!(
        "  runs whose value equals the record's own index: {:?}",
        t.run_equals_own_index
    );
    println!("  the record's own index (+14) names:");
    for (type_code, n) in &t.own_index_points_at {
        println!("    {:<28} : {n}", psm_name(*type_code));
    }
    println!(
        "  shape 2: run value vs the decoder's two-hop char style -- \
         agree {}, differ {}, unresolved {}",
        t.one_hop_agrees, t.one_hop_differs, t.one_hop_unresolved
    );
    println!(
        "    of those that differ, the lettering itself -- same {}, differs {}, unresolved {}",
        t.lettering_same, t.lettering_differs, t.lettering_unresolved
    );
    for example in &t.lettering_examples {
        println!("      {example}");
    }

    println!("\n--- multi-run labels, run by run ---");
    for (text, runs) in &t.run_samples {
        println!("  {text:?}  ({} chars)", text.chars().count());
        for run in runs {
            println!(
                "    len {:>3}  selector {}  value {:#010x}",
                run.length, run.selector, run.value
            );
        }
    }

    println!("\n=== verdict ===");
    if !t.unreadable.is_empty() {
        println!(
            "PARTIAL: {} fixture(s) were unreadable this run. Re-run once they\n\
             are free before quoting any number here.",
            t.unreadable.len()
        );
    }
    if t.account_closes == t.total && t.total > 0 {
        println!(
            "The read order is the layout. `22 + body_len + tail_len` reproduces\n\
             every payload length exactly, so there is no residue left to explain:\n\
             tail kind 2's \"extra 32 or 40 bytes\" are the fixed part of a longer\n\
             placement record (68 bytes) plus one optional 8-byte field, and\n\
             shape 3's A/B entries are formatting runs, not doubles."
        );
    } else {
        println!(
            "The account does not close on every record. The formula above is\n\
             therefore not yet the whole layout -- see the MISS lines."
        );
    }
}
