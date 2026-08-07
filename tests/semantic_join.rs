//! Phase 38 S3 integration: [`pid_parse::PidSemanticIndex`] joins the real
//! published fixtures onto their own sheet decode by the S1 two-hop rule
//! (`docs/analysis/2026-08-07-graphic-oid-is-the-semantic-join.md`).
//!
//! Fixtures are optional, as everywhere in this suite: tests skip cleanly
//! when the `SmartPlant` samples are not present.

use std::path::PathBuf;

use pid_parse::{PidDocument, PidParser, PidSemanticHit, PidSemanticIndex};

fn load(fixture: &str) -> Option<(PathBuf, PidDocument)> {
    let path = PathBuf::from(fixture);
    if !path.exists() {
        eprintln!("skipping: fixture {fixture} not found");
        return None;
    }
    let doc = PidParser::new()
        .parse_file(&path)
        .unwrap_or_else(|error| panic!("failed to parse {fixture}: {error}"));
    Some((path, doc))
}

/// Every `igLineString2d` oid of the document — the family S1 proved the
/// published aggregates reference (12/12 on this fixture).
fn linestring_oids(doc: &PidDocument) -> Vec<u32> {
    doc.sheet_streams
        .iter()
        .filter_map(|sheet| sheet.geometry.as_ref())
        .flat_map(|geometry| geometry.decoded_iglinestrings.iter().map(|r| r.oid))
        .collect()
}

#[test]
fn dwg0202_publish_pair_resolves_directly_and_through_aggregates() {
    let Some((path, doc)) =
        load("test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid")
    else {
        return;
    };
    let index = PidSemanticIndex::load_beside(&path, &doc)
        .expect("the DWG-0202 publish pair ships a _Data.xml beside the .pid");

    // S1 §2: 39 published representations with 39 distinct GraphicOIDs.
    assert_eq!(index.len(), 39);

    // Hop 1: every published oid resolves directly.
    for object in index.objects() {
        assert!(
            matches!(
                index.resolve(object.graphic_oid),
                Some(PidSemanticHit::Direct(_))
            ),
            "published oid {} must resolve directly",
            object.graphic_oid
        );
    }

    // Hop 2: the published DependencyObject aggregates reach their leaves.
    // S1 §3 showed all 12 aggregates reference an igLineString2d, so at
    // least 12 linestring oids must resolve via a dependency.
    let via_dependency = linestring_oids(&doc)
        .into_iter()
        .filter(|oid| {
            matches!(
                index.resolve(*oid),
                Some(PidSemanticHit::ViaDependency { .. })
            )
        })
        .count();
    assert!(
        via_dependency >= 12,
        "expected at least the 12 S1 aggregate leaves to resolve via \
         dependency, got {via_dependency}"
    );

    // The join carries real labels, not empty shells.
    assert!(
        index.objects().any(|object| object.label().is_some()),
        "at least one published object must carry an ItemTag or Name"
    );
}

#[test]
fn a01_publish_pair_loads_but_claims_no_false_joins() {
    // A01's four published oids sit in bytes the typed decode does not
    // reach yet (S1 §4: coverage gap, not a pairing mismatch). The index
    // must load, expose the four objects, and refuse to join any decoded
    // sheet oid to them.
    let Some((path, doc)) = load("test-file/export-test/publish-data/A01/A01.pid") else {
        return;
    };
    let index = PidSemanticIndex::load_beside(&path, &doc)
        .expect("the A01 publish pair ships a _Data.xml beside the .pid");

    assert_eq!(index.len(), 4);

    let decoded_oids: Vec<u32> = doc
        .sheet_streams
        .iter()
        .filter_map(|sheet| sheet.geometry.as_ref())
        .flat_map(|geometry| {
            geometry
                .decoded_iglines
                .iter()
                .map(|r| r.oid)
                .chain(geometry.decoded_iglinestrings.iter().map(|r| r.oid))
                .chain(geometry.decoded_igpoints.iter().map(|r| r.oid))
                .chain(geometry.decoded_igtextboxes.iter().map(|r| r.oid))
                .chain(geometry.decoded_igsymbols.iter().map(|r| r.oid))
        })
        .collect();
    assert!(!decoded_oids.is_empty(), "A01 decodes graphic records");
    for oid in decoded_oids {
        assert!(
            index.resolve(oid).is_none(),
            "A01 oid {oid} must not join: its published oids are outside \
             the typed decode (S1 coverage-gap verdict)"
        );
    }
}
