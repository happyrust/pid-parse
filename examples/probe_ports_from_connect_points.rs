//! Where each connect-point record hangs, and whether the port rule matches.
//!
//! N4-2 step 1 (`docs/plans/2026-10-11-publish-native-evidence-via-ida-bridge.md`).
//! Read-only. `cargo run --example probe_ports_from_connect_points`.
//!
//! `PipingPoint` / `SignalPoint` are not written into `P&IDAttributes`
//! (`docs/analysis/n2b-identities-the-drawing-does-not-write.md`). Their
//! `_ConnectPoint` rows sit on the symbol definition. This probe:
//!
//! * lists every `_ConnectPoint` field, and every field whose name contains
//!   `Flow` or `PointNumber`, so the ordinal's home is measured rather than
//!   assumed;
//! * hangs each row on the placed item whose `definition_site_ref` /
//!   `definition_sheet_ref` (`igSymbol2d`) is that definition;
//! * rebuilds `PIDPipingPort` / `PIDSignalPort` UIDs as
//!   `ModelID . Str(point number)` (`ConstructUIDForPort`, `0x5F5CFE70`)
//!   and scores every candidate number field against the reference;
//! * rebuilds `PIDProcessPoint` as `PipeRun ModelID . PPT`
//!   (`ConstructUIDForProcessPoint`, `0x5F5CFD40`) — these are not connect
//!   points, so they are counted in their own block;
//! * rebuilds `PPC` / `SPC` / `PRP` as `prefix-UID1-UID2` with UID1 the
//!   derived object and UID2 the parent (the reference `_Data.xml` order);
//! * maps `FlowDirection` ordinals through `PortFlowDirectionConv`
//!   (`0x5F583230`) and compares the 298 published values.
//!
//! A class whose wrong + extra + missing is not 0 is printed `HOLD` and
//! must not be loaded. Nothing is written.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};

use pid_parse::parsers::attribute_rows::{decode_attribute_row, AttributeValue};
use pid_parse::parsers::sheet_records::{
    decode_igsymbol_at, parse_live_psm_header, sheet_record_starts,
};
use quick_xml::events::Event;
use quick_xml::Reader;

const DEFAULT_CORPUS: &str = r"D:\work\cad\PID自研软件验证";
const CORPUS_ENV: &str = "PID_PARSE_SELF_CHECK_DIR";
const CHAIN_MAGIC: u32 = 0x6C90_F544;
const FREE_FORM_ATTR_SET: u16 = 0x0089;
const SYMBOL: u16 = 0x00CE;

const FLOW_1: &str = "@{23F94152-B87D-11D6-80D4-00A0C92487AF}";
const FLOW_2: &str = "@{23F94146-B87D-11D6-80D4-00A0C92487AF}";
const FLOW_3: &str = "@{23F9414C-B87D-11D6-80D4-00A0C92487AF}";
const FLOW_4: &str = "@{BCB3F097-66CA-487B-9AF4-D8FCF18624A3}";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Piping,
    Signal,
    Duct,
    Other,
}

struct PointHead {
    parent: u32,
    sub_type: u16,
    index: u32,
    len: u32,
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct PointRow {
    site: u32,
    described: u32,
    described_type: u16,
    parent: u32,
    parent_type: u16,
    sub_type: u16,
    index: u32,
    payload_len: u32,
    bytes: Vec<u8>,
    ancestors: Vec<u32>,
    fields: BTreeMap<String, String>,
    kind: Kind,
}

struct Placement {
    definition_site_ref: u32,
    definition_sheet_ref: u32,
    model_id: Option<String>,
}

struct Item {
    model_id: String,
    model_item_type: String,
}

/// uid -> (name, flow). Flow is empty when the reference omits it.
struct RefSide {
    ports: BTreeMap<String, (String, String, String)>,
    process: BTreeMap<String, (String, String)>,
    rels: BTreeMap<String, (String, String, String)>,
}

struct Diff {
    same: usize,
    wrong: usize,
    extra: usize,
    missing: usize,
    missing_ids: Vec<String>,
    extra_ids: Vec<String>,
    wrong_ids: Vec<String>,
}

fn main() {
    let drawings = corpus();
    if drawings.is_empty() {
        println!("skip: no <stem>.pid beside a <stem>_Data.xml (set {CORPUS_ENV})");
        return;
    }
    let mut total_flow_ref = 0usize;
    let mut total_flow_same = 0usize;
    for (stem, pid, reference) in &drawings {
        match survey(stem, pid, reference) {
            Some((flow_ref, flow_same)) => {
                total_flow_ref += flow_ref;
                total_flow_same += flow_same;
            }
            None => println!("skip: {stem} did not open"),
        }
    }
    println!("corpus ConnectionFlowDirection ref={total_flow_ref} same={total_flow_same}");
}

fn corpus() -> Vec<(String, PathBuf, PathBuf)> {
    let corpus =
        std::env::var_os(CORPUS_ENV).map_or_else(|| PathBuf::from(DEFAULT_CORPUS), PathBuf::from);
    let mut drawings: Vec<(String, PathBuf, PathBuf)> = std::fs::read_dir(&corpus)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = name.strip_suffix("_Data.xml")?.to_string();
            let pid = corpus.join(format!("{stem}.pid"));
            pid.is_file().then(|| (stem, pid, entry.path()))
        })
        .collect();
    drawings.sort();
    drawings
}

