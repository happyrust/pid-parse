//! What the 53-sheet plant backup leaves off its pages, pinned.
//!
//! `examples/probe_which_records_reach_no_page.rs` accounts for every record
//! of every place a page of the backup draws from — the drawings' `Sheet*`
//! streams, their `/JSite*/PSMcluster0` symbol caches, the `.sym` and `.igr`
//! members of the `RefData~*.zip` beside them — and found exactly one record
//! with ink that reached no page: `EF井场`'s solid dot, an `igBoundary2d` over
//! a circle. This pins that it now does, so the sheet census of the backup
//! refuses no boundary at all. See
//! `docs/analysis/2026-11-07-every-undrawn-record-of-the-plant-backup-is-inkless.md`.
//!
//! Soft-skips when the backup is not on this machine, which is what CI sees.

use std::path::{Path, PathBuf};

use pid_parse::{build_normalized_geometry, PidGraphicKind, PidParser};

/// The extracted plant backup, if this machine has one: under
/// `PID_TEST_DATA_DIR`, then the two local copies of `pid-test-data`.
fn backup_dir() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
    roots.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
    roots
        .into_iter()
        .map(|root| root.join("backup-plant-data"))
        .find(|candidate| candidate.is_dir())
}

fn collect_pids(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_pids(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "pid") {
            out.push(path);
        }
    }
}

/// `EF井场`'s `/Sheet6` oid 52844 fills the `igCircle2d` oid 52772 at
/// `(0.8518786, 0.1303864)` m, r = 0.463 mm — a dot drawn by hand on the
/// sheet. Its `0x73` circle member and drawing-side trailer under sub-type
/// `0x0000` made the boundary decoder refuse it, so the page stroked the
/// circle and never filled it.
#[test]
fn the_backup_refuses_no_boundary_and_fills_the_ef_dot() {
    let Some(backup) = backup_dir() else {
        eprintln!("skipping: no backup-plant-data directory is present");
        return;
    };
    let mut pids = Vec::new();
    collect_pids(&backup, &mut pids);
    let mut refused_boundaries = Vec::new();
    let mut ef_ring = None;
    for path in &pids {
        let doc = PidParser::new()
            .parse_file(path)
            .unwrap_or_else(|error| panic!("{} does not parse: {error}", path.display()));
        let geometry = build_normalized_geometry(&doc);
        refused_boundaries.extend(
            geometry
                .refused_graphic_records
                .iter()
                .filter(|record| record.type_code == 0x0013)
                .map(|record| format!("{} {}", path.display(), record.stream_path)),
        );
        if path
            .file_stem()
            .is_some_and(|stem| stem.to_string_lossy().starts_with("EF井场"))
        {
            ef_ring = geometry.entities.iter().find_map(|entity| {
                let PidGraphicKind::Polyline { points, closed } = &entity.kind else {
                    return None;
                };
                (entity.graphic_oid == Some(52844)).then(|| (points.clone(), *closed))
            });
        }
    }
    assert!(
        refused_boundaries.is_empty(),
        "igBoundary2d refused on {refused_boundaries:?}"
    );
    let Some((points, closed)) = ef_ring else {
        panic!("EF井场's oid 52844 projects no boundary ring");
    };
    assert!(closed);
    assert_eq!(points.len(), 26, "a whole turn in 0.25 rad chords");
    let centre = (0.851_878_6, 0.130_386_4);
    for point in &points {
        let radius = (point.x - centre.0).hypot(point.y - centre.1);
        assert!(
            (radius - 0.000_463_1).abs() < 1e-6,
            "a ring vertex {radius} m from the dot's centre"
        );
    }
}
