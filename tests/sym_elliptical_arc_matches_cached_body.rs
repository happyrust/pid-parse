//! A `.sym` body's `0x007E igEllipticalArc2d` dome and the same dome a
//! drawing's own cached body already draws must trace the identical
//! rational B-spline.
//!
//! `Diaphragm Actuator.sym` (`Instrumentation/Actuators/Diaphragm
//! Actuator.sym` in `RefData~4~681.zip`) carries one `0x007E` record
//! (`/Sheet6` oid 429): the dome bulging out of the actuator's housing.
//! The `I井场` backup drawing places the same symbol and caches its body
//! at `/JSite37311` (`oid 79`, the only elliptical arc that `JSite`
//! holds) -- the drawing's own copy of the identical vendor record,
//! read off a different file. `symbol_library::read_symbol_geometry`'s
//! `TYPE_ELLIPTICALARC` arm and `geometry.rs`'s cached-body path both
//! call `bspline::elliptical_arc_directed` with the same five
//! arguments, so if the two records' fields agree, their B-splines must
//! agree bit-for-bit -- there is no separate "drawing" formula and
//! "`.sym`" formula to drift apart.
//!
//! Soft-skips when either fixture is absent, like `elliptical_arcs.rs`.
//!
//! Reads `RefData~4~681.zip` straight out of the backup package, so the
//! whole file is gated on the `refdata-archive` feature the same way
//! `src/refdata_archive.rs` is; it compiles to nothing without it rather
//! than breaking a `cargo test -p pid-parse` run that opts out of the
//! `zip` dependency (the workspace gates still build it, since `pid-plot`
//! requests the feature on the shared `pid-parse` dependency).
#![cfg(feature = "refdata-archive")]

use pid_parse::parsers::sheet_records::PSM_TYPE_CODE_IGELLIPTICALARC2D;
use pid_parse::symbol_library::{read_symbol_geometry_from_bytes, SymbolPrimitive};
use pid_parse::{build_normalized_geometry, PidParser};
use std::path::PathBuf;

/// The drawing that places `Diaphragm Actuator.sym` and caches its body.
const DRAWING: &str = "I井场 注采阀组工艺及自控流程图.pid";

/// `JSite` storage the drawing caches the actuator's body at.
const CACHED_SITE_PATH: &str = "/JSite37311";

/// Member name inside `RefData~4~681.zip`.
const SYM_MEMBER: &str = "Instrumentation/Actuators/Diaphragm Actuator.sym";

/// Where the shelled-out plant backup sits, if this machine has one —
/// same search order `pid-plot/tests/export_plant_backup.rs` uses.
fn backup_data_dir() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("PID_TEST_DATA_DIR") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(PathBuf::from(r"D:\work\cad\pid-test-data"));
    roots.push(PathBuf::from(r"D:\work\plant-code\cad\pid-test-data"));
    roots.into_iter().find(|root| root.is_dir())
}

/// The drawing file and its sibling `RefData~4~681.zip`, if both exist
/// under the same backup data root.
fn fixtures() -> Option<(PathBuf, PathBuf)> {
    let root = backup_data_dir()?;
    let drawing = root
        .join("backup-plant-data")
        .join("00")
        .join("00")
        .join(DRAWING);
    let archive = root.join("RefData~4~681.zip");
    if drawing.is_file() && archive.is_file() {
        Some((drawing, archive))
    } else {
        eprintln!(
            "skipping: {} / {} not found under {}",
            drawing.display(),
            archive.display(),
            root.display()
        );
        None
    }
}

