//! Aggregate byte-audit report across every stream in a [`PidPackage`].
//!
//! Phase 12b-2-minimal: wire together the `_with_trace` parser variants
//! landed across 12b-1 / 12b-1b / 12b-1c / 12b-1d into a single entry
//! point that yields a whole-file byte coverage picture.
//!
//! The report classifies each stream into one of two buckets:
//! - **Traced** — a parser with a `_with_trace` variant was run against
//!   the stream; the full [`ParserTrace`] is retained.
//! - **Unregistered** — no parser was wired up for this stream path, so
//!   every byte surfaces as leftover and the summary records
//!   `parser_name = None`. This is exactly the "coverage trajectory"
//!   signal roadmap Phase 4 asks for: when a future parser migration
//!   lands, the previously-unregistered stream flips to traced and its
//!   coverage ratio jumps — easy to flag as a regression guard.
//!
//! `pid_inspect --byte-audit` uses this same aggregate model for both
//! text output and JSON / baseline comparison workflows.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::byte_audit::{ByteRange, ParserTrace, ParserTraceBuilder, TraceConfidence};
use crate::package::PidPackage;
use crate::parsers;
use crate::writer::summary_write::{DOC_SUMMARY_PATH, SUMMARY_INFO_PATH};

/// Per-stream rollup pulled from the matching [`ParserTrace`] (when a
/// parser was registered) or synthesized from the raw stream length
/// (when no parser covers the path).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StreamAuditSummary {
    /// Normalized stream path, e.g. `/PSMclustertable`.
    pub path: String,
    /// Total bytes in the stream.
    pub total_bytes: u64,
    /// Bytes a parser claimed. Zero for unregistered streams.
    pub consumed_bytes: u64,
    /// Bytes the parser did not claim. Equals `total_bytes` for
    /// unregistered streams.
    pub leftover_bytes: u64,
    /// `consumed_bytes / total_bytes`, or `0.0` when `total_bytes == 0`.
    pub coverage_ratio: f32,
    /// The parser name (e.g. `"parse_psm_roots"`) whose trace covers
    /// this stream, or `None` when no parser is registered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parser_name: Option<String>,
}

/// Whole-package byte audit: per-stream summaries + roll-up totals.
///
/// Serializable so CI / tooling can diff reports across revisions — the
/// primary use case roadmap Phase 4 asks for (regression guard against
/// coverage drops).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ByteAuditReport {
    /// Every `ParserTrace` the aggregation produced. Order follows
    /// [`PidPackage::streams`] iteration (`BTreeMap` on normalized path).
    pub traces: Vec<ParserTrace>,
    /// Sum of `stream_size` for every stream in the package.
    pub total_file_bytes: u64,
    /// Sum of `consumed_bytes` across traced streams.
    pub overall_consumed: u64,
    /// Sum of `leftover_bytes` across every stream (traced +
    /// unregistered).
    pub overall_leftover: u64,
    /// `overall_consumed / total_file_bytes`, or `0.0` when the package
    /// contains no streams. NaN-safe by construction.
    pub overall_coverage_ratio: f32,
    /// One summary per stream, keyed by path. `BTreeMap` so JSON output
    /// is deterministic.
    pub per_stream: BTreeMap<String, StreamAuditSummary>,
    /// Convenience projection: stream paths that have no registered
    /// parser (sorted for determinism). Useful for CI assertions.
    pub unregistered_paths: Vec<String>,
}

impl ByteAuditReport {
    /// Return `true` iff every stream has a registered parser.
    pub fn all_streams_have_registered_parser(&self) -> bool {
        self.unregistered_paths.is_empty()
    }

    /// Return the number of streams whose trace consumed *every* byte of
    /// the stream (no leftover). Streams with `total_bytes == 0` count
    /// as fully consumed only when a parser was registered.
    pub fn fully_consumed_stream_count(&self) -> usize {
        self.per_stream
            .values()
            .filter(|s| s.leftover_bytes == 0 && s.parser_name.is_some())
            .count()
    }
}

