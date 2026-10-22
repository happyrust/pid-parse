//! Which `igTextBox` shape are the `.sym` text records the library refuses?
//!
//! `symbol_library::decode_text` finds a text record's character count by
//! arithmetic: it tries `+30` and `+22` and believes whichever leaves exactly
//! 36 bytes behind the characters. That rule describes the drawing side's
//! sub-types 1 and 2 only. Sub-type 3 carries its count at `+26` and puts
//! `A + B` formatting runs — 8-byte `(len, selector, style_id)` entries, `A`
//! selector-1 then `B` selector-2 — between the text and the 36-byte tail, so
//! on those records both offsets miss and the exact-fit test can never pass.
//!
//! This probe decides the question from the bytes: every `0x004D` record in
//! every `Sheet*` stream of the library is classified by the discriminator at
//! `+18` and checked against both rules, and every record the library refuses
//! is printed with its shape, its recovered text and the raw words around the
//! count.
//!
//! Cross-checked against the drawing side's `igtextbox_body_shape`
//! (`parsers::sheet_records`, native-reader evidence in
//! `docs/analysis/2026-08-13-igtextbox-has-three-shapes.md`) — the table the
//! library reader now shares, so the shipped `Text` count below comes from
//! `read_symbol_geometry` itself.
//!
//! The rule that shipped before the fix is kept as the "before" column
//! ([`retired_fit`]): it is what makes the 606-record gap measurable.
//!
//! It also reads the tail of every record a shape reads: whether its direction
//! pair is a direction by the drawing side's rule (the unit vector, or a pair
//! scaled to a letter height), the check the shipped reader refuses on, and
//! the tail-kind word at `+20` against how many bytes follow the 36-byte tail,
//! with the flag word and text box of every kind-2 tail.
//!
//! ```text
//! cargo run --example probe_sym_text_shape_gap -- [--root DIR]... [--all]
//! ```
//!
//! Read-only.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::symbol_library::{read_symbol_geometry, SymbolPrimitive};
use pid_parse::PidParser;

const CLUSTER_MAGIC: u32 = 0x6C90_F544;
const CHAIN_START: usize = 8;
const PSM_ENVELOPE_LEN: usize = 6;
const TYPE_TEXTBOX: u16 = 0x004D;

/// Payload bytes before the text body: oid, parent, a `u32`, the sub-type
/// word, and an index.
const BODY_START: usize = 22;
/// Bytes of placement/style that follow every body.
const TAIL_LEN: usize = 36;
/// Where the library looks for the character count, most common first.
const LIBRARY_COUNT_OFFSETS: [usize; 2] = [30, 22];
/// `igTextBox`'s own shape discriminator.
const TEXT_SHAPE_AT: usize = 18;
/// Payload offset of the tail-kind word: `1` for the placement tail alone,
/// `2` for the tail that then carries a text box.
const TAIL_KIND_AT: usize = 20;
/// Bytes a kind-2 tail occupies before the fields its flag word gates.
const KIND2_TAIL_LEN: usize = 68;
/// How far a unit direction pair may stray from length 1.
const DIRECTION_UNIT_TOLERANCE: f64 = 1e-6;
/// The lengths a pair scaled to a letter height takes, in metres.
const LETTER_HEIGHT_M: std::ops::RangeInclusive<f64> = 0.0005..=0.25;

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

