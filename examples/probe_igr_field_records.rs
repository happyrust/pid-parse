//! Map every PSM record inside the binary .igr's /Sheet6 stream and show
//! which record type carries the `1 版` field-cell bytes — the SmartFrame
//! field-substitution family the igTextBox dump misses.
//!
//! Read-only probe.

use cfb::CompoundFile;
use std::collections::BTreeMap;
use std::io::Read;

const PSM_HEADER_LEN: usize = 6;

fn psm_record_end(bytes: &[u8], off: usize) -> Option<usize> {
    let end = off.checked_add(PSM_HEADER_LEN)?;
    if end > bytes.len() {
        return None;
    }
    let ty = u16::from_le_bytes([bytes[off], bytes[off + 1]]) & 0x3FFF;
    if ty == 0 {
        return None;
    }
    let btf = u32::from_le_bytes([
        bytes[off + 2],
        bytes[off + 3],
        bytes[off + 4],
        bytes[off + 5],
    ]) as usize;
    let rec_end = end.checked_add(btf)?;
    (rec_end <= bytes.len()).then_some(rec_end)
}

fn walk_records(bytes: &[u8]) -> Vec<(usize, usize, u16)> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off + PSM_HEADER_LEN <= bytes.len() {
        if let Some(end) = psm_record_end(bytes, off) {
            if end == bytes.len() || psm_record_end(bytes, end).is_some() {
                out.push((
                    off,
                    end,
                    u16::from_le_bytes([bytes[off], bytes[off + 1]]) & 0x3FFF,
                ));
                off = end;
                continue;
            }
        }
        off += 1;
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for arg in std::env::args().skip(1) {
        let mut cfb = CompoundFile::open(std::fs::File::open(&arg)?)?;
        let mut data = Vec::new();
        cfb.open_stream("/Sheet6")?.read_to_end(&mut data)?;
        let recs = walk_records(&data);
        let mut tally: BTreeMap<u16, usize> = BTreeMap::new();
        for (_, _, ty) in &recs {
            *tally.entry(*ty).or_default() += 1;
        }
        println!("{arg}: {} records over {} types", recs.len(), tally.len());
        for (ty, n) in &tally {
            println!("  0x{ty:04X} x{n}");
        }
        // The `1 版` needle: u16 len 3, units '1',' ','版'.
        let needle: Vec<u8> = [0x03, 0x00, 0x31, 0x00, 0x20, 0x00, 0x48, 0x72]
            .into_iter()
            .collect();
        for i in 0..data.len().saturating_sub(needle.len()) {
            if data[i..i + needle.len()] == needle[..] {
                let owner = recs.iter().find(|(s, e, _)| i >= *s && i < *e);
                println!(
                    "needle @{i:#06x} inside record {:?}",
                    owner.map(|(s, e, t)| (*s, *e, format!("0x{t:04X}")))
                );
            }
        }
        // Dump the non-igTextBox text-ish records: type 0x00xx payloads that
        // contain any CJK unit.
        for (s, e, ty) in &recs {
            if *ty == 0x004D {
                continue;
            }
            let body = &data[*s + 6..*e];
            let mut has_cjk = false;
            let mut s16 = String::new();
            for j in (0..body.len().saturating_sub(1)).step_by(2) {
                let u = u16::from_le_bytes([body[j], body[j + 1]]);
                if (0x4E00..0x9FFF).contains(&u) {
                    has_cjk = true;
                }
                s16.push(if (0x20..0xD800).contains(&u) {
                    char::from_u32(u as u32).unwrap_or('�')
                } else {
                    '·'
                });
            }
            if has_cjk {
                println!("rec @{s:#06x} type 0x{ty:04X} len {}: {s16}", e - s);
            }
        }
    }
    Ok(())
}