/// Run every registered `_with_trace` parser against the matching
/// stream of `pkg` and return the aggregate report.
///
/// Unknown / unrecognized stream paths are entered with
/// `parser_name = None`, `consumed_bytes = 0`, and
/// `leftover_bytes = total_bytes`.
pub fn byte_audit_report(pkg: &PidPackage) -> ByteAuditReport {
    let mut traces: Vec<ParserTrace> = Vec::new();
    let mut per_stream: BTreeMap<String, StreamAuditSummary> = BTreeMap::new();
    let mut unregistered_paths: Vec<String> = Vec::new();

    for (path, stream) in &pkg.streams {
        let total_bytes = stream.data.len() as u64;
        match run_registered_parser(path, &stream.data) {
            Some(trace) => {
                let summary = StreamAuditSummary {
                    path: path.clone(),
                    total_bytes,
                    consumed_bytes: trace.consumed_bytes(),
                    leftover_bytes: trace.leftover_bytes(),
                    coverage_ratio: trace.coverage_ratio(),
                    parser_name: Some(trace.parser_name.clone()),
                };
                per_stream.insert(path.clone(), summary);
                traces.push(trace);
            }
            None => {
                let summary = StreamAuditSummary {
                    path: path.clone(),
                    total_bytes,
                    consumed_bytes: 0,
                    leftover_bytes: total_bytes,
                    coverage_ratio: 0.0,
                    parser_name: None,
                };
                per_stream.insert(path.clone(), summary);
                unregistered_paths.push(path.clone());
            }
        }
    }

    let total_file_bytes: u64 = per_stream.values().map(|s| s.total_bytes).sum();
    let overall_consumed: u64 = per_stream.values().map(|s| s.consumed_bytes).sum();
    let overall_leftover: u64 = per_stream.values().map(|s| s.leftover_bytes).sum();
    let overall_coverage_ratio = if total_file_bytes == 0 {
        0.0
    } else {
        overall_consumed as f32 / total_file_bytes as f32
    };

    ByteAuditReport {
        traces,
        total_file_bytes,
        overall_consumed,
        overall_leftover,
        overall_coverage_ratio,
        per_stream,
        unregistered_paths,
    }
}

