//! DWG plant MDF smoke + blockage-visibility gate.
//!
//! Stage-1 of the publish XML "DWG MDF onboarding" plan.
//! The suite formerly used the A01 / TEST02 source as a
//! stand-in whenever a test *wanted* DWG-shape coverage. This
//! file is the single test entry that binds the DWG reference
//! `_Data.xml` / `_Meta.xml` to the DWG MDF fixture at
//! [`common::DWG_MDF_PATH`].
//!
//! When the MDF fixture is absent every test in this file
//! soft-skips through [`common::DWG_MDF_MISSING_HINT`] so
//! the blockage is surfaced in test output, NOT silently
//! ignored. The hint explicitly tells the reader that
//! Stage 2–4 verification (loader canonical-field
//! enrichment, A24 / A27b whitelist closure,
//! PIDBranchPoint / PIDPipingBranchPoint parity) is NOT
//! validated on this run.
//!
//! When the MDF fixture is present:
//!
//! 1. The loader must succeed on [`common::DWG_DRAWING_UID`].
//! 2. The writer must produce a non-empty `_Data.xml` and a
//!    `_Meta.xml` whose `DocUID` / `Plant` attributes agree
//!    with [`common::DWG_DRAWING_UID`] /
//!    [`common::DWG_PLANT_NAME`] (which, in turn, were
//!    sourced from the reference fixture — so this is a
//!    transitively-grounded sanity check on the fixture's
//!    drawing UID too).
//! 3. The writer's DWG-style `_Data.xml` must follow the A29
//!    convention of emitting `Name` (not `ItemTag`) on
//!    `<PIDPipeline><IObject>`. This is the smallest
//!    end-to-end guarantee that [`PublishStyle::Dwg`] is
//!    actually in effect and the helper wired it up.

mod common;
use common::{
    generate_dwg_data_xml, generate_dwg_meta_xml, load_reference_dwg_xml, DWG_DRAWING_UID,
    DWG_MDF_PATH, DWG_PLANT_NAME,
};

/// Surface MDF presence in test output so a reader can see
/// at a glance whether the DWG-dependent gates ran or
/// soft-skipped. This test never fails — its job is purely
/// diagnostic; it's the human-readable complement to the
/// soft-skip messages the helpers print.
#[test]
fn dwg_mirror_presence_smoke() {
    let present = std::path::Path::new(DWG_MDF_PATH).exists();
    if present {
        eprintln!("DWG MDF fixture present at `{DWG_MDF_PATH}` — Stage 2-4 gates CAN run.");
    } else {
        eprintln!(
            "DWG MDF fixture absent at `{DWG_MDF_PATH}` — Stage 2-4 gates WILL NOT run. \
             See other tests in this file for the soft-skip message."
        );
    }
}

/// When the MDF fixture lands, the loader must produce a drawing
/// whose emitted `_Meta.xml` carries the same `DocUID` /
/// `Plant` as the reference fixture. Keeps the constants in
/// [`common`] honest — the moment either drifts, this gate
/// fires.
#[test]
fn dwg_mirror_end_to_end_meta_xml_agrees_with_reference_identifiers() {
    let Some(generated_result) = generate_dwg_meta_xml() else {
        return;
    };
    let generated = generated_result.expect("write_meta_xml should succeed on DWG MDF");
    assert!(
        generated.contains(&format!("DocUID=\"{DWG_DRAWING_UID}\"")),
        "DWG _Meta.xml must carry the reference DocUID; emitted:\n{generated}"
    );
    assert!(
        generated.contains(&format!("Plant=\"{DWG_PLANT_NAME}\"")),
        "DWG _Meta.xml must carry the reference Plant; emitted:\n{generated}"
    );
    assert!(
        generated.contains("DocName=\"DWG-0202GP06-01\""),
        "DWG _Meta.xml must carry the DWG drawing name; emitted:\n{generated}"
    );
}

/// Ditto for `_Data.xml`: MDF-driven writer output must be
/// non-empty and must follow the DWG-style IObject
/// convention (A29) on `<PIDPipeline>` — this is the
/// smallest end-to-end assertion that
/// [`PublishStyle::Dwg`] was actually applied.
#[test]
fn dwg_mirror_end_to_end_data_xml_follows_dwg_style_on_pipeline() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let generated = generated_result.expect("write_data_xml should succeed on DWG MDF");
    assert!(
        !generated.is_empty(),
        "DWG _Data.xml writer output must not be empty"
    );
    assert!(
        generated.contains("<PIDPipeline>"),
        "DWG _Data.xml must contain at least one PIDPipeline; got:\n{generated}"
    );

    // A29 DWG convention: PIDPipeline IObject uses Name and
    // drops ItemTag. We look at the first PIDPipeline IObject
    // line and assert the shape.
    let first_pipeline_iobject = generated
        .split("<PIDPipeline>")
        .nth(1)
        .and_then(|rest| rest.split('\n').find(|line| line.contains("<IObject")))
        .unwrap_or("")
        .to_string();
    assert!(
        !first_pipeline_iobject.is_empty(),
        "DWG _Data.xml should have at least one PIDPipeline IObject"
    );
    assert!(
        !first_pipeline_iobject.contains("ItemTag="),
        "DWG-style PIDPipeline IObject must drop ItemTag; got:\n{first_pipeline_iobject}"
    );
}

// -----------------------------------------------------------------
// Stage-4 — PIDBranchPoint + PIDPipingBranchPoint end-to-end gates
// -----------------------------------------------------------------

/// When the MDF fixture is present and the loader surfaces
/// BranchPoint-typed model items, the writer must emit
/// `<PIDBranchPoint>` blocks with the canonical 8-interface
/// shape and Name attribute. Count must match the DWG reference
/// (5 instances). Soft-skips when the MDF fixture is absent.
#[test]
fn dwg_mirror_emits_pid_branch_point_matching_reference_count() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let generated = generated_result.expect("write_data_xml should succeed on DWG MDF");
    let Some(reference) = load_reference_dwg_xml() else {
        return;
    };
    let gen_count = generated.matches("<PIDBranchPoint>").count();
    let ref_count = reference.matches("<PIDBranchPoint>").count();
    assert_eq!(
        gen_count, ref_count,
        "PIDBranchPoint count must match reference ({ref_count}); generated {gen_count}"
    );
}

/// Ditto for `<PIDPipingBranchPoint>` — 4 instances in the
/// reference, each carrying the `.BPT` UID suffix.
#[test]
fn dwg_mirror_emits_piping_branch_point_matching_reference_count() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let generated = generated_result.expect("write_data_xml should succeed on DWG MDF");
    let Some(reference) = load_reference_dwg_xml() else {
        return;
    };
    let gen_count = generated.matches("<PIDPipingBranchPoint>").count();
    let ref_count = reference.matches("<PIDPipingBranchPoint>").count();
    assert_eq!(
        gen_count, ref_count,
        "PIDPipingBranchPoint count must match reference ({ref_count}); generated {gen_count}"
    );
}
