use super::*;

#[test]
fn a_deleted_object_is_a_missing_finding() {
    let generated = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
    </Container>"#;
    let reference = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <PIDNozzle><IObject UID="N" Name="N-1"/></PIDNozzle>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(!diff.is_clean());
    assert_eq!(diff.objects_not_in_generated, 1);
    assert_eq!(diff.objects_not_in_reference, 0);
    assert_eq!(diff.objects_of_another_class, 0);
    assert_eq!(diff.values_different, 0);
    assert_eq!(diff.values_not_in_reference, 0);
    assert_eq!(diff.rels_not_in_reference, 0);
    assert!(
        diff.findings.iter().any(|f| f == "missing PIDNozzle N"),
        "{diff}"
    );
}

#[test]
fn a_deleted_value_is_a_missing_finding() {
    let reference = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/><IDrawingRepresentation GraphicOID="7"/></PIDInstrument>
    </Container>"#;
    let generated = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(!diff.is_clean());
    assert_eq!(diff.values_not_in_generated, 1);
    assert!(
        diff.findings
            .iter()
            .any(|f| f.contains("IDrawingRepresentation.GraphicOID") && f.contains("7")),
        "{diff}"
    );

    // `finish_publish_block` drops empty values, so `GraphicOID=""` is the
    // same missing value as `GraphicOID` absent.
    let emptied = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/><IDrawingRepresentation GraphicOID=""/></PIDInstrument>
    </Container>"#;
    let same = diff_pid_export(emptied, reference).expect("well-formed");
    assert!(!same.is_clean());
    assert_eq!(same.values_not_in_generated, 1);
}

#[test]
fn a_deleted_rel_is_a_missing_finding() {
    let generated = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <PIDNozzle><IObject UID="B" Name="N-1"/></PIDNozzle>
        <PIDDrawing><IObject UID="D" Name="DRW"/></PIDDrawing>
        <Rel><IObject UID="R0"/><IRel UID1="D" UID2="A" DefUID="DwgRepresentationComposition"/></Rel>
    </Container>"#;
    let reference = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <PIDNozzle><IObject UID="B" Name="N-1"/></PIDNozzle>
        <PIDDrawing><IObject UID="D" Name="DRW"/></PIDDrawing>
        <Rel><IObject UID="R0"/><IRel UID1="D" UID2="A" DefUID="DwgRepresentationComposition"/></Rel>
        <Rel><IObject UID="R1"/><IRel UID1="A" UID2="B" DefUID="DwgRepresentationComposition"/></Rel>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(!diff.is_clean());
    assert_eq!(diff.rels_not_in_generated, 1);
    assert!(
        diff.findings.iter().any(|f| f.starts_with("missing rel ")
            && f.contains("\"A\", \"B\", \"DwgRepresentationComposition\"")),
        "{diff}"
    );
}

#[test]
fn a_reference_only_class_the_file_cannot_state_is_excused() {
    let generated = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
    </Container>"#;
    let reference = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <PIDSignalPort><IObject UID="S"/></PIDSignalPort>
        <PIDPipeline><IObject UID="P" Name="L-1"/></PIDPipeline>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(diff.is_clean(), "{diff}");
    assert_eq!(diff.objects_not_in_generated, 0);
    assert_eq!(diff.objects_excused_by_design, 2);
    let text = diff.exclusions.join("\n");
    assert!(
        text.contains("excused object PIDSignalPort 1") && text.contains("PidTagRole::Port"),
        "{text}"
    );
    assert!(
        text.contains("excused object PIDPipeline 1") && text.contains("PidTagRole::Enrichment"),
        "{text}"
    );
}

#[test]
fn a_reference_only_object_of_a_published_class_is_a_finding() {
    let generated = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
    </Container>"#;
    let reference = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <PIDNozzle><IObject UID="N" Name="N-1"/></PIDNozzle>
        <PIDRepresentation><IObject UID="V"/></PIDRepresentation>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(!diff.is_clean());
    assert_eq!(diff.objects_not_in_generated, 2);
    assert_eq!(diff.objects_excused_by_design, 0);
    assert_eq!(
        diff.findings,
        vec!["missing PIDNozzle N", "missing PIDRepresentation V"]
    );
}