fn survey(stem: &str, pid: &Path, reference: &Path) -> Option<(usize, usize)> {
    let xml = std::fs::read_to_string(reference).ok()?;
    let refs = reference_side(&xml);
    let loaded = load_pid(pid)?;
    println!("== {stem} ==");
    print_census(&loaded);
    let linked = link_points(&loaded);
    println!(
        "link definition_sheet: points {} hit_by_a_placement {} items_with_points {} untyped {}",
        loaded.points.len(),
        linked.points_hit,
        linked.by_item.len(),
        loaded
            .points
            .iter()
            .filter(|p| p.kind == Kind::Other)
            .count()
    );
    untyped_overlap(&linked, &refs);
    sibling_sets(&linked, &loaded.items, &refs);
    let (number_field, scores) = best_number_field(&linked, &refs);
    println!("number_field scores (matched piping+signal, extra):");
    for (name, matched, extra) in scores.iter().take(8) {
        let mark = if *name == number_field { "*" } else { " " };
        println!("  {mark} {name}: matched={matched} extra={extra}");
    }
    let flow_field = flow_field_of(&linked, &number_field, &refs);
    let got = with_process(
        &apply_flow(
            &linked,
            &materialise(&linked, &number_field),
            &number_field,
            flow_field.as_deref(),
        ),
        &loaded.items,
    );
    println!(
        "flow_field: {}",
        flow_field.as_deref().unwrap_or("(none matched)")
    );

    let piping_ref = filter_class(&refs.ports, "PIDPipingPort");
    let signal_ref = filter_class(&refs.ports, "PIDSignalPort");
    let piping = diff_objects(&piping_ref, &got.piping);
    let signal = diff_objects(&signal_ref, &got.signal);
    let process = diff_objects(&refs.process, &got.process);
    println!(
        "PIDPipingPort ref={} {}",
        piping_ref.len(),
        verdict("PIDPipingPort", &piping)
    );
    println!(
        "PIDSignalPort ref={} {}",
        signal_ref.len(),
        verdict("PIDSignalPort", &signal)
    );
    println!(
        "PIDProcessPoint ref={} {}",
        refs.process.len(),
        verdict("PIDProcessPoint", &process)
    );
    print_samples("piping", &piping);
    print_samples("signal", &signal);
    print_samples("process", &process);

    let flow = flow_diff(&piping_ref, &got.piping);
    println!(
        "ConnectionFlowDirection ref_with_value={} same={} wrong={} missing_value={} extra_value={}",
        flow.0, flow.1, flow.2, flow.3, flow.4
    );

    let rels = diff_rels(&refs.rels, &got.rels);
    println!("{}", verdict("PPC/SPC/PRP", &rels));
    print_samples("rel", &rels);
    Some((flow.0, flow.1))
}

struct Loaded {
    points: Vec<PointRow>,
    placements: Vec<Placement>,
    items: Vec<Item>,
    flow_homes: BTreeMap<(String, String), BTreeMap<String, usize>>,
}

struct Linked {
    by_item: HashMap<String, Vec<usize>>,
    points_hit: usize,
    points: Vec<PointRow>,
}

struct Got {
    piping: BTreeMap<String, (String, String)>,
    signal: BTreeMap<String, (String, String)>,
    process: BTreeMap<String, (String, String)>,
    rels: BTreeMap<String, (String, String, String)>,
}

type SiteParentGroups = BTreeMap<(u32, u32), Vec<(u32, u32, usize)>>;

