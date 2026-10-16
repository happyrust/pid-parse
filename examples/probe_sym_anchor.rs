//! Corpus sweep: distribution of a .sym text record's last 4 bytes — is the
//! tail byte an anchor code like the sheet igTextBox's, or a constant?
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::PidParser;

const CLUSTER_MAGIC: u32 = 0x6C90_F544;
const CHAIN_START: usize = 8;
const TYPE_TEXTBOX: u16 = 0x004D;

fn u16_le(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2)
        .and_then(|s| <[u8; 2]>::try_from(s).ok())
        .map(u16::from_le_bytes)
}
fn u32_le(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map(u32::from_le_bytes)
}
fn f64_le(b: &[u8], at: usize) -> Option<f64> {
    b.get(at..at + 8)
        .and_then(|s| <[u8; 8]>::try_from(s).ok())
        .map(f64::from_le_bytes)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("sym")) {
            out.push(p);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::args().nth(1).expect("symbols root"));
    let mut syms = Vec::new();
    collect(&root, &mut syms);
    let mut tails: BTreeMap<u32, usize> = BTreeMap::new();
    let mut last_byte: BTreeMap<u8, usize> = BTreeMap::new();
    let mut f64_fields: BTreeMap<u64, usize> = BTreeMap::new();
    let mut styles: BTreeMap<u32, usize> = BTreeMap::new();
    let mut records = 0usize;
    let mut sample = 0usize;
    for sym in &syms {
        let Ok(pkg) = PidParser::new().parse_package(sym) else {
            continue;
        };
        for (path, raw) in &pkg.streams {
            if !path
                .rsplit('/')
                .next()
                .is_some_and(|n| n.starts_with("Sheet"))
            {
                continue;
            }
            let b = &raw.data;
            if b.len() <= CHAIN_START || u32_le(b, 0) != Some(CLUSTER_MAGIC) {
                continue;
            }
            let mut at = CHAIN_START;
            while at + 6 <= b.len() {
                let (Some(ty), Some(n)) = (u16_le(b, at), u32_le(b, at + 2)) else {
                    break;
                };
                if ty == 0 {
                    break;
                }
                let body = at + 6;
                let end = body + n as usize;
                if end > b.len() {
                    break;
                }
                if ty & 0x0FFF == TYPE_TEXTBOX {
                    let p = &b[body..end];
                    records += 1;
                    let t4 = u32_le(p, p.len() - 4).unwrap_or(0);
                    *tails.entry(t4).or_default() += 1;
                    *last_byte.entry(*p.last().unwrap()).or_default() += 1;
                    *styles.entry(u32_le(p, 14).unwrap_or(0)).or_default() += 1;
                    // the f64 after the insertion point (tail+16)
                    let bits = f64_le(p, p.len() - 20).unwrap_or(0.0).to_bits();
                    *f64_fields.entry(bits).or_default() += 1;
                    if sample < 12 && p.len() > 60 {
                        sample += 1;
                        let tail: Vec<String> = p[p.len() - 36..]
                            .iter()
                            .map(|x| format!("{x:02x}"))
                            .collect();
                        println!(
                            "{} len={} tail={}",
                            sym.file_name().unwrap().to_string_lossy(),
                            p.len(),
                            tail.join("")
                        );
                    }
                }
                at = end;
            }
        }
    }
    println!("records={records}");
    println!(
        "style_idx distribution: {:?}",
        styles.iter().take(15).collect::<Vec<_>>()
    );
    println!("last-4-bytes:");
    for (k, c) in tails.iter().take(15) {
        println!("  {k:#010x} x{c}")
    }
    println!("last byte:");
    for (k, c) in last_byte.iter() {
        println!("  {k:#04x} x{c}")
    }
    println!("f64 at len-20 (after insertion):");
    for (k, c) in f64_fields.iter().take(8) {
        println!("  {} x{c}", f64::from_bits(*k))
    }
    Ok(())
}