#[test]
fn a_ghost_attribute_is_a_missing_finding() {
    let generated = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
    </Container>"#;
    let ghost = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1" GHOSTATTR="ghost-value"/></PIDInstrument>
    </Container>"#;
    let diff = diff_pid_export(generated, ghost).expect("well-formed");
    assert!(!diff.is_clean());
    assert_eq!(diff.values_not_in_generated, 1);
    assert!(
        diff.findings
            .iter()
            .any(|f| f.contains("IObject.GHOSTATTR") && f.contains("ghost-value")),
        "{diff}"
    );

    let excluded = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/><IPBSItem ConstructionStatus="1"/></PIDInstrument>
    </Container>"#;
    let clean = diff_pid_export(generated, excluded).expect("well-formed");
    assert!(clean.is_clean(), "{clean}");
    assert_eq!(clean.values_not_in_generated, 0);
    assert_eq!(clean.values_excused_by_design, 1);
    assert!(
        clean
            .exclusions
            .iter()
            .any(|line| line.contains("IPBSItem.ConstructionStatus")
                && line.contains("plant breakdown structure")),
        "{}",
        clean.exclusions.join("\n")
    );
}

#[test]
fn a_tag_named_element_duplicating_iobject_is_excused() {
    let generated = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
    </Container>"#;
    let duplicate = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/><PT-1 UID="A" Name="PT-1"/></PIDInstrument>
    </Container>"#;
    let diff = diff_pid_export(generated, duplicate).expect("well-formed");
    assert!(diff.is_clean(), "{diff}");
    assert_eq!(diff.values_excused_by_design, 2);
    assert!(
        diff.exclusions
            .iter()
            .any(|line| line.contains("tag-named elements")),
        "{}",
        diff.exclusions.join("\n")
    );

    let differing = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/><PT-1 UID="A" Name="PT-9"/></PIDInstrument>
    </Container>"#;
    let dirty = diff_pid_export(generated, differing).expect("well-formed");
    assert!(!dirty.is_clean());
    assert_eq!(dirty.values_not_in_generated, 1);
    assert!(
        dirty
            .findings
            .iter()
            .any(|f| f.contains("PT-1.Name") && f.contains("PT-9")),
        "{dirty}"
    );
}

#[test]
fn a_reference_only_rel_the_run_states_nowhere_is_excused() {
    let generated = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <PIDNozzle><IObject UID="B" Name="N-1"/></PIDNozzle>
        <Rel><IObject UID="R0"/><IRel UID1="A" UID2="B" DefUID="DrawingItems"/></Rel>
    </Container>"#;
    let reference = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <PIDNozzle><IObject UID="B" Name="N-1"/></PIDNozzle>
        <PIDNozzle><IObject UID="C" Name="N-2"/></PIDNozzle>
        <Rel><IObject UID="R0"/><IRel UID1="A" UID2="B" DefUID="DrawingItems"/></Rel>
        <Rel><IObject UID="R1"/><IRel UID1="A" UID2="B" DefUID="PipingEnd1Conn"/></Rel>
        <Rel><IObject UID="R2"/><IRel UID1="A" UID2="C" DefUID="DrawingItems"/></Rel>
        <Rel><IObject UID="R3"/><IRel UID1="A" UID2="Z" DefUID="DrawingItems"/></Rel>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(!diff.is_clean());
    assert_eq!(diff.rels_excused_by_design, 2);
    assert_eq!(diff.rels_not_in_generated, 1);
    let text = diff.exclusions.join("\n");
    assert!(
        text.contains("excused rel PipingEnd1Conn 1 — only the exp dump states PipingEnd1Conn;"),
        "{text}"
    );
    assert!(
        text.contains("excused rel DrawingItems 1 — its end C is not published by this run"),
        "{text}"
    );
    assert!(
        diff.findings
            .iter()
            .any(|f| f.starts_with("missing rel ") && f.contains("\"A\", \"Z\", \"DrawingItems\"")),
        "{diff}"
    );
}