fn load_pid(path: &Path) -> Option<Loaded> {
    let file = std::fs::File::open(path).ok()?;
    let mut compound = cfb::CompoundFile::open(file).ok()?;
    let stream_paths: Vec<String> = compound
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().replace('\\', "/"))
        .collect();

    let mut parents: HashMap<(u32, u32), u32> = HashMap::new();
    let mut types: HashMap<(u32, u32), u16> = HashMap::new();
    let mut heads: HashMap<(u32, u32), PointHead> = HashMap::new();
    let mut points = Vec::new();
    let mut items = Vec::new();
    let mut graphic_model: HashMap<u32, String> = HashMap::new();
    let mut flow_homes: BTreeMap<(String, String), BTreeMap<String, usize>> = BTreeMap::new();
    let mut seen_item: BTreeSet<(String, String)> = BTreeSet::new();

    for stream_path in &stream_paths {
        let mut data = Vec::new();
        let Ok(mut stream) = compound.open_stream(stream_path.as_str()) else {
            continue;
        };
        if stream.read_to_end(&mut data).is_err()
            || data.get(0..4) != Some(&CHAIN_MAGIC.to_le_bytes()[..])
        {
            continue;
        }
        let site = site_id(stream_path).unwrap_or(0);
        for at in sheet_record_starts(&data) {
            let Some(header) = parse_live_psm_header(&data, at) else {
                continue;
            };
            let Some(payload) = header
                .body_start
                .checked_add(header.bytes_to_follow as usize)
                .and_then(|end| data.get(header.body_start..end))
            else {
                continue;
            };
            let Some(oid) = u32_at(payload, 0) else {
                continue;
            };
            types.insert((site, oid), header.type_code);
            if let Some(parent) = u32_at(payload, 4) {
                parents.insert((site, oid), parent);
            }
            if header.type_code == 0x005E {
                heads.insert(
                    (site, oid),
                    PointHead {
                        parent: u32_at(payload, 4).unwrap_or(0),
                        sub_type: u16_at(payload, 12).unwrap_or(0),
                        index: u32_at(payload, 14).unwrap_or(0),
                        len: header.bytes_to_follow,
                        bytes: payload.to_vec(),
                    },
                );
            }
            if header.type_code != FREE_FORM_ATTR_SET {
                continue;
            }
            let Some(row) = decode_attribute_row(payload) else {
                continue;
            };
            if row.class == "P&IDAttributes" {
                if let Some(model) = item_model(&row) {
                    let item_type = row.text("ModelItemType").unwrap_or_default().to_string();
                    graphic_model.insert(row.described, model.to_string());
                    if seen_item.insert((model.to_string(), item_type.clone())) {
                        items.push(Item {
                            model_id: model.to_string(),
                            model_item_type: item_type,
                        });
                    }
                }
            }
            note_flow(&row.class, site, &row_fields(&row), &mut flow_homes);
            if row.class != "_ConnectPoint" {
                continue;
            }
            let fields = row_fields(&row);
            points.push(PointRow {
                site,
                described: row.described,
                described_type: 0,
                parent: 0,
                parent_type: 0,
                sub_type: 0,
                index: 0,
                payload_len: 0,
                bytes: Vec::new(),
                ancestors: Vec::new(),
                kind: kind_of(&fields),
                fields,
            });
        }
    }

    for point in &mut points {
        point.described_type = types
            .get(&(point.site, point.described))
            .copied()
            .unwrap_or(0);
        point.ancestors = ancestors(&parents, point.site, point.described);
        if let Some(head) = heads.get(&(point.site, point.described)) {
            point.parent = head.parent;
            point.sub_type = head.sub_type;
            point.index = head.index;
            point.payload_len = head.len;
            point.bytes = head.bytes.clone();
            point.parent_type = types.get(&(point.site, head.parent)).copied().unwrap_or(0);
        }
        point
            .fields
            .insert("payload.index".into(), point.index.to_string());
        point
            .fields
            .insert("payload.sub_type".into(), point.sub_type.to_string());
        point
            .fields
            .insert("payload.len".into(), point.payload_len.to_string());
        point
            .fields
            .insert("parent.type".into(), format!("{:#06x}", point.parent_type));
    }
    let mut groups: SiteParentGroups = BTreeMap::new();
    for (index, point) in points.iter().enumerate() {
        groups.entry((point.site, point.parent)).or_default().push((
            point.index,
            point.described,
            index,
        ));
    }
    for members in groups.values_mut() {
        members.sort_unstable();
        for (order, (_, _, index)) in members.iter().enumerate() {
            points[*index]
                .fields
                .insert("sibling.order".into(), (order as u32 + 1).to_string());
        }
    }
    let mut sheet_groups: BTreeMap<(u32, u32), Vec<(u32, u32)>> = BTreeMap::new();
    for ((site, oid), head) in &heads {
        sheet_groups
            .entry((*site, head.parent))
            .or_default()
            .push((head.index, *oid));
    }
    let mut sheet_rank: HashMap<(u32, u32), u32> = HashMap::new();
    for ((site, parent), members) in &mut sheet_groups {
        members.sort_unstable();
        for (order, (_, oid)) in members.iter().enumerate() {
            sheet_rank.insert((*site, *oid), order as u32 + 1);
        }
        let _ = parent;
    }
    for point in &mut points {
        let rank = sheet_rank
            .get(&(point.site, point.described))
            .copied()
            .unwrap_or(0);
        point.fields.insert("sheet.order".into(), rank.to_string());
    }
    let placements = placements_with_models(&stream_paths, &mut compound, &graphic_model);

    Some(Loaded {
        points,
        placements,
        items,
        flow_homes,
    })
}

fn placements_with_models(
    stream_paths: &[String],
    compound: &mut cfb::CompoundFile<std::fs::File>,
    graphic_model: &HashMap<u32, String>,
) -> Vec<Placement> {
    let mut placements = Vec::new();
    for stream_path in stream_paths {
        if site_id(stream_path).is_some() {
            continue;
        }
        let mut data = Vec::new();
        let Ok(mut stream) = compound.open_stream(stream_path.as_str()) else {
            continue;
        };
        if stream.read_to_end(&mut data).is_err()
            || data.get(0..4) != Some(&CHAIN_MAGIC.to_le_bytes()[..])
        {
            continue;
        }
        for at in sheet_record_starts(&data) {
            let Some(header) = parse_live_psm_header(&data, at) else {
                continue;
            };
            if header.type_code != SYMBOL {
                continue;
            }
            let Some(symbol) = decode_igsymbol_at(&data, at) else {
                continue;
            };
            placements.push(Placement {
                definition_site_ref: symbol.definition_site_ref,
                definition_sheet_ref: symbol.definition_sheet_ref,
                model_id: graphic_model.get(&symbol.oid).cloned(),
            });
        }
    }
    placements
}

fn link_points(loaded: &Loaded) -> Linked {
    let mut by_def: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    let mut points_hit = 0usize;
    for (index, point) in loaded.points.iter().enumerate() {
        let mut sheets = BTreeSet::new();
        for placement in &loaded.placements {
            if placement.model_id.is_none() {
                continue;
            }
            if placement.definition_site_ref == point.site
                && point.ancestors.contains(&placement.definition_sheet_ref)
            {
                sheets.insert(placement.definition_sheet_ref);
            }
        }
        if sheets.is_empty() {
            continue;
        }
        points_hit += 1;
        for sheet in sheets {
            by_def.entry((point.site, sheet)).or_default().push(index);
        }
    }
    let mut by_item: HashMap<String, Vec<usize>> = HashMap::new();
    for placement in &loaded.placements {
        let Some(model) = &placement.model_id else {
            continue;
        };
        let Some(indexes) = by_def.get(&(
            placement.definition_site_ref,
            placement.definition_sheet_ref,
        )) else {
            continue;
        };
        by_item
            .entry(model.clone())
            .or_default()
            .extend(indexes.iter().copied());
    }
    for indexes in by_item.values_mut() {
        indexes.sort_unstable();
        indexes.dedup();
    }
    Linked {
        by_item,
        points_hit,
        points: loaded.points.clone(),
    }
}