/// The library copies this machine holds: the corpus's reference share first,
/// then the copy checked in under `test-file/`.
fn library_roots(corpus: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let ref_dir = corpus.join("Ref");
    if ref_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&ref_dir) {
            let mut found: Vec<PathBuf> = entries
                .flatten()
                .filter(|entry| {
                    entry.path().is_dir()
                        && entry
                            .file_name()
                            .to_string_lossy()
                            .to_ascii_lowercase()
                            .contains("symbol")
                })
                .map(|entry| entry.path())
                .collect();
            found.sort();
            roots.extend(found);
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-file/symbols-full");
    if bundled.is_dir() {
        roots.push(bundled);
    }
    roots
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

/// Where a text record puts its characters, per the drawing side's
/// three-shape rule: `(shape, count, text_start, body_len)`.
fn drawn_shape(payload: &[u8]) -> Option<(u16, u16, usize, usize)> {
    let shape = u16_le(payload, TEXT_SHAPE_AT)?;
    match shape {
        1 => {
            let count = u16_le(payload, 22)?;
            Some((1, count, 24, 2 + 2 * count as usize))
        }
        2 => {
            let packed = u32_le(payload, 22)?;
            let count = u16_le(payload, 30)?;
            (packed == (count as u32 | 0x1_0000)).then_some((2, count, 32, 10 + 2 * count as usize))
        }
        3 => {
            let a = u16_le(payload, 22)? as usize;
            let b = u16_le(payload, 24)? as usize;
            let count = u16_le(payload, 26)?;
            Some((3, count, 28, 6 + 2 * count as usize + 8 * (a + b)))
        }
        _ => None,
    }
}

/// Whether the **retired** arithmetic accepts the payload, and at which
/// offset. Mirrors the `decode_text` of `HEAD` before this reading: a
/// non-zero count whose characters plus exactly 36 trailing bytes account
/// for the payload. Kept as the "before" column — the shipped reader now asks
/// the shape table, and the `after` numbers come from
/// [`read_symbol_geometry`].
#[allow(dead_code)]
fn retired_fit(payload: &[u8]) -> Option<usize> {
    LIBRARY_COUNT_OFFSETS.iter().copied().find(|&count_at| {
        let Some(count) = u16_le(payload, count_at) else {
            return false;
        };
        let count = count as usize;
        if count == 0 || count > 1024 {
            return false;
        }
        let end = count_at + 2 + 2 * count;
        payload.len() == end + TAIL_LEN
    })
}

fn read_text(payload: &[u8], start: usize, count: usize) -> Option<String> {
    let mut chars = Vec::with_capacity(count);
    for index in 0..count {
        chars.push(u16_le(payload, start + index * 2)?);
    }
    let text = String::from_utf16(&chars).ok()?;
    (!text.contains('\0')).then_some(text)
}

/// A finite number the text tail could hold: an insertion point or a width.
fn number(payload: &[u8], at: usize) -> Option<f64> {
    let value = f64_le(payload, at)?;
    value.is_finite().then_some(value)
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Default)]
struct Tally {
    records: usize,
    /// Shape discriminator histogram.
    shapes: BTreeMap<u16, usize>,
    /// Shape x library verdict.
    cross: BTreeMap<(u16, bool), usize>,
    /// Retired-rule verdict for records no shape explains.
    unknown_shape_library: BTreeMap<bool, usize>,
    /// Text primitives the shipped reader yields for the whole library, and
    /// the files where it yields fewer than the file's record count.
    shipped_text_primitives: usize,
    files_short: Vec<String>,
    /// Files that still letter nothing with the shipped reader.
    still_silent: Vec<String>,
    /// Records the library refuses but a shape reads.
    rescued: usize,
    /// Records the library refuses, per shape discriminator.
    refused_by_shape: BTreeMap<u16, usize>,
    /// Refused records, by shape and whether the shape's tail ends exactly at
    /// the payload end (`exact`), short of it (`extra` trailing bytes), or past
    /// it (`over`).
    refused_tail: BTreeMap<(u16, &'static str), usize>,
    /// Refused records whose shape states an empty text.
    refused_empty: usize,
    /// Refused records, by how many bytes follow the shape's 36-byte tail.
    extra_bytes: BTreeMap<(u16, usize), usize>,
    /// Refused records whose tail start reads as a metre-scale sheet point
    /// (`|x|, |y| <= 1`), and the points that recur.
    tail_points_sane: usize,
    tail_points: BTreeMap<(i64, i64), usize>,
    /// Refused records whose recovered text is a readable label rather than
    /// reinterpreted binary.
    rescued_plausible: usize,
    rescued_unreadable: usize,
    /// Formatting runs between the text end and the tail start on shape 3.
    gap_runs: BTreeMap<usize, usize>,
    rejects: Vec<String>,
    rescued_texts: BTreeMap<String, usize>,
    /// Files with at least one refused record, and of those the ones whose
    /// whole text yield is refused — the bodies that letter nothing today.
    files_with_refusals: usize,
    text_invisible: Vec<String>,
    /// Every record a shape reads, by how its direction pair classifies.
    direction: BTreeMap<&'static str, usize>,
    /// Records whose pair is neither a unit vector nor a letter height.
    off_direction: Vec<String>,
    /// Every record a shape reads, by the tail-kind word and the bytes after
    /// the 36-byte tail.
    tail_kind: BTreeMap<(u16, usize), usize>,
    /// Kind-2 tails: the flag word at tail `+36` against the bytes past the
    /// 68-byte kind-2 tail.
    kind2_flags: BTreeMap<(u32, usize), usize>,
    /// Kind-2 tails whose box (tail `+40` / `+48`) reads as a size, and not.
    kind2_box_usable: usize,
    kind2_box_unusable: usize,
}

fn main() {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut explicit = false;
    let mut show_all = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                if let Some(dir) = args.next() {
                    roots.push(PathBuf::from(dir));
                    explicit = true;
                }
            }
            "--all" => show_all = true,
            other => {
                roots.push(PathBuf::from(other));
                explicit = true;
            }
        }
    }
    if !explicit {
        roots = library_roots(&corpus_dir());
    }
    if roots.is_empty() {
        println!("no symbol library on this machine; nothing to measure");
        return;
    }

    let mut syms = Vec::new();
    for root in &roots {
        collect_syms(root, &mut syms);
    }
    syms.sort();
    syms.dedup();
    println!("{} .sym files under {} root(s)", syms.len(), roots.len());

    let mut tally = Tally::default();
    for sym in &syms {
        let Ok(pkg) = PidParser::new().parse_package(sym) else {
            continue;
        };
        let name = sym
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let mut accepted_in_file = 0usize;
        let mut refused_in_file = 0usize;
        for (path, raw) in &pkg.streams {
            let Some(stream_name) = path.rsplit('/').next() else {
                continue;
            };
            if !stream_name.starts_with("Sheet") {
                continue;
            }
            let bytes = &raw.data;
            if bytes.len() <= CHAIN_START || u32_le(bytes, 0) != Some(CLUSTER_MAGIC) {
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
                let Some(end) = body.checked_add(btf as usize) else {
                    break;
                };
                if end > bytes.len() {
                    break;
                }
                if raw_type & 0x0FFF == TYPE_TEXTBOX {
                    let payload = &bytes[body..end];
                    let reads = retired_fit(payload).is_some();
                    if reads {
                        accepted_in_file += 1;
                    } else {
                        refused_in_file += 1;
                    }
                    examine(&mut tally, &name, stream_name, payload, show_all);
                    examine_tail(&mut tally, &name, stream_name, payload);
                }
                at = end;
            }
        }
        if refused_in_file > 0 {
            tally.files_with_refusals += 1;
            if accepted_in_file == 0 {
                tally.text_invisible.push(name.clone());
            }
        }
        if let Ok(body) = read_symbol_geometry(sym) {
            let shipped = body
                .primitives
                .iter()
                .filter(|styled| matches!(styled.primitive, SymbolPrimitive::Text { .. }))
                .count();
            tally.shipped_text_primitives += shipped;
            if shipped == 0 && accepted_in_file + refused_in_file > 0 {
                tally.still_silent.push(name.clone());
            }
            if shipped < accepted_in_file + refused_in_file {
                tally.files_short.push(format!(
                    "{name}: {shipped} of {}",
                    accepted_in_file + refused_in_file
                ));
            }
        }
    }

    report(&tally, show_all);
}

