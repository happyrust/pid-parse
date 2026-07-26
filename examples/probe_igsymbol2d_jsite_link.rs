//! Probe the `igSymbol2d` -> `JSite<id>` instance linkage (Phase 35-C).
//!
//! Hypothesis: each placed `igSymbol2d` record embeds the numeric id of
//! the top-level `JSite<id>` storage that carries its symbol reference
//! (`JProperties` -> `.sym` path). `/JSitesList` entries already
//! correlate with those ids (see `src/parsers/jsites_list.rs`), so if a
//! stable payload offset holds a `JSite` id for every record, the
//! connector is proven and `SymbolInstance.symbol_path` can be emitted.
//!
//! Method, per registered fixture:
//! 1. `PidParser::parse_file` -> `JSite` name/id/symbol-path table;
//! 2. raw-scan every top-level `/Sheet*` stream for validated
//!    `igSymbol2d` records (same walk as the decoder: type `0x00CE`,
//!    `btf` in `[113, 200]`, matrix tag `02 00 A7 50` present);
//! 3. scan each payload at every byte offset for `u32` LE values that
//!    equal a `JSite` id of the same package;
//! 4. print per-record matches and a cross-record offset histogram.
//!
//! Read-only: no parser, schema, or model change.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use cfb::CompoundFile;
use pid_parse::api::PidParser;

const TYPE_IGSYMBOL2D: u16 = 0x00CE;
const PSM_HEADER_LEN: usize = 6;
const MATRIX_TAG: [u8; 4] = [0x02, 0x00, 0xA7, 0x50];

const FIXTURES: &[&str] = &[
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    "工艺管道及仪表流程-1.pid",
    "D06.pid",
    "export-test/publish-data/A01/A01.pid",
    "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
];

fn test_file_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test-file")
}

/// Validated `igSymbol2d` walk over one stream's bytes, mirroring the
/// production decoder's acceptance rules.
fn collect_symbol_payloads(bytes: &[u8]) -> Vec<(usize, Vec<u8>)> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off + PSM_HEADER_LEN + 16 <= bytes.len() {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        if type_word & 0x3FFF != TYPE_IGSYMBOL2D {
            off += 1;
            continue;
        }
        let btf = u32::from_le_bytes([
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
        ]) as usize;
        if !(113..=200).contains(&btf) || off + PSM_HEADER_LEN + btf > bytes.len() {
            off += 1;
            continue;
        }
        let start = off + PSM_HEADER_LEN;
        let payload = &bytes[start..start + btf];
        if !payload.windows(MATRIX_TAG.len()).any(|w| w == MATRIX_TAG) {
            off += 1;
            continue;
        }
        out.push((off, payload.to_vec()));
        off = start + btf;
    }
    out
}

fn jsite_id(name: &str) -> Option<u32> {
    name.strip_prefix("JSite")?.parse().ok()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = test_file_root();
    // offset -> (records matching at that offset, total records seen with payload long enough)
    let mut global_hist: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    let mut global_records = 0usize;

    for rel in FIXTURES {
        let path = root.join(rel);
        if !path.exists() {
            println!("--- {rel}: MISSING, skipped");
            continue;
        }
        let doc = PidParser::new().parse_file(&path)?;
        let mut ids: BTreeMap<u32, (String, Option<String>)> = BTreeMap::new();
        for js in &doc.jsites {
            if let Some(id) = jsite_id(&js.name) {
                ids.insert(id, (js.name.clone(), js.symbol_path.clone()));
            }
        }
        println!("\n=== {rel} ===");
        println!("  JSites ({}):", ids.len());
        for (id, (name, sym)) in &ids {
            println!(
                "    {name} (id={id}) -> {}",
                sym.as_deref().unwrap_or("<no .sym path>")
            );
        }
        let id_set: BTreeSet<u32> = ids.keys().copied().collect();

        let mut cfb = CompoundFile::open(std::fs::File::open(&path)?)?;
        let sheet_paths: Vec<PathBuf> = cfb
            .walk()
            .filter(|e| e.is_stream())
            .map(|e| e.path().to_path_buf())
            .filter(|p| {
                let s = p.to_string_lossy().replace('\\', "/");
                // top-level /Sheet* only: exactly one separator segment
                s.starts_with("/Sheet") && s.matches('/').count() == 1
            })
            .collect();

        let mut fixture_records = 0usize;
        let mut matched_ids: BTreeSet<u32> = BTreeSet::new();
        for sheet in &sheet_paths {
            let mut bytes = Vec::new();
            cfb.open_stream(sheet)?.read_to_end(&mut bytes)?;
            let records = collect_symbol_payloads(&bytes);
            if records.is_empty() {
                continue;
            }
            let sheet_name = sheet.to_string_lossy().replace('\\', "/");
            println!("  {sheet_name}: {} igSymbol2d record(s)", records.len());
            for (rec_idx, (off, payload)) in records.iter().enumerate() {
                fixture_records += 1;
                global_records += 1;
                let oid = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
                let parent = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
                let mut hits: Vec<(usize, u32)> = Vec::new();
                for pos in 0..payload.len().saturating_sub(3) {
                    let v = u32::from_le_bytes([
                        payload[pos],
                        payload[pos + 1],
                        payload[pos + 2],
                        payload[pos + 3],
                    ]);
                    if id_set.contains(&v) {
                        hits.push((pos, v));
                        matched_ids.insert(v);
                    }
                }
                for pos in 0..payload.len().saturating_sub(3) {
                    let entry = global_hist.entry(pos).or_insert((0, 0));
                    entry.1 += 1;
                    if hits.iter().any(|(p, _)| *p == pos) {
                        entry.0 += 1;
                    }
                }
                let hit_str = if hits.is_empty() {
                    "-".to_string()
                } else {
                    hits.iter()
                        .map(|(p, v)| format!("+{p}={v}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                println!(
                    "    [{rec_idx}] @0x{off:06x} oid={oid} parent={parent} id-hits: {hit_str}"
                );
                if rec_idx < 2 {
                    let dump = payload.len().min(96);
                    for chunk in (0..dump).step_by(16) {
                        let end = (chunk + 16).min(dump);
                        let hex = payload[chunk..end]
                            .iter()
                            .map(|b| format!("{b:02X}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!("        +{chunk:03}: {hex}");
                    }
                }
            }
        }
        println!(
            "  fixture summary: {fixture_records} record(s); matched JSite ids: {:?} / known {:?}",
            matched_ids, id_set
        );
    }

    println!("\n=== cross-fixture offset histogram (offsets matching in >=50% of records) ===");
    println!("  total records: {global_records}");
    for (pos, (hit, seen)) in &global_hist {
        if *seen > 0 && hit * 2 >= *seen && *hit > 1 {
            println!("  +{pos}: {hit}/{seen}");
        }
    }
    Ok(())
}