fn untyped_overlap(linked: &Linked, refs: &RefSide) {
    let piping: BTreeSet<&str> = refs
        .ports
        .iter()
        .filter(|(_, (class, _, _))| class == "PIDPipingPort")
        .map(|(uid, _)| uid.as_str())
        .collect();
    let signal: BTreeSet<&str> = refs
        .ports
        .iter()
        .filter(|(_, (class, _, _))| class == "PIDSignalPort")
        .map(|(uid, _)| uid.as_str())
        .collect();
    println!("untyped uid overlap (kind ignored):");
    for name in [
        "payload.index",
        "sibling.order",
        "sheet.order",
        "payload.sub_type",
        "AngleCount",
        "MaxConnectedLines",
    ] {
        let mut hit_piping = 0usize;
        let mut hit_signal = 0usize;
        let mut both = 0usize;
        let mut extra = 0usize;
        let mut seen = BTreeSet::new();
        let mut by_sub: BTreeMap<u16, (usize, usize)> = BTreeMap::new();
        let mut by_parent: BTreeMap<u16, (usize, usize)> = BTreeMap::new();
        for (model, indexes) in &linked.by_item {
            for index in indexes {
                let Some(point) = linked.points.get(*index) else {
                    continue;
                };
                let Some(number) = point
                    .fields
                    .get(name)
                    .map(|v| v.trim())
                    .filter(|v| !v.is_empty())
                else {
                    continue;
                };
                let uid = format!("{model}.{number}");
                if !seen.insert(uid.clone()) {
                    continue;
                }
                let in_piping = piping.contains(uid.as_str());
                let in_signal = signal.contains(uid.as_str());
                match (in_piping, in_signal) {
                    (true, true) => both += 1,
                    (true, false) => hit_piping += 1,
                    (false, true) => hit_signal += 1,
                    (false, false) => extra += 1,
                }
                if in_piping || in_signal {
                    let sub = by_sub.entry(point.sub_type).or_insert((0, 0));
                    let parent = by_parent.entry(point.parent_type).or_insert((0, 0));
                    if in_piping {
                        sub.0 += 1;
                        parent.0 += 1;
                    }
                    if in_signal {
                        sub.1 += 1;
                        parent.1 += 1;
                    }
                }
            }
        }
        println!(
            "  {name}: piping={hit_piping} signal={hit_signal} both={both} extra={extra} sub_type(piping,signal)={by_sub:?} parent_type={by_parent:?}"
        );
    }
}

