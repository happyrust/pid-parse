//! Where does a placed parametric instance keep its actual parameters?
//!
//! `docs/analysis/2026-09-18-the-parametric-chain-closes-on-the-template-not-the-instance.md`
//! left it open: the instance's `0x00BD JSymbolInformation` copy repeats the
//! library defaults, and the stretched Manifold's `Left' = 57.909` /
//! `Top' = 35.59` were in no `Double Value` or `JDim`. A brute-force scan of
//! every stream for those numbers as `f64` found them in one record of the
//! instance's own `PSMcluster0`: type `0x00ED`, which
//! `tools/psm_type_clsid.py` resolves to `symbol.dex` **`JFlavorHolder`**.
//!
//! This probe walks every nested `PSMcluster0`, pairs each `JFlavorHolder`
//! with the `JSymbolInformation` of the same storage, and prints the values
//! the holder carries next to the variables the symbol information names --
//! the pairing the analysis document tabulates. Two shapes appear:
//!
//! * the **instance** holder (`Imagineer Document`): a `u16` count at +24,
//!   then one `01 01 00 00 00` + `f64` per variable, in the symbol
//!   information's variable order -- the instance's actual parameters;
//! * the **template** holder (`Server Document`): no values, a `u32` at +40
//!   naming the `JSymbolInformation` whose `parent_ref` is this holder.
//!
//! ```powershell
//! cargo run --example probe_jflavorholder_carries_instance_parameters
//! ```

use std::io::Read;

use pid_parse::parsers::sheet_records::{decode_symbol_informations, sheet_record_starts};

const FIXTURES: [&str; 4] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
];

/// `symbol.dex` `JFlavorHolder`.
const PSM_TYPE_CODE_JFLAVOR_HOLDER: u16 = 0x00ED;

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// The `01 01 00 00 00` + `f64` entries after the record's `"Sheets"` name.
fn holder_values(record: &[u8]) -> Vec<f64> {
    let name_end = record
        .windows(12)
        .position(|w| {
            w == "Sheets"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<u8>>()
        })
        .map_or(record.len(), |at| at + 12);
    let mut values = Vec::new();
    let mut at = name_end;
    while at + 13 <= record.len() {
        if record[at..at + 5] == [1, 1, 0, 0, 0] {
            let v = f64::from_le_bytes(record[at + 5..at + 13].try_into().expect("8 bytes"));
            if v.is_finite() {
                values.push(v);
                at += 13;
                continue;
            }
        }
        at += 1;
    }
    values
}

fn main() {
    for path in FIXTURES {
        let file = std::fs::File::open(path).expect("open fixture");
        let mut cfb = cfb::CompoundFile::open(file).expect("compound file");
        let clusters: Vec<String> = cfb
            .walk()
            .filter(|e| e.is_stream())
            .map(|e| e.path().to_string_lossy().replace('\\', "/"))
            .filter(|p| p.ends_with("/PSMcluster0") && p.matches('/').count() == 2)
            .collect();
        println!("== {path}");
        for stream in clusters {
            let mut d = Vec::new();
            cfb.open_stream(&stream)
                .expect("stream")
                .read_to_end(&mut d)
                .expect("read");
            let mut starts: Vec<usize> = sheet_record_starts(&d);
            starts.sort_unstable();
            let infos = decode_symbol_informations(&d);
            let holders: Vec<(usize, usize)> = starts
                .iter()
                .enumerate()
                .filter(|(_, &s)| u16_at(&d, s) == Some(PSM_TYPE_CODE_JFLAVOR_HOLDER))
                .map(|(i, &s)| (s, starts.get(i + 1).copied().unwrap_or(d.len())))
                .collect();
            let with_variables = infos.iter().filter(|si| !si.variables.is_empty()).count();
            if with_variables == 0 {
                println!(
                    "   {stream}: {} JFlavorHolder, {} JSymbolInformation, none with variables",
                    holders.len(),
                    infos.len()
                );
                continue;
            }
            println!(
                "   {stream}: {} JFlavorHolder, {} JSymbolInformation ({with_variables} with variables)",
                holders.len(),
                infos.len()
            );
            for si in infos.iter().filter(|si| !si.variables.is_empty()) {
                let vars: Vec<String> = si
                    .variables
                    .iter()
                    .map(|v| format!("{} = {:.6}", v.name, v.value))
                    .collect();
                println!(
                    "      0x00BD oid {} parent {}: {}",
                    si.oid,
                    si.parent_ref,
                    vars.join(", ")
                );
            }
            for (s, end) in holders {
                let record = &d[s..end];
                let oid = u32_at(record, 6).unwrap_or(0);
                let count = u16_at(record, 24).unwrap_or(0);
                let values = holder_values(record);
                if values.is_empty() {
                    // Template shape: +40 names the symbol information.
                    let names = u32_at(record, 40).unwrap_or(0);
                    if infos
                        .iter()
                        .any(|si| si.oid == names && !si.variables.is_empty())
                    {
                        println!(
                            "      0x00ED oid {oid}: template shape, no values; +40 names JSymbolInformation {names}"
                        );
                    }
                    continue;
                }
                let listed: Vec<String> = values.iter().map(|v| format!("{v:.6}")).collect();
                println!(
                    "      0x00ED oid {oid}: instance shape, count {count}, values [{}]  ({} bytes, +20 = {})",
                    listed.join(", "),
                    record.len(),
                    u32_at(record, 20).unwrap_or(0)
                );
            }
        }
    }
}
