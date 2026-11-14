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

/// What the association records of a set of documents add up to.
#[derive(Debug, Default, PartialEq)]
struct AssociationTally {
    /// `ComplexString` records.
    strings: usize,
    /// Their curve members.
    curves: usize,
    /// Their constraint members.
    constraints: usize,
    /// Group records.
    groups: usize,
    /// The groups' members, by family: lines, arcs, B-splines, elliptical
    /// arcs, boundaries, points, rectangles.
    group_members: [usize; 7],
    /// Records whose range scale is not one, strings and groups together.
    scaled: usize,
    /// Line members whose range box was held against the line.
    boxes_held: usize,
    /// Of those, the ones whose box is the line only at the stated scale:
    /// read in 1/32768 m it is not within the slack of the line.
    boxes_off_at_scale_one: usize,
}

/// Hold every `ComplexString` and Group of one document's sheet streams to
/// its members; see the test below for what is held.
fn hold_associations(
    label: &str,
    package: &pid_parse::PidPackage,
    tally: &mut AssociationTally,
    hosts: &mut Vec<String>,
    wrong: &mut Vec<String>,
) {
    use pid_parse::parsers::sheet_records::{
        decode_igcomplexstrings, decode_iggroups, decode_igline_at, parse_psm_header,
        sheet_record_starts, SheetAssociationMember, IGCOMPLEXSTRING2D_CONSTRAINT_MEMBER,
    };
    use std::collections::BTreeMap;

    const LINE: u16 = 0x0018;
    const ARC: u16 = 0x0061;
    const KEY_POINT: u16 = 0x0019;
    /// The families a group's members come from, in the tally's order.
    const GROUPED: [u16; 7] = [LINE, ARC, 0x005D, 0x007E, 0x0013, 0x005E, 0x0020];
    /// How far a line's range box may sit from the line: half the widest
    /// pen a template strokes a frame with, and the box's own resolution.
    const BOX_SLACK_M: f64 = 0.001;

    /// What the chain says of one record: where it starts, its type code,
    /// whether the native reader skips it, and the owner / layer / style
    /// words of its payload.
    struct Entry {
        at: usize,
        type_code: u16,
        skipped: bool,
        owner: u32,
        layer: u32,
        style: u32,
    }

    let word = |payload: &[u8], at: usize| {
        payload
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };

    for (stream, raw) in &package.streams {
        if !stream
            .rsplit('/')
            .next()
            .is_some_and(|leaf| leaf.starts_with("Sheet"))
        {
            continue;
        }
        let data = raw.data.as_slice();
        let mut by_oid: BTreeMap<u32, Vec<Entry>> = BTreeMap::new();
        for at in sheet_record_starts(data) {
            let Some(header) = parse_psm_header(data, at) else {
                continue;
            };
            let end = header.body_start + header.bytes_to_follow as usize;
            let Some(payload) = data.get(header.body_start..end) else {
                continue;
            };
            let Some(oid) = word(payload, 0) else {
                continue;
            };
            by_oid.entry(oid).or_default().push(Entry {
                at,
                type_code: header.type_code,
                skipped: data.get(at + 1).is_some_and(|high| high & 0x80 != 0),
                owner: word(payload, 4).unwrap_or(u32::MAX),
                layer: word(payload, 8).unwrap_or(u32::MAX),
                style: word(payload, 14).unwrap_or(u32::MAX),
            });
        }
        let site = |oid: u32| format!("{label} {stream} oid {oid}");
        // The one live record of this oid, or why there is not one.
        let member = |oid: u32| -> Result<&Entry, String> {
            let live: Vec<&Entry> = by_oid
                .get(&oid)
                .map(|entries| entries.iter().filter(|entry| !entry.skipped).collect())
                .unwrap_or_default();
            match live.as_slice() {
                [one] => Ok(one),
                other => Err(format!("{} live record(s) of oid {oid}", other.len())),
            }
        };
        // A line member's range box, read in the listing record's own unit,
        // is the line's extent to within the slack. The box is on a grid of
        // that unit and a template string's is grown by half its pen, so
        // the two differ by a fraction of a millimetre; read in the other
        // unit the corpora use, the box is half or double the size and
        // hundreds of millimetres off.
        let hold_box = |owner: u32,
                        entry: &SheetAssociationMember,
                        found: &Entry,
                        range_scale: f64,
                        wrong: &mut Vec<String>|
         -> Option<bool> {
            if found.type_code != LINE {
                return None;
            }
            let Some(line) = decode_igline_at(data, found.at) else {
                wrong.push(format!(
                    "{}: line member {} does not decode",
                    site(owner),
                    entry.oid
                ));
                return None;
            };
            let extent = [
                line.start.0.min(line.end.0),
                line.start.1.min(line.end.1),
                line.start.0.max(line.end.0),
                line.start.1.max(line.end.1),
            ];
            let near = |boxed: [f64; 4]| {
                boxed
                    .iter()
                    .zip(&extent)
                    .all(|(boxed, own)| (boxed - own).abs() <= BOX_SLACK_M)
            };
            let boxed = entry.range_box_m(range_scale);
            if !near(boxed) {
                wrong.push(format!(
                    "{}: member {}'s range box reads {boxed:?} m at scale {range_scale}, \
                     the line runs {extent:?}",
                    site(owner),
                    entry.oid
                ));
            }
            // Whether the unit matters to this box.
            Some(!near(entry.range_box_m(1.0)))
        };

        let decoded_strings = decode_igcomplexstrings(data);
        let decoded_groups = decode_iggroups(data);
        if !decoded_strings.is_empty() || !decoded_groups.is_empty() {
            hosts.push(format!(
                "{label} {stream}: {} string(s), {} group(s)",
                decoded_strings.len(),
                decoded_groups.len()
            ));
        }
        for string in &decoded_strings {
            tally.strings += 1;
            if string.range_scale != 1.0 {
                tally.scaled += 1;
            }
            let mut listed: Vec<u32> = Vec::new();
            for entry in &string.members {
                let found = match member(entry.oid) {
                    Ok(found) => found,
                    Err(why) => {
                        wrong.push(format!("{}: member {why}", site(string.oid)));
                        continue;
                    }
                };
                if found.owner != string.oid {
                    wrong.push(format!(
                        "{}: member {} is owned by {}",
                        site(string.oid),
                        entry.oid,
                        found.owner
                    ));
                }
                if entry.word == IGCOMPLEXSTRING2D_CONSTRAINT_MEMBER {
                    tally.constraints += 1;
                    if found.type_code != KEY_POINT {
                        wrong.push(format!(
                            "{}: constraint member {} is a 0x{:04X}",
                            site(string.oid),
                            entry.oid,
                            found.type_code
                        ));
                    }
                } else {
                    tally.curves += 1;
                    listed.push(entry.oid);
                    if !matches!(found.type_code, LINE | ARC) {
                        wrong.push(format!(
                            "{}: curve member {} is a 0x{:04X}",
                            site(string.oid),
                            entry.oid,
                            found.type_code
                        ));
                    }
                    if (found.layer, found.style) != (string.sheet_layer_ref, string.index) {
                        wrong.push(format!(
                            "{}: curve member {} is on layer {} with style {}, the string on \
                             layer {} with style {}",
                            site(string.oid),
                            entry.oid,
                            found.layer,
                            found.style,
                            string.sheet_layer_ref,
                            string.index
                        ));
                    }
                    if let Some(off) = hold_box(string.oid, entry, found, string.range_scale, wrong)
                    {
                        tally.boxes_held += 1;
                        tally.boxes_off_at_scale_one += usize::from(off);
                    }
                }
            }
            // The list is the whole string: no curve claims it from
            // outside.
            for (oid, entries) in &by_oid {
                for entry in entries {
                    if !entry.skipped
                        && entry.owner == string.oid
                        && matches!(entry.type_code, LINE | ARC)
                        && !listed.contains(oid)
                    {
                        wrong.push(format!(
                            "{}: 0x{:04X} oid {oid} names the string and is not on its list",
                            site(string.oid),
                            entry.type_code
                        ));
                    }
                }
            }
        }
        for group in &decoded_groups {
            tally.groups += 1;
            if group.range_scale != 1.0 {
                tally.scaled += 1;
            }
            for entry in &group.members {
                match member(entry.oid) {
                    Ok(found) => {
                        if found.owner != group.oid {
                            wrong.push(format!(
                                "{}: member {} is owned by {}",
                                site(group.oid),
                                entry.oid,
                                found.owner
                            ));
                        }
                        match GROUPED.iter().position(|code| *code == found.type_code) {
                            Some(family) => tally.group_members[family] += 1,
                            None => wrong.push(format!(
                                "{}: member {} is a 0x{:04X}, a family no group of the \
                                 corpora lists",
                                site(group.oid),
                                entry.oid,
                                found.type_code
                            )),
                        }
                        if let Some(off) =
                            hold_box(group.oid, entry, found, group.range_scale, wrong)
                        {
                            tally.boxes_held += 1;
                            tally.boxes_off_at_scale_one += usize::from(off);
                        }
                    }
                    Err(why) => wrong.push(format!("{}: member {why}", site(group.oid))),
                }
            }
        }
    }
}