fn sibling_sets(linked: &Linked, items: &[Item], refs: &RefSide) {
    let item_type: HashMap<&str, &str> = items
        .iter()
        .map(|item| (item.model_id.as_str(), item.model_item_type.as_str()))
        .collect();
    let mut ref_piping: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    let mut ref_signal: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    let mut ref_flow: HashMap<&str, &str> = HashMap::new();
    for (uid, (class, _, flow)) in &refs.ports {
        let Some((parent, number)) = uid.rsplit_once('.') else {
            continue;
        };
        let bucket = if class == "PIDPipingPort" {
            &mut ref_piping
        } else if class == "PIDSignalPort" {
            &mut ref_signal
        } else {
            continue;
        };
        bucket.entry(parent).or_default().insert(number);
        if class == "PIDPipingPort" && !flow.is_empty() {
            ref_flow.insert(uid.as_str(), flow.as_str());
        }
    }
    let mut exact = 0usize;
    let mut superset = 0usize;
    let mut missing = 0usize;
    let mut extra_numbers = 0usize;
    let mut missing_numbers = 0usize;
    let mut hit_piping = 0usize;
    let mut hit_signal = 0usize;
    let mut opc_extra = 0usize;
    let mut other_extra = 0usize;
    let mut examples = Vec::new();
    let mut votes: BTreeMap<usize, BTreeMap<u8, BTreeMap<String, usize>>> = BTreeMap::new();
    for (model, indexes) in &linked.by_item {
        let mut got = BTreeSet::new();
        for index in indexes {
            let Some(point) = linked.points.get(*index) else {
                continue;
            };
            let Some(number) = point.fields.get("sibling.order") else {
                continue;
            };
            got.insert(number.clone());
            let uid = format!("{model}.{number}");
            if let Some(flow) = ref_flow.get(uid.as_str()) {
                for (offset, byte) in point.bytes.iter().enumerate() {
                    *votes
                        .entry(offset)
                        .or_default()
                        .entry(*byte)
                        .or_default()
                        .entry((*flow).to_string())
                        .or_insert(0) += 1;
                }
            }
        }
        let piping = ref_piping.get(model.as_str()).cloned().unwrap_or_default();
        let signal = ref_signal.get(model.as_str()).cloned().unwrap_or_default();
        let mut wanted = piping.clone();
        wanted.extend(signal.iter().copied());
        if wanted.is_empty() && got.is_empty() {
            continue;
        }
        let extra: Vec<_> = got
            .iter()
            .filter(|number| !wanted.contains(number.as_str()))
            .cloned()
            .collect();
        let miss: Vec<_> = wanted
            .iter()
            .filter(|number| !got.contains(**number))
            .map(|number| (*number).to_string())
            .collect();
        extra_numbers += extra.len();
        missing_numbers += miss.len();
        if item_type.get(model.as_str()).copied() == Some("OPC") {
            opc_extra += extra.len();
        } else {
            other_extra += extra.len();
        }
        for number in &got {
            if piping.contains(number.as_str()) {
                hit_piping += 1;
            }
            if signal.contains(number.as_str()) {
                hit_signal += 1;
            }
        }
        if miss.is_empty() && extra.is_empty() {
            exact += 1;
        } else if miss.is_empty() {
            superset += 1;
        } else {
            missing += 1;
        }
        if examples.len() < 8 && (!miss.is_empty() || !extra.is_empty()) {
            examples.push(format!(
                "{} type={} got={} piping={} signal={} extra={} missing={}",
                model,
                item_type.get(model.as_str()).copied().unwrap_or("?"),
                got.iter().cloned().collect::<Vec<_>>().join(","),
                piping.iter().copied().collect::<Vec<_>>().join(","),
                signal.iter().copied().collect::<Vec<_>>().join(","),
                extra.join(","),
                miss.join(",")
            ));
        }
        if !miss.is_empty() && examples.iter().all(|line| !line.starts_with("HEX ")) {
            for index in indexes {
                let Some(point) = linked.points.get(*index) else {
                    continue;
                };
                let hex = point
                    .bytes
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                examples.push(format!(
                    "HEX {model} order={} parent={} {hex}",
                    point
                        .fields
                        .get("sibling.order")
                        .map(String::as_str)
                        .unwrap_or("?"),
                    point.parent
                ));
            }
        }
    }
    let mut unlinked = 0usize;
    for parent in ref_piping.keys().chain(ref_signal.keys()) {
        if !linked.by_item.contains_key(*parent) {
            unlinked += 1;
        }
    }
    println!(
        "sibling.order sets: exact={exact} superset={superset} missing={missing} extra_numbers={extra_numbers} (opc={opc_extra} other={other_extra}) missing_numbers={missing_numbers} ref_parents_unlinked={unlinked}"
    );
    println!(
        "sibling.order coverage: piping {hit_piping}/{} signal {hit_signal}/{} — ports HOLD (number is not a field; FlowDirection is not in the attribute sets)",
        ref_piping.values().map(|n| n.len()).sum::<usize>(),
        ref_signal.values().map(|n| n.len()).sum::<usize>()
    );
    for line in &examples {
        println!("  {line}");
    }
    let mut ranked: Vec<(usize, usize, usize)> = votes
        .iter()
        .map(|(offset, by_byte)| {
            let pure = by_byte.values().filter(|flows| flows.len() == 1).count();
            let mixed = by_byte.values().filter(|flows| flows.len() > 1).count();
            (*offset, pure, mixed)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
    match ranked.first() {
        Some((offset, pure, mixed)) => println!(
            "flow vs point payload: best byte offset={offset} values_with_one_flow={pure} values_with_many_flows={mixed}"
        ),
        None => println!("flow vs point payload: no sibling.order hit a piping port that has ConnectionFlowDirection"),
    }
}

fn best_number_field(linked: &Linked, refs: &RefSide) -> (String, Vec<(String, usize, usize)>) {
    let mut names = BTreeSet::new();
    for point in &linked.points {
        for name in point.fields.keys() {
            names.insert(name.clone());
        }
    }
    let piping: BTreeSet<&str> = refs
        .ports
        .iter()
        .filter(|(_, (class, _, _))| class == "PIDPipingPort")
        .map(|(uid, _)| uid.as_str())
        .collect();
    let signal: BTreeSet<&str> = refs
        .ports
        .iter()
        .filter(|(_, (class, _, _))| class == "PIDSignalPort")
        .map(|(uid, _)| uid.as_str())
        .collect();
    let mut scores = Vec::new();
    for name in &names {
        let got = materialise(linked, name);
        let matched = got
            .piping
            .keys()
            .filter(|uid| piping.contains(uid.as_str()))
            .count()
            + got
                .signal
                .keys()
                .filter(|uid| signal.contains(uid.as_str()))
                .count();
        let extra = got
            .piping
            .keys()
            .filter(|uid| !piping.contains(uid.as_str()))
            .count()
            + got
                .signal
                .keys()
                .filter(|uid| !signal.contains(uid.as_str()))
                .count();
        scores.push((name.clone(), matched, extra));
    }
    scores.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0)));
    let best = scores
        .first()
        .map(|(n, _, _)| n.clone())
        .unwrap_or_default();
    (best, scores)
}

fn materialise(linked: &Linked, number_field: &str) -> Got {
    let mut piping = BTreeMap::new();
    let mut signal = BTreeMap::new();
    let mut rels = BTreeMap::new();
    for (model, indexes) in &linked.by_item {
        for index in indexes {
            let Some(point) = linked.points.get(*index) else {
                continue;
            };
            let Some(number) = point
                .fields
                .get(number_field)
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
            else {
                continue;
            };
            let (bucket, prefix) = match point.kind {
                Kind::Piping => (&mut piping, "PPC"),
                Kind::Signal => (&mut signal, "SPC"),
                Kind::Duct | Kind::Other => continue,
            };
            let uid = format!("{model}.{number}");
            bucket
                .entry(uid.clone())
                .or_insert_with(|| (number.to_string(), String::new()));
            let rel = format!("{prefix}-{uid}-{model}");
            rels.entry(rel)
                .or_insert_with(|| (uid, model.clone(), prefix.to_string()));
        }
    }
    // Process points are filled by the caller from items; this function only
    // sees linked connect points. The wrapper below adds them.
    let _ = &rels;
    Got {
        piping,
        signal,
        process: BTreeMap::new(),
        rels,
    }
}

fn apply_flow(linked: &Linked, got: &Got, number_field: &str, flow_field: Option<&str>) -> Got {
    let Some(flow_field) = flow_field else {
        return clone_got(got);
    };
    let mut piping = got.piping.clone();
    for (model, indexes) in &linked.by_item {
        for index in indexes {
            let Some(point) = linked.points.get(*index) else {
                continue;
            };
            if point.kind != Kind::Piping {
                continue;
            }
            let Some(number) = point
                .fields
                .get(number_field)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let uid = format!("{model}.{number}");
            let Some(ordinal) = point
                .fields
                .get(flow_field)
                .and_then(|v| v.parse::<i32>().ok())
            else {
                continue;
            };
            let Some(published) = flow_published(ordinal) else {
                continue;
            };
            if let Some((_, flow)) = piping.get_mut(&uid) {
                if flow.is_empty() {
                    *flow = published.to_string();
                } else if flow != published {
                    *flow = String::from("CONFLICT");
                }
            }
        }
    }
    Got {
        piping,
        signal: got.signal.clone(),
        process: got.process.clone(),
        rels: got.rels.clone(),
    }
}

