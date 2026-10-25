//! Corpus-wide check: every 0x002F dash record whose payload is longer than
//! 50+8N (complex-dash candidates) and every 0x0034 record's reference field
//! at +74, across all five fixture .pid files.

use std::io::Read;

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    let s: [u8; 4] = b.get(o..o + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(s))
}
fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    let s: [u8; 2] = b.get(o..o + 2)?.try_into().ok()?;
    Some(u16::from_le_bytes(s))
}
fn f64_at(b: &[u8], o: usize) -> Option<f64> {
    let s: [u8; 8] = b.get(o..o + 8)?.try_into().ok()?;
    Some(f64::from_le_bytes(s))
}

fn scan(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfb = cfb::CompoundFile::open(std::fs::File::open(path)?)?;
    let mut data = Vec::new();
    cfb.open_stream("/StyleCluster")?.read_to_end(&mut data)?;
    let short = std::path::Path::new(path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    eprintln!("===== {short}");
    let mut at = 8usize;
    while let (Some(tw), Some(btf)) = (u16_at(&data, at), u32_at(&data, at + 2)) {
        if btf == 0 || at + 6 + btf as usize > data.len() {
            break;
        }
        let payload = &data[at + 6..at + 6 + btf as usize];
        let tc = tw & 0x3FFF;
        match tc {
            0x002F => {
                let count = u16_at(payload, 48).unwrap_or(0);
                let want = 50 + 8 * count as usize;
                if payload.len() != want {
                    let sid = u32_at(payload, 14).unwrap_or(0);
                    eprintln!(
                        "  dash id={sid} btf={btf} count={count} len={} want={want} tail={:+4}B",
                        payload.len(),
                        payload.len() as isize - want as isize
                    );
                    for i in 0..count as usize {
                        if let Some(v) = f64_at(payload, 50 + 8 * i) {
                            eprintln!("    seg[{i}]={v:?}");
                        }
                    }
                    if let Some(r) = u32_at(payload, 50 + 8 * count as usize) {
                        eprintln!("    tail ref={r}");
                    }
                }
            }
            0x0034 => {
                let sid = u32_at(payload, 14).unwrap_or(0);
                let oid = u32_at(payload, 0).unwrap_or(0);
                eprintln!("  0x0034 id={sid} oid={oid:#06x} btf={btf}");
                for o in (16..payload.len().saturating_sub(4)).step_by(2) {
                    if let Some(v) = u32_at(payload, o) {
                        if (1..=400).contains(&v) {
                            eprintln!("    +{o:02X} u32={v}");
                        }
                    }
                }
            }
            _ => {}
        }
        at += 6 + btf as usize;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in [
        r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\A井场 注采阀组工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\B井场 注采阀组工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\LT井场 注采阀组工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\HE井场 注采阀组工艺及自控流程图.pid",
    ] {
        let _ = scan(path);
    }
    Ok(())
}