fn examine(tally: &mut Tally, name: &str, stream: &str, payload: &[u8], show_all: bool) {
    tally.records += 1;
    let shape = u16_le(payload, TEXT_SHAPE_AT).unwrap_or(u16::MAX);
    *tally.shapes.entry(shape).or_default() += 1;
    let library = retired_fit(payload).is_some();
    *tally.cross.entry((shape, library)).or_default() += 1;

    let drawn = drawn_shape(payload);
    if drawn.is_none() {
        *tally.unknown_shape_library.entry(library).or_default() += 1;
    }

    // What a shape-aware reader would recover: the tail must start where the
    // body says and the text must lie entirely inside the body.
    let rescued;
    if let Some((shape, count, text_start, body_len)) = drawn {
        let tail_start = BODY_START + body_len;
        let fits =
            tail_start + TAIL_LEN <= payload.len() && text_start + 2 * count as usize <= tail_start;
        if fits {
            let text = read_text(payload, text_start, count as usize);
            if let Some(text) = text {
                if !library {
                    let point = (number(payload, tail_start), number(payload, tail_start + 8));
                    if let (Some(x), Some(y)) = point {
                        if x.abs() <= 1.0 && y.abs() <= 1.0 {
                            tally.tail_points_sane += 1;
                        }
                        let quantised = ((x * 1e5).round() as i64, (y * 1e5).round() as i64);
                        *tally.tail_points.entry(quantised).or_default() += 1;
                    }
                    tally.rescued += 1;
                    if text.chars().all(|c| !c.is_control() || c == '\r') {
                        tally.rescued_plausible += 1;
                    } else {
                        tally.rescued_unreadable += 1;
                    }
                    *tally.rescued_texts.entry(text.clone()).or_default() += 1;
                }
                if shape == 3 {
                    let gap = (tail_start - (text_start + 2 * count as usize)) / 8;
                    *tally.gap_runs.entry(gap).or_default() += 1;
                }
                let text_end = text_start + 2 * count as usize;
                rescued = format!(
                    "shape={shape} count={count} text={text:?} tail@{tail_start} \
                     ({:+} B) at_text_end={:?}/{:?} at_tail={:?}/{:?}",
                    payload.len() as i64 - (tail_start + TAIL_LEN) as i64,
                    number(payload, text_end),
                    number(payload, text_end + 8),
                    number(payload, tail_start),
                    number(payload, tail_start + 8),
                );
            } else {
                rescued = format!("shape={shape} count={count} text=<unreadable UTF-16>");
            }
            if !library {
                *tally.refused_by_shape.entry(shape).or_default() += 1;
                let tail_end = tail_start + TAIL_LEN;
                let where_tail = match tail_end.cmp(&payload.len()) {
                    std::cmp::Ordering::Equal => "exact",
                    std::cmp::Ordering::Less => "extra",
                    std::cmp::Ordering::Greater => "over",
                };
                *tally.refused_tail.entry((shape, where_tail)).or_default() += 1;
                if count == 0 {
                    tally.refused_empty += 1;
                }
                if tail_end < payload.len() {
                    *tally
                        .extra_bytes
                        .entry((shape, payload.len() - tail_end))
                        .or_default() += 1;
                }
            }
        } else {
            rescued = format!(
                "shape={shape} count={count} body={body_len} but tail would start at \
                 {tail_start} of {}",
                payload.len()
            );
        }
    } else {
        rescued = format!("no shape explains discriminator {shape}");
    }

    if !library || show_all {
        tally.rejects.push(format!(
            "{name} [{stream}] payload={} library_fit={} {rescued}\n      \
             shape@18={shape} words@18..30={:?}\n      bytes@18..={}",
            payload.len(),
            retired_fit(payload).map_or_else(|| "-".to_owned(), |at| at.to_string()),
            (0..6)
                .map(|i| u16_le(payload, 18 + i * 2))
                .collect::<Vec<_>>(),
            hex(&payload[18.min(payload.len())..payload.len().min(66)]),
        ));
    }
}

