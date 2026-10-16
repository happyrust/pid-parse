//! Locate the title-block field replacement values (`∽`, `1版`, …) inside
//! the .pid compound file: which stream hosts them, at what offset, and
//! what the bytes around each hit look like.
//!
//! Read-only probe; no parser change.

use cfb::CompoundFile;
use std::io::Read;

/// UTF-16LE unit patterns to hunt.
const NEEDLES: &[(&str, &[u16])] = &[
    ("∽ U+223D", &[0x223D]),
    ("版 U+7248", &[0x7248]),
    ("比例", &[0x6BD4, 0x4F8B]),
    ("施工图", &[0x65BD, 0x5DE5, 0x56FE]),
    ("阶段", &[0x9636, 0x6BB5]),
];

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for arg in std::env::args().skip(1) {
        let mut cfb = CompoundFile::open(std::fs::File::open(&arg)?)?;
        let streams: Vec<_> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|e| e.path().to_path_buf())
            .collect();
        println!("{arg}: {} streams", streams.len());
        for sp in streams {
            let mut data = Vec::new();
            cfb.open_stream(&sp)?.read_to_end(&mut data)?;
            let mut hits: Vec<(usize, &str)> = Vec::new();
            for (label, units) in NEEDLES {
                let byte_pat: Vec<u8> = units.iter().flat_map(|u| u.to_le_bytes()).collect();
                let stop = data.len().saturating_sub(byte_pat.len());
                for i in (0..stop).step_by(2) {
                    if data[i..i + byte_pat.len()] == byte_pat[..] {
                        hits.push((i, label));
                    }
                }
            }
            if hits.is_empty() {
                continue;
            }
            hits.sort();
            println!(
                "  {} ({} bytes): {} hit(s)",
                sp.display(),
                data.len(),
                hits.len()
            );
            for (off, label) in &hits {
                // Widest printable run of UTF-16 units around the hit.
                let lo = off.saturating_sub(80) & !1;
                let hi = (off + 120).min(data.len() - 1) & !1;
                let mut s = String::new();
                let mut i = lo;
                while i + 1 < hi {
                    let u = u16_at(&data, i);
                    s.push(if (0x20..0xD800).contains(&u) {
                        char::from_u32(u as u32).unwrap_or('�')
                    } else {
                        '·'
                    });
                    i += 2;
                }
                println!("    @{off:06x} {label}: …{s}…");
            }
        }
    }
    Ok(())
}