/// A ComplexString or a Group draws through the records it lists, and lists
/// every record that names it.
///
/// Neither record carries a coordinate. What stands in for one is the
/// members' own records, so that is what this holds, on every committed
/// drawing and template — the ones on disk, and the border templates inside
/// the committed backups' `RefData~*.zip`:
///
/// * each member is a live record on the same stream's chain whose owner
///   word (payload `+4`) is the association's oid;
/// * a string's curve members are `igLine2d` / `igArc2d` on the string's
///   layer with the string's style id — the pen the string would stroke
///   them with is the pen they stroke themselves with — and its other
///   members are the `KeyPoint Constraint` records that join them;
/// * no line or arc of the stream names a string as its owner without being
///   on its list, so the list is the whole string;
/// * a group's members are records of families that draw, or deliberately
///   do not, on their own: the two rectangles of a border, or the lines,
///   arcs, B-splines, elliptical arcs, boundaries and point of a logo;
/// * the range box of every line member is the line's own extent to within
///   a millimetre when it is read in the unit the record states,
///   `range_scale / 32768` m. The scale is not always one.
///
/// That is the ground for the two families emitting nothing: every stroke
/// is already a record that emits. See
/// `docs/analysis/2026-11-10-what-the-sixty-unreferenced-pages-leave-undrawn.md`
/// §10.4 and §16.
#[test]
fn a_complex_string_and_a_group_own_the_records_that_draw_them() {
    let mut documents = Vec::new();
    committed_documents(std::path::Path::new("test-file"), &mut documents);

    let mut hosts: Vec<String> = Vec::new();
    let mut wrong: Vec<String> = Vec::new();
    let mut on_disk = AssociationTally::default();
    for path in &documents {
        let package = PidParser::new()
            .parse_package(path)
            .unwrap_or_else(|error| panic!("{} should parse: {error}", path.display()));
        hold_associations(
            &path.display().to_string(),
            &package,
            &mut on_disk,
            &mut hosts,
            &mut wrong,
        );
    }

    // The border templates a committed backup keeps in its reference data,
    // each distinct file once: two packages carry the same archive.
    #[cfg_attr(not(feature = "refdata-archive"), allow(unused_mut))]
    let mut archived = AssociationTally::default();
    #[cfg_attr(not(feature = "refdata-archive"), allow(unused_mut))]
    let mut templates = 0usize;
    #[cfg(feature = "refdata-archive")]
    {
        use pid_parse::refdata_archive::{archive_members, read_archive_file};
        let mut seen: Vec<Vec<u8>> = Vec::new();
        let mut archives = Vec::new();
        committed_archives(std::path::Path::new("test-file"), &mut archives);
        for archive in &archives {
            let members = archive_members(archive, "igr")
                .unwrap_or_else(|error| panic!("{}: {error}", archive.display()));
            for member in members {
                let label = member.label().display().to_string();
                let bytes =
                    read_archive_file(&member).unwrap_or_else(|error| panic!("{label}: {error}"));
                if seen.contains(&bytes) {
                    continue;
                }
                let package = pid_parse::PidPackage::from_bytes(&bytes)
                    .unwrap_or_else(|error| panic!("{label} should parse: {error}"));
                seen.push(bytes);
                templates += 1;
                hold_associations(&label, &package, &mut archived, &mut hosts, &mut wrong);
            }
        }
    }

    for host in &hosts {
        eprintln!("{host}");
    }
    eprintln!(
        "on disk, {} document(s): {on_disk:?}\nin the archives, {templates} template(s): \
         {archived:?}",
        documents.len()
    );
    assert!(
        wrong.is_empty(),
        "{} member(s) are not what their association takes them for:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    // `CPECCHB-SQ-A2.igr`'s three strings — the inner frame (four lines and
    // four constraints), the title block's right and bottom edges (three and
    // two) and its top and left (two and one) — and the Group of the two
    // rectangles of `A01.pid`'s embedded border. All at scale one.
    assert_eq!(
        on_disk,
        AssociationTally {
            strings: 3,
            curves: 9,
            constraints: 7,
            groups: 1,
            group_members: [0, 0, 0, 0, 0, 0, 2],
            scaled: 0,
            boxes_held: 9,
            boxes_off_at_scale_one: 0,
        },
        "the association records of the committed documents on disk"
    );
    #[cfg(feature = "refdata-archive")]
    assert_eq!(
        archived, ARCHIVED_ASSOCIATIONS,
        "the association records of the templates in the committed archives"
    );
}

/// The records of one document that list children with a range box — its
/// sheets' own tables, its strings and its groups — each as `(what, oid,
/// scale it states, distance in metres from the origin to the far corner of
/// the box that holds its children)`, with the place it was read at.
fn range_scales(
    label: &str,
    package: &pid_parse::PidPackage,
) -> Vec<(String, &'static str, u32, f64, f64)> {
    use pid_parse::parsers::sheet_display_order::find_sheet_child_table;
    use pid_parse::parsers::sheet_records::{
        decode_igcomplexstrings, decode_iggroups, parse_live_psm_header, sheet_record_starts,
    };

    /// The far corner of the boxes that state an extent, in units.
    fn far_units(boxes: impl Iterator<Item = [i16; 4]>) -> Option<f64> {
        let mut far: Option<(i32, i32)> = None;
        for range in boxes {
            // A child with no extent states an inverted box.
            if range[0] > range[2] || range[1] > range[3] {
                continue;
            }
            let x = i32::from(range[0]).abs().max(i32::from(range[2]).abs());
            let y = i32::from(range[1]).abs().max(i32::from(range[3]).abs());
            far = Some(far.map_or((x, y), |(held_x, held_y)| (held_x.max(x), held_y.max(y))));
        }
        far.map(|(x, y)| f64::from(x).hypot(f64::from(y)))
    }

    let mut found = Vec::new();
    for (stream, raw) in &package.streams {
        let Some(cut) = stream.rfind('/') else {
            continue;
        };
        let (storage, leaf) = (&stream[..cut], &stream[cut + 1..]);
        let Some(sheet_oid) = leaf
            .strip_prefix("Sheet")
            .and_then(|number| number.parse::<u32>().ok())
        else {
            continue;
        };
        let data = raw.data.as_slice();
        let place = format!("{label} {stream}");
        let mut hold = |what: &'static str, oid: u32, scale: f64, far: Option<f64>| {
            if let Some(far) = far {
                found.push((place.clone(), what, oid, scale, far * scale / 32768.0));
            }
        };
        for string in decode_igcomplexstrings(data) {
            let far = far_units(string.members.iter().map(|member| member.range_box));
            hold("string", string.oid, string.range_scale, far);
        }
        for group in decode_iggroups(data) {
            let far = far_units(group.members.iter().map(|member| member.range_box));
            hold("group", group.oid, group.range_scale, far);
        }
        // The sheet's own table: the one whose children are records of this
        // stream.
        let live: std::collections::HashSet<u32> = sheet_record_starts(data)
            .into_iter()
            .filter_map(|at| {
                let header = parse_live_psm_header(data, at)?;
                let oid = data.get(header.body_start..header.body_start + 4)?;
                Some(u32::from_le_bytes([oid[0], oid[1], oid[2], oid[3]]))
            })
            .collect();
        let table = package
            .streams
            .get(&format!("{storage}/PSMcluster0"))
            .and_then(|cluster| {
                find_sheet_child_table(&cluster.data, sheet_oid, |table| {
                    table.children.iter().all(|child| live.contains(&child.oid))
                })
            });
        if let Some(table) = table {
            let far = far_units(table.children.iter().map(|child| child.range));
            hold("sheet", sheet_oid, table.range_scale, far);
        }
    }
    found
}

