//! What the 53-sheet plant backup leaves off its pages, pinned.
//!
//! `examples/probe_which_records_reach_no_page.rs` accounts for every record
//! of every place a page of the backup draws from — the drawings' `Sheet*`
//! streams, their `/JSite*/PSMcluster0` symbol caches, the `.sym` and `.igr`
//! members of the `RefData~*.zip` beside them — and found exactly one record
//! with ink that reached no page: `EF井场`'s solid dot, an `igBoundary2d` over
//! a circle. This pins that it now does, and pins the sheet census the parser
//! keeps for the 53 drawings, so a decoder that starts refusing a shape, or a
//! census walk that starts reading phantoms, shows up here. See
//! `docs/analysis/2026-11-07-every-undrawn-record-of-the-plant-backup-is-inkless.md`.
//!
//! Soft-skips when the backup is not on this machine, which is what CI sees.

use std::collections::BTreeMap;
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

/// Records no decoder claims, by type code, over every sheet stream of the
/// 53 drawings: `D04`'s one `ComplexString` — an association whose six
/// stroke members draw on their own — and the five `KeyPoint Constraint`s
/// it lists beside them.
const UNDECODED: &[(u16, usize)] = &[(0x0019, 5), (0x0021, 1)];

/// Records a decoder refused: one `DependencyObject` on `A井场` (36 bytes,
/// no member). The family draws nothing whether it decodes or not.
const REFUSED: &[(u16, usize)] = &[(0x00FA, 1)];

/// Records the native reader seeks past (`0x8000` in the type word). 23 of
/// them were hidden under census-walk phantoms until the census read the
/// streams' own chains.
const SKIPPED: &[(u16, usize)] = &[
    (0x0018, 46),
    (0x003D, 74),
    (0x004D, 640),
    (0x0059, 1),
    (0x005E, 163),
    (0x0084, 157),
    (0x00CE, 98),
    (0x00FA, 370),
];

/// The census the parser keeps for the backup's sheet streams is the one the
/// probe measures on their chains, and `EF井场`'s dot fills.
///
/// `EF井场`'s `/Sheet6` oid 52844 fills the `igCircle2d` oid 52772 at
/// `(0.8518786, 0.1303864)` m, r = 0.463 mm — a dot drawn by hand on the
/// sheet. Its `0x73` circle member and drawing-side trailer under sub-type
/// `0x0000` made the boundary decoder refuse it, so the page stroked the
/// circle and never filled it.
#[test]
fn the_backup_census_is_its_chains_and_the_ef_dot_fills() {
    let Some(backup) = backup_dir() else {
        eprintln!("skipping: no backup-plant-data directory is present");
        return;
    };
    let mut pids = Vec::new();
    collect_pids(&backup, &mut pids);
    let mut undecoded: BTreeMap<u16, usize> = BTreeMap::new();
    let mut refused: BTreeMap<u16, usize> = BTreeMap::new();
    let mut skipped: BTreeMap<u16, usize> = BTreeMap::new();
    let mut refused_boundaries = Vec::new();
    let mut ef_ring = None;
    for path in &pids {
        let doc = PidParser::new()
            .parse_file(path)
            .unwrap_or_else(|error| panic!("{} does not parse: {error}", path.display()));
        for sheet in &doc.sheet_streams {
            let Some(geometry) = sheet.geometry.as_ref() else {
                continue;
            };
            for row in &geometry.undecoded_type_codes {
                *undecoded.entry(row.type_code).or_default() += row.count;
            }
            for row in &geometry.refused_records {
                *refused.entry(row.type_code).or_default() += row.count;
                if row.type_code == 0x0013 {
                    refused_boundaries.push(format!("{} {}", path.display(), sheet.path));
                }
            }
            for row in &geometry.skipped_records {
                *skipped.entry(row.type_code).or_default() += row.count;
            }
        }
        if path
            .file_stem()
            .is_some_and(|stem| stem.to_string_lossy().starts_with("EF井场"))
        {
            let geometry = build_normalized_geometry(&doc);
            ef_ring = geometry.entities.iter().find_map(|entity| {
                let PidGraphicKind::Polyline { points, closed } = &entity.kind else {
                    return None;
                };
                (entity.graphic_oid == Some(52844)).then(|| (points.clone(), *closed))
            });
        }
    }
    assert!(
        pids.len() >= 53,
        "{} drawings under {}",
        pids.len(),
        backup.display()
    );
    assert_eq!(
        undecoded,
        UNDECODED.iter().copied().collect(),
        "undecoded records by type code"
    );
    assert_eq!(
        refused,
        REFUSED.iter().copied().collect(),
        "refused records by type code"
    );
    assert_eq!(
        skipped,
        SKIPPED.iter().copied().collect(),
        "records the native reader skips, by type code"
    );
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
