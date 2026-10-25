//! Raw-dump the three Christmas Tree 02 igSymbol2d records of W.pid and
//! hunt for a ~1.18 scale factor anywhere in the payload.

use cfb::CompoundFile;
use std::io::Read;

const PSM_HEADER_LEN: usize = 6;

fn read_f64(p: &[u8], at: usize) -> Option<f64> {
    let c = p.get(at..at + 8)?;
    Some(f64::from_le_bytes(c.try_into().ok()?))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cfb = CompoundFile::open(std::fs::File::open(
        r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
    )?)?;
    let mut bytes = Vec::new();
    cfb.open_stream("/Sheet6")?.read_to_end(&mut bytes)?;
    let mut off = 0usize;
    let mut n_tree = 0usize;
    while off + PSM_HEADER_LEN + 16 <= bytes.len() {
        let type_word = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
        if type_word & 0x3FFF != 0x00CE {
            off += 1;
            continue;
        }
        let btf = u32::from_le_bytes(bytes[off + 2..off + 6].try_into()?) as usize;
        if !(113..=400).contains(&btf) || off + PSM_HEADER_LEN + btf > bytes.len() {
            off += 1;
            continue;
        }
        let p = &bytes[off + PSM_HEADER_LEN..off + PSM_HEADER_LEN + btf];
        let oid = u32::from_le_bytes(p[0..4].try_into()?);
        if ![36561, 36856, 37141].contains(&oid) {
            off += PSM_HEADER_LEN + btf;
            continue;
        }
        n_tree += 1;
        println!("=== oid={oid} @0x{off:06x} btf={btf}");
        for start in (0..p.len()).step_by(16) {
            let end = (start + 16).min(p.len());
            let hex: Vec<String> = p[start..end].iter().map(|b| format!("{b:02X}")).collect();
            println!("  +{start:03}: {}", hex.join(" "));
        }
        // every f64 window in [0.5, 2.5]
        println!("  -- f64 in 0.5..2.5 --");
        for at in 0..p.len().saturating_sub(8) {
            if let Some(v) = read_f64(p, at) {
                if (0.5..2.5).contains(&v) {
                    println!("    +{at:3}: {v:.6}");
                }
            }
        }
        off += PSM_HEADER_LEN + btf;
    }
    println!("tree records dumped: {n_tree}");
    Ok(())
}