/// The scale a record states for its children's range boxes is set by how
/// far from the origin they reach: `1.0` while the far corner of the box
/// that holds them is less than a metre away, `2.0` from one metre to two.
///
/// Three records carry such a list — a sheet's own (`0x0114 JSheet`), a
/// `ComplexString` and a Group — and the rule is the same for all three. It
/// is how far the children reach that decides, not the paper, the template,
/// the layer, the owner, the sheet's own scale or the size of what is
/// listed:
/// `A1-W.igr` keeps the group of its border, which reaches (841, 594) mm,
/// at `2.0`, and the group of its logo, 14 mm long and 718 mm out, at `1.0`
/// — on one sheet, under one owner, on no layer; and `A0-W.igr`'s logo, the
/// same 14 mm, sits 1 064 mm out and states `2.0`.
///
/// Held on every committed drawing and template, the templates inside the
/// committed archives included. The corpora outside the repository hold the
/// two records nearest the metre — a sheet at 998.0 mm under `1.0` and a
/// string at 1 015.9 mm under `2.0` — and no record that breaks the rule:
/// `examples/probe_what_decides_the_scale_of_a_range_box.rs`, 1 293 records
/// of 885 documents; see
/// `docs/analysis/2026-11-12-a-range-scale-is-set-by-how-far-the-children-reach.md`.
///
/// What is held is the rule that fits them, and it is a pair: the reach
/// measured as the straight-line distance to the far corner, and the line
/// at one metre. The readings do not give either half alone — the far
/// corner's p-norms part the two scales for every p from 1.63 up, and it is
/// the line at a round metre that leaves p between 1.99 and 2.10 (the
/// note, §2.4).
#[test]
fn a_range_scale_is_set_by_how_far_the_children_reach() {
    let mut documents = Vec::new();
    committed_documents(std::path::Path::new("test-file"), &mut documents);
    let mut found = Vec::new();
    for path in &documents {
        let package = PidParser::new()
            .parse_package(path)
            .unwrap_or_else(|error| panic!("{} should parse: {error}", path.display()));
        found.extend(range_scales(&path.display().to_string(), &package));
    }
    #[cfg(feature = "refdata-archive")]
    {
        use pid_parse::refdata_archive::{archive_members, read_archive_file};
        let mut seen: Vec<Vec<u8>> = Vec::new();
        let mut archives = Vec::new();
        committed_archives(std::path::Path::new("test-file"), &mut archives);
        for archive in &archives {
            let members = archive_members(archive, "igr")
                .unwrap_or_else(|error| panic!("{}: {error}", archive.display()));
            for member in members {
                let label = member.label().display().to_string();
                let bytes =
                    read_archive_file(&member).unwrap_or_else(|error| panic!("{label}: {error}"));
                if seen.contains(&bytes) {
                    continue;
                }
                let package = pid_parse::PidPackage::from_bytes(&bytes)
                    .unwrap_or_else(|error| panic!("{label} should parse: {error}"));
                seen.push(bytes);
                found.extend(range_scales(&label, &package));
            }
        }
    }

    let mut wrong: Vec<String> = Vec::new();
    let mut tally: std::collections::BTreeMap<(&str, String), usize> =
        std::collections::BTreeMap::new();
    for (place, what, oid, scale, far) in &found {
        let called_for = if *far < 1.0 { 1.0 } else { 2.0 };
        if *scale != called_for || *far >= 2.0 {
            wrong.push(format!(
                "{place}: {what} {oid} states {scale} with its far corner {:.1} mm out",
                far * 1000.0
            ));
        }
        *tally.entry((what, format!("{scale}"))).or_default() += 1;
    }
    eprintln!("range scales of the committed documents: {tally:?}");
    assert!(
        wrong.is_empty(),
        "{} record(s) state a scale their children's reach does not call for:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    // Both scales are among the sheets read, so the rule was put to both.
    let stated = |what: &str, scale: &str| {
        tally
            .get(&(what, scale.to_owned()))
            .copied()
            .unwrap_or_default()
    };
    assert!(
        stated("sheet", "1") >= 6 && stated("sheet", "2") >= 4,
        "sheets of both scales: {tally:?}"
    );
    assert_eq!(stated("string", "1"), 3, "`CPECCHB-SQ-A2.igr`'s three");

    // One sheet, one owner, no layer, two scales: the border and the logo
    // of `A1-W.igr`.
    #[cfg(feature = "refdata-archive")]
    {
        assert_eq!(
            (stated("group", "1"), stated("group", "2")),
            (8, 5),
            "the groups of the archived templates and of `A01.pid`'s border: {tally:?}"
        );
        let of_a1: Vec<(u32, f64, f64)> = found
            .iter()
            .filter(|(place, what, ..)| *what == "group" && place.contains("A1-W.igr"))
            .map(|(_, _, oid, scale, far)| (*oid, *scale, (far * 1000.0).round()))
            .collect();
        assert_eq!(
            of_a1,
            [(93, 2.0, 1030.0), (369, 1.0, 718.0)],
            "`A1-W.igr`'s border and logo"
        );
    }
}

/// Every `RefData~*.zip` the repository commits under `dir`, path-sorted.
#[cfg(feature = "refdata-archive")]
fn committed_archives(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<std::path::PathBuf> =
        entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "symbols-full") {
                continue;
            }
            committed_archives(&path, out);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("RefData~") && name.ends_with(".zip"))
        {
            out.push(path);
        }
    }
}

/// What the eleven templates inside the committed archives hold: no string,
/// and twelve groups. Each of the eight `A?-W` templates has the group of
/// its border's two rectangles; `A0-W.igr` to `A3-W.igr` each have, besides,
/// the 37 records of a logo — sixteen lines, seven arcs, four B-splines,
/// four elliptical arcs, five boundaries and a point. Five of the twelve
/// state a range scale of two: the borders of the two A0 and the two A1
/// templates, and `A0-W.igr`'s logo — whose sixteen lines are the members
/// that show the scale is the unit: read in 1/32768 m, not one of their
/// boxes is within a millimetre of its line.
#[cfg(feature = "refdata-archive")]
const ARCHIVED_ASSOCIATIONS: AssociationTally = AssociationTally {
    strings: 0,
    curves: 0,
    constraints: 0,
    groups: 12,
    group_members: [64, 28, 16, 16, 20, 4, 16],
    scaled: 5,
    boxes_held: 64,
    boxes_off_at_scale_one: 16,
};
