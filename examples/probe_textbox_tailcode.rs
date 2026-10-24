//! Census of the igTextBox 4-byte trailer (after the direction pair) across
//! every real drawing and the border template: the last byte reads as a
//! per-record anchor code.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;

fn main() {
    let files = [
        r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A1.igr",
        r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A0.igr",
    ];
    let mut census: BTreeMap<[u8; 4], Vec<String>> = BTreeMap::new();
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
                // The placement tail ends the body: last 4 bytes before
                // byte_range.end minus nothing — the trailer is the final
                // 4 bytes of the *payload*, which is byte_range minus the
                // 6-byte PSM header.
                let end = r.byte_range.end;
                let tail: [u8; 4] = data[end - 4..end].try_into().unwrap();
                census
                    .entry(tail)
                    .or_default()
                    .push(format!("{name}:{}:{:?}", r.oid, r.text));
            }
        }
    }
    for (tail, uses) in &census {
        println!("tail {:02X?} count={}", tail, uses.len());
        for u in uses.iter().take(8) {
            println!("    {u}");
        }
    }
}
