//! Phase 38 S3 integration: [`pid_parse::PidSemanticIndex`] joins the real
//! published fixtures onto their own sheet decode by the S1 two-hop rule
//! (`docs/analysis/2026-08-07-graphic-oid-is-the-semantic-join.md`).
//!
//! Fixtures are optional, as everywhere in this suite: tests skip cleanly
//! when the `SmartPlant` samples are not present.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pid_parse::{
    build_normalized_geometry, ParseOptions, PidDocument, PidParser, PidSemanticHit,
    PidSemanticIndex,
};

const A01: &str = "test-file/export-test/publish-data/A01/A01.pid";
const DWG0202: &str = "test-file/export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid";

fn load(fixture: &str) -> Option<(PathBuf, PidDocument)> {
    load_with(fixture, PidParser::new())
}

fn load_with(fixture: &str, parser: PidParser) -> Option<(PathBuf, PidDocument)> {
    let path = PathBuf::from(fixture);
    if !path.exists() {
        eprintln!("skipping: fixture {fixture} not found");
        return None;
    }
    let doc = parser
        .parse_file(&path)
        .unwrap_or_else(|error| panic!("failed to parse {fixture}: {error}"));
    Some((path, doc))
}

/// What each drawn entity that joins resolves to: the published object's
/// `GraphicOID`, its class and label, and the aggregate when the join went
/// through one -- keyed by the entity's own oid, the way OpenCADStudio asks.
type Joins = BTreeMap<u32, (u32, String, Option<String>, Option<u32>)>;

fn joins(doc: &PidDocument, index: &PidSemanticIndex) -> Joins {
    build_normalized_geometry(doc)
        .entities
        .iter()
        .filter_map(|entity| entity.graphic_oid)
        .filter_map(|oid| {
            let hit = index.resolve(oid)?;
            let via = match hit {
                PidSemanticHit::Direct(_) => None,
                PidSemanticHit::ViaDependency { dependency_oid, .. } => Some(dependency_oid),
            };
            let object = hit.object();
            Some((
                oid,
                (
                    object.graphic_oid,
                    object.class.clone(),
                    object.label().map(str::to_string),
                    via,
                ),
            ))
        })
        .collect()
}

fn published_xml(path: &std::path::Path) -> String {
    let stem = path.file_stem().unwrap().to_string_lossy();
    std::fs::read_to_string(path.with_file_name(format!("{stem}_Data.xml")))
        .expect("the publish pair ships a _Data.xml")
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
    let Some((path, doc)) = load(DWG0202) else {
        return;
    };
    let index = PidSemanticIndex::load_beside(&path, &doc)
        .expect("the DWG-0202 publish pair ships a _Data.xml beside the .pid");

    // S1 §2: 39 published representations with 39 distinct GraphicOIDs.
    assert_eq!(index.len(), 39);

    // The drawing was not saved after publishing: every representation's
    // UID row describes the record its GraphicOID names, so the UID join
    // moves nothing (docs/analysis/2026-10-08-a01-representation-uid-is-the-join.md).
    assert_eq!((index.uid_joined(), index.stale_graphic_oids()), (39, 0));
    let by_graphic_oid = PidSemanticIndex::from_xml(&published_xml(&path), &doc);
    assert_eq!(joins(&doc, &index), joins(&doc, &by_graphic_oid));
    let joining_entities = build_normalized_geometry(&doc)
        .entities
        .iter()
        .filter_map(|entity| entity.graphic_oid)
        .filter(|oid| index.resolve(*oid).is_some())
        .count();
    assert_eq!(joining_entities, 41, "41 drawn entities join, as before");

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
fn a01_publish_pair_joins_by_representation_uid() {
    // A01 was saved again after it was published: its four GraphicOIDs now
    // name attribute rows, not graphics, and the GraphicOID join finds
    // nothing. Each representation's UID still sits in one attribute row,
    // and that row's space-map edge names the record it describes
    // (docs/analysis/2026-10-08-a01-representation-uid-is-the-join.md). The
    // Geometry profile is the one OpenCADStudio parses with.
    for (profile, parser) in [
        ("full", PidParser::new()),
        (
            "geometry",
            PidParser::with_options(ParseOptions::geometry()),
        ),
    ] {
        let Some((path, doc)) = load_with(A01, parser) else {
            return;
        };
        let index = PidSemanticIndex::load_beside(&path, &doc)
            .expect("the A01 publish pair ships a _Data.xml beside the .pid");

        assert_eq!(index.len(), 4, "{profile}");
        assert_eq!(
            (index.uid_joined(), index.stale_graphic_oids()),
            (4, 4),
            "{profile}: every published GraphicOID is stale, every UID joins"
        );
        let pipeline = Some("PH- 0102102-DN250 mm-B5-P-40.000 in".to_string());
        assert_eq!(
            joins(&doc, &index),
            Joins::from([
                (51, (24606, "PIDNozzle".to_string(), None, None)),
                (
                    184,
                    (
                        24601,
                        "PIDProcessVessel".to_string(),
                        Some("V 010121A".to_string()),
                        None,
                    ),
                ),
                (275, (24615, "PIDPipeline".to_string(), pipeline, Some(417))),
            ]),
            "{profile}: the vessel, its nozzle and the pipeline's run, nothing else"
        );

        let by_graphic_oid = PidSemanticIndex::from_xml(&published_xml(&path), &doc);
        assert!(
            joins(&doc, &by_graphic_oid).is_empty(),
            "{profile}: the GraphicOID join alone still reaches no drawn entity"
        );
    }
}
