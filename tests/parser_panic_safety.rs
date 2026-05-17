//! Panic-safety smoke tests for every byte-level parser entry point.
//!
//! Each `pub fn parse_*` / `probe_*` / `scan_*` in [`pid_parse::parsers`]
//! that takes raw bytes (`&[u8]`) is contractually required to handle
//! arbitrary input without panicking — malformed bytes, truncated
//! buffers, integer-overflow bait, invalid UTF-8 sequences, etc. must
//! all surface as `None` / empty / error variants instead of unwinding.
//!
//! This test feeds a curated adversarial corpus to every entry point
//! and lets the test runner catch any panic. The corpus is tiny on
//! purpose: enumerating short lengths around the parsers' header
//! sizes, all-zero / all-`0xFF` fillers, byte-cycle patterns, a few
//! "magic-bait" payloads (`{`, `<`, `Relationship.`, `P&IDAttributes`,
//! `SmartPlantPID.a`), a previously-panic-prone UTF-8 lossy fixture,
//! and three deterministic xorshift32 streams for coverage of weird
//! mid-stream bit patterns.
//!
//! When this test fails, the panic message points at the offending
//! parser and input shape — fix the parser, then add a focused
//! regression test in the parser's own `#[cfg(test)] mod tests` so
//! the bug stays nailed down even if this corpus moves on.

use std::collections::HashSet;

use pid_parse::parsers::app_object::parse_app_object;
use pid_parse::parsers::cluster_header::{parse_header, parse_string_table};
use pid_parse::parsers::doc_version::parse_doc_version3;
use pid_parse::parsers::doc_version2::parse_doc_version2;
use pid_parse::parsers::dynamic_attr_records::{extract_record_trailers, parse_attribute_records};
use pid_parse::parsers::jproperties::parse_jproperties;
use pid_parse::parsers::psm_tables::{
    parse_psm_cluster_table, parse_psm_roots, parse_psm_segment_table,
};
use pid_parse::parsers::relationship_probe::probe_relationships;
use pid_parse::parsers::sheet_endpoint_records::parse_endpoint_records;
use pid_parse::parsers::sheet_probe::{probe_sheet_stream, SheetProbeOptions};
use pid_parse::parsers::sheet_records::{
    decode_graphic_group_at, decode_graphic_groups, decode_igline_at, decode_iglines,
    decode_iglinestring_at, decode_iglinestrings, decode_igpoint_at, decode_igpoints,
    decode_igsymbol_at, decode_igsymbols, decode_igtextbox_at, decode_igtextboxes,
    decode_jstyle_override_at, decode_jstyle_overrides, decode_primitive_line_at,
    decode_primitive_lines, decode_sub_record_0x0010_at, decode_sub_records_0x0010,
};
use pid_parse::parsers::string_scan::{scan_ascii_strings, scan_guids, scan_utf16le_strings};
use pid_parse::parsers::tagged_stg_list::parse_tagged_stg_list;

