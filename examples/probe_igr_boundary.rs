//! Raw-dump the 0x0013 records inside CPE-SQ-A0.igr to see which strict-decoder
//! rule they break.

use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A0.igr";
    let mut cfb = cfb::CompoundFile::open(std::fs::File::open(path)?)?;
    let names: Vec<String> = cfb
        .walk()
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect();
    for name in &names {
        if !name.starts_with("/Sheet") {
            continue;
        }
        let mut data = Vec::new();
        cfb.open_stream(name)?.read_to_end(&mut data)?;
        eprintln!("== {name} len={}", data.len());
        let mut off = 0usize;
        while off + 6 <= data.len() {
            let tw = u16::from_le_bytes([data[off], data[off + 1]]);
            if tw & 0x3FFF != 0x0013 {
                off += 1;
                continue;
            }
            let btf = u32::from_le_bytes(data[off + 2..off + 6].try_into()?) as usize;
            if !(20..=4000).contains(&btf) || off + 6 + btf > data.len() {
                off += 1;
                continue;
            }
            let p = &data[off + 6..off + 6 + btf];
            if p.len() < 30 {
                off += 1;
                continue;
            }
            let u32at = |i: usize| u32::from_le_bytes(p[i..i + 4].try_into().unwrap());
            let u16at = |i: usize| u16::from_le_bytes(p[i..i + 2].try_into().unwrap());
            let n = u32at(22) as usize;
            println!("@{off:#x} btf={btf} oid={:#x} parent={:#x} layer={:#x} subtype={:#06x} idx={} u32@18={} segcnt={} expbtf={}",
                u32at(0), u32at(4), u32at(8), u16at(12), u32at(14), u32at(18), n, 49+41*n);
            print!("   bytes:");
            for b in p.iter().take(80) {
                print!(" {b:02x}");
            }
            println!();
            off += 6 + btf;
        }
    }
    Ok(())
}
