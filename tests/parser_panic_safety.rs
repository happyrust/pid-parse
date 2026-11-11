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
//! The record families are held twice over. Every family decodes where
//! its stream's record chain starts a record and nowhere else, so on bytes
//! that are not a chain a `decode_*s` call returns before a payload is
//! looked at: the corpus above exercises the chain walk through them, not
//! the decoders. Two things reach the decoders:
//!
//! * every `decode_*_at(data, offset)` entry point is called at **every**
//!   offset of every input, one past its end and at the top of the address
//!   space ([`AT_ENTRIES`], [`exercise_every_offset`]) — what a scan that
//!   slid a byte at a time used to do, and what a caller with an offset of
//!   its own can still do;
//! * [`planted_streams`] are well-formed chains — the magic, the record
//!   count, records end to end — whose records carry each family's type
//!   code over lengths at and around the ones the family reads, with noise,
//!   zeros or `0xFF` for a payload. The chain admits them, so the
//!   `decode_*s` entry points hand garbage of the right length to every
//!   payload decoder, counted tails included.
//!
//! When this test fails, the panic message points at the offending
//! parser and input shape — fix the parser, then add a focused
//! regression test in the parser's own `#[cfg(test)] mod tests` so
//! the bug stays nailed down even if this corpus moves on.

use std::collections::HashSet;

use pid_parse::parsers::app_object::parse_app_object;
use pid_parse::parsers::attribute_rows::decode_attribute_row;
use pid_parse::parsers::cluster_header::{
    decode_cluster_body_record_at, decode_cluster_body_records, decode_psm_cluster0_body_records,
    decode_style_cluster_body_records, decode_unclustered_da_body_records, parse_header,
    parse_string_table,
};
use pid_parse::parsers::doc_version::parse_doc_version3;
use pid_parse::parsers::doc_version2::parse_doc_version2;
use pid_parse::parsers::dynamic_attr_records::{
    extract_record_trailers, parse_attribute_records, parse_attribute_records_chain_scoped,
};
use pid_parse::parsers::jproperties::parse_jproperties;
use pid_parse::parsers::jsites_list::parse_jsites_list;
use pid_parse::parsers::psm_tables::{
    parse_psm_cluster_table, parse_psm_roots, parse_psm_segment_table, parse_psm_space_map,
};
use pid_parse::parsers::relationship_probe::probe_relationships;
use pid_parse::parsers::sheet_endpoint_records::parse_endpoint_records;
use pid_parse::parsers::sheet_probe::{probe_sheet_stream, SheetProbeOptions};
use pid_parse::parsers::sheet_records::{
    bspcurve_geometry, collect_normalized_f64_pairs, coordinate_pair_spatial_analysis,
    decode_attribute_fragment_at, decode_attribute_fragments, decode_dependency_object_at,
    decode_dependency_objects, decode_double_value_at, decode_double_values, decode_flavor_holders,
    decode_igarc_at, decode_igarcs, decode_igboundaries, decode_igboundary_at,
    decode_igbspcurve_at, decode_igbspcurves, decode_igcircle_at, decode_igcircles,
    decode_igcomplexstring_at, decode_igcomplexstrings, decode_igdimension_at, decode_igdimensions,
    decode_igellipse_at, decode_igellipses, decode_igellipticalarc_at, decode_igellipticalarcs,
    decode_iggroup_at, decode_iggroups, decode_igline_at, decode_iglines, decode_iglinestring_at,
    decode_iglinestrings, decode_igpoint_at, decode_igpoints, decode_igrectangle_at,
    decode_igrectangles, decode_igsym_boundary_at, decode_igsymbol_at, decode_igsymbols,
    decode_igtextbox_at, decode_igtextboxes, decode_jstyle_override_at, decode_jstyle_overrides,
    decode_primitive_line_at, decode_primitive_lines, decode_smartframe_at, decode_smartframes,
    decode_standard_relation_at, decode_standard_relations, decode_sub_record_0x0010_at,
    decode_sub_records_0x0010, decode_symbol_information_at, decode_symbol_informations,
    decode_variables, decode_variables_at, parse_psm_header, sheet_record_starts, IgLine2dDecoder,
    PsmRecordDecoder, SheetChain, SPATIAL_ANALYSIS_DEFAULT_GRID_N,
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

    push_space_map_bait(&mut out);

    out
}

