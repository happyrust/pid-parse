//! Phase 25-A Slice A probe: dump per-fixture / per-sheet spatial
//! cluster distribution of `normalized_f64_pair` evidence.
//!
//! This probe is read-only and does **not** promote anything. It feeds
//! `docs/analysis/2026-05-23-phase25-f64-spatial-distribution.md` by
//! surfacing each (fixture, /Sheet*) pair's:
//!
//! - `pair_count`: total normalized f64 pairs found
//! - `bbox`: bounding box of pairs in normalized [0, 1]² space
//! - `cluster_count`: number of clusters after N×N grid bucketing +
//!   adjacent-cell merge (default N=20)
//! - `largest_cluster_size`: pairs in the biggest cluster
//! - `median_cluster_size`: median pair count across clusters
//! - `ascii_heatmap`: 20×20 ASCII grid showing pair density
//!
//! Algorithm is deterministic (grid offset = floor(x * N) / floor(y * N))
//! and panic-safe (bounded slice reads, no unwrap).
//!
//! Output is plain markdown to stdout; redirect via:
//! `cargo run --release --example probe_phase25_f64_spatial > out.md`.
//!
//! Reads no Phase 23 / Phase 24 invariants; the goal is to characterise
//! the spatial signal so Slice B can decide between full DTO landing
//! and negative closeout.

use std::collections::BTreeSet;
use std::path::Path;

use pid_parse::{
    parsers::{
        sheet_probe::{probe_sheet_stream, SheetProbeOptions},
        sheet_records::sheet_record_shape_inventory,
    },
    PidDocument, PidParser,
};

const FIXTURES: &[&str] = &[
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
    "test-file/D06.pid",
];

const GRID_N: usize = 20;
const HEATMAP_THRESHOLDS: [(usize, char); 5] = [(0, '·'), (1, '░'), (4, '▒'), (12, '▓'), (36, '█')];

#[derive(Debug, Clone)]
struct SpatialReport {
    fixture: String,
    sheet_path: String,
    pair_count: usize,
    structured_pair_count: usize,
    bbox: Option<((f64, f64), (f64, f64))>,
    cluster_count: usize,
    largest_cluster_size: usize,
    median_cluster_size: usize,
    grid_counts: Vec<Vec<usize>>,
}

fn normalized_coordinate(value: f64) -> bool {
    value.is_finite() && (-1.0e-9..=1.0 + 1.0e-9).contains(&value)
}

fn normalized_pair(x: f64, y: f64) -> bool {
    normalized_coordinate(x) && normalized_coordinate(y) && (x.abs() > 1.0e-12 || y.abs() > 1.0e-12)
}

fn scan_normalized_pairs_exhaustive(data: &[u8]) -> Vec<(f64, f64)> {
    let mut pairs = Vec::new();
    if data.len() < 16 {
        return pairs;
    }
    for (relative_offset, window) in data.windows(16).enumerate() {
        if relative_offset % 4 != 0 {
            continue;
        }
        let Some(x_bytes) = window.get(0..8) else {
            continue;
        };
        let Some(y_bytes) = window.get(8..16) else {
            continue;
        };
        let x = f64::from_le_bytes([
            x_bytes[0], x_bytes[1], x_bytes[2], x_bytes[3], x_bytes[4], x_bytes[5], x_bytes[6],
            x_bytes[7],
        ]);
        let y = f64::from_le_bytes([
            y_bytes[0], y_bytes[1], y_bytes[2], y_bytes[3], y_bytes[4], y_bytes[5], y_bytes[6],
            y_bytes[7],
        ]);
        if normalized_pair(x, y) {
            pairs.push((x, y));
        }
    }
    pairs
}

fn scan_normalized_pairs_structured(
    data: &[u8],
    pkg_parsed: &PidDocument,
    sheet_path: &str,
) -> Vec<(f64, f64)> {
    let Some(sheet) = pkg_parsed
        .sheet_streams
        .iter()
        .find(|s| s.path == sheet_path)
    else {
        return Vec::new();
    };
    let probe = probe_sheet_stream(
        &sheet.name,
        &sheet.path,
        data,
        &SheetProbeOptions::default(),
    );
    let field_xs: Vec<u32> = pkg_parsed
        .object_graph
        .as_ref()
        .map(|graph| graph.objects.iter().filter_map(|o| o.field_x).collect())
        .unwrap_or_default();
    let inventory = sheet_record_shape_inventory(data, &probe, &field_xs);

    let mut pairs = Vec::new();
    for offset in inventory
        .records
        .iter()
        .filter_map(|record| record.f64_coordinate_offset)
        .collect::<BTreeSet<_>>()
    {
        let Some(window) = data.get(offset..offset + 16) else {
            continue;
        };
        let x = f64::from_le_bytes([
            window[0], window[1], window[2], window[3], window[4], window[5], window[6], window[7],
        ]);
        let y = f64::from_le_bytes([
            window[8], window[9], window[10], window[11], window[12], window[13], window[14],
            window[15],
        ]);
        if normalized_pair(x, y) {
            pairs.push((x, y));
        }
    }
    pairs
}

