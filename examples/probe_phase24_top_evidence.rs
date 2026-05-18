//! Phase 24 Task 24-01 probe: dump per-fixture / per-sheet
//! `SheetCoordinatePageMetadataInvestigationReport.top_evidence`
//! entries as a markdown table for analysis.
//!
//! This probe is read-only and does not promote anything. It feeds
//! `docs/analysis/2026-05-18-phase24-coordinate-page-metadata-candidates.md`
//! by surfacing each top-evidence candidate's marker / range / numeric
//! shape so the Phase 24 Task 24-02 review checkpoint has a single
//! evidence table to argue from.
//!
//! Output is plain markdown printed to stdout; redirect to a file via
//! `cargo run --release --example probe_phase24_top_evidence > out.md`.

use std::path::Path;

use pid_parse::{
    build_normalized_geometry,
    parsers::{
        sheet_probe::{probe_sheet_stream, SheetProbeOptions},
        sheet_records::{
            coordinate_page_metadata_investigation_report, sheet_record_shape_inventory,
            SheetCoordinatePageMetadataCandidateKind,
        },
    },
    PidParser,
};

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/D06.pid",
];

#[derive(Debug, Clone)]
struct TopEvidenceRow {
    fixture: String,
    sheet_path: String,
    marker_type: Option<u16>,
    range_len: usize,
    support: usize,
    candidate_kind: SheetCoordinatePageMetadataCandidateKind,
    candidate_i32_pairs: usize,
    candidate_f64_pairs: usize,
    normalized_f64_pairs: usize,
    page_dimension_scalar_matches: usize,
    example_offset: usize,
    example_hex_prefix: String,
}

fn kind_label(kind: SheetCoordinatePageMetadataCandidateKind) -> &'static str {
    match kind {
        SheetCoordinatePageMetadataCandidateKind::PageDimensionScalarLike => {
            "PageDimensionScalarLike"
        }
        SheetCoordinatePageMetadataCandidateKind::NormalizedF64CoordinateLike => {
            "NormalizedF64CoordinateLike"
        }
        SheetCoordinatePageMetadataCandidateKind::I32CoordinateDomainLike => {
            "I32CoordinateDomainLike"
        }
        SheetCoordinatePageMetadataCandidateKind::MixedNumeric => "MixedNumeric",
        SheetCoordinatePageMetadataCandidateKind::InsufficientEvidence => "InsufficientEvidence",
    }
}

fn marker_label(marker: Option<u16>) -> String {
    match marker {
        Some(value) => format!("0x{value:04X} ({value})"),
        None => "—".to_string(),
    }
}

fn collect_rows() -> Vec<TopEvidenceRow> {
    let parser = PidParser::new();
    let mut rows = Vec::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            eprintln!("skipping: fixture {fixture} not found");
            continue;
        }

        let pkg = match parser.parse_package(path) {
            Ok(pkg) => pkg,
            Err(e) => {
                eprintln!("skipping: failed to parse {fixture}: {e}");
                continue;
            }
        };

        let field_xs: Vec<_> = pkg
            .parsed
            .object_graph
            .as_ref()
            .map(|graph| {
                graph
                    .objects
                    .iter()
                    .filter_map(|object| object.field_x)
                    .collect()
            })
            .unwrap_or_default();
        let normalized = build_normalized_geometry(&pkg.parsed);
        let page_dim = normalized.page_dimensions_mm;

        for sheet in pkg
            .parsed
            .sheet_streams
            .iter()
            .filter(|sheet| sheet.path.starts_with("/Sheet"))
        {
            let Some(raw_sheet) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let probe = probe_sheet_stream(
                &sheet.name,
                &sheet.path,
                &raw_sheet.data,
                &SheetProbeOptions::default(),
            );
            let inventory = sheet_record_shape_inventory(&raw_sheet.data, &probe, &field_xs);
            let report = coordinate_page_metadata_investigation_report(
                &raw_sheet.data,
                &inventory,
                page_dim,
            );

            for evidence in &report.top_evidence {
                rows.push(TopEvidenceRow {
                    fixture: fixture.to_string(),
                    sheet_path: sheet.path.clone(),
                    marker_type: evidence.marker_type,
                    range_len: evidence.range_len,
                    support: evidence.support,
                    candidate_kind: evidence.candidate_kind,
                    candidate_i32_pairs: evidence.candidate_i32_pairs,
                    candidate_f64_pairs: evidence.candidate_f64_pairs,
                    normalized_f64_pairs: evidence.normalized_f64_pairs,
                    page_dimension_scalar_matches: evidence.page_dimension_scalar_matches,
                    example_offset: evidence.example_offset,
                    example_hex_prefix: evidence.example_hex_prefix.clone(),
                });
            }
        }
    }

    rows
}