/// Append `PSMspacemap` payloads that clear the magic sniffer and then
/// lie about every length behind it.
///
/// Without these the generic corpus never gets past
/// [`parse_psm_space_map`](pid_parse::parsers::psm_tables::parse_psm_space_map)'s
/// first four bytes, so its free-list and entry walks would go
/// unexercised. Both magic forms appear, and the truncation sweep chops
/// each one into every prefix, which is what reaches the header's
/// individual `u16` reads.
fn push_space_map_bait(out: &mut Vec<Vec<u8>>) {
    // A free list far longer than the stream can hold.
    let mut runaway_free_list = b"tseg".to_vec();
    runaway_free_list.extend_from_slice(&[0xFF; 8]);
    out.push(runaway_free_list);

    // An empty free list followed by a live entry claiming 65535
    // members, so the entry walk's length arithmetic runs on a count
    // the stream cannot back.
    let mut runaway_entry = b"tseg".to_vec();
    runaway_entry.extend_from_slice(&1u16.to_le_bytes()); // entry count
    runaway_entry.extend_from_slice(&1u16.to_le_bytes()); // slot capacity
    runaway_entry.extend_from_slice(&0u16.to_le_bytes()); // next free index
    runaway_entry.extend_from_slice(&0u16.to_le_bytes()); // free-list length
    runaway_entry.extend_from_slice(&0x0002_1FFFu32.to_le_bytes()); // live head
    runaway_entry.extend_from_slice(&0xFFFFu16.to_le_bytes()); // span
    runaway_entry.extend_from_slice(&0xFFFFu16.to_le_bytes()); // member count
    out.push(runaway_entry);

    // The legacy magic, whose header the reader accepts on the same terms.
    let mut legacy = b"sseg".to_vec();
    legacy.extend_from_slice(&[0x00; 8]);
    out.push(legacy);
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

/// Reinterpret `data` as consecutive 16-byte `(f64, f64)` pairs at
/// 4-byte alignment **without** the normalized-range filter, so
/// `NaN` / `Infinity` / out-of-range coordinates reach
/// [`coordinate_pair_spatial_analysis`] and exercise its clamp path.
fn bytes_as_raw_f64_pairs(data: &[u8]) -> Vec<(f64, f64)> {
    let mut pairs = Vec::new();
    for (offset, window) in data.windows(16).enumerate() {
        if offset % 4 != 0 {
            continue;
        }
        let x = f64::from_le_bytes([
            window[0], window[1], window[2], window[3], window[4], window[5], window[6], window[7],
        ]);
        let y = f64::from_le_bytes([
            window[8], window[9], window[10], window[11], window[12], window[13], window[14],
            window[15],
        ]);
        pairs.push((x, y));
    }
    pairs
}

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

    // Phase 29 Slice B/C: audit-only `/PSMcluster0`, `/StyleCluster`,
    // and `/Unclustered Dynamic Attributes` body record-chain walkers.
    let _ = decode_psm_cluster0_body_records(input);
    let _ = decode_style_cluster_body_records(input);
    let _ = decode_unclustered_da_body_records(input);
    let _ = decode_attribute_row(input);
    // The layer subsystem's readers walk the same chain: a set's bitmaps and
    // layer table are length-prefixed by the file, so every count must be
    // bounds-checked against adversarial lengths.
    let _ = pid_parse::parsers::sheet_layers::decode_sheet_layers(input);
    let _ = pid_parse::parsers::view_filter_sets::decode_view_filter_sets(input);
    // The display order reads a `JSheet` child table by the count the file
    // states, looks for it in a cluster by the sheet's oid, and walks a sheet
    // stream against it. Oid 0 is the one every run of zeros matches.
    // The paper is read from the tail of that record, behind two names whose
    // lengths the file states; it is called on the input itself and not only
    // behind a table that decoded, which few of these inputs hold. A frame's
    // site is a word at a fixed offset of a record the chain walk reads.
    {
        use pid_parse::parsers::sheet_display_order::{
            decode_sheet_child_table, decode_sheet_paper, find_sheet_child_table,
            sheet_display_order, sheet_display_ranks, smart_frame_sites,
        };
        let _ = decode_sheet_paper(input);
        for skipped in [1usize, 9, 61, 102] {
            let _ = decode_sheet_paper(input.get(skipped..).unwrap_or(&[]));
        }
        let _ = smart_frame_sites(input);
        if let Some(table) = decode_sheet_child_table(input) {
            let _ = sheet_display_ranks(input, &table);
        }
        for sheet_oid in [0, 6, u32::MAX] {
            let _ = find_sheet_child_table(input, sheet_oid, |_| false);
            let _ = find_sheet_child_table(input, sheet_oid, |_| true);
            let _ = sheet_display_order(input, input, sheet_oid);
        }
    }
    let _ = decode_cluster_body_records(input, 0);

    let _ = parse_doc_version3(input);
    let _ = parse_doc_version2(input);

    let _ = parse_attribute_records(input);
    let _ = parse_attribute_records_chain_scoped(input);
    let _ = extract_record_trailers(input);

    let _ = parse_jproperties(input);
    let _ = parse_jsites_list(input);

    let _ = parse_psm_roots(input);
    let _ = parse_psm_cluster_table(input);
    let _ = parse_psm_segment_table(input);
    let _ = parse_psm_space_map(input);

    let _ = probe_relationships(input);

    let _ = parse_endpoint_records("/SheetX", input, &empty_rels);
    let _ = probe_sheet_stream("SheetX", "/SheetX", input, &opts);

    let _ = scan_ascii_strings(input, 16);
    let _ = scan_guids(input, 16);
    let _ = scan_utf16le_strings(input, 4, 16);

    let _ = parse_tagged_stg_list(input);

    let _ = parse_app_object(input);

    // The record families' stream entry points. Each walks the stream's
    // record chain and decodes where it starts a record, so on bytes that
    // are not a chain these return at the walk; `planted_streams` are the
    // inputs that carry them into the payload decoders. The `decode_*_at`
    // twins are called at every offset by `exercise_every_offset`.
    let _ = decode_primitive_lines(input);
    let _ = decode_iglines(input);
    let _ = IgLine2dDecoder.scan(input);
    let _ = decode_iglinestrings(input);
    let _ = decode_igpoints(input);
    let _ = decode_igtextboxes(input);
    let _ = decode_igsymbols(input);
    let _ = decode_dependency_objects(input);
    let _ = decode_igboundaries(input);
    // `0x0115` igDimension / JDim: the record's own `main_len` decides
    // where its main area ends and whether a closing word follows, and the
    // measured-geometry slot is read at a fixed offset inside that area —
    // so a lying length must reject the record rather than index past the
    // payload.
    let _ = decode_igdimensions(input);
    // The symbol-information / expression family (`0x00BD`, `0x00C7`,
    // `0x00EA`, `0x006F`, `0x00ED`). All read counted, variable-length
    // tails -- name and formula strings, member and operand lists -- so a
    // corrupt count must reject the record rather than index past the
    // payload.
    let _ = decode_double_values(input);
    let _ = decode_variables(input);
    let _ = decode_symbol_informations(input);
    let _ = decode_standard_relations(input);
    let _ = decode_flavor_holders(input);
    // The curve families: fixed layouts, so the risk is the trailing flag
    // byte and the doubles running off a short payload.
    let _ = decode_igcircles(input);
    let _ = decode_igarcs(input);
    let _ = decode_igellipticalarcs(input);
    let _ = decode_igellipses(input);
    // `0x0020` igRectangle2d and `0x005D` igBspCurve2d: both read counted
    // tails -- an edge list, poles / weights / knots -- so a corrupt count
    // must reject the record rather than index past the payload.
    let _ = decode_igrectangles(input);
    let _ = decode_igbspcurves(input);
    let _ = bspcurve_geometry(input);
    // `0x0021` ComplexString and `0x007B` Group: both read a member count
    // and that many 14-byte entries, so a count the payload cannot hold
    // must reject the record rather than index past it.
    let _ = decode_igcomplexstrings(input);
    let _ = decode_iggroups(input);
    // `0x003D` igSmartFrame2d. The page a drawing's border record proves is
    // read out of this, so a malformed frame must fail the record rather
    // than the parse.
    let _ = decode_smartframes(input);
    // The three families no sheet chain holds a record of (`0x0030`,
    // `0x0010`); their decoders still read at any offset.
    let _ = decode_jstyle_overrides(input);
    let _ = decode_sub_records_0x0010(input);
    let _ = decode_attribute_fragments(input);

    // The chain walk every family decodes at, its all-or-nothing reading,
    // and the fault a stream whose framing does not hold is reported as:
    // whatever the bytes, the walk ends inside them and the fault, when
    // there is one, can be said.
    let chain = SheetChain::walk(input);
    assert!(chain.end <= input.len());
    assert_eq!(chain.unread_bytes(), input.len() - chain.end);
    let _ = sheet_record_starts(input);
    if let Some(fault) = pid_parse::StreamChainFault::of_stream("/SheetX", input) {
        assert!(fault.describe().contains("/SheetX"));
    }

    // Phase 25-A: normalized f64 pair scan + spatial-distribution
    // analysis. `collect_normalized_f64_pairs` is the byte-level entry;
    // chain into `coordinate_pair_spatial_analysis` with degenerate
    // (`0`, `1`) and default grids, plus a raw unfiltered pair vector to
    // drive the NaN / Infinity / out-of-range clamp path.
    let spatial_pairs = collect_normalized_f64_pairs(input);
    let _ = coordinate_pair_spatial_analysis(&spatial_pairs, 0);
    let _ = coordinate_pair_spatial_analysis(&spatial_pairs, 1);
    let _ = coordinate_pair_spatial_analysis(&spatial_pairs, SPATIAL_ANALYSIS_DEFAULT_GRID_N);
    let raw_pairs = bytes_as_raw_f64_pairs(input);
    let _ = coordinate_pair_spatial_analysis(&raw_pairs, SPATIAL_ANALYSIS_DEFAULT_GRID_N);
}