fn clone_got(got: &Got) -> Got {
    Got {
        piping: got.piping.clone(),
        signal: got.signal.clone(),
        process: got.process.clone(),
        rels: got.rels.clone(),
    }
}

fn flow_field_of(linked: &Linked, number_field: &str, refs: &RefSide) -> Option<String> {
    let got = materialise(linked, number_field);
    let mut names = BTreeSet::new();
    for point in &linked.points {
        if point.kind == Kind::Piping {
            for name in point.fields.keys() {
                names.insert(name.clone());
            }
        }
    }
    let mut best: Option<(String, usize)> = None;
    for name in names {
        let trial = apply_flow(linked, &got, number_field, Some(&name));
        let same = flow_diff(&filter_class(&refs.ports, "PIDPipingPort"), &trial.piping).1;
        if same > 0 && best.as_ref().is_none_or(|(_, n)| same > *n) {
            best = Some((name, same));
        }
    }
    best.map(|(name, _)| name)
}

fn with_process(got: &Got, items: &[Item]) -> Got {
    let mut process = BTreeMap::new();
    let mut rels = got.rels.clone();
    for item in items {
        if item.model_item_type != "PipeRun" {
            continue;
        }
        let uid = format!("{}.PPT", item.model_id);
        process.insert(uid.clone(), (String::new(), String::new()));
        let rel = format!("PRP-{uid}-{}", item.model_id);
        rels.insert(rel, (uid, item.model_id.clone(), "PRP".into()));
    }
    Got {
        piping: got.piping.clone(),
        signal: got.signal.clone(),
        process,
        rels,
    }
}

fn print_census(loaded: &Loaded) {
    let mut field_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut field_values: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut described_types: BTreeMap<u16, usize> = BTreeMap::new();
    let mut by_site: BTreeMap<u32, usize> = BTreeMap::new();
    for point in &loaded.points {
        *by_site.entry(point.site).or_insert(0) += 1;
        *described_types.entry(point.described_type).or_insert(0) += 1;
        for (name, value) in &point.fields {
            *field_counts.entry(name.clone()).or_insert(0) += 1;
            let values = field_values.entry(name.clone()).or_default();
            if values.len() < 6 {
                values.insert(value.clone());
            }
        }
    }
    println!(
        "connect_points={} placements={} with_model={} items={} piperun={}",
        loaded.points.len(),
        loaded.placements.len(),
        loaded
            .placements
            .iter()
            .filter(|p| p.model_id.is_some())
            .count(),
        loaded.items.len(),
        loaded
            .items
            .iter()
            .filter(|i| i.model_item_type == "PipeRun")
            .count()
    );
    println!("connect_points by JSite: {by_site:?}");
    println!("described type_code: {described_types:?}");
    println!("_ConnectPoint fields:");
    for (name, count) in &field_counts {
        let values: Vec<&str> = field_values
            .get(name)
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        println!("  {name} n={count} e.g. {}", values.join(" | "));
    }
    println!("fields whose name contains Flow or PointNumber (class, field -> value counts):");
    let mut shown = 0usize;
    for ((class, field), values) in &loaded.flow_homes {
        if !field_is_interesting(field) && !class_is_interesting(class) {
            continue;
        }
        let mut pairs: Vec<(&str, usize)> = values.iter().map(|(v, n)| (v.as_str(), *n)).collect();
        pairs.sort_by_key(|pair| std::cmp::Reverse(pair.1));
        pairs.truncate(4);
        let rendered: Vec<String> = pairs.iter().map(|(v, n)| format!("{v}×{n}")).collect();
        println!("  {class}.{field}: {}", rendered.join(", "));
        shown += 1;
        if shown == 40 {
            println!("  …");
            break;
        }
    }
    if shown == 0 {
        println!("  (none)");
    }
    let sheets: BTreeSet<(u32, u32)> = loaded
        .placements
        .iter()
        .filter(|p| p.model_id.is_some())
        .map(|p| (p.definition_site_ref, p.definition_sheet_ref))
        .collect();
    println!(
        "placed items' distinct (definition_site, definition_sheet)={}",
        sheets.len()
    );
    println!(
        "payload.index {:?} sub_type {:?} parent_type {:?} len {:?}",
        hist(loaded.points.iter().map(|p| p.index.to_string())),
        hist(loaded.points.iter().map(|p| p.sub_type.to_string())),
        hist(
            loaded
                .points
                .iter()
                .map(|p| format!("{:#06x}", p.parent_type))
        ),
        hist(loaded.points.iter().map(|p| p.payload_len.to_string()))
    );
    let mut per_parent: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for point in &loaded.points {
        *per_parent.entry((point.site, point.parent)).or_insert(0) += 1;
    }
    println!(
        "points per parent {:?}",
        hist(per_parent.values().map(|n| n.to_string()))
    );
}

fn hist(values: impl Iterator<Item = String>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value).or_insert(0) += 1;
    }
    counts
}

fn field_is_interesting(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("flow") || n.contains("pointnumber") || n.contains("point_number")
}

fn class_is_interesting(class: &str) -> bool {
    class.contains("ConnectPoint") || class.contains("PipingPoint") || class.contains("SignalPoint")
}

fn filter_class(
    ports: &BTreeMap<String, (String, String, String)>,
    class: &str,
) -> BTreeMap<String, (String, String)> {
    ports
        .iter()
        .filter(|(_, (c, _, _))| c == class)
        .map(|(uid, (_, name, flow))| (uid.clone(), (name.clone(), flow.clone())))
        .collect()
}