#[test]
fn sym_dome_matches_the_drawings_own_cached_dome() {
    let Some((drawing_path, archive_path)) = fixtures() else {
        return;
    };

    // --- the `.sym` path -----------------------------------------------
    let sym_bytes = pid_parse::refdata_archive::read_archive_member(&archive_path, SYM_MEMBER)
        .unwrap_or_else(|e| panic!("reading {SYM_MEMBER} from {archive_path:?}: {e}"));
    let sym_geometry = read_symbol_geometry_from_bytes(&sym_bytes)
        .unwrap_or_else(|e| panic!("decoding {SYM_MEMBER}: {e}"));
    let sym_arcs: Vec<_> = sym_geometry
        .primitives
        .iter()
        .filter(|p| matches!(p.primitive, SymbolPrimitive::BSpline { .. }))
        .collect();
    assert_eq!(
        sym_arcs.len(),
        1,
        "Diaphragm Actuator.sym carries exactly one 0x{PSM_TYPE_CODE_IGELLIPTICALARC2D:04X} dome"
    );
    let SymbolPrimitive::BSpline {
        poles: sym_poles,
        weights: sym_weights,
        knots: sym_knots,
    } = &sym_arcs[0].primitive
    else {
        unreachable!("filtered to BSpline above");
    };

    // --- the drawing's cached body --------------------------------------
    let doc = PidParser::new()
        .parse_file(&drawing_path)
        .unwrap_or_else(|e| panic!("parsing {drawing_path:?}: {e}"));
    let cached_site = doc
        .jsites
        .iter()
        .find(|site| site.path == CACHED_SITE_PATH)
        .unwrap_or_else(|| panic!("{CACHED_SITE_PATH} is a definition cache in {DRAWING}"));
    let nested = cached_site
        .nested_geometry
        .as_ref()
        .unwrap_or_else(|| panic!("{CACHED_SITE_PATH} has no nested_geometry"));
    assert_eq!(
        nested.elliptical_arcs.len(),
        1,
        "{CACHED_SITE_PATH} caches exactly one elliptical arc"
    );
    let cached_arc = &nested.elliptical_arcs[0];

    // The two records, read off two different files, must carry the same
    // fields bit-for-bit -- same vendor record, two copies.
    assert_eq!(
        cached_arc.sweep_start.to_bits(),
        3.141_592_600_265_042_f64.to_bits()
    );
    assert_eq!(
        cached_arc.sweep_end.to_bits(),
        std::f64::consts::TAU.to_bits()
    );
    assert_eq!(cached_arc.center_x.to_bits(), 0.0_f64.to_bits());
    assert_eq!(
        cached_arc.center_y.to_bits(),
        0.008_889_999_999_999_999_f64.to_bits()
    );
    assert_eq!(cached_arc.major_x.to_bits(), (-0.003_175_f64).to_bits());
    assert_eq!(
        cached_arc.ratio.to_bits(),
        0.800_000_000_102_480_4_f64.to_bits()
    );
    assert_eq!(cached_arc.flag, 1, "the dome's own direction byte");

    // Feed the drawing's own fields through the exact same formula the
    // cached-body path (`geometry.rs`) and the `.sym` path
    // (`symbol_library::decode_primitive`) both call, and require the
    // result to equal the `.sym` reading bit-for-bit.
    let (expected_poles, expected_weights, expected_knots) =
        pid_parse::bspline::elliptical_arc_directed(
            (cached_arc.center_x, cached_arc.center_y),
            (cached_arc.major_x, cached_arc.major_y),
            cached_arc.ratio,
            cached_arc.sweep_start,
            cached_arc.sweep_end,
            cached_arc.flag == 0,
        )
        .expect("the cached arc's own fields pass the same gate");
    assert_eq!(*sym_poles, expected_poles);
    assert_eq!(*sym_weights, expected_weights);
    assert_eq!(*sym_knots, expected_knots);

    // And the normalized-geometry view the drawing actually renders from
    // carries the identical B-spline on the symbol definition that owns
    // this `JSite`'s layers.
    let normalized = build_normalized_geometry(&doc);
    let definition = normalized
        .symbol_definitions
        .iter()
        .find(|d| d.layers.binary_search(&cached_arc.sheet_layer_ref).is_ok())
        .unwrap_or_else(|| {
            panic!(
                "a symbol definition owns layer {}",
                cached_arc.sheet_layer_ref
            )
        });
    let drawn: Vec<_> = definition
        .primitives
        .iter()
        .filter_map(|p| match p {
            SymbolPrimitive::BSpline {
                poles,
                weights,
                knots,
            } if *poles == expected_poles => Some((poles, weights, knots)),
            _ => None,
        })
        .collect();
    assert_eq!(
        drawn.len(),
        1,
        "exactly one B-spline on the definition matches the dome's poles"
    );
    assert_eq!(*drawn[0].1, expected_weights);
    assert_eq!(*drawn[0].2, expected_knots);
}