/// A text box side a kind-2 tail can hold: finite, positive, sheet-sized.
fn usable_size(payload: &[u8], at: usize) -> bool {
    f64_le(payload, at).is_some_and(|value| value.is_finite() && value > 0.0 && value <= 1.0e9)
}

/// The tail of a record a shape reads: its direction pair under the drawing
/// side's rule, and its tail kind against what follows the 36-byte tail.
fn examine_tail(tally: &mut Tally, name: &str, stream: &str, payload: &[u8]) {
    let Some((shape, count, _, body_len)) = drawn_shape(payload) else {
        return;
    };
    let tail_start = BODY_START + body_len;
    if count == 0 || tail_start + TAIL_LEN > payload.len() {
        return;
    }
    let extra = payload.len() - (tail_start + TAIL_LEN);
    let kind = u16_le(payload, TAIL_KIND_AT).unwrap_or(u16::MAX);
    *tally.tail_kind.entry((kind, extra)).or_default() += 1;

    let cos = f64_le(payload, tail_start + 16).unwrap_or(f64::NAN);
    let sin = f64_le(payload, tail_start + 24).unwrap_or(f64::NAN);
    let length = cos.hypot(sin);
    let class = if (length - 1.0).abs() <= DIRECTION_UNIT_TOLERANCE {
        "unit"
    } else if LETTER_HEIGHT_M.contains(&length) {
        "letter height"
    } else {
        "neither"
    };
    *tally.direction.entry(class).or_default() += 1;
    if class == "neither" {
        tally.off_direction.push(format!(
            "{name} [{stream}] shape {shape} count {count} pair ({cos:e}, {sin:e}) +{extra} B"
        ));
    }

    let kind2_extra = KIND2_TAIL_LEN - TAIL_LEN;
    if kind == 2 && extra >= kind2_extra {
        let flags = u32_le(payload, tail_start + TAIL_LEN).unwrap_or(u32::MAX);
        let past_box = extra - kind2_extra;
        *tally.kind2_flags.entry((flags, past_box)).or_default() += 1;
        if usable_size(payload, tail_start + 40) && usable_size(payload, tail_start + 48) {
            tally.kind2_box_usable += 1;
        } else {
            tally.kind2_box_unusable += 1;
        }
    }
}