fn diff_objects(
    reference: &BTreeMap<String, (String, String)>,
    got: &BTreeMap<String, (String, String)>,
) -> Diff {
    let mut same = 0usize;
    let mut wrong = 0usize;
    let mut wrong_ids = Vec::new();
    let mut missing_ids = Vec::new();
    for (uid, (name, flow)) in reference {
        match got.get(uid) {
            Some((got_name, got_flow)) if got_name == name && flow_eq(got_flow, flow) => same += 1,
            Some(_) => {
                wrong += 1;
                if wrong_ids.len() < 8 {
                    wrong_ids.push(uid.clone());
                }
            }
            None => {
                if missing_ids.len() < 8 {
                    missing_ids.push(uid.clone());
                }
            }
        }
    }
    let missing = reference
        .keys()
        .filter(|uid| !got.contains_key(*uid))
        .count();
    let mut extra_ids = Vec::new();
    let extra = got
        .keys()
        .filter(|uid| !reference.contains_key(*uid))
        .count();
    for uid in got.keys() {
        if !reference.contains_key(uid) && extra_ids.len() < 8 {
            extra_ids.push(uid.clone());
        }
    }
    Diff {
        same,
        wrong,
        extra,
        missing,
        missing_ids,
        extra_ids,
        wrong_ids,
    }
}

fn flow_eq(got: &str, reference: &str) -> bool {
    if got == "CONFLICT" {
        return false;
    }
    normalize_flow(got) == normalize_flow(reference)
}

fn normalize_flow(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('@')
        .trim_matches(|c| c == '{' || c == '}')
        .to_ascii_uppercase()
}

fn diff_rels(
    reference: &BTreeMap<String, (String, String, String)>,
    got: &BTreeMap<String, (String, String, String)>,
) -> Diff {
    let mut same = 0usize;
    let mut wrong = 0usize;
    let mut wrong_ids = Vec::new();
    let mut missing_ids = Vec::new();
    for (uid, (uid1, uid2, def)) in reference {
        let prefix = match def.as_str() {
            "PipingPortComposition" => "PPC",
            "SignalPortComposition" => "SPC",
            "ProcessPointCollection" => "PRP",
            _ => continue,
        };
        let built = format!("{prefix}-{uid1}-{uid2}");
        match got.get(uid) {
            Some((g1, g2, gpre)) if g1 == uid1 && g2 == uid2 && gpre == prefix && built == *uid => {
                same += 1;
            }
            Some(_) => {
                wrong += 1;
                if wrong_ids.len() < 8 {
                    wrong_ids.push(uid.clone());
                }
            }
            None => {
                if missing_ids.len() < 8 {
                    missing_ids.push(format!("{uid} built={built}"));
                }
            }
        }
    }
    let wanted: BTreeSet<&str> = reference
        .iter()
        .filter(|(_, (_, _, def))| {
            matches!(
                def.as_str(),
                "PipingPortComposition" | "SignalPortComposition" | "ProcessPointCollection"
            )
        })
        .map(|(uid, _)| uid.as_str())
        .collect();
    let missing = wanted.iter().filter(|uid| !got.contains_key(**uid)).count();
    let mut extra_ids = Vec::new();
    let extra = got
        .keys()
        .filter(|uid| !wanted.contains(uid.as_str()))
        .count();
    for uid in got.keys() {
        if !wanted.contains(uid.as_str()) && extra_ids.len() < 8 {
            extra_ids.push(uid.clone());
        }
    }
    Diff {
        same,
        wrong,
        extra,
        missing,
        missing_ids,
        extra_ids,
        wrong_ids,
    }
}

fn flow_diff(
    reference: &BTreeMap<String, (String, String)>,
    got: &BTreeMap<String, (String, String)>,
) -> (usize, usize, usize, usize, usize) {
    let mut ref_with = 0usize;
    let mut same = 0usize;
    let mut wrong = 0usize;
    let mut missing_value = 0usize;
    for (uid, (_, flow)) in reference {
        if flow.is_empty() {
            continue;
        }
        ref_with += 1;
        match got.get(uid).map(|(_, g)| g.as_str()) {
            Some(g) if flow_eq(g, flow) => same += 1,
            Some("") => missing_value += 1,
            Some(_) => wrong += 1,
            None => missing_value += 1,
        }
    }
    let mut extra_value = 0usize;
    for (uid, (_, flow)) in got {
        if flow.is_empty() || flow == "CONFLICT" {
            continue;
        }
        let ref_flow = reference.get(uid).map(|(_, f)| f.as_str()).unwrap_or("");
        if ref_flow.is_empty() {
            extra_value += 1;
        }
    }
    (ref_with, same, wrong, missing_value, extra_value)
}

fn verdict(class: &str, diff: &Diff) -> String {
    let mark = if diff.wrong + diff.extra + diff.missing == 0 {
        "EMIT"
    } else {
        "HOLD"
    };
    format!(
        "{class} {mark} same={} wrong={} extra={} missing={}",
        diff.same, diff.wrong, diff.extra, diff.missing
    )
}

fn print_samples(label: &str, diff: &Diff) {
    if diff.wrong + diff.extra + diff.missing == 0 {
        return;
    }
    if !diff.wrong_ids.is_empty() {
        println!("  {label} wrong e.g. {}", diff.wrong_ids.join(", "));
    }
    if !diff.missing_ids.is_empty() {
        println!("  {label} missing e.g. {}", diff.missing_ids.join(", "));
    }
    if !diff.extra_ids.is_empty() {
        println!("  {label} extra e.g. {}", diff.extra_ids.join(", "));
    }
}

