//! For the five real drawings: find igTextBox records whose text contains
//! "PGGAS", print their sheet_layer_ref, then print each sheet's layer table
//! (oid, name, displayed) so we can tell whether the spec strings sit on a
//! hidden layer.

use std::io::Read;
use std::path::PathBuf;

fn main() {
    let files = [
        r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid",
    ];
    for file in files {
        let f = std::fs::File::open(file).unwrap();
        let mut cfb = cfb::CompoundFile::open(f).unwrap();
        let name = PathBuf::from(file)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        println!("=== {name}");
        let doc = pid_parse::PidParser::new()
            .parse_file(std::path::Path::new(file))
            .unwrap();
        for (site, layers) in &doc.sheet_layers {
            println!("  site {site}:");
            for l in layers {
                println!(
                    "    oid={:>5} num={:>3} displayed={:?} name={:?}",
                    l.oid, l.layer_number, l.displayed, l.name
                );
            }
        }
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
                if r.text.contains("PGGAS") || r.text.contains("<<") {
                    println!(
                        "  HIT sheet={} oid={} layer_ref={} text={:?}",
                        path.file_name().unwrap().to_string_lossy(),
                        r.oid,
                        r.sheet_layer_ref,
                        r.text
                    );
                }
            }
        }
    }
}