/// Build the adversarial input corpus shared by every parser below.
///
/// Categories (kept small so the smoke test stays fast):
/// 1. Empty + a sweep of short lengths around the parsers' header
///    sizes (4, 8, 16, 32, …) filled with `0x00` / `0xFF` / a byte
///    cycle, to exercise `pos + N <= data.len()` boundary conditions.
/// 2. ASCII / binary "bait" payloads that hit the parsers' magic
///    sniffers without satisfying the rest of the layout — e.g. a
///    bare `{`, an invalid GUID, a truncated `Relationship.<hex>`,
///    a `P&IDAttributes` substring, a stray `SmartPlantPID.a`
///    record fragment, and the `0xFF '{' AAAA…AAAA 0xFF` UTF-8
///    lossy fixture that previously panicked
///    [`scan_guids`](pid_parse::parsers::string_scan::scan_guids).
/// 3. Three xorshift32 pseudo-random streams (`256`, `1024`,
///    `4096` bytes) seeded with `0xDEAD_BEEF`, `0xCAFE_BABE`,
///    `0x1234_5678` so the corpus is deterministic across runs.
fn adversarial_inputs() -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();

    out.push(Vec::new());

    let lengths: [usize; 18] = [
        1, 2, 3, 4, 7, 8, 15, 16, 31, 32, 63, 64, 127, 128, 255, 256, 1024, 4096,
    ];
    for &len in &lengths {
        out.push(vec![0u8; len]);
        out.push(vec![0xFFu8; len]);
        out.push((0..len).map(|i| (i & 0xFF) as u8).collect());
    }

    out.push(b"<".to_vec());
    out.push(b"<<<>>>".to_vec());
    out.push(b"<Tag></Tag>".to_vec());
    out.push(b"<Tag>value".to_vec()); // unterminated
    out.push(b"{".to_vec());
    out.push(b"{ABCDEFGH-1234-5678-9ABC-DEF012345678}".to_vec());
    out.push(b"{0F7B8ABD-0C4E-493F-A3C7-F06FD03AD6AA}".to_vec());

    // UTF-8-lossy fixture that previously panicked `scan_guids` with
    // `end byte index 41 is not a char boundary`.
    let mut bait = Vec::with_capacity(40);
    bait.push(0xFF);
    bait.push(b'{');
    bait.extend_from_slice(&[b'A'; 36]);
    bait.push(0xFF);
    out.push(bait);

    out.push(b"Relationship.".to_vec()); // bare tag, no GUID
    out.push(b"Relationship.NOT_HEX_GUID_PADDING_PADDING".to_vec());
    let mut rel_ok = b"Relationship.".to_vec();
    rel_ok.extend_from_slice(b"0F7B8ABD0C4E493FA3C7F06FD03AD6AA");
    out.push(rel_ok);

    out.push(b"P&IDAttributes".to_vec());
    out.push(b"DrawingID\0BADBADBADBADBADBADBADBADBADBADBA".to_vec());

    out.push(b"SmartPlantPID.a".to_vec());

    out
}

/// Append three deterministic xorshift32 pseudo-random byte buffers
/// to the corpus. xorshift32 is good enough for "bytes look random
/// across the whole buffer" without pulling in a `rand` dependency.
fn push_xorshift_corpus(out: &mut Vec<Vec<u8>>) {
    for &(seed, len) in &[
        (0xDEAD_BEEFu32, 256usize),
        (0xCAFE_BABE, 1024),
        (0x1234_5678, 4096),
    ] {
        let mut state = seed;
        let mut buf = Vec::with_capacity(len);
        for _ in 0..len {
            // xorshift32 from Marsaglia's 2003 paper.
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            buf.push((state & 0xFF) as u8);
        }
        out.push(buf);
    }
}

/// Maximum prefix length swept by [`parsers_do_not_panic_on_truncated_inputs`].
///
/// Truncations cover the byte ranges every parser actually checks
/// (header magic, length prefixes, per-record headers — all under
/// 64 bytes for the formats this crate handles). Capping here
/// keeps the second test linear in corpus size instead of
/// O(corpus * max_len).
const TRUNCATION_SWEEP_LIMIT: usize = 64;