fn flow_published(ordinal: i32) -> Option<&'static str> {
    match ordinal {
        1 | 5 => Some(FLOW_1),
        2 => Some(FLOW_2),
        3 => Some(FLOW_3),
        4 => Some(FLOW_4),
        _ => None,
    }
}

fn kind_of(fields: &BTreeMap<String, String>) -> Kind {
    let mut values = Vec::new();
    if let Some(kind) = fields.get("Type") {
        values.push(kind.clone());
    }
    values.extend(fields.values().cloned());
    for value in values {
        match value.as_str() {
            "PipingPoint" => return Kind::Piping,
            "SignalPoint" => return Kind::Signal,
            "DuctingPoint" => return Kind::Duct,
            _ => {}
        }
    }
    Kind::Other
}

fn row_fields(row: &pid_parse::parsers::attribute_rows::AttributeRow) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for field in &row.fields {
        fields
            .entry(field.name.clone())
            .or_insert_with(|| match &field.value {
                AttributeValue::Text(text) => text.trim().to_string(),
                AttributeValue::Integer(n) => n.to_string(),
                AttributeValue::Boolean(b) => {
                    if *b {
                        "true".into()
                    } else {
                        "false".into()
                    }
                }
            });
    }
    fields
}

fn note_flow(
    class: &str,
    site: u32,
    fields: &BTreeMap<String, String>,
    homes: &mut BTreeMap<(String, String), BTreeMap<String, usize>>,
) {
    let _ = site;
    for (name, value) in fields {
        if field_is_interesting(name) || class_is_interesting(class) {
            *homes
                .entry((class.to_string(), name.clone()))
                .or_default()
                .entry(value.clone())
                .or_insert(0) += 1;
        }
    }
}

fn item_model(row: &pid_parse::parsers::attribute_rows::AttributeRow) -> Option<&str> {
    if row.class != "P&IDAttributes" {
        return None;
    }
    if !matches!(row.text("DrawingItemType"), Some("Symbol" | "Connector")) {
        return None;
    }
    row.text("ModelID").filter(|model| model.len() > 1)
}

fn ancestors(parents: &HashMap<(u32, u32), u32>, site: u32, start: u32) -> Vec<u32> {
    let mut out = vec![start];
    let mut cur = start;
    for _ in 0..24 {
        let Some(parent) = parents.get(&(site, cur)).copied() else {
            break;
        };
        if parent == 0 || parent == cur || out.contains(&parent) {
            break;
        }
        out.push(parent);
        cur = parent;
    }
    out
}

fn site_id(path: &str) -> Option<u32> {
    path.split(['/', '\\']).find_map(|seg| {
        seg.strip_prefix("JSite")
            .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
            .and_then(|id| id.parse().ok())
    })
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    let bytes = data.get(at..at + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn reference_side(xml: &str) -> RefSide {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut depth = 0usize;
    let mut class = String::new();
    let mut uid = String::new();
    let mut name = String::new();
    let mut flow = String::new();
    let mut uid1 = String::new();
    let mut uid2 = String::new();
    let mut def_uid = String::new();
    let mut ports = BTreeMap::new();
    let mut process = BTreeMap::new();
    let mut rels = BTreeMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if depth == 1 {
                    class = start.name().as_ref().to_string();
                    uid.clear();
                    name.clear();
                    flow.clear();
                    uid1.clear();
                    uid2.clear();
                    def_uid.clear();
                }
                if depth == 2 {
                    capture(
                        start.name().as_ref(),
                        &start,
                        &mut uid,
                        &mut name,
                        &mut flow,
                        &mut uid1,
                        &mut uid2,
                        &mut def_uid,
                    );
                }
                depth += 1;
            }
            Ok(Event::Empty(start)) => {
                if depth == 2 {
                    capture(
                        start.name().as_ref(),
                        &start,
                        &mut uid,
                        &mut name,
                        &mut flow,
                        &mut uid1,
                        &mut uid2,
                        &mut def_uid,
                    );
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 1 && !uid.is_empty() {
                    match class.as_str() {
                        "PIDPipingPort" | "PIDSignalPort" => {
                            ports.insert(uid.clone(), (class.clone(), name.clone(), flow.clone()));
                        }
                        "PIDProcessPoint" => {
                            process.insert(uid.clone(), (name.clone(), flow.clone()));
                        }
                        "Rel" => {
                            if matches!(
                                def_uid.as_str(),
                                "PipingPortComposition"
                                    | "SignalPortComposition"
                                    | "ProcessPointCollection"
                            ) {
                                rels.insert(
                                    uid.clone(),
                                    (uid1.clone(), uid2.clone(), def_uid.clone()),
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    RefSide {
        ports,
        process,
        rels,
    }
}

#[allow(clippy::too_many_arguments)]
fn capture(
    tag: &str,
    start: &quick_xml::events::BytesStart<'_>,
    uid: &mut String,
    name: &mut String,
    flow: &mut String,
    uid1: &mut String,
    uid2: &mut String,
    def_uid: &mut String,
) {
    let attr = |key: &str| {
        start
            .attributes()
            .flatten()
            .find_map(|a| (a.key.as_ref() == key).then(|| a.value.to_string()))
    };
    if tag == "IObject" {
        if let Some(value) = attr("UID") {
            *uid = value;
        }
        if let Some(value) = attr("Name") {
            *name = value;
        }
    } else if tag == "IConnection" {
        if let Some(value) = attr("ConnectionFlowDirection") {
            *flow = value;
        }
    } else if tag == "IRel" {
        if let Some(value) = attr("UID1") {
            *uid1 = value;
        }
        if let Some(value) = attr("UID2") {
            *uid2 = value;
        }
        if let Some(value) = attr("DefUID") {
            *def_uid = value;
        }
    }
}
