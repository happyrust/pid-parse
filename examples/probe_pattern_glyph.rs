//! Read group 0x201D's member strokes — the ⊢ glyph the hydraulic
//! pattern embeds, and confirm the complex-dash chain:
//! SimpleLine(28) +54 -> DashType(27) +66 -> Pattern(26,0x0034) +4C
//! -> PointSymbol(25) +26 -> Group(0x201D).

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
        let oid = u32_at(payload, 0).unwrap_or(0);
        if tc == 0x007B && oid == 0x201D {
            eprintln!("group 0x201D btf={btf}");
            let count = u16_at(payload, 16).unwrap_or(0);
            eprintln!("members={count}");
            for i in 0..count as usize {
                let m = u32_at(payload, 28 + 14 * i).unwrap_or(0);
                eprintln!("  member oid={m:#06X}");
            }
        }
        if tc == 0x0018 {
            // igLine2d — parent group oid at +4; dump the ones in 0x201D
            if u32_at(payload, 4) == Some(0x201D) {
                let (x0, y0, x1, y1) = (
                    f64_at(payload, 18).unwrap_or(0.0),
                    f64_at(payload, 26).unwrap_or(0.0),
                    f64_at(payload, 34).unwrap_or(0.0),
                    f64_at(payload, 42).unwrap_or(0.0),
                );
                eprintln!(
                    "  line oid={oid:#06X} ({:.4},{:.4})-({:.4},{:.4})m = ({:.2},{:.2})-({:.2},{:.2})mm",
                    x0, y0, x1, y1,
                    x0 * 1000.0, y0 * 1000.0, x1 * 1000.0, y1 * 1000.0
                );
            }
        }
        at += 6 + btf as usize;
    }
    Ok(())
}