#[test]
fn a_pid_export_accounts_for_classes_it_does_not_publish_and_fails_a_wrong_value() {
    let reference = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <PIDPipeline><IObject UID="P" Name="L-1"/></PIDPipeline>
        <Rel><IObject UID="R"/><IRel UID1="A" UID2="D" DefUID="DrawingItems"/></Rel>
    </Container>"#;
    let generated = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <Rel><IObject UID="R"/><IRel UID1="A" UID2="D" DefUID="DrawingItems"/></Rel>
    </Container>"#;
    let clean = diff_pid_export(generated, reference).expect("well-formed");
    assert!(clean.is_clean(), "{clean}");
    assert_eq!(clean.objects, 1);
    assert_eq!(clean.rels, 1);
    assert_eq!(clean.values, 1);
    assert_eq!(clean.objects_excused_by_design, 1);
    assert!(
        clean
            .exclusions
            .iter()
            .any(|line| line.contains("PIDPipeline") && line.contains("\"$derived\"")),
        "{}",
        clean.exclusions.join("\n")
    );

    let wrong = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-2"/></PIDInstrument>
    </Container>"#;
    let dirty = diff_pid_export(wrong, reference).expect("well-formed");
    assert!(!dirty.is_clean());
    assert_eq!(dirty.values_different, 1);
    assert!(dirty.findings[0].contains("PT-2"));
}

#[test]
fn the_excused_accounting_prints_with_findings_not_on_a_clean_run() {
    let generated = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
    </Container>"#;
    let reference = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <PIDSignalPort><IObject UID="S"/></PIDSignalPort>
    </Container>"#;
    let clean = diff_pid_export(generated, reference).expect("well-formed");
    assert!(clean.is_clean(), "{clean}");
    assert!(!clean.exclusions.is_empty());
    let shown = clean.to_string();
    assert!(!shown.contains("excused "), "{shown}");
    assert!(!shown.contains("reference-only: "), "{shown}");

    let dirty_reference = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <PIDSignalPort><IObject UID="S"/></PIDSignalPort>
        <PIDNozzle><IObject UID="N" Name="N-1"/></PIDNozzle>
    </Container>"#;
    let dirty = diff_pid_export(generated, dirty_reference).expect("well-formed");
    assert!(!dirty.is_clean());
    let shown = dirty.to_string();
    assert!(shown.contains("missing PIDNozzle N"), "{shown}");
    assert!(shown.contains("reference-only: "), "{shown}");
    assert!(shown.contains("excused object PIDSignalPort 1"), "{shown}");
}

#[test]
fn excluded_attribute_classes_are_the_measured_union() {
    let classes = crate::publish::catalog::EXCLUDED_ATTRIBUTE_CLASSES;
    assert_eq!(classes.len(), 8);
    let mut pairs = Vec::new();
    for class in classes {
        assert!(!class.class.is_empty(), "unnamed exclusion class");
        assert!(
            !class.reason.is_empty(),
            "{} has an empty reason",
            class.class
        );
        for (interface, attributes) in class.members {
            let mut sorted = attributes.to_vec();
            sorted.sort_unstable();
            let mut unique = sorted.clone();
            unique.dedup();
            assert_eq!(
                *attributes,
                sorted.as_slice(),
                "{} {interface} attributes not sorted",
                class.class
            );
            assert_eq!(
                sorted, unique,
                "{} {interface} attributes repeat",
                class.class
            );
            for attribute in *attributes {
                pairs.push((*interface, *attribute));
            }
        }
    }
    assert_eq!(pairs.len(), 41);
    let mut unique = pairs.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), pairs.len(), "member repeated across classes");
    assert!(!pairs.contains(&("IObject", "Name")));
    assert!(!pairs.contains(&("IObject", "UID")));
}

