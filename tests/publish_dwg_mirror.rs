//! DWG end-to-end publish gate — the committed backup package.
//!
//! The DWG plant is committed as a whole backup package
//! (`test-file/backup-test/DWG-0202GP06-01_p`): an Oracle `exp` dump
//! (`Export.dmp`), the `SPPIDDataMap.xml` inside `RefData~4~804.zip`,
//! and the drawing's own `.pid`. There is no `Export.mdf` — the
//! database line cannot read this plant, and the thirteen gates that
//! used to wait for one never ran.
//!
//! They now run on the Oracle line the CLI's `--oracle-exp` takes:
//! the committed `.pid` published against its package's own dump and
//! data map. `common::generate_dwg_data_xml` / `generate_dwg_meta_xml`
//! run exactly that and hand back the documents a delivery writes.
//!
//! What this file holds:
//!
//! 1. The `_Meta.xml` carries the reference's `DocUID` / `Plant` /
//!    `DocName`.
//! 2. The `_Data.xml` follows the DWG convention of naming a
//!    `<PIDPipeline><IObject>` by `Name` and not `ItemTag`.
//! 3. The two branch-point tags are counted against the reference,
//!    with the registered difference stated: the Oracle line writes
//!    four `PIDBranchPoint` where the reference carries five (the
//!    fifth is the OPC point whose representation this drawing's
//!    `.pid` does not place), and four `PIDPipingBranchPoint`, which
//!    is the reference's count.

mod common;
use common::{
    generate_dwg_data_xml, generate_dwg_meta_xml, load_reference_dwg_xml, DWG_DRAWING_UID,
    DWG_PLANT_NAME,
};

/// The branch points the Oracle line writes, and what the reference
/// carries. The gap is the OPC point on a representation the `.pid`
/// does not hold: `oracle-exp opc:` on the run says `4 PIDBranchPoint
/// written`, and the reference's fifth (`799B9008DB6848F28127C78AC745C5A8`,
/// name `278`) sits on a representation this drawing never places.
const DWG_PID_BRANCH_POINTS: usize = 4;
const DWG_REFERENCE_PID_BRANCH_POINTS: usize = 5;
const DWG_PIPING_BRANCH_POINTS: usize = 4;
const DWG_REFERENCE_PIPING_BRANCH_POINTS: usize = 4;

/// The emitted `_Meta.xml` must carry the reference fixture's
/// `DocUID` / `Plant` / `DocName`. Keeps the constants in [`common`]
/// honest — the moment either drifts, this gate fires.
#[test]
fn dwg_meta_xml_agrees_with_reference_identifiers() {
    let Some(generated_result) = generate_dwg_meta_xml() else {
        return;
    };
    let generated = generated_result.expect("the package's meta document renders");
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

/// The `_Data.xml` must be non-empty and must follow the DWG-style
/// `IObject` convention on `<PIDPipeline>`: `Name`, no `ItemTag`.
#[test]
fn dwg_data_xml_follows_dwg_style_on_pipeline() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let generated = generated_result.expect("the package's data document renders");
    assert!(
        !generated.is_empty(),
        "DWG _Data.xml writer output must not be empty"
    );
    assert!(
        generated.contains("<PIDPipeline>"),
        "DWG _Data.xml must contain at least one PIDPipeline; got:\n{generated}"
    );

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
        first_pipeline_iobject.contains("Name="),
        "DWG-style PIDPipeline IObject must state Name; got:\n{first_pipeline_iobject}"
    );
    assert!(
        !first_pipeline_iobject.contains("ItemTag="),
        "DWG-style PIDPipeline IObject must drop ItemTag; got:\n{first_pipeline_iobject}"
    );
}

/// `<PIDBranchPoint>` is written once per OPC point the drawing
/// places. The reference carries one more, on a representation this
/// `.pid` does not hold; the difference is registered above.
#[test]
fn dwg_emits_the_pid_branch_points_the_drawing_places() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let generated = generated_result.expect("the package's data document renders");
    let Some(reference) = load_reference_dwg_xml() else {
        return;
    };
    let gen_count = generated.matches("<PIDBranchPoint>").count();
    let ref_count = reference.matches("<PIDBranchPoint>").count();
    assert_eq!(
        (gen_count, ref_count),
        (DWG_PID_BRANCH_POINTS, DWG_REFERENCE_PID_BRANCH_POINTS),
        "PIDBranchPoint: the Oracle line writes {gen_count}, the reference carries {ref_count}"
    );
}

/// `<PIDPipingBranchPoint>` — four in the reference, each carrying
/// the `.BPT` UID suffix, and four from the Oracle line.
#[test]
fn dwg_emits_the_piping_branch_points_the_reference_carries() {
    let Some(generated_result) = generate_dwg_data_xml() else {
        return;
    };
    let generated = generated_result.expect("the package's data document renders");
    let Some(reference) = load_reference_dwg_xml() else {
        return;
    };
    let gen_count = generated.matches("<PIDPipingBranchPoint>").count();
    let ref_count = reference.matches("<PIDPipingBranchPoint>").count();
    assert_eq!(
        (gen_count, ref_count),
        (DWG_PIPING_BRANCH_POINTS, DWG_REFERENCE_PIPING_BRANCH_POINTS),
        "PIDPipingBranchPoint: the Oracle line writes {gen_count}, the reference carries \
         {ref_count}"
    );
    let bpt_uids: Vec<&str> = generated
        .split("<PIDPipingBranchPoint>")
        .skip(1)
        .filter_map(|rest| {
            let uid_line = rest.split('\n').find(|line| line.contains("<IObject"))?;
            let start = uid_line.find("UID=\"")? + 5;
            let end = start + uid_line[start..].find('"')?;
            Some(&uid_line[start..end])
        })
        .collect();
    assert_eq!(bpt_uids.len(), DWG_PIPING_BRANCH_POINTS);
    let bad: Vec<&str> = bpt_uids
        .iter()
        .copied()
        .filter(|uid| !uid.ends_with(".BPT"))
        .collect();
    assert!(
        bad.is_empty(),
        "every PIDPipingBranchPoint IObject UID must end in `.BPT`; offending: {bad:?}"
    );
}
