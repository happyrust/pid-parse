use cfb::CompoundFile;
use pid_parse::parsers::sheet_records::{
    decode_iglines, decode_iglinestrings, decode_igpoints, decode_igtextboxes,
};
use pid_parse::style_link::{stylecluster_path_for_sheet, DocumentStyleTable};
use std::io::Read;
use std::path::Path;

fn read_stream(cfb: &mut CompoundFile<std::fs::File>, path: &str) -> Option<Vec<u8>> {
    let mut s = cfb.open_stream(path).ok()?;
    let mut d = Vec::new();
    s.read_to_end(&mut d).ok()?;
    Some(d)
}

fn main() {
    for path in [
        r"D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A1.igr",
        r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid",
    ] {
        let file = std::fs::File::open(path).unwrap();
        let mut cfb = CompoundFile::open(file).unwrap();
        let sheet_paths: Vec<String> = cfb
            .walk()
            .filter(cfb::Entry::is_stream)
            .map(|e| e.path().to_string_lossy().into_owned())
            .filter(|p| p.rsplit('/').next().unwrap_or("").starts_with("Sheet"))
            .collect();
        let (mut white, mut black, mut other, mut unres) = (0, 0, 0, 0);
        for sheet_path in &sheet_paths {
            let Some(sheet) = read_stream(&mut cfb, sheet_path) else {
                continue;
            };
            let table = read_stream(&mut cfb, &stylecluster_path_for_sheet(sheet_path))
                .map_or_else(DocumentStyleTable::default, |b| {
                    DocumentStyleTable::from_stylecluster_bytes(&b)
                });
            let mut idxs = Vec::new();
            for r in decode_iglinestrings(&sheet) {
                idxs.push(("LS", r.index));
            }
            for r in decode_iglines(&sheet) {
                idxs.push(("L", r.index));
            }
            for r in decode_igpoints(&sheet) {
                idxs.push(("P", r.index));
            }
            for r in decode_igtextboxes(&sheet) {
                idxs.push(("T", r.index));
            }
            for (k, idx) in idxs {
                match table.resolve_line_style(idx) {
                    Some(rs) => match rs.symbology.colour {
                        0xFFFFFF => {
                            white += 1;
                            if white <= 3 {
                                println!("  white {k} idx={idx} w={:.2}", rs.symbology.width_mm());
                            }
                        }
                        0x000000 => black += 1,
                        c => other += 1,
                    },
                    None => unres += 1,
                }
            }
        }
        let name = Path::new(path).file_name().unwrap().to_string_lossy();
        println!("{name}: white={white} black={black} other={other} unres={unres}");
    }
}