fn bounding_box(pairs: &[(f64, f64)]) -> Option<((f64, f64), (f64, f64))> {
    let mut iter = pairs.iter().copied();
    let first = iter.next()?;
    let mut min_x = first.0;
    let mut max_x = first.0;
    let mut min_y = first.1;
    let mut max_y = first.1;
    for (x, y) in iter {
        if x < min_x {
            min_x = x;
        }
        if x > max_x {
            max_x = x;
        }
        if y < min_y {
            min_y = y;
        }
        if y > max_y {
            max_y = y;
        }
    }
    Some(((min_x, min_y), (max_x, max_y)))
}

fn grid_bucket(pairs: &[(f64, f64)], n: usize) -> Vec<Vec<usize>> {
    let mut grid = vec![vec![0usize; n]; n];
    if n == 0 {
        return grid;
    }
    for (x, y) in pairs.iter().copied() {
        let cx = ((x.clamp(0.0, 1.0) * n as f64) as usize).min(n - 1);
        let cy = ((y.clamp(0.0, 1.0) * n as f64) as usize).min(n - 1);
        grid[cy][cx] = grid[cy][cx].saturating_add(1);
    }
    grid
}

fn connected_clusters(grid: &[Vec<usize>]) -> Vec<usize> {
    let h = grid.len();
    if h == 0 {
        return Vec::new();
    }
    let w = grid[0].len();
    let mut visited = vec![vec![false; w]; h];
    let mut sizes = Vec::new();

    for sy in 0..h {
        for sx in 0..w {
            if visited[sy][sx] || grid[sy][sx] == 0 {
                continue;
            }
            let mut stack = vec![(sx, sy)];
            let mut size = 0usize;
            while let Some((x, y)) = stack.pop() {
                if visited[y][x] || grid[y][x] == 0 {
                    continue;
                }
                visited[y][x] = true;
                size = size.saturating_add(grid[y][x]);
                if x > 0 {
                    stack.push((x - 1, y));
                }
                if x + 1 < w {
                    stack.push((x + 1, y));
                }
                if y > 0 {
                    stack.push((x, y - 1));
                }
                if y + 1 < h {
                    stack.push((x, y + 1));
                }
            }
            sizes.push(size);
        }
    }

    sizes
}

fn median(values: &[usize]) -> usize {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

fn density_glyph(count: usize) -> char {
    let mut glyph = '·';
    for &(threshold, ch) in &HEATMAP_THRESHOLDS {
        if count >= threshold {
            glyph = ch;
        }
    }
    glyph
}

fn collect_reports() -> Vec<SpatialReport> {
    let parser = PidParser::new();
    let mut reports = Vec::new();
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
        for sheet in pkg
            .parsed
            .sheet_streams
            .iter()
            .filter(|sheet| sheet.path.starts_with("/Sheet"))
        {
            let Some(raw_sheet) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let pairs = scan_normalized_pairs_exhaustive(&raw_sheet.data);
            let structured =
                scan_normalized_pairs_structured(&raw_sheet.data, &pkg.parsed, &sheet.path);
            let bbox = bounding_box(&pairs);
            let grid = grid_bucket(&pairs, GRID_N);
            let cluster_sizes = connected_clusters(&grid);
            let cluster_count = cluster_sizes.len();
            let largest = cluster_sizes.iter().copied().max().unwrap_or(0);
            let med = median(&cluster_sizes);
            reports.push(SpatialReport {
                fixture: fixture.to_string(),
                sheet_path: sheet.path.clone(),
                pair_count: pairs.len(),
                structured_pair_count: structured.len(),
                bbox,
                cluster_count,
                largest_cluster_size: largest,
                median_cluster_size: med,
                grid_counts: grid,
            });
        }
    }
    reports
}

fn format_bbox(bbox: Option<((f64, f64), (f64, f64))>) -> String {
    match bbox {
        Some(((min_x, min_y), (max_x, max_y))) => {
            format!("[{min_x:.3}, {min_y:.3}] → [{max_x:.3}, {max_y:.3}]")
        }
        None => "—".to_string(),
    }
}