fn print_detail_table(rows: &[TopEvidenceRow]) {
    println!("## Per-fixture/per-sheet top evidence detail");
    println!();
    println!(
        "| # | Fixture | Sheet | Marker | Range | Support | Kind | i32 pairs | f64 pairs | Norm f64 | Page-dim | Offset | Hex prefix (first 32 bytes) |"
    );
    println!("|---|---|---|---|---:|---:|---|---:|---:|---:|---:|---|---|");
    for (idx, row) in rows.iter().enumerate() {
        let prefix_short = if row.example_hex_prefix.len() > 80 {
            format!("{}…", &row.example_hex_prefix[..78])
        } else {
            row.example_hex_prefix.clone()
        };
        println!(
            "| {} | `{}` | `{}` | {} | {} | {} | {} | {} | {} | {} | {} | 0x{:06X} | `{}` |",
            idx + 1,
            row.fixture
                .trim_start_matches("test-file/")
                .trim_start_matches("export-test/publish-data/"),
            row.sheet_path,
            marker_label(row.marker_type),
            row.range_len,
            row.support,
            kind_label(row.candidate_kind),
            row.candidate_i32_pairs,
            row.candidate_f64_pairs,
            row.normalized_f64_pairs,
            row.page_dimension_scalar_matches,
            row.example_offset,
            prefix_short,
        );
    }
}

fn print_marker_aggregate(rows: &[TopEvidenceRow]) {
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Agg {
        rows: usize,
        sheets: std::collections::BTreeSet<(String, String)>,
        total_support: usize,
        total_norm_f64: usize,
        total_page_dim: usize,
        kinds: BTreeMap<&'static str, usize>,
    }

    let mut by_marker: BTreeMap<Option<u16>, Agg> = BTreeMap::new();
    for row in rows {
        let agg = by_marker.entry(row.marker_type).or_default();
        agg.rows += 1;
        agg.sheets
            .insert((row.fixture.clone(), row.sheet_path.clone()));
        agg.total_support += row.support;
        agg.total_norm_f64 += row.normalized_f64_pairs;
        agg.total_page_dim += row.page_dimension_scalar_matches;
        *agg.kinds.entry(kind_label(row.candidate_kind)).or_insert(0) += 1;
    }

    println!();
    println!("## Per-marker_type aggregation across fixtures/sheets");
    println!();
    println!(
        "| Marker | Top-evidence rows | Distinct (fixture, sheet) | Σ support | Σ norm_f64 | Σ page-dim | Kind histogram |"
    );
    println!("|---|---:|---:|---:|---:|---:|---|");
    for (marker, agg) in &by_marker {
        let mut kinds_repr: Vec<String> =
            agg.kinds.iter().map(|(k, v)| format!("{k}={v}")).collect();
        kinds_repr.sort();
        println!(
            "| {} | {} | {} | {} | {} | {} | {} |",
            marker_label(*marker),
            agg.rows,
            agg.sheets.len(),
            agg.total_support,
            agg.total_norm_f64,
            agg.total_page_dim,
            kinds_repr.join(", "),
        );
    }
}

fn print_kind_summary(rows: &[TopEvidenceRow]) {
    use std::collections::BTreeMap;
    let mut hist: BTreeMap<&'static str, usize> = BTreeMap::new();
    for row in rows {
        *hist.entry(kind_label(row.candidate_kind)).or_insert(0) += 1;
    }
    println!();
    println!("## Candidate-kind distribution");
    println!();
    println!("| Kind | Top-evidence rows |");
    println!("|---|---:|");
    for (k, v) in &hist {
        println!("| {} | {} |", k, v);
    }
}

fn print_global_summary(rows: &[TopEvidenceRow]) {
    let total = rows.len();
    let with_page_dim_match: usize = rows
        .iter()
        .filter(|r| r.page_dimension_scalar_matches > 0)
        .count();
    let with_norm_f64: usize = rows.iter().filter(|r| r.normalized_f64_pairs > 0).count();
    let multi_fixture_markers: std::collections::BTreeSet<Option<u16>> = {
        use std::collections::BTreeMap;
        let mut per_marker_fixtures: BTreeMap<Option<u16>, std::collections::BTreeSet<String>> =
            BTreeMap::new();
        for row in rows {
            per_marker_fixtures
                .entry(row.marker_type)
                .or_default()
                .insert(row.fixture.clone());
        }
        per_marker_fixtures
            .into_iter()
            .filter_map(|(marker, set)| (set.len() >= 2).then_some(marker))
            .collect()
    };

    println!();
    println!("## Global summary");
    println!();
    println!("- total top_evidence rows: **{total}**");
    println!("- rows with `page_dimension_scalar_matches > 0`: **{with_page_dim_match}**");
    println!("- rows with `normalized_f64_pairs > 0`: **{with_norm_f64}**");
    println!(
        "- markers present in ≥ 2 fixtures: **{}** ({})",
        multi_fixture_markers.len(),
        multi_fixture_markers
            .iter()
            .map(|m| marker_label(*m))
            .collect::<Vec<_>>()
            .join(", ")
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("# Phase 24 Task 24-01 — `top_evidence` cross-fixture dump");
    println!();
    println!(
        "Generated by `cargo run --release --example probe_phase24_top_evidence`. \
         Source: `coordinate_page_metadata_investigation_report` per (fixture, /Sheet*) pair, \
         capped at 8 top-evidence entries per sheet."
    );

    let rows = collect_rows();
    print_global_summary(&rows);
    print_kind_summary(&rows);
    print_marker_aggregate(&rows);
    print_detail_table(&rows);
    Ok(())
}