/// One `decode_*_at(data, offset)` entry point: its name, the PSM type
/// code its family reads (`None` for the cluster walker, which reads any),
/// and the call.
type AtEntry = (&'static str, Option<u16>, fn(&[u8], usize));

/// Every public `decode_*_at` entry point of the byte-level parsers. A new
/// one belongs here in the commit that adds it;
/// `every_at_entry_point_is_listed` counts the list against the sources.
const AT_ENTRIES: &[AtEntry] = &[
    ("decode_attribute_fragment_at", Some(0x0010), |data, at| {
        let _ = decode_attribute_fragment_at(data, at);
    }),
    ("decode_cluster_body_record_at", None, |data, at| {
        let _ = decode_cluster_body_record_at(data, at);
    }),
    ("decode_dependency_object_at", Some(0x00FA), |data, at| {
        let _ = decode_dependency_object_at(data, at);
    }),
    ("decode_double_value_at", Some(0x00C7), |data, at| {
        let _ = decode_double_value_at(data, at);
    }),
    ("decode_igarc_at", Some(0x0061), |data, at| {
        let _ = decode_igarc_at(data, at);
    }),
    ("decode_igboundary_at", Some(0x0013), |data, at| {
        let _ = decode_igboundary_at(data, at);
    }),
    ("decode_igbspcurve_at", Some(0x005D), |data, at| {
        let _ = decode_igbspcurve_at(data, at);
    }),
    ("decode_igcircle_at", Some(0x0059), |data, at| {
        let _ = decode_igcircle_at(data, at);
    }),
    ("decode_igcomplexstring_at", Some(0x0021), |data, at| {
        let _ = decode_igcomplexstring_at(data, at);
    }),
    ("decode_igdimension_at", Some(0x0115), |data, at| {
        let _ = decode_igdimension_at(data, at);
    }),
    ("decode_igellipse_at", Some(0x0063), |data, at| {
        let _ = decode_igellipse_at(data, at);
    }),
    ("decode_igellipticalarc_at", Some(0x007E), |data, at| {
        let _ = decode_igellipticalarc_at(data, at);
    }),
    ("decode_iggroup_at", Some(0x007B), |data, at| {
        let _ = decode_iggroup_at(data, at);
    }),
    ("decode_igline_at", Some(0x0018), |data, at| {
        let _ = decode_igline_at(data, at);
        // The trait seam under the wrapper keeps its own bar.
        let _ = IgLine2dDecoder.decode_at(data, at);
    }),
    ("decode_iglinestring_at", Some(0x0084), |data, at| {
        let _ = decode_iglinestring_at(data, at);
    }),
    ("decode_igpoint_at", Some(0x005E), |data, at| {
        let _ = decode_igpoint_at(data, at);
    }),
    ("decode_igrectangle_at", Some(0x0020), |data, at| {
        let _ = decode_igrectangle_at(data, at);
    }),
    // A symbol body's boundary: the same type code as a sheet's, another
    // payload grammar.
    ("decode_igsym_boundary_at", Some(0x0013), |data, at| {
        let _ = decode_igsym_boundary_at(data, at);
    }),
    ("decode_igsymbol_at", Some(0x00CE), |data, at| {
        let _ = decode_igsymbol_at(data, at);
    }),
    ("decode_igtextbox_at", Some(0x004D), |data, at| {
        let _ = decode_igtextbox_at(data, at);
    }),
    ("decode_jstyle_override_at", Some(0x0030), |data, at| {
        let _ = decode_jstyle_override_at(data, at);
    }),
    ("decode_primitive_line_at", Some(0x3FE6), |data, at| {
        let _ = decode_primitive_line_at(data, at);
    }),
    ("decode_smartframe_at", Some(0x003D), |data, at| {
        let _ = decode_smartframe_at(data, at);
    }),
    ("decode_standard_relation_at", Some(0x006F), |data, at| {
        let _ = decode_standard_relation_at(data, at);
    }),
    ("decode_sub_record_0x0010_at", Some(0x0010), |data, at| {
        let _ = decode_sub_record_0x0010_at(data, at);
    }),
    ("decode_symbol_information_at", Some(0x00BD), |data, at| {
        let _ = decode_symbol_information_at(data, at);
    }),
    ("decode_variables_at", Some(0x00EA), |data, at| {
        let _ = decode_variables_at(data, at);
    }),
];

/// Call every `_at` entry point, and the envelope helper under them, at
/// every offset of `input`: each byte, the end, one past it, and the two
/// offsets at the top of the address space where `offset + 6` overflows.
///
/// The family decoders used to be reached at every offset by the scan that
/// slid a byte at a time. They decode on the record chain now, so nothing
/// but this walk hands them an offset the chain did not choose — which is
/// what a caller holding an offset of its own does.
fn exercise_every_offset(input: &[u8]) {
    let offsets = (0..=input.len() + 1).chain([usize::MAX - 5, usize::MAX]);
    for at in offsets {
        let _ = parse_psm_header(input, at);
        for (_, _, decode) in AT_ENTRIES {
            decode(input, at);
        }
    }
}

/// The lengths a planted record states: the fixed payload sizes the
/// families read, their neighbours, the smallest the counted families
/// accept, and a few a count can run past.
const PLANTED_LENGTHS: [u32; 28] = [
    1, 4, 5, 6, 17, 18, 22, 26, 34, 42, 43, 44, 49, 50, 51, 58, 59, 60, 74, 75, 76, 90, 98, 149,
    156, 238, 400, 1024,
];

/// What a planted record's payload is made of.
#[derive(Clone, Copy)]
enum Fill {
    /// xorshift32 noise: counts and doubles of every size.
    Noise(u32),
    /// Zeros: empty counts, zero-length strings, zero vectors.
    Zeros,
    /// `0xFF`: counts of four thousand million, NaN doubles.
    Ones,
}

/// Chain streams that reach the payload decoders.
///
/// One stream per type code a family reads, per fill, per pair of flag
/// bits: the chain magic, the count of the records the native reader
/// reads, and one record of that type code for each of
/// [`PLANTED_LENGTHS`], end to end. The chain walks, so every
/// `decode_*s` entry point decodes at each record, and what it finds
/// behind the envelope is a payload of a length the family may accept and
/// content it has to refuse or survive. A last stream of each kind is cut
/// in the middle of a record, which leaves a chain that stops short.
fn planted_streams() -> Vec<Vec<u8>> {
    let mut codes: Vec<u16> = AT_ENTRIES.iter().filter_map(|(_, code, _)| *code).collect();
    // The symbol-information family's fifth member and the two layer
    // classes have no `_at` twin; their stream entry points are in
    // `exercise_all_parsers`.
    codes.extend([0x00ED, 0x0081, 0x0042, 0x0057, 0x0114]);
    codes.sort_unstable();
    codes.dedup();

    let mut out = Vec::new();
    for (index, &code) in codes.iter().enumerate() {
        let seed = 0x9E37_79B9u32.wrapping_mul(index as u32 + 1) | 1;
        for fill in [Fill::Noise(seed), Fill::Zeros, Fill::Ones] {
            for flags in [0u16, 0x8000, 0x4000] {
                let mut state = match fill {
                    Fill::Noise(seed) => seed,
                    Fill::Zeros | Fill::Ones => 0,
                };
                let mut stream = vec![0u8; 8];
                stream[..4].copy_from_slice(&0x6C90_F544u32.to_le_bytes());
                for length in PLANTED_LENGTHS {
                    stream.extend_from_slice(&(code | flags).to_le_bytes());
                    stream.extend_from_slice(&length.to_le_bytes());
                    for _ in 0..length {
                        stream.push(match fill {
                            Fill::Noise(_) => {
                                state ^= state << 13;
                                state ^= state >> 17;
                                state ^= state << 5;
                                (state & 0xFF) as u8
                            }
                            Fill::Zeros => 0,
                            Fill::Ones => 0xFF,
                        });
                    }
                }
                let live = if flags & 0x8000 == 0 {
                    PLANTED_LENGTHS.len() as u32
                } else {
                    0
                };
                stream[4..8].copy_from_slice(&live.to_le_bytes());
                out.push(stream);
            }
        }
        // The same chain cut inside its last record: the walk stops there
        // and the records before it still decode.
        if let Some(whole) = out.last().cloned() {
            out.push(whole[..whole.len() - 7].to_vec());
        }
    }
    out
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
        exercise_every_offset(input);
    }
}

