//! Criterion micro-benchmarks for the three hot paths of `pid-parse`.
//!
//! Scenarios (all gated on the local `test-file/` fixture — missing
//! fixtures cause the benchmark to print a soft-skip notice and register
//! nothing, so `cargo bench` still completes on machines / CI runners
//! without `SmartPlant` samples):
//!
//! | Scenario             | What it measures                                                   |
//! | -------------------- | ------------------------------------------------------------------ |
//! | `parse_pid_a01`      | Cold `.pid` → [`PidDocument`] via [`PidParser::parse_file`]        |
//! | `load_mdf_a01`       | `Export.mdf` → [`PublishDrawing`] via `load_drawing_graph_from_mdf`|
//! | `write_data_xml_a01` | Pre-loaded [`PublishDrawing`] → `<PIDDrawing>` XML (writer only)   |
//!
//! Baseline numbers live in `CHANGELOG.md` under "Performance baseline".
//! This bench is not (yet) wired into CI: criterion numbers drift too
//! much across machines to act as a hard gate, but they are very useful
//! as a local regression signal around refactors and dependency bumps.

use std::hint::black_box;
use std::path::Path;

use criterion::{criterion_group, criterion_main, Criterion};
use pid_parse::parsers::sheet_probe::{probe_sheet_stream, SheetProbeOptions};
use pid_parse::publish::{load_drawing_graph_from_mdf, write_data_xml, PublishDrawing};
use pid_parse::PidParser;

// The A01 fixture is the same one integration tests use. Keep the
// paths in sync with `tests/common/mod.rs`; if either fixture moves,
// update both places.
const A01_PID: &str = "test-file/export-test/publish-data/A01/A01.pid";
const A01_MDF: &str = "test-file/backup-test/TEST02_p/extracted/Export.mdf";
const A01_DRAWING_UID: &str = "D9635C3C898840D1990B7E8BEE1D55DA";
const A01_PLANT_NAME: &str = "TEST02";

// Richest local fixture by decoded-geometry volume. M5-B1 used it to
// quantify the historical duplicate Sheet probe; M5-B2 keeps the same
// scenarios as a before/after regression signal after the reader starts
// carrying the first pass's chunk evidence into geometry-hint scoring.
const GONGYI_PID: &str = "test-file/工艺管道及仪表流程-1.pid";

fn bench_parse_pid(c: &mut Criterion) {
    let path = Path::new(A01_PID);
    if !path.exists() {
        eprintln!("bench skip: {A01_PID} not found (SmartPlant fixture)");
        return;
    }
    c.bench_function("parse_pid_a01", |b| {
        b.iter(|| {
            let doc = PidParser::new()
                .parse_file(black_box(path))
                .expect("A01.pid should parse");
            black_box(doc);
        });
    });
}

fn bench_load_mdf(c: &mut Criterion) {
    let path = Path::new(A01_MDF);
    if !path.exists() {
        eprintln!("bench skip: {A01_MDF} not found (SmartPlant fixture)");
        return;
    }
    c.bench_function("load_mdf_a01", |b| {
        b.iter(|| {
            let drawing = load_drawing_graph_from_mdf(black_box(path), black_box(A01_DRAWING_UID))
                .expect("A01 drawing graph should load");
            black_box(drawing);
        });
    });
}

fn bench_write_data_xml(c: &mut Criterion) {
    let path = Path::new(A01_MDF);
    if !path.exists() {
        eprintln!("bench skip: {A01_MDF} not found (SmartPlant fixture)");
        return;
    }
    // Load once outside the hot loop so we measure the writer in
    // isolation from the MDF reader (which has its own benchmark).
    let drawing: PublishDrawing = load_drawing_graph_from_mdf(path, A01_DRAWING_UID)
        .expect("A01 drawing graph should load for writer bench setup");

    c.bench_function("write_data_xml_a01", |b| {
        b.iter(|| {
            let xml = write_data_xml(black_box(&drawing), black_box(A01_PLANT_NAME))
                .expect("A01 _Data.xml should write");
            black_box(xml);
        });
    });
}

fn bench_parse_pid_gongyi(c: &mut Criterion) {
    let path = Path::new(GONGYI_PID);
    if !path.exists() {
        eprintln!("bench skip: {GONGYI_PID} not found (SmartPlant fixture)");
        return;
    }
    c.bench_function("parse_pid_gongyi", |b| {
        b.iter(|| {
            let doc = PidParser::new()
                .parse_file(black_box(path))
                .expect("gongyi fixture should parse");
            black_box(doc);
        });
    });
}

/// M5 evidence benchmark: cost of one full `probe_sheet_stream` pass over
/// every `Sheet*` stream. PR-B1 used the ratio against `parse_pid_gongyi`
/// as the decision gate; PR-B2 retains it to document the eliminated work.
fn bench_probe_sheet_streams(c: &mut Criterion) {
    let path = Path::new(GONGYI_PID);
    if !path.exists() {
        eprintln!("bench skip: {GONGYI_PID} not found (SmartPlant fixture)");
        return;
    }
    // Collect sheet stream bytes once outside the hot loop via the
    // package path (raw bytes retained).
    let pkg = pid_parse::PidPackage::from_path(path).expect("gongyi package should parse");
    let sheets: Vec<(String, Vec<u8>)> = pkg
        .parsed
        .sheet_streams
        .iter()
        .filter_map(|sheet| {
            pkg.streams
                .get(&sheet.path)
                .map(|raw| (sheet.path.clone(), raw.data.clone()))
        })
        .collect();
    if sheets.is_empty() {
        eprintln!("bench skip: gongyi fixture exposes no sheet streams");
        return;
    }

    c.bench_function("probe_sheet_streams_gongyi", |b| {
        b.iter(|| {
            for (sheet_path, bytes) in &sheets {
                let name = sheet_path.rsplit('/').next().unwrap_or("Sheet");
                let report = probe_sheet_stream(
                    black_box(name),
                    black_box(sheet_path),
                    black_box(bytes),
                    &SheetProbeOptions::default(),
                );
                black_box(report);
            }
        });
    });
}

criterion_group!(
    pid_pipeline,
    bench_parse_pid,
    bench_load_mdf,
    bench_write_data_xml,
    bench_parse_pid_gongyi,
    bench_probe_sheet_streams
);
criterion_main!(pid_pipeline);