fn print_per_sheet_summary(reports: &[SpatialReport]) {
    println!("## Per-fixture/per-sheet spatial summary");
    println!();
    println!("| # | Fixture | Sheet | Pair count (exhaustive) | Pair count (structured) | Cluster count | Largest cluster | Median cluster | Bounding box (normalized) |");
    println!("|---|---|---|---:|---:|---:|---:|---:|---|");
    for (idx, report) in reports.iter().enumerate() {
        println!(
            "| {} | `{}` | `{}` | {} | {} | {} | {} | {} | {} |",
            idx + 1,
            report
                .fixture
                .trim_start_matches("test-file/")
                .trim_start_matches("export-test/publish-data/"),
            report.sheet_path,
            report.pair_count,
            report.structured_pair_count,
            report.cluster_count,
            report.largest_cluster_size,
            report.median_cluster_size,
            format_bbox(report.bbox),
        );
    }
}

fn print_global_summary(reports: &[SpatialReport]) {
    let total_pairs_exhaustive: usize = reports.iter().map(|r| r.pair_count).sum();
    let total_pairs_structured: usize = reports.iter().map(|r| r.structured_pair_count).sum();
    let total_clusters: usize = reports.iter().map(|r| r.cluster_count).sum();
    let sheets_with_zero_pairs = reports.iter().filter(|r| r.pair_count == 0).count();
    let sheets_with_single_cluster = reports
        .iter()
        .filter(|r| r.pair_count > 0 && r.cluster_count <= 1)
        .count();
    let multi_cluster_sheets = reports.iter().filter(|r| r.cluster_count >= 2).count();

    println!();
    println!("## Global summary");
    println!();
    println!("- Sheets analyzed: **{}**", reports.len());
    println!("- Total normalized f64 pairs (exhaustive scan): **{total_pairs_exhaustive}**");
    println!(
        "- Total normalized f64 pairs (structured = inventory.f64_coordinate_offset): **{total_pairs_structured}**"
    );
    println!("- Total clusters across all sheets: **{total_clusters}**");
    println!("- Sheets with 0 pairs: **{sheets_with_zero_pairs}**");
    println!(
        "- Sheets with 1 cluster (or 0): **{}** (potential negative signal if dominant)",
        sheets_with_single_cluster + sheets_with_zero_pairs
    );
    println!(
        "- Sheets with ≥2 clusters: **{multi_cluster_sheets}** (positive signal for Phase 25-A)"
    );
}

fn print_heatmaps(reports: &[SpatialReport]) {
    println!();
    println!("## Per-sheet ASCII heatmaps (20×20 grid, normalized [0, 1]² space)");
    println!();
    println!("Density glyphs: `·` = 0, `░` = 1-3, `▒` = 4-11, `▓` = 12-35, `█` = ≥36.");
    println!();
    for (idx, report) in reports.iter().enumerate() {
        if report.pair_count == 0 {
            continue;
        }
        println!();
        println!(
            "### #{} `{}` `{}` ({} pairs, {} clusters)",
            idx + 1,
            report
                .fixture
                .trim_start_matches("test-file/")
                .trim_start_matches("export-test/publish-data/"),
            report.sheet_path,
            report.pair_count,
            report.cluster_count,
        );
        println!();
        println!("```");
        println!("(0,1)                                       (1,1)");
        for (row_idx, row) in report.grid_counts.iter().enumerate().rev() {
            let mut line = String::with_capacity(row.len());
            for cell in row {
                line.push(density_glyph(*cell));
                line.push(' ');
            }
            let _ = row_idx;
            println!("{line}");
        }
        println!("(0,0)                                       (1,0)");
        println!("```");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("# Phase 25-A Slice A — normalized_f64_pair spatial distribution probe");
    println!();
    println!(
        "Generated by `cargo run --release --example probe_phase25_f64_spatial`. \
         Source: exhaustive 16-byte sliding-window scan of /Sheet* bytes restricted \
         to `(x, y)` pairs where each f64 is finite and ∈ `[-1e-9, 1 + 1e-9]` and at \
         least one of `x`/`y` is non-zero. Cluster algorithm: 20×20 grid bucketing \
         in normalized `[0, 1]²` space + connected-component merge (4-neighbour). \
         Read-only; does not promote anything."
    );
    let reports = collect_reports();
    print_global_summary(&reports);
    print_per_sheet_summary(&reports);
    print_heatmaps(&reports);
    Ok(())
}
