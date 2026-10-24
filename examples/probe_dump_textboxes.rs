//! Dump every igTextBox of the five real drawings + the A1 template to JSON:
//! oid, text, insertion, rotation, trailer code, sub-type, index.

use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

fn u16at(p: &[u8], at: usize) -> Option<u16> {
    p.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn body_len(payload: &[u8], sub: u16) -> Option<usize> {
    match sub {
        1 => Some(2 + 2 * u16at(payload, 22)? as usize),
        2 => Some(10 + 2 * u16at(payload, 30)? as usize),
        3 => {
            let a = u16at(payload, 22)? as usize;
            let b = u16at(payload, 24)? as usize;
            Some(6 + 2 * u16at(payload, 26)? as usize + 8 * (a + b))
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
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "textboxes.json".into());
    let mut w = std::io::BufWriter::new(std::fs::File::create(&out).unwrap());
    writeln!(w, "[").unwrap();
    let mut first = true;
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
                let trailer = payload[22 + bl + 35];
                let line = format!(
                    "{{\"file\":\"{name}\",\"sheet\":\"{}\",\"oid\":{},\"idx\":{},\"sub\":{},\"code\":{},\"x\":{:.6},\"y\":{:.6},\"rot\":{:.4},\"text\":{}}}",
                    path.file_name().unwrap().to_string_lossy(),
                    r.oid,
                    r.index,
                    r.text_sub_type,
                    trailer,
                    r.trailing_double_1,
                    r.trailing_double_2,
                    r.rotation_rad,
                    serde_json::to_string(&r.text).unwrap_or("\"?\"".into())
                );
                if !first {
                    writeln!(w, ",").unwrap();
                }
                first = false;
                write!(w, "{line}").unwrap();
            }
        }
    }
    writeln!(w, "\n]").unwrap();
    eprintln!("wrote {out}");
}
