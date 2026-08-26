//! Does the librarian payload match the grammar `style.dll` itself reads?
//!
//! This is the probe that took the scan apart. Everything the crate used to
//! take out of `0x005A JStyleLibrarian` was found by scanning: a walk for runs
//! of name-like UTF-16, with the `u32` eight bytes past a run read as the
//! `oid`, and a source read backwards from a four-zero trailer. Both were
//! exhaustive on this corpus, and the docstring stated the reason: "names are
//! UTF-16 with neither a terminator nor a length in front of them, so an entry
//! is found by where its text stops".
//!
//! `style.dll` disagrees. `JStyleLib::IJPersistImp`'s vftable
//! (`.rdata:0x100A32FC`) has the `IPersistStream` shape; its `GetClassID`
//! (slot 3) returns the librarian CLSID `{9196D9D1-E94A-11CF-8094-080036CE6C02}`
//! at `.rdata:0x1009DEA4`, and `Load` (slot 5) reaches `sub_100586A0` behind
//! a gate that accepts schema version 1 only. That function transfers each
//! named style as:
//!
//! ```text
//! u16                                        // family / palette index
//! u32
//! u32 name_len_chars ; name_len_chars x u16  // UTF-16LE, no NUL
//! u32 path_len_chars ; path_len_chars x u16  // UTF-16LE, no NUL
//! u32 u32 u32 u32
//! ```
//!
//! So a name's extent is **stated, not inferred**, and the fixed part of an
//! entry is 30 bytes. Two checks fall out of that, and this probe runs both
//! against every librarian payload in the corpus:
//!
//! * the `u32` in front of a name equals its length in characters;
//! * consecutive names sit `30 + 2 * name_len` apart.
//!
//! The second is the one that cannot be a coincidence. It also explains the
//! `+8` gap as arithmetic rather than measurement: past a name lie
//! `u32 path_len` and one more `u32`, so the `oid` read at `+8` is the second
//! of the four trailing words -- and only while the path is empty.
//!
//! `probe_librarian_entry_walk` is the follow-up that walks the whole payload
//! this way; the crate reads the librarian as a sequence now, not a scan.

use std::io::Read;
use std::path::Path;

use cfb::CompoundFile;

const FIXTURES: [&str; 5] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/A01.pid",
];

const CLUSTER_MAGIC: u32 = 0x6C90_F544;
const STREAM_HEADER_LEN: usize = 8;
const PSM_ENVELOPE_LEN: usize = 6;
const PSM_TYPE_CODE_JSTYLE_LIBRARIAN: u16 = 0x005A;

/// Bytes of an entry that are not its two strings, per `sub_100586A0`:
/// `u16` index, `u32`, two `u32` lengths, four trailing `u32`.
const ENTRY_FIXED_LEN: usize = 30;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn read_utf16(payload: &[u8], at: usize, units: usize) -> Option<String> {
    let wide: Vec<u16> = (0..units)
        .map(|i| u16_at(payload, at + i * 2))
        .collect::<Option<_>>()?;
    String::from_utf16(&wide).ok()
}

/// Every maximal run of printable ASCII UTF-16 in a payload, `(start, units)`.
///
/// Even offsets only, and three units minimum: a UTF-16 field cannot begin
/// halfway through a code unit, and shorter runs are indistinguishable from
/// the binary either side of them.
fn text_runs(payload: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut run_start = None;
    let mut at = 0usize;
    while at + 2 <= payload.len() {
        let printable = u16_at(payload, at).is_some_and(|u| (0x20..=0x7E).contains(&u));
        if printable {
            run_start.get_or_insert(at);
        } else if let Some(start) = run_start.take() {
            out.push((start, (at - start) / 2));
        }
        at += 2;
    }
    if let Some(start) = run_start {
        out.push((start, (payload.len() - start) / 2));
    }
    out.retain(|(_, units)| *units >= 3);
    out
}

fn librarian_payloads(path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut cfb) = CompoundFile::open(file) else {
        return Vec::new();
    };
    let cluster_paths: Vec<String> = cfb
        .walk()
        .filter(cfb::Entry::is_stream)
        .map(|entry| entry.path().to_string_lossy().into_owned())
        .filter(|p| p.rsplit('/').next().unwrap_or("") == "StyleCluster")
        .collect();

    let mut out = Vec::new();
    for cluster_path in &cluster_paths {
        let Ok(mut stream) = cfb.open_stream(cluster_path) else {
            continue;
        };
        let mut data = Vec::new();
        if stream.read_to_end(&mut data).is_err() || u32_at(&data, 0) != Some(CLUSTER_MAGIC) {
            continue;
        }
        let mut at = STREAM_HEADER_LEN;
        while let (Some(type_word), Some(bytes_to_follow)) =
            (u16_at(&data, at), u32_at(&data, at + 2))
        {
            let payload_start = at + PSM_ENVELOPE_LEN;
            let Some(end) = payload_start.checked_add(bytes_to_follow as usize) else {
                break;
            };
            if bytes_to_follow == 0 || end > data.len() {
                break;
            }
            if type_word & 0x3FFF == PSM_TYPE_CODE_JSTYLE_LIBRARIAN {
                out.push((cluster_path.clone(), data[payload_start..end].to_vec()));
            }
            at = end;
        }
    }
    out
}

fn main() {
    let mut runs_total = 0usize;
    let mut prefixed = 0usize;
    let mut stride_checked = 0usize;
    let mut stride_holds = 0usize;

    for fixture in FIXTURES {
        let path = Path::new(fixture);
        if !path.exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        println!("\n=== {fixture} ===");
        for (cluster, payload) in librarian_payloads(path) {
            let runs = text_runs(&payload);
            let mut hits = 0usize;
            let mut strides = 0usize;
            let mut stride_ok = 0usize;

            for (i, (start, units)) in runs.iter().enumerate() {
                if start.checked_sub(4).and_then(|at| u32_at(&payload, at)) == Some(*units as u32) {
                    hits += 1;
                }
                // Only meaningful between two entries that both carry a name
                // and no path; a path would push the next entry further out,
                // which is exactly what the stride is here to expose.
                if let Some((next_start, _)) = runs.get(i + 1) {
                    strides += 1;
                    if *next_start == start + units * 2 + ENTRY_FIXED_LEN {
                        stride_ok += 1;
                    }
                }
            }

            runs_total += runs.len();
            prefixed += hits;
            stride_checked += strides;
            stride_holds += stride_ok;
            println!(
                "  {cluster}: {} bytes, {} text runs; {hits} length-prefixed, \
                 {stride_ok}/{strides} at the predicted 30 + 2n stride",
                payload.len(),
                runs.len()
            );
            for (i, (start, units)) in runs.iter().enumerate() {
                let stated = start.checked_sub(4).and_then(|at| u32_at(&payload, at));
                let next_gap = runs
                    .get(i + 1)
                    .map(|(next, _)| *next as isize - (start + units * 2) as isize);
                println!(
                    "      +{start:<6} units {units:<4} stated {:<8} gap-to-next {:<8} {:?}",
                    stated.map(|v| v.to_string()).unwrap_or_default(),
                    next_gap.map(|v| v.to_string()).unwrap_or_default(),
                    read_utf16(&payload, *start, *units).unwrap_or_default()
                );
            }
        }
    }

    println!("\n=== summary ===");
    println!("text runs: {runs_total}, length-prefixed: {prefixed}");
    println!("stride matches 30 + 2n: {stride_holds}/{stride_checked}");
}