/// Dispatch `path` to the right `_with_trace` parser, if any. Returns
/// the finalized [`ParserTrace`] — or `None` when there is no registered
/// parser for the path.
///
/// The dispatcher runs the parser even if it returns `None` at the
/// semantic level (e.g. magic mismatch), because the builder still
/// records whatever bytes **were** consumed before the early-exit. That
/// is exactly the information a byte-audit consumer wants to see.
fn run_registered_parser(path: &str, data: &[u8]) -> Option<ParserTrace> {
    let (parser_name, executed) = match path {
        p if p == SUMMARY_INFO_PATH => ("parse_summary_property_set", {
            let mut b = ParserTraceBuilder::new("parse_summary_property_set");
            let _ = parsers::summary::parse_summary_property_set_with_trace(data, &mut b);
            Some(b)
        }),
        p if p == DOC_SUMMARY_PATH => ("parse_summary_property_set", {
            let mut b = ParserTraceBuilder::new("parse_summary_property_set");
            let _ = parsers::summary::parse_summary_property_set_with_trace(data, &mut b);
            Some(b)
        }),
        "/PSMcluster0" => ("parse_psm_cluster0", {
            let mut b = ParserTraceBuilder::new("parse_psm_cluster0");
            let _ = parsers::cluster_header::parse_psm_cluster0_with_trace(data, &mut b);
            Some(b)
        }),
        "/StyleCluster" => ("parse_cluster_header", {
            let mut b = ParserTraceBuilder::new("parse_cluster_header");
            let _ = parsers::cluster_header::parse_header_with_trace(data, &mut b);
            Some(b)
        }),
        "/Dynamic Attributes Metadata" => ("parse_cluster_header", {
            let mut b = ParserTraceBuilder::new("parse_cluster_header");
            let _ = parsers::cluster_header::parse_header_with_trace(data, &mut b);
            Some(b)
        }),
        "/Unclustered Dynamic Attributes" => ("scan_da_record_trailers", {
            let mut b = ParserTraceBuilder::new("scan_da_record_trailers");
            let _ = parsers::dynamic_attr_records::scan_da_record_trailers_with_trace(data, &mut b);
            Some(b)
        }),
        "/PSMroots" => ("parse_psm_roots", {
            let mut b = ParserTraceBuilder::new("parse_psm_roots");
            let _ = parsers::psm_tables::parse_psm_roots_with_trace(data, &mut b);
            Some(b)
        }),
        "/PSMclustertable" => ("parse_psm_cluster_table", {
            let mut b = ParserTraceBuilder::new("parse_psm_cluster_table");
            let _ = parsers::psm_tables::parse_psm_cluster_table_with_trace(data, &mut b);
            Some(b)
        }),
        "/PSMsegmenttable" => ("parse_psm_segment_table", {
            let mut b = ParserTraceBuilder::new("parse_psm_segment_table");
            let _ = parsers::psm_tables::parse_psm_segment_table_with_trace(data, &mut b);
            Some(b)
        }),
        "/DocVersion2" => ("parse_doc_version2", {
            let mut b = ParserTraceBuilder::new("parse_doc_version2");
            let _ = parsers::doc_version2::parse_doc_version2_with_trace(data, &mut b);
            Some(b)
        }),
        "/DocVersion3" => ("parse_doc_version3", {
            let mut b = ParserTraceBuilder::new("parse_doc_version3");
            let _ = parsers::doc_version::parse_doc_version3_with_trace(data, &mut b);
            Some(b)
        }),
        "/AppObject" => ("parse_app_object", {
            let mut b = ParserTraceBuilder::new("parse_app_object");
            let _ = parsers::app_object::parse_app_object_with_trace(data, &mut b);
            Some(b)
        }),
        "/JTaggedTxtStgList" => ("parse_tagged_stg_list", {
            let mut b = ParserTraceBuilder::new("parse_tagged_stg_list");
            let _ = parsers::tagged_stg_list::parse_tagged_stg_list_with_trace(data, &mut b);
            Some(b)
        }),
        "/TaggedTxtData/Drawing" => ("parse_drawing_xml", {
            Some(trace_utf8_xml_stream(
                "parse_drawing_xml",
                data,
                parsers::drawing_xml::parse_drawing_xml,
            ))
        }),
        "/TaggedTxtData/General" => ("parse_general_xml", {
            Some(trace_utf8_xml_stream(
                "parse_general_xml",
                data,
                parsers::general_xml::parse_general_xml,
            ))
        }),
        path if path.ends_with("/JProperties") => ("parse_jproperties", {
            let mut b = ParserTraceBuilder::new("parse_jproperties");
            let _ = parsers::jproperties::parse_jproperties_with_trace(data, &mut b);
            Some(b)
        }),
        path if top_level_sheet_name(path).is_some() => ("probe_sheet_stream", {
            let mut b = ParserTraceBuilder::new("probe_sheet_stream");
            let sheet_name = top_level_sheet_name(path).expect("guard checked sheet name");
            trace_sheet_text_runs(sheet_name, path, data, &mut b);
            // Also scan for 26-byte endpoint records (Phase 12b-1g):
            // self-contained discriminator-only scan, no need for the
            // DA-side `rel_field_xs` set.
            let _ = parsers::sheet_endpoint_records::scan_endpoint_records_with_trace(data, &mut b);
            Some(b)
        }),
        _ => return None,
    };
    executed
        .map(|b| b.build(path, data.len() as u64))
        .map(|mut t| {
            // parser_name is the canonical name (helper literal above) —
            // keeps the trace robust even if a future caller passes a
            // differently-named builder.
            if t.parser_name.is_empty() {
                t.parser_name = parser_name.to_string();
            }
            t
        })
}

fn trace_utf8_xml_stream<T, E>(
    parser_name: &str,
    data: &[u8],
    parse: impl FnOnce(&str) -> Result<T, E>,
) -> ParserTraceBuilder {
    let mut builder = ParserTraceBuilder::new(parser_name);
    let Ok(xml) = std::str::from_utf8(data) else {
        return builder;
    };
    if parse(xml).is_ok() {
        builder.consume(
            ByteRange::new(0, data.len() as u64),
            TraceConfidence::Decoded,
        );
    }
    builder
}

fn top_level_sheet_name(path: &str) -> Option<&str> {
    let leaf = path.strip_prefix('/')?;
    if leaf.contains('/') || !leaf.starts_with("Sheet") {
        return None;
    }
    Some(leaf)
}

