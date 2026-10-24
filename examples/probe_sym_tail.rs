//! Dump every 0x004D payload in one .sym: header fields, the decoded text,
//! and the trailing bytes after chars+insertion — looking for the style id
//! (payload +14) and a sheet-like anchor byte at the tail.
use std::path::Path;

use pid_parse::PidParser;

const CLUSTER_MAGIC: u32 = 0x6C90_F544;
const CHAIN_START: usize = 8;
const PSM_ENVELOPE_LEN: usize = 6;
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for arg in std::env::args().skip(1) {
        println!("=== {arg}");
        let pkg = PidParser::new().parse_package(Path::new(&arg))?;
        for (path, raw) in &pkg.streams {
            let is_sheet = path
                .rsplit('/')
                .next()
                .is_some_and(|n| n.starts_with("Sheet"));
            if !is_sheet || raw.data.len() <= CHAIN_START {
                continue;
            }
            let bytes = &raw.data;
            if u32_le(bytes, 0) != Some(CLUSTER_MAGIC) {
                continue;
            }
            let mut at = CHAIN_START;
            while at + PSM_ENVELOPE_LEN <= bytes.len() {
                let (Some(ty), Some(btf)) = (u16_le(bytes, at), u32_le(bytes, at + 2)) else {
                    break;
                };
                if ty == 0 {
                    break;
                }
                let body = at + PSM_ENVELOPE_LEN;
                let end = body.saturating_add(btf as usize);
                if end > bytes.len() {
                    break;
                }
                if ty & 0x0FFF == TYPE_TEXTBOX {
                    let p = &bytes[body..end];
                    println!("  {path}@{at} len={}", p.len());
                    println!(
                        "    oid={:08x} aux={:08x} sub={:#06x} style_idx={}",
                        u32_le(p, 0).unwrap_or(0),
                        u32_le(p, 8).unwrap_or(0),
                        u16_le(p, 12).unwrap_or(0),
                        u32_le(p, 14).unwrap_or(0)
                    );
                    // find char count at +22 or +30
                    for &co in &[22usize, 30usize] {
                        if let Some(n) = u16_le(p, co) {
                            let s = co + 2;
                            let e = s + n as usize * 2;
                            if e + 36 == p.len() {
                                let chars: Vec<u16> = (0..n as usize)
                                    .filter_map(|i| u16_le(p, s + i * 2))
                                    .collect();
                                let text = String::from_utf16_lossy(&chars);
                                println!("    count@{co}={n} text={text:?}");
                                println!(
                                    "    ins=({:.5},{:.5})",
                                    f64_le(p, e).unwrap_or(0.),
                                    f64_le(p, e + 8).unwrap_or(0.)
                                );
                                print!("    tail+16..36:");
                                for b in &p[e + 16..e + 36] {
                                    print!(" {b:02x}");
                                }
                                println!();
                            }
                        }
                    }
                    println!(
                        "    hex: {}",
                        p.iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<Vec<_>>()
                            .join("")
                    );
                }
                at = end;
            }
        }
    }
    Ok(())
}
