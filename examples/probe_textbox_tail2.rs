//! Locate the igTextBox 4-byte trailer properly: it sits at
//! `payload[22 + body_len + 32 .. +36]`, after insertion+direction and before
//! the optional 32/40-byte extras. Census trailer values vs sub-type.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;

fn u16at(p: &[u8], at: usize) -> Option<u16> {
    p.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn body_len(payload: &[u8], sub: u16) -> Option<usize> {
    match sub {
        1 => {
            let c = u16at(payload, 22)? as usize;
            Some(2 + 2 * c)
        }
        2 => {
            let c = u16at(payload, 30)? as usize;
            Some(10 + 2 * c)
        }
        3 => {
            let a = u16at(payload, 22)? as usize;
            let b = u16at(payload, 24)? as usize;
            let c = u16at(payload, 26)? as usize;
            Some(6 + 2 * c + 8 * (a + b))
        }
        _ => None,
    }
}

fn main() {
    let files = [
        r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A1.igr",
    ];
    // key: (trailer u32 BE-read as hex string, extra_len)
    let mut census: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    for file in files {
        let f = std::fs::File::open(file).unwrap();
        let mut cfb = cfb::CompoundFile::open(f).unwrap();
        let name = PathBuf::from(file)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let sheets: Vec<PathBuf> = cfb
            .walk()
            .filter(|e| e.is_stream() && e.name().starts_with("Sheet"))
            .map(|e| e.path().to_path_buf())
            .collect();
        for path in sheets {
            let mut data = Vec::new();
            cfb.open_stream(&path)
                .unwrap()
                .read_to_end(&mut data)
                .unwrap();
            for r in pid_parse::parsers::sheet_records::decode_igtextboxes(&data) {
                let payload = &data[r.byte_range.start + 6..r.byte_range.end];
                let Some(bl) = body_len(payload, r.text_sub_type) else {
                    continue;
                };
                let tail_start = 22 + bl;
                let trailer = &payload[tail_start + 32..tail_start + 36];
                let extra = payload.len() - (tail_start + 36);
                let key = format!(
                    "trailer={} extra={extra:02} sub={}",
                    trailer
                        .iter()
                        .map(|b| format!("{b:02X}"))
                        .collect::<String>(),
                    r.text_sub_type
                );
                let e = census.entry(key).or_insert((0, Vec::new()));
                e.0 += 1;
                if e.1.len() < 5 {
                    e.1.push(format!("{name}:oid{}:{:?}", r.oid, r.text));
                }
            }
        }
    }
    for (k, (n, ex)) in &census {
        println!("{k}  count={n}");
        for e in ex {
            println!("    {e}");
        }
    }
}
