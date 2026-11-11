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
/// 53 drawings: the five `KeyPoint Constraint`s of `D04`'s one
/// `ComplexString`. A constraint is not a graphic object — the native
/// graphic predicate turns its type code away — and what it constrains, one
/// curve's end to the next one's start, the curves' own coordinates already
/// state.
///
/// The `ComplexString` itself (`0x0021`, one record) was on this list until
/// the family decoded.
const UNDECODED: &[(u16, usize)] = &[(0x0019, 5)];

/// The `ComplexString` records on the 53 drawings' sheets, as `(drawing,
/// oid, curve members, members)`: `D04`'s `/Sheet6` oid 62, a closed run of
/// four arcs and two lines — 2.6 mm by 7.0 mm at (161.6, 213.5) mm — with
/// the five constraints that join them.
const COMPLEX_STRINGS: &[(&str, u32, usize, usize)] = &[("D04", 62, 6, 11)];

/// Records a decoder refused: none.
///
/// There was one, a `DependencyObject` on `A井场` (`/Sheet6 +251219`, oid
/// 46838) that lists no member: 36 bytes of the family's own grammar, which
/// the decoder turned away for requiring a count of at least one.
const REFUSED: &[(u16, usize)] = &[];

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

/// `igSmartFrame2d` records on the 53 drawings' sheet chains: one page frame
/// on each of 49 sheets, two on `Q井场`, none on the three sheets that hold no
/// border.
///
/// The parser used to report 61. The other ten started inside a text box, a
/// symbol placement or a dependency object, wherever a field's value was 61
/// (`3D 00 00 00`, the family's type code) and what followed read as a
/// length — each claiming 65 to 394 KiB of the stream. See
/// `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
/// §3.3.
const SMART_FRAMES: usize = 51;

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
    let mut smart_frames = 0usize;
    // Records of the two type codes no sheet chain carries (`0x0010`,
    // `0x0030`), counted on the chains themselves: the three families that
    // used to read them are retired.
    let mut chainless = 0usize;
    let mut complex_strings: Vec<(String, u32, usize, usize)> = Vec::new();
    let mut groups = 0usize;
    for path in &pids {
        let package = PidParser::new()
            .parse_package(path)
            .unwrap_or_else(|error| panic!("{} does not parse: {error}", path.display()));
        let doc = &package.parsed;
        for sheet in &doc.sheet_streams {
            if let Some(raw) = package.streams.get(&sheet.path) {
                use pid_parse::parsers::sheet_records::{parse_psm_header, SheetChain};
                chainless += SheetChain::walk(&raw.data)
                    .starts
                    .iter()
                    .filter(|&&at| {
                        parse_psm_header(&raw.data, at).is_some_and(|header| {
                            header.type_code == 0x0010 || header.type_code == 0x0030
                        })
                    })
                    .count();
            }
            let Some(geometry) = sheet.geometry.as_ref() else {
                continue;
            };
            smart_frames += geometry.decoded_igsmartframes.len();
            groups += geometry.decoded_iggroups.len();
            for string in &geometry.decoded_igcomplexstrings {
                // A string states no coordinate; each curve it lists is a
                // line or an arc of this sheet that names it as its owner,
                // on its layer, in its style.
                for oid in string.curve_oids() {
                    let line = geometry
                        .decoded_iglines
                        .iter()
                        .find(|line| line.oid == oid)
                        .map(|line| (line.parent_ref, line.sheet_layer_ref, line.index));
                    let arc = geometry
                        .decoded_igarcs
                        .iter()
                        .find(|arc| arc.oid == oid)
                        .map(|arc| (arc.parent_ref, arc.sheet_layer_ref, arc.index));
                    assert_eq!(
                        line.or(arc),
                        Some((string.oid, string.sheet_layer_ref, string.index)),
                        "{} {}: curve member {oid} of the ComplexString oid {}",
                        path.display(),
                        sheet.path,
                        string.oid
                    );
                }
                complex_strings.push((
                    path.file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    string.oid,
                    string.curve_oids().count(),
                    string.members.len(),
                ));
            }
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
            let geometry = build_normalized_geometry(doc);
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
        complex_strings,
        COMPLEX_STRINGS
            .iter()
            .map(|&(drawing, oid, curves, members)| (drawing.to_string(), oid, curves, members))
            .collect::<Vec<_>>(),
        "ComplexString records on the sheets' chains"
    );
    // The members that are not curves are the constraints the census counts:
    // nothing else on the 53 sheets goes undecoded.
    assert_eq!(
        complex_strings
            .iter()
            .map(|(_, _, curves, members)| members - curves)
            .sum::<usize>(),
        undecoded.get(&0x0019).copied().unwrap_or(0),
        "a ComplexString's constraint members against the undecoded KeyPoint Constraints"
    );
    assert_eq!(groups, 0, "Group records on the sheets' chains");
    assert_eq!(
        skipped,
        SKIPPED.iter().copied().collect(),
        "records the native reader skips, by type code"
    );
    assert!(
        refused_boundaries.is_empty(),
        "igBoundary2d refused on {refused_boundaries:?}"
    );
    assert_eq!(
        smart_frames, SMART_FRAMES,
        "igSmartFrame2d records on the sheets' chains"
    );
    // 72 010 before a family read the chain: 51 692 "0x0010 sub-records",
    // 9 626 "attribute fragments" and 10 649 "JStyleOverride", none of which
    // started a record.
    assert_eq!(
        chainless, 0,
        "records of 0x0010 / 0x0030 on the chains of the backup's sheets"
    );
    // What was held, in the log: a run that skipped prints the line above
    // and not this one.
    eprintln!(
        "plant backup census: {} drawings under {}; undecoded {undecoded:?}, refused {refused:?}, \
         {} native-skipped over {} type codes, {smart_frames} igSmartFrame2d, {chainless} of \
         0x0010 / 0x0030, {} ComplexString, {groups} Group",
        pids.len(),
        backup.display(),
        skipped.values().sum::<usize>(),
        skipped.len(),
        complex_strings.len()
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
