//! Does the symbol-information / expression family read the way the four
//! decoders claim, and does it account for every byte it touches?
//!
//! The four families — `0x00BD` `JSymbolInformation` (`symbol.dex`),
//! `0x00C7` `Double Value Object` and `0x00EA` `Variables Object`
//! (`exprdex.dll`), `0x006F` `Assoc subsystem Standard Relation`
//! (`jengine.dll`) — form one parametric chain: a symbol carries named
//! variables (`Left` / `Right` / `Bottom` / `Top`), each variable's value is
//! also persisted as its own `Double Value`, a `Variables` record groups
//! them, and a relation feeds one into a `0x0115` `JDim` through a
//! `JBExpression` formula. They live only in a `JSite<N>/PSMcluster0`.
//!
//! This probe walks the record chains by their declared lengths — the frame
//! the decoders never see, since `PsmRecordDecoder::scan` slides byte by byte
//! — and holds the two views against each other:
//!
//! 1. **Census.** Per site: how many records of each family the chain walk
//!    finds, and how many each decoder accepts. A decoder that accepts fewer
//!    is rejecting real records; one that accepts more is matching bytes
//!    that are not records at all.
//! 2. **Coverage.** Every byte of every accepted record, against the chain's
//!    own record boundaries. The families are counted, variable-length
//!    shapes, so an off-by-one in a name or operand list would show up as a
//!    range that does not line up with a chain record.
//! 3. **The chain between the families.** Does each `Double Value` name a
//!    `Variables` group that lists it back; does each named variable's
//!    inline `f64` equal the `Double Value` record it points at; are the
//!    relation operands a `JDim` output and `Double Value` inputs.
//! 4. **Exhibits.** Every symbol with its variables, every relation with its
//!    signature and formula, printed per site.
//!
//! ```powershell
//! cargo run --example probe_symbol_information_family_shape
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{
    decode_double_values, decode_standard_relations, decode_symbol_informations, decode_variables,
    sheet_record_starts, PSM_TYPE_CODE_DOUBLE_VALUE, PSM_TYPE_CODE_JSYMBOL_INFORMATION,
    PSM_TYPE_CODE_STANDARD_RELATION, PSM_TYPE_CODE_VARIABLES,
};

const FIXTURES: [&str; 4] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
];

/// The header magic every record-chain stream in this corpus opens with.
const CHAIN_MAGIC: u32 = 0x6C90_F544;
/// `JDim Object`, the class every relation's output operand turns out to be.
const PSM_TYPE_CODE_JDIM: u16 = 0x0115;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// One site's `PSMcluster0`: the raw bytes plus the chain walk's own view of
/// where records start and what type each one is.
struct Cluster {
    site: String,
    data: Vec<u8>,
    /// `record start -> (type code, record end)` straight off the chain.
    framed: BTreeMap<usize, (u16, usize)>,
}

fn walk_chain(data: &[u8]) -> BTreeMap<usize, (u16, usize)> {
    let mut out = BTreeMap::new();
    for start in sheet_record_starts(data) {
        let (Some(type_word), Some(len)) = (u16_at(data, start), u32_at(data, start + 2)) else {
            continue;
        };
        let Some(end) = start
            .checked_add(6)
            .and_then(|at| at.checked_add(len as usize))
        else {
            continue;
        };
        if end <= data.len() {
            out.insert(start, (type_word & 0x3FFF, end));
        }
    }
    out
}

fn load(path: &Path) -> Vec<Cluster> {
    let Ok(file) = std::fs::File::open(path) else {
        return vec![];
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return vec![];
    };
    let stream_paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .collect();
    let mut out = Vec::new();
    for stream_path in stream_paths {
        let normalized = stream_path.replace('\\', "/");
        if !normalized.ends_with("/PSMcluster0") || normalized.matches('/').count() != 2 {
            continue;
        }
        let mut data = Vec::new();
        let Ok(mut stream) = cfb.open_stream(&stream_path) else {
            continue;
        };
        if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CHAIN_MAGIC) {
            continue;
        }
        let framed = walk_chain(&data);
        out.push(Cluster {
            site: normalized
                .trim_start_matches('/')
                .split('/')
                .next()
                .unwrap_or_default()
                .to_string(),
            data,
            framed,
        });
    }
    out
}