fn report(tally: &Tally, show_all: bool) {
    println!("\n== 0x004D records on the library's Sheet* streams ==");
    println!("  total {}", tally.records);

    println!("\n== discriminator at +18 ==");
    for (shape, count) in &tally.shapes {
        println!("  {shape:>6} x{count}");
    }

    println!("\n== shape x whether the retired arithmetic accepts it ==");
    for ((shape, library), count) in &tally.cross {
        println!(
            "  shape {shape:>6}  library {} x{count}",
            if *library { "reads " } else { "refuses" }
        );
    }
    if !tally.unknown_shape_library.is_empty() {
        let unknown = &tally.unknown_shape_library;
        println!("  (discriminator no shape names) {unknown:?}");
    }

    println!("\n== records the library refuses but a shape reads ==");
    println!("  rescued {}", tally.rescued);
    println!(
        "  readable text {}  binary-looking {}",
        tally.rescued_plausible, tally.rescued_unreadable
    );
    println!("  refused, per shape: {:?}", tally.refused_by_shape);
    println!("  refused, by shape and where the shape's tail ends:");
    for (key, count) in &tally.refused_tail {
        println!("    shape {} tail {:<5} x{count}", key.0, key.1);
    }
    println!("  refused, shape says empty text: {}", tally.refused_empty);
    println!(
        "  refused, tail start reads a metre-scale point: {}",
        tally.tail_points_sane
    );
    let mut points: Vec<(&(i64, i64), &usize)> = tally.tail_points.iter().collect();
    points.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    println!(
        "  most common points (x mm, y mm): {}",
        points
            .iter()
            .take(8)
            .map(|((x, y), count)| format!(
                "({:.1}, {:.1}) x{count}",
                *x as f64 / 100.0,
                *y as f64 / 100.0
            ))
            .collect::<Vec<_>>()
            .join("  ")
    );
    let mut extra: Vec<(&(u16, usize), &usize)> = tally.extra_bytes.iter().collect();
    extra.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    println!(
        "  bytes after the tail, top: {}",
        extra
            .iter()
            .take(12)
            .map(|((shape, bytes), count)| format!("shape{shape}+{bytes}B x{count}"))
            .collect::<Vec<_>>()
            .join("  ")
    );
    let mut texts: Vec<(&String, &usize)> = tally.rescued_texts.iter().collect();
    texts.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
    for (text, count) in texts.iter().take(30) {
        println!("  {count:>3}x {text:?}");
    }

    println!("\n== the shipped reader (\u{96c6}\u{62d0}\u{65e7}\u{7b97}\u{672f}\u{7684}\u{53e6}\u{4e00}\u{4fa7}) ==");
    println!(
        "  text primitives over the whole library {}",
        tally.shipped_text_primitives
    );
    println!(
        "  files yielding fewer than their record count {}",
        tally.files_short.len()
    );
    for line in tally.files_short.iter().take(12) {
        println!("  {line}");
    }
    println!(
        "  files that still letter nothing: {}",
        tally.still_silent.len()
    );
    for name in tally.still_silent.iter().take(12) {
        println!("  {name}");
    }

    println!("\n== files that letter nothing under the retired rule ==");
    println!(
        "  files with a refused record {}; of those, all text refused {}",
        tally.files_with_refusals,
        tally.text_invisible.len()
    );
    let mut names = tally.text_invisible.clone();
    names.sort();
    names.dedup();
    for name in names.iter().take(40) {
        println!("  {name}");
    }
    if names.len() > 40 {
        println!("  ... {} more", names.len() - 40);
    }

    println!("\n== shape 3: formatting runs between the text end and the tail start ==");
    for (runs, count) in &tally.gap_runs {
        println!("  {runs} run(s) x{count}");
    }

    println!("\n== the tail of every record a shape reads ==");
    println!("  direction pair under the drawing side's rule:");
    for (class, count) in &tally.direction {
        println!("    {class:<13} x{count}");
    }
    for line in tally.off_direction.iter().take(40) {
        println!("    {line}");
    }
    println!("  tail kind at +20 x bytes after the 36-byte tail:");
    for ((kind, extra), count) in &tally.tail_kind {
        println!("    kind {kind:>5} +{extra:>3} B x{count}");
    }
    println!("  kind 2: flag word at tail +36 x bytes past the 68-byte tail:");
    for ((flags, past), count) in &tally.kind2_flags {
        println!("    flags {flags:#010x} +{past:>3} B x{count}");
    }
    println!(
        "  kind 2: box usable {}  not usable {}",
        tally.kind2_box_usable, tally.kind2_box_unusable
    );

    println!(
        "\n== records the library's arithmetic does not explain{} ==",
        if show_all {
            " (and every record, --all)"
        } else {
            ""
        }
    );
    for line in &tally.rejects {
        println!("  {line}");
    }
}
