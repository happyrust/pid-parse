//! Count igSymbol2d placements with uniform_scale != 1 across all 5 drawings.

use pid_parse::PidParser;

const FILES: &[(&str, &str)] = &[
    (
        "W",
        r"D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid",
    ),
    (
        "A",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器A工艺及自控流程图.pid",
    ),
    (
        "B",
        r"D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
    ),
    (
        "LT",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（一） 低温分离器A工艺及自控流程图.pid",
    ),
    (
        "HE",
        r"D:\work\cad\PID自研软件验证\集注站 烃水露点控制单元（二） 换热器CD及外输阀组工艺及自控流程图.pid",
    ),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for (tag, path) in FILES {
        let doc = PidParser::new().parse_file(path)?;
        let mut total = 0usize;
        let mut scaled = 0usize;
        for sheet in &doc.sheet_streams {
            if let Some(g) = &sheet.geometry {
                for r in &g.decoded_igsymbols {
                    total += 1;
                    if (r.uniform_scale - 1.0).abs() > 1e-9 {
                        scaled += 1;
                        println!("  {tag} oid={} scale={:.4}", r.oid, r.uniform_scale);
                    }
                }
            }
        }
        println!("{tag}: {total} placements, {scaled} scaled");
    }
    Ok(())
}