fn trace_sheet_text_runs(
    sheet_name: &str,
    path: &str,
    data: &[u8],
    trace: &mut ParserTraceBuilder,
) {
    let report = parsers::sheet_probe::probe_sheet_stream(
        sheet_name,
        path,
        data,
        &parsers::sheet_probe::SheetProbeOptions::default(),
    );
    let mut consumed: Vec<(u64, u64)> = Vec::new();
    for run in report.text_runs {
        let start = run.offset as u64;
        let end = run.offset.saturating_add(run.byte_len).min(data.len()) as u64;
        if start >= end {
            continue;
        }
        if consumed
            .iter()
            .any(|&(prev_start, prev_end)| start < prev_end && prev_start < end)
        {
            continue;
        }
        trace.consume(ByteRange::new(start, end), TraceConfidence::Probed);
        consumed.push((start, end));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PidDocument;
    use crate::package::{PidPackage, RawStream};
    use std::collections::BTreeMap;

    fn pkg_with_streams(entries: &[(&str, Vec<u8>)]) -> PidPackage {
        let mut streams = BTreeMap::new();
        for (path, data) in entries {
            streams.insert(
                (*path).to_string(),
                RawStream {
                    path: (*path).to_string(),
                    data: data.clone(),
                    modified: false,
                },
            );
        }
        PidPackage::new(None, streams, PidDocument::default())
    }

    fn make_psm_segment_bytes(count: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&0x6261_7473u32.to_le_bytes()); // STAB_MAGIC
        data.extend_from_slice(&count.to_le_bytes());
        data.resize(data.len() + count as usize, 0x01);
        data
    }

    fn make_doc_version2_bytes() -> Vec<u8> {
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(&0x0001_0034u32.to_le_bytes());
        // One record: op_type=0x82, fixed=[00,00,09], separator=0x00,
        // version=144 LE
        data.extend_from_slice(&[0x82, 0x00, 0x00, 0x09, 0x00, 0x90, 0x00, 0x00, 0x00]);
        data
    }

    fn utf16le(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn byte_audit_report_splits_traced_and_unregistered_streams() {
        let pkg = pkg_with_streams(&[
            ("/PSMsegmenttable", make_psm_segment_bytes(2)),
            ("/DocVersion2", make_doc_version2_bytes()),
            ("/MysteryStream", vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE]),
        ]);

        let report = byte_audit_report(&pkg);

        // Two registered traces, one unregistered.
        assert_eq!(report.traces.len(), 2);
        assert_eq!(
            report.unregistered_paths,
            vec!["/MysteryStream".to_string()]
        );
        assert!(!report.all_streams_have_registered_parser());

        let seg_summary = &report.per_stream["/PSMsegmenttable"];
        assert_eq!(
            seg_summary.parser_name.as_deref(),
            Some("parse_psm_segment_table")
        );
        assert_eq!(seg_summary.leftover_bytes, 0, "segment stream fully traced");

        let dv2_summary = &report.per_stream["/DocVersion2"];
        assert_eq!(
            dv2_summary.parser_name.as_deref(),
            Some("parse_doc_version2")
        );
        assert_eq!(dv2_summary.leftover_bytes, 0);

        let mystery = &report.per_stream["/MysteryStream"];
        assert_eq!(mystery.parser_name, None);
        assert_eq!(mystery.consumed_bytes, 0);
        assert_eq!(mystery.leftover_bytes, 6);
    }

    #[test]
    fn tagged_text_xml_streams_are_registered_and_fully_consumed() {
        let drawing_xml = br#"<Drawing><DrawingNumber>D-001</DrawingNumber></Drawing>"#.to_vec();
        let general_xml =
            br#"<General><Location>C:\plant\drawing.pid</Location></General>"#.to_vec();
        let pkg = pkg_with_streams(&[
            ("/TaggedTxtData/Drawing", drawing_xml.clone()),
            ("/TaggedTxtData/General", general_xml.clone()),
        ]);

        let report = byte_audit_report(&pkg);

        assert_eq!(report.traces.len(), 2);
        assert!(report.unregistered_paths.is_empty());

        let drawing = &report.per_stream["/TaggedTxtData/Drawing"];
        assert_eq!(drawing.parser_name.as_deref(), Some("parse_drawing_xml"));
        assert_eq!(drawing.consumed_bytes, drawing_xml.len() as u64);
        assert_eq!(drawing.leftover_bytes, 0);

        let general = &report.per_stream["/TaggedTxtData/General"];
        assert_eq!(general.parser_name.as_deref(), Some("parse_general_xml"));
        assert_eq!(general.consumed_bytes, general_xml.len() as u64);
        assert_eq!(general.leftover_bytes, 0);
    }

    #[test]
    fn jproperties_streams_are_registered_with_partial_probed_coverage() {
        let mut data = vec![0xFF, 0x00];
        data.extend_from_slice(b"SymbolName");
        data.extend_from_slice(&[0x00, 0x00]);
        data.extend(utf16le("PUMP-101"));
        data.push(0xEE);
        let total = data.len() as u64;
        let pkg = pkg_with_streams(&[("/JSite0001/JProperties", data)]);

        let report = byte_audit_report(&pkg);

        assert!(report.unregistered_paths.is_empty());
        let summary = &report.per_stream["/JSite0001/JProperties"];
        assert_eq!(summary.parser_name.as_deref(), Some("parse_jproperties"));
        assert!(summary.consumed_bytes > 0);
        assert!(summary.consumed_bytes < total);
        assert_eq!(summary.leftover_bytes, total - summary.consumed_bytes);
    }

    fn make_cluster_header(record_count: u32, body_len: u32) -> Vec<u8> {
        let mut h = Vec::new();
        h.extend_from_slice(&0x6C90_F544u32.to_le_bytes()); // CLUSTER_MAGIC
        h.extend_from_slice(&record_count.to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes()); // stream_type
        h.extend_from_slice(&body_len.to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes()); // flags
        h
    }

    #[test]
    fn cluster_streams_are_registered_with_header_only_coverage() {
        // /StyleCluster fixture: header (16B) + opaque body (32B).
        let mut style = make_cluster_header(0, 0);
        style.extend_from_slice(&[0xAB; 32]);
        // /Dynamic Attributes Metadata fixture: header + body.
        let mut da_meta = make_cluster_header(0, 0);
        da_meta.extend_from_slice(&[0xCD; 16]);

        let pkg = pkg_with_streams(&[
            ("/StyleCluster", style.clone()),
            ("/Dynamic Attributes Metadata", da_meta.clone()),
        ]);

        let report = byte_audit_report(&pkg);
        assert!(report.unregistered_paths.is_empty());

        let style_summary = &report.per_stream["/StyleCluster"];
        assert_eq!(
            style_summary.parser_name.as_deref(),
            Some("parse_cluster_header")
        );
        assert_eq!(style_summary.consumed_bytes, 16);
        assert_eq!(style_summary.leftover_bytes, style.len() as u64 - 16);

        let da_summary = &report.per_stream["/Dynamic Attributes Metadata"];
        assert_eq!(
            da_summary.parser_name.as_deref(),
            Some("parse_cluster_header")
        );
        assert_eq!(da_summary.consumed_bytes, 16);
        assert_eq!(da_summary.leftover_bytes, da_meta.len() as u64 - 16);
    }

    #[test]
    fn unclustered_dynamic_attributes_traces_31_byte_record_trailers() {
        // Build a stream with one synthetic trailer-bearing PIDAttributes
        // record. Layout matches `dynamic_attr_records::tests::
        // make_synthetic_da_body_with_one_trailer` so the assertions
        // stay anchored to the same fixture used in the unit tests.
        let mut data = vec![0x00];
        data.extend_from_slice(b"P&IDAttributes");
        data.extend_from_slice(&[0xAB; 5]);
        let mut trailer = Vec::with_capacity(31);
        trailer.extend_from_slice(&[0x89, 0x00]);
        trailer.extend_from_slice(&100u32.to_le_bytes());
        trailer.extend_from_slice(&7u32.to_le_bytes());
        trailer.extend_from_slice(&[0u8; 8]);
        trailer.extend_from_slice(&0x0000_03B7u32.to_le_bytes());
        trailer.extend_from_slice(&[0xFF, 0xFF]);
        trailer.extend_from_slice(&0x0000_00EAu32.to_le_bytes());
        trailer.extend_from_slice(&[0x14, 0x00, 0x00]);
        data.extend_from_slice(&trailer);

        let pkg = pkg_with_streams(&[("/Unclustered Dynamic Attributes", data.clone())]);
        let report = byte_audit_report(&pkg);
        let summary = &report.per_stream["/Unclustered Dynamic Attributes"];
        assert_eq!(
            summary.parser_name.as_deref(),
            Some("scan_da_record_trailers")
        );
        assert_eq!(
            summary.consumed_bytes, 31,
            "exactly one 31-byte trailer should be consumed; got {}",
            summary.consumed_bytes
        );
        assert_eq!(summary.leftover_bytes, data.len() as u64 - 31);
    }

    #[test]
    fn psm_cluster0_stream_traces_header_locator_and_string_table() {
        let mut data = make_cluster_header(0, 0);
        // 12-byte locator gap (Probed).
        data.extend_from_slice(&[0u8; 12]);
        // Two string-table entries anchored at the entry-2 marker.
        data.extend_from_slice(&1u32.to_le_bytes()); // entry 1 index
        data.extend_from_slice(&4u32.to_le_bytes()); // byte_len
        data.extend_from_slice(&utf16le("AB"));
        data.extend_from_slice(&2u32.to_le_bytes()); // entry 2 index
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(&utf16le("CD"));
        // sentinel
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());

        let pkg = pkg_with_streams(&[("/PSMcluster0", data.clone())]);
        let report = byte_audit_report(&pkg);
        let summary = &report.per_stream["/PSMcluster0"];
        assert_eq!(summary.parser_name.as_deref(), Some("parse_psm_cluster0"));
        assert_eq!(
            summary.consumed_bytes,
            data.len() as u64,
            "header + locator gap + string table sentinel must cover every byte",
        );
        assert_eq!(summary.leftover_bytes, 0);
    }

    #[test]
    fn summary_streams_are_registered_and_fully_consumed() {
        // Minimal PropertySetStream: 28-byte prefix + 20-byte section
        // header + 16-byte section body (size + 1 prop + VT_LPSTR).
        let prop_payload = {
            let mut v = Vec::new();
            v.extend_from_slice(&0x0000_001Eu32.to_le_bytes()); // VT_LPSTR
            let bytes = b"Title\0";
            v.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            v.extend_from_slice(bytes);
            v
        };
        let header_len = 8 + 8;
        let total = header_len + prop_payload.len();
        let mut section = Vec::new();
        section.extend_from_slice(&(total as u32).to_le_bytes());
        section.extend_from_slice(&1u32.to_le_bytes());
        section.extend_from_slice(&2u32.to_le_bytes()); // PROPID 2 = Title
        section.extend_from_slice(&(header_len as u32).to_le_bytes());
        section.extend_from_slice(&prop_payload);

        let mut stream = Vec::new();
        stream.extend_from_slice(&0xFFFEu16.to_le_bytes()); // ByteOrder
        stream.extend_from_slice(&0u16.to_le_bytes()); // Version
        stream.extend_from_slice(&0u32.to_le_bytes()); // SystemIdentifier
        stream.extend_from_slice(&[0u8; 16]); // CLSID
        stream.extend_from_slice(&1u32.to_le_bytes()); // NumPropertySets
        stream.extend_from_slice(&[
            0xE0, 0x85, 0x9F, 0xF2, 0xF9, 0x4F, 0x68, 0x10, 0xAB, 0x91, 0x08, 0x00, 0x2B, 0x27,
            0xB3, 0xD9,
        ]); // FMTID_SUMMARY
        let section_offset: u32 = 28 + 20;
        stream.extend_from_slice(&section_offset.to_le_bytes());
        stream.extend_from_slice(&section);

        let pkg = pkg_with_streams(&[
            (SUMMARY_INFO_PATH, stream.clone()),
            (DOC_SUMMARY_PATH, stream.clone()),
        ]);

        let report = byte_audit_report(&pkg);
        assert!(report.unregistered_paths.is_empty());
        for path in [SUMMARY_INFO_PATH, DOC_SUMMARY_PATH] {
            let summary = &report.per_stream[path];
            assert_eq!(
                summary.parser_name.as_deref(),
                Some("parse_summary_property_set"),
                "expected summary trace for {path}"
            );
            assert_eq!(
                summary.consumed_bytes,
                stream.len() as u64,
                "every byte must be claimed for fully-decoded fixture at {path}"
            );
            assert_eq!(summary.leftover_bytes, 0, "no leftover for {path}");
        }
    }

    #[test]
    fn sheet_streams_are_registered_with_partial_text_run_coverage() {
        let mut data = vec![0x11; 8];
        data.extend_from_slice(b"ASCII-TAGS");
        data.extend_from_slice(&[0x00, 0x00]);
        data.extend(utf16le("PUMP-101"));
        data.extend_from_slice(&[0x22; 8]);
        let total = data.len() as u64;
        let pkg = pkg_with_streams(&[("/Sheet6", data)]);

        let report = byte_audit_report(&pkg);

        assert!(report.unregistered_paths.is_empty());
        let summary = &report.per_stream["/Sheet6"];
        assert_eq!(summary.parser_name.as_deref(), Some("probe_sheet_stream"));
        assert!(summary.consumed_bytes > 0);
        assert!(summary.consumed_bytes < total);
        assert_eq!(summary.leftover_bytes, total - summary.consumed_bytes);
    }

    #[test]
    fn sheet_endpoint_records_get_traced_alongside_text_runs() {
        // Build a sheet stream that contains both an ASCII text run and
        // a 26-byte endpoint record. Both should land inside the
        // consumed bucket; the leftover region must shrink relative to
        // a stream with text runs only.
        let mut data = vec![0x33; 8];
        data.extend_from_slice(b"TAG-RUN-ASCII"); // text run candidate
        data.extend_from_slice(&[0x00, 0x00]);
        // Endpoint record signature (26B):
        // u32 rel_fx | u32=0x06 | 6×0 | u16=0x02 | u32 ep_a | u16=0x01 | u32 ep_b
        let mut endpoint = Vec::new();
        endpoint.extend_from_slice(&0x0000_03B7u32.to_le_bytes());
        endpoint.extend_from_slice(&0x0000_0006u32.to_le_bytes());
        endpoint.extend_from_slice(&[0u8; 6]);
        endpoint.extend_from_slice(&0x0002u16.to_le_bytes());
        endpoint.extend_from_slice(&0x0000_02E4u32.to_le_bytes());
        endpoint.extend_from_slice(&[0x01, 0x00]);
        endpoint.extend_from_slice(&0x0000_008Bu32.to_le_bytes());
        data.extend_from_slice(&endpoint);
        data.extend_from_slice(&[0x44; 8]);
        let total = data.len() as u64;

        let pkg = pkg_with_streams(&[("/Sheet6", data)]);
        let report = byte_audit_report(&pkg);
        let summary = &report.per_stream["/Sheet6"];
        assert_eq!(summary.parser_name.as_deref(), Some("probe_sheet_stream"));
        // 26-byte endpoint record + ASCII text run must show up inside
        // the consumed total. We assert the lower bound rather than
        // exact bytes so the test stays robust if `sheet_probe`
        // tightens text-run heuristics in the future.
        assert!(
            summary.consumed_bytes >= 26,
            "expected at least the 26B endpoint record to be claimed; got {}",
            summary.consumed_bytes
        );
        assert!(summary.consumed_bytes < total);
    }

    #[test]
    fn overall_totals_match_sum_of_per_stream() {
        let pkg = pkg_with_streams(&[
            ("/PSMsegmenttable", make_psm_segment_bytes(4)),
            ("/UnknownStream", vec![0x00; 100]),
        ]);

        let report = byte_audit_report(&pkg);
        let sum_total: u64 = report.per_stream.values().map(|s| s.total_bytes).sum();
        let sum_consumed: u64 = report.per_stream.values().map(|s| s.consumed_bytes).sum();
        let sum_leftover: u64 = report.per_stream.values().map(|s| s.leftover_bytes).sum();
        assert_eq!(report.total_file_bytes, sum_total);
        assert_eq!(report.overall_consumed, sum_consumed);
        assert_eq!(report.overall_leftover, sum_leftover);
    }

    #[test]
    fn coverage_ratio_is_zero_for_empty_package_no_nan() {
        let pkg = pkg_with_streams(&[]);
        let report = byte_audit_report(&pkg);
        assert_eq!(report.overall_coverage_ratio, 0.0);
        assert!(!report.overall_coverage_ratio.is_nan());
        assert!(report.traces.is_empty());
        assert!(report.per_stream.is_empty());
        assert!(report.unregistered_paths.is_empty());
    }

    #[test]
    fn report_is_deterministic_across_insertion_orders() {
        let pkg1 = pkg_with_streams(&[
            ("/PSMsegmenttable", make_psm_segment_bytes(2)),
            ("/DocVersion2", make_doc_version2_bytes()),
        ]);
        let pkg2 = pkg_with_streams(&[
            ("/DocVersion2", make_doc_version2_bytes()),
            ("/PSMsegmenttable", make_psm_segment_bytes(2)),
        ]);

        let r1 = byte_audit_report(&pkg1);
        let r2 = byte_audit_report(&pkg2);

        // BTreeMap-backed `per_stream` must be the same regardless of
        // build order. `traces` also iterates via the same BTreeMap so
        // order matches.
        assert_eq!(r1.per_stream, r2.per_stream);
        assert_eq!(r1.traces, r2.traces);
    }

    #[test]
    fn unregistered_paths_are_sorted_alphabetically() {
        let pkg = pkg_with_streams(&[
            ("/Zeta", vec![0; 1]),
            ("/Alpha", vec![0; 1]),
            ("/Middle", vec![0; 1]),
        ]);
        let report = byte_audit_report(&pkg);
        assert_eq!(
            report.unregistered_paths,
            vec![
                "/Alpha".to_string(),
                "/Middle".to_string(),
                "/Zeta".to_string()
            ]
        );
    }

    #[test]
    fn parser_returning_none_still_produces_empty_trace() {
        // A valid magic but truncated record body → parse_psm_segment_table
        // returns None at the semantic layer, but the magic + count
        // bytes were already consumed so the trace reflects that.
        let mut data = Vec::new();
        data.extend_from_slice(&0x6261_7473u32.to_le_bytes()); // stab
        data.extend_from_slice(&10u32.to_le_bytes()); // claim 10 flags
        data.extend_from_slice(&[0x01, 0x02, 0x03]); // only 3 flags present

        let pkg = pkg_with_streams(&[("/PSMsegmenttable", data.clone())]);
        let report = byte_audit_report(&pkg);

        assert_eq!(report.traces.len(), 1, "trace produced even on parse None");
        let trace = &report.traces[0];
        // parser_with_trace short-circuits before consuming flags, so
        // only [0..4] (magic) + [4..8] (count) are consumed.
        assert_eq!(trace.consumed_bytes(), 8);
        assert_eq!(trace.leftover_bytes(), (data.len() - 8) as u64);
    }

    #[test]
    fn fully_consumed_stream_count_tracks_zero_leftover_traced_only() {
        // Fully-consumed traced stream + unregistered stream + traced stream
        // with leftover.
        let mut dv3 = Vec::new();
        for _ in 0..2 {
            let mut rec = Vec::new();
            let mut prod = b"SmartPlantPID.a".to_vec();
            prod.resize(16, 0);
            let mut ver = b"090000.0144".to_vec();
            ver.resize(12, 0);
            let mut op = b"SV".to_vec();
            op.resize(4, 0);
            let mut ts = b"01/01/26 00:00".to_vec();
            ts.resize(16, 0);
            rec.extend(prod);
            rec.extend(ver);
            rec.extend(op);
            rec.extend(ts);
            dv3.extend(rec);
        }
        // Add 4 trailing bytes to `/DocVersion3` so it is partially
        // consumed.
        dv3.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

        let pkg = pkg_with_streams(&[
            ("/PSMsegmenttable", make_psm_segment_bytes(2)), // full
            ("/DocVersion3", dv3),                           // partial
            ("/Mystery", vec![0; 32]),                       // unregistered
        ]);

        let report = byte_audit_report(&pkg);
        // Only the PSMsegmenttable stream counts: DocVersion3 has
        // trailing leftover (legacy `trailing_bytes` surfaces in trace),
        // Mystery has no parser.
        assert_eq!(report.fully_consumed_stream_count(), 1);
    }
}