#[test]
fn excluded_attribute_lookup_covers_every_member() {
    use crate::publish::catalog::excluded_attribute;
    for class in crate::publish::catalog::EXCLUDED_ATTRIBUTE_CLASSES {
        for (interface, attributes) in class.members {
            for attribute in *attributes {
                assert!(
                    excluded_attribute(interface, attribute).is_some(),
                    "{interface}.{attribute} not covered"
                );
            }
        }
    }
    for (interface, attribute) in [
        ("IObject", "GHOSTATTR"),
        ("IObject", "Name"),
        ("IObject", "UID"),
        ("PIDFoo", "Bar"),
    ] {
        assert!(
            excluded_attribute(interface, attribute).is_none(),
            "{interface}.{attribute} should stay a finding"
        );
    }
}

#[test]
fn pid_file_cannot_state_is_the_dump_supplied_role_set() {
    use crate::publish::catalog::{pid_file_cannot_state, PidTagRole};
    for (tag, role) in [
        ("PIDSignalPort", PidTagRole::Port),
        ("PIDPipingPort", PidTagRole::Port),
        ("PIDPipeline", PidTagRole::Enrichment),
        ("PIDBranchPoint", PidTagRole::Enrichment),
    ] {
        assert_eq!(
            pid_file_cannot_state(tag).map(|(_, found)| found),
            Some(role),
            "{tag}"
        );
    }
    for tag in [
        "PIDNozzle",
        "PIDProcessVessel",
        "PIDRepresentation",
        "PIDDrawing",
        "PIDProcessPoint",
        "PIDPipingConnector",
        "PIDPipingBranchPoint",
        "PIDSignalConnector",
        "PIDFoo",
    ] {
        assert_eq!(pid_file_cannot_state(tag), None, "{tag}");
    }
}

#[test]
fn a_deleted_value_on_a_name_paired_pipeline_is_a_missing_finding() {
    let generated = r#"<Container>
        <PIDPipeline><IObject UID="PG" Name="L-1"/></PIDPipeline>
    </Container>"#;
    let reference = r#"<Container>
        <PIDPipeline><IObject UID="PR" Name="L-1"/><IPipelineOccurrence SequenceNumber="7"/></PIDPipeline>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(!diff.is_clean());
    assert_eq!(diff.objects_paired_by_name, 1);
    assert_eq!(diff.values_not_in_generated, 1);
    assert!(
        diff.findings
            .iter()
            .any(|f| f.contains("IPipelineOccurrence.SequenceNumber") && f.contains("PR")),
        "{diff}"
    );
}

#[test]
fn a_reference_only_name_on_an_unnamed_object_is_excused() {
    let generated = r#"<Container>
        <PIDPipingConnector><IObject UID="C"/></PIDPipingConnector>
    </Container>"#;
    let reference = r#"<Container>
        <PIDPipingConnector><IObject UID="C" Name="BV1001406A"/></PIDPipingConnector>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(diff.is_clean(), "{diff}");
    assert_eq!(diff.values_not_in_generated, 0);
    assert_eq!(diff.values_excused_by_design, 1);
    assert!(
        diff.exclusions
            .iter()
            .any(|line| line.contains("PIDPipingConnector.IObject.Name")),
        "{}",
        diff.exclusions.join("\n")
    );

    // A stated `IObject.Name` that disagrees with the reference's is still
    // a differing-value finding, never excused.
    let named = r#"<Container>
        <PIDInstrument><IObject UID="I" Name="PT-2"/></PIDInstrument>
    </Container>"#;
    let other = r#"<Container>
        <PIDInstrument><IObject UID="I" Name="PT-1"/></PIDInstrument>
    </Container>"#;
    let dirty = diff_pid_export(named, other).expect("well-formed");
    assert!(!dirty.is_clean());
    assert_eq!(dirty.values_different, 1);
    assert_eq!(dirty.values_not_in_generated, 0);
    assert_eq!(dirty.values_excused_by_design, 0);
}

