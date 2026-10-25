//! Hexdump the JStyleSimpleLine record whose style_id is 28 (Hydraulic)
//! and look at every reference slot — where does the ⊢ pattern live?

use std::io::Read;

const STYLE_ID_OFFSET: usize = 14;

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    let s: [u8; 4] = b.get(o..o + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(s))
}
fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    let s: [u8; 2] = b.get(o..o + 2)?.try_into().ok()?;
    Some(u16::from_le_bytes(s))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid";
    let mut cfb = cfb::CompoundFile::open(std::fs::File::open(path)?)?;
    let mut data = Vec::new();
    for entry in cfb.walk() {
        eprintln!("stream {}", entry.path().display());
    }
    cfb.open_stream("/StyleCluster")?.read_to_end(&mut data)?;

    // pass 1: style_id -> (type_code, payload-offset)
    let mut index_of = std::collections::BTreeMap::<u32, (u16, usize, u32)>::new();
    let mut at = 8usize;
    while let (Some(tw), Some(btf)) = (u16_at(&data, at), u32_at(&data, at + 2)) {
        if btf == 0 || at + 6 + btf as usize > data.len() {
            break;
        }
        let payload = &data[at + 6..at + 6 + btf as usize];
        if let Some(sid) = u32_at(payload, STYLE_ID_OFFSET) {
            index_of.insert(sid, (tw & 0x3FFF, at + 6, btf));
        }
        at += 6 + btf as usize;
    }

    for target in [28u32, 53, 27, 52, 26, 25, 5, 56] {
        let Some((tc, poff, btf)) = index_of.get(&target).copied() else {
            continue;
        };
        let payload = &data[poff..poff + btf as usize];
        eprintln!("=== style_id {target} type=0x{tc:04X} btf={btf} payload@{poff}");
        for (i, ch) in payload.chunks(16).enumerate() {
            eprint!("  +{:02X}:", i * 16);
            for b in ch {
                eprint!(" {b:02X}");
            }
            eprintln!();
        }
        for off in (0..payload.len().saturating_sub(4)).step_by(4) {
            if let Some(v) = u32_at(payload, off) {
                if let Some((t, _, _)) = index_of.get(&v) {
                    eprintln!("    +{off:02X} -> style_id {v} type=0x{t:04X}");
                }
            }
        }
    }
    Ok(())
}
