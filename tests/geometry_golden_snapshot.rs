//! Phase 0 safety net for the PSM-decoder deepening refactor
//! (`docs/plans/2026-07-16-psm-decoder-deepening-refactor-rfc-cn.md`).
//!
//! This golden snapshot pins the **entire** normalized geometry
//! projection — every [`pid_parse::PidGraphicEntity`] emitted by
//! [`pid_parse::build_normalized_geometry`] — for each locally
//! available `.pid` fixture. The refactor collapses the 11 per-family
//! `decode_*` scaffolds behind two seams (an L4 `PsmRecordDecoder`
//! trait and an L6 `GeometryEmitter` trait). None of those phases may
//! change observable output: this snapshot is the conservation truth
//! they are all measured against.
//!
//! Workflow (insta-style, no extra dependency):
//!
//! * Normal run — deserialize the committed golden JSON and assert the
//!   current projection matches it byte-for-byte (after canonical
//!   pretty serialization).
//! * `UPDATE_GEOMETRY_GOLDEN=1 cargo test --test geometry_golden_snapshot`
//!   — (re)write the golden files from current behaviour. Use this once
//!   to establish the baseline, and only afterwards when an
//!   **intentional** behaviour change is being blessed (call it out in
//!   the PR).
//! * Missing fixture — soft-skip (CI and contributors without
//!   `SmartPlant` samples), mirroring `parse_real_files.rs`.
//! * Fixture present but golden missing and no bless env — hard fail
//!   with instructions, so a genuinely new baseline is never silently
//!   skipped.

use pid_parse::build_normalized_geometry;
use std::path::{Path, PathBuf};

/// One fixture to snapshot. `slug` is the stable golden filename stem —
/// kept explicit (not derived from `path`) so non-ASCII fixtures map to
/// portable golden filenames.
struct GoldenCase {
    /// Fixture path relative to `test-file/`.
    path: &'static str,
    /// Golden filename stem under `tests/golden/geometry/`.
    slug: &'static str,
}

const GOLDEN_CASES: &[GoldenCase] = &[
    GoldenCase {
        path: "DWG-0201GP06-01.pid",
        slug: "dwg-0201gp06-01",
    },
    GoldenCase {
        path: "DWG-0202GP06-01.pid",
        slug: "dwg-0202gp06-01",
    },
    GoldenCase {
        path: "工艺管道及仪表流程-1.pid",
        slug: "gongyi-guandao-1",
    },
    GoldenCase {
        path: "D06.pid",
        slug: "d06",
    },
    GoldenCase {
        path: "export-test/publish-data/A01/A01.pid",
        slug: "publish-a01",
    },
    GoldenCase {
        path: "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
        slug: "publish-dwg-0202gp06-01",
    },
];

fn fixture_path(case: &GoldenCase) -> PathBuf {
    Path::new("test-file").join(case.path)
}

fn golden_path(case: &GoldenCase) -> PathBuf {
    Path::new("tests")
        .join("golden")
        .join("geometry")
        .join(format!("{}.json", case.slug))
}

fn bless_enabled() -> bool {
    std::env::var_os("UPDATE_GEOMETRY_GOLDEN").is_some_and(|v| v != "0" && !v.is_empty())
}

/// Canonical, deterministic serialization of the normalized geometry
/// projection for one fixture. Pretty-printed with a trailing newline so
/// the golden files diff cleanly in review.
fn render_snapshot(path: &Path) -> String {
    let doc = pid_parse::PidParser::new()
        .parse_file(path)
        .unwrap_or_else(|e| panic!("failed to parse fixture {}: {e}", path.display()));
    let geometry = build_normalized_geometry(&doc);
    let mut json = serde_json::to_string_pretty(&geometry.entities)
        .expect("normalized geometry entities are serializable");
    json.push('\n');
    json
}

#[test]
fn normalized_geometry_matches_golden_snapshot() {
    let mut checked = 0usize;
    let mut skipped: Vec<&str> = Vec::new();

    for case in GOLDEN_CASES {
        let fixture = fixture_path(case);
        if !fixture.exists() {
            skipped.push(case.path);
            continue;
        }

        let actual = render_snapshot(&fixture);
        let golden = golden_path(case);

        if bless_enabled() {
            if let Some(parent) = golden.parent() {
                std::fs::create_dir_all(parent)
                    .unwrap_or_else(|e| panic!("failed to create {}: {e}", parent.display()));
            }
            std::fs::write(&golden, actual.as_bytes())
                .unwrap_or_else(|e| panic!("failed to write golden {}: {e}", golden.display()));
            eprintln!("blessed golden: {}", golden.display());
            checked += 1;
            continue;
        }

        let expected = match std::fs::read_to_string(&golden) {
            Ok(contents) => contents.replace("\r\n", "\n"),
            Err(_) => panic!(
                "golden snapshot missing for fixture `{}` (expected at {}). \
                 Fixture is present, so this is a new baseline: run \
                 `UPDATE_GEOMETRY_GOLDEN=1 cargo test --test geometry_golden_snapshot` \
                 once to establish it, then commit the golden file.",
                case.path,
                golden.display()
            ),
        };

        pretty_assertions::assert_eq!(
            expected,
            actual,
            "normalized geometry projection for `{}` changed. If this is an \
             intentional behaviour change, re-bless with \
             UPDATE_GEOMETRY_GOLDEN=1 and call it out in the PR; otherwise it \
             is a refactor regression.",
            case.path
        );
        checked += 1;
    }

    eprintln!(
        "geometry golden snapshot: checked={checked}, skipped(missing)={:?}",
        skipped
    );

    assert!(
        checked > 0 || !skipped.is_empty(),
        "no golden cases registered"
    );
}