#[test]
fn a_reference_pipeline_at_a_generated_uid_is_not_double_counted() {
    // The A01 shape: the generated document publishes this UID as another
    // class, so the generated-side walk already reports the class mismatch
    // and the reference-side walk must not count the object again.
    let generated = r#"<Container>
        <PIDPipingConnector><IObject UID="P"/></PIDPipingConnector>
    </Container>"#;
    let reference = r#"<Container>
        <PIDPipeline><IObject UID="P" Name="L-1"/></PIDPipeline>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(!diff.is_clean());
    assert_eq!(diff.objects_of_another_class, 1);
    assert!(
        diff.findings
            .iter()
            .any(|f| f == "class PIDPipingConnector P: reference PIDPipeline"),
        "{diff}"
    );
    assert_eq!(diff.objects_not_in_generated, 0);
    assert_eq!(diff.objects_excused_by_design, 0);
    assert_eq!(diff.objects_paired_by_name, 0);
    assert_eq!(diff.values_excused_by_design, 1);
}

#[test]
fn a_file_relation_class_the_run_lost_entirely_is_a_finding() {
    // Both ends are published and the run states no `ProcessPointCollection`
    // at all: the `.pid` alone yields that class, so its absence is a loss.
    let generated = r#"<Container>
        <PIDProcessPoint><IObject UID="P"/></PIDProcessPoint>
        <PIDPipingConnector><IObject UID="C"/></PIDPipingConnector>
    </Container>"#;
    let reference = r#"<Container>
        <PIDProcessPoint><IObject UID="P"/></PIDProcessPoint>
        <PIDPipingConnector><IObject UID="C"/></PIDPipingConnector>
        <Rel><IObject UID="R1"/><IRel UID1="P" UID2="C" DefUID="ProcessPointCollection"/></Rel>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(!diff.is_clean(), "{diff}");
    assert_eq!(diff.rels_not_in_generated, 1);
    assert_eq!(diff.rels_excused_by_design, 0);
}

#[test]
fn a_dump_relation_class_is_excused_only_while_the_run_has_no_dump_content() {
    let reference = r#"<Container>
        <PIDPipingPort><IObject UID="A.1" Name="1"/></PIDPipingPort>
        <PIDPipingConnector><IObject UID="RUN"/></PIDPipingConnector>
        <Rel><IObject UID="R1"/><IRel UID1="A.1" UID2="RUN" DefUID="PipingEnd1Conn"/></Rel>
    </Container>"#;
    // A numbered port is content only an exp run writes, so a run that has
    // one and states no `PipingEnd1Conn` between two published ends lost it.
    let exp_run = r#"<Container>
        <PIDPipingPort><IObject UID="A.1" Name="1"/></PIDPipingPort>
        <PIDPipingConnector><IObject UID="RUN"/></PIDPipingConnector>
    </Container>"#;
    let lost = diff_pid_export(exp_run, reference).expect("well-formed");
    assert!(!lost.is_clean(), "{lost}");
    assert_eq!(lost.rels_not_in_generated, 1);
    assert_eq!(lost.rels_excused_by_design, 0);

    // A `.pid`-only run carries none of the dump's content: the port is an
    // excused object and the relation an excused class.
    let pid_only = r#"<Container>
        <PIDPipingConnector><IObject UID="RUN"/></PIDPipingConnector>
    </Container>"#;
    let excused = diff_pid_export(pid_only, reference).expect("well-formed");
    assert!(excused.is_clean(), "{excused}");
    assert_eq!(excused.objects_excused_by_design, 1);
    assert_eq!(excused.rels_excused_by_design, 1);
    let text = excused.exclusions.join("\n");
    assert!(
        text.contains("excused rel PipingEnd1Conn 1 — only the exp dump states PipingEnd1Conn;"),
        "{text}"
    );
}

#[test]
fn a_relation_class_this_line_never_writes_is_excused() {
    let generated = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <PIDNozzle><IObject UID="B" Name="N-1"/></PIDNozzle>
    </Container>"#;
    let reference = r#"<Container>
        <PIDInstrument><IObject UID="A" Name="PT-1"/></PIDInstrument>
        <PIDNozzle><IObject UID="B" Name="N-1"/></PIDNozzle>
        <Rel><IObject UID="R1"/><IRel UID1="A" UID2="B" DefUID="ItemGroup"/></Rel>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(diff.is_clean(), "{diff}");
    assert_eq!(diff.rels_excused_by_design, 1);
    let text = diff.exclusions.join("\n");
    assert!(
        text.contains("excused rel ItemGroup 1 — this line writes no ItemGroup relation"),
        "{text}"
    );
}