/// Run every parser entry point against `input` exactly once.
///
/// Shared by the two `#[test]`s below so the corpus walk and the
/// truncation walk stay in lock-step — adding a new entry point
/// here automatically guards both axes.
fn exercise_all_parsers(input: &[u8]) {
    let opts = SheetProbeOptions::default();
    let empty_rels: HashSet<u32> = HashSet::new();

    let _ = parse_header(input);
    let _ = parse_string_table(input, 0);
    let _ = parse_string_table(input, input.len().saturating_sub(1));
    let _ = parse_string_table(input, input.len());

    let _ = parse_doc_version3(input);
    let _ = parse_doc_version2(input);

    let _ = parse_attribute_records(input);
    let _ = extract_record_trailers(input);

    let _ = parse_jproperties(input);

    let _ = parse_psm_roots(input);
    let _ = parse_psm_cluster_table(input);
    let _ = parse_psm_segment_table(input);

    let _ = probe_relationships(input);

    let _ = parse_endpoint_records("/SheetX", input, &empty_rels);
    let _ = probe_sheet_stream("SheetX", "/SheetX", input, &opts);

    let _ = scan_ascii_strings(input, 16);
    let _ = scan_guids(input, 16);
    let _ = scan_utf16le_strings(input, 4, 16);

    let _ = parse_tagged_stg_list(input);

    let _ = parse_app_object(input);

    // Phase 14 Slice D: PSM `GLine2d` PrimitiveLine decoder.
    let _ = decode_primitive_lines(input);
    let _ = decode_primitive_line_at(input, 0);
    if !input.is_empty() {
        let _ = decode_primitive_line_at(input, input.len() - 1);
        let _ = decode_primitive_line_at(input, input.len());
    }

    // Phase 14 Slice J: PSM `igLine2d` decoder.
    let _ = decode_iglines(input);
    let _ = decode_igline_at(input, 0);
    if !input.is_empty() {
        let _ = decode_igline_at(input, input.len() - 1);
        let _ = decode_igline_at(input, input.len());
    }

    // Phase 14 Slice K: PSM `igLineString2d` polyline decoder.
    let _ = decode_iglinestrings(input);
    let _ = decode_iglinestring_at(input, 0);
    if !input.is_empty() {
        let _ = decode_iglinestring_at(input, input.len() - 1);
        let _ = decode_iglinestring_at(input, input.len());
    }

    // Phase 14 Slice L: PSM `igPoint2d` decoder.
    let _ = decode_igpoints(input);
    let _ = decode_igpoint_at(input, 0);
    if !input.is_empty() {
        let _ = decode_igpoint_at(input, input.len() - 1);
        let _ = decode_igpoint_at(input, input.len());
    }

    // Phase 14 Slice M: PSM `igTextBox` decoder.
    let _ = decode_igtextboxes(input);
    let _ = decode_igtextbox_at(input, 0);
    if !input.is_empty() {
        let _ = decode_igtextbox_at(input, input.len() - 1);
        let _ = decode_igtextbox_at(input, input.len());
    }

    // Phase 14 Slice N: PSM `igSymbol2d` decoder.
    let _ = decode_igsymbols(input);
    let _ = decode_igsymbol_at(input, 0);
    if !input.is_empty() {
        let _ = decode_igsymbol_at(input, input.len() - 1);
        let _ = decode_igsymbol_at(input, input.len());
    }

    // Phase 15 Slice C: PSM `0x00FA` GraphicGroup decoder.
    let _ = decode_graphic_groups(input);
    let _ = decode_graphic_group_at(input, 0);
    if !input.is_empty() {
        let _ = decode_graphic_group_at(input, input.len() - 1);
        let _ = decode_graphic_group_at(input, input.len());
    }

    // Phase 16 Slice D: PSM `0x0030` JStyleOverride decoder (real
    // RAD `style.dll` `JStyleOverride` Version-3 IO).
    let _ = decode_jstyle_overrides(input);
    let _ = decode_jstyle_override_at(input, 0);
    if !input.is_empty() {
        let _ = decode_jstyle_override_at(input, input.len() - 1);
        let _ = decode_jstyle_override_at(input, input.len());
    }

    // Phase 18: PSM `0x0010` sub-record family audit-only decoder.
    let _ = decode_sub_records_0x0010(input);
    let _ = decode_sub_record_0x0010_at(input, 0);
    if !input.is_empty() {
        let _ = decode_sub_record_0x0010_at(input, input.len() - 1);
        let _ = decode_sub_record_0x0010_at(input, input.len());
    }
}

#[test]
fn parsers_do_not_panic_on_adversarial_inputs() {
    let mut corpus = adversarial_inputs();
    push_xorshift_corpus(&mut corpus);

    for input in &corpus {
        // Each call below must return normally; any panic here will
        // surface as a test failure with a useful pointer to the
        // offending parser and the input shape.
        exercise_all_parsers(input);
    }
}

/// Truncation sweep: for every adversarial input, also feed each
/// proper prefix `input[..k]` for `k` in `0..=min(len, 64)`.
///
/// This catches off-by-one regressions that the full-length corpus
/// can mask: a guard like `pos + 4 <= data.len()` may pass on the
/// padded input but fail on a prefix that ends one byte short of
/// a header field. The 64-byte cap is plenty to cover every
/// fixed-size header this crate parses (cluster magic = 16 B,
/// `DocVersion3` record = 48 B, `AppObject` entry header = 20 B,
/// sheet endpoint record = 26 B, etc.).
#[test]
fn parsers_do_not_panic_on_truncated_inputs() {
    let mut corpus = adversarial_inputs();
    push_xorshift_corpus(&mut corpus);

    for input in &corpus {
        let max_k = input.len().min(TRUNCATION_SWEEP_LIMIT);
        for k in 0..=max_k {
            exercise_all_parsers(&input[..k]);
        }
    }
}