fn main() {
    let mut totals: BTreeMap<&str, usize> = BTreeMap::new();
    let mut chain_totals: BTreeMap<&str, usize> = BTreeMap::new();
    let mut off_frame = Vec::new();
    let mut value_in_group = (0usize, 0usize);
    let mut variable_agrees = (0usize, 0usize);
    let mut operand_families: BTreeMap<String, usize> = BTreeMap::new();

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        let clusters = load(path);
        if clusters.is_empty() {
            println!("== {fixture}: no JSite PSMcluster0 (fixture absent?)");
            continue;
        }
        println!("\n================ {fixture} ================");

        for cluster in &clusters {
            let values = decode_double_values(&cluster.data);
            let groups = decode_variables(&cluster.data);
            let symbols = decode_symbol_informations(&cluster.data);
            let relations = decode_standard_relations(&cluster.data);

            // 1. Census: the chain's count of each family against the
            //    decoders' count.
            let mut by_chain: BTreeMap<u16, usize> = BTreeMap::new();
            for (type_code, _) in cluster.framed.values() {
                *by_chain.entry(*type_code).or_default() += 1;
            }
            let census = [
                (
                    "0x00BD JSymbolInformation",
                    PSM_TYPE_CODE_JSYMBOL_INFORMATION,
                    symbols.len(),
                ),
                (
                    "0x00C7 Double Value",
                    PSM_TYPE_CODE_DOUBLE_VALUE,
                    values.len(),
                ),
                ("0x00EA Variables", PSM_TYPE_CODE_VARIABLES, groups.len()),
                (
                    "0x006F Standard Relation",
                    PSM_TYPE_CODE_STANDARD_RELATION,
                    relations.len(),
                ),
            ];
            println!("\n-- [{}] --", cluster.site);
            for (label, type_code, decoded) in census {
                let chained = by_chain.get(&type_code).copied().unwrap_or_default();
                let verdict = if chained == decoded { "ok" } else { "MISMATCH" };
                println!("   {label:<28} chain {chained:>4}   decoded {decoded:>4}   {verdict}");
                *totals.entry(label).or_default() += decoded;
                *chain_totals.entry(label).or_default() += chained;
            }

            // 2. Coverage: every accepted record must be a chain record.
            let mut ranges: Vec<(usize, usize, &str)> = Vec::new();
            ranges.extend(
                values
                    .iter()
                    .map(|r| (r.byte_range.start, r.byte_range.end, "0x00C7")),
            );
            ranges.extend(
                groups
                    .iter()
                    .map(|r| (r.byte_range.start, r.byte_range.end, "0x00EA")),
            );
            ranges.extend(
                symbols
                    .iter()
                    .map(|r| (r.byte_range.start, r.byte_range.end, "0x00BD")),
            );
            ranges.extend(
                relations
                    .iter()
                    .map(|r| (r.byte_range.start, r.byte_range.end, "0x006F")),
            );
            for (start, end, label) in ranges {
                match cluster.framed.get(&start) {
                    Some((_, framed_end)) if *framed_end == end => {}
                    other => off_frame.push(format!(
                        "{} [{}] {label} at {start}..{end} vs chain {other:?}",
                        fixture, cluster.site
                    )),
                }
            }

            // 3. The chain between the families.
            let value_by_oid: BTreeMap<u32, f64> = values
                .iter()
                .map(|value| (value.oid, value.value))
                .collect();
            let members: BTreeMap<u32, BTreeSet<u32>> = groups
                .iter()
                .map(|group| (group.oid, group.members.iter().copied().collect()))
                .collect();
            for value in &values {
                value_in_group.1 += 1;
                if members
                    .get(&value.parent_ref)
                    .is_some_and(|listed| listed.contains(&value.oid))
                {
                    value_in_group.0 += 1;
                }
            }
            let type_of: BTreeMap<u32, u16> = cluster
                .framed
                .iter()
                .filter_map(|(start, (type_code, _))| {
                    u32_at(&cluster.data, start + 6).map(|oid| (oid, *type_code))
                })
                .collect();

            // 4. Exhibits.
            for symbol in &symbols {
                if symbol.variables.is_empty() {
                    continue;
                }
                let rendered: Vec<String> = symbol
                    .variables
                    .iter()
                    .map(|variable| {
                        variable_agrees.1 += 1;
                        // A site can hold a symbol whose value objects live
                        // nowhere in it — that is absence, not disagreement.
                        let note = match value_by_oid.get(&variable.value_ref) {
                            Some(value) if *value == variable.value => {
                                variable_agrees.0 += 1;
                                ""
                            }
                            Some(_) => " (VALUE DISAGREES)",
                            None => " (no value object in this site)",
                        };
                        format!(
                            "{}={:.6} -> {}{note}",
                            variable.name, variable.value, variable.value_ref
                        )
                    })
                    .collect();
                println!(
                    "   symbol {:<6} extents ({:.5}, {:.5})  {}",
                    symbol.oid,
                    symbol.extents.0,
                    symbol.extents.1,
                    rendered.join("  ")
                );
            }
            for relation in &relations {
                let operands: Vec<String> = relation
                    .operands
                    .iter()
                    .enumerate()
                    .map(|(index, operand)| {
                        let family = type_of.get(operand).copied().unwrap_or_default();
                        let role = if index == 0 { "out" } else { "in" };
                        *operand_families
                            .entry(format!("{role} 0x{family:04X}"))
                            .or_default() += 1;
                        format!("{role} {operand}(0x{family:04X})")
                    })
                    .collect();
                println!(
                    "   relation {:<6} {:<10} {:<24} {}",
                    relation.oid,
                    relation.signature,
                    operands.join(" "),
                    relation.formula
                );
            }
        }
    }

    println!("\n================ verdict ================");
    for (label, decoded) in &totals {
        let chained = chain_totals.get(label).copied().unwrap_or_default();
        println!(
            "  {label:<28} chain {chained:>4}   decoded {decoded:>4}   {}",
            if chained == *decoded {
                "ok"
            } else {
                "MISMATCH"
            }
        );
    }
    println!(
        "  every accepted record lands on a chain record: {}",
        if off_frame.is_empty() {
            "yes".to_string()
        } else {
            format!("NO — {} exceptions: {off_frame:?}", off_frame.len())
        }
    );
    println!(
        "  Double Value listed by the group it names: {} / {}",
        value_in_group.0, value_in_group.1
    );
    println!(
        "  named variable agrees with its value record: {} / {} (the rest name a value \
         object that is not in this site)",
        variable_agrees.0, variable_agrees.1
    );
    println!("  relation operand families: {operand_families:?}");
    println!(
        "  (the output should be 0x{PSM_TYPE_CODE_JDIM:04X} JDim every time; inputs are \
         0x00C7 values)"
    );
}
