//! Decode the 0x0034 pattern record (style_id 26) field by field —
//! u16/u32/f64 scan looking for the tick lead-in offset and element table.

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let mut cfb = cfb::CompoundFile::open(std::fs::File::open(path)?)?;
    let mut data = Vec::new();
    cfb.open_stream("/StyleCluster")?.read_to_end(&mut data)?;

    let mut at = 8usize;
    while let (Some(tw), Some(btf)) = (u16_at(&data, at), u32_at(&data, at + 2)) {
        if btf == 0 || at + 6 + btf as usize > data.len() {
            break;
        }
        let payload = &data[at + 6..at + 6 + btf as usize];
        let tc = tw & 0x3FFF;
        let sid = u32_at(payload, 14).unwrap_or(0);
        if tc == 0x0034 {
            eprintln!("=== 0x0034 style_id={sid} btf={btf}");
            // u16/u32 scan for small ints
            for o in (0..payload.len()).step_by(2) {
                if let Some(v) = u16_at(payload, o) {
                    if (1..=16).contains(&v) {
                        eprintln!("  +{o:02X} u16={v}");
                    }
                }
            }
            for o in 0..payload.len().saturating_sub(4) {
                if let Some(v) = u32_at(payload, o) {
                    if (17..=300).contains(&v) {
                        eprintln!("  +{o:02X} u32={v} (ref?)");
                    }
                }
            }
            // f64 scan: sane metric values
            for o in (0..payload.len().saturating_sub(8)).step_by(4) {
                if let Some(v) = f64_at(payload, o) {
                    if v.is_finite() && v.abs() > 1e-6 && v.abs() < 1.0 {
                        eprintln!("  +{o:02X} f64={v:.6} = {:.3}mm", v * 1000.0);
                    }
                }
            }
        }
        at += 6 + btf as usize;
    }
    Ok(())
}