#[test]
fn a_connector_rel_lost_beside_a_name_paired_pipeline_is_a_finding() {
    // The synthesized pipeline pairs with the reference's by name although
    // its UID differs, so the rel's pipeline end is published and a lost
    // connector rel is a finding rather than an unpublished end.
    let generated = r#"<Container>
        <PIDPipeline><IObject UID="G" Name="L-1"/></PIDPipeline>
        <PIDPipingConnector><IObject UID="C"/></PIDPipingConnector>
        <PIDPipingConnector><IObject UID="D"/></PIDPipingConnector>
        <Rel><IObject UID="PCN-C-G"/><IRel UID1="C" UID2="G" DefUID="PipingConnectors"/></Rel>
    </Container>"#;
    let reference = r#"<Container>
        <PIDPipeline><IObject UID="R" Name="L-1"/></PIDPipeline>
        <PIDPipingConnector><IObject UID="C"/></PIDPipingConnector>
        <PIDPipingConnector><IObject UID="D"/></PIDPipingConnector>
        <Rel><IObject UID="PCN-C-R"/><IRel UID1="C" UID2="R" DefUID="PipingConnectors"/></Rel>
        <Rel><IObject UID="PCN-D-R"/><IRel UID1="D" UID2="R" DefUID="PipingConnectors"/></Rel>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(!diff.is_clean(), "{diff}");
    assert_eq!(diff.rels_paired_by_name, 1);
    assert_eq!(diff.rels_not_in_generated, 1);
    assert_eq!(diff.rels_excused_by_design, 0);
}

#[test]
fn a_class_this_line_names_that_lost_every_name_is_a_finding() {
    // `PIDInstrument` takes its name from the instrument rule; a run that
    // names none of its instruments lost the rule, so the reference's name
    // is a missing value rather than the vendor's composition.
    let generated = r#"<Container>
        <PIDInstrument><IObject UID="I"/></PIDInstrument>
    </Container>"#;
    let reference = r#"<Container>
        <PIDInstrument><IObject UID="I" Name="PT-1"/></PIDInstrument>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(!diff.is_clean(), "{diff}");
    assert_eq!(diff.values_not_in_generated, 1);
    assert_eq!(diff.values_excused_by_design, 0);

    // The exp dump numbers every port it writes, so an unnamed port is a loss
    // too.
    let unnamed_port = r#"<Container>
        <PIDPipingPort><IObject UID="N.1"/></PIDPipingPort>
    </Container>"#;
    let numbered_port = r#"<Container>
        <PIDPipingPort><IObject UID="N.1" Name="1"/></PIDPipingPort>
    </Container>"#;
    let lost = diff_pid_export(unnamed_port, numbered_port).expect("well-formed");
    assert!(!lost.is_clean(), "{lost}");
    assert_eq!(lost.values_not_in_generated, 1);
}

#[test]
fn an_unnamed_row_of_a_class_this_run_names_elsewhere_is_excused() {
    // The inline-component row carries a name and the inline-instrument row
    // lacks a part of the rule: the class's naming works, so the reference's
    // name on the unnamed row is the vendor's composition.
    let generated = r#"<Container>
        <PIDInlineInstrument><IObject UID="V" Name="BV-1"/></PIDInlineInstrument>
        <PIDInlineInstrument><IObject UID="F"/></PIDInlineInstrument>
    </Container>"#;
    let reference = r#"<Container>
        <PIDInlineInstrument><IObject UID="V" Name="BV-1"/></PIDInlineInstrument>
        <PIDInlineInstrument><IObject UID="F" Name="FT-753"/></PIDInlineInstrument>
    </Container>"#;
    let diff = diff_pid_export(generated, reference).expect("well-formed");
    assert!(diff.is_clean(), "{diff}");
    assert_eq!(diff.values_excused_by_design, 1);
}
