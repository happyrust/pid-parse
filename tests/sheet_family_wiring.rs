//! M3-PR17 wiring-consistency gate: for every local fixture sheet, the
//! populated [`pid_parse::SheetGeometry`] family fields must agree with
//! a fresh re-decode of the same raw bytes through the
//! [`pid_parse::SHEET_RECORD_FAMILIES`] registry.
//!
//! This closes the historical silent gap where a decoder existed and
//! its cross-fixture ratchet (which re-decodes raw bytes directly)
//! stayed green, while the `streams/cluster.rs` wiring for the family
//! was missing — so parsed documents quietly lacked the records.
//! `attribute_fragments` sitting outside the old 12-conjunct empty
//! check is exactly the class of drift this test now surfaces.
//!
//! Fixtures soft-skip when absent, mirroring `tests/parse_real_files.rs`.

use pid_parse::{PidParser, SHEET_RECORD_FAMILIES};

const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    "工艺管道及仪表流程-1.pid",
    "export-test/publish-data/A01/A01.pid",
    "D06.pid",
];

#[test]
fn sheet_geometry_family_fields_match_registry_redecode() {
    let mut checked_sheets = 0usize;
    let mut checked_fixtures = 0usize;

    for fixture in FIXTURES {
        let path = format!("test-file/{fixture}");
        if !std::path::Path::new(&path).exists() {
            eprintln!("skipping: fixture {path} not found");
            continue;
        }
        let pkg = PidParser::new()
            .parse_package(&path)
            .unwrap_or_else(|err| panic!("fixture {path} should parse: {err}"));
        checked_fixtures += 1;

        for sheet in &pkg.parsed.sheet_streams {
            let Some(raw) = pkg.streams.get(&sheet.path) else {
                continue;
            };
            let bytes = raw.data.as_slice();
            checked_sheets += 1;

            match &sheet.geometry {
                Some(geometry) => {
                    for family in SHEET_RECORD_FAMILIES {
                        let wired = (family.record_count)(geometry);
                        let redecoded = (family.decoded_ranges)(bytes).len();
                        assert_eq!(
                            wired, redecoded,
                            "{fixture} {}: family {} has {wired} wired record(s) in \
                             SheetGeometry but {redecoded} record(s) on raw re-decode — \
                             cluster wiring and decoder disagree",
                            sheet.path, family.name
                        );
                    }
                }
                None => {
                    // No geometry evidence at all: every family must
                    // also re-decode to zero records, otherwise the
                    // cluster pass dropped a populated sheet.
                    for family in SHEET_RECORD_FAMILIES {
                        let redecoded = (family.decoded_ranges)(bytes).len();
                        assert_eq!(
                            redecoded, 0,
                            "{fixture} {}: sheet has no SheetGeometry but family {} \
                             re-decodes {redecoded} record(s) from raw bytes",
                            sheet.path, family.name
                        );
                    }
                }
            }
        }
    }

    if checked_fixtures == 0 {
        eprintln!("skipping: no local fixtures available for wiring consistency check");
        return;
    }
    eprintln!(
        "wiring consistency verified on {checked_sheets} sheet stream(s) across \
         {checked_fixtures} fixture(s) x {} families",
        SHEET_RECORD_FAMILIES.len()
    );
    assert!(
        checked_sheets > 0,
        "fixtures parsed but no sheet streams were checked"
    );
}

/// Every drawing and border template the repository commits under
/// `test-file`, by walking it: `.pid` and `.igr` alike, since a template is
/// read through the same families.
fn committed_documents(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<std::path::PathBuf> =
        entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            // A machine-local symbol library a checkout may link in; its
            // `.sym` bodies are not read through the sheet families.
            if path.file_name().is_some_and(|name| name == "symbols-full") {
                continue;
            }
            committed_documents(&path, out);
        } else if path.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("pid") || extension.eq_ignore_ascii_case("igr")
        }) {
            out.push(path);
        }
    }
}

/// A family decodes the records its stream's chain starts, and nothing else.
///
/// A `Sheet*` stream is an 8-byte header and then records end to end; the
/// native reader walks them in that order and reads nothing else as a
/// record. A decode that starts anywhere a chain record does not is a byte
/// pattern inside some other record's payload — and across the corpora that
/// was all a decoder could accept from three whole families: `0x0010`
/// "sub-records" were a graphic record's own sub-type word and style id,
/// "attribute fragments" the same envelope inside a text box, and
/// `JStyleOverride` the length word (`0x0030` = 48) of the block every
/// `DependencyObject` ends with. `igSmartFrame2d` took thirteen more from
/// any field whose value is 61. See
/// `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
/// §3; the corpus-wide measurement is
/// `examples/probe_which_decoded_records_start_off_the_chain.rs`.
///
/// Held on every committed drawing and template, so it runs wherever the
/// repository is checked out.
#[test]
fn every_decoded_record_starts_where_its_streams_chain_starts_one() {
    use pid_parse::parsers::sheet_records::{parse_psm_header, sheet_record_starts};
    use std::collections::BTreeMap;

    let mut documents = Vec::new();
    committed_documents(std::path::Path::new("test-file"), &mut documents);
    let drawings = documents
        .iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pid"))
        })
        .count();
    assert!(
        drawings >= 7,
        "{drawings} committed drawings under test-file; the fixture set is seven"
    );

    let mut streams = 0usize;
    let mut decoded = 0usize;
    let mut off_chain: Vec<String> = Vec::new();
    for path in &documents {
        let package = PidParser::new()
            .parse_package(path)
            .unwrap_or_else(|error| panic!("{} should parse: {error}", path.display()));
        for (stream, raw) in &package.streams {
            if !stream
                .rsplit('/')
                .next()
                .is_some_and(|leaf| leaf.starts_with("Sheet"))
            {
                continue;
            }
            let data = raw.data.as_slice();
            let chain: BTreeMap<usize, usize> = sheet_record_starts(data)
                .into_iter()
                .filter_map(|at| {
                    let header = parse_psm_header(data, at)?;
                    Some((at, header.body_start + header.bytes_to_follow as usize))
                })
                .collect();
            // Every committed sheet stream is a chain or its header alone;
            // one that is neither has nothing to hold a decode against, and
            // then no family may decode from it at all.
            assert!(
                !chain.is_empty() || data.len() <= 8 || {
                    SHEET_RECORD_FAMILIES
                        .iter()
                        .all(|family| (family.decoded_ranges)(data).is_empty())
                },
                "{} {stream}: the chain does not walk and a family still decodes from it",
                path.display()
            );
            streams += 1;
            for family in SHEET_RECORD_FAMILIES {
                for range in (family.decoded_ranges)(data) {
                    decoded += 1;
                    if chain.get(&range.start) != Some(&range.end) {
                        off_chain.push(format!(
                            "{} {stream} +{}..+{}: {} (0x{:04X})",
                            path.display(),
                            range.start,
                            range.end,
                            family.name,
                            family.type_code
                        ));
                    }
                }
            }
        }
    }
    eprintln!(
        "{decoded} decoded record(s) over {streams} sheet stream(s) of {} document(s), {} off the chain",
        documents.len(),
        off_chain.len()
    );
    assert!(streams > 0, "no sheet stream was checked");
    assert!(
        off_chain.is_empty(),
        "{} decoded record(s) do not start where the stream's chain starts one; the first ten:\n{}",
        off_chain.len(),
        off_chain
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