/// The streams whose chain walks: every stream entry point gets past the
/// chain gate, and every `_at` entry point is tried at every offset of a
/// buffer that is envelopes and payloads from end to end.
#[test]
fn record_decoders_do_not_panic_on_planted_chains() {
    let streams = planted_streams();
    let mut reached = 0usize;
    for stream in &streams {
        // The plant is a chain: this is what makes the stream entry points
        // decode rather than return at the walk.
        let chain = SheetChain::walk(stream);
        assert!(
            chain.starts.len() >= PLANTED_LENGTHS.len() - 1,
            "a planted stream walks {} record(s)",
            chain.starts.len()
        );
        reached += chain.starts.len();
        exercise_all_parsers(stream);
        exercise_every_offset(stream);
    }
    eprintln!(
        "{} planted stream(s), {reached} record(s) handed to the decoders",
        streams.len()
    );
    assert!(
        streams.len() >= 240 && reached >= streams.len() * (PLANTED_LENGTHS.len() - 1),
        "{} planted stream(s) hand the decoders {reached} record(s)",
        streams.len()
    );
}

/// The list above is every `decode_*_at` the parsers export: the sources
/// are read for `pub fn …_at(`, so an entry point added without a row here
/// fails this rather than going unexercised.
#[test]
fn every_at_entry_point_is_listed() {
    let mut exported: Vec<String> = Vec::new();
    let parsers = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/parsers");
    let mut files: Vec<_> = std::fs::read_dir(&parsers)
        .unwrap_or_else(|error| panic!("{}: {error}", parsers.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect();
    files.sort();
    for file in files {
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("{}: {error}", file.display()));
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("pub fn ") else {
                continue;
            };
            let name = rest.split('(').next().unwrap_or_default();
            if name.ends_with("_at") {
                exported.push(name.to_string());
            }
        }
    }
    exported.sort();
    let mut listed: Vec<&str> = AT_ENTRIES.iter().map(|(name, _, _)| *name).collect();
    listed.sort_unstable();
    assert_eq!(
        listed, exported,
        "AT_ENTRIES against the `pub fn …_at(` of src/parsers"
    );
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
            exercise_every_offset(&input[..k]);
        }
    }
    // A planted chain cut at every length: each record in turn is the one
    // the stream ends inside, at each of its bytes.
    for stream in planted_streams().iter().step_by(31) {
        for k in 0..=stream.len() {
            let cut = &stream[..k];
            let starts = SheetChain::walk(cut).starts;
            // The record the cut falls in, and the one before it.
            for &at in starts.iter().rev().take(2) {
                for (_, _, decode) in AT_ENTRIES {
                    decode(cut, at);
                }
            }
            if let Some(&last) = starts.last() {
                // And where the next record would have started.
                let _ = parse_psm_header(cut, last);
                let next = SheetChain::walk(cut).end;
                for (_, _, decode) in AT_ENTRIES {
                    decode(cut, next);
                }
            }
        }
    }
}
